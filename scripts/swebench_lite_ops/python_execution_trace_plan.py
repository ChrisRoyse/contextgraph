#!/usr/bin/env python3
"""Plan INV-010 execution-trace generation by SWE-bench command family."""

from __future__ import annotations

import argparse
import hashlib
import json
import shlex
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

from .python_execution_trace_smoke import import_swebench_runtime


SCHEMA_VERSION = 1
TASK_ID = "INV-010"
ARTIFACT_KIND = "execution_trace_command_plan"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_DATASET = "SWE-bench/SWE-bench_Lite"
DEFAULT_SPLIT = "test"
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/inv-010-execution-aware-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/execution-traces/inv-010-v1")
EXPECTED_CATEGORIES = {
    "compile_error",
    "delete_test_call",
    "known_good",
    "off_by_one",
    "over_engineer",
    "subtle_flip",
    "swap_variable",
    "wrong_file",
}
CURRENT_SUPPORTED_FAMILY = "pytest_direct"


class ExecutionTracePlanError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise ExecutionTracePlanError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "EXECUTION_TRACE_PLAN_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(canonical_json(row) + "\n")
            count += 1
    tmp.replace(path)
    return count


def read_json(path: Path, code: str) -> Any:
    require(path.is_file(), code, {"path": str(path)})
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ExecutionTracePlanError(f"{code}: invalid JSON at {path}: {exc}") from exc


def load_corpus_entries(corpus_root: Path, expected_rows: int | None) -> list[dict[str, Any]]:
    doc = read_json(corpus_root / "index.json", "EXECUTION_TRACE_PLAN_CORPUS_INDEX_MISSING")
    entries = doc.get("entries")
    require(isinstance(entries, list), "EXECUTION_TRACE_PLAN_CORPUS_INDEX_BAD_SCHEMA")
    if expected_rows is not None:
        require(
            len(entries) == expected_rows,
            "EXECUTION_TRACE_PLAN_CORPUS_ROW_COUNT_MISMATCH",
            {"expected": expected_rows, "actual": len(entries)},
        )
    out: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for idx, raw in enumerate(entries):
        require(isinstance(raw, dict), "EXECUTION_TRACE_PLAN_CORPUS_ROW_NOT_OBJECT", {"index": idx})
        task_id = str(raw.get("task_id") or "")
        repo = str(raw.get("repo") or "")
        category = str(raw.get("category") or "")
        patch_path = str(raw.get("patch_path") or "")
        require(task_id, "EXECUTION_TRACE_PLAN_TASK_ID_MISSING", {"index": idx})
        require(repo, "EXECUTION_TRACE_PLAN_REPO_MISSING", {"index": idx, "task_id": task_id})
        require(category, "EXECUTION_TRACE_PLAN_CATEGORY_MISSING", {"index": idx, "task_id": task_id})
        require(patch_path, "EXECUTION_TRACE_PLAN_PATCH_PATH_MISSING", {"index": idx, "task_id": task_id})
        key = (task_id, category)
        require(
            key not in seen,
            "EXECUTION_TRACE_PLAN_DUPLICATE_CORPUS_ROW",
            {"task_id": task_id, "category": category},
        )
        seen.add(key)
        out.append(
            {
                "row_index": idx,
                "task_id": task_id,
                "repo": repo,
                "category": category,
                "patch_path": patch_path,
            }
        )
    return out


