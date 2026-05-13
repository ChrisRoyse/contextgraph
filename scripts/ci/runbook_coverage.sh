#!/usr/bin/env bash
# TASK-RUN-012: Operator runbook alert-coverage CI lint.
#
# For every `MEJEPA_*` error / alert class documented under §5 of
# `docs/futurebuild/19_OPERATOR_RUNBOOK.md`, this script grep-asserts that
# the same identifier appears at least once in the Rust source tree
# (`crates/**/*.rs`). It also checks operator-alert emit sites in Rust have a
# runbook §5 heading. Orphan runbook entries and undocumented emitted alerts
# both fail the lint so the next operator never finds a live alert without a
# playbook.
#
# Per CLAUDE.md §3.6 "no backwards compatibility": when an alert is
# renamed the runbook entry must be renamed in the same PR; this lint
# enforces that. Exit non-zero on any orphan.

set -euo pipefail

REPO_ROOT="${MEJEPA_RUNBOOK_COVERAGE_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
RUNBOOK="${MEJEPA_RUNBOOK_COVERAGE_RUNBOOK:-${REPO_ROOT}/docs/futurebuild/19_OPERATOR_RUNBOOK.md}"
CRATES_ROOT="${MEJEPA_RUNBOOK_COVERAGE_CRATES_ROOT:-${REPO_ROOT}/crates}"

if [[ ! -f "${RUNBOOK}" ]]; then
  echo "MEJEPA_RUNBOOK_COVERAGE_RUNBOOK_MISSING: ${RUNBOOK} not found" >&2
  exit 1
fi

# Extract identifiers from §5 alert subsections. The runbook convention is
# `### 5.N \`MEJEPA_*\`` so we grep for that exact prefix.
mapfile -t alert_codes < <(
  awk '
    /^## /        { in_section=0 }
    /^## .*§5|^## 5\b|^## 5\./ { in_section=1; next }
    in_section && /^### / {
      while (match($0, /MEJEPA_[A-Z][A-Z0-9_]+/)) {
        print substr($0, RSTART, RLENGTH)
        $0 = substr($0, RSTART + RLENGTH)
      }
    }
  ' "${RUNBOOK}" | sort -u
)

