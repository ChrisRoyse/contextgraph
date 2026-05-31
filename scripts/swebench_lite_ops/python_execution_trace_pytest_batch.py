#!/usr/bin/env python3
"""Batch supported command-family execution-trace generation for INV-010 / #774."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
from argparse import Namespace
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Iterable

from .python_execution_trace_smoke import (
    DEFAULT_CORPUS_ROOT,
    DEFAULT_DATASET,
    DEFAULT_SPLIT,
    DEFAULT_VENV_PYTHON,
    ExecutionTraceSmokeError,
    run_trace_smoke,
)


SCHEMA_VERSION = 1
TASK_ID = "INV-010"
ARTIFACT_KIND = "execution_trace_pytest_batch"
PROGRESS_ARTIFACT_KIND = "execution_trace_pytest_batch_progress"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
AIWONDER_HOT_ROOT = "/zfs/hot/contextgraph/"
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/inv-010-execution-aware-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/execution-traces/inv-010-v1")
DEFAULT_RUNNABLE_ROWS = Path(
    "/zfs/archive/contextgraph/fsv/inv-010-execution-aware-fsv/"
    "run-20260530T191928Z-trace-plan/trace-plan/runnable_current_pytest_rows.jsonl"
)
SUPPORTED_COMMAND_FAMILY = "pytest_direct"
SUPPORTED_STATUS = "supported_current"
SUPPORTED_HANDLER = "pytest_sys_settrace_v1"
COMMAND_FAMILY_CONFIGS: dict[str, dict[str, str]] = {
    "pytest_direct": {
        "artifact_kind": "execution_trace_pytest_batch",
        "bounded_reason": "bounded_subset_run_not_full_direct_pytest_batch",
        "complete_key": "direct_pytest_batch_complete",
        "complete_reason": "direct_pytest_trace_generation_complete_for_supported_rows",
        "failure_reason": "one_or_more_direct_pytest_trace_rows_failed",
        "file_prefix": "direct_pytest",
        "fraction_key": "direct_pytest_trace_fraction_of_corpus",
        "generated_key": "direct_pytest_trace_rows_generated",
        "handler": "pytest_sys_settrace_v1",
        "incomplete_reason": "direct_pytest_trace_generation_incomplete",
        "live_reason": "live_sys_settrace_artifacts_generated_for_direct_pytest_rows",
        "progress_artifact_kind": "execution_trace_pytest_batch_progress",
        "run_suffix": "pytest-batch",
    },
    "django_runtests": {
        "artifact_kind": "execution_trace_django_runtests_batch",
        "bounded_reason": "bounded_subset_run_not_full_django_runtests_batch",
        "complete_key": "django_runtests_batch_complete",
        "complete_reason": "django_runtests_trace_generation_complete_for_supported_rows",
        "failure_reason": "one_or_more_django_runtests_trace_rows_failed",
        "file_prefix": "django_runtests",
        "fraction_key": "django_runtests_trace_fraction_of_corpus",
        "generated_key": "django_runtests_trace_rows_generated",
        "handler": "python_script_sys_settrace_v1",
        "incomplete_reason": "django_runtests_trace_generation_incomplete",
        "live_reason": "live_sys_settrace_artifacts_generated_for_django_runtests_rows",
        "progress_artifact_kind": "execution_trace_django_runtests_batch_progress",
        "run_suffix": "django-runtests-batch",
    },
}
TARGET_SIDE_KEYS = {
    "actual_oracle_pass",
    "completed_ids",
    "completed_instances",
    "error_ids",
    "error_instances",
    "oracle_all_passed",
    "oracle_exception",
    "oracle_verdict_sha256",
    "resolved_ids",
    "resolved_instances",
    "unresolved_ids",
    "unresolved_instances",
}


class ExecutionTracePytestBatchError(RuntimeError):
    pass


TraceRunner = Callable[[Namespace], dict[str, Any]]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise ExecutionTracePytestBatchError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "EXECUTION_TRACE_PYTEST_BATCH_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


def require_hot_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_HOT_ROOT),
        "EXECUTION_TRACE_PYTEST_BATCH_NON_AIWONDER_HOT_PATH",
        {label: str(path), "required_prefix": AIWONDER_HOT_ROOT},
    )


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
        raise ExecutionTracePytestBatchError(f"{code}: invalid JSON at {path}: {exc}") from exc


def read_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    require(path.is_file(), code, {"path": str(path)})
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ExecutionTracePytestBatchError(
                    f"{code}: invalid JSONL line {line_no} at {path}: {exc}"
                ) from exc
            require(
                isinstance(row, dict),
                "EXECUTION_TRACE_PYTEST_BATCH_ROW_NOT_OBJECT",
                {"path": str(path), "line": line_no},
            )
            rows.append(row)
    require(rows, "EXECUTION_TRACE_PYTEST_BATCH_ROWS_EMPTY", {"path": str(path)})
    return rows


def sanitize_part(value: str) -> str:
    cleaned = re.sub(r"[^A-Za-z0-9_.-]+", "_", value).strip("._-")
    return cleaned[:120] or "unknown"


def command_family_config(command_family: str | None) -> dict[str, str]:
    family = command_family or SUPPORTED_COMMAND_FAMILY
    require(
        family in COMMAND_FAMILY_CONFIGS,
        "EXECUTION_TRACE_PYTEST_BATCH_UNSUPPORTED_BATCH_COMMAND_FAMILY",
        {"command_family": family, "supported": sorted(COMMAND_FAMILY_CONFIGS)},
    )
    config = dict(COMMAND_FAMILY_CONFIGS[family])
    config["command_family"] = family
    return config


def args_command_family_config(args: argparse.Namespace) -> dict[str, str]:
    return command_family_config(str(getattr(args, "command_family", SUPPORTED_COMMAND_FAMILY) or SUPPORTED_COMMAND_FAMILY))


def progress_basename(config: dict[str, str], suffix: str) -> str:
    return f"{config['file_prefix']}_trace_batch_progress.{suffix}"


def load_runnable_rows(path: Path, command_family: str | None = None) -> list[dict[str, Any]]:
    config = command_family_config(command_family)
    rows = read_jsonl(path, "EXECUTION_TRACE_PYTEST_BATCH_RUNNABLE_ROWS_MISSING")
    out: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for idx, row in enumerate(rows):
        forbidden = sorted(set(row) & TARGET_SIDE_KEYS)
        require(
            not forbidden,
            "EXECUTION_TRACE_PYTEST_BATCH_TARGET_SIDE_FIELD_IN_ROW_PLAN",
            {"line_index": idx, "forbidden_keys": forbidden},
        )
        task_id = str(row.get("task_id") or "")
        category = str(row.get("category") or "")
        require(task_id, "EXECUTION_TRACE_PYTEST_BATCH_TASK_ID_MISSING", {"line_index": idx})
        require(category, "EXECUTION_TRACE_PYTEST_BATCH_CATEGORY_MISSING", {"line_index": idx, "task_id": task_id})
        require(
            str(row.get("command_family") or "") == config["command_family"],
            "EXECUTION_TRACE_PYTEST_BATCH_UNSUPPORTED_COMMAND_FAMILY",
            {
                "task_id": task_id,
                "category": category,
                "command_family": row.get("command_family"),
                "expected_command_family": config["command_family"],
            },
        )
        require(
            str(row.get("support_status") or "") == SUPPORTED_STATUS,
            "EXECUTION_TRACE_PYTEST_BATCH_UNSUPPORTED_STATUS",
            {"task_id": task_id, "category": category, "support_status": row.get("support_status")},
        )
        require(
            str(row.get("handler") or "") == config["handler"],
            "EXECUTION_TRACE_PYTEST_BATCH_UNSUPPORTED_HANDLER",
            {
                "task_id": task_id,
                "category": category,
                "handler": row.get("handler"),
                "expected_handler": config["handler"],
            },
        )
        key = (task_id, category)
        require(
            key not in seen,
            "EXECUTION_TRACE_PYTEST_BATCH_DUPLICATE_ROW",
            {"task_id": task_id, "category": category},
        )
        seen.add(key)
        out.append(
            {
                "schema_version": SCHEMA_VERSION,
                "source_line_index": idx,
                "row_index": int(row.get("row_index") if row.get("row_index") is not None else idx),
                "task_id": task_id,
                "repo": str(row.get("repo") or ""),
                "category": category,
                "command_family": config["command_family"],
                "support_status": SUPPORTED_STATUS,
                "handler": config["handler"],
            }
        )
    return out


def select_rows(
    rows: list[dict[str, Any]],
    *,
    offset: int,
    limit: int | None,
    categories: list[str] | None,
    task_ids: list[str] | None,
) -> list[dict[str, Any]]:
    require(offset >= 0, "EXECUTION_TRACE_PYTEST_BATCH_NEGATIVE_OFFSET", {"offset": offset})
    if limit is not None:
        require(limit > 0, "EXECUTION_TRACE_PYTEST_BATCH_NON_POSITIVE_LIMIT", {"limit": limit})
    category_filter = set(categories or [])
    task_filter = set(task_ids or [])
    selected = [
        row
        for row in rows
        if (not category_filter or row["category"] in category_filter)
        and (not task_filter or row["task_id"] in task_filter)
    ]
    selected = selected[offset:]
    if limit is not None:
        selected = selected[:limit]
    require(
        selected,
        "EXECUTION_TRACE_PYTEST_BATCH_SELECTED_ROWS_EMPTY",
        {"offset": offset, "limit": limit, "categories": sorted(category_filter), "task_ids": sorted(task_filter)},
    )
    return selected


def row_run_id(row: dict[str, Any]) -> str:
    return f"row-{int(row['row_index']):04d}-{sanitize_part(row['task_id'])}-{sanitize_part(row['category'])}"


def row_key(row: dict[str, Any] | None) -> dict[str, Any] | None:
    if row is None:
        return None
    return {
        "task_id": str(row.get("task_id") or ""),
        "category": str(row.get("category") or ""),
        "row_index": int(row.get("row_index") or 0),
        "source_line_index": int(row.get("source_line_index") or 0),
        "row_run_id": str(row.get("row_run_id") or row_run_id(row)),
    }


def copy_trace(src: Path, dest: Path, code: str) -> str:
    require(src.is_file(), code, {"src": str(src)})
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".tmp")
    shutil.copyfile(src, tmp)
    tmp.replace(dest)
    return sha256_file(dest)


def run_one_row(
    *,
    row: dict[str, Any],
    args: argparse.Namespace,
    fsv_run_root: Path,
    state_run_root: Path,
    trace_runner: TraceRunner,
) -> dict[str, Any]:
    config = args_command_family_config(args)
    child_run_id = row_run_id(row)
    smoke_args = Namespace(
        dataset_name=args.dataset_name,
        split=args.split,
        corpus_root=Path(args.corpus_root),
        output_root=fsv_run_root / "row-smokes",
        state_root=state_run_root / "row-smokes",
        venv_python=Path(args.venv_python),
        task_id=row["task_id"],
        category=row["category"],
        run_id=child_run_id,
        namespace=args.namespace,
        arch=args.arch,
        tag=args.tag,
        instance_timeout=args.instance_timeout,
        max_events=args.max_events,
        command_family=config["command_family"],
        allow_non_aiwonder_roots=bool(args.allow_non_aiwonder_roots),
    )
    result = trace_runner(smoke_args)
    state_src = Path(str(result.get("state_trace") or ""))
    fsv_src = Path(str(result.get("fsv_trace") or ""))
    trace_rel = Path(row["task_id"]) / f"{row['category']}.execution-trace.json"
    state_dest = state_run_root / "generated-traces" / trace_rel
    fsv_dest = fsv_run_root / "generated-traces" / trace_rel
    state_sha = copy_trace(
        state_src,
        state_dest,
        "EXECUTION_TRACE_PYTEST_BATCH_STATE_TRACE_MISSING_AFTER_SMOKE",
    )
    fsv_sha = copy_trace(
        fsv_src,
        fsv_dest,
        "EXECUTION_TRACE_PYTEST_BATCH_FSV_TRACE_MISSING_AFTER_SMOKE",
    )
    require(
        state_sha == fsv_sha,
        "EXECUTION_TRACE_PYTEST_BATCH_TRACE_HASH_MISMATCH",
        {"state_trace_sha256": state_sha, "fsv_trace_sha256": fsv_sha},
    )
    return {
        **row,
        "status": "succeeded",
        "row_run_id": child_run_id,
        "state_trace": str(state_dest),
        "fsv_trace": str(fsv_dest),
        "state_trace_sha256": state_sha,
        "fsv_trace_sha256": fsv_sha,
        "source_state_trace": str(state_src),
        "source_fsv_trace": str(fsv_src),
        "trace_summary": result.get("trace_summary") or {},
    }


def result_artifacts(row: dict[str, Any] | None) -> dict[str, Any] | None:
    if row is None:
        return None
    if row.get("status") == "succeeded":
        return {
            "state_trace": row.get("state_trace"),
            "fsv_trace": row.get("fsv_trace"),
            "state_trace_sha256": row.get("state_trace_sha256"),
            "fsv_trace_sha256": row.get("fsv_trace_sha256"),
            "trace_summary": row.get("trace_summary") or {},
        }
    if row.get("status") == "failed":
        return {"error": row.get("error")}
    return None


def build_progress_snapshot(
    *,
    args: argparse.Namespace,
    run_id: str,
    fsv_run_root: Path,
    state_run_root: Path,
    selected_rows: list[dict[str, Any]],
    row_results: list[dict[str, Any]],
    phase: str,
    progress_sequence: int,
    current_row: dict[str, Any] | None,
    last_completed_row: dict[str, Any] | None,
) -> dict[str, Any]:
    config = args_command_family_config(args)
    successes = [row for row in row_results if row.get("status") == "succeeded"]
    failures = [row for row in row_results if row.get("status") == "failed"]
    attempted = len(row_results)
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": config["progress_artifact_kind"],
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "command_family": config["command_family"],
        "handler": config["handler"],
        "phase": phase,
        "progress_sequence": progress_sequence,
        "counts": {
            "selected_rows": len(selected_rows),
            "attempted_rows": attempted,
            "succeeded_rows": len(successes),
            "failed_rows": len(failures),
            "remaining_rows": max(len(selected_rows) - attempted, 0),
        },
        "current_row_key": row_key(current_row),
        "last_completed_row_key": row_key(last_completed_row),
        "last_completed_status": last_completed_row.get("status") if last_completed_row else None,
        "last_completed_artifacts": result_artifacts(last_completed_row),
        "selection": {
            "offset": args.offset,
            "limit": args.limit,
            "categories": sorted(args.categories or []),
            "task_ids": sorted(args.task_ids or []),
        },
        "artifact_paths": {
            "fsv_progress": str(fsv_run_root / progress_basename(config, "json")),
            "state_progress": str(state_run_root / progress_basename(config, "json")),
            "fsv_progress_history": str(fsv_run_root / progress_basename(config, "jsonl")),
            "state_progress_history": str(state_run_root / progress_basename(config, "jsonl")),
            "generated_trace_fsv_root": str(fsv_run_root / "generated-traces"),
            "generated_trace_state_root": str(state_run_root / "generated-traces"),
        },
    }


def write_progress_artifacts(
    *,
    args: argparse.Namespace,
    run_id: str,
    fsv_run_root: Path,
    state_run_root: Path,
    selected_rows: list[dict[str, Any]],
    row_results: list[dict[str, Any]],
    progress_history: list[dict[str, Any]],
    phase: str,
    current_row: dict[str, Any] | None = None,
    last_completed_row: dict[str, Any] | None = None,
) -> dict[str, Path]:
    config = args_command_family_config(args)
    snapshot = build_progress_snapshot(
        args=args,
        run_id=run_id,
        fsv_run_root=fsv_run_root,
        state_run_root=state_run_root,
        selected_rows=selected_rows,
        row_results=row_results,
        phase=phase,
        progress_sequence=len(progress_history),
        current_row=current_row,
        last_completed_row=last_completed_row,
    )
    progress_history.append(snapshot)
    fsv_progress_path = fsv_run_root / progress_basename(config, "json")
    state_progress_path = state_run_root / fsv_progress_path.name
    fsv_history_path = fsv_run_root / progress_basename(config, "jsonl")
    state_history_path = state_run_root / fsv_history_path.name
    write_json(fsv_progress_path, snapshot)
    write_json(state_progress_path, snapshot)
    write_jsonl(fsv_history_path, progress_history)
    write_jsonl(state_history_path, progress_history)
    return {
        "fsv_progress": fsv_progress_path,
        "state_progress": state_progress_path,
        "fsv_history": fsv_history_path,
        "state_history": state_history_path,
    }


def write_artifact_hashes(root: Path, paths: list[Path]) -> Path:
    rows = []
    for path in sorted(paths):
        if path.is_file():
            rows.append({"path": str(path), "sha256": sha256_file(path), "bytes": path.stat().st_size})
    out = root / "artifact_hashes.json"
    write_json(out, {"artifact_count": len(rows), "artifacts": rows})
    return out


def build_manifest(
    *,
    args: argparse.Namespace,
    run_id: str,
    fsv_run_root: Path,
    state_run_root: Path,
    runnable_rows: list[dict[str, Any]],
    selected_rows: list[dict[str, Any]],
    row_results: list[dict[str, Any]],
    selected_rows_path: Path,
    batch_rows_path: Path,
    trace_index_path: Path,
    failed_rows_path: Path,
    progress_path: Path,
    progress_history_path: Path,
) -> dict[str, Any]:
    config = args_command_family_config(args)
    successes = [row for row in row_results if row.get("status") == "succeeded"]
    failures = [row for row in row_results if row.get("status") == "failed"]
    category_success = Counter(row["category"] for row in successes)
    category_selected = Counter(row["category"] for row in selected_rows)
    expected_corpus_rows = int(args.expected_corpus_rows)
    selected_all = len(selected_rows) == len(runnable_rows) and not args.categories and not args.task_ids and args.limit is None and args.offset == 0
    complete = selected_all and len(successes) == len(runnable_rows) and not failures
    reasons: list[str] = []
    if failures:
        reasons.append(config["failure_reason"])
    if not selected_all:
        reasons.append(config["bounded_reason"])
    if not complete:
        reasons.append(config["incomplete_reason"])
    if successes:
        reasons.append(config["live_reason"])
    trace_fraction = round(len(successes) / expected_corpus_rows, 10) if expected_corpus_rows else 0.0
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": config["artifact_kind"],
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "inputs": {
            "dataset_name": args.dataset_name,
            "split": args.split,
            "corpus_root": str(args.corpus_root),
            "runnable_rows": str(args.runnable_rows),
            "runnable_row_count": len(runnable_rows),
            "expected_runnable_rows": args.expected_runnable_rows,
            "expected_corpus_rows": expected_corpus_rows,
            "command_family": config["command_family"],
            "handler": config["handler"],
            "selection": {
                "offset": args.offset,
                "limit": args.limit,
                "categories": sorted(args.categories or []),
                "task_ids": sorted(args.task_ids or []),
            },
        },
        "execution": {
            "fsv_run_root": str(fsv_run_root),
            "state_run_root": str(state_run_root),
            "namespace": args.namespace,
            "arch": args.arch,
            "tag": args.tag,
            "instance_timeout": args.instance_timeout,
            "max_events": args.max_events,
            "continue_on_error": bool(args.continue_on_error),
        },
        "summary": {
            "selected_rows": len(selected_rows),
            "attempted_rows": len(row_results),
            "succeeded_rows": len(successes),
            "failed_rows": len(failures),
            "trace_rows_generated": len(successes),
            "trace_fraction_of_corpus": trace_fraction,
            config["generated_key"]: len(successes),
            config["fraction_key"]: trace_fraction,
            "selected_rows_by_cell": dict(sorted(category_selected.items())),
            "succeeded_rows_by_cell": dict(sorted(category_success.items())),
        },
        "decision": {
            "batch_complete": complete,
            config["complete_key"]: complete,
            "trace_generation_required": True,
            "readiness_rerun_required": bool(successes),
            "per_cell_oracle_correlation_lift_measured": False,
            "promotion_allowed": False,
            "canonical_issue": "#774",
            "decision_reasons": reasons or [config["complete_reason"]],
        },
        "doctrine_policy": {
            "measurement_only": True,
            "target_labels_used_as_live_inputs": False,
            "oracle_verdicts_classified_target_side_only": True,
            "flat_vector_concat_used": False,
            "slot_identity_preserved": True,
            "predictor_weights_changed": False,
            "inner_llm_used": False,
            "aiwonder_durable_root_required": not bool(args.allow_non_aiwonder_roots),
        },
        "artifacts": {
            "selected_rows": str(selected_rows_path),
            "batch_rows": str(batch_rows_path),
            "generated_trace_index": str(trace_index_path),
            "failed_rows": str(failed_rows_path),
            "progress": str(progress_path),
            "progress_history": str(progress_history_path),
            "generated_trace_fsv_root": str(fsv_run_root / "generated-traces"),
            "generated_trace_state_root": str(state_run_root / "generated-traces"),
        },
    }


def run_batch(args: argparse.Namespace, trace_runner: TraceRunner = run_trace_smoke) -> dict[str, Any]:
    config = args_command_family_config(args)
    allow_non_aiwonder = bool(args.allow_non_aiwonder_roots)
    corpus_root = Path(args.corpus_root)
    runnable_rows_path = Path(args.runnable_rows)
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    venv_python = Path(args.venv_python)
    require_archive_path(corpus_root, "corpus_root", allow_non_aiwonder)
    require_archive_path(runnable_rows_path, "runnable_rows", allow_non_aiwonder)
    require_archive_path(output_root, "output_root", allow_non_aiwonder)
    require_archive_path(state_root, "state_root", allow_non_aiwonder)
    require_hot_path(venv_python, "venv_python", allow_non_aiwonder)
    if not allow_non_aiwonder:
        require(venv_python.is_file(), "EXECUTION_TRACE_PYTEST_BATCH_VENV_PYTHON_MISSING", {"venv_python": str(venv_python)})

    runnable_rows = load_runnable_rows(runnable_rows_path, command_family=config["command_family"])
    if args.expected_runnable_rows is not None:
        require(
            len(runnable_rows) == int(args.expected_runnable_rows),
            "EXECUTION_TRACE_PYTEST_BATCH_RUNNABLE_ROW_COUNT_MISMATCH",
            {"expected": int(args.expected_runnable_rows), "actual": len(runnable_rows)},
        )
    selected_rows = select_rows(
        runnable_rows,
        offset=int(args.offset),
        limit=args.limit,
        categories=args.categories,
        task_ids=args.task_ids,
    )

    run_id = str(args.run_id or f"run-{utc_stamp()}-{config['run_suffix']}")
    fsv_run_root = output_root / run_id / config["run_suffix"]
    state_run_root = state_root / run_id / config["run_suffix"]
    require(
        not fsv_run_root.exists() or not any(fsv_run_root.iterdir()),
        "EXECUTION_TRACE_PYTEST_BATCH_OUTPUT_ROOT_NOT_EMPTY",
        {"fsv_run_root": str(fsv_run_root)},
    )
    require(
        not state_run_root.exists() or not any(state_run_root.iterdir()),
        "EXECUTION_TRACE_PYTEST_BATCH_STATE_ROOT_NOT_EMPTY",
        {"state_run_root": str(state_run_root)},
    )
    fsv_run_root.mkdir(parents=True, exist_ok=True)
    state_run_root.mkdir(parents=True, exist_ok=True)

    row_results: list[dict[str, Any]] = []
    progress_history: list[dict[str, Any]] = []
    progress_paths = write_progress_artifacts(
        args=args,
        run_id=run_id,
        fsv_run_root=fsv_run_root,
        state_run_root=state_run_root,
        selected_rows=selected_rows,
        row_results=row_results,
        progress_history=progress_history,
        phase="selected",
    )
    for row in selected_rows:
        write_progress_artifacts(
            args=args,
            run_id=run_id,
            fsv_run_root=fsv_run_root,
            state_run_root=state_run_root,
            selected_rows=selected_rows,
            row_results=row_results,
            progress_history=progress_history,
            phase="row_started",
            current_row=row,
        )
        try:
            row_results.append(
                run_one_row(
                    row=row,
                    args=args,
                    fsv_run_root=fsv_run_root,
                    state_run_root=state_run_root,
                    trace_runner=trace_runner,
                )
            )
        except (ExecutionTracePytestBatchError, ExecutionTraceSmokeError, OSError, RuntimeError) as exc:
            failed = {
                **row,
                "status": "failed",
                "row_run_id": row_run_id(row),
                "error": f"{type(exc).__name__}: {exc}",
            }
            row_results.append(failed)
        write_progress_artifacts(
            args=args,
            run_id=run_id,
            fsv_run_root=fsv_run_root,
            state_run_root=state_run_root,
            selected_rows=selected_rows,
            row_results=row_results,
            progress_history=progress_history,
            phase="row_completed",
            last_completed_row=row_results[-1],
        )
        if row_results[-1].get("status") == "failed" and not args.continue_on_error:
            break

    progress_paths = write_progress_artifacts(
        args=args,
        run_id=run_id,
        fsv_run_root=fsv_run_root,
        state_run_root=state_run_root,
        selected_rows=selected_rows,
        row_results=row_results,
        progress_history=progress_history,
        phase="batch_finalizing",
        last_completed_row=row_results[-1] if row_results else None,
    )

    selected_rows_path = fsv_run_root / f"selected_{config['file_prefix']}_rows.jsonl"
    batch_rows_path = fsv_run_root / f"{config['file_prefix']}_trace_batch_rows.jsonl"
    trace_index_path = fsv_run_root / "generated_trace_index.jsonl"
    failed_rows_path = fsv_run_root / f"failed_{config['file_prefix']}_rows.jsonl"
    successes = [row for row in row_results if row.get("status") == "succeeded"]
    failures = [row for row in row_results if row.get("status") == "failed"]
    write_jsonl(selected_rows_path, selected_rows)
    write_jsonl(batch_rows_path, row_results)
    write_jsonl(trace_index_path, successes)
    write_jsonl(failed_rows_path, failures)

    state_selected_rows_path = state_run_root / selected_rows_path.name
    state_batch_rows_path = state_run_root / batch_rows_path.name
    state_trace_index_path = state_run_root / trace_index_path.name
    state_failed_rows_path = state_run_root / failed_rows_path.name
    write_jsonl(state_selected_rows_path, selected_rows)
    write_jsonl(state_batch_rows_path, row_results)
    write_jsonl(state_trace_index_path, successes)
    write_jsonl(state_failed_rows_path, failures)

    manifest = build_manifest(
        args=args,
        run_id=run_id,
        fsv_run_root=fsv_run_root,
        state_run_root=state_run_root,
        runnable_rows=runnable_rows,
        selected_rows=selected_rows,
        row_results=row_results,
        selected_rows_path=selected_rows_path,
        batch_rows_path=batch_rows_path,
        trace_index_path=trace_index_path,
        failed_rows_path=failed_rows_path,
        progress_path=progress_paths["fsv_progress"],
        progress_history_path=progress_paths["fsv_history"],
    )
    manifest_path = fsv_run_root / f"{config['file_prefix']}_trace_batch_manifest.json"
    state_manifest_path = state_run_root / manifest_path.name
    write_json(manifest_path, manifest)
    write_json(state_manifest_path, manifest)

    hash_inputs = [
        manifest_path,
        selected_rows_path,
        batch_rows_path,
        trace_index_path,
        failed_rows_path,
        progress_paths["fsv_progress"],
        progress_paths["fsv_history"],
        state_manifest_path,
        state_selected_rows_path,
        state_batch_rows_path,
        state_trace_index_path,
        state_failed_rows_path,
        progress_paths["state_progress"],
        progress_paths["state_history"],
        *[Path(row["state_trace"]) for row in successes],
        *[Path(row["fsv_trace"]) for row in successes],
    ]
    hash_path = write_artifact_hashes(fsv_run_root, hash_inputs)
    state_hash_path = state_run_root / hash_path.name
    write_json(state_hash_path, read_json(hash_path, "EXECUTION_TRACE_PYTEST_BATCH_HASH_JSON_INVALID"))

    result = {
        "ok": not failures,
        "run_id": run_id,
        "command_family": config["command_family"],
        "handler": config["handler"],
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "manifest": str(manifest_path),
        "state_manifest": str(state_manifest_path),
        "batch_rows": str(batch_rows_path),
        "generated_trace_index": str(trace_index_path),
        "failed_rows": str(failed_rows_path),
        "progress": str(progress_paths["fsv_progress"]),
        "progress_history": str(progress_paths["fsv_history"]),
        "artifact_hashes": str(hash_path),
        "summary": manifest["summary"],
        "decision": manifest["decision"],
    }
    if failures and not args.continue_on_error:
        raise ExecutionTracePytestBatchError(
            "EXECUTION_TRACE_PYTEST_BATCH_STOPPED_AFTER_ROW_FAILURE: "
            + canonical_json({"manifest": str(manifest_path), "failed_rows": str(failed_rows_path)})
        )
    return result


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--runnable-rows", type=Path, default=DEFAULT_RUNNABLE_ROWS)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--venv-python", type=Path, default=DEFAULT_VENV_PYTHON)
    parser.add_argument(
        "--command-family",
        choices=sorted(COMMAND_FAMILY_CONFIGS),
        default=SUPPORTED_COMMAND_FAMILY,
        help="Supported trace command family to batch; pytest_direct remains the default.",
    )
    parser.add_argument("--run-id")
    parser.add_argument("--expected-corpus-rows", type=int, default=2400)
    parser.add_argument("--expected-runnable-rows", type=int)
    parser.add_argument("--offset", type=int, default=0)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--category", dest="categories", action="append")
    parser.add_argument("--task-id", dest="task_ids", action="append")
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--arch", default="x86_64")
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--instance-timeout", type=int, default=1800)
    parser.add_argument("--max-events", type=int, default=5000)
    parser.add_argument("--continue-on-error", action="store_true")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    try:
        result = run_batch(args)
    except ExecutionTracePytestBatchError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, indent=2, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["ok"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