def group_entries_by_task(entries: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    grouped: dict[str, dict[str, Any]] = {}
    for row in entries:
        task_id = row["task_id"]
        item = grouped.setdefault(
            task_id,
            {"task_id": task_id, "repo": row["repo"], "categories": [], "row_indices": []},
        )
        require(
            item["repo"] == row["repo"],
            "EXECUTION_TRACE_PLAN_TASK_REPO_CONFLICT",
            {"task_id": task_id, "first_repo": item["repo"], "row_repo": row["repo"]},
        )
        item["categories"].append(row["category"])
        item["row_indices"].append(row["row_index"])
    for item in grouped.values():
        item["categories"] = sorted(item["categories"])
        item["row_indices"] = sorted(item["row_indices"])
    return grouped


def strip_shell_prefixes(tokens: list[str]) -> list[str]:
    out = list(tokens)
    while out and "=" in out[0] and not out[0].startswith("-") and out[0].split("=", 1)[0].isidentifier():
        out = out[1:]
    while out and out[0] in {"timeout", "gtimeout"} and len(out) >= 2:
        out = out[2:]
    while out and out[0] in {"xvfb-run"}:
        out = out[1:]
        while out and out[0].startswith("-"):
            if out[0] in {"-a"}:
                out = out[1:]
            elif len(out) >= 2:
                out = out[2:]
            else:
                out = []
    return out


def parse_command(command: str) -> list[str]:
    try:
        return strip_shell_prefixes(shlex.split(command))
    except ValueError:
        return []


def is_marker_or_setup(command: str) -> bool:
    stripped = command.strip()
    lowered = stripped.lower()
    if not stripped:
        return True
    if "start test output" in lowered or "end test output" in lowered:
        return True
    setup_prefixes = (
        "source ",
        "conda ",
        "cd ",
        "git -c ",
        "git apply ",
        "git config ",
        "git checkout ",
        "git reset ",
        "git show",
        "git status",
        "python -m pip install ",
        "python - <<",
        "cat > ",
        "export ",
        "rm -f ",
    )
    return stripped.startswith(setup_prefixes)


def command_family(command: str) -> dict[str, Any]:
    tokens = parse_command(command)
    if not tokens:
        return {
            "command": command,
            "family": "unparseable",
            "support_status": "unsupported",
            "handler": None,
            "blocker_code": "EXECUTION_TRACE_PLAN_UNPARSEABLE_COMMAND",
        }
    executable = Path(tokens[0]).name
    lowered = [token.lower() for token in tokens]
    if executable == "pytest":
        return {
            "command": command,
            "family": "pytest_direct",
            "support_status": "supported_current",
            "handler": "pytest_sys_settrace_v1",
            "blocker_code": None,
        }
    if executable in {"python", "python3"} and len(tokens) >= 3 and tokens[1:3] == ["-m", "pytest"]:
        return {
            "command": command,
            "family": "python_module_pytest",
            "support_status": "handler_required",
            "handler": "pytest_sys_settrace_v1_needs_module_command_adapter",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_PYTHON_MODULE_PYTEST",
        }
    if executable == "coverage" and "-m" in tokens and "pytest" in lowered:
        return {
            "command": command,
            "family": "coverage_pytest",
            "support_status": "handler_required",
            "handler": "pytest_sys_settrace_v1_needs_coverage_adapter",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_COVERAGE_PYTEST",
        }
    if "pytest" in lowered:
        return {
            "command": command,
            "family": "pytest_wrapped",
            "support_status": "handler_required",
            "handler": "pytest_sys_settrace_v1_needs_shell_wrapper_adapter",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_WRAPPED_PYTEST",
        }
    script_token = next((token for token in tokens if Path(token).name == "runtests.py"), "")
    if script_token:
        return {
            "command": command,
            "family": "django_runtests",
            "support_status": "handler_required",
            "handler": "python_script_sys_settrace_required",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_DJANGO_RUNTESTS",
        }
    if executable in {"tox", "tox4"}:
        return {
            "command": command,
            "family": "tox",
            "support_status": "handler_required",
            "handler": "tox_command_trace_adapter_required",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_TOX",
        }
    if tokens[0] in {"bin/test", "./bin/test"}:
        return {
            "command": command,
            "family": "sympy_bin_test",
            "support_status": "handler_required",
            "handler": "python_script_sys_settrace_required",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_SYMPY_BIN_TEST",
        }
    if executable in {"python", "python3"} and any(token.endswith(".py") for token in tokens[1:]):
        return {
            "command": command,
            "family": "python_script",
            "support_status": "handler_required",
            "handler": "python_script_sys_settrace_required",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_GENERIC_PYTHON_SCRIPT",
        }
    if tokens[0].startswith("./") and tokens[0].endswith(".py"):
        return {
            "command": command,
            "family": "python_script",
            "support_status": "handler_required",
            "handler": "python_script_sys_settrace_required",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_GENERIC_PYTHON_SCRIPT",
        }
    if executable == "python" and "-m" in tokens and "unittest" in lowered:
        return {
            "command": command,
            "family": "python_unittest",
            "support_status": "handler_required",
            "handler": "python_module_sys_settrace_required",
            "blocker_code": "EXECUTION_TRACE_PLAN_UNSUPPORTED_UNITTEST",
        }
    return {
        "command": command,
        "family": "unknown",
        "support_status": "unsupported",
        "handler": None,
        "blocker_code": "EXECUTION_TRACE_PLAN_UNKNOWN_TEST_COMMAND",
    }


def test_output_commands(eval_script_list: list[str]) -> list[str]:
    start = None
    end = None
    for idx, command in enumerate(eval_script_list):
        lowered = command.lower()
        if "start test output" in lowered and start is None:
            start = idx
        if "end test output" in lowered and start is not None:
            end = idx
            break
    if start is not None and end is not None and end > start:
        candidates = eval_script_list[start + 1:end]
    else:
        candidates = eval_script_list
    return [command for command in candidates if not is_marker_or_setup(command)]


def classify_eval_script(eval_script_list: list[str]) -> dict[str, Any]:
    commands = test_output_commands(eval_script_list)
    classifications = [command_family(command) for command in commands]
    test_like = [item for item in classifications if item["family"] not in {"unparseable", "unknown"}]
    if len(test_like) == 0:
        return {
            "command_family": "unknown",
            "support_status": "unsupported",
            "handler": None,
            "blocker_code": "EXECUTION_TRACE_PLAN_NO_TEST_COMMAND",
            "test_command_count": len(commands),
            "test_commands": commands,
            "classifications": classifications,
        }
    if len(test_like) > 1:
        families = sorted({item["family"] for item in test_like})
        return {
            "command_family": "multiple_test_commands",
            "support_status": "unsupported",
            "handler": None,
            "blocker_code": "EXECUTION_TRACE_PLAN_MULTIPLE_TEST_COMMANDS",
            "test_command_count": len(test_like),
            "test_commands": [item["command"] for item in test_like],
            "classifications": classifications,
            "families": families,
        }
    item = dict(test_like[0])
    item.update(
        {
            "command_family": item.pop("family"),
            "test_command_count": 1,
            "test_commands": [item["command"]],
            "classifications": classifications,
        }
    )
    return item


def load_spec_fixture(path: Path) -> dict[str, list[str]]:
    payload = read_json(path, "EXECUTION_TRACE_PLAN_SPEC_FIXTURE_MISSING")
    if isinstance(payload, dict) and "tasks" in payload:
        payload = payload["tasks"]
    require(isinstance(payload, dict), "EXECUTION_TRACE_PLAN_SPEC_FIXTURE_BAD_SCHEMA")
    out: dict[str, list[str]] = {}
    for task_id, raw in payload.items():
        if isinstance(raw, dict):
            raw = raw.get("eval_script_list")
        require(isinstance(raw, list), "EXECUTION_TRACE_PLAN_SPEC_FIXTURE_TASK_BAD_SCHEMA", {"task_id": task_id})
        out[str(task_id)] = [str(item) for item in raw]
    return out


def load_real_eval_scripts(
    *,
    dataset_name: str,
    split: str,
    task_ids: list[str],
    namespace: str,
    tag: str,
    arch: str,
) -> dict[str, list[str]]:
    runtime = import_swebench_runtime()
    dataset = runtime["load_swebench_dataset"](dataset_name, split, task_ids)
    by_task = {str(item.get(runtime["KEY_INSTANCE_ID"]) or ""): dict(item) for item in dataset}
    missing = sorted(set(task_ids) - set(by_task))
    require(
        not missing,
        "EXECUTION_TRACE_PLAN_DATASET_TASKS_MISSING",
        {"missing_sample": missing[:10], "missing_count": len(missing)},
    )
    out: dict[str, list[str]] = {}
    for task_id in task_ids:
        test_spec = runtime["make_test_spec"](
            by_task[task_id],
            namespace=namespace,
            instance_image_tag=tag,
            env_image_tag=tag,
            arch=arch,
        )
        out[task_id] = [str(command) for command in list(test_spec.eval_script_list)]
    return out


def build_task_plan(
    *,
    task_group: dict[str, dict[str, Any]],
    eval_scripts: dict[str, list[str]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for task_id in sorted(task_group):
        require(
            task_id in eval_scripts,
            "EXECUTION_TRACE_PLAN_EVAL_SCRIPT_MISSING",
            {"task_id": task_id},
        )
        eval_script = eval_scripts[task_id]
        classification = classify_eval_script(eval_script)
        group = task_group[task_id]
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "task_id": task_id,
                "repo": group["repo"],
                "categories": group["categories"],
                "row_count": len(group["categories"]),
                "row_indices": group["row_indices"],
                "command_family": classification["command_family"],
                "support_status": classification["support_status"],
                "handler": classification["handler"],
                "blocker_code": classification["blocker_code"],
                "test_command_count": classification["test_command_count"],
                "test_commands": classification["test_commands"],
                "eval_script_sha256": sha256_bytes(canonical_json(eval_script).encode("utf-8")),
            }
        )
    return rows


def build_row_plan(
    *,
    entries: list[dict[str, Any]],
    task_plan_by_id: dict[str, dict[str, Any]],
    state_run_root: Path,
    fsv_run_root: Path,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for entry in entries:
        task_plan = task_plan_by_id[entry["task_id"]]
        trace_rel = Path(entry["task_id"]) / f"{entry['category']}.execution-trace.json"
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "task_id": entry["task_id"],
                "repo": entry["repo"],
                "category": entry["category"],
                "row_index": entry["row_index"],
                "command_family": task_plan["command_family"],
                "support_status": task_plan["support_status"],
                "handler": task_plan["handler"],
                "blocker_code": task_plan["blocker_code"],
                "planned_state_trace_path": str(state_run_root / "generated-traces" / trace_rel),
                "planned_fsv_trace_path": str(fsv_run_root / "generated-traces" / trace_rel),
            }
        )
    return rows


