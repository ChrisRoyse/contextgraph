# Claude Code Hooks Reference

**⚠️ CONTEXT GRAPH USES NATIVE CLAUDE CODE HOOKS EXCLUSIVELY**

This project integrates with Claude Code via **NATIVE hooks** configured in `.claude/settings.json` — NOT internal/built-in hooks or custom middleware. Shell script executors call `context-graph-cli` commands. This reduces complexity by ~71% vs. building custom hook infrastructure.

---

Shell commands or LLM prompts that execute at lifecycle events for deterministic automation.

## Characteristics

| Feature | Value |
|---------|-------|
| Execution | Shell commands or LLM prompts |
| Input | JSON via stdin |
| Output | JSON via stdout + exit codes |
| Timeout | 60s default (configurable) |
| Parallelization | Multiple matching hooks run in parallel |

## Events Matrix

| Event | Blocking | Matchers | Version | When |
|-------|----------|----------|---------|------|
| PreToolUse | ✓ (exit 2) | ✓ | Original | Before tool executes |
| PostToolUse | - | ✓ | Original | After tool completes |
| PermissionRequest | ✓ | ✓ | 2.0.45 | Permission dialog about to show |
| UserPromptSubmit | ✓ (exit 2) | - | Original | User submits prompt |
| Stop | ✓ | - | Original | Claude finishes responding |
| SubagentStop | ✓ | - | 1.0.41 | Subagent finishes |
| SessionStart | - | - | Original | Session begins/resumes |
| SessionEnd | - | - | 1.0.85 | Session terminates |
| PreCompact | - | ✓ (`auto`/`manual`) | Original | Before context compaction |
| Notification | - | - | Original | Notification sent |

## Capabilities Matrix

| Hook | Block | Approve | Modify | Log | Feedback |
|------|-------|---------|--------|-----|----------|
| PreToolUse | ✓ | ✓ | ✓ | ✓ | - |
| PostToolUse | - | - | - | ✓ | ✓ |
| PermissionRequest | ✓ | ✓ | ✓ | - | - |
| UserPromptSubmit | ✓ | - | - | ✓ | - |
| Stop | ✓* | - | - | ✓ | - |
| SubagentStop | ✓* | - | - | ✓ | - |
| SessionStart | - | - | - | ✓ | context |
| SessionEnd | - | - | - | ✓ | - |
| PreCompact | - | - | - | ✓ | - |
| Notification | - | - | - | ✓ | - |

*Force continuation with `decision: "block"` + `continue: true`

## Decision Options

| Decision | Effect | Applicable Hooks |
|----------|--------|------------------|
| `allow` | Bypass dialog, execute | PreToolUse, PermissionRequest |
| `deny` | Block, show reason | PreToolUse, PermissionRequest |
| `ask` | Show dialog (default) | PreToolUse, PermissionRequest |
| `block` | Block with reason | PostToolUse, Stop, SubagentStop |
| `approve` | Allow stop | Stop, SubagentStop |

## Input Schema

Base fields (all hooks):
```
{session_id, transcript_path, cwd, permission_mode, hook_event_name}
```

| Event | Additional Fields | Types |
|-------|-------------------|-------|
| PreToolUse | tool_name, tool_input, tool_use_id? | str, Record, str? |
| PostToolUse | tool_name, tool_input, tool_response, tool_use_id? | str, Record, Record, str? |
| PermissionRequest | tool_name, tool_input | str, Record |
| UserPromptSubmit | prompt | str |
| Stop | stop_hook_active | bool |
| SubagentStop | stop_hook_active | bool |
| SessionStart | source (`startup`/`resume`/`clear`) | str |
| SessionEnd | reason (`exit`/`clear`/`logout`/`prompt_input_exit`/`other`) | str |
| PreCompact | trigger (`auto`/`manual`), custom_instructions | str, str |
| Notification | message | str |

## Output Schema

Base: `{continue?, stopReason?, suppressOutput?, systemMessage?}`

**PreToolUse**:
```json
{"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "allow|deny|ask", "permissionDecisionReason?": "...", "updatedInput?": {...}}}
```

**PermissionRequest**:
```json
{"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow|deny", "message?": "...", "updatedInput?": {...}}}}
```

