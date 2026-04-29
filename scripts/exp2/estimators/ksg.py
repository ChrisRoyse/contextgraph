"""Direct KSG-1 mutual-information and KL entropy estimators."""

from __future__ import annotations

import math

import numpy as np
from scipy.special import digamma
from sklearn.neighbors import KDTree, NearestNeighbors

from scripts.exp2.common import Exp2Error


def _validate_matrix(x: np.ndarray, label: str, jitter_seed: int | None = None) -> np.ndarray:
    arr = np.asarray(x, dtype=np.float64)
    if arr.ndim != 2 or arr.shape[0] < 4:
        raise Exp2Error(f"{label}: expected [N,d] with N>=4, got {arr.shape}")
    if not np.isfinite(arr).all():
        raise Exp2Error(f"{label}: contains NaN or Inf")
    if np.unique(arr, axis=0).shape[0] != arr.shape[0]:
        if jitter_seed is None:
            raise Exp2Error(f"{label}: duplicate rows make KSG undefined without deterministic tie-break jitter")
        rng = np.random.default_rng(jitter_seed)
        arr = arr + rng.normal(0.0, 1e-12, size=arr.shape)
    return arr


def ksg_mi(u: np.ndarray, v: np.ndarray, k: int = 5, jitter_seed: int | None = None) -> float:
    """Estimate I(U;V) in nats with KSG-1 and Chebyshev distance."""

    u = _validate_matrix(u, "u", jitter_seed=jitter_seed)
    v = _validate_matrix(v, "v", jitter_seed=None if jitter_seed is None else jitter_seed + 1)
    if u.shape[0] != v.shape[0]:
        raise Exp2Error(f"u/v sample mismatch: {u.shape[0]} vs {v.shape[0]}")
    n = u.shape[0]
    if not 1 <= k < n:
        raise Exp2Error(f"k must satisfy 1 <= k < n, got k={k}, n={n}")
    joint = np.hstack([u, v])
    nn_joint = NearestNeighbors(metric="chebyshev", n_neighbors=k + 1).fit(joint)
    distances, _ = nn_joint.kneighbors(joint)
    # KSG-1 uses strict-less-than: shrink eps by one ULP so query_radius (<=) excludes the kth neighbour.
    eps = np.nextafter(distances[:, k], 0.0)
    # KDTree.query_radius accepts an array of per-query radii under sklearn>=1.3 where
    # NearestNeighbors.radius_neighbors does not (TypeError on the cython path at dim>=20).
    # `count_only=True` returns the count of points within r including self; subtract 1 for self.
    kdt_u = KDTree(u, metric="chebyshev")
    kdt_v = KDTree(v, metric="chebyshev")
    n_u = np.maximum(kdt_u.query_radius(u, r=eps, count_only=True) - 1, 0)
    n_v = np.maximum(kdt_v.query_radius(v, r=eps, count_only=True) - 1, 0)
    value = float(digamma(k) + digamma(n) - np.mean(digamma(n_u + 1) + digamma(n_v + 1)))
    if not math.isfinite(value):
        raise Exp2Error("KSG MI produced non-finite value")
    if value < -1e-6:
        raise Exp2Error(f"KSG MI produced negative estimate {value}; inspect duplicate/noisy inputs")
    return max(0.0, value)


def ksg_entropy(x: np.ndarray, k: int = 5, jitter_seed: int | None = None) -> float:
    """Estimate differential entropy in nats with Kozachenko-Leonenko."""

    x = _validate_matrix(x, "x", jitter_seed=jitter_seed)
    n, d = x.shape
    if not 1 <= k < n:
        raise Exp2Error(f"k must satisfy 1 <= k < n, got k={k}, n={n}")
    nn = NearestNeighbors(metric="chebyshev", n_neighbors=k + 1).fit(x)
    distances, _ = nn.kneighbors(x)
    eps = distances[:, k]
    if np.any(eps <= 0):
        raise Exp2Error("KSG entropy encountered zero neighbor radius")
    # Unit ball volume for L_inf in d dimensions is 2^d.
    h = float(digamma(n) - digamma(k) + d * np.mean(np.log(eps)) + d * math.log(2.0))
    if not math.isfinite(h):
        raise Exp2Error("KSG entropy produced non-finite value")
    return h
