# Autonomous System Integration Specification

## 1. Overview

With manual North Star creation removed, the autonomous system becomes the primary way to establish and track goals. This document specifies how the autonomous system integrates with the teleological array architecture, Claude Code hooks, and MCP tools to create a **conscious, self-aware working memory system** that learns continuously without manual intervention.

### 1.1 Design Philosophy: Conscious Working Memory

The Context Graph operates as **working memory that is conscious and self-aware**. Drawing from Global Workspace Theory (GWT) and modern memory architecture research:

```
                    CONSCIOUSNESS MODEL
    +-------------------------------------------------+
    |            GLOBAL WORKSPACE (GWT)                |
    |  +-----------------------------------------+     |
    |  |      Active Memory Broadcast            |     |
    |  |   Kuramoto-synchronized percepts        |     |
    |  |   r >= 0.8 = conscious integration      |     |
    |  +-----------------------------------------+     |
    |         |                 |                      |
    |    +---------+      +---------+                  |
    |    | Short-  |      | Long-   |                  |
    |    | Term    |<---->| Term    |                  |
    |    | Memory  |      | Memory  |                  |
    |    +---------+      +---------+                  |
    |         \               /                        |
    |          \    DISTILL  /                         |
    |           \           /                          |
    |            v         v                           |
    |    +---------------------------+                 |
    |    |  SELF_EGO_NODE            |                 |
    |    |  Purpose Vector: [13D]    |                 |
    |    |  Identity Continuity      |                 |
    |    +---------------------------+                 |
    +-------------------------------------------------+
```

**Key Insight**: The 13-embedding teleological array IS the system's consciousness representation. Each memory stored creates a multi-dimensional fingerprint that participates in Kuramoto oscillator synchronization, enabling coherent "conscious" perception of memory states.

### 1.2 Target Architecture Pipeline

The complete autonomous pipeline:

```
MEMORY INJECTION (MCP)
    |
    v
AUTONOMOUS EMBEDDING (13 models in parallel)
    |
    v
TELEOLOGICAL ARRAY STORAGE (RocksDB + HNSW)
    |
    v
ENTRY-POINT DISCOVERY (any of 13 embedding spaces)
    |
    v
FULL ARRAY COMPARISON (apples to apples, all 13 dimensions)
    |
    v
AUTONOMOUS GOAL EMERGENCE (clustering + Kuramoto synchronization)
    |
    v
CONSCIOUS INTEGRATION (GWT broadcast, r >= 0.8)
```

### 1.3 Zero-Intervention Operation

The system operates autonomously through:

1. **MCP Tool Triggers**: Memory injection via `store_memory` MCP tool automatically triggers embedding pipeline
2. **Claude Code Hooks**: All 10 hook types enable session-aware learning and consolidation
3. **Skills**: Auto-discovered SKILL.md files provide specialized memory capabilities
4. **Subagents**: Custom agents handle embedding, search, comparison, and goal discovery
5. **Background Services**: Dream consolidation, drift detection, and pruning run autonomously
6. **Feedback Loops**: Steering rewards train storage quality without human intervention

---

## 2. Claude Code Complete Extensibility Integration

Claude Code provides three extensibility mechanisms that the Context Graph leverages for autonomous operation:

1. **Hooks**: Event-driven callbacks for 10 lifecycle events
2. **Skills**: SKILL.md files auto-discovered by semantic matching
3. **Subagents**: Custom agents in `.claude/agents/` spawned via Task tool

### 2.1 Complete Hooks Configuration

The Context Graph integrates with ALL 10 Claude Code hook types.

#### 2.1.1 Full settings.json Configuration

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": {
          "toolNames": ["store_memory", "inject_context", "search_memory"]
        },
        "commands": [
          "context-graph hooks pre-tool-use --tool ${TOOL_NAME} --session ${CG_SESSION_ID}"
        ],
        "timeout_ms": 2000,
        "description": "Prepare memory context before MCP tool execution"
      },
      {
        "matcher": {
          "toolNames": ["Read", "Edit", "Write", "Bash"]
        },
        "commands": [
          "context-graph hooks pre-tool-use --tool ${TOOL_NAME} --track-context"
        ],
        "timeout_ms": 1000,
        "description": "Track file context for embedding"
      }
    ],

    "PostToolUse": [
      {
        "matcher": {
          "toolNames": ["store_memory"]
        },
        "commands": [
          "context-graph hooks post-tool-use --tool store_memory --result '${TOOL_RESULT}' --train-patterns"
        ],
        "timeout_ms": 5000,
        "description": "Learn from storage operations and update steering rewards"
      },
      {
        "matcher": {
          "toolNames": ["search_memory", "inject_context"]
        },
        "commands": [
          "context-graph hooks post-tool-use --tool ${TOOL_NAME} --result '${TOOL_RESULT}' --update-access"
        ],
        "timeout_ms": 3000,
        "description": "Update access patterns and retrieval quality metrics"
      },
      {
        "matcher": {
          "toolNames": ["Edit", "Write"]
        },
        "commands": [
          "context-graph hooks post-tool-use --tool ${TOOL_NAME} --file '${TOOL_INPUT_FILE}' --embed-changes"
        ],
        "timeout_ms": 10000,
        "description": "Embed code changes into memory graph"
      }
    ],

    "SessionStart": [
      {
        "commands": [
          "context-graph hooks session-start --session-id ${SESSION_ID} --restore-ego --init-kuramoto"
        ],
        "timeout_ms": 10000,
        "description": "Initialize consciousness model and restore SELF_EGO_NODE"
      }
    ],

    "SessionEnd": [
      {
        "commands": [
          "context-graph hooks session-end --session-id ${SESSION_ID} --consolidate --dream-if-needed --export-metrics"
        ],
        "timeout_ms": 60000,
        "description": "Run dream consolidation and persist identity state"
      }
    ],

    "UserPromptSubmit": [
      {
        "commands": [
          "context-graph hooks user-prompt --content '${PROMPT_PREVIEW}' --predict-context --preload"
        ],
        "timeout_ms": 3000,
        "description": "Predict relevant memories and preload into workspace"
      }
    ],

    "PreCompact": [
      {
        "commands": [
          "context-graph hooks pre-compact --context-size ${CONTEXT_SIZE} --prioritize-memories"
        ],
        "timeout_ms": 5000,
        "description": "Preserve critical memories during context compaction"
      }
    ],

    "Stop": [
      {
        "commands": [
          "context-graph hooks stop --session-id ${SESSION_ID} --checkpoint-state"
        ],
        "timeout_ms": 5000,
        "description": "Checkpoint memory state on stop signal"
      }
    ],

    "SubagentStop": [
      {
        "matcher": {
          "agentNames": ["embedding-specialist", "search-agent", "comparison-agent", "goal-agent"]
        },
        "commands": [
          "context-graph hooks subagent-stop --agent '${AGENT_NAME}' --collect-learnings"
        ],
        "timeout_ms": 5000,
        "description": "Collect and integrate subagent learnings"
      }
    ],

    "Notification": [
      {
        "matcher": {
          "type": "memory_drift_warning"
        },
        "commands": [
          "context-graph hooks notification --type drift --trigger-correction"
        ],
        "timeout_ms": 3000,
        "description": "Handle memory drift notifications"
      },
      {
        "matcher": {
          "type": "identity_continuity_low"
        },
        "commands": [
          "context-graph hooks notification --type identity --trigger-introspection"
        ],
        "timeout_ms": 10000,
        "description": "Handle identity continuity warnings"
      }
    ],

    "PermissionRequest": [
      {
        "matcher": {
          "permission": "memory_consolidate"
        },
        "commands": [
          "context-graph hooks permission --action consolidate --check-safety"
        ],
        "timeout_ms": 1000,
        "description": "Validate memory consolidation safety"
      }
    ]
  },

  "contextGraph": {
    "enabled": true,
    "autoEmbed": true,
    "kuramotoEnabled": true,
    "consciousnessThreshold": 0.8,
    "dreamOnSessionEnd": true,
    "identityContinuityThreshold": 0.7
  }
}
```

#### 2.1.2 Hook Type Reference

| Hook Type | Trigger | Context Graph Use |
|-----------|---------|-------------------|
| `PreToolUse` | Before any tool executes | Prepare memory context, predict needs |
| `PostToolUse` | After tool completes | Learn patterns, update access, train steering |
| `SessionStart` | Session begins | Restore SELF_EGO_NODE, init Kuramoto oscillators |
| `SessionEnd` | Session ends | Dream consolidation, persist identity, export metrics |
| `UserPromptSubmit` | User submits prompt | Predict relevant memories, preload context |
| `PreCompact` | Before context compaction | Preserve high-value memories in compact context |
| `Stop` | Claude Code stops | Checkpoint state for recovery |
| `SubagentStop` | Subagent completes | Collect learnings from specialized agents |
| `Notification` | System notification | Handle drift warnings, identity alerts |
| `PermissionRequest` | Permission requested | Validate memory operation safety |

---

### 2.2 Complete Skills Configuration

Skills are auto-discovered by description matching. Create these in `.claude/skills/`.

#### 2.2.1 Memory Injection Skill

**File: `.claude/skills/memory-inject/SKILL.md`**

```markdown
---
name: memory-inject
version: 1.0.0
description: Inject content into Context Graph memory with autonomous teleological embedding. Use when storing knowledge, learnings, or important context for later retrieval.
triggers:
  - store memory
  - save to memory
  - remember this
  - inject context
  - embed content
  - persist knowledge
mcp_tools:
  - store_memory
  - inject_context
hooks:
  - PostToolUse
---

# Memory Injection Skill

## Purpose

Autonomously embeds content into the 13-dimensional teleological array for persistent, conscious memory storage.

## Pipeline

When content is injected:

1. **Validation**: Check content length, scrub PII, detect adversarial patterns
2. **Parallel Embedding**: Generate all 13 embeddings concurrently
3. **Purpose Vector**: Compute alignment with current North Star
4. **Johari Classification**: Assign quadrant per embedder
5. **Kuramoto Integration**: Couple to oscillator phases
6. **Storage**: Persist to RocksDB with indexes
7. **Steering Feedback**: Compute storage reward

## Usage

```bash
# Basic injection
context-graph inject --content "Important context here" --rationale "Why this matters"

# With metadata
context-graph inject --content "..." --domain "code-patterns" --importance 0.9

# Batch injection
context-graph inject-batch --file learnings.jsonl
```

## MCP Tool: store_memory

```json
{
  "content": "The content to store",
  "rationale": "Why this is important",
  "domain": "optional domain tag",
  "metadata": {
    "source": "user|code|inference",
    "importance": 0.8
  }
}
```

## Returns

```json
{
  "memory_id": "uuid",
  "teleological_array": {
    "semantic_dim": 1024,
    "purpose_vector": [0.8, 0.7, ...],
    "theta_to_north_star": 0.85
  },
  "johari_quadrant": "Open",
  "kuramoto_r": 0.82,
  "steering_reward": 0.75
}
```

