#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/_common.sh
source "${SCRIPT_DIR}/_common.sh"

INPUT="$(cat)"
if ! jq -e . >/dev/null 2>&1 <<<"${INPUT}"; then
  cgreality_emit_additional_context "SessionStart" "ccreality hook input was not valid JSON; source_of_truth=stdin"
  exit 0
fi

SESSION_ID="$(jq -r '.session_id // ""' <<<"${INPUT}")"
SOURCE="$(jq -r '.source // ""' <<<"${INPUT}")"
MODEL="$(jq -r '.model // ""' <<<"${INPUT}")"
RUNTIME_ROOT="$(cgreality_runtime_root)"
TARGET_INSTANCE="$(cgreality_target_instance)"

if [ -z "${RUNTIME_ROOT}" ]; then
  CTX="ContextGraph reality loop is not currently running. Active runtime root: not set. Use cgreality MCP tools only after a smoke or attempt has produced durable artifacts. Reality-boundary rules apply: never edit .env files, swebench official evaluators, tests/oracle_**, reward_specs, or verifiers."
else
  LATEST_SUMMARY=""
  if [ -d "${RUNTIME_ROOT}" ]; then
    SUMMARY_PATH="$(find "${RUNTIME_ROOT}" -path '*/attempt-summary.json' -type f -print 2>/dev/null | sort | tail -n 1 || true)"
    if [ -n "${SUMMARY_PATH}" ]; then
      # Phase 13 T13.1: branch on record_kind so synthetic-smoke summaries
      # (ccreality_smoke_attempt_summary, flat shape) and full attempt summaries
      # (with .attempt.* nested object) both produce a usable compact summary
      # without falsely reporting a "read failed" condition.
      RECORD_KIND="$(jq -r '.record_kind // ""' "${SUMMARY_PATH}" 2>/dev/null || echo "")"
      if [ "${RECORD_KIND}" = "ccreality_smoke_attempt_summary" ]; then
        if SUMMARY_COMPACT="$(jq -c '{
          record_kind: .record_kind,
          attempt_number: .attempt,
          task_id: .task_id,
          synthetic: .synthetic,
          validation_status: .validation.status,
          official_evidence: (.official_evidence != null),
          problem_reality_packet: .problem_reality_packet_path,
          reality_signal_packet: .reality_signal_packet_path
        }' "${SUMMARY_PATH}" 2>/dev/null)"; then
          LATEST_SUMMARY="Latest attempt summary: ${SUMMARY_COMPACT}. Source of truth: $(cgreality_file_sot "${SUMMARY_PATH}")."
        else
          LATEST_SUMMARY="Latest smoke attempt summary read failed. Source of truth: $(cgreality_file_sot "${SUMMARY_PATH}")."
        fi
      elif SUMMARY_COMPACT="$(jq -c '{
        record_kind: "contextgraph_attempt_summary_hook_compact",
        attempt_number: .attempt.attempt,
        status: .attempt.status,
        error_code: .attempt.error_code,
        message: .attempt.message,
        remediation: .attempt.remediation,
        patch_sha256: .attempt.patch_sha256,
        ledger_counts: .ledger_counts,
        score: .score.score,
        reward: .reward.reward,
        model_response: {
          status: .model_response.status,
          error_code: .model_response.error_code,
          message_sha256: .attempt.response_sha256
        }
      }' "${SUMMARY_PATH}" 2>/dev/null)"; then
        LATEST_SUMMARY="Latest attempt summary: ${SUMMARY_COMPACT}. Source of truth: $(cgreality_file_sot "${SUMMARY_PATH}")."
      else
        LATEST_SUMMARY="Latest attempt summary read failed. Source of truth: $(cgreality_file_sot "${SUMMARY_PATH}"). Remediation: inspect malformed attempt-summary.json before relying on session-start context."
      fi
    fi
  else
    LATEST_SUMMARY="Active runtime root path is set but missing on disk. Source of truth: $(cgreality_file_sot "${RUNTIME_ROOT_FILE}")."
  fi
  CTX="$(cat <<EOF
Active reality-loop runtime root: ${RUNTIME_ROOT}
Target SWE-bench instance: ${TARGET_INSTANCE:-unset}
Session source: ${SOURCE:-unset}
Model: ${MODEL:-unset}
Success predicate: official SWE-bench report places target instance in resolved_ids.
${LATEST_SUMMARY}
ContextGraph MCP tools available under prefix mcp__cgreality__. Confirm active state with mcp__cgreality__reality_latest_root before altering reality.
Reality-boundary rules:
- Never edit .env files
- Never edit swebench official evaluators
- Never edit tests/oracle_** benchmark fixtures
- Never edit crates/*/src/reward_specs/** or crates/*/src/verifiers/**
- Never claim SWE-bench success unless official resolved_ids contains the target instance
EOF
)"
fi

cgreality_emit_additional_context "SessionStart" "${CTX}"
cgreality_log_event "SessionStart" "$(jq -c --arg session_id "${SESSION_ID}" --arg source "${SOURCE}" '{session_id: $session_id, source: $source}' <<<"{}")"
exit 0
