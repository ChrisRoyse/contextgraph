# Next Actions

## Priority 1 - Improve Inner Model Repair Behavior

The production SWE-bench task was not solved. The harness correctly rejected invalid edits. The next optimization target is the inner Gemma prompt/tool behavior:

- Forbid byte-identical edit resubmission after a syntax probe failure.
- Require re-opening the exact file span after any `py_compile` failure.
- Require preserving open call delimiters when editing inside multiline expressions.
- Add a reward penalty for repeating a failed edit's argument hash.

Source evidence: `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/.../recommendation-turn-01.json`.

## Priority 2 - Keep Verifiers Aligned With Runtime Contracts

The hardening pass fixed stale expectations in the smoke and Phase 0-8 verifier. Future changes to tool surfaces must update:

- `scripts/cc/tests/smoke.sh`
- `scripts/cc/tests/verify_phase0_8_hardening.sh`
- `scripts/cc/tests/fsv/cases/*`
- `docs/report-analysis/04_AI_OPERATOR_INSTRUCTIONS.md`

## Priority 3 - Add CI Entry Points

Recommended CI commands:

```bash
cargo fmt --check
cargo test -p context-graph-cli --bin reality-loop -- --skip ignored --test-threads=1
cargo test -p context-graph-mcp --lib reality_loop
bash scripts/cc/tests/smoke.sh
bash scripts/cc/tests/fsv.sh
```

CI should upload `/tmp/cgreality-*` logs as artifacts.

## Priority 4 - Re-run A Real Production Attempt

After prompt/reward changes, run another real SWE-bench Lite attempt:

```bash
./target/release/reality-loop attempt \
  --task-manifest tasks/swebench-lite/psf__requests-2317.json \
  --runtime-root /tmp/ccreality-prod-2 \
  --context-graph-bin target/release/context-graph \
  --max-attempts 1 \
  --official-eval-each-attempt
```

Then run:

```bash
claude --print "/ccreality-iterate" \
  --model opus \
  --max-turns 25 \
  --output-format stream-json \
  --verbose \
  --include-partial-messages
```

Verify by reading the attempt summary, SQLite ledger, shift log, and recommendation artifact.

