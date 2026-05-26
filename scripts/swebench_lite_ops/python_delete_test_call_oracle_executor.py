from __future__ import annotations

import fcntl
import hashlib
import json
import os
import shutil
import subprocess
import time
from collections import Counter
from pathlib import Path
from typing import Any

from .core import DEFAULT_DATASET, DEFAULT_SPLIT, SwebenchOpsError, sha256_file, tail, utc_now, write_json_checked


DEFAULT_PATCH_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-delete-test-call-regeneration-v2-patches"
)
DEFAULT_PATCHES_JSONL = DEFAULT_PATCH_ROOT / "delete_test_call_candidate_patches.jsonl"
DEFAULT_BATCH_PLAN_JSONL = DEFAULT_PATCH_ROOT / "delete_test_call_oracle_batch_plan.jsonl"
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-delete-test-call-regeneration-v3-oracle-outcomes"
)
DEFAULT_RUN_ROOT = Path(
    "/zfs/archive/contextgraph/swebench-runs/"
    "task-py-g-135c-delete-test-call-regeneration-v3-oracle-runs"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-135c-delete-test-call-oracle-executor-fsv"
)
DEFAULT_SWEBENCH_PYTHON = Path("/zfs/hot/contextgraph/venv/bin/python3")
DEFAULT_HARNESS_LOCK = Path("/zfs/archive/contextgraph/state/locks/swebench-harness.lock")
DEFAULT_MODEL_NAME = "contextgraph-delete-test-call-regeneration"
DEFAULT_TASK_ID = "TASK-PY-G-135C"


def add_arguments(parser) -> None:
    parser.add_argument("--task-id", default=DEFAULT_TASK_ID)
    parser.add_argument("--candidate-patches", type=Path, default=DEFAULT_PATCHES_JSONL)
    parser.add_argument("--batch-plan", type=Path, default=DEFAULT_BATCH_PLAN_JSONL)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--run-root", type=Path, default=DEFAULT_RUN_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--swebench-python", type=Path, default=DEFAULT_SWEBENCH_PYTHON)
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--model-name", default=DEFAULT_MODEL_NAME)
    parser.add_argument("--run-id-prefix", default="task-py-g-135c-dtc-oracle")
    parser.add_argument("--batch-index", type=int, action="append", default=None)
    parser.add_argument("--candidate-row-id", action="append", default=None)
    parser.add_argument("--max-batches", type=int, default=None)
    parser.add_argument("--max-predictions-per-batch", type=int, default=None)
    parser.add_argument("--max-workers", type=int, default=1)
    parser.add_argument("--instance-timeout", type=int, default=1800)
    parser.add_argument("--overall-timeout", type=int, default=7200)
    parser.add_argument("--harness-lock", type=Path, default=DEFAULT_HARNESS_LOCK)
    parser.add_argument("--clean-output", action="store_true")


