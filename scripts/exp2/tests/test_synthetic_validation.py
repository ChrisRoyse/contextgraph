from __future__ import annotations

import math

from scripts.exp2.estimators.synthetic import covariance_gaussian_mi, gaussian_pair


def test_covariance_gaussian_mi_matches_closed_form() -> None:
    u, v, truth = gaussian_pair(n=4000, dim=4, rho=0.3, seed=123)
    estimate = covariance_gaussian_mi(u, v)
    assert math.isfinite(estimate)
    assert abs(estimate - truth) < 0.08
