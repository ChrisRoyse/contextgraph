# Claude Code CLI — Hook Parameters Reference

What data Claude Code exposes to hooks at each lifecycle event. All hooks receive JSON via **stdin** and communicate back through **exit codes** and **stdout**.

---

## Common Input Fields (All Hooks)

Every hook event receives these fields in the JSON payload on stdin:

| Field | Type | Description |
|---|---|---|
| `session_id` | `string` | UUID identifier for the current session |
| `transcript_path` | `string` | Absolute path to the session transcript JSONL file (e.g. `~/.claude/projects/.../abc123.jsonl`) |
| `cwd` | `string` | Current working directory when the hook was invoked |
| `permission_mode` | `string` | Active permission mode: `"default"`, `"plan"`, `"acceptEdits"`, `"dontAsk"`, or `"bypassPermissions"` |
| `hook_event_name` | `string` | Name of the event that fired (e.g. `"PreToolUse"`, `"Stop"`) |

---

## Environment Variables (All Hooks)

| Variable | Availability | Description |
|---|---|---|
| `CLAUDE_PROJECT_DIR` | All hooks | Absolute path to the project root |
| `CLAUDE_CODE_REMOTE` | All hooks | Set to `"true"` in remote/web environments; unset in local CLI |
| `CLAUDE_PLUGIN_ROOT` | Plugin hooks | Root directory of the plugin (for portable script paths) |
| `CLAUDE_ENV_FILE` | **SessionStart only** | File path to write `export` statements that persist env vars for subsequent Bash commands |

---

## Exit Code Semantics (All Hooks)

| Exit Code | Meaning | Behavior |
|---|---|---|
| `0` | Success | stdout parsed for JSON output; action proceeds |
| `2` | Blocking error | stderr fed back to Claude as error; action blocked (for blockable events) |
| Other | Non-blocking error | stderr shown in verbose mode; execution continues |

---

## Universal JSON Output Fields (All Hooks)

When a hook exits 0 and prints JSON to stdout, these fields are recognized across all events:

| Field | Default | Description |
|---|---|---|
| `continue` | `true` | If `false`, Claude stops processing entirely |
| `stopReason` | none | Message shown to user when `continue` is `false` |
| `suppressOutput` | `false` | If `true`, hides stdout from verbose mode |
| `systemMessage` | none | Warning message shown to user |

---

## Hook Events — Detailed Input & Output

### 1. SessionStart

**When:** Session begins (new or resumed) or after `/clear` or compaction.

**Matcher values:** `startup`, `resume`, `clear`, `compact`

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `source` | `string` | How the session started: `"startup"`, `"resume"`, `"clear"`, or `"compact"` |
| `model` | `string` | Model identifier (e.g. `"claude-sonnet-4-5-20250929"`) |
| `agent_type` | `string` | *(optional)* Agent name if started with `claude --agent <name>` |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "SessionStart",
  "source": "startup",
  "model": "claude-sonnet-4-5-20250929"
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `additionalContext` | String added to Claude's context (via `hookSpecificOutput`) |

Plain text stdout is also added as context for Claude. `CLAUDE_ENV_FILE` is available to persist environment variables.

#### Can Block? No

---

### 2. UserPromptSubmit

**When:** User submits a prompt, before Claude processes it.

**Matcher:** Not supported (fires on every occurrence).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `prompt` | `string` | The text the user submitted |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "UserPromptSubmit",
  "prompt": "Write a function to calculate factorial"
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `decision` | `"block"` prevents the prompt from being processed and erases it |
| `reason` | Shown to user when decision is `"block"` |
| `additionalContext` | String added to Claude's context (via `hookSpecificOutput`) |

Plain text stdout (non-JSON) is also added as context on exit 0.

#### Can Block? Yes (exit 2 or `decision: "block"`)

---

### 3. PreToolUse

**When:** After Claude creates tool parameters, before executing the tool call.

**Matcher:** Tool name — `Bash`, `Edit`, `Write`, `Read`, `Glob`, `Grep`, `Task`, `WebFetch`, `WebSearch`, `mcp__<server>__<tool>`, etc.

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `tool_name` | `string` | Name of the tool being called |
| `tool_input` | `object` | Tool-specific parameters (see below) |
| `tool_use_id` | `string` | Unique identifier for this tool use |

