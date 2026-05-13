---
namespace: reports
agent_id: codex
timestamp: 2026-05-13T11:03:36Z
status: complete
tags: phase-h, task-loop-003, smap, superseded-decisions, issue-132, fsv
---

# Session Report: Phase H SMAP Superseded Decisions

## Objective

Close GitHub issue #132 / `TASK-LOOP-003` by marking retired inner-actor decision files as superseded and proving the SMAP state with FSV readback.

## Files Written or Updated in ./memory/

- Updated `memory/decisions/agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md` with `status: superseded` and `superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md`.
- Updated `memory/decisions/agent-73-harness-editor--wallclock-budget-raise.md` with the same supersession pointer.
- Updated `memory/decisions/agent-58-harness-editor--post-apply-action-message-clarification.md` with the same supersession pointer.
- Updated `memory/decisions/agent-47-harness-editor--per-attempt-wallclock-budget.md` with the same supersession pointer.
- Updated `memory/decisions/agent-111-coordinator--reality-run-attempt-fire-and-forget.md` with the same supersession pointer.

## Source-of-Truth Verification

- FSV script: `scripts/ci/phase_h_smap_superseded_decisions_fsv.sh`.
- FSV artifact: `/mnt/d/contextgraph/fsv/phase-h-smap-superseded-decisions-fsv/smap_superseded_decisions_fsv.json`.
- FSV sha256: `1090e03002ef0a6e7e5a6b4b175c480900a7cc1d08c9d900ad57f0a116a3f49f`.
- `build_release_sha`: `3874e1ce`.
- Result: `all_passed=true`, `readback_equal=true`.

The FSV readback proves five targeted legacy decision files have `status: superseded` plus the forward pointer, total `status: superseded` count in `memory/decisions` is 6, active decision files have zero current-contract matches for the retired inner-actor path, and a synthetic active Qwen/reality-run contract is detected.

## Tracked Repo Changes

- Added `scripts/ci/phase_h_smap_superseded_decisions_fsv.sh`.
- Updated `docs/futurebuild/11_DELETION_LIST_INNER_LOOP.md`, `12_IMPLEMENTATION_PHASES.md`, and `13_TASK_INDEX.md`.
- Commit: `3874e1ce docs(loop): supersede retired smap decisions`.

## Residual Risk

`memory/` is intentionally gitignored, so the decision frontmatter changes are local SMAP state. The tracked FSV harness will fail if those local SMAP edits disappear or if an active decision reasserts the retired inner-actor contract.
