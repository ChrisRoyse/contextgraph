#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

LOG_PATH="${CGREALITY_STOP_MEJEPA_LOG:-${CGREALITY_DROOT:-/zfs/archive/contextgraph}/runtime/stop-mejepa-self-verify.jsonl}"
BLOCKED_DIR="${CGREALITY_STOP_MEJEPA_BLOCKED_DIR:-$(dirname "${LOG_PATH}")/stop-mejepa-self-verify-blocked}"
BREAKER_STATE_DIR="${CGREALITY_STOP_MEJEPA_BREAKER_STATE_DIR:-$(dirname "${LOG_PATH}")/stop-mejepa-self-verify-breaker}"

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
    --arg db_path "${CONTEXTGRAPH_MEJEPA_INFER_DB:-/zfs/archive/contextgraph/storage/contextgraph-rocksdb}" \
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

stop_mejepa_artifact_id() {
  printf '%s-%s' "$(date -u +%Y%m%dT%H%M%SZ)" "$$"
}

stop_mejepa_write_blocked_artifact() {
  local session_id="$1"
  local reason_code="$2"
  local reason="$3"
  local remediation="$4"
  local detail="$5"
  local stderr_tail="${6:-}"
  local breaker_open="${7:-false}"
  local timeout_count="${8:-0}"
  local artifact_path
  local payload

  artifact_path="${BLOCKED_DIR}/$(stop_mejepa_artifact_id)-${reason_code}.json"
  if ! mkdir -p "${BLOCKED_DIR}"; then
    stop_mejepa_warn_unreachable "${session_id}" "failed to create stop-hook blocked artifact directory: ${BLOCKED_DIR}" "${stderr_tail}"
    return 0
  fi

  payload="$(jq -nc \
    --arg ts "$(date -u +%FT%TZ)" \
    --arg session_id "${session_id}" \
    --arg reason_code "${reason_code}" \
    --arg reason "${reason}" \
    --arg remediation "${remediation}" \
    --arg detail "${detail}" \
    --arg stderr_tail "${stderr_tail}" \
    --arg artifact_path "${artifact_path}" \
    --arg log_path "${LOG_PATH}" \
    --arg db_path "${DB_PATH:-${CONTEXTGRAPH_MEJEPA_INFER_DB:-/zfs/archive/contextgraph/storage/contextgraph-rocksdb}}" \
    --argjson breaker_open "${breaker_open}" \
    --argjson timeout_count "${timeout_count}" \
    '{
      schema_version: 1,
      status: "blocked_operator_action_required",
      error_code: $reason_code,
      timestamp: $ts,
      session_id: $session_id,
      reason: $reason,
      remediation: $remediation,
      detail: $detail,
      stderr_tail: $stderr_tail,
      breaker: {
        open: $breaker_open,
        timeout_count: $timeout_count
      },
      operator_action: {
        required: true,
        summary: $remediation
      },
      source_of_truth: {
        artifact_path: $artifact_path,
        hook_log: $log_path,
        db_path: $db_path
      }
    }')"
  printf '%s\n' "${payload}" > "${artifact_path}"
  sync -f "${artifact_path}" 2>/dev/null || true
  stop_mejepa_log_event "${reason_code}" "${payload}"
  printf '%s\n' "${payload}" >&2
}

stop_mejepa_timeout_state_path() {
  local session_id="$1"
  printf '%s/%s-timeouts.json' "${BREAKER_STATE_DIR}" "${session_id:-unknown-session}"
}

stop_mejepa_reset_timeout_state() {
  local session_id="$1"
  rm -f "$(stop_mejepa_timeout_state_path "${session_id}")" 2>/dev/null || true
}

stop_mejepa_record_timeout() {
  local session_id="$1"
  local threshold="$2"
  local state_path
  local previous_count
  local timeout_count
  local now_ts
  local state_json

  state_path="$(stop_mejepa_timeout_state_path "${session_id}")"
  mkdir -p "${BREAKER_STATE_DIR}"
  if [ -f "${state_path}" ]; then
    previous_count="$(jq -r '.timeout_count // 0' "${state_path}" 2>/dev/null || printf '0')"
  else
    previous_count=0
  fi
  timeout_count=$((previous_count + 1))
  now_ts="$(date -u +%FT%TZ)"
  state_json="$(jq -nc \
    --arg session_id "${session_id}" \
    --arg first_timeout_at "$(jq -r '.first_timeout_at // empty' "${state_path}" 2>/dev/null || true)" \
    --arg last_timeout_at "${now_ts}" \
    --arg state_path "${state_path}" \
    --argjson timeout_count "${timeout_count}" \
    --argjson threshold "${threshold}" \
    '{
      schema_version: 1,
      session_id: $session_id,
      timeout_count: $timeout_count,
      threshold: $threshold,
      first_timeout_at: (if $first_timeout_at == "" then $last_timeout_at else $first_timeout_at end),
      last_timeout_at: $last_timeout_at,
      breaker_open: ($timeout_count >= $threshold),
      source_of_truth: {state_path: $state_path}
    }')"
  printf '%s\n' "${state_json}" > "${state_path}"
  sync -f "${state_path}" 2>/dev/null || true
  printf '%s' "${timeout_count}"
}

