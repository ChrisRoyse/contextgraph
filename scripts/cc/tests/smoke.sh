#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$PROJECT_ROOT"

RUN_STAMP="${CGREALITY_SMOKE_RUN_STAMP:-$(date +%s)}"
RUN_DATE="$(date +%F)"
SMOKE_ROOT="${CGREALITY_SMOKE_ROOT:-/tmp/cgreality-smoke-${RUN_STAMP}}"
SESSION_ID="phase09-smoke-${RUN_STAMP}"
RESULTS_PATH="$PROJECT_ROOT/docs/ccreality/SMOKE_RESULTS_${RUN_DATE}.md"
REPORT_JSON="$SMOKE_ROOT/smoke-report.json"
LOG_PATH="$SMOKE_ROOT/smoke.log"
MCP_STDERR="$SMOKE_ROOT/mcp.stderr.log"
CLAUDE_LAYER3_JSON="$SMOKE_ROOT/claude-layer3.json"
PROBE_FILE="$PROJECT_ROOT/scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt"
PROBE_BACKUP="$SMOKE_ROOT/post_tool_use_target.backup"

mkdir -p "$SMOKE_ROOT"
: >"$MCP_STDERR"
exec > >(tee -a "$LOG_PATH") 2>&1

export CONTEXTGRAPH_ROOT="$PROJECT_ROOT"
export CGREALITY_GOVERNED_PATHS="$PROJECT_ROOT/config/ccreality_governed_paths.json"
export CGREALITY_RECOMMENDATION_SCHEMA="$PROJECT_ROOT/config/ccreality_recommendation_schema.json"

declare -a RESULT_ROWS=()
declare -a SOURCE_OF_TRUTH_READS=()
declare -a EDGE_ROWS=()

cleanup() {
  local status=$?
  if [[ -f "$PROBE_BACKUP" ]]; then
    cp "$PROBE_BACKUP" "$PROBE_FILE"
  fi
  return "$status"
}
trap cleanup EXIT

fail() {
  echo "SMOKE FAIL: $*" >&2
  exit 1
}

