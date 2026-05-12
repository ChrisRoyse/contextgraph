#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT}"

bash -n scripts/cc/*.sh
python3 -m py_compile scripts/cc/*.py

if rg -n --pcre2 'curl\s+[^|;\n]*\|\s*(bash|sh)|wget\s+[^|;\n]*\|\s*(bash|sh)|eval\s+"\$\{|eval\s+\$[A-Za-z_]' scripts/cc; then
  printf 'CGREALITY_HOOK_SCRIPT_LINT_FAILED: hook contains shell-injection prone pattern.\n' >&2
  exit 1
fi

printf 'CGREALITY_HOOK_SCRIPT_LINT_OK\n'
