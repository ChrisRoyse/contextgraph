# Phase 0 — Specification

## Objectives

1. Capture the current commit SHA so we can roll back if the cutover fails.
2. Create a dedicated branch (`ccreality-cutover`) so all subsequent work is isolated from `main`.
3. Verify the working tree is in a known state (or document any uncommitted diff).

## Scope

- Read git state.
- Create one git branch.
- Write one file to `/tmp`.

## Out of scope

- Pushing to remote.
- Modifying any source file.
- Running cargo or any build.

## Deliverables

| Deliverable | Path | Format |
|-------------|------|--------|
| Branch | `ccreality-cutover` | git branch pointing at base commit |
| Rollback SHA | `/tmp/ccreality-cutover-base.sha` | one line, 40-char hex SHA, no trailing newline OK |

## AI-verifiable end-state

```bash
git rev-parse --quiet --verify ccreality-cutover \
  && test -s /tmp/ccreality-cutover-base.sha \
  && grep -qE '^[0-9a-f]{40}$' /tmp/ccreality-cutover-base.sha \
  && [ "$(git rev-parse ccreality-cutover)" = "$(cat /tmp/ccreality-cutover-base.sha)" ]
```

Returns exit 0 if and only if:
- The branch exists.
- The SHA file exists, is non-empty, contains a valid 40-char hex SHA.
- The branch points at exactly the SHA recorded in the file.

## Non-goals

- Not creating any tag.
- Not protecting the branch (no branch protection rules; not relevant for local work).
- Not setting up upstream tracking.

## Time budget

5 minutes.