def summarize_plan(entries: list[dict[str, Any]], task_rows: list[dict[str, Any]], row_rows: list[dict[str, Any]]) -> dict[str, Any]:
    task_family_counts = Counter(row["command_family"] for row in task_rows)
    row_family_counts = Counter(row["command_family"] for row in row_rows)
    task_status_counts = Counter(row["support_status"] for row in task_rows)
    row_status_counts = Counter(row["support_status"] for row in row_rows)
    rows_by_cell_and_status: dict[str, Counter[str]] = defaultdict(Counter)
    for row in row_rows:
        rows_by_cell_and_status[row["category"]][row["support_status"]] += 1
    supported_rows = int(row_status_counts.get("supported_current", 0))
    corpus_rows = len(entries)
    return {
        "task_count": len(task_rows),
        "corpus_rows": corpus_rows,
        "categories": sorted({row["category"] for row in entries}),
        "task_command_family_counts": dict(sorted(task_family_counts.items())),
        "row_command_family_counts": dict(sorted(row_family_counts.items())),
        "task_support_status_counts": dict(sorted(task_status_counts.items())),
        "row_support_status_counts": dict(sorted(row_status_counts.items())),
        "row_support_status_by_cell": {
            cell: dict(sorted(counter.items()))
            for cell, counter in sorted(rows_by_cell_and_status.items())
        },
        "current_supported_rows": supported_rows,
        "current_supported_trace_fraction": round(supported_rows / corpus_rows, 10) if corpus_rows else 0.0,
        "rows_requiring_new_handler": int(row_status_counts.get("handler_required", 0)),
        "rows_unsupported": int(row_status_counts.get("unsupported", 0)),
    }