def command_python_delete_test_call_oracle_executor(args) -> dict[str, Any]:
    validate_args(args)
    candidates = load_candidate_patches(args.candidate_patches)
    batch_plan = load_batch_plan(args.batch_plan)
    selected_batches = select_batches(
        candidates,
        batch_plan,
        batch_indexes=args.batch_index,
        candidate_row_ids=normalize_repeated_ids(args.candidate_row_id),
        max_batches=args.max_batches,
        max_predictions_per_batch=args.max_predictions_per_batch,
    )
    if not selected_batches:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_NO_SELECTED_BATCHES",
            "no oracle batches selected",
            "Select at least one batch or candidate row before running the oracle.",
            {"batch_indexes": args.batch_index, "candidate_row_ids": args.candidate_row_id},
        )
    validate_selected_candidate_patches(selected_batches, candidates)

    if args.clean_output and args.output_root.exists():
        remove_output_root_checked(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.run_root.mkdir(parents=True, exist_ok=True)

    predictions_root = args.output_root / "predictions"
    outcomes_path = args.output_root / "delete_test_call_oracle_outcomes.jsonl"
    failures_path = args.output_root / "delete_test_call_oracle_command_failures.jsonl"
    manifest_path = args.output_root / "delete_test_call_oracle_run_manifest.json"

    run_stamp = utc_now().replace("+00:00", "Z").replace(":", "").replace("-", "")
    all_outcomes: list[dict[str, Any]] = read_jsonl(outcomes_path) if outcomes_path.exists() and not args.clean_output else []
    command_failures: list[dict[str, Any]] = read_jsonl(failures_path) if failures_path.exists() and not args.clean_output else []
    loaded_existing_outcome_count = len(all_outcomes)
    existing_outcome_ids = validate_existing_outcomes(all_outcomes, outcomes_path)
    original_selected_batch_count = len(selected_batches)
    original_selected_candidate_count = sum(len(batch["candidate_row_ids"]) for batch in selected_batches)
    original_selected_candidate_ids = [candidate_id for batch in selected_batches for candidate_id in batch["candidate_row_ids"]]
    skipped_existing_batches = 0
    selected_batches, skipped_existing_batches = filter_existing_outcome_batches(
        selected_batches,
        existing_outcome_ids=existing_outcome_ids,
        candidates=candidates,
    )
    batch_summaries: list[dict[str, Any]] = []

    for ordinal, batch in enumerate(selected_batches):
        batch_outcomes, batch_summary = run_oracle_batch(
            args=args,
            batch=batch,
            candidates=candidates,
            predictions_root=predictions_root,
            run_stamp=run_stamp,
            selection_ordinal=ordinal,
        )
        batch_summaries.append(batch_summary)
        if batch_summary["harness_exit_code"] != 0:
            command_failures.append(batch_summary)
            write_jsonl_checked(outcomes_path, all_outcomes)
            write_jsonl_checked(failures_path, command_failures)
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_HARNESS_FAILED",
                f"SWE-bench harness failed for {batch_summary['batch_key']}",
                "Inspect stdout_tail/stderr_tail and Docker artifacts; do not infer oracle labels from failed infrastructure.",
                batch_summary,
            )
        all_outcomes.extend(batch_outcomes)
        existing_outcome_ids.update(row["candidate_row_id"] for row in batch_outcomes)
        write_jsonl_checked(outcomes_path, all_outcomes)
        write_jsonl_checked(failures_path, command_failures)

    write_jsonl_checked(outcomes_path, all_outcomes)
    write_jsonl_checked(failures_path, command_failures)

    by_status = Counter(row["oracle_status"] for row in all_outcomes)
    by_partition_sublabel_status = Counter(
        (
            row.get("stable_partition", "unknown"),
            row.get("assertion_duty_sublabel", "unknown"),
            row.get("oracle_status", "unknown"),
        )
        for row in all_outcomes
    )
    outcomes_by_candidate_id = {row["candidate_row_id"]: row for row in all_outcomes}
    selected_outcomes = [outcomes_by_candidate_id.get(candidate_id) for candidate_id in original_selected_candidate_ids]
    selected_outcome_rows = [row for row in selected_outcomes if row is not None]
    selected_outcomes_complete = len(selected_outcome_rows) == len(original_selected_candidate_ids)
    manifest = {
        "schema_version": 1,
        "task_id": args.task_id,
        "artifact_kind": "delete_test_call_oracle_outcome_manifest",
        "created_at": utc_now(),
        "candidate_patches": str(args.candidate_patches),
        "candidate_patches_sha256": sha256_file(args.candidate_patches),
        "batch_plan": str(args.batch_plan),
        "batch_plan_sha256": sha256_file(args.batch_plan),
        "output_root": str(args.output_root),
        "run_root": str(args.run_root),
        "swebench_python": str(args.swebench_python),
        "dataset_name": args.dataset_name,
        "split": args.split,
        "namespace": args.namespace,
        "model_name": args.model_name,
        "selected_batch_count": original_selected_batch_count,
        "pending_batch_count": len(selected_batches),
        "skipped_existing_batch_count": skipped_existing_batches,
        "attempted_candidate_count": original_selected_candidate_count,
        "pending_candidate_count": sum(len(batch["candidate_row_ids"]) for batch in selected_batches),
        "outcome_rows": len(all_outcomes),
        "selected_outcome_rows": len(selected_outcome_rows),
        "command_failure_rows": len(command_failures),
        "oracle_status_counts": dict(sorted(by_status.items())),
        "counts_by_partition_sublabel_status": counter_to_dict(by_partition_sublabel_status),
        "batch_summaries": batch_summaries,
        "outcomes_jsonl": str(outcomes_path),
        "outcomes_jsonl_sha256": sha256_file(outcomes_path),
        "failures_jsonl": str(failures_path),
        "failures_jsonl_sha256": sha256_file(failures_path),
        "executor_all_passed": selected_outcomes_complete
        and not command_failures
        and all(row["oracle_status"] not in {"missing_report", "report_invalid", "infra_error"} for row in selected_outcome_rows),
        "target_oracle_labels_used_as_features": False,
        "candidate_row_id_keyed_outcomes": True,
        "resumed_from_existing_outcomes": loaded_existing_outcome_count > 0,
    }
    manifest["all_passed"] = manifest["executor_all_passed"]
    write_json_checked(manifest_path, manifest)

    readback = {
        "manifest_exists": manifest_path.is_file(),
        "outcomes_exists": outcomes_path.is_file(),
        "failures_exists": failures_path.is_file(),
        "manifest_sha256": sha256_file(manifest_path),
        "outcomes_line_count": count_lines(outcomes_path),
        "failures_line_count": count_lines(failures_path),
        "distinct_candidate_row_ids": len({row["candidate_row_id"] for row in all_outcomes}),
        "distinct_source_task_ids": len({row["source_task_id"] for row in all_outcomes}),
        "prediction_file_count": len(list(predictions_root.glob("**/*.jsonl"))) if predictions_root.exists() else 0,
        "report_file_count": sum(1 for row in all_outcomes if row.get("report_path") and Path(row["report_path"]).is_file()),
    }
    return {
        "ok": manifest["executor_all_passed"],
        "all_passed": manifest["executor_all_passed"],
        "command": "python-delete-test-call-oracle-executor",
        "checked_at": utc_now(),
        "manifest": manifest,
        "readback": readback,
        "outcome_sample": all_outcomes[:3],
        "command_failure_sample": command_failures[:3],
    }


