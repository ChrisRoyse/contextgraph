#!/usr/bin/env python3
"""Measure #774 execution-trace feature utility without promoting a predictor."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

try:
    from .python_per_cell_smooth_ece import metric_block
except ImportError:  # pragma: no cover - direct script execution on aiwonder.
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from swebench_lite_ops.python_per_cell_smooth_ece import metric_block  # type: ignore


SCHEMA_VERSION = 1
TASK_ID = "INV-010"
ARTIFACT_KIND = "execution_aware_trace_lift_measurement"
FORMULA_VERSION = "execution_aware_trace_lift_v1"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_TRACE_ROOT = Path("/zfs/archive/contextgraph/state/execution-traces/inv-010-v1")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/inv-010-execution-aware-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/execution-aware-audit/inv-010-v1")
CONTESTED_MIXED_CELLS = ("off_by_one", "subtle_flip", "swap_variable")
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
DIRECT_OUTCOME_KEYS_EXCLUDED_FROM_FEATURES = {
    "captured_exception",
    "execution_outcome",
    "pytest_exit_code",
    "pytest_args",
}
FEATURE_NAMES = (
    "log_events_recorded",
    "log_events_seen",
    "log_lines_executed",
    "log_files_touched",
    "log_elapsed_ms",
    "log_variable_states",
    "log_unique_trace_files",
    "log_unique_trace_functions",
    "trace_truncated",
    "line_event_fraction",
    "call_event_fraction",
    "return_event_fraction",
    "exception_event_fraction",
    "line_coverage_density",
    "variable_state_fraction",
)


class ExecutionAwareLiftMeasurementError(RuntimeError):
    pass


@dataclass(frozen=True)
class PredictionRow:
    split: str
    row_index: int
    task_id: str
    repo: str
    category: str
    label: int
    base_score: float

    @property
    def key(self) -> tuple[str, str]:
        return (self.task_id, self.category)


@dataclass(frozen=True)
class TraceFeature:
    task_id: str
    category: str
    path: str
    sha256: str
    producer: str
    features: dict[str, float]
    feature_source_summary: dict[str, Any]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise ExecutionAwareLiftMeasurementError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "EXECUTION_AWARE_LIFT_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


def reject_mounted_drive(label: str, path: Path) -> None:
    text = str(path)
    require(
        not (text.startswith("/mnt/c/") or text.startswith("/mnt/d/")),
        "EXECUTION_AWARE_LIFT_MOUNTED_DRIVE_FORBIDDEN",
        {label: text},
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
        raise ExecutionAwareLiftMeasurementError(f"{code}: invalid JSON at {path}: {exc}") from exc


def read_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    require(path.is_file(), code, {"path": str(path)})
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ExecutionAwareLiftMeasurementError(
                    f"{code}: invalid JSONL line {line_no} at {path}: {exc}"
                ) from exc
            require(isinstance(value, dict), "EXECUTION_AWARE_LIFT_JSONL_ROW_NOT_OBJECT", {"path": str(path), "line": line_no})
            rows.append(value)
    return rows


def load_ship_gate_rows(path: Path, expected_calibration: int, expected_holdout: int) -> tuple[list[PredictionRow], list[PredictionRow], dict[str, str]]:
    payload = read_json(path, "EXECUTION_AWARE_LIFT_SHIP_GATE_REPORT_MISSING")
    execute = payload.get("execute_and_inspect")
    require(isinstance(execute, dict), "EXECUTION_AWARE_LIFT_SHIP_GATE_EXECUTE_MISSING")
    calibration_key, calibration_rows = load_prediction_rows(
        execute,
        split="calibration",
        expected=expected_calibration,
        preferred=("calibration_predictions", "calibration_raw_predictions", "calibration_rank_lift_predictions"),
    )
    holdout_key, holdout_rows = load_prediction_rows(
        execute,
        split="holdout",
        expected=expected_holdout,
        preferred=("holdout_predictions", "holdout_raw_predictions", "holdout_rank_lift_predictions"),
    )
    return calibration_rows, holdout_rows, {"calibration": calibration_key, "holdout": holdout_key}


def load_prediction_rows(
    execute: dict[str, Any], *, split: str, expected: int, preferred: tuple[str, ...]
) -> tuple[str, list[PredictionRow]]:
    key = next((candidate for candidate in preferred if isinstance(execute.get(candidate), list)), None)
    require(key is not None, "EXECUTION_AWARE_LIFT_PREDICTION_ROWS_MISSING", {"split": split, "preferred": preferred})
    raw_rows = execute[key]  # type: ignore[index]
    require(len(raw_rows) == expected, "EXECUTION_AWARE_LIFT_SPLIT_COUNT_MISMATCH", {"split": split, "expected": expected, "actual": len(raw_rows)})
    rows: list[PredictionRow] = []
    for index, value in enumerate(raw_rows):
        require(isinstance(value, dict), "EXECUTION_AWARE_LIFT_PREDICTION_ROW_NOT_OBJECT", {"split": split, "index": index})
        task_id = required_string(value, "task_id", split, index)
        category = required_string(value, "category", split, index)
        repo = required_string(value, "repo", split, index)
        if "patch_path" in value:
            reject_mounted_drive("patch_path", Path(str(value["patch_path"])))
        actual = value.get("actual_oracle_pass")
        require(isinstance(actual, bool), "EXECUTION_AWARE_LIFT_ACTUAL_LABEL_REQUIRED", {"split": split, "index": index, "task_id": task_id})
        score = required_score(value, "predicted_oracle_pass", split, index)
        rows.append(PredictionRow(split=split, row_index=index, task_id=task_id, repo=repo, category=category, label=1 if actual else 0, base_score=score))
    seen: set[tuple[str, str]] = set()
    for row in rows:
        require(row.key not in seen, "EXECUTION_AWARE_LIFT_DUPLICATE_SPLIT_ROW", {"split": split, "task_id": row.task_id, "category": row.category})
        seen.add(row.key)
    return key, rows


def required_string(row: dict[str, Any], key: str, split: str, index: int) -> str:
    value = row.get(key)
    require(isinstance(value, str) and value, "EXECUTION_AWARE_LIFT_STRING_REQUIRED", {"key": key, "split": split, "index": index})
    return value


def required_score(row: dict[str, Any], key: str, split: str, index: int) -> float:
    value = row.get(key)
    require(isinstance(value, (int, float)) and math.isfinite(float(value)), "EXECUTION_AWARE_LIFT_SCORE_REQUIRED", {"key": key, "split": split, "index": index})
    score = float(value)
    require(0.0 <= score <= 1.0, "EXECUTION_AWARE_LIFT_SCORE_OUT_OF_RANGE", {"key": key, "score": score})
    return score


def load_corpus_entries(corpus_root: Path, expected_rows: int | None) -> list[dict[str, Any]]:
    doc = read_json(corpus_root / "index.json", "EXECUTION_AWARE_LIFT_CORPUS_INDEX_MISSING")
    entries = doc.get("entries")
    require(isinstance(entries, list), "EXECUTION_AWARE_LIFT_CORPUS_INDEX_BAD_SCHEMA")
    if expected_rows is not None:
        require(len(entries) == expected_rows, "EXECUTION_AWARE_LIFT_CORPUS_ROW_COUNT_MISMATCH", {"expected": expected_rows, "actual": len(entries)})
    out: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for index, value in enumerate(entries):
        require(isinstance(value, dict), "EXECUTION_AWARE_LIFT_CORPUS_ROW_NOT_OBJECT", {"index": index})
        task_id = str(value.get("task_id") or "")
        repo = str(value.get("repo") or "")
        category = str(value.get("category") or "")
        require(task_id, "EXECUTION_AWARE_LIFT_CORPUS_TASK_ID_MISSING", {"index": index})
        require(repo, "EXECUTION_AWARE_LIFT_CORPUS_REPO_MISSING", {"index": index, "task_id": task_id})
        require(category, "EXECUTION_AWARE_LIFT_CORPUS_CATEGORY_MISSING", {"index": index, "task_id": task_id})
        key = (task_id, category)
        require(key not in seen, "EXECUTION_AWARE_LIFT_CORPUS_DUPLICATE_ROW", {"task_id": task_id, "category": category})
        seen.add(key)
        out.append({"row_index": index, "task_id": task_id, "repo": repo, "category": category})
    return out


def discover_trace_features(trace_root: Path) -> tuple[dict[tuple[str, str], TraceFeature], dict[str, Any]]:
    require(trace_root.is_dir(), "EXECUTION_AWARE_LIFT_TRACE_ROOT_MISSING", {"trace_root": str(trace_root)})
    candidates: dict[tuple[str, str], list[TraceFeature]] = defaultdict(list)
    scanned_files = 0
    trace_json_files = 0
    duplicate_keys = 0
    direct_outcome_key_counts: Counter[str] = Counter()
    for path in sorted(trace_root.rglob("*.json")):
        if not path.is_file() or "trace" not in path.name.lower():
            continue
        scanned_files += 1
        payload = read_json(path, "EXECUTION_AWARE_LIFT_TRACE_JSON_INVALID")
        if not isinstance(payload, dict):
            continue
        if not has_trace_feature_shape(payload):
            continue
        trace_json_files += 1
        forbidden_hits = sorted(find_forbidden_keys(payload, TARGET_SIDE_KEYS))
        require(
            not forbidden_hits,
            "EXECUTION_AWARE_LIFT_TARGET_SIDE_KEY_IN_TRACE_PAYLOAD",
            {"path": str(path), "forbidden_keys": forbidden_hits[:20]},
        )
        for key in find_forbidden_keys(payload, DIRECT_OUTCOME_KEYS_EXCLUDED_FROM_FEATURES):
            direct_outcome_key_counts[key] += 1
        task_id = str(payload.get("task_id") or infer_task_id_from_path(path))
        category = str(payload.get("category") or infer_category_from_path(path))
        require(task_id, "EXECUTION_AWARE_LIFT_TRACE_TASK_ID_MISSING", {"path": str(path)})
        require(category, "EXECUTION_AWARE_LIFT_TRACE_CATEGORY_MISSING", {"path": str(path)})
        features, source_summary = extract_trace_features(payload)
        candidates[(task_id, category)].append(
            TraceFeature(
                task_id=task_id,
                category=category,
                path=str(path),
                sha256=sha256_file(path),
                producer=str(payload.get("producer") or "unknown"),
                features=features,
                feature_source_summary=source_summary,
            )
        )
    selected: dict[tuple[str, str], TraceFeature] = {}
    for key, rows in sorted(candidates.items()):
        if len(rows) > 1:
            duplicate_keys += 1
        selected[key] = sorted(rows, key=lambda item: item.path)[-1]
    return selected, {
        "trace_root": str(trace_root),
        "scanned_json_files_with_trace_in_name": scanned_files,
        "eligible_trace_payload_files": trace_json_files,
        "unique_trace_keys": len(selected),
        "duplicate_trace_keys": duplicate_keys,
        "duplicate_resolution": "lexicographically_last_path",
        "direct_outcome_keys_seen_but_excluded_from_feature_vector": dict(sorted(direct_outcome_key_counts.items())),
    }


def has_trace_feature_shape(payload: dict[str, Any]) -> bool:
    return any(key in payload for key in ("coverage", "line_coverage", "runtime_metrics", "steps", "trace_events", "variable_states"))


def find_forbidden_keys(value: Any, forbidden: set[str], *, max_list_items: int = 10) -> set[str]:
    hits: set[str] = set()

    def visit(item: Any) -> None:
        if isinstance(item, dict):
            for key, child in item.items():
                if key in forbidden:
                    hits.add(key)
                visit(child)
        elif isinstance(item, list):
            for child in item[:max_list_items]:
                visit(child)

    visit(value)
    return hits


def infer_task_id_from_path(path: Path) -> str:
    for parent in reversed(path.parts):
        if "__" in parent:
            return parent
    return ""


def infer_category_from_path(path: Path) -> str:
    name = path.name
    for suffix in (".execution-trace.json", "-trace.json", ".json"):
        if name.endswith(suffix):
            return name[: -len(suffix)]
    return ""


def extract_trace_features(payload: dict[str, Any]) -> tuple[dict[str, float], dict[str, Any]]:
    coverage = payload.get("coverage") if isinstance(payload.get("coverage"), dict) else {}
    runtime = payload.get("runtime_metrics") if isinstance(payload.get("runtime_metrics"), dict) else {}
    steps = payload.get("steps") if isinstance(payload.get("steps"), dict) else {}
    event_counts = steps.get("event_counts") if isinstance(steps.get("event_counts"), dict) else {}
    trace_events = payload.get("trace_events") if isinstance(payload.get("trace_events"), list) else []
    line_coverage = payload.get("line_coverage") if isinstance(payload.get("line_coverage"), list) else []
    variable_states = payload.get("variable_states") if isinstance(payload.get("variable_states"), list) else []
    events_recorded = non_negative_number(coverage.get("events_recorded"), len(trace_events))
    events_seen = non_negative_number(coverage.get("events_seen"), sum(non_negative_number(v, 0.0) for v in event_counts.values()) or events_recorded)
    lines_executed = non_negative_number(coverage.get("lines_executed"), len(line_coverage))
    files_touched = non_negative_number(coverage.get("files_touched"), unique_file_count(trace_events))
    elapsed_ms = non_negative_number(runtime.get("elapsed_ms"), 0.0)
    variable_state_count = float(len(variable_states))
    unique_trace_files = unique_file_count(trace_events)
    unique_trace_functions = unique_function_count(trace_events)
    event_total = sum(non_negative_number(v, 0.0) for v in event_counts.values())
    if event_total <= 0.0:
        event_total = events_seen if events_seen > 0 else max(1.0, events_recorded)
    line_count = non_negative_number(event_counts.get("line"), 0.0)
    call_count = non_negative_number(event_counts.get("call"), 0.0)
    return_count = non_negative_number(event_counts.get("return"), 0.0)
    exception_count = non_negative_number(event_counts.get("exception"), 0.0)
    truncated = bool(coverage.get("truncated")) or bool(runtime.get("trace_stopped_at_max_events"))
    features = {
        "log_events_recorded": math.log1p(events_recorded),
        "log_events_seen": math.log1p(events_seen),
        "log_lines_executed": math.log1p(lines_executed),
        "log_files_touched": math.log1p(files_touched),
        "log_elapsed_ms": math.log1p(elapsed_ms),
        "log_variable_states": math.log1p(variable_state_count),
        "log_unique_trace_files": math.log1p(unique_trace_files),
        "log_unique_trace_functions": math.log1p(unique_trace_functions),
        "trace_truncated": 1.0 if truncated else 0.0,
        "line_event_fraction": line_count / event_total,
        "call_event_fraction": call_count / event_total,
        "return_event_fraction": return_count / event_total,
        "exception_event_fraction": exception_count / event_total,
        "line_coverage_density": lines_executed / max(1.0, files_touched),
        "variable_state_fraction": variable_state_count / max(1.0, events_recorded),
    }
    for name in FEATURE_NAMES:
        require(name in features and math.isfinite(features[name]), "EXECUTION_AWARE_LIFT_FEATURE_NONFINITE", {"feature": name, "value": features.get(name)})
    return features, {
        "events_recorded": int(events_recorded),
        "events_seen": int(events_seen),
        "lines_executed": int(lines_executed),
        "files_touched": int(files_touched),
        "variable_states": int(variable_state_count),
        "unique_trace_files": int(unique_trace_files),
        "unique_trace_functions": int(unique_trace_functions),
        "truncated": truncated,
        "direct_outcome_fields_excluded": sorted(DIRECT_OUTCOME_KEYS_EXCLUDED_FROM_FEATURES),
    }


def non_negative_number(value: Any, default: float) -> float:
    if isinstance(value, bool):
        return 1.0 if value else 0.0
    if isinstance(value, (int, float)) and math.isfinite(float(value)):
        return max(0.0, float(value))
    return float(default)


def unique_file_count(events: list[Any]) -> float:
    files = {str(event.get("file") or "") for event in events if isinstance(event, dict) and event.get("file")}
    return float(len(files))


def unique_function_count(events: list[Any]) -> float:
    functions = {str(event.get("function") or "") for event in events if isinstance(event, dict) and event.get("function")}
    return float(len(functions))


def build_feature_rows(
    rows: list[PredictionRow],
    traces: dict[tuple[str, str], TraceFeature],
    *,
    include_labels: bool,
) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for row in rows:
        trace = traces.get(row.key)
        require(trace is not None, "EXECUTION_AWARE_LIFT_SPLIT_TRACE_MISSING", {"split": row.split, "task_id": row.task_id, "category": row.category})
        payload = {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "execution_aware_trace_feature_row",
            "formula_version": FORMULA_VERSION,
            "split": row.split,
            "row_index": row.row_index,
            "task_id": row.task_id,
            "repo": row.repo,
            "category": row.category,
            "base_predicted_oracle_pass": row.base_score,
            "trace_path": trace.path,
            "trace_sha256": trace.sha256,
            "trace_producer": trace.producer,
            "features": trace.features,
            "feature_source_summary": trace.feature_source_summary,
        }
        if include_labels:
            payload["actual_oracle_pass"] = bool(row.label)
        out.append(payload)
    return out


def fit_and_measure(
    calibration: list[PredictionRow],
    holdout: list[PredictionRow],
    traces: dict[tuple[str, str], TraceFeature],
    *,
    ridge_alpha: float,
    min_lift: float,
) -> dict[str, Any]:
    import numpy as np

    cal_trace = [traces[row.key] for row in calibration]
    hold_trace = [traces[row.key] for row in holdout]
    means, stds = feature_normalization(cal_trace)
    y_cal = np.asarray([row.label for row in calibration], dtype=np.float64)
    y_hold = np.asarray([row.label for row in holdout], dtype=np.float64)
    base_cal = np.asarray([row.base_score for row in calibration], dtype=np.float64)
    base_hold = np.asarray([row.base_score for row in holdout], dtype=np.float64)
    X_static_cal = np.column_stack([np.ones(len(calibration)), [logit(value) for value in base_cal]])
    X_static_hold = np.column_stack([np.ones(len(holdout)), [logit(value) for value in base_hold]])
    X_trace_cal = np.column_stack([X_static_cal, feature_matrix(cal_trace, means, stds)])
    X_trace_hold = np.column_stack([X_static_hold, feature_matrix(hold_trace, means, stds)])
    static_coeff = fit_ridge(X_static_cal, y_cal, ridge_alpha)
    trace_coeff = fit_ridge(X_trace_cal, y_cal, ridge_alpha)
    static_hold_scores = clip_scores(X_static_hold @ static_coeff)
    trace_hold_scores = clip_scores(X_trace_hold @ trace_coeff)
    raw_hold_scores = clip_scores(base_hold)
    global_metrics = {
        "raw_static_baseline": metric_block(y_hold.astype(int).tolist(), raw_hold_scores.tolist()),
        "static_calibrated_baseline": metric_block(y_hold.astype(int).tolist(), static_hold_scores.tolist()),
        "trace_augmented": metric_block(y_hold.astype(int).tolist(), trace_hold_scores.tolist()),
    }
    global_delta = metric_delta(global_metrics["static_calibrated_baseline"], global_metrics["trace_augmented"])
    global_raw_delta = metric_delta(global_metrics["raw_static_baseline"], global_metrics["trace_augmented"])
    per_cell_rows = per_cell_lift_rows(holdout, raw_hold_scores.tolist(), static_hold_scores.tolist(), trace_hold_scores.tolist())
    by_cell = {row["mutation_category"]: row for row in per_cell_rows}
    promotion_blockers: list[str] = []
    for cell in CONTESTED_MIXED_CELLS:
        cell_delta = by_cell.get(cell, {}).get("delta_vs_static_calibrated", {}).get("correlation_delta")
        if not isinstance(cell_delta, (int, float)):
            promotion_blockers.append(f"{cell}:correlation_delta_unavailable")
        elif cell_delta < min_lift:
            promotion_blockers.append(f"{cell}:correlation_delta_below_{min_lift:g}")
    global_corr_delta = global_delta.get("correlation_delta")
    if not isinstance(global_corr_delta, (int, float)) or global_corr_delta < min_lift:
        promotion_blockers.append(f"global:correlation_delta_below_{min_lift:g}")
    global_brier_delta = global_delta.get("brier_delta")
    if isinstance(global_brier_delta, (int, float)) and global_brier_delta > 1.0e-9:
        promotion_blockers.append("global_brier_regressed_vs_static_calibrated")
    return {
        "feature_normalization": {
            "fit_partition": "calibration",
            "feature_names": list(FEATURE_NAMES),
            "means": means,
            "stds": stds,
        },
        "fit_partition": {
            "calibration_rows_used": len(calibration),
            "holdout_rows_used_for_fitting": 0,
            "ridge_alpha": ridge_alpha,
            "static_model": coefficient_payload(static_coeff, ["intercept", "base_logit"]),
            "trace_model": coefficient_payload(trace_coeff, ["intercept", "base_logit", *FEATURE_NAMES]),
        },
        "report_partition": {
            "holdout_rows": len(holdout),
            "global_metrics": global_metrics,
            "global_delta_vs_static_calibrated": global_delta,
            "global_delta_vs_raw_static": global_raw_delta,
            "per_cell_rows": per_cell_rows,
        },
        "decision": {
            "proxy_feature_lift_measured": True,
            "primary_delta": "trace_augmented_vs_static_calibrated",
            "min_required_correlation_lift": min_lift,
            "required_cells": list(CONTESTED_MIXED_CELLS),
            "promotion_allowed": not promotion_blockers,
            "promotion_blockers": promotion_blockers,
        },
    }


def feature_normalization(traces: list[TraceFeature]) -> tuple[dict[str, float], dict[str, float]]:
    means: dict[str, float] = {}
    stds: dict[str, float] = {}
    for name in FEATURE_NAMES:
        values = [trace.features[name] for trace in traces]
        mean = sum(values) / len(values)
        variance = sum((value - mean) ** 2 for value in values) / len(values)
        std = math.sqrt(variance)
        means[name] = mean
        stds[name] = std if std > 1.0e-12 else 1.0
    return means, stds


def feature_matrix(traces: list[TraceFeature], means: dict[str, float], stds: dict[str, float]) -> Any:
    import numpy as np

    return np.asarray(
        [[(trace.features[name] - means[name]) / stds[name] for name in FEATURE_NAMES] for trace in traces],
        dtype=np.float64,
    )


def fit_ridge(X: Any, y: Any, alpha: float) -> Any:
    import numpy as np

    XtX = X.T @ X
    penalty = np.eye(XtX.shape[0], dtype=np.float64) * float(alpha)
    penalty[0, 0] = 0.0
    return np.linalg.solve(XtX + penalty, X.T @ y)


def clip_scores(values: Any) -> Any:
    import numpy as np

    return np.clip(np.asarray(values, dtype=np.float64), 0.001, 0.999)


def logit(value: float) -> float:
    p = max(0.001, min(0.999, float(value)))
    return math.log(p / (1.0 - p))


def coefficient_payload(coefficients: Any, names: list[str]) -> list[dict[str, Any]]:
    return [
        {"feature": name, "coefficient": float(round(float(value), 12))}
        for name, value in zip(names, coefficients.tolist())
    ]


def per_cell_lift_rows(
    holdout: list[PredictionRow],
    raw_scores: list[float],
    static_scores: list[float],
    trace_scores: list[float],
) -> list[dict[str, Any]]:
    by_cell: dict[str, list[int]] = defaultdict(list)
    for idx, row in enumerate(holdout):
        by_cell[row.category].append(idx)
    rows: list[dict[str, Any]] = []
    for category, indexes in sorted(by_cell.items()):
        labels = [holdout[idx].label for idx in indexes]
        raw = [raw_scores[idx] for idx in indexes]
        static = [static_scores[idx] for idx in indexes]
        trace = [trace_scores[idx] for idx in indexes]
        raw_metrics = metric_block(labels, raw)
        static_metrics = metric_block(labels, static)
        trace_metrics = metric_block(labels, trace)
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "execution_aware_trace_lift_cell_metric",
                "formula_version": FORMULA_VERSION,
                "cell": f"{category}::python",
                "mutation_category": category,
                "language": "python",
                "holdout_rows": len(indexes),
                "oracle_pass_count": sum(labels),
                "oracle_fail_count": len(labels) - sum(labels),
                "raw_static_baseline": raw_metrics,
                "static_calibrated_baseline": static_metrics,
                "trace_augmented": trace_metrics,
                "delta_vs_static_calibrated": metric_delta(static_metrics, trace_metrics),
                "delta_vs_raw_static": metric_delta(raw_metrics, trace_metrics),
            }
        )
    return rows


def metric_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, float | None]:
    return {
        "correlation_delta": none_delta(before.get("prediction_oracle_correlation"), after.get("prediction_oracle_correlation")),
        "brier_delta": none_delta(before.get("brier"), after.get("brier")),
        "fixed_bin_ece_delta": none_delta(before.get("fixed_bin_ece"), after.get("fixed_bin_ece")),
        "smooth_ece_delta": none_delta(before.get("smooth_ece"), after.get("smooth_ece")),
        "accuracy_delta": none_delta(before.get("accuracy_at_half"), after.get("accuracy_at_half")),
    }


def none_delta(before: Any, after: Any) -> float | None:
    if before is None or after is None:
        return None
    return float(after) - float(before)


def artifact_hashes(root: Path, paths: list[Path]) -> Path:
    rows = []
    for path in sorted(paths):
        if path.is_file():
            rows.append({"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path)})
    out = root / "artifact_hashes.json"
    write_json(out, {"artifact_count": len(rows), "artifacts": rows})
    return out


def copy_artifacts_to_state(fsv_paths: list[Path], fsv_root: Path, state_root: Path) -> list[Path]:
    copied: list[Path] = []
    for path in fsv_paths:
        relative = path.relative_to(fsv_root)
        target = state_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        require(sha256_file(path) == sha256_file(target), "EXECUTION_AWARE_LIFT_STATE_COPY_HASH_MISMATCH", {"source": str(path), "state": str(target)})
        copied.append(target)
    return copied


def run_measurement(args: argparse.Namespace) -> dict[str, Any]:
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    ship_gate_report = Path(args.ship_gate_report)
    corpus_root = Path(args.corpus_root)
    trace_root = Path(args.trace_root)
    for label, path in (
        ("output_root", output_root),
        ("state_root", state_root),
        ("ship_gate_report", ship_gate_report),
        ("corpus_root", corpus_root),
        ("trace_root", trace_root),
    ):
        reject_mounted_drive(label, path)
    require_archive_path(output_root, "output_root", bool(args.allow_non_aiwonder_roots))
    require_archive_path(state_root, "state_root", bool(args.allow_non_aiwonder_roots))
    require_archive_path(trace_root, "trace_root", bool(args.allow_non_aiwonder_roots))
    run_id = str(args.run_id or f"run-{utc_stamp()}-lift-measurement")
    fsv_run_root = output_root / run_id
    state_run_root = state_root / run_id
    require(not fsv_run_root.exists(), "EXECUTION_AWARE_LIFT_OUTPUT_ROOT_EXISTS", str(fsv_run_root))
    require(not state_run_root.exists(), "EXECUTION_AWARE_LIFT_STATE_ROOT_EXISTS", str(state_run_root))
    fsv_run_root.mkdir(parents=True, exist_ok=False)
    state_run_root.mkdir(parents=True, exist_ok=False)

    calibration, holdout, prediction_sources = load_ship_gate_rows(
        ship_gate_report,
        expected_calibration=int(args.expected_calibration_count),
        expected_holdout=int(args.expected_holdout_count),
    )
    corpus_entries = load_corpus_entries(corpus_root, int(args.expected_corpus_rows) if args.expected_corpus_rows else None)
    traces, trace_inventory = discover_trace_features(trace_root)
    corpus_keys = {(row["task_id"], row["category"]) for row in corpus_entries}
    trace_keys = set(traces)
    missing_corpus_trace = sorted(corpus_keys - trace_keys)
    extra_trace_keys = sorted(trace_keys - corpus_keys)
    require(
        len(corpus_keys) == len(corpus_entries),
        "EXECUTION_AWARE_LIFT_CORPUS_KEY_COUNT_MISMATCH",
        {"corpus_rows": len(corpus_entries), "corpus_keys": len(corpus_keys)},
    )
    coverage_fraction = len(corpus_keys & trace_keys) / len(corpus_keys) if corpus_keys else 0.0
    require(
        coverage_fraction >= float(args.min_trace_coverage),
        "EXECUTION_AWARE_LIFT_TRACE_COVERAGE_BELOW_THRESHOLD",
        {"coverage_fraction": coverage_fraction, "min_trace_coverage": args.min_trace_coverage, "missing_count": len(missing_corpus_trace)},
    )
    require(not missing_corpus_trace, "EXECUTION_AWARE_LIFT_CORPUS_TRACE_MISSING", {"missing_count": len(missing_corpus_trace), "sample": missing_corpus_trace[:20]})
    measurement = fit_and_measure(
        calibration,
        holdout,
        traces,
        ridge_alpha=float(args.ridge_alpha),
        min_lift=float(args.min_correlation_lift),
    )
    calibration_feature_rows = build_feature_rows(calibration, traces, include_labels=True)
    holdout_feature_rows = build_feature_rows(holdout, traces, include_labels=True)
    all_corpus_feature_rows = [
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "execution_aware_trace_corpus_feature_row",
            "formula_version": FORMULA_VERSION,
            "row_index": row["row_index"],
            "task_id": row["task_id"],
            "repo": row["repo"],
            "category": row["category"],
            "trace_path": traces[(row["task_id"], row["category"])].path,
            "trace_sha256": traces[(row["task_id"], row["category"])].sha256,
            "trace_producer": traces[(row["task_id"], row["category"])].producer,
            "features": traces[(row["task_id"], row["category"])].features,
            "feature_source_summary": traces[(row["task_id"], row["category"])].feature_source_summary,
        }
        for row in corpus_entries
    ]
    split_features_path = fsv_run_root / "execution_aware_trace_split_features.jsonl"
    corpus_features_path = fsv_run_root / "execution_aware_trace_corpus_features.jsonl"
    lift_rows_path = fsv_run_root / "execution_aware_trace_lift_rows.jsonl"
    source_samples_path = fsv_run_root / "execution_aware_trace_lift_source_samples.jsonl"
    split_rows = calibration_feature_rows + holdout_feature_rows
    write_jsonl(split_features_path, split_rows)
    write_jsonl(corpus_features_path, all_corpus_feature_rows)
    write_jsonl(lift_rows_path, measurement["report_partition"]["per_cell_rows"])
    write_jsonl(source_samples_path, split_rows[:20] + all_corpus_feature_rows[:20])
    cell_counts = Counter(row["category"] for row in corpus_entries)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "canonical_issue": "#774",
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "source_of_truth": {
            "ship_gate_report": {"path": str(ship_gate_report), "sha256": sha256_file(ship_gate_report)},
            "corpus_index": {"path": str(corpus_root / "index.json"), "sha256": sha256_file(corpus_root / "index.json")},
            "trace_root": str(trace_root),
            "trace_inventory": trace_inventory,
        },
        "raw_prediction_sources": prediction_sources,
        "row_counts": {
            "corpus_rows": len(corpus_entries),
            "calibration_rows": len(calibration),
            "holdout_rows": len(holdout),
            "trace_rows_joined_to_corpus": len(corpus_keys & trace_keys),
            "extra_trace_keys_outside_corpus": len(extra_trace_keys),
            "split_feature_rows": len(split_rows),
        },
        "cell_counts": dict(sorted(cell_counts.items())),
        "trace_coverage": {
            "coverage_fraction": float(round(coverage_fraction, 10)),
            "missing_corpus_trace_rows": len(missing_corpus_trace),
            "min_trace_coverage_required": float(args.min_trace_coverage),
        },
        "modeling_policy": {
            "offline_measurement_only": True,
            "baseline": "static_calibrated_from_calibration_split",
            "candidate": "static_calibrated_plus_trace_shape_features",
            "holdout_rows_used_for_fitting": 0,
            "direct_outcome_fields_excluded_from_live_feature_vector": sorted(DIRECT_OUTCOME_KEYS_EXCLUDED_FROM_FEATURES),
            "target_side_oracle_labels_used_only_for_offline_fit_and_report": True,
            "feature_names": list(FEATURE_NAMES),
        },
        "measurement": measurement,
        "decision": {
            **measurement["decision"],
            "execution_aware_pretraining_ready": True,
            "trace_generation_required": False,
            "predictor_weights_changed": False,
        },
        "doctrine_policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "oracle_verdicts_classified_target_side_only": True,
            "predictor_weights_changed": False,
            "inner_llm_used": False,
            "aiwonder_durable_root_required": True,
        },
        "artifacts": {
            "split_features": str(split_features_path),
            "corpus_features": str(corpus_features_path),
            "lift_rows": str(lift_rows_path),
            "source_samples": str(source_samples_path),
            "state_run_root": str(state_run_root),
        },
    }
    manifest_path = fsv_run_root / "execution_aware_trace_lift_manifest.json"
    write_json(manifest_path, manifest)
    fsv_paths = [manifest_path, split_features_path, corpus_features_path, lift_rows_path, source_samples_path]
    state_paths = copy_artifacts_to_state(fsv_paths, fsv_run_root, state_run_root)
    hash_path = artifact_hashes(fsv_run_root, [*fsv_paths, *state_paths])
    state_hash_path = state_run_root / hash_path.name
    write_json(state_hash_path, read_json(hash_path, "EXECUTION_AWARE_LIFT_ARTIFACT_HASHES_INVALID"))
    return {
        "ok": True,
        "run_id": run_id,
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "manifest": str(manifest_path),
        "artifact_hashes": str(hash_path),
        "decision": manifest["decision"],
        "trace_coverage": manifest["trace_coverage"],
        "global_delta_vs_static_calibrated": measurement["report_partition"]["global_delta_vs_static_calibrated"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Measure #774 execution-aware trace utility lift")
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--trace-root", type=Path, default=DEFAULT_TRACE_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--expected-corpus-rows", type=int, default=2400)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--ridge-alpha", type=float, default=10.0)
    parser.add_argument("--min-correlation-lift", type=float, default=0.005)
    parser.add_argument("--min-trace-coverage", type=float, default=0.95)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = run_measurement(args)
    except ExecutionAwareLiftMeasurementError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, indent=2, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
