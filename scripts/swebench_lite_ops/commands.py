from __future__ import annotations

import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .core import (
    EXPECTED_LITE_COUNT,
    SwebenchOpsError,
    ensure_dir,
    ensure_file,
    load_task_manifests,
    official_image_ref,
    parse_json_tail,
    python_bin,
    run,
    utc_now,
)
from .docker_cache import cache_status, docker_overview, inspect_image


def expected_ids(args: Any, fallback: str | None = None) -> list[str]:
    if args.expected_instance_ids is None:
        return [fallback or args.sample_task]
    return args.expected_instance_ids


def require_clean_cache(status: dict[str, Any]) -> None:
    if not status["passes_cache_hygiene"]:
        raise SwebenchOpsError(
            "SWEBENCH_CACHE_NOT_CLEAN",
            "local SWE-bench Docker cache is missing required images or has duplicates/stale images",
            "Pull the required image and prune unexpected SWE-bench images before continuing.",
            status,
        )


def probe_python_env(venv: Path, dataset_name: str, split: str, timeout: int) -> dict[str, Any]:
    python = python_bin(venv)
    ensure_file(python, "SWEBENCH_VENV_PYTHON_MISSING", "SWE-bench venv Python")
    snippet = r"""
import json
import logging
logging.getLogger("httpx").setLevel(logging.WARNING)
logging.getLogger("huggingface_hub").setLevel(logging.WARNING)
import datasets
import docker
import swebench
from datasets import load_dataset
ds = load_dataset(DATASET_NAME, split=SPLIT)
print(json.dumps({
    "swebench_version": getattr(swebench, "__version__", "unknown"),
    "datasets_version": datasets.__version__,
    "docker_py_version": docker.__version__,
    "dataset_name": DATASET_NAME,
    "split": SPLIT,
    "count": len(ds),
    "ids": [row["instance_id"] for row in ds],
}, sort_keys=True))
""".replace("DATASET_NAME", repr(dataset_name)).replace("SPLIT", repr(split))
    env = os.environ.copy()
    env.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")
    result = run([str(python), "-c", snippet], timeout=timeout, env=env, stdout_limit=120000)
    payload = parse_json_tail(result)
    if payload["count"] != EXPECTED_LITE_COUNT:
        raise SwebenchOpsError(
            "SWEBENCH_DATASET_COUNT_MISMATCH",
            f"expected {EXPECTED_LITE_COUNT} dataset rows, got {payload['count']}",
            "Verify the dataset name/split and Hugging Face cache.",
            payload,
        )
    return {"probe": result.to_json(), "payload": payload}


def check_remote_manifest(ref: str, timeout: int) -> dict[str, Any]:
    result = run(["docker", "manifest", "inspect", ref], timeout=timeout, stdout_limit=200000)
    payload = json.loads(result.stdout_tail)
    return {
        "image": ref,
        "schema_version": payload.get("schemaVersion"),
        "media_type": payload.get("mediaType"),
        "layers": len(payload.get("layers", [])),
        "manifests": len(payload.get("manifests", [])),
    }


def command_preflight(args: Any) -> dict[str, Any]:
    before = {
        "swebench_root_exists": args.swebench_root.is_dir(),
        "venv_python_exists": python_bin(args.venv).is_file(),
        "tasks_dir_exists": args.tasks_dir.is_dir(),
    }
    ensure_dir(args.swebench_root, "SWEBENCH_SOURCE_TREE_MISSING", "official SWE-bench source tree")
    manifests = load_task_manifests(args.tasks_dir)
    task_ids = sorted(manifests)
    python_probe = probe_python_env(args.venv, args.dataset_name, args.split, args.timeout)
    dataset_ids = set(python_probe["payload"]["ids"])
    manifest_ids = set(task_ids)
    if dataset_ids != manifest_ids:
        raise SwebenchOpsError(
            "SWEBENCH_DATASET_MANIFEST_MISMATCH",
            "checked-in SWE-bench Lite manifests do not match the official dataset ids",
            "Regenerate tasks/swebench-lite from the official dataset before corpus generation.",
            {
                "dataset_only": sorted(dataset_ids - manifest_ids)[:20],
                "manifest_only": sorted(manifest_ids - dataset_ids)[:20],
                "dataset_count": len(dataset_ids),
                "manifest_count": len(manifest_ids),
            },
        )
    sample_image = official_image_ref(args.sample_task, args.namespace, args.arch, args.tag)
    status = cache_status(expected_ids(args), args.namespace, args.arch, args.tag)
    if args.require_clean_cache:
        require_clean_cache(status)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "official_dataset": args.dataset_name,
            "official_split": args.split,
            "task_manifests": str(args.tasks_dir),
            "swebench_root": str(args.swebench_root),
            "venv_python": str(python_bin(args.venv)),
            "docker_state": "docker info + docker images",
        },
        "before_state": before,
        "after_state": {
            "task_manifest_count": len(manifests),
            "dataset_count": python_probe["payload"]["count"],
            "dataset_first_id": python_probe["payload"]["ids"][0],
            "dataset_last_id": python_probe["payload"]["ids"][-1],
            "sample_image": sample_image,
            "remote_manifest": check_remote_manifest(sample_image, args.timeout),
            "docker": docker_overview(args.timeout),
            "cache_status": status,
        },
        "python_probe": python_probe,
        "passes": True,
    }


