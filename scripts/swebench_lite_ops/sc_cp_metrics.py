"""Self-calibrating conformal metrics for #761.

This module implements the binary slice needed by the ship-gate FSV:

* Venn-Abers-style calibrated probability intervals from the calibration
  split.
* Split-conformal expansion of those intervals using calibration residuals.
* Holdout-only reporting of interval coverage, class-set coverage, Brier, ECE,
  and per-cell coverage.

It is intentionally dependency-light (numpy only) so aiwonder can run it in
the existing SWE-bench venv without installing external conformal packages.
"""

from __future__ import annotations

import math
from collections import Counter, defaultdict
from dataclasses import dataclass
from typing import Any, Iterable, Sequence

import numpy as np

from .honest_pearson_metrics import (
    brier_score,
    ece_l1,
    isotonic_regression,
    pearson_correlation,
)


@dataclass(frozen=True)
class ScCpRow:
    row_index: int
    task_id: str
    repo: str
    category: str
    language: str
    raw_score: float
    label: int
    va_lower: float
    va_upper: float
    va_point: float
    prediction_bin: int
    interval_tau: float
    interval_tau_source: str
    interval_low: float
    interval_high: float
    interval_covered: bool
    class_tau: float
    class_tau_source: str
    class_set: tuple[int, ...]
    class_set_covered: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "row_index": self.row_index,
            "task_id": self.task_id,
            "repo": self.repo,
            "category": self.category,
            "language": self.language,
            "cell": cell_key(self.category, self.language),
            "raw_score": round_float(self.raw_score),
            "label": self.label,
            "venn_abers": {
                "lower": round_float(self.va_lower),
                "upper": round_float(self.va_upper),
                "point": round_float(self.va_point),
                "width": round_float(self.va_upper - self.va_lower),
            },
            "prediction_bin": self.prediction_bin,
            "probability_interval": {
                "low": round_float(self.interval_low),
                "high": round_float(self.interval_high),
                "tau": round_float(self.interval_tau),
                "tau_source": self.interval_tau_source,
                "covered": self.interval_covered,
            },
            "classification_set": {
                "classes": list(self.class_set),
                "tau": round_float(self.class_tau),
                "tau_source": self.class_tau_source,
                "covered": self.class_set_covered,
            },
        }


@dataclass(frozen=True)
class VennAbersPrediction:
    lower: float
    upper: float
    point: float


def round_float(value: float) -> float | None:
    if not math.isfinite(value):
        return None
    return float(round(value, 10))


def cell_key(category: str, language: str) -> str:
    return f"{category}::{language}"


def conformal_tau(scores: Sequence[float], alpha: float) -> float:
    if not scores:
        raise ValueError("conformal_tau: requires at least one score")
    if not math.isfinite(alpha) or alpha <= 0.0 or alpha >= 1.0:
        raise ValueError(f"conformal_tau: alpha must be in (0,1), got {alpha!r}")
    finite = [float(score) for score in scores]
    if not all(math.isfinite(score) for score in finite):
        raise ValueError("conformal_tau: non-finite score")
    finite.sort()
    idx = math.ceil((len(finite) + 1) * (1.0 - alpha)) - 1
    idx = max(0, min(idx, len(finite) - 1))
    return min(1.0, max(0.0, finite[idx]))


def prediction_bin(probability: float, bin_count: int) -> int:
    if bin_count <= 0:
        raise ValueError("prediction_bin: bin_count must be positive")
    if not math.isfinite(probability):
        raise ValueError("prediction_bin: probability must be finite")
    clipped = min(1.0, max(0.0, probability))
    return min(bin_count - 1, int(clipped * bin_count))


