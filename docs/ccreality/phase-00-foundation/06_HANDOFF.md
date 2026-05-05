# Phase 0 — Handoff

## What's true after Phase 0

- `git branch --show-current` returns `ccreality-cutover`.
- `/tmp/ccreality-cutover-base.sha` exists with the base SHA.
- Working tree is unchanged from before Phase 0 (no source files modified).
- The pre-Phase-0 planning docs in `docs/ccreality/*` are still present and uncommitted (or committed; either is fine — they were created in a prior session).

## Completion Record

- Completed on 2026-05-05.
- Base SHA: `411443fdcbd8db4005cfe46d44f64f7698c2f11b`.
- Source of truth: `.git/refs/heads/ccreality-cutover` and `/tmp/ccreality-cutover-base.sha`.
- Phase 0 FSV tested empty, malformed, and mismatched SHA-file states; each failed verification before restore, and the final restored verification returned exit 0.

## Rollback recipe (when needed)

If you need to abort the entire cutover at any later point:
```bash
git checkout main
git branch -D ccreality-cutover  # only after merging or explicitly discarding
git checkout "$(cat /tmp/ccreality-cutover-base.sha)" -- <path>  # to restore a specific file
```

The `/tmp/ccreality-cutover-base.sha` file is the durable rollback target across the whole cutover. Don't delete it until Phase 12 commits.

## Next phase

→ [phase-01-excision/](../phase-01-excision/README.md)

Phase 1 is the largest single phase (1–2 hours). It excises ~3,400 lines of DeepSeek/OpenAI code from `crates/context-graph-cli/src/bin/reality-loop.rs` and lifts a shared governed-edit module out into its own file. Read its `00_SPEC.md`, `01_PRECONDITIONS.md`, `02_TASKS.md`, `03_FILE_SPECS.md`, `04_VERIFICATION.md`, `05_RECOVERY.md`, `06_HANDOFF.md` before starting.