section() {
  echo
  echo "### $*"
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

record_layer() {
  local layer="$1"
  local proof="$2"
  RESULT_ROWS+=("| ${layer} | PASS | ${proof} |")
  echo "LAYER PASS: ${layer} :: ${proof}"
}

record_source_read() {
  SOURCE_OF_TRUTH_READS+=("$1")
  echo "SOURCE OF TRUTH READ: $1"
}

line_count() {
  local path="$1"
  if [[ -f "$path" ]]; then
    wc -l <"$path" | tr -d ' '
  else
    printf '0'
  fi
}

sha_or_missing() {
  local path="$1"
  if [[ -f "$path" ]]; then
    sha256sum "$path" | awk '{print "sha256:" $1}'
  else
    printf 'missing'
  fi
}

file_uri_to_path() {
  local uri="$1"
  [[ "$uri" == file:* ]] || fail "expected file URI, got: $uri"
  python3 - "$uri" <<'PY'
import sys
from urllib.parse import urlparse, unquote

uri = sys.argv[1]
parsed = urlparse(uri)
if parsed.scheme != "file":
    raise SystemExit(f"not a file URI: {uri}")
print(unquote(parsed.path))
PY
}

require_json_file() {
  local path="$1"
  [[ -s "$path" ]] || fail "expected non-empty JSON file: $path"
  jq -e . "$path" >/dev/null || fail "invalid JSON file: $path"
}

require_file_contains() {
  local path="$1"
  local needle="$2"
  [[ -f "$path" ]] || fail "expected file to exist: $path"
  rg -F "$needle" "$path" >/dev/null || fail "expected $path to contain: $needle"
}

require_file_uri_readable() {
  local uri="$1"
  local path
  path="$(file_uri_to_path "$uri")"
  [[ -s "$path" ]] || fail "source-of-truth file missing or empty: $path"
  printf '%s\n' "$path"
}

mcp_request_json() {
  local request_id="$1"
  local tool_name="$2"
  local args_json="$3"
  jq -nc \
    --arg id "$request_id" \
    --arg tool "$tool_name" \
    --argjson args "$args_json" \
    '{jsonrpc:"2.0", id:$id, method:"tools/call", params:{name:$tool, arguments:$args}}'
}

mcp_call_raw() {
  local tool_name="$1"
  local args_json="$2"
  local request raw call_tag
  call_tag="${tool_name}-$(date +%s%N)-$$"
  request="$(mcp_request_json "$call_tag" "$tool_name" "$args_json")"
  raw="$(printf '%s\n' "$request" | timeout 90s ./target/debug/context-graph-mcp --mode reality-loop 2>>"$MCP_STDERR")" || {
    echo "$raw" >"$SMOKE_ROOT/mcp-${call_tag}-failed.raw.json"
    fail "MCP call failed: $tool_name; stderr at $MCP_STDERR"
  }
  [[ -n "$raw" ]] || fail "MCP call produced empty stdout: $tool_name"
  printf '%s\n' "$raw" >"$SMOKE_ROOT/mcp-${call_tag}.raw.json"
  jq -e . >/dev/null <<<"$raw" || fail "MCP call did not return JSON: $tool_name"
  printf '%s\n' "$raw"
}

mcp_call_text() {
  local tool_name="$1"
  local args_json="$2"
  local raw text
  raw="$(mcp_call_raw "$tool_name" "$args_json")"
  if jq -e '.error' >/dev/null <<<"$raw"; then
    printf '%s\n' "$raw"
    return 0
  fi
  text="$(jq -er '.result.content[0].text' <<<"$raw")" || fail "MCP response missing result.content[0].text for $tool_name"
  printf '%s\n' "$text"
}

mcp_tools_json() {
  local request raw call_tag
  call_tag="tools-list-$(date +%s%N)-$$"
  request="$(jq -nc --arg id "$call_tag" '{jsonrpc:"2.0", id:$id, method:"tools/list"}')"
  raw="$(printf '%s\n' "$request" | timeout 90s ./target/debug/context-graph-mcp --mode reality-loop 2>>"$MCP_STDERR")" || {
    fail "MCP tools/list failed; stderr at $MCP_STDERR"
  }
  printf '%s\n' "$raw" >"$SMOKE_ROOT/mcp-tools-list.raw.json"
  jq -e . >/dev/null <<<"$raw" || fail "MCP tools/list did not return JSON"
  printf '%s\n' "$raw"
}

active_root_text() {
  mcp_call_text "reality_latest_root" "$(jq -nc '{}')"
}

ledger_rows() {
  local ledger="$1"
  sqlite3 "$ledger" "select record_kind, count(*) from ledger_records group by record_kind order by record_kind;"
}

ledger_total() {
  local ledger="$1"
  sqlite3 "$ledger" "select count(*) from ledger_records;"
}

recommendation_count() {
  local runtime_root="$1"
  [[ -d "$runtime_root" ]] || fail "recommendation runtime root missing: $runtime_root"
  find "$runtime_root" -type f -path '*/claude-code-optimizer/recommendation-turn-*.json' | wc -l | tr -d ' '
}

shift_line_count() {
  local runtime_root="$1"
  local session="$2"
  line_count "$runtime_root/cgreality-shift-log/${session}.jsonl"
}

print_state_pair() {
  local label="$1"
  local before="$2"
  local after="$3"
  echo "EDGE STATE ${label} BEFORE: ${before}"
  echo "EDGE STATE ${label} AFTER: ${after}"
  EDGE_ROWS+=("| ${label} | ${before} | ${after} |")
}

section "Layer 0 - Environment"
for cmd in bash jq sha256sum sqlite3 cargo shellcheck python3 claude timeout rg; do
  need_cmd "$cmd"
done
python3 - <<'PY'
import jsonschema
print(f"jsonschema {jsonschema.__version__}")
PY
bash --version | head -n 1
jq --version
sqlite3 --version | awk '{print "sqlite3 " $1}'
cargo --version
claude --version
record_layer "0 - Environment" "commands resolved; versions printed in $LOG_PATH"

section "Layer 1 - Reality Loop Binary"
cargo build --quiet --bin reality-loop
LAYER1_ROOT="$SMOKE_ROOT/layer1-runtime"
./target/debug/reality-loop smoke --runtime-root "$LAYER1_ROOT" --label "phase09-layer1" >"$SMOKE_ROOT/layer1-smoke.json"
require_json_file "$SMOKE_ROOT/layer1-smoke.json"
jq -e '.status == "ok"' "$SMOKE_ROOT/layer1-smoke.json" >/dev/null || fail "reality-loop smoke status was not ok"
LAYER1_ATTEMPT_SUMMARY="$(file_uri_to_path "$(jq -r '.source_of_truth.attempt_summary' "$SMOKE_ROOT/layer1-smoke.json")")"
LAYER1_ATTEMPT_DIR="$(dirname "$LAYER1_ATTEMPT_SUMMARY")"
[[ -d "$LAYER1_ATTEMPT_DIR" ]] || fail "Layer 1 attempt directory was not created"
for key in attempt_summary reality_signal problem_reality; do
  artifact_path="$(file_uri_to_path "$(jq -r --arg key "$key" '.source_of_truth[$key]' "$SMOKE_ROOT/layer1-smoke.json")")"
  require_json_file "$artifact_path"
  record_source_read "$artifact_path"
done
record_layer "1 - Reality loop" "runtime root $LAYER1_ROOT with attempt artifacts read back"

section "Layer 2 - MCP Server"
cargo build --quiet --bin context-graph --bin context-graph-mcp
TOOLS_RAW="$(mcp_tools_json)"
printf '%s\n' "$TOOLS_RAW" | jq '.result.tools' >"$SMOKE_ROOT/mcp-tools-list.json"
TOOL_COUNT="$(jq '.result.tools | length' "$SMOKE_ROOT/mcp-tools-list.raw.json")"
[[ "$TOOL_COUNT" -ge 30 ]] || fail "expected at least 30 MCP tools, got $TOOL_COUNT"
for tool in reality_latest_root reality_run_smoke reality_shift_compare_to_my_view harness_open_window harness_apply_line_window_edit harness_verify_state optimizer_record_recommendation; do
  jq -e --arg tool "$tool" '.result.tools[] | select(.name == $tool)' "$SMOKE_ROOT/mcp-tools-list.raw.json" >/dev/null || fail "MCP tools/list missing $tool"
done
record_source_read "$SMOKE_ROOT/mcp-tools-list.raw.json"
record_layer "2 - MCP server" "$TOOL_COUNT tools listed; required ccreality tools present"

section "Layer 3 - Claude Code MCP Registration"
claude mcp list >"$SMOKE_ROOT/claude-mcp-list.txt"
require_file_contains "$SMOKE_ROOT/claude-mcp-list.txt" "cgreality:"
require_file_contains "$SMOKE_ROOT/claude-mcp-list.txt" "Connected"
claude --print \
  "Call mcp__cgreality__reality_latest_root exactly once. Return only a concise confirmation containing the active runtime_root." \
  --output-format json \
  --max-turns 3 \
  --permission-mode acceptEdits \
  >"$CLAUDE_LAYER3_JSON"
require_json_file "$CLAUDE_LAYER3_JSON"
jq -e '(.permission_denials // []) | length == 0' "$CLAUDE_LAYER3_JSON" >/dev/null || fail "Claude reported permission denials in Layer 3"
jq -e '.result | type == "string" and contains("runtime_root")' "$CLAUDE_LAYER3_JSON" >/dev/null || fail "Claude result did not confirm runtime_root"
record_source_read "$SMOKE_ROOT/claude-mcp-list.txt"
record_source_read "$CLAUDE_LAYER3_JSON"
record_layer "3 - Claude MCP registration" "Claude reported cgreality Connected and invoked reality_latest_root"

section "Layer 4 - Hooks"
HOOK_SESSION_OUT="$SMOKE_ROOT/hook-session-start.json"
printf '%s\n' '{"session_id":"phase09-hook","source":"startup","model":"haiku"}' | \
  "$PROJECT_ROOT/scripts/cc/hook_session_start.sh" >"$HOOK_SESSION_OUT"
require_json_file "$HOOK_SESSION_OUT"
jq -e '.hookSpecificOutput.additionalContext | contains("Active reality-loop runtime root")' "$HOOK_SESSION_OUT" >/dev/null || fail "SessionStart hook did not emit active runtime context"
HOOK_LOG="$LAYER1_ROOT/cgreality-hook-events.jsonl"
HOOK_BEFORE="$(line_count "$HOOK_LOG")"
PRETOOL_ALLOW_OUT="$SMOKE_ROOT/hook-pretool-allow.json"
printf '%s\n' '{"session_id":"phase09-hook","tool_name":"Edit","tool_input":{"file_path":"docs/ccreality/00_INDEX.md"},"tool_use_id":"phase09-pretool"}' | \
  "$PROJECT_ROOT/scripts/cc/hook_pre_tool_use_edit.sh" >"$PRETOOL_ALLOW_OUT"
require_json_file "$PRETOOL_ALLOW_OUT"
jq -e '.hookSpecificOutput.permissionDecision == "allow"' "$PRETOOL_ALLOW_OUT" >/dev/null || fail "PreToolUse hook did not allow governed ccreality doc"
PREEDIT_STATE="$LAYER1_ROOT/cgreality-hook-state/preedit/phase09-hook/phase09-pretool.json"
require_json_file "$PREEDIT_STATE"
HOOK_AFTER="$(line_count "$HOOK_LOG")"
[[ "$HOOK_AFTER" -gt "$HOOK_BEFORE" ]] || fail "hook event log did not grow"
record_source_read "$HOOK_SESSION_OUT"
record_source_read "$PRETOOL_ALLOW_OUT"
record_source_read "$PREEDIT_STATE"
record_source_read "$HOOK_LOG"
record_layer "4 - Hooks" "SessionStart context, PreToolUse allow, and hook state file read back"

section "Layer 5 - Skill-Driven Full Loop"
SKILL_FILE="$PROJECT_ROOT/.claude/skills/ccreality-smoke/SKILL.md"
[[ -f "$SKILL_FILE" ]] || fail "ccreality smoke skill missing: $SKILL_FILE"
for needle in "optimizer_record_recommendation" "reality_run_smoke" "harness_verify_state" "source_of_truth_readbacks"; do
  require_file_contains "$SKILL_FILE" "$needle"
done
LAYER5_ROOT="$SMOKE_ROOT/layer5-skill-runtime"
RUN_SMOKE_TEXT="$(mcp_call_text "reality_run_smoke" "$(jq -nc --arg runtime_root "$LAYER5_ROOT" --arg label "phase09-layer5" --arg session "$SESSION_ID" '{runtime_root:$runtime_root,label:$label,session_id:$session}')")"
printf '%s\n' "$RUN_SMOKE_TEXT" >"$SMOKE_ROOT/layer5-run-smoke.json"
require_json_file "$SMOKE_ROOT/layer5-run-smoke.json"
jq -e '.status == "success" and .stdout.status == "ok"' "$SMOKE_ROOT/layer5-run-smoke.json" >/dev/null || fail "Layer 5 reality_run_smoke did not return success with stdout.status ok"
LATEST_TEXT="$(active_root_text)"
printf '%s\n' "$LATEST_TEXT" >"$SMOKE_ROOT/layer5-latest-root.json"
require_json_file "$SMOKE_ROOT/layer5-latest-root.json"
LAYER5_ACTIVE_ROOT="$(jq -r '.runtime_root' "$SMOKE_ROOT/layer5-latest-root.json")"
LAYER5_RUN_ID="$(jq -r '.active_run_id' "$SMOKE_ROOT/layer5-latest-root.json")"
LAYER5_TASK_ID="$(jq -r '.active_task_id' "$SMOKE_ROOT/layer5-latest-root.json")"
[[ "$LAYER5_ACTIVE_ROOT" == "$LAYER5_ROOT" ]] || fail "active root mismatch: expected $LAYER5_ROOT got $LAYER5_ACTIVE_ROOT"
[[ "$LAYER5_RUN_ID" != "null" && -n "$LAYER5_RUN_ID" ]] || fail "Layer 5 active run_id missing"
LAYER5_LEDGER="$LAYER5_ROOT/ledger.sqlite"
./target/debug/context-graph blackbox init --ledger "$LAYER5_LEDGER" >/dev/null
./target/debug/context-graph blackbox record run \
  --ledger "$LAYER5_LEDGER" \
  --task "Phase 9 ccreality smoke for reality run ${LAYER5_RUN_ID}" \
  --agent "cgreality_phase09_smoke" \
  --model "haiku" \
  --task-kind "ccreality_smoke_run" \
  --risk-tier "high" >"$SMOKE_ROOT/layer5-blackbox-run.json"
require_json_file "$SMOKE_ROOT/layer5-blackbox-run.json"
jq -e '.run_id | type == "string" and length > 0' "$SMOKE_ROOT/layer5-blackbox-run.json" >/dev/null || fail "blackbox record run did not return a run_id"
DECISION_TEXT="$(mcp_call_text "optimizer_record_decision" "$(jq -nc --arg session "$SESSION_ID" --arg model "haiku" --arg run_id "$LAYER5_RUN_ID" '{claude_session_id:$session,claude_model:$model,run_id:$run_id,attempt:1,turn_number:1,policy:"phase09_smoke",should_run:false,reason:"Phase 9 smoke records a deterministic no-change decision before analysis."}')")"
printf '%s\n' "$DECISION_TEXT" >"$SMOKE_ROOT/layer5-decision.json"
require_json_file "$SMOKE_ROOT/layer5-decision.json"
jq -e '.ledger_append.status == "ok" and (.ledger_record_id | type == "string")' "$SMOKE_ROOT/layer5-decision.json" >/dev/null || fail "optimizer_record_decision did not record"
SUMMARY_TEXT="$(mcp_call_text "reality_attempt_summary" "$(jq -nc --arg run_id "$LAYER5_RUN_ID" '{run_id:$run_id,attempt:1}')")"
SIGNAL_TEXT="$(mcp_call_text "reality_signal" "$(jq -nc --arg run_id "$LAYER5_RUN_ID" '{run_id:$run_id,attempt:1}')")"
PROBLEM_TEXT="$(mcp_call_text "reality_problem_packet" "$(jq -nc --arg run_id "$LAYER5_RUN_ID" '{run_id:$run_id,attempt:1}')")"
printf '%s\n' "$SUMMARY_TEXT" >"$SMOKE_ROOT/layer5-summary.json"
printf '%s\n' "$SIGNAL_TEXT" >"$SMOKE_ROOT/layer5-signal.json"
printf '%s\n' "$PROBLEM_TEXT" >"$SMOKE_ROOT/layer5-problem-packet.json"
for path in "$SMOKE_ROOT/layer5-summary.json" "$SMOKE_ROOT/layer5-signal.json" "$SMOKE_ROOT/layer5-problem-packet.json"; do
  require_json_file "$path"
done
SUMMARY_SOT="$(jq -r '.source_of_truth' "$SMOKE_ROOT/layer5-summary.json")"
SIGNAL_SOT="$(jq -r '.source_of_truth' "$SMOKE_ROOT/layer5-signal.json")"
PROBLEM_SOT="$(jq -r '.source_of_truth' "$SMOKE_ROOT/layer5-problem-packet.json")"
SUMMARY_FILE="$(require_file_uri_readable "$SUMMARY_SOT")"
SIGNAL_FILE="$(require_file_uri_readable "$SIGNAL_SOT")"
PROBLEM_FILE="$(require_file_uri_readable "$PROBLEM_SOT")"
OPEN_TEXT="$(mcp_call_text "harness_open_window" "$(jq -nc --arg path "docs/ccreality/00_INDEX.md" '{path:$path,start_line:1,end_line:5}')")"
printf '%s\n' "$OPEN_TEXT" >"$SMOKE_ROOT/layer5-open-window.json"
require_json_file "$SMOKE_ROOT/layer5-open-window.json"
jq -e '.status == "ok" and .start_line == 1 and .end_line == 5 and (.sha256 | type == "string") and (.source_of_truth | type == "string")' "$SMOKE_ROOT/layer5-open-window.json" >/dev/null || fail "harness_open_window did not return expected window"
OPEN_SOURCE_FILE="$(require_file_uri_readable "$(jq -r '.source_of_truth' "$SMOKE_ROOT/layer5-open-window.json")")"
COMMAND_TEXT="$(mcp_call_text "harness_run_command" "$(jq -nc --arg session "$SESSION_ID" '{session_id:$session,command:"git status --short",cwd:"."}')")"
printf '%s\n' "$COMMAND_TEXT" >"$SMOKE_ROOT/layer5-command.json"
require_json_file "$SMOKE_ROOT/layer5-command.json"
jq -e '.status == "ok" and .process.exit_code == 0' "$SMOKE_ROOT/layer5-command.json" >/dev/null || fail "harness_run_command git status --short failed"
COMMAND_OUTPUT_URI="$(jq -r '.source_of_truth.output' "$SMOKE_ROOT/layer5-command.json")"
COMMAND_OUTPUT_FILE="$(require_file_uri_readable "$COMMAND_OUTPUT_URI")"
VERIFY_TEXT="$(mcp_call_text "harness_verify_state" "$(jq -nc --arg session "$SESSION_ID" '{session_id:$session,scope:"reality_loop_only"}')")"
printf '%s\n' "$VERIFY_TEXT" >"$SMOKE_ROOT/layer5-verify-state.json"
require_json_file "$SMOKE_ROOT/layer5-verify-state.json"
jq -e '.status == "ok"' "$SMOKE_ROOT/layer5-verify-state.json" >/dev/null || fail "harness_verify_state did not pass"
VERIFY_LOG_URI="$(jq -r '.source_of_truth' "$SMOKE_ROOT/layer5-verify-state.json")"
VERIFY_LOG_FILE="$(require_file_uri_readable "$VERIFY_LOG_URI")"
RECOMMENDATION_TEXT="$(mcp_call_text "optimizer_record_recommendation" "$(jq -nc \
  --arg session "$SESSION_ID" \
  --arg model "haiku" \
  --arg run_id "$LAYER5_RUN_ID" \
  --arg summary "$SUMMARY_SOT" \
  --arg signal "$SIGNAL_SOT" \
  --arg problem "$PROBLEM_SOT" \
  --arg command "$COMMAND_OUTPUT_URI" \
  --arg verify "$VERIFY_LOG_URI" \
  --argjson created "$(date +%s)" \
  '{schema_version:1,record_kind:"ccreality_optimizer_recommendation",claude_session_id:$session,claude_model:$model,run_id:$run_id,attempt:1,turn_number:1,status:"no_change",reason:"Phase 9 deterministic smoke completed all read-only verification steps without requiring source changes.",source_files_changed:[],verification_commands:["git status --short","harness_verify_state reality_loop_only"],source_of_truth_readbacks:[$summary,$signal,$problem,$command,$verify],next_gemma_prompt_delta:"No prompt delta is required by the Phase 9 smoke.",expected_reward_signal_delta:"No reward signal delta is required by the Phase 9 smoke.",remaining_risks:[],memories_created:[],memories_boosted:[],harness_transitions_minted:[],shift_log_excerpt:[],cost_estimate_usd:0,created_at_unix:$created,source_of_truth:"file:/tmp/cgreality-pending-recommendation.json"}')")"
printf '%s\n' "$RECOMMENDATION_TEXT" >"$SMOKE_ROOT/layer5-recommendation-result.json"
require_json_file "$SMOKE_ROOT/layer5-recommendation-result.json"
jq -e '.ledger_append.status == "ok" and (.recommendation_path | type == "string")' "$SMOKE_ROOT/layer5-recommendation-result.json" >/dev/null || fail "optimizer_record_recommendation did not record"
RECOMMENDATION_URI="$(jq -r '.recommendation_path' "$SMOKE_ROOT/layer5-recommendation-result.json")"
RECOMMENDATION_PATH="$(file_uri_to_path "$RECOMMENDATION_URI")"
require_json_file "$RECOMMENDATION_PATH"
LEDGER_ROWS_FILE="$SMOKE_ROOT/layer5-ledger-rows.txt"
ledger_rows "$LAYER5_LEDGER" >"$LEDGER_ROWS_FILE"
require_file_contains "$LEDGER_ROWS_FILE" "agent_run|1"
require_file_contains "$LEDGER_ROWS_FILE" "tool_call|"
RECOMMENDATION_COUNT_AFTER="$(recommendation_count "$LAYER5_ROOT")"
[[ "$RECOMMENDATION_COUNT_AFTER" -ge 1 ]] || fail "Layer 5 recommendation file was not physically created"
record_source_read "$SKILL_FILE"
record_source_read "$SMOKE_ROOT/layer5-blackbox-run.json"
record_source_read "$SUMMARY_FILE"
record_source_read "$SIGNAL_FILE"
record_source_read "$PROBLEM_FILE"
record_source_read "$OPEN_SOURCE_FILE"
record_source_read "$COMMAND_OUTPUT_FILE"
record_source_read "$VERIFY_LOG_FILE"
record_source_read "$RECOMMENDATION_PATH"
record_source_read "$LAYER5_LEDGER"
record_source_read "$LEDGER_ROWS_FILE"
record_layer "5 - Full loop" "runtime, analysis, command, verify, recommendation, and ledger rows read back"

section "Layer 6 - Drift Detection"
cp "$PROBE_FILE" "$PROBE_BACKUP"
PROBE_ORIGINAL_SHA="$(sha_or_missing "$PROBE_FILE")"
DRIFT_SESSION="${SESSION_ID}-drift"
PROBE_OPEN_TEXT="$(mcp_call_text "harness_open_window" "$(jq -nc --arg path "scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt" '{path:$path,start_line:1,end_line:1}')")"
printf '%s\n' "$PROBE_OPEN_TEXT" >"$SMOKE_ROOT/layer6-probe-open.json"
require_json_file "$SMOKE_ROOT/layer6-probe-open.json"
PROBE_OBSERVED_SHA="$(jq -r '.sha256' "$SMOKE_ROOT/layer6-probe-open.json")"
SHIFTED_LINE="phase09-shifted-${RUN_STAMP}"
APPLY_TEXT="$(mcp_call_text "harness_apply_line_window_edit" "$(jq -nc --arg session "$DRIFT_SESSION" --arg path "scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt" --arg replace "$SHIFTED_LINE\n" --arg observed "$PROBE_OBSERVED_SHA" '{session_id:$session,tool_use_id:"phase09-drift-apply",path:$path,start_line:1,end_line:1,replace:$replace,observed_sha256:$observed,reason:"Phase 9 drift probe applies a governed edit before external mutation."}')")"
printf '%s\n' "$APPLY_TEXT" >"$SMOKE_ROOT/layer6-apply.json"
require_json_file "$SMOKE_ROOT/layer6-apply.json"
jq -e '.status == "ok" and (.before_sha256 | type == "string") and (.after_sha256 | type == "string") and (.harness_transition_path | type == "string")' "$SMOKE_ROOT/layer6-apply.json" >/dev/null || fail "harness_apply_line_window_edit did not report a durable mutation"
APPLY_TRANSITION_FILE="$(require_file_uri_readable "$(jq -r '.harness_transition_path' "$SMOKE_ROOT/layer6-apply.json")")"
APPLY_DIFF_FILE="$(require_file_uri_readable "$(jq -r '.git_diff_path' "$SMOKE_ROOT/layer6-apply.json")")"
PROBE_AFTER_HARNESS_SHA="$(sha_or_missing "$PROBE_FILE")"
APPLY_AFTER_SHA="$(jq -r '.after_sha256' "$SMOKE_ROOT/layer6-apply.json")"
[[ "$PROBE_AFTER_HARNESS_SHA" == "$APPLY_AFTER_SHA" ]] || fail "probe file sha did not match harness apply after sha"
printf 'phase09-external-drift-%s\n' "$RUN_STAMP" >"$PROBE_FILE"
PROBE_AFTER_EXTERNAL_SHA="$(sha_or_missing "$PROBE_FILE")"
COMPARE_TEXT="$(mcp_call_text "reality_shift_compare_to_my_view" "$(jq -nc --arg session "$DRIFT_SESSION" --arg path "scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt" '{session_id:$session,files:[$path]}')")"
printf '%s\n' "$COMPARE_TEXT" >"$SMOKE_ROOT/layer6-shift-compare.json"
require_json_file "$SMOKE_ROOT/layer6-shift-compare.json"
jq -e '.files[0].drift == "external_drift_detected"' "$SMOKE_ROOT/layer6-shift-compare.json" >/dev/null || fail "drift compare did not detect external drift"
cp "$PROBE_BACKUP" "$PROBE_FILE"
PROBE_RESTORED_SHA="$(sha_or_missing "$PROBE_FILE")"
[[ "$PROBE_RESTORED_SHA" == "$PROBE_ORIGINAL_SHA" ]] || fail "probe file was not restored after drift test"
print_state_pair "drift detection" "sha=${PROBE_AFTER_HARNESS_SHA}" "external_sha=${PROBE_AFTER_EXTERNAL_SHA}; drift=external_drift_detected; restored_sha=${PROBE_RESTORED_SHA}"
record_source_read "$SMOKE_ROOT/layer6-apply.json"
record_source_read "$APPLY_TRANSITION_FILE"
record_source_read "$APPLY_DIFF_FILE"
record_source_read "$SMOKE_ROOT/layer6-shift-compare.json"
record_source_read "$LAYER5_ROOT/cgreality-shift-log/${DRIFT_SESSION}.jsonl"
record_layer "6 - Drift detection" "external drift detected against governed shift log and probe restored"

section "Layer 7 - Deny Enforcement"
SETTINGS_FILE="$PROJECT_ROOT/.claude/settings.json"
jq -e '.permissions.deny[] | select(. == "Read(.env)" or . == "Edit(.env)" or . == "Write(.env)" or . == "MultiEdit(.env)")' "$SETTINGS_FILE" >/dev/null || fail ".claude/settings.json does not deny .env access"
ENV_BEFORE_SHA="$(sha_or_missing "$PROJECT_ROOT/.env")"
ACTIVE_HOOK_LOG="$LAYER5_ROOT/cgreality-hook-events.jsonl"
HOOK_LOG_BEFORE_DENY="$(line_count "$ACTIVE_HOOK_LOG")"
PRETOOL_DENY_OUT="$SMOKE_ROOT/hook-pretool-deny-env.json"
printf '%s\n' '{"session_id":"phase09-hook","tool_name":"Edit","tool_input":{"file_path":".env"},"tool_use_id":"phase09-deny-env"}' | \
  "$PROJECT_ROOT/scripts/cc/hook_pre_tool_use_edit.sh" >"$PRETOOL_DENY_OUT"
require_json_file "$PRETOOL_DENY_OUT"
jq -e '.hookSpecificOutput.permissionDecision == "deny"' "$PRETOOL_DENY_OUT" >/dev/null || fail "PreToolUse hook did not deny .env"
ENV_AFTER_SHA="$(sha_or_missing "$PROJECT_ROOT/.env")"
[[ "$ENV_AFTER_SHA" == "$ENV_BEFORE_SHA" ]] || fail ".env changed during deny enforcement test"
HOOK_LOG_AFTER_DENY="$(line_count "$ACTIVE_HOOK_LOG")"
[[ "$HOOK_LOG_AFTER_DENY" -gt "$HOOK_LOG_BEFORE_DENY" ]] || fail "hook event log did not grow for deny event"
print_state_pair "deny enforcement" ".env_sha=${ENV_BEFORE_SHA}; hook_lines=${HOOK_LOG_BEFORE_DENY}" ".env_sha=${ENV_AFTER_SHA}; hook_lines=${HOOK_LOG_AFTER_DENY}; decision=deny"
record_source_read "$SETTINGS_FILE"
record_source_read "$PRETOOL_DENY_OUT"
record_source_read "$ACTIVE_HOOK_LOG"
record_layer "7 - Deny enforcement" ".env denied by settings and hook, file state unchanged"

section "Layer 8 - Invalid Write Rejection"
LEDGER_BEFORE_INVALID="$(ledger_total "$LAYER5_LEDGER")"
RECS_BEFORE_INVALID="$(recommendation_count "$LAYER5_ROOT")"
SHIFT_BEFORE_INVALID="$(shift_line_count "$LAYER5_ROOT" "$SESSION_ID")"
INVALID_TEXT="$(mcp_call_text "optimizer_record_recommendation" "$(jq -nc --arg session "$SESSION_ID" --arg run_id "$LAYER5_RUN_ID" '{schema_version:1,record_kind:"ccreality_optimizer_recommendation",claude_session_id:$session,claude_model:"not-a-real-model",run_id:$run_id,attempt:1,turn_number:2,status:"no_change",reason:"invalid model should fail",source_files_changed:[],verification_commands:[],source_of_truth_readbacks:[],next_gemma_prompt_delta:"none",expected_reward_signal_delta:"none",remaining_risks:[],memories_created:[],memories_boosted:[],harness_transitions_minted:[],shift_log_excerpt:[],cost_estimate_usd:0,created_at_unix:1,source_of_truth:"file:/tmp/invalid.json"}')")"
printf '%s\n' "$INVALID_TEXT" >"$SMOKE_ROOT/layer8-invalid-recommendation.json"
require_json_file "$SMOKE_ROOT/layer8-invalid-recommendation.json"
jq -e '.status == "error" and .error_code == "CCREALITY_RECOMMENDATION_SCHEMA_INVALID"' "$SMOKE_ROOT/layer8-invalid-recommendation.json" >/dev/null || fail "invalid recommendation did not return schema error"
LEDGER_AFTER_INVALID="$(ledger_total "$LAYER5_LEDGER")"
RECS_AFTER_INVALID="$(recommendation_count "$LAYER5_ROOT")"
SHIFT_AFTER_INVALID="$(shift_line_count "$LAYER5_ROOT" "$SESSION_ID")"
[[ "$LEDGER_AFTER_INVALID" == "$LEDGER_BEFORE_INVALID" ]] || fail "ledger changed after invalid recommendation"
[[ "$RECS_AFTER_INVALID" == "$RECS_BEFORE_INVALID" ]] || fail "recommendation count changed after invalid recommendation"
[[ "$SHIFT_AFTER_INVALID" == "$SHIFT_BEFORE_INVALID" ]] || fail "shift log changed after invalid recommendation"
print_state_pair "invalid recommendation" "ledger=${LEDGER_BEFORE_INVALID}; recommendations=${RECS_BEFORE_INVALID}; shift_lines=${SHIFT_BEFORE_INVALID}" "ledger=${LEDGER_AFTER_INVALID}; recommendations=${RECS_AFTER_INVALID}; shift_lines=${SHIFT_AFTER_INVALID}; error=CCREALITY_RECOMMENDATION_SCHEMA_INVALID"
record_source_read "$SMOKE_ROOT/layer8-invalid-recommendation.json"
record_source_read "$LAYER5_LEDGER"
record_layer "8 - Invalid write rejection" "schema-invalid recommendation rejected with no ledger, file, or shift mutation"

section "Boundary Edge Case Audit"
EDGE_A_BEFORE="shift_lines=$(shift_line_count "$LAYER5_ROOT" "$SESSION_ID")"
EDGE_A_TEXT="$(mcp_call_text "reality_shift_compare_to_my_view" "$(jq -nc --arg session "$SESSION_ID" '{session_id:$session,files:{bad:true}}')")"
printf '%s\n' "$EDGE_A_TEXT" >"$SMOKE_ROOT/edge-empty-invalid-files.json"
require_json_file "$SMOKE_ROOT/edge-empty-invalid-files.json"
jq -e '.status == "error" and .error_code == "CCREALITY_ARG_FILES_NOT_ARRAY"' "$SMOKE_ROOT/edge-empty-invalid-files.json" >/dev/null || fail "invalid files edge case did not fail with CCREALITY_ARG_FILES_NOT_ARRAY"
EDGE_A_AFTER="shift_lines=$(shift_line_count "$LAYER5_ROOT" "$SESSION_ID"); error=CCREALITY_ARG_FILES_NOT_ARRAY"
print_state_pair "invalid files argument" "$EDGE_A_BEFORE" "$EDGE_A_AFTER"

EDGE_B_BEFORE="ledger=$(ledger_total "$LAYER5_LEDGER"); recommendations=$(recommendation_count "$LAYER5_ROOT")"
OVER_LIMIT_FILES="$(jq -nc '[range(0;26) | "scripts/cc/tests/over-limit-\(.).txt"]')"
EDGE_B_TEXT="$(mcp_call_text "optimizer_record_recommendation" "$(jq -nc --arg session "$SESSION_ID" --arg run_id "$LAYER5_RUN_ID" --argjson files "$OVER_LIMIT_FILES" '{schema_version:1,record_kind:"ccreality_optimizer_recommendation",claude_session_id:$session,claude_model:"haiku",run_id:$run_id,attempt:1,turn_number:3,status:"changed",reason:"over-limit source file list should fail schema validation",source_files_changed:$files,verification_commands:[],source_of_truth_readbacks:[],next_gemma_prompt_delta:"none",expected_reward_signal_delta:"none",remaining_risks:[],memories_created:[],memories_boosted:[],harness_transitions_minted:[],shift_log_excerpt:["synthetic"],cost_estimate_usd:0,created_at_unix:1,source_of_truth:"file:/tmp/over-limit.json"}')")"
printf '%s\n' "$EDGE_B_TEXT" >"$SMOKE_ROOT/edge-over-limit-recommendation.json"
require_json_file "$SMOKE_ROOT/edge-over-limit-recommendation.json"
jq -e '.status == "error" and .error_code == "CCREALITY_RECOMMENDATION_SCHEMA_INVALID"' "$SMOKE_ROOT/edge-over-limit-recommendation.json" >/dev/null || fail "over-limit recommendation did not fail schema validation"
EDGE_B_AFTER="ledger=$(ledger_total "$LAYER5_LEDGER"); recommendations=$(recommendation_count "$LAYER5_ROOT"); error=CCREALITY_RECOMMENDATION_SCHEMA_INVALID"
print_state_pair "over-limit recommendation" "$EDGE_B_BEFORE" "$EDGE_B_AFTER"
[[ "$(ledger_total "$LAYER5_LEDGER")" == "$(sed -E 's/^ledger=([0-9]+).*/\1/' <<<"$EDGE_B_BEFORE")" ]] || fail "ledger changed after over-limit recommendation"
[[ "$(recommendation_count "$LAYER5_ROOT")" == "$(sed -E 's/.*recommendations=([0-9]+).*/\1/' <<<"$EDGE_B_BEFORE")" ]] || fail "recommendation count changed after over-limit recommendation"

EDGE_C_BEFORE="hook_lines=$(line_count "$ACTIVE_HOOK_LOG")"
EDGE_C_OUT="$SMOKE_ROOT/edge-empty-hook-input.json"
printf '%s\n' '{}' | "$PROJECT_ROOT/scripts/cc/hook_pre_tool_use_edit.sh" >"$EDGE_C_OUT"
require_json_file "$EDGE_C_OUT"
jq -e '.hookSpecificOutput.permissionDecision == "allow"' "$EDGE_C_OUT" >/dev/null || fail "empty hook input did not produce allow decision"
EDGE_C_AFTER="hook_lines=$(line_count "$ACTIVE_HOOK_LOG"); decision=allow"
print_state_pair "empty hook input" "$EDGE_C_BEFORE" "$EDGE_C_AFTER"
[[ "$(line_count "$ACTIVE_HOOK_LOG")" -gt "$(sed -E 's/hook_lines=([0-9]+)/\1/' <<<"$EDGE_C_BEFORE")" ]] || fail "hook log did not grow for empty hook input"
record_source_read "$SMOKE_ROOT/edge-empty-invalid-files.json"
record_source_read "$SMOKE_ROOT/edge-over-limit-recommendation.json"
record_source_read "$EDGE_C_OUT"
record_source_read "$ACTIVE_HOOK_LOG"

section "Final Source Of Truth"
echo "Layer 5 ledger rows:"
cat "$LEDGER_ROWS_FILE"
echo "Layer 5 recommendation path: $RECOMMENDATION_PATH"
echo "Layer 5 task_id: $LAYER5_TASK_ID"
echo "Layer 5 run_id: $LAYER5_RUN_ID"
echo "Layer 5 runtime root: $LAYER5_ROOT"

{
  echo "# Smoke Results - $RUN_DATE"
  echo
  echo "- Run stamp: \`$RUN_STAMP\`"
  echo "- Session: \`$SESSION_ID\`"
  echo "- Smoke root: \`$SMOKE_ROOT\`"
  echo "- Log: \`$LOG_PATH\`"
  echo "- Report JSON: \`$REPORT_JSON\`"
  echo "- Source of truth: runtime files under \`$SMOKE_ROOT\`, hook logs under the active runtime root, and BlackBox ledger \`$LAYER5_LEDGER\`."
  echo
  echo "| Layer | Status | Physical readback |"
  echo "| --- | --- | --- |"
  printf '%s\n' "${RESULT_ROWS[@]}"
  echo
  echo "## Edge Cases"
  echo
  echo "| Case | Before | After |"
  echo "| --- | --- | --- |"
  printf '%s\n' "${EDGE_ROWS[@]}"
  echo
  echo "## Source-Of-Truth Reads"
  echo
  printf -- "- \`%s\`\n" "${SOURCE_OF_TRUTH_READS[@]}" | sort -u
  echo
  echo "## Ledger Rows"
  echo
  echo '```text'
  cat "$LEDGER_ROWS_FILE"
  echo '```'
} >"${RESULTS_PATH}.tmp"
mv "${RESULTS_PATH}.tmp" "$RESULTS_PATH"

jq -nc \
  --arg run_stamp "$RUN_STAMP" \
  --arg session "$SESSION_ID" \
  --arg smoke_root "$SMOKE_ROOT" \
  --arg log "$LOG_PATH" \
  --arg results "$RESULTS_PATH" \
  --arg runtime_root "$LAYER5_ROOT" \
  --arg run_id "$LAYER5_RUN_ID" \
  --arg task_id "$LAYER5_TASK_ID" \
  --arg recommendation_path "$RECOMMENDATION_PATH" \
  --arg ledger "$LAYER5_LEDGER" \
  --argjson layer_count "${#RESULT_ROWS[@]}" \
  --argjson edge_count "${#EDGE_ROWS[@]}" \
  '{run_stamp:$run_stamp,session:$session,smoke_root:$smoke_root,log:$log,results:$results,runtime_root:$runtime_root,run_id:$run_id,task_id:$task_id,recommendation_path:$recommendation_path,ledger:$ledger,layer_count:$layer_count,edge_count:$edge_count,status:"pass"}' \
  >"$REPORT_JSON"
require_json_file "$REPORT_JSON"
record_source_read "$RESULTS_PATH"
record_source_read "$REPORT_JSON"

section "Complete"
echo "Smoke pass."
