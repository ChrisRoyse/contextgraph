#!/bin/bash
# Context Graph - Trim Stale Build Deps
#
# Keeps only the newest version of each crate's compiled artifact in target/*/deps/.
# Run after builds to prevent unbounded accumulation of 600MB+ stale binaries.
#
# Usage:
#   ./scripts/trim-stale-deps.sh              # Trim both debug and release
#   ./scripts/trim-stale-deps.sh --dry-run    # Preview only
#   ./scripts/trim-stale-deps.sh release      # Trim release only
#   ./scripts/trim-stale-deps.sh debug        # Trim debug only
#
# Designed to run automatically via: cargo build && ./scripts/trim-stale-deps.sh

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DRY_RUN=false
PROFILES=("debug" "release")

for arg in "$@"; do
    case $arg in
        --dry-run) DRY_RUN=true ;;
        debug) PROFILES=("debug") ;;
        release) PROFILES=("release") ;;
    esac
done

# Our workspace crate prefixes (large artifacts worth trimming)
CRATE_PREFIXES=(
    "context_graph_mcp"
    "context_graph_cli"
    "libcontext_graph_benchmark"
    "libcontext_graph_core"
    "libcontext_graph_embeddings"
    "libcontext_graph_storage"
    "libcontext_graph_mcp"
    "libcontext_graph_graph"
    "libcontext_graph_causal_agent"
    "libcontext_graph_cuda"
    "libcontext_graph_graph_agent"
    "libcontext_graph_test_utils"
    "libllama_cpp_sys_2"
)

total_deleted=0
total_freed_kb=0

for profile in "${PROFILES[@]}"; do
    deps_dir="$PROJECT_DIR/target/$profile/deps"
    [ -d "$deps_dir" ] || continue

    for prefix in "${CRATE_PREFIXES[@]}"; do
        # Find all non-.d files for this prefix
        mapfile -t files < <(find "$deps_dir" -maxdepth 1 -name "${prefix}-*" ! -name "*.d" -type f 2>/dev/null)
        count=${#files[@]}

        if [ "$count" -le 1 ]; then
            continue  # Nothing to trim
        fi

        # Keep the newest file (by mtime).
        # find -printf avoids ARG_MAX; sort -n | tail -1 avoids SIGPIPE (reads all input).
        newest=$(find "$deps_dir" -maxdepth 1 -name "${prefix}-*" ! -name "*.d" -type f -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2-)

        for f in "${files[@]}"; do
            [ -f "$f" ] || continue
            if [ "$f" != "$newest" ]; then
                kb=$(du -sk "$f" 2>/dev/null | cut -f1) || kb=0
                kb=${kb:-0}
                total_freed_kb=$(( total_freed_kb + kb ))
                total_deleted=$(( total_deleted + 1 ))

                # Also count the .d file
                d_file="${f}.d"
                if [ -f "$d_file" ]; then
                    d_kb=$(du -sk "$d_file" 2>/dev/null | cut -f1) || d_kb=0
                    d_kb=${d_kb:-0}
                    total_freed_kb=$(( total_freed_kb + d_kb ))
                    total_deleted=$(( total_deleted + 1 ))
                fi

                if [ "$DRY_RUN" = false ]; then
                    rm -f "$f" "${f}.d"
                fi
            fi
        done
    done
done

if [ "$total_deleted" -gt 0 ]; then
    if [ "$total_freed_kb" -ge 1048576 ]; then
        freed_human="$(( total_freed_kb / 1048576 ))GB"
    elif [ "$total_freed_kb" -ge 1024 ]; then
        freed_human="$(( total_freed_kb / 1024 ))MB"
    else
        freed_human="${total_freed_kb}KB"
    fi

    if [ "$DRY_RUN" = true ]; then
        echo "trim-stale-deps: would delete $total_deleted stale artifacts ($freed_human)"
    else
        echo "trim-stale-deps: deleted $total_deleted stale artifacts ($freed_human)"
    fi
fi
