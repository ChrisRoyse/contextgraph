#!/bin/bash
# Claude Code Hook: SessionStart
# Timeout: 5000ms
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error (CLI passthrough), 4=invalid_input
#
# Input from Claude Code: {"session_id":"...", "timestamp":"..."}
# Transforms to HookInput format expected by CLI

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

# Parse input JSON using jq
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
PREVIOUS_SESSION_ID=$(echo "$INPUT" | jq -r '.previous_session_id // empty')
TIMESTAMP_MS=$(date +%s%3N)
CWD=$(pwd)

# Generate session ID if not provided (better than bare $RANDOM)
if [ -z "$SESSION_ID" ]; then
    SESSION_ID="session-$(uuidgen 2>/dev/null || echo "$(date +%s)-$$-$RANDOM")"
fi

# Build HookInput JSON with snake_case and tagged enum format
# Note: payload uses tag="type" content="data" format per serde config
if [ -n "$PREVIOUS_SESSION_ID" ]; then
    HOOK_INPUT=$(cat <<EOF
{
    "hook_type": "session_start",
    "session_id": "$SESSION_ID",
    "timestamp_ms": $TIMESTAMP_MS,
    "payload": {
        "type": "session_start",
        "data": {
            "cwd": "$CWD",
            "source": "cli",
            "previous_session_id": "$PREVIOUS_SESSION_ID"
        }
    }
}
EOF
)
else
    HOOK_INPUT=$(cat <<EOF
{
    "hook_type": "session_start",
    "session_id": "$SESSION_ID",
    "timestamp_ms": $TIMESTAMP_MS,
    "payload": {
        "type": "session_start",
        "data": {
            "cwd": "$CWD",
            "source": "cli"
        }
    }
}
EOF
)
fi

# Execute CLI with 5s timeout
# Note: CLI exit codes pass through (including exit code 3 for db_error)
echo "$HOOK_INPUT" | timeout 5s "$CONTEXT_GRAPH_CLI" hooks session-start --stdin --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 5000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
