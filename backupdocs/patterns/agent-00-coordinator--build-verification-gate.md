---
namespace: patterns
agent_id: agent-00-coordinator
timestamp: 2026-04-14T23:32:00Z
status: active
tags: patterns, build, verification, workspace, all-targets
---

# Pattern: Build Verification Gate for Struct Changes

## Rule
Any agent that modifies a struct used across multiple crates MUST verify with:
```
env CUDA_HOME=/usr/local/cuda cargo check --workspace --all-targets
```

NOT `cargo check -p <crate>` — per-crate checks miss test-mode and integration-test call sites in downstream crates.

## Why
Rust enforces all struct fields at every initializer. A new field on a public struct requires updating every `Struct { ... }` call site in the workspace, including:
- Lib sources (`src/**/*.rs`)
- Test-mode code (`#[cfg(test)]` modules)
- Integration tests (`tests/*.rs`)
- Test helpers in other crates that depend on the crate

`cargo check -p <crate>` only compiles the crate's own lib. Downstream crates' test files are not compiled.

## Worked Example
Adding `edge_type_distribution: [u32; 8]` to `TrainingRecord` in `context-graph-core` broke:
1. `context-graph-mcp` (MCP handler constructor)
2. `context-graph-cli` (integration test fixture)
3. `context-graph-storage` (unit test `mock_record` + integration test fixture)

All four surfaced only under `cargo check --workspace --all-targets`, not under `cargo check -p context-graph-core`.

## Mandatory Sequence for Struct Changes
1. Make the change to the type definition.
2. Run `env CUDA_HOME=/usr/local/cuda cargo check --workspace --all-targets`.
3. Fix every E0063 / missing-field error it reports.
4. Re-run until clean.
5. Only then run `cargo test --workspace` to validate semantics.

## Counterexample (what NOT to do)
```
cargo check -p context-graph-core           # ✓ passes
cargo test -p context-graph-core --lib      # ✓ passes
# → Agent declares success
# → Other 4 call sites still broken, workspace build fails
```
