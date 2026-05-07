#!/usr/bin/env bash
# Consolidated hardening gate for ccreality phases 0-8.
set -euo pipefail

ROOT="${ROOT:-/home/cabdru/contextgraph}"
cd "${ROOT}"

export CONTEXTGRAPH_ROOT="${ROOT}"
export CGREALITY_GOVERNED_PATHS="${ROOT}/config/ccreality_governed_paths.json"
export CGREALITY_RECOMMENDATION_SCHEMA="${ROOT}/config/ccreality_recommendation_schema.json"

RUN_STAMP="${RUN_STAMP:-$(date +%s)}"
AUDIT_ROOT="${AUDIT_ROOT:-/tmp/cgreality-phase0-8-hardening-${RUN_STAMP}}"
FSV_ROOT="${FSV_ROOT:-/tmp/cgreality-phase0-8-fsv-${RUN_STAMP}}"
REPORT="${REPORT:-/tmp/ccreality-phase0-8-hardening-${RUN_STAMP}.json}"
PHASE2_SMOKE_ROOT="${AUDIT_ROOT}/phase02-smoke"
PHASE4_SMOKE_ROOT="${AUDIT_ROOT}/phase04-smoke"

section() {
  printf '\n=== %s ===\n' "$1"
}

require_cmd() {
  local name="$1"
  command -v "${name}" >/dev/null 2>&1 || {
    printf 'required command missing: %s\n' "${name}" >&2
    exit 1
  }
}

assert_no_rg_i() {
  local pattern="$1"
  local file="$2"
  if rg -inq "${pattern}" "${file}"; then
    printf 'forbidden pattern matched in %s: %s\n' "${file}" "${pattern}" >&2
    exit 1
  fi
}

assert_no_rg() {
  local pattern="$1"
  local file="$2"
  if rg -q "${pattern}" "${file}"; then
    printf 'forbidden pattern matched in %s: %s\n' "${file}" "${pattern}" >&2
    exit 1
  fi
}

assert_not_ignored() {
  local file="$1"
  if git check-ignore -q "${file}"; then
    printf 'file is unexpectedly ignored: %s\n' "${file}" >&2
    exit 1
  fi
}

mcp_raw() {
  local request="$1"
  printf '%s\n' "${request}" \
    | timeout 30s ./target/debug/context-graph-mcp --mode reality-loop 2>"${AUDIT_ROOT}/mcp-stderr.log"
}

mcp_tools_json() {
  mcp_raw '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
}

mcp_call_text() {
  local name="$1"
  local args_json="$2"
  local request
  request="$(jq -nc --arg name "${name}" --argjson args "${args_json}" '{
    jsonrpc: "2.0",
    id: 1,
    method: "tools/call",
    params: {
      name: $name,
      arguments: $args
    }
  }')"
  mcp_raw "${request}" | jq -er '.result.content[0].text'
}

jsonl_count() {
  local path="$1"
  if [ -f "${path}" ]; then
    wc -l < "${path}" | tr -d ' '
  else
    printf '0'
  fi
}

find_ledger() {
  find "$1" \( -name blackbox.sqlite -o -name ledger.sqlite \) -type f | sort | head -1
}

ledger_count() {
  local ledger="$1"
  sqlite3 "${ledger}" 'select count(*) from ledger_records;'
}

recommendation_file_count() {
  local attempt_dir="$1"
  find "${attempt_dir}/claude-code-optimizer" -name 'recommendation-turn-*.json' -type f 2>/dev/null | wc -l | tr -d ' '
}

source_file_from_uri() {
  sed 's#^file:##'
}

require_cmd cargo
require_cmd claude
require_cmd git
require_cmd jq
require_cmd python3
require_cmd rg
require_cmd shellcheck
require_cmd sqlite3
python3 -c 'import jsonschema' >/dev/null

mkdir -p "${AUDIT_ROOT}"

section "Phase 0 source-of-truth gate"
test -s /tmp/ccreality-cutover-base.sha
BASE_SHA="$(cat /tmp/ccreality-cutover-base.sha)"
grep -qE '^[0-9a-f]{40}$' <<<"${BASE_SHA}"
git rev-parse --quiet --verify ccreality-cutover >/dev/null
git merge-base --is-ancestor "${BASE_SHA}" ccreality-cutover
printf 'source_of_truth.phase0.base_sha=%s\n' "${BASE_SHA}"
printf 'source_of_truth.phase0.branch_head=%s\n' "$(git rev-parse ccreality-cutover)"

section "Phase 1 excision gate"
LEGACY_MODEL_PATTERN="$(printf '%s|%s|%s|%s' "deep""seek" "op""enai" "gpt""-5" "chat""gpt")"
LEGACY_FN_PATTERN="$(printf '%s|%s|%s|%s|%s' \
  "fn run_op""enai_loop_optimizer" \
  "fn decide_deep""seek_optimizer_trigger" \
  "fn deep""seek_chat_request_body" \
  "fn post_deep""seek_chat_completion" \
  "fn execute_op""enai_optimizer_tool")"
LEGACY_FSV_FN_PATTERN="$(printf '%s|%s|%s|%s|%s|%s|%s' \
  "fn deep""seek_trigger_decision_fsv" \
  "fn op""enai_replay_filters" \
  "fn op""enai_self_reality_packet_records" \
  "fn op""enai_error_classification" \
  "fn deep""seek_tool_schema" \
  "fn deep""seek_tool_call_replay" \
  "fn deep""seek_cost_preflight")"