def validate_args(args) -> None:
    for name in ("candidate_patches", "batch_plan"):
        path = getattr(args, name)
        if not path.is_file():
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_INPUT_MISSING",
                f"{name} missing: {path}",
                "Run python-delete-test-call-regeneration before oracle execution.",
                {"field": name, "path": str(path)},
            )
    if not args.swebench_python.is_file() or not os.access(args.swebench_python, os.X_OK):
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_SWEBENCH_PYTHON_MISSING",
            f"SWE-bench Python executable missing or not executable: {args.swebench_python}",
            "Restore the SWE-bench harness virtualenv before running Docker oracle jobs.",
            {"swebench_python": str(args.swebench_python)},
        )
    for name in ("candidate_patches", "batch_plan", "output_root", "run_root", "readback_root"):
        path = getattr(args, name)
        if has_mounted_drive_fragment(path):
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_MOUNTED_PATH_REJECTED",
                f"{name} points at mounted-drive path: {path}",
                "Use aiwonder /zfs/archive/contextgraph or /zfs/hot/contextgraph roots only.",
                {"field": name, "path": str(path)},
            )
    positive_int_fields = ("max_batches", "max_predictions_per_batch", "max_workers", "instance_timeout", "overall_timeout")
    for field in positive_int_fields:
        value = getattr(args, field)
        if value is not None and value <= 0:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_BAD_POSITIVE_INT",
                f"--{field.replace('_', '-')} must be positive when set: {value}",
                "Use a positive value.",
                {"field": field, "value": value},
            )
    if args.batch_index and args.candidate_row_id:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_MIXED_SELECTION_MODES",
            "do not mix --batch-index and --candidate-row-id",
            "Use batch selection or explicit candidate selection, not both.",
            {"batch_index": args.batch_index, "candidate_row_id": args.candidate_row_id},
        )


def load_candidate_patches(path: Path) -> dict[str, dict[str, Any]]:
    rows: dict[str, dict[str, Any]] = {}
    for line_number, row in enumerate(read_jsonl(path), start=1):
        candidate_id = require_str(row, "candidate_row_id", path, line_number)
        if candidate_id in rows:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_DUPLICATE_CANDIDATE_ROW_ID",
                f"duplicate candidate_row_id in patch rows: {candidate_id}",
                "Regenerate patch artifacts with unique candidate ids.",
                {"path": str(path), "line_number": line_number, "candidate_row_id": candidate_id},
            )
        rows[candidate_id] = row
    if not rows:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_PATCH_ROWS_EMPTY",
            f"candidate patch JSONL is empty: {path}",
            "Run patch materialization before oracle execution.",
            {"path": str(path)},
        )
    return rows


