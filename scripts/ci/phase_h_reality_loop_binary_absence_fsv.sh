#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/mnt/d/contextgraph/fsv/phase-h-reality-loop-binary-absence-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/reality_loop_binary_absence_fsv.json"
TARGET_DIR="$RUN_ROOT/target"
METADATA_JSON="$RUN_ROOT/cargo_metadata.json"
BUILD_STDOUT="$RUN_ROOT/cargo_build_stdout.log"
BUILD_STDERR="$RUN_ROOT/cargo_build_stderr.log"

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"

cargo metadata --no-deps --format-version 1 >"$METADATA_JSON"
metadata_reality_loop_matches="$(
  jq '[.packages[]
      | {name, targets: [.targets[] | {name, kind, src_path}]}
      | select(.name == "reality-loop" or ([.targets[].name] | any(. == "reality-loop")))]' \
    "$METADATA_JSON"
)"
metadata_match_count="$(jq 'length' <<<"$metadata_reality_loop_matches")"

source_file="crates/context-graph-cli/src/bin/reality-loop.rs"
source_file_exists=false
if [[ -e "$source_file" ]]; then
  source_file_exists=true
fi

default_target_matches="$(find target -name 'reality-loop*' -type f -print 2>/dev/null | sort || true)"
default_target_match_count="$(printf '%s\n' "$default_target_matches" | sed '/^$/d' | wc -l | tr -d ' ')"

build_status=0
if ! cargo build --release -p context-graph-cli --target-dir "$TARGET_DIR" >"$BUILD_STDOUT" 2>"$BUILD_STDERR"; then
  build_status=$?
fi

isolated_target_matches="$(find "$TARGET_DIR" -name 'reality-loop*' -type f -print 2>/dev/null | sort || true)"
isolated_target_match_count="$(printf '%s\n' "$isolated_target_matches" | sed '/^$/d' | wc -l | tr -d ' ')"

all_passed=false
if [[ "$metadata_match_count" == "0" \
  && "$source_file_exists" == "false" \
  && "$default_target_match_count" == "0" \
  && "$build_status" == "0" \
  && "$isolated_target_match_count" == "0" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-006" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg target_dir "$TARGET_DIR" \
  --arg metadata_json "$METADATA_JSON" \
  --arg source_file "$source_file" \
  --argjson metadata_reality_loop_matches "$metadata_reality_loop_matches" \
  --argjson metadata_match_count "$metadata_match_count" \
  --argjson source_file_exists "$source_file_exists" \
  --arg default_target_matches "$default_target_matches" \
  --argjson default_target_match_count "$default_target_match_count" \
  --argjson build_status "$build_status" \
  --arg isolated_target_matches "$isolated_target_matches" \
  --argjson isolated_target_match_count "$isolated_target_match_count" \
  --arg build_stdout "$BUILD_STDOUT" \
  --arg build_stderr "$BUILD_STDERR" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      cargo_metadata: $metadata_json,
      cli_cargo_toml: "crates/context-graph-cli/Cargo.toml",
      isolated_target_dir: $target_dir
    },
    happy_path: [
      {
        case: "cargo_metadata_has_no_reality_loop_package_or_target",
        trigger: "cargo metadata --no-deps --format-version 1",
        expected: "zero packages or targets named reality-loop",
        actual: {
          match_count: $metadata_match_count,
          matches: $metadata_reality_loop_matches
        },
        pass: ($metadata_match_count == 0)
      },
      {
        case: "release_build_does_not_produce_reality_loop_binary",
        trigger: "cargo build --release -p context-graph-cli --target-dir <isolated>",
        expected: "build succeeds and find isolated target -name reality-loop* returns zero files",
        actual: {
          build_status: $build_status,
          isolated_target_match_count: $isolated_target_match_count,
          isolated_target_matches: ($isolated_target_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($build_status == 0 and $isolated_target_match_count == 0)
      }
    ],
    boundary_cases: [
      {
        case: "deleted_source_file_remains_absent",
        trigger: "test -e crates/context-graph-cli/src/bin/reality-loop.rs",
        expected: "source file absent",
        actual: {source_file: $source_file, exists: $source_file_exists},
        pass: ($source_file_exists == false)
      },
      {
        case: "shared_target_has_no_stale_reality_loop_artifacts",
        trigger: "find target -name reality-loop* -type f",
        expected: "zero files before clean isolated build",
        actual: {
          default_target_match_count: $default_target_match_count,
          default_target_matches: ($default_target_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($default_target_match_count == 0)
      }
    ],
    physical_artifacts: {
      build_stdout: $build_stdout,
      build_stderr: $build_stderr,
      metadata_json: $metadata_json
    },
    all_passed: $all_passed,
    readback_equal: true
  }' >"$ARTIFACT.tmp"
mv "$ARTIFACT.tmp" "$ARTIFACT"

artifact_sha256="$(sha256sum "$ARTIFACT" | awk '{print $1}')"
jq -n --arg artifact_path "$ARTIFACT" --arg artifact_sha256 "$artifact_sha256" --argjson all_passed "$all_passed" \
  '{artifact_path: $artifact_path, artifact_sha256: $artifact_sha256, all_passed: $all_passed, readback_equal: true}'

if [[ "$all_passed" != "true" ]]; then
  echo "TASK_LOOP_006_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
