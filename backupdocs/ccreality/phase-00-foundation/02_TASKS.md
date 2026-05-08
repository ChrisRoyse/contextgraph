# Phase 0 — Tasks

Execute these in order. Each task has an explicit verification.

## Task 0.1 — Read current git state

Run:
```bash
cd /home/cabdru/contextgraph
git status --porcelain | head -50
git rev-parse HEAD
git branch --show-current
```

If `git status --porcelain` is non-empty, the working tree has uncommitted changes. **Decision point:**
- If the diff is the planning docs from prior session (`docs/ccreality/`, `CLAUDE.md` addendum, deleted `docs2/.raw/*`), this is expected and OK.
- If unexpected files appear modified, STOP and report to operator.

If `git branch --show-current` is not `main`, STOP and report. The cutover should branch from `main`.

## Task 0.2 — Capture base SHA

Run:
```bash
git rev-parse HEAD > /tmp/ccreality-cutover-base.sha
```

Verify:
```bash
test -s /tmp/ccreality-cutover-base.sha \
  && grep -qE '^[0-9a-f]{40}$' /tmp/ccreality-cutover-base.sha
echo "exit=$?"
```

Expected: `exit=0`.

If the file contains a trailing newline, that's fine — `grep -E` matches the line, not the file.

## Task 0.3 — Create cutover branch

Run:
```bash
cd /home/cabdru/contextgraph
git checkout -b ccreality-cutover
```

Verify:
```bash
[ "$(git branch --show-current)" = "ccreality-cutover" ]
echo "exit=$?"
```

Expected: `exit=0`.

If a branch named `ccreality-cutover` already exists from a previous attempt:
- If it points at the same SHA captured in Task 0.2 and the working tree matches what we expect, just `git checkout ccreality-cutover` instead of `-b`.
- If it points elsewhere, STOP and ask operator whether to delete or use a different name.

## Task 0.4 — Confirm branch SHA matches captured SHA

```bash
[ "$(git rev-parse ccreality-cutover)" = "$(cat /tmp/ccreality-cutover-base.sha)" ]
echo "exit=$?"
```

Expected: `exit=0`.

If they differ, the branch creation accidentally moved the SHA (shouldn't happen with `git checkout -b`). Recover by:
```bash
git checkout ccreality-cutover
git reset --hard "$(cat /tmp/ccreality-cutover-base.sha)"
```
And re-run Task 0.4.

## Task 0.5 — Run the full verification block

See [04_VERIFICATION.md](04_VERIFICATION.md). Phase is complete when that block returns exit 0.
