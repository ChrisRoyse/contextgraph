from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest

from ..python_panel_oracle_mi_probe import (
    ACTIVE_EMBEDDERS,
    CodeStateAccumulator,
    PanelMiError,
    _sufficiency_decision,
    finalize_dataset,
    power_calibration_gate,
    screened_classifier_lower_bound,
    screened_top_k_for,
    standardize_and_reduce,
)


def _binary_signal_with_true_mi(n: int, d: int, slope: float, seed: int):
    """y ~ Bernoulli(sigmoid(slope * x0)) embedded in d-dim noise. Returns
    (X, y, true_mi_bits) where true MI is computed from the generating probs."""
    rng = np.random.default_rng(seed)
    X = rng.standard_normal((n, d)).astype(np.float32)
    p = 1.0 / (1.0 + np.exp(-slope * X[:, 0].astype(np.float64)))
    y = (rng.random(n) < p).astype(np.int64)

    def hb(q):
        q = np.clip(q, 1e-12, 1 - 1e-12)
        return -(q * np.log2(q) + (1 - q) * np.log2(1 - q))

    h_y = hb(np.array([p.mean()]))[0]          # H(Y) = Hb(P(Y=1))
    h_y_given_x = float(np.mean(hb(p)))         # E_x[H(Y|X=x)]
    return X, y, float(h_y - h_y_given_x)


def _high_dim_with_signal(n: int, d: int, signal_dims: int, seed: int):
    """Build (X, y) where y is a near-deterministic function of `signal_dims`
    real columns, embedded in d-dimensional noise. True MI is high."""
    rng = np.random.default_rng(seed)
    X = rng.standard_normal((n, d)).astype(np.float32)
    w = rng.standard_normal(signal_dims)
    score = X[:, :signal_dims] @ w
    y = (score > np.median(score)).astype(np.int64)
    flip = rng.random(n) < 0.02
    y = np.where(flip, 1 - y, y)
    return X, y


def test_screened_lower_bound_recovers_high_dim_signal() -> None:
    # 1 signal dim hidden in 2000 noise dims: the dense reduction path drowns it
    # (FSV showed ~0.009 bits); train-only screening must recover it.
    X, y = _high_dim_with_signal(n=800, d=2000, signal_dims=1, seed=1)
    out = screened_classifier_lower_bound(X=X, y=y, label="signal", seed=11, holdout_fraction=0.25)
    assert out["valid"] is True
    assert out["estimator"] == "screened_train_only_logistic_lower_bound"
    assert out["lower_bound_bits"] > 0.3, out


def test_screened_lower_bound_zero_for_independent_label() -> None:
    rng = np.random.default_rng(2)
    X = rng.standard_normal((800, 2000)).astype(np.float32)
    y = (rng.random(800) < 0.5).astype(np.int64)  # independent of X
    out = screened_classifier_lower_bound(X=X, y=y, label="noise", seed=11, holdout_fraction=0.25)
    assert out["valid"] is True
    assert out["lower_bound_bits"] < 0.1, out  # no false positive


def test_screened_lower_bound_fails_closed_on_degenerate_labels() -> None:
    rng = np.random.default_rng(3)
    X = rng.standard_normal((200, 500)).astype(np.float32)
    y = np.ones(200, dtype=np.int64)  # single class
    out = screened_classifier_lower_bound(X=X, y=y, label="degenerate", seed=11, holdout_fraction=0.25)
    assert out["valid"] is False
    assert out["invalid_reason"] == "DEGENERATE_ORACLE_LABELS"


def test_screened_top_k_budget_is_clamped() -> None:
    assert screened_top_k_for(50) == 8       # floor
    assert screened_top_k_for(2400) == 240   # n/10
    assert screened_top_k_for(100000) == 256  # ceiling


def test_power_gate_passes_when_signal_detectable() -> None:
    # Enough samples for screening to recover a planted signal -> NOT underpowered.
    X, y = _high_dim_with_signal(n=800, d=1500, signal_dims=5, seed=4)
    gate = power_calibration_gate(X=X, y=y, seed=11, holdout_fraction=0.25)
    assert gate["estimator_underpowered"] is False, gate
    assert gate["recovered_bits"] >= gate["detectable_floor_bits"]


def test_power_gate_flags_underpowered_when_too_few_samples() -> None:
    # 40 samples vs 3000 dims: below detection floor -> must flag underpowered
    # rather than letting a low real reading masquerade as "insufficient".
    X, y = _high_dim_with_signal(n=40, d=3000, signal_dims=2, seed=5)
    gate = power_calibration_gate(X=X, y=y, seed=11, holdout_fraction=0.25)
    assert gate["estimator_underpowered"] is True, gate