# Fallback: if §5 is not findable by header, accept any `### N.N MEJEPA_*`
# heading. This keeps the lint resilient to small refactors of the runbook.
if [[ ${#alert_codes[@]} -eq 0 ]]; then
  mapfile -t alert_codes < <(
    grep -oE '### [0-9]+\.[0-9]+ `MEJEPA_[A-Z][A-Z0-9_]+`' "${RUNBOOK}" \
      | grep -oE 'MEJEPA_[A-Z][A-Z0-9_]+' \
      | sort -u
  )
fi

if [[ ${#alert_codes[@]} -eq 0 ]]; then
  echo "MEJEPA_RUNBOOK_COVERAGE_NO_ALERTS: no MEJEPA_* alert classes in ${RUNBOOK} §5" >&2
  exit 1
fi

mapfile -t actionability_failures < <(
  awk '
    function finish_entry() {
      if (code == "") {
        return
      }
      if (!investigate_has_shell) {
        print code "|MEJEPA_RUNBOOK_ACTIONABILITY_MISSING_INVESTIGATE"
      }
      if (!resolution_has_shell) {
        print code "|MEJEPA_RUNBOOK_ACTIONABILITY_MISSING_RESOLUTION"
      }
    }

    function reset_entry() {
      code = ""
      block = ""
      investigate_has_shell = 0
      resolution_has_shell = 0
      in_shell_fence = 0
      shell_block_has_command = 0
      shell_block_target = ""
    }

    function close_shell_fence() {
      if (in_shell_fence && shell_block_has_command) {
        if (shell_block_target == "investigate") {
          investigate_has_shell = 1
        } else if (shell_block_target == "resolution") {
          resolution_has_shell = 1
        }
      }
      in_shell_fence = 0
      shell_block_has_command = 0
      shell_block_target = ""
    }

    BEGIN {
      reset_entry()
      in_section = 0
    }

    /^## / {
      if (in_section) {
        close_shell_fence()
        finish_entry()
      }
      in_section = 0
      reset_entry()
    }

    /^## .*§5|^## 5\b|^## 5\./ {
      in_section = 1
      next
    }

    !in_section {
      next
    }

    /^### / {
      close_shell_fence()
      finish_entry()
      reset_entry()
      if (match($0, /MEJEPA_[A-Z][A-Z0-9_]+/)) {
        code = substr($0, RSTART, RLENGTH)
      }
      next
    }

    code == "" {
      next
    }

    in_shell_fence && /^```/ {
      close_shell_fence()
      next
    }

    in_shell_fence {
      if ($0 ~ /[^[:space:]]/ && $0 !~ /^[[:space:]]*#/) {
        shell_block_has_command = 1
      }
      next
    }

    /^\*\*Investigate:\*\*/ {
      block = "investigate"
      next
    }

    /^\*\*Resolution:\*\*/ {
      block = "resolution"
      next
    }

    /^\*\*[^*]+:\*\*/ {
      block = ""
      next
    }

    /^```(bash|sh|shell)[[:space:]]*$/ {
      if (block == "investigate" || block == "resolution") {
        in_shell_fence = 1
        shell_block_target = block
        shell_block_has_command = 0
      }
      next
    }

    END {
      if (in_section) {
        close_shell_fence()
        finish_entry()
      }
    }
  ' "${RUNBOOK}" | sort -u
)

if [[ ${#actionability_failures[@]} -gt 0 ]]; then
  for failure in "${actionability_failures[@]}"; do
    code="${failure%%|*}"
    error_code="${failure#*|}"
    echo "${error_code}: ${code}" >&2
  done
  exit 1
fi

orphans=()
for code in "${alert_codes[@]}"; do
  if ! grep -RnE --include="*.rs" "\b${code}\b" "${CRATES_ROOT}" >/dev/null 2>&1; then
    orphans+=("${code}")
  fi
done

if [[ ${#orphans[@]} -gt 0 ]]; then
  echo "MEJEPA_RUNBOOK_COVERAGE_ORPHAN_ALERTS: documented but never emitted:" >&2
  for code in "${orphans[@]}"; do
    echo "  - ${code}" >&2
  done
  exit 1
fi

mapfile -t emitted_alert_codes < <(
  {
    grep -RhoE --include="*.rs" 'error_code[[:space:]]*=[[:space:]]*"MEJEPA_[A-Z][A-Z0-9_]+' "${CRATES_ROOT}" || true
    grep -RhoE --include="*.rs" 'code[[:space:]]*=[[:space:]]*"MEJEPA_[A-Z][A-Z0-9_]+' "${CRATES_ROOT}" || true
    grep -RhoE --include="*.rs" 'summary[[:space:]]*=[[:space:]]*format!\("MEJEPA_[A-Z][A-Z0-9_]+' "${CRATES_ROOT}" || true
  } \
    | sed -E 's/.*"(MEJEPA_[A-Z][A-Z0-9_]+).*/\1/' \
    | grep -E '(ALERT|FLAPPING|UNAVAILABLE|BROKEN|DETECTED|EXCEEDED|UNRECOVERABLE)$' \
    | sort -u
)

missing_runbook=()
for code in "${emitted_alert_codes[@]}"; do
  found=0
  for documented in "${alert_codes[@]}"; do
    if [[ "${documented}" == "${code}" ]]; then
      found=1
      break
    fi
  done
  if [[ "${found}" -eq 0 ]]; then
    missing_runbook+=("${code}")
  fi
done

if [[ ${#missing_runbook[@]} -gt 0 ]]; then
  echo "MEJEPA_RUNBOOK_COVERAGE_UNDOCUMENTED_ALERTS: emitted but missing from runbook §5:" >&2
  for code in "${missing_runbook[@]}"; do
    echo "  - ${code}" >&2
  done
  exit 1
fi

echo "runbook_coverage_ok: ${#alert_codes[@]} alert classes verified against ${CRATES_ROOT}/**/*.rs; ${#emitted_alert_codes[@]} emitted operator alerts documented"
