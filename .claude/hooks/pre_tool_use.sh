#!/bin/bash
# Claude Code Hook: PreToolUse
# Wrapper Timeout: 500ms (CLI internal: <100ms, process overhead: ~200-400ms)
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 4=invalid_input
#
# CRITICAL: CLI logic must complete in <100ms. No database operations allowed.
# The 500ms wrapper timeout accounts for bash/process startup overhead.
# Input from Claude Code: {"tool_name":"...", "tool_input":{...}}

set -euo pipefail

INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

# Validate JSON input
if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi

# Find CLI binary
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi

if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null && [ ! -x "$CONTEXT_GRAPH_CLI" ]; then
    echo '{"success":false,"error":"CLI binary not found","exit_code":1}' >&2
    exit 1
fi

# Parse input - extract session_id and tool info
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default-session"')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // "unknown"')

# Execute CLI - FAST PATH (no DB)
# Note: The 100ms budget is for the overall hook. Process startup adds overhead.
# We allow 500ms for the shell wrapper (100ms CLI + 400ms process overhead).
# The CLI itself MUST complete in <100ms - no database access allowed.
timeout 0.5s "$CONTEXT_GRAPH_CLI" hooks pre-tool \
    --session-id "$SESSION_ID" \
    --tool-name "$TOOL_NAME" \
    --fast-path true \
    --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 500ms - process startup exceeded budget","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
