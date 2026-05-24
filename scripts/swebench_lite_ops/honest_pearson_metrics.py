"""Honest predictor-vs-oracle metrics for #695 rank-lift Goodhart fix.

Replaces the doctrine-conflated 'rank-lift' gate metric with a three-number
report that lets an operator see exactly what is being measured:

1. **raw_pearson**: Pearson correlation between the predictor's raw
   `predicted_oracle_pass` and `actual_oracle_pass`. This is what
   `prediction_oracle_correlation` should mean per #695. Honest baseline.
2. **isotonic_pearson**: Pearson correlation after a monotonic isotonic
   regression on the predictor scores. Isotonic is rank-preserving (Platt's
   1999 / Niculescu-Mizil & Caruana ICML 2005), so this CAN improve Brier
   and ECE but CANNOT change Pearson by more than ~1e-6 in floating-point
   noise. If isotonic_pearson - raw_pearson > 0.01, the recalibration is
   broken or the labels were leaked.
3. **brier** and **ece_l1**: calibration quality scalars. Should improve
   under isotonic. Used to spot situations where Pearson is preserved but
   probabilistic calibration was poor.

Plus a **predictor_ablation** field: replaces every predicted_oracle_pass
with a constant (default 0.5) and recomputes raw_pearson on the ablated
input. Any positive ablation result means the predictor is not the source
of the gate-relevant correlation — exactly the test #695 specifies.

This module is pure Python + numpy; no scipy/sklearn dependency. The
isotonic regression is the pool-adjacent-violators algorithm (Brunk 1955,
Best & Chakravarti 1990); see test cases for verification against the
monotonic-fit specification.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Sequence

import numpy as np


@dataclass(frozen=True)
class HonestPearsonReport:
    sample_count: int
    raw_pearson: float
    isotonic_pearson: float
    brier_raw: float
    brier_isotonic: float
    ece_l1_raw: float
    ece_l1_isotonic: float
    predictor_ablation_raw_pearson: float
    rank_lift_minus_raw_pearson: float | None
    rank_safe_minus_raw_pearson: float | None

    def to_dict(self) -> dict:
        return {
            "schema_version": 1,
            "artifact_kind": "honest_predictor_pearson_metrics_v1",
            "sample_count": self.sample_count,
            "raw_pearson": _round(self.raw_pearson),
            "isotonic_pearson": _round(self.isotonic_pearson),
            "isotonic_minus_raw_pearson": _round(self.isotonic_pearson - self.raw_pearson),
            "brier_raw": _round(self.brier_raw),
            "brier_isotonic": _round(self.brier_isotonic),
            "ece_l1_raw": _round(self.ece_l1_raw),
            "ece_l1_isotonic": _round(self.ece_l1_isotonic),
            "predictor_ablation": {
                "ablated_score_constant": 0.5,
                "ablated_raw_pearson": _round(self.predictor_ablation_raw_pearson),
                "predictor_contribution_pearson_pp": _round(
                    self.raw_pearson - self.predictor_ablation_raw_pearson
                ),
            },
            "rank_lift_minus_raw_pearson": (
                _round(self.rank_lift_minus_raw_pearson)
                if self.rank_lift_minus_raw_pearson is not None
                else None
            ),
            "rank_safe_minus_raw_pearson": (
                _round(self.rank_safe_minus_raw_pearson)
                if self.rank_safe_minus_raw_pearson is not None
                else None
            ),
        }


def _round(value: float) -> float:
    if not np.isfinite(value):
        return float("nan")
    return float(np.float64(value).round(10))


def pearson_correlation(predictions: Sequence[float], labels: Sequence[float]) -> float:
    """Sample Pearson correlation. Returns NaN when either side has zero variance."""
    if len(predictions) != len(labels):
        raise ValueError(
            f"pearson_correlation: length mismatch predictions={len(predictions)} labels={len(labels)}"
        )
    if len(predictions) < 2:
        raise ValueError("pearson_correlation: requires at least two samples")
    p = np.asarray(predictions, dtype=np.float64)
    y = np.asarray(labels, dtype=np.float64)
    if not (np.isfinite(p).all() and np.isfinite(y).all()):
        raise ValueError("pearson_correlation: non-finite values in input")
    p_var = p.var()
    y_var = y.var()
    if p_var == 0.0 or y_var == 0.0:
        return float("nan")
    return float(np.corrcoef(p, y)[0, 1])


def brier_score(predictions: Sequence[float], labels: Sequence[float]) -> float:
    """Mean squared error treating labels as 0/1 ground truth probabilities."""
    p = np.asarray(predictions, dtype=np.float64)
    y = np.asarray(labels, dtype=np.float64)
    if p.shape != y.shape:
        raise ValueError(f"brier_score: shape mismatch predictions={p.shape} labels={y.shape}")
    return float(np.mean((p - y) ** 2))


def ece_l1(predictions: Sequence[float], labels: Sequence[float], *, bins: int = 10) -> float:
    """Expected Calibration Error (L1) with equal-width binning on [0, 1]."""
    if bins < 1:
        raise ValueError("ece_l1: bins must be >= 1")
    p = np.asarray(predictions, dtype=np.float64)
    y = np.asarray(labels, dtype=np.float64)
    if p.shape != y.shape:
        raise ValueError(f"ece_l1: shape mismatch predictions={p.shape} labels={y.shape}")
    edges = np.linspace(0.0, 1.0, bins + 1)
    total = 0.0
    n = len(p)
    for idx in range(bins):
        lo = edges[idx]
        hi = edges[idx + 1]
        # last bin is closed on the right; others half-open on the right
        if idx == bins - 1:
            mask = (p >= lo) & (p <= hi)
        else:
            mask = (p >= lo) & (p < hi)
        bucket = int(mask.sum())
        if bucket == 0:
            continue
        confidence = float(p[mask].mean())
        accuracy = float(y[mask].mean())
        total += (bucket / n) * abs(confidence - accuracy)
    return total


def isotonic_regression(
    predictions: Sequence[float], labels: Sequence[float]
) -> list[float]:
    """Pool-Adjacent-Violators isotonic monotonic-non-decreasing fit.

    Returns the fitted value for each input prediction in input order. The
    fit is computed in sorted-prediction order then unscrambled back to the
    caller's order. Ties in prediction values are pooled together before the
    PAV pass to keep the result a function of the input rank vector.

    Equivalent to sklearn.isotonic.IsotonicRegression(out_of_bounds='clip')
    when applied to the same train/predict input pair (the standard
    diagnostic use). For out-of-sample isotonic, callers should fit on a
    calibration split and apply via the resulting piecewise-constant map.
    """
    p = np.asarray(predictions, dtype=np.float64)
    y = np.asarray(labels, dtype=np.float64)
    if p.shape != y.shape:
        raise ValueError(f"isotonic_regression: shape mismatch p={p.shape} y={y.shape}")
    n = len(p)
    if n == 0:
        return []
    order = np.argsort(p, kind="stable")
    ps = p[order]
    ys = y[order].copy()
    # Pre-pool rows whose input prediction is tied. PAV must operate on
    # distinct x-values, otherwise tied inputs that happen to fall on either
    # side of a monotonicity inversion get split across blocks even though
    # they share the same x. Group by exact-equal predicted value with the
    # group's mean label as the starting value and the group size as the
    # weight.
    pooled_values: list[float] = []
    pooled_weights: list[float] = []
    pooled_sizes: list[int] = []
    idx = 0
    while idx < n:
        end = idx + 1
        while end < n and ps[end] == ps[idx]:
            end += 1
        block_y = ys[idx:end]
        pooled_values.append(float(block_y.mean()))
        pooled_weights.append(float(end - idx))
        pooled_sizes.append(end - idx)
        idx = end
    values = np.asarray(pooled_values, dtype=np.float64)
    weights = np.asarray(pooled_weights, dtype=np.float64)
    block_size = np.asarray(pooled_sizes, dtype=np.int64)
    # PAV: walk forward, pool any inversion until monotonic non-decreasing.
    i = 0
    while i + 1 < len(values):
        if values[i] <= values[i + 1] + 1e-15:
            i += 1
            continue
        # merge block [i+1] into [i]
        total_w = weights[i] + weights[i + 1]
        new_val = (values[i] * weights[i] + values[i + 1] * weights[i + 1]) / total_w
        new_size = block_size[i] + block_size[i + 1]
        values = np.concatenate([values[:i], [new_val], values[i + 2 :]])
        weights = np.concatenate([weights[:i], [total_w], weights[i + 2 :]])
        block_size = np.concatenate(
            [block_size[:i], [new_size], block_size[i + 2 :]]
        )
        if i > 0:
            i -= 1
    # expand blocks back out to per-row predictions, in sorted order, then unscramble
    expanded = np.empty(n, dtype=np.float64)
    cursor = 0
    for value, size in zip(values, block_size):
        expanded[cursor : cursor + size] = value
        cursor += int(size)
    result = np.empty(n, dtype=np.float64)
    result[order] = expanded
    return result.tolist()


def honest_predictor_report(
    predictions: Sequence[float],
    labels: Sequence[float],
    *,
    rank_lift_predictions: Sequence[float] | None = None,
    rank_safe_predictions: Sequence[float] | None = None,
    ablation_constant: float = 0.5,
    ece_bins: int = 10,
) -> HonestPearsonReport:
    """Compute the #695 honest gate-metric triple plus the ablation diagnostic.

    Args:
        predictions: per-row predictor `predicted_oracle_pass`.
        labels: per-row `actual_oracle_pass` (0 or 1).
        rank_lift_predictions: optional per-row rank-lift composite scores
            (predictor + exemplar features). When provided, the report
            records `rank_lift_minus_raw_pearson` so reviewers can see how
            much extra correlation the exemplar features contribute on top
            of the predictor alone. If exemplar features are doing most of
            the work, this number is large and the rank-lift gate metric is
            measuring exemplars, not the predictor (#695's central claim).
        rank_safe_predictions: optional rank-preserving recalibration of the
            predictor (Spearman-like). Used the same way as rank_lift but
            for the smaller `rank_safe` lift.
        ablation_constant: constant value substituted for predictions in the
            predictor-ablation calculation. Default 0.5 (an even probabilistic
            prior). Anything in (0, 1) works; only the predictor's
            *correlation contribution* is what we are isolating.
        ece_bins: equal-width bin count for the ECE estimator. Default 10.
    """
    if len(predictions) != len(labels):
        raise ValueError(
            f"honest_predictor_report: length mismatch predictions={len(predictions)} labels={len(labels)}"
        )
    n = len(predictions)
    if n < 2:
        raise ValueError("honest_predictor_report: requires at least two samples")
    raw = pearson_correlation(predictions, labels)
    iso_preds = isotonic_regression(predictions, labels)
    iso_pearson = pearson_correlation(iso_preds, labels)
    brier_r = brier_score(predictions, labels)
    brier_i = brier_score(iso_preds, labels)
    ece_r = ece_l1(predictions, labels, bins=ece_bins)
    ece_i = ece_l1(iso_preds, labels, bins=ece_bins)
    ablated = [float(ablation_constant)] * n
    ablation_pearson = pearson_correlation(ablated, labels)
    rank_lift_delta = None
    if rank_lift_predictions is not None:
        if len(rank_lift_predictions) != n:
            raise ValueError(
                f"honest_predictor_report: rank_lift_predictions length {len(rank_lift_predictions)} != {n}"
            )
        rank_lift_delta = pearson_correlation(rank_lift_predictions, labels) - raw
    rank_safe_delta = None
    if rank_safe_predictions is not None:
        if len(rank_safe_predictions) != n:
            raise ValueError(
                f"honest_predictor_report: rank_safe_predictions length {len(rank_safe_predictions)} != {n}"
            )
        rank_safe_delta = pearson_correlation(rank_safe_predictions, labels) - raw
    return HonestPearsonReport(
        sample_count=n,
        raw_pearson=raw,
        isotonic_pearson=iso_pearson,
        brier_raw=brier_r,
        brier_isotonic=brier_i,
        ece_l1_raw=ece_r,
        ece_l1_isotonic=ece_i,
        predictor_ablation_raw_pearson=ablation_pearson,
        rank_lift_minus_raw_pearson=rank_lift_delta,
        rank_safe_minus_raw_pearson=rank_safe_delta,
    )
