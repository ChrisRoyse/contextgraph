#!/usr/bin/env bash
set -euo pipefail

# retired-inner-actor audit: historical, do not reanimate the local LLM paths
# named below; synthetic matches prove the detector fails closed.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/zfs/archive/contextgraph/fsv/phase-h-no-inner-actor-models-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/no_inner_actor_models_fsv.json"
HAPPY_JSON="$RUN_ROOT/happy_scan.json"
SYNTHETIC_JSON="$RUN_ROOT/synthetic_scan.json"
SYNTHETIC_ROOT="$RUN_ROOT/synthetic_retired_paths"
UNWRITABLE_OUTPUT_PATH="$RUN_ROOT/unwritable-output-path"

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"

write_json_checked() {
  local output_path="$1"
  local payload="$2"
  local tmp_path="${output_path}.tmp.$$"
  local expected_canonical readback_canonical

  if [[ -d "$output_path" ]]; then
    echo "PHASE_H_NO_INNER_ACTOR_OUTPUT_PATH_IS_DIRECTORY: $output_path" >&2
    return 1
  fi
  expected_canonical="$(jq -S . <<<"$payload")"
  if ! printf '%s\n' "$payload" >"$tmp_path"; then
    echo "PHASE_H_NO_INNER_ACTOR_WRITE_FAILED: $tmp_path" >&2
    rm -f "$tmp_path"
    return 1
  fi
  if ! readback_canonical="$(jq -S . "$tmp_path")"; then
    echo "PHASE_H_NO_INNER_ACTOR_READBACK_PARSE_FAILED: $tmp_path" >&2
    rm -f "$tmp_path"
    return 1
  fi
  if [[ "$readback_canonical" != "$expected_canonical" ]]; then
    echo "PHASE_H_NO_INNER_ACTOR_READBACK_MISMATCH: $tmp_path" >&2
    rm -f "$tmp_path"
    return 1
  fi
  mv "$tmp_path" "$output_path"
  if ! readback_canonical="$(jq -S . "$output_path")"; then
    echo "PHASE_H_NO_INNER_ACTOR_FINAL_READBACK_PARSE_FAILED: $output_path" >&2
    return 1
  fi
  if [[ "$readback_canonical" != "$expected_canonical" ]]; then
    echo "PHASE_H_NO_INNER_ACTOR_FINAL_READBACK_MISMATCH: $output_path" >&2
    return 1
  fi
}

DEFAULT_MODEL_ROOT="${INNER_ACTOR_MODEL_ROOT:-/zfs/hot/contextgraph/models}"
HAPPY_MODEL_ROOT="$DEFAULT_MODEL_ROOT"
HAPPY_MODEL_ROOT_MODE="configured"
if [[ ! -e "$HAPPY_MODEL_ROOT" ]]; then
  HAPPY_MODEL_ROOT="$RUN_ROOT/empty-model-root"
  HAPPY_MODEL_ROOT_MODE="local_empty_root"
  mkdir -p "$HAPPY_MODEL_ROOT"
fi

set +e
INNER_ACTOR_MODEL_ROOT="$HAPPY_MODEL_ROOT" OUTPUT_JSON="$HAPPY_JSON" \
  scripts/audit/no_inner_actor_models.sh >"$RUN_ROOT/happy_scan.stdout" 2>"$RUN_ROOT/happy_scan.stderr"
happy_status=$?
set -e
happy_match_count="$(jq '.match_count' "$HAPPY_JSON")"
happy_missing_root_count="$(jq '.missing_root_count' "$HAPPY_JSON")"

mkdir -p "$SYNTHETIC_ROOT/models" "$SYNTHETIC_ROOT/.ollama/models" "$SYNTHETIC_ROOT/vendor/llama.cpp"
touch "$SYNTHETIC_ROOT/models/Qwen3.6-27B-Q6_K_XL.gguf"
touch "$SYNTHETIC_ROOT/models/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"
touch "$SYNTHETIC_ROOT/vendor/llama.cpp/llama-server"

set +e
INNER_ACTOR_MODEL_ROOT="$HAPPY_MODEL_ROOT" INNER_ACTOR_EXTRA_SCAN_ROOTS="$SYNTHETIC_ROOT" OUTPUT_JSON="$SYNTHETIC_JSON" \
  scripts/audit/no_inner_actor_models.sh >"$RUN_ROOT/synthetic_scan.stdout" 2>"$RUN_ROOT/synthetic_scan.stderr"
