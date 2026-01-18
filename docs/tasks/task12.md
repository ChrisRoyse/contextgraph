# Task 12: Create dream-consolidation SKILL.md

## Metadata
- **Task ID**: TASK-GAP-012
- **Phase**: 3 (Skills Framework)
- **Priority**: Medium
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: task02 (TASK-GAP-003 - directory must exist)

## Objective

Create the dream-consolidation skill SKILL.md file as specified by PRD Section 9.3. This skill triggers memory consolidation via NREM and REM dream phases via the `/dream-consolidation` command. The skill file documents how Claude should use the `get_memetic_status` and `trigger_consolidation` MCP tools.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 3.3.4 for skill specification
- `/home/cabdru/contextgraph/.claude/skills/dream-consolidation/` - Directory must exist (created in task02)
- `/home/cabdru/contextgraph/CLAUDE.md` - Dream layer specification for constitution compliance

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/.claude/skills/dream-consolidation/SKILL.md`

## Implementation Steps

### Step 1: Create SKILL.md

Create the skill file with proper frontmatter and content following the Claude Code skill format.

## Code/Content to Implement

### /home/cabdru/contextgraph/.claude/skills/dream-consolidation/SKILL.md

```markdown
---
model: sonnet
user_invocable: true
---

# Dream Consolidation

Trigger memory consolidation via NREM and REM dream phases.

**Keywords**: dream, consolidate, nrem, rem, blind spots, entropy, churn

## Instructions

When the user requests consolidation or system metrics indicate need:

1. Call `get_memetic_status` to check current entropy and churn
2. Evaluate if consolidation is recommended (entropy > 0.7 AND churn > 0.5 per AP-70)
3. If recommended or user insists, call `trigger_consolidation`
4. Report results including blind spots discovered

## MCP Tools

- `get_memetic_status`: Get system health and metrics
  - Returns: `{ entropy: f32, coherence: f32, churn: f32, layers: LayerStatus }`

- `trigger_consolidation`: Execute dream cycle
  - Parameters:
    - `blocking` (optional): Wait for completion (default true)
    - `dry_run` (optional): Show what would happen without executing (default false)
  - Returns: `{ nrem_result: NREMResult, rem_result: REMResult, blind_spots: BlindSpot[] }`

## Dream Phases

### NREM (Non-REM)
- **Duration**: ~3 minutes
- **Purpose**: Hebbian learning replay
- **Formula**: Delta_w_ij = eta x phi_i x phi_j for high-importance edges
- **Effect**: Strengthens frequently-used connections
- **Parameters**: learning_rate=0.01, weight_decay=0.001, recency_bias=0.8

### REM
- **Duration**: ~2 minutes
- **Purpose**: Blind spot discovery via hyperbolic random walk
- **Model**: Poincare ball (curvature=-1.0, dimensions=64)
- **Effect**: Discovers unexpected connections between distant concepts
- **Blind Spot Criteria**: min_semantic_distance=0.7, require_shared_causal=true

## Output Format

```
Dream Consolidation [completed|dry-run]

Pre-Dream Metrics:
- Entropy: 0.XX (threshold: 0.7)
- Churn: 0.XX (threshold: 0.5)
- Status: [Recommended|Not needed but requested]

NREM Phase (3 min):
- Edges strengthened: N
- Weight adjustments: +X.XX avg increase
- High-importance replays: N

REM Phase (2 min):
- Blind spots discovered: N
- New connections: [list of discovered links]
- Exploration depth: N hops

Post-Dream Metrics:
- Entropy: 0.XX (change: -0.XX)
- Coherence: 0.XX (change: +0.XX)

Recommendation: [next steps or "System stable"]
```

## Trigger Conditions (per AP-70)

Dream consolidation is recommended when BOTH conditions are met:
- Entropy > 0.7 (topic distribution is highly scattered)
- Churn > 0.5 (topic membership is unstable)

If only one condition is met, consolidation may still help but is not critical.

## Edge Cases

- **Not recommended**: "Current metrics (entropy=X.XX, churn=X.XX) don't indicate consolidation need. Proceed anyway?"
- **Dream in progress**: "Dream cycle already in progress. Status: [NREM|REM], ETA: X seconds"
- **Dry run**: Show projected effects without execution
- **No blind spots found**: "REM exploration complete - no blind spots discovered (knowledge graph is well-connected)"
- **Low memory count**: "Insufficient memories for meaningful consolidation (need 10+ for reliable patterns)"

## Example Usage

User: "Should I run dream consolidation?"
Response: Call `get_memetic_status({})`, check entropy and churn against thresholds, advise accordingly.

User: "Run dream consolidation"
Response: Call `get_memetic_status({})` first, then `trigger_consolidation({blocking: true})` and report results.

User: "What would happen if I run consolidation?"
Response: Call `trigger_consolidation({dry_run: true})` to preview effects.

User: "My knowledge seems fragmented"
Response: Check `get_memetic_status({})` - if entropy high, suggest consolidation.
```

## Definition of Done

- [ ] File exists at `/home/cabdru/contextgraph/.claude/skills/dream-consolidation/SKILL.md`
- [ ] Frontmatter contains `model: sonnet` (per PRD Section 9.3)
- [ ] Frontmatter contains `user_invocable: true`
- [ ] Keywords documented: dream, consolidate, nrem, rem, blind spots, entropy, churn
- [ ] MCP tools documented: get_memetic_status, trigger_consolidation
- [ ] Dream phases documented (NREM, REM) with duration and purpose
- [ ] Trigger conditions documented (entropy > 0.7 AND churn > 0.5 per AP-70)
- [ ] Edge cases documented
- [ ] File is valid Markdown

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file exists
test -f .claude/skills/dream-consolidation/SKILL.md && echo "SKILL.md exists"

# Verify frontmatter has correct model (sonnet)
head -5 .claude/skills/dream-consolidation/SKILL.md
# Should show:
# ---
# model: sonnet
# user_invocable: true
# ---

# Verify keywords are documented
grep -i "keywords" .claude/skills/dream-consolidation/SKILL.md
# Should show: dream, consolidate, nrem, rem, blind spots, entropy, churn

# Verify MCP tools are documented
grep "get_memetic_status\|trigger_consolidation" .claude/skills/dream-consolidation/SKILL.md
# Should show both tools mentioned

# Verify trigger conditions documented (AP-70)
grep -i "0.7\|0.5" .claude/skills/dream-consolidation/SKILL.md
# Should show entropy and churn thresholds
```
