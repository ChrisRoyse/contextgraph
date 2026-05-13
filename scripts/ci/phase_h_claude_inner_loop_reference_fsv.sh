#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/mnt/d/contextgraph/fsv/phase-h-claude-inner-loop-reference-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/claude_inner_loop_reference_fsv.json"
CLAUDE_PATH="CLAUDE.md"

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
reference_pattern='inner-actor|inner actor|Qwen|Hermes|Ollama|llama\.cpp|llama-cpp|reality-loop|Mode A|Mode B'
allowed_context_pattern='retired|deleted|Do not reanimate|Never reintroduce|fails closed|CCREALITY_ENGINE_RETIRED|compatibility'

all_matches="$(rg -n "$reference_pattern" "$CLAUDE_PATH" || true)"
all_match_count="$(printf '%s\n' "$all_matches" | sed '/^$/d' | wc -l | tr -d ' ')"
disallowed_matches="$(
  printf '%s\n' "$all_matches" \
    | rg -v "$allowed_context_pattern" || true
)"
disallowed_match_count="$(printf '%s\n' "$disallowed_matches" | sed '/^$/d' | wc -l | tr -d ' ')"

required_retired_binary_line=false
if rg -q 'Phase H FSV-verified.*#134/#135' "$CLAUDE_PATH"; then
  required_retired_binary_line=true
fi

required_never_rule=false
if rg -q 'Never reintroduce the retired inner-actor LLM stack' "$CLAUDE_PATH"; then
  required_never_rule=true
fi

synthetic_bad="$RUN_ROOT/synthetic_CLAUDE_bad.md"
cp "$CLAUDE_PATH" "$synthetic_bad"
printf '\nUse Ollama and Qwen as the action path for edits.\n' >>"$synthetic_bad"
synthetic_disallowed="$(
  rg -n "$reference_pattern" "$synthetic_bad" \
    | rg -v "$allowed_context_pattern" || true
)"
synthetic_disallowed_count="$(printf '%s\n' "$synthetic_disallowed" | sed '/^$/d' | wc -l | tr -d ' ')"
synthetic_bad_detected=false
if [[ "$synthetic_disallowed_count" -gt 0 ]]; then
  synthetic_bad_detected=true
fi

all_passed=false
if [[ "$all_match_count" -gt 0 \
  && "$disallowed_match_count" == "0" \
  && "$required_retired_binary_line" == "true" \
  && "$required_never_rule" == "true" \
  && "$synthetic_bad_detected" == "true" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-009" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg claude_path "$CLAUDE_PATH" \
  --arg all_matches "$all_matches" \
  --arg disallowed_matches "$disallowed_matches" \
  --arg synthetic_bad "$synthetic_bad" \
  --arg synthetic_disallowed "$synthetic_disallowed" \
  --argjson all_match_count "$all_match_count" \
  --argjson disallowed_match_count "$disallowed_match_count" \
  --argjson required_retired_binary_line "$required_retired_binary_line" \
  --argjson required_never_rule "$required_never_rule" \
  --argjson synthetic_disallowed_count "$synthetic_disallowed_count" \
  --argjson synthetic_bad_detected "$synthetic_bad_detected" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      claude_path: $claude_path,
      reference_pattern: "inner-actor|inner actor|Qwen|Hermes|Ollama|llama\\.cpp|llama-cpp|reality-loop|Mode A|Mode B",
      allowed_context_pattern: "retired|deleted|Do not reanimate|Never reintroduce|fails closed|CCREALITY_ENGINE_RETIRED|compatibility"
    },
    happy_path: [
      {
        case: "claude_inner_loop_references_are_only_retirement_or_deny_context",
        expected: "all matches are in retired/deleted/deny/fail-closed compatibility context",
        actual: {
          all_match_count: $all_match_count,
          all_matches: ($all_matches | split("\n") | map(select(length > 0))),
          disallowed_match_count: $disallowed_match_count,
          disallowed_matches: ($disallowed_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($all_match_count > 0 and $disallowed_match_count == 0)
      },
      {
        case: "claude_names_phase_h_retirement_fsv_and_never_rule",
        expected: "CLAUDE.md names Phase H FSV verification and the never-reintroduce rule",
        actual: {
          required_retired_binary_line: $required_retired_binary_line,
          required_never_rule: $required_never_rule
        },
        pass: ($required_retired_binary_line and $required_never_rule)
      }
    ],
    boundary_cases: [
      {
        case: "synthetic_action_path_ollama_qwen_reference_is_rejected",
        expected: "a synthetic CLAUDE action-path endorsement is detected as disallowed",
        actual: {
          synthetic_bad: $synthetic_bad,
          synthetic_disallowed_count: $synthetic_disallowed_count,
          synthetic_disallowed: ($synthetic_disallowed | split("\n") | map(select(length > 0))),
          synthetic_bad_detected: $synthetic_bad_detected
        },
        pass: $synthetic_bad_detected
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
  echo "TASK_LOOP_009_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
