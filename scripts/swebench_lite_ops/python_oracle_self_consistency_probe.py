#!/usr/bin/env python3
"""Run the INV-035 / #760 SWE-bench oracle self-consistency probe.

The probe samples one mutation row per SWE-bench task, runs the same patch
through the Docker oracle repeatedly, and writes per-replica reports plus
pairwise agreement summaries. Completion still requires manual SoT readback;
this command only materializes the artifacts to inspect.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_INDEX = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/inv-035-oracle-noise-floor-fsv")
DEFAULT_PYTHON = Path("/zfs/hot/contextgraph/venv/bin/python3")
DEFAULT_HARNESS_LOCK = Path("/zfs/archive/contextgraph/state/locks/swebench-harness.lock")
DEFAULT_CATEGORIES = (
    "known_good",
    "compile_error",
    "wrong_file",
    "off_by_one",
    "subtle_flip",
    "swap_variable",
    "over_engineer",
)


class ProbeError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise ProbeError(f"ORACLE_SELF_CONSISTENCY_READBACK_MISMATCH: {path}")


def write_json_checked(path: Path, value: Any) -> None:
    write_text_checked(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_jsonl_checked(path: Path, rows: list[dict[str, Any]]) -> None:
    write_text_checked(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def load_index(path: Path) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if not path.is_file():
        raise ProbeError(f"ORACLE_SELF_CONSISTENCY_INDEX_MISSING: {path}")
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    entries = payload.get("entries")
    if not isinstance(entries, list) or not entries:
        raise ProbeError("ORACLE_SELF_CONSISTENCY_INDEX_HAS_NO_ENTRIES")
    return payload, entries


def require_aiwonder_path(path: Path, label: str) -> Path:
    text = str(path)
    if not (text.startswith("/zfs/archive/contextgraph/") or text.startswith("/zfs/hot/contextgraph/")):
        raise ProbeError(f"ORACLE_SELF_CONSISTENCY_NOT_AIWONDER_PATH: {label}={path}")
    return path


def stable_score(seed: str, *parts: str) -> str:
    return hashlib.sha256(("|".join((seed, *parts))).encode("utf-8")).hexdigest()


def repo_quotas(task_counts: Counter[str], sample_size: int) -> dict[str, int]:
    repos = sorted(task_counts)
    if sample_size < len(repos):
        raise ProbeError(
            f"ORACLE_SELF_CONSISTENCY_SAMPLE_TOO_SMALL_FOR_REPOS: sample={sample_size} repos={len(repos)}"
        )
    quotas = {repo: 1 for repo in repos}
    remaining = sample_size - len(repos)
    total = sum(task_counts.values())
    raw = {repo: remaining * (task_counts[repo] / total) for repo in repos}
    for repo, value in raw.items():
        add = int(value)
        quotas[repo] += add
        remaining -= add
    for repo, _ in sorted(raw.items(), key=lambda item: (-(item[1] - int(item[1])), item[0])):
        if remaining <= 0:
            break
        quotas[repo] += 1
        remaining -= 1
    return quotas


def select_rows(entries: list[dict[str, Any]], sample_size: int, seed: str) -> list[dict[str, Any]]:
    task_entries: dict[str, dict[str, Any]] = {}
    by_task_category: dict[tuple[str, str], dict[str, Any]] = {}
    for row in entries:
        task_id = str(row.get("task_id") or "")
        repo = str(row.get("repo") or "")
        category = str(row.get("category") or "")
        if not task_id or not repo or not category:
            continue
        task_entries.setdefault(task_id, {"task_id": task_id, "repo": repo})
        by_task_category[(task_id, category)] = row

    repo_to_tasks: dict[str, list[str]] = defaultdict(list)
    for task in task_entries.values():
        repo_to_tasks[task["repo"]].append(task["task_id"])
    task_counts = Counter({repo: len(tasks) for repo, tasks in repo_to_tasks.items()})
    quotas = repo_quotas(task_counts, sample_size)

    selected_tasks: list[tuple[str, str]] = []
    for repo, quota in sorted(quotas.items()):
        ranked = sorted(repo_to_tasks[repo], key=lambda task_id: stable_score(seed, repo, task_id))
        selected_tasks.extend((repo, task_id) for task_id in ranked[:quota])
    selected_tasks = sorted(selected_tasks, key=lambda item: stable_score(seed, item[0], item[1], "selected"))

    selected: list[dict[str, Any]] = []
    for index, (repo, task_id) in enumerate(selected_tasks):
        category = DEFAULT_CATEGORIES[index % len(DEFAULT_CATEGORIES)]
        row = by_task_category.get((task_id, category))
        if row is None:
            raise ProbeError(f"ORACLE_SELF_CONSISTENCY_SELECTED_ROW_MISSING: {task_id} {category}")
        selected.append(dict(row))
    if len(selected) != sample_size:
        raise ProbeError(f"ORACLE_SELF_CONSISTENCY_SELECTION_COUNT_MISMATCH: {len(selected)} != {sample_size}")
    if len({row["task_id"] for row in selected}) != sample_size:
        raise ProbeError("ORACLE_SELF_CONSISTENCY_SELECTION_DUPLICATE_TASK")
    return selected


def materialize_predictions(
    *,
    selected: list[dict[str, Any]],
    corpus_root: Path,
    output_root: Path,
    replica_index: int,
    model_name: str,
) -> tuple[Path, list[str]]:
    predictions: list[dict[str, Any]] = []
    for row in selected:
        patch_path = corpus_root / str(row["patch_path"])
        if not patch_path.is_file():
            raise ProbeError(f"ORACLE_SELF_CONSISTENCY_PATCH_MISSING: {patch_path}")
        expected_sha = str(row.get("patch_sha256") or "")
        actual_sha = sha256_file(patch_path)
        if expected_sha != actual_sha:
            raise ProbeError(
                f"ORACLE_SELF_CONSISTENCY_PATCH_SHA_MISMATCH: {row['task_id']} {row['category']} {expected_sha} {actual_sha}"
            )
        predictions.append(
            {
                "instance_id": row["task_id"],
                "model_name_or_path": model_name,
                "model_patch": patch_path.read_text(encoding="utf-8"),
            }
        )
    path = output_root / "predictions" / f"replica-{replica_index:02d}.jsonl"
    write_jsonl_checked(path, predictions)
    return path, [str(row["task_id"]) for row in selected]


def report_path(output_root: Path, run_id: str, model_name: str, instance_id: str) -> Path:
    return output_root / "logs" / "run_evaluation" / run_id / model_name.replace("/", "__") / instance_id / "report.json"


def status_from_report(path: Path, instance_id: str) -> dict[str, Any]:
    if not path.is_file():
        return {"oracle_status": "missing_report", "resolved": None, "report_path": str(path)}
    with path.open("r", encoding="utf-8") as handle:
        report = json.load(handle)
    body = report.get(instance_id)
    if not isinstance(body, dict):
        return {"oracle_status": "bad_report_shape", "resolved": None, "report_path": str(path)}
    if body.get("patch_is_None") or not body.get("patch_exists"):
        status = "empty_or_missing_patch"
    elif not body.get("patch_successfully_applied"):
        status = "patch_not_applied"
    elif body.get("resolved"):
        status = "resolved"
    else:
        status = "unresolved"
    tests_status = body.get("tests_status") or {}
    tests_summary: dict[str, dict[str, int]] = {}
    for transition in ("FAIL_TO_PASS", "PASS_TO_PASS", "FAIL_TO_FAIL", "PASS_TO_FAIL"):
        block = tests_status.get(transition) or {}
        tests_summary[transition] = {
            "success": len(block.get("success") or []),
            "failure": len(block.get("failure") or []),
        }
    return {
        "oracle_status": status,
        "resolved": bool(body.get("resolved")),
        "patch_successfully_applied": bool(body.get("patch_successfully_applied")),
        "patch_exists": bool(body.get("patch_exists")),
        "patch_is_none": bool(body.get("patch_is_None")),
        "tests_summary": tests_summary,
        "report_path": str(path),
        "report_sha256": sha256_file(path),
    }


def run_one_replica(args: argparse.Namespace, output_root: Path, selected: list[dict[str, Any]], replica_index: int) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    run_id = f"{args.run_id_prefix}-{output_root.name.replace('run-', '')}-replica-{replica_index:02d}"
    predictions_path, instance_ids = materialize_predictions(
        selected=selected,
        corpus_root=args.corpus_root,
        output_root=output_root,
        replica_index=replica_index,
        model_name=args.model_name,
    )
    command = [
        str(args.swebench_python),
        "-m",
        "swebench.harness.run_evaluation",
        "--dataset_name",
        args.dataset_name,
        "--split",
        args.split,
        "--predictions_path",
        str(predictions_path),
        "--max_workers",
        str(args.max_workers),
        "--open_file_limit",
        str(args.open_file_limit),
        "--instance_ids",
        *instance_ids,
        "--run_id",
        run_id,
        "--timeout",
        str(args.instance_timeout),
        "--cache_level",
        args.cache_level,
        "--clean",
        "False",
        "--namespace",
        args.namespace,
        "--report_dir",
        str(output_root),
    ]
    replica_root = output_root / "replicas" / f"replica-{replica_index:02d}"
    replica_root.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with args.harness_lock.open("a+", encoding="utf-8") as lock_file:
        import fcntl

        fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
        lock_file.seek(0)
        lock_file.truncate()
        lock_file.write(f"pid={os.getpid()} run_id={run_id} replica={replica_index}\n")
        lock_file.flush()
        os.fsync(lock_file.fileno())
        completed = subprocess.run(
            command,
            cwd=str(output_root),
            text=True,
            capture_output=True,
            timeout=args.overall_timeout,
            check=False,
        )
    duration_ms = int((time.monotonic() - started) * 1000)
    write_text_checked(replica_root / "stdout.txt", completed.stdout)
    write_text_checked(replica_root / "stderr.txt", completed.stderr)

    outcomes: list[dict[str, Any]] = []
    for row in selected:
        instance_id = str(row["task_id"])
        path = report_path(output_root, run_id, args.model_name, instance_id)
        parsed = status_from_report(path, instance_id)
        outcomes.append(
            {
                "schema_version": 1,
                "artifact_kind": "oracle_self_consistency_replica_outcome",
                "replica_index": replica_index,
                "run_id": run_id,
                "task_id": instance_id,
                "repo": row["repo"],
                "mutation_category": row["category"],
                "bucket": row["bucket"],
                "source_patch_path": str(args.corpus_root / str(row["patch_path"])),
                "source_patch_sha256": row["patch_sha256"],
                "index_oracle_all_passed": bool(row.get("oracle_all_passed")),
                **parsed,
            }
        )

    summary = {
        "schema_version": 1,
        "artifact_kind": "oracle_self_consistency_replica_summary",
        "replica_index": replica_index,
        "run_id": run_id,
        "exit_code": completed.returncode,
        "duration_ms": duration_ms,
        "prediction_count": len(instance_ids),
        "predictions_path": str(predictions_path),
        "predictions_sha256": sha256_file(predictions_path),
        "command": command,
        "stdout_sha256": sha256_file(replica_root / "stdout.txt"),
        "stderr_sha256": sha256_file(replica_root / "stderr.txt"),
        "outcome_counts": dict(sorted(Counter(row["oracle_status"] for row in outcomes).items())),
    }
    write_json_checked(replica_root / "summary.json", summary)
    return outcomes, summary


def pairwise_agreement(statuses: list[str]) -> dict[str, Any]:
    pairs = 0
    agreeing = 0
    disagreements: list[dict[str, Any]] = []
    for left in range(len(statuses)):
        for right in range(left + 1, len(statuses)):
            pairs += 1
            if statuses[left] == statuses[right]:
                agreeing += 1
            else:
                disagreements.append({"left": left, "right": right, "left_status": statuses[left], "right_status": statuses[right]})
    return {
        "pair_count": pairs,
        "agreeing_pair_count": agreeing,
        "agreement": agreeing / pairs if pairs else None,
        "status_counts": dict(sorted(Counter(statuses).items())),
        "disagreements": disagreements,
    }


def summarize_probe(output_root: Path, selected: list[dict[str, Any]], outcomes: list[dict[str, Any]], summaries: list[dict[str, Any]], args: argparse.Namespace) -> dict[str, Any]:
    by_task: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in outcomes:
        by_task[row["task_id"]].append(row)

    agreement_rows: list[dict[str, Any]] = []
    for selected_row in selected:
        task_id = str(selected_row["task_id"])
        rows = sorted(by_task.get(task_id, []), key=lambda row: row["replica_index"])
        statuses = [str(row["oracle_status"]) for row in rows]
        agreement = pairwise_agreement(statuses)
        agreement_rows.append(
            {
                "schema_version": 1,
                "artifact_kind": "oracle_self_consistency_instance_agreement",
                "task_id": task_id,
                "repo": selected_row["repo"],
                "mutation_category": selected_row["category"],
                "bucket": selected_row["bucket"],
                "replica_count": len(rows),
                **agreement,
            }
        )

    valid_agreements = [row["agreement"] for row in agreement_rows if isinstance(row.get("agreement"), float)]
    oracle_self_consistency = sum(valid_agreements) / len(valid_agreements) if valid_agreements else None
    write_jsonl_checked(output_root / "replica_outcomes.jsonl", outcomes)
    write_jsonl_checked(output_root / "per_instance_agreement.jsonl", agreement_rows)
    write_jsonl_checked(output_root / "replica_summaries.jsonl", summaries)

    probe_complete = (
        len(outcomes) == args.sample_size * args.replicas
        and len(agreement_rows) == args.sample_size
        and all(summary["exit_code"] == 0 for summary in summaries)
        and all(row["replica_count"] == args.replicas for row in agreement_rows)
    )
    manifest = {
        "schema_version": 1,
        "artifact_kind": "oracle_self_consistency_probe_manifest",
        "task_id": "INV-035/#760",
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_of_truth": {
            "index": str(args.index),
            "corpus_root": str(args.corpus_root),
            "output_root": str(output_root),
            "replica_outcomes": str(output_root / "replica_outcomes.jsonl"),
            "per_instance_agreement": str(output_root / "per_instance_agreement.jsonl"),
        },
        "path_policy": {
            "aiwonder_only": True,
            "no_mnt_c_or_mnt_d": True,
        },
        "sample_size": args.sample_size,
        "replicas": args.replicas,
        "expected_report_count": args.sample_size * args.replicas,
        "observed_outcome_count": len(outcomes),
        "oracle_self_consistency": oracle_self_consistency,
        "decision_rule": {
            "gte_0_97": "noise floor non-binding",
            "gte_0_95_lt_0_97": "re-frame Tier-1 target tight against oracle floor",
            "gte_0_93_lt_0_95": "target 0.95 unreachable at face value; use floor-minus-epsilon framing",
            "lt_0_93": "oracle harness refinement / flakiness modeling required",
        },
        "probe_complete": probe_complete,
        "replica_exit_codes": [summary["exit_code"] for summary in summaries],
        "repo_counts": dict(sorted(Counter(row["repo"] for row in selected).items())),
        "category_counts": dict(sorted(Counter(row["category"] for row in selected).items())),
        "oracle_status_counts": dict(sorted(Counter(row["oracle_status"] for row in outcomes).items())),
    }
    write_json_checked(output_root / "oracle_self_consistency_manifest.json", manifest)

    readback = {
        "manifest_sha256": sha256_file(output_root / "oracle_self_consistency_manifest.json"),
        "replica_outcomes_sha256": sha256_file(output_root / "replica_outcomes.jsonl"),
        "per_instance_agreement_sha256": sha256_file(output_root / "per_instance_agreement.jsonl"),
        "replica_summaries_sha256": sha256_file(output_root / "replica_summaries.jsonl"),
        "selection_sha256": sha256_file(output_root / "selection.jsonl"),
        "probe_complete": probe_complete,
        "oracle_self_consistency": oracle_self_consistency,
    }
    write_json_checked(output_root / "readback_hashes.json", readback)
    return manifest


def outcomes_from_summary(args: argparse.Namespace, output_root: Path, selected: list[dict[str, Any]], summary: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    replica_index = int(summary["replica_index"])
    run_id = str(summary["run_id"])
    for row in selected:
        instance_id = str(row["task_id"])
        path = report_path(output_root, run_id, args.model_name, instance_id)
        parsed = status_from_report(path, instance_id)
        rows.append(
            {
                "schema_version": 1,
                "artifact_kind": "oracle_self_consistency_replica_outcome",
                "replica_index": replica_index,
                "run_id": run_id,
                "task_id": instance_id,
                "repo": row["repo"],
                "mutation_category": row["category"],
                "bucket": row["bucket"],
                "source_patch_path": str(args.corpus_root / str(row["patch_path"])),
                "source_patch_sha256": row["patch_sha256"],
                "index_oracle_all_passed": bool(row.get("oracle_all_passed")),
                **parsed,
            }
        )
    return rows


def load_existing_summaries(output_root: Path, replicas: int) -> list[dict[str, Any]]:
    summaries: list[dict[str, Any]] = []
    for replica_index in range(replicas):
        path = output_root / "replicas" / f"replica-{replica_index:02d}" / "summary.json"
        if not path.is_file():
            raise ProbeError(f"ORACLE_SELF_CONSISTENCY_REPLICA_SUMMARY_MISSING: {path}")
        summaries.append(read_json(path))
    return summaries


def recompute_existing_probe(args: argparse.Namespace, selected: list[dict[str, Any]]) -> dict[str, Any]:
    summaries = load_existing_summaries(args.output_root, args.replicas)
    outcomes: list[dict[str, Any]] = []
    for summary in summaries:
        outcomes.extend(outcomes_from_summary(args, args.output_root, selected, summary))
    write_json_checked(
        args.output_root / "run_status.json",
        {
            "completed_replicas": len(summaries),
            "requested_replicas": args.replicas,
            "outcome_count": len(outcomes),
            "last_replica_exit_code": summaries[-1]["exit_code"] if summaries else None,
            "updated_at_utc": datetime.now(timezone.utc).isoformat(),
        },
    )
    return summarize_probe(args.output_root, selected, outcomes, summaries, args)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", type=Path, default=DEFAULT_INDEX)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument("--corpus-root", type=Path, default=None)
    parser.add_argument("--swebench-python", type=Path, default=DEFAULT_PYTHON)
    parser.add_argument("--harness-lock", type=Path, default=DEFAULT_HARNESS_LOCK)
    parser.add_argument("--sample-size", type=int, default=30)
    parser.add_argument("--replicas", type=int, default=10)
    parser.add_argument("--seed", default="inv-035-oracle-self-consistency-v1")
    parser.add_argument("--dataset-name", default="princeton-nlp/SWE-bench_Lite")
    parser.add_argument("--split", default="test")
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--model-name", default="contextgraph-inv035-oracle-self-consistency")
    parser.add_argument("--run-id-prefix", default="inv035-oracle-self-consistency")
    parser.add_argument("--max-workers", type=int, default=4)
    parser.add_argument("--open-file-limit", type=int, default=4096)
    parser.add_argument("--instance-timeout", type=int, default=1800)
    parser.add_argument("--overall-timeout", type=int, default=14400)
    parser.add_argument("--cache-level", default="instance")
    parser.add_argument("--selection-only", action="store_true")
    parser.add_argument("--recompute-existing", action="store_true")
    parser.add_argument("--rerun-replica", type=int)
    args = parser.parse_args(argv)
    if args.sample_size < 1 or args.replicas < 1:
        raise ProbeError("ORACLE_SELF_CONSISTENCY_INVALID_SAMPLE_OR_REPLICA_COUNT")
    args.index = require_aiwonder_path(args.index, "index")
    args.output_parent = require_aiwonder_path(args.output_parent, "output_parent")
    if args.output_root is None:
        args.output_root = args.output_parent / f"run-{utc_stamp()}"
    args.output_root = require_aiwonder_path(args.output_root, "output_root")
    if args.corpus_root is None:
        args.corpus_root = args.index.parent
    args.corpus_root = require_aiwonder_path(args.corpus_root, "corpus_root")
    args.harness_lock = require_aiwonder_path(args.harness_lock, "harness_lock")
    if not args.swebench_python.is_file():
        raise ProbeError(f"ORACLE_SELF_CONSISTENCY_PYTHON_MISSING: {args.swebench_python}")
    if args.rerun_replica is not None and not (0 <= args.rerun_replica < args.replicas):
        raise ProbeError(f"ORACLE_SELF_CONSISTENCY_RERUN_REPLICA_INVALID: {args.rerun_replica}")
    return args


def main(argv: list[str]) -> int:
    try:
        args = parse_args(argv)
        payload, entries = load_index(args.index)
        selected = select_rows(entries, args.sample_size, args.seed)
        args.output_root.mkdir(parents=True, exist_ok=True)
        selection_rows = [
            {
                "schema_version": 1,
                "artifact_kind": "oracle_self_consistency_selection_row",
                "ordinal": index,
                "task_id": row["task_id"],
                "repo": row["repo"],
                "bucket": row["bucket"],
                "mutation_category": row["category"],
                "patch_path": str(args.corpus_root / str(row["patch_path"])),
                "patch_sha256": row["patch_sha256"],
                "index_oracle_all_passed": bool(row.get("oracle_all_passed")),
            }
            for index, row in enumerate(selected)
        ]
        write_jsonl_checked(args.output_root / "selection.jsonl", selection_rows)
        write_json_checked(
            args.output_root / "probe_config.json",
            {
                "schema_version": 1,
                "artifact_kind": "oracle_self_consistency_probe_config",
                "index": str(args.index),
                "index_sha256": sha256_file(args.index),
                "corpus_sha256": payload.get("corpus_sha256"),
                "output_root": str(args.output_root),
                "sample_size": args.sample_size,
                "replicas": args.replicas,
                "seed": args.seed,
                "selection_sha256": sha256_file(args.output_root / "selection.jsonl"),
            },
        )
        if args.selection_only:
            return 0
        if args.rerun_replica is not None:
            run_one_replica(args, args.output_root, selected, args.rerun_replica)
            manifest = recompute_existing_probe(args, selected)
            print(json.dumps({"output_root": str(args.output_root), "oracle_self_consistency": manifest["oracle_self_consistency"], "probe_complete": manifest["probe_complete"]}, sort_keys=True))
            return 0 if manifest["probe_complete"] else 1
        if args.recompute_existing:
            manifest = recompute_existing_probe(args, selected)
            print(json.dumps({"output_root": str(args.output_root), "oracle_self_consistency": manifest["oracle_self_consistency"], "probe_complete": manifest["probe_complete"]}, sort_keys=True))
            return 0 if manifest["probe_complete"] else 1

        outcomes: list[dict[str, Any]] = []
        summaries: list[dict[str, Any]] = []
        for replica_index in range(args.replicas):
            replica_outcomes, replica_summary = run_one_replica(args, args.output_root, selected, replica_index)
            outcomes.extend(replica_outcomes)
            summaries.append(replica_summary)
            write_json_checked(
                args.output_root / "run_status.json",
                {
                    "completed_replicas": len(summaries),
                    "requested_replicas": args.replicas,
                    "outcome_count": len(outcomes),
                    "last_replica_exit_code": replica_summary["exit_code"],
                    "updated_at_utc": datetime.now(timezone.utc).isoformat(),
                },
            )
        manifest = summarize_probe(args.output_root, selected, outcomes, summaries, args)
        print(json.dumps({"output_root": str(args.output_root), "oracle_self_consistency": manifest["oracle_self_consistency"], "probe_complete": manifest["probe_complete"]}, sort_keys=True))
        return 0 if manifest["probe_complete"] else 1
    except subprocess.TimeoutExpired as exc:
        print(f"ORACLE_SELF_CONSISTENCY_HARNESS_TIMEOUT: {exc}", file=sys.stderr)
        return 124
    except ProbeError as exc:
        print(str(exc), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
