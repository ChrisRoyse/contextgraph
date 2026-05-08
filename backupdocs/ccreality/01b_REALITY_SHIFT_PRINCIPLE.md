# Reality-Shift Principle (Core Design Invariant)

> "In order for the AI to alter reality it must always understand the state of
> reality so it can see always how reality is shifting."

This is the central, non-negotiable design principle of ccreality. Every other piece of architecture in this directory exists to serve it. If a future change conflicts with this principle, the change is wrong.

## The principle in one sentence

**No alteration without current state, and no alteration without a delta back into perception.**

## Why this matters

Coding-agent failures cluster around two pathologies:

1. **Stale-state edits** — the agent reasons against an old snapshot and edits a file that has since changed (by another tool, another session, or another process). The edit succeeds, but it was a fix to a problem that no longer exists.
2. **Open-loop edits** — the agent edits, doesn't observe the consequence, then keeps reasoning as if the edit "worked." Errors compound silently for many turns.

The reality-shift principle closes both:
- **Pre-alter:** every alter tool requires the agent to first perceive current state (`harness_open_window` returns the current SHA; `harness_apply_line_window_edit` requires that SHA back as `observed_sha256`).
- **Post-alter:** every alter tool returns the delta (before-SHA, after-SHA, lines added/removed, cargo check outcome, git diff) AND mints a durable shift artifact that the next turn perceives via the reality-drip hooks.

## Three derivatives the agent must always have

At any moment during an optimizer turn, Claude Code can answer all three questions:

### 1. **Current state** — "what is reality right now?"
- `mcp__cgreality__reality_latest_root` → the active runtime root + run id + target instance
- `mcp__cgreality__reality_attempt_summary` → the latest attempt artifact verbatim
- `mcp__cgreality__harness_git_status` → working tree state
- Each artifact returns its own SHA-256 in the `source_of_truth` envelope

### 2. **Recent shift** — "what has changed since I started this turn?"
- `mcp__cgreality__reality_shift_log {session_id}` → ordered list of every alter tool call this Claude Code session made, with before/after deltas
- `mcp__cgreality__reality_compare_attempts {a, b}` → high-level diff between two attempts
- `mcp__cgreality__reality_harness_transitions {run_id, attempt}` → every governed-tool mutation in the current attempt
- The `UserPromptSubmit` hook re-injects the latest 3 shift events on every prompt

### 3. **Drift since last perception** — "what changed without me knowing?"
- `mcp__cgreality__reality_replay_artifact {path}` → re-reads the file, returns current SHA. Compare to the SHA the agent saw earlier.
- The `cc-reality-source-of-truth-readback` subagent automates this for every file the writer touched.

## The new MCP tool: `reality_shift_log`

Added to the `cgreality` MCP server. Reads the per-session JSONL append log:

```
<runtime_root>/cgreality-shift-log/<claude_session_id>.jsonl
```

Each line:

```json
{
  "shift_id": "uuid",
  "timestamp_unix_ns": "integer",
  "tool_name": "harness_apply_line_window_edit",
  "tool_use_id": "string",
  "subject": { "type": "file", "path": "crates/.../reality-loop.rs" },
  "before": { "sha256": "...", "size_bytes": 123, "mtime_unix": 1234 },
  "after":  { "sha256": "...", "size_bytes": 456, "mtime_unix": 5678 },
  "delta_summary": { "lines_added": 12, "lines_removed": 4, "diff_stat": "..." },
  "verification": { "cargo_check": "PASS", "smoke": "SKIPPED" },
  "harness_transition_path": "file:..."
}
```

`mcp__cgreality__reality_shift_log` input:
```json
{ "session_id": "string", "limit": "integer (default 20)", "since_shift_id": "string or null" }
```

Output: an array of shift records.

The MCP server's alter tools (`harness_apply_line_window_edit`, `store_memory`, `boost_importance`, `forget_concept`, `merge_concepts`, `optimizer_record_*`) MUST append a shift record after every successful mutation. The append is fsync'd before the tool returns, so a crash mid-tool either commits the shift or leaves no record.

## The new MCP tool: `reality_shift_compare_to_my_view`

Lets Claude check whether reality has shifted in ways its session didn't cause.

Input: `{ "session_id": "string", "files": ["string"] }`

Behavior: For each file, read current SHA from disk. Look up the most recent shift in this session that touched the file → its `after.sha256`. If they match, the file is unchanged since this session's last write. If they differ AND no shift in this session caused the change, **another writer mutated reality between the agent's actions** — the agent must re-perceive before continuing.