stop_mejepa_db_state_blocked() {
  local session_id="$1"
  local stale_seconds="${CGREALITY_MEJEPA_STOP_VERIFY_STALE_DB_SECONDS:-0}"
  if [ ! -d "${DB_PATH}" ]; then
    stop_mejepa_write_blocked_artifact \
      "${session_id}" \
      "MEJEPA_STOP_HOOK_DB_MISSING" \
      "ME-JEPA stop-self-verify cannot open the inference RocksDB path because the directory is missing." \
      "Start or repair the ME-JEPA daemon/storage path, then rerun stop-self-verify before treating this Stop as verified." \
      "db_path=${DB_PATH}"
    return 0
  fi
  if [ "${stale_seconds}" != "0" ]; then
    local mtime
    local now
    local age
    mtime="$(stat -c %Y "${DB_PATH}" 2>/dev/null || printf '0')"
    now="$(date +%s)"
    age=$((now - mtime))
    if [ "${age}" -gt "${stale_seconds}" ]; then
      stop_mejepa_write_blocked_artifact \
        "${session_id}" \
        "MEJEPA_STOP_HOOK_DB_STALE" \
        "ME-JEPA inference RocksDB path is older than the configured freshness window." \
        "Confirm the daemon is writing live predictions to this db_path or update CONTEXTGRAPH_MEJEPA_INFER_DB before trusting Stop verification." \
        "db_path=${DB_PATH}; age_seconds=${age}; stale_seconds=${stale_seconds}"
      return 0
    fi
  fi
  return 1
}

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  stop_mejepa_warn_unreachable "" "Stop hook received invalid JSON on stdin; cannot map current session." ""
  exit 2
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

DB_PATH="${CONTEXTGRAPH_MEJEPA_INFER_DB:-/zfs/archive/contextgraph/storage/contextgraph-rocksdb}"
MAX_PREDICTIONS="${CGREALITY_MEJEPA_STOP_VERIFY_MAX_PREDICTIONS:-50}"
TIMEOUT_SECONDS="${CGREALITY_MEJEPA_STOP_VERIFY_TIMEOUT_SECONDS:-6}"
TIMEOUT_BREAKER_THRESHOLD="${CGREALITY_MEJEPA_STOP_VERIFY_TIMEOUT_BREAKER_THRESHOLD:-3}"

if ! CLI_BIN="$(cgreality_context_graph_cli_bin)"; then
  stop_mejepa_write_blocked_artifact \
    "${SESSION_ID}" \
    "MEJEPA_STOP_HOOK_CLI_MISSING" \
    "context-graph-cli executable was not found, so Stop verification could not run." \
    "Build context-graph-cli or set CGREALITY_CONTEXT_GRAPH_CLI to the correct executable, then rerun stop-self-verify." \
    "PATH=${PATH:-}"
  exit 2
fi

TMP_OUT="$(mktemp /tmp/mejepa-stop-self-verify-out-XXXXXX.json)"
TMP_ERR="$(mktemp /tmp/mejepa-stop-self-verify-err-XXXXXX.log)"
trap 'rm -f "${TMP_OUT}" "${TMP_ERR}"' EXIT

if stop_mejepa_db_state_blocked "${SESSION_ID}"; then
  exit 2
fi

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
if [ "${CLI_EXIT}" -eq 124 ]; then
  TIMEOUT_COUNT="$(stop_mejepa_record_timeout "${SESSION_ID}" "${TIMEOUT_BREAKER_THRESHOLD}")"
  if [ "${TIMEOUT_COUNT}" -ge "${TIMEOUT_BREAKER_THRESHOLD}" ]; then
    stop_mejepa_write_blocked_artifact \
      "${SESSION_ID}" \
      "MEJEPA_STOP_HOOK_TIMEOUT_BREAKER_OPEN" \
      "Repeated stop-self-verify timeouts tripped the bounded Stop-hook circuit breaker." \
      "Inspect the daemon, CLI, and RocksDB health; then remove the breaker state file after confirming verification is healthy." \
      "timeout_seconds=${TIMEOUT_SECONDS}; threshold=${TIMEOUT_BREAKER_THRESHOLD}; state_path=$(stop_mejepa_timeout_state_path "${SESSION_ID}")" \
      "${CLI_STDERR}" \
      true \
      "${TIMEOUT_COUNT}"
    exit 2
  fi
  stop_mejepa_write_blocked_artifact \
    "${SESSION_ID}" \
    "MEJEPA_STOP_HOOK_VERIFY_TIMEOUT" \
    "stop-self-verify timed out before it could prove current predictions are verified." \
    "Rerun Stop after the daemon/CLI responds, or inspect the blocked artifact and breaker state before operator override." \
    "timeout_seconds=${TIMEOUT_SECONDS}; threshold=${TIMEOUT_BREAKER_THRESHOLD}; state_path=$(stop_mejepa_timeout_state_path "${SESSION_ID}")" \
    "${CLI_STDERR}" \
    false \
    "${TIMEOUT_COUNT}"
  exit 2
fi
if [ "${CLI_EXIT}" -ne 0 ] || ! jq -e . >/dev/null 2>&1 <<<"${CLI_OUTPUT}"; then
  stop_mejepa_write_blocked_artifact \
    "${SESSION_ID}" \
    "MEJEPA_STOP_HOOK_DAEMON_UNREACHABLE" \
    "stop-self-verify command failed or returned invalid JSON." \
    "Repair the daemon/CLI path, then rerun stop-self-verify before treating this Stop as verified." \
    "exit=${CLI_EXIT}" \
    "${CLI_STDERR}"
  exit 2
fi
stop_mejepa_reset_timeout_state "${SESSION_ID}"

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
