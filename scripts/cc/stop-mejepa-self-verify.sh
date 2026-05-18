#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

LOG_PATH="${CGREALITY_STOP_MEJEPA_LOG:-${CGREALITY_DROOT:-/mnt/d/contextgraph}/runtime/stop-mejepa-self-verify.jsonl}"

stop_mejepa_log_event() {
  local event="$1"
  local payload="${2:-{}}"
  local line
  if jq -e . >/dev/null 2>&1 <<<"${payload}"; then
    line="$(jq -nc --arg ts "$(date -u +%FT%TZ)" --arg event "${event}" --argjson payload "${payload}" '{
      timestamp: $ts,
      event: $event,
      payload: $payload
    }')"
  else
    line="$(jq -nc --arg ts "$(date -u +%FT%TZ)" --arg event "${event}" --arg payload "${payload}" '{
      timestamp: $ts,
      event: $event,
      payload: $payload
    }')"
  fi
  if ! mkdir -p "$(dirname "${LOG_PATH}")"; then
    printf '{"error_code":"MEJEPA_STOP_HOOK_LOG_DIR_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${LOG_PATH}")" >&2
    return 0
  fi
  local lock="${LOG_PATH}.lock"
  if ! (
    flock -x 9
    printf '%s\n' "${line}" >> "${LOG_PATH}"
    sync -f "${LOG_PATH}" 2>/dev/null || true
  ) 9>"${lock}"; then
    printf '{"error_code":"MEJEPA_STOP_HOOK_LOG_WRITE_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${LOG_PATH}")" >&2
  fi
}

stop_mejepa_warn_unreachable() {
  local session_id="$1"
  local detail="$2"
  local stderr_tail="$3"
  local payload
  payload="$(jq -nc \
    --arg session_id "${session_id}" \
    --arg detail "${detail}" \
    --arg stderr_tail "${stderr_tail}" \
    --arg log_path "${LOG_PATH}" \
    --arg db_path "${CONTEXTGRAPH_MEJEPA_INFER_DB:-/mnt/d/contextgraph/storage/contextgraph-rocksdb}" \
    '{
      error_code: "MEJEPA_STOP_HOOK_DAEMON_UNREACHABLE",
      session_id: $session_id,
      detail: $detail,
      stderr_tail: $stderr_tail,
      source_of_truth: {
        db_path: $db_path,
        hook_log: $log_path
      }
    }')"
  stop_mejepa_log_event "MEJEPA_STOP_HOOK_DAEMON_UNREACHABLE" "${payload}"
  printf '%s\n' "${payload}" >&2
}

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  stop_mejepa_warn_unreachable "" "Stop hook received invalid JSON on stdin; cannot map current session." ""
  exit 0
fi

STOP_HOOK_ACTIVE="$(jq -r '.stop_hook_active // false' <<<"${INPUT}")"
SESSION_RAW="$(jq -r '.session_id // env.CLAUDE_SESSION_ID // ""' <<<"${INPUT}")"
SESSION_ID="$(cgreality_session_hex32 "${SESSION_RAW}")"

if [ "${STOP_HOOK_ACTIVE}" = "true" ]; then
  stop_mejepa_log_event "MEJEPA_STOP_HOOK_ACTIVE_BYPASS" "$(jq -nc --arg session_id "${SESSION_ID}" '{session_id: $session_id, reason: "stop_hook_active"}')"
  exit 0
fi

CONFIG_PATH="${MEJEPA_DAEMON_CONFIG:-${HOME:-/home/cabdru}/.config/mejepa/daemon.toml}"
if [ -f "${CONFIG_PATH}" ] && grep -Eq '^[[:space:]]*stop_hook_enabled[[:space:]]*=[[:space:]]*false([[:space:]]*(#.*)?)?$' "${CONFIG_PATH}"; then
  stop_mejepa_log_event "MEJEPA_STOP_HOOK_OPERATOR_DISABLED" "$(jq -nc --arg session_id "${SESSION_ID}" --arg config_path "${CONFIG_PATH}" '{session_id: $session_id, config_path: $config_path}')"
  exit 0
fi

if ! CLI_BIN="$(cgreality_context_graph_cli_bin)"; then
  stop_mejepa_warn_unreachable "${SESSION_ID}" "context-graph-cli executable not found." ""
  exit 0
fi

DB_PATH="${CONTEXTGRAPH_MEJEPA_INFER_DB:-/mnt/d/contextgraph/storage/contextgraph-rocksdb}"
MAX_PREDICTIONS="${CGREALITY_MEJEPA_STOP_VERIFY_MAX_PREDICTIONS:-50}"
TIMEOUT_SECONDS="${CGREALITY_MEJEPA_STOP_VERIFY_TIMEOUT_SECONDS:-6}"
TMP_OUT="$(mktemp /tmp/mejepa-stop-self-verify-out-XXXXXX.json)"
TMP_ERR="$(mktemp /tmp/mejepa-stop-self-verify-err-XXXXXX.log)"
trap 'rm -f "${TMP_OUT}" "${TMP_ERR}"' EXIT

set +e
timeout "${TIMEOUT_SECONDS}" "${CLI_BIN}" mejepa stop-self-verify \
  --db-path "${DB_PATH}" \
  --session-id "${SESSION_ID}" \
  --max-predictions "${MAX_PREDICTIONS}" \
  >"${TMP_OUT}" 2>"${TMP_ERR}"
CLI_EXIT=$?
set -e

CLI_STDERR="$(tail -c 4096 "${TMP_ERR}" 2>/dev/null || true)"
CLI_OUTPUT="$(cat "${TMP_OUT}" 2>/dev/null || true)"
if [ "${CLI_EXIT}" -ne 0 ] || ! jq -e . >/dev/null 2>&1 <<<"${CLI_OUTPUT}"; then
  stop_mejepa_warn_unreachable "${SESSION_ID}" "stop-self-verify command failed or returned invalid JSON; exit=${CLI_EXIT}." "${CLI_STDERR}"
  exit 0
fi

UNVERIFIED_COUNT="$(jq -r '.unverified_count // 0' <<<"${CLI_OUTPUT}")"
BLOCK="$(jq -r '.block // false' <<<"${CLI_OUTPUT}")"
if [ "${BLOCK}" = "true" ] && [ "${UNVERIFIED_COUNT}" != "0" ]; then
  stop_mejepa_log_event "MEJEPA_STOP_HOOK_UNVERIFIED_PREDICTIONS" "${CLI_OUTPUT}"
  jq -c '{
    error_code: "MEJEPA_STOP_HOOK_UNVERIFIED_PREDICTIONS",
    message: "ME-JEPA has unverified non-Abstain predictions for this session. Run the recommended tests or bind Confirmed/Refuted verification rows before stopping.",
    session_id,
    unverified_count,
    prediction_ids: (.unverified_predictions | map(.prediction_id)),
    recommended_commands,
    truncated_recommended_command_count,
    source_of_truth
  }' <<<"${CLI_OUTPUT}" >&2
  exit 2
fi

stop_mejepa_log_event "MEJEPA_STOP_HOOK_ALLOWED" "${CLI_OUTPUT}"
exit 0
