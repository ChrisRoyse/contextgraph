# 20 — Hooks Layer

Source files covered: `.claude/settings.json`, `scripts/cc/hook_session_start.sh`, `scripts/cc/hook_pre_tool_use_edit.sh`, `scripts/cc/hook_post_tool_use_edit.sh`, `scripts/cc/hook_user_prompt_submit.sh`, `scripts/cc/hook_stop.sh`, `scripts/cc/hook_pre_compact.sh`, `crates/context-graph-cli/src/commands/hooks/`, `docs/futurebuild/08_LIVE_PREDICTION_AGENT_API.md §3`, `docs/futurebuild/06_REALITY_LOOP_REFACTOR.md §7`.

> **Hooks are the bridge from Claude Code's runtime to the daemon's continuous learning loop.** Every Edit/Write is captured. Every UserPromptSubmit gets an injected prediction (futurebuild). Every turn must end with `optimizer_record_recommendation`.

---

## 1. The 6 hook types

| Hook | Trigger | Timeout | Purpose |
|---|---|---|---|
| `SessionStart` | conversation start | 5,000 ms | Inject active runtime root (D-drive) + reality-boundary rules |
| `UserPromptSubmit` | every user prompt | 2,000 ms | Inject last 3 shift entries + latest `RealityPrediction` markdown |
| `PreToolUse` Edit/Write | before each edit | 500 ms (FAST PATH, NO DB) | Record pre-edit SHA |
| `PostToolUse` Edit/Write | after each edit | 3,000 ms | Record after_sha + cargo check (Rust files) + git diff + shift-log JSONL append |
| `Stop` | end of turn | (synchronous) | Require `optimizer_record_recommendation` — block turn end |
| `PreCompact` | context compaction | 20,000 ms | SMAP narrative dump |

Additional Claude Code hooks (not currently used by ME-JEPA): `SessionEnd`, `TaskCompleted`.

---

## 2. `.claude/settings.json`

```jsonc
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": ".*",
        "hooks": [{ "type": "command", "command": "scripts/cc/hook_session_start.sh", "timeout": 5000 }]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": ".*",
        "hooks": [{ "type": "command", "command": "scripts/cc/hook_user_prompt_submit.sh", "timeout": 2000 }]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [{ "type": "command", "command": "scripts/cc/hook_pre_tool_use_edit.sh", "timeout": 500 }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [{ "type": "command", "command": "scripts/cc/hook_post_tool_use_edit.sh", "timeout": 3000 }]
      }
    ],
    "Stop": [
      {
        "matcher": ".*",
        "hooks": [{ "type": "command", "command": "scripts/cc/hook_stop.sh" }]
      }
    ],
    "PreCompact": [
      {
        "matcher": ".*",
        "hooks": [{ "type": "command", "command": "scripts/cc/hook_pre_compact.sh", "timeout": 20000 }]
      }
    ]
  }
}
```

---

## 3. `SessionStart`

| Input (env) | Output (injection) |
|---|---|
| `CLAUDE_SESSION_ID` | "ContextGraph reality loop is not currently running. Active runtime root: not set." (when no daemon) OR daemon-attached runtime root |

Behavior:
1. Resolve active runtime root: read `/mnt/d/contextgraph/state/cgreality/active_runtime_root` if present.
2. Emit reality-boundary rules: "never edit .env files, swebench official evaluators, tests/oracle_**, reward_specs, or verifiers."
3. Inject into Claude Code's session-init context.

Implementation: `crates/context-graph-cli/src/commands/hooks/session_start.rs`.

---

## 4. `PreToolUse` Edit/Write (FAST PATH, NO DB)

| Input (env) | Output |
|---|---|
| `TOOL_NAME`, `TOOL_INPUT` (JSON with `file_path`), `CLAUDE_SESSION_ID` | (stderr line emitted as system-reminder): `Pre-edit reality: file=<rel-path> current_sha=<sha> current_line_count=<n>. Governed-path gate: REMOVED 2026-05-07; SHA recorded for provenance only.` |

Behavior:
1. **Read current file SHA-256** (cheap, no DB).
2. **Record line count**.
3. Emit pre-edit reality line for post-edit delta computation.
4. **DO NOT touch DB**. The governed-path gate was removed 2026-05-07 ("the AI needs to be able to do anything it needs to do").

Implementation: `scripts/cc/hook_pre_tool_use_edit.sh` + `crates/context-graph-cli/src/commands/hooks/pre_tool.rs`.

---

## 5. `PostToolUse` Edit/Write

| Input | Output |
|---|---|
| Pre-edit SHA from PreToolUse, current after-edit state | (stderr): `Post-edit reality: file=<path> after_sha=<sha>. No active runtime root exists, so no harness transition was recorded. Active-root source of truth: file:/mnt/d/contextgraph/state/cgreality/active_runtime_root.` |

