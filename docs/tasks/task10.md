# Task 10: Create memory-inject SKILL.md

## Metadata
- **Task ID**: TASK-GAP-010
- **Phase**: 3 (Skills Framework)
- **Priority**: Medium
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: task02 (TASK-GAP-003 - directory must exist)

## Objective

Create the memory-inject skill SKILL.md file as specified by PRD Section 9.3. This skill retrieves and injects contextual memories for the current task via the `/memory-inject` command. The skill file documents how Claude should use the `inject_context` MCP tool.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 3.3.2 for skill specification
- `/home/cabdru/contextgraph/.claude/skills/memory-inject/` - Directory must exist (created in task02)

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/.claude/skills/memory-inject/SKILL.md`

## Implementation Steps

### Step 1: Create SKILL.md

Create the skill file with proper frontmatter and content following the Claude Code skill format.

## Code/Content to Implement

### /home/cabdru/contextgraph/.claude/skills/memory-inject/SKILL.md

```markdown
---
model: haiku
user_invocable: true
---

# Memory Inject

Retrieve and inject contextual memories for the current task.

**Keywords**: memory, context, inject, retrieve, recall, background

## Instructions

When the user needs context, background, or wants to recall previous work:

1. Determine the query from user input or current task context
2. Call `inject_context` with the query
3. Present retrieved memories with relevance scores
4. If verbose requested, include similarity scores per embedding space

## MCP Tools

- `inject_context`: Retrieve and format relevant memories
  - Parameters:
    - `query` (required): Search query text
    - `max_tokens` (optional): Token budget (default 1000)
    - `verbosity` (optional): "compact" | "standard" | "verbose"
  - Returns: `{ memories: Memory[], total_found: u32, token_count: u32 }`

## Output Format

### Compact
Brief memory summaries within token budget:
```
Relevant Context:
- [Memory summary 1] (relevance: 0.XX)
- [Memory summary 2] (relevance: 0.XX)
[N results, M tokens]
```

### Standard
Memory content with source and timestamp:
```
Retrieved Memories (N found):

1. [Memory content]
   - Source: HookDescription | Created: 2024-01-15 10:30
   - Relevance: 0.XX

2. [Memory content]
   - Source: ClaudeResponse | Created: 2024-01-14 15:45
   - Relevance: 0.XX

Token usage: M/max_tokens
```

### Verbose
Include similarity scores per semantic space (E1, E5, E6, E7, E10, E12, E13):
```
Retrieved Memories (N found):

1. [Memory content]
   - Source: MDFileChunk | Created: 2024-01-15 10:30
   - Relevance: 0.XX
   - Similarity by Space:
     E1_Semantic: 0.XX
     E5_Causal: 0.XX
     E7_Code: 0.XX
     ...
```

## Edge Cases

- **No relevant memories**: "No relevant memories found for this query."
- **Token budget exceeded**: Automatically truncates; note "Results truncated to fit budget"
- **Empty query**: Prompt user to provide a search query
- **Very broad query**: Warn that results may be less focused; suggest more specific terms

## Example Usage

User: "What did we discuss about authentication?"
Response: Call `inject_context({query: "authentication", verbosity: "standard"})` and present results.

User: "Get me background on this project"
Response: Call `inject_context({query: "[current project context]", max_tokens: 500})` for compact overview.

User: "I need detailed context with similarity scores"
Response: Call `inject_context({query: "[topic]", verbosity: "verbose"})` for full breakdown.
```

## Definition of Done

- [ ] File exists at `/home/cabdru/contextgraph/.claude/skills/memory-inject/SKILL.md`
- [ ] Frontmatter contains `model: haiku` (per PRD Section 9.3)
- [ ] Frontmatter contains `user_invocable: true`
- [ ] Keywords documented: memory, context, inject, retrieve, recall, background
- [ ] MCP tool documented: inject_context
- [ ] Output formats documented (compact, standard, verbose)
- [ ] Edge cases documented (no memories, token exceeded)
- [ ] File is valid Markdown

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file exists
test -f .claude/skills/memory-inject/SKILL.md && echo "SKILL.md exists"

# Verify frontmatter has correct model (haiku)
head -5 .claude/skills/memory-inject/SKILL.md
# Should show:
# ---
# model: haiku
# user_invocable: true
# ---

# Verify keywords are documented
grep -i "keywords" .claude/skills/memory-inject/SKILL.md
# Should show: memory, context, inject, retrieve, recall, background

# Verify MCP tool is documented
grep "inject_context" .claude/skills/memory-inject/SKILL.md
# Should show tool mentioned

# Verify verbosity levels documented
grep -E "compact|standard|verbose" .claude/skills/memory-inject/SKILL.md
# Should show all three levels
```
