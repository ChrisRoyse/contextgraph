---
namespace: decisions
agent_id: coordinator
timestamp: 2026-04-22T01:20:00Z
status: active
tags: v4.2, persistence, lmdb, extended-ensemble, architecture, rocksdb-alternative
---

# Decision: LMDB sidecar store for extended (E14-E17) embedder outputs

## Context

User wants the 4 new embedders (BGE-M3, Jina-v3, Qwen3, BGE-reranker) to be
**permanently integrated** with the existing E1-E13 daemon stack — stored
durably, queryable, correlated with existing views. Direct quote:

> "we can add all new rows/columns or whatever to the rocksdb to incorporate
>  all of these new embedders. i want them to be added to the new system
>  permanently"

## Options considered

1. **Modify Rust daemon** — add E14-E17 CF column families, extend
   SourceMetadata, rebuild HNSW indexes, wire into mine_contrastive_pairs
   and score_against_constellation.
   - Pros: one source of truth, fully integrated.
   - Cons: the 4 new embedders are HF Python models (transformers +
     FlagEmbedding). To run them inline in Rust we'd need ONNX exports
     (BGE-M3 has one but Jina-v3 LoRA adapter, Qwen3 not trivially) plus
     custom ONNX Runtime bindings, OR we run a Python sidecar service
     that the daemon calls via IPC/HTTP. Either way: 40-80 hours of Rust
     work, plus re-ingesting all 2770 training records, 313 Sonnets,
     plus fresh HNSW builds.

2. **LMDB sidecar store** — Python-side permanent persistence keyed by
   memory_id (UUID, same as daemon's CF_FINGERPRINTS). A parallel
   column-family-equivalent with its own atomic commits.
   - Pros: 250 lines of Python vs 2000 lines of Rust. Installs cleanly
     (`python3 -c "import lmdb"` available, v2.1.1). ACID-compliant. Fast
     (memory-mapped). Deterministic layout. Works with existing daemon
     unchanged.
   - Cons: existing daemon tools (score_against_constellation, etc.) can't
     see the new views directly — need a Python-side extended scorer that
     queries both. This is fine for our use case because our
     CAMD-DPO miner is already Python-side.

3. **SQLite with BLOB columns** — also available (sqlite3 3.50.2).
   - Pros: ubiquitous, SQL query support.
   - Cons: slower for bulk numpy writes, no memory-map, BLOB handling
     awkward for variable-length ColBERT matrices.

## Decision

**Option 2 — LMDB sidecar.**

Path: `~/.contextgraph/extended_embeddings.lmdb/`
6 sub-DBs: bge_m3_dense, bge_m3_sparse, bge_m3_colbert, jina_v3_classif,
qwen3_instruct, meta. All keyed by memory_id (UUID bytes).

Implementation: `scripts/v4_2/extended_store.py` (380 lines).

## Rationale

- **Permanence**: data.mdb file persists across reboots; LMDB is single-file
  like SQLite, easy to back up.
- **Integration surface**: `ExtendedStore` class API mirrors what daemon
  exposes (`get_memory_fingerprint`, `search_by_embedder`). A thin
  `ExtendedConstellationScorer` in `camd_lib.py` can combine daemon-13 +
  ext-5 views into an 18-view reward.
- **Incrementally growable**: `backfill_extended.py --target sonnets` now,
  `--target shakespeare` later, `--target all` eventually.
- **No Rust risk**: avoids touching the production daemon that serves the
  inference stack and MCP clients.
- **Provenance**: every record carries ExtendedMeta with model version
  hashes + content sha256, so we can detect drift when embedders update.

## Implications for downstream

- `camd_lib.py` gets an `ExtendedConstellationScorer` that wraps both
  daemon RPC and LMDB reads. Signature:
  `score_extended(text: str) -> tuple[list[float], list[float]]` returns
  (daemon_13_cosines, extended_5_cosines).
- `mine_camd_pairs.py` becomes `mine_camd_pairs_v2.py` (or a --extended
  flag) that uses 18-dim reward.
- `analyze_correlations.py` (Phase 4) reports which daemon views are
  redundant with extended views (|r| > 0.9 → candidate for retirement) and
  which are orthogonal (|r| < 0.3 → genuinely new signal worth keeping).
- Extended Sonnets constellation: computed via
  `extended_constellation.py`, persisted as
  `~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz`
  — contains centroids + inv-covariance for Mahalanobis reward.

## Non-goals

- NOT modifying the Rust daemon this phase.
- NOT replacing CF_CONTRASTIVE_PAIRS; extended scoring is layered on top.
- NOT implementing an HNSW index for extended views this phase (linear
  scan over 313 Sonnets is fine at this scale; upgrade to faiss if
  needed later).

## Related

- See also: `docs/V4_2_NOVEL_METHODS.md` Part 1 (the 13-embedder asset)
- See also: `memory/context/current-phase-v4_2-extended-ensemble.md`
- See also: `memory/handoff/agent-v42-1-to-v42-2--ensemble-class-ready.md`
- Tags: v4.2, persistence, lmdb, extended-ensemble