#### Tool Input Schemas

**Bash:**
| Field | Type | Description |
|---|---|---|
| `command` | `string` | Shell command to execute |
| `description` | `string` | *(optional)* Description of what the command does |
| `timeout` | `number` | *(optional)* Timeout in milliseconds |
| `run_in_background` | `boolean` | *(optional)* Whether to run in background |

**Write:**
| Field | Type | Description |
|---|---|---|
| `file_path` | `string` | Absolute path to file |
| `content` | `string` | Content to write |

**Edit:**
| Field | Type | Description |
|---|---|---|
| `file_path` | `string` | Absolute path to file |
| `old_string` | `string` | Text to find and replace |
| `new_string` | `string` | Replacement text |
| `replace_all` | `boolean` | *(optional)* Replace all occurrences |

**Read:**
| Field | Type | Description |
|---|---|---|
| `file_path` | `string` | Absolute path to file |
| `offset` | `number` | *(optional)* Line number to start reading from |
| `limit` | `number` | *(optional)* Number of lines to read |

**Glob:**
| Field | Type | Description |
|---|---|---|
| `pattern` | `string` | Glob pattern (e.g. `"**/*.ts"`) |
| `path` | `string` | *(optional)* Directory to search in |

**Grep:**
| Field | Type | Description |
|---|---|---|
| `pattern` | `string` | Regex pattern to search for |
| `path` | `string` | *(optional)* File or directory to search |
| `glob` | `string` | *(optional)* Glob pattern to filter files |
| `output_mode` | `string` | *(optional)* `"content"`, `"files_with_matches"`, or `"count"` |
| `-i` | `boolean` | *(optional)* Case insensitive |
| `multiline` | `boolean` | *(optional)* Enable multiline matching |

**WebFetch:**
| Field | Type | Description |
|---|---|---|
| `url` | `string` | URL to fetch |
| `prompt` | `string` | Prompt to run on fetched content |

**WebSearch:**
| Field | Type | Description |
|---|---|---|
| `query` | `string` | Search query |
| `allowed_domains` | `array` | *(optional)* Only include results from these domains |
| `blocked_domains` | `array` | *(optional)* Exclude results from these domains |

**Task (subagent):**
| Field | Type | Description |
|---|---|---|
| `prompt` | `string` | Task for the agent to perform |
| `description` | `string` | Short description |
| `subagent_type` | `string` | Agent type (e.g. `"Explore"`, `"Plan"`) |
| `model` | `string` | *(optional)* Model alias override |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {
    "command": "npm test"
  },
  "tool_use_id": "toolu_01ABC123..."
}
```

#### Output (event-specific via `hookSpecificOutput`)

| Field | Description |
|---|---|
| `permissionDecision` | `"allow"` bypasses permission, `"deny"` blocks the call, `"ask"` prompts the user |
| `permissionDecisionReason` | Explanation — shown to user for allow/ask, shown to Claude for deny |
| `updatedInput` | Modifies tool input parameters before execution |
| `additionalContext` | String added to Claude's context before execution |

#### Can Block? Yes (exit 2 or `permissionDecision: "deny"`)

---

### 4. PermissionRequest

**When:** A permission dialog is about to be shown to the user.

**Matcher:** Tool name (same as PreToolUse).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `tool_name` | `string` | Name of the tool requesting permission |
| `tool_input` | `object` | Tool-specific parameters |
| `permission_suggestions` | `array` | *(optional)* The "always allow" options the user would see |

Note: Does **not** include `tool_use_id` (unlike PreToolUse).

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "PermissionRequest",
  "tool_name": "Bash",
  "tool_input": {
    "command": "rm -rf node_modules"
  },
  "permission_suggestions": [
    { "type": "toolAlwaysAllow", "tool": "Bash" }
  ]
}
```

#### Output (event-specific via `hookSpecificOutput`)

