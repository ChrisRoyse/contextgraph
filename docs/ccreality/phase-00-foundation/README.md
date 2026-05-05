# Phase 0 — Foundation

**Estimated duration:** 5 minutes.
**Status:** complete.
**Depends on:** none.
**Unblocks:** Phase 1 (Excision).

## What this phase does

Creates the cutover branch and captures the base SHA for rollback. This is a 5-minute safety net — every subsequent phase modifies the working tree, and this is the known-good commit to revert to if the cutover goes wrong.

## AI-verifiable end-state

```bash
git rev-parse --quiet --verify ccreality-cutover \
  && test -s /tmp/ccreality-cutover-base.sha \
  && grep -qE '^[0-9a-f]{40}$' /tmp/ccreality-cutover-base.sha
```

Exit 0 = phase complete.

## Completion Record

- Completed on branch `ccreality-cutover`.
- Base SHA: `411443fdcbd8db4005cfe46d44f64f7698c2f11b`.
- Rollback source of truth: `/tmp/ccreality-cutover-base.sha`.
- Final Phase 0 verification returned exit 0 immediately after branch creation.

## Files

- [00_SPEC.md](00_SPEC.md) — objectives, scope, end-state.
- [02_TASKS.md](02_TASKS.md) — atomic tasks.
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification commands.
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after, pointer to Phase 1.

## Reference docs

- [../19_MIGRATION_RUNBOOK.md](../19_MIGRATION_RUNBOOK.md) — Stage 0 in the runbook.
