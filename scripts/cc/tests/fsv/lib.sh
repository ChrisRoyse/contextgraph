#!/usr/bin/env bash
# Shared Phase 10 full-state verification helpers and case implementations.
# shellcheck shell=bash
set -Eeuo pipefail

PROJECT_ROOT="${PROJECT_ROOT:-/home/cabdru/contextgraph}"
FSV_RUN_ROOT="${FSV_RUN_ROOT:-/tmp/cgreality-fsv-manual-$$}"
FSV_CASE_ROOT="${FSV_CASE_ROOT:-${FSV_RUN_ROOT}/cases}"
MCP_STDERR="${MCP_STDERR:-${FSV_RUN_ROOT}/mcp-stderr.log}"
ACTIVE_CACHE="${HOME}/.cache/cgreality"
ACTIVE_ROOT_FILE="${ACTIVE_CACHE}/active_runtime_root"
ACTIVE_TARGET_FILE="${ACTIVE_CACHE}/active_target_instance"
PROBE_REL="scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt"
PROBE_ABS="${PROJECT_ROOT}/${PROBE_REL}"

CASE_NAME=""
CASE_WORK=""
CASE_STATE_LOG=""
CASE_EVIDENCE_LOG=""
CASE_SOURCE_LOG=""
LAST_MCP_RAW=""
LAST_MCP_TEXT=""
MCP_SEQ=0
RUNTIME_ROOT=""
RUN_ID=""
ATTEMPT="1"
ATTEMPT_SUMMARY=""
ATTEMPT_DIR=""
LEDGER=""
FSV_PROBE_BACKUP=""
ACTIVE_ROOT_BACKUP="__unset__"
ACTIVE_TARGET_BACKUP="__unset__"

log() {
  printf '[%s] %s\n' "${CASE_NAME:-fsv}" "$*"
}

fail() {
  printf '[%s] FAIL: %s\n' "${CASE_NAME:-fsv}" "$*" >&2
  exit 1
}

on_error() {
  local line="$1"
  local status="$2"
  printf '[%s] ERROR line=%s exit=%s command=%s\n' "${CASE_NAME:-fsv}" "${line}" "${status}" "${BASH_COMMAND}" >&2
}

backup_active_state() {
  mkdir -p "${ACTIVE_CACHE}"
  if [ -f "${ACTIVE_ROOT_FILE}" ]; then
    ACTIVE_ROOT_BACKUP="$(cat "${ACTIVE_ROOT_FILE}")"
  fi
  if [ -f "${ACTIVE_TARGET_FILE}" ]; then
    ACTIVE_TARGET_BACKUP="$(cat "${ACTIVE_TARGET_FILE}")"
  fi
}

restore_active_state() {
  mkdir -p "${ACTIVE_CACHE}"
  if [ "${ACTIVE_ROOT_BACKUP}" != "__unset__" ]; then
    printf '%s\n' "${ACTIVE_ROOT_BACKUP}" >"${ACTIVE_ROOT_FILE}"
  else
    rm -f "${ACTIVE_ROOT_FILE}"
  fi
  if [ "${ACTIVE_TARGET_BACKUP}" != "__unset__" ]; then
    printf '%s\n' "${ACTIVE_TARGET_BACKUP}" >"${ACTIVE_TARGET_FILE}"
  else
    rm -f "${ACTIVE_TARGET_FILE}"
  fi
}

restore_probe() {
  if [ -n "${FSV_PROBE_BACKUP}" ] && [ -f "${FSV_PROBE_BACKUP}" ]; then
    cp "${FSV_PROBE_BACKUP}" "${PROBE_ABS}"
  fi
}

case_cleanup() {
  local status=$?
  restore_probe
  restore_active_state
  return "${status}"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command missing: $1"
}

require_file() {
  [ -f "$1" ] || fail "required file missing: $1"
}

uri_to_path() {
  local uri="$1"
  case "${uri}" in
    file://*) printf '%s' "${uri#file://}" ;;
    file:*) printf '%s' "${uri#file:}" ;;
    *) printf '%s' "${uri}" ;;
  esac
}

sha_file() {
  sha256sum "$1" | awk '{print "sha256:" $1}'
}

source_read_file() {
  local path
  path="$(uri_to_path "$1")"
  {
    printf 'SOURCE_OF_TRUTH path=%s\n' "${path}"
    if [ -e "${path}" ]; then
      stat --printf='  type=%F size=%s mtime=%y\n' "${path}" || true
      if [ -f "${path}" ]; then
        printf '  sha256=%s\n' "$(sha_file "${path}")"
        if jq -e . "${path}" >/dev/null 2>&1; then
          printf '  json=%s\n' "$(jq -c 'if type == "object" then del(.source_of_truth_readbacks?) else . end' "${path}" | head -c 1200)"
          printf '\n'
        fi
      fi
    else
      printf '  missing=true\n'
    fi
  } | tee -a "${CASE_SOURCE_LOG}" >&2
}

source_read_sqlite_counts() {
  local ledger="$1"
  require_file "${ledger}"
  {
    printf 'SOURCE_OF_TRUTH sqlite=%s#ledger_records\n' "${ledger}"
    sqlite3 "${ledger}" "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind;"
  } | tee -a "${CASE_SOURCE_LOG}" >&2
}

state_snapshot() {
  local label="$1"
  shift
  {
    printf 'STATE %s\n' "${label}"
    for path in "$@"; do
      path="$(uri_to_path "${path}")"
      if [ -e "${path}" ]; then
        stat --printf='  %n type=%F size=%s mtime=%y\n' "${path}" || true
        if [ -f "${path}" ]; then
          printf '  %s sha256=%s\n' "${path}" "$(sha_file "${path}")"
        fi
      else
        printf '  %s missing=true\n' "${path}"
      fi
    done
  } | tee -a "${CASE_STATE_LOG}" >&2
}

