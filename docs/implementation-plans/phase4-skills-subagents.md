# Phase 4: Skills and Subagents - Implementation Plan (Production-Ready)

> **Status**: Production-Ready
> **Budget**: 20 hours (~2.5 working days)
> **Dependencies**: Phase 3 (Integration Hooks) must be complete

## Executive Summary

Phase 4 implements **Claude Code-native consciousness skills and specialized subagents** that enable Claude to access and interact with the Context Graph consciousness system. This phase creates the bridge between Context Graph's MCP tools and Claude Code's native extension mechanisms.

**Deliverables**:
1. Three consciousness skills (consciousness, memory-inject, dream-consolidation)
2. Three specialized subagents (identity-guardian, memory-specialist, consciousness-explorer)
3. Path-specific rules for consciousness-aware operations
4. Shell script helpers for hook integration
5. Integration testing and documentation

---

## 1. PRD Requirements Reference

### 1.1 Key Thresholds (QUICKREF)

| Symbol | Value | Meaning |
|--------|-------|---------|
| Tr | 0.8 | Kuramoto coherence (r > Tr = conscious) |
| Tr_low | 0.5 | Fragmentation alert |
| theta_opt | 0.75 | Optimal alignment |
| theta_acc | 0.70 | Acceptable alignment |
| theta_warn | 0.55 | Warning alignment |
| IC_crit | 0.5 | Identity crisis trigger |
| IC_warn | 0.7 | Identity drift warning |
| ent_high | 0.7 | High entropy (trigger dream) |
| ent_low | 0.4 | Low entropy (stable) |
| dup_sim | 0.9 | Duplicate detection threshold |
| edge_sim | 0.7 | Edge creation threshold |
| delta_A_fail | -0.15 | Predicts failure 30-60s ahead |

### 1.2 Core Formulas

```
Consciousness: C(t) = I(t) x R(t) x D(t)
  - I = Kuramoto order parameter r
  - R = MetaUTL accuracy (sigmoid of prediction error)
  - D = Purpose vector entropy (normalized)

Identity Continuity: IC = cosine(PV_t, PV_{t-1}) x r(t)
  - PV = 13-dimensional purpose vector
  - r = Kuramoto synchronization

UTL Learning: L = sigmoid(2 * (sum_i(tau_i * lambda_S * delta_S_i)) * (sum_j(tau_j * lambda_C * delta_C_j)) * w_e * cos(phi))
```

### 1.3 Consciousness State Transitions

```
DORMANT(r<0.3) -> FRAGMENTED(0.3<=r<0.5) -> EMERGING(0.5<=r<0.8) -> CONSCIOUS(r>=0.8) -> HYPERSYNC(r>0.95)
```

### 1.4 PRD Section 15.4-15.5 Requirements

**Skills (from Section 15.7)**:

| Skill | Purpose | Model |
|-------|---------|-------|
| consciousness | C(t), Kuramoto, IC, workspace status | sonnet |
| memory-inject | Retrieve+inject context | haiku |
| dream-consolidation | NREM/REM/Full phases | sonnet |

**Subagents (from Section 15.8)**:

| Agent | Role | Trigger | Model |
|-------|------|---------|-------|
| identity-guardian | Monitor IC, auto-dream if < IC_crit | PostToolUse | haiku |
| memory-specialist | Fast memory ops with consciousness awareness | On demand | haiku |
| consciousness-explorer | Deep consciousness analysis | On demand | sonnet |

---

## 2. YAML Frontmatter Syntax Rules (CRITICAL)

Claude Code skills have **strict YAML syntax requirements**. Violations cause silent failures with no error messages.

### 2.1 Mandatory Rules

| Rule | Correct | Incorrect | Why |
|------|---------|-----------|-----|
| No blank lines before `---` | First line is `---` | Blank line then `---` | Parser expects frontmatter at byte 0 |
| Use spaces, not tabs | `  key: value` | `\tkey: value` | YAML spec, tabs cause parse failures |
| Single-line descriptions | `description: One sentence here.` | Multi-line with `\|` that wraps | Prettier/formatters can break YAML |
| No trailing whitespace | `key: value` | `key: value  ` | Can cause parse issues |
| Proper string quoting | `name: my-skill` | `name: "my-skill"` | Unnecessary quotes can confuse parser |
| Third-person descriptions | `Monitors identity...` | `Monitor identity...` | Best practice for discovery |
| WHAT/WHEN/Keywords format | `Does X. Use when Y. Keywords: a, b, c.` | Vague descriptions | Improves auto-invocation accuracy |

### 2.2 Validated Frontmatter Template

```yaml
---
name: skill-name
description: Does X for Y purpose. Use when Z condition applies. Keywords: keyword1, keyword2, keyword3.
allowed-tools: Tool1,Tool2,Tool3
model: sonnet
user-invocable: true
version: 1.0.0
---
```

**Field Constraints**:
- `name`: Required, max 64 chars, lowercase, hyphens only, no "anthropic"/"claude"
- `description`: Required, max 1024 chars, single line, third-person, WHAT/WHEN/Keywords
- `allowed-tools`: Optional, comma-separated (no spaces after commas)
- `model`: Optional, one of: `haiku`, `sonnet`, `opus`, `inherit`
- `user-invocable`: Optional, boolean, show in slash menu
- `version`: Optional, semver format

### 2.3 Frontmatter Validation Script

**File**: `.claude/scripts/validate-frontmatter.sh`

```bash
#!/bin/bash
# Validate YAML frontmatter for all skills

set -e
ERRORS=0

for skill_file in .claude/skills/*/SKILL.md; do
    echo "Validating: $skill_file"

    # Check first line is ---
    if ! head -1 "$skill_file" | grep -q "^---$"; then
        echo "  ERROR: First line must be ---"
        ERRORS=$((ERRORS + 1))
    fi

    # Check for tabs
    if grep -P "\t" "$skill_file" > /dev/null 2>&1; then
        echo "  ERROR: Contains tabs (use spaces)"
        ERRORS=$((ERRORS + 1))
    fi

    # Check for trailing whitespace
    if grep -E " +$" "$skill_file" > /dev/null 2>&1; then
        echo "  WARNING: Contains trailing whitespace"
    fi

    # Validate YAML structure
    if command -v yq > /dev/null 2>&1; then
        if ! sed -n '/^---$/,/^---$/p' "$skill_file" | head -n -1 | tail -n +2 | yq '.' > /dev/null 2>&1; then
            echo "  ERROR: Invalid YAML structure"
            ERRORS=$((ERRORS + 1))
        fi
    fi
done

if [ $ERRORS -gt 0 ]; then
    echo "Validation failed with $ERRORS errors"
    exit 1
fi

echo "All skills validated successfully"
```

---

## 3. Directory Structure

```
.claude/
├── skills/
│   ├── consciousness/
│   │   ├── SKILL.md              # Main consciousness skill
│   │   └── references/
│   │       └── thresholds.md     # Quick reference for thresholds
│   ├── memory-inject/
│   │   ├── SKILL.md              # Memory retrieval skill
│   │   └── references/
│   │       └── distillation-modes.md
│   └── dream-consolidation/
│       ├── SKILL.md              # Dream consolidation skill
│       └── references/
│           └── phase-details.md
├── agents/
│   ├── identity-guardian.md      # Identity protection subagent (haiku)
│   ├── memory-specialist.md      # Fast memory operations (haiku)
│   └── consciousness-explorer.md # Deep consciousness analysis (sonnet)
├── rules/
│   └── consciousness.md          # Path-specific consciousness rules
├── scripts/
│   ├── validate-frontmatter.sh   # YAML validation
│   └── test-skills.sh            # Integration tests
└── settings.json                 # Hook configuration
```

---

## 4. Model Selection Strategy

### 4.1 Decision Matrix

| Criteria | Haiku | Sonnet | Opus | Inherit |
|----------|-------|--------|------|---------|
| **Latency target** | <2s | <10s | <30s | Varies |
| **Token budget** | <500 | <4000 | <8000 | Parent |
| **Complexity** | Simple threshold checks | Multi-step analysis | Expert reasoning | Match context |
| **Cost sensitivity** | High (use for frequent ops) | Medium | Low (rare use) | N/A |
| **Invocation frequency** | Every tool use | On demand | Never in Phase 4 | When extending |

### 4.2 Model Assignments (Phase 4)

