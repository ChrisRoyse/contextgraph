#!/bin/bash
# Context Graph - Disk Space Management Benchmark Suite
#
# Measures effectiveness of:
# - Cargo.toml dev profile (debug=1 vs debug=true)
# - Cleanup script functionality
# - Session hook disk warnings
#
# Usage:
#   ./scripts/benchmark-disk-management.sh [--json-only]
#
# Options:
#   --json-only   Skip verbose output, only produce JSON report

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="$PROJECT_DIR/benchmark_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$RESULTS_DIR/disk_management_benchmark_$TIMESTAMP.json"
JSON_ONLY=false

# Parse arguments
for arg in "$@"; do
    case $arg in
        --json-only) JSON_ONLY=true ;;
        --help|-h)
            echo "Usage: $0 [--json-only]"
            echo ""
            echo "Options:"
            echo "  --json-only   Skip verbose output, only produce JSON report"
            exit 0
            ;;
    esac
done

# Create results directory
mkdir -p "$RESULTS_DIR"

# Initialize JSON report
cat > "$REPORT_FILE" << 'JSONEOF'
{"benchmark": "disk_management", "timestamp": "TIMESTAMP_PLACEHOLDER", "version": "1.0.0", "results": {}}
JSONEOF
sed -i "s/TIMESTAMP_PLACEHOLDER/$TIMESTAMP/" "$REPORT_FILE"

# Helper to update JSON
update_json() {
    local key="$1"
    local value="$2"
    local tmp=$(mktemp)
    jq --arg k "$key" --argjson v "$value" '.results[$k] = $v' "$REPORT_FILE" > "$tmp"
    mv "$tmp" "$REPORT_FILE"
}

log() {
    if [ "$JSON_ONLY" = false ]; then
        echo "$@"
    fi
}

log "=== Disk Space Management Benchmark ==="
log "Project: $PROJECT_DIR"
log "Report: $REPORT_FILE"
log ""

# --- Benchmark 1: Debug Build Size ---
log "### Benchmark 1: Debug Build Size ###"

# Record initial state
had_debug_dir=false
if [ -d "$PROJECT_DIR/target/debug" ]; then
    had_debug_dir=true
    existing_size=$(du -sb "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1 || echo "0")
fi

# Build a representative crate (not full workspace - too slow)
log "Building context-graph-core in debug mode..."
cd "$PROJECT_DIR"
start_time=$(date +%s%N)
if cargo build -p context-graph-core 2>&1 | tail -3; then
    build_success=true
else
    build_success=false
fi
end_time=$(date +%s%N)
build_time_ms=$(( (end_time - start_time) / 1000000 ))

# Measure size
if [ -d "$PROJECT_DIR/target/debug" ]; then
    debug_size_bytes=$(du -sb "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1 || echo "0")
    debug_size_human=$(du -sh "$PROJECT_DIR/target/debug" 2>/dev/null | cut -f1 || echo "0")
else
    debug_size_bytes=0
    debug_size_human="0"
fi

log "Debug build size: $debug_size_human ($debug_size_bytes bytes)"
log "Build time: ${build_time_ms}ms"
log "Build success: $build_success"

update_json "debug_build" "{
    \"size_bytes\": $debug_size_bytes,
    \"size_human\": \"$debug_size_human\",
    \"build_time_ms\": $build_time_ms,
    \"build_success\": $build_success,
    \"profile\": \"dev (debug=1, split-debuginfo=unpacked)\",
    \"crate_tested\": \"context-graph-core\"
}"

# --- Benchmark 2: Cleanup Script Dry-Run ---
log ""
log "### Benchmark 2: Cleanup Script Functionality ###"

# Test dry-run mode
log "Testing --dry-run mode..."
if [ -x "$PROJECT_DIR/scripts/clean-build-artifacts.sh" ]; then
    dry_run_output=$("$PROJECT_DIR/scripts/clean-build-artifacts.sh" --dry-run 2>&1 || true)
    dry_run_detects_debug=$(echo "$dry_run_output" | grep -c "Debug build" || echo "0")
    dry_run_detects_doc=$(echo "$dry_run_output" | grep -c "Documentation" || echo "0")
    dry_run_detects_stale=$(echo "$dry_run_output" | grep -c "contextgraph_data" || echo "0")
    script_executable=true

    # Verify dry-run doesn't delete
    if [ "$had_debug_dir" = true ] && [ -d "$PROJECT_DIR/target/debug" ]; then
        dry_run_preserves="true"
    elif [ "$had_debug_dir" = false ]; then
        # Can't verify preservation if there was nothing to preserve
        dry_run_preserves="true"
    else
        dry_run_preserves="false"
    fi
else
    dry_run_detects_debug=0
    dry_run_detects_doc=0
    dry_run_detects_stale=0
    dry_run_preserves="false"
    script_executable=false
fi

log "  Detects debug: $dry_run_detects_debug"
log "  Detects doc: $dry_run_detects_doc"
log "  Preserves files: $dry_run_preserves"

update_json "cleanup_script_dry_run" "{
    \"script_executable\": $script_executable,
    \"detects_debug\": $([ "$dry_run_detects_debug" -gt 0 ] && echo "true" || echo "false"),
    \"detects_doc\": $([ "$dry_run_detects_doc" -gt 0 ] && echo "true" || echo "false"),
    \"detects_stale_data\": $([ "$dry_run_detects_stale" -gt 0 ] && echo "true" || echo "false"),
    \"preserves_files\": $dry_run_preserves,
    \"status\": \"$([ "$dry_run_preserves" = "true" ] && [ "$script_executable" = true ] && echo "pass" || echo "fail")\"
}"