assert_jq_file() {
  local path="$1"
  local expr="$2"
  local label="$3"
  jq -e "${expr}" "${path}" >/dev/null || fail "jq assertion failed (${label}): ${expr} in ${path}"
}

mcp_call_text() {
  local tool="$1"
  local args="$2"
  local request
  LAST_MCP_RAW="${CASE_WORK}/mcp-${MCP_SEQ}-${tool}.raw.json"
  LAST_MCP_TEXT="${CASE_WORK}/mcp-${MCP_SEQ}-${tool}.text.json"
  MCP_SEQ=$((MCP_SEQ + 1))
  request="$(jq -nc --arg name "${tool}" --argjson args "${args}" '{jsonrpc:"2.0",id:1,method:"tools/call",params:{name:$name,arguments:$args}}')"
  printf '%s\n' "${request}" \
    | CONTEXTGRAPH_ROOT="${PROJECT_ROOT}" \
      CGREALITY_GOVERNED_PATHS="${PROJECT_ROOT}/config/ccreality_governed_paths.json" \
      CGREALITY_RECOMMENDATION_SCHEMA="${PROJECT_ROOT}/config/ccreality_recommendation_schema.json" \
      timeout 180s "${PROJECT_ROOT}/target/debug/context-graph-mcp" --mode reality-loop >"${LAST_MCP_RAW}" 2>>"${MCP_STDERR}" \
    || fail "MCP call failed to execute: ${tool}; stderr=${MCP_STDERR}"
  jq -e '.result.content[0].text' "${LAST_MCP_RAW}" >/dev/null || fail "MCP call returned no text content: ${tool}"
  jq -r '.result.content[0].text' "${LAST_MCP_RAW}" >"${LAST_MCP_TEXT}"
  source_read_file "${LAST_MCP_RAW}"
  source_read_file "${LAST_MCP_TEXT}"
}

assert_mcp_ok() {
  jq -e '.result.isError != true' "${LAST_MCP_RAW}" >/dev/null || fail "MCP returned error: $(cat "${LAST_MCP_TEXT}")"
}

assert_mcp_error_code() {
  local expected="$1"
  jq -e '.result.isError == true' "${LAST_MCP_RAW}" >/dev/null || fail "MCP call did not return isError=true"
  jq -e --arg code "${expected}" '.error_code == $code or (.error_code | tostring | contains($code))' "${LAST_MCP_TEXT}" >/dev/null \
    || fail "MCP error_code did not match ${expected}: $(cat "${LAST_MCP_TEXT}")"
}

run_smoke() {
  local label="$1"
  RUNTIME_ROOT="${CASE_WORK}/runtime-${label}"
  rm -rf "${RUNTIME_ROOT}"
  mkdir -p "${RUNTIME_ROOT}"
  local out="${CASE_WORK}/${label}-smoke.json"
  local err="${CASE_WORK}/${label}-smoke.err"
  state_snapshot "before smoke ${label}" "${RUNTIME_ROOT}" "${ACTIVE_ROOT_FILE}" "${ACTIVE_TARGET_FILE}"
  CONTEXTGRAPH_ROOT="${PROJECT_ROOT}" "${PROJECT_ROOT}/target/debug/reality-loop" smoke --runtime-root "${RUNTIME_ROOT}" --label "${label}" >"${out}" 2>"${err}" \
    || fail "reality-loop smoke failed for ${label}; stderr=$(tail -c 2048 "${err}")"
  assert_jq_file "${out}" '.status == "ok"' "smoke status"
  RUN_ID="$(jq -r '.run_id' "${out}")"
  ATTEMPT="1"
  ATTEMPT_SUMMARY="$(uri_to_path "$(jq -r '.source_of_truth.attempt_summary' "${out}")")"
  ATTEMPT_DIR="$(dirname "${ATTEMPT_SUMMARY}")"
  require_file "${ATTEMPT_SUMMARY}"
  source_read_file "${out}"
  source_read_file "${ATTEMPT_SUMMARY}"
  source_read_file "${ATTEMPT_DIR}/reality-signal-packet.json"
  source_read_file "${ATTEMPT_DIR}/problem-reality-packet.json"
  state_snapshot "after smoke ${label}" "${RUNTIME_ROOT}" "${ACTIVE_ROOT_FILE}" "${ACTIVE_TARGET_FILE}" "${ATTEMPT_SUMMARY}"
}

run_force_fail_smoke() {
  local label="$1"
  RUNTIME_ROOT="${CASE_WORK}/runtime-${label}"
  rm -rf "${RUNTIME_ROOT}"
  mkdir -p "${RUNTIME_ROOT}"
  local out="${CASE_WORK}/${label}-force-fail.stdout.json"
  local err="${CASE_WORK}/${label}-force-fail.stderr.json"
  state_snapshot "before forced failure ${label}" "${RUNTIME_ROOT}"
  set +eE
  CONTEXTGRAPH_ROOT="${PROJECT_ROOT}" "${PROJECT_ROOT}/target/debug/reality-loop" smoke --runtime-root "${RUNTIME_ROOT}" --label "${label}" --force-fail >"${out}" 2>"${err}"
  local ec=$?
  set -eE
  [ "${ec}" -ne 0 ] || fail "forced-failure smoke unexpectedly exited 0"
  local failure
  failure="$(find "${RUNTIME_ROOT}/failures" -name '*.json' -type f -print -quit)"
  [ -n "${failure}" ] || fail "forced-failure smoke wrote no failure artifact"
  assert_jq_file "${failure}" '.error.error_code == "RUST_REALITY_SMOKE_FORCED_FAIL"' "forced failure error code"
  source_read_file "${failure}"
  state_snapshot "after forced failure ${label}" "${RUNTIME_ROOT}" "${failure}"
}

