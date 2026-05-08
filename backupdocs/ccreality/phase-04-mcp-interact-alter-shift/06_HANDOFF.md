# Phase 4 — Handoff

## What's true after Phase 4

- The cgreality MCP server exposes ≥30 tools across ANALYZE, INTERACT, ALTER, SHIFT families.
- Every alter call produces a shift-log record; the shift-log is queryable via `reality_shift_log`.
- Drift detection works via `reality_shift_compare_to_my_view`.
- Single-writer enforcement via `~/.cache/cgreality/edit_lock`.
- Recommendation schema validation via `optimizer_record_recommendation`.
- Path policy enforcement via `path_policy.rs` (loaded from `config/ccreality_governed_paths.json`).
- INTERACT tools shell out to `target/debug/reality-loop` cleanly.
- Memory mutators (`store_memory`, `boost_importance`, `forget_concept`, `merge_concepts`) re-exposed under `cgreality` with namespace injection.

## What's still missing

- **Hooks** (Phase 6) — the bash scripts that inject reality and enforce the Stop gate.
- **`.claude/` scaffolding** (Phase 7) — settings.json, .mcp.json, loop.md, agents, skills.
- **CLAUDE.md addendum** (Phase 8) — project memory.
- **Smoke and FSV** (Phases 9, 10).

A Claude Code session running `claude --mode default` (no .claude/settings.json yet) could already call the cgreality MCP tools after Phase 4 completes, but without the hooks and skills, the operator would have to manually invoke each tool.

## Verification record

- Completed on 2026-05-05.
- `cargo build -p context-graph-mcp --quiet` passes.
- `cargo test -p context-graph-mcp --lib reality_loop --quiet` passes.
- `cargo test -p context-graph-mcp --lib handlers::tests::mcp_protocol_e2e_test::test_e2e_tools_list_all_tools --quiet` passes.
- Progress log source of truth: `/tmp/ccreality-phase-progress.log`, containing `phase-04: PASS`.
- Manual FSV source of truth: `/tmp/ccreality-fsv-1777999742.log`.
- Ledger FSV source of truth: `/tmp/ccreality-phase4-ledger-fsv-1778000024.log`.
- Physical ledger readback: `/tmp/cgreality-phase04-ledger-fsv-1778000024/ledger.sqlite` contains `agent_run|1` and `tool_call|2`; the tool-call rows are `optimizer_record_decision` and `optimizer_record_recommendation`.
- Physical shift-log readback: `/tmp/cgreality-phase04-ledger-fsv-1778000024/cgreality-shift-log/phase4-ledger-fsv-1778000024.jsonl` contains 4 shift records.
- Edge cases manually verified with before/after source-of-truth reads: denied `.env` path, stale edit SHA, rejected SQL injection WHERE clause.

## Next phase

→ [phase-06-hooks/](../phase-06-hooks/README.md)

Phase 5 is already complete, and Phase 4 now has the completed source-of-truth verification record listed above. Phase 6 may start. Do not bypass the Phase 4 MCP tools from hooks; the hooks depend on `optimizer_record_harness_transition`, shift-log readback, path policy, and schema validation.
