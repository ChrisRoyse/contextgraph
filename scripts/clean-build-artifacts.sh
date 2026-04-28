#!/bin/bash
# Context Graph - Build Artifact & Disk Space Management
#
# Usage:
#   ./scripts/clean-build-artifacts.sh              # Standard cleanup (debug + backups)
#   ./scripts/clean-build-artifacts.sh --dry-run    # Preview what would be deleted
#   ./scripts/clean-build-artifacts.sh --aggressive  # Also clean release deps, HF cache, pip
#   ./scripts/clean-build-artifacts.sh --check       # Just report disk usage, no deletion
#
# Categories cleaned:
#   1. target/debug/  (incremental cache, stale test binaries)
#   2. target temp dirs and browser/session artifacts
#   3. Python bytecode/cache files
#   4. Backup files   (corrupted DBs, model backups, stale data)
#   5. /tmp test DBs  (leftover RocksDB/CUDA FSV directories)
#   6. Old /tmp runtime logs and heartbeat sidecars
#   7. [aggressive] Release deps, HF model cache, pip cache
#
# This script is safe to run at any time. Release binaries are preserved
# unless --aggressive is used.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$PROJECT_DIR/target"
DRY_RUN=false
AGGRESSIVE=false
CHECK_ONLY=false
TOTAL_FREED=0

# Parse arguments
for arg in "$@"; do
    case $arg in
        --dry-run) DRY_RUN=true ;;
        --aggressive) AGGRESSIVE=true ;;
        --check) CHECK_ONLY=true ;;
        --help|-h)
            echo "Usage: $0 [--dry-run] [--aggressive] [--check]"
            echo ""
            echo "Options:"
            echo "  --dry-run     Show what would be deleted without deleting"
            echo "  --aggressive  Also clean release deps, HF cache, pip cache"
            echo "  --check       Just report disk usage, no deletion"
            exit 0
            ;;
    esac
done

# =============================================================================
# Helpers
# =============================================================================

# Get size in KB for arithmetic
size_kb() {
    if [ -e "$1" ]; then
        du -sk "$1" 2>/dev/null | cut -f1
    else
        echo 0
    fi
}

# Human-readable size
human_size() {
    local kb=$1
    if [ "$kb" -ge 1048576 ]; then
        echo "$(( kb / 1048576 ))GB"
    elif [ "$kb" -ge 1024 ]; then
        echo "$(( kb / 1024 ))MB"
    else
        echo "${kb}KB"
    fi
}

# Delete directory/file with size reporting
clean_path() {
    local path="$1"
    local desc="$2"

    if [ -e "$path" ]; then
        local kb
        kb=$(size_kb "$path")
        local size
        size=$(human_size "$kb")
        if [ "$CHECK_ONLY" = true ]; then
            printf "  %-50s %8s\n" "$desc" "$size"
        elif [ "$DRY_RUN" = true ]; then
            printf "  %-50s %8s  [would delete]\n" "$desc" "$size"
        else
            rm -rf "$path"
            printf "  %-50s %8s  [deleted]\n" "$desc" "$size"
        fi
        TOTAL_FREED=$(( TOTAL_FREED + kb ))
    fi
}

# =============================================================================
# Report
# =============================================================================

echo "=== Context Graph Disk Space Management ==="
echo "Project: $PROJECT_DIR"
echo "Mode: $([ "$CHECK_ONLY" = true ] && echo "CHECK" || ([ "$DRY_RUN" = true ] && echo "DRY RUN" || echo "CLEANUP"))"
if [ "$AGGRESSIVE" = true ]; then echo "Level: AGGRESSIVE"; fi
echo ""

# Disk status
df_line=$(df -h "$PROJECT_DIR" 2>/dev/null | tail -1)
disk_pct=$(echo "$df_line" | awk '{print $5}' | sed 's/%//')
echo "Disk: $df_line"
if [ "$disk_pct" -ge 90 ] 2>/dev/null; then
    echo "WARNING: Disk usage at ${disk_pct}% — cleanup strongly recommended"
elif [ "$disk_pct" -ge 80 ] 2>/dev/null; then
    echo "NOTICE: Disk usage at ${disk_pct}% — consider cleanup"
fi
echo ""

# =============================================================================
# Category 1: Debug Build Artifacts
# =============================================================================

echo "--- Debug Build Artifacts ---"
if [ -d "$TARGET_DIR/debug" ]; then
    clean_path "$TARGET_DIR/debug" "target/debug/ (incremental + deps + binaries)"
else
    echo "  target/debug/ not present"
fi
echo ""

# =============================================================================
# Category 2: Target Temp + Browser Session Artifacts
# =============================================================================

echo "--- Target Temp / Browser Session Artifacts ---"
clean_path "$TARGET_DIR/tmp" "target/tmp/"
clean_path "$TARGET_DIR/utl_fsv" "target/utl_fsv/"
clean_path "$PROJECT_DIR/.playwright-mcp" ".playwright-mcp/ session artifacts"
clean_path "$PROJECT_DIR/docs/shakespeareplan/runs" "docs/shakespeareplan/runs/ generated FSV runs"
clean_path "$PROJECT_DIR/tmp" "tmp/ scratch artifacts"
echo ""