def load_batch_plan(path: Path) -> list[dict[str, Any]]:
    rows = read_jsonl(path)
    if not rows:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_BATCH_PLAN_EMPTY",
            f"oracle batch plan is empty: {path}",
            "Run patch materialization before oracle execution.",
            {"path": str(path)},
        )
    return rows


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SwebenchOpsError(
                    "DELETE_TEST_CALL_ORACLE_BAD_JSONL",
                    f"invalid JSONL at {path}:{line_number}",
                    "Regenerate the malformed artifact before oracle execution.",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            if not isinstance(row, dict):
                raise SwebenchOpsError(
                    "DELETE_TEST_CALL_ORACLE_JSONL_ROW_NOT_OBJECT",
                    f"JSONL row is not an object at {path}:{line_number}",
                    "Regenerate the malformed artifact before oracle execution.",
                    {"path": str(path), "line_number": line_number},
                )
            rows.append(row)
    return rows


def normalize_repeated_ids(values: list[str] | None) -> list[str] | None:
    if not values:
        return None
    ids: list[str] = []
    for value in values:
        ids.extend(part.strip() for part in value.split(",") if part.strip())
    return ids


def select_batches(
    candidates: dict[str, dict[str, Any]],
    batch_plan: list[dict[str, Any]],
    *,
    batch_indexes: list[int] | None,
    candidate_row_ids: list[str] | None,
    max_batches: int | None,
    max_predictions_per_batch: int | None,
) -> list[dict[str, Any]]:
    if candidate_row_ids:
        missing = [candidate_id for candidate_id in candidate_row_ids if candidate_id not in candidates]
        if missing:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_CANDIDATE_ID_NOT_FOUND",
                f"candidate ids missing from patch rows: {missing[:5]}",
                "Select candidate_row_id values from delete_test_call_candidate_patches.jsonl.",
                {"missing_candidate_row_ids": missing[:20]},
            )
        task_ids = [require_candidate_field(candidates[candidate_id], "source_task_id") for candidate_id in candidate_row_ids]
        ensure_unique_task_ids(task_ids, "explicit-candidate-selection")
        return [
            {
                "schema_version": 1,
                "artifact_kind": "delete_test_call_oracle_batch",
                "batch_index": -1,
                "batch_key": "delete-test-call-regeneration-selected-candidates",
                "prediction_count": len(candidate_row_ids),
                "source_task_ids": task_ids,
                "candidate_row_ids": candidate_row_ids,
                "patch_paths": [require_candidate_field(candidates[candidate_id], "patch_path") for candidate_id in candidate_row_ids],
            }
        ]

    selected = list(batch_plan)
    if batch_indexes:
        wanted = set(batch_indexes)
        selected = [row for row in selected if int(row.get("batch_index", -10**9)) in wanted]
        found = {int(row.get("batch_index", -10**9)) for row in selected}
        missing_indexes = sorted(wanted - found)
        if missing_indexes:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_BATCH_INDEX_NOT_FOUND",
                f"batch indexes missing from plan: {missing_indexes}",
                "Select existing batch indexes from delete_test_call_oracle_batch_plan.jsonl.",
                {"missing_batch_indexes": missing_indexes},
            )
    selected.sort(key=lambda row: int(row.get("batch_index", 0)))
    if max_batches is not None:
        selected = selected[:max_batches]

    normalized: list[dict[str, Any]] = []
    for row in selected:
        candidate_ids = list(row.get("candidate_row_ids") or [])
        if max_predictions_per_batch is not None:
            candidate_ids = candidate_ids[:max_predictions_per_batch]
        if not candidate_ids:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_BATCH_EMPTY_AFTER_FILTER",
                f"batch {row.get('batch_key')} contains no candidates after filtering",
                "Use a larger --max-predictions-per-batch or select a non-empty batch.",
                {"batch": row},
            )
        missing = [candidate_id for candidate_id in candidate_ids if candidate_id not in candidates]
        if missing:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_BATCH_CANDIDATE_MISSING",
                f"batch references missing candidate ids: {missing[:5]}",
                "Regenerate patch artifacts and batch plan from the same materializer run.",
                {"batch_key": row.get("batch_key"), "missing_candidate_row_ids": missing[:20]},
            )
        task_ids = [require_candidate_field(candidates[candidate_id], "source_task_id") for candidate_id in candidate_ids]
        ensure_unique_task_ids(task_ids, str(row.get("batch_key")))
        out = dict(row)
        out["candidate_row_ids"] = candidate_ids
        out["source_task_ids"] = task_ids
        out["patch_paths"] = [require_candidate_field(candidates[candidate_id], "patch_path") for candidate_id in candidate_ids]
        out["prediction_count"] = len(candidate_ids)
        normalized.append(out)
    return normalized


