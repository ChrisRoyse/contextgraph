#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

TMPROOT="$(mktempdir)"
trap 'rm -rf "${TMPROOT}"' EXIT

FAKE_CLI="${TMPROOT}/context-graph-cli"
FAKE_CALLS="${TMPROOT}/fake-cli-calls.log"
cat > "${FAKE_CLI}" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${FAKE_CALLS}"
case "${FAKE_MODE:-allowed}" in
  unverified)
    jq -nc '{
      tool: "context-graph-cli mejepa stop-self-verify",
      source_of_truth: {
        db_path: env.CONTEXTGRAPH_MEJEPA_INFER_DB,
        live_prediction_cf: "CF_MEJEPA_LIVE_PREDICTIONS",
        verification_cf: "CF_MEJEPA_PREDICTION_VERIFICATIONS",
        session_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        max_predictions: 50
      },
      session_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      predictions_scanned: 1,
      verified_prediction_count: 0,
      abstained_prediction_count: 0,
      unverified_count: 1,
      unverified_predictions: [{
        prediction_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        task_id: "task-stop",
        verdict: "fail",
        language: "python",
        created_at_unix_ms: 1779000000000,
        severity_score: 10.0,
        recommended_commands: ["pytest tests/test_stop.py::test_gate -q"]
      }],
      recommended_commands: ["pytest tests/test_stop.py::test_gate -q"],
      truncated_recommended_command_count: 0,
      block: true
    }'
    ;;
  fail)
    echo "synthetic CLI failure" >&2
    exit 1
    ;;
  timeout)
    sleep 3
    ;;
  allowed|*)
    jq -nc '{
      tool: "context-graph-cli mejepa stop-self-verify",
      source_of_truth: {
        db_path: env.CONTEXTGRAPH_MEJEPA_INFER_DB,
        live_prediction_cf: "CF_MEJEPA_LIVE_PREDICTIONS",
        verification_cf: "CF_MEJEPA_PREDICTION_VERIFICATIONS",
        session_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        max_predictions: 50
      },
      session_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      predictions_scanned: 1,
      verified_prediction_count: 1,
      abstained_prediction_count: 0,
      unverified_count: 0,
      unverified_predictions: [],
      recommended_commands: [],
      truncated_recommended_command_count: 0,
      block: false
    }'
    ;;
esac
SH
chmod +x "${FAKE_CLI}"

export FAKE_CLI FAKE_CALLS
export CGREALITY_CONTEXT_GRAPH_CLI="${FAKE_CLI}"
export CGREALITY_STOP_MEJEPA_LOG="${TMPROOT}/stop-mejepa-self-verify.jsonl"
export CGREALITY_STOP_MEJEPA_BLOCKED_DIR="${TMPROOT}/blocked-artifacts"
export CGREALITY_STOP_MEJEPA_BREAKER_STATE_DIR="${TMPROOT}/breaker-state"
export CONTEXTGRAPH_MEJEPA_INFER_DB="${TMPROOT}/infer-db"
mkdir -p "${CONTEXTGRAPH_MEJEPA_INFER_DB}"
export CGREALITY_MEJEPA_STOP_VERIFY_TIMEOUT_SECONDS=1
export CGREALITY_MEJEPA_STOP_VERIFY_TIMEOUT_BREAKER_THRESHOLD=2
export CGREALITY_MEJEPA_STOP_VERIFY_MAX_PREDICTIONS=50
export MEJEPA_DAEMON_CONFIG="${TMPROOT}/daemon.toml"

HOOK="${SCRIPT_DIR}/../../stop-mejepa-self-verify.sh"
STOP_JSON='{"session_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","stop_reason":"end","stop_hook_active":false}'
ACTIVE_JSON='{"session_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","stop_reason":"end","stop_hook_active":true}'

run_hook() {
  local input="$1"
  local stdout="$2"
  local stderr="$3"
  set +e
  "${HOOK}" <<< "${input}" > "${stdout}" 2> "${stderr}"
  local ec=$?
  set -e
  printf '%s' "${ec}"
}