synthetic_status=$?
set -e
synthetic_match_count="$(jq '.match_count' "$SYNTHETIC_JSON")"
synthetic_detected_qwen="$(jq --arg p "$SYNTHETIC_ROOT/models/Qwen3.6-27B-Q6_K_XL.gguf" 'any(.matches[]; .path == $p and .rule_id == "qwen3_6_27b")' "$SYNTHETIC_JSON")"
synthetic_detected_hermes="$(jq --arg p "$SYNTHETIC_ROOT/models/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf" 'any(.matches[]; .path == $p and .rule_id == "hermes_2_pro_mistral_7b")' "$SYNTHETIC_JSON")"
synthetic_detected_ollama="$(jq --arg p "$SYNTHETIC_ROOT/.ollama" 'any(.matches[]; .path == $p and .rule_id == "ollama_directory")' "$SYNTHETIC_JSON")"
synthetic_detected_llama="$(jq --arg p "$SYNTHETIC_ROOT/vendor/llama.cpp/llama-server" 'any(.matches[]; .path == $p and .rule_id == "llama_cpp_binary_or_vendor")' "$SYNTHETIC_JSON")"

mkdir -p "$UNWRITABLE_OUTPUT_PATH"
set +e
INNER_ACTOR_MODEL_ROOT="$HAPPY_MODEL_ROOT" OUTPUT_JSON="$UNWRITABLE_OUTPUT_PATH" \
  scripts/audit/no_inner_actor_models.sh >"$RUN_ROOT/unwritable_output.stdout" 2>"$RUN_ROOT/unwritable_output.stderr"
unwritable_status=$?
set -e
unwritable_failed_closed=false
if [[ "$unwritable_status" != "0" ]] \
  && rg -q "NO_INNER_ACTOR_MODELS_OUTPUT_PATH_IS_DIRECTORY|NO_INNER_ACTOR_MODELS_WRITE_FAILED|Is a directory|Permission denied" "$RUN_ROOT/unwritable_output.stderr"; then
  unwritable_failed_closed=true
fi

all_passed=false
if [[ "$happy_status" == "0" \
  && "$happy_match_count" == "0" \
  && "$happy_missing_root_count" == "0" \
  && "$synthetic_status" != "0" \
  && "$synthetic_match_count" -ge "4" \
  && "$synthetic_detected_qwen" == "true" \
  && "$synthetic_detected_hermes" == "true" \
  && "$synthetic_detected_ollama" == "true" \
  && "$synthetic_detected_llama" == "true" \
  && "$unwritable_failed_closed" == "true" ]]; then
  all_passed=true
fi

