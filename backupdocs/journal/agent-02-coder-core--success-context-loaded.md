---
namespace: journal
agent_id: agent-02-coder-core
timestamp: 2026-04-15T04:17:00Z
status: active
tags: context-loaded, ready-to-implement, phase-2
---

# Context loaded: starting Phase 2 (core types)

## Verified ground truth (against code, not memory)
- `TrainingRecord` at `crates/context-graph-core/src/training/mod.rs` line 69, struct ends at line 146 with `tucker_core: Option<TuckerCore>`.
- `TRAINING_RECORD_VERSION` at line 55, currently `1`.
- `GraphLinkEdgeType` at `crates/context-graph-core/src/graph_linking/edge_type.rs`, 8 variants u8 repr, `COUNT = 8`. Confirmed `as_u8()`, `primary_embedder_index()`, `is_asymmetric()` accessors all public.
- `TypedEdge` at `crates/context-graph-core/src/graph_linking/typed_edge.rs`, all accessor methods present (source, target, edge_type, weight, direction, embedder_scores returns `[f32; 13]`, agreement_count, agreeing_embedders).
- `AnomalyKind` and `ContrastivePair` confirmed in `contrastive/types.rs`.
- `CoreError::SerializationError(String)` is canonical (legacy.rs:72).

## Plan
1. Modify `training/mod.rs`: bump version to 2, add NUM_EDGE_TYPE_DISTRIBUTION const, add `edge_type_distribution: [u32; 8]` field to TrainingRecord at end, register `pub mod edge_signature`, re-export.
2. Create `training/edge_signature.rs` per spec.
3. Update `training/tests.rs`: add field to constructors, add 2 new tests.
4. Create `typed_edge_export/mod.rs` + `tests.rs`.
5. Create `llm_edge_validation/mod.rs` + `tests.rs`.
6. Register both new modules in `lib.rs`.
7. Run cargo check + cargo test --lib.
