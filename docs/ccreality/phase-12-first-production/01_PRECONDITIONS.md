# Phase 12 — Preconditions

## Required state

1. **All previous phases verified.**
   ```bash
   wc -l /tmp/ccreality-phase-progress.log
   ```
   Should show ≥12 PASS lines (phase-00 through phase-11).

2. **Smoke and FSV both passed recently.**
   - `bash scripts/cc/tests/smoke.sh` returns 0.
   - The most recent `docs/ccreality/FSV_RESULTS_*.md` shows `FAIL: 0`.

3. **Real Anthropic API access.** Either via Claude.ai login or `ANTHROPIC_API_KEY`. Verify:
   ```bash
   claude --print "Reply with one word: ok" --output-format json | jq -r '.result' | head -c 10
   ```
   Should return `ok` or similar.

4. **Docker available for SWE-bench official eval.** Verify:
   ```bash
   docker version | head -3
   ```
   If Docker isn't running, start it. SWE-bench Lite eval pulls images of significant size (~1 GB).

5. **Ollama running with gemma4:e4b loaded.** Verify:
   ```bash
   ollama list | grep gemma4
   ```

6. **A SWE-bench Lite task manifest selected.** Either pick from `tasks/swebench-lite/` if it exists, or create one. See Task 12.1.

## Forbidden actions

- No `git push` until commit is reviewed.
- No `git rebase` on `ccreality-cutover` (the commit will be one big one; we keep it that way for atomicity).
- No partial revert of the working tree (atomic — either commit all of Phases 1–11, or nothing).

## Recommended SWE-bench instance for first light

Pick an instance that:
- Has a small `fail_to_pass` (1–2 tests).
- Requires a small patch (≤30 LOC change).
- Is in a project with fast Docker startup.

Examples that have historically been first-light friendly: `pylint-dev__pylint-*` issues with single-function fixes, `astropy__astropy-*` with isolated unit additions.

The operator's choice. The cutover doesn't require Gemma to actually solve the instance — just that the engine produces an attempt artifact and the optimizer produces a recommendation.
