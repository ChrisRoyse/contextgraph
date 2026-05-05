#!/bin/bash
# Claude Code Hook: UserPromptSubmit
# Timeout: 2000ms
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error (CLI passthrough), 4=invalid_input
#
# Input from Claude Code: {"prompt":"...", "session_id":"..."}
# Uses stdin approach to avoid shell injection with user-controlled prompt text

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

# Parse input - extract needed fields
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default-session"')
TIMESTAMP_MS=$(date +%s%3N)

# Extract prompt safely via jq (handles special chars, newlines)
PROMPT=$(echo "$INPUT" | jq -r '.prompt // ""')

# Build HookInput JSON - using jq to safely embed the prompt
HOOK_INPUT=$(jq -n \
    --arg hook_type "user_prompt_submit" \
    --arg session_id "$SESSION_ID" \
    --argjson timestamp_ms "$TIMESTAMP_MS" \
    --arg prompt "$PROMPT" \
    '{
        hook_type: $hook_type,
        session_id: $session_id,
        timestamp_ms: $timestamp_ms,
        payload: {
            type: "user_prompt_submit",
            data: {
                prompt: $prompt,
                context: []
            }
        }
    }')

# Execute CLI with 2s timeout using stdin (avoids shell injection).
# `set -euo pipefail` would otherwise exit immediately on GNU timeout's 124
# before we can translate it into the project timeout JSON.
# Note: CLI exit codes pass through (including exit code 3 for db_error)
# Note: --session-id is required even when using --stdin
set +e
echo "$HOOK_INPUT" | timeout 2s "$CONTEXT_GRAPH_CLI" hooks prompt-submit \
    --session-id "$SESSION_ID" \
    --stdin true \
    --format json
exit_code=$?
set -e

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 2000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
