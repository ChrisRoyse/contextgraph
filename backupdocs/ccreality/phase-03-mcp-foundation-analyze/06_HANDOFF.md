# Phase 3 — Handoff

## What's true after Phase 3

- `crates/context-graph-mcp/src/handlers/tools/reality_loop/` exists with 5 module files.
- `cargo build -p context-graph-mcp` succeeds.
- `--mode reality-loop` registers 11 read-only `reality_*` tools.
- `path_policy` loads from `$CGREALITY_GOVERNED_PATHS` and fails closed if the policy file is missing or malformed.
- The MCP server's stdout is JSON-RPC only; logging goes to stderr.
- Memory/graph/causal tools (existing) are still available; the new mode adds tools without removing any.
- A smoke attempt's artifacts can be queried end-to-end via the MCP tools.

## Completion Record

- Completed on 2026-05-05.
- Canonical verification log: `/tmp/phase-03-verify.log`, ending with `Phase 3 verification: ALL PASS`.
- Progress log source of truth: `/tmp/ccreality-phase-progress.log`, containing `phase-03: PASS`.
- MCP readback source of truth: `./target/debug/context-graph-mcp --mode reality-loop` returned a parseable JSON-RPC `tools/list` response and live tool-call payloads for `reality_latest_root`, `reality_attempt_summary`, `reality_problem_packet`, and `reality_replay_artifact`.

## Later phase status

Phase 4 has since completed the items that were intentionally missing at the Phase 3 handoff:

- **ALTER tools:** `harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_git_diff`, `harness_git_status`, `harness_verify_state`, `optimizer_record_decision`, `optimizer_record_recommendation`, `optimizer_record_harness_transition`.
- **INTERACT tools:** `reality_run_attempt`, `reality_run_smoke`, `reality_run_official_eval`, `reality_run_targeted_test`, `reality_recompile_reward`, `reality_query_ledger`.
- **Shift-log tools:** `reality_shift_log`, `reality_shift_compare_to_my_view`.
- **Memory mutation tools re-exposed under cgreality:** `store_memory`, `boost_importance`, `forget_concept`, `merge_concepts`.

The current branch is no longer read-only at the MCP layer; Phase 4 added the governed write half and verified it with FSV.

## Next phase

→ [phase-04-mcp-interact-alter-shift/](../phase-04-mcp-interact-alter-shift/README.md)

Phase 4 is now complete. The current next phase is [phase-06-hooks/](../phase-06-hooks/README.md).
