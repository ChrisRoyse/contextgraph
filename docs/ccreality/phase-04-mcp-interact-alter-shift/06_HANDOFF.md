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

After Phase 4 verification passes, append to `/tmp/ccreality-phase-progress.log`:
```
phase-04: PASS, <ISO date>, MCP tool count=<N>
```

## Next phase

→ [phase-05-config-files/](../phase-05-config-files/README.md)

Phase 5 creates the two JSON config files referenced by Phases 3 and 4. It can be done before, alongside, or after Phase 4 — but it MUST be done before any tool call actually executes (Phase 4's verification calls path_policy and schema_validator, both of which need the config files).

If you executed Phase 5 already (recommended order: 0 → 1 → 5 → 2 → 3 → 4), then proceed to Phase 6 (hooks).

If you skipped Phase 5, do it next.