def write_artifact_hashes(root: Path, paths: list[Path]) -> Path:
    rows = []
    for path in sorted(paths):
        if path.is_file():
            rows.append({"path": str(path), "sha256": sha256_file(path), "bytes": path.stat().st_size})
    out = root / "artifact_hashes.json"
    write_json(out, {"artifact_count": len(rows), "artifacts": rows})
    return out


def run_plan(args: argparse.Namespace) -> dict[str, Any]:
    allow_non_aiwonder = bool(getattr(args, "allow_non_aiwonder_roots", False))
    corpus_root = Path(args.corpus_root)
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    require_archive_path(corpus_root, "corpus_root", allow_non_aiwonder)
    require_archive_path(output_root, "output_root", allow_non_aiwonder)
    require_archive_path(state_root, "state_root", allow_non_aiwonder)

    run_id = str(args.run_id or f"run-{utc_stamp()}")
    fsv_run_root = output_root / run_id / "trace-plan"
    state_run_root = state_root / run_id / "trace-plan"
    require(
        not fsv_run_root.exists() or not any(fsv_run_root.iterdir()),
        "EXECUTION_TRACE_PLAN_OUTPUT_ROOT_NOT_EMPTY",
        {"fsv_run_root": str(fsv_run_root)},
    )
    require(
        not state_run_root.exists() or not any(state_run_root.iterdir()),
        "EXECUTION_TRACE_PLAN_STATE_ROOT_NOT_EMPTY",
        {"state_run_root": str(state_run_root)},
    )
    fsv_run_root.mkdir(parents=True, exist_ok=True)
    state_run_root.mkdir(parents=True, exist_ok=True)

    entries = load_corpus_entries(corpus_root, args.expected_corpus_rows)
    task_group = group_entries_by_task(entries)
    task_ids = sorted(task_group)
    if args.spec_fixture:
        eval_scripts = load_spec_fixture(Path(args.spec_fixture))
    else:
        eval_scripts = load_real_eval_scripts(
            dataset_name=args.dataset_name,
            split=args.split,
            task_ids=task_ids,
            namespace=args.namespace,
            tag=args.tag,
            arch=args.arch,
        )
    task_rows = build_task_plan(task_group=task_group, eval_scripts=eval_scripts)
    task_plan_by_id = {row["task_id"]: row for row in task_rows}
    row_rows = build_row_plan(
        entries=entries,
        task_plan_by_id=task_plan_by_id,
        state_run_root=state_run_root,
        fsv_run_root=fsv_run_root,
    )
    summary = summarize_plan(entries, task_rows, row_rows)
    unsupported_rows = [row for row in row_rows if row["support_status"] != "supported_current"]
    runnable_rows = [row for row in row_rows if row["support_status"] == "supported_current"]
    unexpected_categories = sorted(set(summary["categories"]) - EXPECTED_CATEGORIES)
    decision_reasons: list[str] = []
    if summary["current_supported_trace_fraction"] < 0.95:
        decision_reasons.append("current_trace_handler_coverage_below_readiness_threshold")
    if summary["rows_requiring_new_handler"] > 0:
        decision_reasons.append("typed_trace_handlers_required")
    if summary["rows_unsupported"] > 0:
        decision_reasons.append("unsupported_command_families_require_fail_closed_blockers")
    if unexpected_categories:
        decision_reasons.append("unexpected_mutation_categories_present")
    if not decision_reasons:
        decision_reasons.append("all_rows_supported_by_current_pytest_trace_handler")

    task_plan_path = fsv_run_root / "execution_trace_task_plan.jsonl"
    row_plan_path = fsv_run_root / "execution_trace_row_plan.jsonl"
    runnable_rows_path = fsv_run_root / "runnable_current_pytest_rows.jsonl"
    unsupported_rows_path = fsv_run_root / "unsupported_command_rows.jsonl"
    write_jsonl(task_plan_path, task_rows)
    write_jsonl(row_plan_path, row_rows)
    write_jsonl(runnable_rows_path, runnable_rows)
    write_jsonl(unsupported_rows_path, unsupported_rows)

    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "inputs": {
            "dataset_name": args.dataset_name,
            "split": args.split,
            "corpus_root": str(corpus_root),
            "corpus_index_path": str(corpus_root / "index.json"),
            "corpus_index_sha256": sha256_file(corpus_root / "index.json"),
            "spec_fixture": str(args.spec_fixture) if args.spec_fixture else None,
            "namespace": args.namespace,
            "arch": args.arch,
            "tag": args.tag,
        },
        "summary": summary,
        "decision": {
            "full_corpus_trace_generation_ready_with_current_handler": (
                summary["current_supported_trace_fraction"] >= 0.95
            ),
            "trace_generation_required": True,
            "per_cell_oracle_correlation_lift_measured": False,
            "promotion_allowed": False,
            "canonical_issue": "#774",
            "decision_reasons": decision_reasons,
        },
        "doctrine_policy": {
            "measurement_only": True,
            "target_labels_used_as_live_inputs": False,
            "oracle_verdicts_classified_target_side_only": True,
            "flat_vector_concat_used": False,
            "slot_identity_preserved": True,
            "predictor_weights_changed": False,
            "inner_llm_used": False,
            "aiwonder_durable_root_required": not allow_non_aiwonder,
        },
        "artifacts": {
            "fsv_run_root": str(fsv_run_root),
            "state_run_root": str(state_run_root),
            "task_plan": str(task_plan_path),
            "row_plan": str(row_plan_path),
            "runnable_current_pytest_rows": str(runnable_rows_path),
            "unsupported_command_rows": str(unsupported_rows_path),
        },
        "next_actions": [
            "run python_execution_trace_smoke.py across runnable_current_pytest_rows",
            "add typed sys.settrace handlers for the largest handler_required command families",
            "rerun readiness audit with the generated execution-trace state root",
        ],
    }
    manifest_path = fsv_run_root / "execution_trace_command_plan_manifest.json"
    write_json(manifest_path, manifest)

    state_manifest_path = state_run_root / manifest_path.name
    state_task_plan_path = state_run_root / task_plan_path.name
    state_row_plan_path = state_run_root / row_plan_path.name
    state_runnable_rows_path = state_run_root / runnable_rows_path.name
    state_unsupported_rows_path = state_run_root / unsupported_rows_path.name
    write_json(state_manifest_path, manifest)
    write_jsonl(state_task_plan_path, task_rows)
    write_jsonl(state_row_plan_path, row_rows)
    write_jsonl(state_runnable_rows_path, runnable_rows)
    write_jsonl(state_unsupported_rows_path, unsupported_rows)
    hash_path = write_artifact_hashes(
        fsv_run_root,
        [
            manifest_path,
            task_plan_path,
            row_plan_path,
            runnable_rows_path,
            unsupported_rows_path,
            state_manifest_path,
            state_task_plan_path,
            state_row_plan_path,
            state_runnable_rows_path,
            state_unsupported_rows_path,
        ],
    )
    state_hash_path = state_run_root / hash_path.name
    write_json(state_hash_path, read_json(hash_path, "EXECUTION_TRACE_PLAN_HASH_JSON_INVALID"))
    return {
        "ok": True,
        "run_id": run_id,
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "manifest": str(manifest_path),
        "task_plan": str(task_plan_path),
        "row_plan": str(row_plan_path),
        "runnable_current_pytest_rows": str(runnable_rows_path),
        "unsupported_command_rows": str(unsupported_rows_path),
        "artifact_hashes": str(hash_path),
        "summary": summary,
        "decision": manifest["decision"],
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id")
    parser.add_argument("--expected-corpus-rows", type=int, default=2400)
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--arch", default="x86_64")
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--spec-fixture", type=Path)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    try:
        result = run_plan(args)
    except ExecutionTracePlanError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, indent=2, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
