# Phase 0 — Verification

## End-state check

Run from the project root:
```bash
cd /home/cabdru/contextgraph
git rev-parse --quiet --verify ccreality-cutover \
  && test -s /tmp/ccreality-cutover-base.sha \
  && grep -qE '^[0-9a-f]{40}$' /tmp/ccreality-cutover-base.sha \
  && [ "$(git rev-parse ccreality-cutover)" = "$(cat /tmp/ccreality-cutover-base.sha)" ]
echo "phase-00 verification exit=$?"
```

**Pass criterion:** `phase-00 verification exit=0`.

## What each clause checks

1. `git rev-parse --quiet --verify ccreality-cutover` — branch exists.
2. `test -s /tmp/ccreality-cutover-base.sha` — file exists and is non-empty.
3. `grep -qE '^[0-9a-f]{40}$' /tmp/ccreality-cutover-base.sha` — file contents are a valid SHA-1 hex string.
4. `[ "$(git rev-parse ccreality-cutover)" = "$(cat /tmp/ccreality-cutover-base.sha)" ]` — the branch is currently pointing at exactly the SHA we recorded.

## Failure modes

| Symptom | Diagnosis | Fix |
|---------|-----------|-----|
| Clause 1 fails | branch not created | re-run Task 0.3 |
| Clause 2 fails | SHA file not written | re-run Task 0.2 |
| Clause 3 fails | SHA file content malformed | inspect; re-run Task 0.2 |
| Clause 4 fails | branch moved off the captured SHA | reset branch to captured SHA: `git checkout ccreality-cutover && git reset --hard "$(cat /tmp/ccreality-cutover-base.sha)"` |

## Stable across re-runs

Re-running this verification after a successful Phase 0 returns exit 0. After Phase 1+ (which add commits to the branch), the branch SHA will diverge from the captured SHA — that's expected, but Phase 0's verification will start failing clause 4.

For this reason, Phase 0's verification is only meaningful immediately after Phase 0 runs. Subsequent phases do not re-run it.
