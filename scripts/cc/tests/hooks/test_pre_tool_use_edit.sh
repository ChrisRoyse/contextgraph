#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

backup_active_state
TMPROOT="$(mktempdir)"
trap 'rm -rf "${TMPROOT}"; restore_active_state' EXIT

ATTEMPT_DIR="$(setup_real_smoke_runtime_root "${TMPROOT}" "hook-pre-tool")"
TARGET="$(active_target_from_attempt_dir "${ATTEMPT_DIR}")"
printf '%s\n' "${TMPROOT}" > "${ACTIVE_ROOT_FILE}"
printf '%s\n' "${TARGET}" > "${ACTIVE_TARGET_FILE}"

OUT="$(printf '%s\n' '{"session_id":"sess-pre","tool_use_id":"tool-pre-allow","tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/docs/ccreality/00_INDEX.md"}}' \
  | "${SCRIPT_DIR}/../../hook_pre_tool_use_edit.sh")"
EC=$?
assert_exit 0 "${EC}" "allow_exit"
assert_jq "${OUT}" '.hookSpecificOutput.permissionDecision' "allow" "allow_decision"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("Pre-edit state source of truth")' "true" "allow_state_context"

STATE_PATH="${TMPROOT}/cgreality-hook-state/preedit/sess-pre/tool-pre-allow.json"
assert_file "${STATE_PATH}" "pretool_state_file"
assert_jq "$(cat "${STATE_PATH}")" '.record_kind' "ccreality_pretool_edit_state" "state_record_kind"
assert_jq "$(cat "${STATE_PATH}")" '.rel_path' "docs/ccreality/00_INDEX.md" "state_rel_path"

OUT="$(printf '%s\n' '{"session_id":"sess-pre","tool_use_id":"tool-pre-deny-env","tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/.env"}}' \
  | "${SCRIPT_DIR}/../../hook_pre_tool_use_edit.sh")"
EC=$?
assert_exit 0 "${EC}" "deny_env_exit"
assert_jq "${OUT}" '.hookSpecificOutput.permissionDecision' "deny" "deny_env_decision"
assert_jq "${OUT}" '.hookSpecificOutput.permissionDecisionReason | contains("deny matched")' "true" "deny_env_reason"

OUT="$(printf '%s\n' '{"session_id":"sess-pre","tool_use_id":"tool-pre-deny-swebench","tool_name":"Write","tool_input":{"file_path":"/home/cabdru/contextgraph/swebench/eval/runner.py"}}' \
  | "${SCRIPT_DIR}/../../hook_pre_tool_use_edit.sh")"
assert_jq "${OUT}" '.hookSpecificOutput.permissionDecision' "deny" "deny_swebench_decision"

HOOK_LOG="${TMPROOT}/cgreality-hook-events.jsonl"
assert_file "${HOOK_LOG}" "pretool_hook_log"
jq -e 'select(.event == "PreToolUseEditAllow")' "${HOOK_LOG}" >/dev/null
jq -e 'select(.event == "PreToolUseEditDeny")' "${HOOK_LOG}" >/dev/null

echo "test_pre_tool_use_edit.sh: PASS"