Output:
```json
{
  "files": [{
    "path": "string",
    "current_sha256": "string",
    "session_last_known_sha256": "string or null",
    "drift": "no_change | session_caused | external_drift_detected",
    "external_writer_hint": "string or null"
  }]
}
```

`external_drift_detected` is a strong signal — Claude should call `harness_open_window` again and re-form its plan against the new reality.

## How this changes the alter-tool contract

Every alter tool follows **mutate → fsync → re-read → diff → log shift → return readback**:

1. `mutate` — apply the change.
2. `fsync` — ensure the change is durable on disk.
3. `re-read` — read back from disk, compute new SHA.
4. `diff` — compute before/after delta.
5. `log shift` — append to `cgreality-shift-log/<session>.jsonl`, fsync.
6. `return readback` — return the delta + shift record path to the caller.

If any step fails, the tool returns an error and does NOT log a shift (since the shift didn't happen). The agent must retry against fresh state.

## How the reality-drip protocol changes

The `UserPromptSubmit` hook now also injects the **last 3 shift events** in the current Claude Code session, as factual statements:

> Reality shifts in this session so far:
> 1. (12s ago) edited crates/.../reality-loop.rs:5126-5180. cargo check PASS. shift_id=abc123.
> 2. (8s ago) stored memory uuid=def456 in namespace=ccreality.
> 3. (3s ago) opened file window crates/.../reality-loop.rs:6010-6050. SHA=sha256:....

This makes "how reality is shifting" a permanent feature of the agent's prompt context.

## How subagents respect the principle

- Read-only subagents (diagnostician, reward-analyst, swe-triage, readback) MUST NOT advance state. They read shift logs and artifact SHAs but don't append shifts.
- The writer subagent (harness-editor) MUST consult `reality_shift_log` before its first edit, AND after every cargo verification, to confirm no external drift occurred between perception and action.
- The readback subagent's job IS the drift check — it calls `reality_shift_compare_to_my_view` for every file the writer claimed to change.

## Why this is more than "log every action"

Plenty of agent harnesses log actions. The shift principle requires three additional things:

1. **The log is queryable mid-turn**, not just post-hoc. Claude can call `reality_shift_log` to see its own history while reasoning.
2. **The log is automatically re-injected** as factual statements at every prompt. Claude doesn't have to remember to look.
3. **The log enables drift detection.** By comparing what Claude thinks reality is to what reality actually is, we catch the stale-state pathology before it produces a wrong edit.

## Smoke test for the principle

Add to `/ccreality-smoke`: after the no-op file open in step 3, separately mutate the same file from outside the Claude Code session (the operator does it manually as part of the smoke). Then call `reality_shift_compare_to_my_view`. Verify it returns `external_drift_detected`. This proves the drift-detection path works.

## Failure modes and recoveries

| Failure | Detection | Recovery |
|---------|-----------|----------|
| Agent edits stale snapshot | `harness_apply_line_window_edit` returns `HARNESS_EDIT_SHA_MISMATCH` | Re-call `harness_open_window`; reason against new state |
| External writer mutates between agent's read and write | `reality_shift_compare_to_my_view` reports `external_drift_detected` | Re-perceive everything in the affected files; re-plan |
| Shift log gets out of sync with disk (e.g., file deleted) | Subsequent `harness_open_window` reports file missing | Re-form plan; if the missing file was the optimization target, surface to operator |
| Two writers race on the same shift log | Append uses O_APPEND atomic semantics; shifts may interleave but never corrupt | Acceptable; readers see all shifts in timestamp order |
| Shift log grows unbounded over a long-lived session | Log rotation at session end (rename to `<session_id>.jsonl.gz`) | Operational, not a correctness issue |

## Cross-references

- [01_ARCHITECTURE](01_ARCHITECTURE.md) — overall split of roles
- [02_REALITY_DRIP_PROTOCOL](02_REALITY_DRIP_PROTOCOL.md) — when reality is injected
- [03_MCP_REALITY_TOOLS_SPEC](03_MCP_REALITY_TOOLS_SPEC.md) — `reality_shift_log` and `reality_shift_compare_to_my_view` are added there
- [13_STRUCTURED_OUTPUT_SCHEMA](13_STRUCTURED_OUTPUT_SCHEMA.md) — recommendation must include `shift_log_excerpt`
- [18_FSV_PLAN](18_FSV_PLAN.md) — drift-detection FSV cases

## Single-sentence test

"Could the agent, at any moment in its turn, answer the question 'has reality changed under me since I last looked?' with a yes/no/where based on tools and hooks alone?" If yes, the principle holds. If no, the design has regressed.
