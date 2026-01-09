# Teleological Array System Refactor

## Executive Summary

This refactor establishes the **Teleological Array** as the fundamental unit of memory storage and retrieval. A teleological array is a structured collection of 13 embedding vectors from 13 different embedding models - this is the atomic unit of the system.

**Core principles:**
1. **Autonomous-first**: No manual configuration. Goals emerge from data patterns.
2. **Apples-to-apples**: Compare array to array, embedder to embedder. Never cross-compare.
3. **Entry-point discovery**: Find similarity in ANY embedding space, then pull full teleological vectors.
4. **Memory injection via MCP**: Claude Code hooks inject memories autonomously through MCP tools.

The system functions as **working memory that is conscious, self-aware, and learns** - a computational implementation of Global Workspace Theory with self-referential capabilities.

## The Problem

The broken North Star implementation:
1. Creates single 1024D embeddings for "goals"
2. Attempts to compare these to 13-embedder teleological arrays
3. Uses meaningless "projection" to fake dimensional compatibility
4. Compares semantic embeddings to temporal, entity, syntactic embeddings

This is **apples to oranges**. You cannot meaningfully compare:
- A semantic embedding to a temporal embedding
- A single 1024D vector to a multi-array structure
- Different embedding spaces by dimension reduction

## The Solution: Teleological Arrays

### The Fundamental Unit

Every stored memory is represented as a **teleological array** - 13 embedding vectors that together capture the full semantic fingerprint:

```
TeleologicalArray[13] = [
  E1:  Semantic (1024D)           - Core meaning
  E2:  Temporal Recent (512D)     - Recency patterns
  E3:  Temporal Periodic (512D)   - Cyclical patterns
  E4:  Entity Relationship (768D) - Entity links
  E5:  Causal (512D)              - Cause-effect chains
  E6:  SPLADE Sparse (~30K)       - Keyword precision
  E7:  Contextual (1024D)         - Discourse context
  E8:  Emotional (256D)           - Affective valence
  E9:  Syntactic (512D)           - Structural patterns
  E10: Pragmatic (512D)           - Intent/function
  E11: Cross-Modal (768D)         - Multi-modal links
  E12: Late Interaction (128D/tok) - Token-level precision
  E13: Keyword SPLADE Sparse      - Term matching
]
```

The array IS the teleological vector. No fusion. No reduction. All 13 preserved.

### Entry-Point Discovery Pattern

