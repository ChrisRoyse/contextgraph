#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_emit_pretool_decision "deny" "ccreality hook input was not valid JSON" "Source of truth: stdin."
  exit 0
fi

TOOL_NAME="$(jq -r '.tool_name // ""' <<<"${INPUT}")"
FILE_PATH="$(jq -r '.tool_input.file_path // ""' <<<"${INPUT}")"
SESSION_ID="$(jq -r '.session_id // ""' <<<"${INPUT}")"
TOOL_USE_ID="$(jq -r '.tool_use_id // ""' <<<"${INPUT}")"
ROOT="${CONTEXTGRAPH_ROOT_RESOLVED}"
RUNTIME_ROOT="$(cgreality_runtime_root)"

if [ -z "${FILE_PATH}" ]; then
  cgreality_emit_pretool_decision "allow" "no file_path supplied" "No file path was present in tool_input; no governed-path verdict was possible."
  cgreality_log_event "PreToolUseEditNoPath" "$(jq -nc --arg tool_name "${TOOL_NAME}" '{tool_name: $tool_name}')"
  exit 0
fi

PATHS="$(cgreality_normalized_paths "${FILE_PATH}" "${ROOT}")"
ABS_PATH="$(printf '%s' "${PATHS}" | cut -f1)"
REL_PATH="$(printf '%s' "${PATHS}" | cut -f2)"
VERDICT="$(cgreality_policy_verdict "${ABS_PATH}" "${REL_PATH}")"
DECISION="$(jq -r '.decision' <<<"${VERDICT}")"
REASON="$(jq -r '.reason' <<<"${VERDICT}")"
POLICY_SOURCE="$(jq -r '.source_of_truth // ""' <<<"${VERDICT}")"

SHA="$(cgreality_sha256_or_missing "${ABS_PATH}")"
LINE_COUNT="$(cgreality_line_count_or_zero "${ABS_PATH}")"

if [ "${DECISION}" = "deny" ]; then
  CTX="Pre-edit reality: file=${REL_PATH} current_sha=${SHA}. Governed-path policy: deny. ${REASON}. Policy source: ${POLICY_SOURCE}."
  cgreality_emit_pretool_decision "deny" "${REASON}" "${CTX}"
  cgreality_log_event "PreToolUseEditDeny" "$(jq -nc --arg path "${REL_PATH}" --arg sha "${SHA}" --arg reason "${REASON}" '{path: $path, sha256: $sha, reason: $reason}')"
  exit 0
fi

STATE_PATH=""
if [ -n "${RUNTIME_ROOT}" ] && [ -d "${RUNTIME_ROOT}" ]; then
  STATE_PATH="$(cgreality_preedit_state_path "${RUNTIME_ROOT}" "${SESSION_ID}" "${TOOL_USE_ID}")"
  STATE_SOURCE_OF_TRUTH="$(cgreality_file_sot "${STATE_PATH}")"
  mkdir -p "$(dirname "${STATE_PATH}")"
  jq -nc \
    --arg record_kind "ccreality_pretool_edit_state" \
    --arg session_id "${SESSION_ID}" \
    --arg tool_use_id "${TOOL_USE_ID}" \
    --arg tool_name "${TOOL_NAME}" \
    --arg file_path "${FILE_PATH}" \
    --arg abs_path "${ABS_PATH}" \
    --arg rel_path "${REL_PATH}" \
    --arg sha "${SHA}" \
    --argjson line_count "${LINE_COUNT}" \
    --arg policy_source "${POLICY_SOURCE}" \
    --arg created_at "$(date -u +%FT%TZ)" \
    --arg source_of_truth "${STATE_SOURCE_OF_TRUTH}" '{
      record_kind: $record_kind,
      schema_version: 1,
      session_id: $session_id,
      tool_use_id: $tool_use_id,
      tool_name: $tool_name,
      file_path: $file_path,
      abs_path: $abs_path,
      rel_path: $rel_path,
      before_sha256: $sha,
      before_line_count: $line_count,
      policy_source: $policy_source,
      created_at: $created_at,
      source_of_truth: $source_of_truth
    }' > "${STATE_PATH}"
  if ! sync -f "${STATE_PATH}" 2>/dev/null; then
    cgreality_log_event "PreToolUseEditStateSyncFailed" "$(jq -nc --arg state_path "${STATE_PATH}" '{state_path: $state_path}')"
  fi
fi

CTX="Pre-edit reality: file=${REL_PATH} current_sha=${SHA} current_line_count=${LINE_COUNT}. Governed-path policy: allow. ${REASON}. Policy source: ${POLICY_SOURCE}."
if [ -n "${STATE_PATH}" ]; then
  CTX="${CTX} Pre-edit state source of truth: $(cgreality_file_sot "${STATE_PATH}")."
fi

cgreality_emit_pretool_decision "allow" "ccreality governed path allow" "${CTX}"
cgreality_log_event "PreToolUseEditAllow" "$(jq -nc --arg path "${REL_PATH}" --arg sha "${SHA}" --arg state_path "${STATE_PATH}" '{path: $path, sha256: $sha, preedit_state_path: $state_path}')"
exit 0
