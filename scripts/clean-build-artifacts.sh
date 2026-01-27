#!/bin/bash
# Context Graph - Build Artifact Cleanup Script
#
# Usage:
#   ./scripts/clean-build-artifacts.sh [--dry-run] [--aggressive]
#
# Options:
#   --dry-run     Show what would be deleted without deleting
#   --aggressive  Also clean release incremental and deps

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$PROJECT_DIR/target"
DRY_RUN=false
AGGRESSIVE=false

# Parse arguments
for arg in "$@"; do
    case $arg in
        --dry-run) DRY_RUN=true ;;
        --aggressive) AGGRESSIVE=true ;;
        --help|-h)
            echo "Usage: $0 [--dry-run] [--aggressive]"
            echo ""
            echo "Options:"
            echo "  --dry-run     Show what would be deleted without deleting"
            echo "  --aggressive  Also clean release incremental and deps"
            exit 0
            ;;
    esac
done

echo "=== Context Graph Build Cleanup ==="
echo "Project: $PROJECT_DIR"
echo "Dry run: $DRY_RUN"
echo ""

# Calculate sizes before
if [ -d "$TARGET_DIR" ]; then
    before_size=$(du -sh "$TARGET_DIR" 2>/dev/null | cut -f1 || echo "0")
    echo "Current target/ size: $before_size"
else
    echo "No target/ directory found"
    exit 0
fi
echo ""

# Function to delete with size reporting
clean_dir() {
    local path="$1"
    local desc="$2"

    if [ -d "$path" ]; then
        local size=$(du -sh "$path" 2>/dev/null | cut -f1)
        echo "  $desc: $size"
        if [ "$DRY_RUN" = false ]; then
            rm -rf "$path"
            echo "    -> Deleted"
        else
            echo "    -> Would delete"
        fi
    fi
}

echo "Cleaning debug artifacts..."
clean_dir "$TARGET_DIR/debug" "Debug build"

echo ""
echo "Cleaning incremental compilation..."
if [ -d "$TARGET_DIR/debug/incremental" ]; then
    clean_dir "$TARGET_DIR/debug/incremental" "Debug incremental"
fi
if [ "$AGGRESSIVE" = true ]; then
    clean_dir "$TARGET_DIR/release/incremental" "Release incremental"
fi

echo ""
echo "Cleaning doc artifacts..."
clean_dir "$TARGET_DIR/doc" "Documentation"

echo ""
echo "Cleaning old benchmark results..."
if [ -d "$TARGET_DIR/criterion" ]; then
    local_size=$(du -sh "$TARGET_DIR/criterion" 2>/dev/null | cut -f1)
    echo "  Criterion benchmarks: $local_size"
    if [ "$DRY_RUN" = false ]; then
        # Keep benchmark data but could be cleaned if needed
        echo "    -> Kept (use --aggressive to remove)"
    fi
fi

if [ "$AGGRESSIVE" = true ]; then
    echo ""
    echo "Aggressive mode: cleaning deps and build caches..."
    clean_dir "$TARGET_DIR/release/deps" "Release deps"
    clean_dir "$TARGET_DIR/release/build" "Release build scripts"
    clean_dir "$TARGET_DIR/criterion" "Criterion benchmarks"
fi

# Clean stale migration data in project root
echo ""
echo "Cleaning stale data folders..."
for stale_dir in "$PROJECT_DIR"/contextgraph_data_incompatible_* "$PROJECT_DIR"/contextgraph_data_backup_*; do
    if [ -d "$stale_dir" ]; then
        clean_dir "$stale_dir" "$(basename "$stale_dir")"
    fi
done

# Final size
if [ "$DRY_RUN" = false ] && [ -d "$TARGET_DIR" ]; then
    echo ""
    after_size=$(du -sh "$TARGET_DIR" 2>/dev/null | cut -f1 || echo "0")
    echo "Final target/ size: $after_size (was $before_size)"
fi

echo ""
echo "Cleanup complete!"
