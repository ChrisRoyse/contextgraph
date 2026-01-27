#!/bin/bash
# Context Graph - Build Size Comparison Benchmark
#
# Compares debug build sizes with different profile settings:
# - New profile: debug=1, split-debuginfo=unpacked
# - Old profile: debug=true (full DWARF)
#
# WARNING: This modifies Cargo.toml temporarily and does clean builds
#
# Usage:
#   ./scripts/benchmark-build-size-comparison.sh [--crate NAME] [--skip-restore]
#
# Options:
#   --crate NAME    Crate to build (default: context-graph-core)
#   --skip-restore  Don't restore Cargo.toml on completion (for debugging)

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_TOML="$PROJECT_DIR/Cargo.toml"
CARGO_TOML_BACKUP="$PROJECT_DIR/Cargo.toml.benchmark_backup"
TEST_CRATE="context-graph-core"
SKIP_RESTORE=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --crate)
            TEST_CRATE="$2"
            shift 2
            ;;
        --skip-restore)
            SKIP_RESTORE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--crate NAME] [--skip-restore]"
            echo ""
            echo "Options:"
            echo "  --crate NAME    Crate to build (default: context-graph-core)"
            echo "  --skip-restore  Don't restore Cargo.toml on completion"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "=== Build Size Comparison Benchmark ==="
echo "Project: $PROJECT_DIR"
echo "Crate: $TEST_CRATE"
echo ""
echo "WARNING: This will modify Cargo.toml and do clean builds"
echo "Press Ctrl+C within 3 seconds to abort..."
sleep 3
echo ""

# Backup Cargo.toml
cp "$CARGO_TOML" "$CARGO_TOML_BACKUP"
echo "Backed up Cargo.toml to $CARGO_TOML_BACKUP"

cleanup() {
    if [ "$SKIP_RESTORE" = false ]; then
        echo ""
        echo "Restoring Cargo.toml..."
        if [ -f "$CARGO_TOML_BACKUP" ]; then
            mv "$CARGO_TOML_BACKUP" "$CARGO_TOML"
            echo "Restored successfully"
        fi
    else
        echo ""
        echo "Skipping restore (--skip-restore specified)"
        echo "Backup available at: $CARGO_TOML_BACKUP"
    fi
}
trap cleanup EXIT

cd "$PROJECT_DIR"

# --- Test 1: New Profile (current settings) ---
echo "### Test 1: New Profile (debug=1, split-debuginfo) ###"

# Clean debug directory
rm -rf "$PROJECT_DIR/target/debug"

# Build
echo "Building $TEST_CRATE..."
start_time=$(date +%s%N)
if cargo build -p "$TEST_CRATE" 2>&1 | tail -3; then
    new_build_success=true
else
    new_build_success=false
fi
end_time=$(date +%s%N)
new_build_time_ms=$(( (end_time - start_time) / 1000000 ))

# Measure size
if [ -d "$PROJECT_DIR/target/debug" ]; then
    new_size=$(du -sb "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1)
    new_size_human=$(du -sh "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1)
else
    new_size=0
    new_size_human="0"
fi

echo "Size: $new_size_human ($new_size bytes)"
echo "Build time: ${new_build_time_ms}ms"
echo ""

# --- Test 2: Old Profile (debug=true) ---
echo "### Test 2: Old Profile (debug=true, no split-debuginfo) ###"

# Modify Cargo.toml to use old profile
# Replace debug = 1 with debug = true
sed -i 's/debug = 1/debug = true/' "$CARGO_TOML"
# Remove split-debuginfo line
sed -i '/split-debuginfo/d' "$CARGO_TOML"
# Remove package override section
sed -i '/\[profile.dev.package."\*"\]/,/debug = false/d' "$CARGO_TOML"

echo "Modified Cargo.toml to use old profile settings"

# Clean debug directory
rm -rf "$PROJECT_DIR/target/debug"

# Build
echo "Building $TEST_CRATE..."
start_time=$(date +%s%N)
if cargo build -p "$TEST_CRATE" 2>&1 | tail -3; then
    old_build_success=true
else
    old_build_success=false
fi
end_time=$(date +%s%N)
old_build_time_ms=$(( (end_time - start_time) / 1000000 ))

# Measure size
if [ -d "$PROJECT_DIR/target/debug" ]; then
    old_size=$(du -sb "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1)
    old_size_human=$(du -sh "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1)
else
    old_size=0
    old_size_human="0"
fi

echo "Size: $old_size_human ($old_size bytes)"
echo "Build time: ${old_build_time_ms}ms"
echo ""

# --- Calculate Results ---
echo "### Results ###"
echo ""

if [ "$old_size" -gt 0 ] && [ "$new_size" -gt 0 ]; then
    # Calculate size reduction percentage
    size_diff=$((old_size - new_size))
    reduction_percent=$(( 100 - (new_size * 100 / old_size) ))

    # Calculate time difference
    time_diff=$((new_build_time_ms - old_build_time_ms))
    if [ "$old_build_time_ms" -gt 0 ]; then
        time_change_percent=$(( (time_diff * 100) / old_build_time_ms ))
    else
        time_change_percent=0
    fi

    echo "Old profile (debug=true):          $old_size_human ($old_size bytes)"
    echo "New profile (debug=1):             $new_size_human ($new_size bytes)"
    echo "Size reduction:                    ${reduction_percent}% (${size_diff} bytes saved)"
    echo ""
    echo "Old profile build time:            ${old_build_time_ms}ms"
    echo "New profile build time:            ${new_build_time_ms}ms"
    if [ "$time_diff" -lt 0 ]; then
        echo "Build time improvement:            $((-time_diff))ms faster"
    elif [ "$time_diff" -gt 0 ]; then
        echo "Build time overhead:               ${time_diff}ms slower"
    else
        echo "Build time change:                 No change"
    fi
    echo ""

    # Evaluation
    if [ "$reduction_percent" -ge 40 ]; then
        echo "SUCCESS: Achieved ${reduction_percent}% size reduction (target: 40-60%)"
        exit_code=0
    elif [ "$reduction_percent" -ge 20 ]; then
        echo "PARTIAL: ${reduction_percent}% size reduction (below 40% target)"
        echo "         Note: CUDA builds typically show larger reductions"
        exit_code=0
    else
        echo "MINIMAL: Only ${reduction_percent}% size reduction"
        echo "         Profile changes may not be effective for this crate"
        exit_code=1
    fi
else
    echo "ERROR: Could not measure build sizes"
    echo "  Old size: $old_size"
    echo "  New size: $new_size"
    exit_code=1
fi

echo ""
echo "### Build Status ###"
echo "New profile build: $([ "$new_build_success" = true ] && echo "SUCCESS" || echo "FAILED")"
echo "Old profile build: $([ "$old_build_success" = true ] && echo "SUCCESS" || echo "FAILED")"

# Clean up debug directory to save space
echo ""
echo "Cleaning up debug directory..."
rm -rf "$PROJECT_DIR/target/debug"

exit $exit_code