assert_no_rg_i "${LEGACY_MODEL_PATTERN}" crates/context-graph-cli/src/bin/reality-loop.rs
test -f crates/context-graph-cli/src/governed_edit.rs
for symbol in project_file_window project_apply_line_window_edits validate_governed_path run_governed_command verify_project_state; do
  rg -q "pub fn ${symbol}" crates/context-graph-cli/src/governed_edit.rs
done
test -f crates/context-graph-cli/src/lib.rs
rg -q 'pub mod governed_edit' crates/context-graph-cli/src/lib.rs
cargo build -p context-graph-cli --bin reality-loop --quiet
cargo test -p context-graph-cli --bin reality-loop --quiet -- --skip ignored --test-threads=1
cargo test -p context-graph-cli --lib governed_edit --quiet
REALITY_LOOP_LINES="$(wc -l < crates/context-graph-cli/src/bin/reality-loop.rs | tr -d ' ')"
[ "${REALITY_LOOP_LINES}" -ge 6500 ] && [ "${REALITY_LOOP_LINES}" -le 8500 ]
assert_no_rg 'DEFAULT_OPENAI|DEEPSEEK_V4_PRO|DEFAULT_OPTIMIZER' crates/context-graph-cli/src/bin/reality-loop.rs
assert_no_rg "${LEGACY_FN_PATTERN}" crates/context-graph-cli/src/bin/reality-loop.rs
assert_no_rg "${LEGACY_FSV_FN_PATTERN}" crates/context-graph-cli/src/bin/reality-loop.rs
printf 'source_of_truth.phase1.reality_loop_lines=%s\n' "${REALITY_LOOP_LINES}"

section "Phase 2 engine subcommands gate"
cargo build -p context-graph-cli --bin reality-loop --quiet
HELP="$(./target/debug/reality-loop --help 2>&1)"
grep -q 'attempt' <<<"${HELP}"
grep -q 'smoke' <<<"${HELP}"
grep -q 'replay' <<<"${HELP}"
grep -q 'verify' <<<"${HELP}"
ATTEMPT_HELP="$(./target/debug/reality-loop attempt --help 2>&1)"
for flag in --task-manifest --runtime-root --max-attempts --official-eval-each-attempt; do
  grep -q -- "${flag}" <<<"${ATTEMPT_HELP}"
done
if grep -qiE "${LEGACY_MODEL_PATTERN}|optimizer-trigger|optimizer-each" <<<"${ATTEMPT_HELP}"; then
  printf 'forbidden optimizer flag survived in attempt help\n' >&2
  exit 1
fi
rm -rf "${PHASE2_SMOKE_ROOT}"
./target/debug/reality-loop smoke --runtime-root "${PHASE2_SMOKE_ROOT}" --label phase02-hardening >"${AUDIT_ROOT}/phase02-smoke.json" 2>"${AUDIT_ROOT}/phase02-smoke.err"
test -d "${PHASE2_SMOKE_ROOT}"
PHASE2_RUN_DIR="$(find "${PHASE2_SMOKE_ROOT}" -maxdepth 1 -mindepth 1 -type d | sort | head -1)"
test -n "${PHASE2_RUN_DIR}"
PHASE2_ATTEMPT_DIR="$(find "${PHASE2_RUN_DIR}" -name smoke-attempt-1 -type d | sort | head -1)"
test -n "${PHASE2_ATTEMPT_DIR}"
for artifact in attempt-summary.json reality-signal-packet.json problem-reality-packet.json; do
  test -s "${PHASE2_ATTEMPT_DIR}/${artifact}"
done
[ "$(cat "${HOME}/.cache/cgreality/active_runtime_root")" = "${PHASE2_SMOKE_ROOT}" ]
test -s "${HOME}/.cache/cgreality/active_target_instance"
./target/debug/reality-loop replay --attempt-summary-path "${PHASE2_ATTEMPT_DIR}/attempt-summary.json" --label phase02-hardening >"${AUDIT_ROOT}/phase02-replay.json" 2>"${AUDIT_ROOT}/phase02-replay.err"
test -s "${PHASE2_ATTEMPT_DIR}/reality-signal-packet.phase02-hardening.json"
./target/debug/reality-loop verify --runtime-root "${PHASE2_SMOKE_ROOT}" >"${AUDIT_ROOT}/phase02-verify.json" 2>"${AUDIT_ROOT}/phase02-verify.err"
JSON_COUNT="$(jq -er '.json_artifact_count' "${AUDIT_ROOT}/phase02-verify.json")"
[ "${JSON_COUNT}" -ge 3 ]
if ./target/debug/reality-loop smoke --runtime-root "${AUDIT_ROOT}/phase02-force-fail" --label fail-test --force-fail >"${AUDIT_ROOT}/phase02-fail.json" 2>"${AUDIT_ROOT}/phase02-fail.err"; then
  printf 'force-fail smoke unexpectedly succeeded\n' >&2
  exit 1
fi
grep -q 'RUST_REALITY_SMOKE_FORCED_FAIL' "${AUDIT_ROOT}/phase02-fail.err"
printf 'source_of_truth.phase2.attempt_dir=%s\n' "${PHASE2_ATTEMPT_DIR}"
printf 'source_of_truth.phase2.verify_json_artifacts=%s\n' "${JSON_COUNT}"

section "Phase 3 MCP analyze gate"
cargo build -p context-graph-mcp --quiet
for f in mod.rs analyze.rs path_policy.rs errors.rs helpers.rs; do
  test -f "crates/context-graph-mcp/src/handlers/tools/reality_loop/${f}"