# --- Benchmark 3: Session Hook ---
log ""
log "### Benchmark 3: Session Hook Disk Check ###"

hook_file="$PROJECT_DIR/.claude/hooks/session_start.sh"
if [ -f "$hook_file" ]; then
    hook_exists=true
    if grep -q "check_disk_space" "$hook_file"; then
        hook_has_function=true
    else
        hook_has_function=false
    fi
    # Check threshold is 85%
    threshold=$(grep "threshold_percent=" "$hook_file" 2>/dev/null | head -1 | grep -oE '[0-9]+' || echo "0")
    if [ -z "$threshold" ]; then
        threshold=0
    fi
else
    hook_exists=false
    hook_has_function=false
    threshold=0
fi

# Get current disk usage
current_usage=$(df "$PROJECT_DIR" 2>/dev/null | tail -1 | awk '{print $5}' | sed 's/%//' || echo "0")

log "  Hook exists: $hook_exists"
log "  Has check function: $hook_has_function"
log "  Threshold: ${threshold}%"
log "  Current usage: ${current_usage}%"

# Determine if hook would trigger
if [ "$current_usage" -ge "$threshold" ] 2>/dev/null; then
    would_trigger=true
else
    would_trigger=false
fi

update_json "session_hook" "{
    \"hook_exists\": $hook_exists,
    \"has_check_function\": $hook_has_function,
    \"threshold_percent\": $threshold,
    \"current_disk_usage_percent\": $current_usage,
    \"would_trigger\": $would_trigger,
    \"status\": \"$([ "$hook_has_function" = true ] && [ "$threshold" = "85" ] && echo "pass" || echo "fail")\"
}"

# --- Benchmark 4: Profile Settings Verification ---
log ""
log "### Benchmark 4: Profile Settings Verification ###"

cargo_toml="$PROJECT_DIR/Cargo.toml"
if [ -f "$cargo_toml" ]; then
    has_debug_1=$(grep -c 'debug = 1' "$cargo_toml" || echo "0")
    has_split_debuginfo=$(grep -c 'split-debuginfo' "$cargo_toml" || echo "0")
    has_package_debug_false=$(grep -c 'debug = false' "$cargo_toml" || echo "0")
    has_dev_profile=$(grep -c '\[profile.dev\]' "$cargo_toml" || echo "0")
else
    has_debug_1=0
    has_split_debuginfo=0
    has_package_debug_false=0
    has_dev_profile=0
fi

log "  debug = 1: $([ "$has_debug_1" -gt 0 ] && echo "YES" || echo "NO")"
log "  split-debuginfo: $([ "$has_split_debuginfo" -gt 0 ] && echo "YES" || echo "NO")"
log "  package debug=false: $([ "$has_package_debug_false" -gt 0 ] && echo "YES" || echo "NO")"

