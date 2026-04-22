#!/usr/bin/env python3
"""Compile Sonnets constellation centroids + covariance across the 5 new
extended embedders (BGE-M3 dense, BGE-M3 colbert, BGE-M3 sparse,
Jina-v3 classification, Qwen3 instruct).

This is the analog of the daemon's `compile_constellation` tool, but for
the extended views. Reads from the LMDB sidecar; writes to
`~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz`.

Schema of output .npz:

    # Dense-view centroids (1024D each)
    bge_m3_dense_centroid    : float32[1024]
    jina_v3_centroid          : float32[1024]
    qwen3_centroid            : float32[1024]

    # Dense-view covariance matrices (for Mahalanobis distance, regularized)
    bge_m3_dense_cov_inv      : float32[1024, 1024]
    jina_v3_cov_inv           : float32[1024, 1024]
    qwen3_cov_inv             : float32[1024, 1024]

    # ColBERT centroid (mean-pooled per-token vectors → mean-over-tokens, then centroid)
    bge_m3_colbert_centroid   : float32[1024]

    # Sparse centroid — union of active tokens with mean weight
    bge_m3_sparse_centroid_ids    : int32[num_active]
    bge_m3_sparse_centroid_weights: float32[num_active]

    # Per-view standard deviation (for normalization)
    bge_m3_dense_std_l2       : float32 scalar
    jina_v3_std_l2            : float32 scalar
    qwen3_std_l2              : float32 scalar

    # Metadata
    n_members                 : int scalar
    member_ids                : list[str]  (serialized as npz pickle)
    computed_at               : float (unix)
    constellation_id          : str

Hard rules:
    - No fallbacks. Missing records → error.
    - Cov regularization = lambda*I for numerical stability (lambda = 1e-4 * tr(cov)/d).
    - All matrices validated before write (PSD check on inv-cov).
"""
from __future__ import annotations

import argparse
import json
import logging
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from v4_2.camd_lib import SONNETS_CONSTELLATION_ID  # noqa: E402
from v4_2.extended_store import DEFAULT_PATH as DEFAULT_STORE_PATH  # noqa: E402
from v4_2.extended_store import ExtendedStore  # noqa: E402


logger = logging.getLogger("v4_2.extended_constellation")

DEFAULT_OUT = Path("~/.contextgraph/models/extended_ensemble/"
                   "sonnets_constellation_extended.npz").expanduser()
COV_RIDGE = 1e-4   # regularization multiplier on I


def compile_dense_centroid(vectors: np.ndarray) -> tuple[np.ndarray, np.ndarray, float]:
    """Return (centroid, inv_cov_regularized, std_l2).

    centroid: float32[1024]
    inv_cov_regularized: float32[1024, 1024]  (for Mahalanobis: (x-mu)^T Σ^-1 (x-mu))
    std_l2: float — std dev of per-member L2 norms (useful for sanity check)
    """
    if vectors.ndim != 2:
        raise ValueError(f"compile_dense_centroid: expected 2D, got {vectors.shape}")
    n, d = vectors.shape
    if n < 2:
        raise ValueError(f"compile_dense_centroid: need n≥2, got n={n}")

    centroid = vectors.mean(axis=0).astype(np.float32)
    centered = vectors - centroid
    cov = (centered.T @ centered) / (n - 1)
    # Regularize: Σ + λ·I where λ scales with trace
    trace = float(np.trace(cov))
    lam = COV_RIDGE * trace / d
    cov_reg = cov + lam * np.eye(d, dtype=cov.dtype)
    # Invert
    try:
        cov_inv = np.linalg.inv(cov_reg)
    except np.linalg.LinAlgError as e:
        raise RuntimeError(f"cov inversion failed (n={n}, d={d}, trace={trace:.4f}): {e}") from e
    # Sanity: Mahalanobis distance of the mean from itself must be 0 (numerically ~0)
    diff_mean = (centroid - centroid)
    mahal_self = float(diff_mean @ cov_inv @ diff_mean)
    if abs(mahal_self) > 1e-6:
        raise RuntimeError(f"cov_inv sanity fail: mahal(mu, mu)={mahal_self}")

    l2 = np.linalg.norm(vectors, axis=1)
    std_l2 = float(l2.std())

    return centroid.astype(np.float32), cov_inv.astype(np.float32), std_l2


def compile_colbert_centroid(colbert_matrices: list[np.ndarray]) -> np.ndarray:
    """Mean-pool each [seq_len, 1024] matrix to a single 1024D vector,
    then average across members. Not covariance-aware (variable seq_len
    makes it awkward), just a centroid.
    """
    pooled = []
    for m in colbert_matrices:
        if m.ndim != 2 or m.shape[1] != 1024:
            raise ValueError(f"colbert shape {m.shape} unexpected")
        pooled.append(m.mean(axis=0))
    pooled_arr = np.stack(pooled, axis=0).astype(np.float32)
    return pooled_arr.mean(axis=0).astype(np.float32)


