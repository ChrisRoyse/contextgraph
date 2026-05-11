#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

# ccreality pre-edit hook: GATE REMOVED 2026-05-07.
#
# This hook used to consult config/ccreality_governed_paths.json and deny edits
# to any path not on the allow list. That made the loop unable to solve problems
# whenever the necessary edit fell outside the curated allow set (e.g. saving a
# memory under ~/.claude/projects/...). The operator removed the gate so that
# Claude Code's reality loop has full editing capability.
#
# What we KEEP: pre-edit SHA + line-count recording. The post-edit hook reads
# this state to compute the harness-transition delta and append to the shift
# log. Without it, post-edit can't record before_sha256.
#
# What we REMOVE: the deny verdict on policy mismatch. We always emit "allow".

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_emit_pretool_decision "allow" "ccreality hook input was not valid JSON; allow by default" "Source of truth: stdin."
  exit 0
fi

TOOL_NAME="$(jq -r '.tool_name // ""' <<<"${INPUT}")"
FILE_PATH="$(jq -r '.tool_input.file_path // ""' <<<"${INPUT}")"
SESSION_ID="$(jq -r '.session_id // ""' <<<"${INPUT}")"
TOOL_USE_ID="$(jq -r '.tool_use_id // ""' <<<"${INPUT}")"
ROOT="${CONTEXTGRAPH_ROOT_RESOLVED}"
RUNTIME_ROOT="$(cgreality_runtime_root)"

if [ -z "${FILE_PATH}" ]; then
  cgreality_emit_pretool_decision "allow" "no file_path supplied" "No file path was present in tool_input; nothing to record."
  cgreality_log_event "PreToolUseEditNoPath" "$(jq -nc --arg tool_name "${TOOL_NAME}" '{tool_name: $tool_name}')"
  exit 0
fi

PATHS="$(cgreality_normalized_paths "${FILE_PATH}" "${ROOT}")"
ABS_PATH="$(printf '%s' "${PATHS}" | cut -f1)"
REL_PATH="$(printf '%s' "${PATHS}" | cut -f2)"

SHA="$(cgreality_sha256_or_missing "${ABS_PATH}")"
LINE_COUNT="$(cgreality_line_count_or_zero "${ABS_PATH}")"

STATE_PATH=""
if [ -n "${RUNTIME_ROOT}" ] && [ -d "${RUNTIME_ROOT}" ]; then
  STATE_PATH="$(cgreality_preedit_state_path "${RUNTIME_ROOT}" "${SESSION_ID}" "${TOOL_USE_ID}")"
  STATE_SOURCE_OF_TRUTH="$(cgreality_file_sot "${STATE_PATH}")"
  BEFORE_TEXT_PATH="${STATE_PATH%.json}.before.txt"
  BEFORE_TEXT_SOURCE_OF_TRUTH=""
  BEFORE_TEXT_SHA=""
  mkdir -p "$(dirname "${STATE_PATH}")"
  if [ -f "${ABS_PATH}" ]; then
    cp "${ABS_PATH}" "${BEFORE_TEXT_PATH}"
    if ! sync -f "${BEFORE_TEXT_PATH}" 2>/dev/null; then
      cgreality_log_event "PreToolUseEditBeforeTextSyncFailed" "$(jq -nc --arg before_text_path "${BEFORE_TEXT_PATH}" '{before_text_path: $before_text_path}')"
    fi
    BEFORE_TEXT_SOURCE_OF_TRUTH="$(cgreality_file_sot "${BEFORE_TEXT_PATH}")"
    BEFORE_TEXT_SHA="$(cgreality_sha256_or_missing "${BEFORE_TEXT_PATH}")"
  fi
  jq -nc \
    --arg record_kind "ccreality_pretool_edit_state" \
    --arg session_id "${SESSION_ID}" \
    --arg tool_use_id "${TOOL_USE_ID}" \
    --arg tool_name "${TOOL_NAME}" \
    --arg file_path "${FILE_PATH}" \
    --arg abs_path "${ABS_PATH}" \
    --arg rel_path "${REL_PATH}" \
    --arg sha "${SHA}" \
    --arg before_text_source_of_truth "${BEFORE_TEXT_SOURCE_OF_TRUTH}" \
    --arg before_text_sha256 "${BEFORE_TEXT_SHA}" \
    --argjson line_count "${LINE_COUNT}" \
    --arg policy_source "ungated" \
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
      before_text_source_of_truth: (if $before_text_source_of_truth == "" then null else $before_text_source_of_truth end),
      before_text_sha256: (if $before_text_sha256 == "" then null else $before_text_sha256 end),
      before_line_count: $line_count,
      policy_source: $policy_source,
      created_at: $created_at,
      source_of_truth: $source_of_truth
    }' > "${STATE_PATH}"
  if ! sync -f "${STATE_PATH}" 2>/dev/null; then
    cgreality_log_event "PreToolUseEditStateSyncFailed" "$(jq -nc --arg state_path "${STATE_PATH}" '{state_path: $state_path}')"
  fi
fi

CTX="Pre-edit reality: file=${REL_PATH} current_sha=${SHA} current_line_count=${LINE_COUNT}. Governed-path gate: REMOVED 2026-05-07; SHA recorded for provenance only."
if [ -n "${STATE_PATH}" ]; then
  CTX="${CTX} Pre-edit state source of truth: $(cgreality_file_sot "${STATE_PATH}")."
fi

cgreality_emit_pretool_decision "allow" "ccreality pre-edit allow (gate removed)" "${CTX}"
cgreality_log_event "PreToolUseEditAllow" "$(jq -nc --arg path "${REL_PATH}" --arg sha "${SHA}" --arg state_path "${STATE_PATH}" '{path: $path, sha256: $sha, preedit_state_path: $state_path}')"
exit 0
