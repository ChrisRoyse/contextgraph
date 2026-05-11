---
task_id: TASK-LOOP-007
title: Hook output paths to D-drive (cgreality-shift-log)
phase: A
status: complete (2026-05-11 — code already shipped; operational SoT migration documented)
blocked_by: []
blocks: []
effort: 0.25d
spec_refs: [SPEC-LOOP.md REQ-LOOP-06, TECH-LOOP.md §3]
---

## Audit outcome (2026-05-11)

The task body below is **stale**. Verified by reading the actual hook scripts:

- `scripts/cc/_common.sh:5-6` already pins `CGREALITY_CACHE="${CGREALITY_CACHE_DIR:-/mnt/d/contextgraph/state/cgreality}"` and `RUNTIME_ROOT_FILE="${CGREALITY_CACHE}/active_runtime_root"` — both D-drive paths.
- `scripts/cc/hook_user_prompt_submit.sh:45` and `scripts/cc/hook_pre_compact.sh:21` both compute `SHIFT_LOG="${RUNTIME_ROOT}/cgreality-shift-log/$(cgreality_safe_id "${SESSION_ID}").jsonl"` where `RUNTIME_ROOT` is the *content* of the SoT file, not a hardcoded path.
- `grep -r "/tmp/contextgraph-rust-reality-loop" scripts/cc/` returns 0 hits — the legacy path is no longer referenced anywhere.

**Therefore the code-level migration is already complete.** The only remaining work is operational: an operator must write a D-drive path (e.g. `/mnt/d/contextgraph/runtime`) into `/mnt/d/contextgraph/state/cgreality/active_runtime_root`. Right now that file is empty, so shift-log writes silently no-op — the fail-closed default.

Per CLAUDE.md §11.13 and the post-pivot ME-JEPA architecture, **shift-log writing during normal Claude Code operation is now optional** — the data flywheel (TASK-FLYWHEEL-003/004) replaces shift-log-as-training-data, so an empty `active_runtime_root` is the steady state until/unless the operator wants per-edit forensic logs.

No further code change required for TASK-LOOP-007.

## Verification contract

Inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). SoT readback, ≥3 boundary cases, NO mocks, fail-closed errors, Occam's-razor justification, §6 evidence artifacts mandatory. `ASSUME THE TASK IS WRONG` — audit `scripts/cc/hook_post_tool_use_*.sh` content before editing; the actual current writer file name may have drifted from the path listed in `## Inputs`.

## Goal

Update the PostToolUse hook scripts so shift-log entries write to `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` instead of `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session>.jsonl`. Same JSON shape; same filename convention; new directory.

## Inputs

- `scripts/cc/hook_post_tool_use_edit.sh` — current PostToolUse:Edit writer (records SHA, computes deltas)
- `scripts/cc/hook_user_prompt_submit.sh` — **(audited 2026-05-11)** writes shift-log references; check before changes
- `scripts/cc/hook_pre_compact.sh` — **(audited 2026-05-11)** also references shift-log paths
- `docs/futurebuild/specs/technical/TECH-LOOP.md` §3 (the updated script template)
- `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` §4.4

**Audit (2026-05-11):** `scripts/cc/hook_post_tool_use_edit.sh` is the documented writer per CLAUDE.md §11.6 surface 4, but `grep -l cgreality-shift-log scripts/cc/*.sh` shows the *actual* path references live in `hook_user_prompt_submit.sh` and `hook_pre_compact.sh`. Either (a) the writer was renamed and the spec is stale, or (b) the shift log is read by submit/compact but written elsewhere. **Verify by running an Edit and checking which file path ends up touched.** Do not just edit `hook_post_tool_use_edit.sh` blindly.

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
