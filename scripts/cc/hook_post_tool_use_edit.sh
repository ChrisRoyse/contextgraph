#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_emit_additional_context "PostToolUse" "Post-edit reality: ccreality hook input was not valid JSON; source_of_truth=stdin."
  exit 0
fi

TOOL_NAME="$(jq -r '.tool_name // ""' <<<"${INPUT}")"
FILE_PATH="$(jq -r '.tool_input.file_path // ""' <<<"${INPUT}")"
SESSION_ID="$(jq -r '.session_id // ""' <<<"${INPUT}")"
TOOL_USE_ID="$(jq -r '.tool_use_id // ""' <<<"${INPUT}")"
ROOT="${CONTEXTGRAPH_ROOT_RESOLVED}"
RUNTIME_ROOT="$(cgreality_runtime_root)"

if [ -z "${FILE_PATH}" ]; then
  cgreality_emit_additional_context "PostToolUse" "Post-edit reality: no file_path supplied; no transition recorded."
  cgreality_log_event "PostToolUseEditNoPath" "$(jq -nc --arg tool_name "${TOOL_NAME}" '{tool_name: $tool_name}')"
  exit 0
fi

PATHS="$(cgreality_normalized_paths "${FILE_PATH}" "${ROOT}")"
ABS_PATH="$(printf '%s' "${PATHS}" | cut -f1)"
REL_PATH="$(printf '%s' "${PATHS}" | cut -f2)"

if [ ! -f "${ABS_PATH}" ]; then
  CTX="Post-edit reality: file=${REL_PATH} is missing after tool use; no harness transition recorded."
  cgreality_emit_additional_context "PostToolUse" "${CTX}"
  cgreality_log_event "PostToolUseEditMissingFile" "$(jq -nc --arg path "${REL_PATH}" '{path: $path}')"
  exit 0
fi

AFTER_SHA="$(cgreality_sha256_or_missing "${ABS_PATH}")"
AFTER_LINES="$(cgreality_line_count_or_zero "${ABS_PATH}")"
BEFORE_SHA=""
BEFORE_LINES="0"
STATE_PATH=""

if [ -n "${RUNTIME_ROOT}" ] && [ -d "${RUNTIME_ROOT}" ]; then
  STATE_PATH="$(cgreality_preedit_state_path "${RUNTIME_ROOT}" "${SESSION_ID}" "${TOOL_USE_ID}")"
  if [ -f "${STATE_PATH}" ]; then
    BEFORE_SHA="$(jq -r '.before_sha256 // ""' "${STATE_PATH}")"
    BEFORE_LINES="$(jq -r '.before_line_count // 0' "${STATE_PATH}")"
  else
    CTX="Post-edit reality: ERROR CCREALITY_PREEDIT_STATE_MISSING. Expected PreToolUse state was not found at $(cgreality_file_sot "${STATE_PATH}"). File=${REL_PATH} after_sha=${AFTER_SHA}. Harness transition was not recorded because before_sha256 is required."
    cgreality_emit_additional_context "PostToolUse" "${CTX}"
    cgreality_log_event "PostToolUseEditPreStateMissing" "$(jq -nc --arg path "${REL_PATH}" --arg state_path "${STATE_PATH}" --arg after_sha "${AFTER_SHA}" '{path: $path, expected_state_path: $state_path, after_sha256: $after_sha}')"
    exit 0
  fi
else
  CTX="Post-edit reality: file=${REL_PATH} after_sha=${AFTER_SHA}. No active runtime root exists, so no harness transition was recorded. Active-root source of truth: $(cgreality_file_sot "${RUNTIME_ROOT_FILE}")."
  cgreality_emit_additional_context "PostToolUse" "${CTX}"
  cgreality_log_event "PostToolUseEditNoRuntimeRoot" "$(jq -nc --arg path "${REL_PATH}" --arg after_sha "${AFTER_SHA}" '{path: $path, after_sha256: $after_sha}')"
  exit 0
fi

LINES_ADDED=0
LINES_REMOVED=0
if [ "${AFTER_LINES}" -gt "${BEFORE_LINES}" ]; then
  LINES_ADDED=$((AFTER_LINES - BEFORE_LINES))
elif [ "${BEFORE_LINES}" -gt "${AFTER_LINES}" ]; then
  LINES_REMOVED=$((BEFORE_LINES - AFTER_LINES))
fi

DIFF_STAT="no git diff stat for path"
if [ -d "${ROOT}/.git" ]; then
  DIFF_STAT="$(cd "${ROOT}" && git diff --stat -- "${REL_PATH}" 2>/tmp/cgreality-post-hook-git-diff.stderr | tail -n 1 || true)"
  if [ -z "${DIFF_STAT}" ]; then
    DIFF_STAT="git diff stat empty for ${REL_PATH}"
  fi
fi

