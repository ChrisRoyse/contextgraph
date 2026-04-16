# Claude Code AI Agent Reference

> Complete reference for building hooks, skills, subagents, and slash commands in Claude Code (Jan 2026).

---

## Quick Reference

| Extension | Location | Trigger | Purpose |
|-----------|----------|---------|---------|
| CLAUDE.md | `./CLAUDE.md` | Auto-loaded | Persistent project memory |
| Slash Command | `.claude/commands/*.md` | Manual `/cmd` | User-invoked prompts |
| Skill | `.claude/skills/*/SKILL.md` | Auto by context | Model-invoked capabilities |
| Subagent | `.claude/agents/*.md` | Task tool | Isolated AI instances |
| Hook | `settings.json` | Lifecycle events | Deterministic automation |

---

## 1. Hooks System

Hooks are shell commands or LLM prompts that execute at specific lifecycle points. Configure in `~/.claude/settings.json`, `.claude/settings.json`, or `.claude/settings.local.json`.

### Hook Events

| Event | Trigger | Blocks? | Use Case |
|-------|---------|---------|----------|
| **PreToolUse** | Before tool runs | Yes | Validate/modify/block tools |
| **PostToolUse** | After tool completes | No | Format, log, post-process |
| **PermissionRequest** | Permission dialog shown | Yes | Auto-allow/deny operations |
| **UserPromptSubmit** | User submits message | Yes | Add context, validate input |
| **Stop** | Main agent finishes | Yes | Force continuation, verify completion |
| **SubagentStop** | Subagent finishes | Yes | Validate subagent output |
| **SessionStart** | Session begins/resumes | No | Load context, setup env |
| **SessionEnd** | Session terminates | No | Cleanup, logging |
| **PreCompact** | Before context compression | No | Backup transcripts |
| **Notification** | Notification sent | No | Custom notifications |

### Hook Types

**Command Hook** (`type: "command"`): Executes shell command
```json
{
  "type": "command",
  "command": "/path/to/script.sh",
  "timeout": 30000
}
```

**Prompt Hook** (`type: "prompt"`): LLM evaluation (Stop/SubagentStop only)
```json
{
  "type": "prompt",
  "prompt": "Evaluate if all tasks complete. Context: $ARGUMENTS",
  "timeout": 30000
}
```

### Configuration Schema

```json
{
  "hooks": {
    "HookEvent": [
      {
        "matcher": "ToolPattern",
        "hooks": [
          { "type": "command|prompt", "command|prompt": "...", "timeout": 30000 }
        ]
      }
    ]
  }
}
```

### Matchers

Matchers filter which tools trigger hooks (PreToolUse, PostToolUse, PermissionRequest only).

| Pattern | Matches |
|---------|---------|
| `"Write"` | Write tool only |
| `"Write\|Edit"` | Write OR Edit |
| `"*"` or `""` | All tools |
| `"Bash(npm test*)"` | Bash with specific args |
| `"mcp__github__.*"` | MCP tool pattern |
| `"Notebook.*"` | Regex pattern |

**Note**: Matchers are case-sensitive.

### Exit Codes

| Code | Meaning | Behavior |
|------|---------|----------|
| `0` | Success | stdout processed; action continues |
| `2` | Blocking error | stderr fed to Claude; action prevented |
| Other | Non-blocking error | stderr shown; action continues |

### JSON Output (stdout)

**Command hooks** can return structured JSON:
```json
{
  "decision": "allow|deny|ask",
  "reason": "explanation for Claude",
  "continue": true,
  "stopReason": "shown to user when continue=false",
  "suppressOutput": false,
  "systemMessage": "warning shown to user",
  "updatedInput": { "modified": "tool input" }
}
```

**PreToolUse decisions**:
- `"allow"`: Bypass permission, execute tool
- `"deny"`: Block tool, show reason to Claude
- `"ask"`: Show permission dialog to user

**Prompt hooks** (Stop/SubagentStop) return:
```json
{
  "decision": "approve|block",
  "reason": "explanation",
  "continue": true|false
}
```

### Environment Variables

| Variable | Scope | Value |
|----------|-------|-------|
| `CLAUDE_PROJECT_DIR` | All hooks | Project root path |
| `CLAUDE_CODE_REMOTE` | All hooks | `"true"` if web |
| `CLAUDE_ENV_FILE` | SessionStart only | Env file path for persistence |
| `CLAUDE_TOOL_INPUT_*` | Tool hooks | Tool input fields |

