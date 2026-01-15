# Claude Code Skills Reference

Modular, filesystem-based prompt extensions that transform Claude into domain specialists. Skills load context on-demand via progressive disclosure.

## Skills vs Other Extensions

| Extension | Location | Trigger | Best For |
|-----------|----------|---------|----------|
| CLAUDE.md | `./CLAUDE.md` | Auto-loaded | Project conventions |
| Slash Command | `.claude/commands/*.md` | Manual `/cmd` | Explicit tasks |
| Skill | `.claude/skills/*/SKILL.md` | Auto by context | Domain expertise |
| Subagent | `.claude/agents/*.md` | Task tool | Parallel work |
| Hook | `settings.json` | Lifecycle events | Automation |

## Decision Matrix

| Question | CLAUDE.md | Slash Cmd | Skill | Subagent |
|----------|-----------|-----------|-------|----------|
| Always-on? | ✓ | - | - | - |
| User triggers? | - | ✓ | - | - |
| Auto-triggers? | ✓ | - | ✓ | ✓ |
| Separate context? | - | - | - | ✓ |
| Bundled resources? | - | - | ✓ | ✓ |

## SKILL.md Template

```yaml
---
name: skill-name          # Required: max 64 chars, lowercase, hyphens
description: |            # Required: max 1024 chars, triggers discovery
  What. When. Keywords.
allowed-tools: Read,Glob  # Optional: comma-separated, scoped Bash(git:*)
model: sonnet             # Optional: haiku|sonnet|opus|inherit
---
# Title

## Overview
Purpose and capabilities.

## Instructions
1. Step one
2. Step two

## Examples
Input → Output

## Resources
- `{baseDir}/scripts/helper.py`
- `{baseDir}/references/docs.md`
```

## Frontmatter Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `name` | ✓ | string | Unique ID, max 64 chars, no "anthropic"/"claude" |
| `description` | ✓ | string | Discovery trigger, max 1024 chars |
| `allowed-tools` | - | string | Permitted tools (CLI only) |
| `model` | - | string | `inherit`/`sonnet`/`opus`/`haiku` |
| `version` | - | string | Semantic version |
| `disable-model-invocation` | - | boolean | Block auto-invocation |
| `user-invocable` | - | boolean | Show in `/` menu |

**Syntax Rules**: Start with `---`, use spaces (not tabs), avoid multiline descriptions (Prettier wrapping causes silent failures).

## File Organization

```
.claude/skills/my-skill/
├── SKILL.md           # Required
├── scripts/           # Execute via bash (code never loads into context)
├── references/        # Read when needed
├── templates/         # Copy/modify
└── assets/            # Binary files
```

| Location | Scope | Priority |
|----------|-------|----------|
| `.claude/skills/` | Project | Highest |
| `~/.claude/skills/` | Personal | Medium |
| Plugin `skills/` | Distribution | Lowest |

## Progressive Disclosure

| Level | Content | When Loaded | Token Cost |
|-------|---------|-------------|------------|
| Metadata | name + description | Always (startup) | ~100/skill |
| Instructions | SKILL.md body | On trigger | <5k |
| Resources | Bundled files | On demand | Unlimited |

Always use `{baseDir}` for file paths. Never hardcode absolute paths.

**Script efficiency**: Script code stays on filesystem; only output enters context.

## Slash Commands

| Aspect | Skill | Slash Command |
|--------|-------|---------------|
| Trigger | Auto | Manual `/cmd` |
| Location | `.claude/skills/*/SKILL.md` | `.claude/commands/*.md` |
| Resources | Full directory | Single file |

**Dynamic content syntax**:
- `$ARGUMENTS` / `$1`, `$2` - Argument substitution
- `@filepath` - File content injection
- `!`command`` - Inline bash execution

## Subagents

| Aspect | Skill | Subagent |
|--------|-------|----------|
| Context | Same as main | Isolated window |
| Purpose | Domain expertise | Delegate parallel work |
| Result | Modifies main | Returns summary |
| Spawning | Can spawn | Cannot spawn |

**Built-in types**:

| Type | Model | Mode | Use Case |
|------|-------|------|----------|
| Explore | Haiku | Read-only | Fast codebase search |
| Plan | Sonnet | Read-only | Analysis, planning |
| general-purpose | Sonnet | Read/Write | Complex multi-step |

**Task tool params**: `{prompt, subagent_type, description}` required; `{model?, run_in_background?, resume?}` optional.

**Constraints**: Cannot spawn subagents; background cannot use MCP tools.

## Pre-Built Skills

| Skill | skill_id | Capabilities |
|-------|----------|--------------|
| PowerPoint | `pptx` | Create/edit presentations |
| Excel | `xlsx` | Spreadsheets, charts |
| Word | `docx` | Documents, formatting |
| PDF | `pdf` | Generate PDF reports |

```bash
# Install in Claude Code
/plugin marketplace add anthropics/skills
/plugin install document-skills@anthropic-agent-skills
```

## Best Practices

### DO
- Write descriptions in third person with WHAT/WHEN/keywords
- Keep SKILL.md under 500 lines
- Use `{baseDir}` for paths
- Keep references one level deep
- Provide step-by-step instructions
- Handle errors in scripts
- Test with Haiku, Sonnet, Opus

### DON'T
- Use first/second person in descriptions
- Be vague ("Helps with documents")
- Explain concepts Claude already knows
- Create deeply nested references
- Hardcode absolute paths
- Use Windows-style paths (`\`)
- Assume packages are installed

## Security

**Risks**:

| Risk | Mitigation |
|------|------------|
| Malicious instructions | Audit SKILL.md content |
| Data exfiltration | Review bundled scripts |
| Tool misuse | Check allowed-tools |
| External dependencies | Avoid URLs in skills |

**Safe practices**:
```yaml
allowed-tools: Read,Grep,Glob              # Restrict tools
disable-model-invocation: true              # Block auto-invocation
# Never hardcode credentials - use env vars
```

## Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| Skill not discovered | Poor description | Improve keywords |
| Skill not loading | Invalid YAML | Check syntax |
| Wrong skill triggered | Overlapping descriptions | Make more specific |
| Script fails | Missing dependencies | List in SKILL.md |
| Bash denied | allowed-tools restriction | Add required tools |
| Context overflow | Skill too large | Split into references |

**Debug**:
```bash
claude --debug
"What skills are available?"
"Use the [skill-name] skill to..."
```

**Frontmatter gotchas**: No blank lines before `---`; spaces not tabs; multiline descriptions cause silent failures.

## API Usage

```python
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    messages=[{"role": "user", "content": "Create a PowerPoint"}],
    tools=[{"type": "code_execution"}],
    container={"skills": ["pptx"]},
    betas=["code-execution-2025-08-25", "skills-2025-10-02"]
)
```

## Resources

- Docs: https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview
- CLI: https://code.claude.com/docs/en/skills
- Skills repo: https://github.com/anthropics/skills
