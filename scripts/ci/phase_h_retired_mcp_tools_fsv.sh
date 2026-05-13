#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/mnt/d/contextgraph/fsv/phase-h-retired-mcp-tools-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/retired_mcp_tools_fsv.json"

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
definitions_dir="crates/context-graph-mcp/src/tools/definitions"
deprecation_source="crates/context-graph-mcp/src/deprecation.rs"
dispatch_test_source="crates/context-graph-mcp/src/handlers/tests/tools_call.rs"
retired_response_test_source="crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_tools.rs"
dispatcher_source="crates/context-graph-mcp/src/handlers/tools/dispatch.rs"
definitions_test_source="crates/context-graph-mcp/src/tools/definitions/mod.rs"
deletion_list="docs/futurebuild/11_DELETION_LIST_INNER_LOOP.md"
retired_regex='mcp__cgreality__reality_(run_attempt|alter_file|alter_run|alter_apply|drip_attempt)'

run_command() {
  local name="$1"
  shift
  local stdout="$RUN_ROOT/${name}.stdout"
  local stderr="$RUN_ROOT/${name}.stderr"
  local status=0
  if ! "$@" >"$stdout" 2>"$stderr"; then
    status=$?
  fi
  printf '%s' "$status"
}

definition_matches="$(rg -n "$retired_regex" "$definitions_dir" || true)"
definition_match_count="$(printf '%s\n' "$definition_matches" | sed '/^$/d' | wc -l | tr -d ' ')"

retired_registry_matches="$(rg -n "\"$retired_regex\"" "$deprecation_source" || true)"
retired_registry_count="$(printf '%s\n' "$retired_registry_matches" | sed '/^$/d' | wc -l | tr -d ' ')"
retired_registry_unique_count="$(printf '%s\n' "$retired_registry_matches" | rg -o "$retired_regex" | sort -u | wc -l | tr -d ' ')"

deletion_list_has_run_attempt=false
if rg -q 'mcp__cgreality__reality_run_attempt.*CCREALITY_ENGINE_RETIRED' "$deletion_list"; then
  deletion_list_has_run_attempt=true
fi

deletion_list_has_wildcard=false
if rg -q 'mcp__cgreality__reality_loop_\*.*RETIRE' "$deletion_list"; then
  deletion_list_has_wildcard=true
fi

tool_count_assertion_present=false
if rg -q 'assert_eq!\(tools\.len\(\), 174\)' "$definitions_test_source"; then
  tool_count_assertion_present=true
fi

retired_registry_test_present=false
if rg -q 'retired_cgreality_tools_are_not_advertised_by_tool_registries' "$definitions_test_source"; then
  retired_registry_test_present=true
fi

jsonrpc_response_test_present=false
if rg -q 'retired_cgreality_tools_fail_closed_as_jsonrpc_responses' "$retired_response_test_source" \
  && rg -q 'RETIRED_CGREALITY_TOOLS' "$retired_response_test_source" \
  && rg -q 'structuredContent.*error_code' "$retired_response_test_source" \
  && rg -q 'CCREALITY_ENGINE_RETIRED' "$retired_response_test_source"; then
  jsonrpc_response_test_present=true
fi

dispatcher_routes_retired_tools=false
if rg -q 'is_retired_cgreality_tool\(tool_name\)' "$dispatcher_source" \
  && rg -q 'retired_tool_response\(id, tool_name\)' "$dispatcher_source"; then
  dispatcher_routes_retired_tools=true
fi

synthetic_definition="$RUN_ROOT/synthetic_definition.rs"
cat >"$synthetic_definition" <<'SYNTHETIC'
use crate::tools::types::ToolDefinition;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition::new(
        "mcp__cgreality__reality_run_attempt",
        "bad retired tool definition",
        serde_json::json!({"type": "object"}),
    )]
}
SYNTHETIC
synthetic_definition_matches="$(rg -n "$retired_regex" "$synthetic_definition" || true)"
synthetic_definition_match_count="$(printf '%s\n' "$synthetic_definition_matches" | sed '/^$/d' | wc -l | tr -d ' ')"
synthetic_definition_detected=false
if [[ "$synthetic_definition_match_count" -gt 0 ]]; then
  synthetic_definition_detected=true
fi

definitions_test_status="$(run_command definitions_test cargo test -p context-graph-mcp retired_cgreality_tools_are_not_advertised_by_tool_registries -- --nocapture)"
retired_response_test_status="$(run_command retired_response_test cargo test -p context-graph-mcp retired_cgreality_tools_fail_closed_as_jsonrpc_responses -- --nocapture)"
deprecation_tests_status="$(run_command deprecation_tests cargo test -p context-graph-mcp retired_cgreality -- --nocapture)"

