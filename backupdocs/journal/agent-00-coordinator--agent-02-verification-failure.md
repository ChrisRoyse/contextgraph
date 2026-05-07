---
namespace: journal
agent_id: agent-00-coordinator
timestamp: 2026-04-14T23:30:00Z
status: active
severity: medium
tags: verification, agent-02, false-success, test-call-sites
---

# Agent-02 claimed success but workspace was broken

## What Happened
Agent-02-coder-core returned with a Phase-2 completion report stating:
> cargo check (PASSED) ... Finished `dev` profile ... in 1.89s
> cargo test (PASSED) ... 2042 passed; 0 failed

Both commands were run with `-p context-graph-core` (single crate only). The agent did NOT run `cargo check --workspace --all-targets`.

Personal verification with `cargo check --workspace --all-targets` revealed FOUR unresolved E0063 call-site errors on the new `edge_type_distribution` field:

1. `crates/context-graph-mcp/src/handlers/tools/training_tools.rs:510` — MCP handler constructor
2. `crates/context-graph-cli/tests/export_training_integration.rs:52` — CLI integration test
3. `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs:332` — storage unit test mock_record
4. `crates/context-graph-storage/tests/training_records_integration.rs:39` — storage integration test

## Why It Matters
- The user explicitly instructed: *"Make sure you always check subagents work personally yourself and don't rely on their responses to be correct without verifying/validating what they've done yourself."*
- This is a pattern: per-crate builds miss downstream call sites.
- Future agents MUST run `cargo check --workspace --all-targets` as their verification gate, not `cargo check -p <crate>`.

## Resolution (me)
I fixed all four sites:
- MCP handler: computed `edge_type_distribution` inline from `outgoing_edges: Vec<TrainingEdge>` via `.iter() → saturating_add(1)` indexed by `edge_type as usize`.
- CLI + storage integration tests: set `edge_type_distribution: {let mut d=[0u32;8]; d[0]=1; d}` to match the one SemanticSimilar outgoing edge already seeded in the fixture.
- Storage unit test mock_record: `edge_type_distribution: [0u32; 8]` (zero outgoing edges in the mock).

`cargo check --workspace --all-targets` now passes in 31.55s.

## Lesson for Future Agents
Build verification gate for any agent that changes `TrainingRecord` (or any widely-used struct):
```
env CUDA_HOME=/usr/local/cuda cargo check --workspace --all-targets
```
Must pass. Per-crate check is insufficient because struct field changes affect test call sites in downstream crates that only compile under `--all-targets` or `cargo test`.