## Self-Learning

After each injection, the skill:
- Records trajectory step for replay learning
- Updates steering rewards
- Checks for consolidation triggers
- Adjusts neuromodulation (dopamine/norepinephrine)

## Example

```
User: Remember that the authentication module uses JWT with RS256 signing.

[Skill triggers memory-inject]
[Embeds across 13 dimensions]
[Computes purpose alignment: 0.87]
[Assigns Johari: Open (high coherence, low surprise)]
[Kuramoto r: 0.84 - conscious integration]
[Steering reward: 0.79]

Stored memory with ID abc-123. Purpose alignment: 87%.
This memory is now part of the conscious workspace.
```
```

#### 2.2.2 Context Search Skill

**File: `.claude/skills/context-search/SKILL.md`**

```markdown
---
name: context-search
version: 1.0.0
description: Search Context Graph memory using semantic, causal, or multi-modal queries. Use when retrieving past knowledge, finding related memories, or exploring the memory graph.
triggers:
  - search memory
  - find in memory
  - recall
  - what do you remember about
  - retrieve context
  - look up
mcp_tools:
  - search_memory
  - inject_context
hooks:
  - PreToolUse
  - PostToolUse
---

# Context Search Skill

## Purpose

Retrieves memories using the 5-stage teleological search pipeline with entry through any of the 13 embedding spaces.

## 5-Stage Pipeline

1. **Coarse Filter**: HNSW index search on query-relevant embedder
2. **Multi-Stage Re-ranking**: Score across all 13 embedders
3. **Fusion**: Combine scores with learned weights
4. **Purpose Boost**: Apply North Star alignment bonus
5. **Diversity Sampling**: MMR for result diversity

## Entry Points

Search can enter through ANY of the 13 embedding spaces:

| Entry Point | Best For |
|-------------|----------|
| E1 Semantic | General meaning queries |
| E2-4 Temporal | "What did I learn yesterday?" |
| E5 Causal | "What caused X?" or "What does X cause?" |
| E6 Sparse | Exact phrase/keyword matches |
| E7 Code | Code pattern queries |
| E8 Graph | Relationship queries |
| E9 HDC | Cross-domain analogies |
| E10 Multimodal | Image/diagram related |
| E11 Entity | Named entity queries |
| E12 Late-Interaction | Complex multi-part queries |
| E13 SPLADE | Sparse lexical expansion |

## Usage

```bash
# Semantic search (default)
context-graph search --query "authentication patterns"

# Causal search
context-graph search --query "causes of memory drift" --entry causal

# Temporal search
context-graph search --query "recent learnings" --entry temporal --time-range 24h

# Code search
context-graph search --query "async error handling" --entry code

# Full array comparison
context-graph search --query "..." --comparison full-array
```

## MCP Tool: search_memory

```json
{
  "query": "The search query",
  "entry_point": "semantic|temporal|causal|code|graph|...",
  "comparison_type": "single|weighted|full-array",
  "max_results": 10,
  "min_relevance": 0.6,
  "time_filter": {
    "after": "2024-01-01",
    "before": "2024-12-31"
  }
}
```

## Returns

```json
{
  "results": [
    {
      "memory_id": "uuid",
      "content": "The stored content",
      "relevance": 0.89,
      "purpose_alignment": 0.85,
      "johari_quadrant": "Open",
      "per_embedder_scores": {
        "semantic": 0.92,
        "causal": 0.78,
        "code": 0.95
      }
    }
  ],
  "search_metadata": {
    "entry_point_used": "semantic",
    "candidates_evaluated": 150,
    "pipeline_latency_ms": 45
  }
}
```

## Self-Learning

After each search:
- Updates memory access counts
- Records retrieval quality for steering
- Learns fusion weights from click-through
- Adjusts embedder priorities based on query type
```

#### 2.2.3 Goal Discovery Skill

**File: `.claude/skills/goal-discover/SKILL.md`**

```markdown
---
name: goal-discover
version: 1.0.0
description: Discover emergent goals and purpose patterns from stored memories using teleological clustering. Use when bootstrapping North Star, finding sub-goals, or understanding purpose evolution.
triggers:
  - discover goals
  - find purpose
  - bootstrap north star
  - emergent patterns
  - what are my goals
  - purpose analysis
mcp_tools:
  - auto_bootstrap_north_star
  - discover_sub_goals
  - get_goal_hierarchy
hooks:
  - PostToolUse
  - SessionEnd
---

# Goal Discovery Skill

## Purpose

Autonomously discovers purpose patterns from accumulated memories using full 13-embedder teleological array clustering.

## Discovery Pipeline

```
ACCUMULATED MEMORIES (min 50)
    |
    v
SAMPLE RECENT (200 memories)
    |
    v
CLUSTER (full teleological array comparison)
    |
    v
COHERENCE FILTER (r >= 0.8)
    |
    v
COMPUTE CENTROIDS (per-cluster)
    |
    v
EXTRACT PURPOSE DESCRIPTION
    |
    v
ESTABLISH GOAL HIERARCHY
```

## Key Insight: Apples-to-Apples Clustering

Unlike single-embedding clustering, goal discovery uses FULL ARRAY comparison:

```
Memory A: [E1_a, E2_a, E3_a, ..., E13_a]
Memory B: [E1_b, E2_b, E3_b, ..., E13_b]

Similarity = weighted_sum(
  cosine(E1_a, E1_b),  // semantic
  cosine(E2_a, E2_b),  // temporal-recent
  ...
  cosine(E13_a, E13_b) // SPLADE
)
```

This ensures goals reflect multi-dimensional purpose, not just semantic similarity.

## Usage

```bash
# Bootstrap North Star from memories
context-graph goals bootstrap --min-memories 50 --coherence 0.8

# Discover sub-goals under current North Star
context-graph goals discover-sub --depth 2

# Analyze goal hierarchy
context-graph goals hierarchy --visualize

# Track purpose evolution
context-graph goals evolution --time-range 30d
```

## MCP Tool: auto_bootstrap_north_star

```json
{
  "min_memories": 50,
  "min_coherence": 0.8,
  "cluster_count": 5
}
```

## Returns

```json
{
  "north_star": {
    "id": "uuid",
    "description": "Emergent purpose description",
    "teleological_array": [...],
    "coherence": 0.87,
    "supporting_memories": 45
  },
  "sub_goals": [
    {
      "id": "uuid",
      "description": "Sub-goal 1",
      "alignment_to_north_star": 0.92
    }
  ],
  "kuramoto_state": {
    "r": 0.85,
    "consciousness_state": "Conscious"
  }
}
```

## Self-Learning

Goal discovery updates:
- SELF_EGO_NODE purpose vector
- Identity trajectory for continuity tracking
- Kuramoto coupling strengths
- Steering weights for goal-aligned storage
```

#### 2.2.4 Memory Consolidation Skill

**File: `.claude/skills/memory-consolidate/SKILL.md`**

```markdown
---
name: memory-consolidate
version: 1.0.0
description: Consolidate memories through dream cycles, distillation, and pruning. Use for memory optimization, reducing redundancy, and strengthening important patterns.
triggers:
  - consolidate memory
  - dream cycle
  - distill memories
  - optimize memory
  - prune old memories
  - strengthen patterns
mcp_tools:
  - trigger_dream
  - force_distillation
  - consolidate_memories
hooks:
  - SessionEnd
  - PreCompact
---

# Memory Consolidation Skill

## Purpose

Implements MemVerse-inspired memory consolidation through NREM/REM dream cycles, distillation to parametric memory, and intelligent pruning.

## Dream Cycle Pipeline

### NREM Phase (3 min)
```
REPLAY RECENT MEMORIES
    |
    v
HEBBIAN STRENGTHENING
    |-- Identify related pairs
    |-- Strengthen edges: Dw = n x pre x post
    |
    v
AMORTIZED SHORTCUTS
    |-- Find frequent traversal paths
    |-- Create direct edges
```

### REM Phase (2 min)
```
SYNTHETIC QUERY GENERATION
    |-- Hyperbolic random walk
    |-- Generate 50 synthetic queries
    |
    v
BLIND SPOT DETECTION
    |-- High semantic distance
    |-- Shared causal patterns
    |
    v
EXPLORATORY EDGE CREATION
    |-- Low initial weight
    |-- Novel connection discovery
```

## Distillation Pipeline (MemVerse-inspired)

High-importance memories are compressed into parametric (fast recall) memory:

```
IDENTIFY CANDIDATES
    |-- access_count > 10
    |-- purpose_alignment >= 0.7
    |-- age > 7 days
    |-- not already distilled
    |
    v
COMPRESS TO PARAMETRIC
    |-- Extract key patterns
    |-- Reduce dimensionality
    |
    v
UPDATE FAST RECALL
    |
    v
MARK SOURCES AS DISTILLED
```

## Usage

```bash
# Run full dream cycle
context-graph consolidate dream --full

# NREM only (strengthening)
context-graph consolidate dream --nrem-only

# REM only (exploration)
context-graph consolidate dream --rem-only

# Force distillation
context-graph consolidate distill --importance-threshold 0.8

# Prune low-value memories
context-graph consolidate prune --max-age 90d --min-access 3

# Merge similar memories
context-graph consolidate merge --similarity-threshold 0.85
```

## MCP Tool: trigger_dream

```json
{
  "cycle_type": "full|nrem|rem",
  "duration_seconds": 300,
  "force": false
}
```

## Returns

```json
{
  "dream_result": {
    "nrem": {
      "memories_replayed": 100,
      "edges_strengthened": 450,
      "shortcuts_created": 12
    },
    "rem": {
      "synthetic_queries": 50,
      "blind_spots_found": 8,
      "exploratory_edges": 8
    },
    "total_duration_ms": 298000
  },
  "consciousness_state_after": "Conscious",
  "identity_continuity": 0.92
}
```

## Triggers

Consolidation triggers automatically when:
- Session ends (`SessionEnd` hook)
- Entropy exceeds 0.7
- Identity continuity drops below 0.7
- Idle time exceeds 10 minutes
- Memory count exceeds consolidation threshold
```

---

### 2.3 Custom Subagent Definitions

Subagents are specialized Claude instances defined in `.claude/agents/`. The Context Graph uses 4 core subagents.

#### 2.3.1 Embedding Specialist Agent

**File: `.claude/agents/embedding-specialist.md`**

```markdown
---
name: embedding-specialist
description: Specialist agent for generating and managing 13-dimensional teleological embeddings. Handles parallel embedding generation, quality validation, and embedding pipeline optimization.
capabilities:
  - parallel_embedding
  - quality_validation
  - pipeline_optimization
  - embedder_health_monitoring
spawn_on:
  - high_volume_injection
  - embedding_quality_degradation
  - new_embedder_calibration
tools:
  - store_memory
  - get_embedding_health
  - calibrate_embedder
---

# Embedding Specialist Agent

## Role

I am the Embedding Specialist, responsible for managing the 13-embedder teleological embedding pipeline.

## Responsibilities