done
./target/debug/context-graph-mcp --help 2>&1 | grep -q -- '--mode'
cargo test -p context-graph-mcp --lib reality_loop::path_policy --quiet
TOOLS_LIST="$(mcp_tools_json)"
ANALYZE_COUNT="$(jq -r '.result.tools[].name' <<<"${TOOLS_LIST}" | grep -cE '^reality_(latest_root|attempt_summary|official_report|problem_packet|signal|failure|trigger_decision|harness_transitions|compare_attempts|audit_trail|replay_artifact)$')"
[ "${ANALYZE_COUNT}" -ge 11 ]
LATEST_PAYLOAD="$(mcp_call_text reality_latest_root '{}')"
jq -e '.runtime_root and .success_predicate' <<<"${LATEST_PAYLOAD}" >/dev/null
PHASE3_RUN_ID="$(jq -r '.active_run_id' <<<"${LATEST_PAYLOAD}")"
PHASE3_TASK_ID="$(jq -r '.active_task_id' <<<"${LATEST_PAYLOAD}")"
[ "${PHASE3_RUN_ID}" != "null" ] && [ "${PHASE3_TASK_ID}" != "null" ]
SUMMARY="$(mcp_call_text reality_attempt_summary "$(jq -nc --arg run_id "${PHASE3_RUN_ID}" '{run_id: $run_id, attempt: 1}')")"
jq -e '.source_of_truth and .attempt' <<<"${SUMMARY}" >/dev/null
PROBLEM="$(mcp_call_text reality_problem_packet "$(jq -nc --arg run_id "${PHASE3_RUN_ID}" '{run_id: $run_id, compact: true}')")"
jq -e '.source_of_truth' <<<"${PROBLEM}" >/dev/null
REPLAY="$(mcp_call_text reality_replay_artifact "$(jq -nc --arg path "${PHASE2_ATTEMPT_DIR}/attempt-summary.json" '{path: $path}')")"
jq -e '.exists == true and (.sha256 | startswith("sha256:"))' <<<"${REPLAY}" >/dev/null
mcp_tools_json | jq -e '.result.tools | length > 0' >/dev/null
printf 'source_of_truth.phase3.active_run_id=%s\n' "${PHASE3_RUN_ID}"
printf 'source_of_truth.phase3.active_task_id=%s\n' "${PHASE3_TASK_ID}"

section "Phase 4 MCP interact alter shift gate"
cargo build -p context-graph-mcp --quiet
for f in alter.rs interact.rs shift_log.rs optimizer.rs lock.rs schema_validator.rs; do
  test -f "crates/context-graph-mcp/src/handlers/tools/reality_loop/${f}"
done
cargo test -p context-graph-mcp --lib reality_loop --quiet
TOOL_NAMES="$(mcp_tools_json | jq -r '.result.tools[].name')"
TOOL_COUNT="$(wc -l <<<"${TOOL_NAMES}" | tr -d ' ')"
[ "${TOOL_COUNT}" -ge 28 ]
for expected in \
  reality_run_attempt reality_run_smoke reality_run_official_eval reality_run_targeted_test \
  reality_recompile_reward reality_query_ledger \
  harness_open_window harness_apply_line_window_edit harness_run_command \
  harness_git_diff harness_git_status harness_verify_state \
  optimizer_record_decision optimizer_record_recommendation optimizer_record_harness_transition \
  reality_shift_log reality_shift_compare_to_my_view; do
  grep -qx "${expected}" <<<"${TOOL_NAMES}"
done
for forbidden in search_graph get_memory_neighbors get_audit_trail store_memory boost_importance forget_concept merge_concepts; do
  if grep -qx "${forbidden}" <<<"${TOOL_NAMES}"; then
    printf 'lightweight cgreality server advertised unimplemented/full-server tool: %s\n' "${forbidden}" >&2
    exit 1
  fi
done
rm -rf "${PHASE4_SMOKE_ROOT}"
RUN_SMOKE_RES="$(mcp_call_text reality_run_smoke "$(jq -nc --arg runtime_root "${PHASE4_SMOKE_ROOT}" '{runtime_root: $runtime_root, label: "phase04-hardening"}')")"
jq -e '.status == "success"' <<<"${RUN_SMOKE_RES}" >/dev/null
OPEN_RES="$(mcp_call_text harness_open_window '{"path":"docs/ccreality/00_INDEX.md","start_line":1,"end_line":5}')"
jq -er '.sha256' <<<"${OPEN_RES}" | grep -qE '^sha256:[a-f0-9]{64}$'
DENY_RES="$(mcp_call_text harness_open_window '{"path":".env","start_line":1,"end_line":5}')"
jq -e '.error_code == "CCREALITY_HARNESS_PATH_DENIED" or .error_code == "CCREALITY_GOVERNED_PROJECT_PATH_DENIED"' <<<"${DENY_RES}" >/dev/null
STALE_RES="$(mcp_call_text harness_apply_line_window_edit '{"path":"docs/ccreality/00_INDEX.md","start_line":1,"end_line":1,"observed_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000","replace":"# wrong"}')"
jq -e '.error_code | contains("SHA_MISMATCH")' <<<"${STALE_RES}" >/dev/null
SHIFT_RES="$(mcp_call_text reality_shift_log '{"session_id":"phase04-hardening","limit":5}')"
jq -e '.shifts | type == "array"' <<<"${SHIFT_RES}" >/dev/null
BAD_REC_RES="$(mcp_call_text optimizer_record_recommendation '{"status":"changed","reason":"missing required fields"}')"
jq -e '.error_code == "CCREALITY_RECOMMENDATION_SCHEMA_INVALID"' <<<"${BAD_REC_RES}" >/dev/null
LEDGER_DENIED_RES="$(mcp_call_text reality_query_ledger '{"run_id":"any","where":"1=1; DROP TABLE ledger_records"}')"
jq -e '.error_code == "CCREALITY_LEDGER_QUERY_REJECTED"' <<<"${LEDGER_DENIED_RES}" >/dev/null
rm -f "${HOME}/.cache/cgreality/edit_lock"
mcp_call_text harness_apply_line_window_edit '{"path":"docs/ccreality/00_INDEX.md","start_line":1,"end_line":1,"observed_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000","replace":"x"}' >/dev/null
test ! -f "${HOME}/.cache/cgreality/edit_lock"
mcp_tools_json | jq -e '.result.tools | length > 0' >/dev/null
printf 'source_of_truth.phase4.tool_count=%s\n' "${TOOL_COUNT}"