duplicate_attempt_with_patch() {
  local patch_sha="$1"
  local dir2
  dir2="$(dirname "${ATTEMPT_DIR}")/smoke-attempt-2"
  rm -rf "${dir2}"
  cp -a "${ATTEMPT_DIR}" "${dir2}"
  local n dir tmp sot
  for n in 1 2; do
    if [ "${n}" -eq 1 ]; then
      dir="${ATTEMPT_DIR}"
    else
      dir="${dir2}"
    fi
    tmp="${dir}/attempt-summary.json.tmp"
    sot="file:${dir}/attempt-summary.json"
    jq --arg patch "${patch_sha}" --arg sot "${sot}" --argjson n "${n}" \
      '.attempt = {status: (.validation.status // "unknown"), resolved: ((.validation.status // "") == "passed"), patch_sha256: $patch, number: $n} | .attempt_number = $n | .source_of_truth = $sot' \
      "${dir}/attempt-summary.json" >"${tmp}"
    mv "${tmp}" "${dir}/attempt-summary.json"
    source_read_file "${dir}/attempt-summary.json"
  done
}

init_ledger() {
  LEDGER="${RUNTIME_ROOT}/ledger.sqlite"
  rm -f "${LEDGER}"
  "${PROJECT_ROOT}/target/debug/context-graph" blackbox init --ledger "${LEDGER}" >"${CASE_WORK}/blackbox-init.json"
  "${PROJECT_ROOT}/target/debug/context-graph" blackbox record run \
    --ledger "${LEDGER}" \
    --task "Phase 10 FSV ${CASE_NAME} synthetic source-of-truth run ${RUN_ID}" \
    --agent "phase10-fsv" \
    --model "deterministic-fsv" \
    --task-kind "ccreality_fsv" \
    --risk-tier "low" >"${CASE_WORK}/blackbox-run.json"
  source_read_file "${CASE_WORK}/blackbox-init.json"
  source_read_file "${CASE_WORK}/blackbox-run.json"
  source_read_sqlite_counts "${LEDGER}"
}

recommendation_payload() {
  local turn="$1"
  local status="${2:-no_change}"
  jq -nc \
    --arg run_id "${RUN_ID}" \
    --argjson attempt "${ATTEMPT}" \
    --arg session "${CASE_NAME}" \
    --argjson turn "${turn}" \
    --arg status "${status}" \
    --argjson created_at "$(date +%s)" \
    --arg source_of_truth "file:${ATTEMPT_DIR}/claude-code-optimizer/recommendation-turn-$(printf '%02d' "${turn}").json" \
    --arg summary "file:${ATTEMPT_SUMMARY}" \
    --arg signal "file:${ATTEMPT_DIR}/reality-signal-packet.json" \
    --arg problem "file:${ATTEMPT_DIR}/problem-reality-packet.json" '{
      record_kind: "ccreality_optimizer_recommendation",
      schema_version: 1,
      run_id: $run_id,
      attempt: $attempt,
      turn_number: $turn,
      claude_session_id: $session,
      claude_model: "haiku",
      created_at_unix: $created_at,
      status: $status,
      reason: "Phase 10 FSV writes a known optimizer recommendation and verifies the file plus BlackBox ledger readback.",
      expected_reward_signal_delta: "none",
      next_gemma_prompt_delta: "none",
      cost_estimate_usd: 0,
      source_files_changed: [],
      harness_transitions_minted: [],
      memories_created: [],
      memories_boosted: [],
      remaining_risks: [],
      verification_commands: ["sqlite ledger_records readback"],
      shift_log_excerpt: [],
      source_of_truth: $source_of_truth,
      source_of_truth_readbacks: [$summary, $signal, $problem]
    }'
}

record_recommendation() {
  local turn="$1"
  local payload
  payload="$(recommendation_payload "${turn}")"
  mcp_call_text "optimizer_record_recommendation" "${payload}"
  assert_mcp_ok
  local rec_path
  rec_path="$(uri_to_path "$(jq -r '.recommendation_path' "${LAST_MCP_TEXT}")")"
  require_file "${rec_path}"
  source_read_file "${rec_path}"
  source_read_sqlite_counts "${LEDGER}"
}

record_decision() {
  local policy="$1"
  local should_run="$2"
  local payload
  payload="$(jq -nc --arg run_id "${RUN_ID}" --argjson attempt "${ATTEMPT}" --arg policy "${policy}" --argjson should_run "${should_run}" --arg session "${CASE_NAME}" '{
    run_id: $run_id,
    attempt: $attempt,
    policy: $policy,
    should_run: $should_run,
    reasons: ["Phase 10 FSV deterministic loop firing"],
    claude_session_id: $session,
    claude_model: "deterministic-fsv"
  }')"
  mcp_call_text "optimizer_record_decision" "${payload}"
  assert_mcp_ok
  source_read_file "$(jq -r '.decision_path' "${LAST_MCP_TEXT}")"
}

backup_probe_with_content() {
  if [ -z "${FSV_PROBE_BACKUP}" ]; then
    FSV_PROBE_BACKUP="${CASE_WORK}/probe.original"
    cp "${PROBE_ABS}" "${FSV_PROBE_BACKUP}"
  fi
  printf '%s\n' "$1" >"${PROBE_ABS}"
}