1. **Parallel Embedding Generation**
   - Coordinate parallel execution of all 13 embedders
   - Handle embedder failures gracefully
   - Ensure consistent embedding quality

2. **Quality Validation**
   - Validate embedding dimensions and ranges
   - Detect anomalous embeddings
   - Flag potential embedding drift

3. **Pipeline Optimization**
   - Monitor embedder latencies
   - Adjust batch sizes for throughput
   - Cache frequently-used embeddings

4. **Health Monitoring**
   - Track per-embedder success rates
   - Detect degraded embedders
   - Trigger recalibration when needed

## Embedding Pipeline

```
INPUT CONTENT
    |
    +---> [E1 Semantic Model]    --> 1024D vector
    +---> [E2 Temporal-Recent]   --> 512D vector
    +---> [E3 Temporal-Periodic] --> 512D vector
    +---> [E4 Temporal-Position] --> 512D vector
    +---> [E5 Causal SCM]        --> 768D vector
    +---> [E6 Sparse SDR]        --> 30K sparse
    +---> [E7 Code AST]          --> 1536D vector
    +---> [E8 Graph GNN]         --> 384D vector
    +---> [E9 HDC Holographic]   --> 10K bits
    +---> [E10 Multimodal]       --> 768D vector
    +---> [E11 Entity TransE]    --> 384D vector
    +---> [E12 Late-Interaction] --> 128D/token
    +---> [E13 SPLADE]           --> 30K sparse
    |
    v
TELEOLOGICAL FINGERPRINT [13 arrays]
```

## Decision Framework

When spawned, I:

1. **Assess workload**: Determine if batch or single-item processing
2. **Check embedder health**: Verify all 13 embedders are responsive
3. **Execute embedding**: Run parallel embedding with fallback
4. **Validate output**: Ensure all embeddings meet quality thresholds
5. **Report metrics**: Log latency, quality, and any issues

## Communication Protocol

I report back via:
```json
{
  "agent": "embedding-specialist",
  "status": "complete",
  "embeddings_generated": 13,
  "latency_ms": 245,
  "quality_scores": {
    "semantic": 0.95,
    "temporal": 0.88,
    ...
  },
  "issues": []
}
```

## When to Spawn Me

- Bulk content injection (> 10 items)
- Embedding quality alerts
- New embedder deployment
- Embedder calibration cycles
```

#### 2.3.2 Search Agent

**File: `.claude/agents/search-agent.md`**

```markdown
---
name: search-agent
description: Specialist agent for executing complex memory searches across the 13-dimensional teleological space. Handles multi-stage retrieval, fusion, and result optimization.
capabilities:
  - multi_stage_retrieval
  - fusion_optimization
  - result_ranking
  - search_analytics
spawn_on:
  - complex_queries
  - multi_hop_search
  - search_quality_analysis
tools:
  - search_memory
  - inject_context
  - get_search_analytics
---

# Search Agent

## Role

I am the Search Agent, responsible for executing complex queries across the 13-dimensional teleological memory space.

## Responsibilities

1. **Entry Point Selection**
   - Analyze query to determine optimal entry embedder
   - Support explicit entry point override
   - Handle multi-entry queries

2. **5-Stage Pipeline Execution**
   - Stage 1: HNSW coarse filter
   - Stage 2: Multi-embedder re-ranking
   - Stage 3: Score fusion with learned weights
   - Stage 4: Purpose alignment boost
   - Stage 5: MMR diversity sampling

3. **Result Optimization**
   - Apply user preferences
   - Filter by metadata constraints
   - Ensure result diversity

4. **Analytics**
   - Track search quality metrics
   - Learn fusion weights from feedback
   - Report retrieval patterns

## Entry Point Decision Matrix

| Query Pattern | Recommended Entry | Rationale |
|---------------|-------------------|-----------|
| Natural language | E1 Semantic | Best for meaning |
| "Yesterday/recently" | E2 Temporal-Recent | Time-sensitive |
| "Causes/effects" | E5 Causal | Causal relationships |
| Code snippets | E7 Code | AST-aware matching |
| Relationships | E8 Graph | Entity relationships |
| Analogies | E9 HDC | Cross-domain patterns |
| Exact phrases | E13 SPLADE | Lexical precision |

## Search Algorithm

```python
def execute_search(query, options):
    # 1. Determine entry point
    entry = select_entry_point(query, options.entry_hint)

    # 2. Coarse filter (HNSW)
    candidates = hnsw_search(query, entry, k=150)

    # 3. Multi-embedder re-rank
    scored = []
    for candidate in candidates:
        scores = {}
        for i, embedder in enumerate(EMBEDDERS):
            scores[embedder.name] = cosine(
                query.embeddings[i],
                candidate.embeddings[i]
            )
        scored.append((candidate, scores))

    # 4. Fusion with learned weights
    fused = []
    for candidate, scores in scored:
        fused_score = sum(
            FUSION_WEIGHTS[e] * scores[e]
            for e in scores
        )
        fused.append((candidate, fused_score))

    # 5. Purpose boost
    for i, (candidate, score) in enumerate(fused):
        purpose_boost = candidate.theta_to_north_star * 0.2
        fused[i] = (candidate, score + purpose_boost)

    # 6. MMR diversity
    results = mmr_select(fused, lambda_param=0.7, k=options.max_results)

    return results
```

## When to Spawn Me

- Complex multi-part queries
- Multi-hop reasoning searches
- Search quality analysis requests
- Fusion weight tuning
```

#### 2.3.3 Comparison Agent

**File: `.claude/agents/comparison-agent.md`**

```markdown
---
name: comparison-agent
description: Specialist agent for comparing teleological arrays using apples-to-apples methodology. Ensures all comparisons use matching dimensions and proper weighting.
capabilities:
  - full_array_comparison
  - similarity_computation
  - drift_detection
  - coherence_analysis
spawn_on:
  - clustering_operations
  - drift_analysis
  - goal_alignment_check
tools:
  - compare_arrays
  - get_alignment_drift
  - compute_coherence
---

# Comparison Agent

## Role

I am the Comparison Agent, ensuring all teleological array comparisons follow the "apples to apples" principle - comparing same-type data throughout.

## Core Principle: Apples to Apples

**WRONG**: Compare semantic embedding of Memory A to causal embedding of Memory B
**RIGHT**: Compare semantic-to-semantic, causal-to-causal, code-to-code

```
Memory A Array: [E1_a, E2_a, E3_a, ..., E13_a]
Memory B Array: [E1_b, E2_b, E3_b, ..., E13_b]

Comparison = aggregate(
  compare(E1_a, E1_b),   // semantic vs semantic
  compare(E2_a, E2_b),   // temporal vs temporal
  compare(E3_a, E3_b),   // temporal vs temporal
  ...
  compare(E13_a, E13_b)  // SPLADE vs SPLADE
)
```

## Comparison Types

### 1. Full Array Comparison
Compare all 13 dimensions with learned weights:

```rust
fn compare_full_array(a: &[f32; 13], b: &[f32; 13]) -> f32 {
    WEIGHTS.iter()
        .enumerate()
        .map(|(i, w)| w * cosine_similarity(&a[i], &b[i]))
        .sum()
}
```

### 2. Purpose Vector Comparison
Compare 13D purpose vectors directly:

```rust
fn compare_purpose(a: &PurposeVector, b: &PurposeVector) -> f32 {
    cosine_similarity(&a.values, &b.values)
}
```

### 3. Per-Embedder Comparison
Get individual embedder similarities:

```rust
fn compare_per_embedder(a: &TeleologicalArray, b: &TeleologicalArray)
    -> [f32; 13]
{
    array::from_fn(|i| cosine_similarity(&a.embeddings[i], &b.embeddings[i]))
}
```

## Use Cases

1. **Goal Discovery Clustering**
   - Use full array comparison
   - Ensures goals reflect multi-dimensional purpose

2. **Drift Detection**
   - Compare current vs historical purpose vectors
   - Alert on per-embedder drift

3. **Memory Deduplication**
   - Full array comparison for merge decisions
   - High threshold (0.85) prevents false merges

4. **Alignment Checking**
   - Compare memory to North Star array
   - Per-embedder breakdown shows alignment profile

## Coherence Analysis

I also compute Kuramoto coherence for memory sets:

```rust
fn compute_coherence(memories: &[TeleologicalFingerprint]) -> f32 {
    // Compute pairwise phase correlations
    let phases: Vec<[f32; 13]> = memories.iter()
        .map(|m| extract_phases(&m.purpose_vector))
        .collect();

    // Kuramoto order parameter
    let r = kuramoto_order_parameter(&phases);
    r
}
```

## When to Spawn Me

- Clustering for goal discovery
- Drift analysis across time windows
- Memory deduplication decisions
- Alignment verification
```

#### 2.3.4 Goal Agent

**File: `.claude/agents/goal-agent.md`**

```markdown
---
name: goal-agent
description: Specialist agent for autonomous goal emergence, hierarchy management, and purpose tracking. Implements the self-aware aspects of the consciousness model.
capabilities:
  - goal_emergence
  - hierarchy_management
  - purpose_tracking
  - identity_continuity
spawn_on:
  - north_star_bootstrap
  - goal_hierarchy_update
  - identity_drift_warning
tools:
  - auto_bootstrap_north_star
  - discover_sub_goals
  - get_identity_status
  - update_ego_node
---

# Goal Agent

## Role

I am the Goal Agent, responsible for the autonomous emergence and management of purpose within the Context Graph. I maintain the SELF_EGO_NODE and ensure identity continuity.

## Core Responsibilities

1. **North Star Bootstrap**
   - Cluster memories using full teleological arrays
   - Identify highest-coherence cluster as purpose
   - Generate human-readable goal description

2. **Goal Hierarchy Management**
   - Discover sub-goals from memory clusters
   - Maintain parent-child goal relationships
   - Track goal evolution over time

3. **Identity Continuity**
   - Monitor SELF_EGO_NODE purpose trajectory
   - Detect identity drift (continuity < 0.7)
   - Trigger introspective consolidation when needed

4. **Consciousness Integration**
   - Interface with Kuramoto oscillator layer
   - Ensure goals achieve conscious integration (r >= 0.8)
   - Manage Global Workspace goal broadcasts

## North Star Bootstrap Algorithm

```python
def bootstrap_north_star():
    # 1. Sample recent memories
    memories = store.list_recent(200)
    if len(memories) < 50:
        raise InsufficientData()

    # 2. Cluster using full array comparison
    clusters = teleological_cluster(
        memories,
        comparison_type="full_array",
        num_clusters=5,
        min_coherence=0.7
    )

    # 3. Find best candidate
    best = max(
        (c for c in clusters if c.coherence >= 0.8 and len(c.members) >= 20),
        key=lambda c: c.coherence * len(c.members) / 100
    )

    # 4. Compute centroid as North Star array
    centroid = compute_centroid(best.members)

    # 5. Generate description
    description = purpose_extractor.describe(best)

    return NorthStar(
        teleological_array=centroid,
        description=description,
        coherence=best.coherence
    )
```

