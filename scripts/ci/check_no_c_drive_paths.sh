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

raw_matches="$(
  {
    rg -n --hidden --glob '!target/**' --glob '!**/__pycache__/**' --glob '!**/*.pyc' \
      -- "${slash_c}" "${scan_roots[@]}" || true
    rg -n --hidden --glob '!target/**' --glob '!**/__pycache__/**' --glob '!**/*.pyc' \
      --pcre2 "${drive_regex}" "${scan_roots[@]}" || true
  } | sed \
      -e '/scripts\/ci\/check_no_c_drive_paths.sh/d' \
      -e '/crates\/context-graph-paths\/src\/lib.rs/d'
)"

allow_c_drive_guard_reference() {
  local row="$1"
  case "${row}" in
    *'df_capacity("/mnt/c")?'*) return 0 ;;
    *'!hot_root.starts_with("/mnt/c")'* ) return 0 ;;
    *'!paths.packed_root().starts_with("/mnt/c")'* ) return 0 ;;
    *'not Windows mounted /mnt/c'* ) return 0 ;;
    *'df -B1 --output=source,size,used,avail,pcent,target / /mnt/c /mnt/d'* ) return 0 ;;
    'crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs:'*':            "/mnt/c",') return 0 ;;
    *) return 1 ;;
  esac
}

matches=""
while IFS= read -r row; do
  if [ -z "${row}" ]; then
    continue
  fi
  if allow_c_drive_guard_reference "${row}"; then
    continue
  fi
  matches+="${row}"$'\n'
done <<< "${raw_matches}"
matches="${matches%$'\n'}"

if [ -n "${matches}" ]; then
  printf 'CGREALITY_C_DRIVE_PATH_LINT_FAILED: C-drive paths are not allowed in code, hooks, config, or CI.\n' >&2
  printf '%s\n' "${matches}" >&2
  exit 1
fi

printf 'CGREALITY_C_DRIVE_PATH_LINT_OK\n'