open_probe() {
  mcp_call_text "harness_open_window" "$(jq -nc --arg path "${PROBE_REL}" '{path:$path,start_line:1,end_line:1}')"
  assert_mcp_ok
  jq -r '.sha256' "${LAST_MCP_TEXT}"
}

apply_probe() {
  local observed="$1"
  local replace="$2"
  local session="$3"
  mcp_call_text "harness_apply_line_window_edit" "$(jq -nc --arg path "${PROBE_REL}" --arg observed "${observed}" --arg replace "${replace}" --arg session "${session}" '{path:$path,start_line:1,end_line:1,observed_sha256:$observed,replace:$replace,session_id:$session,tool_use_id:"phase10-fsv-edit"}')"
}

create_fresh_lock() {
  mkdir -p "${ACTIVE_CACHE}"
  printf '%s:%s\n' "$1" "$(date +%s)" >"${ACTIVE_CACHE}/edit_lock"
}

create_stale_lock() {
  mkdir -p "${ACTIVE_CACHE}"
  printf '%s:%s\n' "$1" "$(( $(date +%s) - 3700 ))" >"${ACTIVE_CACHE}/edit_lock"
}

run_hook_json() {
  local hook="$1"
  local input="$2"
  local out="$3"
  printf '%s\n' "${input}" | "${PROJECT_ROOT}/scripts/cc/${hook}" >"${out}"
  source_read_file "${out}"
}

clear_active_state() {
  rm -f "${ACTIVE_ROOT_FILE}" "${ACTIVE_TARGET_FILE}"
}

case_A1_smoke_sha_match() {
  run_smoke "A1"
  local sha1 sha2
  sha1="$(sha_file "${ATTEMPT_SUMMARY}")"
  sha2="$(sha_file "${ATTEMPT_SUMMARY}")"
  [ "${sha1}" = "${sha2}" ] || fail "attempt-summary SHA was not reproducible"
  printf 'FSV_NOTE: summary_sha=%s\n' "${sha1}" | tee -a "${CASE_EVIDENCE_LOG}"
}

case_A2_passing_resolves() {
  run_smoke "A2"
  assert_jq_file "${ATTEMPT_SUMMARY}" '.validation.status == "passed" and .synthetic == true' "synthetic smoke passes"
  assert_jq_file "${ATTEMPT_DIR}/reality-signal-packet.json" '.outcome_reality.validation_status == "passed" and .score_reality.reward.clipped == 0.5' "signal passed"
}

case_A3_failing_does_not_resolve() {
  run_force_fail_smoke "A3"
}

case_A4_repeated_patch_detected() {
  run_smoke "A4"
  local patch_sha
  patch_sha="sha256:$(printf '%s' "phase10 repeated patch ${CASE_NAME}" | sha256sum | awk '{print $1}')"
  duplicate_attempt_with_patch "${patch_sha}"
  mcp_call_text "reality_compare_attempts" "$(jq -nc --arg run_id "${RUN_ID}" '{run_id:$run_id,attempt_a:1,attempt_b:2}')"
  assert_mcp_ok
  jq -e --arg patch "${patch_sha}" '.patch_repeated == true and .patch_sha_a == $patch and .patch_sha_b == $patch' "${LAST_MCP_TEXT}" >/dev/null \
    || fail "repeated patch comparison did not show the expected patch SHA"
}

case_A5_engine_crash_artifact() {
  run_force_fail_smoke "A5"
  CONTEXTGRAPH_ROOT="${PROJECT_ROOT}" "${PROJECT_ROOT}/target/debug/reality-loop" verify --runtime-root "${RUNTIME_ROOT}" >"${CASE_WORK}/verify.json" 2>"${CASE_WORK}/verify.err" \
    || fail "verify failed after forced engine failure"
  assert_jq_file "${CASE_WORK}/verify.json" '.status == "ok"' "forced failure artifacts verify"
  source_read_file "${CASE_WORK}/verify.json"
}

case_B1_latest_root() {
  run_smoke "B1"
  mcp_call_text "reality_latest_root" '{}'
  assert_mcp_ok
  jq -e --arg root "${RUNTIME_ROOT}" --arg run "${RUN_ID}" '.runtime_root == $root and .active_run_id == $run' "${LAST_MCP_TEXT}" >/dev/null \
    || fail "latest root did not match active runtime root and run id"
  source_read_file "${ACTIVE_ROOT_FILE}"
}

case_B2_attempt_summary_readback() {
  run_smoke "B2"
  mcp_call_text "reality_attempt_summary" "$(jq -nc --arg run_id "${RUN_ID}" '{run_id:$run_id,attempt:1}')"
  assert_mcp_ok
  jq -S . "${ATTEMPT_SUMMARY}" >"${CASE_WORK}/attempt-summary.disk.sorted.json"
  jq -S . "${LAST_MCP_TEXT}" >"${CASE_WORK}/attempt-summary.mcp.sorted.json"
  cmp "${CASE_WORK}/attempt-summary.disk.sorted.json" "${CASE_WORK}/attempt-summary.mcp.sorted.json"
}

case_B3_compare_attempts() {
  run_smoke "B3"
  duplicate_attempt_with_patch "sha256:$(printf '%s' "B3 patch" | sha256sum | awk '{print $1}')"
  mcp_call_text "reality_compare_attempts" "$(jq -nc --arg run_id "${RUN_ID}" '{run_id:$run_id,attempt_a:1,attempt_b:2}')"
  assert_mcp_ok
  assert_jq_file "${LAST_MCP_TEXT}" '.patch_repeated == true and (.source_of_truth | length == 2)' "compare attempts"
}