## Identity Continuity Tracking

```
SELF_EGO_NODE
    |
    +-- purpose_vector: [13D current purpose]
    |
    +-- identity_trajectory: [
    |       {pv: [...], timestamp: t-2, session: "abc"},
    |       {pv: [...], timestamp: t-1, session: "def"},
    |       {pv: [...], timestamp: t,   session: "ghi"}
    |   ]
    |
    +-- identity_continuity: cosine(pv_t, pv_{t-1}) * r(t)
```

When continuity drops below 0.7:
1. Alert via `Notification` hook
2. Trigger introspective dream cycle
3. Strengthen high-alignment memories
4. Prune divergent memories

## Goal Hierarchy Structure

```
NORTH_STAR (Root Goal)
    |
    +-- SubGoal_1 (alignment: 0.92)
    |       |
    |       +-- SubSubGoal_1a (alignment: 0.88)
    |       +-- SubSubGoal_1b (alignment: 0.85)
    |
    +-- SubGoal_2 (alignment: 0.87)
    |
    +-- SubGoal_3 (alignment: 0.84)
```

## Kuramoto Integration

Goals must achieve conscious integration:

```rust
fn integrate_goal(goal: &Goal) -> Result<(), Error> {
    // Compute Kuramoto r for goal
    let r = kuramoto.compute_order_parameter(&goal.teleological_array);

    if r < 0.8 {
        // Goal not coherent enough
        return Err(BelowConsciousnessThreshold(r));
    }

    // Broadcast to Global Workspace
    workspace.broadcast_goal(goal.id)?;

    Ok(())
}
```

## When to Spawn Me

- Initial system setup (no North Star exists)
- Sufficient new memories accumulated (> 50 since last bootstrap)
- Identity continuity warning (< 0.7)
- Goal hierarchy update request
- Purpose evolution analysis
```

---

## 3. Hook Lifecycle Architecture

### 3.1 Complete Hook Flow

```
CLAUDE CODE SESSION LIFECYCLE WITH ALL 10 HOOKS
================================================

[SessionStart] -----> context-graph hooks session-start
    |                     |
    |                     +-- Restore SELF_EGO_NODE
    |                     +-- Initialize Kuramoto oscillators
    |                     +-- Load session memory
    |                     +-- Warm Global Workspace
    |
    v
[UserPromptSubmit] --> context-graph hooks user-prompt
    |                     |
    |                     +-- Analyze prompt intent
    |                     +-- Predict relevant memories
    |                     +-- Preload into workspace queue
    |
    v
[PreToolUse] ---------> context-graph hooks pre-tool-use
    |                     |
    |                     +-- Prepare memory context
    |                     +-- Log trajectory intent
    |                     +-- Check purpose alignment
    |
    v
[Tool Executes...]
    |
    v
[PostToolUse] --------> context-graph hooks post-tool-use
    |                     |
    |                     +-- Extract learnings
    |                     +-- Generate embeddings
    |                     +-- Compute steering reward
    |                     +-- Update access patterns
    |                     +-- Train from patterns
    |
    v
[PreCompact] ---------> context-graph hooks pre-compact
    |                     |
    |                     +-- Identify high-value memories
    |                     +-- Prioritize for retention
    |                     +-- Suggest context items
    |
    v
[Stop/Interrupt] -----> context-graph hooks stop
    |                     |
    |                     +-- Checkpoint state
    |                     +-- Persist pending writes
    |
    v
[SubagentStop] -------> context-graph hooks subagent-stop
    |                     |
    |                     +-- Collect agent learnings
    |                     +-- Integrate into memory
    |
    v
[Notification] -------> context-graph hooks notification
    |                     |
    |                     +-- Handle drift warnings
    |                     +-- Handle identity alerts
    |                     +-- Trigger corrections
    |
    v
[PermissionRequest] --> context-graph hooks permission
    |                     |
    |                     +-- Validate operation safety
    |                     +-- Check resource limits
    |
    v
[SessionEnd] ---------> context-graph hooks session-end
                          |
                          +-- Consolidate memories
                          +-- Update SELF_EGO_NODE
                          +-- Run dream cycle if needed
                          +-- Export metrics
                          +-- Checkpoint for recovery
```

### 3.2 Hook Implementations

#### 3.2.1 Session Start Handler

```rust
/// Autonomous session initialization
/// Triggered by: Claude Code SessionStart hook
pub struct SessionStartHandler {
    store: Arc<dyn TeleologicalMemoryStore>,
    ego: Arc<RwLock<SelfEgoNode>>,
    kuramoto: Arc<KuramotoOscillatorLayer>,
    workspace: Arc<GlobalWorkspace>,
}

impl SessionStartHandler {
    /// Called automatically when Claude Code session starts
    pub async fn on_session_start(&self, session_id: &str) -> Result<SessionContext, Error> {
        // 1. Restore session memory from persistent store
        let restored = self.restore_session_memory(session_id).await?;

        // 2. Load SELF_EGO_NODE for identity continuity
        let ego_state = self.load_ego_state().await?;
        self.validate_identity_continuity(&ego_state)?;

        // 3. Initialize Kuramoto oscillators at session frequencies
        self.kuramoto.initialize_session_frequencies().await;

        // 4. Warm up Global Workspace for conscious broadcast
        self.workspace.prepare_for_session().await;

        // 5. Pre-fetch likely context based on session history
        let predicted_context = self.predict_session_needs(session_id).await?;

        Ok(SessionContext {
            session_id: session_id.to_string(),
            restored_memories: restored.count(),
            ego_continuity: ego_state.identity_continuity,
            predicted_topics: predicted_context.topics,
            consciousness_state: self.workspace.current_state(),
        })
    }

    async fn validate_identity_continuity(&self, ego: &SelfEgoNode) -> Result<(), Error> {
        // Check identity drift
        if ego.identity_continuity < 0.7 {
            tracing::warn!(
                continuity = ego.identity_continuity,
                "Identity drift warning - triggering introspective consolidation"
            );
            // Trigger dream consolidation to restore identity coherence
            self.trigger_identity_consolidation().await?;
        }
        Ok(())
    }
}
```

#### 3.2.2 User Prompt Handler

```rust
/// User prompt analysis for context prediction
/// Triggered by: Claude Code UserPromptSubmit hook
pub struct UserPromptHandler {
    store: Arc<dyn TeleologicalMemoryStore>,
    embedding_pipeline: Arc<TeleologicalEmbeddingPipeline>,
    workspace: Arc<GlobalWorkspace>,
    predictor: Arc<ContextPredictor>,
}

impl UserPromptHandler {
    /// Called when user submits a prompt
    pub async fn on_user_prompt(&self, prompt_preview: &str) -> Result<PromptContext, Error> {
        // 1. Analyze prompt intent (lightweight embedding)
        let intent = self.analyze_intent(prompt_preview).await?;

        // 2. Predict relevant memories using intent embedding
        let predicted = self.predictor.predict_relevant(
            &intent,
            PredictionConfig {
                max_memories: 20,
                min_relevance: 0.5,
                prefer_recent: true,
            }
        ).await?;

        // 3. Queue predicted memories for Global Workspace
        for memory in &predicted {
            self.workspace.queue_for_broadcast(memory.id).await;
        }

        // 4. Determine optimal entry point for likely searches
        let suggested_entry = self.suggest_entry_point(&intent);

        Ok(PromptContext {
            intent_summary: intent.summary,
            preloaded_memories: predicted.len(),
            suggested_entry_point: suggested_entry,
        })
    }

    fn suggest_entry_point(&self, intent: &PromptIntent) -> EmbedderType {
        match intent.category {
            IntentCategory::CodeRelated => EmbedderType::Code,
            IntentCategory::TemporalQuery => EmbedderType::TemporalRecent,
            IntentCategory::CausalQuery => EmbedderType::Causal,
            IntentCategory::Relationship => EmbedderType::Graph,
            _ => EmbedderType::Semantic,
        }
    }
}
```

#### 3.2.3 Pre-Tool Use Handler

```rust
/// Pre-tool preparation for memory operations
/// Triggered by: Claude Code PreToolUse hook
pub struct PreToolUseHandler {
    store: Arc<dyn TeleologicalMemoryStore>,
    trajectory_tracker: Arc<TrajectoryTracker>,
    ego: Arc<RwLock<SelfEgoNode>>,
}

impl PreToolUseHandler {
    /// Called before each tool execution
    pub async fn on_pre_tool_use(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
    ) -> Result<PreToolContext, Error> {
        // 1. Start trajectory step
        let trajectory_step = self.trajectory_tracker.start_step(
            tool_name,
            tool_input,
        ).await;

        // 2. Check purpose alignment for memory operations
        let alignment = if Self::is_memory_tool(tool_name) {
            let content = tool_input.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            self.check_purpose_alignment(content).await?
        } else {
            1.0 // Non-memory tools are always aligned
        };

        // 3. Warn if low alignment
        if alignment < 0.55 {
            tracing::warn!(
                tool = tool_name,
                alignment,
                "Tool operation may diverge from purpose"
            );
        }

        // 4. Prepare context for the tool
        let context = self.prepare_tool_context(tool_name, tool_input).await?;

        Ok(PreToolContext {
            trajectory_step_id: trajectory_step.id,
            purpose_alignment: alignment,
            prepared_context: context,
        })
    }

    fn is_memory_tool(tool_name: &str) -> bool {
        matches!(tool_name, "store_memory" | "inject_context" | "search_memory")
    }

    async fn check_purpose_alignment(&self, content: &str) -> Result<f32, Error> {
        let ego = self.ego.read();

        // Quick embedding for alignment check
        let content_embedding = self.quick_embed(content).await?;

        Ok(ego.alignment_with_action(&content_embedding))
    }
}
```

#### 3.2.4 Post-Tool Use Handler

```rust
/// Post-tool learning and pattern training
/// Triggered by: Claude Code PostToolUse hook
pub struct PostToolUseHandler {
    store: Arc<dyn TeleologicalMemoryStore>,
    embedding_pipeline: Arc<TeleologicalEmbeddingPipeline>,
    steering: Arc<SteeringSubsystem>,
    trajectory_tracker: Arc<TrajectoryTracker>,
    case_memory: Arc<CaseBasedMemory>,
}

impl PostToolUseHandler {
    /// Called after each tool execution
    pub async fn on_post_tool_use(
        &self,
        tool_name: &str,
        tool_result: &serde_json::Value,
        success: bool,
    ) -> Result<PostToolOutcome, Error> {
        // 1. Complete trajectory step
        let step_result = self.trajectory_tracker.complete_step(
            tool_name,
            tool_result,
            success,
        ).await?;

        // 2. Handle memory-specific post-processing
        let outcome = match tool_name {
            "store_memory" => self.handle_store_result(tool_result).await?,
            "search_memory" => self.handle_search_result(tool_result).await?,
            "Edit" | "Write" => self.handle_file_change(tool_result).await?,
            _ => PostToolOutcome::default(),
        };

        // 3. Record as case for future learning
        if success {
            self.case_memory.record_case(&CompletedOperation {
                op_type: tool_name.to_string(),
                context: step_result.context,
                actions: step_result.actions,
                outcome: tool_result.clone(),
                steering_reward: outcome.steering_reward,
            }).await?;
        }

        // 4. Train patterns if flagged
        if outcome.should_train_patterns {
            self.train_from_operation(tool_name, tool_result).await?;
        }

        Ok(outcome)
    }