CARGO_STATUS=""
CARGO_STDERR_TAIL=""
if [[ "${REL_PATH}" == crates/context-graph-cli/* && "${REL_PATH}" == *.rs ]]; then
  CARGO_ERR="$(mktemp /tmp/cgreality-cargo-check-XXXXXX.stderr)"
  if (cd "${ROOT}" && timeout 60 cargo check -p context-graph-cli --no-default-features --lib --quiet 2>"${CARGO_ERR}"); then
    CARGO_STATUS="PASS"
  else
    CARGO_STATUS="FAIL"
    CARGO_STDERR_TAIL="$(tail -c 2048 "${CARGO_ERR}" 2>/dev/null || true)"
  fi
  rm -f "${CARGO_ERR}"
fi

CLI_BIN=""
if ! CLI_BIN="$(cgreality_context_graph_cli_bin)"; then
  CTX="Post-edit reality: ERROR CCREALITY_CONTEXT_GRAPH_CLI_MISSING. Expected executable at ${CGREALITY_CONTEXT_GRAPH_CLI:-${CONTEXTGRAPH_ROOT_RESOLVED}/target/debug/context-graph-cli}. File=${REL_PATH} after_sha=${AFTER_SHA}. Harness transition was not recorded."
  cgreality_emit_additional_context "PostToolUse" "${CTX}"
  cgreality_log_event "PostToolUseEditCliMissing" "$(jq -nc --arg path "${REL_PATH}" --arg expected "${CGREALITY_CONTEXT_GRAPH_CLI:-${CONTEXTGRAPH_ROOT_RESOLVED}/target/debug/context-graph-cli}" '{path: $path, expected_cli: $expected}')"
  exit 0
fi

CLI_ERR="$(mktemp /tmp/cgreality-record-transition-XXXXXX.stderr)"
TRANSITION_JSON=""
if TRANSITION_JSON="$(timeout 30 "${CLI_BIN}" ccreality record-harness-transition \
  --session-id "${SESSION_ID}" \
  --tool-use-id "${TOOL_USE_ID}" \
  --tool-name "${TOOL_NAME}" \
  --file-path "${REL_PATH}" \
  --before-sha256 "${BEFORE_SHA}" \
  --after-sha256 "${AFTER_SHA}" \
  --preedit-state-path "${STATE_PATH}" \
  --after-source-path "${ABS_PATH}" \
  --lines-added "${LINES_ADDED}" \
  --lines-removed "${LINES_REMOVED}" \
  --git-diff-stat "${DIFF_STAT}" \
  --cargo-status "${CARGO_STATUS}" \
  --cargo-stderr-tail "${CARGO_STDERR_TAIL}" \
  --format json-compact 2>"${CLI_ERR}")"; then
  if jq -e . >/dev/null 2>&1 <<<"${TRANSITION_JSON}"; then
    TRANSITION_PATH="$(jq -r '.transition_path // ""' <<<"${TRANSITION_JSON}")"
    SHIFT_ID="$(jq -r '.shift_id // ""' <<<"${TRANSITION_JSON}")"
    CTX="Post-edit reality: file=${REL_PATH} before_sha=${BEFORE_SHA} after_sha=${AFTER_SHA}. Lines added=${LINES_ADDED} removed=${LINES_REMOVED}. Diff stat: ${DIFF_STAT}. Harness transition: ${TRANSITION_PATH}. Shift id: ${SHIFT_ID}."
    if [ -n "${CARGO_STATUS}" ]; then
      CTX="${CTX} cargo check -p context-graph-cli --no-default-features --lib: ${CARGO_STATUS}."
    fi
    cgreality_emit_additional_context "PostToolUse" "${CTX}"
    cgreality_log_event "PostToolUseEditRecorded" "$(jq -nc --arg path "${REL_PATH}" --arg before_sha "${BEFORE_SHA}" --arg after_sha "${AFTER_SHA}" --arg transition_path "${TRANSITION_PATH}" --arg shift_id "${SHIFT_ID}" '{path: $path, before_sha256: $before_sha, after_sha256: $after_sha, transition_path: $transition_path, shift_id: $shift_id}')"
    rm -f "${CLI_ERR}"
    exit 0
  fi
  CLI_TAIL="context-graph-cli emitted non-JSON stdout: ${TRANSITION_JSON}"
else
  CLI_EXIT=$?
  CLI_TAIL="$(tail -c 2048 "${CLI_ERR}" 2>/dev/null || true)"
  CLI_TAIL="context-graph-cli exit=${CLI_EXIT}; stderr_tail=${CLI_TAIL}"
fi
rm -f "${CLI_ERR}"

CTX="Post-edit reality: ERROR CCREALITY_HARNESS_TRANSITION_RECORD_FAILED. File=${REL_PATH} before_sha=${BEFORE_SHA} after_sha=${AFTER_SHA}. ${CLI_TAIL}. Expected source of truth: active runtime root ${RUNTIME_ROOT}."
cgreality_emit_additional_context "PostToolUse" "${CTX}"
cgreality_log_event "PostToolUseEditRecordFailed" "$(jq -nc --arg path "${REL_PATH}" --arg before_sha "${BEFORE_SHA}" --arg after_sha "${AFTER_SHA}" --arg error_tail "${CLI_TAIL}" '{path: $path, before_sha256: $before_sha, after_sha256: $after_sha, error_tail: $error_tail}')"
exit 0