| Field | Description |
|---|---|
| `decision.behavior` | `"allow"` grants permission, `"deny"` denies it |
| `decision.updatedInput` | *(allow only)* Modifies tool input before execution |
| `decision.updatedPermissions` | *(allow only)* Applies permission rules (equivalent to user selecting "always allow") |
| `decision.message` | *(deny only)* Tells Claude why permission was denied |
| `decision.interrupt` | *(deny only)* If `true`, stops Claude entirely |

#### Can Block? Yes (exit 2 or `decision.behavior: "deny"`)

---

### 5. PostToolUse

**When:** Immediately after a tool completes successfully.

**Matcher:** Tool name (same as PreToolUse).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `tool_name` | `string` | Name of the tool that ran |
| `tool_input` | `object` | Arguments sent to the tool |
| `tool_response` | `string` or `object` | Result returned by the tool |
| `tool_use_id` | `string` | Unique identifier for this tool use |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "PostToolUse",
  "tool_name": "Write",
  "tool_input": {
    "file_path": "/path/to/file.txt",
    "content": "file content"
  },
  "tool_response": {
    "filePath": "/path/to/file.txt",
    "success": true
  },
  "tool_use_id": "toolu_01ABC123..."
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `decision` | `"block"` prompts Claude with the reason |
| `reason` | Explanation shown to Claude when decision is `"block"` |
| `additionalContext` | Additional context for Claude (via `hookSpecificOutput`) |
| `updatedMCPToolOutput` | *(MCP tools only)* Replaces the tool's output |

#### Can Block? No (tool already ran; `"block"` sends feedback but doesn't undo)

---

### 6. PostToolUseFailure

**When:** A tool execution fails (throws error or returns failure).

**Matcher:** Tool name (same as PreToolUse).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `tool_name` | `string` | Name of the tool that failed |
| `tool_input` | `object` | Arguments sent to the tool |
| `tool_use_id` | `string` | Unique identifier for this tool use |
| `error` | `string` | Description of what went wrong |
| `is_interrupt` | `boolean` | *(optional)* Whether failure was caused by user interruption |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "PostToolUseFailure",
  "tool_name": "Bash",
  "tool_input": {
    "command": "npm test"
  },
  "tool_use_id": "toolu_01ABC123...",
  "error": "Command exited with non-zero status code 1",
  "is_interrupt": false
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `additionalContext` | Additional context for Claude alongside the error (via `hookSpecificOutput`) |

#### Can Block? No (tool already failed)

---

### 7. Notification

**When:** Claude Code sends a notification.

**Matcher values:** `permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_dialog`

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `message` | `string` | Notification text |
| `title` | `string` | *(optional)* Notification title |
| `notification_type` | `string` | Which notification type fired |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "Notification",
  "message": "Claude needs your permission to use Bash",
  "title": "Permission needed",
  "notification_type": "permission_prompt"
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `additionalContext` | String added to Claude's context |

#### Can Block? No

---

### 8. SubagentStart

**When:** A subagent is spawned via the Task tool.

**Matcher:** Agent type name — `Bash`, `Explore`, `Plan`, or custom agent names from `.claude/agents/`.

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `agent_id` | `string` | Unique identifier for the subagent |
| `agent_type` | `string` | Agent type name |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "SubagentStart",
  "agent_id": "agent-abc123",
  "agent_type": "Explore"
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `additionalContext` | String added to the subagent's context (via `hookSpecificOutput`) |

#### Can Block? No (but can inject context)

---

### 9. SubagentStop

**When:** A subagent finishes responding.

**Matcher:** Agent type name (same as SubagentStart).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `stop_hook_active` | `boolean` | `true` if Claude is already continuing due to a prior stop hook |
| `agent_id` | `string` | Unique identifier for the subagent |
| `agent_type` | `string` | Agent type name (used for matcher filtering) |
| `agent_transcript_path` | `string` | Path to the subagent's own transcript (in nested `subagents/` folder) |

Note: `transcript_path` = main session transcript; `agent_transcript_path` = subagent's transcript.

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "~/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "SubagentStop",
  "stop_hook_active": false,
  "agent_id": "def456",
  "agent_type": "Explore",
  "agent_transcript_path": "~/.claude/projects/.../abc123/subagents/agent-def456.jsonl"
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `decision` | `"block"` prevents the subagent from stopping |
| `reason` | Required when `decision` is `"block"` — tells Claude why it should continue |

