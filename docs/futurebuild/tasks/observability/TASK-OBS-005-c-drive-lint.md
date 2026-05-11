---
task_id: TASK-OBS-005
title: C-drive lint in CI (prevent regression to non-D-drive paths)
phase: A
status: open
blocked_by: []
blocks: []
effort: 0.25d
spec_refs: [SPEC-OBSERVABILITY.md REQ-OBS-FB-09, TECH-LOOP.md §6]
---

## Verification contract

Inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). SoT readback (the CI exit code IS the SoT here), ≥3 boundary cases below, NO mocks, fail-closed errors, Occam's-razor justification. `ASSUME THE TASK IS WRONG` — confirm `.github/workflows/` exists before wiring CI; if the repo uses a different CI runner, adapt the integration point.

## Goal

Add a CI lint that grep-asserts no source file introduces a hardcoded path matching `/home/`, `~/.cache/`, or `/tmp/contextgraph-` outside allowed exceptions. Prevents regression to non-D-drive paths after the FSV migration sweep (TASK-OBS-006).

## Inputs

- `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` §4.2 (the lint script template)
- `docs/futurebuild/specs/technical/TECH-LOOP.md` §6
- existing CI config: `.github/workflows/*.yml` or local CI runner config
- repository root for grep targets

## Outputs

- `ci/lints/no_c_drive_paths.sh` — NEW (per TECH-LOOP.md §6)
- `.github/workflows/ci.yml` — extended to invoke the lint as a step before `cargo build`
- FSV: deliberate-violation test fixture under `tests/lints/`

## Definition of done

1. `bash ci/lints/no_c_drive_paths.sh` returns exit 0 on the current source tree (after FSV migration).
2. Introducing a deliberate violation (e.g., hardcoded `/home/cabdru/somewhere` in a `.rs` file) → lint exits 1 with an error pointing at the file:line.
3. Allowed exceptions don't trigger:
   - source tree paths inside `/home/cabdru/contextgraph/` for code/doc/config files
   - `/tmp/` references in test scratch directories that are wrapped in `tempfile::TempDir`
   - documentation files under `docs/` (full paths in docs are OK)
   - SMAP files under `memory/`
   - `.claude/` config (the wrapper script's own path references)
4. CI step is wired into the workflow before `cargo build`.
5. `bash ci/lints/no_c_drive_paths.sh` runs in < 5 seconds on the workspace.

## FSV evidence path

n/a — lint is its own evidence; CI step output captured.

## Boundary cases

- File contains the literal string in a comment ("don't use /tmp/contextgraph-test/") → lint flags it; comment must be reworded
- Generated file (e.g., from build.rs) contains paths → lint must skip generated paths or `// generated; lint:allow` comment
- Test fixture directory under `tests/fixtures/` references `/tmp/` → allowed if wrapped in tempfile