### stdin JSON Input

```json
{
  "session_id": "abc123",
  "transcript_path": "/path/to/transcript.jsonl",
  "cwd": "/current/directory",
  "permission_mode": "default|acceptEdits|bypassPermissions|plan",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {"command": "npm test"}
}
```

### Complete Hook Examples

**Auto-format after file edits**:
```json
{
  "hooks": {
    "PostToolUse": [{
      "matcher": "Write|Edit",
      "hooks": [{
        "type": "command",
        "command": "prettier --write \"$CLAUDE_TOOL_INPUT_FILE_PATH\" 2>/dev/null || true"
      }]
    }]
  }
}
```

**Block sensitive files**:
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Write|Edit",
      "hooks": [{
        "type": "command",
        "command": "if echo \"$CLAUDE_TOOL_INPUT_FILE_PATH\" | grep -qE '\\.(env|pem|key)$'; then echo '{\"decision\":\"deny\",\"reason\":\"Blocked: sensitive file\"}'; exit 0; fi"
      }]
    }]
  }
}
```

**Auto-approve test commands**:
```json
{
  "hooks": {
    "PermissionRequest": [{
      "matcher": "Bash(npm test*)",
      "hooks": [{
        "type": "command",
        "command": "echo '{\"decision\":\"allow\",\"reason\":\"Auto-approved test command\"}'"
      }]
    }]
  }
}
```

**Inject context on session start**:
```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "echo '## Context'; git status --short; echo '---'; cat TODO.md 2>/dev/null || true"
      }]
    }]
  }
}
```

**Force completion verification (prompt hook)**:
```json
{
  "hooks": {
    "Stop": [{
      "hooks": [{
        "type": "prompt",
        "prompt": "Review if Claude completed ALL requested tasks. If incomplete, respond with {\"decision\":\"block\",\"reason\":\"Tasks remaining\",\"continue\":true}. If complete, respond {\"decision\":\"approve\",\"continue\":false}. Context: $ARGUMENTS"
      }]
    }]
  }
}
```

**Validate subagent output**:
```json
{
  "hooks": {
    "SubagentStop": [{
      "hooks": [{
        "type": "prompt",
        "prompt": "Verify subagent fully completed assignment. Return {\"decision\":\"block\"} if work incomplete. Context: $ARGUMENTS"
      }]
    }]
  }
}
```

**Log all bash commands**:
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "echo \"$(date '+%Y-%m-%d %H:%M:%S'): $CLAUDE_TOOL_INPUT_COMMAND\" >> ~/.claude/bash.log"
      }]
    }]
  }
}
```

---

## 2. Skills System

Skills are model-invoked capabilities Claude automatically uses based on context matching. They load progressively to minimize context usage.

### Directory Structure

```
.claude/skills/my-skill/
├── SKILL.md           # Required: frontmatter + instructions
├── scripts/           # Optional: executable code
│   └── process.py
├── references/        # Optional: documentation for context
│   └── api.md
└── assets/            # Optional: templates, binary files
    └── template.html
```

Locations: `~/.claude/skills/` (user) or `.claude/skills/` (project)

### SKILL.md Structure

```yaml
---
name: my-skill
description: |
  What this skill does and when to use it.
  Include keywords users might mention.
  Example: "Analyze code quality and suggest improvements.
  Use when reviewing code or checking for issues."
allowed-tools: Read,Grep,Glob,Bash(npm:*)
model: sonnet
user-invocable: true
disable-model-invocation: false
version: 1.0.0
---

# Skill Title

## Overview
What this skill accomplishes.

## Instructions
1. Step one
2. Step two

## Resources
- Scripts: `{baseDir}/scripts/process.py`
- Reference: `{baseDir}/references/api.md`
```

### Frontmatter Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `name` | Yes | string | Identifier (lowercase, hyphens) |
| `description` | Yes | string | Discovery trigger - be comprehensive |
| `allowed-tools` | No | string | Comma-separated tools |
| `model` | No | string | `sonnet`, `opus`, `haiku`, or `inherit` |
| `user-invocable` | No | bool | Show in `/` menu (default: true) |
| `disable-model-invocation` | No | bool | Block Skill tool access |
| `version` | No | string | Semantic version |

### allowed-tools Syntax

```yaml
# Basic tools
allowed-tools: Read,Write,Bash,Glob,Grep

# Scoped bash commands
allowed-tools: Bash(git:*),Bash(npm:*),Read,Grep

# Specific command patterns
allowed-tools: Bash(git status:*),Bash(git diff:*),Read
```