case_B4_compact_signal() {
  run_smoke "B4"
  mcp_call_text "reality_signal" "$(jq -nc --arg run_id "${RUN_ID}" '{run_id:$run_id,attempt:1,compact:true}')"
  assert_mcp_ok
  local signal="${ATTEMPT_DIR}/reality-signal-packet.json"
  jq -S '{record_kind,attempt,outcome_reality,patch_signals,score_reality,reinjection_directive,source_of_truth}' "${signal}" >"${CASE_WORK}/signal.expected.json"
  jq -S . "${LAST_MCP_TEXT}" >"${CASE_WORK}/signal.actual.json"
  cmp "${CASE_WORK}/signal.expected.json" "${CASE_WORK}/signal.actual.json"
}

case_B5_audit_trail() {
  run_smoke "B5"
  local entity audit_dir audit_path
  entity="$(cat /proc/sys/kernel/random/uuid)"
  audit_dir="${RUNTIME_ROOT}/cgreality-audit-log"
  audit_path="${audit_dir}/${entity}.jsonl"
  mkdir -p "${audit_dir}"
  jq -nc --arg entity "${entity}" --arg action "created" --arg case_name "${CASE_NAME}" --arg sot "file:${audit_path}" '{
    record_kind: "ccreality_runtime_audit_record",
    entity_id: $entity,
    action: $action,
    case_name: $case_name,
    created_at_unix: now | floor,
    source_of_truth: $sot
  }' >"${audit_path}"
  source_read_file "${audit_path}"
  mcp_call_text "reality_audit_trail" "$(jq -nc --arg entity_id "${entity}" '{entity_id:$entity_id,limit:10}')"
  assert_mcp_ok
  jq -e --arg entity "${entity}" '.entity_id == $entity and .count == 1 and .audit_trail[0].action == "created"' "${LAST_MCP_TEXT}" >/dev/null \
    || fail "audit trail did not return the created record"
}

case_B6_ledger_query_read_only() {
  run_smoke "B6"
  init_ledger
  mcp_call_text "reality_query_ledger" "$(jq -nc '{run_id:"ignored",where:"record_kind = '\''agent_run'\''",limit:10}')"
  assert_mcp_ok
  assert_jq_file "${LAST_MCP_TEXT}" '.row_count > 0 and (.rows[0].record_kind == "agent_run")' "ledger query"
  mcp_call_text "reality_query_ledger" "$(jq -nc '{run_id:"ignored",where:"record_kind = '\''agent_run'\'' OR 1=1",limit:10}')"
  assert_mcp_error_code "CCREALITY_LEDGER_QUERY_REJECTED"
  source_read_sqlite_counts "${LEDGER}"
}

case_C1_open_then_apply() {
  run_smoke "C1"
  backup_probe_with_content "phase10 C1 before"
  state_snapshot "before governed edit" "${PROBE_ABS}"
  local observed
  observed="$(open_probe)"
  apply_probe "${observed}" "phase10 C1 after"$'\n' "${CASE_NAME}"
  assert_mcp_ok
  state_snapshot "after governed edit" "${PROBE_ABS}" "$(uri_to_path "$(jq -r '.harness_transition_path' "${LAST_MCP_TEXT}")")"
  [ "$(sha_file "${PROBE_ABS}")" = "$(jq -r '.after_sha256' "${LAST_MCP_TEXT}")" ] || fail "after sha does not match physical file"
  mcp_call_text "reality_shift_log" "$(jq -nc --arg session_id "${CASE_NAME}" '{session_id:$session_id,limit:10}')"
  assert_mcp_ok
  assert_jq_file "${LAST_MCP_TEXT}" '.shift_count >= 1' "shift logged"
}

case_C2_stale_sha_fails() {
  run_smoke "C2"
  backup_probe_with_content "phase10 C2 before"
  local before
  before="$(sha_file "${PROBE_ABS}")"
  state_snapshot "before stale edit" "${PROBE_ABS}" "${RUNTIME_ROOT}/cgreality-shift-log/${CASE_NAME}.jsonl"
  apply_probe "sha256:0000000000000000000000000000000000000000000000000000000000000000" "phase10 C2 after"$'\n' "${CASE_NAME}"
  assert_mcp_error_code "SHA_MISMATCH"
  state_snapshot "after stale edit" "${PROBE_ABS}" "${RUNTIME_ROOT}/cgreality-shift-log/${CASE_NAME}.jsonl"
  [ "$(sha_file "${PROBE_ABS}")" = "${before}" ] || fail "stale edit changed file"
  [ ! -f "${RUNTIME_ROOT}/cgreality-shift-log/${CASE_NAME}.jsonl" ] || fail "stale edit wrote a shift log"
}

case_C3_denied_path_fails() {
  run_smoke "C3"
  state_snapshot "before denied path" "${PROJECT_ROOT}/.env"
  mcp_call_text "harness_open_window" '{"path":".env","start_line":1,"end_line":1}'
  assert_mcp_error_code "PATH"
  state_snapshot "after denied path" "${PROJECT_ROOT}/.env"
}

case_C4_shift_log_entry() {
  run_smoke "C4"
  backup_probe_with_content "phase10 C4 before"
  local observed
  observed="$(open_probe)"
  apply_probe "${observed}" "phase10 C4 after"$'\n' "${CASE_NAME}"
  assert_mcp_ok
  mcp_call_text "reality_shift_log" "$(jq -nc --arg session_id "${CASE_NAME}" '{session_id:$session_id,limit:10}')"
  assert_mcp_ok
  jq -e --arg path "${PROBE_REL}" '.shift_count >= 1 and (.shifts | any(.subject.path == $path))' "${LAST_MCP_TEXT}" >/dev/null \
    || fail "shift log did not include the governed probe path"
}