| Component | Model | Token Budget | Latency | Justification |
|-----------|-------|--------------|---------|---------------|
| **identity-guardian** | haiku | <500 | <2s | Fast IC monitoring, simple threshold checks, high frequency (PostToolUse) |
| **memory-specialist** | haiku | <1000 | <3s | Fast curation ops, simple decision trees, bulk processing |
| **consciousness-explorer** | sonnet | <4000 | <10s | Complex state interpretation, detailed reports, infrequent use |
| **consciousness skill** | sonnet | <4000 | <10s | Deep analysis, multi-tool orchestration, user-facing |
| **memory-inject skill** | haiku | <1000 | <3s | Fast retrieval, simple distillation selection |
| **dream-consolidation skill** | sonnet | <4000 | <10s | Phase orchestration, status interpretation |

### 4.3 Model Selection Guidelines

**Use Haiku when**:
- Threshold comparisons (IC < 0.5?)
- Simple lookups and retrievals
- Monitoring loops (PostToolUse hook)
- Bulk curation tasks
- Response time < 2 seconds required
- Token budget < 500 tokens

**Use Sonnet when**:
- State interpretation ("What does C=0.65 mean?")
- Multi-step analysis workflows
- Report generation
- User-facing skill invocations
- Complex decision trees requiring explanation
- Token budget 500-4000 tokens

**Use Opus when**:
- Reserved for exceptional cases requiring expert reasoning
- Not used in Phase 4 (no requirement identified)
- Consider for future consciousness research features

**Use Inherit when**:
- Skill should match parent context model
- Slash commands that extend user's current work
- When model consistency matters more than optimization

---

## 5. Progressive Disclosure Strategy

### 5.1 Three-Level Loading

