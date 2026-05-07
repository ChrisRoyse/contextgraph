---
namespace: handoff
agent_id: agent-02-coder-core
timestamp: 2026-04-15T04:36:00Z
status: ready-for-pickup
tags: handoff, phase-2-complete, phase-3-ready, storage, typed-edges
---

# Handoff: Phase 2 Core Types Landed → Phase 3 (Storage) Ready

## Summary for agent-03 (storage)

All Phase-2 core types and constants are now landed in `context-graph-core`.
Storage code (Phase 3) can now `use` them directly without further core
changes.

## What is now available in `context_graph_core`

### `context_graph_core::training`
- `pub const TRAINING_RECORD_VERSION: u8 = 2;`  (was 1)
- `pub const NUM_EDGE_TYPE_DISTRIBUTION: usize = 8;`
- `pub use edge_signature::{compute_edge_type_distribution, NUM_EDGE_TYPE_DISTRIBUTION};`
- `TrainingRecord` now has `pub edge_type_distribution: [u32; NUM_EDGE_TYPE_DISTRIBUTION]` at end of struct.
- New module `pub mod edge_signature;` with `compute_edge_type_distribution<I>(edges: I) -> [u32; 8]` where `I: IntoIterator<Item = &'a TypedEdge>`.

### `context_graph_core::typed_edge_export` (NEW MODULE)
- `pub const TYPED_EDGE_RECORD_VERSION: u8 = 1;`
- `pub struct TypedEdgeTrainingRecord` — full wire type per spec (source_memory_id, target_memory_id, edge_type, edge_type_name, weight, direction, embedder_scores, agreement_count, agreeing_embedders, content joins, mechanism_type, llm_validation, exported_at, exporter_version).
- `pub struct LLMValidationSummary` — compact LLM verdict view embedded in records.
- `pub enum LLMVerdict { Valid, Invalid, Reclassify { new_edge_type: u8 } }` with `as_str()` helper.

### `context_graph_core::llm_edge_validation` (NEW MODULE)
- `pub const LLM_EDGE_VALIDATION_VERSION: u8 = 1;`
- `pub struct LLMEdgeValidation` — full persisted row (validated_at, verdict, confidence, rationale, auto_derived_weight, validator_version, prompt_hash[32]). Re-uses `LLMVerdict` from `typed_edge_export`.

## Build state at handoff
- `env CUDA_HOME=/usr/local/cuda cargo check -p context-graph-core` -> Finished, no errors, no NEW warnings.
- `env CUDA_HOME=/usr/local/cuda cargo test -p context-graph-core --lib` -> 2042 passed, 0 failed, 0 ignored.

## What Phase 3 (storage) needs to do next
Per the master spec at `memory/handoff/agent-00-to-02--typed-edges-implementation-spec.md` Phase 3:

1. In `crates/context-graph-storage/src/teleological/column_families.rs`:
   - Add `pub const CF_TYPED_EDGE_RECORDS: &str = "typed_edge_records";`
   - Add `pub const CF_TYPED_EDGE_VALIDATIONS: &str = "typed_edge_validations";`
   - Add 33-byte-prefix CF options helpers `typed_edge_records_cf_options(cache)` and `typed_edge_validations_cf_options(cache)`.
   - Append both to `TELEOLOGICAL_CFS` array.
   - Bump `TELEOLOGICAL_CF_COUNT` 26 -> 28.
   - Add 2 ColumnFamilyDescriptor rows to `get_all_teleological_cf_descriptors`.

2. In `crates/context-graph-storage/src/column_families.rs`:
   - Bump `TOTAL_COLUMN_FAMILIES` 57 -> 59 (and update the comment).

3. Create three new submodules under `teleological/rocksdb_store/`:
   - `typed_edge_export.rs` (CRUD + encode/decode for `CF_TYPED_EDGE_RECORDS`).
   - `llm_validation.rs` (CRUD + encode/decode for `CF_TYPED_EDGE_VALIDATIONS`, sharing the 33-byte composite key helper).
   - `anomaly_derivation.rs` (F2: walk `CF_TYPED_EDGES` + classify_edge_as_anomaly + write to `CF_CONTRASTIVE_PAIRS`).

4. Modify `teleological/rocksdb_store/export.rs::build_training_record`:
   - After populating `outgoing_edges: Vec<TrainingEdge>`, compute the
     8-dim distribution from those edges (use a tiny helper
     `edge_type_distribution_from_training_edges(&[TrainingEdge]) -> [u32; 8]`
     since `TrainingEdge.edge_type` is already `u8`).
   - Set `record.edge_type_distribution = ...` before emitting the record.
   - DO NOT use `compute_edge_type_distribution` directly here because the
     iterator has already been collapsed from `TypedEdge` to `TrainingEdge`.
     The two-line helper is the right move — file as a duplicate-of-purpose
     in your discoveries if you'd rather re-walk.

5. Three new integration tests:
   - `tests/typed_edge_export_integration.rs`
   - `tests/llm_validation_integration.rs`
   - `tests/anomaly_derivation_integration.rs`
   See spec lines 916-933 for the exact assertions.

## Risks / things to watch
- Existing `CF_TRAINING_RECORDS` rows on disk (if any test fixture or dev DB)
  are now **unreadable** by the v2 decoder. Either nuke the dev DB, or test
  with a fresh `TempDir`. The decoder MUST reject v1 with structured error.
- `TrainingRecord` struct now has 1 extra field; any test/example that
  builds it via struct-literal needs `edge_type_distribution: [0u32; 8]` (or
  meaningful values). I already updated `training/tests.rs::sample_record()`.
  Search the workspace for direct constructions:
    `Grep TrainingRecord {` workspace-wide.

## Files touched in Phase 2
Modified:
- `crates/context-graph-core/src/lib.rs`  (registered 2 new modules)
- `crates/context-graph-core/src/training/mod.rs`  (version bump + field + re-exports + docs)
- `crates/context-graph-core/src/training/tests.rs`  (new tests + fixture update)

Created:
- `crates/context-graph-core/src/training/edge_signature.rs`
- `crates/context-graph-core/src/typed_edge_export/mod.rs`
- `crates/context-graph-core/src/typed_edge_export/tests.rs`
- `crates/context-graph-core/src/llm_edge_validation/mod.rs`
- `crates/context-graph-core/src/llm_edge_validation/tests.rs`
