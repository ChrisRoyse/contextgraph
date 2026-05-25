"""Measure out-of-fold prediction lift from geometry label quality."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file, utc_now
from .python_per_cell_smooth_ece import metric_block, pearson_float_or_none, reject_mounted_drive


SCHEMA_VERSION = 1
ARTIFACT_KIND = "label_quality_downstream_lift"
FORMULA_VERSION = "label_quality_downstream_lift_v1"

DEFAULT_LABEL_QUALITY_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-673-label-quality-embedder-signal-v2-static-ast-v1"
)
DEFAULT_GEOMETRY_LABEL_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-136-per-embedder-geometry-labels-v2-static-ast-v1"
)
DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_ACTIVE_ABSTAIN_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-679-label-quality-downstream-lift-v2-static-ast-v1"
)

WEAK_CELLS = ("delete_test_call", "off_by_one", "subtle_flip")
BETA_GRID = (-1.0, -0.65, -0.35, -0.18, -0.08, 0.0, 0.08, 0.18, 0.35, 0.65, 1.0)


class LabelQualityDownstreamLiftError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the ship-gate row traces, geometry memberships, and label-quality score artifacts; do not promote labels until this materializer has partition-safe measured output.",
            details,
        )


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise LabelQualityDownstreamLiftError(code, message, details)


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    require(path.is_file(), "LABEL_QUALITY_LIFT_INPUT_MISSING", str(path))
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise LabelQualityDownstreamLiftError(
                    "LABEL_QUALITY_LIFT_BAD_JSONL",
                    f"{path}:{line_no}: {exc}",
                    {"path": str(path), "line_no": line_no},
                ) from exc
            require(isinstance(row, dict), "LABEL_QUALITY_LIFT_JSONL_ROW_NOT_OBJECT", f"{path}:{line_no}")
            yield line_no, row


def load_json(path: Path) -> dict[str, Any]:
    require(path.is_file(), "LABEL_QUALITY_LIFT_INPUT_MISSING", str(path))
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise LabelQualityDownstreamLiftError(
            "LABEL_QUALITY_LIFT_BAD_JSON",
            f"{path}: {exc}",
            {"path": str(path), "error": str(exc)},
        ) from exc
    require(isinstance(value, dict), "LABEL_QUALITY_LIFT_JSON_NOT_OBJECT", str(path))
    return value


def atomic_json_dump(path: Path, value: dict[str, Any]) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, sort_keys=True, indent=2, separators=(",", ": ")) + "\n"
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(payload, encoding="utf-8")
    tmp.replace(path)
    return "sha256:" + hashlib.sha256(payload.encode("utf-8")).hexdigest()


class JsonlSink:
    def __init__(self, path: Path):
        self.path = path
        self.tmp = path.with_suffix(path.suffix + ".tmp")
        self.digest = hashlib.sha256()
        self.count = 0
        self.handle: Any = None

    def __enter__(self) -> "JsonlSink":
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.handle = self.tmp.open("w", encoding="utf-8")
        return self

    def write(self, row: dict[str, Any]) -> None:
        require(self.handle is not None, "LABEL_QUALITY_LIFT_SINK_NOT_OPEN", str(self.path))
        encoded = json.dumps(row, sort_keys=True, separators=(",", ":")).encode("utf-8")
        self.handle.write(encoded.decode("utf-8"))
        self.handle.write("\n")
        self.digest.update(encoded)
        self.digest.update(b"\n")
        self.count += 1

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        if self.handle is not None:
            self.handle.close()
        if exc_type is None:
            self.tmp.replace(self.path)
        elif self.tmp.exists():
            self.tmp.unlink()

    def artifact(self) -> dict[str, Any]:
        return {
            "path": str(self.path),
            "rows": self.count,
            "bytes": self.path.stat().st_size if self.path.exists() else 0,
            "sha256": "sha256:" + self.digest.hexdigest(),
        }


@dataclass(frozen=True)
class PredictionRow:
    split: str
    row_index: int
    task_id: str
    repo: str
    category: str
    patch_path: str
    patch_sha256: str
    label: int
    score: float
    active_after_abstain: bool | None = None

    @property
    def key(self) -> tuple[str, str, str]:
        return (self.split, self.task_id, self.category)


@dataclass
class FeatureAccumulator:
    membership_rows: int = 0
    missing_quality_rows: int = 0
    live_rows: int = 0
    signal_sum: float = 0.0
    noise_sum: float = 0.0
    physical_sum: float = 0.0
    interpretability_sum: float = 0.0
    stability_sum: float = 0.0
    support_sum: int = 0
    quality_classes: Counter[str] = None  # type: ignore[assignment]
    sample_labels: list[dict[str, Any]] = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        if self.quality_classes is None:
            self.quality_classes = Counter()
        if self.sample_labels is None:
            self.sample_labels = []

    def add(self, quality: dict[str, Any] | None) -> None:
        self.membership_rows += 1
        if quality is None:
            self.missing_quality_rows += 1
            return
        self.quality_classes[str(quality.get("quality_class") or "unknown")] += 1
        if quality.get("live_prediction_feature_allowed") is True:
            self.live_rows += 1
            self.signal_sum += float(quality.get("signal_score") or 0.0)
        self.noise_sum += float(quality.get("noise_score") or 0.0)
        self.physical_sum += float(quality.get("physical_validity_score") or 0.0)
        self.interpretability_sum += float(quality.get("interpretability_score") or 0.0)
        self.stability_sum += float(quality.get("stability_score") or 0.0)
        try:
            self.support_sum += int(quality.get("support_count") or 0)
        except (TypeError, ValueError):
            pass
        if len(self.sample_labels) < 3:
            self.sample_labels.append(
                {
                    "label_id": quality.get("label_id"),
                    "label_name": quality.get("label_name"),
                    "quality_class": quality.get("quality_class"),
                    "signal_score": quality.get("signal_score"),
                    "noise_score": quality.get("noise_score"),
                }
            )

    def to_feature(self) -> dict[str, Any]:
        denom = max(1, self.membership_rows)
        live_denom = max(1, self.live_rows)
        mean_signal = self.signal_sum / live_denom
        mean_noise = self.noise_sum / denom
        return {
            "membership_rows": self.membership_rows,
            "missing_quality_rows": self.missing_quality_rows,
            "live_label_rows": self.live_rows,
            "mean_signal": mean_signal,
            "mean_noise": mean_noise,
            "mean_physical_validity": self.physical_sum / denom,
            "mean_interpretability": self.interpretability_sum / denom,
            "mean_stability": self.stability_sum / denom,
            "mean_support_count": self.support_sum / denom,
            "signal_minus_noise": mean_signal - mean_noise,
            "quality_classes": dict(sorted(self.quality_classes.items())),
            "sample_labels": self.sample_labels,
        }


def command_python_label_quality_downstream_lift(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    output_root = args.output_root.expanduser().resolve(strict=False)
    label_quality_root = args.label_quality_root.expanduser().resolve(strict=False)
    geometry_label_root = args.geometry_label_root.expanduser().resolve(strict=False)
    ship_gate_report = args.ship_gate_report.expanduser().resolve(strict=False)
    active_abstain_report = args.active_abstain_report.expanduser().resolve(strict=False)

    for field, path in (
        ("output_root", output_root),
        ("label_quality_root", label_quality_root),
        ("geometry_label_root", geometry_label_root),
        ("ship_gate_report", ship_gate_report),
        ("active_abstain_report", active_abstain_report),
    ):
        reject_mounted_drive(field, path)

    scores_path = label_quality_root / "label_quality_scores.jsonl"
    quality_manifest_path = label_quality_root / "label_quality_manifest.json"
    memberships_path = geometry_label_root / "embedder_cluster_memberships.jsonl"
    geometry_manifest_path = geometry_label_root / "embedder_geometry_label_manifest.json"
    for path in (scores_path, quality_manifest_path, memberships_path, geometry_manifest_path, ship_gate_report, active_abstain_report):
        require(path.is_file(), "LABEL_QUALITY_LIFT_INPUT_MISSING", str(path))

    before = {
        "output_root_exists": output_root.exists(),
        "output_root_files": sorted(path.name for path in output_root.iterdir()) if output_root.exists() else [],
    }
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)

    ship = load_json(ship_gate_report)
    execute = require_object(ship.get("execute_and_inspect"), "ship_gate.execute_and_inspect")
    active = load_json(active_abstain_report)
    active_flags = load_active_flags(active)
    calibration_rows = load_prediction_rows(
        execute,
        split="calibration",
        preferred_key="calibration_rank_lift_predictions",
        fallback_key="calibration_predictions",
        expected=args.expected_calibration_count,
        active_flags={},
    )
    holdout_rows = load_prediction_rows(
        execute,
        split="holdout",
        preferred_key="holdout_predictions",
        fallback_key="holdout_rank_lift_predictions",
        expected=args.expected_holdout_count,
        active_flags=active_flags,
    )
    rows = calibration_rows + holdout_rows
    target_keys = {row.key for row in rows}

    quality_by_cluster = load_quality_scores(scores_path)
    features = collect_features(
        memberships_path,
        quality_by_cluster,
        target_keys=target_keys,
        expected_embedders=tuple(args.expected_embedders.split(",")),
    )
    feature_rows_path = output_root / "label_quality_row_features.jsonl"
    lift_rows_path = output_root / "label_quality_downstream_lift.jsonl"
    source_samples_path = output_root / "label_quality_lift_source_samples.jsonl"

    row_feature_artifact, sample_artifact = write_feature_rows(
        feature_rows_path,
        source_samples_path,
        rows,
        features,
        expected_embedders=tuple(args.expected_embedders.split(",")),
    )
    lift_rows, summary_rows = build_lift_rows(
        rows,
        features,
        expected_embedders=tuple(args.expected_embedders.split(",")),
        smooth_ece_regression_budget=float(args.smooth_ece_regression_budget),
        min_featured_rows=int(args.min_featured_rows),
    )
    with JsonlSink(lift_rows_path) as sink:
        for row in lift_rows:
            sink.write(row)
    lift_artifact = sink.artifact()

    summary = {
        "artifact_kind": "label_quality_downstream_lift_summary",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": started_at,
        "githubIssue": 679,
        "prediction_promotion_allowed": any(row["prediction_promotion_allowed"] for row in lift_rows),
        "promotion_policy": "calibration-selected beta; holdout-only reporting; block if aggregate Brier/ECE regresses or any weak cell regresses",
        "source_partitions": {
            "fit_partition": "calibration",
            "report_partition": "holdout",
            "holdout_used_for_fitting": False,
            "target_side_oracle_used_for_label_generation": False,
        },
        "row_counts": {
            "calibration": len(calibration_rows),
            "holdout": len(holdout_rows),
            "active_holdout": sum(1 for row in holdout_rows if row.active_after_abstain is True),
            "abstain_holdout": sum(1 for row in holdout_rows if row.active_after_abstain is False),
        },
        "by_embedder": summary_rows,
    }
    summary_sha = atomic_json_dump(output_root / "label_quality_downstream_lift_summary.json", summary)

    checks = [
        {
            "check": "split_counts_match_ship_gate",
            "passed": len(calibration_rows) == args.expected_calibration_count and len(holdout_rows) == args.expected_holdout_count,
            "actual": {"calibration": len(calibration_rows), "holdout": len(holdout_rows)},
            "expected": {"calibration": args.expected_calibration_count, "holdout": args.expected_holdout_count},
        },
        {
            "check": "row_features_written_for_all_split_rows_and_embedders",
            "passed": row_feature_artifact["rows"] == len(rows) * len(tuple(args.expected_embedders.split(","))),
            "actual": row_feature_artifact["rows"],
            "expected": len(rows) * len(tuple(args.expected_embedders.split(","))),
        },
        {
            "check": "downstream_rows_are_measured_not_placeholders",
            "passed": all(row.get("out_of_fold_lift_status") != "not_computed_in_structural_quality_slice" for row in lift_rows),
            "actual": [row.get("out_of_fold_lift_status") for row in lift_rows],
            "expected": "measured_calibration_to_holdout rows",
        },
        {
            "check": "holdout_not_used_for_beta_selection",
            "passed": all(row["fit_partition"]["holdout_rows_used_for_fitting"] == 0 for row in lift_rows),
            "actual": [row["fit_partition"] for row in lift_rows],
            "expected": "0 holdout rows used for fitting",
        },
        {
            "check": "promotion_blocked_on_any_weak_cell_regression",
            "passed": all(
                (not row["per_cell_regression_check"]["weak_cell_regressions"])
                or row["prediction_promotion_allowed"] is False
                for row in lift_rows
            ),
            "actual": [
                {
                    "embedder_id": row["embedder_id"],
                    "weak_cell_regressions": row["per_cell_regression_check"]["weak_cell_regressions"],
                    "promotion": row["prediction_promotion_allowed"],
                }
                for row in lift_rows
            ],
            "expected": "any weak-cell regression blocks promotion",
        },
        {
            "check": "source_samples_written_for_manual_fsv",
            "passed": sample_artifact["rows"] > 0,
            "actual": sample_artifact["rows"],
            "expected": ">0",
        },
    ]
    all_passed = all(bool(check["passed"]) for check in checks)
    require(all_passed, "LABEL_QUALITY_LIFT_CHECK_FAILED", str(checks))

    manifest = {
        "artifact_kind": "label_quality_downstream_lift_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": started_at,
        "all_passed": True,
        "acceptance_status": "downstream_lift_measured; predictor_promotion_allowed_only_for_nonregressing_embedder_rows",
        "before": before,
        "source_of_truth": {
            "ship_gate_report": inventory(ship_gate_report),
            "active_abstain_report": inventory(active_abstain_report),
            "label_quality_scores": inventory(scores_path),
            "label_quality_manifest": inventory(quality_manifest_path),
            "geometry_memberships": inventory(memberships_path),
            "geometry_manifest": inventory(geometry_manifest_path),
        },
        "parameters": {
            "expected_calibration_count": args.expected_calibration_count,
            "expected_holdout_count": args.expected_holdout_count,
            "expected_embedders": list(args.expected_embedders.split(",")),
            "beta_grid": list(BETA_GRID),
            "weak_cells": list(WEAK_CELLS),
            "smooth_ece_regression_budget": float(args.smooth_ece_regression_budget),
            "min_featured_rows": int(args.min_featured_rows),
        },
        "outputs": {
            "label_quality_row_features": row_feature_artifact,
            "label_quality_lift_source_samples": sample_artifact,
            "label_quality_downstream_lift": lift_artifact,
            "label_quality_downstream_lift_summary": {
                "path": str(output_root / "label_quality_downstream_lift_summary.json"),
                "bytes": (output_root / "label_quality_downstream_lift_summary.json").stat().st_size,
                "sha256": summary_sha,
            },
        },
        "checks": checks,
    }
    manifest_sha = atomic_json_dump(output_root / "label_quality_downstream_lift_manifest.json", manifest)
    manifest["outputs"]["label_quality_downstream_lift_manifest"] = {
        "path": str(output_root / "label_quality_downstream_lift_manifest.json"),
        "bytes": (output_root / "label_quality_downstream_lift_manifest.json").stat().st_size,
        "sha256": manifest_sha,
    }
    return manifest


def require_object(value: Any, name: str) -> dict[str, Any]:
    require(isinstance(value, dict), "LABEL_QUALITY_LIFT_OBJECT_REQUIRED", name)
    return value


def require_array(value: Any, name: str) -> list[Any]:
    require(isinstance(value, list), "LABEL_QUALITY_LIFT_ARRAY_REQUIRED", name)
    return value


def required_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    require(isinstance(value, str) and value, "LABEL_QUALITY_LIFT_STRING_REQUIRED", key, {"row": row})
    return value


def required_bool(row: dict[str, Any], key: str) -> bool:
    value = row.get(key)
    require(isinstance(value, bool), "LABEL_QUALITY_LIFT_BOOL_REQUIRED", key, {"row": row})
    return value


def required_score(row: dict[str, Any], key: str) -> float:
    value = row.get(key)
    require(isinstance(value, (int, float)) and math.isfinite(float(value)), "LABEL_QUALITY_LIFT_SCORE_REQUIRED", key)
    score = float(value)
    require(0.0 <= score <= 1.0, "LABEL_QUALITY_LIFT_SCORE_RANGE", key, {"score": score})
    return score


def load_prediction_rows(
    execute: dict[str, Any],
    *,
    split: str,
    preferred_key: str,
    fallback_key: str,
    expected: int,
    active_flags: dict[tuple[str, str], bool],
) -> list[PredictionRow]:
    key = preferred_key if isinstance(execute.get(preferred_key), list) else fallback_key
    values = require_array(execute.get(key), key)
    require(len(values) == expected, "LABEL_QUALITY_LIFT_SPLIT_COUNT_MISMATCH", key, {"expected": expected, "actual": len(values)})
    rows = []
    for index, value in enumerate(values):
        row = require_object(value, f"{key}[{index}]")
        patch_path = required_string(row, "patch_path")
        reject_mounted_drive("patch_path", Path(patch_path))
        task_id = required_string(row, "task_id")
        category = required_string(row, "category")
        rows.append(
            PredictionRow(
                split=split,
                row_index=index,
                task_id=task_id,
                repo=required_string(row, "repo"),
                category=category,
                patch_path=patch_path,
                patch_sha256=required_string(row, "patch_sha256"),
                label=1 if required_bool(row, "actual_oracle_pass") else 0,
                score=required_score(row, "predicted_oracle_pass"),
                active_after_abstain=active_flags.get((task_id, category)) if split == "holdout" else None,
            )
        )
    return rows


def load_active_flags(active: dict[str, Any]) -> dict[tuple[str, str], bool]:
    out: dict[tuple[str, str], bool] = {}
    rows = require_array(active.get("score_sharpness_abstain_holdout_row_diagnostics"), "score_sharpness_abstain_holdout_row_diagnostics")
    for index, value in enumerate(rows):
        row = require_object(value, f"active_holdout[{index}]")
        task_id = required_string(row, "task_id")
        category = required_string(row, "category")
        active_value = row.get("active_after_abstain")
        require(isinstance(active_value, bool), "LABEL_QUALITY_LIFT_ACTIVE_FLAG_REQUIRED", f"{task_id}:{category}")
        out[(task_id, category)] = active_value
    return out


def load_quality_scores(path: Path) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for _line_no, row in iter_jsonl(path):
        cluster_id = str(row.get("cluster_id") or "")
        require(cluster_id, "LABEL_QUALITY_LIFT_CLUSTER_ID_MISSING", str(path))
        out[cluster_id] = {
            "label_id": row.get("label_id"),
            "label_name": row.get("label_name"),
            "embedder_id": row.get("embedder_id"),
            "quality_class": row.get("quality_class"),
            "live_prediction_feature_allowed": row.get("live_prediction_feature_allowed") is True,
            "signal_score": row.get("signal_score"),
            "noise_score": row.get("noise_score"),
            "physical_validity_score": row.get("physical_validity_score"),
            "interpretability_score": row.get("interpretability_score"),
            "stability_score": row.get("stability_score"),
            "support_count": row.get("support_count"),
        }
    return out


def collect_features(
    path: Path,
    quality_by_cluster: dict[str, dict[str, Any]],
    *,
    target_keys: set[tuple[str, str, str]],
    expected_embedders: tuple[str, ...],
) -> dict[tuple[str, str, str, str], FeatureAccumulator]:
    features: dict[tuple[str, str, str, str], FeatureAccumulator] = defaultdict(FeatureAccumulator)
    expected = set(expected_embedders)
    target_splits_by_cell: dict[tuple[str, str], set[str]] = defaultdict(set)
    for split, task_id, category in target_keys:
        target_splits_by_cell[(task_id, category)].add(split)
    seen = 0
    retained = 0
    for _line_no, row in iter_jsonl(path):
        partition = str(row.get("partition") or "")
        task_id = str(row.get("task_instance_id") or "")
        category = str(row.get("mutation_category") or "")
        embedder_id = str(row.get("embedder_id") or "")
        if embedder_id not in expected:
            continue
        key = (partition, task_id, category)
        seen += 1
        if key in target_keys:
            target_splits = (partition,)
        elif partition in ("", "unknown"):
            target_splits = tuple(sorted(target_splits_by_cell.get((task_id, category), ())))
        else:
            continue
        cluster_id = str(row.get("cluster_id") or "")
        require(cluster_id, "LABEL_QUALITY_LIFT_MEMBERSHIP_CLUSTER_ID_MISSING", f"{path}:{_line_no}")
        quality = quality_by_cluster.get(cluster_id)
        for split in target_splits:
            features[(split, task_id, category, embedder_id)].add(quality)
            retained += 1
    require(retained > 0, "LABEL_QUALITY_LIFT_NO_MEMBERSHIPS_JOINED", str(path), {"seen": seen})
    return features


def write_feature_rows(
    feature_rows_path: Path,
    source_samples_path: Path,
    rows: list[PredictionRow],
    features: dict[tuple[str, str, str, str], FeatureAccumulator],
    *,
    expected_embedders: tuple[str, ...],
) -> tuple[dict[str, Any], dict[str, Any]]:
    with JsonlSink(feature_rows_path) as feature_sink, JsonlSink(source_samples_path) as sample_sink:
        sample_count = 0
        for row in rows:
            for embedder_id in expected_embedders:
                feature = features[(row.split, row.task_id, row.category, embedder_id)].to_feature()
                payload = {
                    "artifact_kind": "label_quality_row_feature",
                    "schema_version": SCHEMA_VERSION,
                    "formula_version": FORMULA_VERSION,
                    "split": row.split,
                    "row_index": row.row_index,
                    "task_id": row.task_id,
                    "category": row.category,
                    "embedder_id": embedder_id,
                    "actual_oracle_pass": bool(row.label),
                    "base_predicted_oracle_pass": row.score,
                    "active_after_abstain": row.active_after_abstain,
                    **feature,
                }
                feature_sink.write(payload)
                if sample_count < 36 and feature["membership_rows"] > 0:
                    sample_sink.write(payload)
                    sample_count += 1
    return feature_sink.artifact(), sample_sink.artifact()


def build_lift_rows(
    rows: list[PredictionRow],
    features: dict[tuple[str, str, str, str], FeatureAccumulator],
    *,
    expected_embedders: tuple[str, ...],
    smooth_ece_regression_budget: float,
    min_featured_rows: int,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    lift_rows = []
    summary_rows = []
    calibration = [row for row in rows if row.split == "calibration"]
    holdout = [row for row in rows if row.split == "holdout"]
    for embedder_id in expected_embedders:
        cal_values = [feature_value(row, embedder_id, features) for row in calibration]
        mean = sum(cal_values) / len(cal_values)
        variance = sum((value - mean) ** 2 for value in cal_values) / len(cal_values)
        std = math.sqrt(variance)
        featured_cal = sum(1 for row in calibration if feature_membership_count(row, embedder_id, features) > 0)
        featured_hold = sum(1 for row in holdout if feature_membership_count(row, embedder_id, features) > 0)
        beta, calibration_scores, calibration_metrics = select_beta(calibration, cal_values, mean, std)
        holdout_scores = [adjust_score(row.score, beta, zscore(feature_value(row, embedder_id, features), mean, std)) for row in holdout]
        baseline_holdout = metric_block([row.label for row in holdout], [row.score for row in holdout])
        adjusted_holdout = metric_block([row.label for row in holdout], holdout_scores)
        active_before, active_after = subset_metrics(holdout, [row.score for row in holdout], holdout_scores, True)
        abstain_before, abstain_after = subset_metrics(holdout, [row.score for row in holdout], holdout_scores, False)
        per_cell = per_cell_metrics(holdout, [row.score for row in holdout], holdout_scores)
        weak_association = weak_cell_association(holdout, embedder_id, features, mean, std)
        regressions = find_regressions(per_cell, smooth_ece_regression_budget=smooth_ece_regression_budget)
        aggregate_delta = metric_delta(baseline_holdout, adjusted_holdout)
        active_delta = metric_delta(active_before, active_after)
        abstain_delta = metric_delta(abstain_before, abstain_after)
        blockers = []
        if std <= 1.0e-12:
            blockers.append("calibration_feature_variance_degenerate")
        if featured_cal < min_featured_rows or featured_hold < min_featured_rows:
            blockers.append("insufficient_featured_rows")
        if (aggregate_delta.get("brier_delta") or 0.0) >= 0.0:
            blockers.append("holdout_brier_not_improved")
        if (aggregate_delta.get("smooth_ece_delta") or 0.0) > smooth_ece_regression_budget:
            blockers.append("holdout_smooth_ece_regressed")
        if regressions["weak_cell_regressions"]:
            blockers.append("weak_cell_regression")
        promotion_allowed = not blockers
        row = {
            "artifact_kind": ARTIFACT_KIND,
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "embedder_id": embedder_id,
            "out_of_fold_lift_status": "measured_calibration_to_holdout",
            "diagnostic_only": False,
            "fit_partition": {
                "calibration_rows_used": len(calibration),
                "holdout_rows_used_for_fitting": 0,
                "selected_beta": beta,
                "feature_mean": mean,
                "feature_std": std,
                "featured_calibration_rows": featured_cal,
                "calibration_metrics_after_beta": calibration_metrics,
            },
            "report_partition": {"holdout_rows": len(holdout), "featured_holdout_rows": featured_hold},
            "baseline_holdout_metrics": baseline_holdout,
            "adjusted_holdout_metrics": adjusted_holdout,
            "metric_deltas": aggregate_delta,
            "active_pool": {"before": active_before, "after": active_after, "deltas": active_delta},
            "abstain_pool": {"before": abstain_before, "after": abstain_after, "deltas": abstain_delta},
            "weak_cell_association": weak_association,
            "per_cell_regression_check": regressions,
            "prediction_promotion_allowed": promotion_allowed,
            "promotion_blockers": blockers,
        }
        lift_rows.append(row)
        summary_rows.append(
            {
                "embedder_id": embedder_id,
                "selected_beta": beta,
                "prediction_promotion_allowed": promotion_allowed,
                "promotion_blockers": blockers,
                "brier_delta": aggregate_delta.get("brier_delta"),
                "smooth_ece_delta": aggregate_delta.get("smooth_ece_delta"),
                "abstain_pool_smooth_ece_delta": abstain_delta.get("smooth_ece_delta"),
                "weak_cell_regressions": regressions["weak_cell_regressions"],
            }
        )
    return lift_rows, summary_rows


def feature_value(row: PredictionRow, embedder_id: str, features: dict[tuple[str, str, str, str], FeatureAccumulator]) -> float:
    return float(features[(row.split, row.task_id, row.category, embedder_id)].to_feature()["signal_minus_noise"])


def feature_membership_count(row: PredictionRow, embedder_id: str, features: dict[tuple[str, str, str, str], FeatureAccumulator]) -> int:
    return int(features[(row.split, row.task_id, row.category, embedder_id)].membership_rows)


def zscore(value: float, mean: float, std: float) -> float:
    if std <= 1.0e-12:
        return 0.0
    return (value - mean) / std


def select_beta(rows: list[PredictionRow], feature_values: list[float], mean: float, std: float) -> tuple[float, list[float], dict[str, Any]]:
    labels = [row.label for row in rows]
    best_beta = 0.0
    best_scores = [row.score for row in rows]
    best_metrics = metric_block(labels, best_scores)
    for beta in BETA_GRID:
        scores = [adjust_score(row.score, beta, zscore(value, mean, std)) for row, value in zip(rows, feature_values)]
        metrics = metric_block(labels, scores)
        if better(metrics, best_metrics):
            best_beta = beta
            best_scores = scores
            best_metrics = metrics
    return best_beta, best_scores, best_metrics


def better(candidate: dict[str, Any], incumbent: dict[str, Any]) -> bool:
    cb = candidate.get("brier")
    ib = incumbent.get("brier")
    if cb is None or ib is None:
        return False
    if cb < ib - 1.0e-9:
        return True
    if abs(cb - ib) <= 1.0e-9:
        ce = candidate.get("smooth_ece")
        ie = incumbent.get("smooth_ece")
        if ce is not None and ie is not None and ce < ie:
            return True
    return False


def adjust_score(base: float, beta: float, z_value: float) -> float:
    return max(0.001, min(0.999, sigmoid(logit(base) + beta * z_value)))


def logit(value: float) -> float:
    p = max(0.001, min(0.999, value))
    return math.log(p / (1.0 - p))


def sigmoid(value: float) -> float:
    if value >= 0:
        denom = 1.0 + math.exp(-value)
        return 1.0 / denom
    exp_value = math.exp(value)
    return exp_value / (1.0 + exp_value)


def subset_metrics(rows: list[PredictionRow], before: list[float], after: list[float], active_value: bool) -> tuple[dict[str, Any], dict[str, Any]]:
    selected = [(row, b, a) for row, b, a in zip(rows, before, after) if row.active_after_abstain is active_value]
    labels = [row.label for row, _b, _a in selected]
    return metric_block(labels, [b for _row, b, _a in selected]), metric_block(labels, [a for _row, _b, a in selected])


def per_cell_metrics(rows: list[PredictionRow], before: list[float], after: list[float]) -> dict[str, dict[str, Any]]:
    by_cell: dict[str, list[tuple[PredictionRow, float, float]]] = defaultdict(list)
    for row, b, a in zip(rows, before, after):
        by_cell[row.category].append((row, b, a))
    out = {}
    for cell, selected in sorted(by_cell.items()):
        labels = [row.label for row, _b, _a in selected]
        out[cell] = {
            "before": metric_block(labels, [b for _row, b, _a in selected]),
            "after": metric_block(labels, [a for _row, _b, a in selected]),
            "delta": metric_delta(
                metric_block(labels, [b for _row, b, _a in selected]),
                metric_block(labels, [a for _row, _b, a in selected]),
            ),
        }
    return out


def metric_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    return {
        "correlation_delta": none_delta(before.get("prediction_oracle_correlation"), after.get("prediction_oracle_correlation")),
        "brier_delta": none_delta(before.get("brier"), after.get("brier")),
        "smooth_ece_delta": none_delta(before.get("smooth_ece"), after.get("smooth_ece")),
        "accuracy_delta": none_delta(before.get("accuracy_at_half"), after.get("accuracy_at_half")),
    }


def none_delta(before: Any, after: Any) -> float | None:
    if before is None or after is None:
        return None
    return float(after) - float(before)


def weak_cell_association(
    rows: list[PredictionRow],
    embedder_id: str,
    features: dict[tuple[str, str, str, str], FeatureAccumulator],
    mean: float,
    std: float,
) -> dict[str, Any]:
    out = {}
    for cell in WEAK_CELLS:
        selected = [row for row in rows if row.category == cell]
        feature_scores = [zscore(feature_value(row, embedder_id, features), mean, std) for row in selected]
        labels = [row.label for row in selected]
        errors = [abs(row.score - row.label) for row in selected]
        out[cell] = {
            "n": len(selected),
            "feature_oracle_correlation": pearson_float_or_none([float(v) for v in labels], feature_scores),
            "feature_baseline_error_correlation": pearson_float_or_none(errors, feature_scores),
            "feature_mean": sum(feature_scores) / len(feature_scores) if feature_scores else None,
            "oracle_pass_rate": sum(labels) / len(labels) if labels else None,
        }
    return out


def find_regressions(per_cell: dict[str, dict[str, Any]], *, smooth_ece_regression_budget: float) -> dict[str, Any]:
    regressions = []
    weak_regressions = []
    for cell, metrics in sorted(per_cell.items()):
        delta = metrics["delta"]
        reasons = []
        brier_delta = delta.get("brier_delta")
        ece_delta = delta.get("smooth_ece_delta")
        corr_delta = delta.get("correlation_delta")
        if brier_delta is not None and brier_delta > 1.0e-9:
            reasons.append(f"brier_regressed:{brier_delta:.9f}")
        if ece_delta is not None and ece_delta > smooth_ece_regression_budget:
            reasons.append(f"smooth_ece_regressed:{ece_delta:.9f}")
        if corr_delta is not None and corr_delta < -1.0e-9:
            reasons.append(f"correlation_regressed:{corr_delta:.9f}")
        if reasons:
            item = {"cell": cell, "reasons": reasons, "delta": delta}
            regressions.append(item)
            if cell in WEAK_CELLS:
                weak_regressions.append(item)
    return {"all_regressions": regressions, "weak_cell_regressions": weak_regressions}


def inventory(path: Path) -> dict[str, Any]:
    return {"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path)}


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--label-quality-root", type=Path, default=DEFAULT_LABEL_QUALITY_ROOT)
    parser.add_argument("--geometry-label-root", type=Path, default=DEFAULT_GEOMETRY_LABEL_ROOT)
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--active-abstain-report", type=Path, default=DEFAULT_ACTIVE_ABSTAIN_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--expected-embedders", default="e1,e6,e7,e8,e9,e10,e12,e13,e14")
    parser.add_argument("--smooth-ece-regression-budget", type=float, default=0.003)
    parser.add_argument("--min-featured-rows", type=int, default=30)
    parser.add_argument("--clean-output", action="store_true")
