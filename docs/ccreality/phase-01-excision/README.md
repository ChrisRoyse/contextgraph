# Phase 1 — Excision (DeepSeek/OpenAI Removal + Governed-Edit Module Lift)

**Estimated duration:** 1–2 hours.
**Status:** pending.
**Depends on:** Phase 0.
**Unblocks:** Phase 2.
**Lines of code touched:** ~3,400 deleted, ~1,000 moved.

## What this phase does

Two things, in this order:

1. **Lift** the provider-neutral governed-edit helpers (`project_file_window`, `project_apply_line_window_edits`, `run_openai_project_command`, `verify_openai_project_state`, `validate_optimizer_project_path`) out of `reality-loop.rs` and into a new module `crates/context-graph-cli/src/governed_edit.rs`. Adapt the tests with them.

2. **Delete** every DeepSeek/OpenAI/gpt-5/chatgpt code path: 16 constants, 17 CLI flags, 2 `AttemptRecord` fields, 32 functions, 11 tests, the entire optimizer-trigger + run_openai_loop_optimizer block in `run()`, and 11 keys from the failure-artifact JSON.

After this phase: `reality-loop.rs` is ~7,500 lines (down from 10,873), no provider-specific HTTP code, the engine is provider-agnostic.

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph

# 1. No DeepSeek/OpenAI text in the engine
! rg -nq "deepseek|openai|gpt-5|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs

# 2. New governed-edit module exists with the lifted helpers
test -f crates/context-graph-cli/src/governed_edit.rs \
  && rg -q "pub fn project_file_window" crates/context-graph-cli/src/governed_edit.rs \
  && rg -q "pub fn project_apply_line_window_edits" crates/context-graph-cli/src/governed_edit.rs \
  && rg -q "pub fn validate_governed_path" crates/context-graph-cli/src/governed_edit.rs

# 3. Engine builds + tests pass
cargo build -p context-graph-cli --bin reality-loop --quiet \
  && cargo test -p context-graph-cli --bin reality-loop --quiet -- --skip ignored

# 4. Reality-loop.rs is in expected size range (regression guard)
LC=$(wc -l < crates/context-graph-cli/src/bin/reality-loop.rs)
[ "$LC" -ge 6500 ] && [ "$LC" -le 8500 ]
```

All four blocks must exit 0.

## Files

- [00_SPEC.md](00_SPEC.md) — objectives, deliverables, end-state.
- [01_PRECONDITIONS.md](01_PRECONDITIONS.md) — must read before starting.
- [02_TASKS.md](02_TASKS.md) — ordered atomic tasks (the long one).
- [03_FILE_SPECS.md](03_FILE_SPECS.md) — content of new `governed_edit.rs` + final shape of edited files.
- [04_VERIFICATION.md](04_VERIFICATION.md) — full verification commands.
- [05_RECOVERY.md](05_RECOVERY.md) — what to do when cargo errors appear.
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after; pointer to Phase 2.

## Reference docs

- [../04_EXCISION_PLAN.md](../04_EXCISION_PLAN.md) — the full audit-driven excision map (line ranges, function lists).
- [../05_REALITY_ENGINE_RESPONSIBILITY.md](../05_REALITY_ENGINE_RESPONSIBILITY.md) — what the engine should look like after.
