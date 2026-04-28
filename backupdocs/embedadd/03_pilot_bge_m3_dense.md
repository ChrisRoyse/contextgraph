# Phase 1 Pilot — BGE-M3 Dense (E14)

Full spec for the first extended embedder integration. This phase proves the
end-to-end pattern; subsequent phases reuse it.

## What we're adding

**Model:** `BAAI/bge-m3` using only the dense output head.
- Architecture: XLM-RoBERTa-Large encoder
- Parameters: 567M
- Output: 1024D dense vector via CLS pooling + L2 normalization
- Context: 8192 tokens
- Tokenizer: XLM-RoBERTa SentencePiece

**Why BGE-M3 as pilot:**
- Closest architecture to existing E1 Semantic (encoder → dense vector)
- XLM-RoBERTa is very close to BERT (which E1 uses); ~80% code reuse possible
- Well-documented HF release, stable weight format
- Our Python-side evidence shows it discriminates Shakespeare style
  (discrimination gap +0.086 vs modern in Phase 1 tests)

## Architectural comparison — E1 Semantic vs BGE-M3

| Component | E1 (e5-large-v2) | BGE-M3 (pilot) | Reuse? |
|-----------|-------------------|-----------------|--------|
| Tokenizer | BERT WordPiece | XLM-RoBERTa SentencePiece | No — new tokenizer family |
| Embeddings | Token + position + **segment** | Token + position (no segments) | Partial |
| Attention | Self-attention, abs position | Self-attention, abs position | Yes |
| Layer norm | Post-LN | Post-LN | Yes |
| FFN | GELU | GELU | Yes |
| Pooling | Mean (current) | CLS | Simple change |
| Output head | None (raw embedding) | None (raw embedding) | Yes |
| Normalization | L2 | L2 | Yes |

Reuse estimate: 70-80%. New code mostly in tokenizer integration +
removal of segment embeddings.

## Directory structure

```
crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/
├── mod.rs              (~50 lines)  Module exports
├── types.rs            (~100)       Config + state enum
├── constants.rs        (~30)        Model dims, special tokens
├── model.rs            (~200)       BgeM3DenseModel struct, embed_single/batch
├── loader.rs           (~250)       Safetensors weight loading (XLM-RoBERTa)
├── tokenizer.rs        (~150)       SentencePiece wrapper
├── attention.rs        (~300)       Multi-head self-attention (reuse from semantic/)
├── embeddings.rs       (~100)       Token + position (NO segment)
├── encoder.rs          (~80)        Layer stack
├── ffn.rs              (~100)       GELU FFN (reuse)
├── layer_norm.rs       (~60)        LayerNorm (reuse)
├── pooling.rs          (~50)        CLS pooling (simpler than mean)
├── gpu_forward.rs      (~150)       End-to-end CUDA forward pass
├── trait_impl.rs       (~50)        EmbeddingModel trait impl
└── tests/
    ├── mod.rs          (~20)
    ├── loader_test.rs  (~150)       Weight-load happy/error paths
    ├── forward_test.rs (~200)       Determinism, batching, tokenizer
    └── discrimination_test.rs (~100)  Shakespeare vs modern gap
```

**Total: ~1,800-2,000 lines**

## Model weights

Download at load time via HF cache (respecting HF_TOKEN env):
- `BAAI/bge-m3/pytorch_model.bin` or `model.safetensors` (root weights)
- `BAAI/bge-m3/tokenizer.json` (XLM-RoBERTa SentencePiece)
- `BAAI/bge-m3/config.json` (model config)

**Note:** BGE-M3 root directory contains dense + sparse + ColBERT weights
together. For Phase 1 we load only the encoder (shared) + the linear head
for dense. Subsequent phases share the encoder and load only the additional
head weights.

## Storage schema

### New CF: `CF_E14_BGE_M3_DENSE`

**Key format:** `memory_id` (16 bytes UUID) — same as existing embedder CFs.

**Value format:** 4-byte dim header (= 1024) + 4096 bytes f32 little-endian
vector. Total 4100 bytes per record.

**Declaration** in `column_families.rs`:
```rust
pub const CF_E14_BGE_M3_DENSE: &str = "e14_bge_m3_dense";
```

**Registration** at DB-open in `repository/open.rs`:
```rust
ColumnFamilyDescriptor::new(CF_E14_BGE_M3_DENSE, opts.clone()),
```

### New HNSW index: `EmbedderIndex::E14BgeM3Dense`

Parameters (match E1 baseline):
- M = 16
- ef_construction = 200
- ef_search = 100
- max_elements = 2,000,000 (same as MAX_VECTORS_PER_INDEX)
- distance = cosine

Registration in `indexes/registry.rs`:
```rust
registry.insert(
    EmbedderIndex::E14BgeM3Dense,
    Arc::new(HnswEmbedderIndex::new(
        EmbedderIndex::E14BgeM3Dense,
        HnswConfig::default(),
    )?),
);
```

## MCP surface changes

### `search_by_embedder`

`embedder: string` now accepts `"E14"` or `"bge_m3_dense"`.

Extend `embedder_dtos.rs::EmbedderId::from_str`:
```rust
"E14" | "E14_BGE_M3_DENSE" | "bge_m3_dense" => Some(Self::BgeM3Dense),
```

Also extend the `assert_eq!(EmbedderId::from_str("E14"), None)` test (it
should now return Some).

### `score_against_constellation`

Response field `per_embedder_cosine` changes from 13-element to 14-element
(after Phase 1). Document in tool description. **Python consumers must
update to accept variable length.**

### `get_memory_fingerprint`

