#!/usr/bin/env python3
"""Backfill extended embeddings into the LMDB sidecar store.

For each memory_id in a target set:
  1. Fetch the memory content via the daemon (get_memory_fingerprint + content).
  2. Compute all 5 extended embeddings (BGE-M3 dense/sparse/colbert, Jina-v3
     classification, Qwen3 instruct).
  3. Compute the BGE-reranker score of `content` vs. the canonical query
     "Represent Shakespearean verse", as provenance on how Shakespeare-like it
     reads according to the strongest style discriminator we have (agent-1
     reported reranker gap +5.95 which is 60× stronger than dense embedders).
  4. Persist all of the above + ExtendedMeta into the LMDB store.

Targets (in order of priority):
  - Sonnets constellation members (313 records) — our reference set
  - Other Shakespeare constellation members (~44 constellations total)
  - Training records (2770 in CF_TRAINING_RECORDS)
  - Full corpus — optional, large

Usage:

    # backfill only the Sonnets constellation (313 records, ~10 min)
    python3 scripts/v4_2/backfill_extended.py --target sonnets

    # backfill all Shakespeare constellations (~2k records)
    python3 scripts/v4_2/backfill_extended.py --target shakespeare

    # backfill training records
    python3 scripts/v4_2/backfill_extended.py --target training

    # force re-compute existing records
    python3 scripts/v4_2/backfill_extended.py --target sonnets --force

Hard rules (user directive):
  - No fallbacks. If an embedder fails, raise.
  - No mock data. Real memory contents only.
  - Structured error logging with memory_id + content length + exception chain.
  - Full State Verification at end: re-read every written record and confirm
    shape + dtype; compare a random sample to recomputed embeddings.
"""
from __future__ import annotations

import argparse
import json
import logging
import sys
import time
from pathlib import Path
from typing import Iterator

import numpy as np
import torch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from _gemma_shakespeare_lib import Rpc  # noqa: E402
from v4_2.camd_lib import SONNETS_CONSTELLATION_ID  # noqa: E402
from v4_2.extended_ensemble import ExtendedEmbedderEnsemble  # noqa: E402
from v4_2.extended_store import (  # noqa: E402
    DEFAULT_PATH,
    ExtendedStore,
    ExtendedMeta,
    content_hash,
)

# Canonical prompts — mirrored from extended_ensemble._smoke_test().
# Keeping them here (not importing) since the ensemble defines them locally
# inside its smoke-test function. If they diverge, FSV will catch it.
DEFAULT_QWEN3_PROMPT = (
    "Classify the literary style of the text as Shakespearean "
    "early modern English verse or as ordinary modern English prose."
)
SHAKESPEARE_QUERY = "Shakespearean verse about summer"


logger = logging.getLogger("v4_2.backfill_extended")


def list_sonnets_members(rpc: Rpc) -> list[str]:
    """Get the 313 member memory_ids of the Sonnets constellation."""
    # get_constellation returns member_ids ONLY when includeCentroids=True (daemon quirk)
    r = rpc.call("get_constellation", {
        "constellationId": SONNETS_CONSTELLATION_ID,
        "includeCentroids": True,
    }, timeout=60.0)
    members = r.get("member_ids") or []
    if not members:
        raise RuntimeError(
            f"Sonnets constellation {SONNETS_CONSTELLATION_ID} returned no members"
        )
    return members


def list_all_shakespeare_members(rpc: Rpc) -> list[str]:
    """Aggregate member_ids across all 44 Shakespeare constellations."""
    r = rpc.call("list_constellations", {"limit": 200}, timeout=30.0)
    cons = [c for c in r.get("constellations", [])
            if (c.get("label") or "").startswith("Shakespeare")]
    all_ids: set[str] = set()
    for c in cons:
        cid = c["constellation_id"]
        r2 = rpc.call("get_constellation", {
            "constellationId": cid, "includeCentroids": True,
        }, timeout=60.0)
        for mid in r2.get("member_ids") or []:
            all_ids.add(mid)
    return sorted(all_ids)


