# Phase 2 — Handoff

## What's true after Phase 2

- `./target/debug/reality-loop` requires a subcommand: `attempt`, `smoke`, `replay`, or `verify`.
- The four subcommands compile and produce expected artifacts on disk.
- `~/.cache/cgreality/active_runtime_root` is written by `attempt` and `smoke` on start.
- All Phase 1 tests still pass.
- The MCP server (Phase 4) can shell out to any of the four subcommands.

## Side-effect: external scripts that invoked the binary

If any script (CI YAML, dev shell, dashboard, etc.) invoked the binary as `reality-loop --task-manifest=...`, it now needs `reality-loop attempt --task-manifest=...`. Task 2.7 grepped for these; double-check after Phase 2 if a script suddenly fails.

## Next phase

→ [phase-03-mcp-foundation-analyze/](../phase-03-mcp-foundation-analyze/README.md)

Phase 3 is the largest single phase after Phase 1 (2–4 hours). It creates the new `cgreality` MCP server module under `crates/context-graph-mcp/src/handlers/tools/reality_loop/` and implements all the ANALYZE-family tools (reality_*, memory tool re-exports, audit trail). Phase 5 (config files) can run before, after, or in parallel with Phase 3.