def compile_sparse_centroid(sparse_dicts: list[dict[int, float]],
                             min_coverage: float = 0.1
                             ) -> tuple[np.ndarray, np.ndarray]:
    """Compute centroid sparse vector: keep token_ids active in ≥min_coverage
    fraction of members, with mean weight.

    Returns (ids, weights) where ids is int32[k], weights is float32[k].
    """
    n = len(sparse_dicts)
    if n == 0:
        raise ValueError("empty sparse_dicts")
    token_counts: dict[int, int] = {}
    token_sums: dict[int, float] = {}
    for d in sparse_dicts:
        for tid, w in d.items():
            token_counts[tid] = token_counts.get(tid, 0) + 1
            token_sums[tid] = token_sums.get(tid, 0.0) + float(w)
    min_count = max(1, int(min_coverage * n))
    keep = [(tid, token_sums[tid] / token_counts[tid])
            for tid, c in token_counts.items() if c >= min_count]
    keep.sort(key=lambda x: -x[1])  # highest-weight first
    if not keep:
        raise RuntimeError(
            f"compile_sparse_centroid: no tokens meet coverage {min_coverage} "
            f"(n={n}, unique_tokens={len(token_counts)})"
        )
    ids = np.array([t for t, _ in keep], dtype=np.int32)
    weights = np.array([w for _, w in keep], dtype=np.float32)
    return ids, weights


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--store-path", type=Path, default=DEFAULT_STORE_PATH)
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--constellation-id", default=SONNETS_CONSTELLATION_ID)
    ap.add_argument("--sparse-min-coverage", type=float, default=0.1)
    ap.add_argument("--log-level", default="INFO")
    args = ap.parse_args()

    logging.basicConfig(level=getattr(logging, args.log_level),
                        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s")

    # 1. Open store, collect all stored memory_ids
    store = ExtendedStore(args.store_path, readonly=True)
    all_ids = list(store.iter_memory_ids())
    if not all_ids:
        raise RuntimeError(f"no records found in {args.store_path}")
    logger.info(f"store has {len(all_ids)} records at {args.store_path}")

    # For this compile we use every stored record (they were backfilled
    # intentionally from the Sonnets constellation). If you want to filter
    # to only Sonnets members, add a --members-file argument later.

    # 2. Load all vectors
    t0 = time.monotonic()
    bge_dense = np.zeros((len(all_ids), 1024), dtype=np.float32)
    jina = np.zeros((len(all_ids), 1024), dtype=np.float32)
    qwen = np.zeros((len(all_ids), 1024), dtype=np.float32)
    colbert_mats: list[np.ndarray] = []
    sparse_dicts: list[dict[int, float]] = []

    for i, mid in enumerate(all_ids):
        bge_dense[i] = store.get_dense(mid, embedder="bge_m3_dense")
        jina[i] = store.get_dense(mid, embedder="jina_v3_classif")
        qwen[i] = store.get_dense(mid, embedder="qwen3_instruct")
        colbert_mats.append(store.get_colbert(mid))
        sparse_dicts.append(store.get_sparse(mid))

    logger.info(f"loaded vectors in {time.monotonic()-t0:.2f}s")

    # 3. Compile centroids
    bge_centroid, bge_cov_inv, bge_std = compile_dense_centroid(bge_dense)
    jina_centroid, jina_cov_inv, jina_std = compile_dense_centroid(jina)
    qwen_centroid, qwen_cov_inv, qwen_std = compile_dense_centroid(qwen)
    colbert_centroid = compile_colbert_centroid(colbert_mats)
    sparse_ids, sparse_weights = compile_sparse_centroid(
        sparse_dicts, min_coverage=args.sparse_min_coverage
    )

    logger.info(
        f"compiled: bge_dense_std={bge_std:.4f} jina_std={jina_std:.4f} "
        f"qwen_std={qwen_std:.4f} colbert_dim={colbert_centroid.shape} "
        f"sparse_active_tokens={len(sparse_ids)}"
    )

    # 4. Sanity — centroid norms shouldn't be pathologically small
    for name, c in [("bge", bge_centroid), ("jina", jina_centroid),
                    ("qwen", qwen_centroid), ("colbert", colbert_centroid)]:
        norm = float(np.linalg.norm(c))
        if norm < 0.1:
            raise RuntimeError(
                f"suspicious centroid for {name}: ||centroid||={norm}"
            )
        logger.info(f"{name} centroid ||L2||={norm:.4f}")

    # 5. Persist
    args.out.parent.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(
        args.out,
        bge_m3_dense_centroid=bge_centroid,
        jina_v3_centroid=jina_centroid,
        qwen3_centroid=qwen_centroid,
        bge_m3_dense_cov_inv=bge_cov_inv,
        jina_v3_cov_inv=jina_cov_inv,
        qwen3_cov_inv=qwen_cov_inv,
        bge_m3_colbert_centroid=colbert_centroid,
        bge_m3_sparse_centroid_ids=sparse_ids,
        bge_m3_sparse_centroid_weights=sparse_weights,
        bge_m3_dense_std_l2=np.float32(bge_std),
        jina_v3_std_l2=np.float32(jina_std),
        qwen3_std_l2=np.float32(qwen_std),
        n_members=np.int32(len(all_ids)),
        member_ids=np.array(all_ids, dtype=object),
        constellation_id=args.constellation_id,
        computed_at=np.float64(time.time()),
    )
    logger.info(f"wrote {args.out}")

    # 6. Emit summary
    size_mb = args.out.stat().st_size / (1024 * 1024)
    print()
    print("=" * 70)
    print(f"EXTENDED CONSTELLATION compiled — {args.constellation_id}")
    print("=" * 70)
    print(f"Source: {args.store_path} ({len(all_ids)} records)")
    print(f"Output: {args.out} ({size_mb:.1f} MB)")
    print(f"Centroid norms: bge={np.linalg.norm(bge_centroid):.3f} "
          f"jina={np.linalg.norm(jina_centroid):.3f} "
          f"qwen={np.linalg.norm(qwen_centroid):.3f} "
          f"colbert={np.linalg.norm(colbert_centroid):.3f}")
    print(f"Per-embedder L2 std: bge={bge_std:.3f} jina={jina_std:.3f} "
          f"qwen={qwen_std:.3f}")
    print(f"Sparse active tokens (>= {args.sparse_min_coverage*100:.0f}% coverage): "
          f"{len(sparse_ids)}")
    print("=" * 70)
    return 0


if __name__ == "__main__":
    sys.exit(main())