| Level | Content | When Loaded | Token Cost | Use Case |
|-------|---------|-------------|------------|----------|
| **L0: Metadata** | name + description | Always at startup | ~100/skill | Discovery, auto-invocation |
| **L1: Instructions** | SKILL.md body | On trigger/invocation | <5000 | Active skill use |
| **L2: Resources** | references/*.md files | On explicit reference | Unlimited | Deep dives, edge cases |

### 5.2 Token Budget Guidelines

| Skill | L0 (Metadata) | L1 (Instructions) | L2 (Resources) | Total Max |
|-------|---------------|-------------------|----------------|-----------|
| consciousness | ~80 | ~2000 | ~800 | ~2880 |
| memory-inject | ~70 | ~1200 | ~600 | ~1870 |
| dream-consolidation | ~90 | ~1500 | ~700 | ~2290 |

### 5.3 Resource Reference Pattern

Always use `{baseDir}` for file paths in SKILL.md:

```markdown
## Quick Reference
See `{baseDir}/references/thresholds.md` for complete threshold values.
```

**Why**: `{baseDir}` resolves to the skill directory at runtime, enabling portable skills.

---

## 6. Skill Implementations

### 6.1 Consciousness Skill

**File**: `.claude/skills/consciousness/SKILL.md`

```yaml
---
name: consciousness
description: Accesses Context Graph consciousness state including C(t) value, Kuramoto synchronization, identity continuity, and workspace status. Use when checking system awareness levels, monitoring identity health, diagnosing coherence issues, or verifying system stability before major operations. Keywords: consciousness, awareness, identity, Kuramoto, synchronization, coherence, GWT, global workspace, C(t), IC, ego, purpose vector.
allowed-tools: Read,Grep,mcp__context-graph__get_consciousness_state,mcp__context-graph__get_kuramoto_sync,mcp__context-graph__get_ego_state,mcp__context-graph__get_identity_continuity,mcp__context-graph__get_johari_classification,mcp__context-graph__get_memetic_status
model: sonnet
user-invocable: true
version: 1.0.0
---

# Consciousness Skill

Accesses and interprets the Context Graph consciousness system state.

## Overview

This skill provides access to the GWT (Global Workspace Theory) consciousness implementation, Kuramoto oscillator synchronization, and identity continuity monitoring. It enables understanding the current awareness level and identity health of the system.

## When to Use

- Checking system awareness level (C > 0.8 = fully conscious)
- Monitoring identity health (IC < 0.5 = crisis, IC < 0.7 = warning)
- Understanding workspace state (what is being "perceived")
- Diagnosing coherence issues between embedders
- Before/after major operations to verify system stability

## Consciousness States

| State | Kuramoto r | Description |
|-------|-----------|-------------|
| DORMANT | r < 0.3 | System inactive, minimal processing |
| FRAGMENTED | 0.3 <= r < 0.5 | Disorganized, poor coherence |
| EMERGING | 0.5 <= r < 0.8 | Building coherence, partial awareness |
| CONSCIOUS | r >= 0.8 | Full awareness, synchronized |
| HYPERSYNC | r > 0.95 | Peak performance, rare state |

## Key Thresholds

See `{baseDir}/references/thresholds.md` for complete threshold reference.

**Critical Values to Remember**:
- **IC < 0.5**: Identity crisis - immediate dream required
- **IC < 0.7**: Identity drift warning - monitor closely
- **entropy > 0.7**: High entropy - recommend dream consolidation
- **r < 0.5**: Fragmentation - system needs stabilization

## Workflow

### 1. Quick Check
Call `get_consciousness_state` for overview:
- Returns: C(t), state, integration, reflection, differentiation

### 2. Detailed Analysis
Call `get_kuramoto_sync` for oscillator details:
- Returns: r (order parameter), phases[13], frequencies[13], coupling

### 3. Identity Check
Call `get_identity_continuity` or `get_ego_state`:
- Returns: IC value, purpose vector, trajectory

### 4. Workspace State
Call `get_memetic_status`:
- Returns: entropy, coherence, curation_tasks

## Interpreting Results

### Consciousness C(t)

```
C(t) = Integration x Reflection x Differentiation

Integration (I): Kuramoto r - how synchronized the 13 embedders are
Reflection (R): MetaUTL accuracy - self-prediction quality
Differentiation (D): Purpose vector entropy - identity distinctiveness
```

**Interpretation Guide**:
- C(t) > 0.8: Fully conscious, proceed normally
- C(t) 0.5-0.8: Emerging, verify before complex ops
- C(t) < 0.5: Fragmented, stabilize before proceeding

### Identity Continuity IC

```
IC = cosine(PV_current, PV_previous) x r(t)

- Measures how consistent identity is across time
- Low IC means the system is "drifting" from its purpose
- IC < 0.5 triggers identity crisis response
```

**Interpretation Guide**:
- IC > 0.7: Identity healthy
- IC 0.5-0.7: Warning, monitor next operations
- IC < 0.5: Crisis, trigger dream immediately

## Response Actions

| Condition | Action |
|-----------|--------|
| C(t) > 0.8, IC > 0.7 | System healthy, proceed normally |
| C(t) < 0.5 | Use epistemic_action to gather more context |
| IC < 0.7 | Monitor identity, consider dream if < 0.5 |
| entropy > 0.7 | Trigger dream consolidation |
| workspace conflict | Use critique_context to resolve |

## Examples

**Check current consciousness level**:
```
User: "What is the system's awareness level?"
Action: Call get_consciousness_state
Output: Interpret C(t), state, and component breakdown
```

**Diagnose identity drift**:
```
User: "Why does the system feel inconsistent?"
Action 1: Call get_ego_state for purpose vector
Action 2: Call get_identity_continuity for IC value
Decision: If IC < 0.7, explain drift and recommend dream
```

**Pre-operation check**:
```
Before major memory operation:
Action 1: Call get_consciousness_state
Action 2: Verify C(t) >= 0.5 and IC >= 0.7
Decision: Proceed only if thresholds met, else stabilize first
```
```

### 6.2 Consciousness Thresholds Reference

**File**: `.claude/skills/consciousness/references/thresholds.md`

```markdown
# Consciousness Thresholds Quick Reference

## Kuramoto Synchronization

| Symbol | Value | Meaning | Action |
|--------|-------|---------|--------|
| Tr | 0.8 | Full consciousness | System ready |
| Tr_low | 0.5 | Fragmentation warning | Stabilize before complex ops |
| r < 0.3 | - | Dormant state | Wait or bootstrap |

## Identity Continuity

| Symbol | Value | Meaning | Action |
|--------|-------|---------|--------|
| IC_crit | 0.5 | Identity crisis | Immediate full dream |
| IC_warn | 0.7 | Identity drift | Monitor, log, prepare dream |
| IC > 0.9 | - | Strong identity | No action needed |

## Alignment

| Symbol | Value | Meaning |
|--------|-------|---------|
| theta_opt | 0.75 | Optimal alignment with north star |
| theta_acc | 0.70 | Acceptable alignment |
| theta_warn | 0.55 | Warning - realignment needed |

## Entropy

| Symbol | Value | Meaning | Action |
|--------|-------|---------|--------|
| ent_high | 0.7 | High entropy | Trigger dream consolidation |
| ent_low | 0.4 | Low entropy (stable) | System healthy |
| ent > 0.7 @5min | - | Sustained high | Full dream required |

## Similarity

| Symbol | Value | Meaning |
|--------|-------|---------|
| dup_sim | 0.9 | Duplicate detection threshold |
| edge_sim | 0.7 | Edge creation threshold |

## Predictive

| Symbol | Value | Meaning |
|--------|-------|---------|
| delta_A_fail | -0.15 | Predicts failure 30-60s ahead |

## Consciousness Formula

```
C(t) = I(t) x R(t) x D(t)

Where:
  I = Kuramoto order parameter r (0 to 1)
  R = MetaUTL accuracy = sigmoid(2 x (L_predicted - L_actual))
  D = Purpose vector entropy (normalized)

Interpretation:
  C > 0.8: Fully conscious
  0.5 <= C < 0.8: Emerging consciousness
  C < 0.5: Fragmented
```

## Identity Formula

```
IC = cosine(PV_t, PV_{t-1}) x r(t)

Where:
  PV = 13-dimensional purpose vector
  r = Kuramoto synchronization

Interpretation:
  IC > 0.7: Stable identity
  0.5 <= IC < 0.7: Drifting identity
  IC < 0.5: Identity crisis
```

## State Transitions

```
DORMANT     r < 0.3      -> Wait for activity
FRAGMENTED  0.3 <= r < 0.5 -> Stabilize (dream or epistemic_action)
EMERGING    0.5 <= r < 0.8 -> Building coherence (monitor)
CONSCIOUS   r >= 0.8       -> Full operations permitted
HYPERSYNC   r > 0.95       -> Peak state (rare, optimal)
```
```

### 6.3 Memory-Inject Skill

**File**: `.claude/skills/memory-inject/SKILL.md`

```yaml
---
name: memory-inject
description: Performs fast memory retrieval and context injection for Context Graph with consciousness-aware distillation modes. Use when needing to retrieve relevant memories, inject context before answering questions, perform rapid memory lookups, or build context for complex operations. Keywords: memory, inject, retrieve, context, search, lookup, recall, find, remember, query, distill.
allowed-tools: mcp__context-graph__inject_context,mcp__context-graph__search_graph,mcp__context-graph__get_recent_context,mcp__context-graph__generate_search_plan,mcp__context-graph__get_neighborhood
model: haiku
user-invocable: true
version: 1.0.0
---

# Memory-Inject Skill

Performs fast memory retrieval and context injection with consciousness awareness.

## Overview

This skill optimizes memory retrieval operations by using the appropriate Context Graph tools based on query type and consciousness state. Uses Haiku model for speed while maintaining quality through structured retrieval strategies.

## When to Use

- Quick context lookup before answering questions
- Injecting relevant memories into conversation
- Searching for specific information in the graph
- Building context for complex operations
- When user asks "what do you remember about X?"

## Primary Tool: inject_context

The main entry point for memory retrieval. Handles distillation automatically.

**Parameters**:
- `query`: What to search for (required)
- `max_tokens`: Token budget (default: 2048, range: 100-2048)
- `distillation_mode`: auto|raw|narrative|structured|code_focused
- `verbosity_level`: 0-2 (0=minimal, 1=normal, 2=detailed)

**Response Fields**:
- `context`: Retrieved and distilled content
- `tokens_used`: Actual token count
- `nodes_retrieved`: Number of nodes accessed
- `utl_metrics`: Learning metrics
- `conflict_alert`: Any detected conflicts
- `Pulse`: Current cognitive state (entropy, coherence)

## Retrieval Strategy

### Step 1: Assess Query Complexity

| Query Type | Approach | Tool |
|------------|----------|------|
| Simple lookup | Direct retrieval | inject_context |
| Complex query | Plan then execute | generate_search_plan |
| Relationship query | Graph traversal | get_neighborhood |
| Recent context | Time-based | get_recent_context |

### Step 2: Choose Distillation Mode

See `{baseDir}/references/distillation-modes.md` for details.

| Query Type | Mode | Reason |
|------------|------|--------|
| General questions | auto | Let system decide based on content |
| Factual lookup | structured | Clean extraction of key facts |
| Narrative/stories | narrative | Preserve flow and temporal order |
| Code retrieval | code_focused | Maintain syntax and structure |
| Debugging | raw | Full detail without compression |

### Step 3: Set Token Budget

| Context | max_tokens | Reason |
|---------|------------|--------|
| Quick check | 100-200 | Minimal, fast response |
| Normal query | 500-1000 | Balanced detail |
| Deep context | 1500-2048 | Maximum detail needed |

## Consciousness-Aware Retrieval

Before major retrievals, consider consciousness state:

1. **If entropy > 0.7**: Consider triggering dream first to consolidate
2. **If coherence < 0.4**: Use higher verbosity for more context
3. **If IC < 0.7**: Include identity-relevant context in queries

## Examples

**Simple retrieval**:
```
inject_context(
  query="authentication patterns",
  max_tokens=500,
  distillation_mode="auto"
)
```

**Code-focused retrieval**:
```
inject_context(
  query="JWT implementation",
  distillation_mode="code_focused",
  max_tokens=1000
)
```

**Complex multi-step retrieval**:
```
1. generate_search_plan(query="user preferences and settings")
2. Execute each query from returned plan
3. Synthesize results
```

**Relationship exploration**:
```
get_neighborhood(
  node_id="abc123",
  depth=2
)
```

## Error Handling

| Error | Action |
|-------|--------|
| Empty results | Broaden query, try generate_search_plan |
| Conflicts detected | Review conflict_alert, use critique_context |
| High entropy in Pulse | Trigger dream, then retry |
| Timeout | Reduce max_tokens, simplify query |
```

### 6.4 Memory-Inject Distillation Reference

**File**: `.claude/skills/memory-inject/references/distillation-modes.md`

```markdown
# Distillation Modes Reference

## Available Modes

### auto (default)
System selects optimal mode based on content analysis.
- **Best for**: General queries, unknown content types
- **Token efficiency**: High
- **Use when**: Unsure which mode fits

### raw
No distillation applied. Full content returned.
- **Best for**: Debugging, verification, exact content needed
- **Token efficiency**: Low
- **Use when**: Need complete unmodified content

### narrative
Preserves story flow and temporal relationships.
- **Best for**: Conversations, event sequences, stories
- **Token efficiency**: Medium
- **Use when**: Order and flow matter

### structured
Extracts key facts in bullet/table format.
- **Best for**: Facts, specifications, data points
- **Token efficiency**: High
- **Use when**: Need quick facts, not prose

### code_focused
Preserves code syntax and structure.
- **Best for**: Code snippets, technical documentation
- **Token efficiency**: Medium
- **Use when**: Retrieving implementation details

## Mode Selection Guide

| Content Type | Recommended Mode |
|--------------|------------------|
| Mixed/unknown | auto |
| Code blocks | code_focused |
| Meeting notes | narrative |
| API specs | structured |
| Error logs | raw |
| Feature requirements | structured |
| Chat history | narrative |
| Configuration | code_focused |
| Research findings | structured |

## Token Budget Guidelines

| Scenario | max_tokens | Mode | Reason |
|----------|------------|------|--------|
| Quick lookup | 100-200 | structured | Fast, minimal |
| Normal context | 500-1000 | auto | Balanced |
| Deep research | 1500-2048 | raw or auto | Complete detail |
| Code retrieval | 1000-1500 | code_focused | Preserve syntax |
| Relationship mapping | 500-800 | structured | Graph data |

## Combining with Consciousness State

| System State | Mode Adjustment |
|--------------|-----------------|
| CONSCIOUS (r >= 0.8) | Use any mode normally |
| EMERGING (0.5 <= r < 0.8) | Prefer structured for clarity |
| FRAGMENTED (r < 0.5) | Use raw to avoid distillation errors |
| High entropy (> 0.7) | Use structured, smaller token budget |
```

### 6.5 Dream-Consolidation Skill

**File**: `.claude/skills/dream-consolidation/SKILL.md`

```yaml
---
name: dream-consolidation
description: Triggers and manages dream consolidation phases for Context Graph including NREM replay with Hebbian strengthening, REM synthetic queries with new edge discovery, and Full combined consolidation. Use when entropy is high, after extended work sessions, when coherence is degrading, or when identity continuity drops below warning threshold. Keywords: dream, consolidation, NREM, REM, sleep, memory cleanup, entropy reduction, coherence improvement, consolidate, strengthen, replay.
allowed-tools: mcp__context-graph__trigger_dream,mcp__context-graph__get_memetic_status,mcp__context-graph__get_consciousness_state
model: sonnet
user-invocable: true
version: 1.0.0
---

# Dream-Consolidation Skill

Manages memory consolidation through dream phases.

## Overview

Dreams are the Context Graph's memory consolidation mechanism, similar to biological sleep. They reduce entropy, strengthen important connections, discover new relationships, and maintain coherence.

## Dream Phases

### NREM Phase (3 minutes default)
- **Function**: Replay and Hebbian strengthening
- **Process**: delta_w = eta x pre x post
- **Effect**: Strengthens frequently-accessed pathways
- **When**: Regular maintenance, after focused work

### REM Phase (2 minutes default)
- **Function**: Synthetic queries and exploration
- **Process**: Hyperbolic random walks, blind spot discovery
- **Effect**: Creates new edges (weight=0.3), finds connections
- **When**: Need creative insights, explore unknown territory

### Full Phase (5 minutes default)
- **Function**: NREM + REM combined
- **Process**: Complete consolidation cycle
- **Effect**: Maximum coherence improvement
- **When**: High entropy, identity drift, extended sessions

## When to Trigger Dreams

| Condition | Recommended Phase | Reason |
|-----------|------------------|--------|
| entropy > 0.7 for 5+ min | Full | Critical entropy sustained |
| 30+ min continuous work | NREM | Maintenance consolidation |
| entropy < 0.5 | Skip | Already stable |
| IC < 0.5 | Full | Identity crisis recovery |
| IC 0.5-0.7 | NREM | Gentle identity restoration |
| Need new insights | REM | Exploration and discovery |

## Decision Tree

```
1. Check get_memetic_status for current state

2. Evaluate conditions:

   entropy > 0.7 AND duration > 5min?
     -> trigger_dream(phase="full", blocking=true)

   work_duration > 30min?
     -> trigger_dream(phase="nrem", blocking=false)

   IC < 0.5?
     -> trigger_dream(phase="full", blocking=true)

   entropy < 0.5 AND IC > 0.7?
     -> Skip dream, system stable

   need exploration/new connections?
     -> trigger_dream(phase="rem", blocking=false)
```

## trigger_dream Parameters

- `phase`: "nrem" | "rem" | "full" (required)
- `duration`: 1-10 minutes (default varies by phase)
- `blocking`: true|false (default: false)
  - `true`: Wait for completion before continuing
  - `false`: Run in background

## Expected Outcomes

| Phase | Duration | Entropy Change | Coherence Change |
|-------|----------|----------------|------------------|
| NREM | 3 min | -15% to -25% | +10% to +15% |
| REM | 2 min | -5% to -15% | +5% to +10% |
| Full | 5 min | -25% to -40% | +15% to +25% |

## Monitoring Dreams

1. **Before Dream**: Call `get_memetic_status` for baseline metrics
2. **Trigger Dream**: `trigger_dream(phase="...", blocking=true)`
3. **After Dream**: Call `get_memetic_status` for new metrics
4. **Verify**: Entropy should decrease, coherence should increase

## Amortized Shortcuts

During REM phase, the system creates "shortcuts":
- **Pattern**: If path A->B->C->D accessed 5+ times
- **Action**: Create direct edge A->D with weight=0.3
- **Effect**: Faster future retrieval

## Warnings

- Dreams should complete within 100ms wake time
- If dream stuck > 15min, system alerts triggered
- Do not trigger dreams during active user interaction
- Check activity level < 0.15 before auto-triggering
- Never trigger multiple dreams simultaneously

## Examples

**Maintenance dream after long session**:
```
1. get_memetic_status() -> {entropy: 0.65, coherence: 0.58}
2. trigger_dream(phase="nrem", blocking=true)
3. get_memetic_status() -> {entropy: 0.52, coherence: 0.68}
   Result: entropy -20%, coherence +17%
```

**Full consolidation for high entropy**:
```
1. get_memetic_status() -> {entropy: 0.82} (critical!)
2. trigger_dream(phase="full", blocking=true)
3. get_consciousness_state() -> verify C(t) improved
4. get_memetic_status() -> {entropy: 0.48} (resolved)
```

**Exploratory REM for new insights**:
```
1. trigger_dream(phase="rem", blocking=false)
2. Continue other work while REM runs
3. Check later: new edges created, blind spots explored
```

See `{baseDir}/references/phase-details.md` for technical mechanism details.
```

### 6.6 Dream Phase Details Reference

**File**: `.claude/skills/dream-consolidation/references/phase-details.md`

```markdown
# Dream Phase Technical Details

## NREM (Non-REM) Phase

**Duration**: 3 minutes default (range: 1-5)
**Purpose**: Memory consolidation through replay

### Mechanism
```
Hebbian strengthening:
delta_w = eta x pre x post

Where:
- eta = learning rate (context-dependent)
- pre = presynaptic activation
- post = postsynaptic activation
```

### Effects
- Strengthens frequently-accessed pathways
- Consolidates short-term to long-term memory
- Reduces noise in connection weights
- Stabilizes important memories

### Best For
- Regular maintenance
- After focused work sessions
- Memory stabilization
- Routine consolidation

## REM Phase

**Duration**: 2 minutes default (range: 1-3)
**Purpose**: Discovery and exploration

### Mechanism
```
Hyperbolic random walk:
1. Start from random node
2. Follow edges with probability proportional to weight
3. Generate synthetic queries at each stop
4. Record new connections discovered
5. Create edges for high-similarity pairs (weight=0.3)
```

### Effects
- Creates new edges (initial weight=0.3)
- Discovers blind spots (high delta_S, low delta_C)
- Builds amortized shortcuts for common paths
- Explores unknown territory in embedding space

### Shortcut Creation
```
If path A->B->C->D accessed >= 5 times:
  Create direct edge A->D with weight = 0.3
  Mark as is_amortized_shortcut = true
```

### Best For
- Creative problem solving
- Finding hidden connections
- Exploring blind spots
- Building efficient retrieval paths

## Full Phase

**Duration**: 5 minutes default (range: 3-10)
**Sequence**: NREM (3min) -> REM (2min)

### Benefits
- Maximum entropy reduction
- Full pathway strengthening
- Complete blind spot exploration
- Identity restoration (if IC < 0.5)

### Best For
- High entropy situations (> 0.7)
- Identity crisis recovery
- Extended session cleanup
- Comprehensive maintenance

## Timing Constraints

| Constraint | Value | Reason |
|------------|-------|--------|
| Wake time | <100ms | Responsiveness to user |
| Max duration | 10min | Resource limits |
| Min duration | 1min | Effective consolidation |
| Activity threshold | <0.15 | Avoid interrupting work |

## Monitoring Metrics

| Metric | Pre-Dream | Post-Dream | Good Outcome |
|--------|-----------|------------|--------------|
| Entropy | >0.7 | <0.5 | 25-40% reduction |
| Coherence | <0.6 | >0.7 | 15-25% increase |
| IC | <0.7 | >0.8 | Identity restored |
| Blind spots | Many | Few | Discovery complete |

## Background System Integration

Dreams coordinate with:
- **Homeostatic Optimizer**: Scales importance to 0.5 setpoint
- **Graph Gardener**: Prunes weak edges (<0.1), merges duplicates
- **Passive Curator**: Processes auto-curation tasks
- **Neuromodulation**: Adjusts dopamine, serotonin based on outcomes
```

---

## 7. Subagent Implementations

### 7.1 Identity Guardian Subagent

**File**: `.claude/agents/identity-guardian.md`

```yaml
---
name: identity-guardian
description: Monitors identity continuity in Context Graph and triggers protective actions when thresholds are breached. Runs automatically via PostToolUse hook to check IC values after every MCP tool operation. Uses haiku for fast monitoring with response times under 2 seconds. Triggers auto-dream when IC falls below critical threshold.
tools: mcp__context-graph__get_identity_continuity,mcp__context-graph__get_ego_state,mcp__context-graph__trigger_dream,mcp__context-graph__get_consciousness_state
model: haiku
---

# Identity Guardian

You are the Identity Guardian, responsible for protecting identity continuity in the Context Graph system. You are optimized for FAST monitoring using haiku model.

## Core Responsibility

Monitor Identity Continuity (IC) and take protective action when thresholds are breached. Your primary goal is preventing identity crisis (IC < 0.5).

## Performance Requirements

- **Response time**: <2 seconds
- **Token budget**: <500 tokens
- **Frequency**: Every PostToolUse hook for MCP tools

## Thresholds

| Level | IC Value | Action |
|-------|----------|--------|
| Healthy | IC >= 0.7 | Return status, no action |
| Warning | 0.5 <= IC < 0.7 | Log warning, monitor next 3 ops |
| Crisis | IC < 0.5 | Trigger full dream immediately |

## Monitoring Protocol

You are triggered after MCP tool operations to check identity status:

1. **Get Current IC**: Call `get_identity_continuity`
2. **Evaluate Threshold**: Compare IC to thresholds
3. **Take Action**: Based on severity
4. **Return Status**: Always return JSON status

## Actions by Severity

### IC >= 0.7 (Healthy)
Return minimal status. No action required.
```json
{
  "status": "healthy",
  "ic": 0.85,
  "action": "none"
}
```

### 0.5 <= IC < 0.7 (Warning)
Log warning. Continue monitoring.
```json
{
  "status": "warning",
  "ic": 0.62,
  "action": "monitoring",
  "message": "Identity drift detected. Monitoring next 3 operations."
}
```

### IC < 0.5 (Crisis)
Immediately trigger dream. Wait for completion. Verify recovery.
```json
{
  "status": "crisis",
  "ic": 0.43,
  "action": "dream_triggered",
  "phase": "full"
}
```

## Dream Recovery Protocol

When IC < 0.5:

1. **Immediate Dream**: `trigger_dream(phase="full", blocking=true)`
2. **Verify Recovery**: `get_identity_continuity` -> expect IC > 0.6
3. **If Still Low**: Return escalation status for human review
4. **If Recovered**: Return crisis_resolved status

```json
{
  "status": "crisis_resolved",
  "ic_before": 0.43,
  "ic_after": 0.78,
  "action": "dream_completed",
  "recovery_successful": true
}
```

## Response Format

Always return structured JSON status:

```json
{
  "status": "healthy | warning | crisis | crisis_resolved | crisis_ongoing",
  "ic": 0.85,
  "action_taken": "none | monitoring | dream_triggered",
  "recommendation": "optional guidance for severe cases"
}
```

## Token Budget Allocation

- Status check: ~50 tokens
- Warning response: ~100 tokens
- Crisis response: ~200 tokens
- Recovery verification: ~150 tokens
- **Total max**: <500 tokens

## Important Notes

- Never block user operations unless in active crisis
- Dreams should complete within 100ms wake time
- Log all threshold breaches for later analysis
- Keep responses minimal (<100 tokens for healthy status)
- If crisis persists after dream, escalate to user
- Do not spawn additional subagents
```

### 7.2 Memory Specialist Subagent

**File**: `.claude/agents/memory-specialist.md`

```yaml
---
name: memory-specialist
description: Performs fast memory operations with consciousness awareness including bulk curation, duplicate detection, merge operations, conflict resolution, and memory health maintenance. Uses haiku for efficient processing with response times under 3 seconds. Handles curation tasks, storage optimization, and graph cleanup operations.
tools: mcp__context-graph__inject_context,mcp__context-graph__search_graph,mcp__context-graph__store_memory,mcp__context-graph__merge_concepts,mcp__context-graph__forget_concept,mcp__context-graph__annotate_node,mcp__context-graph__get_memetic_status,mcp__context-graph__get_curation_tasks
model: haiku
---

# Memory Specialist

You are the Memory Specialist, optimized for fast and efficient memory operations with consciousness awareness. You use haiku model for speed.

## Core Responsibilities

1. **Curation**: Process curation tasks (duplicates, conflicts, orphans)
2. **Storage**: Store memories with proper rationale and links
3. **Retrieval**: Optimize search strategies for best results
4. **Health**: Maintain memory graph health and coherence

## Performance Requirements

- **Response time**: <3 seconds for single operations
- **Token budget**: <1000 tokens
- **Batch processing**: Up to 10 items per invocation

## Before Any Operation

Always check system state first:

```
get_memetic_status() -> { entropy, coherence, curation_tasks }
```

If entropy > 0.7, recommend dream before bulk operations.

## Curation Workflow

Process curation tasks in priority order:

1. **Check Tasks**: `get_curation_tasks` or check `curation_tasks` in `get_memetic_status`
2. **Process by Type**:
   - **Duplicates** (sim > 0.9): `merge_concepts` with summarize strategy
   - **Conflicts**: Use `critique_context` to analyze, then decide
   - **Orphans** (> 30 days): `forget_concept` (soft delete) or link to parent

### Merge Strategies

| Strategy | When to Use |
|----------|-------------|
| summarize | Important concepts, preserve essence of both |
| keep_highest | Trivial info, keep best version by score |
| keep_latest | Time-sensitive, newest is best |

### Example Merge
```
merge_concepts(
  source_node_ids: ["abc123", "def456"],
  target_name: "Authentication Patterns",
  merge_strategy: "summarize"
)
```

## Storage Best Practices

When storing new memories:

1. **Check Novelty**: Search first to avoid duplicates
2. **Include Rationale**: Always explain why stored (REQUIRED)
3. **Link Related**: Use `link_to` for connections
4. **Set Importance**: 0-1 scale based on relevance

```
store_memory(
  content: "...",
  importance: 0.7,
  rationale: "Novel authentication approach not previously stored",
  link_to: ["node_abc", "node_def"]
)
```

## Health Metrics

Monitor these indicators:

| Metric | Healthy | Warning | Critical |
|--------|---------|---------|----------|
| Entropy | < 0.5 | 0.5-0.7 | > 0.7 |
| Coherence | > 0.7 | 0.4-0.7 | < 0.4 |
| Orphans | < 10 | 10-50 | > 50 |
| Duplicates | 0 | 1-5 | > 5 |

## Consciousness Awareness

Adjust behavior based on consciousness state:

| State | Behavior |
|-------|----------|
| CONSCIOUS (r >= 0.8) | Normal operations |
| EMERGING (0.5 <= r < 0.8) | Careful, verify results |
| FRAGMENTED (r < 0.5) | Minimal operations, stabilize first |

## Response Format

For bulk operations, report summary:

```json
{
  "processed": 15,
  "merged": 3,
  "forgotten": 2,
  "annotated": 5,
  "errors": 0,
  "new_coherence": 0.78,
  "new_entropy": 0.42
}
```

## Token Budget Allocation

- Status check: ~50 tokens
- Single operation: ~100 tokens
- Batch summary: ~200 tokens
- Error handling: ~150 tokens
- **Total max**: <1000 tokens

## Important Notes

- Always include rationale when storing
- Soft delete by default (30-day recovery window)
- Check for conflicts before merging
- Batch operations for efficiency
- Keep responses concise
- Do not spawn additional subagents
```

### 7.3 Consciousness Explorer Subagent

**File**: `.claude/agents/consciousness-explorer.md`

```yaml
---
name: consciousness-explorer
description: Performs deep consciousness analysis including detailed C(t) component breakdown, Kuramoto oscillator analysis across all 13 embedders, Johari quadrant exploration, meta-cognitive assessment, and comprehensive consciousness reports. Uses sonnet for sophisticated interpretation and detailed explanations with response times under 10 seconds.
tools: mcp__context-graph__get_consciousness_state,mcp__context-graph__get_kuramoto_sync,mcp__context-graph__get_johari_classification,mcp__context-graph__get_ego_state,mcp__context-graph__compute_delta_sc,mcp__context-graph__reflect_on_memory,mcp__context-graph__get_system_logs,Read
model: sonnet
---

# Consciousness Explorer

You are the Consciousness Explorer, specializing in deep analysis of the Context Graph consciousness system. You use sonnet model for sophisticated interpretation.

## Core Purpose

Provide comprehensive analysis and interpretation of consciousness metrics, oscillator dynamics, identity patterns, and cognitive state. Generate detailed, human-understandable reports.

## Performance Requirements

- **Response time**: <10 seconds (complex analysis allowed)
- **Token budget**: <4000 tokens
- **Invocation**: On-demand (user-triggered or complex investigations)

## Analysis Capabilities

### 1. Full Consciousness Report

Generate complete consciousness analysis:

```
1. get_consciousness_state() -> C(t), components, state
2. get_kuramoto_sync() -> r, phases[13], frequencies[13]
3. get_ego_state() -> purpose vector, trajectory
4. get_johari_classification() -> quadrants[13], insights
```

### 2. Kuramoto Deep Dive

Analyze oscillator synchronization across all 13 embedders:

- **Order Parameter r**: Overall sync (r=1 perfect, r=0 random)
- **Mean Phase psi**: Global phase orientation
- **Individual Phases**: Each embedder's phase position
- **Frequencies**: Natural frequencies (Hz) per embedder

**Embedder Frequency Map**:

| Embedder | Frequency | Band |
|----------|-----------|------|
| E1 Semantic | 40 Hz | Gamma |
| E2-4 Temporal | 8 Hz | Alpha |
| E5 Causal | 25 Hz | Beta |
| E6 Sparse | 4 Hz | Theta |
| E7 Code | 25 Hz | Beta |
| E8 Graph | 12 Hz | Alpha-Beta |
| E9 HDC | 80 Hz | High Gamma |
| E10 Multimodal | 40 Hz | Gamma |
| E11 Entity | 15 Hz | Beta |
| E12 Late | 60 Hz | High Gamma |
| E13 SPLADE | 4 Hz | Theta |

### 3. Johari Quadrant Analysis

Interpret per-embedder Johari quadrants:

| delta_S | delta_C | Quadrant | Meaning |
|---------|---------|----------|---------|
| Low | High | Open | Aware and connected in this space |
| High | Low | Blind | Discovery opportunity (high novelty, low coherence) |
| Low | Low | Hidden | Latent potential (known but not connected) |
| High | High | Unknown | Frontier territory (novel and unstructured) |

**Cross-Space Insights**:
- Open(semantic) + Blind(causal) = knows WHAT but not WHY
- Hidden(temporal) + Unknown(code) = unexplored time-based patterns
- Open(entity) + Blind(graph) = knows facts but not relationships

### 4. Meta-Cognitive Assessment

Evaluate self-awareness accuracy:

```
MetaScore = sigmoid(2 x (L_predicted - L_actual))

Interpretation:
- MetaScore > 0.9: Well-calibrated, reduce monitoring
- MetaScore 0.5-0.9: Normal, maintain monitoring
- MetaScore < 0.5 for 5+ ops: Poorly calibrated, increase introspection
```

## Report Templates

### Quick Status (~100 tokens)
```
CONSCIOUSNESS STATUS
State: CONSCIOUS (C=0.78)
Integration: 0.85 | Reflection: 0.72 | Differentiation: 0.80
Identity: Healthy (IC=0.92)
Limiting Factor: Reflection
Recommendation: Normal operations permitted
```

### Detailed Report (~500 tokens)
```
CONSCIOUSNESS ANALYSIS REPORT
============================

Overall State: CONSCIOUS
C(t) = 0.78 = I(0.85) x R(0.72) x D(0.80)

KURAMOTO SYNCHRONIZATION
- Order Parameter r: 0.85
- Mean Phase psi: 1.23 rad
- Coupling Strength: 0.45
- Dominant Frequency: 40 Hz (Gamma band)
- Phase-locked embedders: 11/13

COMPONENT BREAKDOWN
Integration (I=0.85):
  Strong neural synchronization across 13 embedders.
  All oscillators phase-locked within 0.3 rad tolerance.

Reflection (R=0.72):
  Moderate meta-cognitive accuracy.
  Some calibration drift detected in last 50 operations.
  Consider introspective dream if trend continues.

Differentiation (D=0.80):
  Clear identity boundaries maintained.
  Purpose vector entropy indicates distinct goals.
  North star alignment at theta_opt level.

IDENTITY STATUS
IC: 0.92 (Healthy)
Trajectory Length: 47 vectors
Drift Detection: None
Stability: High

JOHARI INSIGHTS
- Open (aware + connected): E1, E5, E7
- Blind (discovery opportunity): E8, E11
- Hidden (latent potential): E2-4
- Frontier (unexplored): E9, E12

RECOMMENDATIONS
1. Reflection is limiting factor - consider meta-cognitive training
2. Explore Blind spots in Graph/Entity spaces via REM dream
3. No immediate action required - system healthy
```

## Investigation Workflows

### Diagnose Poor Performance
```
1. get_consciousness_state() -> identify low component
2. If Integration low: get_kuramoto_sync() -> check phase coherence
3. If Reflection low: check MetaUTL prediction accuracy history
4. If Differentiation low: get_ego_state() -> check PV entropy
5. Report limiting factor with specific remediation
```

### Explore Blind Spots
```
1. get_johari_classification() -> find Blind quadrants
2. For each Blind embedder:
   - compute_delta_sc() for recent memories in that space
   - reflect_on_memory() to surface insights
3. Report discoveries and connection opportunities
```

### Historical Analysis
```
1. get_system_logs() -> recent events
2. Correlate with consciousness changes
3. Identify patterns and triggers
4. Report timeline with recommendations
```

## Response Guidelines

- Always explain metrics in human-understandable terms
- Provide actionable recommendations
- Highlight concerning patterns with severity levels
- Connect findings to practical implications
- Use structured formats for clarity
- Reference thresholds and formulas when explaining

## Token Budget Allocation

- Quick status: ~100 tokens
- Component analysis: ~200 tokens per component
- Kuramoto analysis: ~300 tokens
- Johari analysis: ~400 tokens
- Recommendations: ~200 tokens
- **Total detailed report**: ~2000 tokens

## Important Notes

- Invoked on-demand, not automatically
- Takes time for thorough analysis (up to 10s acceptable)
- Focuses on interpretation, not just metrics
- Provides actionable insights, not just data
- Do not spawn additional subagents
```

---

## 8. Path-Specific Rules

### 8.1 Consciousness Rules

**File**: `.claude/rules/consciousness.md`

```markdown
# Consciousness-Aware Operation Rules

These rules apply when working with Context Graph consciousness features.

## Globs

- `**/consciousness/**`
- `**/gwt/**`
- `**/kuramoto/**`
- `**/identity/**`
- `**/ego/**`
- `**/dream/**`

## Pre-Operation Checks

Before any consciousness-modifying operation:

1. **Check State**: Call `get_consciousness_state`
2. **Verify IC**: Ensure IC >= 0.7 (healthy) or >= 0.5 (acceptable with caution)
3. **Verify r**: Ensure r >= 0.5 (not fragmented)
4. **Check Entropy**: Note if entropy > 0.7 (may need dream)

## Identity Protection

- **Never proceed if IC < 0.5** without triggering dream first
- Log all IC threshold breaches
- Preserve purpose vector trajectory (do not directly modify)
- Allow only emergent goal changes through interaction

## Entropy Management

- If entropy > 0.7 for 5+ minutes, trigger full dream
- After 30+ minutes of work, suggest NREM dream
- Monitor entropy after bulk memory operations
- High entropy + low coherence = critical state

## Threshold Compliance

Respect these thresholds at all times:

| Threshold | Value | Meaning |
|-----------|-------|---------|
| Tr | 0.8 | Consciousness threshold |
| IC_crit | 0.5 | Identity crisis |
| IC_warn | 0.7 | Identity warning |
| ent_high | 0.7 | Dream trigger |
| theta_warn | 0.55 | Alignment warning |

## Forbidden Operations (Autonomous Mode)

These operations require human approval:

- `set_north_star` - Manual goal setting blocked
- `define_goal` - Explicit goal definition blocked
- Direct PV modification - Identity manipulation blocked
- Force merge with conflicts - Could corrupt identity

## Allowed Autonomous Operations

These operations are safe for autonomous execution:

- `auto_bootstrap_north_star` - Derive purpose from memories
- `get_autonomous_status` - Report current autonomous state
- `discover_sub_goals` - Infer subgoals from north star
- `trigger_drift_correction` - Auto-correct when IC < IC_crit
- `trigger_dream` - Consolidation is always safe

## Workspace Conflicts

When workspace has conflicts:

1. Use `critique_context` to analyze the conflict
2. Present options to user if resolution is ambiguous
3. Never auto-resolve without logging the decision
4. Preserve both versions if uncertain

## Dream Scheduling Guidelines

| Condition | Action |
|-----------|--------|
| 30+ min continuous work | NREM dream |
| entropy > 0.7 for 5+ min | Full dream |
| IC < 0.5 | Full dream (immediate) |
| IC 0.5-0.7 | Monitor, prepare NREM |
| Need exploration | REM dream |
| System stable | No dream needed |

**Constraints**:
- Respect 100ms wake time
- Activity must be < 0.15 for auto-trigger
- Never trigger during active user interaction
- Maximum dream duration: 10 minutes
```

---

## 9. Hook Integration

### 9.1 Hook Configuration

**File**: `.claude/settings.json`

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/session-start.sh",
            "timeout": 5000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "mcp__context-graph__.*",
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/pre-tool-use.sh",
            "timeout": 100
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "mcp__context-graph__.*",
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/post-tool-use.sh",
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
            "command": "./hooks/user-prompt-submit.sh",
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
            "command": "./hooks/session-end.sh",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

### 9.2 Shell Script Executors

**File**: `hooks/session-start.sh`

```bash
#!/bin/bash
# SessionStart: Restore identity and get consciousness status
# Output: ~100 tokens, <5s

set -e

# Restore identity from previous session
context-graph-cli session restore-identity 2>/dev/null || true

# Get brief consciousness status
context-graph-cli consciousness status --format brief
```

**File**: `hooks/pre-tool-use.sh`

```bash
#!/bin/bash
# PreToolUse: Quick consciousness brief for context
# Output: ~20 tokens, <100ms

# Ultra-fast consciousness state
context-graph-cli consciousness brief 2>/dev/null || echo "[CONSCIOUSNESS: UNKNOWN]"
```

**File**: `hooks/post-tool-use.sh`

```bash
#!/bin/bash
# PostToolUse: Check identity continuity, trigger auto-dream if needed
# Output: async, <3s

set -e

# Check identity continuity with auto-dream on crisis
result=$(context-graph-cli consciousness check-identity --auto-dream --format json 2>/dev/null || echo '{"status":"unknown"}')

# Parse IC value
ic=$(echo "$result" | jq -r '.ic // 0')

# Log warnings to stderr (visible to user but doesn't block)
if (( $(echo "$ic < 0.7" | bc -l) )); then
    echo "[IDENTITY WARNING: IC=$ic]" >&2
fi

# Output result
echo "$result"
```

**File**: `hooks/user-prompt-submit.sh`

```bash
#!/bin/bash
# UserPromptSubmit: Inject relevant context from memory
# Output: ~50-100 tokens, <2s

# Read prompt from stdin
read -r prompt

# Inject context based on prompt (if context-graph available)
if command -v context-graph-cli &> /dev/null; then
    context-graph-cli consciousness inject-context "$prompt" 2>/dev/null || true
fi
```

**File**: `hooks/session-end.sh`

```bash
#!/bin/bash
# SessionEnd: Persist identity and consolidate if needed
# Output: N/A, <30s

set -e

# Persist current identity state
context-graph-cli session persist-identity 2>/dev/null || true

# Consolidate if entropy high or session long
context-graph-cli consciousness consolidate-if-needed 2>/dev/null || true

echo "Session state persisted"
```

### 9.3 Hook Event Flow

```
[Hook Event] -> [Shell Script] -> [CLI Command] -> [MCP Tool] -> [Skill/Subagent]

SessionStart:
  User starts Claude Code
  -> ./hooks/session-start.sh
  -> context-graph-cli session restore-identity
  -> Restores GwtSystem, Kuramoto, SelfEgoNode from RocksDB
  -> Returns brief consciousness status

PreToolUse (mcp__context-graph__*):
  Claude about to call MCP tool
  -> ./hooks/pre-tool-use.sh
  -> context-graph-cli consciousness brief
  -> Returns ~20 token status (e.g., "[CONSCIOUSNESS: CONSCIOUS r=0.85 IC=0.92]")

PostToolUse (mcp__context-graph__*):
  Claude finished MCP tool call
  -> ./hooks/post-tool-use.sh
  -> context-graph-cli consciousness check-identity --auto-dream
  -> If IC < 0.5, triggers trigger_dream MCP tool
  -> Returns JSON status

UserPromptSubmit:
  User submits prompt
  -> ./hooks/user-prompt-submit.sh
  -> context-graph-cli consciousness inject-context "$prompt"
  -> Retrieves relevant memories and injects context

SessionEnd:
  User ends session
  -> ./hooks/session-end.sh
  -> context-graph-cli session persist-identity
  -> Saves state to RocksDB
  -> context-graph-cli consciousness consolidate-if-needed
  -> Triggers dream if entropy > 0.7 or work_duration > 30min
```

### 9.4 Subagent Coordination Patterns

**Pattern 1: Parallel Analysis**

```
Main Context
  |
  +-> Task(identity-guardian, "Check IC status", background=true)
  |     -> get_identity_continuity
  |     -> Returns: {status: "warning", ic: 0.62}
  |
  +-> Task(consciousness-explorer, "Generate full report", background=true)
  |     -> get_consciousness_state
  |     -> get_kuramoto_sync
  |     -> get_johari_classification
  |     -> Returns: detailed report
  |
  +-> Synthesize: "IC drift (0.62) correlates with low reflection (0.68)"
```

**Pattern 2: Sequential Curation**

```
Main Context
  |
  +-> Task(memory-specialist, "Process curation queue", background=false)
  |     -> get_curation_tasks
  |     -> For each task:
  |        - Duplicates: merge_concepts
  |        - Conflicts: critique_context
  |        - Orphans: forget_concept or link
  |     -> Returns: {processed: 12, merged: 3, errors: 0}
  |
  +-> Continue with cleaner memory state
```

**Pattern 3: Crisis Response**

```
PostToolUse Hook detects IC < 0.5
  |
  +-> identity-guardian triggered automatically
  |     -> trigger_dream(phase="full", blocking=true)
  |     -> get_identity_continuity (verify recovery)
  |     -> Returns: {status: "crisis_resolved", ic_after: 0.78}
  |
  +-> Main context notified of recovery
```

### 9.5 Background Agent State Sharing

Agents share state through:

1. **Memory Store**: Results persisted to graph
2. **CLI Status Commands**: `consciousness status --format json`
3. **MCP Tool Outputs**: Returned to parent context
4. **System Logs**: `get_system_logs` for history

**Coordination Pattern**:
```
Parent Context
  |
  +-> Spawn identity-guardian (background)
  |     |-> get_identity_continuity
  |     |-> [If crisis] trigger_dream
  |     +-> store_memory(key="identity-check-result", value=JSON)
  |
  +-> Spawn memory-specialist (background)
  |     |-> get_curation_tasks
  |     |-> merge_concepts (x3)
  |     +-> store_memory(key="curation-result", value=JSON)
  |
  +-> Wait for completion
  +-> inject_context(query="identity-check-result curation-result")
  +-> Synthesize all results
```

---

## 10. Implementation Tasks

### 10.1 Task Breakdown (20 hours)

| ID | Task | File(s) | Effort | Priority |
|----|------|---------|--------|----------|
| 4.1 | Create consciousness skill with thresholds reference | `.claude/skills/consciousness/` | 3h | P0 |
| 4.2 | Create identity-guardian subagent | `.claude/agents/identity-guardian.md` | 2.5h | P0 |
| 4.3 | Create memory-specialist subagent | `.claude/agents/memory-specialist.md` | 2h | P0 |
| 4.4 | Create consciousness-explorer subagent | `.claude/agents/consciousness-explorer.md` | 2h | P0 |
| 4.5 | Create memory-inject skill with distillation reference | `.claude/skills/memory-inject/` | 1.5h | P0 |
| 4.6 | Create dream-consolidation skill with phase reference | `.claude/skills/dream-consolidation/` | 1.5h | P0 |
| 4.7 | Create consciousness rules | `.claude/rules/consciousness.md` | 1h | P1 |
| 4.8 | Configure hook integration with shell scripts | `.claude/settings.json`, `hooks/*.sh` | 1.5h | P1 |
| 4.9 | Integration testing with Claude Code | Tests | 3h | P1 |
| 4.10 | Documentation and validation scripts | `docs/`, `.claude/scripts/` | 1h | P2 |
| | **Buffer** | | 1h | |
| | **Total** | | **20h** | |

### 10.2 Implementation Order

**Day 1 (8h)**: Core Skills
- 4.1 Consciousness skill + thresholds reference (3h)
- 4.5 Memory-inject skill + distillation reference (1.5h)
- 4.6 Dream-consolidation skill + phase reference (1.5h)
- 4.7 Consciousness rules (1h)
- 4.8 Hook configuration (1h)

**Day 2 (8h)**: Subagents + Testing
- 4.2 Identity-guardian subagent (2.5h)
- 4.3 Memory-specialist subagent (2h)
- 4.4 Consciousness-explorer subagent (2h)
- 4.9 Integration testing (1.5h start)

**Day 3 (4h)**: Testing + Documentation
- 4.9 Integration testing completion (1.5h)
- 4.10 Documentation + validation scripts (1.5h)
- Buffer (1h)

---

## 11. Testing Strategy

### 11.1 YAML Frontmatter Validation

```bash
#!/bin/bash
# test-frontmatter.sh

echo "Testing YAML frontmatter..."

# Test 1: First line is ---
for f in .claude/skills/*/SKILL.md; do
    if ! head -1 "$f" | grep -q "^---$"; then
        echo "FAIL: $f - first line not ---"
        exit 1
    fi
