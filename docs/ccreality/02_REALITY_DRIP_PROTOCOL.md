# Reality-Drip Protocol

> "Hook a reality loop into Claude Code where it is constantly fed the reality
> it needs to be fed to be able to do the job it needs to complete."

This is the design center. Claude Code is **always** receiving fresh reality during a turn, never just at the start. Six injection points, plus the always-available MCP query channel.

## Relationship to the reality-shift principle

The drip protocol is the **mechanism**. The shift principle (see [01b_REALITY_SHIFT_PRINCIPLE](01b_REALITY_SHIFT_PRINCIPLE.md)) is the **goal**. Drip injections deliver three derivatives every turn:
1. **Current state** — runtime root, latest attempt summary, latest signal compact (SessionStart, UserPromptSubmit).
2. **Recent shift** — latest 3 shift-log entries from this Claude Code session (UserPromptSubmit, PostToolUse).
3. **Drift detection** — file SHAs at each PreToolUse Edit; the readback subagent calls `reality_shift_compare_to_my_view` on demand.

If a drip injection ever delivers stale reality (e.g. an old runtime root), the shift principle is violated. The drip path must always return current state.

## Six injection points

| # | Hook | Trigger | What gets injected | Cap |
|---|------|---------|---------------------|-----|
| 1 | `SessionStart` (matcher: `startup\|resume`) | Session begins or resumes | Runtime root, target SWE instance, success predicate, latest run summary path, the explicit reality-boundary rules | 10 KB |
| 2 | `UserPromptSubmit` | Each user prompt before Claude sees it | Latest trigger decision, reality-signal compact, top-3 unresolved attempts, latest harness-transition outcome, **last 3 shift-log entries from this session** | 10 KB |
| 3 | `PreToolUse` (matcher: `Edit\|Write`) | Before Claude edits a file | Current file SHA-256, governed-path policy verdict, recent harness transitions touching this file | 10 KB |
| 4 | `PostToolUse` (matcher: `Edit\|Write`) | After Claude's edit succeeds | `cargo check` exit + new SHA + `git diff --stat` + writes a `harness-transition` record | 10 KB |
| 5 | `Stop` | Claude is about to end its turn | Refuses to stop until `optimizer_record_recommendation` MCP tool was called this session; injects the missing-recommendation reason | n/a |
| 6 | `PreCompact` | Context about to be compacted | Re-injects runtime root + last trigger decision + last reality-signal compact so they survive compaction | n/a |

## Plus: always-on MCP channel

At any moment in the turn, Claude can call `mcp__cgreality__reality_*` to pull any artifact:

