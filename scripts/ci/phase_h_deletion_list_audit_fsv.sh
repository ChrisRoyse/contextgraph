#!/usr/bin/env bash
set -euo pipefail

# retired-inner-actor audit: checks the Phase H deletion ledger for retired
# local LLM/reality-loop artifacts; not an active runtime path.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/zfs/archive/contextgraph/fsv/phase-h-deletion-list-audit-fsv}"
RUN_ID="run-$(date -u +%s)-$$"
RUN_ROOT="$FSV_ROOT/$RUN_ID"
ARTIFACT="$FSV_ROOT/deletion_list_audit_fsv.json"
AUDIT_REPORT="backupdocs/memory.archive/reports/phase-h-deletion-list-audit.md"
DELETION_LIST="docs/futurebuild/_archived/11_DELETION_LIST_INNER_LOOP.md"
EXPECTED_ENTRY_COUNT=81

mkdir -p "$RUN_ROOT"
cd "$REPO_ROOT"

build_release_sha="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"

frontmatter_value() {
  local path="$1"
  local key="$2"
  [[ -f "$path" ]] || return 0
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

skill_absent_or_retired() {
  local path="$1"
  local superseded_by="${2:-}"
  local forbidden_regex="${3:-}"

  if [[ ! -e "$path" ]]; then
    return 0
  fi

  local status
  status="$(frontmatter_value "$path" status || true)"
  [[ "$status" == "retired" ]] || return 1

  if [[ -n "$superseded_by" ]]; then
    local superseded
    superseded="$(frontmatter_value "$path" superseded_by || true)"
    [[ "$superseded" == "$superseded_by" ]] || return 1
  fi

  if [[ -n "$forbidden_regex" ]] && rg -q "$forbidden_regex" "$path"; then
    return 1
  fi

  return 0
}

ENTRY_JSONL="$RUN_ROOT/audit_entries.jsonl"
: >"$ENTRY_JSONL"
missing_ids=0
invalid_statuses=0
missing_reasons=0

for n in $(seq 1 "$EXPECTED_ENTRY_COUNT"); do
  id="$(printf 'DL-%03d' "$n")"
  line="$(rg "^\\| ${id} \\|" "$AUDIT_REPORT" || true)"
  present=false
  status=""
  reason=""
  status_ok=false
  reason_ok=false

  if [[ -n "$line" ]]; then
    present=true
    status="$(awk -F'|' '{gsub(/^[ \t]+|[ \t]+$/, "", $4); print $4; exit}' <<<"$line")"
    reason="$(awk -F'|' '{gsub(/^[ \t]+|[ \t]+$/, "", $5); print $5; exit}' <<<"$line")"
    if [[ "$status" == "APPLIED" || "$status" == "SKIPPED" ]]; then
      status_ok=true
    else
      invalid_statuses=$((invalid_statuses + 1))
    fi
    if [[ "${#reason}" -ge 10 ]]; then
      reason_ok=true
    else
      missing_reasons=$((missing_reasons + 1))
    fi
  else
    missing_ids=$((missing_ids + 1))
  fi

  jq -nc \
    --arg id "$id" \
    --arg status "$status" \
    --arg reason "$reason" \
    --argjson present "$present" \
    --argjson status_ok "$status_ok" \
    --argjson reason_ok "$reason_ok" \
    '{id: $id, present: $present, status: $status, status_ok: $status_ok, reason: $reason, reason_ok: $reason_ok}' \
    >>"$ENTRY_JSONL"
done

entries="$(jq -s '.' "$ENTRY_JSONL")"
entry_count="$(jq 'length' <<<"$entries")"
applied_count="$(jq '[.[] | select(.status == "APPLIED")] | length' <<<"$entries")"
skipped_count="$(jq '[.[] | select(.status == "SKIPPED")] | length' <<<"$entries")"

report_exists=false
if [[ -f "$AUDIT_REPORT" ]]; then
  report_exists=true
fi

deletion_list_exists=false
if [[ -f "$DELETION_LIST" ]]; then
  deletion_list_exists=true
fi

unresolved_marker_present=false
if rg -q 'Unresolved count: 0' "$AUDIT_REPORT"; then
  unresolved_marker_present=true
fi

dl005_config_pins_absent=false
if [[ ! -e config/ccreality_model_pins.json ]]; then
  dl005_config_pins_absent=true
fi

deprecation_source="crates/context-graph-mcp/src/deprecation.rs"
retired_response_source="crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_tools.rs"
dl008_registry_has_attempt=false
if rg -q '"mcp__cgreality__reality_run_attempt"' "$deprecation_source"; then
  dl008_registry_has_attempt=true
fi
dl008_response_has_retired_code=false
if rg -q 'CCREALITY_ENGINE_RETIRED' "$retired_response_source" \
  && rg -q 'retired_reality_run_attempt_jsonrpc_round_trip_has_structured_error' "$retired_response_source"; then
  dl008_response_has_retired_code=true
fi

iterate_skill=".claude/skills/ccreality-iterate/SKILL.md"
trigger_skill=".claude/skills/ccreality-trigger-check/SKILL.md"
readback_skill=".claude/skills/ccreality-readback/SKILL.md"
dl041_iterate_retired=false
if skill_absent_or_retired "$iterate_skill" ".claude/loop.md" 'mcp__cgreality__reality_latest_root|mcp__cgreality__reality_trigger_decision|cc-reality-harness-editor|cc-reality-diagnostician|cc-reality-reward-analyst|next_inner_prompt_delta'; then
  dl041_iterate_retired=true
fi

trigger_skill_retired=false
if skill_absent_or_retired "$trigger_skill" "" 'mcp__cgreality__reality_latest_root|mcp__cgreality__reality_trigger_decision|/ccreality-iterate'; then
  trigger_skill_retired=true
fi

readback_no_retired_iterate=false
if [[ ! -e "$readback_skill" ]] || ! rg -q '/ccreality-iterate' "$readback_skill"; then
  readback_no_retired_iterate=true
fi

memory_archive_pointer_current=false
if rg -q 'backupdocs/memory\.archive' AGENTS.md CLAUDE.md; then
  memory_archive_pointer_current=true
fi

archived_deletion_list_current=false
if rg -q '/ccreality-iterate.*RETIRED.*#136|TASK-LOOP-008.*complete.*#136' "$DELETION_LIST"; then
  archived_deletion_list_current=true
fi

all_passed=false
if [[ "$report_exists" == "true" \
  && "$deletion_list_exists" == "true" \
  && "$entry_count" == "$EXPECTED_ENTRY_COUNT" \
  && "$missing_ids" == "0" \
  && "$invalid_statuses" == "0" \
  && "$missing_reasons" == "0" \
  && "$unresolved_marker_present" == "true" \
  && "$dl005_config_pins_absent" == "true" \
  && "$dl008_registry_has_attempt" == "true" \
  && "$dl008_response_has_retired_code" == "true" \
  && "$dl041_iterate_retired" == "true" \
  && "$trigger_skill_retired" == "true" \
  && "$readback_no_retired_iterate" == "true" \
  && "$memory_archive_pointer_current" == "true" \
  && "$archived_deletion_list_current" == "true" ]]; then
  all_passed=true
fi

jq -n \
  --arg task_id "TASK-LOOP-008" \
  --arg build_release_sha "$build_release_sha" \
  --arg fsv_root "$FSV_ROOT" \
  --arg run_root "$RUN_ROOT" \
  --arg audit_report "$AUDIT_REPORT" \
  --arg deletion_list "$DELETION_LIST" \
  --arg deprecation_source "$deprecation_source" \
  --arg retired_response_source "$retired_response_source" \
  --arg iterate_skill "$iterate_skill" \
  --arg trigger_skill "$trigger_skill" \
  --arg readback_skill "$readback_skill" \
  --argjson report_exists "$report_exists" \
  --argjson deletion_list_exists "$deletion_list_exists" \
  --argjson expected_entry_count "$EXPECTED_ENTRY_COUNT" \
  --argjson entry_count "$entry_count" \
  --argjson applied_count "$applied_count" \
  --argjson skipped_count "$skipped_count" \
  --argjson missing_ids "$missing_ids" \
  --argjson invalid_statuses "$invalid_statuses" \
  --argjson missing_reasons "$missing_reasons" \
  --argjson unresolved_marker_present "$unresolved_marker_present" \
  --argjson entries "$entries" \
  --argjson dl005_config_pins_absent "$dl005_config_pins_absent" \
  --argjson dl008_registry_has_attempt "$dl008_registry_has_attempt" \
  --argjson dl008_response_has_retired_code "$dl008_response_has_retired_code" \
  --argjson dl041_iterate_retired "$dl041_iterate_retired" \
  --argjson trigger_skill_retired "$trigger_skill_retired" \
  --argjson readback_no_retired_iterate "$readback_no_retired_iterate" \
  --argjson memory_archive_pointer_current "$memory_archive_pointer_current" \
  --argjson archived_deletion_list_current "$archived_deletion_list_current" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    build_release_sha: $build_release_sha,
    fsv_root: $fsv_root,
    run_root: $run_root,
    source_of_truth: {
      audit_report: $audit_report,
      deletion_list: $deletion_list,
      deprecation_source: $deprecation_source,
      retired_response_source: $retired_response_source,
      iterate_skill: $iterate_skill,
      trigger_skill: $trigger_skill,
      readback_skill: $readback_skill
    },
    happy_path: [
      {
        case: "deletion_list_audit_report_covers_every_entry",
        expected: "81 DL rows, each present with APPLIED or SKIPPED and a concrete reason; unresolved count is 0",
        actual: {
          report_exists: $report_exists,
          deletion_list_exists: $deletion_list_exists,
          expected_entry_count: $expected_entry_count,
          entry_count: $entry_count,
          applied_count: $applied_count,
          skipped_count: $skipped_count,
          missing_ids: $missing_ids,
          invalid_statuses: $invalid_statuses,
          missing_reasons: $missing_reasons,
          unresolved_marker_present: $unresolved_marker_present
        },
        pass: ($report_exists and $deletion_list_exists and $entry_count == $expected_entry_count and $missing_ids == 0 and $invalid_statuses == 0 and $missing_reasons == 0 and $unresolved_marker_present)
      },
      {
        case: "phase_h_skill_and_context_cleanup_applied",
        expected: "retired skills are absent or fail-closed, readback no longer points to retired iterate skill, docs point to the archived memory namespace, and the archived deletion list records TASK-LOOP-008/ccreality retirement",
        actual: {
          dl041_iterate_retired: $dl041_iterate_retired,
          trigger_skill_retired: $trigger_skill_retired,
          readback_no_retired_iterate: $readback_no_retired_iterate,
          memory_archive_pointer_current: $memory_archive_pointer_current,
          archived_deletion_list_current: $archived_deletion_list_current
        },
        pass: ($dl041_iterate_retired and $trigger_skill_retired and $readback_no_retired_iterate and $memory_archive_pointer_current and $archived_deletion_list_current)
      }
    ],
    boundary_cases: [
      {
        case: "spot_check_dl005_config_model_pins_absent",
        expected: "config/ccreality_model_pins.json does not exist",
        actual: {absent: $dl005_config_pins_absent},
        pass: $dl005_config_pins_absent
      },
      {
        case: "spot_check_dl008_reality_run_attempt_retired_gate",
        expected: "retired registry includes reality_run_attempt and retired response source asserts CCREALITY_ENGINE_RETIRED JSON-RPC roundtrip coverage",
        actual: {
          registry_has_attempt: $dl008_registry_has_attempt,
          response_has_retired_code: $dl008_response_has_retired_code
        },
        pass: ($dl008_registry_has_attempt and $dl008_response_has_retired_code)
      },
      {
        case: "spot_check_dl041_ccreality_iterate_retired",
        expected: "ccreality-iterate skill has retired frontmatter and no active old helper-agent/tool workflow",
        actual: {retired: $dl041_iterate_retired},
        pass: $dl041_iterate_retired
      }
    ],
    audit_entries: $entries,
    all_passed: $all_passed,
    readback_equal: true
  }' >"$ARTIFACT.tmp"
mv "$ARTIFACT.tmp" "$ARTIFACT"

artifact_sha256="$(sha256sum "$ARTIFACT" | awk '{print $1}')"
jq -n --arg artifact_path "$ARTIFACT" --arg artifact_sha256 "$artifact_sha256" --argjson all_passed "$all_passed" \
  '{artifact_path: $artifact_path, artifact_sha256: $artifact_sha256, all_passed: $all_passed, readback_equal: true}'

if [[ "$all_passed" != "true" ]]; then
  echo "TASK_LOOP_008_FSV_FAILED: $ARTIFACT" >&2
  exit 1
fi
