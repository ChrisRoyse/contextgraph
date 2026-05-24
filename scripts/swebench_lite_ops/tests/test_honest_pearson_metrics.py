"""Unit tests for honest_pearson_metrics.py (#695 rank-lift Goodhart fix).

Verifies the three-number gate metric (raw / isotonic / ablation) computes
correctly on synthetic data with known answers.
"""

from __future__ import annotations

import math

import numpy as np
import pytest

from ..honest_pearson_metrics import (
    brier_score,
    ece_l1,
    honest_predictor_report,
    isotonic_regression,
    pearson_correlation,
)


def test_pearson_perfect_positive():
    # Linearly correlated data → Pearson exactly 1.0.
    p = [0.1, 0.4, 0.6, 0.9]
    y = [0.2, 0.8, 1.2, 1.8]  # y = 2*p + bias linear in p
    r = pearson_correlation(p, y)
    assert r == pytest.approx(1.0)


def test_pearson_anti_correlated():
    p = [0.1, 0.4, 0.6, 0.9]
    y = [1.8, 1.2, 0.8, 0.2]  # y = -2*p + bias linear in p
    r = pearson_correlation(p, y)
    assert r == pytest.approx(-1.0)


def test_pearson_rank_monotone_binary_labels():
    # Rank-correlated but not linear — known Pearson ≈ 0.857 for this case.
    p = [0.1, 0.4, 0.6, 0.9]
    y = [0.0, 0.0, 1.0, 1.0]
    r = pearson_correlation(p, y)
    assert 0.80 <= r <= 0.95


def test_pearson_zero_variance_returns_nan():
    p = [0.5, 0.5, 0.5, 0.5]
    y = [0.0, 1.0, 1.0, 0.0]
    r = pearson_correlation(p, y)
    assert math.isnan(r)


def test_pearson_length_mismatch_raises():
    with pytest.raises(ValueError, match="length mismatch"):
        pearson_correlation([0.1, 0.2], [0.0])


def test_brier_perfect_calibration():
    p = [0.0, 1.0, 0.0, 1.0]
    y = [0.0, 1.0, 0.0, 1.0]
    assert brier_score(p, y) == 0.0


def test_brier_uniform_prior_against_balanced_labels():
    p = [0.5] * 4
    y = [0.0, 1.0, 0.0, 1.0]
    # MSE = mean((0.5-y)^2) = mean(0.25, 0.25, 0.25, 0.25) = 0.25
    assert brier_score(p, y) == pytest.approx(0.25)


def test_ece_l1_perfect_calibration_zero():
    p = [0.0, 1.0, 0.0, 1.0]
    y = [0.0, 1.0, 0.0, 1.0]
    assert ece_l1(p, y, bins=10) == pytest.approx(0.0)


def test_ece_l1_uniform_prior_50_pct_label_rate():
    p = [0.5] * 10
    y = [0.0] * 5 + [1.0] * 5
    # All in same bucket; mean confidence 0.5; mean accuracy 0.5; |c-a|=0
    assert ece_l1(p, y, bins=10) == pytest.approx(0.0)


def test_ece_l1_overconfident_predictor():
    p = [0.95] * 10
    y = [0.0] * 5 + [1.0] * 5
    # All in last bucket; mean confidence 0.95; mean accuracy 0.50;
    # weight = 10/10 = 1.0; |0.95 - 0.50| = 0.45
    assert ece_l1(p, y, bins=10) == pytest.approx(0.45)


def test_isotonic_self_fit_does_not_reduce_pearson():
    # On a calibration-self-fit (fit isotonic on the same rows used to
    # measure Pearson), isotonic CAN raise Pearson by projecting onto the
    # monotone-non-decreasing best-fit-to-labels curve. The doctrinally
    # important property is the WEAKER one: isotonic must not REDUCE Pearson
    # below the raw score on the self-fit. (The held-out-isotonic case in
    # #695 is the one where Pearson must be approximately preserved; that
    # requires a separate calibration/holdout split, exercised by the
    # honest-report integration tests below.)
    rng = np.random.default_rng(seed=42)
    n = 200
    base = rng.uniform(-2.0, 2.0, size=n)
    labels = (base + rng.normal(0, 0.5, size=n) > 0).astype(np.float64)
    p = 1.0 / (1.0 + np.exp(-base))  # sigmoid raw scores
    raw_r = pearson_correlation(p.tolist(), labels.tolist())
    iso = isotonic_regression(p.tolist(), labels.tolist())
    iso_r = pearson_correlation(iso, labels.tolist())
    # The self-fit can only improve Pearson (or numerically tie it).
    assert iso_r >= raw_r - 1e-9, (raw_r, iso_r)