def venn_abers_predict(
    calibration_scores: Sequence[float],
    calibration_labels: Sequence[int],
    query_score: float,
) -> VennAbersPrediction:
    if len(calibration_scores) != len(calibration_labels):
        raise ValueError(
            "venn_abers_predict: calibration_scores/calibration_labels length mismatch"
        )
    if not calibration_scores:
        raise ValueError("venn_abers_predict: requires calibration rows")
    if not math.isfinite(query_score):
        raise ValueError("venn_abers_predict: query score must be finite")
    scores = [float(score) for score in calibration_scores]
    labels = [float(label) for label in calibration_labels]
    if not all(math.isfinite(score) for score in scores):
        raise ValueError("venn_abers_predict: non-finite calibration score")
    if any(label not in (0.0, 1.0) for label in labels):
        raise ValueError("venn_abers_predict: labels must be binary 0/1")
    p0 = _isotonic_query_value(scores, labels, float(query_score), 0.0)
    p1 = _isotonic_query_value(scores, labels, float(query_score), 1.0)
    lower = min(p0, p1)
    upper = max(p0, p1)
    denom = 1.0 - p0 + p1
    if denom <= 1.0e-12:
        point = (lower + upper) / 2.0
    else:
        point = p1 / denom
    return VennAbersPrediction(
        lower=min(1.0, max(0.0, lower)),
        upper=min(1.0, max(0.0, upper)),
        point=min(1.0, max(0.0, point)),
    )


def _isotonic_query_value(
    calibration_scores: Sequence[float],
    calibration_labels: Sequence[float],
    query_score: float,
    query_label: float,
) -> float:
    scores = list(calibration_scores) + [float(query_score)]
    labels = list(calibration_labels) + [float(query_label)]
    fitted = isotonic_regression(scores, labels)
    return float(fitted[-1])


def leave_one_out_venn_abers(
    calibration_scores: Sequence[float], calibration_labels: Sequence[int]
) -> list[VennAbersPrediction]:
    if len(calibration_scores) != len(calibration_labels):
        raise ValueError("leave_one_out_venn_abers: length mismatch")
    if len(calibration_scores) < 2:
        raise ValueError("leave_one_out_venn_abers: requires at least two rows")
    out: list[VennAbersPrediction] = []
    scores = list(calibration_scores)
    labels = list(calibration_labels)
    for idx, score in enumerate(scores):
        train_scores = scores[:idx] + scores[idx + 1 :]
        train_labels = labels[:idx] + labels[idx + 1 :]
        out.append(venn_abers_predict(train_scores, train_labels, score))
    return out


def interval_residual(label: int, lower: float, upper: float) -> float:
    target = float(label)
    if target < lower:
        return lower - target
    if target > upper:
        return target - upper
    return 0.0


def class_residual(label: int, point_probability: float) -> float:
    if label == 1:
        return 1.0 - point_probability
    if label == 0:
        return point_probability
    raise ValueError(f"class_residual: label must be 0/1, got {label!r}")


def classification_set(point_probability: float, tau: float) -> tuple[int, ...]:
    classes: list[int] = []
    if point_probability <= tau:
        classes.append(0)
    if 1.0 - point_probability <= tau:
        classes.append(1)
    return tuple(classes)