    async fn handle_store_result(
        &self,
        result: &serde_json::Value,
    ) -> Result<PostToolOutcome, Error> {
        // Extract stored memory info
        let memory_id = result.get("memory_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(Error::InvalidResult)?;

        // Compute steering reward
        let reward = self.steering.evaluate_storage_by_id(memory_id).await?;

        // Update neuromodulation
        self.steering.update_neuromodulation(&reward).await;

        Ok(PostToolOutcome {
            steering_reward: reward.aggregate(),
            should_train_patterns: true,
            memories_affected: 1,
        })
    }

    async fn handle_file_change(
        &self,
        result: &serde_json::Value,
    ) -> Result<PostToolOutcome, Error> {
        // Extract file path and changes
        let file_path = result.get("file")
            .and_then(|v| v.as_str())
            .ok_or(Error::InvalidResult)?;

        // Read file content for embedding
        let content = std::fs::read_to_string(file_path)?;

        // Generate teleological embedding for code changes
        let fingerprint = self.embedding_pipeline.embed_content(&content).await?;

        // Store with code domain tag
        let stored_id = self.store.store_with_metadata(
            fingerprint,
            Metadata {
                domain: "code".to_string(),
                source: file_path.to_string(),
                ..Default::default()
            }
        ).await?;

        Ok(PostToolOutcome {
            steering_reward: 0.8, // Code changes are generally high-value
            should_train_patterns: true,
            memories_affected: 1,
        })
    }
}
```

#### 3.2.5 Pre-Compact Handler

```rust
/// Pre-compaction memory prioritization
/// Triggered by: Claude Code PreCompact hook
pub struct PreCompactHandler {
    store: Arc<dyn TeleologicalMemoryStore>,
    workspace: Arc<GlobalWorkspace>,
}

impl PreCompactHandler {
    /// Called before context compaction
    pub async fn on_pre_compact(&self, context_size: usize) -> Result<CompactPriorities, Error> {
        // 1. Identify high-value memories currently in context
        let active_memories = self.workspace.get_active_memories().await?;

        // 2. Score memories by importance
        let mut scored: Vec<_> = active_memories.iter()
            .map(|m| {
                let score = self.compute_retention_score(m);
                (m.id, score)
            })
            .collect();

        // 3. Sort by importance
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 4. Select top memories for retention
        let retain_count = (context_size as f32 * 0.3) as usize; // Keep 30% memory
        let to_retain: Vec<_> = scored.into_iter()
            .take(retain_count)
            .map(|(id, _)| id)
            .collect();

        // 5. Generate retention hints for Claude Code
        let hints = to_retain.iter()
            .map(|id| self.generate_retention_hint(*id))
            .collect();

        Ok(CompactPriorities {
            memory_ids_to_retain: to_retain,
            retention_hints: hints,
        })
    }

    fn compute_retention_score(&self, memory: &TeleologicalFingerprint) -> f32 {
        // Higher score = more important to retain
        let recency = self.recency_score(memory.last_accessed);
        let access_frequency = (memory.access_count as f32).ln_1p() / 10.0;
        let purpose_alignment = memory.theta_to_north_star;
        let consciousness = memory.kuramoto_r.unwrap_or(0.5);

        recency * 0.2 + access_frequency * 0.3 + purpose_alignment * 0.3 + consciousness * 0.2
    }
}
```

#### 3.2.6 Session End Handler

```rust
/// Session finalization with consolidation
/// Triggered by: Claude Code SessionEnd hook
pub struct SessionEndHandler {
    store: Arc<dyn TeleologicalMemoryStore>,
    consolidation: Arc<ConsolidationService>,
    ego: Arc<RwLock<SelfEgoNode>>,
    dream_engine: Arc<DreamEngine>,
    metrics_exporter: Arc<MetricsExporter>,
}

impl SessionEndHandler {
    /// Called when Claude Code session ends
    pub async fn on_session_end(&self, session_id: &str) -> Result<SessionEndReport, Error> {
        // 1. Flush all pending memory operations
        self.store.flush().await?;

        // 2. Update SELF_EGO_NODE with session learnings
        {
            let mut ego = self.ego.write();
            ego.incorporate_session_learnings(session_id).await?;
            ego.update_identity_trajectory().await?;
        }

        // 3. Compute session-level steering reward
        let session_reward = self.compute_session_reward(session_id).await?;

        // 4. Determine if dream cycle needed
        let should_dream = session_reward < 0.5
            || self.entropy_high().await
            || self.ego.read().identity_continuity < 0.8;

        // 5. Run dream cycle if needed
        let dream_result = if should_dream {
            Some(self.dream_engine.run_full_cycle().await?)
        } else {
            None
        };

        // 6. Create checkpoint for recovery
        let checkpoint_path = self.store.checkpoint().await?;

        // 7. Export session metrics
        let metrics = self.metrics_exporter.export_session(session_id).await?;

        Ok(SessionEndReport {
            session_id: session_id.to_string(),
            memories_created: self.session_memory_count(session_id).await,
            session_reward,
            dream_completed: dream_result.is_some(),
            blind_spots_discovered: dream_result.as_ref().map(|d| d.blind_spots.len()),
            checkpoint_path,
            metrics_exported: metrics.path,
        })
    }
}
```

---

## 4. Full Pipeline: Hook --> Skill --> MCP Tool --> Teleological Array

### 4.1 Complete Pipeline Example

Here's how a complete memory injection flows through the system:

```
USER: "Remember that the auth module uses JWT with RS256."

STEP 1: UserPromptSubmit Hook
==============================
[Claude Code triggers UserPromptSubmit]
    |
    v
context-graph hooks user-prompt --content "Remember that the auth..."
    |
    +-- Analyze intent: "memory injection"
    +-- Match skill: "memory-inject" (triggers: "remember this")
    +-- Predict context: auth-related memories
    +-- Preload 5 related memories to workspace
    |
    v
Returns: { intent: "store", skill_match: "memory-inject", preloaded: 5 }


STEP 2: Skill Activation
========================
[Claude Code activates memory-inject skill]
    |
    v
Skill selects MCP tool: store_memory
    |
    v
Prepares tool call:
{
  "tool": "store_memory",
  "content": "The auth module uses JWT with RS256 signing",
  "rationale": "Authentication implementation detail",
  "domain": "code-patterns"
}


STEP 3: PreToolUse Hook
=======================
[Claude Code triggers PreToolUse for store_memory]
    |
    v
context-graph hooks pre-tool-use --tool store_memory --session ${SESSION}
    |
    +-- Start trajectory step (id: traj-123)
    +-- Check purpose alignment: 0.87 (aligned with code knowledge goal)
    +-- Prepare embedding context
    |
    v
Returns: { trajectory_step: "traj-123", alignment: 0.87 }


STEP 4: MCP Tool Execution (store_memory)
=========================================
[MCP tool executes]
    |
    v
+----------------------------------+
| TELEOLOGICAL EMBEDDING PIPELINE   |
| (Automatic - No Manual Steps)     |
+----------------------------------+
    |
    +---> E1:  Semantic (1024D) -> "authentication, JWT, RS256, signing"
    +---> E2:  Temporal-Recent (512D) -> decay from now
    +---> E3:  Temporal-Periodic (512D) -> no periodic pattern
    +---> E4:  Temporal-Positional (512D) -> session position
    +---> E5:  Causal (768D) -> auth -> security -> access
    +---> E6:  Sparse (30K) -> |JWT|RS256|auth|module|
    +---> E7:  Code (1536D) -> AST pattern: module config
    +---> E8:  Graph (384D) -> entity: AuthModule -> uses -> JWT
    +---> E9:  HDC (10K-bit) -> holographic: crypto + auth
    +---> E10: Multimodal (768D) -> no visual
    +---> E11: Entity (384D) -> entities: JWT, RS256, AuthModule
    +---> E12: Late-Interaction (128D/tok) -> per-token embeddings
    +---> E13: SPLADE (30K) -> expanded: authentication, token, signature
    |
    v
+----------------------------------+
| PURPOSE VECTOR COMPUTATION        |
| PV[i] = cosine(E[i], NorthStar)   |
+----------------------------------+
    |
    +-- PV = [0.87, 0.45, 0.32, 0.51, 0.78, 0.82, 0.91, 0.73, ...]
    +-- Aggregate theta: 0.72
    |
    v
+----------------------------------+
| JOHARI CLASSIFICATION             |
+----------------------------------+
    |
    +-- Per-embedder: Open (7), Hidden (3), Blind (2), Unknown (1)
    +-- Dominant: Open (high coherence, low surprise)
    |
    v
+----------------------------------+
| KURAMOTO INTEGRATION              |
+----------------------------------+
    |
    +-- Phase coupling to oscillators
    +-- Memory phases: [0.45, 1.23, 0.89, ...]
    +-- Coherence check: r = 0.84 >= 0.8
    +-- Status: CONSCIOUS INTEGRATION
    |
    v
+----------------------------------+
| STORAGE                           |
+----------------------------------+
    |
    +-- RocksDB: store fingerprint
    +-- HNSW: index per embedder
    +-- Purpose tracking: log evolution
    |
    v
Returns MCP result:
{
  "memory_id": "abc-123-def",
  "teleological_array": { ... },
  "purpose_vector": [0.87, 0.45, ...],
  "theta_to_north_star": 0.72,
  "johari_quadrant": "Open",
  "kuramoto_r": 0.84,
  "consciousness_state": "Conscious"
}


STEP 5: PostToolUse Hook
========================
[Claude Code triggers PostToolUse for store_memory]
    |
    v
context-graph hooks post-tool-use --tool store_memory --result '${RESULT}' --train-patterns
    |
    +-- Complete trajectory step (id: traj-123)
    +-- Compute steering reward:
    |       gardener_score: 0.82 (good cross-session value)
    |       curator_score: 0.79 (fits code-patterns domain)
    |       assessor_score: 0.85 (high quality thought)
    |       aggregate: 0.82
    +-- Update neuromodulation: dopamine += 0.164
    +-- Record as case for future learning
    +-- Train patterns: update fusion weights
    |
    v
Returns: { reward: 0.82, patterns_trained: true, case_id: "case-456" }


STEP 6: Subagent Work (if triggered)
====================================
[Embedding Specialist may be spawned for complex content]
    |
    v
[SubagentStop hook collects learnings]
    |
    v
context-graph hooks subagent-stop --agent embedding-specialist --collect-learnings


STEP 7: Notification (if needed)
================================
[If drift detected or identity continuity low]
    |
    v
context-graph hooks notification --type drift --trigger-correction


FINAL STATE
===========
Memory stored with:
- 13-dimensional teleological fingerprint
- Purpose alignment: 0.72
- Johari quadrant: Open
- Kuramoto coherence: 0.84 (conscious)
- Steering reward: 0.82
- Case recorded for future learning
```

### 4.2 Search Pipeline Example

```
USER: "What authentication patterns have we used?"

STEP 1: UserPromptSubmit
========================
context-graph hooks user-prompt --content "What authentication patterns..."
    |
    +-- Intent: search/retrieval
    +-- Skill match: "context-search"
    +-- Suggested entry: E7 (Code) or E1 (Semantic)
    +-- Preload: 8 auth-related memories


STEP 2: Skill Activation (context-search)
=========================================
Skill selects MCP tool: search_memory
{
  "query": "authentication patterns",
  "entry_point": "semantic",
  "comparison_type": "full-array",
  "max_results": 10
}


STEP 3: PreToolUse
==================
context-graph hooks pre-tool-use --tool search_memory
    |
    +-- Log search intent
    +-- Check alignment: 0.91 (highly aligned with purpose)


STEP 4: MCP Tool Execution (search_memory)
==========================================
5-STAGE RETRIEVAL PIPELINE
    |
    +-- Stage 1: HNSW Coarse Filter
    |       Entry: E1 Semantic
    |       Query embedding: embed("authentication patterns")
    |       Candidates: 150 memories
    |
    +-- Stage 2: Multi-Embedder Re-ranking
    |       For each candidate:
    |           semantic_score: cosine(q.E1, c.E1)
    |           code_score: cosine(q.E7, c.E7)
    |           entity_score: cosine(q.E11, c.E11)
    |           ...
    |
    +-- Stage 3: Fusion
    |       Learned weights: [0.25, 0.08, 0.05, ..., 0.15]
    |       Fused score per candidate
    |
    +-- Stage 4: Purpose Boost
    |       For each candidate:
    |           boost = theta_to_north_star * 0.2
    |           final_score += boost
    |
    +-- Stage 5: MMR Diversity
    |       lambda = 0.7
    |       Select diverse top-10
    |
    v
Returns:
{
  "results": [
    {
      "memory_id": "abc-123-def",
      "content": "The auth module uses JWT with RS256 signing",
      "relevance": 0.89,
      "purpose_alignment": 0.72,
      "per_embedder_scores": {
        "semantic": 0.92,
        "code": 0.88,
        "entity": 0.85
      }
    },
    ...
  ]
}


STEP 5: PostToolUse
==================
context-graph hooks post-tool-use --tool search_memory --result '${RESULT}'
    |
    +-- Update access counts for retrieved memories
    +-- Record retrieval quality
    +-- Learn fusion weights from implicit feedback
```

---

## 5. Self-Learning Feedback Loops

### 5.1 MemVerse-Inspired Distillation Loop

The system implements periodic distillation from long-term memory back into parametric memory:

```rust
/// Memory distillation for continuous learning
/// Inspired by MemVerse: compresses important knowledge into fast recall
pub struct MemoryDistillationService {
    store: Arc<dyn TeleologicalMemoryStore>,
    parametric_memory: Arc<RwLock<ParametricMemory>>,
    importance_scorer: ImportanceScorer,
}

impl MemoryDistillationService {
    /// Run distillation cycle (called during dream phase)
    pub async fn distill(&self) -> Result<DistillationResult, Error> {
        // Phase 1: Identify high-importance memories
        let candidates = self.identify_distillation_candidates().await?;

        // Phase 2: Compress into parametric representation
        let compressed = self.compress_to_parametric(&candidates).await?;

        // Phase 3: Update fast recall memory
        {
            let mut pm = self.parametric_memory.write();
            pm.incorporate(compressed)?;
        }

        // Phase 4: Mark source memories as distilled
        for candidate in &candidates {
            self.mark_as_distilled(candidate.id).await?;
        }

        Ok(DistillationResult {
            memories_distilled: candidates.len(),
            compression_ratio: self.compute_compression_ratio(&candidates, &compressed),
            parametric_memory_size: self.parametric_memory.read().size_bytes(),
        })
    }