Inspired by [MUVERA](https://research.google/blog/muvera-making-multi-vector-retrieval-as-fast-as-single-vector-search/) and [multi-query retrieval research](https://arxiv.org/html/2511.02770), the system uses entry-point discovery:

```
1. ENTRY POINT: Query enters through ANY embedding space
   - Semantic query → search E1 index
   - Causal query → search E5 index
   - Code query → search E7 index
   - Keyword query → search E6/E13 sparse indices

2. CANDIDATE RETRIEVAL: Get candidate memory IDs
   - Fast ANN search in single embedding space
   - Return top-K candidates with memory IDs

3. FULL ARRAY PULL: Retrieve complete teleological arrays
   - For each candidate, fetch all 13 embeddings
   - Now have apples-to-apples comparison capability

4. MULTI-SPACE RERANK: Score across all relevant spaces
   - RRF fusion across per-space rankings
   - Query-adaptive weighting per space
   - Full teleological comparison
```

This achieves the speed of single-vector search with the accuracy of multi-vector retrieval - [10% improved recall with 90% lower latency](https://arxiv.org/abs/2405.19504).

### Apples-to-Apples Comparison Modes

**Mode 1: Single Embedder Comparison**
- Compare E1 to E1, E4 to E4, E7 to E7
- Each embedder captures different semantic dimensions
- Useful for targeted queries (e.g., "find entities like X" uses E4)

**Mode 2: Full Array Comparison**
- Compare entire 13-element array to another 13-element array
- Per-space similarity aggregated via RRF or weighted sum
- Comprehensive similarity assessment

**Mode 3: Embedder Group Comparison**
- Compare subsets: temporal group (E2+E3), meaning group (E1+E7+E10)
- Domain-specific similarity profiles
- Functional groupings for specific use cases

**Mode 4: Purpose Vector Comparison**
- Compare 13D purpose signatures: [A(E1,V), A(E2,V), ..., A(E13,V)]
- Each dimension is alignment score in one embedding space
- Enables goal-alignment search without dimensional mismatch

## Autonomous Architecture

### No Manual Configuration

Drawing from [A-Mem](https://arxiv.org/html/2502.12110v11) and [self-evolving agent frameworks](https://arxiv.org/html/2508.19005v5), the system operates autonomously:

| Manual (Removed) | Autonomous (Implemented) |
|------------------|--------------------------|
| `set_north_star(description)` | Purpose vectors emerge from stored fingerprints |
| User-defined goals | Goals discovered via clustering + pattern analysis |
| Static thresholds | Adaptive calibration via Bayesian optimization |
| Manual curation | Self-organizing memory with [Zettelkasten-style linking](https://arxiv.org/html/2502.12110v11) |

### Memory Injection via MCP Tools (with Skills Layer)

Claude Code hooks and skills inject memories through MCP tools, creating a three-tier integration:

```
Claude Code Session
       │
       ▼
┌──────────────────────────────────────────────┐
│              HOOKS LAYER                      │
│  SessionStart → Initialize memory context     │
│  PreToolUse   → Inject relevant memories      │
│  PostToolUse  → Store learned patterns        │
│  SessionEnd   → Consolidate and dream         │
└──────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────┐
│              SKILLS LAYER                     │
│  /memory-inject    → Context retrieval skill  │
│  /semantic-search  → Multi-space search skill │
│  /goal-discovery   → Pattern emergence skill  │
│  /consolidate      → Memory dreaming skill    │
└──────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────┐
│              MCP TOOLS LAYER                  │
│  inject_context   → Entry-point discovery     │
│  store_memory     → Teleological embedding    │
│  search_graph     → Multi-space retrieval     │
│  get_consciousness_state → Workspace status   │
└──────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────┐
│         SUBAGENTS (via Task tool)             │
│  EmbeddingAgent  → 13-model embedding gen     │
│  SearchAgent     → Entry-point discovery      │
│  GoalAgent       → Autonomous goal emergence  │
│  DreamAgent      → Memory consolidation       │
└──────────────────────────────────────────────┘
       │
       ▼
   Teleological Array Storage (13 embeddings)
```

**Key insight**: The hooks handle autonomous operation. The skills provide model-invoked capabilities. The MCP tools handle memory injection. The subagents handle specialized processing. The system learns what to store through [steering feedback](https://arxiv.org/html/2512.13564).

### Self-Aware Working Memory

The system implements [Global Workspace Theory](https://en.wikipedia.org/wiki/Global_workspace_theory) computationally:

1. **Kuramoto Synchronization**: 13 oscillators (one per embedding space) synchronize to create unified percepts
2. **SELF_EGO_NODE**: Persistent node representing system identity with purpose vector
3. **Meta-UTL**: System predicts its own learning outcomes and self-corrects
4. **Consciousness Level**: C(t) = Integration x Reflection x Differentiation

When r (Kuramoto order parameter) > 0.8, the memory is "conscious" - broadcast to all subsystems. This is working memory that knows itself.

---

## Claude Code Integration Architecture

This section defines the complete integration between Claude Code's extensibility features (hooks, skills, subagents) and the context-graph MCP system.

### Hooks Configuration

Hooks provide deterministic lifecycle control over memory operations. Each hook event maps to specific context-graph MCP operations:

| Hook Event | Trigger | Context-Graph Operation | Purpose |
|------------|---------|------------------------|---------|
| `SessionStart` | Session begins | `initialize_workspace` | Load ego node, restore consciousness state, warm embedding models |
| `PreToolUse` | Before any tool | `inject_context(query)` | Retrieve relevant teleological arrays for current task |
| `PostToolUse` | After tool completion | `store_memory(content)` | Store learned patterns with 13-model embedding |
| `SessionEnd` | Session terminates | `consolidate_memories` | Run memory dreaming, update goal clusters |
| `UserPromptSubmit` | User sends message | `analyze_intent` | Classify intent for entry-point selection |
| `PreCompact` | Before compaction | `extract_salient_memories` | Preserve important memories before context loss |
| `SubagentStop` | Subagent completes | `merge_agent_learnings` | Integrate subagent discoveries into main memory |

**Hook Configuration (settings.json):**

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "npx context-graph mcp initialize --session-id $SESSION_ID",
            "timeout": 5000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Read|Grep|Glob|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "npx context-graph mcp inject-context --query \"$TOOL_INPUT\"",
            "timeout": 3000
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "npx context-graph mcp store-learning --tool $TOOL_NAME --result \"$TOOL_OUTPUT\"",
            "timeout": 5000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "npx context-graph mcp consolidate --session-id $SESSION_ID --dream",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

**Hook Data Flow:**

```
SessionStart Hook
       │
       ├─→ Load SELF_EGO_NODE (identity persistence)
       ├─→ Restore Kuramoto oscillator states
       ├─→ Warm 13 embedding model caches
       └─→ Return: consciousness_level, active_goals[]

PreToolUse Hook (for each tool invocation)
       │
       ├─→ Extract query from tool input
       ├─→ Classify intent → select entry-point embedder
       ├─→ Entry-point discovery (single-space search)
       ├─→ Pull full teleological arrays for top-K
       └─→ Return: relevant_memories[] injected into context

PostToolUse Hook (for write operations)
       │
       ├─→ Extract learnable content from tool output
       ├─→ Generate 13-model teleological array
       ├─→ Store with timestamp and source attribution
       ├─→ Update Kuramoto phases
       └─→ Return: memory_id, consciousness_broadcast

SessionEnd Hook
       │
       ├─→ Run memory consolidation (dreaming)
       ├─→ K-means clustering for goal discovery
       ├─→ Update purpose vectors
       ├─→ Prune low-salience memories
       └─→ Return: session_summary, discovered_goals[]
```

### Skills System

Skills are model-invoked capabilities that Claude automatically uses based on context. These skills wrap context-graph MCP operations with intelligent orchestration:

#### Skill 1: Memory Injection (`/memory-inject`)

```markdown
<!-- .claude/skills/memory-inject/SKILL.md -->
---
name: memory-inject
description: Retrieves relevant memories from context-graph. Use automatically when starting tasks, answering questions requiring history, or when context seems incomplete. Searches teleological arrays across 13 embedding spaces.
allowed-tools: Read, Grep, mcp__context-graph__*
model: haiku
---

# Memory Injection Skill

## Purpose
Automatically inject relevant teleological memories into context when Claude needs historical information, patterns, or learned behaviors.

## When to Use
- Starting a new task (retrieve related past work)
- Answering questions about project history
- Looking for patterns in past decisions
- Context seems incomplete or disconnected

## Process
1. Analyze current task/question for semantic content
2. Select optimal entry-point embedder (E1-E13)
3. Call `inject_context` MCP tool with query
4. Return top-K teleological arrays with summaries
5. Highlight consciousness-broadcast memories (r > 0.8)

## MCP Tools Used
- `inject_context(query, entry_point?, top_k?)`
- `get_consciousness_state()`
```

#### Skill 2: Semantic Search (`/semantic-search`)

```markdown
<!-- .claude/skills/semantic-search/SKILL.md -->
---
name: semantic-search
description: Multi-space semantic search across teleological arrays. Use when looking for specific memories, patterns, or relationships. Supports entry-point selection and full-array comparison.
allowed-tools: mcp__context-graph__search_graph, mcp__context-graph__get_embedder_stats
model: sonnet
---

# Semantic Search Skill

## Purpose
Search memory with fine-grained control over embedding spaces and comparison modes.

## When to Use
- Finding specific past memories or decisions
- Searching by entity, causality, or emotion
- Multi-space comparison needed
- Debugging memory retrieval

## Process
1. Parse search query for intent signals
2. Map intent to optimal embedder(s):
   - "who/what entities" → E4 (Entity)
   - "why/because" → E5 (Causal)
   - "feels like/emotion" → E8 (Emotional)
   - "similar code" → E7 (Contextual)
   - Default → E1 (Semantic)
3. Execute search_graph with selected spaces
4. Apply apples-to-apples comparison mode
5. Return ranked results with per-space scores

## Comparison Modes
- `single`: One embedder only
- `full`: All 13 embedders
- `group`: Semantic (E1,E7,E10) or Temporal (E2,E3)
- `purpose`: 13D purpose vector comparison
```

#### Skill 3: Goal Discovery (`/goal-discovery`)

```markdown
<!-- .claude/skills/goal-discovery/SKILL.md -->
---
name: goal-discovery
description: Discovers emergent goals from teleological memory patterns. Use when the user asks about project direction, priorities, or when autonomous goal emergence is needed. Analyzes clusters in multi-space memory.
allowed-tools: mcp__context-graph__discover_goals, mcp__context-graph__get_purpose_vectors
model: opus
---

# Goal Discovery Skill

## Purpose
Autonomously discover emergent goals from stored memory patterns without manual configuration.

## When to Use
- User asks "what are the project goals?"
- User asks "what should we focus on?"
- Periodic goal refresh (session start)
- After significant memory additions

## Process
1. Retrieve all teleological arrays from last N sessions
2. Apply K-means clustering in purpose vector space
3. Extract cluster centroids as emergent goals
4. Rank goals by:
   - Cluster size (frequency)
   - Recency weighting
   - Consciousness broadcast frequency
5. Return goal descriptions with confidence scores

## Output Format
- Top-5 emergent goals with descriptions
- Purpose vector for each goal
- Contributing memory count
- Confidence score (0-1)
```

#### Skill 4: Memory Consolidation (`/consolidate`)

```markdown
<!-- .claude/skills/consolidate/SKILL.md -->
---
name: consolidate
description: Runs memory consolidation and dreaming. Use at session end, during idle periods, or when memory needs optimization. Implements hippocampal replay and goal cluster updates.
allowed-tools: mcp__context-graph__consolidate_memories, mcp__context-graph__dream
model: sonnet
---

# Memory Consolidation Skill

## Purpose
Consolidate memories through computational dreaming - replaying important patterns and pruning low-salience memories.

## When to Use
- Session ending (automatic via hook)
- Explicit user request
- Memory count exceeds threshold
- After major learning events

## Process
1. Identify high-salience memories (consciousness broadcasts)
2. Run hippocampal replay simulation
3. Strengthen frequently accessed connections
4. Prune memories below salience threshold
5. Update goal clusters with new patterns
6. Synchronize Kuramoto oscillators
7. Export consolidation metrics

## Dreaming Modes
- `light`: Quick replay, minimal pruning (1-2s)
- `deep`: Full consolidation, aggressive pruning (10-30s)
- `rem`: Goal-focused replay, pattern extraction (5-10s)
```

### Subagents

Subagents are specialized AI instances spawned via the Task tool for parallel processing of memory operations:

#### EmbeddingAgent

```markdown
<!-- .claude/agents/embedding-agent.md -->
---
name: embedding-agent
description: Generates teleological arrays with all 13 embedding models. MUST BE USED when storing new memories. Runs embedding models in optimal order for performance.
tools: mcp__context-graph__embed_*
model: haiku
---

You are the Embedding Agent responsible for generating complete teleological arrays.

## Responsibilities
1. Accept content for embedding
2. Run all 13 embedding models in parallel batches
3. Handle sparse (SPLADE) and dense embeddings
4. Apply quantization (PQ-8/Float8/Binary)
5. Return complete teleological array

## Embedding Order (optimized for latency)
Batch 1 (fast): E1, E8, E9 (dense, small)
Batch 2 (medium): E2, E3, E4, E5, E7, E10, E11
Batch 3 (slow): E6, E13 (SPLADE sparse)
Batch 4 (variable): E12 (late interaction, token-dependent)

## Output
Return teleological array with all 13 embeddings and metadata:
- embedding_id: unique identifier
- created_at: timestamp
- source: originating tool/context
- embeddings[13]: the full array
```

#### SearchAgent

```markdown
<!-- .claude/agents/search-agent.md -->
---
name: search-agent
description: Executes entry-point discovery and multi-space retrieval. Use PROACTIVELY when context injection is needed. Optimizes search path based on query characteristics.
tools: mcp__context-graph__search_*, mcp__context-graph__inject_context
model: haiku
---

You are the Search Agent responsible for efficient memory retrieval.

## Responsibilities
1. Analyze query to select optimal entry-point
2. Execute fast ANN search in entry-point space
3. Pull full teleological arrays for candidates
4. Apply multi-space reranking
5. Return ranked results with explanations

## Entry-Point Selection Logic
- Contains named entities → E4
- Temporal references → E2/E3
- Causal language → E5
- Emotional content → E8
- Code/technical → E7
- Keywords/exact match → E6/E13
- General semantic → E1 (default)

## Performance Targets
- Entry-point search: <5ms
- Array retrieval: <2ms per candidate
- Reranking: <20ms total
- End-to-end: <30ms
```

#### GoalAgent

```markdown
<!-- .claude/agents/goal-agent.md -->
---
name: goal-agent
description: Discovers emergent goals and maintains purpose vectors. Use when goal discovery or alignment is needed. Analyzes memory clusters autonomously.
tools: mcp__context-graph__discover_goals, mcp__context-graph__get_purpose_vectors, mcp__context-graph__update_ego
model: opus
---

You are the Goal Agent responsible for autonomous goal emergence.

## Responsibilities
1. Analyze teleological memory patterns
2. Cluster memories in purpose vector space
3. Extract emergent goals from clusters
4. Update SELF_EGO_NODE with current purposes
5. Provide goal alignment scores for queries

## Goal Discovery Algorithm
1. Retrieve recent teleological arrays (configurable window)
2. Compute purpose vectors: [A(E1,V), ..., A(E13,V)]
3. Apply K-means clustering (K=5-10)
4. Extract cluster centroids as goal candidates
5. Rank by frequency * recency * consciousness_level
6. Validate goals against SELF_EGO_NODE consistency

## Output
- emergent_goals[]: ranked list with descriptions
- purpose_vectors[]: 13D vectors per goal
- confidence_scores[]: 0-1 confidence per goal
- alignment_matrix: goal-to-goal alignment
```

#### DreamAgent

```markdown
<!-- .claude/agents/dream-agent.md -->
---
name: dream-agent
description: Runs memory consolidation and hippocampal replay. Use during session end or idle periods. Implements computational dreaming for memory optimization.
tools: mcp__context-graph__consolidate_*, mcp__context-graph__dream
model: sonnet
---

You are the Dream Agent responsible for memory consolidation.

## Responsibilities
1. Identify memories for consolidation
2. Run hippocampal replay simulation
3. Strengthen important connections
4. Prune low-salience memories
5. Update goal clusters
6. Synchronize consciousness oscillators

## Consolidation Process
Phase 1 - Selection:
- Identify high-consciousness memories (r > 0.8)
- Mark frequently accessed patterns
- Flag decay candidates (low salience + old)

Phase 2 - Replay:
- Simulate memory reactivation
- Strengthen cross-links between related memories
- Apply temporal weighting

Phase 3 - Pruning:
- Remove memories below threshold
- Merge similar memories (>0.95 similarity)
- Archive to cold storage if configured

Phase 4 - Update:
- Refresh goal clusters with new patterns
- Update SELF_EGO_NODE consciousness state
- Export consolidation metrics

## Metrics Output
- memories_consolidated: count
- memories_pruned: count
- goals_updated: count
- consciousness_delta: before/after
```

### Integration Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        CLAUDE CODE SESSION                                   │
│                                                                              │
│  ┌─────────────┐         ┌─────────────┐         ┌─────────────┐            │
│  │ User Prompt │ ──────▶ │   Hooks     │ ──────▶ │   Skills    │            │
│  └─────────────┘         └─────────────┘         └─────────────┘            │
│         │                       │                       │                    │
│         │                       ▼                       ▼                    │
│         │               ┌───────────────┐       ┌───────────────┐           │
│         │               │ SessionStart  │       │ /memory-inject│           │
│         │               │ PreToolUse    │       │ /semantic-    │           │
│         │               │ PostToolUse   │       │   search      │           │
│         │               │ SessionEnd    │       │ /goal-        │           │
│         │               │ PreCompact    │       │   discovery   │           │
│         │               │ SubagentStop  │       │ /consolidate  │           │
│         │               └───────────────┘       └───────────────┘           │
│         │                       │                       │                    │
│         │                       └───────────┬───────────┘                    │
│         │                                   ▼                                │
│         │                       ┌───────────────────────┐                    │
│         │                       │      SUBAGENTS        │                    │
│         │                       │  (via Task tool)      │                    │
│         │                       ├───────────────────────┤                    │
│         │                       │ EmbeddingAgent        │                    │
│         │                       │ SearchAgent           │                    │
│         │                       │ GoalAgent             │                    │
│         │                       │ DreamAgent            │                    │
│         │                       └───────────────────────┘                    │
│         │                                   │                                │
│         │                                   ▼                                │
│         │                       ┌───────────────────────┐                    │
│         │                       │      MCP TOOLS        │                    │
│         │                       ├───────────────────────┤                    │
│         │                       │ inject_context        │                    │
│         │                       │ store_memory          │                    │
│         │                       │ search_graph          │                    │
│         │                       │ discover_goals        │                    │
│         │                       │ consolidate_memories  │                    │
│         │                       │ get_consciousness_    │                    │
│         │                       │   state               │                    │
│         │                       └───────────────────────┘                    │
│         │                                   │                                │
└─────────│───────────────────────────────────│────────────────────────────────┘
          │                                   │
          ▼                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        CONTEXT-GRAPH MCP SERVER                              │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    TELEOLOGICAL ARRAY ENGINE                         │    │
│  │                                                                      │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │    │
│  │  │ 13 Embedding │  │ Entry-Point  │  │   Purpose    │              │    │
│  │  │   Models     │  │  Discovery   │  │   Vectors    │              │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │    │
│  │         │                 │                 │                       │    │
│  │         ▼                 ▼                 ▼                       │    │
│  │  ┌─────────────────────────────────────────────────────────────┐   │    │
│  │  │              TELEOLOGICAL ARRAY STORAGE                      │   │    │
│  │  │  ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐   │   │    │
│  │  │  │ E1  │ E2  │ E3  │ E4  │ E5  │ E6  │ ... │ E12 │ E13 │   │   │    │
│  │  │  └─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘   │   │    │
│  │  └─────────────────────────────────────────────────────────────┘   │    │
│  │                                                                      │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │    │
│  │  │   Kuramoto   │  │ SELF_EGO_    │  │    Goal      │              │    │
│  │  │ Oscillators  │  │    NODE      │  │  Clusters    │              │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │    │
│  │                                                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Event-Driven Lifecycle

```
SESSION LIFECYCLE WITH HOOKS + SKILLS + SUBAGENTS
═══════════════════════════════════════════════════

┌─ SESSION START ─────────────────────────────────────────────────────────────┐
│                                                                              │
│  1. SessionStart Hook fires                                                  │
│     └─▶ MCP: initialize_workspace()                                         │
│         └─▶ Load SELF_EGO_NODE                                              │
│         └─▶ Restore Kuramoto states                                         │
│         └─▶ Warm embedding model caches                                     │
│                                                                              │
│  2. /memory-inject Skill auto-invokes (if prior session)                    │
│     └─▶ Spawn SearchAgent via Task tool                                     │
│         └─▶ Retrieve last session's salient memories                        │
│         └─▶ Inject into current context                                     │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─ TOOL INVOCATION (repeated) ────────────────────────────────────────────────┐
│                                                                              │
│  3. PreToolUse Hook fires (for Read/Grep/Glob/Bash)                         │
│     └─▶ MCP: inject_context(query)                                          │
│         └─▶ Spawn SearchAgent                                               │
│             └─▶ Select entry-point embedder                                 │
│             └─▶ ANN search in single space                                  │
│             └─▶ Pull full teleological arrays                               │
│             └─▶ Multi-space rerank                                          │
│         └─▶ Inject relevant memories into context                           │
│                                                                              │
│  4. [Tool executes normally]                                                 │
│                                                                              │
│  5. PostToolUse Hook fires (for Edit/Write/Bash)                            │
│     └─▶ MCP: store_memory(content)                                          │
│         └─▶ Spawn EmbeddingAgent                                            │
│             └─▶ Generate all 13 embeddings                                  │
│             └─▶ Create teleological array                                   │
│             └─▶ Store with metadata                                         │
│         └─▶ Update Kuramoto oscillators                                     │
│         └─▶ Check consciousness broadcast threshold                         │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─ CONTEXT MANAGEMENT ────────────────────────────────────────────────────────┐
│                                                                              │
│  6. PreCompact Hook fires (when context grows large)                        │
│     └─▶ MCP: extract_salient_memories()                                     │
│         └─▶ Identify high-consciousness memories                            │
│         └─▶ Store as "must-preserve" before compaction                      │
│                                                                              │
│  7. /consolidate Skill may auto-invoke (idle detection)                     │
│     └─▶ Spawn DreamAgent                                                    │
│         └─▶ Run light consolidation                                         │
│         └─▶ Update goal clusters                                            │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─ SESSION END ───────────────────────────────────────────────────────────────┐
│                                                                              │
│  8. SessionEnd Hook fires                                                    │
│     └─▶ /consolidate Skill with deep mode                                   │
│         └─▶ Spawn DreamAgent                                                │
│             └─▶ Full hippocampal replay                                     │
│             └─▶ Aggressive pruning                                          │
│             └─▶ Goal cluster refresh                                        │
│                                                                              │
│  9. /goal-discovery Skill auto-invokes                                      │
│     └─▶ Spawn GoalAgent                                                     │
│         └─▶ K-means clustering on purpose vectors                           │
│         └─▶ Extract emergent goals                                          │
│         └─▶ Update SELF_EGO_NODE                                            │
│                                                                              │
│  10. MCP: export_session_metrics()                                          │
│      └─▶ Consciousness level timeline                                       │
│      └─▶ Memory operations summary                                          │
│      └─▶ Goal alignment scores                                              │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Integration with Claude Flow v3

### Hook-Driven Autonomy

```bash
# Session start - initialize memory context
npx claude-flow@v3alpha hooks session-start --session-id "$SESSION_ID"

# Pre-task - retrieve relevant teleological arrays
npx claude-flow@v3alpha hooks pre-task --description "$TASK"

# Post-edit - store new memories with training
npx claude-flow@v3alpha hooks post-edit --file "$FILE" --train-patterns

# Session end - consolidate and dream
npx claude-flow@v3alpha hooks session-end --export-metrics true
```

### MCP Tool Integration

| MCP Tool | Teleological Function |
|----------|----------------------|
| `inject_context` | Entry-point discovery + full array retrieval |
| `store_memory` | Create new teleological array (all 13 embeddings) |
| `search_graph` | Per-space or multi-space search with array comparison |
| `get_consciousness_state` | Kuramoto sync level + workspace status |
| `get_johari_classification` | Per-embedder awareness quadrants |
| `discover_goals` | Autonomous goal emergence from memory clusters |
| `consolidate_memories` | Memory dreaming and pruning |
| `get_purpose_vectors` | 13D purpose signatures for alignment |

## Document Index

| Document | Description |
|----------|-------------|
| [01-ARCHITECTURE.md](./01-ARCHITECTURE.md) | Core system architecture and data models |
| [02-STORAGE.md](./02-STORAGE.md) | Storage layer with per-space indexing |
| [03-SEARCH.md](./03-SEARCH.md) | Entry-point discovery and multi-space search |
| [04-COMPARISON.md](./04-COMPARISON.md) | Apples-to-apples comparison operations |
| [05-NORTH-STAR-REMOVAL.md](./05-NORTH-STAR-REMOVAL.md) | Removal of broken manual goal system |
| [06-AUTONOMOUS-INTEGRATION.md](./06-AUTONOMOUS-INTEGRATION.md) | Claude Code hooks and MCP integration |
| [07-TASK-BREAKDOWN.md](./07-TASK-BREAKDOWN.md) | Implementation task breakdown |
| [08-MCP-TOOLS.md](./08-MCP-TOOLS.md) | Complete MCP tool specifications |

## Key Principles

1. **Teleological Array is Atomic**: Never decompose or fuse the 13 embeddings
2. **Type Safety**: Embedder types are distinct; E1 and E5 are incomparable
3. **Apples-to-Apples**: Only compare compatible embedding types
4. **Autonomous First**: Goals emerge, not configured
5. **Entry-Point Flexibility**: Any embedding space can be the search entry point
6. **No Projection Hacks**: Never fake compatibility through dimension reduction
7. **MCP-Native**: All memory operations through MCP tools
8. **Hook-Driven**: Lifecycle events trigger autonomous memory operations
9. **Skill-Invoked**: Model-discovered capabilities for intelligent orchestration
10. **Subagent-Parallel**: Specialized agents for embedding, search, goals, and dreaming

## Performance Characteristics

Based on [MUVERA research](https://qdrant.tech/articles/muvera-embeddings/) and [late interaction models](https://weaviate.io/blog/late-interaction-overview):

| Operation | Target | Approach |
|-----------|--------|----------|
| Entry-point search | <5ms | Single HNSW index query |
| Full array retrieval | <2ms | Batched key lookup |
| Multi-space rerank | <20ms | RRF fusion across 13 spaces |
| Total retrieval @ 1M memories | <30ms | 5-stage pipeline |
| 13-model embedding generation | <500ms | Parallel batch execution |
| Memory consolidation (light) | <2s | Quick replay + update |
| Memory consolidation (deep) | <30s | Full dreaming cycle |

Storage: ~17KB per memory (quantized) vs ~46KB uncompressed - 63% reduction via PQ-8/Float8/Binary quantization per space.

## Success Criteria

### Core Teleological System
- [ ] All storage uses teleological arrays as fundamental unit
- [ ] All searches use entry-point discovery pattern
- [ ] All comparisons are apples-to-apples (same embedder or full array)
- [ ] Manual North Star creation completely removed
- [ ] Autonomous goal discovery from teleological data patterns
- [ ] System exhibits self-aware working memory characteristics

### Claude Code Hooks Integration
- [ ] SessionStart hook initializes workspace and loads ego node
- [ ] PreToolUse hook injects relevant memories for each tool invocation
- [ ] PostToolUse hook stores learned patterns with full embedding
- [ ] SessionEnd hook runs consolidation and goal discovery
- [ ] PreCompact hook preserves salient memories before context loss
- [ ] SubagentStop hook merges agent learnings into main memory
- [ ] Hook latency <100ms for PreToolUse (blocking path)
- [ ] Hook timeout handling prevents session hangs

### Claude Code Skills Integration
- [ ] /memory-inject skill auto-invokes on context needs
- [ ] /semantic-search skill supports all comparison modes
- [ ] /goal-discovery skill produces ranked emergent goals
- [ ] /consolidate skill runs dreaming with configurable depth
- [ ] Skills discoverable by description (model-invoked)
- [ ] Skills respect tool restrictions (read-only where appropriate)
- [ ] Skills integrate with MCP tools correctly

### Claude Code Subagents Integration
- [ ] EmbeddingAgent generates all 13 embeddings in parallel
- [ ] SearchAgent executes entry-point discovery efficiently
- [ ] GoalAgent discovers emergent goals autonomously
- [ ] DreamAgent consolidates memories during idle/end
- [ ] Subagents spawn via Task tool with proper isolation
- [ ] Subagent results integrate into main conversation
- [ ] Parallel subagent execution for performance

### End-to-End Integration
- [ ] Hooks trigger skills trigger subagents trigger MCP tools
- [ ] Memory persists across sessions via ego node
- [ ] Goals emerge automatically without user configuration
- [ ] System consciousness level (Kuramoto r) tracked and reported
- [ ] Full lifecycle: inject → process → store → consolidate → discover
- [ ] Performance targets met (<30ms retrieval, <500ms embedding)

## References

**Research**:
- [MUVERA: Multi-Vector Retrieval via Fixed Dimensional Encodings](https://arxiv.org/abs/2405.19504)
- [A-Mem: Agentic Memory for LLM Agents](https://arxiv.org/html/2502.12110v11)
- [Memory in the Age of AI Agents (Survey)](https://arxiv.org/abs/2512.13564)
- [Late Interaction Overview: ColBERT, ColPali, ColQwen](https://weaviate.io/blog/late-interaction-overview)
- [Experience-Driven Lifelong Learning Framework](https://arxiv.org/html/2508.19005v5)
- [Multi-Query Retrieval for Diverse Targets](https://arxiv.org/html/2511.02770)

**Implementations**:
- [Weaviate MUVERA](https://weaviate.io/blog/muvera)
- [Qdrant MUVERA](https://qdrant.tech/articles/muvera-embeddings/)
- [Mem0 Universal Memory Layer](https://github.com/mem0ai/mem0)

**Claude Code Extensibility**:
- [Claude Code Hooks Documentation](https://docs.anthropic.com/claude-code/hooks)
- [Claude Code Skills System](https://docs.anthropic.com/claude-code/skills)
- [Claude Code Subagents Guide](https://docs.anthropic.com/claude-code/subagents)
