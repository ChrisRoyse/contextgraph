# Touchpoints — Every File That Must Change

Categorized list of the 938 `ModelId::` / `NUM_EMBEDDERS` references across
153 files. Use as a checklist when adding a new embedder.

## Tier 1 — Compiler-enforced (match arms; compiler yells if you miss one)

These use exhaustive match on `ModelId`. Adding a new variant will cause
compile errors until every site is updated.

**Core embeddings crate:**

| File | Lines | What to extend |
|------|-------|---------------|
| `crates/context-graph-embeddings/src/types/model_id/core.rs` | 15-296 | Enum + dimension() + max_tokens() + tokenizer_family() + as_str() + all() + production() + latency_budget_ms() |
| `crates/context-graph-embeddings/src/types/model_id/tokenizer.rs` | entire | TokenizerFamily enum variants |
| `crates/context-graph-embeddings/src/models/factory/default_factory.rs` | match on ModelId | Factory constructor for new variant |
| `crates/context-graph-embeddings/src/models/registry/loader.rs` | ~40 | Registry routing |
| `crates/context-graph-embeddings/src/models/registry/operations.rs` | ~60 | Load / unload routing |
| `crates/context-graph-embeddings/src/warm/loader/operations.rs:63` | 1 | `ModelId::try_from()` recognizes new string |
| `crates/context-graph-embeddings/src/adapters/**.rs` | varies | Any adapter crossing ModelId |
| `crates/context-graph-embeddings/src/error.rs` | possibly | Error enum references |

**Storage crate:**

| File | Lines | What to extend |
|------|-------|---------------|
| `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs` | NUM_EMBEDDERS | 13 → 14 (per phase) |
| `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs` | EmbedderIndex enum | Add variant |
| `crates/context-graph-storage/src/teleological/indexes/hnsw_config/*.rs` | varies | All methods on EmbedderIndex (match arms) |
| `crates/context-graph-storage/src/teleological/indexes/registry.rs` | ~300 | Register new HNSW index at init |
| `crates/context-graph-storage/src/teleological/indexes/embedder_index.rs` | EmbedderIndexOps trait | Dim/CF lookups |
| `crates/context-graph-storage/src/teleological/column_families.rs` | declarations + docs | Add CF const, update docs block |
| `crates/context-graph-storage/src/teleological/repository/*.rs` | CF list | Register CF at DB open |
| `crates/context-graph-storage/src/teleological/serialization/*.rs` | any fixed-size serializers | Handle new dim if serialized format is position-indexed |

**MCP crate:**

| File | Lines | What to extend |
|------|-------|---------------|
| `crates/context-graph-mcp/src/handlers/tools/embedder_dtos.rs` | 13 + tests | `EmbedderId` enum + `from_str()` + test asserting "E14" is NOT None |
| `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` | 1,564 | Every match on EmbedderId |
| `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` | 2,296 | Score fns returning per_embedder_cosine |
| `crates/context-graph-mcp/src/handlers/tools/graph_tools.rs` | varies | Search dispatch |
| `crates/context-graph-mcp/src/handlers/tools/embedder_dtos.rs:13` | enum | Core DTO enum |

**Core crate:**

| File | Lines | What to extend |
|------|-------|---------------|
| `crates/context-graph-core/src/fusion/mod.rs` | ~600 | RRF fusion — no ModelId match here, but weight arrays |
| `crates/context-graph-core/src/contrastive/mining.rs` | 104-134 | AnomalyKind classifier — may want new rules for extended embedders |

**Benchmark crate:**

| File | Lines | What to extend |
|------|-------|---------------|
| `crates/context-graph-benchmark/src/realdata/config.rs:160-180` | `EmbedderName::parse_from` | String parser |
| `crates/context-graph-benchmark/src/bin/embedder_stress.rs:116-128` | same | String parser |
| `crates/context-graph-benchmark/src/realdata/config.rs:476` | test | Test assertion |