def build_sc_cp_report(
    calibration_rows: Sequence[dict[str, Any]],
    holdout_rows: Sequence[dict[str, Any]],
    *,
    alpha: float = 0.10,
    bin_count: int = 5,
    min_bin_size: int = 20,
    min_cell_rows: int = 20,
    source_label: str = "ship_gate_fsv",
) -> dict[str, Any]:
    cal = [_coerce_input_row(row, idx, "calibration") for idx, row in enumerate(calibration_rows)]
    hol = [_coerce_input_row(row, idx, "holdout") for idx, row in enumerate(holdout_rows)]
    if len(cal) < 2:
        raise ValueError("build_sc_cp_report: calibration split requires at least two rows")
    if not hol:
        raise ValueError("build_sc_cp_report: holdout split is empty")
    cal_scores = [row["score"] for row in cal]
    cal_labels = [row["label"] for row in cal]
    cal_va = leave_one_out_venn_abers(cal_scores, cal_labels)
    interval_residuals = [
        interval_residual(row["label"], va.lower, va.upper)
        for row, va in zip(cal, cal_va)
    ]
    class_residuals = [
        class_residual(row["label"], va.point)
        for row, va in zip(cal, cal_va)
    ]
    cal_bins = [prediction_bin(va.point, bin_count) for va in cal_va]
    global_interval_tau = conformal_tau(interval_residuals, alpha)
    global_class_tau = conformal_tau(class_residuals, alpha)
    interval_bin_tau, interval_bin_counts = _bin_taus(
        cal_bins, interval_residuals, alpha, bin_count, min_bin_size, global_interval_tau
    )
    class_bin_tau, class_bin_counts = _bin_taus(
        cal_bins, class_residuals, alpha, bin_count, min_bin_size, global_class_tau
    )

    rows: list[ScCpRow] = []
    for raw in hol:
        va = venn_abers_predict(cal_scores, cal_labels, raw["score"])
        bin_id = prediction_bin(va.point, bin_count)
        interval_tau = interval_bin_tau[bin_id]
        interval_tau_source = (
            f"prediction_bin_{bin_id}"
            if interval_bin_counts[bin_id] >= min_bin_size
            else "global_fallback"
        )
        class_tau = class_bin_tau[bin_id]
        class_tau_source = (
            f"prediction_bin_{bin_id}"
            if class_bin_counts[bin_id] >= min_bin_size
            else "global_fallback"
        )
        low = max(0.0, va.lower - interval_tau)
        high = min(1.0, va.upper + interval_tau)
        label = int(raw["label"])
        class_set = classification_set(va.point, class_tau)
        rows.append(
            ScCpRow(
                row_index=int(raw["row_index"]),
                task_id=str(raw["task_id"]),
                repo=str(raw["repo"]),
                category=str(raw["category"]),
                language=str(raw["language"]),
                raw_score=float(raw["score"]),
                label=label,
                va_lower=va.lower,
                va_upper=va.upper,
                va_point=va.point,
                prediction_bin=bin_id,
                interval_tau=interval_tau,
                interval_tau_source=interval_tau_source,
                interval_low=low,
                interval_high=high,
                interval_covered=(float(label) >= low and float(label) <= high),
                class_tau=class_tau,
                class_tau_source=class_tau_source,
                class_set=class_set,
                class_set_covered=(label in class_set),
            )
        )

    raw_scores = [row["score"] for row in hol]
    labels = [row["label"] for row in hol]
    va_points = [row.va_point for row in rows]
    global_block = coverage_block(rows, min_rows=1)
    per_cell = {
        cell: coverage_block(cell_rows, min_rows=min_cell_rows)
        for cell, cell_rows in sorted(_group_rows_by_cell(rows).items())
    }
    eligible_cells = {
        cell: block
        for cell, block in per_cell.items()
        if block["eligible_for_decision"] is True
    }
    coverage_min = 1.0 - alpha - 0.02
    coverage_max = 1.0 - alpha + 0.02
    global_in_band = _in_band(global_block["interval_coverage"], coverage_min, coverage_max)
    eligible_out_of_band = [
        cell
        for cell, block in eligible_cells.items()
        if not _in_band(block["interval_coverage"], coverage_min, coverage_max)
    ]
    brier_raw = brier_score(raw_scores, labels)
    brier_va = brier_score(va_points, labels)
    ece_raw = ece_l1(raw_scores, labels, bins=10)
    ece_va = ece_l1(va_points, labels, bins=10)
    calibration_regressions = []
    if brier_va > brier_raw + 1.0e-9:
        calibration_regressions.append("brier_regressed")
    if ece_va > ece_raw + 1.0e-9:
        calibration_regressions.append("ece_regressed")
    promotion_allowed = (
        global_in_band
        and not eligible_out_of_band
        and not calibration_regressions
        and len(eligible_cells) == len(per_cell)
    )
    return {
        "schema_version": 1,
        "artifact_kind": "inv_017_sc_cp_report",
        "source_label": source_label,
        "policy": {
            "method": "venn_abers_split_conformal_probability_interval",
            "alpha": alpha,
            "target_coverage": 1.0 - alpha,
            "coverage_band_min": coverage_min,
            "coverage_band_max": coverage_max,
            "prediction_bin_count": bin_count,
            "min_bin_size": min_bin_size,
            "min_cell_rows": min_cell_rows,
            "fit_partition": "calibration_only",
            "holdout_used_for_fitting": False,
        },
        "calibration_fit": {
            "sample_count": len(cal),
            "label_counts": _label_counts(cal_labels),
            "global_interval_tau": round_float(global_interval_tau),
            "global_class_tau": round_float(global_class_tau),
            "prediction_bin_counts": {
                str(idx): interval_bin_counts[idx] for idx in range(bin_count)
            },
            "interval_tau_by_prediction_bin": {
                str(idx): round_float(interval_bin_tau[idx]) for idx in range(bin_count)
            },
            "class_tau_by_prediction_bin": {
                str(idx): round_float(class_bin_tau[idx]) for idx in range(bin_count)
            },
            "fallback_bins": [
                idx for idx, count in enumerate(interval_bin_counts) if count < min_bin_size
            ],
        },
        "holdout": {
            **global_block,
            "raw_metrics": metric_payload(raw_scores, labels),
            "venn_abers_point_metrics": metric_payload(va_points, labels),
            "brier_delta_va_minus_raw": round_float(brier_va - brier_raw),
            "ece_delta_va_minus_raw": round_float(ece_va - ece_raw),
            "raw_prediction_pearson": round_float(pearson_correlation(raw_scores, labels)),
            "venn_abers_point_pearson": round_float(pearson_correlation(va_points, labels)),
        },
        "per_cell": per_cell,
        "decision": {
            "promotion_allowed": promotion_allowed,
            "global_interval_coverage_in_band": global_in_band,
            "eligible_cell_count": len(eligible_cells),
            "cell_count": len(per_cell),
            "eligible_cells_outside_band": eligible_out_of_band,
            "cells_below_min_rows": [
                cell for cell, block in per_cell.items() if not block["eligible_for_decision"]
            ],
            "calibration_regressions": calibration_regressions,
            "result": (
                "promote_sc_cp_default"
                if promotion_allowed
                else "do_not_promote_sc_cp_on_current_split"
            ),
        },
        "row_predictions": [row.to_dict() for row in rows],
    }


