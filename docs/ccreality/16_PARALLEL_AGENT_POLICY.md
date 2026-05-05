# Parallel-Agent Policy

## The single-writer rule

**At any moment, exactly one agent process may hold the harness-edit privilege.** Critics run in parallel; the writer runs alone.

Enforced three ways:

### 1. MCP server tool grant

Only `cc-reality-harness-editor` and the main session have `mcp__cgreality__harness_apply_line_window_edit` in their tool list. All other subagents (diagnostician, reward-analyst, swe-failure-triage, readback) explicitly do NOT include it.

### 2. Hook-side edit lock

The `PreToolUse Edit/Write` hook checks for a lock file at `~/.cache/cgreality/edit_lock`. If present and held by a different session/subagent, the hook returns:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "ccreality single-writer rule: another writer holds the edit lock (session=<other-id>, since=<unix>). Wait or coordinate."
  }
}
```

The lock is acquired in `cc-reality-harness-editor`'s prelude (and on the main session if it edits directly) by writing `<session_id>:<unix_ts>` into the lock file with `O_EXCL`. Released on `Stop` hook or `SubagentStop`.

### 3. MCP server reservation

Before applying any edit, `harness_apply_line_window_edit` calls into a small reservation service inside the MCP server: it sets a process-local "edit owner" field. If a parallel call comes in (within the same MCP server process) it returns `CCREALITY_HARNESS_EDIT_OWNER_BUSY`.

The combination of the three layers means:
- Two subagents in the same session can't both edit (layer 2 — file lock).
- Two MCP-server-driven calls can't race (layer 3 — process reservation).
- Two Claude Code sessions on the same project can't both edit (layer 2 again — same lock file across sessions).

## Why we don't allow parallel writers

Two writers on the same harness file race in two ways:
1. **SHA mismatch**: writer A reads SHA, writer B writes, writer A writes against stale SHA → `HARNESS_EDIT_SHA_MISMATCH` and a wasted turn.
2. **Semantic incoherence**: writer A's edit assumes the file shape before writer B's edit, even if both pass cargo check, the combined diff may be nonsense.

The shift-principle helps detect both, but the cleanest fix is to never let the race start.

## Critic parallelism (encouraged)

Read-only subagents may run concurrently. The orchestrator (main session or `/loop` skill) often kicks off:

```
Task → cc-reality-diagnostician
   ┌─ Task → cc-reality-reward-analyst       ┐
   ├─ Task → cc-reality-swe-failure-triage   │  in parallel
   └─ Task → cc-reality-source-of-truth-readback (later)
```

These don't interfere because they don't write. They each consume MCP analyze tools, which are idempotent reads.

The MCP server is single-threaded per session by default (Claude Code spawns one stdio child per session); concurrent tool calls within a session are serialized. This is fine — analyze tool latency is <200 ms.

## Across-session parallelism

Two operators on two machines could theoretically run two Claude Code sessions against the same git working tree. We discourage this:

- The edit lock prevents conflicting edits, but coordination overhead is high.
- Reality-shift artifacts get interleaved.
- Engine attempts can't be parallelized (Gemma + Docker + GPU are single-occupant).

If two-operator collaboration is needed, give each operator their own clone of the repo and their own `~/.cache/cgreality/active_runtime_root`. Merge harness changes via git PR review, not via concurrent edit.

## Across-runtime-root parallelism (allowed)

If the operator runs **two separate reality-loop runs** for two different SWE-bench instances simultaneously (e.g. one Gemma + Docker per GPU), each gets its own runtime root. The MCP server can serve different sessions for different runtime roots concurrently — no single-writer conflict because they touch different artifact trees.

The harness source code, however, is shared across runtime roots. So the harness-edit lock is global (single file in `~/.cache/cgreality/`). One writer at a time across the whole machine.

This bounds throughput: the rate at which we can improve the harness is bounded by one Opus session per harness commit. That's fine — harness optimization is not a high-frequency operation.

## Lock recovery

If a writer crashes holding the lock:
- The lock file contains `<session_id>:<unix_ts>`.
- A new writer attempts to acquire; if the existing session_id is not a live Claude Code session (checked via `pgrep` or via Claude Code's session list), the new writer takes over after a 30-second grace period.
- Stale locks older than 1 hour are auto-cleaned by the SessionStart hook.

## Why not use `flock(2)`

Writers don't always live for the duration of an edit (subagents come and go). A flock would expire when the subagent exits, before the recommendation is recorded. A small explicit lock file with PID + timestamp + session_id, manually released on Stop, is more robust here.

## Audit

Every lock acquisition and release is logged to `<runtime_root>/cgreality-edit-lock-events.jsonl`. The `cc-reality-source-of-truth-readback` subagent reviews this on demand to detect cross-session interference.