#### Can Block? Yes (exit 2 or `decision: "block"`)

---

### 10. Stop

**When:** Main Claude Code agent finishes responding. Does **not** fire on user interrupt.

**Matcher:** Not supported (fires on every occurrence).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `stop_hook_active` | `boolean` | `true` if Claude is already continuing due to a prior stop hook (use to prevent infinite loops) |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "~/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "Stop",
  "stop_hook_active": true
}
```

#### Output (event-specific)

| Field | Description |
|---|---|
| `decision` | `"block"` prevents Claude from stopping |
| `reason` | Required when `decision` is `"block"` — tells Claude why it should continue |

#### Can Block? Yes (exit 2 or `decision: "block"`)

---

### 11. TeammateIdle

**When:** An agent team teammate is about to go idle after finishing its turn.

**Matcher:** Not supported (fires on every occurrence).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `teammate_name` | `string` | Name of the teammate going idle |
| `team_name` | `string` | Name of the team |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "TeammateIdle",
  "teammate_name": "researcher",
  "team_name": "my-project"
}
```

#### Output

Exit code 2 only (no JSON decision control). stderr is fed back as feedback.

#### Can Block? Yes (exit 2 keeps teammate working)

---

### 12. TaskCompleted

**When:** A task is being marked as completed (via TaskUpdate or when a teammate finishes its turn with in-progress tasks).

**Matcher:** Not supported (fires on every occurrence).

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `task_id` | `string` | Identifier of the task being completed |
| `task_subject` | `string` | Title of the task |
| `task_description` | `string` | *(optional)* Detailed task description |
| `teammate_name` | `string` | *(optional)* Name of the teammate completing the task |
| `team_name` | `string` | *(optional)* Name of the team |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "TaskCompleted",
  "task_id": "task-001",
  "task_subject": "Implement user authentication",
  "task_description": "Add login and signup endpoints",
  "teammate_name": "implementer",
  "team_name": "my-project"
}
```

#### Output

Exit code 2 only (no JSON decision control). stderr is fed back as feedback.

#### Can Block? Yes (exit 2 prevents task completion)

---

### 13. PreCompact

**When:** Before a context compaction operation.

**Matcher values:** `manual` (from `/compact`), `auto` (context window full)

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `trigger` | `string` | `"manual"` or `"auto"` |
| `custom_instructions` | `string` | What the user passed to `/compact` (empty for auto) |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "PreCompact",
  "trigger": "manual",
  "custom_instructions": ""
}
```

#### Can Block? No

---

### 14. SessionEnd

**When:** Session terminates.

**Matcher values:** `clear`, `logout`, `prompt_input_exit`, `bypass_permissions_disabled`, `other`

#### Input Fields

| Field | Type | Description |
|---|---|---|
| `reason` | `string` | Why the session ended (see matcher values above) |