section "Phase 5 config gate"
jq -e . config/ccreality_governed_paths.json >/dev/null
jq -e . config/ccreality_recommendation_schema.json >/dev/null
jq -e '.allow | length > 0' config/ccreality_governed_paths.json >/dev/null
jq -e '.deny | length > 0' config/ccreality_governed_paths.json >/dev/null
jq -e '.deny | any(. == ".env")' config/ccreality_governed_paths.json >/dev/null
jq -e '.deny | any(. | startswith("swebench/"))' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. | startswith("crates/context-graph-cli/"))' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == ".gitignore")' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == "CLAUDE.md")' config/ccreality_governed_paths.json >/dev/null
jq -e '.["$schema"] | startswith("https://json-schema.org/draft/")' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.["$id"]' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.title == "ccreality optimizer recommendation"' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.required | any(. == "shift_log_excerpt")' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.properties.status.enum | type == "array"' config/ccreality_recommendation_schema.json >/dev/null
jq -e . docs/ccreality/phase-05-config-files/sample_recommendation.json >/dev/null
python3 - <<'PY'
import json
import jsonschema

with open("config/ccreality_recommendation_schema.json", encoding="utf-8") as handle:
    schema = json.load(handle)
with open("docs/ccreality/phase-05-config-files/sample_recommendation.json", encoding="utf-8") as handle:
    sample = json.load(handle)
jsonschema.Draft202012Validator.check_schema(schema)
jsonschema.validate(sample, schema)
try:
    jsonschema.validate({"status": "changed", "reason": "missing required fields"}, schema)
except jsonschema.ValidationError:
    pass
else:
    raise SystemExit("schema accepted an incomplete recommendation")
PY
printf 'source_of_truth.phase5.allow_count=%s\n' "$(jq '(.allow|length)' config/ccreality_governed_paths.json)"
printf 'source_of_truth.phase5.deny_count=%s\n' "$(jq '(.deny|length)' config/ccreality_governed_paths.json)"

section "Phase 6 hooks gate"
for script in _common.sh hook_session_start.sh hook_user_prompt_submit.sh hook_pre_tool_use_edit.sh hook_post_tool_use_edit.sh hook_pre_compact.sh hook_stop.sh; do
  test -x "scripts/cc/${script}"
done
for test_script in _common_helpers.sh test_session_start.sh test_user_prompt_submit.sh test_pre_tool_use_edit.sh test_post_tool_use_edit.sh test_pre_compact.sh test_stop.sh; do
  test -x "scripts/cc/tests/hooks/${test_script}"
done
for test_file in scripts/cc/tests/hooks/test_*.sh; do
  bash "${test_file}" >/dev/null
done
for hook in hook_session_start hook_user_prompt_submit hook_pre_compact; do
  OUT="$(printf '%s\n' '{"session_id":"phase6-hardening","source":"startup","prompt":"x","trigger":"auto","current_context_size":100}' | bash "scripts/cc/${hook}.sh")"
  if [ -n "${OUT}" ]; then
    jq -e . >/dev/null <<<"${OUT}"
  fi