artifact_readback_status=true
artifact_payload="$(jq -n \
  --arg task_id "TASK-LOOP-005" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg audit_script "scripts/audit/no_inner_actor_models.sh" \
  --arg happy_model_root "$HAPPY_MODEL_ROOT" \
  --arg happy_model_root_mode "$HAPPY_MODEL_ROOT_MODE" \
  --arg happy_json "$HAPPY_JSON" \
  --arg synthetic_json "$SYNTHETIC_JSON" \
  --arg synthetic_root "$SYNTHETIC_ROOT" \
  --arg unwritable_output_path "$UNWRITABLE_OUTPUT_PATH" \
  --argjson happy_status "$happy_status" \
  --argjson happy_match_count "$happy_match_count" \
  --argjson happy_missing_root_count "$happy_missing_root_count" \
  --argjson synthetic_status "$synthetic_status" \
  --argjson synthetic_match_count "$synthetic_match_count" \
  --argjson synthetic_detected_qwen "$synthetic_detected_qwen" \
  --argjson synthetic_detected_hermes "$synthetic_detected_hermes" \
  --argjson synthetic_detected_ollama "$synthetic_detected_ollama" \
  --argjson synthetic_detected_llama "$synthetic_detected_llama" \
  --argjson unwritable_status "$unwritable_status" \
  --argjson unwritable_failed_closed "$unwritable_failed_closed" \
  --argjson all_passed "$all_passed" \
  --argjson readback_status "$artifact_readback_status" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      audit_script: $audit_script,
      happy_model_root: $happy_model_root,
      happy_model_root_mode: $happy_model_root_mode,
      happy_scan_json: $happy_json,
      synthetic_scan_json: $synthetic_json,
      synthetic_root: $synthetic_root,
      unwritable_output_path: $unwritable_output_path
    },
    happy_path: [
      {
        case: "real_model_root_and_source_tree_have_zero_retired_inner_actor_paths",
        trigger: "scripts/audit/no_inner_actor_models.sh",
        expected: "exit 0, zero matches, zero missing scan roots",
        actual: {
          exit_status: $happy_status,
          match_count: $happy_match_count,
          missing_root_count: $happy_missing_root_count,
          model_root: $happy_model_root,
          model_root_mode: $happy_model_root_mode
        },
        pass: ($happy_status == 0 and $happy_match_count == 0 and $happy_missing_root_count == 0)
      }
    ],
    boundary_cases: [
      {
        case: "synthetic_qwen_placeholder_model_file_fails_closed",
        trigger: "INNER_ACTOR_EXTRA_SCAN_ROOTS=<synthetic> scripts/audit/no_inner_actor_models.sh",
        expected: "non-zero exit and Qwen3.6-27B placeholder detected",
        actual: {
          exit_status: $synthetic_status,
          match_count: $synthetic_match_count,
          detected: $synthetic_detected_qwen
        },
        pass: ($synthetic_status != 0 and $synthetic_detected_qwen)
      },
      {
        case: "synthetic_hermes_placeholder_model_file_fails_closed",
        trigger: "INNER_ACTOR_EXTRA_SCAN_ROOTS=<synthetic> scripts/audit/no_inner_actor_models.sh",
        expected: "non-zero exit and Hermes-2-Pro-Mistral-7B placeholder detected",
        actual: {
          exit_status: $synthetic_status,
          match_count: $synthetic_match_count,
          detected: $synthetic_detected_hermes
        },
        pass: ($synthetic_status != 0 and $synthetic_detected_hermes)
      },
      {
        case: "synthetic_ollama_directory_and_llama_cpp_binary_paths_fail_closed",
        trigger: "INNER_ACTOR_EXTRA_SCAN_ROOTS=<synthetic> scripts/audit/no_inner_actor_models.sh",
        expected: "non-zero exit and both Ollama directory and llama.cpp binary path detected",
        actual: {
          exit_status: $synthetic_status,
          match_count: $synthetic_match_count,
          ollama_detected: $synthetic_detected_ollama,
          llama_cpp_detected: $synthetic_detected_llama
        },
        pass: ($synthetic_status != 0 and $synthetic_detected_ollama and $synthetic_detected_llama)
      },
      {
        case: "artifact_readback_write_failure_fails_closed",
        trigger: "OUTPUT_JSON=<directory> scripts/audit/no_inner_actor_models.sh",
        expected: "non-zero exit and write/readback failure surfaced",
        actual: {
          exit_status: $unwritable_status,
          output_path: $unwritable_output_path,
          failed_closed: $unwritable_failed_closed
        },
        pass: ($unwritable_status != 0 and $unwritable_failed_closed)
      }
    ],
    physical_artifacts: {
      happy_scan_stdout: ($run_root + "/happy_scan.stdout"),
      happy_scan_stderr: ($run_root + "/happy_scan.stderr"),
      synthetic_scan_stdout: ($run_root + "/synthetic_scan.stdout"),
      synthetic_scan_stderr: ($run_root + "/synthetic_scan.stderr"),
      unwritable_output_stdout: ($run_root + "/unwritable_output.stdout"),
      unwritable_output_stderr: ($run_root + "/unwritable_output.stderr")
    },
    all_passed: $all_passed,
    readback_equal: $readback_status
  }')"
write_json_checked "$ARTIFACT" "$artifact_payload"

artifact_sha256="$(sha256sum "$ARTIFACT" | awk '{print $1}')"
final_readback_status=false
if [[ "$(jq -S . "$ARTIFACT")" == "$(jq -S . <<<"$artifact_payload")" ]]; then
  final_readback_status=true
fi
jq -n --arg artifact_path "$ARTIFACT" --arg artifact_sha256 "$artifact_sha256" --argjson all_passed "$all_passed" --argjson readback_equal "$final_readback_status" \
  '{artifact_path: $artifact_path, artifact_sha256: $artifact_sha256, all_passed: $all_passed, readback_equal: $readback_equal}'

if [[ "$all_passed" != "true" ]]; then
  echo "TASK_LOOP_005_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
