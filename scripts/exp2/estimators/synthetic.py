"""Synthetic Gaussian ground-truth validation helpers."""

from __future__ import annotations

import math

import numpy as np


def gaussian_pair(n: int, dim: int, rho: float, seed: int = 42) -> tuple[np.ndarray, np.ndarray, float]:
    rng = np.random.default_rng(seed)
    u = rng.normal(size=(n, dim)).astype(np.float32)
    eps = rng.normal(size=(n, dim)).astype(np.float32)
    v = (rho * u + math.sqrt(max(0.0, 1.0 - rho * rho)) * eps).astype(np.float32)
    truth = -0.5 * math.log(max(1e-12, 1.0 - rho * rho)) * dim
    return u, v, truth


def covariance_gaussian_mi(u: np.ndarray, v: np.ndarray) -> float:
    x = np.hstack([u, v]).astype(np.float64)
    cov = np.cov(x, rowvar=False)
    d = u.shape[1]
    cu = cov[:d, :d]
    cv = cov[d:, d:]
    c = cov
    sign_u, log_u = np.linalg.slogdet(cu)
    sign_v, log_v = np.linalg.slogdet(cv)
    sign_c, log_c = np.linalg.slogdet(c)
    if sign_u <= 0 or sign_v <= 0 or sign_c <= 0:
        return float("nan")
    return float(0.5 * (log_u + log_v - log_c))
