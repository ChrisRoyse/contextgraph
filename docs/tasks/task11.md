# Task 11: Create semantic-search SKILL.md

## Metadata
- **Task ID**: TASK-GAP-011
- **Phase**: 3 (Skills Framework)
- **Priority**: Medium
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: task02 (TASK-GAP-003 - directory must exist)

## Objective

Create the semantic-search skill SKILL.md file as specified by PRD Section 9.3. This skill searches the knowledge graph using multi-space retrieval via the `/semantic-search` command. The skill file documents how Claude should use the `search_graph` MCP tool with different search modes.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 3.3.3 for skill specification
- `/home/cabdru/contextgraph/.claude/skills/semantic-search/` - Directory must exist (created in task02)

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/.claude/skills/semantic-search/SKILL.md`

## Implementation Steps

### Step 1: Create SKILL.md

Create the skill file with proper frontmatter and content following the Claude Code skill format.

## Code/Content to Implement

### /home/cabdru/contextgraph/.claude/skills/semantic-search/SKILL.md

```markdown
---
model: haiku
user_invocable: true
---

# Semantic Search

Search the knowledge graph using multi-space retrieval.

**Keywords**: search, find, query, lookup, semantic, causal

## Instructions

When the user wants to search for specific information:

1. Parse the query and optional mode (semantic, causal, code, entity)
2. Call `search_graph` with appropriate parameters
3. Present results ranked by relevance

## MCP Tools

- `search_graph`: Multi-space vector search
  - Parameters:
    - `query` (required): Search text
    - `top_k` (optional): Number of results (default 10)
    - `mode` (optional): "semantic" | "causal" | "code" | "entity"
    - `min_similarity` (optional): Minimum similarity threshold (default 0.3)
  - Returns: `{ results: SearchResult[], mode: string }`

## Mode Behavior

- **semantic** (default): Prioritizes E1 (Semantic) embedder
  - Best for: General meaning-based searches
  - Example: "discussions about user preferences"

- **causal**: Prioritizes E5 (Causal) embedder for "why/because" queries
  - Best for: Understanding cause-effect relationships
  - Example: "why did we choose this architecture"

- **code**: Prioritizes E7 (Code) embedder for technical content
  - Best for: Finding code snippets, technical implementations
  - Example: "authentication middleware implementation"

- **entity**: Prioritizes E11 (Entity/TransE) embedder for named entities
  - Best for: Finding specific things by name
  - Example: "references to UserService class"

## Output Format

```
Search Results (N found):

1. [Content preview]
   - Relevance: 0.XX
   - Source: HookDescription | Created: 2024-01-15
   - Dominant Embedder: E7_Code

2. [Content preview]
   - Relevance: 0.XX
   - Source: ClaudeResponse | Created: 2024-01-14
   - Dominant Embedder: E1_Semantic

...
```

## Edge Cases

- **No results**: "No memories match your search criteria."
- **Mode-specific empty**: "No [causal|code|entity] matches found. Try default semantic search."
- **Low similarity scores**: If all results < 0.5, note "Results have moderate relevance - consider refining query"
- **Empty query**: Prompt user to provide a search query

## Example Usage

User: "Find discussions about authentication"
Response: Call `search_graph({query: "authentication", mode: "semantic"})` and present results.

User: "Search for code related to error handling"
Response: Call `search_graph({query: "error handling", mode: "code"})` for technical results.

User: "Why did we decide to use PostgreSQL?"
Response: Call `search_graph({query: "PostgreSQL decision rationale", mode: "causal"})` for causal context.

User: "Find references to the ConfigService"
Response: Call `search_graph({query: "ConfigService", mode: "entity"})` for entity-based results.
```

## Definition of Done

- [ ] File exists at `/home/cabdru/contextgraph/.claude/skills/semantic-search/SKILL.md`
- [ ] Frontmatter contains `model: haiku` (per PRD Section 9.3)
- [ ] Frontmatter contains `user_invocable: true`
- [ ] Keywords documented: search, find, query, lookup, semantic, causal
- [ ] MCP tool documented: search_graph
- [ ] All 4 modes documented (semantic, causal, code, entity)
- [ ] Edge cases documented (no results, mode-specific empty)
- [ ] File is valid Markdown

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file exists
test -f .claude/skills/semantic-search/SKILL.md && echo "SKILL.md exists"

# Verify frontmatter has correct model (haiku)
head -5 .claude/skills/semantic-search/SKILL.md
# Should show:
# ---
# model: haiku
# user_invocable: true
# ---

# Verify keywords are documented
grep -i "keywords" .claude/skills/semantic-search/SKILL.md
# Should show: search, find, query, lookup, semantic, causal

# Verify MCP tool is documented
grep "search_graph" .claude/skills/semantic-search/SKILL.md
# Should show tool mentioned

# Verify all modes documented
grep -E "semantic|causal|code|entity" .claude/skills/semantic-search/SKILL.md | wc -l
# Should show multiple lines
```