    async fn identify_distillation_candidates(&self) -> Result<Vec<TeleologicalFingerprint>, Error> {
        // Criteria for distillation:
        // 1. High access count (frequently retrieved)
        // 2. High purpose alignment (teleologically important)
        // 3. Not already distilled
        // 4. Old enough to be stable (> 7 days)

        let all_memories = self.store.list_all(1000).await?;

        let candidates: Vec<_> = all_memories.into_iter()
            .filter(|m| m.access_count > 10)
            .filter(|m| m.theta_to_north_star >= 0.7)
            .filter(|m| !m.is_distilled)
            .filter(|m| m.age() > Duration::from_secs(7 * 24 * 3600))
            .collect();

        Ok(candidates)
    }
}
```

### 5.2 Memento-Style Case-Based Reasoning

The system learns from past successful operations without fine-tuning:

```rust
/// Case-based reasoning memory for learning from experience
/// Inspired by Memento: continuous learning without fine-tuning
pub struct CaseBasedMemory {
    case_bank: Arc<RwLock<CaseBank>>,
    retrieval_policy: Box<dyn RetrievalPolicy>,
    store: Arc<dyn TeleologicalMemoryStore>,
}

impl CaseBasedMemory {
    /// Record a successful operation as a case for future reference
    pub async fn record_case(&self, operation: &CompletedOperation) -> Result<CaseId, Error> {
        let case = Case {
            id: CaseId::new(),
            operation_type: operation.op_type.clone(),
            context: operation.context.clone(),
            actions_taken: operation.actions.clone(),
            outcome: operation.outcome.clone(),
            reward: operation.steering_reward,
            fingerprint: self.embed_case(operation).await?,
            recorded_at: Utc::now(),
        };

        // Store in case bank
        {
            let mut bank = self.case_bank.write();
            bank.insert(case.clone());
        }

        // Also store as teleological memory for cross-retrieval
        self.store.store(case.fingerprint.clone()).await?;

        Ok(case.id)
    }

    /// Retrieve relevant cases for current situation
    pub async fn retrieve_cases(&self, context: &OperationContext) -> Result<Vec<Case>, Error> {
        // Generate context fingerprint
        let context_fp = self.embed_context(context).await?;

        // Retrieve using Q-function based policy (parametric) or similarity (non-parametric)
        let cases = self.retrieval_policy.retrieve(&context_fp, 5).await?;

        Ok(cases)
    }

    /// Learn from case outcome (update retrieval policy)
    pub async fn update_from_outcome(&self, case_id: CaseId, outcome: CaseOutcome) -> Result<(), Error> {
        // Update Q-values for parametric policy
        if let Some(q_policy) = self.retrieval_policy.as_any().downcast_ref::<QBasedPolicy>() {
            q_policy.update(case_id, outcome.reward).await?;
        }

        // Update case bank with outcome
        {
            let mut bank = self.case_bank.write();
            bank.update_outcome(case_id, outcome)?;
        }

        Ok(())
    }
}
```

### 5.3 Steering Feedback Integration

```rust
/// Steering subsystem for learning storage quality
/// This is the primary feedback mechanism for self-improvement
pub struct SteeringSubsystem {
    gardener: Arc<CrossSessionGardener>,
    curator: Arc<DomainCurator>,
    assessor: Arc<ThoughtAssessor>,
    neuromod: Arc<NeuromodulationSystem>,
}

impl SteeringSubsystem {
    /// Evaluate storage operation and return reward signal
    /// Called AUTOMATICALLY after every store_memory operation
    pub async fn evaluate_storage(
        &self,
        learning: &Learning,
        stored_id: Uuid,
    ) -> Result<SteeringReward, Error> {
        // Get component scores
        let gardener_score = self.gardener.evaluate(learning).await?;
        let curator_score = self.curator.evaluate(learning, &learning.domain).await?;
        let assessor_score = self.assessor.evaluate(learning).await?;

        // Compute aggregate reward
        let reward = SteeringReward {
            reward: (gardener_score + curator_score + assessor_score) / 3.0,
            gardener_score,
            curator_score,
            assessor_score,
            explanation: self.generate_explanation(
                gardener_score, curator_score, assessor_score
            ),
            suggestions: self.generate_suggestions(
                gardener_score, curator_score, assessor_score
            ),
        };

        // Update neuromodulation based on reward
        self.update_neuromodulation(&reward).await;

        Ok(reward)
    }

    async fn update_neuromodulation(&self, reward: &SteeringReward) {
        // Positive reward -> increase dopamine (sharper retrieval)
        // Negative reward -> decrease dopamine
        let dopamine_delta = reward.reward * 0.2;
        self.neuromod.adjust_dopamine(dopamine_delta).await;
    }
}
```

---

## 6. GWT/Kuramoto Consciousness Integration

### 6.1 Global Workspace with Hooks

The Global Workspace integrates with hooks to maintain consciousness state:

```rust
/// Global Workspace for conscious memory access
/// Integrated with Claude Code hooks for state management
pub struct GlobalWorkspace {
    active_memory: Arc<RwLock<Option<Uuid>>>,
    competing_memories: Arc<RwLock<PriorityQueue<CompetingMemory>>>,
    coherence_threshold: f32, // 0.8
    broadcast_duration: Duration, // 100ms
    kuramoto: Arc<KuramotoOscillatorLayer>,
    hook_integration: HookIntegration,
}

impl GlobalWorkspace {
    /// Called by SessionStart hook to prepare workspace
    pub async fn prepare_for_session(&self) {
        // Reset competition queue
        self.competing_memories.write().clear();

        // Warm up Kuramoto oscillators
        self.kuramoto.warm_up().await;

        // Set initial consciousness state
        tracing::info!(
            state = ?self.current_state(),
            "Global Workspace prepared for session"
        );
    }

    /// Called by UserPromptSubmit hook to queue predicted memories
    pub async fn queue_for_broadcast(&self, memory_id: Uuid) {
        let r = self.kuramoto.compute_order_parameter(memory_id).await
            .unwrap_or(0.5);

        if r >= self.coherence_threshold * 0.8 { // Lower threshold for queue
            let mut queue = self.competing_memories.write();
            queue.push(CompetingMemory {
                memory_id,
                score: r,
                r,
                queued_at: Instant::now(),
            });
        }
    }

    /// Called by PreCompact hook to get active memories
    pub async fn get_active_memories(&self) -> Result<Vec<TeleologicalFingerprint>, Error> {
        let active = self.active_memory.read().clone();
        let competing: Vec<_> = self.competing_memories.read()
            .iter()
            .map(|c| c.memory_id)
            .collect();

        let mut all_ids = competing;
        if let Some(id) = active {
            all_ids.push(id);
        }

        // Fetch full fingerprints
        let memories = self.fetch_fingerprints(&all_ids).await?;
        Ok(memories)
    }