def build_mondrian_conformal_report(
    calibration_rows: Sequence[dict[str, Any]],
    holdout_rows: Sequence[dict[str, Any]],
    *,
    alpha: float = 0.10,
    min_cell_rows: int = 20,
    in_band_cell_fraction: float = 0.80,
    source_label: str = "ship_gate_fsv",
) -> dict[str, Any]:
    cal = [_coerce_input_row(row, idx, "calibration") for idx, row in enumerate(calibration_rows)]
    hol = [_coerce_input_row(row, idx, "holdout") for idx, row in enumerate(holdout_rows)]
    if len(cal) < 2:
        raise ValueError(
            "build_mondrian_conformal_report: calibration split requires at least two rows"
        )
    if not hol:
        raise ValueError("build_mondrian_conformal_report: holdout split is empty")
    if min_cell_rows < 2:
        raise ValueError("build_mondrian_conformal_report: min_cell_rows must be >= 2")
    if (
        not math.isfinite(in_band_cell_fraction)
        or in_band_cell_fraction <= 0.0
        or in_band_cell_fraction > 1.0
    ):
        raise ValueError(
            "build_mondrian_conformal_report: in_band_cell_fraction must be in (0,1]"
        )

    cal_scores = [row["score"] for row in cal]
    cal_labels = [row["label"] for row in cal]
    global_cal_va = leave_one_out_venn_abers(cal_scores, cal_labels)
    global_interval_tau = conformal_tau(
        [
            interval_residual(row["label"], va.lower, va.upper)
            for row, va in zip(cal, global_cal_va)
        ],
        alpha,
    )
    global_class_tau = conformal_tau(
        [class_residual(row["label"], va.point) for row, va in zip(cal, global_cal_va)],
        alpha,
    )

    cell_fits = _mondrian_cell_fits(cal, alpha=alpha, min_cell_rows=min_cell_rows)
    rows: list[ScCpRow] = []
    for raw in hol:
        cell = cell_key(str(raw["category"]), str(raw["language"]))
        fit = cell_fits.get(cell)
        if fit is not None and fit["eligible"]:
            fit_scores = fit["scores"]
            fit_labels = fit["labels"]
            va = venn_abers_predict(fit_scores, fit_labels, raw["score"])
            interval_tau = float(fit["interval_tau"])
            class_tau = float(fit["class_tau"])
            interval_tau_source = f"mondrian_cell:{cell}"
            class_tau_source = f"mondrian_cell:{cell}"
        else:
            va = venn_abers_predict(cal_scores, cal_labels, raw["score"])
            interval_tau = global_interval_tau
            class_tau = global_class_tau
            interval_tau_source = f"global_diagnostic_fallback:{cell}"
            class_tau_source = f"global_diagnostic_fallback:{cell}"
        low = max(0.0, va.lower - interval_tau)
        high = min(1.0, va.upper + interval_tau)
        label = int(raw["label"])
        class_set = classification_set(va.point, class_tau)
        rows.append(
            ScCpRow(
                row_index=int(raw["row_index"]),
                task_id=str(raw["task_id"]),
                repo=str(raw["repo"]),
                category=str(raw["category"]),
                language=str(raw["language"]),
                raw_score=float(raw["score"]),
                label=label,
                va_lower=va.lower,
                va_upper=va.upper,
                va_point=va.point,
                prediction_bin=-1,
                interval_tau=interval_tau,
                interval_tau_source=interval_tau_source,
                interval_low=low,
                interval_high=high,
                interval_covered=(float(label) >= low and float(label) <= high),
                class_tau=class_tau,
                class_tau_source=class_tau_source,
                class_set=class_set,
                class_set_covered=(label in class_set),
            )
        )

    raw_scores = [row["score"] for row in hol]
    labels = [row["label"] for row in hol]
    va_points = [row.va_point for row in rows]
    global_block = coverage_block(rows, min_rows=1)
    holdout_by_cell = _group_rows_by_cell(rows)
    per_cell = {
        cell: _mondrian_cell_report_block(
            cell,
            cell_rows,
            cell_fits.get(cell),
            min_cell_rows=min_cell_rows,
        )
        for cell, cell_rows in sorted(holdout_by_cell.items())
    }
    coverage_min = 1.0 - alpha - 0.02
    coverage_max = 1.0 - alpha + 0.02
    eligible_cells = {
        cell: block
        for cell, block in per_cell.items()
        if block["eligible_for_decision"] is True
        and block["mondrian_fit_eligible"] is True
    }
    cells_in_band = [
        cell
        for cell, block in eligible_cells.items()
        if _in_band(block["interval_coverage"], coverage_min, coverage_max)
    ]
    cells_below_band = [
        cell
        for cell, block in eligible_cells.items()
        if isinstance(block["interval_coverage"], (int, float))
        and float(block["interval_coverage"]) < coverage_min
    ]
    cells_above_band = [
        cell
        for cell, block in eligible_cells.items()
        if isinstance(block["interval_coverage"], (int, float))
        and float(block["interval_coverage"]) > coverage_max
    ]
    eligible_cell_fraction_in_band = (
        len(cells_in_band) / len(eligible_cells) if eligible_cells else 0.0
    )
    support_gap_cells = [
        cell
        for cell, block in per_cell.items()
        if block["eligible_for_decision"] is not True
        or block["mondrian_fit_eligible"] is not True
    ]
    brier_raw = brier_score(raw_scores, labels)
    brier_va = brier_score(va_points, labels)
    ece_raw = ece_l1(raw_scores, labels, bins=10)
    ece_va = ece_l1(va_points, labels, bins=10)
    calibration_regressions = []
    if brier_va > brier_raw + 1.0e-9:
        calibration_regressions.append("brier_regressed")
    if ece_va > ece_raw + 1.0e-9:
        calibration_regressions.append("ece_regressed")
    promotion_allowed = (
        len(eligible_cells) > 0
        and not support_gap_cells
        and eligible_cell_fraction_in_band >= in_band_cell_fraction
        and not calibration_regressions
    )
    return {
        "schema_version": 1,
        "artifact_kind": "inv_019_mondrian_conformal_report",
        "source_label": source_label,
        "policy": {
            "method": "mondrian_cell_venn_abers_split_conformal",
            "mondrian_category": "mutation_category_x_language",
            "alpha": alpha,
            "target_coverage": 1.0 - alpha,
            "coverage_band_min": coverage_min,
            "coverage_band_max": coverage_max,
            "min_cell_rows": min_cell_rows,
            "required_eligible_cell_fraction_in_band": in_band_cell_fraction,
            "fit_partition": "calibration_only",
            "holdout_used_for_fitting": False,
            "diagnostic_global_fallback_blocks_promotion": True,
        },
        "calibration_fit": {
            "sample_count": len(cal),
            "label_counts": _label_counts(cal_labels),
            "cell_count": len(cell_fits),
            "eligible_cell_fit_count": sum(
                1 for fit in cell_fits.values() if fit["eligible"] is True
            ),
            "global_diagnostic_interval_tau": round_float(global_interval_tau),
            "global_diagnostic_class_tau": round_float(global_class_tau),
            "per_cell": {
                cell: _public_mondrian_fit_block(fit)
                for cell, fit in sorted(cell_fits.items())
            },
        },
        "holdout": {
            **global_block,
            "raw_metrics": metric_payload(raw_scores, labels),
            "mondrian_venn_abers_point_metrics": metric_payload(va_points, labels),
            "brier_delta_mondrian_minus_raw": round_float(brier_va - brier_raw),
            "ece_delta_mondrian_minus_raw": round_float(ece_va - ece_raw),
            "raw_prediction_pearson": round_float(pearson_correlation(raw_scores, labels)),
            "mondrian_venn_abers_point_pearson": round_float(
                pearson_correlation(va_points, labels)
            ),
        },
        "per_cell": per_cell,
        "decision": {
            "promotion_allowed": promotion_allowed,
            "eligible_cell_count": len(eligible_cells),
            "cell_count": len(per_cell),
            "cells_in_band_count": len(cells_in_band),
            "eligible_cell_fraction_in_band": round_float(eligible_cell_fraction_in_band),
            "cells_in_band": cells_in_band,
            "cells_below_band": cells_below_band,
            "cells_above_band": cells_above_band,
            "support_gap_cells": support_gap_cells,
            "calibration_regressions": calibration_regressions,
            "result": (
                "promote_mondrian_conformal_probe"
                if promotion_allowed
                else "do_not_promote_mondrian_conformal_on_current_split"
            ),
        },
        "row_predictions": [row.to_dict() for row in rows],
    }