### Progressive Disclosure

1. **Metadata**: Name + description loaded at startup (~100 tokens)
2. **SKILL.md body**: Loaded when skill triggers (<5k words)
3. **Bundled resources**: Loaded on-demand by Claude

### Best Practices

- **Description is key**: Include all "when to use" triggers
- **Keep SKILL.md < 500 lines**: Minimize context bloat
- **Use references/**: Split large documentation
- **Use scripts/**: Deterministic, token-efficient code
- **Use {baseDir}**: Reference bundled files
- **Write for Claude**: Don't explain obvious concepts

### Skill Examples

**Code Quality Analyzer**:
```yaml
---
name: code-analyzer
description: |
  Analyze code quality, complexity metrics, and maintainability.
  Use when reviewing code, checking quality, or looking for improvements.
allowed-tools: Read,Grep,Glob
model: sonnet
---

# Code Analyzer

## Process
1. Identify target files
2. Analyze complexity (cyclomatic, cognitive)
3. Check for code smells
4. Generate prioritized recommendations

## Output Format
- Summary of findings
- Detailed issues with line numbers
- Prioritized action items
```

**PDF Processor**:
```yaml
---
name: pdf
description: |
  Extract and analyze text from PDF documents.
  Use when users need to read, process, or extract data from PDFs.
allowed-tools: Read,Bash(python:*),Write
---

# PDF Processor

## Quick Start
Use pdfplumber for text extraction:
```python
import pdfplumber
with pdfplumber.open("file.pdf") as pdf:
    text = "\n".join(page.extract_text() for page in pdf.pages)
```

## Scripts
- Extract text: `{baseDir}/scripts/extract.py`
- Form filling: See `{baseDir}/references/forms.md`
```

---

## 3. Subagents (Task Tool)

Subagents are isolated Claude instances with separate context windows, spawned via the Task tool.

### Task Tool Parameters

| Parameter | Required | Type | Description |
|-----------|----------|------|-------------|
| `prompt` | Yes | string | Task instructions |
| `subagent_type` | Yes | string | Agent type identifier |
| `description` | Yes | string | 3-5 word summary |
| `model` | No | string | `sonnet`, `opus`, `haiku` |
| `run_in_background` | No | bool | Async execution |
| `resume` | No | string | Agent ID to continue |

### Built-in Subagent Types

| Type | Model | Mode | Tools | Use Case |
|------|-------|------|-------|----------|
| **Explore** | Haiku | Read-only | Glob,Grep,Read | Fast codebase search |
| **Plan** | Sonnet | Read-only | Glob,Grep,Read | Analysis, planning |
| **general-purpose** | Sonnet | Read/Write | All | Complex multi-step tasks |

### Task Tool Usage

```python
# Fast exploration
Task(
    prompt="Find all authentication-related files and patterns",
    subagent_type="Explore",
    description="Search auth files"
)

# Complex implementation
Task(
    prompt="Refactor the user service to use dependency injection",
    subagent_type="general-purpose",
    description="Refactor user service",
    model="opus"
)

# Background execution
Task(
    prompt="Analyze security vulnerabilities in src/",
    subagent_type="security-reviewer",
    run_in_background=True,
    description="Security scan"
)

# Resume previous agent
Task(
    prompt="Continue the analysis",
    subagent_type="general-purpose",
    resume="agent-id-abc123",
    description="Continue analysis"
)

# Parallel execution (multiple Task calls in one message)
Task(prompt="Analyze src/services/", subagent_type="Explore", description="Analyze services")
Task(prompt="Analyze src/components/", subagent_type="Explore", description="Analyze components")
```

### Custom Subagent Definition

**.claude/agents/security-reviewer.md**:
```yaml
---
name: security-reviewer
description: |
  Security specialist. Use PROACTIVELY for security analysis,
  vulnerability assessment, and code audits.
tools: Read,Grep,Glob
model: opus
---

# Security Reviewer

You are a security specialist focusing on:

## Authentication & Authorization
- Auth mechanism validation
- Authorization checks
- Privilege escalation risks

## Data Protection
- Sensitive data exposure
- Encryption usage
- Injection vulnerabilities (SQL, XSS, command)

## Output Format
For each finding:
1. Clear description
2. Risk level (Critical/High/Medium/Low)
3. Specific remediation
```

### Subagent Frontmatter Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `name` | Yes | string | Identifier |
| `description` | Yes | string | When to use (include "PROACTIVELY" for auto-use) |
| `tools` | No | string | Comma-separated tool list |
| `model` | No | string | `sonnet`, `opus`, `haiku`, `inherit` |
| `hooks` | No | object | Scoped hooks (PreToolUse, PostToolUse, Stop) |

### Subagent with Hooks

```yaml
---
name: careful-coder
description: Coder that validates all edits
tools: Read,Write,Edit,Bash
hooks:
  PreToolUse:
    - matcher: "Write|Edit"
      hooks:
        - type: command
          command: "echo 'Validating edit...'"
  Stop:
    - hooks:
        - type: prompt
          prompt: "Verify all code compiles and tests pass before stopping."
---
```

### Key Constraints

- Subagents **cannot spawn other subagents**
- Background subagents **cannot use MCP tools**
- Background subagents **auto-deny** non-preapproved permissions
- Results returned to main agent only (not visible to user)

---

## 4. Slash Commands

Slash commands are user-invoked prompts stored as markdown files.

### Locations

- **Project**: `.claude/commands/*.md`
- **Personal**: `~/.claude/commands/*.md`
- **Namespaced**: `.claude/commands/frontend/component.md` → `/project:frontend:component`

### Command Structure

```yaml
---
description: Brief description for help menu
argument-hint: <required> [optional]
allowed-tools: Read,Bash(git:*)
model: haiku
disable-model-invocation: true
---

# Command content

$ARGUMENTS = all arguments
$1, $2 = positional arguments
@filepath = file content injection
!`command` = inline bash execution (requires allowed-tools with Bash)
```

### Frontmatter Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `description` | No | string | Help menu text |
| `argument-hint` | No | string | Argument placeholder |
| `allowed-tools` | No | string | Permitted tools |
| `model` | No | string | Model override |
| `disable-model-invocation` | No | bool | Block Skill tool access |

### Command Examples

**Git Commit Command**:
```yaml
---
description: Create a git commit with staged changes
argument-hint: [message]
allowed-tools: Bash(git:*)
---

## Current State
- Status: !`git status --short`
- Staged: !`git diff --cached --stat`
- Recent: !`git log --oneline -5`

## Task
Create a commit with message: $ARGUMENTS

Follow conventional commit format. Include Co-Authored-By footer.
```

**PR Review Command**:
```yaml
---
description: Review a pull request
argument-hint: <pr-number>
allowed-tools: Bash(gh:*),Read,Grep
---

Review PR #$ARGUMENTS focusing on:

## Checklist
1. Code quality and style
2. Security vulnerabilities
3. Performance implications
4. Test coverage
5. Documentation updates

## PR Details
!`gh pr view $1 --json title,body,files`
```

**Component Generator**:
```yaml
---
description: Generate a React component
argument-hint: <ComponentName>
---

Create component $ARGUMENTS following pattern in:
@src/components/Button/Button.tsx

Use types from:
@src/types/components.ts

Include:
- TypeScript types
- Unit tests
- Storybook story
```

**Read-Only Analysis**:
```yaml
---
description: Analyze codebase without modifications
allowed-tools: Read,Grep,Glob
---

Analyze the codebase structure for $ARGUMENTS.
Report findings without making any changes.
```

---

## 5. CLAUDE.md (Memory System)

Project instructions auto-loaded at startup.

### Hierarchy (highest to lowest priority)

1. Enterprise CLAUDE.md (managed)
2. Project `./CLAUDE.md`
3. Modular rules `.claude/rules/*.md`
4. User `~/.claude/CLAUDE.md`
5. Local `./CLAUDE.local.md` (gitignored)

### Template

```markdown
# Project: MyApp

## Build Commands
- `npm run build` - Production build
- `npm test` - Run tests
- `npm run lint` - Lint code

## Code Style
- TypeScript strict mode
- Functional components
- Named exports

## Architecture
- `/src/components` - React components
- `/src/services` - API services
- `/src/utils` - Utilities

## Important
- Never commit .env files
- Run tests before commits
- Use conventional commits
```

### Path-Specific Rules

`.claude/rules/components.md`:
```yaml
---
globs: src/components/**/*.tsx
---

