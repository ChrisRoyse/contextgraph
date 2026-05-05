# Phase 3 — MCP Foundation + ANALYZE Tool Family

**Estimated duration:** 2–4 hours.
**Status:** complete.
**Depends on:** Phase 1 (lifted module), Phase 2 (engine subcommands/smoke contract), and Phase 5 (config files).
**Unblocks:** Phase 4.
**New code:** ~1,500–2,500 lines of Rust under `crates/context-graph-mcp/src/handlers/tools/reality_loop/`.

## What this phase does

1. Creates the `reality_loop` MCP module skeleton inside `context-graph-mcp`.
2. Wires it into the existing MCP server with a `--mode reality-loop` flag (or equivalent — see Q2 in [../20_OPEN_QUESTIONS.md](../20_OPEN_QUESTIONS.md)).
3. Implements the ANALYZE-family MCP tools:
   - `reality_latest_root`, `reality_attempt_summary`, `reality_official_report`, `reality_problem_packet`, `reality_signal`, `reality_failure`, `reality_trigger_decision`, `reality_harness_transitions`, `reality_compare_attempts`, `reality_audit_trail`, `reality_replay_artifact`.
4. Re-exports the existing memory/graph/causal tools under the same `cgreality` namespace.

This is the read half of the MCP surface. Phase 4 adds INTERACT and ALTER (the write half).

Current state: Phase 4 has since added and verified the write half. This Phase 3 README remains the audit record for the read-side foundation.

## Completion Record

- `cargo build -p context-graph-mcp` passes.
- `cargo test -p context-graph-mcp --lib reality_loop::path_policy --quiet` passes.
- `tools/list` over `./target/debug/context-graph-mcp --mode reality-loop` read back the 11 Phase 3 ANALYZE `reality_*` tools.
- Live MCP calls read the active smoke runtime root and attempt artifacts with `source_of_truth` fields.
- Canonical verification log: `/tmp/phase-03-verify.log`, ending with `Phase 3 verification: ALL PASS`.
- Progress log source of truth: `/tmp/ccreality-phase-progress.log`, containing `phase-03: PASS`.

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph
# 1. MCP server builds
cargo build -p context-graph-mcp --quiet

# 2. The reality_loop module exists with the expected tool registrations
test -d crates/context-graph-mcp/src/handlers/tools/reality_loop
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/mod.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/analyze.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/errors.rs

# 3. tools/list returns at least the expected analyze tool count
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop \
  | jq -r '.result.tools[].name' \
  | grep -cE '^reality_(latest_root|attempt_summary|official_report|problem_packet|signal|failure|trigger_decision|harness_transitions|compare_attempts|audit_trail|replay_artifact)$' \
  | grep -q '^11$'

# 4. A live tool call returns a properly-shaped payload
# (uses the smoke runtime root from Phase 2)
RES=$(echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"reality_latest_root","arguments":{}}}' \
       | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop)
echo "$RES" | jq -e '.result.content[0].text | fromjson | .runtime_root' >/dev/null
```

All four blocks must succeed.

## Files

- [00_SPEC.md](00_SPEC.md) — scope, deliverables, end-state.
- [01_PRECONDITIONS.md](01_PRECONDITIONS.md) — what must be true to start.
- [02_TASKS.md](02_TASKS.md) — atomic tasks.
- [03_FILE_SPECS.md](03_FILE_SPECS.md) — exact module structure + tool implementations.
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification block.
- [05_RECOVERY.md](05_RECOVERY.md) — common failures (rmcp logging-on-stdout corruption, schema mismatches).
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after.

## Reference docs

- [../03_MCP_REALITY_TOOLS_SPEC.md](../03_MCP_REALITY_TOOLS_SPEC.md) — the full tool specification (ANALYZE section + INTERACT section + ALTER section). Phase 3 implements the ANALYZE half.
- [../05_REALITY_ENGINE_RESPONSIBILITY.md](../05_REALITY_ENGINE_RESPONSIBILITY.md) — what the engine produces (the artifacts the ANALYZE tools read).
