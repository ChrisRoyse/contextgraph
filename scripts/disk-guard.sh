#!/bin/bash
# Context Graph - Disk Space Guard
#
# Lightweight check to run before builds. Warns at 80%, blocks at 95%.
# Add to your workflow:
#   ./scripts/disk-guard.sh && cargo build --release
#
# Or source it in .bashrc for automatic checking:
#   alias cargo='~/contextgraph/scripts/disk-guard.sh silent; command cargo'

set -euo pipefail

SILENT=false
[ "${1:-}" = "silent" ] && SILENT=true

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WARN_THRESHOLD=80
BLOCK_THRESHOLD=95

usage_pct=$(df "$PROJECT_DIR" 2>/dev/null | tail -1 | awk '{print $5}' | sed 's/%//')

if [ -z "$usage_pct" ]; then
    exit 0  # Can't determine usage, don't block
fi

if [ "$usage_pct" -ge "$BLOCK_THRESHOLD" ] 2>/dev/null; then
    echo "" >&2
    echo "DISK GUARD: BLOCKED — disk at ${usage_pct}% (>= ${BLOCK_THRESHOLD}%)" >&2
    echo "Run: ./scripts/clean-build-artifacts.sh" >&2
    echo "Quick fix: rm -rf target/debug" >&2
    echo "" >&2
    exit 1
fi

if [ "$usage_pct" -ge "$WARN_THRESHOLD" ] 2>/dev/null && [ "$SILENT" = false ]; then
    echo "" >&2
    echo "DISK GUARD: disk at ${usage_pct}% — consider running ./scripts/clean-build-artifacts.sh" >&2

    # Quick breakdown of top consumers
    if [ -d "$PROJECT_DIR/target/debug" ]; then
        debug_size=$(du -sh "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1)
        echo "  target/debug: $debug_size (safe to delete: rm -rf target/debug)" >&2
    fi
    echo "" >&2
fi

exit 0