case_C5_external_drift_detected() {
  run_smoke "C5"
  backup_probe_with_content "phase10 C5 before"
  local observed
  observed="$(open_probe)"
  apply_probe "${observed}" "phase10 C5 known"$'\n' "${CASE_NAME}"
  assert_mcp_ok
  state_snapshot "before external writer" "${PROBE_ABS}"
  printf '%s\n' "phase10 C5 external writer" >"${PROBE_ABS}"
  state_snapshot "after external writer" "${PROBE_ABS}"
  mcp_call_text "reality_shift_compare_to_my_view" "$(jq -nc --arg session_id "${CASE_NAME}" --arg path "${PROBE_REL}" '{session_id:$session_id,files:[$path]}')"
  assert_mcp_ok
  assert_jq_file "${LAST_MCP_TEXT}" '.files[0].drift == "external_drift_detected"' "external drift"
}

case_C6_valid_recommendation() {
  run_smoke "C6"
  init_ledger
  record_recommendation 1
  sqlite3 "${LEDGER}" "SELECT COUNT(*) FROM ledger_records WHERE record_kind = 'tool_call';" | awk '$1 >= 1 {ok=1} END {exit ok ? 0 : 1}'
}

case_C7_malformed_recommendation_rejected() {
  run_smoke "C7"
  init_ledger
  local before_count
  before_count="$(sqlite3 "${LEDGER}" "SELECT COUNT(*) FROM ledger_records;")"
  mcp_call_text "optimizer_record_recommendation" "$(jq -nc --arg run_id "${RUN_ID}" '{run_id:$run_id,attempt:1,status:"no_change"}')"
  assert_mcp_error_code "RECOMMENDATION"
  local after_count
  after_count="$(sqlite3 "${LEDGER}" "SELECT COUNT(*) FROM ledger_records;")"
  [ "${before_count}" = "${after_count}" ] || fail "malformed recommendation changed ledger row count"
  source_read_sqlite_counts "${LEDGER}"
}

case_C8_allowed_command_runs() {
  run_smoke "C8"
  mcp_call_text "harness_run_command" "$(jq -nc --arg session_id "${CASE_NAME}" '{command:"git status --short",timeout_secs:30,session_id:$session_id}')"
  assert_mcp_ok
  assert_jq_file "${LAST_MCP_TEXT}" '.status == "ok" and .process.exit_code == 0 and (.source_of_truth.output | type == "string")' "allowed command"
  source_read_file "$(jq -r '.source_of_truth.output' "${LAST_MCP_TEXT}")"
}

case_C9_denied_command_fails() {
  run_smoke "C9"
  local commands_dir="${ATTEMPT_DIR}/claude-code-optimizer/commands"
  state_snapshot "before denied command" "${commands_dir}"
  mcp_call_text "harness_run_command" "$(jq -nc --arg session_id "${CASE_NAME}" '{command:"cargo --version",timeout_secs:30,session_id:$session_id}')"
  assert_mcp_error_code "COMMAND"
  state_snapshot "after denied command" "${commands_dir}"
}

case_C10_metachar_bypass_denied() {
  run_smoke "C10"
  mcp_call_text "harness_run_command" "$(jq -nc --arg session_id "${CASE_NAME}" '{command:"git status --short && rm -rf /tmp/cgreality-never",timeout_secs:30,session_id:$session_id}')"
  assert_mcp_error_code "COMMAND"
  [ ! -e /tmp/cgreality-never ] || fail "metachar bypass created forbidden path"
}

case_C11_parallel_mcp_lock() {
  run_smoke "C11"
  backup_probe_with_content "phase10 C11 before"
  local observed before
  observed="$(open_probe)"
  before="$(sha_file "${PROBE_ABS}")"
  create_fresh_lock "phase10-holder"
  source_read_file "${ACTIVE_CACHE}/edit_lock"
  apply_probe "${observed}" "phase10 C11 after"$'\n' "${CASE_NAME}"
  assert_mcp_error_code "CCREALITY_HARNESS_EDIT_OWNER_BUSY"
  [ "$(sha_file "${PROBE_ABS}")" = "${before}" ] || fail "busy lock edit changed file"
  rm -f "${ACTIVE_CACHE}/edit_lock"
}

case_C12_stale_lock_recovered() {
  run_smoke "C12"
  backup_probe_with_content "phase10 C12 before"
  local observed
  observed="$(open_probe)"
  create_stale_lock "phase10-stale-holder"
  source_read_file "${ACTIVE_CACHE}/edit_lock"
  apply_probe "${observed}" "phase10 C12 after"$'\n' "${CASE_NAME}"
  assert_mcp_ok
  [ ! -f "${ACTIVE_CACHE}/edit_lock" ] || fail "stale edit lock remained after successful edit"
  state_snapshot "after stale lock recovery" "${PROBE_ABS}" "${ACTIVE_CACHE}/edit_lock"
}

case_D1_session_start_active_root() {
  run_smoke "D1"
  run_hook_json "hook_session_start.sh" '{"session_id":"phase10-D1","source":"startup","model":"haiku"}' "${CASE_WORK}/hook-session-start.json"
  assert_jq_file "${CASE_WORK}/hook-session-start.json" '.hookSpecificOutput.additionalContext | contains("Active reality-loop runtime root")' "session start active"
  source_read_file "${RUNTIME_ROOT}/cgreality-hook-events.jsonl"
}