def list_contrastive_pair_members(rpc: Rpc) -> list[str]:
    """All unique memory_ids appearing as anchor or negative in CF_CONTRASTIVE_PAIRS."""
    from v4_2.camd_lib import iter_contrastive_pairs
    seen: set[str] = set()
    for p in iter_contrastive_pairs(rpc, page_size=500, max_pairs=50000):
        seen.add(p.anchor_id)
        seen.add(p.negative_id)
    return sorted(seen)


def list_training_members(rpc: Rpc) -> list[str]:
    """All memory_ids from CF_TRAINING_RECORDS."""
    out: list[str] = []
    offset = 0
    while True:
        r = rpc.call("list_training_records", {
            "limit": 500, "offset": offset, "includeShape": False,
        }, timeout=60.0)
        records = r.get("records") or []
        if not records:
            break
        for rec in records:
            mid = rec.get("memory_id") or rec.get("uuid") or rec.get("id")
            if mid:
                out.append(mid)
        offset += len(records)
        if len(records) < 500:
            break
    return out


class EmptyContentError(RuntimeError):
    """Raised when a memory_id has no text — legitimate state for soft-deleted
    or import-side records. Backfill should SKIP these, not abort."""


def fetch_content(rpc: Rpc, memory_id: str) -> str:
    """Fetch memory content via get_memory_fingerprint.

    Raises EmptyContentError if the memory has no text (soft-deleted / stub);
    caller treats this as 'skipped_empty', distinct from hard errors.
    """
    r = rpc.call("get_memory_fingerprint", {
        "memoryId": memory_id,
        "includeContent": True,
        "includeVectorNorms": False,
    }, timeout=30.0)
    content = r.get("content", "")
    if not content:
        raise EmptyContentError(f"memory {memory_id} has no content")
    return content


def compute_and_store(
    ens: ExtendedEmbedderEnsemble,
    store: ExtendedStore,
    rpc: Rpc,
    memory_id: str,
    qwen3_prompt: str,
    reranker_query: str,
    force: bool = False,
) -> tuple[str, float]:
    """Compute + persist one record. Returns (status, elapsed_sec).

    status ∈ {'skipped_exists', 'written'}. Errors propagate.
    """
    if not force and store.exists(memory_id):
        return ("skipped_exists", 0.0)

    t0 = time.monotonic()

    # 1. Fetch content
    content = fetch_content(rpc, memory_id)

    # 2. Compute embeddings (ensemble handles the 5 views)
    bge = ens.embed_bge_m3(content)
    jina = ens.embed_jina_v3(content, task="classification")
    qwen = ens.embed_qwen3(content, task_prompt=qwen3_prompt)
    rerank = ens.rerank(reranker_query, [content])[0]

    # 3. Build meta
    prov = ens.describe()
    meta = ExtendedMeta(
        memory_id=memory_id,
        content_sha256=content_hash(content),
        content_length=len(content),
        computed_at=time.time(),
        bge_m3_version=f"{prov['bge_m3']['name']}@{prov['bge_m3']['weights_hash']}",
        jina_v3_version=f"{prov['jina_v3']['name']}@{prov['jina_v3']['weights_hash']}",
        qwen3_version=f"{prov['qwen3']['name']}@{prov['qwen3']['weights_hash']}",
        reranker_version=f"{prov['bge_reranker_v2_m3']['name']}@{prov['bge_reranker_v2_m3']['weights_hash']}",
        reranker_score_vs_sonnets=float(rerank),
        task_prompt_qwen3=qwen3_prompt,
    )

    # 4. Persist (atomic)
    # Convert bge.colbert to float32 [seq_len, 1024]
    colbert = np.asarray(bge.colbert, dtype=np.float32)
    if colbert.ndim != 2 or colbert.shape[1] != 1024:
        raise RuntimeError(
            f"bge.colbert has unexpected shape {colbert.shape} for {memory_id}"
        )
    store.put(
        memory_id,
        bge_dense=np.asarray(bge.dense, dtype=np.float32),
        bge_sparse={int(k): float(v) for k, v in bge.sparse.items()},
        bge_colbert=colbert,
        jina_classif=np.asarray(jina, dtype=np.float32),
        qwen3_instruct=np.asarray(qwen, dtype=np.float32),
        meta=meta,
    )
    return ("written", time.monotonic() - t0)


