#!/usr/bin/env bash
set -euo pipefail

ROOT="${CLAUDE_PROJECT_DIR:-${CONTEXTGRAPH_ROOT:-/home/cabdru/contextgraph}}"
# shellcheck source=scripts/cc/_common.sh
source "${ROOT}/scripts/cc/_common.sh"

DROOT="${CGREALITY_DROOT:-/mnt/d/contextgraph}"
CAPTURE_LOG="${CGREALITY_POSTTOOLUSE_CAPTURE_LOG:-${DROOT}/runtime/posttooluse-mejepa-capture.jsonl}"

posttooluse_capture_log_event() {
  local event="$1"
  local payload="${2:-}"
  if [ -z "${payload}" ]; then
    payload="{}"
  fi
  if ! mkdir -p "$(dirname "${CAPTURE_LOG}")"; then
    printf '{"status":"error","error_code":"MEJEPA_CAPTURE_LOG_DIR_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${CAPTURE_LOG}")" >&2
    return 0
  fi
  local line
  if jq -e . >/dev/null 2>&1 <<<"${payload}"; then
    line="$(jq -nc --arg ts "$(date -u +%FT%TZ)" --arg ev "${event}" --argjson payload "${payload}" '{
      timestamp: $ts,
      event: $ev,
      payload: $payload
    }')"
  else
    line="$(jq -nc --arg ts "$(date -u +%FT%TZ)" --arg ev "${event}" --arg payload "${payload}" '{
      timestamp: $ts,
      event: $ev,
      payload: $payload
    }')"
  fi
  if ! (
    flock -x 9
    printf '%s\n' "${line}" >> "${CAPTURE_LOG}"
    sync -f "${CAPTURE_LOG}" 2>/dev/null || true
  ) 9>"${CAPTURE_LOG}.lock"; then
    printf '{"status":"error","error_code":"MEJEPA_CAPTURE_LOG_WRITE_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${CAPTURE_LOG}")" >&2
  fi
}

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_droot_log_event "PostToolUseBashTestCaptureInvalidJson" '{"status":"invalid_json"}'
  posttooluse_capture_log_event "PostToolUseBashTestCaptureInvalidJson" '{"status":"invalid_json"}'
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
INFER_DB="${CONTEXTGRAPH_MEJEPA_INFER_DB:-/mnt/d/contextgraph/storage/contextgraph-rocksdb}"
VERIFY_TIMEOUT_SECONDS="${CGREALITY_MEJEPA_VERIFY_TIMEOUT_SECONDS:-2}"