**Stop/SubagentStop**:
```json
{"decision": "approve|block", "reason?": "...", "continue": true|false}
```

## Configuration

| Location | Scope | Priority |
|----------|-------|----------|
| `~/.claude/settings.json` | User (all projects) | Lowest |
| `.claude/settings.json` | Project (shared) | Medium |
| `.claude/settings.local.json` | Project (personal) | Highest |

**Structure**:
```json
{
  "hooks": {
    "EventName": [{
      "matcher": "ToolPattern",
      "hooks": [{"type": "command|prompt", "command": "...", "timeout": 30000}]
    }]
  }
}
```

## Matchers

| Pattern | Matches |
|---------|---------|
| `"Write"` | Write tool only |
| `"Write\|Edit"` | Write OR Edit |
| `"*"` or `""` | All tools |
| `"Bash(npm test*)"` | Bash with specific args |
| `"mcp__github__.*"` | MCP tool regex |

Matchers are **case-sensitive**. Only applicable to PreToolUse, PostToolUse, PermissionRequest.

## Exit Codes

| Code | Meaning | Behavior |
|------|---------|----------|
| 0 | Success | stdout processed, action continues |
| 2 | Blocking error | stderr fed to Claude, action prevented |
| Other | Non-blocking error | stderr shown to user, action continues |

## Environment Variables

| Variable | Scope | Description |
|----------|-------|-------------|
| `CLAUDE_PROJECT_DIR` | All | Absolute path to project root |
| `CLAUDE_CODE_REMOTE` | All | `"true"` if web environment |
| `CLAUDE_ENV_FILE` | SessionStart | Path for persistent env vars |
| `CLAUDE_FILE_PATHS` | PostToolUse | File paths for formatters |

**Note**: `CLAUDE_TOOL_INPUT_*` variables may be empty; prefer parsing JSON from stdin.

## Command vs Prompt Hooks

| Aspect | Command | Prompt |
|--------|---------|--------|
| Execution | Shell command | LLM (Haiku) |
| Supported | All events | Stop, SubagentStop only |
| Use for | Deterministic, fast | Judgment calls |
| Variable | - | `$ARGUMENTS` → hook input |

## Tool Input Reference

**File Operations**:
- Read: `{file_path, offset?, limit?}`
- Write: `{file_path, content}`
- Edit: `{file_path, old_string, new_string, replace_all?}`
- Glob: `{pattern, path?}`
- Grep: `{pattern, path?, output_mode?, glob?, type?, -i?, -n?, -A?, -B?, -C?, multiline?, head_limit?}`

**Execution**:
- Bash: `{command, description?, timeout?, run_in_background?}`

**Task**:
- Task: `{description, prompt, subagent_type, model?, run_in_background?, resume?}`
- TodoWrite: `{todos: [{content, status, activeForm}]}`

## Essential Patterns

### Block Dangerous Commands
```python
#!/usr/bin/env python3
import json, re, sys
DANGEROUS = [r"rm\s+-rf\s+/", r"sudo\s+", r"chmod\s+777"]
payload = json.loads(sys.stdin.read())
cmd = payload.get("tool_input", {}).get("command", "")
for p in DANGEROUS:
    if re.search(p, cmd):
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": f"Blocked: {p}"}}))
        sys.exit(0)
print("{}")
```

### Auto-Format After Edits
```json
{"hooks": {"PostToolUse": [{"matcher": "Write|Edit", "hooks": [{"type": "command", "command": "prettier --write \"$CLAUDE_FILE_PATHS\" 2>/dev/null || true"}]}]}}
```

### Auto-Approve Tests
```json
{"hooks": {"PermissionRequest": [{"matcher": "Bash(npm test*)|Bash(pytest*)", "hooks": [{"type": "command", "command": "echo '{\"hookSpecificOutput\":{\"hookEventName\":\"PermissionRequest\",\"decision\":{\"behavior\":\"allow\"}}}'"}]}]}}
```

### Session Context Injection
```bash
#!/bin/bash
echo "## Context"; git status --short; echo "---"
```

## Security Checklist