done
ENV_DENY="$(printf '%s\n' '{"tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/.env"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${ENV_DENY}")" = "deny" ]
BENIGN_ALLOW="$(printf '%s\n' '{"tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/docs/ccreality/00_INDEX.md"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${BENIGN_ALLOW}")" = "allow" ]
ROOT_MEMORY_ALLOW="$(printf '%s\n' '{"tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/CLAUDE.md"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${ROOT_MEMORY_ALLOW}")" = "allow" ]
GITIGNORE_ALLOW="$(printf '%s\n' '{"tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/.gitignore"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${GITIGNORE_ALLOW}")" = "allow" ]
shellcheck scripts/cc/*.sh scripts/cc/tests/hooks/*.sh

section "Phase 7 Claude scaffolding gate"
jq -e . .claude/settings.json >/dev/null
jq -e '."$schema" == "https://json.schemastore.org/claude-code-settings.json"' .claude/settings.json >/dev/null
jq -e '.model and (.permissions.allow | type == "array") and (.permissions.deny | type == "array")' .claude/settings.json >/dev/null
jq -e '.permissions.allow | any(. == "mcp__cgreality__*")' .claude/settings.json >/dev/null
jq -e '.hooks.SessionStart and .hooks.UserPromptSubmit and .hooks.PreToolUse and .hooks.PostToolUse and .hooks.Stop' .claude/settings.json >/dev/null
jq -e '.env.CONTEXTGRAPH_ROOT' .claude/settings.json >/dev/null
jq -e . .mcp.json >/dev/null
jq -e '(.mcpServers | keys) == ["cgreality"]' .mcp.json >/dev/null
jq -e '.mcpServers.cgreality.type == "stdio"' .mcp.json >/dev/null
jq -e '.mcpServers.cgreality.command' .mcp.json >/dev/null
jq -e '.mcpServers.cgreality.args | any(. == "--mode")' .mcp.json >/dev/null
jq -r '.hooks.SessionStart[0].hooks[0].command' .claude/settings.json | grep -q 'scripts/cc/hook_session_start.sh'
jq -r '.hooks.UserPromptSubmit[0].hooks[0].command' .claude/settings.json | grep -q 'scripts/cc/hook_user_prompt_submit.sh'
jq -r '.hooks.PreToolUse[0].hooks[0].command' .claude/settings.json | grep -q 'scripts/cc/hook_pre_tool_use_edit.sh'
jq -r '.hooks.PostToolUse[0].hooks[0].command' .claude/settings.json | grep -q 'scripts/cc/hook_post_tool_use_edit.sh'
jq -r '.hooks.Stop[0].hooks[0].command' .claude/settings.json | grep -q 'scripts/cc/hook_stop.sh'
test -s .claude/loop.md
grep -q 'Iteration procedure' .claude/loop.md
grep -q 'mcp__cgreality__' .claude/loop.md
for agent in diagnostician harness-editor reward-analyst source-of-truth-readback swe-failure-triage; do
  agent_file=".claude/agents/cc-reality-${agent}.md"
  test -f "${agent_file}"
  head -1 "${agent_file}" | grep -q '^---$'
  grep -q "^name: cc-reality-${agent}$" "${agent_file}"
  grep -q '^description:' "${agent_file}"
  grep -q '^tools:' "${agent_file}"
  grep -q '^model:' "${agent_file}"
done
for skill in iterate smoke trigger-check readback; do
  skill_file=".claude/skills/ccreality-${skill}/SKILL.md"
  test -f "${skill_file}"
  head -1 "${skill_file}" | grep -q '^---$'
  grep -q "^name: ccreality-${skill}$" "${skill_file}"
  grep -q '^description:' "${skill_file}"
  DESC_LEN="$(grep '^description:' "${skill_file}" | head -1 | wc -c | tr -d ' ')"
  [ "${DESC_LEN}" -le 1100 ]
done
ALLOWED_TOOLS="$(mcp_tools_json | jq -r '.result.tools[].name')"
for agent_file in .claude/agents/cc-reality-*.md; do
  TOOLS_LINE="$(grep '^tools:' "${agent_file}" | head -1 | sed 's/^tools: *//')"
  for tool in $(printf '%s' "${TOOLS_LINE}" | tr ',' '\n' | tr -d ' '); do
    if [[ "${tool}" == mcp__cgreality__* ]]; then
      bare="${tool#mcp__cgreality__}"
      grep -qx "${bare}" <<<"${ALLOWED_TOOLS}" || {
        printf '%s references missing MCP tool %s\n' "${agent_file}" "${bare}" >&2
        exit 1
      }
    fi
  done
done
jq -e '.allow | any(. == ".claude/settings.json")' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == ".mcp.json")' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == ".gitignore")' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == "CLAUDE.md")' config/ccreality_governed_paths.json >/dev/null
claude mcp list | grep -q '^cgreality: .*Connected$'
CGREALITY_CACHE_DIR="${AUDIT_ROOT}/phase7-claude-cache" \
  claude --print 'Call mcp__cgreality__reality_latest_root exactly once. Return only compact JSON with active_run_id, runtime_root, and source_of_truth.' \
    --model haiku \
    --output-format json \
    --max-budget-usd 0.30 \
    --no-session-persistence \
  | jq -e '.permission_denials | length == 0' >/dev/null

section "Phase 8 project memory gate"
test -s CLAUDE.md
grep -q 'Outer Reality-Loop Optimizer Role' CLAUDE.md
grep -q 'Reality-shift principle' CLAUDE.md
grep -q 'mcp__cgreality__optimizer_record_recommendation' CLAUDE.md
HEADER_COUNT="$(grep -c 'Outer Reality-Loop Optimizer Role' CLAUDE.md)"
[ "${HEADER_COUNT}" -eq 1 ]
LAST_HEADER_LINE="$(grep -n '^## ' CLAUDE.md | tail -1)"
grep -q 'ccreality' <<<"${LAST_HEADER_LINE}"
assert_not_ignored CLAUDE.md
git check-ignore -q CLAUDE.local.md
git check-ignore -q clipcannon/CLAUDE.md
jq -e '.allow | any(. == "CLAUDE.md")' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == ".gitignore")' config/ccreality_governed_paths.json >/dev/null
SRC_START="$(grep -n '^---$' docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md | head -1 | cut -d: -f1)"
SRC_END="$(grep -n '^---$' docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md | tail -1 | cut -d: -f1)"
sed -n "${SRC_START},${SRC_END}p" docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md >"${AUDIT_ROOT}/phase8-source-addendum.md"
CLAUDE_START="$(grep -n 'Outer Reality-Loop Optimizer Role$' CLAUDE.md | cut -d: -f1)"
CLAUDE_START=$((CLAUDE_START - 2))
tail -n +"${CLAUDE_START}" CLAUDE.md >"${AUDIT_ROOT}/phase8-claude-addendum.md"
diff -u "${AUDIT_ROOT}/phase8-source-addendum.md" "${AUDIT_ROOT}/phase8-claude-addendum.md" >/dev/null
rm -rf "${AUDIT_ROOT}/phase8-claude-cache"
mkdir -p "${AUDIT_ROOT}/phase8-claude-cache"
CGREALITY_CACHE_DIR="${AUDIT_ROOT}/phase8-claude-cache" \
  claude --print 'From your loaded project memory, return exactly the heading line of the ccreality section and the next two non-empty lines. Do not use tools. Return only those three lines.' \
    --model haiku \
    --output-format json \
    --max-budget-usd 0.25 \
    --no-session-persistence \
    --tools "" \
  | jq -r '.result' \
  | grep -q 'CLAUDE.md project-memory discovery'