export FAKE_MODE=fail
EC="$(run_hook "${ACTIVE_JSON}" "${TMPROOT}/active.out" "${TMPROOT}/active.err")"
assert_exit 0 "${EC}" "stop_self_verify_active_bypass"
if [ -f "${FAKE_CALLS}" ]; then
  echo "FAIL [stop_self_verify_active_bypass]: CLI should not be called" >&2
  exit 1
fi

printf 'stop_hook_enabled = false\n' > "${MEJEPA_DAEMON_CONFIG}"
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/disabled.out" "${TMPROOT}/disabled.err")"
assert_exit 0 "${EC}" "stop_self_verify_operator_disabled"
if [ -f "${FAKE_CALLS}" ]; then
  echo "FAIL [stop_self_verify_operator_disabled]: CLI should not be called" >&2
  exit 1
fi
rm -f "${MEJEPA_DAEMON_CONFIG}"

EC="$(run_hook 'not-json' "${TMPROOT}/invalid-json.out" "${TMPROOT}/invalid-json.err")"
assert_exit 2 "${EC}" "stop_self_verify_invalid_json_exit"
assert_jq "$(cat "${TMPROOT}/invalid-json.err")" '.error_code' "MEJEPA_STOP_HOOK_DAEMON_UNREACHABLE" "stop_self_verify_invalid_json_error"

rm -rf "${CONTEXTGRAPH_MEJEPA_INFER_DB}"
export FAKE_MODE=allowed
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/db-missing.out" "${TMPROOT}/db-missing.err")"
assert_exit 2 "${EC}" "stop_self_verify_db_missing_exit"
assert_jq "$(cat "${TMPROOT}/db-missing.err")" '.error_code' "MEJEPA_STOP_HOOK_DB_MISSING" "stop_self_verify_db_missing_error"
assert_jq "$(cat "${TMPROOT}/db-missing.err")" '.operator_action.required' "true" "stop_self_verify_db_missing_operator_action"
mkdir -p "${CONTEXTGRAPH_MEJEPA_INFER_DB}"

touch -d '1970-01-01 UTC' "${CONTEXTGRAPH_MEJEPA_INFER_DB}"
export CGREALITY_MEJEPA_STOP_VERIFY_STALE_DB_SECONDS=1
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/db-stale.out" "${TMPROOT}/db-stale.err")"
assert_exit 2 "${EC}" "stop_self_verify_db_stale_exit"
assert_jq "$(cat "${TMPROOT}/db-stale.err")" '.error_code' "MEJEPA_STOP_HOOK_DB_STALE" "stop_self_verify_db_stale_error"
unset CGREALITY_MEJEPA_STOP_VERIFY_STALE_DB_SECONDS
touch "${CONTEXTGRAPH_MEJEPA_INFER_DB}"

export CGREALITY_CONTEXT_GRAPH_CLI="${TMPROOT}/missing-context-graph-cli"
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/cli-missing.out" "${TMPROOT}/cli-missing.err")"
assert_exit 2 "${EC}" "stop_self_verify_cli_missing_exit"
assert_jq "$(cat "${TMPROOT}/cli-missing.err")" '.error_code' "MEJEPA_STOP_HOOK_CLI_MISSING" "stop_self_verify_cli_missing_error"
assert_jq "$(cat "${TMPROOT}/cli-missing.err")" '.operator_action.required' "true" "stop_self_verify_cli_missing_operator_action"
export CGREALITY_CONTEXT_GRAPH_CLI="${FAKE_CLI}"

export FAKE_MODE=fail
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/unreachable.out" "${TMPROOT}/unreachable.err")"
assert_exit 2 "${EC}" "stop_self_verify_unreachable_exit"
assert_jq "$(cat "${TMPROOT}/unreachable.err")" '.error_code' "MEJEPA_STOP_HOOK_DAEMON_UNREACHABLE" "stop_self_verify_unreachable_error"
assert_jq "$(cat "${TMPROOT}/unreachable.err")" '.operator_action.required' "true" "stop_self_verify_unreachable_operator_action"

