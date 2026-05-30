#!/usr/bin/env python3
"""Temporal Goodhart audit for BLINDSPOT-008b / #731."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

import numpy as np

from .honest_pearson_metrics import pearson_correlation


SCHEMA_VERSION = 1
TASK_ID = "TASK-731"
ARTIFACT_KIND = "temporal_goodhart_audit"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_TASKS_DIR = Path("tasks/swebench-lite")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/blindspot-008b-temporal-goodhart-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/temporal-goodhart-audit")


class TemporalGoodhartAuditError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise TemporalGoodhartAuditError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "TEMPORAL_GOODHART_AUDIT_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


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
        raise TemporalGoodhartAuditError(f"{code}: invalid JSON at {path}: {exc}") from exc


def parse_created_at(value: Any, *, task_id: str) -> datetime:
    require(
        isinstance(value, str) and value,
        "TEMPORAL_GOODHART_AUDIT_CREATED_AT_MISSING",
        {"task_id": task_id, "value": value},
    )
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise TemporalGoodhartAuditError(
            f"TEMPORAL_GOODHART_AUDIT_CREATED_AT_INVALID: task_id={task_id} value={value!r}"
        ) from exc
    require(
        parsed.tzinfo is not None,
        "TEMPORAL_GOODHART_AUDIT_CREATED_AT_NOT_TIMEZONED",
        {"task_id": task_id, "value": value},
    )
    return parsed.astimezone(timezone.utc)


def finite_score(value: Any, *, field: str, index: int, split: str) -> float:
    require(
        isinstance(value, (int, float)) and not isinstance(value, bool),
        "TEMPORAL_GOODHART_AUDIT_ROW_BAD_SCORE",
        {"split": split, "index": index, "field": field, "value": value},
    )
    out = float(value)
    require(
        math.isfinite(out),
        "TEMPORAL_GOODHART_AUDIT_ROW_NONFINITE_SCORE",
        {"split": split, "index": index, "field": field, "value": value},
    )
    return out


def round_float(value: float | None) -> float | None:
    if value is None or not math.isfinite(value):
        return None
    return float(round(value, 10))


def safe_pearson(scores: list[float], labels: list[float]) -> float | None:
    if len(scores) < 2:
        return None
    try:
        value = pearson_correlation(scores, labels)
    except ValueError:
        return None
    if not math.isfinite(value):
        return None
    return value


def linear_slope(xs: list[float], ys: list[float]) -> float | None:
    if len(xs) < 2 or len(xs) != len(ys):
        return None
    x = np.asarray(xs, dtype=np.float64)
    y = np.asarray(ys, dtype=np.float64)
    if not (np.isfinite(x).all() and np.isfinite(y).all()):
        return None
    x_var = float(((x - x.mean()) ** 2).mean())
    if x_var <= 0.0:
        return None
    return float(((x - x.mean()) * (y - y.mean())).mean() / x_var)


def _require_object(value: Any, label: str) -> dict[str, Any]:
    require(
        isinstance(value, dict),
        "TEMPORAL_GOODHART_AUDIT_BAD_INPUT",
        {"label": label, "actual_type": type(value).__name__},
    )
    return value


def _require_array(value: Any, label: str) -> list[Any]:
    require(
        isinstance(value, list),
        "TEMPORAL_GOODHART_AUDIT_BAD_INPUT",
        {"label": label, "actual_type": type(value).__name__},
    )
    return value


def coerce_prediction_row(value: Any, index: int, split: str) -> dict[str, Any]:
    require(
        isinstance(value, dict),
        "TEMPORAL_GOODHART_AUDIT_ROW_NOT_OBJECT",
        {"split": split, "index": index, "actual_type": type(value).__name__},
    )
    task_id = str(value.get("task_id") or "")
    repo = str(value.get("repo") or "")
    category = str(value.get("category") or "")
    require(task_id, "TEMPORAL_GOODHART_AUDIT_ROW_TASK_ID_MISSING", {"split": split, "index": index})
    require(repo, "TEMPORAL_GOODHART_AUDIT_ROW_REPO_MISSING", {"split": split, "index": index})
    require(category, "TEMPORAL_GOODHART_AUDIT_ROW_CATEGORY_MISSING", {"split": split, "index": index})
    actual = value.get("actual_oracle_pass")
    require(
        isinstance(actual, (bool, int, float)),
        "TEMPORAL_GOODHART_AUDIT_ROW_LABEL_MISSING",
        {"split": split, "index": index, "task_id": task_id},
    )
    score = finite_score(value.get("predicted_oracle_pass"), field="predicted_oracle_pass", index=index, split=split)
    return {
        "row_index": index,
        "split": split,
        "task_id": task_id,
        "repo": repo,
        "category": category,
        "predicted_oracle_pass": score,
        "actual_oracle_pass": bool(actual),
        "label": 1.0 if bool(actual) else 0.0,
    }


def raw_prediction_rows(execute: dict[str, Any], split_label: str) -> tuple[str, list[dict[str, Any]]]:
    for key in (
        f"{split_label}_raw_predictions",
        f"{split_label}_predictions",
        f"{split_label}_rank_lift_predictions",
    ):
        if key in execute and execute.get(key) is not None:
            rows = _require_array(execute.get(key), key)
            return key, [coerce_prediction_row(row, idx, split_label) for idx, row in enumerate(rows)]
    raise TemporalGoodhartAuditError(
        f"TEMPORAL_GOODHART_AUDIT_MISSING_ROWS: split={split_label}"
    )


def load_task_metadata(path: Path) -> dict[str, dict[str, Any]]:
    doc = read_json(path, "TEMPORAL_GOODHART_AUDIT_METADATA_MISSING")
    rows = doc.get("tasks") if isinstance(doc, dict) else doc
    require(isinstance(rows, list), "TEMPORAL_GOODHART_AUDIT_METADATA_NOT_LIST", {"path": str(path)})
    by_task: dict[str, dict[str, Any]] = {}
    for row in rows:
        require(isinstance(row, dict), "TEMPORAL_GOODHART_AUDIT_METADATA_ROW_NOT_OBJECT", row)
        task_id = str(row.get("task_id") or row.get("instance_id") or "")
        require(task_id, "TEMPORAL_GOODHART_AUDIT_METADATA_TASK_ID_MISSING", row)
        require(task_id not in by_task, "TEMPORAL_GOODHART_AUDIT_METADATA_DUPLICATE_TASK", {"task_id": task_id})
        created_at = parse_created_at(row.get("created_at"), task_id=task_id)
        by_task[task_id] = {
            "task_id": task_id,
            "repo": str(row.get("repo") or ""),
            "created_at": created_at.isoformat().replace("+00:00", "Z"),
            "created_at_unix": created_at.timestamp(),
            "year": created_at.year,
            "source_url": str(row.get("source_url") or row.get("html_url") or ""),
        }
    return by_task


def load_task_ids_from_dir(tasks_dir: Path, expected_task_count: int | None) -> list[str]:
    require(tasks_dir.is_dir(), "TEMPORAL_GOODHART_AUDIT_TASKS_DIR_MISSING", {"tasks_dir": str(tasks_dir)})
    paths = sorted(tasks_dir.glob("*.json"))
    require(paths, "TEMPORAL_GOODHART_AUDIT_TASKS_EMPTY", {"tasks_dir": str(tasks_dir)})
    if expected_task_count is not None:
        require(
            len(paths) == expected_task_count,
            "TEMPORAL_GOODHART_AUDIT_TASK_COUNT_MISMATCH",
            {"expected": expected_task_count, "actual": len(paths)},
        )
    task_ids: list[str] = []
    for path in paths:
        row = read_json(path, "TEMPORAL_GOODHART_AUDIT_TASK_JSON_INVALID")
        task_id = str(row.get("instance_id") or path.stem)
        require(task_id, "TEMPORAL_GOODHART_AUDIT_TASK_ID_MISSING", {"path": str(path)})
        task_ids.append(task_id)
    return task_ids


def enrich_rows(rows: list[dict[str, Any]], metadata: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    missing = sorted({row["task_id"] for row in rows} - set(metadata))
    require(
        not missing,
        "TEMPORAL_GOODHART_AUDIT_METADATA_INCOMPLETE_FOR_REPORT",
        {"missing_task_ids": missing[:20], "missing_count": len(missing)},
    )
    out: list[dict[str, Any]] = []
    for row in rows:
        meta = metadata[row["task_id"]]
        out.append({**row, **{k: meta[k] for k in ("created_at", "created_at_unix", "year")}})
    return out


def year_metrics(rows: list[dict[str, Any]], baseline_pearson: float | None) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    by_year: dict[int, list[dict[str, Any]]] = {}
    for row in rows:
        by_year.setdefault(int(row["year"]), []).append(row)
    for year, year_rows in sorted(by_year.items()):
        scores = [row["predicted_oracle_pass"] for row in year_rows]
        labels = [row["label"] for row in year_rows]
        value = safe_pearson(scores, labels)
        out.append(
            {
                "year": year,
                "row_count": len(year_rows),
                "task_count": len({row["task_id"] for row in year_rows}),
                "repo_count": len({row["repo"] for row in year_rows}),
                "label_counts": dict(sorted(Counter("pass" if row["actual_oracle_pass"] else "fail" for row in year_rows).items())),
                "raw_pearson": round_float(value),
                "drop_vs_natural_holdout_pearson": (
                    round_float(baseline_pearson - value)
                    if baseline_pearson is not None and value is not None
                    else None
                ),
                "pearson_defined": value is not None,
            }
        )
    return out


def newest_task_rows(rows: list[dict[str, Any]], newest_task_count: int) -> list[dict[str, Any]]:
    require(
        newest_task_count > 0,
        "TEMPORAL_GOODHART_AUDIT_BAD_NEWEST_TASK_COUNT",
        {"newest_task_count": newest_task_count},
    )
    task_order = sorted(
        {
            row["task_id"]: float(row["created_at_unix"])
            for row in rows
        }.items(),
        key=lambda item: (item[1], item[0]),
        reverse=True,
    )
    newest = {task_id for task_id, _ in task_order[:newest_task_count]}
    return [row for row in rows if row["task_id"] in newest]


def temporal_candidate_split(
    task_ids: list[str],
    metadata: dict[str, dict[str, Any]],
    *,
    holdout_task_count: int,
) -> dict[str, Any]:
    require(
        0 < holdout_task_count < len(task_ids),
        "TEMPORAL_GOODHART_AUDIT_BAD_TEMPORAL_HOLDOUT_TASK_COUNT",
        {"holdout_task_count": holdout_task_count, "task_count": len(task_ids)},
    )
    missing = sorted(set(task_ids) - set(metadata))
    require(
        not missing,
        "TEMPORAL_GOODHART_AUDIT_METADATA_INCOMPLETE_FOR_TASKS_DIR",
        {"missing_task_ids": missing[:20], "missing_count": len(missing)},
    )
    ordered = sorted(
        task_ids,
        key=lambda task_id: (float(metadata[task_id]["created_at_unix"]), task_id),
    )
    holdout = ordered[-holdout_task_count:]
    train = ordered[:-holdout_task_count]
    return {
        "available": True,
        "temporal_disjoint_retraining_performed": False,
        "train_task_count": len(train),
        "holdout_task_count": len(holdout),
        "train_created_at_min": metadata[train[0]]["created_at"] if train else None,
        "train_created_at_max": metadata[train[-1]]["created_at"] if train else None,
        "holdout_created_at_min": metadata[holdout[0]]["created_at"] if holdout else None,
        "holdout_created_at_max": metadata[holdout[-1]]["created_at"] if holdout else None,
        "holdout_task_ids_sample": holdout[:10],
        "holdout_year_counts": dict(sorted(Counter(metadata[task_id]["year"] for task_id in holdout).items())),
    }


def build_manifest(
    *,
    ship_gate_report: Path,
    metadata_json: Path,
    calibration_source: str,
    holdout_source: str,
    calibration_rows: list[dict[str, Any]],
    holdout_rows: list[dict[str, Any]],
    metadata: dict[str, dict[str, Any]],
    task_ids: list[str],
    newest_task_count: int,
    temporal_holdout_task_count: int,
    min_years: int,
    drop_threshold: float,
    severe_drop_threshold: float,
    per_year_path: Path,
    newest_rows_path: Path,
    state_run_root: Path,
    run_id: str,
) -> dict[str, Any]:
    require(holdout_rows, "TEMPORAL_GOODHART_AUDIT_HOLDOUT_EMPTY")
    require(calibration_rows, "TEMPORAL_GOODHART_AUDIT_CALIBRATION_EMPTY")
    enriched_calibration = enrich_rows(calibration_rows, metadata)
    enriched_holdout = enrich_rows(holdout_rows, metadata)
    scores = [row["predicted_oracle_pass"] for row in enriched_holdout]
    labels = [row["label"] for row in enriched_holdout]
    natural = safe_pearson(scores, labels)
    years = sorted({int(row["year"]) for row in enriched_holdout})
    per_year = year_metrics(enriched_holdout, natural)
    slope_rows = [row for row in per_year if row["raw_pearson"] is not None]
    slope = linear_slope(
        [float(row["year"]) for row in slope_rows],
        [float(row["raw_pearson"]) for row in slope_rows],
    )
    newest_rows = newest_task_rows(enriched_holdout, newest_task_count)
    newest_scores = [row["predicted_oracle_pass"] for row in newest_rows]
    newest_labels = [row["label"] for row in newest_rows]
    newest_corr = safe_pearson(newest_scores, newest_labels)
    newest_drop = (
        natural - newest_corr
        if natural is not None and newest_corr is not None
        else None
    )
    candidate = temporal_candidate_split(
        task_ids,
        metadata,
        holdout_task_count=temporal_holdout_task_count,
    )
    current_holdout_tasks = {row["task_id"] for row in enriched_holdout}
    # Use the full sorted holdout set for coverage, not just the sample.
    ordered_all = sorted(task_ids, key=lambda task_id: (float(metadata[task_id]["created_at_unix"]), task_id))
    full_candidate_holdout = set(ordered_all[-temporal_holdout_task_count:])
    missing_candidate_from_current_holdout = sorted(full_candidate_holdout - current_holdout_tasks)
    candidate["candidate_holdout_tasks_present_in_current_holdout"] = sorted(full_candidate_holdout & current_holdout_tasks)[:20]
    candidate["candidate_holdout_tasks_missing_from_current_holdout_count"] = len(missing_candidate_from_current_holdout)
    candidate["candidate_holdout_tasks_missing_from_current_holdout_sample"] = missing_candidate_from_current_holdout[:20]
    year_bad = len(years) < min_years
    newest_bad = newest_drop is not None and newest_drop > drop_threshold
    severe_newest_bad = newest_drop is not None and newest_drop > severe_drop_threshold
    slope_bad = slope is not None and slope < -0.01
    candidate_missing = len(missing_candidate_from_current_holdout) > 0
    full_required = year_bad or newest_bad or slope_bad or candidate_missing
    decision_reasons: list[str] = []
    if year_bad:
        decision_reasons.append("current_holdout_temporal_coverage_insufficient")
    if newest_bad:
        decision_reasons.append("newest_current_holdout_window_drops_correlation")
    if slope_bad:
        decision_reasons.append("per_year_correlation_slope_negative")
    if candidate_missing:
        decision_reasons.append("candidate_temporal_holdout_not_covered_by_current_holdout")
    if not decision_reasons:
        decision_reasons.append("temporal_goodhart_minor_on_current_fsv")
    write_jsonl(per_year_path, per_year)
    write_jsonl(newest_rows_path, newest_rows)
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "ship_gate_report_path": str(ship_gate_report),
        "ship_gate_report_sha256": sha256_file(ship_gate_report),
        "task_metadata_path": str(metadata_json),
        "task_metadata_sha256": sha256_file(metadata_json),
        "raw_prediction_sources": {
            "calibration": calibration_source,
            "holdout": holdout_source,
        },
        "row_counts": {
            "calibration_rows": len(enriched_calibration),
            "holdout_rows": len(enriched_holdout),
            "metadata_task_rows": len(metadata),
            "per_year_metric_rows": len(per_year),
            "newest_window_rows": len(newest_rows),
        },
        "temporal_coverage": {
            "calibration_year_counts": dict(sorted(Counter(row["year"] for row in enriched_calibration).items())),
            "holdout_year_counts": dict(sorted(Counter(row["year"] for row in enriched_holdout).items())),
            "holdout_year_count": len(years),
            "min_years": min_years,
            "holdout_created_at_min": min(row["created_at"] for row in enriched_holdout),
            "holdout_created_at_max": max(row["created_at"] for row in enriched_holdout),
            "current_holdout_temporal_claim_allowed": not year_bad and not candidate_missing,
        },
        "metrics": {
            "natural_holdout_raw_pearson": round_float(natural),
            "newest_task_count": newest_task_count,
            "newest_current_holdout_raw_pearson": round_float(newest_corr),
            "newest_current_holdout_drop": round_float(newest_drop),
            "per_year_raw_pearson_slope": round_float(slope),
            "drop_threshold": drop_threshold,
            "severe_drop_threshold": severe_drop_threshold,
            "severe_newest_drop": severe_newest_bad,
        },
        "temporal_candidate_split": candidate,
        "decision": {
            "temporal_disjoint_retraining_performed": False,
            "temporal_goodhart_risk_detected_on_current_fsv": bool(newest_bad or slope_bad),
            "full_temporal_disjoint_evaluation_required": full_required,
            "canonical_full_execution_issue": "#769" if full_required else None,
            "decision_reasons": decision_reasons,
            "ship_gate_temporal_generalization_claim_allowed": not full_required,
        },
        "doctrine_policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "predictor_weights_changed": False,
            "temporal_disjoint_retraining_performed": False,
            "aiwonder_durable_root_required": True,
        },
        "artifacts": {
            "per_year_holdout_metrics": str(per_year_path),
            "newest_holdout_rows": str(newest_rows_path),
            "state_run_root": str(state_run_root),
        },
    }


def write_artifact_hashes(root: Path, paths: list[Path]) -> Path:
    rows = []
    for path in sorted(paths):
        if path.is_file():
            rows.append({"path": str(path), "sha256": sha256_file(path), "bytes": path.stat().st_size})
    out = root / "artifact_hashes.json"
    write_json(out, {"artifact_count": len(rows), "artifacts": rows})
    return out


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    require_archive_path(output_root, "output_root", bool(args.allow_non_aiwonder_roots))
    require_archive_path(state_root, "state_root", bool(args.allow_non_aiwonder_roots))
    run_id = str(args.run_id or f"run-{utc_stamp()}")
    fsv_run_root = output_root / run_id
    state_run_root = state_root / run_id
    fsv_run_root.mkdir(parents=True, exist_ok=False)
    state_run_root.mkdir(parents=True, exist_ok=False)

    payload = read_json(Path(args.ship_gate_report), "TEMPORAL_GOODHART_AUDIT_SHIP_GATE_REPORT_MISSING")
    execute = _require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    calibration_source, calibration_rows = raw_prediction_rows(execute, "calibration")
    holdout_source, holdout_rows = raw_prediction_rows(execute, "holdout")
    metadata = load_task_metadata(Path(args.task_metadata_json))
    task_ids = load_task_ids_from_dir(Path(args.tasks_dir), args.expected_task_count)
    per_year_path = fsv_run_root / "per_year_holdout_metrics.jsonl"
    newest_rows_path = fsv_run_root / "newest_current_holdout_rows.jsonl"
    manifest = build_manifest(
        ship_gate_report=Path(args.ship_gate_report),
        metadata_json=Path(args.task_metadata_json),
        calibration_source=calibration_source,
        holdout_source=holdout_source,
        calibration_rows=calibration_rows,
        holdout_rows=holdout_rows,
        metadata=metadata,
        task_ids=task_ids,
        newest_task_count=int(args.newest_task_count),
        temporal_holdout_task_count=int(args.temporal_holdout_task_count),
        min_years=int(args.min_years),
        drop_threshold=float(args.drop_threshold),
        severe_drop_threshold=float(args.severe_drop_threshold),
        per_year_path=per_year_path,
        newest_rows_path=newest_rows_path,
        state_run_root=state_run_root,
        run_id=run_id,
    )
    manifest_path = fsv_run_root / "temporal_goodhart_manifest.json"
    write_json(manifest_path, manifest)
    state_manifest_path = state_run_root / manifest_path.name
    state_per_year_path = state_run_root / per_year_path.name
    state_newest_rows_path = state_run_root / newest_rows_path.name
    write_json(state_manifest_path, manifest)
    write_jsonl(state_per_year_path, [json.loads(line) for line in per_year_path.read_text(encoding="utf-8").splitlines()])
    write_jsonl(state_newest_rows_path, [json.loads(line) for line in newest_rows_path.read_text(encoding="utf-8").splitlines()])
    artifacts = [manifest_path, per_year_path, newest_rows_path, state_manifest_path, state_per_year_path, state_newest_rows_path]
    hash_path = write_artifact_hashes(fsv_run_root, artifacts)
    state_hash_path = state_run_root / hash_path.name
    write_json(state_hash_path, read_json(hash_path, "TEMPORAL_GOODHART_AUDIT_HASH_JSON_INVALID"))
    return {
        "ok": True,
        "run_id": run_id,
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "manifest": str(manifest_path),
        "per_year_holdout_metrics": str(per_year_path),
        "newest_current_holdout_rows": str(newest_rows_path),
        "artifact_hashes": str(hash_path),
        "decision": manifest["decision"],
        "temporal_coverage": manifest["temporal_coverage"],
        "metrics": manifest["metrics"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit temporal Goodhart risk from a ship-gate FSV report")
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--task-metadata-json", type=Path, required=True)
    parser.add_argument("--tasks-dir", type=Path, default=DEFAULT_TASKS_DIR)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--expected-task-count", type=int, default=300)
    parser.add_argument("--newest-task-count", type=int, default=10)
    parser.add_argument("--temporal-holdout-task-count", type=int, default=60)
    parser.add_argument("--min-years", type=int, default=4)
    parser.add_argument("--drop-threshold", type=float, default=0.05)
    parser.add_argument("--severe-drop-threshold", type=float, default=0.10)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = run_audit(args)
    except TemporalGoodhartAuditError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, indent=2, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
