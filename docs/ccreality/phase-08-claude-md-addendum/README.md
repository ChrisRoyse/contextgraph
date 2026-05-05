# Phase 8 — CLAUDE.md Addendum

**Estimated duration:** 5 minutes.
**Status:** pending.
**Depends on:** Phases 0–7.
**Unblocks:** Phase 9.

## What this phase does

Appends the ccreality role addendum to `/home/cabdru/contextgraph/CLAUDE.md`. The addendum is the project-memory contract that tells every Claude Code session in this project how to behave when running as the outer reality-loop optimizer.

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph
grep -q "ccreality — Outer Reality-Loop Optimizer Role" CLAUDE.md
grep -q "Reality-shift principle" CLAUDE.md
grep -q "mcp__cgreality__optimizer_record_recommendation" CLAUDE.md
```

All three must succeed.

## Files

- [00_SPEC.md](00_SPEC.md) — scope.
- [02_TASKS.md](02_TASKS.md) — atomic tasks.
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification.
- [06_HANDOFF.md](06_HANDOFF.md) — handoff to Phase 9.

## Reference docs

- [../10_CLAUDE_MD_PROJECT_MEMORY.md](../10_CLAUDE_MD_PROJECT_MEMORY.md) — the verbatim addendum block.
