#!/usr/bin/env bash
# Shared helpers for ccreality Claude Code hooks.
set -euo pipefail

CGREALITY_CACHE="${CGREALITY_CACHE_DIR:-/mnt/d/contextgraph/state/cgreality}"
RUNTIME_ROOT_FILE="${CGREALITY_CACHE}/active_runtime_root"
TARGET_INSTANCE_FILE="${CGREALITY_CACHE}/active_target_instance"
CONTEXTGRAPH_ROOT_RESOLVED="${CONTEXTGRAPH_ROOT:-${CLAUDE_PROJECT_DIR:-/home/cabdru/contextgraph}}"

cgreality_runtime_root() {
  if [ -f "${RUNTIME_ROOT_FILE}" ]; then
    head -n 1 "${RUNTIME_ROOT_FILE}"
  fi
}

cgreality_target_instance() {
  if [ -f "${TARGET_INSTANCE_FILE}" ]; then
    head -n 1 "${TARGET_INSTANCE_FILE}"
  fi
}

cgreality_safe_id() {
  printf '%s' "$1" | tr -c 'A-Za-z0-9_.-' '_'
}

cgreality_file_sot() {
  printf 'file:%s' "$1"
}

cgreality_truncate_context() {
  head -c 9216
}

cgreality_emit_additional_context() {
  local hook_event="$1"
  local context="$2"
  context="$(printf '%s' "${context}" | cgreality_truncate_context)"
  jq -nc --arg event "${hook_event}" --arg ctx "${context}" '{
    "hookSpecificOutput": {
      "hookEventName": $event,
      "additionalContext": $ctx
    }
  }'
}

cgreality_emit_block() {
  local reason="$1"
  jq -nc --arg r "${reason}" '{
    "decision": "block",
    "reason": $r
  }'
}

cgreality_emit_pretool_decision() {
  local decision="$1"
  local reason="$2"
  local context="$3"
  context="$(printf '%s' "${context}" | cgreality_truncate_context)"
  jq -nc --arg d "${decision}" --arg r "${reason}" --arg c "${context}" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": $d,
      "permissionDecisionReason": $r,
      "additionalContext": $c
    }
  }'
}

cgreality_log_event() {
  local event="$1"
  local payload="${2:-{}}"
  local runtime_root
  runtime_root="$(cgreality_runtime_root)"
  if [ -z "${runtime_root}" ]; then
    return 0
  fi
  local log="${runtime_root}/cgreality-hook-events.jsonl"
  if ! mkdir -p "$(dirname "${log}")"; then
    printf '{"status":"error","error_code":"CCREALITY_HOOK_EVENT_LOG_DIR_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${log}")" >&2
    return 0
  fi
  if jq -e . >/dev/null 2>&1 <<<"${payload}"; then
    if ! jq -nc --arg ts "$(date -u +%FT%TZ)" --arg ev "${event}" --argjson payload "${payload}" '{
      timestamp: $ts,
      event: $ev,
      payload: $payload
    }' >> "${log}"; then
      printf '{"status":"error","error_code":"CCREALITY_HOOK_EVENT_LOG_WRITE_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${log}")" >&2
      return 0
    fi
  else
    if ! jq -nc --arg ts "$(date -u +%FT%TZ)" --arg ev "${event}" --arg payload "${payload}" '{
      timestamp: $ts,
      event: $ev,
      payload: $payload
    }' >> "${log}"; then
      printf '{"status":"error","error_code":"CCREALITY_HOOK_EVENT_LOG_WRITE_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${log}")" >&2
      return 0
    fi
  fi
  if ! sync -f "${log}" 2>/dev/null; then
    printf '{"status":"error","error_code":"CCREALITY_HOOK_EVENT_LOG_SYNC_FAILED","source_of_truth":"%s"}\n' "$(cgreality_file_sot "${log}")" >&2
  fi
}

cgreality_context_graph_cli_bin() {
  # Operator override via CGREALITY_CONTEXT_GRAPH_CLI wins. Without an override,
  # use the newest executable local build so hook/CLI schema changes cannot be
  # masked by an older release binary while still allowing target/debug to be
  # deleted in production.
  local bin="${CGREALITY_CONTEXT_GRAPH_CLI:-}"
  if [ -z "${bin}" ]; then
    local release_bin="${CONTEXTGRAPH_ROOT_RESOLVED}/target/release/context-graph-cli"
    local debug_bin="${CONTEXTGRAPH_ROOT_RESOLVED}/target/debug/context-graph-cli"
    if [ -x "${release_bin}" ] && [ -x "${debug_bin}" ]; then
      if [ "${debug_bin}" -nt "${release_bin}" ]; then
        bin="${debug_bin}"
      else
        bin="${release_bin}"
      fi
    elif [ -x "${release_bin}" ]; then
      bin="${release_bin}"
    else
      bin="${debug_bin}"
    fi
  fi
  if [ ! -x "${bin}" ]; then
    return 1
  fi
  printf '%s' "${bin}"
}

