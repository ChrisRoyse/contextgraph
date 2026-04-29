"""Deterministic percentile bootstrap utilities."""

from __future__ import annotations

from collections.abc import Callable

import numpy as np

from scripts.exp2.common import Exp2Error


def bootstrap_ci(
    point_fn: Callable[[np.ndarray], float],
    n_items: int,
    n_resamples: int = 10000,
    seed: int = 42,
    confidence: float = 0.95,
) -> tuple[float, float, np.ndarray]:
    """Return low/high percentile CI plus the full bootstrap sample vector."""

    if n_items <= 1:
        raise Exp2Error(f"bootstrap requires n_items > 1, got {n_items}")
    if n_resamples <= 0:
        raise Exp2Error(f"n_resamples must be positive, got {n_resamples}")
    rng = np.random.default_rng(seed)
    values = np.empty(n_resamples, dtype=np.float64)
    for i in range(n_resamples):
        idx = rng.integers(0, n_items, size=n_items)
        value = float(point_fn(idx))
        if not np.isfinite(value):
            raise Exp2Error(f"bootstrap point_fn returned non-finite at resample {i}: {value}")
        values[i] = value
    alpha = (1.0 - confidence) / 2.0
    low, high = np.quantile(values, [alpha, 1.0 - alpha])
    return float(low), float(high), values