def _coerce_input_row(value: dict[str, Any], index: int, split: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{split}[{index}] must be an object")
    score = value.get("predicted_oracle_pass")
    actual = value.get("actual_oracle_pass")
    if not isinstance(score, (int, float)) or not math.isfinite(float(score)):
        raise ValueError(f"{split}[{index}].predicted_oracle_pass must be finite numeric")
    if not isinstance(actual, (bool, int, float)):
        raise ValueError(f"{split}[{index}].actual_oracle_pass must be bool|number")
    category = str(value.get("category") or value.get("mutation_category") or "unknown")
    return {
        "row_index": int(value.get("row_index", index)),
        "task_id": str(value.get("task_id") or value.get("instance_id") or f"{split}-{index}"),
        "repo": str(value.get("repo") or "unknown"),
        "category": category,
        "language": str(value.get("language") or "python"),
        "score": min(1.0, max(0.0, float(score))),
        "label": 1 if bool(actual) else 0,
    }


def _bin_taus(
    bins: Sequence[int],
    residuals: Sequence[float],
    alpha: float,
    bin_count: int,
    min_bin_size: int,
    fallback_tau: float,
) -> tuple[list[float], list[int]]:
    grouped: dict[int, list[float]] = defaultdict(list)
    for bin_id, residual in zip(bins, residuals):
        grouped[int(bin_id)].append(float(residual))
    taus = []
    counts = []
    for bin_id in range(bin_count):
        values = grouped.get(bin_id, [])
        counts.append(len(values))
        if len(values) >= min_bin_size:
            taus.append(conformal_tau(values, alpha))
        else:
            taus.append(fallback_tau)
    return taus, counts


def _group_rows_by_cell(rows: Iterable[ScCpRow]) -> dict[str, list[ScCpRow]]:
    grouped: dict[str, list[ScCpRow]] = defaultdict(list)
    for row in rows:
        grouped[cell_key(row.category, row.language)].append(row)
    return grouped


def _group_input_rows_by_cell(rows: Iterable[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[cell_key(str(row["category"]), str(row["language"]))].append(row)
    return grouped


def _mondrian_cell_fits(
    rows: Sequence[dict[str, Any]],
    *,
    alpha: float,
    min_cell_rows: int,
) -> dict[str, dict[str, Any]]:
    grouped = _group_input_rows_by_cell(rows)
    fits: dict[str, dict[str, Any]] = {}
    for cell, cell_rows in sorted(grouped.items()):
        scores = [float(row["score"]) for row in cell_rows]
        labels = [int(row["label"]) for row in cell_rows]
        eligible = len(cell_rows) >= min_cell_rows and len(cell_rows) >= 2
        fit: dict[str, Any] = {
            "cell": cell,
            "scores": scores,
            "labels": labels,
            "sample_count": len(cell_rows),
            "label_counts": _label_counts(labels),
            "eligible": eligible,
            "status": "ready" if eligible else "insufficient_calibration_cell_support",
            "interval_tau": None,
            "class_tau": None,
            "loocv_interval_coverage": None,
            "loocv_class_set_coverage": None,
            "loocv_mean_interval_width": None,
            "loocv_mean_class_set_size": None,
        }
        if eligible:
            loo = leave_one_out_venn_abers(scores, labels)
            interval_residuals = [
                interval_residual(label, va.lower, va.upper)
                for label, va in zip(labels, loo)
            ]
            class_residuals = [
                class_residual(label, va.point) for label, va in zip(labels, loo)
            ]
            interval_tau = conformal_tau(interval_residuals, alpha)
            class_tau = conformal_tau(class_residuals, alpha)
            intervals = [
                (
                    max(0.0, va.lower - interval_tau),
                    min(1.0, va.upper + interval_tau),
                )
                for va in loo
            ]
            class_sets = [classification_set(va.point, class_tau) for va in loo]
            interval_covered = sum(
                1
                for label, (low, high) in zip(labels, intervals)
                if float(label) >= low and float(label) <= high
            )
            class_covered = sum(
                1 for label, class_set in zip(labels, class_sets) if label in class_set
            )
            fit.update(
                {
                    "interval_tau": interval_tau,
                    "class_tau": class_tau,
                    "loocv_interval_coverage": interval_covered / len(labels),
                    "loocv_class_set_coverage": class_covered / len(labels),
                    "loocv_mean_interval_width": sum(
                        high - low for low, high in intervals
                    )
                    / len(intervals),
                    "loocv_mean_class_set_size": sum(
                        len(class_set) for class_set in class_sets
                    )
                    / len(class_sets),
                }
            )
        fits[cell] = fit
    return fits


def _public_mondrian_fit_block(fit: dict[str, Any]) -> dict[str, Any]:
    return {
        "sample_count": fit["sample_count"],
        "label_counts": fit["label_counts"],
        "eligible": fit["eligible"],
        "status": fit["status"],
        "interval_tau": round_float(fit["interval_tau"])
        if fit["interval_tau"] is not None
        else None,
        "class_tau": round_float(fit["class_tau"]) if fit["class_tau"] is not None else None,
        "loocv_interval_coverage": round_float(fit["loocv_interval_coverage"])
        if fit["loocv_interval_coverage"] is not None
        else None,
        "loocv_class_set_coverage": round_float(fit["loocv_class_set_coverage"])
        if fit["loocv_class_set_coverage"] is not None
        else None,
        "loocv_mean_interval_width": round_float(fit["loocv_mean_interval_width"])
        if fit["loocv_mean_interval_width"] is not None
        else None,
        "loocv_mean_class_set_size": round_float(fit["loocv_mean_class_set_size"])
        if fit["loocv_mean_class_set_size"] is not None
        else None,
    }


def _mondrian_cell_report_block(
    cell: str,
    rows: Sequence[ScCpRow],
    fit: dict[str, Any] | None,
    *,
    min_cell_rows: int,
) -> dict[str, Any]:
    block = coverage_block(rows, min_rows=min_cell_rows)
    fit_block = _public_mondrian_fit_block(fit) if fit is not None else None
    block.update(
        {
            "calibration_sample_count": fit["sample_count"] if fit is not None else 0,
            "calibration_label_counts": fit["label_counts"]
            if fit is not None
            else _label_counts([]),
            "mondrian_fit_eligible": fit["eligible"] if fit is not None else False,
            "mondrian_fit_status": fit["status"] if fit is not None else "missing_cell_fit",
            "mondrian_fit": fit_block,
            "used_global_diagnostic_fallback": any(
                row.interval_tau_source == f"global_diagnostic_fallback:{cell}"
                for row in rows
            ),
        }
    )
    return block


def coverage_block(rows: Sequence[ScCpRow], *, min_rows: int) -> dict[str, Any]:
    n = len(rows)
    labels = [row.label for row in rows]
    set_sizes = [len(row.class_set) for row in rows]
    interval_covered = sum(1 for row in rows if row.interval_covered)
    class_covered = sum(1 for row in rows if row.class_set_covered)
    return {
        "sample_count": n,
        "label_counts": _label_counts(labels),
        "eligible_for_decision": n >= min_rows,
        "interval_coverage": round_float(interval_covered / n) if n else None,
        "class_set_coverage": round_float(class_covered / n) if n else None,
        "mean_class_set_size": round_float(sum(set_sizes) / n) if n else None,
        "empty_class_set_count": sum(1 for size in set_sizes if size == 0),
        "singleton_class_set_count": sum(1 for size in set_sizes if size == 1),
        "two_class_set_count": sum(1 for size in set_sizes if size == 2),
        "mean_interval_width": round_float(
            sum(row.interval_high - row.interval_low for row in rows) / n
        )
        if n
        else None,
    }


def metric_payload(scores: Sequence[float], labels: Sequence[int]) -> dict[str, Any]:
    return {
        "sample_count": len(scores),
        "brier": round_float(brier_score(scores, labels)),
        "ece_l1": round_float(ece_l1(scores, labels, bins=10)),
        "pearson": round_float(pearson_correlation(scores, labels)),
    }


def _label_counts(labels: Sequence[int]) -> dict[str, int]:
    counts = Counter(int(label) for label in labels)
    return {"fail_0": counts.get(0, 0), "pass_1": counts.get(1, 0)}


def _in_band(value: Any, lower: float, upper: float) -> bool:
    return isinstance(value, (int, float)) and lower <= float(value) <= upper