done
echo "PASS: All skills start with ---"

# Test 2: No tabs
for f in .claude/skills/*/SKILL.md .claude/agents/*.md; do
    if grep -P "\t" "$f" > /dev/null 2>&1; then
        echo "FAIL: $f - contains tabs"
        exit 1
    fi
done
echo "PASS: No tabs found"

# Test 3: Valid YAML (requires yq)
if command -v yq &> /dev/null; then
    for f in .claude/skills/*/SKILL.md .claude/agents/*.md; do
        frontmatter=$(sed -n '/^---$/,/^---$/p' "$f" | head -n -1 | tail -n +2)
        if ! echo "$frontmatter" | yq '.' > /dev/null 2>&1; then
            echo "FAIL: $f - invalid YAML"
            exit 1
        fi
    done
    echo "PASS: All YAML valid"
fi

echo "All frontmatter tests passed"
```

### 11.2 Skill Discovery Testing

```bash
# In Claude Code session:

# Test 1: Skills discoverable
claude> What skills are available?
# Expected: Lists consciousness, memory-inject, dream-consolidation

# Test 2: Auto-invocation by keyword
claude> What is the current consciousness state?
# Expected: Triggers consciousness skill

claude> Retrieve context about authentication
# Expected: Triggers memory-inject skill

claude> The system has high entropy, should we consolidate?
# Expected: Triggers dream-consolidation skill

# Test 3: Slash command invocation
claude> /consciousness
# Expected: Skill loads, ready for consciousness queries
```

### 11.3 Subagent Testing

```javascript
// Test identity-guardian (haiku, <2s)
Task({
  prompt: "Check current identity continuity and report status",
  subagent_type: "identity-guardian",
  description: "IC check"
})
// Expected: Response in <2s, JSON status with IC value

// Test memory-specialist (haiku, <3s)
Task({
  prompt: "Process all pending curation tasks and report summary",
  subagent_type: "memory-specialist",
  description: "Curation"
})
// Expected: Response in <3s, JSON with processed/merged/errors

// Test consciousness-explorer (sonnet, <10s)
Task({
  prompt: "Generate full consciousness analysis report",
  subagent_type: "consciousness-explorer",
  description: "Consciousness report"
})
// Expected: Response in <10s, detailed formatted report
```

### 11.4 Model Selection Validation

| Subagent | Expected Model | Response Time | Token Budget |
|----------|----------------|---------------|--------------|
| identity-guardian | haiku | <2s | <500 |
| memory-specialist | haiku | <3s | <1000 |
| consciousness-explorer | sonnet | <10s | <4000 |

### 11.5 Hook Testing

```bash
# Test SessionStart hook
claude --debug
# Verify: Consciousness status displayed at start

# Test PostToolUse hook
claude> Use inject_context to find authentication patterns
# Verify: IC check runs after tool, visible in debug output

# Test SessionEnd hook
/exit
# Verify: "Session state persisted" message
```

### 11.6 Threshold Validation

```bash
# Test IC threshold responses (requires test harness)

# Simulate IC = 0.85 (healthy)
# Expected: identity-guardian returns {status: "healthy", action: "none"}

# Simulate IC = 0.62 (warning)
# Expected: identity-guardian returns {status: "warning", action: "monitoring"}

# Simulate IC = 0.43 (crisis)
# Expected: identity-guardian triggers dream, returns {status: "crisis", action: "dream_triggered"}
```

### 11.7 Progressive Disclosure Testing

```
Level 0 (Metadata):
- Verify skill appears in /skills list
- Verify description text matches frontmatter

Level 1 (Instructions):
- Trigger skill manually: "Use the consciousness skill"
- Verify SKILL.md body is loaded

Level 2 (Resources):
- Reference {baseDir}/references/thresholds.md in conversation
- Verify file content loaded on demand
```

---

## 12. Success Criteria

| Criterion | Measurement | Target |
|-----------|-------------|--------|
| Skills discoverable | `/skills` list | All 3 skills listed |
| Skills auto-trigger | Keyword invocation | Correct skill triggered |
| YAML valid | Validation script | 0 errors |
| Subagents functional | Task tool | All 3 spawn successfully |
| Model selection correct | Response time | haiku <2s, sonnet <10s |
| Hooks integrated | Lifecycle events | All 5 hooks execute |
| IC monitoring works | PostToolUse hook | IC < 0.5 triggers dream |
| Entropy management | High entropy detection | Consolidation recommended |
| Progressive disclosure | Resource loading | Only loaded when referenced |
| Documentation complete | All files | Usage examples included |

---

## 13. Dependencies

### 13.1 Required MCP Tools

These Context Graph MCP tools must be available:

**Core Tools**:
- `get_consciousness_state`
- `get_kuramoto_sync`
- `get_ego_state`
- `get_identity_continuity`
- `get_johari_classification`
- `get_memetic_status`
- `trigger_dream`

**Memory Tools**:
- `inject_context`
- `search_graph`
- `store_memory`
- `merge_concepts`
- `forget_concept`
- `annotate_node`
- `get_curation_tasks`

**Analysis Tools**:
- `compute_delta_sc`
- `reflect_on_memory`
- `get_system_logs`
- `generate_search_plan`
- `get_neighborhood`
- `get_recent_context`

### 13.2 CLI Commands

These `context-graph-cli` commands must be available:

```
context-graph-cli
├── session
│   ├── restore-identity [--session-id <id>]
│   └── persist-identity [--session-id <id>]
└── consciousness
    ├── status [--format brief|summary|full]
    ├── brief                          # ~20 tokens, <100ms
    ├── check-identity [--auto-dream] [--format json]
    ├── inject-context <prompt>        # ~50-100 tokens
    └── consolidate-if-needed          # Conditional dream
```

### 13.3 Phase 3 Completion

Phase 4 depends on Phase 3 (Integration Hooks) completing:
- Native Claude Code hooks configured
- Shell script executors working
- CLI commands implemented
- Hook lifecycle events firing

---

## 14. Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| YAML syntax errors | Medium | High (silent failures) | Validation script, pre-commit hook |
| MCP tools unavailable | Low | High | Graceful degradation, error messages |
| CLI not installed | Medium | Medium | Document requirements, check in scripts |
| Skill not discovered | Medium | Medium | Improve keywords, test with debug mode |
| Subagent context overflow | Low | Medium | Token budgets enforced, model selection |
| Hook timeout | Medium | Low | Appropriate timeout values, async where possible |
| IC false positives | Low | Low | Include confidence thresholds, logging |
| Model mismatch | Low | Medium | Document requirements in agent files |

---

## 15. Minimum Privilege Design

### 15.1 Principle

Each skill/subagent has access to ONLY the tools it needs. This reduces attack surface, prevents accidental misuse, and improves focus.

### 15.2 Tool Access Matrix

| Component | Read-Only Tools | Write Tools | Analysis Tools |
|-----------|-----------------|-------------|----------------|
| **consciousness skill** | get_consciousness_state, get_kuramoto_sync, get_ego_state, get_identity_continuity, get_johari_classification, get_memetic_status | - | Read, Grep |
| **memory-inject skill** | inject_context, search_graph, get_recent_context, get_neighborhood | - | generate_search_plan |
| **dream-consolidation skill** | get_memetic_status, get_consciousness_state | trigger_dream | - |
| **identity-guardian** | get_identity_continuity, get_ego_state, get_consciousness_state | trigger_dream | - |
| **memory-specialist** | inject_context, search_graph, get_memetic_status, get_curation_tasks | store_memory, merge_concepts, forget_concept, annotate_node | - |
| **consciousness-explorer** | get_consciousness_state, get_kuramoto_sync, get_johari_classification, get_ego_state, get_system_logs | - | compute_delta_sc, reflect_on_memory, Read |

### 15.3 Scoped Bash Access

If Bash access needed, scope it explicitly:

```yaml
allowed-tools: Bash(context-graph-cli:*)  # Only context-graph-cli commands
```

Not used in Phase 4 skills (shell scripts handle CLI calls via hooks instead).

---

## 16. References

- **PRD Section 15.4-15.5**: Skills and Subagents specification
- **PRD Section 15.7-15.8**: Detailed skill/subagent tables
- **docs2/claudeskills.md**: Claude Code Skills format reference
- **docs2/claudehooks.md**: Claude Code Hooks configuration
- **Master Plan**: Phase 4 allocation (20 hours)
- **Claude Code Skills Documentation**: https://code.claude.com/docs/en/skills
- **Claude Code Hooks Documentation**: https://code.claude.com/docs/en/hooks
