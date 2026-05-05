#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_emit_additional_context "PreCompact" "ccreality hook input was not valid JSON; source_of_truth=stdin."
  exit 0
fi

SESSION_ID="$(jq -r '.session_id // ""' <<<"${INPUT}")"
TRIGGER="$(jq -r '.trigger // ""' <<<"${INPUT}")"
RUNTIME_ROOT="$(cgreality_runtime_root)"
TARGET_INSTANCE="$(cgreality_target_instance)"

CTX="(post-compaction reality re-injection) trigger=${TRIGGER:-unset}. "
if [ -n "${RUNTIME_ROOT}" ] && [ -d "${RUNTIME_ROOT}" ]; then
  CTX="${CTX}runtime_root=${RUNTIME_ROOT} target_instance=${TARGET_INSTANCE:-unset}. Reality-boundary rules unchanged."
  SHIFT_LOG="${RUNTIME_ROOT}/cgreality-shift-log/$(cgreality_safe_id "${SESSION_ID}").jsonl"
  if [ -f "${SHIFT_LOG}" ]; then
    RECENT_SHIFTS="$(tail -n 3 "${SHIFT_LOG}" 2>/dev/null | jq -sc 'map({shift_id, tool_name, delta: (.delta_summary // {})})' 2>/dev/null || printf '[]')"
    CTX="${CTX} Last 3 reality shifts: ${RECENT_SHIFTS}. Source of truth: $(cgreality_file_sot "${SHIFT_LOG}")."
  fi
elif [ -n "${RUNTIME_ROOT}" ]; then
  CTX="${CTX}active runtime root is set but missing on disk: ${RUNTIME_ROOT}. Source of truth: $(cgreality_file_sot "${RUNTIME_ROOT_FILE}")."
else
  CTX="${CTX}no active runtime root."
fi

cgreality_emit_additional_context "PreCompact" "${CTX}"
cgreality_log_event "PreCompact" "$(jq -nc --arg session_id "${SESSION_ID}" --arg trigger "${TRIGGER}" '{session_id: $session_id, trigger: $trigger}')"
exit 0