# =============================================================================
# Category 3: Python Caches
# =============================================================================

echo "--- Python Caches ---"
pycache_count=0
pycache_kb=0
while IFS= read -r cache_path; do
    [ -e "$cache_path" ] || continue
    kb=$(size_kb "$cache_path")
    pycache_kb=$(( pycache_kb + kb ))
    pycache_count=$(( pycache_count + 1 ))
    if [ "$CHECK_ONLY" = false ] && [ "$DRY_RUN" = false ]; then
        rm -rf "$cache_path"
    fi
done < <(
    find "$PROJECT_DIR" \
        \( -path "$PROJECT_DIR/.git" \
        -o -path "$PROJECT_DIR/target" \
        -o -path "$PROJECT_DIR/models" \
        -o -path "$PROJECT_DIR/data" \
        -o -path "$PROJECT_DIR/shakespear" \) -prune \
        -o \( -type d \( -name __pycache__ -o -name .pytest_cache \) -prune -print \) \
        -o \( -type f -name '*.pyc' -print \)
)
if [ "$pycache_count" -gt 0 ]; then
    pycache_size=$(human_size "$pycache_kb")
    if [ "$CHECK_ONLY" = true ]; then
        printf "  %-50s %8s\n" "Python caches ($pycache_count paths)" "$pycache_size"
    elif [ "$DRY_RUN" = true ]; then
        printf "  %-50s %8s  [would delete]\n" "Python caches ($pycache_count paths)" "$pycache_size"
    else
        printf "  %-50s %8s  [deleted]\n" "Python caches ($pycache_count paths)" "$pycache_size"
    fi
    TOTAL_FREED=$(( TOTAL_FREED + pycache_kb ))
else
    echo "  No Python caches found"
fi
echo ""

# =============================================================================
# Category 4: Stale Release Artifacts (only incremental + old deps)
# =============================================================================

echo "--- Release Stale Artifacts ---"
if [ -d "$TARGET_DIR/release/incremental" ]; then
    clean_path "$TARGET_DIR/release/incremental" "target/release/incremental/"
fi

# Stale databases inside release dir
for stale in "$TARGET_DIR"/release/contextgraph_data* ; do
    [ -e "$stale" ] && clean_path "$stale" "$(basename "$stale") (stale DB in target/)"
done

if [ "$AGGRESSIVE" = true ]; then
    if [ -d "$TARGET_DIR/release/deps" ]; then
        clean_path "$TARGET_DIR/release/deps" "target/release/deps/ (rebuilds on next compile)"
    fi
    if [ -d "$TARGET_DIR/release/build" ]; then
        clean_path "$TARGET_DIR/release/build" "target/release/build/ (CUDA kernels rebuild)"
    fi
fi
echo ""

# =============================================================================
# Category 5: Backup and Stale Data
# =============================================================================

echo "--- Backup / Stale Data ---"
# Corrupted/incompatible database backups
for stale_dir in "$PROJECT_DIR"/contextgraph_data_corrupted_* "$PROJECT_DIR"/contextgraph_data_incompatible_* "$PROJECT_DIR"/contextgraph_data_backup_*; do
    [ -e "$stale_dir" ] && clean_path "$stale_dir" "$(basename "$stale_dir")"
done

# Model snapshots/caches are intentionally preserved. They are large by design
# and should stay ready for warm starts and offline use.
if [ -d "$PROJECT_DIR/models" ]; then
    printf "  %-50s %8s  [preserved]\n" "models/ weights and snapshots" "$(du -sh "$PROJECT_DIR/models" 2>/dev/null | cut -f1 || echo unknown)"
fi

# Stale benchmark data
for stale_data in "$PROJECT_DIR"/data/hf_benchmark_arxiv_only_backup "$PROJECT_DIR"/data/hf_benchmark/temp_wikipedia; do
    [ -e "$stale_data" ] && clean_path "$stale_data" "data/$(echo "$stale_data" | sed "s|$PROJECT_DIR/data/||")"
done
echo ""

# =============================================================================
# Category 6: /tmp Test Remnants
# =============================================================================

echo "--- /tmp Test Remnants ---"
tmp_count=0
tmp_kb=0
for tmp_dir in /tmp/db-* /tmp/ocr-processor-test-* /tmp/contextgraph-cuda13-fsv.* /tmp/contextgraph-cuda13-fsv.partial; do
    if [ -e "$tmp_dir" ]; then
        kb=$(size_kb "$tmp_dir")
        tmp_kb=$(( tmp_kb + kb ))
        tmp_count=$(( tmp_count + 1 ))
        if [ "$CHECK_ONLY" = false ] && [ "$DRY_RUN" = false ]; then
            rm -rf "$tmp_dir"
        fi
    fi
