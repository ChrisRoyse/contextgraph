# Phase 4 — Preconditions

## Required state

1. **Phase 3 verification passes.** `tools/list` returns the 11 reality_* tools. `cargo build -p context-graph-mcp` succeeds. The MCP server has correct stderr-only logging.
2. **Phase 5 verification passes.** `config/ccreality_governed_paths.json` and `config/ccreality_recommendation_schema.json` exist and are valid JSON. (Phase 5 can be done before or in parallel with Phase 4; do it before 4 if not yet done.)
3. **Phase 1 governed_edit module is on the path.** The MCP crate must be able to depend on `context_graph_cli::governed_edit`. Verify the workspace dependency:
   ```bash
   grep -A 3 'context-graph-cli' /home/cabdru/contextgraph/crates/context-graph-mcp/Cargo.toml
   ```
   If not present, add:
   ```toml
   context-graph-cli = { path = "../context-graph-cli" }
   ```
   And test with `cargo build -p context-graph-mcp`.
4. **Phase 2 engine binary exists at `target/debug/reality-loop`.** Verify:
   ```bash
   test -x ./target/debug/reality-loop
   ./target/debug/reality-loop --help | grep -q "smoke"
   ```
5. **A smoke runtime root from Phase 2 or 3 exists.** Verify `cat ~/.cache/cgreality/active_runtime_root` returns a path that exists.

## Required reading

- [../03_MCP_REALITY_TOOLS_SPEC.md](../03_MCP_REALITY_TOOLS_SPEC.md) — full INTERACT/ALTER/shift specs.
- [../01b_REALITY_SHIFT_PRINCIPLE.md](../01b_REALITY_SHIFT_PRINCIPLE.md) — the discipline.
- [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md) — the recommendation schema (full JSON Schema).
- [../16_PARALLEL_AGENT_POLICY.md](../16_PARALLEL_AGENT_POLICY.md) — single-writer rule.

## What to learn from existing code

1. How the existing `memory_tools::store_memory` is structured. We re-export it under `cgreality` so the namespace works; the underlying behavior is unchanged.
2. How the existing tools handle `tool_use_id` — Phase 4 alter tools record this in shift-log records and harness-transition artifacts.
3. Whether `context-graph-mcp` already depends on `rusqlite` or `sqlx` for ledger queries. We need read-only SQL for `reality_query_ledger`. Re-use whatever's already there.

## Required tool dependencies (add to Cargo.toml if missing)

```toml
jsonschema = "0.18"          # for recommendation schema validation
fs2 = "0.4"                  # for advisory file locking (alternative to nix::fcntl::flock)
uuid = { version = "1", features = ["v4"] }  # for shift_id generation
```

## What to NOT do

- Do not let any alter tool succeed without writing a shift-log entry. The shift-log is the heart of the reality-shift principle.
- Do not let `harness_run_command` execute anything outside the allowlist. Even with `defaultMode: acceptEdits`, the MCP server's allowlist is the second line of defense.
- Do not skip the SHA check in `harness_apply_line_window_edit`. Stale edits MUST fail with `HARNESS_EDIT_SHA_MISMATCH`.
- Do not modify Phase 3's analyze tools. If you find a bug, fix it and re-verify Phase 3 — but don't change behavior in ways that affect Phase 4 in this phase.
- Do not write new tests in `reality-loop.rs`; this is the MCP crate.
