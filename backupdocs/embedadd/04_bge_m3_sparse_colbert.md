# Phase 2 — BGE-M3 Sparse (E15) + ColBERT (E16)

Extend the BGE-M3 backbone (loaded in Phase 1 for E14 dense) with two
additional output heads. Because the encoder is shared, this phase is
mostly additive.

## What we're adding

| Slot | Model output | Shape | HNSW index? | Use case |
|------|--------------|-------|-------------|----------|
| E15 | BGE-M3 sparse (MLM-head lexical) | sparse Vec<(u32, f32)> over 250k vocab | No (inverted index) | Lexical match, like E13 SPLADE |
| E16 | BGE-M3 ColBERT (per-token) | float32[seq_len, 1024] | Per-token CF | Late-interaction retrieval, like E12 |

Both share the E14 encoder + all its infrastructure.

## Architectural reuse

**Shared backbone:** 95% of encoder code from Phase 1 is reused. What's new
is the output head attached after the final encoder layer:

- **Sparse head:** MLM head (linear projection to vocab_size) + ReLU + log;
  sparse-ify by keeping tokens above a threshold. Conceptually similar to
  the existing E13 SPLADE head but uses BGE-M3's vocab (not BERT's).
- **ColBERT head:** linear projection from 1024D to 1024D, outputs per-token
  vectors (no pooling). Very similar to existing E12 LateInteraction.

## Directory structure

```
crates/context-graph-embeddings/src/models/pretrained/bge_m3_sparse/
├── mod.rs              (~50)
├── types.rs            (~80)
├── mlm_head.rs         (~150)     Sparse projection head (new)
├── model.rs            (~180)     BgeM3SparseModel struct (delegates encoder to E14)
├── loader.rs           (~100)     Load only the sparse head weights
├── trait_impl.rs       (~60)
├── tests/              (~300)

crates/context-graph-embeddings/src/models/pretrained/bge_m3_colbert/
├── mod.rs              (~50)
├── types.rs            (~80)
├── linear_head.rs      (~120)     ColBERT projection head (new)
├── model.rs            (~180)     BgeM3ColbertModel struct (delegates encoder to E14)
├── loader.rs           (~100)     Load only the ColBERT head weights
├── trait_impl.rs       (~60)
├── tests/              (~300)

crates/context-graph-embeddings/src/models/pretrained/bge_m3_shared/
├── mod.rs              (~30)      Shared encoder registration
├── backbone.rs         (~100)     Shared XLM-RoBERTa backbone cache
```

**Total: ~1,500 new lines on top of Phase 1.**

## Shared backbone pattern (new for this codebase)

E14/E15/E16 share one set of encoder weights. Loading them 3× into VRAM
would waste ~2.3 GB. Instead:

```rust
// In bge_m3_shared/backbone.rs
lazy_static! {
    static ref BGE_M3_BACKBONE: OnceCell<Arc<XlmRobertaEncoder>> = OnceCell::new();
}

pub fn get_or_load_backbone(cfg: &SingleModelConfig)
    -> EmbeddingResult<Arc<XlmRobertaEncoder>>
{
    BGE_M3_BACKBONE.get_or_try_init(|| load_xlm_roberta_encoder(cfg)).cloned()
}
```

Each head (E14/E15/E16) calls `get_or_load_backbone()` in its model ctor.
First call loads; subsequent calls return the Arc.

## Storage schema

### E15 — sparse inverted index

**Two CFs:**

- `CF_E15_BGE_M3_SPARSE_STORED`: key=memory_id, value=sparse Vec<(u32,f32)>
  bincode-serialized. Stored form for reconstruction.
- `CF_E15_BGE_M3_SPARSE_INVERTED`: key=vocab_token_id(4 bytes) + memory_id(16
  bytes), value=weight(4 bytes f32). Same pattern as existing
  `CF_E13_SPLADE_INVERTED`. Enables sparse lookup.

### E16 — per-token storage

**One CF:**

- `CF_E16_BGE_M3_COLBERT`: key=memory_id, value = 4-byte seq_len header +
  seq_len × 1024 × 4 bytes f32 array. Same pattern as `CF_E12_LATE_INTERACTION`.

## HNSW indexes

- **E15:** No HNSW (sparse — uses inverted index; search via
  `search_by_tokens`)
- **E16:** No HNSW for per-token storage (retrieval uses MaxSim against
  E12/E16 combined). Optionally build a per-doc pooled ColBERT index later
  for coarse filtering.

## MCP surface changes

- `search_by_embedder` accepts "E15" (routes to `search_by_tokens`-like
  path), "E16" (routes to MaxSim path)
- `score_against_constellation` grows to 16-element per_embedder_cosine
- `search_by_tokens` adds optional `embedder: "E15"` param
- New MCP tool `compute_colbert_maxsim` — query tokens vs stored E16 vectors
  (or extend existing MaxSim tool if present)

## Tests

Per-head tests:

**E15 sparse:**
- Loader loads MLM head weights
- Output is sparse (most weights zero, few non-zero)
- Shakespeare vs modern sparse-cosine gap ≥ 0.05
- Inverted index stores + retrieves correctly

**E16 ColBERT:**
- Output shape [seq_len, 1024] for variable seq_len
- ColBERT MaxSim Shakespeare vs modern gap ≥ 0.05 (matches our Phase 1
  Python-side value of +0.083)
- Per-token L2 norms ≈ 1.0

**Shared backbone:**
- Loading E14+E15+E16 uses only ONE copy of encoder weights in VRAM
- Memory budget: total GPU usage ≤ 6 GB for all 3 BGE-M3 heads combined

## Re-ingestion

New command:
```
context-graph-cli migrate reembed --embedder E15
context-graph-cli migrate reembed --embedder E16
```

Or a combined:
```
context-graph-cli migrate reembed --embedder BGE_M3_ALL
```
which runs all 3 heads in a single pass through the encoder (most efficient).

## FSV protocol

Run the 6-check FSV twice: once for E15, once for E16. Additional check:
**backbone sharing** — load both models, assert `Arc::strong_count(&backbone)
== 2`, memory footprint is ≤ single-load + small delta for heads.

## Definition of Done

- [ ] All 7 layers integrated for E15 and E16
- [ ] Shared backbone verified (single Arc in VRAM)
- [ ] 6-check FSV passes for both
- [ ] Python parity test (Rust sparse vs HF sparse: Jaccard ≥ 0.9 on top-50
      tokens; Rust ColBERT vs HF ColBERT: cos ≥ 0.995 on per-token avg)
- [ ] Reingestion successful
- [ ] MCP search_by_embedder works with "E15" and "E16"
- [ ] Existing E13 SPLADE tests still pass (no regression)
