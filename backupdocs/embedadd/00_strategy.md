# Strategy — Extended Embedder Integration

## Why native Rust integration

The Rust daemon's embedder layer is 75,523 lines of hand-written candle
code for 13 models. Public MCP tools (`search_by_embedder`,
`score_against_constellation`, `mine_contrastive_pairs`, etc.) operate on
`ModelId` enum values and fixed-length arrays indexed by `EmbedderIndex`.
For the 4 new embedders to be first-class citizens — queryable via MCP,
participating in contrastive mining, scorable against constellations, usable
from any downstream tool — they must be **native candle implementations at
parity with the existing 13**, not Python-bridged.

This doc defines the phased approach to get there.

## Strategy: one embedder at a time, with FSV gates

Adding all 4 embedders at once is not feasible:
- 938 ModelId references across 153 files all need audit
- 4 different architectures (2 XLM-RoBERTa encoder, 1 decoder-only,
  1 cross-encoder), some with shared backbones
- Re-ingestion of 2,770+ records takes hours per embedder

**Phase-gated approach**: integrate embedders in order of increasing
architectural novelty. Each phase ships a fully working embedder at parity
with E1-E13, passes FSV, gets merged, then next phase starts.

## Phase plan

### Phase 0 — Reference study + touchpoint audit (complete before any code)

**Deliverables:**
- `01_reference_patterns.md` — how existing embedders are wired end-to-end
- `02_touchpoints.md` — every file that must change, categorized

**Exit criteria:** docs reviewed, no surprises left.

### Phase 1 — Pilot: BGE-M3 dense (E14)

Pilot establishes the pattern for every subsequent embedder.

**Target model:** `BAAI/bge-m3`, using only its dense-vector output.
XLM-RoBERTa encoder → [CLS] pooling → 1024D normalized vector.

**Why first:** architecturally closest to E1 Semantic (BERT/RoBERTa family,
encoder, mean-pool or CLS-pool → single dense vector). Maximum code reuse,
minimum new patterns.

**Scope:**
- `ModelId::BgeM3Dense = 14` added to enum; `NUM_EMBEDDERS` = 14
- New model impl at `crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/` (~1,400 lines, modeled on `semantic/`)
- New CF: `CF_E14_BGE_M3_DENSE` for dense vectors
- New HNSW: `EmbedderIndex::E14BgeM3Dense`
- Loader adapts existing safetensors → XLM-RoBERTa weights pattern
- All 938 ModelId touchpoints updated (exhaustive match arms extended)
- `search_by_embedder` accepts "E14" / "bge_m3_dense"
- `score_against_constellation` returns 14 per-embedder cosines
- Integration tests at parity with existing embedders
- Re-ingest Sonnets 313 members through E14

**Exit criteria:** 6-check FSV passes on E14.

**Estimate:** 5,000-6,000 lines of code across 40-60 files, 10-20h focused work.

### Phase 2 — BGE-M3 sparse (E15) + ColBERT (E16)

Leverage Phase 1's backbone — same XLM-RoBERTa encoder, add 2 projection
heads.

**Target model:** same `BAAI/bge-m3` weights, using sparse + ColBERT heads.
- Sparse: MLM head → sparse vocab vector (30K dim, analogous to E13 SPLADE)
- ColBERT: linear projection → 128/token matrix (analogous to E12 LateInteraction)

**Why second:** all 3 heads share BGE-M3's encoder; once E14 is working, E15
and E16 add ~1,500 lines each by reusing the encoder from E14. Cheapest
incremental embedders in the whole plan.

**Scope:**
- `ModelId::BgeM3Sparse = 15`, `ModelId::BgeM3Colbert = 16`
- Shared encoder — new `bge_m3_shared_backbone.rs` module under bge_m3_dense/
- 2 new projection heads
- 2 new CFs (`CF_E15_BGE_M3_SPARSE_INVERTED`, `CF_E16_BGE_M3_COLBERT`)
- 2 new indexes (sparse inverted, per-token ColBERT storage)
- `NUM_EMBEDDERS` = 16

**Exit criteria:** 6-check FSV passes on E15 + E16.

**Estimate:** 4,000-5,000 lines across 30-50 files, 8-12h.

### Phase 3 — Jina-v3 with task LoRA (E17)

First new pattern: dynamic task-LoRA adapters on top of an XLM-RoBERTa base.
Different LoRA for each task (retrieval-query, retrieval-passage, separation,
classification, text-matching). We use `classification` for style
discrimination.

**Target model:** `jinaai/jina-embeddings-v3`, classification task LoRA.

