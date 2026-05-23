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

ATTEMPT_DIR="$(setup_real_smoke_runtime_root "${TMPROOT}" "hook-post-tool")"
TARGET="$(active_target_from_attempt_dir "${ATTEMPT_DIR}")"
printf '%s\n' "${TMPROOT}" > "${ACTIVE_ROOT_FILE}"
printf '%s\n' "${TARGET}" > "${ACTIVE_TARGET_FILE}"

FAKE_CLI="${TMPROOT}/context-graph-cli"
FAKE_TRANSITION="${TMPROOT}/harness-transitions/round-001.json"
FAKE_SHIFT_LOG="${TMPROOT}/cgreality-shift-log/sess-post.jsonl"
cat > "${FAKE_CLI}" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [ "${FAKE_CLI_MODE:-success}" = "fail" ]; then
  printf 'synthetic record failure\n' >&2
  exit 42
fi
file_path=""
lines_added="0"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --file-path)
      file_path="$2"
      shift 2
      ;;
    --lines-added)
      lines_added="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
mkdir -p "$(dirname "${FAKE_TRANSITION}")" "$(dirname "${FAKE_SHIFT_LOG}")"
jq -nc --arg file_path "${file_path}" --argjson lines_added "${lines_added}" '{
  record_kind: "ccreality_harness_transition",
  file_path: $file_path,
  lines_added: $lines_added
}' > "${FAKE_TRANSITION}"
jq -nc '{tool_name:"optimizer_record_harness_transition"}' >> "${FAKE_SHIFT_LOG}"
printf '{"transition_path":"%s","shift_id":"shift-synthetic-001"}\n' "${FAKE_TRANSITION}"
SH
chmod +x "${FAKE_CLI}"
export CGREALITY_CONTEXT_GRAPH_CLI="${FAKE_CLI}"
export FAKE_TRANSITION FAKE_SHIFT_LOG

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

MISSING_ERR="${TMPROOT}/missing-preedit.err"
set +e
MISSING_OUT="$(printf '%s\n' '{"session_id":"sess-post","tool_use_id":"tool-post-missing-state","tool_name":"Edit","tool_input":{"file_path":"scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt"}}' \
  | "${SCRIPT_DIR}/../../hook_post_tool_use_edit.sh" 2>"${MISSING_ERR}")"
MISSING_EC=$?
set -e
assert_exit 1 "${MISSING_EC}" "missing_preedit_exit"
assert_jq "${MISSING_OUT}" '.hookSpecificOutput.additionalContext | contains("CCREALITY_PREEDIT_STATE_MISSING")' "true" "missing_preedit_context"
grep -q "CCREALITY_PREEDIT_STATE_MISSING" "${MISSING_ERR}"
jq -e 'select(.event == "PostToolUseEditPreStateMissing")' "${HOOK_LOG}" >/dev/null

echo "test_post_tool_use_edit.sh: PASS"