section "Full State Verification source of truth"
cargo build -p context-graph-cli --bin context-graph --quiet
rm -rf "${FSV_ROOT}"
./target/debug/reality-loop smoke --runtime-root "${FSV_ROOT}" --label phase0-8-fsv >"${AUDIT_ROOT}/fsv-smoke.json" 2>"${AUDIT_ROOT}/fsv-smoke.err"
FSV_LATEST="$(mcp_call_text reality_latest_root '{}')"
FSV_RUN_ID="$(jq -r '.active_run_id' <<<"${FSV_LATEST}")"
FSV_TASK_ID="$(jq -r '.active_task_id' <<<"${FSV_LATEST}")"
[ "${FSV_RUN_ID}" != "null" ] && [ "${FSV_TASK_ID}" != "null" ]
FSV_ATTEMPT_DIR="${FSV_ROOT}/${FSV_RUN_ID}/${FSV_TASK_ID}/smoke-attempt-1"
test -d "${FSV_ATTEMPT_DIR}"
FSV_LEDGER="${FSV_ROOT}/ledger.sqlite"
./target/debug/context-graph blackbox init --ledger "${FSV_LEDGER}" >/dev/null
./target/debug/context-graph blackbox record run \
  --ledger "${FSV_LEDGER}" \
  --task "ccreality phase 0-8 hardening full-state verification" \
  --agent "cgreality_phase0_8_hardening" \
  --model "manual-fsv" \
  --task-kind "ccreality_hardening_audit" \
  --risk-tier "high" >/dev/null
test -s "${FSV_LEDGER}"
printf 'FSV SOURCE OF TRUTH runtime_root=%s\n' "${FSV_ROOT}"
printf 'FSV SOURCE OF TRUTH attempt_dir=%s\n' "${FSV_ATTEMPT_DIR}"
printf 'FSV SOURCE OF TRUTH ledger=%s\n' "${FSV_LEDGER}"
printf 'FSV BEFORE ledger rows=%s\n' "$(ledger_count "${FSV_LEDGER}")"
sqlite3 "${FSV_LEDGER}" 'select record_kind || "|" || count(*) from ledger_records group by record_kind order by record_kind;'

FSV_SESSION="phase0-8-fsv-${RUN_STAMP}"
TRANSITION_RES="$(mcp_call_text optimizer_record_harness_transition "$(jq -nc --arg run_id "${FSV_RUN_ID}" --arg session "${FSV_SESSION}" '{
  run_id: $run_id,
  attempt: 1,
  claude_session_id: $session,
  action: "phase0_8_hardening_fsv_readback",
  outcome: "recorded",
  before: {
    phase0_8_gate: "not_yet_recorded"
  },
  after: {
    phase0_8_gate: "recording_recommendation"
  }
}')")"
if ! jq -e '.transition_path' <<<"${TRANSITION_RES}" >/dev/null; then
  printf 'optimizer_record_harness_transition failed: %s\n' "${TRANSITION_RES}" >&2
  exit 1
fi
TRANSITION_PATH="$(jq -r '.transition_path' <<<"${TRANSITION_RES}" | source_file_from_uri)"
test -s "${TRANSITION_PATH}"
printf 'FSV AFTER transition_path=%s\n' "${TRANSITION_PATH}"

GOOD_REC="${AUDIT_ROOT}/fsv-good-recommendation.json"
NOW_UNIX="$(date +%s)"
jq -nc \
  --arg session "${FSV_SESSION}" \
  --arg run_id "${FSV_RUN_ID}" \
  --arg summary_sot "file:${FSV_ATTEMPT_DIR}/attempt-summary.json" \
  --arg signal_sot "file:${FSV_ATTEMPT_DIR}/reality-signal-packet.json" \
  --arg transition_sot "file:${TRANSITION_PATH}" \
  --arg config_sot "file:${ROOT}/config/ccreality_governed_paths.json" \
  --argjson created_at "${NOW_UNIX}" '{
    schema_version: 1,
    record_kind: "ccreality_optimizer_recommendation",
    claude_session_id: $session,
    claude_model: "haiku",
    run_id: $run_id,
    attempt: 1,
    turn_number: 1,
    status: "changed",
    reason: "Phase 0-8 hardening audit verified branch ancestry, engine/MCP/runtime artifacts, governed path policy, Claude Code settings, project memory, and physical ledger writes.",
    source_files_changed: [
      "config/ccreality_governed_paths.json",
      "docs/ccreality/phase-00-foundation/04_VERIFICATION.md"
    ],
    verification_commands: [
      "scripts/cc/tests/verify_phase0_8_hardening.sh",
      "sqlite3 ledger readback",
      "claude mcp list"
    ],
    source_of_truth_readbacks: [
      $summary_sot,
      $signal_sot,
      $transition_sot,
      $config_sot
    ],
    next_inner_prompt_delta: "No inner-actor prompt delta; this recommendation records outer-loop hardening state only.",
    expected_reward_signal_delta: "No SWE-bench reward delta expected from this audit-only recommendation.",
    remaining_risks: [],
    memories_created: [],
    memories_boosted: [],
    harness_transitions_minted: [
      $transition_sot
    ],
    shift_log_excerpt: [
      "phase0-8-hardening-fsv"
    ],
    created_at_unix: $created_at,
    source_of_truth: "file:/tmp/placeholder/recommendation-turn-01.json"
  }' >"${GOOD_REC}"