OUTPUT="$(jq -r '
  [
    (.tool_response.stdout? // empty),
    (.tool_response.stderr? // empty),
    (.tool_response.output? // empty),
    (if (.tool_response? | type) == "string" then .tool_response else empty end),
    (.tool_response.content[]?.text? // empty)
  ] | map(select(type == "string" and length > 0)) | join("\n")
' <<<"${INPUT}")"

is_test_runner_command() {
  local lowered
  lowered="$(printf '%s' "${COMMAND}" | tr 'A-Z' 'a-z')"
  [[ "${lowered}" == *pytest* || "${lowered}" == *"cargo test"* || "${lowered}" == *unittest* || "${lowered}" == *jest* || "${lowered}" == *vitest* || "${lowered}" == *"npm test"* || "${lowered}" == *"pnpm test"* ]]
}

write_ambiguous_event() {
  local events_path="$1"
  local ts event_id
  ts="$(date -u +%s%3N)"
  event_id="$(printf '%s' "${SESSION_ID}|${TOOL_USE_ID}|${COMMAND}|${ts}|ambiguous" | sha256sum | awk '{print substr($1, 1, 32)}')"
  jq -nc \
    --arg event_id "${event_id}" \
    --argjson ts "${ts}" \
    --arg session_id "${SESSION_ID}" \
    --arg tool_use_id "${TOOL_USE_ID}" \
    --arg command "${COMMAND}" \
    '{
      event_id: $event_id,
      ts: $ts,
      session_id: $session_id,
      tool_use_id: $tool_use_id,
      command: $command,
      framework: "unknown",
      test_id: "ambiguous::no-clear-test-marker",
      outcome: "unknown",
      duration_ms: null,
      error_log: "",
      source: "claude_post_tool_use_bash_ambiguous",
      sequence: 0,
      line_no: 0,
      summary_count: null
    }' > "${events_path}"
}

append_events_to_outcome_log() {
  local events_path="$1"
  local out_dir out_path
  out_dir="${CGREALITY_TEST_OUTCOME_DIR:-/mnt/d/contextgraph/runtime/test-outcomes}"
  out_path="${out_dir}/${SESSION_ID}.jsonl"
  mkdir -p "${out_dir}"
  (
    flock -x 9
    cat "${events_path}" >> "${out_path}"
    sync -f "${out_path}" 2>/dev/null || true
  ) 9>"${out_path}.lock"
  printf '%s' "${out_path}"
}

edited_file_args() {
  local runtime_root shift_log
  runtime_root="$(cgreality_runtime_root)"
  if [ -z "${runtime_root}" ]; then
    return 0
  fi
  shift_log="${runtime_root}/cgreality-shift-log/$(cgreality_safe_id "${SESSION_ID}").jsonl"
  if [ ! -f "${shift_log}" ]; then
    return 0
  fi
  tail -n 50 "${shift_log}" | jq -r '
    select(length > 0)
    | fromjson?
    | [
        .subject.file_path?,
        .subject.path?,
        .delta_summary.file_path?,
        .harness_transition.file_path?
      ][]
    | select(type == "string" and length > 0)
  ' | sort -u | sed 's/^/--edited-file\n/'
}

run_prediction_binding() {
  local events_path="$1"
  local evidence_path="$2"
  local ambiguous="$3"
  local cli_bin
  if ! cli_bin="$(cgreality_context_graph_cli_bin)"; then
    printf '{"status":"warning","error_code":"MEJEPA_VERIFICATION_CLI_UNAVAILABLE","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${events_path}")" >&2
    cgreality_droot_log_event "PostToolUseBashVerificationCliUnavailable" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command}')"
    posttooluse_capture_log_event "PostToolUseBashVerificationCliUnavailable" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" --arg evidence_path "${evidence_path}" '{session_id: $session_id, command: $command, evidence_path: $evidence_path}')"
    return 0
  fi
  local args=(
    mejepa bind-test-outcomes
    --db-path "${INFER_DB}"
    --events-jsonl "${events_path}"
    --evidence-path "${evidence_path}"
  )
  while IFS= read -r item; do
    if [ -n "${item}" ]; then
      args+=("${item}")
    fi
  done < <(edited_file_args)
  if [ "${ambiguous}" = "true" ]; then
    args+=(--ambiguous --session-id "${SESSION_ID}" --tool-use-id "${TOOL_USE_ID}" --command "${COMMAND}")
  fi
  local bind_out bind_err exit_code
  bind_out="$(mktemp /tmp/cgreality-verification-bind-XXXXXX.out)"
  bind_err="$(mktemp /tmp/cgreality-verification-bind-XXXXXX.err)"
  if timeout "${VERIFY_TIMEOUT_SECONDS}" "${cli_bin}" "${args[@]}" >"${bind_out}" 2>"${bind_err}"; then
    local written
    written="$(jq -r '.verifications_written // 0' "${bind_out}" 2>/dev/null || printf '0')"
    cgreality_droot_log_event "PostToolUseBashVerificationBound" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" --argjson written "${written}" '{session_id: $session_id, command: $command, verifications_written: $written}')"
    posttooluse_capture_log_event "PostToolUseBashVerificationBound" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" --arg evidence_path "${evidence_path}" --argjson written "${written}" '{session_id: $session_id, command: $command, evidence_path: $evidence_path, verifications_written: $written}')"
  else
    exit_code=$?
    local tail_text code
    tail_text="$(tail -c 2048 "${bind_err}" 2>/dev/null || true)"
    code="MEJEPA_VERIFICATION_BIND_FAILED"
    if [ "${exit_code}" = "124" ]; then
      code="MEJEPA_HOOK_TIMEOUT"
    fi
    printf '{"status":"warning","error_code":"%s","detail":%s,"source_of_truth":"%s"}\n' "${code}" "$(jq -Rn --arg text "${tail_text}" '$text')" "$(cgreality_file_sot "${events_path}")" >&2
    cgreality_droot_log_event "PostToolUseBashVerificationBindFailed" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" --arg code "${code}" --arg detail "${tail_text}" '{session_id: $session_id, command: $command, error_code: $code, detail: $detail}')"
    posttooluse_capture_log_event "PostToolUseBashVerificationBindFailed" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" --arg code "${code}" --arg detail "${tail_text}" --arg evidence_path "${evidence_path}" '{session_id: $session_id, command: $command, error_code: $code, detail: $detail, evidence_path: $evidence_path}')"
  fi
  rm -f "${bind_out}" "${bind_err}"
}

TMP_EVENTS="$(mktemp /tmp/cgreality-test-outcomes-XXXXXX.jsonl)"
trap 'rm -f "${TMP_EVENTS}"' EXIT

if [ -z "${OUTPUT}" ]; then
  cgreality_droot_log_event "PostToolUseBashTestCaptureNoOutput" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command}')"
  posttooluse_capture_log_event "PostToolUseBashTestCaptureNoOutput" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command}')"
  if is_test_runner_command; then
    write_ambiguous_event "${TMP_EVENTS}"
    OUT_PATH="$(append_events_to_outcome_log "${TMP_EVENTS}")"
    run_prediction_binding "${TMP_EVENTS}" "${OUT_PATH}" "false"
    cgreality_emit_additional_context "PostToolUse" "Test outcome capture: wrote 1 inconclusive event to $(cgreality_file_sot "${OUT_PATH}")."
    posttooluse_capture_log_event "PostToolUseBashTestCaptureAmbiguousRecorded" "$(jq -nc --arg session_id "${SESSION_ID}" --arg path "${OUT_PATH}" '{session_id: $session_id, path: $path, event_count: 1}')"
  fi
  exit 0
