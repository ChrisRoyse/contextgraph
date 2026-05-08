# Phase 12 — First Production Attempt + Cutover Commit

**Estimated duration:** 1 hour.
**Status:** complete.
**Depends on:** Phases 0–11.
**Unblocks:** project ready for ongoing ccreality use.

## What this phase does

1. Picks one real SWE-bench Lite instance.
2. Runs one real engine attempt.
3. Runs one real `/ccreality-iterate` against that attempt.
4. Verifies the recommendation artifact and shift log.
5. Records the per-phase progress log into a final Phase 12 commit.
6. Leaves `ccreality-cutover` ready for review; optional squashing is a review-time decision, not part of Phase 12 verification.

This is the cutover finishing line. After Phase 12, the branch is ready for review and merge to `main`.

Final result: see [../PHASE12_RESULTS_2026-05-05.md](../PHASE12_RESULTS_2026-05-05.md). The production runtime root is `/tmp/ccreality-prod-1`; final FSV log is `/tmp/ccreality-phase12-fsv-20260505T225350Z.log`.

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph
# 1. A real attempt exists
test -d /tmp/ccreality-prod-1
ATTEMPT_SUMMARY=$(find /tmp/ccreality-prod-1 -name attempt-summary.json | head -1)
test -n "$ATTEMPT_SUMMARY"

# 2. A recommendation artifact exists
RECOMMENDATION=$(find /tmp/ccreality-prod-1 -path '*/claude-code-optimizer/recommendation-*.json' | head -1)
test -n "$RECOMMENDATION"
jq -e '.status' "$RECOMMENDATION" >/dev/null

# 3. Shift log has entries
SHIFT_LOG=$(find /tmp/ccreality-prod-1/cgreality-shift-log -name '*.jsonl' | head -1)
test -s "$SHIFT_LOG"

# 4. Branch has cutover commits ahead of main
[ "$(git log --oneline ccreality-cutover ^main | wc -l)" -ge 1 ]

# 5. Phase progress log exists with all 13 entries
test -s /tmp/ccreality-phase-progress.log
[ "$(grep -c PASS /tmp/ccreality-phase-progress.log)" -ge 13 ]
```

## Files

- [00_SPEC.md](00_SPEC.md), [01_PRECONDITIONS.md](01_PRECONDITIONS.md), [02_TASKS.md](02_TASKS.md), [04_VERIFICATION.md](04_VERIFICATION.md), [06_HANDOFF.md](06_HANDOFF.md).

## Reference docs

- [../19_MIGRATION_RUNBOOK.md](../19_MIGRATION_RUNBOOK.md) §"Stage 9" + §"Stage 10".
- [../11_LOOP_PROMPT.md](../11_LOOP_PROMPT.md).