GOOD_RES="$(mcp_call_text optimizer_record_recommendation "$(cat "${GOOD_REC}")")"
if ! jq -e '.recommendation_path' <<<"${GOOD_RES}" >/dev/null; then
  printf 'optimizer_record_recommendation failed: %s\n' "${GOOD_RES}" >&2
  exit 1
fi
REC_PATH="$(jq -r '.recommendation_path' <<<"${GOOD_RES}" | source_file_from_uri)"
test -s "${REC_PATH}"
jq -e '.record_kind == "ccreality_optimizer_recommendation" and .run_id == "'"${FSV_RUN_ID}"'" and .turn_number == 1' "${REC_PATH}" >/dev/null
printf 'FSV AFTER recommendation_path=%s\n' "${REC_PATH}"
printf 'FSV AFTER recommendation_sha=%s\n' "$(sha256sum "${REC_PATH}" | awk '{print "sha256:" $1}')"
printf 'FSV AFTER ledger rows=%s\n' "$(ledger_count "${FSV_LEDGER}")"
sqlite3 "${FSV_LEDGER}" 'select record_kind || "|" || count(*) from ledger_records group by record_kind order by record_kind;'
sqlite3 -line "${FSV_LEDGER}" 'select sequence, record_kind, run_id, substr(payload_json, 1, 240) as payload_prefix from ledger_records order by sequence desc limit 5;'
REC_LEDGER_MATCH="$(sqlite3 "${FSV_LEDGER}" "select count(*) from ledger_records where record_kind = 'tool_call' and payload_json like '%ccreality_optimizer_recommendation%';")"
[ "${REC_LEDGER_MATCH}" -ge 1 ]
SHIFT_LOG="${FSV_ROOT}/cgreality-shift-log/phase0-8-fsv-${RUN_STAMP}.jsonl"
test -s "${SHIFT_LOG}"
grep -q 'optimizer_record_recommendation' "${SHIFT_LOG}"
printf 'FSV AFTER shift_log=%s lines=%s\n' "${SHIFT_LOG}" "$(jsonl_count "${SHIFT_LOG}")"
tail -n 3 "${SHIFT_LOG}"

section "Boundary and edge case audit"
EDGE_BAD_REC_BEFORE_ROWS="$(ledger_count "${FSV_LEDGER}")"
EDGE_BAD_REC_BEFORE_FILES="$(recommendation_file_count "${FSV_ATTEMPT_DIR}")"
EDGE_BAD_REC_BEFORE_SHIFT_LINES="$(jsonl_count "${SHIFT_LOG}")"
printf 'EDGE bad_recommendation BEFORE rows=%s files=%s shift_lines=%s\n' "${EDGE_BAD_REC_BEFORE_ROWS}" "${EDGE_BAD_REC_BEFORE_FILES}" "${EDGE_BAD_REC_BEFORE_SHIFT_LINES}"
EDGE_BAD_REC_RES="$(mcp_call_text optimizer_record_recommendation '{"status":"changed","reason":"missing required fields"}')"
jq -e '.error_code == "CCREALITY_RECOMMENDATION_SCHEMA_INVALID"' <<<"${EDGE_BAD_REC_RES}" >/dev/null
EDGE_BAD_REC_AFTER_ROWS="$(ledger_count "${FSV_LEDGER}")"
EDGE_BAD_REC_AFTER_FILES="$(recommendation_file_count "${FSV_ATTEMPT_DIR}")"
EDGE_BAD_REC_AFTER_SHIFT_LINES="$(jsonl_count "${SHIFT_LOG}")"
printf 'EDGE bad_recommendation AFTER rows=%s files=%s shift_lines=%s error=%s\n' "${EDGE_BAD_REC_AFTER_ROWS}" "${EDGE_BAD_REC_AFTER_FILES}" "${EDGE_BAD_REC_AFTER_SHIFT_LINES}" "$(jq -r '.error_code' <<<"${EDGE_BAD_REC_RES}")"
[ "${EDGE_BAD_REC_AFTER_ROWS}" = "${EDGE_BAD_REC_BEFORE_ROWS}" ]
[ "${EDGE_BAD_REC_AFTER_FILES}" = "${EDGE_BAD_REC_BEFORE_FILES}" ]
[ "${EDGE_BAD_REC_AFTER_SHIFT_LINES}" = "${EDGE_BAD_REC_BEFORE_SHIFT_LINES}" ]

OVER_LIMIT_REC="${AUDIT_ROOT}/fsv-over-limit-recommendation.json"
jq '.turn_number = 2 | .source_files_changed = [range(0; 26) | "docs/ccreality/phase-05-config-files/over-limit-\(.).md"] | .source_of_truth = "file:/tmp/placeholder/recommendation-turn-02.json"' "${GOOD_REC}" >"${OVER_LIMIT_REC}"
EDGE_LIMIT_BEFORE_ROWS="$(ledger_count "${FSV_LEDGER}")"
EDGE_LIMIT_BEFORE_FILES="$(recommendation_file_count "${FSV_ATTEMPT_DIR}")"
printf 'EDGE over_limit BEFORE rows=%s files=%s\n' "${EDGE_LIMIT_BEFORE_ROWS}" "${EDGE_LIMIT_BEFORE_FILES}"
EDGE_LIMIT_RES="$(mcp_call_text optimizer_record_recommendation "$(cat "${OVER_LIMIT_REC}")")"
jq -e '.error_code == "CCREALITY_RECOMMENDATION_SCHEMA_INVALID"' <<<"${EDGE_LIMIT_RES}" >/dev/null
EDGE_LIMIT_AFTER_ROWS="$(ledger_count "${FSV_LEDGER}")"
EDGE_LIMIT_AFTER_FILES="$(recommendation_file_count "${FSV_ATTEMPT_DIR}")"
printf 'EDGE over_limit AFTER rows=%s files=%s error=%s\n' "${EDGE_LIMIT_AFTER_ROWS}" "${EDGE_LIMIT_AFTER_FILES}" "$(jq -r '.error_code' <<<"${EDGE_LIMIT_RES}")"
[ "${EDGE_LIMIT_AFTER_ROWS}" = "${EDGE_LIMIT_BEFORE_ROWS}" ]
[ "${EDGE_LIMIT_AFTER_FILES}" = "${EDGE_LIMIT_BEFORE_FILES}" ]

