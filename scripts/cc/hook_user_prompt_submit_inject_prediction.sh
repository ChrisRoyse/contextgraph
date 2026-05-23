#!/usr/bin/env bash
set -euo pipefail

ROOT="${CLAUDE_PROJECT_DIR:-${CONTEXTGRAPH_ROOT:-/home/cabdru/contextgraph}}"
# shellcheck source=scripts/cc/_common.sh
source "${ROOT}/scripts/cc/_common.sh"

INPUT="$(cat)"
RAW_SESSION_ID="$(jq -r '.session_id // env.CLAUDE_SESSION_ID // ""' <<<"${INPUT}")"
SESSION_ID="$(cgreality_session_hex32 "${RAW_SESSION_ID}")"
HTTP_PORT="${CONTEXT_GRAPH_HTTP_PORT:-3101}"
MCP_URL="${CONTEXTGRAPH_MCP_HTTP_URL:-http://127.0.0.1:${HTTP_PORT}/mcp}"
RENDERER="${ROOT}/scripts/cc/render_prediction_markdown.py"
AGENT_HISTORY_ROOT="${CGREALITY_AGENT_HISTORY_ROOT:-${CGREALITY_DROOT:-/zfs/archive/contextgraph}/runtime/agent-history}"
AGENT_HISTORY_PATH="${AGENT_HISTORY_ROOT}/$(cgreality_safe_id "${SESSION_ID}")-injections.jsonl"
COLD_START_MARKDOWN=$'### Reality prediction\n(no prediction available yet; ME-JEPA observing)\n'

log_injection_event() {
  local status="$1"
  local detail="$2"
  local payload
  payload="$(jq -nc \
    --arg status "${status}" \
    --arg detail "${detail}" \
    --arg session_id "${SESSION_ID}" \
    --arg raw_session_id "${RAW_SESSION_ID}" \
    --arg url "${MCP_URL}" \
    '{status: $status, detail: $detail, session_id: $session_id, raw_session_id: $raw_session_id, mcp_url: $url}')"
  cgreality_droot_log_event "mejepa_prediction_injection" "${payload}"
}

emit_warning() {
  local code="$1"
  local detail="$2"
  jq -nc \
    --arg status "warning" \
    --arg code "${code}" \
    --arg detail "${detail}" \
    --arg source_of_truth "$(cgreality_file_sot "${AGENT_HISTORY_PATH}")" \
    '{status: $status, error_code: $code, detail: $detail, source_of_truth: $source_of_truth}' >&2
}

write_injection_history() {
  local status="$1"
  local injected="$2"
  local markdown="$3"
  local detail="$4"
  local bytes
  bytes="$(printf '%s' "${markdown}" | wc -c | tr -d ' ')"
  if ! mkdir -p "${AGENT_HISTORY_ROOT}"; then
    emit_warning "MEJEPA_INJECT_HISTORY_WRITE_FAILED" "could not create ${AGENT_HISTORY_ROOT}"
    return 0
  fi
  local line
  line="$(jq -nc \
    --arg ts "$(date -u +%FT%TZ)" \
    --arg status "${status}" \
    --arg detail "${detail}" \
    --arg session_id "${SESSION_ID}" \
    --arg raw_session_id "${RAW_SESSION_ID}" \
    --arg mcp_url "${MCP_URL}" \
    --arg source_of_truth "$(cgreality_file_sot "${AGENT_HISTORY_PATH}")" \
    --arg markdown "${markdown}" \
    --argjson injected "${injected}" \
    --argjson rendered_bytes "${bytes}" \
    '{
      timestamp: $ts,
      hook: "UserPromptSubmit",
      event: "mejepa_prediction_injection",
      status: $status,
      detail: $detail,
      injected: $injected,
      rendered_bytes: $rendered_bytes,
      session_id: $session_id,
      raw_session_id: $raw_session_id,
      mcp_url: $mcp_url,
      source_of_truth: $source_of_truth,
      markdown: $markdown
    }')"
  local lock="${AGENT_HISTORY_PATH}.lock"
  if ! (
    flock -x 9
    printf '%s\n' "${line}" >> "${AGENT_HISTORY_PATH}"
    sync -f "${AGENT_HISTORY_PATH}" 2>/dev/null || true
  ) 9>"${lock}"; then
    emit_warning "MEJEPA_INJECT_HISTORY_WRITE_FAILED" "could not append ${AGENT_HISTORY_PATH}"
  fi
}

REQUEST="$(jq -nc \
  --arg session_id "${SESSION_ID}" \
  '{jsonrpc:"2.0", id:"mejepa-prediction-injection", method:"tools/call", params:{name:"mcp__cgreality__mejepa_predict_latest", arguments:{sessionId:$session_id, limit:1}}}')"

if ! RESPONSE="$(curl --silent --show-error --fail-with-body --max-time 4 \
  -H 'content-type: application/json' \
  -H 'accept: application/json, text/event-stream' \
  --data "${REQUEST}" \
  "${MCP_URL}" 2>&1)"; then
  log_injection_event "daemon_unavailable" "${RESPONSE}"
  write_injection_history "mcp_unreachable" "false" "" "${RESPONSE}"
  emit_warning "MEJEPA_INJECT_MCP_UNREACHABLE" "${RESPONSE}"
  exit 1
fi

if jq -e '.result.isError == true' >/dev/null 2>&1 <<<"${RESPONSE}"; then
  DETAIL="$(jq -c '.result.content // .result // .' <<<"${RESPONSE}")"
  log_injection_event "tool_error" "${DETAIL}"
  write_injection_history "tool_error" "false" "" "${DETAIL}"
  emit_warning "MEJEPA_INJECT_TOOL_ERROR" "${DETAIL}"
  exit 1
fi

if ! MARKDOWN="$(printf '%s' "${RESPONSE}" | "${RENDERER}" --max-bytes 4096 2>&1)"; then
  log_injection_event "render_failed" "${MARKDOWN}"
  write_injection_history "render_failed" "false" "" "${MARKDOWN}"
  emit_warning "MEJEPA_INJECT_RENDER_FAILED" "${MARKDOWN}"
  exit 1
fi

if [ -z "${MARKDOWN}" ]; then
  log_injection_event "no_prediction" "no live ME-JEPA prediction for session"
  MARKDOWN="${COLD_START_MARKDOWN}"
  write_injection_history "cold_start" "true" "${MARKDOWN}" "no live ME-JEPA prediction for session"
  cgreality_emit_additional_context "UserPromptSubmit" "${MARKDOWN}"
  exit 0
fi

write_injection_history "injected" "true" "${MARKDOWN}" "live ME-JEPA prediction rendered"
cgreality_emit_additional_context "UserPromptSubmit" "${MARKDOWN}"
