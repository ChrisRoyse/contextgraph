# Reference Patterns — How Existing Embedders Are Wired

This is a concrete walkthrough of how `E1 Semantic` is integrated end-to-end,
with file and line citations. Use this as the template for every new embedder.

## The 7 layers of integration

A new embedder must be integrated at all 7 of these layers. Skip any one and
the system won't compile or will silently misbehave.

1. **ModelId enum** — `crates/context-graph-embeddings/src/types/model_id/core.rs`
2. **Model implementation** — `crates/context-graph-embeddings/src/models/pretrained/{name}/`
3. **Factory / registry** — `crates/context-graph-embeddings/src/models/factory/` and `models/registry/`
4. **Storage CFs** — `crates/context-graph-storage/src/teleological/column_families.rs`
5. **HNSW EmbedderIndex** — `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs`
6. **MCP tool handlers** — `crates/context-graph-mcp/src/handlers/tools/` (especially `embedder_tools.rs`, `embedder_dtos.rs`)
7. **Tests + benchmarks** — `crates/context-graph-embeddings/tests/`, `crates/context-graph-storage/tests/`, `crates/context-graph-benchmark/src/`

## Layer 1 — ModelId enum

**File:** `crates/context-graph-embeddings/src/types/model_id/core.rs`

**Pattern to extend:**
```rust
pub enum ModelId {
    Semantic = 0,
    // ... E2-E10 ...
    Kepler = 13,
    BgeM3Dense = 14,    // NEW
    BgeM3Sparse = 15,   // NEW
    // ...
}
```

**Methods that must be extended (exhaustive match — compiler will catch omissions):**
- `dimension()` — native output dim (1024 for BGE-M3)
- `projected_dimension()` — post-projection dim (same as dimension for dense)
- `is_custom()` — false (BGE-M3 is pretrained)
- `is_pretrained()` — automatically true (derived from is_custom)
- `max_tokens()` — 8192 for BGE-M3
- `tokenizer_family()` — XlmRobertaSentencePiece (NEW enum variant needed — existing codebase has `BertWordpiece` and `RobertaBpe` only; XLM-RoBERTa uses SentencePiece)
- `as_str()` — "bge_m3_dense"
- `latency_budget_ms()` — ~8ms for 568M-param model
- `all()` — add to the 14-element array
- `production()` — add to the production array

**Also:** `NUM_EMBEDDERS` constant at `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs`. Must update from 13 → 14, 14 → 15, etc.

## Layer 2 — Model implementation

**Directory:** `crates/context-graph-embeddings/src/models/pretrained/{name}/`

**Required files (pattern from `semantic/`):**

| File | Purpose | Typical LOC |
|------|---------|-------------|
| `mod.rs` | Module exports | ~50 |
| `types.rs` | Config + state enum | ~100 |
| `model.rs` | Main struct, `new()`, `embed_batch()`, `embed_single()` | ~200 |
| `loader.rs` | Load safetensors weights | ~200 |
| `attention.rs` | Multi-head attention | ~300 |
| `embeddings.rs` | Token/position embeddings + layer norm | ~100 |
| `encoder.rs` | Stack of encoder layers | ~100 |
| `ffn.rs` | Feed-forward network | ~100 |
| `layer_norm.rs` | LayerNorm implementation | ~60 |
| `pooling.rs` | Pooling head (CLS or mean-pool) | ~80 |
| `gpu_forward.rs` | End-to-end forward pass on CUDA | ~150 |
| `trait_impl.rs` | Implement `EmbeddingModel` trait | ~50 |
| `constants.rs` | Model-specific constants | ~20 |
| `tests/` | Unit + integration tests | ~300 |

**Total for a new encoder embedder:** ~1,800-2,500 lines.

For BGE-M3 the encoder is XLM-RoBERTa which is very close to BERT but uses
SentencePiece tokenization and RoBERTa-style (no token type IDs) — may need
adjustments to `embeddings.rs`.

## Layer 3 — Factory / registry

**File:** `crates/context-graph-embeddings/src/models/factory/default_factory.rs`

Maps `ModelId` → model instance. Must add a match arm for each new variant.

**File:** `crates/context-graph-embeddings/src/models/registry/loader.rs` and
`operations.rs`. Registry loads models lazily and caches them. Must understand
the new variant to route load requests properly.

**File:** `crates/context-graph-embeddings/src/warm/loader/operations.rs:63` —
`ModelId::try_from(model_id)` must recognize new string names.

## Layer 4 — Storage column families

**File:** `crates/context-graph-storage/src/teleological/column_families.rs`

**Pattern:**
```rust
pub const CF_E14_BGE_M3_DENSE: &str = "e14_bge_m3_dense";
```

**Also required:**
- Opening the CF at RocksDB initialization — `crates/context-graph-storage/src/teleological/repository/open.rs` (or similar) lists every CF to open. **If you add a CF and forget to register it here, RocksDB will refuse to open the DB after upgrade** (all CFs must be listed).

