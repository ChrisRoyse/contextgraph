#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

backup_active_state
TMPROOT="$(mktempdir)"
TMPTRANS="$(mktemp /tmp/cgreality-hook-transcript-XXXXXX.jsonl)"
trap 'rm -f "${TMPTRANS}"; rm -rf "${TMPROOT}"; restore_active_state' EXIT

rm -f "${ACTIVE_ROOT_FILE}"
OUT="$("${SCRIPT_DIR}/../../hook_stop.sh" <<< "{\"session_id\":\"sess-stop-none\",\"transcript_path\":\"${TMPTRANS}\",\"stop_reason\":\"end\"}"; printf '\nEXIT=%s' "$?")"
EC="$(tail -n 1 <<<"${OUT}" | sed 's/EXIT=//')"
assert_exit 0 "${EC}" "stop_no_root_exit"

ATTEMPT_DIR="$(setup_real_smoke_runtime_root "${TMPROOT}" "hook-stop")"
TARGET="$(active_target_from_attempt_dir "${ATTEMPT_DIR}")"
printf '%s\n' "${TMPROOT}" > "${ACTIVE_ROOT_FILE}"
printf '%s\n' "${TARGET}" > "${ACTIVE_TARGET_FILE}"

OUT="$("${SCRIPT_DIR}/../../hook_stop.sh" <<< "{\"session_id\":\"sess-stop\",\"transcript_path\":\"${TMPTRANS}\",\"stop_reason\":\"end\",\"stop_hook_active\":false}")"
assert_jq "${OUT}" '.decision' "block" "stop_block_decision"
assert_jq "${OUT}" '.reason | contains("has not recorded a structured recommendation")' "true" "stop_block_reason"

REC_DIR="${ATTEMPT_DIR}/claude-code-optimizer"
mkdir -p "${REC_DIR}"
REC_PATH="${REC_DIR}/recommendation-turn-01.json"
jq -nc --arg session_id "sess-stop" --arg sot "file:${REC_PATH}" '{
  record_kind: "ccreality_optimizer_recommendation",
  schema_version: 1,
  claude_session_id: $session_id,
  status: "no_change",
  reason: "Phase 6 Stop hook unit test recommendation artifact.",
  source_of_truth: $sot
}' > "${REC_PATH}"

OUT="$("${SCRIPT_DIR}/../../hook_stop.sh" <<< "{\"session_id\":\"sess-stop\",\"transcript_path\":\"${TMPTRANS}\",\"stop_reason\":\"end\"}"; printf '\nEXIT=%s' "$?")"
EC="$(tail -n 1 <<<"${OUT}" | sed 's/EXIT=//')"
assert_exit 0 "${EC}" "stop_allow_exit"

assert_file "${REC_PATH}" "stop_recommendation_artifact"
assert_jq "$(cat "${REC_PATH}")" '.claude_session_id' "sess-stop" "stop_recommendation_session"
assert_file "${CGREALITY_CACHE}/last_session_marker" "stop_marker"

HOOK_LOG="${TMPROOT}/cgreality-hook-events.jsonl"
assert_file "${HOOK_LOG}" "stop_hook_log"
jq -e 'select(.event == "StopBlocked")' "${HOOK_LOG}" >/dev/null
jq -e 'select(.event == "StopAllowed")' "${HOOK_LOG}" >/dev/null

echo "test_stop.sh: PASS"
