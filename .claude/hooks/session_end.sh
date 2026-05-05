#!/bin/bash
# Claude Code Hook: SessionEnd
# Timeout: 30000ms (30 seconds for full persistence)
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error, 4=invalid_input
#
# Input from Claude Code: {"session_id":"...", "reason":"...", "stats":{...}}

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

# Parse input
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default-session"')
DURATION_MS=$(echo "$INPUT" | jq -r '.stats.duration_ms // "0"')

# Execute CLI with 30s timeout
timeout 30s "$CONTEXT_GRAPH_CLI" hooks session-end \
    --session-id "$SESSION_ID" \
    --duration-ms "$DURATION_MS" \
    --generate-summary true \
    --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 30000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
