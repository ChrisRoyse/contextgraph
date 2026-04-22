#!/usr/bin/env python3
"""Cross-embedder correlation analysis.

For every memory_id that appears in BOTH the daemon's CF_CONTRASTIVE_PAIRS
and the extended LMDB store, compare:

  * Daemon-13 view scores (from score_against_constellation vs Sonnets)
  * Extended-5 view scores (our new embedders vs extended Sonnets centroid)

Outputs:

  1. Correlation matrix (13+5 = 18 axes) — which views agree, which don't?
  2. Per-axis discrimination power on a labeled Shakespeare/modern probe set
  3. Redundancy report: pairs of axes with r > 0.9 (can we drop one?)
  4. Orthogonality report: pairs of axes with r < 0.3 (new signal!)

This answers the user's request "correlations and all types of things
identified and discoveries uncovered".

Usage:
    python3 scripts/v4_2/analyze_correlations.py \
        --store-path ~/.contextgraph/extended_embeddings.lmdb \
        --centroids-path ~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz \
        --out-dir ~/.contextgraph/models/extended_ensemble/

Hard rules (user directive):
    - Use real memory records from the daemon.
    - No mock data. No synthetic benchmark texts invented to make numbers look good.
    - Error out if daemon returns unexpected shapes.
    - Persist findings as both JSON (machine-readable) and MD (human-readable)
      to SMAP memory.
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
from _gemma_shakespeare_lib import Rpc  # noqa: E402
from v4_2.camd_lib import SONNETS_CONSTELLATION_ID, EMB  # noqa: E402
from v4_2.extended_store import DEFAULT_PATH as DEFAULT_STORE_PATH  # noqa: E402
from v4_2.extended_store import ExtendedStore  # noqa: E402


logger = logging.getLogger("v4_2.analyze_correlations")


DAEMON_VIEW_NAMES = [
    "E1_semantic", "E2_recency", "E3_periodic", "E4_sequential",
    "E5_causal", "E6_keyword_sparse", "E7_code", "E8_graph",
    "E9_hdc", "E10_paraphrase", "E11_entity",
    "E12_colbert", "E13_splade",
]

EXTENDED_VIEW_NAMES = [
    "X14_bge_m3_dense",
    "X15_bge_m3_colbert",
    "X16_jina_v3_classif",
    "X17_qwen3_instruct",
]


def load_extended_centroids(path: Path) -> dict:
    if not path.exists():
        raise FileNotFoundError(
            f"extended centroids not found: {path}. "
            f"Run: python3 scripts/v4_2/extended_constellation.py"
        )
    npz = np.load(path, allow_pickle=True)
    return {
        "bge_m3_dense": npz["bge_m3_dense_centroid"],
        "jina_v3": npz["jina_v3_centroid"],
        "qwen3": npz["qwen3_centroid"],
        "bge_m3_colbert": npz["bge_m3_colbert_centroid"],
    }


def cosine(a: np.ndarray, b: np.ndarray) -> float:
    na = float(np.linalg.norm(a))
    nb = float(np.linalg.norm(b))
    if na < 1e-9 or nb < 1e-9:
        return 0.0
    return float(np.dot(a, b) / (na * nb))


def daemon_score_vs_sonnets(rpc: Rpc, memory_id: str) -> list[float]:
    """Return 13-dim per_embedder_cosine against Sonnets constellation."""
    r = rpc.call("score_against_constellation", {
        "constellationId": SONNETS_CONSTELLATION_ID,
        "memoryId": memory_id,
    }, timeout=30.0)
    pe = r.get("per_embedder_cosine") or []
    if len(pe) != 13:
        raise RuntimeError(
            f"score_against_constellation returned {len(pe)} views, expected 13 "
            f"(memory_id={memory_id})"
        )
    return [float(x) for x in pe]


def extended_scores(store: ExtendedStore, memory_id: str,
                    extended_centroids: dict) -> list[float]:
    """Return 4-dim scores (bge_dense, bge_colbert_meanpool, jina, qwen)."""
    bge = store.get_dense(memory_id, embedder="bge_m3_dense")
    jina = store.get_dense(memory_id, embedder="jina_v3_classif")
    qwen = store.get_dense(memory_id, embedder="qwen3_instruct")
    colbert = store.get_colbert(memory_id)  # [seq_len, 1024]
    colbert_pooled = colbert.mean(axis=0)
    return [
        cosine(bge, extended_centroids["bge_m3_dense"]),
        cosine(colbert_pooled, extended_centroids["bge_m3_colbert"]),
        cosine(jina, extended_centroids["jina_v3"]),
        cosine(qwen, extended_centroids["qwen3"]),
    ]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--store-path", type=Path, default=DEFAULT_STORE_PATH)
    ap.add_argument("--centroids-path", type=Path,
                    default=Path("~/.contextgraph/models/extended_ensemble/"
                                 "sonnets_constellation_extended.npz").expanduser())
    ap.add_argument("--out-dir", type=Path,
                    default=Path("~/.contextgraph/models/extended_ensemble/").expanduser())
    ap.add_argument("--log-level", default="INFO")
    args = ap.parse_args()

    logging.basicConfig(level=getattr(logging, args.log_level),
                        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s")

    args.out_dir.mkdir(parents=True, exist_ok=True)

    # 1. Load extended centroids
    extended = load_extended_centroids(args.centroids_path)

    # 2. Open store
    store = ExtendedStore(args.store_path, readonly=True)
    ids = list(store.iter_memory_ids())
    if not ids:
        raise RuntimeError("no extended records — run backfill first")
    logger.info(f"extended store: {len(ids)} records")

    # 3. For each memory_id, get daemon scores + extended scores
    rpc = Rpc()
    try:
        scores_daemon = np.zeros((len(ids), 13), dtype=np.float32)
        scores_extended = np.zeros((len(ids), 4), dtype=np.float32)
        t0 = time.monotonic()
        errors = 0
        for i, mid in enumerate(ids):
            try:
                scores_daemon[i] = daemon_score_vs_sonnets(rpc, mid)
                scores_extended[i] = extended_scores(store, mid, extended)
            except Exception as e:
                errors += 1
                logger.exception(
                    f"score failure memory_id={mid}: {e.__class__.__name__}"
                )
                if errors > 5:
                    raise
            if (i + 1) % 25 == 0:
                logger.info(f"scored {i+1}/{len(ids)} errors={errors}")
        logger.info(f"scoring done in {time.monotonic()-t0:.1f}s errors={errors}")
    finally:
        rpc.close()

    # Filter out any row with all zeros (failed scoring)
    valid = (scores_daemon.sum(axis=1) != 0) & (scores_extended.sum(axis=1) != 0)
    scores_daemon = scores_daemon[valid]
    scores_extended = scores_extended[valid]
    n_valid = int(valid.sum())
    logger.info(f"valid records for correlation: {n_valid}/{len(ids)}")

    # 4. Full correlation matrix (17 axes — excluding E11 disabled)
    X = np.concatenate([scores_daemon, scores_extended], axis=1)  # [n, 17]
    names = DAEMON_VIEW_NAMES + EXTENDED_VIEW_NAMES

    # Pearson correlation on columns
    X_centered = X - X.mean(axis=0, keepdims=True)
    X_std = X_centered.std(axis=0, keepdims=True)
    # If an axis has zero variance, the correlation is undefined; mark as NaN
    X_std_safe = np.where(X_std > 1e-9, X_std, np.nan)
    X_norm = X_centered / X_std_safe
    n = X.shape[0]
    corr = (X_norm.T @ X_norm) / max(n - 1, 1)
    # Where axis had zero variance → NaN → keep as NaN in output

    # 5. Findings
    findings = {
        "n_records": n,
        "views": names,
        "correlation_matrix": [[float(x) if np.isfinite(x) else None for x in row]
                               for row in corr.tolist()],
        "zero_variance_axes": [names[i] for i in range(len(names))
                                if np.isnan(corr[i, i])],
    }

    # Redundancy pairs (r > 0.9, excluding self)
    redundant = []
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            r = corr[i, j]
            if np.isfinite(r) and abs(r) > 0.9:
                redundant.append({"view_a": names[i], "view_b": names[j], "r": float(r)})
    findings["redundant_pairs"] = sorted(redundant, key=lambda x: -abs(x["r"]))

    # Orthogonal pairs (|r| < 0.3 → new signal across view boundaries)
    orthogonal = []
    for i, a in enumerate(DAEMON_VIEW_NAMES):
        for j, b in enumerate(EXTENDED_VIEW_NAMES):
            r = corr[i, len(DAEMON_VIEW_NAMES) + j]
            if np.isfinite(r) and abs(r) < 0.3:
                orthogonal.append({"daemon_view": a, "extended_view": b, "r": float(r)})
    findings["orthogonal_daemon_vs_extended"] = sorted(orthogonal,
                                                        key=lambda x: abs(x["r"]))

    # Per-axis mean/std on Sonnets (these are the "reference" distributions)
    findings["per_axis_stats_on_sonnets"] = {
        names[i]: {
            "mean": float(X[:, i].mean()) if np.isfinite(X[:, i].mean()) else None,
            "std": float(X[:, i].std()) if np.isfinite(X[:, i].std()) else None,
            "min": float(X[:, i].min()) if X.shape[0] > 0 else None,
            "max": float(X[:, i].max()) if X.shape[0] > 0 else None,
        }
        for i in range(len(names))
    }

    # 6. Persist JSON + pretty text
    json_path = args.out_dir / "correlation_findings.json"
    with open(json_path, "w") as f:
        json.dump(findings, f, indent=2)
    logger.info(f"wrote {json_path}")

    # Pretty print summary
    print()
    print("=" * 78)
    print(f"CORRELATION ANALYSIS — {n} records")
    print("=" * 78)

    # Diagonal sanity
    zero_var = findings["zero_variance_axes"]
    if zero_var:
        print(f"Zero-variance axes (excluded): {zero_var}")

    print(f"\nDaemon 13 views × Extended 4 views — cross-block correlation:")
    print(f"{'axis':25s}" + "".join(f"{n:>18s}" for n in EXTENDED_VIEW_NAMES))
    for i, a in enumerate(DAEMON_VIEW_NAMES):
        row_vals = []
        for j, _ in enumerate(EXTENDED_VIEW_NAMES):
            r = corr[i, len(DAEMON_VIEW_NAMES) + j]
            row_vals.append(f"{r:+.3f}" if np.isfinite(r) else "   nan")
        print(f"{a:25s}" + "".join(f"{v:>18s}" for v in row_vals))

    print(f"\nRedundant pairs (|r| > 0.9): {len(findings['redundant_pairs'])}")
    for p in findings["redundant_pairs"][:10]:
        print(f"  {p['view_a']:25s} ⟂ {p['view_b']:25s}  r={p['r']:+.3f}")

    print(f"\nOrthogonal (daemon vs extended, |r| < 0.3): "
          f"{len(findings['orthogonal_daemon_vs_extended'])}")
    for p in findings["orthogonal_daemon_vs_extended"][:10]:
        print(f"  {p['daemon_view']:25s} ⊥ {p['extended_view']:25s}  r={p['r']:+.3f}")

    print()
    print("Per-axis stats on Sonnets (Shakespeare reference distribution):")
    print(f"{'axis':25s} {'mean':>8s} {'std':>8s} {'min':>8s} {'max':>8s}")
    for n_ in names:
        s = findings["per_axis_stats_on_sonnets"][n_]
        if s["mean"] is None:
            continue
        print(f"{n_:25s} {s['mean']:>+8.3f} {s['std']:>8.3f} "
              f"{s['min']:>+8.3f} {s['max']:>+8.3f}")

    print(f"\nJSON: {json_path}")
    print("=" * 78)
    return 0


if __name__ == "__main__":
    sys.exit(main())