def command_image_status(args: Any) -> dict[str, Any]:
    load_task_manifests(args.tasks_dir)
    status = cache_status(expected_ids(args), args.namespace, args.arch, args.tag)
    if args.require_clean:
        require_clean_cache(status)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "task_manifests": str(args.tasks_dir),
            "docker_images": "docker images --format '{{json .}}'",
        },
        "image_naming": {
            "rule": "official remote TestSpec.instance_image_key replaces '__' with '_1776_'",
            "sample": {args.sample_task: official_image_ref(args.sample_task, args.namespace, args.arch, args.tag)},
        },
        "after_state": status,
        "passes": True,
    }


def command_pull_images(args: Any) -> dict[str, Any]:
    manifests = load_task_manifests(args.tasks_dir)
    task_ids = args.instance_ids or [args.sample_task]
    unknown = sorted(set(task_ids) - set(manifests))
    if unknown:
        raise SwebenchOpsError(
            "SWEBENCH_UNKNOWN_INSTANCE_ID",
            f"requested instance ids are not in SWE-bench Lite: {unknown[:5]}",
            "Use ids from tasks/swebench-lite or the official Lite dataset.",
            {"unknown": unknown},
        )
    before = cache_status(task_ids, args.namespace, args.arch, args.tag)
    pulls = []
    for task_id in task_ids:
        ref = official_image_ref(task_id, args.namespace, args.arch, args.tag)
        pulls.append(run(["docker", "pull", ref], timeout=args.timeout, stdout_limit=12000).to_json())
    after = cache_status(task_ids, args.namespace, args.arch, args.tag)
    if args.require_clean:
        require_clean_cache(after)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "task_manifests": str(args.tasks_dir),
            "docker_images": "docker image inspect",
        },
        "before_state": before,
        "trigger": {"pulled_task_ids": task_ids, "count": len(task_ids)},
        "pulls": pulls,
        "after_state": after,
        "passes": True,
    }


def command_prune_cache(args: Any) -> dict[str, Any]:
    load_task_manifests(args.tasks_dir)
    keep_ids = expected_ids(args)
    before = cache_status(keep_ids, args.namespace, args.arch, args.tag)
    removals = []
    for ref in before["unexpected_swebench_refs"]:
        if args.yes:
            removals.append(run(["docker", "rmi", ref], timeout=args.timeout, stdout_limit=12000).to_json())
        else:
            removals.append({"dry_run": True, "image": ref})
    after = cache_status(keep_ids, args.namespace, args.arch, args.tag)
    if args.yes:
        require_clean_cache(after)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {"docker_images": "docker images --format '{{json .}}'"},
        "before_state": before,
        "trigger": {"prune": bool(args.yes), "keep_instance_ids": keep_ids},
        "removals": removals,
        "after_state": after,
        "passes": True,
    }


def ensure_smoke_image(args: Any) -> dict[str, Any]:
    before = cache_status([args.instance_id], args.namespace, args.arch, args.tag)
    unexpected = before["unexpected_swebench_refs"] or before["duplicate_instances"] or before["duplicate_image_ids"]
    if unexpected:
        require_clean_cache(before)
    if before["missing_expected_refs"]:
        ref = official_image_ref(args.instance_id, args.namespace, args.arch, args.tag)
        pull = run(["docker", "pull", ref], timeout=args.timeout, stdout_limit=12000).to_json()
        after = cache_status([args.instance_id], args.namespace, args.arch, args.tag)
        require_clean_cache(after)
        return {"before": before, "pull": pull, "after": after}
    return {"before": before, "pull": None, "after": before}


def find_gold_report(run_root: Path, run_id: str) -> Path:
    expected = run_root / f"gold.{run_id}.json"
    ensure_file(expected, "SWEBENCH_GOLD_REPORT_MISSING", "SWE-bench gold run report")
    return expected


