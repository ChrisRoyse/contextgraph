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

5. **the inner actor GGUF model file present for the Rust coding harness.** The default
   path baked into the reality-loop binary is `models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf`
   (BF16, ~15 GB on disk, sha256 pin in `config/ccreality_model_pins.json`). Verify:
   ```bash
   test -f models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf \
     && jq -r '.models.qwen3_6_27b_q6_k_xl.expected_sha256' \
        config/ccreality_model_pins.json
   ```
   The cryptographic-pin gate refuses to load any GGUF whose size or sha256
   deviates from the pin (`RUST_REALITY_INNER_MODEL_SIZE_MISMATCH` or
   `RUST_REALITY_INNER_MODEL_SHA_MISMATCH`).

6. **A SWE-bench Lite task manifest selected.** Either pick from `tasks/swebench-lite/` if it exists, or create one. See Task 12.1.

## Forbidden actions

- No `git push` until commit is reviewed.
- No `git rebase` on `ccreality-cutover` during Phase 12. If a squash is desired, decide that explicitly during review after Phase 12 passes.
- No partial revert of verified earlier phases. Only revert failed Phase 12 edits/artifacts if this phase fails.

## Recommended SWE-bench instance for first light

Pick an instance that:
- Has a small `fail_to_pass` (1–2 tests).
- Requires a small patch (≤30 LOC change).
- Is in a project with fast Docker startup.

Examples that have historically been first-light friendly: `pylint-dev__pylint-*` issues with single-function fixes, `astropy__astropy-*` with isolated unit additions.

The operator's choice. The cutover doesn't require the inner actor to actually solve the instance — just that the engine produces an attempt artifact and the optimizer produces a recommendation.