New embedder filter option `"E14"` / `"bge_m3_dense"` via the `embedders`
param (already variable-length list; just add to allowed values).

### `list_embedder_indexes`

Automatically reflects the new `EmbedderIndex::E14BgeM3Dense`.

### `mine_contrastive_pairs`

Configurable `topKCandidatesPerAnchor` and threshold params already exist.
No API change; internally the similarity_profile vectors grow to 14 slots.
`anomaly_kind` classification rules in `contrastive/mining.rs:104-134`
optionally get a new rule:
```rust
// NEW: high E1 semantic + low E14 BGE-M3-dense → style divergence without
// content divergence. Classify as StyleAnomaly.
```

## Reingestion plan

After E14 code is merged and tested, existing 2,770+ records have no E14
vectors. Options:

**Option A (recommended):** Add migration command
```
context-graph-cli migrate reembed --embedder E14
```
Iterates CF_FINGERPRINTS, calls the E14 model on the content from
CF_CONTENT, writes to CF_E14_BGE_M3_DENSE, updates HNSW. Resumable.

**Option B:** Automatic lazy re-embed on next access. Complex, adds latency
spikes. Not recommended.

**Pick Option A.** Migration tool lives in
`crates/context-graph-cli/src/commands/migrate.rs`.

**Timing estimate** at ~10 docs/sec (E1 baseline): 2,770 docs / 10 = ~5
minutes. BGE-M3 is slightly slower (8K context vs 512 for E1); expect
10-15 minutes.

## Tests

### Unit tests (in `bge_m3_dense/tests/`)

1. `config_validation_errors_zero_batch_size` — invalid config rejected
2. `loader_loads_safetensors_happy_path` — downloads & loads cleanly
3. `loader_rejects_wrong_shape_weights` — fail-fast on bad weights
4. `loader_rejects_missing_tokenizer_file` — fail-fast
5. `forward_produces_1024d_vector` — dimension correctness
6. `forward_is_deterministic` — same input → same output (bit-exact or
   within bf16 tolerance)
7. `forward_respects_max_tokens` — truncates at 8192
8. `tokenizer_roundtrip_unicode` — non-ASCII content
9. `tokenizer_roundtrip_empty_string` — error returned
10. `pooling_uses_cls_token_not_mean` — verify CLS pooling

### Integration tests

1. `bge_m3_dense_e2e_roundtrip` — store memory → embed → HNSW search →
   retrieve → verify vector
2. `bge_m3_dense_discriminates_shakespeare_vs_modern` — Sonnet 18 cosine
   gap vs modern prose ≥ 0.05 (matches Python-side smoke value)
3. `bge_m3_dense_matches_python_output_within_tolerance` — load 10 Sonnets,
   embed in both Rust and Python, compare cosines — must be ≥ 0.995
   (tolerance for bf16 numerics)

### Benchmark

1. Register "E14" in `embedder_stress.rs`
2. Register in `realdata/config.rs` parser
3. Measure throughput on RTX 5090 (baseline target: ≥5 docs/sec on 8K context)

## FSV protocol (6-check, per `10_fsv_protocol.md`)

Exit criteria for Phase 1 — all 6 must pass:

1. **Disk:** `data.mdb` of rocksdb has CF_E14_BGE_M3_DENSE; row count = 2770
   after reingestion
2. **Schema:** 10 random records have exactly 4100 bytes each; dim header = 1024
3. **Determinism:** re-embed 5 random records, cos(stored, recomputed) ≥ 0.9999
4. **Discrimination:** held-out Shakespeare vs modern probe gap ≥ 0.05
5. **Edges:** empty text → error; missing memory_id → KeyError; invalid
   weight shape → panic; config with batch_size=0 → ConfigError
6. **Physical evidence:** dump first 5 floats of 3 random records to log;
   verify non-zero, finite, L2 norm ≈ 1.0

## Risks

| Risk | Mitigation |
|------|-----------|
| XLM-RoBERTa SentencePiece tokenizer not supported by existing `tokenizers` crate integration | Verify early (Task 0); `tokenizers` v0.22 should support; fail-fast if not |
| Weight format mismatch (pytorch_model.bin vs safetensors) | Loader supports both via existing pattern in semantic/loader.rs |
| bf16 vs fp32 numeric mismatch vs HF Python reference | Allow 0.995 cosine tolerance in parity test; document |
| CF addition breaks existing DB open (RocksDB requires all CFs listed) | Auto-create CF at open if missing; add behind feature flag during dev |
| Reingestion OOM at batch of 64 | Default to batch 8; expose CLI flag |
| Existing dashboard/v4_2 scripts break on 14-slot response | Document + coordinate updates; positional compat (E1-E13 stays at 0-12) |

## Definition of Done

- [ ] All 7 layers integrated (ModelId, model impl, factory, storage CF,
      HNSW, MCP, tests)
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes with no regressions
- [ ] 6-check FSV passes
- [ ] Python parity test shows cos ≥ 0.995 vs HF reference on 10 samples
- [ ] Reingestion of all 2,770+ records successful
- [ ] `search_by_embedder(embedder="E14", query="...")` returns sensible
      results via MCP
- [ ] `score_against_constellation` returns 14-element per_embedder_cosine
- [ ] Docs updated in `CLAUDE.md`, `docs/SHAKESPEARE_LORA_TRAINING_SPEC.md`
- [ ] code-simplifier review completed

## Next phase

On green, proceed to `04_bge_m3_sparse_colbert.md`. BGE-M3 sparse + ColBERT
share the encoder with E14 — largely additive work.
