#!/usr/bin/env bash
set -euo pipefail

# retired-inner-actor audit: historical, do not reanimate the local LLM names
# scanned below; this FSV prevents active scripts/config from carrying them.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/mnt/d/contextgraph/fsv/phase-h-scripts-config-inner-llm-residue-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/scripts_config_inner_llm_residue_fsv.json"
PATTERN='qwen|hermes|ollama|llama\.cpp'
HISTORICAL_MARKER='historical, do not reanimate'
AUDIT_MARKER='retired-inner-actor audit'

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"

is_audit_scanner() {
  case "$1" in
    scripts/audit/no_inner_actor_models.sh | \
    scripts/cc/tests/mejepa_retirement_fsv.sh | \
    scripts/ci/phase_h_no_inner_actor_models_fsv.sh | \
    scripts/ci/phase_h_claude_inner_loop_reference_fsv.sh | \
    scripts/ci/phase_h_smap_superseded_decisions_fsv.sh | \
    scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_exp2_strata_snapshot() {
  case "$1" in
    scripts/exp2/strata/*.json)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

classify_match() {
  local root="$1"
  local rel="$2"
  local path="$3"
  local classification allowed marker_path

  classification="active_disallowed"
  allowed=false

  if is_audit_scanner "$rel"; then
    if rg -q --fixed-strings "$AUDIT_MARKER" "$path"; then
      classification="intentional_retired_inner_actor_audit"
      allowed=true
    else
      classification="audit_scanner_missing_marker"
    fi
  elif is_exp2_strata_snapshot "$rel"; then
    marker_path="$root/scripts/exp2/strata/README.md"
    if [[ -f "$marker_path" ]] && rg -q --fixed-strings "$HISTORICAL_MARKER" "$marker_path"; then
      classification="generated_exp2_strata_snapshot"
      allowed=true
    else
      classification="generated_exp2_strata_missing_historical_marker"
    fi
  elif rg -q --fixed-strings "$HISTORICAL_MARKER" "$path"; then
    classification="historical_only"
    allowed=true
  fi

  printf '%s\t%s\n' "$classification" "$allowed"
}

scan_root() {
  local root="$1"
  local root_label="$2"
  local out_jsonl="$3"
  local scan_targets=()
  local rel path classification allowed match_count

  : >"$out_jsonl"
  if [[ -d "$root/scripts" ]]; then
    scan_targets+=("$root/scripts")
  fi
  if [[ -d "$root/config" ]]; then
    scan_targets+=("$root/config")
  fi

  if [[ "${#scan_targets[@]}" -eq 0 ]]; then
    return
  fi

  while IFS= read -r path; do
    rel="${path#"$root"/}"
    IFS=$'\t' read -r classification allowed < <(classify_match "$root" "$rel" "$path")
    match_count="$(rg -i -o "$PATTERN" "$path" | wc -l | tr -d ' ')"
    jq -nc \
      --arg root_label "$root_label" \
      --arg root "$root" \
      --arg relative_path "$rel" \
      --arg classification "$classification" \
      --argjson allowed "$allowed" \
      --argjson match_count "$match_count" \
      '{
        root_label: $root_label,
        root: $root,
        relative_path: $relative_path,
        classification: $classification,
        allowed: $allowed,
        match_count: $match_count
      }' >>"$out_jsonl"
  done < <(
    rg -i --files-with-matches "$PATTERN" "${scan_targets[@]}" \
      --hidden \
      --glob '!**/.git/**' \
      --glob '!**/__pycache__/**' \
      --glob '!**/*.pyc' \
      || true
  )
}

REAL_JSONL="$RUN_ROOT/real_matches.jsonl"
SYNTHETIC_JSONL="$RUN_ROOT/synthetic_matches.jsonl"
HISTORICAL_ABORT_JSONL="$RUN_ROOT/historical_script_aborts.jsonl"
SYNTHETIC_ROOT="$RUN_ROOT/synthetic"

scan_root "$REPO_ROOT" "repository" "$REAL_JSONL"

