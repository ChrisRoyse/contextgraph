# TASK-SESSION-16: Create .claude/settings.json Hook Configuration

```xml
<task_spec id="TASK-SESSION-16" version="1.0">
<metadata>
  <title>Create .claude/settings.json Hook Configuration</title>
  <status>pending</status>
  <layer>surface</layer>
  <sequence>16</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-16</requirement_ref>
    <constitution_ref>ARCH-07</constitution_ref>
    <constitution_ref>AP-50</constitution_ref>
    <constitution_ref>AP-53</constitution_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-11</task_ref>
    <task_ref>TASK-SESSION-12</task_ref>
    <task_ref>TASK-SESSION-13</task_ref>
    <task_ref>TASK-SESSION-14</task_ref>
    <task_ref>TASK-SESSION-15</task_ref>
  </depends_on>
  <estimated_hours>1.0</estimated_hours>
</metadata>
```

## Objective

Configure all 5 hooks in .claude/settings.json per ARCH-07 (native Claude Code hooks) and AP-50 (no internal/built-in hooks). **Direct CLI commands** - no shell script intermediaries.

## Context

Claude Code supports hooks that execute commands at specific lifecycle points. We configure these to integrate Context Graph's consciousness system with Claude Code's workflow.

## Hook Configuration

| Hook | Command | Timeout | Matcher |
|------|---------|---------|---------|
| SessionStart | `context-graph-cli session restore-identity` | 5000ms | None |
| PreToolUse | `context-graph-cli consciousness brief` | 100ms | `mcp__context-graph__*\|Edit\|Write` |
| PostToolUse | `context-graph-cli consciousness check-identity --auto-dream` | 3000ms | `mcp__context-graph__*\|Edit\|Write` |
| UserPromptSubmit | `context-graph-cli consciousness inject-context` | 2000ms | None |
| SessionEnd | `context-graph-cli session persist-identity` | 30000ms | None |

## Matcher Rationale

| Pattern | Tools Matched | Rationale |
|---------|--------------|-----------|
| `mcp__context-graph__*` | All context-graph MCP tools | Core memory operations |
| `Edit` | File edits | Modifies state |
| `Write` | File writes | Modifies state |
| (excluded) `Read` | File reads | Read-only, no IC impact |
| (excluded) `Bash` | Shell commands | Too noisy |

## Implementation Steps

1. Create or update .claude/settings.json
2. Add SessionStart hook with restore-identity command
3. Add PreToolUse hook with brief command and matcher
4. Add PostToolUse hook with check-identity command and matcher
5. Add UserPromptSubmit hook with inject-context command
6. Add SessionEnd hook with persist-identity command
7. Set appropriate timeouts per PRD Section 15.4
8. Use direct commands (not shell scripts per AP-53)

## Input Context Files

```xml
<input_context_files>
  <file purpose="existing_settings">.claude/settings.json</file>
  <file purpose="claude_hook_docs">Claude Code documentation on hooks</file>
</input_context_files>
```

## Files to Create/Modify

| File | Change |
|------|--------|
| `.claude/settings.json` | Create or update hook configuration |

## JSON Configuration

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session restore-identity",
            "timeout": 5000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "mcp__context-graph__*|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness brief",
            "timeout": 100
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "mcp__context-graph__*|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness check-identity --auto-dream",
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
            "command": "context-graph-cli consciousness inject-context",
            "timeout": 2000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session persist-identity",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

## Constitution Compliance

| Requirement | Verification |
|-------------|-------------|
| ARCH-07 | Native Claude Code hooks via .claude/settings.json |
| AP-50 | No internal/built-in hooks |
| AP-53 | Direct CLI commands (no shell scripts) |

## Definition of Done

### Acceptance Criteria

- [ ] All 5 hooks configured in .claude/settings.json
- [ ] SessionStart: restore-identity with 5000ms timeout
- [ ] PreToolUse: brief with 100ms timeout, matcher pattern excludes Read/Bash
- [ ] PostToolUse: check-identity --auto-dream with 3000ms timeout, same matcher
- [ ] UserPromptSubmit: inject-context with 2000ms timeout
- [ ] SessionEnd: persist-identity with 30000ms timeout
- [ ] Direct CLI commands (no shell scripts per AP-53)
- [ ] ARCH-07 compliant (native Claude Code hooks)
- [ ] AP-50 compliant (no internal/built-in hooks)
- [ ] JSON is valid and parseable
- [ ] Claude Code recognizes and executes all hooks

### Constraints

- Valid JSON syntax
- Direct commands only (no shell wrappers)
- Timeouts within Claude Code limits
- Matchers use correct syntax

### Verification Commands

```bash
# Validate JSON
python3 -c "import json; json.load(open('.claude/settings.json'))"

# Check file exists
ls -la .claude/settings.json

# Verify structure
cat .claude/settings.json | jq '.hooks | keys'
```

## Test Cases

### Manual Testing Protocol

1. Start new Claude Code session
2. Verify SessionStart hook fires (check for consciousness summary output)
3. Execute Edit command
4. Verify PreToolUse hook fires (check for brief output)
5. Verify PostToolUse hook fires (check for IC check)
6. Submit user prompt
7. Verify UserPromptSubmit hook fires (check for context injection)
8. End session
9. Verify SessionEnd hook fires (check for persistence)

### TC-SESSION-CONFIG-01: JSON Validity
```bash
#!/bin/bash
# Verify JSON is valid
if python3 -c "import json; json.load(open('.claude/settings.json'))"; then
    echo "PASS: JSON is valid"
else
    echo "FAIL: Invalid JSON"
    exit 1
fi
```

### TC-SESSION-CONFIG-02: Required Keys
```bash
#!/bin/bash
# Verify all required hooks present
REQUIRED_HOOKS="SessionStart PreToolUse PostToolUse UserPromptSubmit SessionEnd"
for hook in $REQUIRED_HOOKS; do
    if cat .claude/settings.json | jq -e ".hooks.$hook" > /dev/null 2>&1; then
        echo "PASS: $hook hook configured"
    else
        echo "FAIL: $hook hook missing"
        exit 1
    fi
done
```

## Exit Conditions

- **Success**: Claude Code recognizes and executes all hooks
- **Failure**: Invalid JSON, hooks not invoked - error out with detailed logging

## Next Task

After completion, proceed to **017-TASK-SESSION-17** (Exit Code Mapping).

```xml
</task_spec>
```