def validate_existing_outcomes(rows: list[dict[str, Any]], path: Path) -> set[str]:
    candidate_ids: set[str] = set()
    for line_number, row in enumerate(rows, start=1):
        candidate_id = row.get("candidate_row_id")
        if not isinstance(candidate_id, str) or not candidate_id:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_EXISTING_OUTCOME_ID_MISSING",
                f"existing outcome row is missing candidate_row_id at {path}:{line_number}",
                "Do not resume from malformed oracle outcomes; move the bad artifact aside and rerun.",
                {"path": str(path), "line_number": line_number},
            )
        if candidate_id in candidate_ids:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_ORACLE_EXISTING_OUTCOME_DUPLICATE_ID",
                f"duplicate candidate_row_id in existing outcomes: {candidate_id}",
                "Do not resume from ambiguous oracle outcomes; move the bad artifact aside and rerun.",
                {"path": str(path), "line_number": line_number, "candidate_row_id": candidate_id},
            )
        candidate_ids.add(candidate_id)
    return candidate_ids


def filter_existing_outcome_batches(
    selected_batches: list[dict[str, Any]],
    *,
    existing_outcome_ids: set[str],
    candidates: dict[str, dict[str, Any]],
) -> tuple[list[dict[str, Any]], int]:
    if not existing_outcome_ids:
        return selected_batches, 0

    pending_batches: list[dict[str, Any]] = []
    skipped_batches = 0
    for batch in selected_batches:
        candidate_ids = [candidate_id for candidate_id in batch["candidate_row_ids"] if candidate_id not in existing_outcome_ids]
        if not candidate_ids:
            skipped_batches += 1
            continue
        if len(candidate_ids) == len(batch["candidate_row_ids"]):
            pending_batches.append(batch)
            continue

        task_ids = [require_candidate_field(candidates[candidate_id], "source_task_id") for candidate_id in candidate_ids]
        ensure_unique_task_ids(task_ids, f"{batch.get('batch_key')}-resume-pending")
        pending_batch = dict(batch)
        pending_batch["candidate_row_ids"] = candidate_ids
        pending_batch["source_task_ids"] = task_ids
        pending_batch["patch_paths"] = [
            require_candidate_field(candidates[candidate_id], "patch_path") for candidate_id in candidate_ids
        ]
        pending_batch["prediction_count"] = len(candidate_ids)
        pending_batches.append(pending_batch)
    return pending_batches, skipped_batches


def run_oracle_batch(
    *,
    args,
    batch: dict[str, Any],
    candidates: dict[str, dict[str, Any]],
    predictions_root: Path,
    run_stamp: str,
    selection_ordinal: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    batch_index = int(batch.get("batch_index", selection_ordinal))
    batch_key = str(batch.get("batch_key") or f"batch-{selection_ordinal:04d}")
    candidate_ids = list(batch["candidate_row_ids"])
    prediction_dir_name = f"batch-{batch_index:04d}" if batch_index >= 0 else f"selected-{selection_ordinal:04d}"
    prediction_dir = predictions_root / prediction_dir_name
    prediction_dir.mkdir(parents=True, exist_ok=True)
    run_id = safe_run_id(f"{args.run_id_prefix}-{run_stamp}-{prediction_dir_name}")
    predictions_path = prediction_dir / f"predictions-{run_id}.jsonl"

    predictions: list[dict[str, Any]] = []
    candidate_rows: list[dict[str, Any]] = []
    for candidate_id in candidate_ids:
        row = candidates[candidate_id]
        validate_candidate_patch(row)
        patch_text = Path(require_candidate_field(row, "patch_path")).read_text(encoding="utf-8")
        predictions.append(
            {
                "instance_id": require_candidate_field(row, "source_task_id"),
                "model_name_or_path": args.model_name,
                "model_patch": patch_text,
            }
        )
        candidate_rows.append(row)
    write_jsonl_checked(predictions_path, predictions)
    predictions_sha = sha256_file(predictions_path)
    instance_ids = [prediction["instance_id"] for prediction in predictions]
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
        "--instance_ids",
        *instance_ids,
        "--run_id",
        run_id,
        "--timeout",
        str(args.instance_timeout),
        "--cache_level",
        "instance",
        "--clean",
        "False",
        "--namespace",
        args.namespace,
        "--report_dir",
        str(args.run_root),
    ]

    result = run_with_harness_lock(command, args.run_root, args.overall_timeout, args.harness_lock, run_id, batch_key)
    batch_summary = {
        "batch_index": batch_index,
        "batch_key": batch_key,
        "candidate_row_ids": candidate_ids,
        "source_task_ids": instance_ids,
        "run_id": run_id,
        "prediction_count": len(predictions),
        "predictions_jsonl": str(predictions_path),
        "predictions_jsonl_sha256": predictions_sha,
        "harness_exit_code": result["exit_code"],
        "harness_duration_ms": result["duration_ms"],
        "harness_command": command,
        "stdout_tail": result["stdout_tail"],
        "stderr_tail": result["stderr_tail"],
    }
    if result["exit_code"] != 0:
        return [], batch_summary

    outcomes = [
        outcome_from_report(
            candidate=row,
            args=args,
            batch_summary=batch_summary,
            predictions_sha=predictions_sha,
        )
        for row in candidate_rows
    ]
    return outcomes, batch_summary


