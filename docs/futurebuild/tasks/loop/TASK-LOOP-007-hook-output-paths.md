---
task_id: TASK-LOOP-007
title: Hook output paths to D-drive (cgreality-shift-log)
phase: A
status: open
blocked_by: []
blocks: []
effort: 0.25d
spec_refs: [SPEC-LOOP.md REQ-LOOP-06, TECH-LOOP.md §3]
---

## Goal

Update the PostToolUse hook scripts so shift-log entries write to `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` instead of `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session>.jsonl`. Same JSON shape; same filename convention; new directory.

## Inputs

- `scripts/cc/hook_post_tool_use_edit.sh` — current writer (writes to `/tmp/`)
- `scripts/cc/hook_post_tool_use_*.sh` — any other PostToolUse scripts
- `docs/futurebuild/specs/technical/TECH-LOOP.md` §3 (the updated script template)
- `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` §4.4

## Outputs

- `scripts/cc/hook_post_tool_use_edit.sh` — replaced with D-drive path version
- (any other affected hook scripts) — same update

## Definition of done

1. `scripts/cc/hook_post_tool_use_edit.sh` writes to `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl`.
2. The directory is created if missing (`mkdir -p`) at hook invocation time.
3. Triggering an Edit on a tracked file → shift-log line lands in the D-drive path within 100ms.
4. Existing `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/` files are NOT migrated automatically (the daemon's shift-subscriber will only consume the new D-drive path).
5. The hook still emits the standard "PostToolUse:Edit hook additional context" line for the agent (existing behavior unchanged).

## FSV evidence path

n/a — operational test only.

## Boundary cases

- `/mnt/d/contextgraph/runtime/cgreality-shift-log/` does not exist → mkdir succeeds; line written
- `/mnt/d/` not mounted → hook fails (exit non-zero); operator's edit completes; shift entry is lost (visible alert)
- Multiple agent sessions running in parallel → each writes to its own `<session>.jsonl` file; no conflict