def test_club_upper_bound_brackets_true_mi() -> None:
    # Lower bound <= true MI <= CLUB upper bound, on a clean known-MI problem.
    X, y, true_mi = _binary_signal_with_true_mi(n=2400, d=200, slope=2.0, seed=7)
    out = screened_classifier_lower_bound(X=X, y=y, label="bracket", seed=11, holdout_fraction=0.25)
    lb = out["lower_bound_bits"]
    ub = out["club_upper_bound_bits"]
    assert 0.05 < true_mi < 0.95, true_mi  # sanity: a real, sub-target signal
    assert lb <= true_mi + 0.12, (lb, true_mi)        # lower bound stays below
    assert ub >= true_mi - 0.12, (ub, true_mi)        # CLUB stays above
    assert ub >= lb, (lb, ub)                          # bracket is ordered


def test_sufficiency_decision_states() -> None:
    # lower bound >= target -> proven sufficient
    d = _sufficiency_decision(target_bits=0.95, lower_bound_bits=0.97, upper_bound_bits=1.1, estimator_underpowered=False)
    assert d["classification"] == "PANEL_SUFFICIENT" and d["panel_sufficient"] is True
    # upper bound < target, powered -> insufficiency supported
    d = _sufficiency_decision(target_bits=0.95, lower_bound_bits=0.20, upper_bound_bits=0.40, estimator_underpowered=False)
    assert d["classification"] == "PANEL_INSUFFICIENT_SUPPORTED" and d["panel_insufficient_supported"] is True
    # underpowered -> unprovable, regardless of a low upper bound
    d = _sufficiency_decision(target_bits=0.95, lower_bound_bits=0.20, upper_bound_bits=0.40, estimator_underpowered=True)
    assert d["classification"] == "ESTIMATOR_UNDERPOWERED_INSUFFICIENCY_UNPROVABLE"
    assert d["panel_insufficient_supported"] is False
    # bracket straddles target -> indeterminate
    d = _sufficiency_decision(target_bits=0.95, lower_bound_bits=0.50, upper_bound_bits=1.2, estimator_underpowered=False)
    assert d["classification"] == "INDETERMINATE_BOUNDS_STRADDLE_TARGET"


def test_finalize_dataset_fails_closed_when_active_slot_dim_missing(tmp_path: Path) -> None:
    ingest_manifest = {
        "slot_dims": {slot: 3 for slot in ACTIVE_EMBEDDERS if slot != "e1"},
        "workspace_state_selector": "candidate",
        "record_counts": {"workspace_state_filter_skipped_records": 128},
        "slot_record_counts": {"e10": 4},
    }
    accumulators = {
        ("task-001", "known_good", "test_patch"): CodeStateAccumulator(
            task_instance_id="task-001",
            mutation_category="known_good",
            workspace_state="test_patch",
            oracle_all_passed=True,
            repo="demo/repo",
        )
    }

    with pytest.raises(PanelMiError, match="PANEL_MI_SLOT_DIM_MISSING"):
        finalize_dataset(
            accumulators=accumulators,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            input_manifest={"oracle": {}, "forward_cache": {}},
            ingest_manifest=ingest_manifest,
        )


def test_finalize_dataset_fails_closed_when_active_slot_dim_invalid(tmp_path: Path) -> None:
    ingest_manifest = {
        "slot_dims": {slot: 3 for slot in ACTIVE_EMBEDDERS},
        "workspace_state_selector": "candidate",
    }
    ingest_manifest["slot_dims"]["e1"] = 0
    accumulators = {
        ("task-001", "known_good", "test_patch"): CodeStateAccumulator(
            task_instance_id="task-001",
            mutation_category="known_good",
            workspace_state="test_patch",
            oracle_all_passed=True,
            repo="demo/repo",
        )
    }

    with pytest.raises(PanelMiError, match="PANEL_MI_SLOT_DIM_INVALID"):
        finalize_dataset(
            accumulators=accumulators,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            input_manifest={"oracle": {}, "forward_cache": {}},
            ingest_manifest=ingest_manifest,
        )


def test_standardize_reduce_is_independent_of_evaluation_seed() -> None:
    rng = np.random.default_rng(759)
    X = rng.normal(0.0, 1.0, size=(48, 32)).astype(np.float32)
    y = np.asarray([idx % 2 for idx in range(48)], dtype=np.int64)

    reduced_a, info_a = standardize_and_reduce(X, y, max_components=8, seed=844)
    reduced_b, info_b = standardize_and_reduce(X, y, max_components=8, seed=845)

    assert np.allclose(reduced_a, reduced_b)
    assert info_a["method"] == info_b["method"]
    assert info_a["output_dim"] == 8
    assert info_b["output_dim"] == 8
    assert info_a["reduction_seed_policy"] == "fixed_independent_of_evaluation_seed"
    assert info_b["reduction_seed_policy"] == "fixed_independent_of_evaluation_seed"
