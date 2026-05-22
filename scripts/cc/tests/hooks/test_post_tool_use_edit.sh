#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

TMPROOT="$(mktempdir)"
export CONTEXTGRAPH_DATA_ROOT="${TMPROOT}"
export CGREALITY_CACHE_DIR="${TMPROOT}/state/cgreality"
CGREALITY_CACHE="${CGREALITY_CACHE_DIR}"
ACTIVE_ROOT_FILE="${CGREALITY_CACHE}/active_runtime_root"
ACTIVE_TARGET_FILE="${CGREALITY_CACHE}/active_target_instance"
backup_active_state
TARGET_FILE="${SCRIPT_DIR}/fixtures/post_tool_use_target.txt"
ORIGINAL_CONTENT="$(cat "${TARGET_FILE}")"
restore_fixture() {
  printf '%s\n' "${ORIGINAL_CONTENT}" > "${TARGET_FILE}"
}
trap 'restore_fixture; rm -rf "${TMPROOT}"; restore_active_state' EXIT

build_ccreality_bins
ATTEMPT_DIR="$(setup_real_smoke_runtime_root "${TMPROOT}" "hook-post-tool")"
TARGET="$(active_target_from_attempt_dir "${ATTEMPT_DIR}")"
printf '%s\n' "${TMPROOT}" > "${ACTIVE_ROOT_FILE}"
printf '%s\n' "${TARGET}" > "${ACTIVE_TARGET_FILE}"

PRE_OUT="$(printf '%s\n' '{"session_id":"sess-post","tool_use_id":"tool-post-record","tool_name":"Edit","tool_input":{"file_path":"scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt"}}' \
  | "${SCRIPT_DIR}/../../hook_pre_tool_use_edit.sh")"
assert_jq "${PRE_OUT}" '.hookSpecificOutput.permissionDecision' "allow" "pre_allow_decision"

printf '%s\nphase6 post hook added line\n' "${ORIGINAL_CONTENT}" > "${TARGET_FILE}"

OUT="$(printf '%s\n' '{"session_id":"sess-post","tool_use_id":"tool-post-record","tool_name":"Edit","tool_input":{"file_path":"scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt"}}' \
  | "${SCRIPT_DIR}/../../hook_post_tool_use_edit.sh")"
EC=$?
assert_exit 0 "${EC}" "post_exit"
assert_jq "${OUT}" '.hookSpecificOutput.hookEventName' "PostToolUse" "post_event"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("Harness transition")' "true" "post_transition_context"

TRANSITION_PATH="$(find "${TMPROOT}" -path '*/harness-transitions/round-*.json' -type f -print -quit)"
assert_file "${TRANSITION_PATH}" "harness_transition_file"
assert_jq "$(cat "${TRANSITION_PATH}")" '.record_kind' "ccreality_harness_transition" "transition_record_kind"
assert_jq "$(cat "${TRANSITION_PATH}")" '.file_path' "scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt" "transition_file_path"
assert_jq "$(cat "${TRANSITION_PATH}")" '.lines_added' "1" "transition_lines_added"

SHIFT_LOG="${TMPROOT}/cgreality-shift-log/sess-post.jsonl"
assert_file "${SHIFT_LOG}" "post_shift_log"
jq -e 'select(.tool_name == "optimizer_record_harness_transition")' "${SHIFT_LOG}" >/dev/null

HOOK_LOG="${TMPROOT}/cgreality-hook-events.jsonl"
assert_file "${HOOK_LOG}" "post_hook_log"
jq -e 'select(.event == "PostToolUseEditRecorded")' "${HOOK_LOG}" >/dev/null

echo "test_post_tool_use_edit.sh: PASS"
