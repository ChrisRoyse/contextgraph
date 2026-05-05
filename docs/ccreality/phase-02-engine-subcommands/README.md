# Phase 2 — Engine Subcommands

**Estimated duration:** 30–45 minutes.
**Status:** next.
**Depends on:** Phase 1 (complete).
**Unblocks:** Phase 3 smoke/analyze wiring. Phase 5 can run independently after Phase 1 and must be complete before Phase 3.

## What this phase does

Refactors `crates/context-graph-cli/src/bin/reality-loop.rs` so the binary takes a subcommand:

```
reality-loop attempt   --task-manifest=… --runtime-root=… [...]
reality-loop smoke     --runtime-root=… [--label=…]
reality-loop replay    --attempt-summary-path=…
reality-loop verify    --runtime-root=… --attempt=N
```

After this phase, the MCP server's `reality_run_attempt`, `reality_run_smoke`, `reality_replay_artifact` tools (Phase 4) can shell out predictably.

This phase also adds the `~/.cache/cgreality/active_runtime_root` and `~/.cache/cgreality/active_target_instance` writers to the `attempt` and `smoke` paths.

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph
cargo build -p context-graph-cli --bin reality-loop --quiet
./target/debug/context-graph-cli-reality-loop --help 2>&1 | grep -E "attempt|smoke|replay|verify" | wc -l
```

Wait — the binary is named `reality-loop` not `context-graph-cli-reality-loop`. Verify with the actual invocation:

```bash
./target/debug/reality-loop --help 2>&1 | grep -cE "attempt|smoke|replay|verify"
```

Should be ≥ 4.

```bash
./target/debug/reality-loop smoke --runtime-root /tmp/cgreality-phase02-smoke --label phase02-test
test -f /tmp/cgreality-phase02-smoke/run-*/smoke-attempt-1/attempt-summary.json
test -f ~/.cache/cgreality/active_runtime_root
```

All three must succeed.

## Files

- [00_SPEC.md](00_SPEC.md) — scope, deliverables, end-state.
- [02_TASKS.md](02_TASKS.md) — the 8 ordered tasks.
- [03_FILE_SPECS.md](03_FILE_SPECS.md) — exact subcommand signatures + active-root writer.
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification block.
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after.

(No PRECONDITIONS doc; the precondition is the completed Phase 1 state recorded in `../phase-01-excision/06_HANDOFF.md`. No RECOVERY doc; failures are simple cargo errors with obvious fixes.)

## Reference docs

- [../05_REALITY_ENGINE_RESPONSIBILITY.md](../05_REALITY_ENGINE_RESPONSIBILITY.md) — the desired engine shape (subcommands and what each does).
