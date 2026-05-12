#!/usr/bin/env bash
set -euo pipefail

ROOT="${CLAUDE_PROJECT_DIR:-${CONTEXTGRAPH_ROOT:-/home/cabdru/contextgraph}}"
# shellcheck source=scripts/cc/_common.sh
source "${ROOT}/scripts/cc/_common.sh"

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_droot_log_event "PostToolUseBashTestCaptureInvalidJson" '{"status":"invalid_json"}'
  exit 0
fi

TOOL_NAME="$(jq -r '.tool_name // ""' <<<"${INPUT}")"
if [ "${TOOL_NAME}" != "Bash" ]; then
  exit 0
fi

RAW_SESSION_ID="$(jq -r '.session_id // env.CLAUDE_SESSION_ID // ""' <<<"${INPUT}")"
SESSION_ID="$(cgreality_session_hex32 "${RAW_SESSION_ID}")"
TOOL_USE_ID="$(jq -r '.tool_use_id // ""' <<<"${INPUT}")"
COMMAND="$(jq -r '.tool_input.command // ""' <<<"${INPUT}")"

OUTPUT="$(jq -r '
  [
    (.tool_response.stdout? // empty),
    (.tool_response.stderr? // empty),
    (.tool_response.output? // empty),
    (if (.tool_response? | type) == "string" then .tool_response else empty end),
    (.tool_response.content[]?.text? // empty)
  ] | map(select(type == "string" and length > 0)) | join("\n")
' <<<"${INPUT}")"

if [ -z "${OUTPUT}" ]; then
  cgreality_droot_log_event "PostToolUseBashTestCaptureNoOutput" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command}')"
  exit 0
fi

TMP_EVENTS="$(mktemp /tmp/cgreality-test-outcomes-XXXXXX.jsonl)"
trap 'rm -f "${TMP_EVENTS}"' EXIT
if ! printf '%s' "${OUTPUT}" | "${ROOT}/scripts/cc/parse_test_output.py" \
  --session-id "${SESSION_ID}" \
  --tool-use-id "${TOOL_USE_ID}" \
  --command "${COMMAND}" > "${TMP_EVENTS}"; then
  cgreality_droot_log_event "PostToolUseBashTestCaptureParseFailed" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command}')"
  exit 0
fi

EVENT_COUNT="$(wc -l < "${TMP_EVENTS}" | tr -d ' ')"
if [ "${EVENT_COUNT}" = "0" ]; then
  exit 0
fi

OUT_DIR="${CGREALITY_TEST_OUTCOME_DIR:-/mnt/d/contextgraph/runtime/test-outcomes}"
OUT_PATH="${OUT_DIR}/${SESSION_ID}.jsonl"
mkdir -p "${OUT_DIR}"
(
  flock -x 9
  cat "${TMP_EVENTS}" >> "${OUT_PATH}"
  sync -f "${OUT_PATH}" 2>/dev/null || true
) 9>"${OUT_PATH}.lock"

cgreality_emit_additional_context "PostToolUse" "Test outcome capture: wrote ${EVENT_COUNT} event(s) to $(cgreality_file_sot "${OUT_PATH}")."
cgreality_droot_log_event "PostToolUseBashTestCaptureRecorded" "$(jq -nc --arg session_id "${SESSION_ID}" --arg path "${OUT_PATH}" --argjson event_count "${EVENT_COUNT}" '{session_id: $session_id, path: $path, event_count: $event_count}')"
