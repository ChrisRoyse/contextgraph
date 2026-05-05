#!/usr/bin/env bash
set -Eeuo pipefail

PROJECT_ROOT="${PROJECT_ROOT:-/home/cabdru/contextgraph}"
FSV_ROOT="${PROJECT_ROOT}/scripts/cc/tests/fsv"
RUN_STAMP="$(date +%s)"
FSV_RUN_ROOT="${FSV_RUN_ROOT:-/tmp/cgreality-fsv-${RUN_STAMP}}"
RESULTS_DATE="$(date -u +%Y-%m-%d)"
RESULTS_FILE="${PROJECT_ROOT}/docs/ccreality/FSV_RESULTS_${RESULTS_DATE}.md"
MASTER_LOG="${FSV_RUN_ROOT}/fsv-master.log"
CASE_LOG_DIR="${FSV_RUN_ROOT}/logs"
MCP_STDERR="${FSV_RUN_ROOT}/mcp-stderr.log"

on_error() {
  local line="$1"
  local status="$2"
  printf 'FSV runner failed at line=%s exit=%s command=%s\n' "${line}" "${status}" "${BASH_COMMAND}" >&2
}
trap 'on_error ${LINENO} $?' ERR

mkdir -p "${CASE_LOG_DIR}" "$(dirname "${RESULTS_FILE}")"
cd "${PROJECT_ROOT}"

{
  printf 'Phase 10 FSV started at %s\n' "$(date -u +%FT%TZ)"
  printf 'project=%s\n' "${PROJECT_ROOT}"
  printf 'run_root=%s\n' "${FSV_RUN_ROOT}"
} >"${MASTER_LOG}"

printf 'Building Phase 10 binaries...\n' | tee -a "${MASTER_LOG}"
{
  cargo build -p context-graph-cli --bin reality-loop --quiet
  cargo build -p context-graph-cli --bin context-graph --quiet
  cargo build -p context-graph-cli --bin context-graph-cli --quiet
  cargo build -p context-graph-mcp --bin context-graph-mcp --quiet
} 2>>"${MASTER_LOG}"

mapfile -t CASES < <(find "${FSV_ROOT}/cases" -maxdepth 1 -type f -name '*.sh' -printf '%f\n' | sort -V)
[ "${#CASES[@]}" -ge 39 ] || {
  printf 'expected at least 39 FSV case scripts, found %s\n' "${#CASES[@]}" >&2
  exit 1
}

BRANCH="$(git branch --show-current 2>/dev/null || printf 'unknown')"
HEAD_SHA="$(git rev-parse HEAD 2>/dev/null || printf 'unknown')"

{
  printf '# FSV Results - %s\n\n' "${RESULTS_DATE}"
  printf "Branch: \`%s\`\n\n" "${BRANCH}"
  printf "HEAD: \`%s\`\n\n" "${HEAD_SHA}"
  printf "Run root: \`%s\`\n\n" "${FSV_RUN_ROOT}"
  printf "Master log: \`%s\`\n\n" "${MASTER_LOG}"
  printf "Source of truth: runtime artifact files under each case work directory, \`~/.cache/cgreality/active_runtime_root\`, shift logs under \`cgreality-shift-log\`, hook event JSONL files, and BlackBox \`ledger_records\` rows in each case ledger.\n\n"
  printf '## Per-case Results\n\n'
  printf '| Case | Status | Time (s) | Notes |\n'
  printf '|------|--------|----------|-------|\n'
} >"${RESULTS_FILE}"

PASS=0
FAIL=0

for case_file in "${CASES[@]}"; do
  case_name="${case_file%.sh}"
  case_log="${CASE_LOG_DIR}/${case_name}.log"
  start="$(date +%s)"
  printf 'Running %s...\n' "${case_name}" | tee -a "${MASTER_LOG}"
  if env FSV_RUN_ROOT="${FSV_RUN_ROOT}" FSV_CASE_ROOT="${FSV_RUN_ROOT}/cases" MCP_STDERR="${MCP_STDERR}" PROJECT_ROOT="${PROJECT_ROOT}" \
      bash "${FSV_ROOT}/cases/${case_file}" >"${case_log}" 2>&1; then
    status="PASS"
    PASS=$((PASS + 1))
    note="log: ${case_log}"
  else
    status="FAIL"
    FAIL=$((FAIL + 1))
    note="log: ${case_log}"
  fi
  end="$(date +%s)"
  elapsed=$((end - start))
  printf "| %s | %s | %s | \`%s\` |\n" "${case_name}" "${status}" "${elapsed}" "${note}" >>"${RESULTS_FILE}"
  printf '[%s] %s (%ss)\n' "${status}" "${case_name}" "${elapsed}" | tee -a "${MASTER_LOG}"
done

{
  printf '\n## Boundary & Edge Case Audit\n\n'
  printf '| Edge Case | Case | Source-of-truth proof |\n'
  printf '|-----------|------|-----------------------|\n'
  printf "| Invalid/stale SHA edit is rejected and file remains unchanged | C2 | \`%s/cases/C2_stale_sha_fails/state.log\` |\n" "${FSV_RUN_ROOT}"
  printf "| Denied path access fails without mutating \`.env\` | C3 | \`%s/cases/C3_denied_path_fails/state.log\` |\n" "${FSV_RUN_ROOT}"
  printf "| Maximum-size artifact returns a pointer rather than inline data | F3 | \`%s/cases/F3_large_artifact_pointer/source-of-truth.log\` |\n" "${FSV_RUN_ROOT}"
  printf '\n## Evidence Logs\n\n'
  printf -- "- Case logs: \`%s\`\n" "${CASE_LOG_DIR}"
  printf -- "- MCP stderr: \`%s\`\n" "${MCP_STDERR}"
  printf -- "- Per-case source-of-truth logs: \`%s/cases/<case>/source-of-truth.log\`\n" "${FSV_RUN_ROOT}"
  printf -- "- Per-case before/after state logs: \`%s/cases/<case>/state.log\`\n" "${FSV_RUN_ROOT}"
  printf '\n## Summary\n\n'
  printf 'PASS: %s, FAIL: %s\n' "${PASS}" "${FAIL}"
} >>"${RESULTS_FILE}"

printf '\nResults written to %s\n' "${RESULTS_FILE}"
printf 'Run root: %s\n' "${FSV_RUN_ROOT}"
printf 'Total: %s, PASS: %s, FAIL: %s\n' "$((PASS + FAIL))" "${PASS}" "${FAIL}"

[ "${FAIL}" -eq 0 ]
