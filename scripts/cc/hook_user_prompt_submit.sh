#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_emit_additional_context "UserPromptSubmit" "ccreality hook input was not valid JSON; source_of_truth=stdin"
  exit 0
fi

SESSION_ID="$(jq -r '.session_id // ""' <<<"${INPUT}")"
RUNTIME_ROOT="$(cgreality_runtime_root)"
TARGET_INSTANCE="$(cgreality_target_instance)"
CTX_PARTS=()

if [ -n "${RUNTIME_ROOT}" ] && [ -d "${RUNTIME_ROOT}" ] && [ -n "${TARGET_INSTANCE}" ]; then
  LATEST_ATTEMPT="$(find "${RUNTIME_ROOT}" -path "*/${TARGET_INSTANCE}/attempt-*" -type d -print 2>/dev/null; find "${RUNTIME_ROOT}" -path "*/${TARGET_INSTANCE}/smoke-attempt-*" -type d -print 2>/dev/null)"
  LATEST_ATTEMPT="$(printf '%s\n' "${LATEST_ATTEMPT}" | sed '/^$/d' | sort -V | tail -n 1 || true)"
  if [ -n "${LATEST_ATTEMPT}" ] && [ -d "${LATEST_ATTEMPT}" ]; then
    CTX_PARTS+=("Latest attempt path: ${LATEST_ATTEMPT}.")
    if [ -f "${LATEST_ATTEMPT}/trigger-decision.json" ]; then
      TRIGGER_COMPACT="$(jq -c '{status, should_run, policy, reasons: (.reasons // [] | map(.reason // .))}' "${LATEST_ATTEMPT}/trigger-decision.json" 2>/dev/null || printf '{}')"
      CTX_PARTS+=("Trigger decision: ${TRIGGER_COMPACT}. Source of truth: $(cgreality_file_sot "${LATEST_ATTEMPT}/trigger-decision.json").")
    fi
    if [ -f "${LATEST_ATTEMPT}/reality-signal-packet.json" ]; then
      SIGNAL_COMPACT="$(jq -c '{
        failure_digest: (.failure_digest // {}),
        official_progress: (.official_test_progress // {}),
        patch_signals: (.patch_signals // {}),
        score_reality: (.score_reality // {}),
        reinjection_directive: (.reinjection_directive // null)
      }' "${LATEST_ATTEMPT}/reality-signal-packet.json" 2>/dev/null || printf '{}')"
      CTX_PARTS+=("Reality signal compact: ${SIGNAL_COMPACT}. Source of truth: $(cgreality_file_sot "${LATEST_ATTEMPT}/reality-signal-packet.json").")
    fi
    if [ -f "${LATEST_ATTEMPT}/attempt-summary.json" ]; then
      SUMMARY_COMPACT="$(jq -c '{record_kind, task_id, attempt, validation, synthetic, source_of_truth}' "${LATEST_ATTEMPT}/attempt-summary.json" 2>/dev/null || printf '{}')"
      CTX_PARTS+=("Attempt summary compact: ${SUMMARY_COMPACT}. Source of truth: $(cgreality_file_sot "${LATEST_ATTEMPT}/attempt-summary.json").")
    fi
  fi
fi

if [ -n "${SESSION_ID}" ] && [ -n "${RUNTIME_ROOT}" ] && [ -d "${RUNTIME_ROOT}" ]; then
  SHIFT_LOG="${RUNTIME_ROOT}/cgreality-shift-log/$(cgreality_safe_id "${SESSION_ID}").jsonl"
  if [ -f "${SHIFT_LOG}" ]; then
    RECENT_SHIFTS="$(tail -n 3 "${SHIFT_LOG}" 2>/dev/null | jq -sc 'map({
      shift_id,
      timestamp_unix_ns,
      tool_name,
      subject: (.subject // {}),
      delta: (.delta_summary // {}),
      verification: (.verification // {})
    })' 2>/dev/null || printf '[]')"
    CTX_PARTS+=("Last 3 reality shifts in this session: ${RECENT_SHIFTS}. Source of truth: $(cgreality_file_sot "${SHIFT_LOG}").")
  fi
fi

if [ "${#CTX_PARTS[@]}" -eq 0 ]; then
  CTX="No active reality-loop attempt to drip. Call mcp__cgreality__reality_run_smoke or mcp__cgreality__reality_run_attempt to produce source-of-truth artifacts."
else
  CTX="$(printf '%s\n\n' "${CTX_PARTS[@]}")"
fi

cgreality_emit_additional_context "UserPromptSubmit" "${CTX}"
cgreality_log_event "UserPromptSubmit" "$(jq -nc --arg session_id "${SESSION_ID}" '{session_id: $session_id}')"
exit 0
