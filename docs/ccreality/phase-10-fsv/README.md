# Phase 10 — Full State Verification (FSV)

**Estimated duration:** 2–4 hours.
**Status:** complete.
**Depends on:** Phase 9.
**Unblocks:** Phase 11.

## What this phase does

Builds the FSV harness from [../18_FSV_PLAN.md](../18_FSV_PLAN.md) — a multi-script suite that exercises every meaningful failure mode across six categories: A (engine artifacts), B (MCP analyze), C (MCP alter + shift), D (hooks), E (drip + drift end-to-end), F (cost and limits).

Each category has 5–12 cases. All A–E cases must pass; F is within tolerance.

## AI-verifiable end-state

```bash
bash /home/cabdru/contextgraph/scripts/cc/tests/fsv.sh
echo "exit=$?"
test -f /home/cabdru/contextgraph/docs/ccreality/FSV_RESULTS_$(date -u +%Y-%m-%d).md
```

Both must succeed.

## Files

- [00_SPEC.md](00_SPEC.md), [02_TASKS.md](02_TASKS.md), [03_FILE_SPECS.md](03_FILE_SPECS.md), [04_VERIFICATION.md](04_VERIFICATION.md), [05_RECOVERY.md](05_RECOVERY.md), [06_HANDOFF.md](06_HANDOFF.md).

## Reference docs

- [../18_FSV_PLAN.md](../18_FSV_PLAN.md) — full case list (A1–A5, B1–B6, C1–C12, D1–D10, E1–E3, F1–F3).
