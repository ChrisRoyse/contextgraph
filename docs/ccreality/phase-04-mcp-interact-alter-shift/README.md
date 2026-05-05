# Phase 4 — MCP INTERACT + ALTER + Shift Log

**Estimated duration:** 3–5 hours.
**Status:** in progress.
**Depends on:** Phase 3 (foundation) + Phase 5 (recommendation schema).
**Unblocks:** Phase 6.
**New code:** ~2,500–4,000 lines under `crates/context-graph-mcp/src/handlers/tools/reality_loop/`.

## What this phase does

Implements the **write half** of the cgreality MCP surface. Three sub-families:

### INTERACT — shell out to the engine
- `reality_run_attempt` — `reality-loop attempt` subprocess.
- `reality_run_smoke` — `reality-loop smoke` subprocess.
- `reality_run_official_eval` — re-runs official Docker eval against a stored attempt.
- `reality_run_targeted_test` — runs one fail_to_pass test.
- `reality_recompile_reward` — re-runs reward computation.
- `reality_query_ledger` — read-only SQL against the BlackBoxFix SQLite.

### ALTER — mutate the harness or the memory store
- `harness_open_window`, `harness_apply_line_window_edit` — uses the lifted `governed_edit` module from Phase 1.
- `harness_run_command` — allowlisted command runner.
- `harness_git_diff`, `harness_git_status`, `harness_verify_state`.
- Memory mutators re-exported with `namespace: "ccreality"` injected: `store_memory`, `boost_importance`, `forget_concept`, `merge_concepts`.
- `optimizer_record_decision`, `optimizer_record_recommendation`, `optimizer_record_harness_transition`.

### SHIFT — perceive the deltas
- `reality_shift_log` — read the per-session JSONL append log.
- `reality_shift_compare_to_my_view` — drift detection.

Every alter call follows the **mutate → fsync → re-read → diff → log shift → return** discipline.

## Current State

- Working-tree module files exist for `alter`, `interact`, `shift_log`, `optimizer`, `lock`, and `schema_validator`.
- `cargo build -p context-graph-mcp --quiet` passes.
- `cargo test -p context-graph-mcp --lib reality_loop --quiet` passes.
- A bounded MCP `tools/list` probe read back 173 total tools and 32 cgreality reality/alter/shift/memory tools.
- Phase 4 is not complete yet: `/tmp/ccreality-phase-progress.log` does not contain `phase-04: PASS`, and the full [04_VERIFICATION.md](04_VERIFICATION.md) block still needs to be run end-to-end.

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph

# 1. MCP server still builds
cargo build -p context-graph-mcp --quiet

# 2. New module files exist
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/alter.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/optimizer.rs

# 3. tools/list includes ≥30 tools (11 from Phase 3 + ≥19 new ones from Phase 4)
TOOLS_COUNT=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop \
  | jq -r '.result.tools[].name' | wc -l)
[ "$TOOLS_COUNT" -ge 30 ]

# 4. End-to-end: open window, apply edit, verify shift logged
# (See 04_VERIFICATION.md for the full chain)
```

## Files

- [00_SPEC.md](00_SPEC.md) — scope, deliverables.
- [01_PRECONDITIONS.md](01_PRECONDITIONS.md) — what must be true.
- [02_TASKS.md](02_TASKS.md) — atomic tasks (long).
- [03_FILE_SPECS.md](03_FILE_SPECS.md) — module bodies.
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification block.
- [05_RECOVERY.md](05_RECOVERY.md) — common failures.
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after.

## Reference docs

- [../03_MCP_REALITY_TOOLS_SPEC.md](../03_MCP_REALITY_TOOLS_SPEC.md) — INTERACT, ALTER, shift-log spec.
- [../01b_REALITY_SHIFT_PRINCIPLE.md](../01b_REALITY_SHIFT_PRINCIPLE.md) — the discipline.
- [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md) — the recommendation schema.
- [../16_PARALLEL_AGENT_POLICY.md](../16_PARALLEL_AGENT_POLICY.md) — single-writer rule (enforced here via edit lock).
