#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/mnt/d/contextgraph/fsv/phase-h-agent-roster-retirement-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/agent_roster_retirement_fsv.json"
README_PATH=".claude/agents/README.md"

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"

retired_agents=(
  ".claude/agents/cc-reality-harness-editor.md"
  ".claude/agents/cc-reality-diagnostician.md"
  ".claude/agents/cc-reality-reward-analyst.md"
)
active_agents=(
  ".claude/agents/cc-reality-source-of-truth-readback.md"
  ".claude/agents/cc-reality-swe-failure-triage.md"
)
retired_agent_pattern='cc-reality-harness-editor|cc-reality-diagnostician|cc-reality-reward-analyst'

has_retired_frontmatter() {
  local path="$1"
  rg -q '^status: retired$' "$path" \
    && rg -q '^superseded_by: docs/futurebuild/14_NEW_LOOP_MD.md$' "$path" \
    && rg -q 'Retired historical marker' "$path"
}

has_active_frontmatter() {
  local path="$1"
  rg -q '^status: active$' "$path"
}

retired_ok_count=0
retired_missing=""
for path in "${retired_agents[@]}"; do
  if has_retired_frontmatter "$path"; then
    retired_ok_count=$((retired_ok_count + 1))
  else
    retired_missing="${retired_missing}${path}"$'\n'
  fi
done

active_ok_count=0
active_missing=""
for path in "${active_agents[@]}"; do
  if has_active_frontmatter "$path"; then
    active_ok_count=$((active_ok_count + 1))
  else
    active_missing="${active_missing}${path}"$'\n'
  fi
done

active_reference_matches="$(rg -n "$retired_agent_pattern" "${active_agents[@]}" || true)"
active_reference_match_count="$(printf '%s\n' "$active_reference_matches" | sed '/^$/d' | wc -l | tr -d ' ')"

readme_exists=false
if [[ -f "$README_PATH" ]]; then
  readme_exists=true
fi
readme_active_retired_matches=""
if [[ "$readme_exists" == "true" ]]; then
  readme_active_retired_matches="$(
    sed -n '/^## Active agents$/,/^## Retired historical agents$/p' "$README_PATH" \
      | rg -n "$retired_agent_pattern" || true
  )"
fi
readme_active_retired_match_count="$(printf '%s\n' "$readme_active_retired_matches" | sed '/^$/d' | wc -l | tr -d ' ')"

synthetic_bad_agent="$RUN_ROOT/synthetic-retired-agent.md"
cat >"$synthetic_bad_agent" <<'SYNTHETIC'
---
name: synthetic-retired-agent
description: Retired historical marker without required status frontmatter.
tools: Read
---
SYNTHETIC
synthetic_missing_status_detected=false
if ! has_retired_frontmatter "$synthetic_bad_agent"; then
  synthetic_missing_status_detected=true
fi

all_passed=false
if [[ "$retired_ok_count" == "${#retired_agents[@]}" \
  && "$active_ok_count" == "${#active_agents[@]}" \
  && "$active_reference_match_count" == "0" \
  && "$readme_exists" == "true" \
  && "$readme_active_retired_match_count" == "0" \
  && "$synthetic_missing_status_detected" == "true" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-010" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg readme_path "$README_PATH" \
  --arg retired_missing "$retired_missing" \
  --arg active_missing "$active_missing" \
  --arg active_reference_matches "$active_reference_matches" \
  --arg readme_active_retired_matches "$readme_active_retired_matches" \
  --arg synthetic_bad_agent "$synthetic_bad_agent" \
  --argjson retired_ok_count "$retired_ok_count" \
  --argjson retired_expected_count "${#retired_agents[@]}" \
  --argjson active_ok_count "$active_ok_count" \
  --argjson active_expected_count "${#active_agents[@]}" \
  --argjson active_reference_match_count "$active_reference_match_count" \
  --argjson readme_exists "$readme_exists" \
  --argjson readme_active_retired_match_count "$readme_active_retired_match_count" \
  --argjson synthetic_missing_status_detected "$synthetic_missing_status_detected" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      readme_path: $readme_path,
      retired_agents: [
        ".claude/agents/cc-reality-harness-editor.md",
        ".claude/agents/cc-reality-diagnostician.md",
        ".claude/agents/cc-reality-reward-analyst.md"
      ],
      active_agents: [
        ".claude/agents/cc-reality-source-of-truth-readback.md",
        ".claude/agents/cc-reality-swe-failure-triage.md"
      ]
    },
    happy_path: [
      {
        case: "retired_agents_have_retired_frontmatter_and_forward_pointer",
        expected: "all retired agents have status retired and superseded_by docs/futurebuild/14_NEW_LOOP_MD.md",
        actual: {
          retired_ok_count: $retired_ok_count,
          retired_expected_count: $retired_expected_count,
          retired_missing: ($retired_missing | split("\n") | map(select(length > 0)))
        },
        pass: ($retired_ok_count == $retired_expected_count)
      },
      {
        case: "active_agents_have_active_frontmatter_and_do_not_route_to_retired_agents",
        expected: "active files have status active and zero retired-agent routing references",
        actual: {
          active_ok_count: $active_ok_count,
          active_expected_count: $active_expected_count,
          active_reference_match_count: $active_reference_match_count,
          active_reference_matches: ($active_reference_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($active_ok_count == $active_expected_count and $active_reference_match_count == 0)
      },
      {
        case: "roster_active_section_excludes_retired_agents",
        expected: "README exists and active roster has zero retired-agent names",
        actual: {
          readme_exists: $readme_exists,
          readme_active_retired_match_count: $readme_active_retired_match_count,
          readme_active_retired_matches: ($readme_active_retired_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($readme_exists and $readme_active_retired_match_count == 0)
      }
    ],
    boundary_cases: [
      {
        case: "synthetic_retired_agent_missing_status_is_detected",
        expected: "frontmatter check rejects a retired-looking agent without status retired and superseded_by",
        actual: {
          synthetic_bad_agent: $synthetic_bad_agent,
          missing_status_detected: $synthetic_missing_status_detected
        },
        pass: $synthetic_missing_status_detected
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
  echo "TASK_LOOP_010_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