**Migration:** Existing databases (e.g., `contextgraph_data/`) do NOT have
the new CF. Either:
- Automatic migration on first open: add CF if missing
- Require user to run a migration command
- Fail-fast with a clear error: "New CF `e14_bge_m3_dense` not in DB; run `context-graph-cli migrate`"

Pick fail-fast + migration command for safety.

## Layer 5 — HNSW EmbedderIndex

**File:** `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs`

**Pattern:**
```rust
pub enum EmbedderIndex {
    E1Semantic,
    // ...
    E14BgeM3Dense,  // NEW
    // ...
}
```

**Methods to extend (exhaustive matches):**
- `dimension()` — 1024 for E14
- `is_dense()` — true for E14 dense; false for sparse heads
- `column_family_name()` — "e14_bge_m3_dense"
- `to_model_id()` → ModelId::BgeM3Dense (if helper exists)

**File:** `crates/context-graph-storage/src/teleological/indexes/registry.rs` —
`EmbedderIndexRegistry::new()` populates the registry. Must register the new
HNSW index at startup.

**HNSW parameter tuning:** use defaults (M=16, ef_construction=200) matching
E1 Semantic. These are in `hnsw_config/constants.rs`.

## Layer 6 — MCP tool handlers

**Critical files:**

### `crates/context-graph-mcp/src/handlers/tools/embedder_dtos.rs`

Contains `EmbedderId` enum (string-parseable, used in DTOs). Must add
`E14 / BgeM3Dense` variant. Must update `from_str`:
```rust
"E14" | "E14_BGE_M3_DENSE" | "bge_m3_dense" => Some(Self::BgeM3Dense),
```

**Critical: the existing test** `assert_eq!(EmbedderId::from_str("E14"), None)`
in this file **will break** — that's correct; update it to assert `Some`.

### `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs`

Implements `search_by_embedder`, `list_embedder_indexes`,
`compare_embedder_views`, `search_cross_embedder_anomalies`, etc.
1,564 lines. Must extend every dispatch/match arm.

### `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs`

Implements `score_against_constellation`, `get_memory_fingerprint`,
`mine_contrastive_pairs`. Returns `per_embedder_cosine: Vec<f64>` of length
N. Downstream clients expect N=13 today; we're changing to N=18 (or 14
after Phase 1). **Breaking change**; must be coordinated with dashboard +
v4_2 scripts.

### New tools (Phase 5 only)

`rerank_with_bge` — new MCP tool for BGE-reranker. Separate handler file.

## Layer 7 — Tests + benchmarks

Every new embedder needs:

1. **Unit tests** next to the model (e.g., `bge_m3_dense/tests/`)
   - Config validation
   - Weight loader happy + error paths
   - Forward pass determinism
   - Tokenizer roundtrip
   - Dimension correctness

2. **Integration tests**
   - `crates/context-graph-storage/tests/` — end-to-end roundtrip storing
     a vector and retrieving it via HNSW
   - `crates/context-graph-storage/tests/full_integration_real_data/` —
     mirror the existing Ed pattern

3. **Benchmark registration**
   - `crates/context-graph-benchmark/src/realdata/config.rs` — add parser
     for "E14" / "E14_BGE_M3_DENSE"
   - `crates/context-graph-benchmark/src/bin/embedder_stress.rs` — same

4. **Constitution / docs**
   - `docs/` — update architecture docs referencing the 13→18 embedder count

## The breaking-change risk

Extending `NUM_EMBEDDERS` from 13 breaks any fixed-size array:

```rust
pub type EmbedderScoreProfile = [f32; NUM_EMBEDDERS];  // was [f32; 13]
```

Any code like this compiles cleanly but any serialized data stored at the
old size becomes unreadable. Strategy:
- Version the serialized format. Existing records carry [f32; 13]; new
  records carry [f32; NUM_EMBEDDERS]. Reader dispatches by version tag.
- OR: require full reingestion (safer but slower).

**Decision for this plan: reingestion. Cleaner, no version-dispatch code
debt. Document the reingestion requirement in each phase.**

## Example: ModelId::Kepler's integration (follow this pattern)

Kepler was the last embedder added. Its integration history:
- `kepler = 13` added to `ModelId` enum
- `models/pretrained/kepler/` created (2017 lines)
- `CF_KEPLER` — Kepler actually reuses `CF_FINGERPRINTS` (its vector is the
  E11 slot) rather than getting its own CF. **This saved work but is
  confusing.** For our new embedders, each gets its own CF for clarity.
- `EmbedderIndex::E11Kepler`? No — Kepler doesn't have its own HNSW; it
  replaced E11's content. (Again, a shortcut the pattern took.)

**For our new embedders, no shortcuts.** Each gets its own CF and HNSW index
for clean isolation. More code, but every embedder is fully independent.

## Touchpoint count

Running `grep -rn "ModelId::\|NUM_EMBEDDERS" crates/`:
- 938 total references
- 153 distinct files

Of these, ~30% are exhaustive match arms (compiler catches omissions)
and ~40% are string parsing / config / tests (runtime-checkable) and the
rest are documentation / comments (won't cause failures but should be
kept current).
