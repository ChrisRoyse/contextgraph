# Phase 3 — Handoff

## What's true after Phase 3

- `crates/context-graph-mcp/src/handlers/tools/reality_loop/` exists with 5 module files.
- `cargo build -p context-graph-mcp` succeeds.
- `--mode reality-loop` registers 11 read-only `reality_*` tools.
- `path_policy` loads from `$CGREALITY_GOVERNED_PATHS` or falls back to a conservative deny list.
- The MCP server's stdout is JSON-RPC only; logging goes to stderr.
- Memory/graph/causal tools (existing) are still available; the new mode adds tools without removing any.
- A smoke attempt's artifacts can be queried end-to-end via the MCP tools.

## What's still missing

- **All ALTER tools** (Phase 4): `harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_git_diff`, `harness_git_status`, `harness_verify_state`, `optimizer_record_decision`, `optimizer_record_recommendation`, `optimizer_record_harness_transition`.
- **All INTERACT tools** (Phase 4): `reality_run_attempt`, `reality_run_smoke`, `reality_run_official_eval`, `reality_run_targeted_test`, `reality_recompile_reward`, `reality_query_ledger`.
- **Shift-log tools** (Phase 4): `reality_shift_log`, `reality_shift_compare_to_my_view`.
- **Memory mutation tools re-exposed under cgreality** (Phase 4): `store_memory`, `boost_importance`, `forget_concept`, `merge_concepts`.

A Claude Code session that uses Phase 3's tool surface only can READ reality but cannot ACT. That's exactly the intended state — we test the read half before adding the write half.

## Next phase

→ [phase-04-mcp-interact-alter-shift/](../phase-04-mcp-interact-alter-shift/README.md)

Phase 4 is the largest MCP-side phase. It implements:
- All ALTER tools (with the `mutate → fsync → re-read → diff → log shift → return` discipline).
- All INTERACT tools (which shell out to the engine binary from Phase 2).
- The shift-log and drift-detection tools.
- The recommendation schema validator.

Estimated 3–5 hours.
