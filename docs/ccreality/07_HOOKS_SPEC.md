# Hooks Spec

Six hook scripts under `scripts/cc/`. They share a common preamble: locate the `context-graph-cli` binary (for MCP-side helpers) and source `~/.cache/cgreality/active_runtime_root`.

All scripts:
- Read JSON from stdin (Claude Code's hook input format).
- Write JSON to stdout via `hookSpecificOutput.additionalContext` (with a 10 KB cap).
- Exit 0 in normal cases. Exit 2 only when blocking is intended (Stop and PreToolUse Edit/Write may block).
- Log to a per-session file under `${runtime_root}/cgreality-hook-events.jsonl` (each call writes one JSONL line for debugging).

This document gives the full bash for each hook plus its JSON contract.

## Common helpers — `scripts/cc/_common.sh`

```bash
#!/usr/bin/env bash
# Shared by every cc hook. Source this at the top of each hook script.
set -euo pipefail

CGREALITY_CACHE="${CGREALITY_CACHE_DIR:-$HOME/.cache/cgreality}"
RUNTIME_ROOT_FILE="${CGREALITY_CACHE}/active_runtime_root"
TARGET_INSTANCE_FILE="${CGREALITY_CACHE}/active_target_instance"

cgreality_runtime_root() {
  if [ -f "$RUNTIME_ROOT_FILE" ]; then
    head -n 1 "$RUNTIME_ROOT_FILE"
  fi
}

cgreality_target_instance() {
  if [ -f "$TARGET_INSTANCE_FILE" ]; then
    head -n 1 "$TARGET_INSTANCE_FILE"
  fi
}

cgreality_emit_additional_context() {
  local hook_event="$1"
  local context="$2"
  jq -nc --arg event "$hook_event" --arg ctx "$context" '{
    "hookSpecificOutput": {
      "hookEventName": $event,
      "additionalContext": $ctx
    }
  }'
}

cgreality_emit_block() {
  local reason="$1"
  jq -nc --arg r "$reason" '{
    "decision": "block",
    "reason": $r
  }'
}

cgreality_emit_pretool_decision() {
  local decision="$1"
  local reason="$2"
  local context="$3"
  jq -nc --arg d "$decision" --arg r "$reason" --arg c "$context" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": $d,
      "permissionDecisionReason": $r,
      "additionalContext": $c
    }
  }'
}

cgreality_log_event() {
  local runtime_root="$(cgreality_runtime_root)"
  if [ -z "$runtime_root" ]; then return 0; fi
  local log="${runtime_root}/cgreality-hook-events.jsonl"
  if ! jq -nc --arg ts "$(date -u +%FT%TZ)" --arg ev "$1" --arg payload "$2" '{
    timestamp: $ts, event: $ev, payload: $payload
  }' >> "$log"; then
    printf '{"status":"error","error_code":"CCREALITY_HOOK_EVENT_LOG_WRITE_FAILED","source_of_truth":"file:%s"}\n' "$log" >&2
  fi
}
```

## 1. SessionStart — `scripts/cc/hook_session_start.sh`

Fires at start of every Claude Code session (interactive or `claude -p`). Injects the active runtime root, target instance, success predicate, reality-boundary rules, and pointer to MCP tools.

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

INPUT="$(cat)"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id // ""')"
SOURCE="$(echo "$INPUT" | jq -r '.source // ""')"

RUNTIME_ROOT="$(cgreality_runtime_root)"
TARGET_INSTANCE="$(cgreality_target_instance)"

if [ -z "$RUNTIME_ROOT" ]; then
  CTX="ContextGraph reality loop is not currently running. Active runtime root: not set. To start a smoke run, call mcp__cgreality__reality_run_smoke. To analyze past runs, call mcp__cgreality__reality_latest_root after starting one. Reality-boundary rules apply: never edit .env, swebench/, tests/oracle_**, reward_specs, or verifiers."
else
  RUN_SUMMARY=""
  if [ -f "$RUNTIME_ROOT/run-summary.json" ]; then
    STATUS="$(jq -r '.status // "unknown"' "$RUNTIME_ROOT/run-summary.json" 2>/dev/null || echo unknown)"
    ATTEMPTS="$(jq -r '.attempts | length' "$RUNTIME_ROOT/run-summary.json" 2>/dev/null || echo 0)"
    RUN_SUMMARY="Run status: $STATUS. Attempts so far: $ATTEMPTS."
  fi
  CTX=$(cat <<EOF
Active reality-loop runtime root: $RUNTIME_ROOT
Target SWE-bench instance: ${TARGET_INSTANCE:-unset}
Success predicate: official SWE-bench report places target instance in resolved_ids
$RUN_SUMMARY
ContextGraph MCP tools available under prefix mcp__cgreality__. Call mcp__cgreality__reality_latest_root first to confirm the active root, then mcp__cgreality__reality_attempt_summary with compact: true to inspect the most recent attempt unless full transcript readback is required.
Reality-boundary rules (deterministic deny list also applies):
- Never edit .env files
- Never edit swebench/ official evaluator
- Never edit tests/oracle_** benchmark fixtures
- Never edit crates/*/src/reward_specs/** or verifiers/**
- Never claim SWE-bench success unless official resolved_ids contains the target instance
EOF
)
fi

cgreality_emit_additional_context "SessionStart" "$CTX"
cgreality_log_event "SessionStart" "$(echo "$INPUT" | jq -c '{session_id, source}')"
exit 0
```

## 2. UserPromptSubmit — `scripts/cc/hook_user_prompt_submit.sh`

Fires before Claude processes each user prompt. Injects the latest trigger decision and reality-signal compact.

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

INPUT="$(cat)"
RUNTIME_ROOT="$(cgreality_runtime_root)"
TARGET_INSTANCE="$(cgreality_target_instance)"

CTX_PARTS=()

if [ -n "$RUNTIME_ROOT" ] && [ -n "$TARGET_INSTANCE" ]; then
  TASK_DIR="$RUNTIME_ROOT/$TARGET_INSTANCE"
  if [ -d "$TASK_DIR" ]; then
    LATEST_ATTEMPT="$(ls -d "$TASK_DIR"/attempt-* 2>/dev/null | sort -V | tail -n 1)"
    if [ -n "$LATEST_ATTEMPT" ] && [ -d "$LATEST_ATTEMPT" ]; then
      ATTEMPT_NUM="$(basename "$LATEST_ATTEMPT" | sed 's/attempt-//')"
      CTX_PARTS+=("Latest attempt: $ATTEMPT_NUM at $LATEST_ATTEMPT")

      if [ -f "$LATEST_ATTEMPT/trigger-decision.json" ]; then
        TRIG="$(jq -c '{status, should_run, policy, reasons: (.reasons // [] | map(.reason))}' "$LATEST_ATTEMPT/trigger-decision.json" 2>/dev/null || echo '{}')"
        CTX_PARTS+=("Trigger decision: $TRIG. Source of truth: file:$LATEST_ATTEMPT/trigger-decision.json")
      fi

      if [ -f "$LATEST_ATTEMPT/reality-signal-packet.json" ]; then
        SIGNAL="$(jq -c '{
          failure_digest: (.failure_digest // {}),
          official_progress: (.official_test_progress // {}),
          patch_signals: (.patch_signals // {}),
          score_reality: (.score_reality // {}),
          reinjection_directive: (.reinjection_directive // null)
        }' "$LATEST_ATTEMPT/reality-signal-packet.json" 2>/dev/null || echo '{}')"
        CTX_PARTS+=("Reality signal compact: $SIGNAL. Source of truth: file:$LATEST_ATTEMPT/reality-signal-packet.json")
      fi
    fi
  fi
fi

if [ ${#CTX_PARTS[@]} -eq 0 ]; then
  CTX="No active reality-loop attempt to drip. Call mcp__cgreality__reality_run_smoke or mcp__cgreality__reality_run_attempt to produce reality."
else
  # Append the last 3 shift-log entries for THIS Claude Code session so the agent
  # always perceives how reality is shifting under its own actions (reality-shift
  # principle: docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md).
  SESSION_ID="$(echo "$INPUT" | jq -r '.session_id // ""')"
  if [ -n "$SESSION_ID" ] && [ -n "$RUNTIME_ROOT" ]; then
    SHIFT_LOG="$RUNTIME_ROOT/cgreality-shift-log/${SESSION_ID}.jsonl"
    if [ -f "$SHIFT_LOG" ]; then
      RECENT_SHIFTS="$(tail -n 3 "$SHIFT_LOG" 2>/dev/null | jq -sc 'map({
        shift_id, timestamp_unix_ns, tool_name,
        subject: (.subject // {}),
        delta: (.delta_summary // {}),
        verification: (.verification // {})
      })' 2>/dev/null || echo '[]')"
      CTX_PARTS+=("Last 3 reality shifts in this session: $RECENT_SHIFTS. Source of truth: file:$SHIFT_LOG")
    fi
  fi
  CTX="$(printf '%s\n\n' "${CTX_PARTS[@]}")"
  # Cap at ~9 KB to leave headroom under Claude Code's 10 KB context limit.
  CTX="$(printf '%s' "$CTX" | head -c 9216)"
fi

cgreality_emit_additional_context "UserPromptSubmit" "$CTX"
cgreality_log_event "UserPromptSubmit" "{}"
exit 0
```

## 3. PreToolUse Edit/Write — `scripts/cc/hook_pre_tool_use_edit.sh`

Fires before any `Edit` or `Write` tool call. Computes the file's current SHA, checks the governed-paths policy, and either allows + injects context or denies.

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

INPUT="$(cat)"
TOOL_NAME="$(echo "$INPUT" | jq -r '.tool_name // ""')"
FILE_PATH="$(echo "$INPUT" | jq -r '.tool_input.file_path // ""')"
GOVERNED_PATHS="${CGREALITY_GOVERNED_PATHS:-$CONTEXTGRAPH_ROOT/config/ccreality_governed_paths.json}"

if [ -z "$FILE_PATH" ]; then
  cgreality_emit_pretool_decision "allow" "no file_path supplied" ""
  exit 0
fi

# Normalize: project-relative if possible.
ROOT="${CONTEXTGRAPH_ROOT:-/home/cabdru/contextgraph}"
REL_PATH="${FILE_PATH#$ROOT/}"

# Deterministic deny matches (covered by settings.json deny too, but we add path-pattern coverage here)
case "$REL_PATH" in
  .env|.env.*|*/.env|swebench/*|tests/oracle_*/*|crates/context-graph-benchmark/datasets/swe_bench_*|crates/*/src/reward_specs/*|crates/*/src/verifiers/*)
    cgreality_emit_pretool_decision "deny" "ccreality reality-boundary deny: path $REL_PATH is in the protected set (oracle/eval/reward/verifier/.env)." ""
    cgreality_log_event "PreToolUseEditDeny" "{\"path\":\"$REL_PATH\"}"
    exit 0
    ;;
esac

# Allow but inject current SHA + recent edit history if any.
SHA="missing"
if [ -f "$FILE_PATH" ]; then
  SHA="sha256:$(sha256sum "$FILE_PATH" | awk '{print $1}')"
fi

CTX="Pre-edit reality: file=$REL_PATH current_sha=$SHA. Governed-path policy: allow."
cgreality_emit_pretool_decision "allow" "ccreality governed path" "$CTX"
cgreality_log_event "PreToolUseEditAllow" "{\"path\":\"$REL_PATH\",\"sha\":\"$SHA\"}"
exit 0
```

## 4. PostToolUse Edit/Write — `scripts/cc/hook_post_tool_use_edit.sh`

Fires after a successful Edit/Write. Recomputes SHA, runs `cargo check` if a Rust file under `crates/context-graph-cli/` changed, captures `git diff --stat`, and writes a harness-transition record by shelling to a CLI helper.

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

INPUT="$(cat)"
TOOL_NAME="$(echo "$INPUT" | jq -r '.tool_name // ""')"
FILE_PATH="$(echo "$INPUT" | jq -r '.tool_input.file_path // ""')"
TOOL_USE_ID="$(echo "$INPUT" | jq -r '.tool_use_id // ""')"
ROOT="${CONTEXTGRAPH_ROOT:-/home/cabdru/contextgraph}"

if [ -z "$FILE_PATH" ] || [ ! -f "$FILE_PATH" ]; then
  cgreality_emit_additional_context "PostToolUse" "Post-edit reality: no file path or file missing post-edit."
  exit 0
fi

REL_PATH="${FILE_PATH#$ROOT/}"
NEW_SHA="sha256:$(sha256sum "$FILE_PATH" | awk '{print $1}')"

DIFF_STAT="$(cd "$ROOT" && git diff --stat -- "$REL_PATH" 2>/dev/null | tail -n 1 || echo 'no diff')"

CARGO_LINE=""
if [[ "$REL_PATH" == crates/context-graph-cli/* && "$REL_PATH" == *.rs ]]; then
  if (cd "$ROOT" && timeout 60 cargo check -p context-graph-cli --bin reality-loop --quiet 2>/tmp/cgreality-cargo-check.stderr); then
    CARGO_LINE="cargo check -p context-graph-cli --bin reality-loop: PASS"
  else
    CARGO_TAIL="$(tail -c 1024 /tmp/cgreality-cargo-check.stderr 2>/dev/null || echo '')"
    CARGO_LINE="cargo check -p context-graph-cli --bin reality-loop: FAIL. stderr_tail: $CARGO_TAIL"
  fi
fi

# Mint a harness-transition record via the cgreality MCP CLI helper if available.
TRANSITION_PATH=""
if command -v context-graph-cli >/dev/null 2>&1; then
  TRANSITION_JSON="$(context-graph-cli ccreality record-harness-transition \
    --tool-use-id "$TOOL_USE_ID" \
    --file "$REL_PATH" \
    --new-sha "$NEW_SHA" \
    --diff-stat "$DIFF_STAT" \
    --cargo-result "$CARGO_LINE" \
    --format json)"
  TRANSITION_PATH="$(printf '%s' "$TRANSITION_JSON" | jq -r '.transition_path')"
else
  cgreality_log_event "HarnessTransitionRecorderMissing" "{\"tool_use_id\":\"$TOOL_USE_ID\",\"file\":\"$REL_PATH\"}"
fi

CTX="Post-edit reality: file=$REL_PATH new_sha=$NEW_SHA. Diff stat: $DIFF_STAT."
if [ -n "$CARGO_LINE" ]; then CTX="$CTX $CARGO_LINE"; fi
if [ -n "$TRANSITION_PATH" ]; then CTX="$CTX harness_transition: file:$TRANSITION_PATH"; fi

cgreality_emit_additional_context "PostToolUse" "$CTX"
cgreality_log_event "PostToolUseEdit" "{\"path\":\"$REL_PATH\",\"new_sha\":\"$NEW_SHA\"}"
exit 0
```

> **Implementation note:** `context-graph-cli ccreality record-harness-transition` is a new thin subcommand that calls into the same shared `governed_edit` module the MCP `harness_apply_line_window_edit` tool uses. Either via this CLI or via direct file write — implementation choice during the build phase.

## 5. PreCompact — `scripts/cc/hook_pre_compact.sh`

Re-injects the same payload as SessionStart so that post-compaction context retains the runtime root and the latest reality summary.

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

INPUT="$(cat)"
RUNTIME_ROOT="$(cgreality_runtime_root)"
TARGET_INSTANCE="$(cgreality_target_instance)"

CTX="(post-compaction reality re-injection) "
if [ -n "$RUNTIME_ROOT" ]; then
  CTX="$CTX runtime_root=$RUNTIME_ROOT target_instance=${TARGET_INSTANCE:-unset}. Reality-boundary rules unchanged."
else
  CTX="$CTX no active runtime root."
fi

cgreality_emit_additional_context "PreCompact" "$CTX"
cgreality_log_event "PreCompact" "{}"
exit 0
```

## 6. Stop — `scripts/cc/hook_stop.sh`

The only hook that intentionally blocks. Refuses to let the turn end until the optimizer has called `mcp__cgreality__optimizer_record_recommendation` in this session.

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

INPUT="$(cat)"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id // ""')"
TRANSCRIPT_PATH="$(echo "$INPUT" | jq -r '.transcript_path // ""')"
RUNTIME_ROOT="$(cgreality_runtime_root)"

# If there's no active runtime root, this is just a normal session, allow stop.
if [ -z "$RUNTIME_ROOT" ]; then
  exit 0
fi

# Look for a recommendation artifact created in this session.
RECOMMENDED=0
if [ -d "$RUNTIME_ROOT" ]; then
  if find "$RUNTIME_ROOT" -path '*/claude-code-optimizer/recommendation-*.json' -newer "${HOME}/.cache/cgreality/last_session_marker" 2>/dev/null | grep -q .; then
    RECOMMENDED=1
  fi
fi

# Alternative: parse transcript for the MCP tool call.
if [ "$RECOMMENDED" -eq 0 ] && [ -f "$TRANSCRIPT_PATH" ]; then
  if grep -q 'mcp__cgreality__optimizer_record_recommendation' "$TRANSCRIPT_PATH" 2>/dev/null; then
    RECOMMENDED=1
  fi
fi

if [ "$RECOMMENDED" -eq 1 ]; then
  touch "${HOME}/.cache/cgreality/last_session_marker"
  cgreality_log_event "StopAllowed" "{}"
  exit 0
fi

# Block.
cgreality_emit_block "ccreality Stop hook: this optimization turn has not yet recorded a structured recommendation. Call mcp__cgreality__optimizer_record_recommendation with the schema in docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md before ending the turn. If no harness change is warranted, record a recommendation with status=\"no_change\" and a brief reason."
cgreality_log_event "StopBlocked" "{}"
exit 0
```

## Hook safety rules

1. **Hook scripts must never exit non-zero except for intentional blocks.** Use `set -euo pipefail`; when a hook observes a non-blocking failure, capture the exact command, exit code, stderr tail, and source-of-truth path inspected in `cgreality_log_event` before returning a JSON envelope.
2. **Hook scripts must never write to stdout outside the JSON envelope.** Any debug print corrupts the hook output. Use `>&2` or `cgreality_log_event`.
3. **Hook scripts must never spawn anything long-running synchronously.** Cap `cargo check` at 60s. Other spawns at 10–30s.
4. **Hook scripts must never read or write files outside the project tree without explicit cause.** They may read `~/.cache/cgreality/*` and write `~/.cache/cgreality/last_session_marker`.
5. **Hook scripts must be idempotent.** A re-fired hook (e.g. session resume) should not corrupt state.

## Bootstrap

The hooks reference `${CONTEXTGRAPH_ROOT}` from `.claude/settings.json` env. They also fall back to `/home/cabdru/contextgraph` if unset. They look for `context-graph-cli` on `$PATH` and fall back to `target/release` then `target/debug`.

## Test coverage

For each hook script, add a bash-level smoke test under `scripts/cc/tests/`:
- Run with synthetic stdin JSON.
- Assert exit code matches expectation.
- Assert stdout JSON matches expected `hookSpecificOutput`.
- Assert log file gets a row.

These run in CI as part of the FSV plan.