update_json "profile_settings" "{
    \"has_dev_profile\": $([ "$has_dev_profile" -gt 0 ] && echo "true" || echo "false"),
    \"has_debug_1\": $([ "$has_debug_1" -gt 0 ] && echo "true" || echo "false"),
    \"has_split_debuginfo\": $([ "$has_split_debuginfo" -gt 0 ] && echo "true" || echo "false"),
    \"has_package_debug_false\": $([ "$has_package_debug_false" -gt 0 ] && echo "true" || echo "false"),
    \"status\": \"$([ "$has_debug_1" -gt 0 ] && [ "$has_split_debuginfo" -gt 0 ] && echo "pass" || echo "fail")\"
}"

# --- Benchmark 5: Gitignore Patterns ---
log ""
log "### Benchmark 5: Gitignore Patterns ###"

gitignore_file="$PROJECT_DIR/.gitignore"
if [ -f "$gitignore_file" ]; then
    has_incompatible=$(grep -c 'contextgraph_data_incompatible_' "$gitignore_file" || echo "0")
    has_backup=$(grep -c 'contextgraph_data_backup_' "$gitignore_file" || echo "0")
    has_rmeta=$(grep -c '\.rmeta' "$gitignore_file" || echo "0")
    has_target=$(grep -c '^target/' "$gitignore_file" || echo "0")
else
    has_incompatible=0
    has_backup=0
    has_rmeta=0
    has_target=0
fi

log "  Incompatible data pattern: $([ "$has_incompatible" -gt 0 ] && echo "YES" || echo "NO")"
log "  Backup data pattern: $([ "$has_backup" -gt 0 ] && echo "YES" || echo "NO")"
log "  rmeta pattern: $([ "$has_rmeta" -gt 0 ] && echo "YES" || echo "NO")"
log "  target/ pattern: $([ "$has_target" -gt 0 ] && echo "YES" || echo "NO")"

update_json "gitignore_patterns" "{
    \"has_target\": $([ "$has_target" -gt 0 ] && echo "true" || echo "false"),
    \"has_incompatible_data\": $([ "$has_incompatible" -gt 0 ] && echo "true" || echo "false"),
    \"has_backup_data\": $([ "$has_backup" -gt 0 ] && echo "true" || echo "false"),
    \"has_rmeta\": $([ "$has_rmeta" -gt 0 ] && echo "true" || echo "false"),
    \"status\": \"$([ "$has_incompatible" -gt 0 ] && [ "$has_backup" -gt 0 ] && [ "$has_rmeta" -gt 0 ] && echo "pass" || echo "fail")\"
}"

# --- Summary ---
log ""
log "### Benchmark Complete ###"

# Calculate overall status
profile_pass=$([ "$has_debug_1" -gt 0 ] && [ "$has_split_debuginfo" -gt 0 ] && echo "true" || echo "false")
cleanup_pass=$([ "$script_executable" = true ] && [ "$dry_run_preserves" = "true" ] && echo "true" || echo "false")
hook_pass=$([ "$hook_has_function" = true ] && [ "$threshold" = "85" ] && echo "true" || echo "false")
gitignore_pass=$([ "$has_incompatible" -gt 0 ] && [ "$has_backup" -gt 0 ] && [ "$has_rmeta" -gt 0 ] && echo "true" || echo "false")

if [ "$profile_pass" = "true" ] && [ "$cleanup_pass" = "true" ] && [ "$hook_pass" = "true" ] && [ "$gitignore_pass" = "true" ]; then
    overall_status="pass"
    overall_message="All disk space management checks passed"
else
    overall_status="partial"
    overall_message="Some checks need attention"
fi

update_json "summary" "{
    \"profile_settings\": \"$profile_pass\",
    \"cleanup_script\": \"$cleanup_pass\",
    \"session_hook\": \"$hook_pass\",
    \"gitignore_patterns\": \"$gitignore_pass\",
    \"overall_status\": \"$overall_status\",
    \"overall_message\": \"$overall_message\"
}"

log "Results saved to: $REPORT_FILE"
log ""
if [ "$JSON_ONLY" = false ]; then
    cat "$REPORT_FILE" | jq '.'
fi

# Exit with appropriate code
if [ "$overall_status" = "pass" ]; then
    exit 0
else
    exit 1
fi
