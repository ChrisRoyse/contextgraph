#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

backup_active_state
TMPROOT=""
trap 'rm -rf "${TMPROOT}"; restore_active_state' EXIT

rm -f "${ACTIVE_ROOT_FILE}"
OUT="$(printf '%s\n' '{"session_id":"sess-session-start-empty","source":"startup"}' \
  | "${SCRIPT_DIR}/../../hook_session_start.sh")"
EC=$?
assert_exit 0 "${EC}" "no_root_exit"
assert_jq "${OUT}" '.hookSpecificOutput.hookEventName' "SessionStart" "no_root_event"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("not currently running")' "true" "no_root_context"

TMPROOT="$(mktempdir)"
ATTEMPT_DIR="$(setup_real_smoke_runtime_root "${TMPROOT}" "hook-session-start")"
TARGET="$(active_target_from_attempt_dir "${ATTEMPT_DIR}")"
printf '%s\n' "${TMPROOT}" > "${ACTIVE_ROOT_FILE}"
printf '%s\n' "${TARGET}" > "${ACTIVE_TARGET_FILE}"

OUT="$(printf '%s\n' '{"session_id":"sess-session-start-real","source":"startup","model":"claude-test"}' \
  | "${SCRIPT_DIR}/../../hook_session_start.sh")"
EC=$?
assert_exit 0 "${EC}" "with_root_exit"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("Active reality-loop runtime root")' "true" "with_root_context"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("Latest attempt summary")' "true" "summary_context"

HOOK_LOG="${TMPROOT}/cgreality-hook-events.jsonl"
assert_file "${HOOK_LOG}" "session_start_hook_log"
jq -e 'select(.event == "SessionStart")' "${HOOK_LOG}" >/dev/null

echo "test_session_start.sh: PASS"