export FAKE_MODE=timeout
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/timeout-1.out" "${TMPROOT}/timeout-1.err")"
assert_exit 2 "${EC}" "stop_self_verify_timeout_first_exit"
assert_jq "$(cat "${TMPROOT}/timeout-1.err")" '.error_code' "MEJEPA_STOP_HOOK_VERIFY_TIMEOUT" "stop_self_verify_timeout_first_error"
assert_jq "$(cat "${TMPROOT}/timeout-1.err")" '.breaker.timeout_count' "1" "stop_self_verify_timeout_first_count"
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/timeout-2.out" "${TMPROOT}/timeout-2.err")"
assert_exit 2 "${EC}" "stop_self_verify_timeout_breaker_exit"
assert_jq "$(cat "${TMPROOT}/timeout-2.err")" '.error_code' "MEJEPA_STOP_HOOK_TIMEOUT_BREAKER_OPEN" "stop_self_verify_timeout_breaker_error"
assert_jq "$(cat "${TMPROOT}/timeout-2.err")" '.breaker.open' "true" "stop_self_verify_timeout_breaker_open"
assert_jq "$(cat "${TMPROOT}/timeout-2.err")" '.operator_action.required' "true" "stop_self_verify_timeout_breaker_operator_action"

export FAKE_MODE=unverified
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/blocked.out" "${TMPROOT}/blocked.err")"
assert_exit 2 "${EC}" "stop_self_verify_block_exit"
assert_jq "$(cat "${TMPROOT}/blocked.err")" '.error_code' "MEJEPA_STOP_HOOK_UNVERIFIED_PREDICTIONS" "stop_self_verify_block_error"
assert_jq "$(cat "${TMPROOT}/blocked.err")" '.prediction_ids[0]' "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" "stop_self_verify_block_prediction_id"
assert_jq "$(cat "${TMPROOT}/blocked.err")" '.recommended_commands[0]' "pytest tests/test_stop.py::test_gate -q" "stop_self_verify_block_command"

export FAKE_MODE=allowed
EC="$(run_hook "${STOP_JSON}" "${TMPROOT}/allowed.out" "${TMPROOT}/allowed.err")"
assert_exit 0 "${EC}" "stop_self_verify_allowed_exit"

assert_file "${CGREALITY_STOP_MEJEPA_LOG}" "stop_self_verify_log"
jq -e 'select(.event == "MEJEPA_STOP_HOOK_DAEMON_UNREACHABLE")' "${CGREALITY_STOP_MEJEPA_LOG}" >/dev/null
jq -e 'select(.event == "MEJEPA_STOP_HOOK_DB_MISSING")' "${CGREALITY_STOP_MEJEPA_LOG}" >/dev/null
jq -e 'select(.event == "MEJEPA_STOP_HOOK_DB_STALE")' "${CGREALITY_STOP_MEJEPA_LOG}" >/dev/null
jq -e 'select(.event == "MEJEPA_STOP_HOOK_CLI_MISSING")' "${CGREALITY_STOP_MEJEPA_LOG}" >/dev/null
jq -e 'select(.event == "MEJEPA_STOP_HOOK_TIMEOUT_BREAKER_OPEN")' "${CGREALITY_STOP_MEJEPA_LOG}" >/dev/null
jq -e 'select(.event == "MEJEPA_STOP_HOOK_UNVERIFIED_PREDICTIONS")' "${CGREALITY_STOP_MEJEPA_LOG}" >/dev/null
jq -e 'select(.event == "MEJEPA_STOP_HOOK_ALLOWED")' "${CGREALITY_STOP_MEJEPA_LOG}" >/dev/null
find "${CGREALITY_STOP_MEJEPA_BLOCKED_DIR}" -type f -name '*MEJEPA_STOP_HOOK_TIMEOUT_BREAKER_OPEN.json' | grep -q .

echo "test_stop_mejepa_self_verify.sh: PASS"
