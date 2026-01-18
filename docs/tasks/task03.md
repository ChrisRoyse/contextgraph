# Task 03: Create Stop Hook Shell Script

## Metadata
- **Task ID**: TASK-GAP-002
- **Phase**: 1 (Foundation)
- **Priority**: Critical
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: None

## Objective

Create the Stop hook shell script as required by PRD Section 9.1. This hook captures Claude's response when a response completes and stores it as a `ClaudeResponse` memory with all 13 embeddings. The shell script follows constitution rule AP-53: "Hook logic MUST be in shell scripts calling context-graph-cli".

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/.claude/hooks/session_end.sh` - Reference for existing hook pattern
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 4.2 for stop hook specification

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/.claude/hooks/stop.sh`

## Implementation Steps

### Step 1: Create the stop.sh script

Create the file at `/home/cabdru/contextgraph/.claude/hooks/stop.sh` with the implementation below.

### Step 2: Make the script executable

```bash
chmod +x /home/cabdru/contextgraph/.claude/hooks/stop.sh
```

### Step 3: Verify the script follows the existing hook pattern

Compare with `session_end.sh` to ensure consistent:
- Error handling with `set -euo pipefail`
- JSON input validation with jq
- CLI binary resolution
- Exit code standards

## Code/Content to Implement

### /home/cabdru/contextgraph/.claude/hooks/stop.sh

```bash
#!/bin/bash
# Claude Code Hook: Stop
# Timeout: 3000ms
#
# Constitution: AP-50, AP-53, ARCH-11
# Exit Codes: 0=success, 1=cli_not_found, 2=timeout, 3=db_error, 4=invalid_input
#
# Input from Claude Code: {"response_text":"...", "session_id":"..."}
# Stores response as ClaudeResponse memory with all 13 embeddings
#
# Per PRD Section 9.1: Stop hook captures Claude's response when a response completes.
# Per ARCH-11: Memory sources include ClaudeResponse.

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

# Parse input JSON
RESPONSE_TEXT=$(echo "$INPUT" | jq -r '.response_text // empty')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
TIMESTAMP_MS=$(date +%s%3N)

# Skip if response is empty
if [ -z "$RESPONSE_TEXT" ]; then
    echo '{"success":true,"skipped":true,"reason":"Empty response"}'
    exit 0
fi

# Truncate response if > 10000 chars (per schema constraint)
RESPONSE_TEXT=$(echo "$RESPONSE_TEXT" | head -c 10000)

# Build HookInput JSON for capture-response command
HOOK_INPUT=$(cat <<EOF
{
    "hook_type": "stop",
    "session_id": "$SESSION_ID",
    "timestamp_ms": $TIMESTAMP_MS,
    "payload": {
        "type": "stop",
        "data": {
            "response_text": $(echo "$RESPONSE_TEXT" | jq -Rs .),
            "source": "claude_response"
        }
    }
}
EOF
)

# Execute CLI with 3s timeout
# Captures response, embeds with all 13 embedders, stores as ClaudeResponse
echo "$HOOK_INPUT" | timeout 3s "$CONTEXT_GRAPH_CLI" hooks capture-response --stdin --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 3000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
```

## Definition of Done

- [ ] Script exists at `/home/cabdru/contextgraph/.claude/hooks/stop.sh`
- [ ] Script is executable (`-x` permission)
- [ ] Script has proper shebang (`#!/bin/bash`)
- [ ] Script uses `set -euo pipefail` for error handling
- [ ] Script validates JSON input using jq
- [ ] Script handles empty response gracefully (exits 0 with skipped=true)
- [ ] Script truncates responses > 10000 characters
- [ ] Script resolves CLI binary from multiple candidate paths
- [ ] Script exits with appropriate codes:
  - 0 = success
  - 1 = cli_not_found
  - 2 = timeout
  - 4 = invalid_input
- [ ] Script completes within 3000ms timeout requirement

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify script exists and is executable
test -x .claude/hooks/stop.sh && echo "Script is executable"

# Verify script syntax
bash -n .claude/hooks/stop.sh && echo "Syntax OK"

# Test with valid input (will fail on CLI not found, but validates JSON handling)
echo '{"response_text":"Test response","session_id":"test-123"}' | .claude/hooks/stop.sh
echo "Exit code: $?"

# Test with empty response (should exit 0 with skipped=true)
echo '{"response_text":"","session_id":"test-123"}' | .claude/hooks/stop.sh
echo "Exit code: $?"

# Test with invalid JSON (should exit 4)
echo 'not json' | .claude/hooks/stop.sh 2>&1
echo "Exit code: $?"

# Test with empty stdin (should exit 4)
echo '' | .claude/hooks/stop.sh 2>&1
echo "Exit code: $?"

# Verify hook follows existing patterns
diff -u <(head -15 .claude/hooks/session_end.sh) <(head -15 .claude/hooks/stop.sh)
# Headers should be similar in structure
```