fi

if ! printf '%s' "${OUTPUT}" | "${ROOT}/scripts/cc/parse_test_output.py" \
  --session-id "${SESSION_ID}" \
  --tool-use-id "${TOOL_USE_ID}" \
  --command "${COMMAND}" > "${TMP_EVENTS}"; then
  printf '{"status":"warning","error_code":"MEJEPA_VERIFICATION_PARSE_FAILURE","source_of_truth":"stdin"}\n' >&2
  cgreality_droot_log_event "PostToolUseBashTestCaptureParseFailed" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command}')"
  posttooluse_capture_log_event "PostToolUseBashTestCaptureParseFailed" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command}')"
  exit 0
fi

EVENT_COUNT="$(wc -l < "${TMP_EVENTS}" | tr -d ' ')"
if [ "${EVENT_COUNT}" = "0" ]; then
  if is_test_runner_command; then
    write_ambiguous_event "${TMP_EVENTS}"
    OUT_PATH="$(append_events_to_outcome_log "${TMP_EVENTS}")"
    run_prediction_binding "${TMP_EVENTS}" "${OUT_PATH}" "false"
    cgreality_emit_additional_context "PostToolUse" "Test outcome capture: wrote 1 inconclusive event to $(cgreality_file_sot "${OUT_PATH}")."
    posttooluse_capture_log_event "PostToolUseBashTestCaptureAmbiguousRecorded" "$(jq -nc --arg session_id "${SESSION_ID}" --arg path "${OUT_PATH}" '{session_id: $session_id, path: $path, event_count: 1}')"
  else
    posttooluse_capture_log_event "PostToolUseBashTestCaptureIgnored" "$(jq -nc --arg session_id "${SESSION_ID}" --arg command "${COMMAND}" '{session_id: $session_id, command: $command, reason: "no_test_outcome_events"}')"
  fi
  exit 0
fi

OUT_PATH="$(append_events_to_outcome_log "${TMP_EVENTS}")"

run_prediction_binding "${TMP_EVENTS}" "${OUT_PATH}" "false"
cgreality_emit_additional_context "PostToolUse" "Test outcome capture: wrote ${EVENT_COUNT} event(s) to $(cgreality_file_sot "${OUT_PATH}")."
cgreality_droot_log_event "PostToolUseBashTestCaptureRecorded" "$(jq -nc --arg session_id "${SESSION_ID}" --arg path "${OUT_PATH}" --argjson event_count "${EVENT_COUNT}" '{session_id: $session_id, path: $path, event_count: $event_count}')"
posttooluse_capture_log_event "PostToolUseBashTestCaptureRecorded" "$(jq -nc --arg session_id "${SESSION_ID}" --arg path "${OUT_PATH}" --argjson event_count "${EVENT_COUNT}" '{session_id: $session_id, path: $path, event_count: $event_count}')"