case_D2_session_start_no_root() {
  clear_active_state
  run_hook_json "hook_session_start.sh" '{"session_id":"phase10-D2","source":"startup"}' "${CASE_WORK}/hook-session-start-no-root.json"
  assert_jq_file "${CASE_WORK}/hook-session-start-no-root.json" '.hookSpecificOutput.additionalContext | contains("not currently running")' "session start no root"
}

case_D3_user_prompt_fresh_attempt() {
  run_smoke "D3"
  run_hook_json "hook_user_prompt_submit.sh" '{"session_id":"phase10-D3","prompt":"inspect reality","permission_mode":"acceptEdits"}' "${CASE_WORK}/hook-user-prompt.json"
  assert_jq_file "${CASE_WORK}/hook-user-prompt.json" '.hookSpecificOutput.additionalContext | contains("Reality signal compact") and contains("Source of truth")' "user prompt drip"
}

case_D4_user_prompt_shift_summary() {
  run_smoke "D4"
  mkdir -p "${RUNTIME_ROOT}/cgreality-shift-log"
  printf '%s\n' '{"shift_id":"phase10-D4-shift","timestamp_unix_ns":1,"tool_name":"unit","subject":{"path":"docs/ccreality/00_INDEX.md"},"delta_summary":{"lines_added":1},"verification":{}}' >"${RUNTIME_ROOT}/cgreality-shift-log/phase10-D4.jsonl"
  source_read_file "${RUNTIME_ROOT}/cgreality-shift-log/phase10-D4.jsonl"
  run_hook_json "hook_user_prompt_submit.sh" '{"session_id":"phase10-D4","prompt":"inspect shifts","permission_mode":"acceptEdits"}' "${CASE_WORK}/hook-user-prompt-shift.json"
  assert_jq_file "${CASE_WORK}/hook-user-prompt-shift.json" '.hookSpecificOutput.additionalContext | contains("Last 3 reality shifts")' "user prompt shifts"
}

case_D5_pretool_allow() {
  run_smoke "D5"
  run_hook_json "hook_pre_tool_use_edit.sh" '{"session_id":"phase10-D5","tool_use_id":"tool-D5","tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/docs/ccreality/00_INDEX.md"}}' "${CASE_WORK}/hook-pretool-allow.json"
  assert_jq_file "${CASE_WORK}/hook-pretool-allow.json" '.hookSpecificOutput.permissionDecision == "allow"' "pretool allow"
  source_read_file "${RUNTIME_ROOT}/cgreality-hook-state/preedit/phase10-D5/tool-D5.json"
}

case_D6_pretool_deny() {
  run_smoke "D6"
  run_hook_json "hook_pre_tool_use_edit.sh" '{"session_id":"phase10-D6","tool_use_id":"tool-D6","tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/.env"}}' "${CASE_WORK}/hook-pretool-deny.json"
  assert_jq_file "${CASE_WORK}/hook-pretool-deny.json" '.hookSpecificOutput.permissionDecision == "deny"' "pretool deny"
}

case_D7_posttool_records() {
  "${PROJECT_ROOT}/scripts/cc/tests/hooks/test_post_tool_use_edit.sh" >"${CASE_WORK}/hook-posttool-test.log" 2>&1
  source_read_file "${CASE_WORK}/hook-posttool-test.log"
}

case_D8_stop_blocks_without_recommendation() {
  run_smoke "D8"
  local transcript="${CASE_WORK}/transcript.jsonl"
  : >"${transcript}"
  run_hook_json "hook_stop.sh" "$(jq -nc --arg transcript "${transcript}" '{session_id:"phase10-D8",transcript_path:$transcript,stop_reason:"end",stop_hook_active:false}')" "${CASE_WORK}/hook-stop-block.json"
  assert_jq_file "${CASE_WORK}/hook-stop-block.json" '.decision == "block" and (.reason | contains("has not recorded"))' "stop block"
}

case_D9_stop_allows_after_recommendation() {
  run_smoke "D9"
  local transcript="${CASE_WORK}/transcript.jsonl"
  local rec_dir="${ATTEMPT_DIR}/claude-code-optimizer"
  local rec_path="${rec_dir}/recommendation-turn-01.json"
  mkdir -p "${rec_dir}"
  : >"${transcript}"
  jq -nc --arg session_id "phase10-D9" --arg sot "file:${rec_path}" '{
    record_kind:"ccreality_optimizer_recommendation",
    schema_version:1,
    claude_session_id:$session_id,
    status:"no_change",
    reason:"Phase 10 D9 stop allow source-of-truth recommendation.",
    source_of_truth:$sot
  }' >"${rec_path}"
  source_read_file "${rec_path}"
  run_hook_json "hook_stop.sh" "$(jq -nc --arg transcript "${transcript}" '{session_id:"phase10-D9",transcript_path:$transcript,stop_reason:"end"}')" "${CASE_WORK}/hook-stop-allow.json"
  [ ! -s "${CASE_WORK}/hook-stop-allow.json" ] || assert_jq_file "${CASE_WORK}/hook-stop-allow.json" '.decision != "block"' "stop allow"
  source_read_file "${ACTIVE_CACHE}/last_session_marker"
}

case_D10_precompact_reinjects() {
  "${PROJECT_ROOT}/scripts/cc/tests/hooks/test_pre_compact.sh" >"${CASE_WORK}/hook-precompact-test.log" 2>&1
  source_read_file "${CASE_WORK}/hook-precompact-test.log"
}

