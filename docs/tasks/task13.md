# Task 13: Create curation SKILL.md

## Metadata
- **Task ID**: TASK-GAP-013
- **Phase**: 3 (Skills Framework)
- **Priority**: Medium
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: task02 (TASK-GAP-003 - directory must exist)

## Objective

Create the curation skill SKILL.md file as specified by PRD Section 9.3. This skill manages knowledge graph curation by merging, forgetting, or boosting memories via the `/curation` command. The skill file documents how Claude should use the curation MCP tools.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 3.3.5 for skill specification
- `/home/cabdru/contextgraph/.claude/skills/curation/` - Directory must exist (created in task02)
- `/home/cabdru/contextgraph/CLAUDE.md` - SEC-06 soft delete requirement

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/.claude/skills/curation/SKILL.md`

## Implementation Steps

### Step 1: Create SKILL.md

Create the skill file with proper frontmatter and content following the Claude Code skill format.

## Code/Content to Implement

### /home/cabdru/contextgraph/.claude/skills/curation/SKILL.md

```markdown
---
model: sonnet
user_invocable: true
---

# Knowledge Curation

Curate the knowledge graph by merging, forgetting, or boosting memories.

**Keywords**: curate, merge, forget, annotate, prune, duplicate

## Instructions

When the user wants to curate knowledge:

1. Call `get_memetic_status` to see pending curation tasks
2. Based on user intent:
   - **merge**: Call `merge_concepts` with source IDs
   - **forget**: Call `forget_concept` with memory ID (soft delete by default)
   - **boost**: Call `boost_importance` with memory ID and delta
3. Confirm action results

## MCP Tools

- `get_memetic_status`: Get curation task suggestions
  - Returns includes `{ curation_tasks: CurationTask[] }`

- `merge_concepts`: Merge duplicate memories
  - Parameters:
    - `source_node_ids` (required): Array of UUIDs to merge
    - `merge_strategy` (optional): "keep_newest" | "combine" (default "combine")
  - Returns: `{ merged_id: UUID, sources_removed: u32 }`

- `forget_concept`: Soft-delete a memory (30-day recovery per SEC-06)
  - Parameters:
    - `node_id` (required): UUID of memory to forget
    - `soft_delete` (optional): Use soft delete (default true)
  - Returns: `{ forgotten_id: UUID, recoverable_until: DateTime }`

- `boost_importance`: Adjust memory importance
  - Parameters:
    - `node_id` (required): UUID of memory
    - `delta` (required): Importance change (-1.0 to 1.0)
  - Returns: `{ node_id: UUID, old_importance: f32, new_importance: f32 }`

## Curation Task Types

### Suggested Merges
When memories have > 90% similarity, the system suggests merging:
```
Suggested Merge:
- Memory A: "[content preview]" (ID: xxx)
- Memory B: "[content preview]" (ID: yyy)
- Similarity: 0.95
Action: merge_concepts({source_node_ids: ["xxx", "yyy"]})
```

### Low-Access Candidates
Memories with low access counts may be candidates for forgetting:
```
Low-Access Memory:
- Content: "[preview]" (ID: xxx)
- Last accessed: 30 days ago
- Access count: 2
Action: Consider forget_concept({node_id: "xxx"})
```

### Importance Adjustments
Manually boost important or demote less useful memories:
```
Importance Boost:
- Memory: "[preview]" (ID: xxx)
- Current importance: 0.5
- Adjustment: +0.2 (for frequently useful content)
Action: boost_importance({node_id: "xxx", delta: 0.2})
```

## Output Format

### Curation Tasks
```
Pending Curation Tasks:

Suggested Merges (N):
1. [Memory A] + [Memory B] - similarity: 0.95
   IDs: xxx, yyy

Review Candidates (N):
2. [Memory] - low access (2x in 30 days)
   ID: zzz

To act: "merge 1" or "forget 2" or "boost ID +0.2"
```

### Action Results
```
[Action] completed:
- [Details of change]
- [Recovery info if applicable]
```

## Soft Delete Recovery (SEC-06)

Per constitution SEC-06, soft-deleted memories are recoverable for 30 days:
- Default behavior is soft delete
- Returns `recoverable_until` timestamp
- Hard delete requires explicit `soft_delete: false`

**WARNING**: Hard delete is irreversible. Always confirm before hard delete.

## Edge Cases

- **Invalid UUID**: "Memory ID [X] not found"
- **Already deleted**: "Memory [X] already soft-deleted. Recoverable until [date]"
- **Importance bounds**: Values automatically clamped to [0.0, 1.0]
- **Merge single ID**: "Need at least 2 memories to merge"
- **No curation tasks**: "No curation tasks pending - knowledge graph is healthy"

## Example Usage

User: "What curation tasks are pending?"
Response: Call `get_memetic_status({})` and present curation_tasks list.

User: "Merge these duplicate memories"
Response: Call `merge_concepts({source_node_ids: ["id1", "id2"]})` and confirm result.

User: "Forget this memory"
Response: Call `forget_concept({node_id: "xxx"})` - soft delete with 30-day recovery.

User: "This memory is really important, boost it"
Response: Call `boost_importance({node_id: "xxx", delta: 0.3})` and show old/new values.

User: "Permanently delete this memory"
Response: Confirm intent, then call `forget_concept({node_id: "xxx", soft_delete: false})`.
```

## Definition of Done

- [ ] File exists at `/home/cabdru/contextgraph/.claude/skills/curation/SKILL.md`
- [ ] Frontmatter contains `model: sonnet` (per PRD Section 9.3)
- [ ] Frontmatter contains `user_invocable: true`
- [ ] Keywords documented: curate, merge, forget, annotate, prune, duplicate
- [ ] MCP tools documented: get_memetic_status, merge_concepts, forget_concept, boost_importance
- [ ] Soft delete recovery documented (30 days per SEC-06)
- [ ] Edge cases documented
- [ ] File is valid Markdown

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file exists
test -f .claude/skills/curation/SKILL.md && echo "SKILL.md exists"

# Verify frontmatter has correct model (sonnet)
head -5 .claude/skills/curation/SKILL.md
# Should show:
# ---
# model: sonnet
# user_invocable: true
# ---

# Verify keywords are documented
grep -i "keywords" .claude/skills/curation/SKILL.md
# Should show: curate, merge, forget, annotate, prune, duplicate

# Verify MCP tools are documented
grep "merge_concepts\|forget_concept\|boost_importance\|get_memetic_status" \
    .claude/skills/curation/SKILL.md
# Should show all tools mentioned

# Verify SEC-06 soft delete documented
grep -i "30.*day\|SEC-06" .claude/skills/curation/SKILL.md
# Should show soft delete recovery info
```