def run_with_harness_lock(
    command: list[str],
    cwd: Path,
    timeout: int,
    lock_path: Path,
    run_id: str,
    batch_key: str,
) -> dict[str, Any]:
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+", encoding="utf-8") as lock_file:
        fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
        lock_file.seek(0)
        lock_file.truncate()
        lock_file.write(f"pid={os.getpid()} run_id={run_id} batch_key={batch_key}\n")
        lock_file.flush()
        os.fsync(lock_file.fileno())
        started = time.monotonic()
        try:
            completed = subprocess.run(
                command,
                cwd=str(cwd),
                text=True,
                capture_output=True,
                timeout=timeout,
                check=False,
            )
            exit_code = completed.returncode
            stdout_text = completed.stdout
            stderr_text = completed.stderr
        except subprocess.TimeoutExpired as exc:
            exit_code = 124
            stdout_text = tail(exc.stdout)
            stderr_text = tail(exc.stderr) + f"\nContextGraph overall timeout after {timeout}s"
        duration_ms = int((time.monotonic() - started) * 1000)
        return {
            "exit_code": exit_code,
            "duration_ms": duration_ms,
            "stdout_tail": tail(stdout_text),
            "stderr_tail": tail(stderr_text),
        }


def outcome_from_report(
    *,
    candidate: dict[str, Any],
    args,
    batch_summary: dict[str, Any],
    predictions_sha: str,
) -> dict[str, Any]:
    instance_id = require_candidate_field(candidate, "source_task_id")
    instance_dir = (
        args.run_root
        / "logs"
        / "run_evaluation"
        / batch_summary["run_id"]
        / args.model_name.replace("/", "__")
        / instance_id
    )
    report_path = instance_dir / "report.json"
    test_output_path = instance_dir / "test_output.txt"
    report_text, synthetic_timeout = read_or_create_timeout_report(
        instance_id,
        report_path,
        test_output_path,
        args.instance_timeout,
    )
    report = parse_instance_report(instance_id, report_text, report_path)
    body = report[instance_id]
    tests_summary = summarize_tests_status(body.get("tests_status") or {})
    patch_applied = bool(body.get("patch_successfully_applied"))
    resolved = bool(body.get("resolved"))
    if body.get("patch_is_None") or not body.get("patch_exists"):
        oracle_status = "empty_or_missing_patch"
    elif not patch_applied:
        oracle_status = "patch_not_applied"
    elif synthetic_timeout or body.get("contextgraph_timeout_synthetic"):
        oracle_status = "timeout"
    elif resolved:
        oracle_status = "resolved"
    else:
        oracle_status = "unresolved"

    return {
        "schema_version": 1,
        "artifact_kind": "delete_test_call_oracle_outcome",
        "candidate_row_id": require_candidate_field(candidate, "candidate_row_id"),
        "source_task_id": instance_id,
        "repo": candidate.get("repo"),
        "stable_partition": candidate.get("stable_partition"),
        "mutation_category": candidate.get("mutation_category"),
        "assertion_duty_sublabel": candidate.get("assertion_duty_sublabel"),
        "source_path": candidate.get("source_path"),
        "patch_path": require_candidate_field(candidate, "patch_path"),
        "patch_sha256": require_candidate_field(candidate, "patch_sha256"),
        "prediction_jsonl": batch_summary["predictions_jsonl"],
        "prediction_jsonl_sha256": predictions_sha,
        "run_id": batch_summary["run_id"],
        "batch_index": batch_summary["batch_index"],
        "batch_key": batch_summary["batch_key"],
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "test_output_path": str(test_output_path),
        "test_output_sha256": sha256_file(test_output_path) if test_output_path.is_file() else None,
        "patch_successfully_applied": patch_applied,
        "patch_exists": bool(body.get("patch_exists")),
        "patch_is_none": bool(body.get("patch_is_None")),
        "resolved": resolved,
        "oracle_all_passed": resolved,
        "oracle_status": oracle_status,
        "timeout_synthetic_report": synthetic_timeout or bool(body.get("contextgraph_timeout_synthetic")),
        "tests_status_summary": tests_summary,
        "harness_duration_ms": batch_summary["harness_duration_ms"],
        "harness_exit_code": batch_summary["harness_exit_code"],
        "candidate_row_id_keyed": True,
        "target_oracle_label_used_as_feature": False,
    }