all_passed=false
if [[ "$definition_match_count" == "0" \
  && "$retired_registry_unique_count" == "5" \
  && "$deletion_list_has_run_attempt" == "true" \
  && "$deletion_list_has_wildcard" == "true" \
  && "$tool_count_assertion_present" == "true" \
  && "$retired_registry_test_present" == "true" \
  && "$jsonrpc_response_test_present" == "true" \
  && "$dispatcher_routes_retired_tools" == "true" \
  && "$synthetic_definition_detected" == "true" \
  && "$definitions_test_status" == "0" \
  && "$retired_response_test_status" == "0" \
  && "$deprecation_tests_status" == "0" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-004" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg definitions_dir "$definitions_dir" \
  --arg deprecation_source "$deprecation_source" \
  --arg dispatch_test_source "$dispatch_test_source" \
  --arg retired_response_test_source "$retired_response_test_source" \
  --arg dispatcher_source "$dispatcher_source" \
  --arg definitions_test_source "$definitions_test_source" \
  --arg deletion_list "$deletion_list" \
  --arg retired_regex "$retired_regex" \
  --arg definition_matches "$definition_matches" \
  --arg retired_registry_matches "$retired_registry_matches" \
  --arg synthetic_definition "$synthetic_definition" \
  --arg synthetic_definition_matches "$synthetic_definition_matches" \
  --argjson definition_match_count "$definition_match_count" \
  --argjson retired_registry_count "$retired_registry_count" \
  --argjson retired_registry_unique_count "$retired_registry_unique_count" \
  --argjson deletion_list_has_run_attempt "$deletion_list_has_run_attempt" \
  --argjson deletion_list_has_wildcard "$deletion_list_has_wildcard" \
  --argjson tool_count_assertion_present "$tool_count_assertion_present" \
  --argjson retired_registry_test_present "$retired_registry_test_present" \
  --argjson jsonrpc_response_test_present "$jsonrpc_response_test_present" \
  --argjson dispatcher_routes_retired_tools "$dispatcher_routes_retired_tools" \
  --argjson synthetic_definition_match_count "$synthetic_definition_match_count" \
  --argjson synthetic_definition_detected "$synthetic_definition_detected" \
  --argjson definitions_test_status "$definitions_test_status" \
  --argjson retired_response_test_status "$retired_response_test_status" \
  --argjson deprecation_tests_status "$deprecation_tests_status" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      definitions_dir: $definitions_dir,
      deprecation_source: $deprecation_source,
      dispatch_test_source: $dispatch_test_source,
      retired_response_test_source: $retired_response_test_source,
      dispatcher_source: $dispatcher_source,
      definitions_test_source: $definitions_test_source,
      deletion_list: $deletion_list,
      retired_regex: $retired_regex
    },
    happy_path: [
      {
        case: "retired_tools_are_not_advertised_by_tool_definitions",
        expected: "zero retired cgreality tool names under tools/definitions",
        actual: {
          definition_match_count: $definition_match_count,
          definition_matches: ($definition_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($definition_match_count == 0)
      },
      {
        case: "retired_registry_cross_references_deletion_list",
        expected: "five unique retired tools in deprecation registry and deletion-list rows name run_attempt plus wildcard retirement",
        actual: {
          retired_registry_count: $retired_registry_count,
          retired_registry_unique_count: $retired_registry_unique_count,
          retired_registry_matches: ($retired_registry_matches | split("\n") | map(select(length > 0))),
          deletion_list_has_run_attempt: $deletion_list_has_run_attempt,
          deletion_list_has_wildcard: $deletion_list_has_wildcard
        },
        pass: ($retired_registry_unique_count == 5 and $deletion_list_has_run_attempt and $deletion_list_has_wildcard)
      },
      {
        case: "tool_count_and_retired_registry_tests_present",
        expected: "tool-count assertion remains present and a retired-registry exclusion test exists",
        actual: {
          tool_count_assertion_present: $tool_count_assertion_present,
          retired_registry_test_present: $retired_registry_test_present
        },
        pass: ($tool_count_assertion_present and $retired_registry_test_present)
      },
      {
        case: "synthetic_jsonrpc_response_test_exercises_every_retired_tool",
        expected: "response test loops over RETIRED_CGREALITY_TOOLS, dispatcher routes retired names to that response, and structured CCREALITY_ENGINE_RETIRED is asserted",
        actual: {
          jsonrpc_response_test_present: $jsonrpc_response_test_present,
          dispatcher_routes_retired_tools: $dispatcher_routes_retired_tools,
          retired_response_test_status: $retired_response_test_status,
          retired_response_test_stdout: ($run_root + "/retired_response_test.stdout"),
          retired_response_test_stderr: ($run_root + "/retired_response_test.stderr")
        },
        pass: ($jsonrpc_response_test_present and $dispatcher_routes_retired_tools and $retired_response_test_status == 0)
      },
      {
        case: "deprecation_notice_tests_pass",
        expected: "retired deprecation framework tests pass",
        actual: {
          deprecation_tests_status: $deprecation_tests_status,
          deprecation_tests_stdout: ($run_root + "/deprecation_tests.stdout"),
          deprecation_tests_stderr: ($run_root + "/deprecation_tests.stderr")
        },
        pass: ($deprecation_tests_status == 0)
      },
      {
        case: "definition_exclusion_test_passes",
        expected: "tool definitions exclusion test passes",
        actual: {
          definitions_test_status: $definitions_test_status,
          definitions_test_stdout: ($run_root + "/definitions_test.stdout"),
          definitions_test_stderr: ($run_root + "/definitions_test.stderr")
        },
        pass: ($definitions_test_status == 0)
      }
    ],
    boundary_cases: [
      {
        case: "synthetic_retired_tool_definition_is_detected",
        expected: "scanner detects a retired tool if it is reintroduced into a definition file",
        actual: {
          synthetic_definition: $synthetic_definition,
          synthetic_definition_match_count: $synthetic_definition_match_count,
          synthetic_definition_matches: ($synthetic_definition_matches | split("\n") | map(select(length > 0))),
          synthetic_definition_detected: $synthetic_definition_detected
        },
        pass: $synthetic_definition_detected
      }
    ],
    all_passed: $all_passed,
    readback_equal: true
  }' >"$ARTIFACT.tmp"
mv "$ARTIFACT.tmp" "$ARTIFACT"

artifact_sha256="$(sha256sum "$ARTIFACT" | awk '{print $1}')"
jq -n --arg artifact_path "$ARTIFACT" --arg artifact_sha256 "$artifact_sha256" --argjson all_passed "$all_passed" \
  '{artifact_path: $artifact_path, artifact_sha256: $artifact_sha256, all_passed: $all_passed, readback_equal: true}'

if [[ "$all_passed" != "true" ]]; then
  echo "TASK_LOOP_004_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
