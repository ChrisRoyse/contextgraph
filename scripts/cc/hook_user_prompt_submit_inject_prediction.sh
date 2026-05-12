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

REQUEST="$(jq -nc \
  --arg session_id "${SESSION_ID}" \
  '{jsonrpc:"2.0", id:"mejepa-prediction-injection", method:"tools/call", params:{name:"mcp__cgreality__mejepa_predict_latest", arguments:{sessionId:$session_id, limit:1}}}')"

if ! RESPONSE="$(curl --silent --show-error --fail-with-body --max-time 4 \
  -H 'content-type: application/json' \
  --data "${REQUEST}" \
  "${MCP_URL}" 2>&1)"; then
  log_injection_event "daemon_unavailable" "${RESPONSE}"
  exit 0
fi

if jq -e '.result.isError == true' >/dev/null 2>&1 <<<"${RESPONSE}"; then
  log_injection_event "tool_error" "$(jq -c '.result.content // .result // .' <<<"${RESPONSE}")"
  exit 0
fi

if ! MARKDOWN="$(printf '%s' "${RESPONSE}" | "${RENDERER}" --max-bytes 4096 2>&1)"; then
  log_injection_event "render_failed" "${MARKDOWN}"
  exit 0
fi

if [ -z "${MARKDOWN}" ]; then
  log_injection_event "no_prediction" "no live ME-JEPA prediction for session"
  exit 0
fi

cgreality_emit_additional_context "UserPromptSubmit" "${MARKDOWN}"
