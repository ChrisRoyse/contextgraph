from __future__ import annotations

import math

import numpy as np

from scripts.exp2.estimators.bootstrap import bootstrap_ci
from scripts.exp2.estimators.ksg import ksg_mi
from scripts.exp2.estimators.synthetic import gaussian_pair


def test_bootstrap_ci_is_deterministic_on_known_mean() -> None:
    data = np.arange(20, dtype=np.float64)

    def point(idx: np.ndarray) -> float:
        return float(np.mean(data[idx]))

    low1, high1, samples1 = bootstrap_ci(point, len(data), n_resamples=200, seed=42)
    low2, high2, samples2 = bootstrap_ci(point, len(data), n_resamples=200, seed=42)
    assert low1 == low2
    assert high1 == high2
    assert np.array_equal(samples1, samples2)
    assert low1 < float(np.mean(data)) < high1


def test_ksg_recovers_low_dimensional_gaussian_mi() -> None:
    u, v, truth = gaussian_pair(n=700, dim=1, rho=0.5, seed=7)
    estimate = ksg_mi(u, v, k=5)
    assert math.isfinite(estimate)
    assert abs(estimate - truth) < 0.15


def test_ksg_independence_near_zero() -> None:
    u, v, _ = gaussian_pair(n=700, dim=1, rho=0.0, seed=11)
    estimate = ksg_mi(u, v, k=5)
    assert 0.0 <= estimate < 0.08
