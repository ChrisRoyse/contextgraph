---
namespace: handoff
agent_id: agent-04-coder-mcp
timestamp: 2026-04-15T08:45:00Z
status: ready-for-pickup
tags: handoff, phase-4-complete, phase-5-ready, mcp-tools, typed-edges
---

# Handoff: Phase 4 MCP Tools Landed → Phase 5 (Integration Tests / FSV) Ready

## Summary for agent-05

All four typed-edge MCP tools are wired end-to-end and build green on
`cargo check --workspace --all-targets`. The MCP lib test suite (340
tests) and graph_linking integration tests (7 tests) pass with zero
failures. Phase 5 can now write real RocksDB integration tests against
the new dispatch surface.

## New tool names

Dispatched by `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`
via `tool_dispatch!` macro:

| Tool name | Feature gate | Phase ID |
|---|---|---|
| `export_typed_edges_corpus` | always | F1 |
| `derive_anomalies_from_edges` | always | F2 |
| `validate_low_confidence_edges` | LLM-gated internally | F4 |
| `list_typed_edge_records` | always | read |

The total MCP tool count is now **75** (bumped from 71) with LLM enabled
and **71** without LLM. Assertions updated in both
`tools/definitions/mod.rs::tests` and
`handlers/tests/mcp_protocol_e2e_test.rs`.

## Parameter schemas

All schemas use `additionalProperties: false`.

### `export_typed_edges_corpus`
```
maxEdges?: int            default 10000  max 1_000_000
includeContent?: bool     default true
includeSourceMetadata?: bool default true
includeMechanismType?: bool default true
joinLLMValidation?: bool  default true
clearExisting?: bool      default false
```

### `derive_anomalies_from_edges`
```
highThreshold?: float     default 0.60
lowThreshold?: float      default 0.30
minDisagreement?: float   default 0.30
maxPairs?: int            default 10000  max 1_000_000
kinds?: array<string>     → always InvalidParams (not yet supported)
```

### `validate_low_confidence_edges`
```
weightCeiling?: float     default 0.60
maxEdges?: int            default 100    max 10_000
sessionFilter?: string    optional
```

### `list_typed_edge_records`
```
limit?: int     default 100  max 1_000
offset?: int    default 0
includeFull?: bool  default false
```

## Response shapes (JSON, as emitted inside `tool_result`)

### `export_typed_edges_corpus`
```json
{
  "status": "exported",
  "exported": N,
  "errors": N,
  "cleared_before_export": N,
  "edges_scanned": N,
  "skipped_after_max": N,
  "duration_ms": N,
  "storage": { "backend": "rocksdb", "column_family": "typed_edge_records", "format": "version_byte + bincode" },
  "record_shape": { "content_included": bool, "source_metadata_included": bool, "mechanism_type_included": bool, "llm_validation_included": bool }
}
```

### `derive_anomalies_from_edges`
```json
{
  "status": "derived",
  "summary": {
    "edges_scanned": N,
    "pairs_written": N,
    "skipped_below_threshold": N,
    "per_kind_counts": { "<kind_snake_case>": N, ... },
    "duration_ms": N
  },
  "config": { "high_threshold": F, "low_threshold": F, "min_disagreement": F, "max_pairs": N }
}
```

### `validate_low_confidence_edges`
Two shapes:

- **When LLM available and edges found**:
  ```json
  {
    "status": "validated",
    "validated": N,
    "already_had_validation": N,
    "errors": N,
    "skipped_above_ceiling": N,
    "skipped_by_session_filter": N,
    "duration_ms": N
  }
  ```
- **When LLM unavailable**:
  ```json
  {
    "status": "skipped",
    "reason": "llm feature not enabled"
      | "graph-agent API not exposed for single-edge validation"
      | "graph-agent LLM not loaded",
    "validated": 0,
    "duration_ms": N
  }
  ```

### `list_typed_edge_records`
```json
{
  "total": N,
  "offset": N,
  "limit": N,
  "returned": N,
  "include_full": bool,
  "records": [
    { "source_memory_id": "...", "target_memory_id": "...", "edge_type": N, "edge_type_name": "..." },
    ...
  ]
}
```
When `include_full=true`, each record additionally carries all fields of
`TypedEdgeTrainingRecord` (weight, direction, embedder_scores,
agreement_count, agreeing_embedders, source_content, target_content,
source_session_id, target_session_id, source_type, target_type,
mechanism_type, llm_validation {validated_at, verdict, confidence,
rationale, validator_version}, exported_at, exporter_version).

Missing records within the offset/limit slice are returned with
`"absent": true` on the composite-key stub rather than dropping silently.

## Error taxonomy observed by handlers

| Condition | Behavior |
|---|---|
| Store is not RocksDbTeleologicalStore | `tool_error(...)` with explanatory message |
| Handler lacks an `EdgeRepository` | `tool_error(...)` with explanatory message |
| `kinds` filter supplied to F2 | `JsonRpcResponse::error` with `InvalidParams (-32602)` |
| Edge's source/target content missing (F4 only — LLM needs it) | logged as error, edge counted as `errors`, loop continues |
| `store_typed_edge_record` RocksDB op fails | logged as error, edge counted as `errors`, loop continues |
| `iter_all_typed_edges` fails | `tool_error(...)` with the propagated error |
| `clear_all_typed_edge_records` fails | `tool_error(...)` with the propagated error |

## Files changed / created

Created:
- `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs` (203 lines)
- `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs` (658 lines)

Modified:
- `crates/context-graph-mcp/src/tools/names.rs` — +12 lines (4 consts + comment)
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` — submodule
  registration, extend ALL_TOOLS, bumped test assertions
- `crates/context-graph-mcp/src/handlers/tools/mod.rs` — `mod typed_edges_tools;`
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` — 4 new arms
- `crates/context-graph-mcp/src/handlers/tests/mcp_protocol_e2e_test.rs`
  — bumped `tools.len()` assertion 71 → 75

## Build verification (gate)

```
env CUDA_HOME=/usr/local/cuda cargo check --workspace --all-targets
# → Finished `dev` profile ... in 7.72s
# → zero errors, zero warnings

env CUDA_HOME=/usr/local/cuda cargo test -p context-graph-mcp -- --test-threads=4
# → test result: ok. 340 passed; 0 failed; 0 ignored  (lib)
# → test result: ok. 7 passed; 0 failed  (graph_linking integration)
```

## Notes for Phase 5 integration tests

1. Use `create_test_handlers_with_real_embeddings_store_access()` (returns
   `(Handlers, Arc<dyn TeleologicalMemoryStore>, TempDir)`) so you can
   downcast the store for FSV assertions. Pattern:
   ```rust
   let rocksdb_store = store
       .as_any()
       .downcast_ref::<RocksDbTeleologicalStore>()
       .expect("TempDir store is RocksDB");
   ```
2. The handlers' `edge_repository` is `None` under `with_defaults`. You
   must use a constructor that populates it (e.g.
   `Handlers::without_llm(..., edge_repository, graph_builder, ...)`).
   The F2/F4 handlers explicitly fail-fast with a clear message when the
   repo is missing.
3. To test F4 without a live LLM, use the stub
   `create_stub_graph_discovery_service()` — its LLM is not loaded, so
   the handler returns `status: "skipped"` with reason `"graph-agent LLM
   not loaded"`. Assert on that for the CI path.
4. For the canonical 6-memory FSV fixture, seed edges via
   `edge_repository.store_typed_edges_batch(&[...])`, call
   `export_typed_edges_corpus` through dispatch, then verify
   `rocksdb_store.count_typed_edge_records().await? == 8` (one per
   seeded edge).
