# Phase 9 — Handoff

## What's true after Phase 9

- `scripts/cc/tests/smoke.sh` exists and exits 0.
- All 8 layers pass.
- A `SMOKE_RESULTS_<date>.md` file exists in `docs/ccreality/`.
- Smoke artifacts are retained under `/tmp/cgreality-smoke-*` for inspection.
- The cutover is end-to-end functional in the smoke environment.
- Latest pass: `/tmp/cgreality-smoke-1778013189/smoke-report.json` with `status: "pass"`, `layer_count: 9`, and `edge_count: 6`.
- Latest ledger source of truth: `/tmp/cgreality-smoke-1778013189/layer5-skill-runtime/ledger.sqlite`, read back as `agent_run|1` and `tool_call|2`.

## What's still missing

- The full FSV suite (Phase 10) — A through F categories with edge cases.
- Documentation cleanup (Phase 11).
- A real production attempt (Phase 12).

## Verification record

```bash
echo "phase-09: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```

## Next phase

→ [phase-10-fsv/](../phase-10-fsv/README.md)