**Why third:** introduces LoRA-merging logic at load time, which is a new
architectural pattern in this codebase. Previous embedders have only ever
loaded frozen weights.

**Scope:**
- `ModelId::JinaV3Classification = 17`
- New model impl with task-LoRA loader (~2,500 lines)
- `CF_E17_JINA_V3`
- `NUM_EMBEDDERS` = 17

**Exit criteria:** 6-check FSV + LoRA-merge correctness (chosen task produces
different output than base).

**Estimate:** 5,000-6,000 lines, 12-18h.

### Phase 4 — Qwen3-Embedding (E18, decoder-only)

Most architecturally novel. Qwen2.5 base is a decoder transformer (causal
attention), not encoder. Fundamentally different forward pass. Task prompt
prepended to input.

**Target model:** `Qwen/Qwen3-Embedding-0.6B`

**Scope:**
- `ModelId::Qwen3Embedding = 18`
- New `qwen3_embedding/` module with decoder forward pass (causal mask,
  RMSNorm, SwiGLU, grouped-query attention, rotary position embeddings)
- Task prompt handling
- `CF_E18_QWEN3_EMBEDDING`
- `NUM_EMBEDDERS` = 18

**Exit criteria:** 6-check FSV + architectural test (decoder output matches
HF reference within bf16 tolerance).

**Estimate:** 6,000-8,000 lines, 20-30h — the biggest phase.

### Phase 5 — BGE-reranker (E19, cross-encoder)

Cross-encoder produces a relevance logit from a (query, passage) pair, not
a vector. Different MCP surface — needs a `rerank()` tool rather than
`search_by_embedder()`.

**Target model:** `BAAI/bge-reranker-v2-m3`

**Scope:**
- `ModelId::BgeRerankerV2M3 = 19`
- XLM-RoBERTa encoder + seq classifier head (~2,500 lines — reuse encoder
  from BGE-M3 pattern)
- Not a vector embedder; does NOT get an HNSW index
- Does NOT get a CF for persistent embeddings (runtime scoring only)
- New MCP tool: `rerank_with_bge` (inputs: query, list of memory_ids;
  outputs: re-ranked list with logits)
- `NUM_EMBEDDERS` = 20 (conceptually — though reranker doesn't quite fit
  the "embedder" taxonomy; may need a separate enum variant `RerankerId`)

**Exit criteria:** 4-check FSV (disk-N/A, schema, determinism, edge cases) +
discrimination test (Shakespeare passage gets higher logit vs modern for
"Shakespearean verse" query).

**Estimate:** 3,500-4,500 lines, 10-15h.

### Phase 6 — Integration sweep

- Update `score_against_constellation` to return 18 views (E1-E18; E19 is
  runtime scorer not persistent embedder)
- Update `mine_contrastive_pairs` classification rules to consider new views
- Update `get_unified_neighbors` RRF fusion to weight 18 views
- Documentation updates
- Re-run full FSV suite at 18-view level

**Exit criteria:** System-wide FSV passes all checks with all 6 new embedders
active.

**Estimate:** 2,000-3,000 lines, 8-12h.

## Total estimate

- Phase 0: docs only (10h)
- Phase 1: 10-20h
- Phase 2: 8-12h
- Phase 3: 12-18h
- Phase 4: 20-30h
- Phase 5: 10-15h
- Phase 6: 8-12h

**Grand total: 78-117 engineer-hours.** For me in session: Phase 1 pilot is
the realistic first milestone. Subsequent phases are modular and can be
done in follow-up sessions.

## Success criteria (system-wide, end state)

1. `ModelId::all()` returns 19 variants (13 original + 6 new)
2. `NUM_EMBEDDERS` is 18 (excluding reranker which isn't a persistent embedder)
3. Every MCP tool that previously returned/operated on 13 views now
   operates on 18
4. All 2,770+ existing training records have been re-ingested through the 6
   new embedders
5. HNSW indexes exist for E14-E18 (E19 is runtime scorer)
6. 6-check FSV passes for each new embedder individually AND for the
   combined 18-view system
7. No regressions in existing E1-E13 tests
8. CAMD-DPO pair miner works against 18-view reward (not just 13+5 Python-side)

## What we explicitly DON'T do in this plan

- Migrate the v4.2 Python-side LMDB sidecar away (it can coexist; eventually
  deprecate once Rust daemon has full integration)
- Change tokenizer handling — XLM-RoBERTa tokenizer is already supported via
  the `tokenizers` crate
- Add ONNX fallback or alternative backend — candle-only
- Optimize inference latency beyond baseline — correctness first
- Expose reranker as an HNSW-searchable embedder — different API surface
