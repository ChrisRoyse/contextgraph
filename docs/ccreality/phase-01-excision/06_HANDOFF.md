# Phase 1 — Handoff

## What's true after Phase 1

- `crates/context-graph-cli/src/bin/reality-loop.rs` contains no DeepSeek/OpenAI/gpt-5/chatgpt text.
- `crates/context-graph-cli/src/governed_edit.rs` exists with five public functions: `project_file_window`, `project_apply_line_window_edits`, `validate_governed_path`, `run_governed_command`, `verify_project_state`.
- `crates/context-graph-cli/src/lib.rs` exists and declares `pub mod governed_edit;`.
- `crates/context-graph-cli/Cargo.toml` has a `[lib]` section.
- `cargo build -p context-graph-cli --bin reality-loop` succeeds.
- `cargo test -p context-graph-cli --bin reality-loop` and `cargo test -p context-graph-cli --lib governed_edit` both pass.
- The engine binary still works exactly as before for Gemma + SWE-bench paths; only the outer optimizer is gone.
- No commit has been made yet (atomicity rule — single commit at end of cutover).

## What's now MISSING from the system

- The outer optimizer is gone but no replacement is wired up yet. **Running a real attempt will succeed but no one will react to its failure.** This is intentional — Phase 12 wires the new optimizer (Claude Code via MCP) into a working state.
- Between Phase 1 and Phase 12, the engine is "headless" — it produces reality but nothing acts on reality. Gemma's inner attempts still run; they just don't get an outer-loop improvement after each failure.

## What to verify before moving to Phase 2

```bash
bash -c "$(cat docs/ccreality/phase-01-excision/04_VERIFICATION.md | sed -n '/^```bash$/,/^```$/p' | grep -v '^```')"
echo "Phase 1 verification exit=$?"
```

Should print `Phase 1 verification exit=0`.

## Optional: snapshot to recovery point

After Phase 1 verification passes, you can record an intermediate point:
```bash
git rev-parse HEAD > /tmp/ccreality-after-phase-01.sha  # only meaningful if you committed; we haven't
# Better: stash the work to a safe reference
git stash push -m "ccreality-phase-01-complete" --keep-index
git stash list | head -1
# Then immediately re-apply
git stash pop
```

Or just trust the working-tree state and proceed; Phase 2's first task verifies Phase 1's output is intact.

## Next phase

→ [phase-02-engine-subcommands/](../phase-02-engine-subcommands/README.md)

Phase 2 reframes the engine binary into four subcommands (`attempt`, `smoke`, `replay`, `verify`) so the MCP server can shell out cleanly. It's a smaller phase (~30 minutes) — clap refactor, dispatch logic, no new domain code.