cgreality_normalized_paths() {
  local input_path="$1"
  local root="${2:-${CONTEXTGRAPH_ROOT_RESOLVED}}"
  local root_abs
  local path_abs
  root_abs="$(realpath -m "${root}")"
  if [[ "${input_path}" = /* ]]; then
    path_abs="$(realpath -m "${input_path}")"
  else
    path_abs="$(realpath -m "${root_abs}/${input_path}")"
  fi
  if [[ "${path_abs}" == "${root_abs}"/* ]]; then
    printf '%s\t%s\n' "${path_abs}" "${path_abs#"${root_abs}/"}"
  else
    printf '%s\t%s\n' "${path_abs}" "${path_abs}"
  fi
}

cgreality_sha256_or_missing() {
  local path="$1"
  if [ -f "${path}" ]; then
    sha256sum "${path}" | awk '{print "sha256:" $1}'
  else
    printf 'missing'
  fi
}

cgreality_line_count_or_zero() {
  local path="$1"
  if [ -f "${path}" ]; then
    wc -l < "${path}" | tr -d ' '
  else
    printf '0'
  fi
}

cgreality_policy_match() {
  local pattern="$1"
  local rel_path="$2"
  local abs_path="$3"
  local value
  if [[ "${pattern}" = /* ]]; then
    value="${abs_path}"
  else
    value="${rel_path}"
  fi
  # The glob pattern is trusted repo configuration and must remain unquoted
  # on the right-hand side so Bash treats it as a pattern.
  # shellcheck disable=SC2053
  [[ "${value}" == ${pattern} ]]
}

cgreality_policy_verdict() {
  local abs_path="$1"
  local rel_path="$2"
  local policy_path="${CGREALITY_GOVERNED_PATHS:-${CONTEXTGRAPH_ROOT_RESOLVED}/config/ccreality_governed_paths.json}"
  if [ ! -f "${policy_path}" ]; then
    jq -nc --arg source "$(cgreality_file_sot "${policy_path}")" '{
      decision: "deny",
      reason: "ccreality governed paths policy file missing",
      source_of_truth: $source
    }'
    return 0
  fi
  if ! jq -e '.title == "ccreality governed paths policy" and (.allow | type == "array") and (.deny | type == "array")' "${policy_path}" >/dev/null; then
    jq -nc --arg source "$(cgreality_file_sot "${policy_path}")" '{
      decision: "deny",
      reason: "ccreality governed paths policy is invalid",
      source_of_truth: $source
    }'
    return 0
  fi

  local pattern
  while IFS= read -r pattern; do
    if cgreality_policy_match "${pattern}" "${rel_path}" "${abs_path}"; then
      jq -nc --arg reason "ccreality governed paths deny matched: ${pattern}" --arg pattern "${pattern}" --arg source "$(cgreality_file_sot "${policy_path}")" '{
        decision: "deny",
        reason: $reason,
        matched_pattern: $pattern,
        source_of_truth: $source
      }'
      return 0
    fi
  done < <(jq -r '.deny[]' "${policy_path}")

  while IFS= read -r pattern; do
    if cgreality_policy_match "${pattern}" "${rel_path}" "${abs_path}"; then
      jq -nc --arg reason "ccreality governed paths allow matched: ${pattern}" --arg pattern "${pattern}" --arg source "$(cgreality_file_sot "${policy_path}")" '{
        decision: "allow",
        reason: $reason,
        matched_pattern: $pattern,
        source_of_truth: $source
      }'
      return 0
    fi
  done < <(jq -r '.allow[]' "${policy_path}")

  jq -nc --arg source "$(cgreality_file_sot "${policy_path}")" '{
    decision: "deny",
    reason: "ccreality governed paths policy has no allow match for this path",
    source_of_truth: $source
  }'
}

cgreality_preedit_state_path() {
  local runtime_root="$1"
  local session_id="$2"
  local tool_use_id="$3"
  local session_safe
  local tool_safe
  session_safe="$(cgreality_safe_id "${session_id:-session-unknown}")"
  tool_safe="$(cgreality_safe_id "${tool_use_id:-tool-use-unknown}")"
  printf '%s/cgreality-hook-state/preedit/%s/%s.json' "${runtime_root}" "${session_safe}" "${tool_safe}"
}
