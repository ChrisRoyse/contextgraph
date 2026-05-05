# Phase 1 — Recovery

Common failures during the excision and how to fix them.

## Failure: cargo check fails after Task 1.6 (constant deletion)

**Symptom:** `error[E0425]: cannot find value 'DEFAULT_OPENAI_OPTIMIZER_MODEL' in this scope` (or similar).

**Cause:** A constant deleted in Task 1.6 is still referenced somewhere we missed (e.g., a `default_value = DEFAULT_OPENAI_*` on a CLI flag we didn't delete, or a help text).

**Fix:** Search for all remaining references:
```bash
rg -n "DEFAULT_OPENAI|DEEPSEEK_V4_PRO|DEFAULT_OPTIMIZER" crates/context-graph-cli/src/bin/reality-loop.rs
```
Each hit is a leftover. Remove that flag/field/help text — it was dependent on the deleted constant. Cross-reference Task 1.7 to confirm it's a flag we should have deleted anyway.

## Failure: test_args() compile error after Task 1.7

**Symptom:** `error[E0560]: struct 'Args' has no field named 'openai_optimizer_each_attempt'` in `test_args`.

**Cause:** Task 1.9 wasn't done before re-running cargo. Re-run Task 1.9 — drop every assignment in `test_args` that mentions a deleted flag.

## Failure: Task 1.11 leaves orphaned variables in `run()`

**Symptom:** `error: unused variable 'optimizer_trigger_decision'` or similar.

**Cause:** Some `let` bindings in `run()` were used only by the deleted block. Delete those bindings.

Common ones:
- `let optimizer_trigger_decision = decide_deepseek_optimizer_trigger(...)`
- `let optimizer_trigger_decision_path = ...`
- `let optimizer_trigger_decision_readback: Value = ...`
- `let openai_optimizer_recommendation = if ... { ... } else { None };`
- `let openai_optimizer_recommendation_path = ...`
- `let openai_optimizer_recommendation_compact = ...`

All of these die with the trigger/optimizer block.

## Failure: cargo check passes but cargo test fails (Task 1.17)

**Symptom:** A surviving test references something we deleted.

**Diagnosis:**
```bash
cargo test -p context-graph-cli --bin reality-loop -- --nocapture --skip ignored 2>&1 | head -50
```

Look for `error[E0425]` or `cannot find function` or `not in scope`.

**Fix:** The test is one of:
1. A test for a function we deleted (e.g., `validate_optimizer_trigger_config` had no test, but a test of `decide_deepseek_optimizer_trigger` is in the deleted list — confirm we deleted it).
2. A test that references a deleted CLI flag in its `test_args` setup — update `test_args` again.
3. A test that references a constant we deleted — replace with a hardcoded value.

If the test is genuinely about provider-specific code, delete it (and add to the Task 1.16 list for documentation purposes).

## Failure: `governed_edit` lib tests fail (Task 1.5)

**Symptom:** A moved test fails at runtime (file not found, SHA mismatch, etc.).

**Diagnosis:** The lifted body references something only present in `reality-loop.rs`. Check imports at the top of `governed_edit.rs`:
- Does it `use crate::sha256_text;`? But `crate` here is the LIB, not the bin. The function may not be in the lib. Solution: either `pub use` it from `lib.rs`, or copy the helper into `governed_edit.rs` as a private function.
- Same for `write_text_checked`, `create_dir_checked`, `fs_error`, `path_str`, `file_sot`.

Fix: In `lib.rs`, add a `pub mod fs_helpers;` and move those helpers into `crates/context-graph-cli/src/fs_helpers.rs`. Then in `bin/reality-loop.rs`, replace the local definitions with `use context_graph_cli::fs_helpers::*;`. This keeps a single source of truth for the helpers.

## Failure: file size out of range (Task 1.18)

**Symptom:** `wc -l reality-loop.rs` returns a number outside 6,500–8,500.

**If too low (<6,500):** We over-deleted. Probably a function we needed got removed. Inspect:
```bash
git diff "$(cat /tmp/ccreality-cutover-base.sha)" -- crates/context-graph-cli/src/bin/reality-loop.rs | grep "^-fn " | head -50
```
This shows every function deleted. Cross-reference with the KEEP list in `00_SPEC.md`. If a KEEP function was deleted, restore it from git:
```bash
git show "$(cat /tmp/ccreality-cutover-base.sha)":crates/context-graph-cli/src/bin/reality-loop.rs | sed -n 'LINE_START,LINE_ENDp' >> /tmp/restore.txt
# Then manually paste back into reality-loop.rs
```

**If too high (>8,500):** We under-deleted. Some openai/deepseek code is still there. Re-grep:
```bash
rg -n "openai|deepseek|optimizer_trigger|optimizer_recommendation" crates/context-graph-cli/src/bin/reality-loop.rs
```

## Failure: cargo build hangs

**Symptom:** `cargo build` runs >60s without progress.

**Cause:** Likely a recursive type or trait bound issue introduced by partial edits.

**Fix:**
```bash
# Cancel the build (Ctrl-C), then check syntax with a fast subset:
cargo check -p context-graph-cli --bin reality-loop --message-format short
```

If this also hangs, restore from base and re-do the failing task more carefully:
```bash
git restore crates/context-graph-cli/src/bin/reality-loop.rs
# then re-execute tasks from the last known-good point
```

## Hard reset (last resort)

If the file is in an unrecoverable state:
```bash
git restore crates/context-graph-cli/src/bin/reality-loop.rs
git restore crates/context-graph-cli/src/governed_edit.rs 2>/dev/null || rm -f crates/context-graph-cli/src/governed_edit.rs
git restore crates/context-graph-cli/src/lib.rs 2>/dev/null
git restore crates/context-graph-cli/Cargo.toml
```

This restores the file to whatever state it was in at the last commit on `ccreality-cutover` (which, if no commits yet on this branch, is the base SHA from Phase 0).

Re-execute the tasks more carefully. The file specs in `03_FILE_SPECS.md` are the authoritative target.

## Diagnosing "I don't know what's wrong"

```bash
cd /home/cabdru/contextgraph
echo "Build:"; cargo check -p context-graph-cli --bin reality-loop 2>&1 | head -20
echo "Test:"; cargo test -p context-graph-cli --bin reality-loop --quiet 2>&1 | head -20
echo "Lib build:"; cargo check -p context-graph-cli --lib 2>&1 | head -20
echo "Lib test:"; cargo test -p context-graph-cli --lib --quiet 2>&1 | head -20
echo "Size:"; wc -l crates/context-graph-cli/src/bin/reality-loop.rs crates/context-graph-cli/src/governed_edit.rs 2>/dev/null
echo "Refs:"; rg -c "openai|deepseek|gpt-5|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs
```

This dumps the current state. Compare each line to the expected post-Phase-1 state in `04_VERIFICATION.md`. The differences point at the unfinished task.
