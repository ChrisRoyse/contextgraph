#!/usr/bin/env python3
"""Alternative CAMD miner — score-and-sort all LMDB records directly, pair
top-quartile chosen with bottom-quartile rejected.

Bypasses the daemon's CF_CONTRASTIVE_PAIRS (which was mined for ANY
embedder disagreement, not specifically style disagreement — see
memory/discoveries/agent-02-sherlock-dpo--pair-mining-uses-semantic-not-style.md).

Pipeline:
  1. Enumerate all N memory_ids in the LMDB extended store.
  2. For each, compute extended_combined_score against Sonnets centroid
     (cheap — all embeddings cached in LMDB; one matrix-cosine op).
  3. Sort records by combined score (descending).
  4. Top-K (chosen) × Bottom-K (rejected) = K² candidate pairs.
  5. Apply E1 semantic similarity check via daemon get_unified_neighbors OR
     skip semantic check and rely on score gap alone.
  6. Emit DPO triples with prompt template + full 18-view provenance.

This produces pairs where chosen is demonstrably more Shakespeare-like than
rejected by construction — the strongest possible CAMD signal.

Usage:
    python3 scripts/v4_2/mine_camd_from_lmdb.py \\
        --top-k 200 --bottom-k 200 --min-gap 0.15 \\
        --max-pairs 5000 \\
        --output ~/.contextgraph/training/v4_2_camd_lmdb.jsonl
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
from v4_2.camd_lib import pick_prompt_template, SONNETS_CONSTELLATION_ID  # noqa: E402
from v4_2.extended_camd import (  # noqa: E402
    DEFAULT_ALPHA_DAEMON, DEFAULT_CENTROIDS_PATH,
    EXTENDED_STYLE_WEIGHTS,
    _cosine, _colbert_vs_centroid, _sparse_cosine,
)
from v4_2.camd_lib import STYLE_WEIGHTS as DAEMON_STYLE_WEIGHTS  # noqa: E402
from v4_2.extended_store import (  # noqa: E402
    DEFAULT_PATH as DEFAULT_STORE_PATH,
    ExtendedStore,
)


logger = logging.getLogger("v4_2.mine_camd_from_lmdb")

DEFAULT_OUTPUT = Path(
    "~/.contextgraph/training/v4_2_camd_lmdb.jsonl"
).expanduser()


def load_centroids(path: Path) -> dict:
    if not path.exists():
        raise FileNotFoundError(f"centroids not found: {path}")
    npz = np.load(path, allow_pickle=True)
    return {
        "bge_m3_dense": npz["bge_m3_dense_centroid"],
        "jina_v3": npz["jina_v3_centroid"],
        "qwen3": npz["qwen3_centroid"],
        "bge_m3_colbert": npz["bge_m3_colbert_centroid"],
        "sparse_ids": npz["bge_m3_sparse_centroid_ids"],
        "sparse_weights": npz["bge_m3_sparse_centroid_weights"],
    }


def score_record(store: ExtendedStore, mid: str, centroids: dict) -> dict:
    """Return extended-combined reward + per-view cosines for one record."""
    bge = store.get_dense(mid, embedder="bge_m3_dense")
    jina = store.get_dense(mid, embedder="jina_v3_classif")
    qwen = store.get_dense(mid, embedder="qwen3_instruct")
    colbert = store.get_colbert(mid)
    sparse = store.get_sparse(mid)
    sparse_centroid = {int(tid): float(w) for tid, w in zip(
        centroids["sparse_ids"], centroids["sparse_weights"])}
    ext_cos = {
        "bge_m3_dense":    _cosine(bge, centroids["bge_m3_dense"]),
        "bge_m3_colbert":  _colbert_vs_centroid(colbert, centroids["bge_m3_colbert"]),
        "bge_m3_sparse":   _sparse_cosine(sparse, sparse_centroid),
        "jina_v3_classif": _cosine(jina, centroids["jina_v3"]),
        "qwen3_instruct":  _cosine(qwen, centroids["qwen3"]),
    }
    extended_combined = sum(EXTENDED_STYLE_WEIGHTS[k] * v
                             for k, v in ext_cos.items())
    return {"extended_combined": extended_combined, "ext_cos": ext_cos}


def fetch_content(rpc: Rpc, memory_id: str) -> str:
    r = rpc.call("get_memory_fingerprint", {
        "memoryId": memory_id, "includeContent": True,
        "includeVectorNorms": False,
    }, timeout=30.0)
    c = r.get("content", "")
    if not c:
        raise RuntimeError(f"empty content for {memory_id}")
    return c


def daemon_score(rpc: Rpc, memory_id: str,
                  constellation_id: str) -> tuple[list[float], float]:
    r = rpc.call("score_against_constellation", {
        "constellationId": constellation_id, "memoryId": memory_id,
    }, timeout=30.0)
    pe = [float(x) for x in (r.get("per_embedder_cosine") or [])]
    if len(pe) != 13:
        raise RuntimeError(f"daemon returned {len(pe)} views")
    return pe, float(r.get("combined_score") or 0.0)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--store-path", type=Path, default=DEFAULT_STORE_PATH)
    ap.add_argument("--centroids-path", type=Path, default=DEFAULT_CENTROIDS_PATH)
    ap.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    ap.add_argument("--top-k", type=int, default=200,
                    help="Size of 'chosen' pool (top-K by extended combined).")
    ap.add_argument("--bottom-k", type=int, default=200,
                    help="Size of 'rejected' pool (bottom-K by extended combined).")
    ap.add_argument("--min-gap", type=float, default=0.15)
    ap.add_argument("--max-pairs", type=int, default=5000)
    ap.add_argument("--include-daemon-scores", action="store_true",
                    help="Also fetch daemon 13-view cosines per record "
                         "(slower — 1 RPC per record).")
    ap.add_argument("--constellation-id", default=SONNETS_CONSTELLATION_ID)
    ap.add_argument("--alpha-daemon", type=float, default=DEFAULT_ALPHA_DAEMON)
    ap.add_argument("--heartbeat-every", type=int, default=500)
    args = ap.parse_args()

    logging.basicConfig(level=logging.INFO,
                        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s")
    args.output.parent.mkdir(parents=True, exist_ok=True)

    # 1. Load store + centroids
    store = ExtendedStore(args.store_path, readonly=True)
    centroids = load_centroids(args.centroids_path)
    all_ids = list(store.iter_memory_ids())
    logger.info(f"LMDB records: {len(all_ids)}")

    # 2. Score all records by extended combined
    t0 = time.monotonic()
    scores: list[dict] = []
    for i, mid in enumerate(all_ids):
        s = score_record(store, mid, centroids)
        scores.append({"memory_id": mid, **s})
        if (i + 1) % 200 == 0:
            logger.info(f"scored {i+1}/{len(all_ids)} in {time.monotonic()-t0:.1f}s")
    logger.info(f"scored {len(all_ids)} records in {time.monotonic()-t0:.1f}s")

    # 3. Sort descending by extended_combined
    scores.sort(key=lambda x: -x["extended_combined"])
    top = scores[:args.top_k]
    bottom = scores[-args.bottom_k:]
    logger.info(
        f"top-K range: {top[-1]['extended_combined']:.3f}..{top[0]['extended_combined']:.3f} | "
        f"bottom-K range: {bottom[0]['extended_combined']:.3f}..{bottom[-1]['extended_combined']:.3f}"
    )

    # 4. Fetch content + optional daemon scores (lazy — only for pairs we emit)
    content_cache: dict[str, str] = {}
    daemon_cache: dict[str, tuple[list[float], float]] = {}

    def get_content(rpc: Rpc, mid: str) -> str:
        if mid not in content_cache:
            content_cache[mid] = fetch_content(rpc, mid)
        return content_cache[mid]

    def get_daemon(rpc: Rpc, mid: str) -> tuple[list[float], float] | None:
        if not args.include_daemon_scores:
            return None
        if mid not in daemon_cache:
            daemon_cache[mid] = daemon_score(rpc, mid, args.constellation_id)
        return daemon_cache[mid]

    # 5. Iterate top × bottom, filter by gap, emit
    rpc = Rpc()
    emitted = 0
    skipped_gap = 0
    skipped_content = 0
    errors = 0
    partial = args.output.with_suffix(args.output.suffix + ".partial")
    try:
        with open(partial, "w", encoding="utf-8") as out:
            for i, chosen_rec in enumerate(top):
                if emitted >= args.max_pairs:
                    break
                for j, rejected_rec in enumerate(bottom):
                    if emitted >= args.max_pairs:
                        break
                    gap = chosen_rec["extended_combined"] - rejected_rec["extended_combined"]
                    if gap < args.min_gap:
                        skipped_gap += 1
                        continue
                    try:
                        chosen_text = get_content(rpc, chosen_rec["memory_id"])
                        rejected_text = get_content(rpc, rejected_rec["memory_id"])
                    except Exception as e:
                        errors += 1
                        logger.exception(
                            f"content fetch: {e.__class__.__name__}: {e}"
                        )
                        if errors > 10:
                            raise RuntimeError(f"too many errors: {errors}")
                        continue

                    chosen_daemon = get_daemon(rpc, chosen_rec["memory_id"])
                    rejected_daemon = get_daemon(rpc, rejected_rec["memory_id"])

                    prompt = pick_prompt_template(emitted).format(
                        first_line=chosen_text.splitlines()[0].strip()[:140],
                        first_words=" ".join(chosen_text.split()[:12]),
                    )

                    obj = {
                        "prompt": prompt,
                        "chosen": chosen_text,
                        "rejected": rejected_text,
                        "meta": {
                            "source": "mine_camd_from_lmdb",
                            "chosen_id": chosen_rec["memory_id"],
                            "rejected_id": rejected_rec["memory_id"],
                            "chosen_extended_combined": chosen_rec["extended_combined"],
                            "rejected_extended_combined": rejected_rec["extended_combined"],
                            "gap_extended_combined": gap,
                            "chosen_extended_per_view": chosen_rec["ext_cos"],
                            "rejected_extended_per_view": rejected_rec["ext_cos"],
                            "top_k_rank_chosen": i,
                            "bottom_k_rank_rejected": j,
                        },
                    }
                    if chosen_daemon is not None and rejected_daemon is not None:
                        daemon_style_chosen = sum(
                            w * chosen_daemon[0][idx]
                            for idx, w in DAEMON_STYLE_WEIGHTS.items()
                        )
                        daemon_style_rejected = sum(
                            w * rejected_daemon[0][idx]
                            for idx, w in DAEMON_STYLE_WEIGHTS.items()
                        )
                        obj["meta"]["chosen_daemon_per_embedder"] = chosen_daemon[0]
                        obj["meta"]["rejected_daemon_per_embedder"] = rejected_daemon[0]
                        obj["meta"]["chosen_daemon_style"] = daemon_style_chosen
                        obj["meta"]["rejected_daemon_style"] = daemon_style_rejected
                        obj["meta"]["gap_daemon_style"] = (
                            daemon_style_chosen - daemon_style_rejected
                        )
                        obj["meta"]["chosen_combined_reward"] = (
                            args.alpha_daemon * daemon_style_chosen
                            + (1 - args.alpha_daemon) * chosen_rec["extended_combined"]
                        )
                        obj["meta"]["rejected_combined_reward"] = (
                            args.alpha_daemon * daemon_style_rejected
                            + (1 - args.alpha_daemon) * rejected_rec["extended_combined"]
                        )
                        obj["meta"]["gap_combined_reward"] = (
                            obj["meta"]["chosen_combined_reward"]
                            - obj["meta"]["rejected_combined_reward"]
                        )

                    out.write(json.dumps(obj, ensure_ascii=False) + "\n")
                    emitted += 1
                    if emitted % args.heartbeat_every == 0:
                        logger.info(
                            f"emitted={emitted}/{args.max_pairs} "
                            f"skipped_gap={skipped_gap} errors={errors}"
                        )
    finally:
        rpc.close()

    if emitted > 0:
        partial.rename(args.output)

    print()
    print("=" * 74)
    print(f"LMDB-INTERNAL CAMD MINE — emitted={emitted}")
    print("=" * 74)
    print(f"Records scored: {len(all_ids)}")
    print(f"Top-K (chosen pool): {args.top_k} "
          f"(scores {top[-1]['extended_combined']:.3f}..{top[0]['extended_combined']:.3f})")
    print(f"Bottom-K (rejected pool): {args.bottom_k} "
          f"(scores {bottom[0]['extended_combined']:.3f}..{bottom[-1]['extended_combined']:.3f})")
    print(f"Min gap threshold: {args.min_gap}")
    print(f"Pairs emitted: {emitted} (max {args.max_pairs})")
    print(f"Pairs skipped by gap: {skipped_gap}")
    print(f"Errors: {errors}")
    print(f"Output: {args.output}")
    print("=" * 74)
    return 0 if emitted > 0 else 2


if __name__ == "__main__":
    sys.exit(main())