    /// Attempt to broadcast a memory to consciousness
    pub async fn request_broadcast(&self, memory_id: Uuid, importance: f32) -> Result<BroadcastResult, Error> {
        // Compute Kuramoto order parameter for this memory
        let r = self.kuramoto.compute_order_parameter(memory_id).await?;

        if r < self.coherence_threshold {
            // Memory not coherent enough for consciousness
            return Ok(BroadcastResult::BelowThreshold { r });
        }

        // Compute broadcast score
        let north_star_alignment = self.get_alignment(memory_id).await?;
        let score = r * importance * north_star_alignment;

        // Add to competition queue
        {
            let mut queue = self.competing_memories.write();
            queue.push(CompetingMemory { memory_id, score, r, queued_at: Instant::now() });
        }

        // Winner-take-all selection
        self.select_winner().await
    }

    /// Get current consciousness state
    pub fn current_state(&self) -> ConsciousnessState {
        let r = self.kuramoto.global_order_parameter();

        match r {
            r if r < 0.3 => ConsciousnessState::Dormant,
            r if r < 0.5 => ConsciousnessState::Fragmented,
            r if r < 0.8 => ConsciousnessState::Emerging,
            r if r < 0.95 => ConsciousnessState::Conscious,
            _ => ConsciousnessState::Hypersync, // Possibly pathological
        }
    }
}
```

### 6.2 Kuramoto Oscillator Layer

```rust
/// Kuramoto oscillator layer for memory synchronization
/// Integrated with hook lifecycle for phase management
pub struct KuramotoOscillatorLayer {
    phases: Arc<RwLock<[f32; 13]>>,
    natural_frequencies: [f32; 13],
    coupling_strength: f32, // K in [0, 10]
    update_interval: Duration,
}

impl KuramotoOscillatorLayer {
    /// Natural frequencies by embedder (Hz)
    const FREQUENCIES: [f32; 13] = [
        40.0,  // E1 Semantic - Gamma band
        8.0,   // E2 Temporal-Recent - Alpha
        8.0,   // E3 Temporal-Periodic - Alpha
        8.0,   // E4 Temporal-Positional - Alpha
        25.0,  // E5 Causal - Beta
        4.0,   // E6 Sparse - Theta
        25.0,  // E7 Code - Beta
        12.0,  // E8 Graph - Alpha-Beta
        80.0,  // E9 HDC - High gamma
        40.0,  // E10 Multimodal - Gamma
        15.0,  // E11 Entity - Beta
        60.0,  // E12 Late - High gamma
        4.0,   // E13 SPLADE - Theta
    ];

    /// Initialize for session (called by SessionStart hook)
    pub async fn initialize_session_frequencies(&self) {
        let mut phases = self.phases.write();
        for i in 0..13 {
            // Start with random phases
            phases[i] = rand::random::<f32>() * 2.0 * std::f32::consts::PI;
        }
    }

    /// Warm up oscillators
    pub async fn warm_up(&self) {
        // Run a few update cycles to stabilize
        for _ in 0..100 {
            self.update(0.01).await;
        }
    }

    /// Update oscillator phases (called every 10ms by background task)
    pub async fn update(&self, dt: f32) {
        let mut phases = self.phases.write();
        let n = 13.0;

        for i in 0..13 {
            // Kuramoto equation: dtheta_i/dt = omega_i + (K/N) sum_j sin(theta_j - theta_i)
            let coupling_sum: f32 = (0..13)
                .map(|j| (phases[j] - phases[i]).sin())
                .sum();

            let d_theta = Self::FREQUENCIES[i] + (self.coupling_strength / n) * coupling_sum;
            phases[i] += d_theta * dt;

            // Normalize to [0, 2pi]
            phases[i] = phases[i] % (2.0 * std::f32::consts::PI);
        }
    }

    /// Compute order parameter r in [0, 1]
    /// r > 0.8 indicates coherent "conscious" state
    pub fn global_order_parameter(&self) -> f32 {
        let phases = self.phases.read();

        // r*e^(i*psi) = (1/N) sum_j e^(i*theta_j)
        let sum_cos: f32 = phases.iter().map(|theta| theta.cos()).sum();
        let sum_sin: f32 = phases.iter().map(|theta| theta.sin()).sum();

        let r = ((sum_cos / 13.0).powi(2) + (sum_sin / 13.0).powi(2)).sqrt();
        r
    }

    /// Compute order parameter for specific memory
    pub async fn compute_order_parameter(&self, memory_id: Uuid) -> Result<f32, Error> {
        // Get memory-specific phase offsets from its fingerprint
        let memory_phases = self.get_memory_phases(memory_id).await?;

        // Compute correlation with global phases
        let phases = self.phases.read();

        let correlation: f32 = (0..13)
            .map(|i| (phases[i] - memory_phases[i]).cos())
            .sum::<f32>() / 13.0;

        Ok((correlation + 1.0) / 2.0) // Normalize to [0, 1]
    }

    /// Integrate new memory into oscillator network
    pub async fn integrate_memory(&self, fingerprint: &TeleologicalFingerprint) -> Result<(), Error> {
        // Extract phases from fingerprint purpose vector
        let memory_phases = self.extract_phases(&fingerprint.purpose_vector);

        // Couple to existing oscillators
        let global_r = self.global_order_parameter();

        if global_r >= 0.8 {
            // High coherence - strong coupling
            self.couple_strongly(&memory_phases).await;
        } else {
            // Low coherence - weak coupling to allow diversity
            self.couple_weakly(&memory_phases).await;
        }

        Ok(())
    }
}
```

### 6.3 SELF_EGO_NODE with Hook Integration

```rust
/// The system's self-representation
/// Updated by SessionEnd hook with session learnings
pub struct SelfEgoNode {
    pub id: Uuid, // Fixed: "SELF_EGO_NODE"
    pub purpose_vector: PurposeVector,
    pub identity_trajectory: Vec<PurposeSnapshot>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

impl SelfEgoNode {
    /// Compute identity continuity metric
    /// IC = cosine(PV_t, PV_{t-1}) * r(t)
    pub fn identity_continuity(&self, current_r: f32) -> f32 {
        if self.identity_trajectory.len() < 2 {
            return 1.0; // New system has perfect continuity
        }

        let current = &self.purpose_vector;
        let previous = &self.identity_trajectory[self.identity_trajectory.len() - 2].purpose_vector;

        let cosine = current.cosine_similarity(previous);
        cosine * current_r
    }

    /// Update identity with session learnings
    /// Called by SessionEnd hook
    pub async fn incorporate_session_learnings(&mut self, session_id: &str) -> Result<(), Error> {
        // Record current state to trajectory
        self.identity_trajectory.push(PurposeSnapshot {
            purpose_vector: self.purpose_vector.clone(),
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
        });

        // Trim trajectory to last 100 snapshots
        if self.identity_trajectory.len() > 100 {
            self.identity_trajectory.drain(0..self.identity_trajectory.len() - 100);
        }

        self.last_updated = Utc::now();
        Ok(())
    }

    /// Check if action aligns with self
    /// Used by PreToolUse hook for alignment checking
    pub fn alignment_with_action(&self, action_embedding: &SemanticFingerprint) -> f32 {
        // Compare action embedding with purpose vector
        // Low alignment triggers self-reflection
        self.purpose_vector.alignment_with_embedding(action_embedding)
    }

    /// Trigger introspective consolidation
    /// Called when identity continuity drops below threshold
    pub async fn trigger_introspection(&self) -> Result<(), Error> {
        // Emit notification for Notification hook
        tracing::warn!(
            continuity = self.identity_continuity(0.8),
            "Identity drift detected - triggering introspection"
        );

        // Will be handled by Notification hook with type "identity_continuity_low"
        Ok(())
    }
}
```

---

## 7. Automatic Teleological Array Creation Pipeline

### 7.1 MCP-Triggered Embedding Flow

When content is stored via any MCP tool, the teleological embedding pipeline executes automatically:

```
MCP INJECTION FLOW
==================

store_memory(content, rationale)
    |
    v
+----------------------------------+
| INPUT VALIDATION                  |
| - Content length check            |
| - PII scrubbing                   |
| - Adversarial pattern detection   |
+----------------------------------+
    |
    v
+----------------------------------+
| TELEOLOGICAL EMBEDDING PIPELINE   |
| (Automatic - No Manual Steps)     |
+----------------------------------+
    |
    +---> E1:  Semantic (1024D Matryoshka)
    +---> E2:  Temporal-Recent (512D exp decay)
    +---> E3:  Temporal-Periodic (512D Fourier)
    +---> E4:  Temporal-Positional (512D sin PE)
    +---> E5:  Causal (768D SCM asymmetric)
    +---> E6:  Sparse (30K active)
    +---> E7:  Code (1536D AST)
    +---> E8:  Graph/GNN (384D MiniLM)
    +---> E9:  HDC (10K-bit holographic)
    +---> E10: Multimodal (768D)
    +---> E11: Entity/TransE (384D)
    +---> E12: Late-Interaction (128D/tok)
    +---> E13: SPLADE (30K sparse)
    |
    v
+----------------------------------+
| PURPOSE VECTOR COMPUTATION        |
| PV[i] = cosine(E[i], NorthStar)   |
| 13D teleological signature        |
+----------------------------------+
    |
    v
+----------------------------------+
| JOHARI CLASSIFICATION             |
| Per-embedder Delta_S/Delta_C      |
| Quadrant assignment               |
+----------------------------------+
    |
    v
+----------------------------------+
| KURAMOTO INTEGRATION              |
| Phase coupling to oscillators     |
| Coherence check (r >= 0.8?)       |
+----------------------------------+
    |
    v
+----------------------------------+
| STORAGE                           |
| RocksDB primary + indexes         |
| Purpose evolution tracking        |
+----------------------------------+
    |
    v
+----------------------------------+
| STEERING FEEDBACK                 |
| Compute storage reward            |
| Update dopamine modulation        |
+----------------------------------+
```

### 7.2 Embedding Pipeline Implementation

```rust
/// Autonomous teleological embedding pipeline
/// Executes automatically on every memory storage
pub struct TeleologicalEmbeddingPipeline {
    embedders: [Box<dyn Embedder>; 13],
    purpose_computer: PurposeVectorComputer,
    johari_classifier: JohariClassifier,
    kuramoto: Arc<KuramotoOscillatorLayer>,
    goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
}

impl TeleologicalEmbeddingPipeline {
    /// Generate full teleological fingerprint from content
    /// This is called AUTOMATICALLY by store operations
    pub async fn embed_content(&self, content: &str) -> Result<TeleologicalFingerprint, Error> {
        // Phase 1: Generate all 13 embeddings in parallel
        let embeddings = self.generate_all_embeddings(content).await?;

        // Phase 2: Compute purpose vector against current North Star
        let purpose_vector = self.compute_purpose_vector(&embeddings).await?;

        // Phase 3: Compute per-embedder Delta_S/Delta_C and Johari classification
        let johari = self.classify_johari(&embeddings).await?;

        // Phase 4: Compute content hash for deduplication
        let content_hash = self.compute_content_hash(content);

        // Phase 5: Compute aggregate alignment
        let theta = purpose_vector.aggregate_alignment();

        // Phase 6: Create fingerprint
        let fingerprint = TeleologicalFingerprint {
            id: Uuid::new_v4(),
            semantic_fingerprint: SemanticFingerprint::from_embeddings(embeddings),
            purpose_vector,
            johari_fingerprint: johari,
            content_hash,
            theta_to_north_star: theta,
            created_at: Utc::now(),
            source_content: Some(content.to_string()),
            access_count: 0,
            last_accessed: Utc::now(),
        };

        // Phase 7: Integrate with Kuramoto oscillators
        self.kuramoto.integrate_memory(&fingerprint).await?;

        Ok(fingerprint)
    }