def read_or_create_timeout_report(
    instance_id: str,
    report_path: Path,
    test_output_path: Path,
    timeout_seconds: int,
) -> tuple[str, bool]:
    if report_path.is_file():
        return report_path.read_text(encoding="utf-8"), False
    if not test_output_path.is_file():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_REPORT_MISSING",
            f"report.json missing and no test_output.txt for {instance_id}",
            "Inspect run_instance.log; the harness did not produce a classifiable oracle artifact.",
            {"instance_id": instance_id, "report_path": str(report_path), "test_output_path": str(test_output_path)},
        )
    test_output = test_output_path.read_text(encoding="utf-8", errors="replace")
    if "Timeout error:" not in test_output:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_REPORT_MISSING",
            f"report.json missing and test_output.txt has no timeout marker for {instance_id}",
            "Inspect run_instance.log; do not infer a code verdict from missing report artifacts.",
            {"instance_id": instance_id, "report_path": str(report_path), "test_output_path": str(test_output_path)},
        )
    timeout_test_id = f"{instance_id}::__swebench_timeout__"
    report = {
        instance_id: {
            "patch_is_None": False,
            "patch_exists": True,
            "patch_successfully_applied": True,
            "resolved": False,
            "tests_status": {
                "FAIL_TO_PASS": {"success": [], "failure": [timeout_test_id]},
                "PASS_TO_PASS": {"success": [], "failure": []},
                "FAIL_TO_FAIL": {"success": [], "failure": []},
                "PASS_TO_FAIL": {"success": [], "failure": []},
            },
            "contextgraph_timeout_synthetic": True,
            "contextgraph_timeout_seconds": timeout_seconds,
        }
    }
    write_json_checked(report_path, report)
    return report_path.read_text(encoding="utf-8"), True


def parse_instance_report(instance_id: str, report_text: str, report_path: Path) -> dict[str, Any]:
    try:
        report = json.loads(report_text)
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_REPORT_BAD_JSON",
            f"report.json is invalid JSON for {instance_id}: {report_path}",
            "Preserve the bad artifact and fix the harness/root cause before consuming outcomes.",
            {"instance_id": instance_id, "report_path": str(report_path), "error": str(exc)},
        ) from exc
    if not isinstance(report, dict) or sorted(report.keys()) != [instance_id]:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_REPORT_INSTANCE_MISMATCH",
            f"report root must contain exactly {instance_id}: {report_path}",
            "Read the per-instance report under the matching run/model/instance directory.",
            {"instance_id": instance_id, "report_path": str(report_path), "report_keys": sorted(report.keys()) if isinstance(report, dict) else None},
        )
    body = report[instance_id]
    if not isinstance(body, dict):
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_REPORT_BODY_INVALID",
            f"report body is not an object for {instance_id}: {report_path}",
            "Preserve the bad artifact and fix the harness/root cause before consuming outcomes.",
            {"instance_id": instance_id, "report_path": str(report_path)},
        )
    return report