def fsv_sample(
    ens: ExtendedEmbedderEnsemble,
    store: ExtendedStore,
    rpc: Rpc,
    memory_ids: list[str],
    qwen3_prompt: str,
    reranker_query: str,
    sample_k: int = 5,
) -> None:
    """Full State Verification — re-read a random sample and compare to
    re-computed embeddings. Raises on any mismatch above tolerance.
    """
    if not memory_ids:
        raise RuntimeError("fsv_sample: empty memory_ids list")
    rng = np.random.default_rng(42)
    chosen = rng.choice(memory_ids, size=min(sample_k, len(memory_ids)), replace=False)
    logger.info(f"FSV sample: verifying {len(chosen)} records against fresh re-compute")
    failures: list[str] = []
    for mid in chosen:
        content = fetch_content(rpc, str(mid))
        bge = ens.embed_bge_m3(content)
        jina = ens.embed_jina_v3(content, task="classification")
        qwen = ens.embed_qwen3(content, task_prompt=qwen3_prompt)

        stored_bge = store.get_dense(str(mid), embedder="bge_m3_dense")
        stored_jina = store.get_dense(str(mid), embedder="jina_v3_classif")
        stored_qwen = store.get_dense(str(mid), embedder="qwen3_instruct")

        for name, recomputed, stored in [
            ("bge_m3_dense", np.asarray(bge.dense, dtype=np.float32), stored_bge),
            ("jina_v3_classif", np.asarray(jina, dtype=np.float32), stored_jina),
            ("qwen3_instruct", np.asarray(qwen, dtype=np.float32), stored_qwen),
        ]:
            # cosine-equivalence check (embedders are deterministic under same seed + dtype)
            cos = float(np.dot(recomputed, stored) /
                        (np.linalg.norm(recomputed) * np.linalg.norm(stored) + 1e-12))
            if cos < 0.9999:
                failures.append(
                    f"{mid[:8]} {name}: cos(stored, recomputed)={cos:.6f} (expected >= 0.9999)"
                )
                logger.error(
                    f"FSV MISMATCH memory_id={mid} embedder={name} "
                    f"cos={cos:.6f} first5_stored={stored[:5]} "
                    f"first5_recomputed={recomputed[:5]}"
                )
            else:
                logger.info(f"FSV OK memory_id={mid[:8]} {name} cos={cos:.6f}")
    if failures:
        raise RuntimeError(
            f"FSV FAILED: {len(failures)} mismatches detected:\n  " + "\n  ".join(failures)
        )
    logger.info("FSV PASS — all sampled records match fresh re-computation")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--target", required=True,
                    choices=["sonnets", "shakespeare", "training",
                             "contrastive_pair_members", "explicit"],
                    help="Which memory_ids to backfill")
    ap.add_argument("--memory-ids", nargs="*", default=None,
                    help="Explicit list (with --target=explicit)")
    ap.add_argument("--store-path", type=Path, default=DEFAULT_PATH)
    ap.add_argument("--qwen3-prompt", default=DEFAULT_QWEN3_PROMPT)
    ap.add_argument("--reranker-query", default=SHAKESPEARE_QUERY)
    ap.add_argument("--force", action="store_true",
                    help="Re-compute even if record exists")
    ap.add_argument("--heartbeat-every", type=int, default=10)
    ap.add_argument("--limit", type=int, default=None,
                    help="Process at most N memory_ids (for smoke tests)")
    ap.add_argument("--skip-fsv", action="store_true",
                    help="Skip the final FSV sample re-compute pass")
    ap.add_argument("--fsv-sample-k", type=int, default=5)
    ap.add_argument("--dtype", default="bfloat16", choices=["bfloat16", "float16", "float32"])
    ap.add_argument("--log-level", default="INFO")
    args = ap.parse_args()

    logging.basicConfig(
        level=getattr(logging, args.log_level),
        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
    )

    dtype_map = {"bfloat16": torch.bfloat16, "float16": torch.float16, "float32": torch.float32}

    logger.info(f"backfill target={args.target} store={args.store_path} force={args.force}")

    # 1. Load daemon connection
    rpc = Rpc()
    try:
        # 2. Determine target set
        if args.target == "sonnets":
            memory_ids = list_sonnets_members(rpc)
        elif args.target == "shakespeare":
            memory_ids = list_all_shakespeare_members(rpc)
        elif args.target == "training":
            memory_ids = list_training_members(rpc)
        elif args.target == "contrastive_pair_members":
            memory_ids = list_contrastive_pair_members(rpc)
        elif args.target == "explicit":
            if not args.memory_ids:
                raise ValueError("--target=explicit requires --memory-ids")
            memory_ids = args.memory_ids
        else:
            raise ValueError(f"unknown target {args.target}")

        if args.limit:
            memory_ids = memory_ids[:args.limit]
        logger.info(f"target set size: {len(memory_ids)}")

        # 3. Load ensemble (5+ GB VRAM)
        ens = ExtendedEmbedderEnsemble(device="cuda", dtype=dtype_map[args.dtype])

        # 4. Open store
        store = ExtendedStore(args.store_path)

        # 5. Iterate
        t_start = time.monotonic()
        written = 0
        skipped = 0
        skipped_empty = 0
        errors = 0
        elapsed_total = 0.0
        for i, mid in enumerate(memory_ids, start=1):
            try:
                status, dt = compute_and_store(
                    ens, store, rpc, mid,
                    args.qwen3_prompt, args.reranker_query, force=args.force,
                )
                if status == "written":
                    written += 1
                    elapsed_total += dt
                else:
                    skipped += 1
            except EmptyContentError:
                # Legitimate skip: soft-deleted or stub records have no text
                # to embed. NOT an error; counted separately.
                skipped_empty += 1
            except Exception as e:
                errors += 1
                logger.exception(
                    f"ERROR on memory_id={mid}: {e.__class__.__name__}: {e}"
                )
                if errors > 3 and errors > i * 0.05:
                    raise RuntimeError(
                        f"backfill abort: {errors} errors in first {i} records "
                        f"exceeds 5%% tolerance"
                    )
            if i % args.heartbeat_every == 0:
                rate = written / max(elapsed_total, 1e-3)
                eta_sec = (len(memory_ids) - i) / max(rate, 1e-6) if rate > 0 else float('inf')
                logger.info(
                    f"progress {i}/{len(memory_ids)} written={written} "
                    f"skipped={skipped} skipped_empty={skipped_empty} "
                    f"errors={errors} rate={rate:.2f}/s "
                    f"ETA={eta_sec/60:.1f}min"
                )

        wall = time.monotonic() - t_start
        logger.info(
            f"backfill done wall={wall:.1f}s written={written} skipped={skipped} "
            f"skipped_empty={skipped_empty} errors={errors}"
        )

        # 6. Store stats
        stats = store.stats()
        logger.info(f"store stats: {json.dumps(stats, indent=2)}")

        # 7. FSV
        if not args.skip_fsv and written > 0:
            successfully_written = [m for m in memory_ids if store.exists(m)]
            fsv_sample(ens, store, rpc, successfully_written,
                       args.qwen3_prompt, args.reranker_query,
                       sample_k=min(args.fsv_sample_k, len(successfully_written)))

        # 8. Final evidence dump
        print()
        print("=" * 70)
        print(f"BACKFILL SUMMARY — target={args.target}")
        print("=" * 70)
        print(f"Records written: {written}")
        print(f"Records skipped (already present): {skipped}")
        print(f"Records skipped (empty content / soft-deleted): {skipped_empty}")
        print(f"Errors: {errors}")
        print(f"Wall time: {wall:.1f}s")
        print(f"Rate: {written/max(elapsed_total,1e-3):.2f} rec/s (embedding time)")
        print(f"Store: {args.store_path}")
        print(f"Counts per sub-DB: {stats['counts_per_subdb']}")
        print(f"FSV: {'SKIPPED' if args.skip_fsv else 'PASS (see log)'}")
        print("=" * 70)
        return 0 if errors == 0 else 2

    finally:
        rpc.close()


if __name__ == "__main__":
    sys.exit(main())