## Tier 2 — Runtime-checkable (no compiler help; tests will catch)

**String parsers:** `from_str`, `parse_from`, config file readers.

- `crates/context-graph-embeddings/src/config/**.rs` — config structs with
  `preload_models: Vec<String>` etc.
- `crates/context-graph-embeddings/src/bin/**.rs` — CLI tools
- `crates/context-graph-mcp/src/handlers/tools/embedder_dtos.rs::from_str`

**Weight registries:** `crates/context-graph-embeddings/src/storage/**.rs` —
routes saved weights by ModelId name.

**Fixed-size arrays (SILENT breakage if missed):**

```rust
// Search for patterns like these
[f32; 13]              // pervasive in profiles, cosine arrays
[Option<T>; NUM_EMBEDDERS]
Vec<f32> with expected length 13
```

Grep: `grep -rE '\[.*;\s*(13|NUM_EMBEDDERS)\]' crates/`

## Tier 3 — Documentation / comments

~200 docstrings, architecture docs, README snippets mention "13 embedders".
Not load-bearing but confusing if stale. Update in same PR.

Files:
- `CLAUDE.md`
- `docs/SHAKESPEARE_LORA_TRAINING_SPEC.md`
- `docs/FULL_SCALE_LORA_PLAN.md`
- `README.md`
- Various rustdoc `//!` blocks

## Tier 4 — Tests that pin the 13 count

Search: `grep -rE 'NUM_EMBEDDERS|\b13\b' crates/*/tests/`

Each of these must be either:
- Updated to expect new count (if testing system invariant)
- Left at 13 (if testing backward compat with v1 data) with explicit comment

Examples:
- `crates/context-graph-embeddings/tests/search_test.rs:34` — `assert NUM_EMBEDDERS == 13`
- `crates/context-graph-embeddings/tests/benchmark_test.rs` — `NUM_EMBEDDERS = MODEL_COUNT = {13}`

## Tier 5 — External touchpoints (NOT in Rust)

Not files, but consumer systems that will break if the MCP schema changes:

- Python `scripts/shakespeare_chat_dashboard.py` — hardcoded `[11, 12, ...]` indices when computing style score. If E14-E18 slot in after E13, these indices stay correct. If we re-order, they break.
- Python `scripts/_gemma_shakespeare_lib.py::score_text` — expects 13-element `per_embedder_cosine`. Must be updated to accept variable length.
- Python `scripts/v4_2/camd_lib.py` — hardcodes daemon-13. Must be updated post-Phase-1.
- Python v4.2 scripts in general — will need coordinated update.

**Key decision:** extend the existing MCP response format (keep old 13 at
positions 0-12, append new embedders at 13+) rather than reordering. This
minimizes consumer breakage.

## Full touchpoint audit per phase

For each phase's checklist, run:

```bash
# 1. Compile-check: will surface all match arms
cargo check --workspace

# 2. Test-check: will surface runtime string parsing
cargo test --workspace -- --nocapture

# 3. External consumer check
grep -rn "per_embedder_cosine\|NUM_EMBEDDERS\|EmbedderIndex\|ModelId::" \
  crates/ scripts/ | grep -v target/
```

This is the definitive three-step audit for every phase.

## Summary by phase

| Phase | New ModelId variants | New CFs | New HNSW indexes | New MCP tools | Approx. files touched |
|-------|---------------------|---------|------------------|---------------|-----------------------|
| 1 (BGE-M3 dense) | 1 | 1 | 1 | 0 (extend existing) | ~40 |
| 2 (BGE-M3 sparse + ColBERT) | 2 | 2 | 2 | 0 | ~30 |
| 3 (Jina-v3) | 1 | 1 | 1 | 0 | ~30 |
| 4 (Qwen3) | 1 | 1 | 1 | 0 | ~35 |
| 5 (BGE-reranker) | 1 | 0 | 0 | 1 | ~25 |
| 6 (integration sweep) | 0 | 0 | 0 | 0 | ~40 |