done
if [ "$tmp_count" -gt 0 ]; then
    local_size=$(human_size "$tmp_kb")
    if [ "$CHECK_ONLY" = true ]; then
        printf "  %-50s %8s\n" "/tmp test databases ($tmp_count dirs)" "$local_size"
    elif [ "$DRY_RUN" = true ]; then
        printf "  %-50s %8s  [would delete]\n" "/tmp test databases ($tmp_count dirs)" "$local_size"
    else
        printf "  %-50s %8s  [deleted]\n" "/tmp test databases ($tmp_count dirs)" "$local_size"
    fi
    TOTAL_FREED=$(( TOTAL_FREED + tmp_kb ))
else
    echo "  No test remnants in /tmp"
fi
echo ""

# =============================================================================
# Category 7: Old /tmp Runtime Logs
# =============================================================================

echo "--- Old /tmp Runtime Logs ---"
log_count=0
log_kb=0
while IFS= read -r log_path; do
    [ -e "$log_path" ] || continue
    kb=$(size_kb "$log_path")
    log_kb=$(( log_kb + kb ))
    log_count=$(( log_count + 1 ))
    if [ "$CHECK_ONLY" = false ] && [ "$DRY_RUN" = false ]; then
        rm -f "$log_path"
    fi
done < <(
    find /tmp -maxdepth 1 -type f \
        \( -name 'vram_watchdog.log*' \
        -o -name 'context-graph-daemon.log*' \
        -o -name 'cg-daemon.log*' \
        -o -name 'shake_*heartbeat*.jsonl*' \
        -o -name 'shake_*_mem.jsonl*' \
        -o -name 'shake_dpo_rejected_pairs.log*' \) \
        -mtime +7 -print 2>/dev/null
)
if [ "$log_count" -gt 0 ]; then
    log_size=$(human_size "$log_kb")
    if [ "$CHECK_ONLY" = true ]; then
        printf "  %-50s %8s\n" "Old /tmp logs ($log_count files, >7 days)" "$log_size"
    elif [ "$DRY_RUN" = true ]; then
        printf "  %-50s %8s  [would delete]\n" "Old /tmp logs ($log_count files, >7 days)" "$log_size"
    else
        printf "  %-50s %8s  [deleted]\n" "Old /tmp logs ($log_count files, >7 days)" "$log_size"
    fi
    TOTAL_FREED=$(( TOTAL_FREED + log_kb ))
else
    echo "  No old /tmp runtime logs found"
fi
echo ""

# =============================================================================
# Category 8: Documentation artifacts
# =============================================================================

if [ -d "$TARGET_DIR/doc" ]; then
    echo "--- Documentation ---"
    clean_path "$TARGET_DIR/doc" "target/doc/"
    echo ""
fi

# =============================================================================
# Category 9 (Aggressive): External Caches
# =============================================================================

if [ "$AGGRESSIVE" = true ]; then
    echo "--- External Caches (aggressive) ---"

    # pip cache
    if command -v pip &>/dev/null; then
        pip_size=$(pip cache info 2>/dev/null | grep -oP 'Cache size: \K[^\s]+' || echo "unknown")
        if [ "$CHECK_ONLY" = true ] || [ "$DRY_RUN" = true ]; then
            printf "  %-50s %8s\n" "pip cache" "$pip_size"
        else
            pip cache purge 2>/dev/null
            printf "  %-50s %8s  [purged]\n" "pip cache" "$pip_size"
        fi
    fi

    # HuggingFace model cache is intentionally preserved so models stay warm
    # and available offline. Do not delete model cache from this cleanup script.
    HF_HUB="$HOME/.cache/huggingface/hub"
    if [ -d "$HF_HUB" ]; then
        hf_kb=$(size_kb "$HF_HUB")
        printf "  %-50s %8s  [preserved]\n" "HuggingFace model cache" "$(human_size "$hf_kb")"
    fi

    # Cargo registry (old crate versions)
    if command -v cargo &>/dev/null; then
        if [ -d "$HOME/.cargo/registry/cache" ]; then
            reg_kb=$(size_kb "$HOME/.cargo/registry/cache")
            reg_size=$(human_size "$reg_kb")
            printf "  %-50s %8s  [run 'cargo cache -a' to trim]\n" "Cargo registry cache" "$reg_size"
        fi
    fi

    echo ""
fi

# =============================================================================
# Summary
# =============================================================================

echo "==========================================="
total_human=$(human_size "$TOTAL_FREED")
if [ "$CHECK_ONLY" = true ]; then
    echo "Total cleanable: $total_human"
elif [ "$DRY_RUN" = true ]; then
    echo "Total would free: $total_human"
else
    echo "Total freed: $total_human"
fi

# Post-cleanup disk status
if [ "$CHECK_ONLY" = false ] && [ "$DRY_RUN" = false ]; then
    echo ""
    df -h "$PROJECT_DIR" | tail -1
fi

echo ""
echo "Done."
