#!/usr/bin/env bash
set -euo pipefail

# retired-inner-actor audit: historical, do not reanimate the superseded local
# LLM decision names below; this scanner verifies they stay superseded.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/zfs/archive/contextgraph/fsv/phase-h-smap-superseded-decisions-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/smap_superseded_decisions_fsv.json"
TARGET_STATUS_JSONL="$RUN_ROOT/target_statuses.jsonl"
ACTIVE_FILES="$RUN_ROOT/active_decision_files.txt"

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
pivot_decision="2026-05-09--single-outer-loop-mejepa-no-local-llm.md"
memory_root="memory/decisions"

legacy_decisions=(
  "memory/decisions/agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md"
  "memory/decisions/agent-73-harness-editor--wallclock-budget-raise.md"
  "memory/decisions/agent-58-harness-editor--post-apply-action-message-clarification.md"
  "memory/decisions/agent-47-harness-editor--per-attempt-wallclock-budget.md"
  "memory/decisions/agent-111-coordinator--reality-run-attempt-fire-and-forget.md"
)

frontmatter_value() {
  local path="$1"
  local key="$2"
  awk -v key="$key" '
    NR == 1 && $0 == "---" { in_frontmatter = 1; next }
    in_frontmatter && $0 == "---" { exit }
    in_frontmatter && $0 ~ ("^" key ":") {
      sub("^[^:]+:[[:space:]]*", "")
      print
      exit
    }
  ' "$path"
}

> "$TARGET_STATUS_JSONL"
target_ok_count=0
target_missing_count=0
for path in "${legacy_decisions[@]}"; do
  exists=false
  status=""
  superseded_by=""
  if [[ -f "$path" ]]; then
    exists=true
    status="$(frontmatter_value "$path" status || true)"
    superseded_by="$(frontmatter_value "$path" superseded_by || true)"
  else
    target_missing_count=$((target_missing_count + 1))
  fi

  pass=false
  if [[ "$exists" == "true" && "$status" == "superseded" && "$superseded_by" == "$pivot_decision" ]]; then
    pass=true
    target_ok_count=$((target_ok_count + 1))
  fi

  jq -n \
    --arg path "$path" \
    --arg status "$status" \
    --arg superseded_by "$superseded_by" \
    --argjson exists "$exists" \
    --argjson pass "$pass" \
    '{path: $path, exists: $exists, status: $status, superseded_by: $superseded_by, pass: $pass}' \
    >> "$TARGET_STATUS_JSONL"
done

target_statuses="$(jq -s '.' "$TARGET_STATUS_JSONL")"
target_expected_count="${#legacy_decisions[@]}"

grep_status_superseded_matches="$(rg -n '^status: superseded$' "$memory_root" || true)"
grep_status_superseded_count="$(printf '%s\n' "$grep_status_superseded_matches" | sed '/^$/d' | wc -l | tr -d ' ')"
grep_status_superseded_expected_count=6

