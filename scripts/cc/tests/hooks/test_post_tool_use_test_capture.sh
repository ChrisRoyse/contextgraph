#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/cc/tests/hooks/_common_helpers.sh
source "${SCRIPT_DIR}/_common_helpers.sh"

backup_active_state
TMPROOT="$(mktempdir)"
trap 'rm -rf "${TMPROOT}"; restore_active_state' EXIT

export CLAUDE_PROJECT_DIR="/home/cabdru/contextgraph"
export CGREALITY_DROOT="${TMPROOT}/droot"
export CGREALITY_TEST_OUTCOME_DIR="${TMPROOT}/test-outcomes"
export CGREALITY_POSTTOOLUSE_CAPTURE_LOG="${TMPROOT}/posttooluse-mejepa-capture.jsonl"
export CGREALITY_CACHE_DIR="${TMPROOT}/state"
export CONTEXTGRAPH_MEJEPA_INFER_DB="${TMPROOT}/infer-db"
export CGREALITY_MEJEPA_VERIFY_TIMEOUT_SECONDS=1

mkdir -p "${CGREALITY_CACHE_DIR}" "${TMPROOT}/runtime/cgreality-shift-log"
printf '%s\n' "${TMPROOT}/runtime" > "${CGREALITY_CACHE_DIR}/active_runtime_root"
printf '%s\n' "synthetic-target" > "${CGREALITY_CACHE_DIR}/active_target_instance"

session_hex() {
  printf '%s' "$1" | sha256sum | awk '{print substr($1, 1, 32)}'
}

FAKE_CLI="${TMPROOT}/context-graph-cli"
FAKE_ARGS="${TMPROOT}/fake-cli-args.jsonl"
cat > "${FAKE_CLI}" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
jq -nc --arg args "$*" '{args: $args}' >> "${FAKE_CLI_ARGS}"
if [ "${FAKE_CLI_SLEEP:-0}" = "1" ]; then
  sleep 2
fi
printf '{"verifications_written":1}\n'
SH
chmod +x "${FAKE_CLI}"
export CGREALITY_CONTEXT_GRAPH_CLI="${FAKE_CLI}"
export FAKE_CLI_ARGS="${FAKE_ARGS}"

SESSION_RAW="sess-capture"
SESSION_ID="$(session_hex "${SESSION_RAW}")"
SHIFT_LOG="${TMPROOT}/runtime/cgreality-shift-log/${SESSION_ID}.jsonl"
jq -nc '{subject:{file_path:"src/demo.py"}} | @json' > "${SHIFT_LOG}"

pytest_payload() {
  jq -nc --arg session_id "${SESSION_RAW}" --arg tool_use_id "$1" --arg command "$2" --arg stdout "$3" '{
    session_id: $session_id,
    tool_use_id: $tool_use_id,
    tool_name: "Bash",
    tool_input: {command: $command},
    tool_response: {stdout: $stdout}
  }'
}

OUT="$(pytest_payload "tool-valid" "pytest tests/test_demo.py" "tests/test_demo.py::test_a FAILED" \
  | "${SCRIPT_DIR}/../../hook_post_tool_use_test_capture.sh")"
assert_jq "${OUT}" '.hookSpecificOutput.hookEventName' "PostToolUse" "valid_hook_event"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("wrote 1 event")' "true" "valid_context"
OUTCOME_LOG="${CGREALITY_TEST_OUTCOME_DIR}/${SESSION_ID}.jsonl"
assert_file "${OUTCOME_LOG}" "test_outcome_log"
jq -e 'select(.outcome == "fail" and .test_id == "tests/test_demo.py::test_a")' "${OUTCOME_LOG}" >/dev/null
assert_file "${CGREALITY_POSTTOOLUSE_CAPTURE_LOG}" "capture_log"
jq -e 'select(.event == "PostToolUseBashVerificationBound")' "${CGREALITY_POSTTOOLUSE_CAPTURE_LOG}" >/dev/null
jq -e 'select(.event == "PostToolUseBashTestCaptureRecorded")' "${CGREALITY_POSTTOOLUSE_CAPTURE_LOG}" >/dev/null
jq -e 'select(.args | contains("--edited-file src/demo.py"))' "${FAKE_ARGS}" >/dev/null

ARGS_BEFORE="$(wc -l < "${FAKE_ARGS}" | tr -d ' ')"
printf '%s\n' "$(pytest_payload "tool-non-test" "printf hi" "hello world")" \
  | "${SCRIPT_DIR}/../../hook_post_tool_use_test_capture.sh" >/dev/null
ARGS_AFTER="$(wc -l < "${FAKE_ARGS}" | tr -d ' ')"
if [ "${ARGS_BEFORE}" != "${ARGS_AFTER}" ]; then
  echo "FAIL [non_test_no_write]: fake CLI was called for non-test command" >&2
  exit 1
fi
jq -e 'select(.event == "PostToolUseBashTestCaptureIgnored")' "${CGREALITY_POSTTOOLUSE_CAPTURE_LOG}" >/dev/null

OUT="$(pytest_payload "tool-ambiguous" "pytest -q" "collected 1 item without a clear marker" \
  | "${SCRIPT_DIR}/../../hook_post_tool_use_test_capture.sh")"
assert_jq "${OUT}" '.hookSpecificOutput.additionalContext | contains("inconclusive event")' "true" "ambiguous_context"
jq -e 'select(.test_id == "ambiguous::no-clear-test-marker" and .outcome == "unknown")' "${OUTCOME_LOG}" >/dev/null
jq -e 'select(.event == "PostToolUseBashTestCaptureAmbiguousRecorded")' "${CGREALITY_POSTTOOLUSE_CAPTURE_LOG}" >/dev/null

ERR="${TMPROOT}/timeout.err"
export FAKE_CLI_SLEEP=1
set +e
pytest_payload "tool-timeout" "pytest tests/test_timeout.py" "tests/test_timeout.py::test_slow PASSED" \
  | "${SCRIPT_DIR}/../../hook_post_tool_use_test_capture.sh" >/dev/null 2>"${ERR}"
EC=$?
set -e
assert_exit 0 "${EC}" "timeout_hook_exit"
grep -q "MEJEPA_HOOK_TIMEOUT" "${ERR}"
jq -e 'select(.event == "PostToolUseBashVerificationBindFailed" and .payload.error_code == "MEJEPA_HOOK_TIMEOUT")' "${CGREALITY_POSTTOOLUSE_CAPTURE_LOG}" >/dev/null

echo "test_post_tool_use_test_capture.sh: PASS"
