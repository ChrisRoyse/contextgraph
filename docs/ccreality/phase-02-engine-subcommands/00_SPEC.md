# Phase 2 — Specification

## Objectives

1. Wrap the existing `Args` (post-Phase-1) into an `attempt` subcommand.
2. Add three new subcommands: `smoke`, `replay`, `verify`.
3. Add an active-root writer that updates `~/.cache/cgreality/active_runtime_root` and `~/.cache/cgreality/active_target_instance` on `attempt`/`smoke` start.

## Scope

- `crates/context-graph-cli/src/bin/reality-loop.rs` — primary edit target (refactor `main` and `Args`).
- `crates/context-graph-cli/Cargo.toml` — no change expected.

Out of scope:
- New domain logic.
- Config-file consumption (Phase 5).
- MCP integration (Phases 3, 4).

## Deliverables

| Subcommand | Wraps | New behavior |
|------------|-------|--------------|
| `attempt` | existing `run()` (post-Phase-1) | Same args; writes active-root file on start |
| `smoke` | NEW: synthetic deterministic attempt | Writes a fake `attempt-summary.json` tree; no Gemma, no Docker |
| `replay` | NEW: re-emit reality-signal-packet | Reads attempt-summary.json from disk; rebuilds and rewrites reality-signal-packet.json |
| `verify` | NEW: full-state verification | Re-reads every artifact under `<runtime>/<task>/attempt-N/`, recomputes SHAs, reports any drift |

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). Summary:
- `cargo build` succeeds.
- `--help` shows all four subcommands.
- `smoke` produces expected artifacts under a temp runtime root.
- `~/.cache/cgreality/active_runtime_root` is written by `smoke`.

## Time budget

30–45 minutes. Mostly clap restructuring; the smoke/replay/verify bodies can be skeletal at this phase (just enough to verify the dispatch works); they're filled out in the FSV plan if needed.

## Backward compatibility note

After this phase, the old invocation `./target/debug/reality-loop --task-manifest=... --runtime-root=...` (with no subcommand) will FAIL because `attempt` is now mandatory. Per the project rule (no backwards compat), this is fine — every caller updates to `./target/debug/reality-loop attempt --task-manifest=... --runtime-root=...`.

If any existing scripts in the repo invoke the old shape, Phase 2 must update them. Check:
```bash
rg -n "context-graph-cli.*reality-loop|target/.*reality-loop" --type sh --type bash --type-add 'tomlconfig:Cargo.toml' --type tomlconfig
```
Update each callsite to use `attempt` subcommand.
