#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

backup_active_state
TMPROOT="$(mktempdir)"
trap 'rm -rf "${TMPROOT}"; restore_active_state' EXIT

ATTEMPT_DIR="$(setup_real_smoke_runtime_root "${TMPROOT}" "hook-pre-compact")"
TARGET="$(active_target_from_attempt_dir "${ATTEMPT_DIR}")"
printf '%s\n' "${TMPROOT}" > "${ACTIVE_ROOT_FILE}"
printf '%s\n' "${TARGET}" > "${ACTIVE_TARGET_FILE}"
mkdir -p "${TMPROOT}/cgreality-shift-log"
printf '%s\n' '{"shift_id":"shift-precompact-1","tool_name":"unit","delta_summary":{"artifact":"file:/tmp/example"}}' > "${TMPROOT}/cgreality-shift-log/sess-precompact.jsonl"

OUT="$(printf '%s\n' '{"session_id":"sess-precompact","trigger":"auto","current_context_size":120000}' \
  | "${SCRIPT_DIR}/../../hook_pre_compact.sh")"
EC=$?
assert_exit 0 "${EC}" "precompact_exit"
assert_jq "${OUT}" '.hookSpecificOutput.hookEventName' "PreCompact" "precompact_event"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("post-compaction reality re-injection")' "true" "precompact_context"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("Last 3 reality shifts")' "true" "precompact_shift_context"

HOOK_LOG="${TMPROOT}/cgreality-hook-events.jsonl"
assert_file "${HOOK_LOG}" "precompact_hook_log"
jq -e 'select(.event == "PreCompact")' "${HOOK_LOG}" >/dev/null

echo "test_pre_compact.sh: PASS"
