#!/usr/bin/env bash
# Fail CI if any source file reintroduces `/mnt/d` or related identifiers.
#
# The legacy `/mnt/d/contextgraph/...` data root was fully retired 2026-05-20.
# Production durable state lives on aiwonder under /zfs/archive/contextgraph/...;
# hot work under /zfs/hot/contextgraph/... . This lint exists only to catch
# accidental reintroduction.
set -euo pipefail

cd "$(dirname "$0")/../.."

PATTERN='/mnt/d|mnt_d|MntD|MNT_D|mntd'

# Restrict to tracked source / docs / scripts / configs; skip git internals,
# build artifacts, and this script itself.
matches=$(
  grep -rIln -E "$PATTERN" \
    --exclude-dir=.git \
    --exclude-dir=target \
    --exclude-dir=node_modules \
    --exclude='no_mnt_d_grep.sh' \
    . 2>/dev/null || true
)

if [ -n "$matches" ]; then
  echo "NO_MNT_D_GREP_FAILED: legacy /mnt/d references reintroduced in:" >&2
  echo "$matches" >&2
  echo "" >&2
  echo "Production durable state lives on aiwonder under /zfs/archive/contextgraph/...;" >&2
  echo "hot work under /zfs/hot/contextgraph/... . Do not reintroduce /mnt/d paths." >&2
  exit 1
fi

echo "no_mnt_d_grep: OK (no /mnt/d references)"
