---
namespace: journal
agent_id: agent-04-coder-mcp
timestamp: 2026-04-15T08:40:00Z
status: success
severity: info
tags: phase-4, mcp-tools, typed-edges, build-green, tests-green
---

# Phase 4 MCP tools landed — 4 new tools, 75 total

## Summary
All four typed-edge MCP tools are implemented, wired into dispatch, and
covered by unit + schema tests. Workspace builds clean with zero warnings.
MCP test suite (340 lib + 7 integration) passes with 0 failures.

## Files touched (all absolute)
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/names.rs`
  — added 4 constants.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions/typed_edges.rs`
  — NEW, 4 schemas + 2 tests.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions/mod.rs`
  — registered submodule, bumped total count 71→75, bumped
  non-LLM count 67→71, added typed_edges::definitions().len() == 4
  assertion.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs`
  — NEW, 4 handlers + 3 helper tests.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs`
  — registered `mod typed_edges_tools;`.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/dispatch.rs`
  — 4 new arms in `tool_dispatch!` after contrastive arms.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/mcp_protocol_e2e_test.rs`
  — bumped `tools.len() == 71` to `tools.len() == 75` with explanatory
  comment.

## Design decisions
1. **F2 kinds filter**: rejected with `McpError::InvalidParams` per spec
   ("Pick the simpler path and file a blocker if the user needs the
   filter"). No blocker raised because the underlying storage method
   doesn't support it and the rejection is explicit and documented in the
   schema description.
2. **F4 LLM integration**: use `GraphDiscoveryService::llm()` →
   `GraphRelationshipLLM::analyze_relationship(src, tgt)` (no expected
   type). Maps `has_connection=true` → `Valid`, false → `Invalid`. See
   `discoveries/agent-04-coder-mcp--graph-agent-api-shape.md` for full
   mapping decisions.
3. **F4 skipped-path triggers**: `llm feature not enabled`,
   `graph-agent API not exposed for single-edge validation`,
   `graph-agent LLM not loaded`. All return `status: "skipped"` with
   reason + `duration_ms`, not an error.
4. **Prompt hash**: stable SHA-256 envelope over inputs actually sent to
   the LLM (tag + len-prefixed source + len-prefixed target). See
   `compute_prompt_hash` in `typed_edges_tools.rs`.
5. **Mechanism-type resolution**: `CausalRelationship` carries no target
   fingerprint id, so we take the first non-empty `mechanism_type` from
   the source's causal relationships (matches the pattern already used in
   `training_tools.rs`).

## Build / test results
- `env CUDA_HOME=/usr/local/cuda cargo check --workspace --all-targets`:
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.72s
  — zero errors, zero warnings.
- `env CUDA_HOME=/usr/local/cuda cargo test -p context-graph-mcp --
  --test-threads=4`: `test result: ok. 340 passed; 0 failed; 0 ignored`
  (lib) + `test result: ok. 7 passed; 0 failed` (integration).

## Tool count
`names.rs` now has **75** tool constants (was 71). Verified via
`test_total_tool_count_and_no_duplicates` assertion that passes with
`tools.len() == 75` under the `llm` feature.