# Component Guidelines
- Use React.FC type
- Include PropTypes
- Add JSDoc comments
```

### File References

```markdown
See @docs/architecture.md for design.
Review @src/types/index.ts for types.
```

---

## 6. Tool Reference

| Tool | Permission | Purpose |
|------|------------|---------|
| Read | None | Read file contents |
| Write | Ask | Create files |
| Edit | Ask | Modify files |
| MultiEdit | Ask | Edit multiple files |
| Glob | None | Find files by pattern |
| Grep | None | Search file contents |
| Bash | Ask | Execute shell commands |
| WebFetch | Ask | Fetch URL contents |
| WebSearch | Ask | Search the web |
| Task | None | Spawn subagents |
| Skill | None | Invoke skills |
| TodoWrite | None | Manage task lists |
| NotebookEdit | Ask | Edit Jupyter notebooks |
| LSP | None | Language server queries |

### Permission Patterns

```json
{
  "permissions": {
    "allow": ["Read", "Glob", "Grep", "Bash(npm:*)", "Bash(git:*)"],
    "deny": ["Bash(rm -rf:*)", "Bash(sudo:*)", "Write(.env)"],
    "ask": ["Edit", "Write", "Bash"]
  }
}
```

---

## 7. Models

| Alias | Model | Best For |
|-------|-------|----------|
| `sonnet` | Claude Sonnet 4.5 | General development (default) |
| `opus` | Claude Opus 4.5 | Complex reasoning, architecture |
| `haiku` | Claude Haiku 4.5 | Fast exploration, simple tasks |
| `inherit` | Session model | Use current model |

**Selection Strategy**:
- **Haiku**: Subagent exploration, quick searches (10-20x cheaper)
- **Sonnet**: Daily development, code reviews, implementations
- **Opus**: Security analysis, complex refactoring, architecture

---

## 8. Component Comparison

| Aspect | CLAUDE.md | Slash Command | Skill | Subagent |
|--------|-----------|---------------|-------|----------|
| Trigger | Auto at startup | Manual `/cmd` | Auto by context | Task tool |
| Scope | Session-wide | Single execution | Single execution | Isolated context |
| State | Persistent | Stateless | Stateless | Separate window |
| Complexity | Instructions | Simple prompts | Multi-file workflows | Full agent |
| Tool Control | Via settings | Frontmatter | Frontmatter | Frontmatter |
| Best For | Conventions | Explicit tasks | Domain expertise | Parallel work |

---

## 9. Best Practices

### For AI Agents

1. **Read before writing**: Always read files before modifying
2. **Use specialized tools**: Prefer Read/Edit over Bash cat/sed
3. **Spawn subagents**: Delegate exploration to Explore agent
4. **Use TodoWrite**: Track multi-step tasks
5. **Respect permissions**: Follow configured allow/deny rules
6. **Minimal changes**: Don't over-engineer solutions
7. **Test after changes**: Run tests after modifications

### For Hooks

1. **Fast execution**: Set appropriate timeouts
2. **Handle errors**: Use proper exit codes
3. **Quote variables**: Always use `"$VAR"`
4. **Validate inputs**: Sanitize hook inputs
5. **Avoid logging secrets**: Don't log credentials

### For Skills

1. **Description is key**: Include comprehensive triggers
2. **Progressive disclosure**: Split large docs into references/
3. **Use scripts/**: Deterministic, token-efficient
4. **Keep SKILL.md lean**: <500 lines
5. **Test before deploying**: Verify skill behavior

### For Subagents

1. **Single responsibility**: One focused task per agent
2. **Action descriptions**: Use "PROACTIVELY" for auto-use
3. **Minimal tools**: Grant only necessary access
4. **Version control**: Store in `.claude/agents/`

---

## 10. Debugging

| Issue | Solution |
|-------|----------|
| Hook not executing | Check path, timeout, test manually with `bash script.sh` |
| Skill not discovered | Improve description keywords |
| Subagent not invoking | Use explicit "Use the X agent" |
| Permission denied | Check `/permissions`, add to allow list |
| Context too large | Use `/compact` or spawn subagents |

**Debug commands**:
```bash
claude --debug          # See skill loading errors
/config                 # Check configuration
/permissions            # View permission rules
/hooks                  # Manage hooks
/mcp test <server>      # Test MCP connection
```

---

## Sources

- [Hooks reference](https://code.claude.com/docs/en/hooks)
- [Agent Skills](https://code.claude.com/docs/en/skills)
- [Create custom subagents](https://code.claude.com/docs/en/sub-agents)
- [Slash commands](https://code.claude.com/docs/en/slash-commands)
- [How to configure hooks](https://claude.com/blog/how-to-configure-hooks)
- [Agent Skills best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Anthropic skills repository](https://github.com/anthropics/skills)