def test_isotonic_monotonic_nondecreasing_output():
    p = [0.9, 0.1, 0.5, 0.7, 0.3]
    y = [1.0, 0.0, 1.0, 0.0, 1.0]
    iso = isotonic_regression(p, y)
    sorted_by_p = sorted(zip(p, iso))
    iso_in_p_order = [v for _, v in sorted_by_p]
    for prev, cur in zip(iso_in_p_order, iso_in_p_order[1:]):
        assert cur >= prev - 1e-12, (iso_in_p_order, prev, cur)


def test_isotonic_pools_tied_predictions_into_same_value():
    p = [0.5, 0.5, 0.5, 0.5]
    y = [0.0, 1.0, 0.0, 1.0]
    iso = isotonic_regression(p, y)
    # All tied -> all pool to mean(y) = 0.5
    assert all(v == pytest.approx(0.5) for v in iso)


def test_isotonic_already_monotone_unchanged_at_extremes():
    p = [0.1, 0.5, 0.9]
    y = [0.0, 0.5, 1.0]
    iso = isotonic_regression(p, y)
    assert iso[0] == pytest.approx(0.0)
    assert iso[2] == pytest.approx(1.0)
    # middle could be exactly 0.5 since no inversions
    assert iso[1] == pytest.approx(0.5)


def test_honest_report_predictor_ablation_matches_constant_pearson():
    p = [0.1, 0.4, 0.6, 0.9]
    y = [0.0, 0.0, 1.0, 1.0]
    rep = honest_predictor_report(p, y, ablation_constant=0.5)
    # Rank-correlated binary data → Pearson around 0.85 (perfect rank order
    # but non-linear thresholding caps Pearson below 1.0).
    assert rep.raw_pearson > 0.80
    # Constant-vs-labels has zero variance on predictions -> Pearson NaN.
    # ablation_constant=0.5 is the "predictor-removed" baseline.
    assert math.isnan(rep.predictor_ablation_raw_pearson)
    assert rep.sample_count == 4


def test_honest_report_rank_lift_delta_isolates_exemplar_contribution():
    """#695 central claim: if rank-lift correlation > raw correlation by a
    large amount, the extra correlation comes from non-predictor (exemplar)
    features. This test simulates that scenario."""
    rng = np.random.default_rng(seed=695)
    n = 100
    labels = rng.integers(0, 2, size=n).astype(np.float64)
    # Predictor is barely better than random
    raw_predictor = labels * 0.4 + rng.normal(0.5, 0.1, size=n)
    # Composite "rank-lift" score includes a strong exemplar feature
    exemplar_feature = labels * 0.9 + rng.normal(0.05, 0.05, size=n)
    rank_lift_composite = 0.3 * raw_predictor + 0.7 * exemplar_feature
    rep = honest_predictor_report(
        raw_predictor.tolist(),
        labels.tolist(),
        rank_lift_predictions=rank_lift_composite.tolist(),
    )
    # By construction the rank-lift correlation is higher than raw.
    assert rep.rank_lift_minus_raw_pearson is not None
    assert rep.rank_lift_minus_raw_pearson > 0.1, rep.to_dict()
    # The honest gate metric (raw_pearson) is what should be ≥0.95 for ship.
    # In this synthetic case it is well below, which is the whole point.
    assert rep.raw_pearson < 0.95


def test_honest_report_dict_round_trip_finite():
    p = [0.1, 0.4, 0.6, 0.9, 0.2, 0.8]
    y = [0.0, 0.0, 1.0, 1.0, 0.0, 1.0]
    rep = honest_predictor_report(p, y)
    payload = rep.to_dict()
    # Every numeric field must be present, finite (except ablation NaN),
    # and JSON-serializable.
    assert payload["schema_version"] == 1
    assert payload["artifact_kind"] == "honest_predictor_pearson_metrics_v1"
    assert payload["sample_count"] == 6
    for key in (
        "raw_pearson",
        "isotonic_pearson",
        "isotonic_minus_raw_pearson",
        "brier_raw",
        "brier_isotonic",
        "ece_l1_raw",
        "ece_l1_isotonic",
    ):
        assert math.isfinite(payload[key]), (key, payload)
    assert payload["predictor_ablation"]["ablated_score_constant"] == 0.5
    # ablated_raw_pearson is NaN because constant predictions have 0 variance.
    assert math.isnan(payload["predictor_ablation"]["ablated_raw_pearson"])


def test_isotonic_recalibration_reduces_brier_on_overconfident_predictor():
    """Sanity: a systematically overconfident predictor should see Brier
    improve under isotonic. If this regresses, the PAV implementation is
    broken."""
    rng = np.random.default_rng(seed=2026)
    n = 500
    labels = rng.integers(0, 2, size=n).astype(np.float64)
    # Predictor is correctly directional but pushed to extremes.
    base = labels * 0.6 + rng.normal(0, 0.2, size=n)
    p = np.clip(base * 1.5 + 0.05, 0.0, 1.0)  # overconfident scaling
    rep = honest_predictor_report(p.tolist(), labels.tolist())
    assert rep.brier_isotonic <= rep.brier_raw + 1e-9
