#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

backup_active_state
TMPROOT="$(mktempdir)"
trap 'rm -rf "${TMPROOT}"; restore_active_state' EXIT

ATTEMPT_DIR="$(setup_real_smoke_runtime_root "${TMPROOT}" "hook-user-prompt")"
TARGET="$(active_target_from_attempt_dir "${ATTEMPT_DIR}")"
printf '%s\n' "${TMPROOT}" > "${ACTIVE_ROOT_FILE}"
printf '%s\n' "${TARGET}" > "${ACTIVE_TARGET_FILE}"

OUT="$(printf '%s\n' '{"session_id":"sess-user-prompt","prompt":"inspect reality","permission_mode":"acceptEdits"}' \
  | "${SCRIPT_DIR}/../../hook_user_prompt_submit.sh")"
EC=$?
assert_exit 0 "${EC}" "ups_exit"
assert_jq "${OUT}" '.hookSpecificOutput.hookEventName' "UserPromptSubmit" "ups_event"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("Attempt summary compact")' "true" "ups_attempt_context"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("Source of truth")' "true" "ups_sot_context"

HOOK_LOG="${TMPROOT}/cgreality-hook-events.jsonl"
assert_file "${HOOK_LOG}" "user_prompt_hook_log"
jq -e 'select(.event == "UserPromptSubmit")' "${HOOK_LOG}" >/dev/null

echo "test_user_prompt_submit.sh: PASS"