def command_gold_smoke(args: Any) -> dict[str, Any]:
    manifests = load_task_manifests(args.tasks_dir)
    if args.instance_id not in manifests:
        raise SwebenchOpsError(
            "SWEBENCH_UNKNOWN_INSTANCE_ID",
            f"instance id is not in SWE-bench Lite manifests: {args.instance_id}",
            "Use one of the checked-in SWE-bench Lite task ids.",
            {"instance_id": args.instance_id},
        )
    python = python_bin(args.venv)
    ensure_file(python, "SWEBENCH_VENV_PYTHON_MISSING", "SWE-bench venv Python")
    image_state = ensure_smoke_image(args)
    image = official_image_ref(args.instance_id, args.namespace, args.arch, args.tag)
    run_id = args.run_id or f"contextgraph-gold-smoke-{datetime.now(timezone.utc).strftime('%Y%m%d%H%M%S')}"
    run_root = args.evidence_root / "gold-smoke" / run_id
    if run_root.exists() and any(run_root.iterdir()):
        raise SwebenchOpsError(
            "SWEBENCH_RUN_ROOT_NOT_EMPTY",
            f"gold-smoke run root already exists and is non-empty: {run_root}",
            "Use a fresh run id so stale reports cannot mask a failed run.",
            {"run_root": str(run_root)},
        )
    run_root.mkdir(parents=True, exist_ok=True)
    command = [
        str(python), "-m", "swebench.harness.run_evaluation",
        "--dataset_name", args.dataset_name,
        "--split", args.split,
        "--predictions_path", "gold",
        "--max_workers", "1",
        "--instance_ids", args.instance_id,
        "--run_id", run_id,
        "--timeout", str(args.instance_timeout),
        "--cache_level", "instance",
        "--clean", "False",
        "--namespace", args.namespace,
        "--report_dir", str(run_root),
    ]
    trigger = run(command, cwd=run_root, timeout=args.timeout)
    report_path = find_gold_report(run_root, run_id)
    report = json.loads(report_path.read_text(encoding="utf-8"))
    instance_dir = run_root / "logs" / "run_evaluation" / run_id / "gold" / args.instance_id
    instance_report_path = instance_dir / "report.json"
    run_log_path = instance_dir / "run_instance.log"
    test_output_path = instance_dir / "test_output.txt"
    patch_path = instance_dir / "patch.diff"
    for path, label in [
        (instance_report_path, "instance report"),
        (run_log_path, "instance run log"),
        (test_output_path, "test output"),
        (patch_path, "gold patch"),
    ]:
        ensure_file(path, "SWEBENCH_GOLD_ARTIFACT_MISSING", label)
    instance_report = json.loads(instance_report_path.read_text(encoding="utf-8"))
    instance_row = instance_report.get(args.instance_id)
    if not instance_row or not instance_row.get("resolved"):
        raise SwebenchOpsError(
            "SWEBENCH_GOLD_SMOKE_UNRESOLVED",
            f"official gold smoke did not resolve {args.instance_id}",
            "Inspect the official run logs and Docker resource state.",
            {"report": report, "instance_report": instance_report},
        )
    if args.instance_id not in report.get("resolved_ids", []):
        raise SwebenchOpsError(
            "SWEBENCH_GOLD_SUMMARY_MISMATCH",
            f"overall report does not list {args.instance_id} as resolved",
            "Inspect the run report and per-instance report for harness inconsistency.",
            {"report": report, "instance_report": instance_report},
        )
    after_images = cache_status([args.instance_id], args.namespace, args.arch, args.tag)
    require_clean_cache(after_images)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "official_dataset": args.dataset_name,
            "official_split": args.split,
            "official_image": image,
            "run_root": str(run_root),
            "overall_report": str(report_path),
            "instance_report": str(instance_report_path),
            "run_log": str(run_log_path),
            "test_output": str(test_output_path),
            "patch": str(patch_path),
            "docker_image_inspect": image,
        },
        "before_state": {"image_state": image_state},
        "trigger": {"command": command, "result": trigger.to_json()},
        "after_state": {
            "overall_report": {
                "completed_instances": report.get("completed_instances"),
                "resolved_instances": report.get("resolved_instances"),
                "error_instances": report.get("error_instances"),
                "incomplete_instances": report.get("incomplete_instances"),
                "resolved_ids": report.get("resolved_ids", []),
            },
            "instance_report": instance_row,
            "cache_status": after_images,
            "image_inspect": inspect_image(image),
        },
        "passes": True,
    }
