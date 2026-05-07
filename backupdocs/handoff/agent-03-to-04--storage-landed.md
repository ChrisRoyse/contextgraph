---
namespace: handoff
agent_id: agent-03-coder-storage
timestamp: 2026-04-15T05:20:00Z
status: ready-for-pickup
tags: handoff, phase-3-complete, phase-4-ready, mcp-tools, typed-edges
---

# Handoff: Phase 3 Storage Landed → Phase 4 (MCP Tools) Ready

## Summary for agent-04 (MCP tools)

All Phase-3 storage CRUD, anomaly derivation, and column families are now
landed in `context-graph-storage`. Phase 4 can wire the four new MCP tools
directly against `RocksDbTeleologicalStore` + `EdgeRepository` with zero
further storage changes.

## What is now available

### Column families (opened at DB init)
- `CF_TYPED_EDGE_RECORDS` — `const CF_TYPED_EDGE_RECORDS: &str = "typed_edge_records"`.
  - Key: 33 bytes `[source:16][target:16][edge_type:u8]`.
  - Value: `[TYPED_EDGE_RECORD_VERSION: u8][bincode(TypedEdgeTrainingRecord)]`.
- `CF_TYPED_EDGE_VALIDATIONS` — `const CF_TYPED_EDGE_VALIDATIONS: &str = "typed_edge_validations"`.
  - Same 33-byte key shape.
  - Value: `[LLM_EDGE_VALIDATION_VERSION: u8][bincode(LLMEdgeValidation)]`.

Both constants live in
`crates/context-graph-storage/src/teleological/column_families.rs` and are
re-exported via `context_graph_storage::teleological::*` (same module as
the existing contrastive constants).

`TELEOLOGICAL_CF_COUNT` = 28 (was 26).
`TOTAL_COLUMN_FAMILIES` = 59 (was 57).

### RocksDbTeleologicalStore methods (Phase 4 can call directly)

All `async fn` returning `CoreResult<...>`.

**Typed-edge export CRUD** (`typed_edge_export.rs`, 359 lines):
- `store_typed_edge_record(&TypedEdgeTrainingRecord) -> ()`
- `get_typed_edge_record(source, target, edge_type) -> Option<TypedEdgeTrainingRecord>`
- `list_typed_edge_record_keys() -> Vec<(Uuid, Uuid, u8)>`
- `count_typed_edge_records() -> usize`
- `clear_all_typed_edge_records() -> usize`

**LLM validation CRUD** (`llm_validation.rs`, 295 lines):
- `store_llm_edge_validation(source, target, edge_type, &LLMEdgeValidation) -> ()`
- `get_llm_edge_validation(source, target, edge_type) -> Option<LLMEdgeValidation>`
- `list_llm_edge_validation_keys() -> Vec<(Uuid, Uuid, u8)>`
- `count_llm_edge_validations() -> usize`
- `clear_all_llm_edge_validations() -> usize`

**Anomaly derivation (F2)** (`anomaly_derivation.rs`, 370 lines):
- `derive_anomalies_from_edges(&EdgeRepository, &AnomalyDerivationConfig) -> AnomalyDerivationSummary`

### New public types

Path: `context_graph_storage::teleological::rocksdb_store`.

```rust
pub use anomaly_derivation::{
    classify_edge_as_anomaly, AnomalyDerivationConfig, AnomalyDerivationSummary,
};
pub use llm_validation::{decode_llm_edge_validation, encode_llm_edge_validation};
pub use typed_edge_export::{
    decode_typed_edge_record, encode_typed_edge_record, typed_edge_record_key,
};
```

- `AnomalyDerivationConfig`:
  - `high_threshold: f32` (default 0.60)
  - `low_threshold: f32` (default 0.30)
  - `max_pairs: usize` (default 10_000)
  - `min_disagreement: f32` (default 0.30)

- `AnomalyDerivationSummary`:
  - `edges_scanned: usize`
  - `pairs_written: usize`
  - `skipped_below_threshold: usize`
  - `per_kind_counts: HashMap<AnomalyKind, usize>`
  - `duration_ms: u64`

## What Phase 4 should do next

1. Add `EXPORT_TYPED_EDGES_CORPUS`, `DERIVE_ANOMALIES_FROM_EDGES`,
   `VALIDATE_LOW_CONFIDENCE_EDGES`, `LIST_TYPED_EDGE_RECORDS` name
   constants in `crates/context-graph-mcp/src/tools/names.rs`.
2. Create `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs`
   with four JSON schemas per spec §Phase 4 (and register the submodule in
   `tools/definitions/mod.rs`).
3. Create `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs`
   with four handlers:
   - Downcast `teleological_store` to `RocksDbTeleologicalStore` — fail
     with `McpError::ToolError { code: -32001 }` if not.
   - Call the storage methods listed above.
   - For `export_typed_edges_corpus`: walk `EdgeRepository::iter_all_typed_edges`,
     optionally join content + source_metadata + mechanism_type (from
     `CF_CAUSAL_RELATIONSHIPS` when edge type is CausalChain) + LLM
     validation via `get_llm_edge_validation`.
4. Register the four tool names in the `tool_dispatch!` macro
   (`handlers/tools/dispatch.rs`).

## Minor Phase-2 amendment landed
While wiring `AnomalyDerivationSummary::per_kind_counts`, I discovered
`AnomalyKind` in `crates/context-graph-core/src/contrastive/types.rs` did
NOT derive `Hash`, so `HashMap<AnomalyKind, _>` wouldn't compile. Added
`Hash` to the derive list. See
`memory/discoveries/agent-03-coder-storage--anomaly-kind-hash-trait-missing.md`.

## Build state at handoff
- `env CUDA_HOME=/usr/local/cuda cargo check --workspace --all-targets` →
  Finished in 10.45s, zero errors, zero warnings.
- `env CUDA_HOME=/usr/local/cuda cargo test -p context-graph-storage --
  --test-threads=4` → 623 passing library/integration tests + 35/50 doc
  tests (15 ignored), zero failures.
  - 15 NEW tests across three integration files (all passing).

## Integration-test fixture gotcha for Phase-4 tests
`RocksDbTeleologicalStore::store_content_async` tolerates missing
fingerprints but `derive_anomalies_from_edges` calls `get_content_async`
on every classified edge's source + target. If you seed typed edges
without matching content rows, the anchor/negative texts on the
`ContrastivePair` will be empty strings (not an error). The integration
test `anomaly_derivation_integration.rs` seeds content first — mirror
that pattern in any Phase-4 E2E test.
