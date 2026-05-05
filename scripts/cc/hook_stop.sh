#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_emit_block "ccreality Stop hook received invalid JSON on stdin; cannot verify optimizer recommendation state."
  exit 0
fi

SESSION_ID="$(jq -r '.session_id // ""' <<<"${INPUT}")"
TRANSCRIPT_PATH="$(jq -r '.transcript_path // ""' <<<"${INPUT}")"
STOP_HOOK_ACTIVE="$(jq -r '.stop_hook_active // false' <<<"${INPUT}")"
RUNTIME_ROOT="$(cgreality_runtime_root)"
MARKER="${CGREALITY_CACHE}/last_session_marker"

if [ -z "${RUNTIME_ROOT}" ]; then
  exit 0
fi

if [ ! -d "${RUNTIME_ROOT}" ]; then
  cgreality_emit_block "ccreality Stop hook: active runtime root is set but missing on disk (${RUNTIME_ROOT}). Source of truth: $(cgreality_file_sot "${RUNTIME_ROOT_FILE}"). Fix the active cgreality cache or run a new smoke/attempt before ending the turn."
  cgreality_log_event "StopBlockedRuntimeRootMissing" "$(jq -nc --arg runtime_root "${RUNTIME_ROOT}" '{runtime_root: $runtime_root}')"
  exit 0
fi

RECOMMENDED=0
RECOMMENDATION_PATH=""
while IFS= read -r candidate; do
  if [ -z "${candidate}" ]; then
    continue
  fi
  if [ -n "${SESSION_ID}" ]; then
    if jq -e --arg session_id "${SESSION_ID}" '.claude_session_id == $session_id' "${candidate}" >/dev/null 2>&1; then
      RECOMMENDED=1
      RECOMMENDATION_PATH="${candidate}"
      break
    fi
  elif jq -e . "${candidate}" >/dev/null 2>&1; then
    RECOMMENDED=1
    RECOMMENDATION_PATH="${candidate}"
    break
  fi
done < <(find "${RUNTIME_ROOT}" -path '*/claude-code-optimizer/recommendation-*.json' -type f -print 2>/dev/null | sort)

if [ "${RECOMMENDED}" -eq 0 ] && [ -f "${MARKER}" ]; then
  RECENT="$(find "${RUNTIME_ROOT}" -path '*/claude-code-optimizer/recommendation-*.json' -type f -newer "${MARKER}" -print -quit 2>/dev/null || true)"
  if [ -n "${RECENT}" ] && jq -e . "${RECENT}" >/dev/null 2>&1; then
    RECOMMENDED=1
    RECOMMENDATION_PATH="${RECENT}"
  fi
fi

if [ "${RECOMMENDED}" -eq 0 ] && [ -n "${TRANSCRIPT_PATH}" ] && [ -f "${TRANSCRIPT_PATH}" ]; then
  if grep -q 'mcp__cgreality__optimizer_record_recommendation' "${TRANSCRIPT_PATH}" 2>/dev/null; then
    RECOMMENDED=1
    RECOMMENDATION_PATH="transcript:${TRANSCRIPT_PATH}"
  fi
fi

if [ "${RECOMMENDED}" -eq 1 ]; then
  mkdir -p "${CGREALITY_CACHE}"
  touch "${MARKER}"
  cgreality_log_event "StopAllowed" "$(jq -nc --arg session_id "${SESSION_ID}" --arg recommendation_path "${RECOMMENDATION_PATH}" '{session_id: $session_id, recommendation_path: $recommendation_path}')"
  exit 0
fi

REASON="ccreality Stop hook: this optimization turn has not recorded a structured recommendation. Call mcp__cgreality__optimizer_record_recommendation with the schema in docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md before ending the turn. If no harness change is warranted, record status=\"no_change\" with source-of-truth readbacks. Runtime root: ${RUNTIME_ROOT}."
if [ "${STOP_HOOK_ACTIVE}" = "true" ]; then
  REASON="${REASON} stop_hook_active=true, so this is a repeated stop-gate check; the recommendation artifact is still missing."
fi
cgreality_emit_block "${REASON}"
cgreality_log_event "StopBlocked" "$(jq -nc --arg session_id "${SESSION_ID}" --arg transcript_path "${TRANSCRIPT_PATH}" '{session_id: $session_id, transcript_path: $transcript_path}')"
exit 0