- [ ] Validate file paths (block `..`)
- [ ] Check for dangerous command patterns
- [ ] Quote all shell variables (`"$VAR"`)
- [ ] Use absolute paths for scripts
- [ ] Never log secrets/credentials
- [ ] Parse JSON from stdin (not env vars)

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Hook not executing | Check path, permissions, test manually |
| Hook executing twice | Known bug in home directory; use project directory |
| Environment vars empty | Use stdin JSON instead |
| Timeout errors | Increase timeout or optimize script |

## Debug Commands

```bash
claude --debug          # Show hook loading/matching details
/hooks                  # Interactive hook management
tail -f transcript.jsonl | jq  # Monitor transcript
```

## Version Reference

SubagentStop(1.0.41), SessionEnd(1.0.85), updatedInput(2.0.10), PromptHooks(2.0.41), PermissionRequest(2.0.45)

Docs: https://code.claude.com/docs/en/hooks

---

## Context Graph Native Hook Configuration

The Context Graph project uses native Claude Code hooks configured via `.claude/settings.json`.

### Architecture

```
Claude Code Hook Event
  → .claude/hooks/<event_name>.sh  (bash script, reads JSON from stdin)
    → context-graph-cli <subcommand>  (compiled Rust binary)
      → TCP JSON-RPC to MCP server on :3100  (via mcp_client.rs)
        → MCP tool handler  (55 registered tools)
          → RocksDB + GPU embedders (13 embedder spaces)
```

### Existing Hook Scripts (`.claude/hooks/`)

| Script | CLI Command | Hook Event |
|--------|-------------|------------|
| `session_start.sh` | `context-graph-cli hooks session-start --stdin --format json` | SessionStart |
| `user_prompt_submit.sh` | `context-graph-cli hooks prompt-submit --session-id ... --stdin true --format json` | UserPromptSubmit |
| `pre_tool_use.sh` | `context-graph-cli hooks pre-tool --session-id ... --tool-name ... --fast-path true --format json` | PreToolUse |
| `post_tool_use.sh` | `context-graph-cli hooks post-tool --session-id ... --tool-name ... --format json` | PostToolUse |
| `session_end.sh` | `context-graph-cli hooks session-end --session-id ... --duration-ms ... --format json` | SessionEnd |
| `stop.sh` | `context-graph-cli memory capture-response --content ... --session-id ... --response-type stop_response` | Stop |

All scripts follow the same pattern:
1. `set -euo pipefail`
2. Read JSON from stdin
3. Validate JSON with `jq empty`
4. Find CLI binary (`$CONTEXT_GRAPH_CLI` env, then `./target/release/`, then `./target/debug/`, then `$HOME/.cargo/bin/`)
5. Call CLI with `timeout`
6. Handle timeout (exit 124→exit 2)
7. Exit codes pass through: 0=success, 1=error, 2=timeout, 3=db_error, 4=invalid_input

### CLI Subcommands for Hooks

```
context-graph-cli hooks session-start    # SessionStart (5s timeout)
context-graph-cli hooks pre-tool         # PreToolUse (500ms, no DB)
context-graph-cli hooks post-tool        # PostToolUse (3s timeout)
context-graph-cli hooks prompt-submit    # UserPromptSubmit (2s timeout)
context-graph-cli hooks session-end      # SessionEnd (30s timeout)
context-graph-cli memory inject-context  # Full context injection
context-graph-cli memory inject-brief    # Brief context (<200 tokens)
context-graph-cli memory capture-memory  # Store hook descriptions
context-graph-cli memory capture-response # Store Claude responses
```

### Prerequisites

1. MCP server must be running on TCP port 3100: `./target/release/context-graph-mcp --transport tcp --port 3100`
2. CLI binary must be built: `cargo build --release -p context-graph-cli`
3. MCP server registered with Claude Code: `claude mcp add context-graph -- ./target/release/context-graph-mcp`

### Why Native Hooks?

| Approach | Effort | Complexity | Maintenance |
|----------|--------|------------|-------------|
| Native Claude Code Hooks | ~25h | Low | Claude team maintains hook system |
| Custom Built-In Hooks | ~80h | High | We maintain hook infrastructure |

**Decision**: Native hooks provide 71% effort reduction with better long-term maintainability.
