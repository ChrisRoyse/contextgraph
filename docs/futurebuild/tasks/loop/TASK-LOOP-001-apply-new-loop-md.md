---
task_id: TASK-LOOP-001
title: Apply new .claude/loop.md content from 14_NEW_LOOP_MD.md
phase: A
status: open
blocked_by: [TASK-FLYWHEEL-001]
blocks: [TASK-LOOP-002, TASK-LOOP-003]
effort: 0.25d
spec_refs: [SPEC-LOOP.md REQ-LOOP-01..05, TECH-LOOP.md §1]
---

## Goal

Replace `.claude/loop.md` content entirely with the block from `docs/futurebuild/14_NEW_LOOP_MD.md` between the `=== BEGIN LOOP MD ===` and `=== END LOOP MD ===` markers. The new content drops the Mode A/B framing, references D-drive paths, includes the daemon-health check at orientation, and removes inner-actor language.

This is a pure content swap — no Rust code touched. The new loop only works correctly if the daemon (FLYWHEEL-001) is running.

## Inputs

- `docs/futurebuild/14_NEW_LOOP_MD.md` — source of truth for the new content
- `.claude/loop.md` — current content (to be replaced)
- `docs/futurebuild/specs/functional/SPEC-LOOP.md` REQ-LOOP-01..05

## Outputs

- `.claude/loop.md` — fully replaced with the new content

## Definition of done

1. `head -50 .claude/loop.md` matches the head-50 of the block in `14_NEW_LOOP_MD.md` exactly (whitespace-identical).
2. `wc -l .claude/loop.md` ≈ matches the new content's line count.
3. `grep -c "Mode A" .claude/loop.md` returns 0.
4. `grep -c "Mode B" .claude/loop.md` returns 0.
5. `grep -c "inner LLM" .claude/loop.md` returns 0.
6. `grep -c "/mnt/d/contextgraph" .claude/loop.md` returns ≥ 5 (D-drive paths referenced).
7. `grep -c "mejepa_record_agent_feedback" .claude/loop.md` returns ≥ 1 (the agent-feedback channel referenced).
8. Run a `/loop` turn (synthetic, no actual work) and confirm the orientation reads work end-to-end with no missing references.

## FSV evidence path

n/a — loop.md is the artifact; SHA captured by the PostToolUse hook.

## Boundary cases

- Existing `.claude/loop.md` already matches → idempotent (no diff produced)
- Operator hand-edits between TASK-LOOP-001 runs → conflict detected; surface to operator for re-application
