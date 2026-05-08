# Phase 9 — Smoke

**Estimated duration:** 1–2 hours.
**Status:** complete.
**Depends on:** Phases 0–8.
**Unblocks:** Phase 10.

## What this phase does

Builds and runs the 8-layer smoke test from [../17_SMOKE_TEST_PLAN.md](../17_SMOKE_TEST_PLAN.md). The smoke is a single bash script (`scripts/cc/tests/smoke.sh`) that exercises every layer of the cutover end-to-end without requiring a real SWE-bench instance or Docker.

## AI-verifiable end-state

```bash
bash /home/cabdru/contextgraph/scripts/cc/tests/smoke.sh
echo "exit=$?"
```

Expected: `exit=0`. The script's last line should be `Smoke pass.`. Latest verified source of truth: `/tmp/cgreality-smoke-1778013189/smoke-report.json` and [../SMOKE_RESULTS_2026-05-05.md](../SMOKE_RESULTS_2026-05-05.md).

## Files

- [00_SPEC.md](00_SPEC.md)
- [02_TASKS.md](02_TASKS.md)
- [03_FILE_SPECS.md](03_FILE_SPECS.md)
- [04_VERIFICATION.md](04_VERIFICATION.md)
- [05_RECOVERY.md](05_RECOVERY.md)
- [06_HANDOFF.md](06_HANDOFF.md)

## Reference docs

- [../17_SMOKE_TEST_PLAN.md](../17_SMOKE_TEST_PLAN.md) — the canonical 8-layer test (Layers 0–8).