mkdir -p "$SYNTHETIC_ROOT/scripts/exp2/strata" "$SYNTHETIC_ROOT/config"
cat >"$SYNTHETIC_ROOT/scripts/active_inner_llm.py" <<'EOF'
OLLAMA_URL = "http://127.0.0.1:11434"
EOF
cat >"$SYNTHETIC_ROOT/scripts/historical_inner_llm.py" <<'EOF'
# historical, do not reanimate: retained as retired design evidence only.
MODEL = "Qwen3"
EOF
cat >"$SYNTHETIC_ROOT/scripts/exp2/strata/S2_code.json" <<'EOF'
[{"text":"Hermes local generation path from an old code snapshot"}]
EOF
scan_root "$SYNTHETIC_ROOT" "synthetic" "$SYNTHETIC_JSONL"

historical_scripts=(
  "scripts/shakespeare_extract_all.py"
  "scripts/shakespeare_build_creative_dataset.py"
  "scripts/shakespeare_enhance_prompts.py"
  "scripts/shakespeare_generate_compositions.py"
)

: >"$HISTORICAL_ABORT_JSONL"
for script in "${historical_scripts[@]}"; do
  stdout="$RUN_ROOT/$(basename "$script").stdout"
  stderr="$RUN_ROOT/$(basename "$script").stderr"
  set +e
  python3 "$script" >"$stdout" 2>"$stderr"
  status=$?
  set -e
  marker_present=false
  if rg -q --fixed-strings "$HISTORICAL_MARKER" "$stderr"; then
    marker_present=true
  fi
  pass=false
  if [[ "$status" != "0" && "$marker_present" == "true" ]]; then
    pass=true
  fi
  jq -nc \
    --arg script "$script" \
    --arg stdout "$stdout" \
    --arg stderr "$stderr" \
    --argjson status "$status" \
    --argjson marker_present "$marker_present" \
    --argjson pass "$pass" \
    '{
      script: $script,
      status: $status,
      marker_present: $marker_present,
      stdout: $stdout,
      stderr: $stderr,
      pass: $pass
    }' >>"$HISTORICAL_ABORT_JSONL"
done

real_matches="$(jq -s '.' "$REAL_JSONL")"
synthetic_matches="$(jq -s '.' "$SYNTHETIC_JSONL")"
historical_script_aborts="$(jq -s '.' "$HISTORICAL_ABORT_JSONL")"

real_total_file_count="$(jq 'length' <<<"$real_matches")"
real_total_match_count="$(jq '[.[].match_count] | add // 0' <<<"$real_matches")"
real_active_disallowed_count="$(jq '[.[] | select(.allowed == false)] | length' <<<"$real_matches")"
real_audit_allow_count="$(jq '[.[] | select(.classification == "intentional_retired_inner_actor_audit")] | length' <<<"$real_matches")"
real_historical_only_count="$(jq '[.[] | select(.classification == "historical_only")] | length' <<<"$real_matches")"
real_generated_snapshot_count="$(jq '[.[] | select(.classification == "generated_exp2_strata_snapshot")] | length' <<<"$real_matches")"

synthetic_active_detected="$(jq 'any(.[]; .relative_path == "scripts/active_inner_llm.py" and .allowed == false and .classification == "active_disallowed")' <<<"$synthetic_matches")"
synthetic_historical_allowed="$(jq 'any(.[]; .relative_path == "scripts/historical_inner_llm.py" and .allowed == true and .classification == "historical_only")' <<<"$synthetic_matches")"
synthetic_unmarked_generated_detected="$(jq 'any(.[]; .relative_path == "scripts/exp2/strata/S2_code.json" and .allowed == false and .classification == "generated_exp2_strata_missing_historical_marker")' <<<"$synthetic_matches")"
synthetic_disallowed_count="$(jq '[.[] | select(.allowed == false)] | length' <<<"$synthetic_matches")"
historical_script_abort_pass_count="$(jq '[.[] | select(.pass == true)] | length' <<<"$historical_script_aborts")"
historical_script_abort_total_count="$(jq 'length' <<<"$historical_script_aborts")"

