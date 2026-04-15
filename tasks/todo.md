# Typed-Edge Training-Data Factory — TODO

## Status Key
- [ ] = todo
- [~] = in progress
- [x] = done
- [!] = blocked (see `memory/blockers/`)

## Phase 1: Spec & Architecture
- [x] Bootstrap memory context (mission-brief-typed-edges-v1.md, current-phase-typed-edges-v1.md)
- [~] Agent-01 architect produces implementation spec

## Phase 2: Core Types (context-graph-core) — DONE (with coordinator-applied fixes)
- [x] Bump TRAINING_RECORD_VERSION to 2 (NO backwards compat)
- [x] Add `edge_type_distribution: [u32; 8]` to TrainingRecord
- [x] Define `TypedEdgeTrainingRecord` struct + TYPED_EDGE_RECORD_VERSION constant
- [x] Define `LLMEdgeValidation` struct + LLM_EDGE_VALIDATION_VERSION constant
- [x] Define `LLMVerdict` enum (Valid | Invalid | Reclassify { new_type: u8 })
- [x] Unit tests for bincode roundtrip of all three new types
- [x] Fix 4 downstream call sites (MCP handler, 3 test fixtures) — coordinator fix, agent missed them
- [x] `cargo check --workspace --all-targets` clean

## Phase 3: Storage (context-graph-storage) — DONE
- [x] Add `CF_TYPED_EDGE_RECORDS` + `CF_TYPED_EDGE_VALIDATIONS` to column_families.rs
- [x] Bump `TOTAL_COLUMN_FAMILIES` 57 → 59
- [x] Bump `TELEOLOGICAL_CF_COUNT` 26 → 28
- [x] `store/get/list/count_typed_edge_record` in new module `rocksdb_store/typed_edge_export.rs`
- [x] `store/get_llm_edge_validation` in `rocksdb_store/llm_validation.rs`
- [x] `derive_anomalies_from_edges` helper with `classify_edge_as_anomaly` classifier
- [x] `AnomalyKind` got `Hash` derive so `per_kind_counts: HashMap<AnomalyKind, usize>` compiles
- [x] 3 real-data integration tests passing (25 tests total: 7 + 7 + 11)
- [x] `cargo check --workspace --all-targets` + `cargo test -p context-graph-storage` both green

## Phase 4: MCP Tools (context-graph-mcp)
- [ ] `export_typed_edges_corpus` tool
- [ ] `derive_anomalies_from_edges` tool
- [ ] `validate_low_confidence_edges` tool (LLM-gated)
- [ ] `list_typed_edge_records` tool
- [ ] Update `tools/names.rs` (71 → 75)

## Phase 5: Integration Tests (NO MOCKS)
- [ ] `storage/tests/typed_edge_export_integration.rs`
- [ ] `storage/tests/llm_validation_integration.rs` (structural-only when LLM disabled)
- [ ] `storage/tests/anomaly_derivation_integration.rs`
- [ ] `core/src/training/tests.rs` — `edge_type_distribution` sum assertions
- [ ] `mcp/src/handlers/tests/typed_edges_tools.rs`

## Phase 6: Full State Verification (Manual, Me)
- [ ] `cargo check --workspace` clean
- [ ] `cargo test --workspace` clean
- [ ] MCP daemon tools/list count = 75
- [ ] Synthetic happy path end-to-end
- [ ] Edge cases: empty CF, malformed record, version mismatch

## Phase 7: Simplifier + Commit
- [ ] code-simplifier review
- [ ] `tasks/typed_edges_fsv_report.md`
- [ ] Git commit + push

## Review
_To be filled in after phases complete._