def summarize_tests_status(tests_status: dict[str, Any]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    total_success = 0
    total_failure = 0
    for transition in ("FAIL_TO_PASS", "PASS_TO_PASS", "FAIL_TO_FAIL", "PASS_TO_FAIL"):
        block = tests_status.get(transition) or {}
        success = list(block.get("success") or [])
        failure = list(block.get("failure") or [])
        out[transition] = {"success": len(success), "failure": len(failure)}
        total_success += len(success)
        total_failure += len(failure)
    out["total_success"] = total_success
    out["total_failure"] = total_failure
    out["total_tests"] = total_success + total_failure
    return out


def validate_candidate_patch(row: dict[str, Any]) -> None:
    patch_path = Path(require_candidate_field(row, "patch_path"))
    if has_mounted_drive_fragment(patch_path):
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_PATCH_MOUNTED_PATH_REJECTED",
            f"patch path is mounted-drive path: {patch_path}",
            "Use aiwonder patch artifacts only.",
            {"candidate_row_id": row.get("candidate_row_id"), "patch_path": str(patch_path)},
        )
    if not patch_path.is_file():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_PATCH_FILE_MISSING",
            f"patch file missing: {patch_path}",
            "Regenerate #615 patch artifacts before oracle execution.",
            {"candidate_row_id": row.get("candidate_row_id"), "patch_path": str(patch_path)},
        )
    expected_sha = require_candidate_field(row, "patch_sha256")
    actual_sha = sha256_file(patch_path)
    if actual_sha != expected_sha:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_PATCH_SHA_MISMATCH",
            f"patch sha mismatch for {row.get('candidate_row_id')}",
            "Do not run oracle on modified patch bytes; regenerate patch artifacts.",
            {
                "candidate_row_id": row.get("candidate_row_id"),
                "patch_path": str(patch_path),
                "expected": expected_sha,
                "actual": actual_sha,
            },
        )
    if patch_path.stat().st_size <= 0:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_EMPTY_PATCH_FILE",
            f"patch file is empty: {patch_path}",
            "Regenerate the patch artifact; empty patch rows cannot produce labels.",
            {"candidate_row_id": row.get("candidate_row_id"), "patch_path": str(patch_path)},
        )


def validate_selected_candidate_patches(
    selected_batches: list[dict[str, Any]],
    candidates: dict[str, dict[str, Any]],
) -> None:
    for batch in selected_batches:
        for candidate_id in batch["candidate_row_ids"]:
            validate_candidate_patch(candidates[candidate_id])


def ensure_unique_task_ids(task_ids: list[str], batch_key: str) -> None:
    duplicates = sorted(task_id for task_id, count in Counter(task_ids).items() if count > 1)
    if duplicates:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_BATCH_DUPLICATE_TASK",
            f"oracle batch contains duplicate source_task_id values: {duplicates[:5]}",
            "SWE-bench accepts one prediction per instance; recolor or split the batch before oracle execution.",
            {"batch_key": batch_key, "duplicate_source_task_ids": duplicates[:20]},
        )


def require_str(row: dict[str, Any], key: str, path: Path, line_number: int) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_REQUIRED_FIELD_MISSING",
            f"required string field missing: {key}",
            "Regenerate the malformed oracle input artifact.",
            {"path": str(path), "line_number": line_number, "key": key},
        )
    return value


def require_candidate_field(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_REQUIRED_CANDIDATE_FIELD_MISSING",
            f"candidate field missing: {key}",
            "Regenerate #615 patch artifacts before oracle execution.",
            {"candidate_row_id": row.get("candidate_row_id"), "key": key},
        )
    return value


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_TEXT_READBACK_MISMATCH",
            f"text readback mismatch at {path}",
            "Investigate the filesystem for partial writes or concurrent writers.",
            {"path": str(path)},
        )


def write_jsonl_checked(path: Path, rows: list[dict[str, Any]]) -> None:
    text = "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows)
    write_text_checked(path, text)


def count_lines(path: Path) -> int:
    if not path.exists():
        return 0
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for _ in handle)


def counter_to_dict(counter: Counter) -> dict[str, int]:
    return {"|".join(map(str, key if isinstance(key, tuple) else (key,))): value for key, value in sorted(counter.items())}


def remove_output_root_checked(path: Path) -> None:
    text = str(path)
    if not text.startswith("/zfs/archive/contextgraph/") or "delete-test-call" not in text:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORACLE_UNSAFE_CLEAN_OUTPUT",
            f"refusing to delete output root: {path}",
            "Use an aiwonder delete-test-call output root for --clean-output.",
            {"output_root": str(path)},
        )
    shutil.rmtree(path)


def safe_run_id(value: str) -> str:
    allowed = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-")
    return "".join(ch if ch in allowed else "-" for ch in value)


def has_mounted_drive_fragment(path: Path) -> bool:
    text = str(path)
    return "/mnt/d" in text or "/mnt/c" in text
