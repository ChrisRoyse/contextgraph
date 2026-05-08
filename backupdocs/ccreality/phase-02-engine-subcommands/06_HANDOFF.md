# Phase 2 — Handoff

## What's true after Phase 2

- `./target/debug/reality-loop` requires a subcommand: `attempt`, `smoke`, `replay`, or `verify`.
- The four subcommands compile and produce expected artifacts on disk.
- `~/.cache/cgreality/active_runtime_root` is written by `attempt` and `smoke` on start.
- All Phase 1 tests still pass.
- The MCP server (Phase 4) can shell out to any of the four subcommands.

## Completion Record

- Completed on 2026-05-05.
- Canonical verification log: `/tmp/phase-02-verify.log`, ending with `Phase 2 verification: ALL PASS`.
- Progress log source of truth: `/tmp/ccreality-phase-progress.log`, containing `phase-02: PASS`.
- Handoff smoke runtime root: `/tmp/cgreality-phase02-ready`.
- Handoff active-root readback: `~/.cache/cgreality/active_runtime_root=/tmp/cgreality-phase02-ready`; `~/.cache/cgreality/active_target_instance=cgreality-smoke-phase02-ready`.
- Manual artifact readbacks verified smoke writes, replay output, verify enumeration, and forced-failure artifact creation under `/tmp/cgreality-phase02-fail/failures/`.

## Side-effect: external scripts that invoked the binary

If any script (CI YAML, dev shell, dashboard, etc.) invoked the binary as `reality-loop --task-manifest=...`, it now needs `reality-loop attempt --task-manifest=...`. Task 2.7 grepped for these; double-check after Phase 2 if a script suddenly fails.

## Next phase

→ [phase-03-mcp-foundation-analyze/](../phase-03-mcp-foundation-analyze/README.md)

Phase 5, Phase 3, and Phase 4 are now complete. The current next phase is [phase-06-hooks/](../phase-06-hooks/README.md).
