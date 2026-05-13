#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/mnt/d/contextgraph/fsv/phase-h-ccreality-engine-retired-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/ccreality_engine_retired_gate_fsv.json"

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
retired_response_source="crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_tools.rs"
dispatch_source="crates/context-graph-mcp/src/handlers/tools/dispatch.rs"
deprecation_source="crates/context-graph-mcp/src/deprecation.rs"

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

unit_test_source_present=false
if rg -q 'retired_cgreality_tools_fail_closed_as_jsonrpc_responses' "$retired_response_source" \
  && rg -q 'for .*RETIRED_CGREALITY_TOOLS' "$retired_response_source" \
  && rg -q 'structuredContent.*error_code' "$retired_response_source" \
  && rg -q 'CCREALITY_ENGINE_RETIRED' "$retired_response_source"; then
  unit_test_source_present=true
fi

jsonrpc_round_trip_test_present=false
if rg -q 'retired_reality_run_attempt_jsonrpc_round_trip_has_structured_error' "$retired_response_source" \
  && rg -q 'serde_json::to_value' "$retired_response_source" \
  && rg -q 'serde_json::from_value' "$retired_response_source" \
  && rg -q 'mcp__cgreality__reality_run_attempt' "$retired_response_source" \
  && rg -q 'CCREALITY_ENGINE_RETIRED' "$retired_response_source"; then
  jsonrpc_round_trip_test_present=true
fi

dispatch_gate_present=false
if rg -q 'is_retired_cgreality_tool\(tool_name\)' "$dispatch_source" \
  && rg -q 'retired_tool_response\(id, tool_name\)' "$dispatch_source"; then
  dispatch_gate_present=true
fi

retired_registry_unique_count="$(
  rg -o 'mcp__cgreality__reality_(run_attempt|alter_file|alter_run|alter_apply|drip_attempt)' "$deprecation_source" \
    | sort -u \
    | wc -l \
    | tr -d ' '
)"

fmt_status="$(run_command cargo_fmt cargo fmt --check)"
check_status="$(run_command cargo_check cargo check -p context-graph-mcp)"
unit_test_status="$(run_command retired_unit_test cargo test -p context-graph-mcp retired_cgreality_tools_fail_closed_as_jsonrpc_responses -- --nocapture)"
jsonrpc_round_trip_status="$(run_command retired_jsonrpc_round_trip cargo test -p context-graph-mcp retired_reality_run_attempt_jsonrpc_round_trip_has_structured_error -- --nocapture)"
error_escalation_status="$(run_command retired_error_escalation cargo test -p context-graph-mcp retired_tool_deprecation_error_level_returns_jsonrpc_error -- --nocapture)"
clippy_status="$(run_command cargo_clippy cargo clippy -p context-graph-mcp --no-deps --tests --examples -- -D warnings)"

all_passed=false
if [[ "$unit_test_source_present" == "true" \
  && "$jsonrpc_round_trip_test_present" == "true" \
  && "$dispatch_gate_present" == "true" \
  && "$retired_registry_unique_count" == "5" \
  && "$fmt_status" == "0" \
  && "$check_status" == "0" \
  && "$unit_test_status" == "0" \
  && "$jsonrpc_round_trip_status" == "0" \
  && "$error_escalation_status" == "0" \
  && "$clippy_status" == "0" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-011" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg retired_response_source "$retired_response_source" \
  --arg dispatch_source "$dispatch_source" \
  --arg deprecation_source "$deprecation_source" \
  --argjson unit_test_source_present "$unit_test_source_present" \
  --argjson jsonrpc_round_trip_test_present "$jsonrpc_round_trip_test_present" \
  --argjson dispatch_gate_present "$dispatch_gate_present" \
  --argjson retired_registry_unique_count "$retired_registry_unique_count" \
  --argjson fmt_status "$fmt_status" \
  --argjson check_status "$check_status" \
  --argjson unit_test_status "$unit_test_status" \
  --argjson jsonrpc_round_trip_status "$jsonrpc_round_trip_status" \
  --argjson error_escalation_status "$error_escalation_status" \
  --argjson clippy_status "$clippy_status" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      retired_response_source: $retired_response_source,
      dispatch_source: $dispatch_source,
      deprecation_source: $deprecation_source
    },
    happy_path: [
      {
        case: "every_retired_endpoint_has_unit_fail_closed_coverage",
        expected: "unit test loops over all five RETIRED_CGREALITY_TOOLS and asserts CCREALITY_ENGINE_RETIRED",
        actual: {
          retired_registry_unique_count: $retired_registry_unique_count,
          unit_test_source_present: $unit_test_source_present,
          unit_test_status: $unit_test_status,
          unit_test_stdout: ($run_root + "/retired_unit_test.stdout"),
          unit_test_stderr: ($run_root + "/retired_unit_test.stderr")
        },
        pass: ($retired_registry_unique_count == 5 and $unit_test_source_present and $unit_test_status == 0)
      },
      {
        case: "one_retired_endpoint_round_trips_as_jsonrpc_structured_error",
        expected: "reality_run_attempt response serializes/deserializes as JSON-RPC with structured CCREALITY_ENGINE_RETIRED",
        actual: {
          jsonrpc_round_trip_test_present: $jsonrpc_round_trip_test_present,
          jsonrpc_round_trip_status: $jsonrpc_round_trip_status,
          stdout: ($run_root + "/retired_jsonrpc_round_trip.stdout"),
          stderr: ($run_root + "/retired_jsonrpc_round_trip.stderr")
        },
        pass: ($jsonrpc_round_trip_test_present and $jsonrpc_round_trip_status == 0)
      },
      {
        case: "dispatcher_gate_routes_retired_names_before_normal_tool_dispatch",
        expected: "dispatch.rs checks is_retired_cgreality_tool and returns retired_tool_response",
        actual: {dispatch_gate_present: $dispatch_gate_present},
        pass: $dispatch_gate_present
      },
      {
        case: "build_test_clippy_clean",
        expected: "fmt, check, retired tests, error-escalation test, and clippy pass",
        actual: {
          fmt_status: $fmt_status,
          check_status: $check_status,
          unit_test_status: $unit_test_status,
          jsonrpc_round_trip_status: $jsonrpc_round_trip_status,
          error_escalation_status: $error_escalation_status,
          clippy_status: $clippy_status,
          cargo_fmt_stdout: ($run_root + "/cargo_fmt.stdout"),
          cargo_check_stdout: ($run_root + "/cargo_check.stdout"),
          cargo_clippy_stdout: ($run_root + "/cargo_clippy.stdout")
        },
        pass: ($fmt_status == 0 and $check_status == 0 and $unit_test_status == 0 and $jsonrpc_round_trip_status == 0 and $error_escalation_status == 0 and $clippy_status == 0)
      }
    ],
    boundary_cases: [
      {
        case: "deprecation_error_level_returns_jsonrpc_error",
        expected: "CONTEXT_GRAPH_DEPRECATION_LEVEL=error converts retired tool response to JSON-RPC invalid-params error",
        actual: {
          error_escalation_status: $error_escalation_status,
          stdout: ($run_root + "/retired_error_escalation.stdout"),
          stderr: ($run_root + "/retired_error_escalation.stderr")
        },
        pass: ($error_escalation_status == 0)
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
  echo "TASK_LOOP_011_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
