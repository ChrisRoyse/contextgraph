---
namespace: discoveries
agent_id: codex
created: 2026-05-09T09:38:38Z
updated: 2026-05-09T09:38:38Z
status: active
tags: mejepa, embedders, ast-chunking, routing, fsv, root-cause
---

# Discovery: Embedder Routing Was Not Spec-Complete or FSV-Proven

## Root Cause

The Phase 1 AST routing surface only exposed a small direct `EntityType -> BTreeSet<EmbedderId>` helper. It did not represent all E1-E21 IDs, did not expose the slot-internal routing keys from REQ-EMBED-04 (`DiffHunk`, `AstNodeSequence`, `CfgBasicBlock`, `DefUseEdge`), and did not have a typed `MEJEPA_EMBED_ROUTING_MISSING` error path. The AST chunker also assumed route lookup could not fail during chunk construction.

## Fix Shape

- `EmbedderId` now covers E1-E21 with projected dimensions and content/learner-state classification.
- Routing is backed by a static const table with 19 entries: 13 entity entries and 6 special routing-key entries.
- `route_for_key` / `route_for_entity_type` return `Result<RoutingResult, RoutingError>`.
- AST chunk construction now propagates missing-route failures as `AstChunkerError::RoutingMissing` with code `MEJEPA_EMBED_ROUTING_MISSING`.
- Compatibility helper `route_to_embedders` remains for existing callers, but the chunker uses the fallible route path.

## Evidence

- Unit: `cargo test -p context-graph-core ast::tests --no-default-features --jobs 1 -- --nocapture` passed 10/10.
- Smoke FSV: `cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv` wrote `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json` with `passes=true`, `panel_dim=5120`, `panel_filled_mask=32767`, 11 real sample files, and 136 AST chunks.
- Direct RocksDB readback for the Phase 0 one-task corpus confirmed `mejepa_mutation_corpus` and `mejepa_oracle_verdicts` each contain 8 keys via `ldb dump --count_only`.

## Practice Note

Tree-sitter explicitly materializes `ERROR` and `MISSING` nodes, so parser recovery must be treated as a failed parse for ME-JEPA training evidence. Code RAG guidance also reinforces AST/CST-aware chunking with context-preserving chunks rather than naive text boundaries.
