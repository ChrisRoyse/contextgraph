# Phase 3 — Specification

## Objectives

1. Create a self-contained `reality_loop` module inside the existing `context-graph-mcp` crate, parallel to the other tool families (memory_tools, graph_tools, causal_tools, etc.).
2. Wire the module into the MCP server's tool registry, gated behind a `--mode reality-loop` CLI flag.
3. Implement 11 ANALYZE-family tools whose responses always include explicit `source_of_truth` references.
4. Re-export the memory/graph/causal tools under the same `cgreality` MCP namespace.
5. Implement the `path_policy` loader that reads `config/ccreality_governed_paths.json` (Phase 5 produces that file).

## Scope

In scope:
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/` — new module tree (~1,500–2,500 LOC).
- `crates/context-graph-mcp/src/handlers/tools/mod.rs` — register the new module.
- `crates/context-graph-mcp/src/main.rs` — add `--mode` flag and dispatch.
- `crates/context-graph-mcp/Cargo.toml` — possibly add `jsonschema` dependency (used in Phase 4) — defer if not needed in Phase 3.

Out of scope:
- ALTER tools (Phase 4).
- INTERACT tools (Phase 4).
- The `reality_shift_log` and `reality_shift_compare_to_my_view` tools (Phase 4).

## Deliverables

```
crates/context-graph-mcp/src/handlers/tools/reality_loop/
├── mod.rs              # module declaration + tool registry
├── analyze.rs          # all 11 analyze-family tools
├── path_policy.rs      # loads ccreality_governed_paths.json
├── errors.rs           # CCRealityError type
└── helpers.rs          # shared utilities (reading runtime root, parsing attempt dirs, etc.)
```

Plus modifications to:
- `crates/context-graph-mcp/src/handlers/tools/mod.rs` (one new `pub mod reality_loop;`)
- `crates/context-graph-mcp/src/main.rs` (--mode CLI + dispatch)

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). Five clauses:
- `cargo build -p context-graph-mcp` succeeds.
- The 5 module files exist.
- `--mode reality-loop --help` returns 0.
- `tools/list` JSON-RPC returns ≥11 reality_* tool names.
- A `tools/call` for `reality_latest_root` returns a payload with a `runtime_root` field (or `null` if no active root).

## Time budget

2–4 hours. Breakdown:
- Tasks 3.1–3.3 (module skeleton + main dispatch): 30 min.
- Task 3.4 (path_policy loader): 30 min.
- Tasks 3.5–3.15 (the 11 analyze tools): 60–120 min total (~5–10 min each).
- Task 3.16 (re-export memory tools): 15 min.
- Task 3.17 (verify): 15 min.
- Iteration on rmcp protocol issues, JSON-RPC framing, stderr-only logging: 0–60 min.

## Logging discipline

**Critical:** rmcp uses stdio for JSON-RPC. Any stdout from the server (including println!, dbg!) corrupts the protocol and Claude Code silently disconnects.

Every log statement MUST go through `tracing` configured to write to stderr:
```rust
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .init();
```

This is asserted by the verification: a smoke test pipes a JSON-RPC request through the server and expects stdout to be valid JSON-RPC only.

## Backward compat

The existing `context-graph-mcp` binary continues to work without `--mode reality-loop`. The new mode is additive. Without the flag, the server registers only the existing tool families (memory, graph, causal, etc.). With the flag, those tools are registered AND the new `reality_*` tools.
