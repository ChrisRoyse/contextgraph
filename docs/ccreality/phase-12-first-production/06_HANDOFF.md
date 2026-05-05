# Phase 12 — Handoff (and Cutover Complete)

## What's true after Phase 12

- One real SWE-bench Lite instance has been processed end-to-end through the new ccreality stack.
- A recommendation artifact exists, schema-validated, with status `changed|no_change|blocked|error`.
- Shift log has at least one entry.
- `ccreality-cutover` contains verified incremental cutover commits plus the final Phase 12 production verification/cutover commit.
- `/tmp/ccreality-phase-progress.log` records 13 PASS entries.

## What now

The branch `ccreality-cutover` is ready for review. Operator decisions:

### Option A — Merge to main
```bash
git checkout main
git merge --no-ff ccreality-cutover
git push  # if remote is set up
```

### Option B — Open a PR for review
```bash
gh pr create --title "feat(ccreality): replace DeepSeek outer optimizer with Claude Code MCP integration" --body "..."
```

### Option C — Continue iterating on ccreality-cutover
Keep editing on the branch; each future session uses `/ccreality-iterate` against ongoing SWE-bench attempts. The branch becomes the production line.

## Long-term operational tasks

- Add cron / systemd timer for autonomous overnight runs (see [../14_TRIGGER_AND_INVOCATION_POLICY.md](../14_TRIGGER_AND_INVOCATION_POLICY.md) §"E. External-watcher cron").
- Run `bash scripts/cc/tests/smoke.sh` as a daily CI gate for the first week.
- Run `bash scripts/cc/tests/fsv.sh` weekly for the first month.
- After 100 production turns, audit `cost_estimate_usd` in recommendations; consider Sonnet for routine writer turns.
- After 30 days, audit shift log for principle violations.

## Verification record

```bash
echo "phase-12: PASS, $(date -u +%FT%TZ), commit=$(git rev-parse HEAD)" >> /tmp/ccreality-phase-progress.log
echo "===== CUTOVER COMPLETE ====="
cat /tmp/ccreality-phase-progress.log
```

## Rollback (if needed later)

If a problem emerges after merge:
```bash
git checkout main
git revert <merge-sha>
# OR
git checkout "$(cat /tmp/ccreality-cutover-base.sha)" -- <specific-paths>
```

`/tmp/ccreality-cutover-base.sha` from Phase 0 is the rollback target.

## Done

The ccreality cutover is complete. Claude Code is now the outer reality-loop optimizer, integrated into the ContextGraph MCP system, with reality continuously fed via hooks and the reality-shift principle enforced through the shift log.

You can now drive the loop:
- `/ccreality-iterate` for one optimizer turn.
- `/ccreality-trigger-check` for a read-only status.
- `/ccreality-smoke` for verification.
- `/ccreality-readback` for drift detection on past recommendations.
- `/loop` for unattended runs.