- `reality_latest_root` — currently active runtime root + run id
- `reality_attempt_summary {run_id, attempt}` — full attempt JSON
- `reality_official_report {run_id, attempt}` — official SWE-bench JSON for that attempt
- `reality_problem_packet {run_id}` — problem-reality packet
- `reality_signal {run_id, attempt}` — reality-signal packet
- `reality_failure {run_id, attempt}` — failure-evidence detail
- `reality_trigger_decision {run_id, attempt}` — why this turn fired
- `reality_harness_transitions {run_id, attempt, round?}` — list or detail
- `reality_compare_attempts {run_id, a, b}` — diff two attempts
- `reality_audit_trail {entity_id}` — ContextGraph audit log
- `reality_shift_log {session_id, limit?, since_shift_id?}` — every alter call this session made, with deltas
- `reality_shift_compare_to_my_view {session_id, files}` — drift detection (file SHA on disk vs session's last-known SHA)

Memory tools are reused unchanged: `search_graph`, `get_memory_neighbors`, `get_unified_neighbors`, `traverse_graph`, `search_by_embedder`, `get_audit_trail`, `get_provenance_chain`, `get_memory_fingerprint`, `extract_entities`. They are exposed under the `cgreality` MCP server with the same schemas.

## Phrasing rule (avoid prompt-injection defense)

Reality must be phrased as **factual statements**, not imperative commands. Source: Claude Code docs — text framed as out-of-band instructions can trigger Claude's prompt-injection defenses, which surfaces the text to the user instead of injecting it as context.

- ❌ Bad: `"You MUST read attempt-3 before editing."`
- ✅ Good: `"Latest trigger decision: should_run=true, policy=failure_or_regression. Reasons: official_swe_not_resolved, repeated_patch_detected. Source of truth: file:/tmp/.../attempt-3/trigger-decision.json"`

The hook scripts in `scripts/cc/` MUST emit reality this way. The MCP tool responses MUST do the same.

## Structured `additionalContext` over plain stdout

Claude Code hooks accept either plain stdout (added to context as text) OR structured JSON via `hookSpecificOutput.additionalContext` (also added but with explicit hook event tagging). We use the structured form because:

1. It survives `--output-format json` parsing in `claude -p` mode.
2. It is explicitly attributed to the hook event, so it is easier to audit.
3. It does not get mistaken for a normal user message in the transcript.

Example PostToolUse Edit emission:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "Reality update: edit applied to crates/context-graph-cli/src/bin/reality-loop.rs. Pre-SHA sha256:a1b2... Post-SHA sha256:c3d4... cargo check (exit 0) PASS. git diff --stat shows 1 file, +12 -4. New harness-transition recorded at file:/tmp/.../attempt-3/harness-transitions/round-04.json"
  }
}
```

## Per-injection-point detail

### 1. SessionStart

Script: `scripts/cc/hook_session_start.sh`

Reads:
- `~/.cache/cgreality/active_runtime_root` (single-line file maintained by reality engine)
- `<runtime_root>/run-summary.json` (latest run)
- `<runtime_root>/active_target_instance` (single-line file)

Emits `additionalContext` with:
- Runtime root absolute path
- Currently active SWE-bench instance ID
- Success predicate (verbatim)
- Reality-boundary rules (verbatim, list of forbidden paths)
- Pointer to ContextGraph MCP server (`mcp__cgreality__*` tool family)
- The 1-line summary of the latest run (status, attempts, resolved or not)

Failure: if no runtime root exists yet, exit 0 with `additionalContext` saying "No active reality-loop runtime root. Operator can call `mcp__cgreality__reality_run_smoke` to create one." Never block session start.

### 2. UserPromptSubmit

Script: `scripts/cc/hook_user_prompt_submit.sh`

Reads:
- The active runtime root.
- `<runtime_root>/<task>/attempt-N/trigger-decision.json` (latest)
- `<runtime_root>/<task>/attempt-N/reality-signal-packet.json` → compact form

Emits `additionalContext` with:
- Trigger decision (status, should_run, policy, reasons)
- Reality-signal compact (failure digest, official progress, patch signals, score reality, reinjection directive)
- The verbatim reinjection directive from reality-signal-packet (this is the field designed for this exact purpose by the existing reality-loop builder)

Failure: if no trigger decision exists for this turn (e.g. no attempt has run yet), emit `additionalContext` saying "No new trigger decision since last optimization turn." Never block.

### 3. PreToolUse Edit/Write

Script: `scripts/cc/hook_pre_tool_use_edit.sh`

Reads `tool_input.file_path` from stdin. Then:
- Computes current SHA-256 of the file.
- Looks up the file in `governed_paths.json` (project-rooted: which paths are editable, which are forbidden).
- Searches recent harness-transitions for any record touching this file in the current run.

Emits:
- `permissionDecision: "deny"` + `permissionDecisionReason` if path is forbidden (`.env`, `crates/context-graph-benchmark/datasets/swe_bench_*`, `swebench/eval/**`, etc.).
- `permissionDecision: "allow"` + `additionalContext` with current SHA and recent edit history if path is allowed.

The `updatedInput` field is left untouched.

### 4. PostToolUse Edit/Write

Script: `scripts/cc/hook_post_tool_use_edit.sh`

Reads `tool_input.file_path`. Then:
- Re-reads the file → new SHA-256.
- Runs `cargo check -p context-graph-cli --bin reality-loop` (timeout 60s) if a Rust file under `crates/context-graph-cli/` changed.
- Runs `git diff --stat -- <file>` to capture line delta.
- Calls `mcp__cgreality__optimizer_record_harness_transition` to mint a transition record with: tool_use_id, file path, pre/post SHA, cargo check result, git diff stat, timestamp.

Emits `additionalContext` with the transition record's source-of-truth path. The cargo failure is included verbatim if non-zero exit.

### 5. Stop

Script: `scripts/cc/hook_stop.sh`

Reads `transcript_path`. Greps for `mcp__cgreality__optimizer_record_recommendation` tool_use within this session.

If absent → emits `decision: "block"` + `reason: "Cannot stop: this optimization turn has not recorded a structured recommendation. Call mcp__cgreality__optimizer_record_recommendation with the schema in docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md."`

If present → exit 0, allow stop. Optionally emits a `Notification` event with the recommendation source-of-truth path so the operator sees it in the terminal.

### 6. PreCompact

Script: `scripts/cc/hook_pre_compact.sh`

Reads the active runtime root + latest trigger decision + latest reality-signal compact.

Emits `additionalContext` with the same payload as SessionStart, so the post-compaction context still has the runtime root and the last reality summary. Never blocks compaction.

## What if a hook fails?

Hooks are best-effort augmentation. With one exception (the `Stop` hook, which intentionally blocks until the recommendation is recorded), no hook should ever block Claude Code from doing its job because of an environment problem. Specifically:

- Missing CLI binary → emit `additionalContext` saying so, exit 0, allow.
- Missing runtime root → emit `additionalContext` saying so, exit 0, allow.
- Missing artifact JSON → exit 0 with `additionalContext` listing what was missing.
- Hook script crashes → debug log; do not block.

The `Stop` hook is the only "hard gate" because the architecture demands the recommendation artifact close every turn.

## Cap discipline

`additionalContext` is capped at 10,000 characters. The reality-signal packet alone can exceed that. The hook scripts MUST:

1. Use the **compact** form of every artifact (`compact_reality_signal_packet`, `compact_problem_reality_packet` already exist in `reality-loop.rs`; expose the same shape via MCP tools so hook scripts can request compact JSON).
2. Cite source-of-truth paths instead of inlining huge JSON. Claude can pull the full artifact via MCP if needed.

Rule: hook output should be **a navigable reality summary**, never an attempt to embed everything. The MCP layer is the random-access channel for full content.

## How this feels to Claude Code

Every prompt the operator submits triggers:

1. Hook 2 (UserPromptSubmit) injects the latest trigger decision and signal compact.
2. Claude Code sees: "Reality update: trigger fired because official_swe_not_resolved and repeated_patch_detected. Failure digest: PatchApplyError on auth.py:42. Reinjection directive: Gemma should re-read the problem-reality packet's oracle paths before drafting a new patch. Source: file:..."
3. Claude reasons, calls `mcp__cgreality__reality_problem_packet` for full detail, calls `mcp__cgreality__reality_compare_attempts` for the diff, decides to edit `reality-loop.rs:5126` (where `agent_tool_specs()` lives).
4. Hook 3 (PreToolUse Edit) injects current SHA + path policy → allow with context.
5. Claude edits.
6. Hook 4 (PostToolUse Edit) runs cargo check, writes a harness-transition record, injects the result.
7. Claude calls `mcp__cgreality__reality_run_smoke` to test the change.
8. ...
9. Claude calls `mcp__cgreality__optimizer_record_recommendation` with the structured payload.
10. Hook 5 (Stop) sees the recommendation, allows the turn to end.

Every step has a reality artifact tied to it. The optimizer's reasoning is auditable from the artifact tree alone.
