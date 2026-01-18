# Task 14: Update settings.json with Stop Hook

## Metadata
- **Task ID**: TASK-GAP-014
- **Phase**: 4 (Integration)
- **Priority**: High
- **Complexity**: Low
- **Estimated Time**: 15 minutes
- **Dependencies**: task03 (TASK-GAP-002 - stop.sh must exist)

## Objective

Update the `.claude/settings.json` file to include the Stop hook configuration. This hook captures Claude's response when a response completes, storing it as a `ClaudeResponse` memory with all 13 embeddings per PRD Section 9.1.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/.claude/settings.json` - Current hook configuration
- `/home/cabdru/contextgraph/.claude/hooks/stop.sh` - Stop hook script (created in task03)
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 4.1 for settings configuration

## Files to Create/Modify

**Files to Modify:**
- `/home/cabdru/contextgraph/.claude/settings.json`

## Implementation Steps

### Step 1: Read current settings.json

Review the current structure to understand where to add the Stop hook.

### Step 2: Add Stop hook configuration

Add the Stop hook after the UserPromptSubmit section, following the same pattern as existing hooks.

### Step 3: Validate JSON syntax

Ensure the file is valid JSON after modification.

## Code/Content to Implement

### /home/cabdru/contextgraph/.claude/settings.json

Update the file to include the Stop hook:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/session_start.sh",
            "timeout": 5000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/session_end.sh",
            "timeout": 30000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/pre_tool_use.sh",
            "timeout": 100
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/post_tool_use.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/user_prompt_submit.sh",
            "timeout": 2000
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/stop.sh",
            "timeout": 3000
          }
        ]
      }
    ]
  }
}
```

## Definition of Done

- [ ] `settings.json` is valid JSON (no syntax errors)
- [ ] Stop hook is configured with correct path: `.claude/hooks/stop.sh`
- [ ] Stop hook timeout is 3000ms (per PRD Section 9.1)
- [ ] Stop hook uses "command" type (per AP-53)
- [ ] All 6 hooks are configured (SessionStart, SessionEnd, PreToolUse, PostToolUse, UserPromptSubmit, Stop)

## Verification

```bash
cd /home/cabdru/contextgraph

# Validate JSON syntax
python3 -m json.tool .claude/settings.json > /dev/null && echo "JSON is valid"

# Alternative validation with jq
jq empty .claude/settings.json && echo "JSON is valid"

# Check Stop hook exists
grep -A5 '"Stop"' .claude/settings.json
# Should show Stop hook configuration

# Verify timeout is 3000
jq '.hooks.Stop[0].hooks[0].timeout' .claude/settings.json
# Should output: 3000

# Verify command path
jq -r '.hooks.Stop[0].hooks[0].command' .claude/settings.json
# Should output: .claude/hooks/stop.sh

# Count total hooks configured
jq '.hooks | keys | length' .claude/settings.json
# Should output: 6

# List all hook names
jq -r '.hooks | keys[]' .claude/settings.json
# Should show: SessionStart, SessionEnd, PreToolUse, PostToolUse, UserPromptSubmit, Stop
```