#### Example Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../abc123.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "SessionEnd",
  "reason": "other"
}
```

#### Can Block? No (cleanup only)

---

## Quick Reference: All Events at a Glance

| # | Event | Matcher On | Event-Specific Input Fields | Can Block? | Decision Mechanism |
|---|---|---|---|---|---|
| 1 | `SessionStart` | session source | `source`, `model`, `agent_type`? | No | stdout = context |
| 2 | `UserPromptSubmit` | *(none)* | `prompt` | Yes | `decision: "block"` or exit 2 |
| 3 | `PreToolUse` | tool name | `tool_name`, `tool_input`, `tool_use_id` | Yes | `permissionDecision` or exit 2 |
| 4 | `PermissionRequest` | tool name | `tool_name`, `tool_input`, `permission_suggestions`? | Yes | `decision.behavior` or exit 2 |
| 5 | `PostToolUse` | tool name | `tool_name`, `tool_input`, `tool_response`, `tool_use_id` | No | `decision: "block"` = feedback |
| 6 | `PostToolUseFailure` | tool name | `tool_name`, `tool_input`, `tool_use_id`, `error`, `is_interrupt`? | No | context only |
| 7 | `Notification` | notification type | `message`, `title`?, `notification_type` | No | context only |
| 8 | `SubagentStart` | agent type | `agent_id`, `agent_type` | No | context injection |
| 9 | `SubagentStop` | agent type | `stop_hook_active`, `agent_id`, `agent_type`, `agent_transcript_path` | Yes | `decision: "block"` or exit 2 |
| 10 | `Stop` | *(none)* | `stop_hook_active` | Yes | `decision: "block"` or exit 2 |
| 11 | `TeammateIdle` | *(none)* | `teammate_name`, `team_name` | Yes | exit 2 only |
| 12 | `TaskCompleted` | *(none)* | `task_id`, `task_subject`, `task_description`?, `teammate_name`?, `team_name`? | Yes | exit 2 only |
| 13 | `PreCompact` | compaction trigger | `trigger`, `custom_instructions` | No | — |
| 14 | `SessionEnd` | exit reason | `reason` | No | — |

`?` = optional field

---

## Hook Handler Types

### Command Hooks (`type: "command"`)

Run a shell command. Receive JSON on stdin, respond via exit codes and stdout.

| Field | Required | Description |
|---|---|---|
| `type` | Yes | `"command"` |
| `command` | Yes | Shell command to execute |
| `timeout` | No | Seconds before canceling (default: 600) |
| `async` | No | If `true`, runs in background without blocking |
| `statusMessage` | No | Custom spinner message |
| `once` | No | If `true`, runs only once per session (skills only) |

### Prompt Hooks (`type: "prompt"`)

Single-turn LLM evaluation. Returns `{ "ok": true/false, "reason": "..." }`.

| Field | Required | Description |
|---|---|---|
| `type` | Yes | `"prompt"` |
| `prompt` | Yes | Prompt text; use `$ARGUMENTS` for hook input JSON |
| `model` | No | Model to use (default: fast model) |
| `timeout` | No | Seconds before canceling (default: 30) |

Supported events: `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`, `UserPromptSubmit`, `Stop`, `SubagentStop`, `TaskCompleted`.

### Agent Hooks (`type: "agent"`)

Multi-turn subagent with tool access (Read, Grep, Glob). Returns `{ "ok": true/false, "reason": "..." }`.

| Field | Required | Description |
|---|---|---|
| `type` | Yes | `"agent"` |
| `prompt` | Yes | Prompt text; use `$ARGUMENTS` for hook input JSON |
| `model` | No | Model to use (default: fast model) |
| `timeout` | No | Seconds before canceling (default: 60) |

Same supported events as prompt hooks.

---

## Variable Substitution (Prompt/Agent Hooks)

These placeholders are available in prompt text:

| Variable | Description |
|---|---|
| `$ARGUMENTS` | Full hook input JSON |
| `$TOOL_INPUT` | Tool input object |
| `$TOOL_RESULT` | Tool result |
| `$USER_PROMPT` | User's prompt text |
| `$TOOL_NAME` | Name of the tool |
| `$SESSION_ID` | Session identifier |
| `$CWD` | Current working directory |

---

## Notable Gaps (Not Currently Exposed)

Based on community requests (GitHub issue #17188), these fields are **not** currently exposed in hook payloads:

| Field | Status |
|---|---|
| Claude's internal reasoning/thinking | Not exposed |
| Session name (from `/rename`) | Not exposed |
| Session start timestamp | Not exposed |
| Model temperature / parameters | Not exposed |
| Token usage / cost | Not exposed |
| `CLAUDE_SESSION_ID` as env var | Not exposed (only in stdin JSON) |

---

## Sources

- [Hooks Reference — Claude Code Docs](https://code.claude.com/docs/en/hooks)
- [How to Configure Hooks — Claude Blog](https://claude.com/blog/how-to-configure-hooks)
- [Hook Development Skill — GitHub](https://github.com/anthropics/claude-code/blob/main/plugins/plugin-dev/skills/hook-development/SKILL.md)
- [Feature Request: Expose Session Metadata — GitHub #17188](https://github.com/anthropics/claude-code/issues/17188)
- [Claude Code Hooks Mastery — GitHub](https://github.com/disler/claude-code-hooks-mastery)