Behavior (per `scripts/cc/hook_post_tool_use_edit.sh`):
1. **Compute after_sha** of the just-edited file.
2. **Run cargo check** for `crates/context-graph-cli/*.rs` files (release profile).
3. **Run `git diff`** (status, stat).
4. **Append shift-log JSONL line** to `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl`:

```jsonc
{
  "ts": "2026-05-11T22:05:00Z",
  "session_id": "01HXYZ...",
  "file": "docs/mejepasystem/00_INDEX.md",
  "before_sha": "missing",
  "after_sha": "eef12ba064723d7eb0d851a037c04ce37b8821a5efa939efe13364d27717c44a",
  "line_count_before": 0,
  "line_count_after": 173,
  "cargo_check_result": null,
  "git_diff_summary": null,
  "hook_id": "post-edit-2026-05-11T22-05-00Z"
}
```

5. **Record harness transition** (if active runtime root is set) via `context-graph-cli ccreality record-harness-transition`.

Implementation: `crates/context-graph-cli/src/commands/hooks/post_tool.rs` + `crates/context-graph-cli/src/commands/ccreality/mod.rs`.

---

## 6. `UserPromptSubmit`

| Input | Output (injection) |
|---|---|
| `CLAUDE_USER_PROMPT`, `CLAUDE_SESSION_ID`, `TOOL_DESCRIPTION?` | Injected markdown into next agent prompt (≤ 4 KB cap per `agent-79-...md` decision) |

Current behavior (shipped, partial):
1. Read last 3 shift-log JSONL entries for this session.
2. Emit them as a compact summary.

**Futurebuild extension `TASK-PREDICT-001`** (Phase A):
1. Read `CF_MEJEPA_LIVE_PREDICTIONS` for this session.
2. If a prediction exists and is < 60s old, render markdown per [`17_prediction_taxonomy_reference.md §10`](17_prediction_taxonomy_reference.md):

```markdown
### Reality prediction (chunks affected since last prompt: N)

**Verdict:** Fail (oracle pass confidence 0.07 [0.04, 0.12])
...
**To respond to a wrong prediction:** call `mejepa_record_agent_feedback` with `prediction_id=...`.
```

3. Bound to ≤ 4 KB; if exceeding, truncate to top-5 per Q4 category. Full prediction queryable via `mejepa_predict_latest`.

---

## 7. `Stop`

Synchronous block: turn cannot end unless `optimizer_record_recommendation` MCP tool was called during the turn.

Behavior (per `scripts/cc/hook_stop.sh`):
1. Inspect ledger / decision artifacts for this turn.
2. If no recommendation recorded → emit error "Stop hook: recommendation required".
3. Otherwise → pass.

`optimizer_record_recommendation` schema requires: `run_id`, `attempt`, `claude_session_id`, `turn_number`, `status`, `target_code_unit`, `recommendation_type`, `hypothesis`, `confidence_score`, `test_plan`, `rollback_plan?`.

---

## 8. `PreCompact`

Context-compaction event. Dumps the current narrative for SMAP capture so post-compact agent inherits.

Behavior:
1. Read live conversation buffer.
2. Extract `decisions`, `discoveries`, `handoffs` into `./memory/journal/<date>--compaction-snapshot.md`.
3. Ensure all SMAP namespaces have recent entries (per CLAUDE.md §6 SMAP discipline).

---

## 9. Hook script implementation pattern

Each `scripts/cc/hook_*.sh` is a thin wrapper:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Read JSON input from stdin (Claude Code convention)
input=$(cat)

# Forward to the CLI handler
context-graph-cli hooks <subcommand> --stdin "$input" --session-id "$CLAUDE_SESSION_ID" --format json
```

The CLI handler in `crates/context-graph-cli/src/commands/hooks/*.rs` does the actual work.

---

## 10. Output paths

Per `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`:

| Output | Path |
|---|---|
| Shift-log JSONLs | `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` |
| Daemon-side state | `/mnt/d/contextgraph/state/cgreality/active_runtime_root` |
| Hook logs (per-tick) | stderr → captured by Claude Code; rotated by host |

**`TASK-LOOP-007`**: Hook output paths migration from `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/` to `/mnt/d/contextgraph/runtime/cgreality-shift-log/` — code shipped 2026-05-11; audit doc updated.

---

## 11. Retired hook behaviors

| Behavior | Status |
|---|---|
| Governed-path gate in `PreToolUse` | REMOVED 2026-05-07 (operator directive); current state correct — gutted to "reject empty/control-char paths only" |
| Drip a fresh `reality_run_attempt` | RETIRED — engine binary deleted; `CCREALITY_ENGINE_RETIRED` |
| Champion / promotion of best inner attempt | RETIRED post-pivot |
| Mode A/B switching in `loop.md` | RETIRED — see [`21_loop_contract.md`](21_loop_contract.md) |

---

## 12. Cross-references

[`13_data_flywheel.md`](13_data_flywheel.md) (shifts feed subscriber), [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md) (`optimizer_record_recommendation`), [`21_loop_contract.md`](21_loop_contract.md) (per-turn flow).