HOOK_LOG="${FSV_ROOT}/cgreality-hook-events.jsonl"
HOOK_BEFORE="$(jsonl_count "${HOOK_LOG}")"
printf 'EDGE hook_env_deny BEFORE hook_log_lines=%s\n' "${HOOK_BEFORE}"
HOOK_DENY_OUT="$(printf '%s\n' '{"session_id":"phase0-8-fsv","tool_use_id":"edge-env","tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/.env"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${HOOK_DENY_OUT}")" = "deny" ]
HOOK_AFTER="$(jsonl_count "${HOOK_LOG}")"
printf 'EDGE hook_env_deny AFTER hook_log_lines=%s decision=%s\n' "${HOOK_AFTER}" "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${HOOK_DENY_OUT}")"
[ "${HOOK_AFTER}" -eq $((HOOK_BEFORE + 1)) ]
tail -n 1 "${HOOK_LOG}"

HOOK_EMPTY_BEFORE="$(jsonl_count "${HOOK_LOG}")"
printf 'EDGE hook_empty_input BEFORE hook_log_lines=%s\n' "${HOOK_EMPTY_BEFORE}"
HOOK_EMPTY_OUT="$(printf '%s\n' '{}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${HOOK_EMPTY_OUT}")" = "allow" ]
HOOK_EMPTY_AFTER="$(jsonl_count "${HOOK_LOG}")"
printf 'EDGE hook_empty_input AFTER hook_log_lines=%s decision=%s reason=%s\n' "${HOOK_EMPTY_AFTER}" "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${HOOK_EMPTY_OUT}")" "$(jq -r '.hookSpecificOutput.permissionDecisionReason' <<<"${HOOK_EMPTY_OUT}")"
[ "${HOOK_EMPTY_AFTER}" -eq $((HOOK_EMPTY_BEFORE + 1)) ]
tail -n 1 "${HOOK_LOG}"

HOOK_ALLOW_BEFORE="$(jsonl_count "${HOOK_LOG}")"
STATE_PATH="${FSV_ROOT}/cgreality-hook-state/preedit/phase0-8-fsv/edge-allow.json"
rm -f "${STATE_PATH}"
printf 'EDGE hook_allow_state BEFORE hook_log_lines=%s state_exists=%s\n' "${HOOK_ALLOW_BEFORE}" "$([ -f "${STATE_PATH}" ] && printf yes || printf no)"
HOOK_ALLOW_OUT="$(printf '%s\n' '{"session_id":"phase0-8-fsv","tool_use_id":"edge-allow","tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/CLAUDE.md"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(jq -r '.hookSpecificOutput.permissionDecision' <<<"${HOOK_ALLOW_OUT}")" = "allow" ]
HOOK_ALLOW_AFTER="$(jsonl_count "${HOOK_LOG}")"
test -s "${STATE_PATH}"
jq -e '.record_kind == "ccreality_pretool_edit_state" and .rel_path == "CLAUDE.md" and (.before_sha256 | startswith("sha256:"))' "${STATE_PATH}" >/dev/null
printf 'EDGE hook_allow_state AFTER hook_log_lines=%s state_exists=yes state_path=%s\n' "${HOOK_ALLOW_AFTER}" "${STATE_PATH}"
[ "${HOOK_ALLOW_AFTER}" -eq $((HOOK_ALLOW_BEFORE + 1)) ]
jq -c '{record_kind, rel_path, before_sha256, source_of_truth}' "${STATE_PATH}"

LEDGER_COUNTS_JSON="$(sqlite3 -json "${FSV_LEDGER}" 'select record_kind, count(*) as count from ledger_records group by record_kind order by record_kind;')"
REPORT_TMP="${REPORT}.tmp"
jq -nc \
  --arg status "ok" \
  --arg audit_root "${AUDIT_ROOT}" \
  --arg fsv_root "${FSV_ROOT}" \
  --arg report "${REPORT}" \
  --arg recommendation_path "${REC_PATH}" \
  --arg ledger "${FSV_LEDGER}" \
  --arg shift_log "${SHIFT_LOG}" \
  --arg hook_log "${HOOK_LOG}" \
  --argjson ledger_counts "${LEDGER_COUNTS_JSON}" '{
    status: $status,
    audit_root: $audit_root,
    fsv_root: $fsv_root,
    report: $report,
    sources_of_truth: {
      recommendation_path: $recommendation_path,
      ledger: $ledger,
      shift_log: $shift_log,
      hook_log: $hook_log
    },
    ledger_counts: $ledger_counts
  }' >"${REPORT_TMP}"
mv "${REPORT_TMP}" "${REPORT}"

section "Evidence of success"
jq . "${REPORT}"
printf 'PHASE 0-8 HARDENING ALL PASS\n'