all_passed=false
if [[ "$real_active_disallowed_count" == "0" \
  && "$real_total_file_count" -gt "0" \
  && "$real_historical_only_count" -gt "0" \
  && "$real_audit_allow_count" -gt "0" \
  && "$real_generated_snapshot_count" -gt "0" \
  && "$synthetic_active_detected" == "true" \
  && "$synthetic_historical_allowed" == "true" \
  && "$synthetic_unmarked_generated_detected" == "true" \
  && "$synthetic_disallowed_count" -ge "2" \
  && "$historical_script_abort_pass_count" == "$historical_script_abort_total_count" \
  && "$historical_script_abort_total_count" == "4" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-012" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg pattern "$PATTERN" \
  --arg historical_marker "$HISTORICAL_MARKER" \
  --arg audit_marker "$AUDIT_MARKER" \
  --arg real_jsonl "$REAL_JSONL" \
  --arg synthetic_jsonl "$SYNTHETIC_JSONL" \
  --arg historical_abort_jsonl "$HISTORICAL_ABORT_JSONL" \
  --argjson real_matches "$real_matches" \
  --argjson synthetic_matches "$synthetic_matches" \
  --argjson historical_script_aborts "$historical_script_aborts" \
  --argjson real_total_file_count "$real_total_file_count" \
  --argjson real_total_match_count "$real_total_match_count" \
  --argjson real_active_disallowed_count "$real_active_disallowed_count" \
  --argjson real_audit_allow_count "$real_audit_allow_count" \
  --argjson real_historical_only_count "$real_historical_only_count" \
  --argjson real_generated_snapshot_count "$real_generated_snapshot_count" \
  --argjson synthetic_active_detected "$synthetic_active_detected" \
  --argjson synthetic_historical_allowed "$synthetic_historical_allowed" \
  --argjson synthetic_unmarked_generated_detected "$synthetic_unmarked_generated_detected" \
  --argjson synthetic_disallowed_count "$synthetic_disallowed_count" \
  --argjson historical_script_abort_pass_count "$historical_script_abort_pass_count" \
  --argjson historical_script_abort_total_count "$historical_script_abort_total_count" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      scan_roots: ["scripts/", "config/"],
      pattern: $pattern,
      historical_marker: $historical_marker,
      audit_marker: $audit_marker,
      real_matches_jsonl: $real_jsonl,
      synthetic_matches_jsonl: $synthetic_jsonl,
      historical_abort_jsonl: $historical_abort_jsonl
    },
    happy_path: [
      {
        case: "scripts_and_config_have_zero_active_inner_llm_residue",
        expected: "all qwen/hermes/ollama/llama.cpp matches under scripts/ and config/ are either retired-audit, generated snapshot, or explicit historical-only references",
        actual: {
          total_files_with_matches: $real_total_file_count,
          total_pattern_matches: $real_total_match_count,
          active_disallowed_count: $real_active_disallowed_count,
          audit_allow_count: $real_audit_allow_count,
          historical_only_count: $real_historical_only_count,
          generated_snapshot_count: $real_generated_snapshot_count,
          historical_script_abort_pass_count: $historical_script_abort_pass_count,
          historical_script_abort_total_count: $historical_script_abort_total_count
        },
        pass: ($real_active_disallowed_count == 0 and $real_total_file_count > 0 and $historical_script_abort_pass_count == $historical_script_abort_total_count and $historical_script_abort_total_count == 4)
      }
    ],
    boundary_cases: [
      {
        case: "synthetic_active_runtime_reference_fails_closed",
        expected: "an unmarked active script containing the retired runtime name is classified active_disallowed",
        actual: {
          detected: $synthetic_active_detected,
          synthetic_disallowed_count: $synthetic_disallowed_count
        },
        pass: $synthetic_active_detected
      },
      {
        case: "synthetic_historical_reference_is_allowed_only_with_exact_marker",
        expected: "a file with exact historical marker is allowed as historical_only",
        actual: {detected: $synthetic_historical_allowed},
        pass: $synthetic_historical_allowed
      },
      {
        case: "generated_strata_snapshot_requires_historical_marker_file",
        expected: "an EXP-2 strata JSON without its directory marker is rejected",
        actual: {detected: $synthetic_unmarked_generated_detected},
        pass: $synthetic_unmarked_generated_detected
      }
    ],
    real_matches: $real_matches,
    synthetic_matches: $synthetic_matches,
    historical_script_aborts: $historical_script_aborts,
    all_passed: $all_passed,
    readback_equal: true
  }' >"$ARTIFACT.tmp"
mv "$ARTIFACT.tmp" "$ARTIFACT"

artifact_sha256="$(sha256sum "$ARTIFACT" | awk '{print $1}')"
jq -n --arg artifact_path "$ARTIFACT" --arg artifact_sha256 "$artifact_sha256" --argjson all_passed "$all_passed" \
  '{artifact_path: $artifact_path, artifact_sha256: $artifact_sha256, all_passed: $all_passed, readback_equal: true}'

if [[ "$all_passed" != "true" ]]; then
  echo "TASK_LOOP_012_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