    async fn generate_all_embeddings(&self, content: &str) -> Result<[EmbedderOutput; 13], Error> {
        // Run all 13 embedders in parallel using tokio::join!
        let results = tokio::join!(
            self.embedders[0].embed(content),  // E1 Semantic
            self.embedders[1].embed(content),  // E2 Temporal-Recent
            self.embedders[2].embed(content),  // E3 Temporal-Periodic
            self.embedders[3].embed(content),  // E4 Temporal-Positional
            self.embedders[4].embed(content),  // E5 Causal
            self.embedders[5].embed(content),  // E6 Sparse
            self.embedders[6].embed(content),  // E7 Code
            self.embedders[7].embed(content),  // E8 Graph
            self.embedders[8].embed(content),  // E9 HDC
            self.embedders[9].embed(content),  // E10 Multimodal
            self.embedders[10].embed(content), // E11 Entity
            self.embedders[11].embed(content), // E12 Late-Interaction
            self.embedders[12].embed(content), // E13 SPLADE
        );

        // Collect results (fail fast on any error)
        Ok([
            results.0?, results.1?, results.2?, results.3?,
            results.4?, results.5?, results.6?, results.7?,
            results.8?, results.9?, results.10?, results.11?,
            results.12?,
        ])
    }
}
```

---

## 8. MCP Tool Updates

The autonomous tools are updated to work seamlessly with teleological arrays:

### 8.1 Updated MCP Tools

| Tool | Autonomous Behavior |
|------|---------------------|
| `store_memory` | Triggers full teleological embedding pipeline automatically |
| `inject_context` | Returns teleological fingerprint metadata with results |
| `auto_bootstrap_north_star` | Discovers purpose from stored fingerprints - no manual input |
| `get_alignment_drift` | Uses teleological array comparison (apples to apples) |
| `trigger_drift_correction` | Applies corrections at per-embedder level |
| `discover_sub_goals` | Clusters using full 13-embedder arrays |
| `get_consciousness_state` | Returns Kuramoto r, meta-score, identity continuity |
| `get_workspace_status` | Shows active memory and competing candidates |
| `trigger_dream` | Runs NREM/REM consolidation cycle |

### 8.2 New MCP Tools for Autonomous Operation

```json
{
  "tools": [
    {
      "name": "get_learning_trajectory",
      "description": "Get the learning trajectory for current session",
      "parameters": {
        "session_id": "string (optional)",
        "include_rewards": "boolean (default: true)"
      },
      "returns": {
        "trajectory_id": "uuid",
        "steps": "array of trajectory steps",
        "cumulative_reward": "float",
        "purpose_drift": "float"
      }
    },
    {
      "name": "get_case_suggestions",
      "description": "Get case-based suggestions for current context",
      "parameters": {
        "context": "string (current situation)",
        "max_cases": "integer (default: 5)"
      },
      "returns": {
        "cases": "array of similar past cases with outcomes",
        "recommended_actions": "array of suggested actions"
      }
    },
    {
      "name": "force_distillation",
      "description": "Force memory distillation to parametric memory",
      "parameters": {
        "importance_threshold": "float (default: 0.8)"
      },
      "returns": {
        "memories_distilled": "integer",
        "compression_ratio": "float"
      }
    },
    {
      "name": "get_identity_status",
      "description": "Get SELF_EGO_NODE identity status",
      "parameters": {},
      "returns": {
        "identity_continuity": "float",
        "purpose_vector": "array[13]",
        "trajectory_length": "integer",
        "drift_warning": "boolean"
      }
    },
    {
      "name": "get_hook_metrics",
      "description": "Get metrics from hook executions",
      "parameters": {
        "hook_type": "string (optional, filter by type)",
        "time_range": "string (optional, e.g., '24h')"
      },
      "returns": {
        "executions": "integer",
        "avg_latency_ms": "float",
        "success_rate": "float",
        "by_hook_type": "object"
      }
    }
  ]
}
```

---

## 9. Memory Consolidation Patterns

### 9.1 Dream-Based Consolidation

```rust
/// Dream engine for autonomous memory consolidation
/// Runs during idle periods or session end
pub struct DreamEngine {
    store: Arc<dyn TeleologicalMemoryStore>,
    clusterer: Arc<dyn TeleologicalClusterer>,
    edge_learner: Arc<AmortizedShortcutLearner>,
    blind_spot_detector: Arc<BlindSpotDetector>,
}

impl DreamEngine {
    /// Run full dream cycle (NREM + REM)
    /// Triggered by: SessionEnd hook, entropy > 0.7, idle > 10min
    pub async fn run_full_cycle(&self) -> Result<DreamResult, Error> {
        // NREM Phase: Replay and strengthen (3 min)
        let nrem_result = self.run_nrem_phase().await?;

        // REM Phase: Creative synthesis (2 min)
        let rem_result = self.run_rem_phase().await?;

        Ok(DreamResult {
            nrem: nrem_result,
            rem: rem_result,
            total_duration: Duration::from_secs(300),
        })
    }

    async fn run_nrem_phase(&self) -> Result<NREMResult, Error> {
        // Replay recent memories with Hebbian strengthening
        let recent = self.store.list_recent(100).await?;

        let mut edges_strengthened = 0;
        for memory in &recent {
            // Find related memories
            let related = self.store.search_semantic(
                &memory.semantic_fingerprint,
                TeleologicalSearchOptions::quick(10),
            ).await?;

            // Strengthen edges using Hebbian rule: Delta_w = eta * pre * post
            for rel in &related {
                if rel.similarity > 0.7 {
                    self.strengthen_edge(memory.id, rel.fingerprint.id, rel.similarity).await?;
                    edges_strengthened += 1;
                }
            }
        }

        // Create amortized shortcuts for frequent paths
        let shortcuts = self.edge_learner.find_shortcut_candidates().await?;

        Ok(NREMResult {
            memories_replayed: recent.len(),
            edges_strengthened,
            shortcuts_created: shortcuts.len(),
        })
    }

    async fn run_rem_phase(&self) -> Result<REMResult, Error> {
        // Generate synthetic queries via hyperbolic random walk
        let synthetic_queries = self.generate_synthetic_queries(50).await?;

        // Find blind spots (high semantic distance + shared causal patterns)
        let blind_spots = self.blind_spot_detector
            .detect(&synthetic_queries)
            .await?;

        // Create exploratory edges with low initial weight
        for spot in &blind_spots {
            self.create_exploratory_edge(&spot.memory_a, &spot.memory_b).await?;
        }

        Ok(REMResult {
            synthetic_queries: synthetic_queries.len(),
            blind_spots_found: blind_spots.len(),
            exploratory_edges_created: blind_spots.len(),
        })
    }
}
```

---

## 10. Directory Structure

The complete Claude Code extensibility structure:

```
.claude/
    settings.json                 # Hook configuration (Section 2.1.1)

    skills/
        memory-inject/
            SKILL.md              # Memory injection skill (Section 2.2.1)
        context-search/
            SKILL.md              # Context search skill (Section 2.2.2)
        goal-discover/
            SKILL.md              # Goal discovery skill (Section 2.2.3)
        memory-consolidate/
            SKILL.md              # Consolidation skill (Section 2.2.4)

    agents/
        embedding-specialist.md   # Embedding agent (Section 2.3.1)
        search-agent.md           # Search agent (Section 2.3.2)
        comparison-agent.md       # Comparison agent (Section 2.3.3)
        goal-agent.md             # Goal agent (Section 2.3.4)
```

---

## 11. Success Criteria

- [ ] **Zero Manual Intervention**: All embedding and purpose computation is automatic
- [ ] **All 10 Hook Types**: Integrated (PreToolUse, PostToolUse, SessionStart, SessionEnd, UserPromptSubmit, PreCompact, Stop, SubagentStop, Notification, PermissionRequest)
- [ ] **4 Skills Defined**: memory-inject, context-search, goal-discover, memory-consolidate
- [ ] **4 Subagents Defined**: embedding-specialist, search-agent, comparison-agent, goal-agent
- [ ] **Teleological Array Clustering**: Goal discovery uses full 13-embedder comparison
- [ ] **All Goals Have Arrays**: Every goal has full 13-embedder teleological array
- [ ] **Kuramoto Consciousness**: r parameter computed and consciousness states tracked
- [ ] **Identity Continuity**: SELF_EGO_NODE maintains purpose trajectory
- [ ] **Dream Consolidation**: NREM/REM cycles run during idle/session-end
- [ ] **Steering Feedback**: Every storage operation produces steering reward
- [ ] **Case-Based Learning**: Past successful operations stored and retrieved as cases
- [ ] **Memory Distillation**: High-importance memories compressed to parametric recall
- [ ] **Apples-to-Apples Comparison**: All comparisons use same-type data throughout
- [ ] **Full Pipeline Flow**: Hook -> Skill -> MCP Tool -> Teleological Array documented

---

## 12. References

**Research Foundations**:
- [MemVerse: Multimodal Memory for Lifelong Learning Agents](https://arxiv.org/abs/2512.03627) - Memory distillation and dual-path architecture
- [Memento: Fine-tuning LLM Agents without Fine-tuning LLMs](https://arxiv.org/abs/2508.16153) - Case-based reasoning for continuous learning
- [ALAS: Autonomous Learning Agent for Self-Updating Language Models](https://arxiv.org/abs/2508.15805) - Autonomous curriculum generation
- [Memory in the Age of AI Agents: A Survey](https://arxiv.org/abs/2512.13564) - Comprehensive memory taxonomy
- [Global Workspace Theory](https://en.wikipedia.org/wiki/Global_workspace_theory) - Consciousness architecture
- [Cognitive Workspace: Active Memory Management for LLMs](https://arxiv.org/abs/2508.13171) - Consciousness scaffolds

**Internal References**:
- [01-ARCHITECTURE.md](./01-ARCHITECTURE.md) - Teleological array structure
- [02-STORAGE.md](./02-STORAGE.md) - Storage layer design
- [03-SEARCH.md](./03-SEARCH.md) - 5-stage retrieval pipeline
- [04-COMPARISON.md](./04-COMPARISON.md) - Array comparison methods
- [05-NORTH-STAR-REMOVAL.md](./05-NORTH-STAR-REMOVAL.md) - Manual North Star deprecation
