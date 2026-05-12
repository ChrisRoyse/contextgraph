#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT}"

scan_roots=(
  ".claude/settings.json"
  ".github"
  "config"
  "crates"
  "scripts/cc"
  "scripts/ci"
  "scripts/5090jepa/local_ci_gate.sh"
)

mnt_slash="mnt"
slash_c="/${mnt_slash}/c"
drive_regex='[Cc]:\\[^nrt0]'

matches="$(
  {
    rg -n --hidden --glob '!target/**' --glob '!**/__pycache__/**' --glob '!**/*.pyc' \
      -- "${slash_c}" "${scan_roots[@]}" || true
    rg -n --hidden --glob '!target/**' --glob '!**/__pycache__/**' --glob '!**/*.pyc' \
      --pcre2 "${drive_regex}" "${scan_roots[@]}" || true
  } | sed \
      -e '/scripts\/ci\/check_no_c_drive_paths.sh/d' \
      -e '/crates\/context-graph-paths\/src\/lib.rs/d'
)"

if [ -n "${matches}" ]; then
  printf 'CGREALITY_C_DRIVE_PATH_LINT_FAILED: C-drive paths are not allowed in code, hooks, config, or CI.\n' >&2
  printf '%s\n' "${matches}" >&2
  exit 1
fi

printf 'CGREALITY_C_DRIVE_PATH_LINT_OK\n'
