# Phase 6 — File Specifications

The bash for the 6 hook scripts and `_common.sh` is in [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md). This doc only specifies the unit-test bodies (which weren't in 07_HOOKS_SPEC).

## Hook scripts (reference [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md))

| Phase 6 file | Source in 07_HOOKS_SPEC.md |
|--------------|---------------------------|
| `scripts/cc/_common.sh` | Section "Common helpers" |
| `scripts/cc/hook_session_start.sh` | Section "1. SessionStart" |
| `scripts/cc/hook_user_prompt_submit.sh` | Section "2. UserPromptSubmit" (the version with the shift-log block) |
| `scripts/cc/hook_pre_tool_use_edit.sh` | Section "3. PreToolUse Edit/Write" |
| `scripts/cc/hook_post_tool_use_edit.sh` | Section "4. PostToolUse Edit/Write" |
| `scripts/cc/hook_pre_compact.sh` | Section "5. PreCompact" |
| `scripts/cc/hook_stop.sh` | Section "6. Stop" |

Copy verbatim. Adjust ONLY:
- The shebang (use `#!/usr/bin/env bash`).
- Make sure each script `source`s `_common.sh` from a path-resolved location:
  ```bash
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  source "$SCRIPT_DIR/_common.sh"
  ```

## Hook unit tests

### `scripts/cc/tests/hooks/test_session_start.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common_helpers.sh"

# Test 1: with no active runtime root
ORIG_ACTIVE="$HOME/.cache/cgreality/active_runtime_root"
BACKUP=""
if [ -f "$ORIG_ACTIVE" ]; then BACKUP="$(cat "$ORIG_ACTIVE")"; rm -f "$ORIG_ACTIVE"; fi

OUT="$(echo '{"session_id":"test-sess-1","source":"startup"}' \
  | "$SCRIPT_DIR/../../hook_session_start.sh")"
EC=$?
assert_exit 0 "$EC" "no_root_exit"
assert_jq "$OUT" '.hookSpecificOutput.hookEventName' "SessionStart" "no_root_event"
assert_jq "$OUT" '.hookSpecificOutput.additionalContext | contains("not currently running") or contains("Active reality-loop")' "true" "no_root_context"

# Restore active root
[ -n "$BACKUP" ] && echo "$BACKUP" > "$ORIG_ACTIVE"

# Test 2: with a real smoke runtime root
TMPROOT="$(mktempdir)"
RDIR="$(setup_real_smoke_runtime_root "$TMPROOT" "hook-session-start")"
echo "$TMPROOT" > "$ORIG_ACTIVE"
echo "cgreality-smoke-hook-session-start" > "$HOME/.cache/cgreality/active_target_instance"

OUT="$(echo '{"session_id":"test-sess-2","source":"startup"}' \
  | "$SCRIPT_DIR/../../hook_session_start.sh")"
EC=$?
assert_exit 0 "$EC" "with_root_exit"
assert_jq "$OUT" '.hookSpecificOutput.additionalContext | contains("Active reality-loop runtime root")' "true" "with_root_context"

# Cleanup
rm -rf "$TMPROOT"
[ -n "$BACKUP" ] && echo "$BACKUP" > "$ORIG_ACTIVE" || rm -f "$ORIG_ACTIVE"
echo "test_session_start.sh: PASS"
```

### `scripts/cc/tests/hooks/test_user_prompt_submit.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common_helpers.sh"

TMPROOT="$(mktempdir)"
RDIR="$(setup_real_smoke_runtime_root "$TMPROOT" "hook-user-prompt")"
ORIG_ACTIVE="$HOME/.cache/cgreality/active_runtime_root"
ORIG_TARGET="$HOME/.cache/cgreality/active_target_instance"
BACKUP_R=""; BACKUP_T=""
[ -f "$ORIG_ACTIVE" ] && BACKUP_R="$(cat "$ORIG_ACTIVE")"
[ -f "$ORIG_TARGET" ] && BACKUP_T="$(cat "$ORIG_TARGET")"
echo "$TMPROOT" > "$ORIG_ACTIVE"
echo "cgreality-smoke-hook-user-prompt" > "$ORIG_TARGET"

OUT="$(echo '{"session_id":"test-sess-3","prompt":"hello","permission_mode":"acceptEdits"}' \
  | "$SCRIPT_DIR/../../hook_user_prompt_submit.sh")"
EC=$?
assert_exit 0 "$EC" "ups_exit"
assert_jq "$OUT" '.hookSpecificOutput.hookEventName' "UserPromptSubmit" "ups_event"
assert_jq "$OUT" '.hookSpecificOutput.additionalContext | contains("Trigger decision") or contains("No active reality")' "true" "ups_context"

rm -rf "$TMPROOT"
[ -n "$BACKUP_R" ] && echo "$BACKUP_R" > "$ORIG_ACTIVE" || rm -f "$ORIG_ACTIVE"
[ -n "$BACKUP_T" ] && echo "$BACKUP_T" > "$ORIG_TARGET" || rm -f "$ORIG_TARGET"
echo "test_user_prompt_submit.sh: PASS"
```

### `scripts/cc/tests/hooks/test_pre_tool_use_edit.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common_helpers.sh"

# Test 1: allow on a benign project file
OUT="$(echo "{\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"/home/cabdru/contextgraph/docs/ccreality/00_INDEX.md\"}}" \
  | "$SCRIPT_DIR/../../hook_pre_tool_use_edit.sh")"
EC=$?
assert_exit 0 "$EC" "allow_exit"
assert_jq "$OUT" '.hookSpecificOutput.permissionDecision' "allow" "allow_decision"

# Test 2: deny on .env
OUT="$(echo "{\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"/home/cabdru/contextgraph/.env\"}}" \
  | "$SCRIPT_DIR/../../hook_pre_tool_use_edit.sh")"
EC=$?
assert_exit 0 "$EC" "deny_exit"
assert_jq "$OUT" '.hookSpecificOutput.permissionDecision' "deny" "deny_decision"

# Test 3: deny on swebench/
OUT="$(echo "{\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"/home/cabdru/contextgraph/swebench/eval/runner.py\"}}" \
  | "$SCRIPT_DIR/../../hook_pre_tool_use_edit.sh")"
assert_jq "$OUT" '.hookSpecificOutput.permissionDecision' "deny" "swebench_deny"

echo "test_pre_tool_use_edit.sh: PASS"
```

### `scripts/cc/tests/hooks/test_post_tool_use_edit.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common_helpers.sh"

TMPFILE="$(mktemp /tmp/cgreality-test-edit-XXXXXX.txt)"
echo "test content" > "$TMPFILE"

OUT="$(echo "{\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$TMPFILE\"},\"tool_use_id\":\"test-tool-use\"}" \
  | "$SCRIPT_DIR/../../hook_post_tool_use_edit.sh")"
EC=$?
assert_exit 0 "$EC" "post_edit_exit"
assert_jq "$OUT" '.hookSpecificOutput.hookEventName' "PostToolUse" "post_edit_event"
assert_jq "$OUT" '.hookSpecificOutput.additionalContext | contains("Post-edit reality")' "true" "post_edit_context"

rm -f "$TMPFILE"
echo "test_post_tool_use_edit.sh: PASS"
```

### `scripts/cc/tests/hooks/test_pre_compact.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common_helpers.sh"

OUT="$(echo '{"trigger":"auto","current_context_size":120000}' \
  | "$SCRIPT_DIR/../../hook_pre_compact.sh")"
EC=$?
assert_exit 0 "$EC" "precompact_exit"
assert_jq "$OUT" '.hookSpecificOutput.hookEventName' "PreCompact" "precompact_event"
assert_jq "$OUT" '.hookSpecificOutput.additionalContext | contains("post-compaction reality re-injection")' "true" "precompact_context"

echo "test_pre_compact.sh: PASS"
```

### `scripts/cc/tests/hooks/test_stop.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common_helpers.sh"

# Test 1: no active runtime → exit 0 (no block)
ORIG_ACTIVE="$HOME/.cache/cgreality/active_runtime_root"
BACKUP=""
[ -f "$ORIG_ACTIVE" ] && BACKUP="$(cat "$ORIG_ACTIVE")"; rm -f "$ORIG_ACTIVE"
TMPTRANS="$(mktemp)"
echo '{}' > "$TMPTRANS"

OUT="$(echo "{\"session_id\":\"test\",\"transcript_path\":\"$TMPTRANS\",\"stop_reason\":\"end\"}" \
  | "$SCRIPT_DIR/../../hook_stop.sh"; echo "EXIT=$?")"
EC="$(echo "$OUT" | tail -1 | sed 's/EXIT=//')"
assert_exit 0 "$EC" "stop_no_root"

# Test 2: active smoke runtime, no recommendation → block (exit 0 with decision: block)
TMPROOT="$(mktempdir)"
RDIR="$(setup_real_smoke_runtime_root "$TMPROOT" "hook-stop-block")"
mkdir -p "$RDIR/claude-code-optimizer"
echo "$TMPROOT" > "$ORIG_ACTIVE"
echo "cgreality-smoke-hook-stop-block" > "$HOME/.cache/cgreality/active_target_instance"
OUT="$("$SCRIPT_DIR/../../hook_stop.sh" <<< "{\"session_id\":\"test-block\",\"transcript_path\":\"$TMPTRANS\",\"stop_reason\":\"end\"}")"
EC=$?
assert_jq "$OUT" '.decision' "block" "stop_block_decision"

# Test 3: with recommendation present → exit 0 (allow)
echo '{"status":"no_change","reason":"smoke"}' > "$RDIR/claude-code-optimizer/recommendation-turn-01.json"
echo 'mcp__cgreality__optimizer_record_recommendation' > "$TMPTRANS"
touch "$HOME/.cache/cgreality/last_session_marker"
OUT="$("$SCRIPT_DIR/../../hook_stop.sh" <<< "{\"session_id\":\"test-allow\",\"transcript_path\":\"$TMPTRANS\",\"stop_reason\":\"end\"}"; echo "EXIT=$?")"
EC="$(echo "$OUT" | tail -1 | sed 's/EXIT=//')"
assert_exit 0 "$EC" "stop_allow"

rm -f "$TMPTRANS"
rm -rf "$TMPROOT"
[ -n "$BACKUP" ] && echo "$BACKUP" > "$ORIG_ACTIVE" || rm -f "$ORIG_ACTIVE"

echo "test_stop.sh: PASS"
```

## Notes on the test bodies

The tests use real `reality-loop smoke` artifacts for runtime state and representative Claude Code hook stdin JSON for the trigger event. They focus on the contract: given a hook event, the hook produces a structured JSON envelope with the expected fields.

For more thorough integration testing, Phase 9's smoke launches actual Claude Code sessions and observes hook fires end-to-end.