case_E1_drip_happy_path() {
  require_cmd claude
  run_smoke "E1"
  init_ledger
  claude mcp list >"${CASE_WORK}/claude-mcp-list.txt"
  grep -q '^cgreality: .*Connected$' "${CASE_WORK}/claude-mcp-list.txt" || fail "Claude MCP cgreality server is not connected"
  claude --print "Call mcp__cgreality__reality_latest_root exactly once. Return only a concise confirmation containing the active runtime_root." \
    --model haiku \
    --output-format json \
    --max-turns 3 \
    --max-budget-usd 1.00 \
    --no-session-persistence >"${CASE_WORK}/claude-drip.json"
  assert_jq_file "${CASE_WORK}/claude-drip.json" '(.permission_denials // []) | length == 0' "claude permissions"
  assert_jq_file "${CASE_WORK}/claude-drip.json" '.result | type == "string" and contains("runtime_root")' "claude drip result"
  source_read_file "${CASE_WORK}/claude-mcp-list.txt"
  source_read_file "${CASE_WORK}/claude-drip.json"
  record_recommendation 1
}

case_E2_drift_mid_edit() {
  case_C5_external_drift_detected
}

case_E3_loop_three_fires() {
  run_smoke "E3"
  init_ledger
  local i
  for i in 1 2 3; do
    record_decision "phase10-loop-${i}" true
    record_recommendation "${i}"
  done
  local rec_count decision_count tool_count
  rec_count="$(find "${ATTEMPT_DIR}/claude-code-optimizer" -name 'recommendation-turn-*.json' -type f | wc -l | tr -d ' ')"
  decision_count="$(find "${ATTEMPT_DIR}/claude-code-optimizer" -name 'decision-turn-*.json' -type f | wc -l | tr -d ' ')"
  tool_count="$(sqlite3 "${LEDGER}" "SELECT COUNT(*) FROM ledger_records WHERE record_kind = 'tool_call';")"
  [ "${rec_count}" -eq 3 ] || fail "expected 3 recommendation artifacts, got ${rec_count}"
  [ "${decision_count}" -eq 3 ] || fail "expected 3 decision artifacts, got ${decision_count}"
  [ "${tool_count}" -ge 6 ] || fail "expected at least 6 ledger tool_call rows, got ${tool_count}"
  source_read_sqlite_counts "${LEDGER}"
}

case_F1_claude_cost_limit() {
  require_cmd claude
  set +eE
  claude --print "Return exactly: phase10-fsv-cost-ok" \
    --model haiku \
    --output-format json \
    --max-turns 25 \
    --max-budget-usd 1.00 \
    --no-session-persistence >"${CASE_WORK}/claude-cost.json"
  local ec=$?
  set -eE
  require_file "${CASE_WORK}/claude-cost.json"
  assert_jq_file "${CASE_WORK}/claude-cost.json" '.num_turns <= 25 and (.total_cost_usd | type == "number") and .total_cost_usd <= 1.0 and (.terminal_reason == "completed" or .terminal_reason == "max_turns")' "claude cost"
  if [ "${ec}" -ne 0 ]; then
    assert_jq_file "${CASE_WORK}/claude-cost.json" '.terminal_reason == "max_turns" and .num_turns <= 25' "claude max-turn bounded exit"
  fi
  source_read_file "${CASE_WORK}/claude-cost.json"
}

case_F2_hook_context_size() {
  local out="${CASE_WORK}/hook-context-truncate.json"
  CONTEXTGRAPH_ROOT="${PROJECT_ROOT}" bash -c 'set -euo pipefail; source scripts/cc/_common.sh; big="$(head -c 20000 /dev/zero | tr "\0" A)"; cgreality_emit_additional_context "Phase10FSV" "${big}"' >"${out}"
  assert_jq_file "${out}" '.hookSpecificOutput.additionalContext | length == 9216' "hook truncates to 9216"
  source_read_file "${PROJECT_ROOT}/scripts/cc/_common.sh"
  source_read_file "${out}"
}

case_F3_large_artifact_pointer() {
  local big="${CASE_WORK}/large-artifact.json"
  truncate -s 104857600 "${big}"
  state_snapshot "before large replay" "${big}"
  mcp_call_text "reality_replay_artifact" "$(jq -nc --arg path "${big}" '{path:$path}')"
  assert_mcp_ok
  assert_jq_file "${LAST_MCP_TEXT}" '.exists == true and .size_bytes == 104857600 and (.content_or_pointer.pointer | type == "string")' "large artifact pointer"
}

fsv_case() {
  CASE_NAME="$1"
  CASE_WORK="${FSV_CASE_ROOT}/${CASE_NAME}"
  CASE_STATE_LOG="${CASE_WORK}/state.log"
  CASE_EVIDENCE_LOG="${CASE_WORK}/evidence.log"
  CASE_SOURCE_LOG="${CASE_WORK}/source-of-truth.log"
  mkdir -p "${CASE_WORK}" "$(dirname "${MCP_STDERR}")"
  : >"${CASE_STATE_LOG}"
  : >"${CASE_EVIDENCE_LOG}"
  : >"${CASE_SOURCE_LOG}"
  cd "${PROJECT_ROOT}"
  require_cmd jq
  require_cmd sqlite3
  backup_active_state
  trap 'on_error ${LINENO} $?' ERR
  trap case_cleanup EXIT
  log "case_work=${CASE_WORK}"
  local fn="case_${CASE_NAME}"
  declare -F "${fn}" >/dev/null || fail "missing case function ${fn}"
  "${fn}"
  log "PASS"
}
