# Task 09: Create topic-explorer SKILL.md

## Metadata
- **Task ID**: TASK-GAP-009
- **Phase**: 3 (Skills Framework)
- **Priority**: Medium
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: task02 (TASK-GAP-003 - directory must exist)

## Objective

Create the topic-explorer skill SKILL.md file as specified by PRD Section 9.3. This skill enables users to explore the emergent topic portfolio via the `/topic-explorer` command. The skill file documents how Claude should use the `get_topic_portfolio` and `get_topic_stability` MCP tools.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 3.3.1 for skill specification
- `/home/cabdru/contextgraph/.claude/skills/topic-explorer/` - Directory must exist (created in task02)

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/.claude/skills/topic-explorer/SKILL.md`

**Files to Delete:**
- `/home/cabdru/contextgraph/.claude/skills/topic-explorer/.gitkeep` (optional, can keep)

## Implementation Steps

### Step 1: Create SKILL.md

Create the skill file with proper frontmatter and content following the Claude Code skill format.

## Code/Content to Implement

### /home/cabdru/contextgraph/.claude/skills/topic-explorer/SKILL.md

```markdown
---
model: sonnet
user_invocable: true
---

# Topic Explorer

Explore emergent topic portfolio, topic stability metrics, and weighted agreement scores.

**Keywords**: topics, portfolio, stability, churn, weighted agreement

## Instructions

When the user asks about topics, topic stability, or the knowledge graph structure:

1. Call `get_topic_portfolio` to retrieve current topics
2. If user asks about stability, also call `get_topic_stability`
3. Format the response showing:
   - Topic names/IDs and their confidence scores
   - Contributing embedding spaces (semantic weight 1.0, relational 0.5, structural 0.5)
   - Current phase (Emerging, Stable, Declining, Merging)
   - If stability requested: churn rate, entropy, dream recommendation

## MCP Tools

- `get_topic_portfolio`: Get all discovered topics with profiles
  - Parameters: `format` (optional): "brief" | "standard" | "verbose"
  - Returns: `{ topics: Topic[], stability: StabilityMetrics }`

- `get_topic_stability`: Get portfolio-level stability metrics
  - Parameters: `hours` (optional): lookback period (default 6)
  - Returns: `{ churn_rate: f32, entropy: f32, phases: PhaseBreakdown, dream_recommended: bool }`

## Output Format

### Brief Format
```
Topics (N discovered):
1. [Topic Name] - confidence: X.XX, phase: Stable
2. [Topic Name] - confidence: X.XX, phase: Emerging
...
Stability: churn=0.XX, entropy=0.XX
```

### Standard Format
Include contributing spaces and member counts:
```
Topics (N discovered):

1. [Topic Name]
   - Confidence: X.XX (weighted_agreement: X.X/8.5)
   - Members: N memories
   - Contributing: E1_Semantic, E5_Causal, E7_Code
   - Phase: Stable

2. [Topic Name]
   ...

Portfolio Stability:
- Churn Rate: 0.XX (healthy < 0.3)
- Entropy: 0.XX
- Phase Distribution: N emerging, N stable, N declining
```

## Edge Cases

- **No topics discovered**: "No topics discovered yet. Topics emerge when memories cluster in 3+ semantic spaces (weighted agreement >= 2.5)."
- **Tier 0 (0 memories)**: "System at Tier 0 - no memories stored yet."
- **High churn warning**: If churn > 0.5, note "High churn detected - consider running /dream-consolidation"
- **Dream recommended**: If entropy > 0.7 AND churn > 0.5, suggest "Dream consolidation recommended to stabilize topics"

## Example Usage

User: "What topics have emerged?"
Response: Call `get_topic_portfolio({format: "standard"})` and present the results.

User: "Is my topic structure stable?"
Response: Call `get_topic_stability({hours: 6})` and interpret the metrics.

User: "Show me a brief overview of topics"
Response: Call `get_topic_portfolio({format: "brief"})` for compact output.
```

## Definition of Done

- [ ] File exists at `/home/cabdru/contextgraph/.claude/skills/topic-explorer/SKILL.md`
- [ ] Frontmatter contains `model: sonnet` (per PRD Section 9.3)
- [ ] Frontmatter contains `user_invocable: true`
- [ ] Keywords documented: topics, portfolio, stability, churn, weighted agreement
- [ ] MCP tools documented: get_topic_portfolio, get_topic_stability
- [ ] Output formats documented (brief, standard)
- [ ] Edge cases documented (no topics, Tier 0, high churn)
- [ ] File is valid Markdown

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file exists
test -f .claude/skills/topic-explorer/SKILL.md && echo "SKILL.md exists"

# Verify frontmatter has correct model
head -5 .claude/skills/topic-explorer/SKILL.md
# Should show:
# ---
# model: sonnet
# user_invocable: true
# ---

# Verify keywords are documented
grep -i "keywords" .claude/skills/topic-explorer/SKILL.md
# Should show: topics, portfolio, stability, churn, weighted agreement

# Verify MCP tools are documented
grep "get_topic_portfolio\|get_topic_stability" .claude/skills/topic-explorer/SKILL.md
# Should show both tools mentioned

# Verify edge cases section exists
grep -A5 "Edge Cases" .claude/skills/topic-explorer/SKILL.md
# Should show edge case documentation
```