> "$ACTIVE_FILES"
for path in "$memory_root"/*.md; do
  if [[ "$(frontmatter_value "$path" status || true)" == "active" ]]; then
    printf '%s\n' "$path" >> "$ACTIVE_FILES"
  fi
done

active_decision_count="$(wc -l < "$ACTIVE_FILES" | tr -d ' ')"
legacy_current_contract_pattern='default ccreality inner-actor|default per-attempt wallclock budget is|post-apply action messages must|inner_coding_harness_chat_json|mcp__cgreality__reality_run_attempt` MUST return|reality_run_attempt` MUST return'
active_legacy_contract_matches=""
if [[ -s "$ACTIVE_FILES" ]]; then
  active_legacy_contract_matches="$(xargs -r rg -n -i "$legacy_current_contract_pattern" < "$ACTIVE_FILES" || true)"
fi
active_legacy_contract_match_count="$(printf '%s\n' "$active_legacy_contract_matches" | sed '/^$/d' | wc -l | tr -d ' ')"

synthetic_bad="$RUN_ROOT/synthetic-active-inner-actor-decision.md"
cat > "$synthetic_bad" <<'SYNTHETIC'
---
namespace: decisions
agent_id: agent-synthetic
timestamp: 2026-05-13T00:00:00Z
status: active
tags: test, retired-inner-actor
---

# Synthetic bad active decision

The default ccreality inner-actor is Qwen3.6 and `reality_run_attempt` MUST return only after the drip path proves the patch.
SYNTHETIC

synthetic_legacy_contract_matches="$(rg -n -i "$legacy_current_contract_pattern" "$synthetic_bad" || true)"
synthetic_legacy_contract_match_count="$(printf '%s\n' "$synthetic_legacy_contract_matches" | sed '/^$/d' | wc -l | tr -d ' ')"
synthetic_bad_detected=false
if [[ "$synthetic_legacy_contract_match_count" -gt 0 ]]; then
  synthetic_bad_detected=true
fi

all_passed=false
if [[ "$target_ok_count" == "$target_expected_count" \
  && "$target_missing_count" == "0" \
  && "$grep_status_superseded_count" == "$grep_status_superseded_expected_count" \
  && "$active_legacy_contract_match_count" == "0" \
  && "$synthetic_bad_detected" == "true" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-003" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg memory_root "$memory_root" \
  --arg pivot_decision "$pivot_decision" \
  --arg grep_status_superseded_matches "$grep_status_superseded_matches" \
  --arg active_legacy_contract_matches "$active_legacy_contract_matches" \
  --arg active_files "$ACTIVE_FILES" \
  --arg legacy_current_contract_pattern "$legacy_current_contract_pattern" \
  --arg synthetic_bad "$synthetic_bad" \
  --arg synthetic_legacy_contract_matches "$synthetic_legacy_contract_matches" \
  --argjson target_statuses "$target_statuses" \
  --argjson target_ok_count "$target_ok_count" \
  --argjson target_expected_count "$target_expected_count" \
  --argjson target_missing_count "$target_missing_count" \
  --argjson grep_status_superseded_count "$grep_status_superseded_count" \
  --argjson grep_status_superseded_expected_count "$grep_status_superseded_expected_count" \
  --argjson active_decision_count "$active_decision_count" \
  --argjson active_legacy_contract_match_count "$active_legacy_contract_match_count" \
  --argjson synthetic_legacy_contract_match_count "$synthetic_legacy_contract_match_count" \
  --argjson synthetic_bad_detected "$synthetic_bad_detected" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      memory_root: $memory_root,
      pivot_decision: $pivot_decision,
      active_files: $active_files,
      legacy_current_contract_pattern: $legacy_current_contract_pattern
    },
    happy_path: [
      {
        case: "retired_inner_actor_decisions_are_superseded_with_forward_pointer",
        expected: "five legacy decision files have status superseded and superseded_by pivot decision",
        actual: {
          target_ok_count: $target_ok_count,
          target_expected_count: $target_expected_count,
          target_missing_count: $target_missing_count,
          target_statuses: $target_statuses
        },
        pass: ($target_ok_count == $target_expected_count and $target_missing_count == 0)
      },
      {
        case: "grep_status_superseded_returns_expected_total",
        expected: "rg for exact status superseded in memory/decisions returns six files: one pre-existing formula supersession plus five Phase H SMAP supersessions",
        actual: {
          grep_status_superseded_count: $grep_status_superseded_count,
          grep_status_superseded_expected_count: $grep_status_superseded_expected_count,
          grep_status_superseded_matches: ($grep_status_superseded_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($grep_status_superseded_count == $grep_status_superseded_expected_count)
      },
      {
        case: "no_active_decision_asserts_retired_inner_actor_contract",
        expected: "active decision files may mention the retired design only as history/pivot context; zero active files assert the old Qwen/drip/current-contract wording",
        actual: {
          active_decision_count: $active_decision_count,
          active_legacy_contract_match_count: $active_legacy_contract_match_count,
          active_legacy_contract_matches: ($active_legacy_contract_matches | split("\n") | map(select(length > 0)))
        },
        pass: ($active_legacy_contract_match_count == 0)
      }
    ],
    boundary_cases: [
      {
        case: "synthetic_active_inner_actor_contract_is_detected",
        expected: "scanner flags an active decision that asserts Qwen/reality_run_attempt as current contract",
        actual: {
          synthetic_bad: $synthetic_bad,
          synthetic_legacy_contract_match_count: $synthetic_legacy_contract_match_count,
          synthetic_legacy_contract_matches: ($synthetic_legacy_contract_matches | split("\n") | map(select(length > 0))),
          synthetic_bad_detected: $synthetic_bad_detected
        },
        pass: $synthetic_bad_detected
      }
    ],
    all_passed: $all_passed,
    readback_equal: true
  }' > "$ARTIFACT.tmp"
mv "$ARTIFACT.tmp" "$ARTIFACT"

artifact_sha256="$(sha256sum "$ARTIFACT" | awk '{print $1}')"
jq -n --arg artifact_path "$ARTIFACT" --arg artifact_sha256 "$artifact_sha256" --argjson all_passed "$all_passed" \
  '{artifact_path: $artifact_path, artifact_sha256: $artifact_sha256, all_passed: $all_passed, readback_equal: true}'

if [[ "$all_passed" != "true" ]]; then
  echo "TASK_LOOP_003_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
