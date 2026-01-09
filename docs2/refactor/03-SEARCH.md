# Search System Refactor Specification

## 1. Overview

The search system enables finding relevant teleological arrays using various comparison strategies. The core innovation is **entry-point discovery**: search ALL 13 embedding spaces simultaneously, find the closest neighbors in each individual space, and if ONE point in ANY space is close to another, those teleological vectors become candidates for full comparison.

### 1.1 Core Principles

1. **Entry-Point Discovery**: Any of the 13 embedding spaces can serve as an entry point
2. **Apples-to-Apples Comparison**: Same embedder types compared to each other (E1 to E1, E5 to E5, etc.)
3. **Union-First Candidate Generation**: If close in ANY space, it's a candidate
4. **Full Array Comparison**: After discovery, compare complete teleological fingerprints
5. **Configurable Fusion**: Search matrices define how to weight and combine results

### 1.2 Search Flow

```
Query TeleologicalArray
         │
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    STAGE 1: ENTRY-POINT DISCOVERY                   │
│                                                                     │
│  Search ALL 13 spaces in PARALLEL:                                 │
│                                                                     │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐       ┌────────┐     │
│  │ E1     │ │ E2     │ │ E3     │ │ E4     │  ...  │ E13    │     │
│  │Semantic│ │Temporal│ │Periodic│ │Position│       │SPLADE  │     │
│  └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘       └───┬────┘     │
│      │          │          │          │                │          │
│      ▼          ▼          ▼          ▼                ▼          │
│  [cand_a]   [cand_b]   [cand_c]   [cand_d]   ...   [cand_n]       │
│                                                                     │
│                    UNION ALL CANDIDATES                             │
│                           │                                         │
│                           ▼                                         │
│              Unique candidate set (IDs only)                        │
└─────────────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                 STAGE 2: FULL TELEOLOGICAL COMPARISON               │
│                                                                     │
│  For each candidate:                                                │
│    1. Fetch complete TeleologicalArray from storage                 │
│    2. Compute per-embedder similarities (13 scores)                 │
│    3. Apply search matrix weights                                   │
│    4. Compute final aggregated score                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    STAGE 3: RANKING & FILTERING                     │
│                                                                     │
│  1. Sort by aggregated score                                        │
│  2. Apply min_similarity threshold                                  │
│  3. Apply teleological alignment filter (if enabled)                │
│  4. Truncate to top_k                                               │
│  5. Optionally analyze correlations                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                            │
                            ▼
                    Final SearchResults
```

## 2. Search Hooks

Claude Code hooks enable automatic query enhancement and context injection at critical points in the search pipeline. These hooks integrate seamlessly with the entry-point discovery architecture.

### 2.1 Hook Integration Architecture

```
User Query (natural language)
         │
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    UserPromptSubmit Hook                            │
│                                                                     │
│  1. Analyze query intent (semantic, temporal, causal, code)        │
│  2. Detect implicit constraints (time ranges, entity refs)          │
│  3. Expand query with synonyms/related concepts                     │
│  4. Select optimal search matrix preset                             │
│  5. Configure discovery strategy based on intent                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
         │
         ▼ (Enhanced Query)
┌─────────────────────────────────────────────────────────────────────┐
│                    PreToolUse Hook (search tools)                   │
│                                                                     │
│  1. Inject session context (recent memories, active goals)          │
│  2. Add teleological purpose alignment filters                      │
│  3. Apply user preference weights to search matrix                  │
│  4. Configure per-space thresholds from learned patterns            │
│  5. Enable/disable spaces based on query type                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
         │
         ▼
    Entry-Point Discovery → Full Comparison → Results
         │
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    PostToolUse Hook (search results)                │
│                                                                     │
│  1. Log search patterns for learning                                │
│  2. Update query-result correlation cache                           │
│  3. Track which embedding spaces contributed most                   │
│  4. Feed results to teleological purpose analyzer                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 UserPromptSubmit Hook Implementation

The `UserPromptSubmit` hook fires before any tool is called, enabling query enhancement.

```typescript
// .claude/hooks/search-query-enhance.ts
import { Hook, HookContext, QueryEnhancement } from '@claude-code/hooks';

interface SearchQueryContext extends HookContext {
  userQuery: string;
  sessionMemories: TeleologicalArray[];
  activeGoals: Purpose[];
}

export const searchQueryEnhanceHook: Hook<SearchQueryContext> = {
  name: 'search-query-enhance',
  event: 'UserPromptSubmit',

  // Only activate for search-related prompts
  matcher: (ctx) => {
    const searchKeywords = ['find', 'search', 'retrieve', 'lookup', 'recall',
                           'remember', 'what was', 'show me', 'get'];
    return searchKeywords.some(kw =>
      ctx.userQuery.toLowerCase().includes(kw)
    );
  },

  async execute(ctx: SearchQueryContext): Promise<QueryEnhancement> {
    const analysis = await analyzeQueryIntent(ctx.userQuery);

    return {
      // Enhanced query with expanded terms
      enhancedQuery: expandQuery(ctx.userQuery, analysis),

      // Recommended search configuration
      searchConfig: {
        preset: selectPreset(analysis),
        discoveryStrategy: selectStrategy(analysis),
        activeSpaces: selectSpaces(analysis),
        minSimilarity: analysis.precision === 'high' ? 0.7 : 0.4,
      },

      // Context to inject
      contextInjection: {
        recentMemories: ctx.sessionMemories.slice(-5),
        purposeAlignment: ctx.activeGoals,
        temporalContext: extractTemporalHints(ctx.userQuery),
      },

      // Metadata for learning
      metadata: {
        intentType: analysis.type,
        confidence: analysis.confidence,
        expansions: analysis.synonymsAdded,
      }
    };
  }
};

// Intent analysis determines optimal search configuration
async function analyzeQueryIntent(query: string): Promise<IntentAnalysis> {
  const patterns = {
    semantic: /what|meaning|about|explain|describe/i,
    temporal: /when|recent|yesterday|last|before|after|during/i,
    causal: /why|cause|because|result|effect|lead to/i,
    code: /function|class|method|api|implementation|code/i,
    entity: /who|person|company|project|named/i,
    relational: /related|similar|like|connected|associated/i,
  };

  const matches: IntentType[] = [];
  for (const [type, pattern] of Object.entries(patterns)) {
    if (pattern.test(query)) matches.push(type as IntentType);
  }

  return {
    type: matches[0] || 'semantic',
    secondaryTypes: matches.slice(1),
    confidence: matches.length > 0 ? 0.8 : 0.5,
    precision: query.includes('exactly') || query.includes('specific') ? 'high' : 'normal',
  };
}

// Select search matrix preset based on intent
function selectPreset(analysis: IntentAnalysis): string {
  const presetMap: Record<IntentType, string> = {
    semantic: 'semantic_dominant',
    temporal: 'temporal_aware',
    causal: 'knowledge_graph',
    code: 'code_focused',
    entity: 'knowledge_graph',
    relational: 'correlation_aware',
  };
  return presetMap[analysis.type] || 'identity';
}

// Select discovery strategy based on intent
function selectStrategy(analysis: IntentAnalysis): DiscoveryStrategy {
  if (analysis.precision === 'high') {
    return { type: 'QuorumN', n: 3 };
  }
  if (analysis.secondaryTypes.length >= 2) {
    return { type: 'UnionAll' };
  }
  return { type: 'Tiered', primary: analysis.type, expandThreshold: 50 };
}

// Select which embedding spaces to activate
function selectSpaces(analysis: IntentAnalysis): EmbedderMask {
  const spaceMap: Record<IntentType, Embedder[]> = {
    semantic: [Embedder.Semantic, Embedder.Contextual, Embedder.LateInteraction],
    temporal: [Embedder.TemporalRecent, Embedder.TemporalPeriodic, Embedder.TemporalPositional],
    causal: [Embedder.Causal, Embedder.Entity, Embedder.Graph],
    code: [Embedder.Code, Embedder.Semantic, Embedder.LateInteraction],
    entity: [Embedder.Entity, Embedder.Semantic, Embedder.Causal],
    relational: [Embedder.Graph, Embedder.Entity, Embedder.Causal],
  };

  const spaces = new Set(spaceMap[analysis.type] || [Embedder.Semantic]);
  analysis.secondaryTypes.forEach(t => {
    spaceMap[t]?.forEach(s => spaces.add(s));
  });

  return EmbedderMask.from([...spaces]);
}
```

### 2.3 PreToolUse Hook Implementation

The `PreToolUse` hook fires before each tool invocation, enabling context injection.

```typescript
// .claude/hooks/search-context-inject.ts
import { Hook, HookContext, ToolCall } from '@claude-code/hooks';

interface SearchToolContext extends HookContext {
  tool: ToolCall;
  sessionState: SessionState;
  teleologicalPurpose: Purpose;
}

export const searchContextInjectHook: Hook<SearchToolContext> = {
  name: 'search-context-inject',
  event: 'PreToolUse',

  // Only activate for search-related MCP tools
  matcher: (ctx) => {
    const searchTools = [
      'search_teleological',
      'search_single_space',
      'search_muvera',
      'search_semantic',
      'search_causal',
      'search_temporal',
    ];
    return searchTools.includes(ctx.tool.name);
  },

  async execute(ctx: SearchToolContext): Promise<ToolCallModification> {
    const params = ctx.tool.parameters;

    // Inject session context into search
    const enhancedParams = {
      ...params,

      // Add recent session context for relevance boosting
      sessionContext: {
        recentArrayIds: ctx.sessionState.recentMemories.map(m => m.id),
        activeTopics: ctx.sessionState.activeTopics,
        conversationEmbedding: ctx.sessionState.conversationEmbedding,
      },

      // Add teleological purpose alignment
      purposeFilter: {
        alignWithPurpose: ctx.teleologicalPurpose.id,
        minAlignment: 0.3,
        boostAligned: true,
      },

      // Inject learned space thresholds from ReasoningBank
      spaceThresholds: await getLearnedThresholds(ctx.tool.name),

      // Add user preference weights
      userWeights: ctx.sessionState.userPreferences?.searchWeights,
    };

    return {
      parameters: enhancedParams,
      metadata: {
        contextInjected: true,
        purposeId: ctx.teleologicalPurpose.id,
        sessionId: ctx.sessionState.id,
      }
    };
  }
};

// Retrieve learned per-space thresholds from past search patterns
async function getLearnedThresholds(toolName: string): Promise<number[] | null> {
  const patterns = await reasoningBank.searchPatterns({
    task: `search-${toolName}`,
    k: 10,
    minReward: 0.8,
  });

  if (patterns.length < 5) return null; // Not enough data

  // Aggregate successful threshold configurations
  const thresholds = new Array(13).fill(0);
  const counts = new Array(13).fill(0);

  patterns.forEach(pattern => {
    if (pattern.output?.spaceThresholds) {
      pattern.output.spaceThresholds.forEach((t: number, i: number) => {
        if (t > 0) {
          thresholds[i] += t;
          counts[i]++;
        }
      });
    }
  });

  return thresholds.map((t, i) => counts[i] > 0 ? t / counts[i] : 0.4);
}
```

### 2.4 PostToolUse Hook for Learning

```typescript
// .claude/hooks/search-result-learn.ts
import { Hook, HookContext, ToolResult } from '@claude-code/hooks';

export const searchResultLearnHook: Hook<HookContext> = {
  name: 'search-result-learn',
  event: 'PostToolUse',

  matcher: (ctx) => ctx.tool?.name?.startsWith('search_'),

  async execute(ctx: HookContext): Promise<void> {
    const result = ctx.toolResult as SearchResults;

    // Store search pattern for learning
    await reasoningBank.storePattern({
      sessionId: ctx.sessionId,
      task: `search-${ctx.tool.name}`,
      input: {
        query: ctx.tool.parameters.query,
        preset: ctx.tool.parameters.preset,
        discoveryStrategy: ctx.tool.parameters.discovery?.strategy,
      },
      output: {
        matchCount: result.matches.length,
        avgSimilarity: result.matches.reduce((s, m) => s + m.similarity, 0) / result.matches.length,
        contributingSpaces: result.discovery_stats.contributing_spaces,
        spaceThresholds: ctx.tool.parameters.spaceThresholds,
      },
      reward: calculateSearchReward(result, ctx),
      success: result.matches.length > 0,
      critique: generateSearchCritique(result),
      tokensUsed: ctx.tokensUsed,
      latencyMs: result.query_time_us / 1000,
    });

    // Update space contribution tracking
    await updateSpaceContributions(result.discovery_stats);
  }
};

function calculateSearchReward(result: SearchResults, ctx: HookContext): number {
  let reward = 0.5; // Base

  // Reward for finding results
  if (result.matches.length > 0) reward += 0.2;

  // Reward for high-quality matches
  const avgSim = result.matches.reduce((s, m) => s + m.similarity, 0) / result.matches.length;
  if (avgSim > 0.7) reward += 0.15;

  // Reward for efficient discovery (fewer candidates evaluated)
  const efficiency = result.matches.length / result.candidates_evaluated;
  if (efficiency > 0.1) reward += 0.1;

  // Penalty for timeout or errors
  if (result.query_time_us > 50000) reward -= 0.1;

  return Math.min(1.0, Math.max(0, reward));
}
```

### 2.5 Hook Configuration

```yaml
# .claude/hooks.yaml
hooks:
  - name: search-query-enhance
    event: UserPromptSubmit
    path: ./hooks/search-query-enhance.ts
    priority: 100
    config:
      enableExpansion: true
      maxExpansions: 5
      learnFromHistory: true

  - name: search-context-inject
    event: PreToolUse
    path: ./hooks/search-context-inject.ts
    priority: 90
    config:
      injectPurpose: true
      injectSession: true
      learnedThresholds: true

  - name: search-result-learn
    event: PostToolUse
    path: ./hooks/search-result-learn.ts
    priority: 80
    config:
      storePatterns: true
      updateContributions: true
      minRewardToStore: 0.3
```

## 3. Search Skills

Claude Code skills provide reusable, auto-invoked search capabilities. Skills are discovered based on user intent and can be explicitly invoked via slash commands.

### 3.1 Skill Discovery Architecture

```
User Input: "find memories related to authentication"
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Skill Discovery Engine                           │
│                                                                     │
│  1. Parse user intent keywords                                      │
│  2. Match against skill triggers:                                   │
│     - "find", "search", "retrieve" → semantic-search                │
│     - "related", "similar", "like" → multi-space-search             │
│     - "caused", "leads to", "because" → causal-search               │
│     - "when", "recent", "yesterday" → temporal-search               │
│  3. Score skill relevance                                           │
│  4. Auto-invoke highest-scoring skill                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                     │
                     ▼
         Invoke: semantic-search skill
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Skill Execution                                  │
│                                                                     │
│  1. Load skill definition from SKILL.md                             │
│  2. Apply skill-specific search configuration                       │
│  3. Execute search via MCP tools                                    │
│  4. Post-process results according to skill template                │
│  5. Format output for user                                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Semantic Search Skill

```yaml
# .claude/skills/semantic-search/SKILL.md
---
name: semantic-search
description: Search memories by semantic meaning and conceptual similarity
version: 1.0.0
triggers:
  - find
  - search
  - lookup
  - "what was"
  - "show me"
  - recall
  - remember
arguments:
  - name: query
    type: string
    required: true
    description: Natural language search query
  - name: top_k
    type: number
    default: 10
    description: Maximum results to return
  - name: min_similarity
    type: number
    default: 0.5
    description: Minimum similarity threshold
---

# Semantic Search Skill

Performs semantic similarity search across the teleological memory system,
prioritizing meaning and conceptual alignment over keyword matching.

## Search Configuration

This skill uses the `semantic_dominant` search matrix:
- 50% weight on E1 (Semantic embeddings)
- Balanced weight on supporting spaces
- UnionAll discovery strategy for broad recall

## Execution Steps

1. **Query Enhancement**
   - Expand query with conceptual synonyms
   - Detect implicit entity references
   - Extract temporal hints if present

2. **Search Execution**
   ```
   SearchQueryBuilder::new()
     .query(embed_query(user_query))
     .preset("semantic")
     .top_k({{top_k}})
     .min_similarity({{min_similarity}})
     .with_breakdown()
     .build()
   ```

3. **Result Formatting**
   - Group by teleological purpose
   - Highlight matching concepts
   - Show similarity breakdown if requested

## Example Usage

```
User: find memories about user authentication
Skill: Searching semantic space for "user authentication"...

Found 8 relevant memories:

1. [0.92] Authentication flow redesign (2024-01-15)
   Purpose: Security Enhancement
   Concepts: OAuth, JWT, session management

2. [0.87] User login rate limiting implementation
   Purpose: Security Enhancement
   Concepts: rate limiting, brute force prevention

3. [0.81] Password hashing migration to Argon2
   Purpose: Security Enhancement
   Concepts: password security, hashing algorithms
```
```

### 3.3 Entry-Point Search Skill

```yaml
# .claude/skills/entry-point-search/SKILL.md
---
name: entry-point-search
description: Multi-space entry-point discovery across all 13 embedding spaces
version: 1.0.0
triggers:
  - explore
  - discover
  - "any connection"
  - "find anything"
  - comprehensive
arguments:
  - name: query
    type: string
    required: true
  - name: candidates_per_space
    type: number
    default: 100
  - name: discovery_strategy
    type: string
    default: UnionAll
    enum: [UnionAll, QuorumN, Tiered, Muvera]
  - name: quorum_n
    type: number
    default: 2
---

# Entry-Point Search Skill

Comprehensive multi-space search that discovers relevant memories through
ANY of the 13 embedding spaces. A memory is considered relevant if it's
close to the query in even ONE embedding space.

## The Entry-Point Advantage

Traditional search only finds semantically similar items. Entry-point search
finds items that are related through:
- **Semantic meaning** (E1)
- **Temporal proximity** (E2-E4)
- **Causal relationships** (E5)
- **Lexical overlap** (E6, E13)
- **Code structure** (E7)
- **Graph connections** (E8)
- **Entity links** (E11)
- **Token-level precision** (E12)

## Search Configuration

```
DiscoveryConfig {
  active_spaces: EmbedderMask::all(),  // All 13 spaces
  candidates_per_space: {{candidates_per_space}},
  strategy: {{discovery_strategy}},
  space_thresholds: None,  // Use defaults
}
```

## Discovery Strategies

| Strategy | Use Case | Trade-off |
|----------|----------|-----------|
| UnionAll | Exploration, recall-focused | Higher recall, more candidates |
| QuorumN(n) | Precision-focused | Only items in N+ spaces |
| Tiered | Balanced | Start narrow, expand if needed |
| Muvera | Performance-critical | 8x faster, slight recall loss |

## Execution Flow

1. **Parallel Space Search**
   - Search all 13 indices simultaneously
   - Collect top candidates from each space
   - Track which spaces discovered each candidate

2. **Candidate Aggregation**
   - Union all discovered candidates
   - Apply discovery strategy filter
   - Deduplicate by memory ID

3. **Full Array Comparison**
   - Fetch complete teleological arrays
   - Compute per-embedder similarities (apples-to-apples)
   - Apply search matrix weights
   - Rank by aggregated score

4. **Result Enrichment**
   - Show discovery provenance (which spaces found it)
   - Include per-embedder breakdown
   - Analyze cross-embedder correlations

## Example Usage

```
User: explore any connection to the payment system refactor

Skill: Searching all 13 embedding spaces...

Discovery Summary:
- E1 (Semantic): 45 candidates
- E5 (Causal): 23 candidates
- E7 (Code): 67 candidates
- E11 (Entity): 12 candidates
- Total unique: 98 candidates

Top Results:

1. [0.89] Payment gateway abstraction layer
   Discovered via: E1 (semantic), E7 (code), E5 (causal)
   Similarity breakdown:
     Semantic: 0.91, Code: 0.88, Causal: 0.79

2. [0.84] Transaction rollback mechanism
   Discovered via: E5 (causal), E7 (code)
   Correlation: Strong causal-code alignment (0.92)

3. [0.78] Stripe API integration notes
   Discovered via: E1 (semantic), E11 (entity)
   Note: Entity "Stripe" linked to payment domain
```
```

### 3.4 Multi-Space Search Skill

```yaml
# .claude/skills/multi-space-search/SKILL.md
---
name: multi-space-search
description: Search with explicit control over which embedding spaces to use
version: 1.0.0
triggers:
  - multi-space
  - combined search
  - hybrid search
  - search using
arguments:
  - name: query
    type: string
    required: true
  - name: spaces
    type: array
    items:
      type: string
      enum: [semantic, temporal, causal, sparse, code, graph, entity, multimodal, splade]
    required: true
  - name: aggregation
    type: string
    default: weighted_sum
    enum: [weighted_sum, rrf, max, min, geometric]
  - name: weights
    type: object
    description: Per-space weights (optional)
---

# Multi-Space Search Skill

Explicitly control which embedding spaces to search and how to combine
their results. Useful when you know the nature of your query.

## Space Selection Guide

| Space | Good For | Example Query |
|-------|----------|---------------|
| semantic | Conceptual meaning | "authentication patterns" |
| temporal | Time-based recall | "what happened yesterday" |
| causal | Cause-effect chains | "why did the build fail" |
| sparse | Exact keywords | "findUserById function" |
| code | Code structure | "async error handling" |
| graph | Relationship traversal | "related to user service" |
| entity | Named entity lookup | "mentions Stripe" |
| multimodal | Cross-modal search | "diagram of architecture" |
| splade | Keyword expansion | "login authentication auth" |

## Aggregation Strategies

- **weighted_sum**: Σ(weight_i * score_i) - balanced fusion
- **rrf**: Reciprocal Rank Fusion - good for hybrid
- **max**: Best score across spaces - OR-like behavior
- **min**: Worst score across spaces - AND-like behavior
- **geometric**: (Π score_i^weight_i) - penalizes low scores

## Example Usage

```
User: /multi-space-search --spaces semantic,code,causal --query "error handling in payment service"

Skill: Searching 3 spaces with weighted_sum aggregation...

Space Contributions:
- Semantic (E1): weight=0.4
- Code (E7): weight=0.4
- Causal (E5): weight=0.2

Results:

1. [0.91] PaymentService.handleTransactionError()
   Scores: semantic=0.88, code=0.95, causal=0.85
   Aggregated: 0.4*0.88 + 0.4*0.95 + 0.2*0.85 = 0.91

2. [0.86] Error recovery workflow documentation
   Scores: semantic=0.92, code=0.72, causal=0.91
   Aggregated: 0.4*0.92 + 0.4*0.72 + 0.2*0.91 = 0.84
```
```

### 3.5 Context Search Skill (Complete Example)

```yaml
# .claude/skills/context-search/SKILL.md
---
name: context-search
description: |
  Search for relevant context by combining session history, teleological
  purpose alignment, and multi-space similarity. Optimized for providing
  context to AI assistants during conversation.
version: 1.0.0
author: contextgraph
triggers:
  - context
  - "get context"
  - "find context"
  - "what context"
  - "relevant context"
  - background
arguments:
  - name: query
    type: string
    required: true
    description: The topic or question needing context
  - name: depth
    type: string
    default: normal
    enum: [shallow, normal, deep]
    description: How much context to retrieve
  - name: recency_weight
    type: number
    default: 0.3
    min: 0
    max: 1
    description: Weight given to recent memories vs semantic relevance
  - name: purpose_alignment
    type: boolean
    default: true
    description: Filter results by teleological purpose alignment
  - name: include_causal_chain
    type: boolean
    default: false
    description: Include causal predecessors and successors
tools:
  - mcp__contextgraph__search_teleological
  - mcp__contextgraph__get_purpose_chain
  - mcp__contextgraph__expand_causal
config:
  max_results:
    shallow: 5
    normal: 15
    deep: 50
  discovery_strategy:
    shallow: Tiered
    normal: UnionAll
    deep: UnionAll
  search_matrix:
    shallow: entry_point_optimized
    normal: correlation_aware
    deep: recall_maximizing
---

# Context Search Skill

Retrieves comprehensive context for a given topic by searching across
multiple embedding spaces and filtering by teleological purpose alignment.

## Overview

This skill is designed for AI assistants that need relevant context to
answer questions or complete tasks. It combines:

1. **Multi-space discovery** - Find context through any relevant dimension
2. **Session awareness** - Boost recent and conversation-relevant items
3. **Purpose alignment** - Filter by teleological goal compatibility
4. **Causal expansion** - Optionally include cause-effect chains

## Search Configuration by Depth

### Shallow (Quick context)
```rust
SearchQuery {
  discovery: DiscoveryConfig {
    strategy: Tiered {
      primary: [Semantic, Sparse],
      expand_threshold: 20,
    },
    candidates_per_space: 50,
  },
  comparison: MatrixStrategy(entry_point_optimized),
  top_k: 5,
}
```

### Normal (Balanced)
```rust
SearchQuery {
  discovery: DiscoveryConfig {
    strategy: UnionAll,
    candidates_per_space: 100,
  },
  comparison: MatrixStrategy(correlation_aware),
  top_k: 15,
}
```

### Deep (Comprehensive)
```rust
SearchQuery {
  discovery: DiscoveryConfig {
    strategy: UnionAll,
    candidates_per_space: 200,
  },
  comparison: MatrixStrategy(recall_maximizing),
  top_k: 50,
}
```

## Execution Steps

### Step 1: Session Context Injection

The skill automatically injects session context via the PreToolUse hook:

```typescript
// Injected by search-context-inject hook
{
  sessionContext: {
    recentArrayIds: [...],  // Last 10 memories accessed
    activeTopics: [...],     // Current conversation topics
    conversationEmbedding: [...],  // Embedding of recent turns
  }
}
```

### Step 2: Query Enhancement

The UserPromptSubmit hook enhances the query:

```typescript
// Enhanced query structure
{
  originalQuery: "{{query}}",
  expandedTerms: [...],  // Synonyms and related concepts
  detectedEntities: [...],  // Named entities in query
  temporalHints: {...},  // Time references if any
}
```

### Step 3: Multi-Space Search

Execute entry-point discovery across all relevant spaces:

```rust
let discovery = EntryPointDiscovery::new(indices);
let candidates = discovery.discover(
  &query_array,
  &config.discovery,
).await?;

// candidates contains items found in ANY space
// with provenance tracking (which spaces found each)
```

### Step 4: Purpose Alignment Filter

If `purpose_alignment` is enabled:

```rust
let aligned = candidates.into_iter()
  .filter(|c| {
    let alignment = compute_purpose_alignment(
      &c.array.purpose,
      &session.active_purpose,
    );
    alignment >= 0.3  // Configurable threshold
  })
  .collect();
```

### Step 5: Causal Chain Expansion

If `include_causal_chain` is enabled:

```rust
for candidate in &aligned {
  // Get causal predecessors (what caused this)
  let predecessors = expand_causal(
    candidate.id,
    CausalDirection::Backward,
    depth: 2,
  ).await?;

  // Get causal successors (what this caused)
  let successors = expand_causal(
    candidate.id,
    CausalDirection::Forward,
    depth: 2,
  ).await?;

  candidate.causal_context = CausalChain {
    predecessors,
    successors,
  };
}
```

### Step 6: Result Formatting

Format results with rich context information:

```
## Context for: "{{query}}"

### Primary Context ({{matches.len()}} items)

{{#each matches}}
#### {{rank}}. {{title}} [{{similarity | format_percent}}]

**Purpose**: {{purpose.description}}
**Alignment**: {{purpose_alignment | format_percent}}
**Discovered via**: {{discovered_via | format_spaces}}

{{summary}}

{{#if causal_context}}
**Causal Chain**:
  Caused by: {{causal_context.predecessors | format_list}}
  Led to: {{causal_context.successors | format_list}}
{{/if}}

---
{{/each}}

### Discovery Statistics
- Spaces searched: {{discovery_stats.active_spaces | count}}
- Candidates found: {{discovery_stats.total_candidates}}
- After purpose filter: {{matches.len}}
- Query time: {{query_time_us | format_duration}}
```

## Example Usage

### Basic Context Search
```
User: /context-search "payment processing architecture"

Skill: Searching for context on "payment processing architecture"...

## Context for: "payment processing architecture"

### Primary Context (8 items)

#### 1. Payment Gateway Abstraction Layer [94%]

**Purpose**: System Architecture
**Alignment**: 92%
**Discovered via**: Semantic, Code, Entity

The payment system uses a gateway abstraction pattern to support
multiple payment providers (Stripe, PayPal, Square). The abstraction
layer handles provider-specific API differences...

---

#### 2. Transaction State Machine Design [89%]

**Purpose**: System Architecture
**Alignment**: 88%
**Discovered via**: Semantic, Causal

Transactions follow a state machine: PENDING → PROCESSING →
COMPLETED/FAILED. State transitions are idempotent and logged
for audit purposes...
```

### Deep Context with Causal Chains
```
User: /context-search --depth deep --include_causal_chain "payment failures"

Skill: Deep context search with causal expansion...

## Context for: "payment failures"

#### 1. Stripe Webhook Timeout Issue [91%]

**Purpose**: Incident Resolution
**Alignment**: 95%

**Causal Chain**:
  Caused by:
    - Network latency spike (Jan 15)
    - Misconfigured timeout settings
  Led to:
    - Payment retry queue overflow
    - Customer complaint ticket #4521
    - Timeout configuration update (deployed Jan 16)

The Stripe webhook endpoint was timing out under load due to...
```

## Integration with Subagents

This skill can be executed by search subagents in a swarm:

```typescript
// Spawn context-search subagent
const agent = await swarm.spawn({
  type: 'context-search-agent',
  task: 'Find context for user query',
  skill: 'context-search',
  params: {
    query: userQuery,
    depth: 'normal',
    purpose_alignment: true,
  }
});
```

## Performance Characteristics

| Depth | Avg Latency | Memory | Use Case |
|-------|-------------|--------|----------|
| shallow | <20ms | ~1MB | Quick lookups |
| normal | <50ms | ~5MB | General context |
| deep | <150ms | ~20MB | Research, analysis |
```

### 3.6 Skill Auto-Discovery Configuration

```yaml
# .claude/skills/config.yaml
skill_discovery:
  enabled: true

  # Trigger keyword matching
  trigger_matching:
    mode: fuzzy  # exact, fuzzy, semantic
    min_confidence: 0.7

  # Priority when multiple skills match
  priority_rules:
    - if: query.contains("code")
      prefer: [code-search, multi-space-search]
    - if: query.contains("recent") or query.contains("yesterday")
      prefer: [temporal-search, context-search]
    - if: query.contains("why") or query.contains("cause")
      prefer: [causal-search, entry-point-search]

  # Default skill when no specific match
  default_skill: semantic-search

  # Skills to always consider
  always_consider:
    - context-search  # Good general fallback

  # Skill combinations
  allow_chaining: true
  max_chain_length: 3

# Auto-invoke configuration
auto_invoke:
  enabled: true

  # Keyword triggers for auto-invocation
  triggers:
    semantic-search:
      keywords: [find, search, lookup, recall, remember]
      confidence_threshold: 0.8

    entry-point-search:
      keywords: [explore, discover, comprehensive, "any connection"]
      confidence_threshold: 0.7

    context-search:
      keywords: [context, background, relevant, "what do we know"]
      confidence_threshold: 0.75

    causal-search:
      keywords: [why, cause, because, "led to", effect, reason]
      confidence_threshold: 0.8

    temporal-search:
      keywords: [when, recent, yesterday, "last week", before, after]
      confidence_threshold: 0.85
```

## 4. Search Subagents

Specialized agents coordinate complex multi-space searches, enabling parallel execution and intelligent result aggregation.

### 4.1 Subagent Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Search Coordinator Agent                         │
│                                                                     │
│  Responsibilities:                                                  │
│  - Parse user query intent                                          │
│  - Select appropriate search subagents                              │
│  - Coordinate parallel execution                                    │
│  - Aggregate and rank results                                       │
│  - Handle timeouts and fallbacks                                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
         │
         ├──────────────────┬──────────────────┬──────────────────┐
         ▼                  ▼                  ▼                  ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ Semantic Search │ │ Causal Search   │ │ Temporal Search │ │ Code Search     │
│ Agent           │ │ Agent           │ │ Agent           │ │ Agent           │
│                 │ │                 │ │                 │ │                 │
│ Spaces: E1, E10 │ │ Spaces: E5, E11 │ │ Spaces: E2-E4   │ │ Spaces: E7, E12 │
│ Matrix: semantic│ │ Matrix: knowl.  │ │ Matrix: temporal│ │ Matrix: code    │
└─────────────────┘ └─────────────────┘ └─────────────────┘ └─────────────────┘
         │                  │                  │                  │
         └──────────────────┴──────────────────┴──────────────────┘
                                    │
                                    ▼
                           Result Aggregation
                                    │
                                    ▼
                           Final Ranked Results
```

### 4.2 Search Coordinator Agent

```typescript
// agents/search-coordinator.ts
import { Agent, AgentConfig, Task } from '@claude-flow/agents';

export const searchCoordinatorAgent: AgentConfig = {
  name: 'search-coordinator',
  type: 'coordinator',
  description: 'Coordinates multi-space search across specialized subagents',

  capabilities: [
    'query-intent-analysis',
    'subagent-orchestration',
    'result-aggregation',
    'timeout-handling',
  ],

  async execute(task: Task): Promise<SearchResults> {
    const { query, options } = task.input;

    // Step 1: Analyze query to determine which subagents to spawn
    const intent = await analyzeQueryIntent(query);
    const subagentConfig = selectSubagents(intent);

    // Step 2: Spawn specialized search subagents in parallel
    const subagents = await Promise.all(
      subagentConfig.map(config =>
        this.spawnSubagent(config.type, {
          query,
          spaces: config.spaces,
          matrix: config.matrix,
          top_k: Math.ceil(options.top_k / subagentConfig.length * 1.5),
        })
      )
    );

    // Step 3: Wait for all subagents with timeout
    const timeout = options.timeout || 5000;
    const subResults = await Promise.race([
      Promise.all(subagents.map(a => a.waitForResult())),
      sleep(timeout).then(() => 'timeout'),
    ]);

    // Step 4: Handle partial results on timeout
    if (subResults === 'timeout') {
      const completed = subagents
        .filter(a => a.status === 'completed')
        .map(a => a.result);
      return this.aggregateResults(completed, { partial: true });
    }

    // Step 5: Aggregate and rank all results
    return this.aggregateResults(subResults as SearchResults[], {
      partial: false,
      fusionStrategy: options.fusion || 'rrf',
    });
  },

  aggregateResults(
    results: SearchResults[],
    options: AggregationOptions
  ): SearchResults {
    // Collect all matches with provenance
    const allMatches: Map<string, AggregatedMatch> = new Map();

    for (const result of results) {
      for (const match of result.matches) {
        const existing = allMatches.get(match.array.id);
        if (existing) {
          // Merge scores using fusion strategy
          existing.scores.push(match.similarity);
          existing.sources.push(result.source);
          existing.discoveredVia = existing.discoveredVia.union(match.discovered_via);
        } else {
          allMatches.set(match.array.id, {
            array: match.array,
            scores: [match.similarity],
            sources: [result.source],
            discoveredVia: match.discovered_via,
          });
        }
      }
    }

    // Apply fusion strategy
    const ranked = [...allMatches.values()].map(m => ({
      ...m,
      finalScore: this.fuseScores(m.scores, options.fusionStrategy),
    }));

    ranked.sort((a, b) => b.finalScore - a.finalScore);

    return {
      matches: ranked.slice(0, options.top_k || 10),
      aggregation: {
        subagentCount: results.length,
        fusionStrategy: options.fusionStrategy,
        partial: options.partial,
      },
    };
  },

  fuseScores(scores: number[], strategy: string): number {
    switch (strategy) {
      case 'rrf':
        // Reciprocal Rank Fusion
        return scores.reduce((sum, s, i) => sum + 1 / (60 + i), 0);
      case 'max':
        return Math.max(...scores);
      case 'avg':
        return scores.reduce((a, b) => a + b, 0) / scores.length;
      case 'geometric':
        return Math.pow(scores.reduce((a, b) => a * b, 1), 1 / scores.length);
      default:
        return scores[0];
    }
  }
};

function selectSubagents(intent: IntentAnalysis): SubagentConfig[] {
  const configs: SubagentConfig[] = [];

  // Always include semantic search
  configs.push({
    type: 'semantic-search-agent',
    spaces: ['E1', 'E10'],
    matrix: 'semantic_dominant',
    weight: 1.0,
  });

  // Add intent-specific subagents
  if (intent.types.includes('causal')) {
    configs.push({
      type: 'causal-search-agent',
      spaces: ['E5', 'E11'],
      matrix: 'knowledge_graph',
      weight: 0.8,
    });
  }

  if (intent.types.includes('temporal')) {
    configs.push({
      type: 'temporal-search-agent',
      spaces: ['E2', 'E3', 'E4'],
      matrix: 'temporal_aware',
      weight: 0.7,
    });
  }

  if (intent.types.includes('code')) {
    configs.push({
      type: 'code-search-agent',
      spaces: ['E7', 'E12'],
      matrix: 'code_focused',
      weight: 0.9,
    });
  }

  if (intent.types.includes('entity')) {
    configs.push({
      type: 'entity-search-agent',
      spaces: ['E11', 'E8'],
      matrix: 'knowledge_graph',
      weight: 0.6,
    });
  }

  return configs;
}
```

### 4.3 Semantic Search Agent

```typescript
// agents/semantic-search-agent.ts
export const semanticSearchAgent: AgentConfig = {
  name: 'semantic-search-agent',
  type: 'specialist',
  description: 'Specialized agent for semantic similarity search',

  capabilities: [
    'semantic-embedding',
    'conceptual-expansion',
    'synonym-matching',
  ],

  defaultConfig: {
    spaces: ['E1', 'E10'],  // Semantic + Multimodal
    matrix: 'semantic_dominant',
    discoveryStrategy: 'Tiered',
    candidatesPerSpace: 100,
  },

  async execute(task: Task): Promise<SearchResults> {
    const { query, spaces, matrix, top_k } = task.input;

    // Create teleological array from query
    const queryArray = await this.embedQuery(query, spaces);

    // Build search query
    const searchQuery = SearchQueryBuilder::new()
      .query(queryArray)
      .preset(matrix)
      .only_spaces(EmbedderMask::from(spaces))
      .candidates_per_space(this.config.candidatesPerSpace)
      .top_k(top_k)
      .with_breakdown()
      .build();

    // Execute search
    const results = await this.searchEngine.search(searchQuery);

    // Enrich results with semantic highlights
    return {
      ...results,
      source: 'semantic-search-agent',
      enrichments: await this.highlightSemanticMatches(query, results.matches),
    };
  },

  async embedQuery(query: string, spaces: string[]): Promise<TeleologicalArray> {
    // Use only the embedders for specified spaces
    const embeddings = await Promise.all(
      spaces.map(space => this.embedders[space].embed(query))
    );

    return TeleologicalArray.partial(embeddings, spaces);
  },

  async highlightSemanticMatches(
    query: string,
    matches: SearchMatch[]
  ): Promise<SemanticHighlight[]> {
    // Find which concepts in the query match which parts of results
    const queryTerms = await extractConcepts(query);

    return matches.map(match => ({
      matchId: match.array.id,
      highlights: findConceptOverlap(queryTerms, match.array.content),
    }));
  }
};
```

### 4.4 Causal Search Agent

```typescript
// agents/causal-search-agent.ts
export const causalSearchAgent: AgentConfig = {
  name: 'causal-search-agent',
  type: 'specialist',
  description: 'Specialized agent for causal relationship search',

  capabilities: [
    'causal-chain-traversal',
    'cause-effect-detection',
    'temporal-ordering',
  ],

  defaultConfig: {
    spaces: ['E5', 'E11', 'E8'],  // Causal + Entity + Graph
    matrix: 'knowledge_graph',
    discoveryStrategy: 'QuorumN',
    quorumN: 2,
    candidatesPerSpace: 150,
  },

  async execute(task: Task): Promise<SearchResults> {
    const { query, direction, depth } = task.input;

    // Detect causal direction in query
    const causalIntent = this.detectCausalDirection(query);

    // Create asymmetric causal embedding
    const queryArray = await this.embedCausalQuery(query, causalIntent);

    // Build search with causal-aware configuration
    const searchQuery = SearchQueryBuilder::new()
      .query(queryArray)
      .preset('knowledge_graph')
      .quorum(this.config.quorumN)
      .candidates_per_space(this.config.candidatesPerSpace)
      .top_k(task.input.top_k * 2)  // Get more for chain expansion
      .with_breakdown()
      .with_correlations()
      .build();

    let results = await this.searchEngine.search(searchQuery);

    // Expand causal chains if requested
    if (depth > 0) {
      results = await this.expandCausalChains(results, depth, causalIntent);
    }

    return {
      ...results,
      source: 'causal-search-agent',
      causalAnalysis: {
        direction: causalIntent,
        chainDepth: depth,
        rootCauses: this.identifyRootCauses(results),
        terminalEffects: this.identifyTerminalEffects(results),
      },
    };
  },

  detectCausalDirection(query: string): CausalDirection {
    const causePatterns = /why|cause|because|due to|led to|resulted from/i;
    const effectPatterns = /what happened|effect|result|consequence|outcome/i;

    if (causePatterns.test(query)) return 'backward';  // Looking for causes
    if (effectPatterns.test(query)) return 'forward';   // Looking for effects
    return 'bidirectional';
  },

  async expandCausalChains(
    results: SearchResults,
    depth: number,
    direction: CausalDirection
  ): Promise<SearchResults> {
    const expanded = new Map<string, CausalNode>();

    // BFS expansion of causal graph
    const queue = results.matches.map(m => ({ id: m.array.id, depth: 0 }));

    while (queue.length > 0) {
      const { id, depth: currentDepth } = queue.shift()!;
      if (currentDepth >= depth || expanded.has(id)) continue;

      const links = await this.getCausalLinks(id, direction);

      for (const link of links) {
        if (!expanded.has(link.targetId)) {
          expanded.set(link.targetId, {
            id: link.targetId,
            relation: link.relation,
            strength: link.strength,
            parentId: id,
            depth: currentDepth + 1,
          });
          queue.push({ id: link.targetId, depth: currentDepth + 1 });
        }
      }
    }

    // Fetch expanded nodes and add to results
    const expandedArrays = await this.storage.fetchBatch([...expanded.keys()]);

    return {
      ...results,
      causalExpansion: {
        nodes: [...expanded.values()],
        arrays: expandedArrays,
      },
    };
  }
};
```

### 4.5 Temporal Search Agent

```typescript
// agents/temporal-search-agent.ts
export const temporalSearchAgent: AgentConfig = {
  name: 'temporal-search-agent',
  type: 'specialist',
  description: 'Specialized agent for time-aware search',

  capabilities: [
    'temporal-parsing',
    'recency-weighting',
    'periodic-pattern-detection',
  ],

  defaultConfig: {
    spaces: ['E2', 'E3', 'E4'],  // TemporalRecent + Periodic + Positional
    matrix: 'temporal_aware',
    discoveryStrategy: 'UnionAll',
    candidatesPerSpace: 100,
  },

  async execute(task: Task): Promise<SearchResults> {
    const { query, timeRange } = task.input;

    // Parse temporal expressions in query
    const temporalContext = this.parseTemporalQuery(query);

    // Create temporally-aware embeddings
    const queryArray = await this.embedTemporalQuery(query, temporalContext);

    // Build search with temporal filters
    const searchQuery = SearchQueryBuilder::new()
      .query(queryArray)
      .preset('temporal_aware')
      .only_spaces(EmbedderMask::from(['E2', 'E3', 'E4', 'E1']))
      .top_k(task.input.top_k)
      .with_breakdown();

    // Apply temporal filters
    if (temporalContext.after) {
      searchQuery.created_after(temporalContext.after);
    }
    if (temporalContext.before) {
      searchQuery.created_before(temporalContext.before);
    }

    const results = await this.searchEngine.search(searchQuery.build());

    // Re-rank by temporal relevance
    const reranked = this.rerankByTemporalRelevance(
      results,
      temporalContext,
      task.input.recency_weight || 0.3
    );

    return {
      ...reranked,
      source: 'temporal-search-agent',
      temporalAnalysis: {
        parsedContext: temporalContext,
        timeDistribution: this.analyzeTimeDistribution(reranked.matches),
        periodicPatterns: this.detectPeriodicPatterns(reranked.matches),
      },
    };
  },

  parseTemporalQuery(query: string): TemporalContext {
    // Parse natural language time expressions
    const patterns = {
      yesterday: () => ({ after: daysAgo(1), before: daysAgo(0) }),
      'last week': () => ({ after: daysAgo(7), before: daysAgo(0) }),
      'last month': () => ({ after: daysAgo(30), before: daysAgo(0) }),
      recent: () => ({ after: daysAgo(3), before: daysAgo(0) }),
      'this morning': () => ({ after: todayAt(6), before: todayAt(12) }),
    };

    for (const [pattern, resolver] of Object.entries(patterns)) {
      if (query.toLowerCase().includes(pattern)) {
        return resolver();
      }
    }

    return {};
  },

  rerankByTemporalRelevance(
    results: SearchResults,
    context: TemporalContext,
    recencyWeight: number
  ): SearchResults {
    const now = Date.now();

    const reranked = results.matches.map(match => {
      const age = now - match.array.created_at.getTime();
      const recencyScore = Math.exp(-age / (7 * 24 * 60 * 60 * 1000)); // 7-day decay

      const finalScore =
        (1 - recencyWeight) * match.similarity +
        recencyWeight * recencyScore;

      return { ...match, similarity: finalScore, originalSimilarity: match.similarity };
    });

    reranked.sort((a, b) => b.similarity - a.similarity);

    return { ...results, matches: reranked };
  }
};
```

### 4.6 Subagent Swarm Configuration

```yaml
# .claude/agents/search-swarm.yaml
swarm:
  name: search-swarm
  topology: hierarchical

  coordinator:
    agent: search-coordinator
    config:
      timeout: 5000
      fusion_strategy: rrf
      min_subagents: 1
      max_subagents: 5

  specialists:
    - agent: semantic-search-agent
      always_include: true
      weight: 1.0

    - agent: causal-search-agent
      include_when:
        intent_contains: [causal, why, cause, effect]
      weight: 0.8

    - agent: temporal-search-agent
      include_when:
        intent_contains: [temporal, when, recent, yesterday]
      weight: 0.7

    - agent: code-search-agent
      include_when:
        intent_contains: [code, function, class, implementation]
      weight: 0.9

    - agent: entity-search-agent
      include_when:
        has_named_entities: true
      weight: 0.6

  aggregation:
    strategy: rrf
    k: 60
    dedup: true
    max_results: 50

  fallback:
    on_timeout: return_partial
    on_error: retry_with_semantic_only
    max_retries: 2
```

### 4.7 Spawning Search Subagents

```typescript
// Example: Spawning search swarm for complex query
async function executeSearchWithSubagents(query: string): Promise<SearchResults> {
  // Initialize swarm
  const swarm = await mcp__claude_flow__swarm_init({
    topology: 'hierarchical',
    maxAgents: 5,
    strategy: 'adaptive',
  });

  // Spawn coordinator
  const coordinator = await mcp__claude_flow__agent_spawn({
    type: 'coordinator',
    name: 'search-coordinator',
    capabilities: ['query-intent-analysis', 'result-aggregation'],
  });

  // Coordinator will spawn specialist subagents based on query intent
  const task = await mcp__claude_flow__task_orchestrate({
    task: `Search for: ${query}`,
    strategy: 'parallel',
    priority: 'high',
  });

  // Wait for results
  const results = await mcp__claude_flow__task_results({
    taskId: task.id,
  });

  return results;
}
```

## 5. Search Engine Interface

### 5.1 Core Trait

```rust
/// Primary search engine for teleological arrays
#[async_trait]
pub trait TeleologicalSearchEngine: Send + Sync {
    /// Execute a search query using entry-point discovery
    async fn search(
        &self,
        query: SearchQuery,
    ) -> Result<SearchResults, SearchError>;

    /// Execute multiple queries in parallel
    async fn search_batch(
        &self,
        queries: Vec<SearchQuery>,
    ) -> Result<Vec<SearchResults>, SearchError>;

    /// Get search statistics
    fn stats(&self) -> SearchStats;
}

/// A search query specifying what to find and how
#[derive(Clone, Debug)]
pub struct SearchQuery {
    /// The query teleological array
    pub query_array: TeleologicalArray,

    /// How to compare arrays (search matrix configuration)
    pub comparison: ComparisonType,

    /// Maximum results to return
    pub top_k: usize,

    /// Entry-point discovery configuration
    pub discovery: DiscoveryConfig,

    /// Optional filters
    pub filter: Option<SearchFilter>,

    /// Optional: minimum similarity threshold
    pub min_similarity: Option<f32>,

    /// Whether to include per-embedder breakdown in results
    pub include_breakdown: bool,

    /// Whether to analyze cross-embedder correlations
    pub analyze_correlations: bool,
}

/// Configuration for entry-point discovery
#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    /// Which embedding spaces to use for discovery (default: all 13)
    pub active_spaces: EmbedderMask,

    /// Candidates per space (higher = better recall, slower)
    pub candidates_per_space: usize,

    /// Discovery strategy
    pub strategy: DiscoveryStrategy,

    /// Per-space similarity thresholds for candidate inclusion
    pub space_thresholds: Option<[f32; 13]>,
}

#[derive(Clone, Copy, Debug)]
pub enum DiscoveryStrategy {
    /// Search all spaces, union all results (default, highest recall)
    UnionAll,

    /// Search all spaces, require match in at least N spaces
    QuorumN(usize),

    /// Search primary spaces first, expand if needed
    Tiered { primary_spaces: EmbedderMask, expand_threshold: usize },

    /// MUVERA-style fixed dimensional encoding
    Muvera { k_sim: usize, dim_proj: usize, r_reps: usize },
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            active_spaces: EmbedderMask::all(), // All 13 spaces
            candidates_per_space: 100,
            strategy: DiscoveryStrategy::UnionAll,
            space_thresholds: None,
        }
    }
}

/// Results from a search query
#[derive(Clone, Debug)]
pub struct SearchResults {
    /// Matched arrays with scores
    pub matches: Vec<SearchMatch>,

    /// Query execution time
    pub query_time_us: u64,

    /// Number of candidates discovered (before filtering)
    pub candidates_discovered: usize,

    /// Number of candidates evaluated (after deduplication)
    pub candidates_evaluated: usize,

    /// Per-space discovery stats
    pub discovery_stats: DiscoveryStats,

    /// Query metadata
    pub metadata: SearchMetadata,
}

#[derive(Clone, Debug)]
pub struct DiscoveryStats {
    /// Candidates found per embedding space
    pub candidates_per_space: [usize; 13],

    /// Time spent per space (microseconds)
    pub time_per_space_us: [u64; 13],

    /// Which spaces contributed to final results
    pub contributing_spaces: EmbedderMask,

    /// Spaces where query had no close matches
    pub empty_spaces: EmbedderMask,
}

#[derive(Clone, Debug)]
pub struct SearchMatch {
    /// The matched teleological array
    pub array: TeleologicalArray,

    /// Overall similarity score [0, 1]
    pub similarity: f32,

    /// Per-embedder similarity scores (apples-to-apples)
    pub embedder_scores: Option<[f32; 13]>,

    /// Which spaces discovered this candidate
    pub discovered_via: EmbedderMask,

    /// Cross-embedder correlation analysis
    pub correlations: Option<CorrelationAnalysis>,

    /// Rank in results (1-indexed)
    pub rank: usize,
}
```

## 6. Entry-Point Discovery Algorithm

### 6.1 The Core Insight

Traditional single-vector search finds items close in ONE embedding space. Our teleological arrays have 13 different spaces, each capturing different aspects:

- E1 (Semantic): What does it mean?
- E2-E4 (Temporal): When is it relevant?
- E5 (Causal): What causes/effects does it have?
- E7 (Code): What code patterns does it match?
- E13 (SPLADE): What keywords does it contain?

**Key insight**: Two memories may be highly relevant even if they're only close in ONE of these spaces. A causal relationship might be captured by E5 even when E1 (semantic) shows low similarity.

### 6.2 Hook-Enhanced Parallel Multi-Space Search

The entry-point discovery is enhanced by hooks at each stage:

```rust
/// Entry-point discovery across all embedding spaces
pub struct EntryPointDiscovery {
    /// Index for each embedding space
    indices: [Arc<dyn EmbedderIndex>; 13],

    /// Default configuration
    config: DiscoveryConfig,

    /// Hook integration
    hooks: Arc<HookRegistry>,
}

impl EntryPointDiscovery {
    /// Discover candidates by searching all 13 spaces in parallel
    ///
    /// Hook integration points:
    /// - PreSearch: Inject session context, modify thresholds
    /// - PostSpaceSearch: Learn from per-space results
    /// - PostAggregation: Filter by purpose alignment
    pub async fn discover(
        &self,
        query_array: &TeleologicalArray,
        config: &DiscoveryConfig,
        hook_context: &HookContext,
    ) -> Result<DiscoveredCandidates, SearchError> {
        // Pre-search hook: inject learned thresholds
        let enhanced_config = self.hooks.invoke(
            "PreSearch",
            PreSearchEvent { config: config.clone(), context: hook_context.clone() }
        ).await?.config;

        let active_spaces: Vec<usize> = enhanced_config.active_spaces.iter_enabled().collect();

        // PARALLEL: Search all active spaces simultaneously
        let space_searches: Vec<_> = active_spaces.iter().map(|&space_idx| {
            let index = self.indices[space_idx].clone();
            let embedding = query_array.embeddings[space_idx].clone();
            let k = enhanced_config.candidates_per_space;
            let threshold = enhanced_config.space_thresholds.map(|t| t[space_idx]);

            async move {
                let start = Instant::now();
                let mut results = index.search(&embedding, k).await?;

                // Apply per-space threshold if configured
                if let Some(thresh) = threshold {
                    results.retain(|(_, score)| *score >= thresh);
                }

                Ok::<_, SearchError>((
                    space_idx,
                    results,
                    start.elapsed().as_micros() as u64,
                ))
            }
        }).collect();

        let space_results = futures::future::try_join_all(space_searches).await?;

        // Post-space-search hook: learn from results
        for (space_idx, results, time_us) in &space_results {
            self.hooks.invoke(
                "PostSpaceSearch",
                PostSpaceSearchEvent {
                    space: *space_idx,
                    result_count: results.len(),
                    time_us: *time_us,
                }
            ).await?;
        }

        // Aggregate candidates with provenance tracking
        let mut candidate_map: HashMap<Uuid, CandidateInfo> = HashMap::new();

        for (space_idx, results, time_us) in space_results {
            for (id, score) in results {
                let entry = candidate_map.entry(id).or_insert_with(|| {
                    CandidateInfo {
                        id,
                        discovered_via: EmbedderMask::empty(),
                        best_scores: [0.0; 13],
                    }
                });

                entry.discovered_via.enable(space_idx);
                entry.best_scores[space_idx] = score;
            }
        }

        // Apply discovery strategy filtering
        let candidates = match enhanced_config.strategy {
            DiscoveryStrategy::UnionAll => {
                candidate_map.into_values().collect()
            }
            DiscoveryStrategy::QuorumN(n) => {
                candidate_map.into_values()
                    .filter(|c| c.discovered_via.count() >= n)
                    .collect()
            }
            DiscoveryStrategy::Tiered { primary_spaces, expand_threshold } => {
                let primary: Vec<_> = candidate_map.values()
                    .filter(|c| c.discovered_via.intersects(primary_spaces))
                    .cloned()
                    .collect();

                if primary.len() >= expand_threshold {
                    primary
                } else {
                    candidate_map.into_values().collect()
                }
            }
            DiscoveryStrategy::Muvera { .. } => {
                self.muvera_discover(query_array, &enhanced_config).await?
            }
        };

        // Post-aggregation hook: filter by purpose alignment
        let filtered_candidates = self.hooks.invoke(
            "PostAggregation",
            PostAggregationEvent {
                candidates: candidates.clone(),
                purpose: hook_context.active_purpose.clone(),
            }
        ).await?.candidates;

        Ok(DiscoveredCandidates {
            candidates: filtered_candidates,
            stats: discovery_stats,
        })
    }
}

#[derive(Clone, Debug)]
struct CandidateInfo {
    id: Uuid,
    discovered_via: EmbedderMask,
    best_scores: [f32; 13],
}
```

### 6.3 MUVERA Integration

For high-performance scenarios, we integrate MUVERA's fixed dimensional encoding approach:

```rust
/// MUVERA-style encoding for fast multi-vector retrieval
/// Reference: https://arxiv.org/abs/2405.19504
pub struct MuveraEncoder {
    /// Number of similarity buckets
    k_sim: usize,

    /// Projection dimension per bucket
    dim_proj: usize,

    /// Number of repetitions
    r_reps: usize,

    /// Random projection matrices per repetition
    projections: Vec<Array2<f32>>,

    /// Bucket assignment hashes
    hash_functions: Vec<RandomHash>,
}

impl MuveraEncoder {
    /// Encode a teleological array into a fixed dimensional encoding (FDE)
    pub fn encode(&self, array: &TeleologicalArray) -> Vec<f32> {
        let mut fde = vec![0.0; self.r_reps * self.k_sim * self.dim_proj];

        for rep in 0..self.r_reps {
            // For each embedding in the array
            for (space_idx, embedding) in array.embeddings.iter().enumerate() {
                // Assign to bucket
                let bucket = self.hash_functions[rep].hash(space_idx) % self.k_sim;

                // Project embedding
                let projected = self.projections[rep].dot(embedding);

                // Accumulate in FDE
                let offset = rep * self.k_sim * self.dim_proj + bucket * self.dim_proj;
                for (i, val) in projected.iter().enumerate() {
                    fde[offset + i] = fde[offset + i].max(*val);
                }
            }
        }

        fde
    }

    /// Compute approximate similarity between FDEs
    pub fn similarity(fde_a: &[f32], fde_b: &[f32]) -> f32 {
        // Inner product approximates MaxSim
        fde_a.iter().zip(fde_b.iter()).map(|(a, b)| a * b).sum::<f32>()
            / (fde_a.len() as f32)
    }
}

impl EntryPointDiscovery {
    /// MUVERA-based discovery for 8-10x speedup
    async fn muvera_discover(
        &self,
        query_array: &TeleologicalArray,
        config: &DiscoveryConfig,
    ) -> Result<Vec<CandidateInfo>, SearchError> {
        let DiscoveryStrategy::Muvera { k_sim, dim_proj, r_reps } = config.strategy else {
            unreachable!()
        };

        let encoder = MuveraEncoder::new(k_sim, dim_proj, r_reps);
        let query_fde = encoder.encode(query_array);

        // Search single MUVERA index
        let candidates = self.muvera_index.search(&query_fde, config.candidates_per_space * 3).await?;

        // Convert to CandidateInfo (discovered_via not tracked in MUVERA mode)
        Ok(candidates.into_iter().map(|(id, _score)| {
            CandidateInfo {
                id,
                discovered_via: EmbedderMask::all(), // MUVERA considers all spaces
                best_scores: [0.0; 13], // Scores computed in stage 2
            }
        }).collect())
    }
}
```

## 7. Full Teleological Array Comparison

### 7.1 Per-Embedder Similarity Computation

After discovery, we fetch full arrays and compute proper per-embedder similarities:

```rust
/// Compute similarities between two teleological arrays
pub struct TeleologicalComparator {
    /// Similarity functions per embedder type
    similarity_fns: [Box<dyn SimilarityFn>; 13],
}

impl TeleologicalComparator {
    pub fn new() -> Self {
        Self {
            similarity_fns: [
                // E1: Semantic (1024D) - cosine similarity
                Box::new(CosineSimilarity),

                // E2: Temporal Recent (512D) - cosine with recency decay
                Box::new(TemporalRecentSimilarity { decay_rate: 0.1 }),

                // E3: Temporal Periodic (512D) - cosine
                Box::new(CosineSimilarity),

                // E4: Temporal Positional (512D) - cosine
                Box::new(CosineSimilarity),

                // E5: Causal (768D) - ASYMMETRIC similarity
                Box::new(AsymmetricCausalSimilarity),

                // E6: Sparse (~30K) - sparse dot product
                Box::new(SparseSimilarity),

                // E7: Code (1536D) - cosine
                Box::new(CosineSimilarity),

                // E8: Graph (384D) - cosine
                Box::new(CosineSimilarity),

                // E9: HDC (binary) - Hamming similarity
                Box::new(HammingSimilarity),

                // E10: Multimodal (768D) - cosine
                Box::new(CosineSimilarity),

                // E11: Entity/TransE (384D) - TransE distance
                Box::new(TransESimilarity),

                // E12: Late Interaction (128D/token) - MaxSim
                Box::new(MaxSimSimilarity),

                // E13: SPLADE (~30K sparse) - sparse dot product
                Box::new(SparseSimilarity),
            ],
        }
    }

    /// Compute per-embedder similarities (apples-to-apples)
    pub fn compute_similarities(
        &self,
        query: &TeleologicalArray,
        candidate: &TeleologicalArray,
    ) -> [f32; 13] {
        let mut scores = [0.0f32; 13];

        for i in 0..13 {
            scores[i] = self.similarity_fns[i].compute(
                &query.embeddings[i],
                &candidate.embeddings[i],
            );
        }

        scores
    }
}

/// Asymmetric similarity for causal embeddings (E5)
/// A causes B != B causes A
pub struct AsymmetricCausalSimilarity;

impl SimilarityFn for AsymmetricCausalSimilarity {
    fn compute(&self, query: &EmbedderOutput, candidate: &EmbedderOutput) -> f32 {
        let EmbedderOutput::Causal { embedding, direction } = query else {
            return 0.0;
        };
        let EmbedderOutput::Causal { embedding: cand_emb, direction: cand_dir } = candidate else {
            return 0.0;
        };

        let base_sim = cosine_similarity(embedding, cand_emb);

        // Direction-aware adjustment
        match (direction, cand_dir) {
            (CausalDirection::Cause, CausalDirection::Effect) => base_sim * 0.8,
            (CausalDirection::Effect, CausalDirection::Cause) => base_sim * 0.8,
            _ => base_sim, // Same direction or bidirectional
        }
    }
}

/// MaxSim for late interaction (E12)
/// For each query token, find max similarity to any doc token
pub struct MaxSimSimilarity;

impl SimilarityFn for MaxSimSimilarity {
    fn compute(&self, query: &EmbedderOutput, candidate: &EmbedderOutput) -> f32 {
        let EmbedderOutput::LateInteraction { token_embeddings: q_tokens } = query else {
            return 0.0;
        };
        let EmbedderOutput::LateInteraction { token_embeddings: c_tokens } = candidate else {
            return 0.0;
        };

        // MaxSim: for each query token, find max similarity to any candidate token
        let mut total = 0.0;
        for q_tok in q_tokens {
            let max_sim = c_tokens.iter()
                .map(|c_tok| cosine_similarity(q_tok, c_tok))
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);
            total += max_sim;
        }

        total / q_tokens.len() as f32
    }
}
```

### 7.2 Full Comparison Pipeline

```rust
/// Full teleological array comparison after discovery
pub struct FullArrayComparison {
    comparator: TeleologicalComparator,
    storage: Arc<dyn TeleologicalStorage>,
}

impl FullArrayComparison {
    /// Compare query to all discovered candidates
    pub async fn compare_candidates(
        &self,
        query: &TeleologicalArray,
        candidates: &[CandidateInfo],
        matrix: &SearchMatrix,
    ) -> Result<Vec<ScoredCandidate>, SearchError> {
        // Fetch all candidate arrays in parallel
        let fetch_futures: Vec<_> = candidates.iter()
            .map(|c| self.storage.fetch(c.id))
            .collect();

        let arrays = futures::future::try_join_all(fetch_futures).await?;

        // Compare each candidate
        let mut scored: Vec<ScoredCandidate> = arrays.into_iter()
            .zip(candidates.iter())
            .map(|(array, info)| {
                // Compute per-embedder similarities
                let embedder_scores = self.comparator.compute_similarities(query, &array);

                // Apply search matrix to get final score
                let final_score = matrix.aggregate(&embedder_scores);

                ScoredCandidate {
                    array,
                    embedder_scores,
                    final_score,
                    discovered_via: info.discovered_via,
                }
            })
            .collect();

        // Sort by final score
        scored.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());

        Ok(scored)
    }
}
```

## 8. Search Matrices

### 8.1 Matrix Structure

A search matrix defines how to weight and combine per-embedder similarities:

```rust
/// 13x13 search matrix for configurable similarity aggregation
#[derive(Clone, Debug)]
pub struct SearchMatrix {
    /// Matrix name for identification
    pub name: String,

    /// 13x13 weight matrix
    /// - Diagonal [i][i]: weight for embedder i's similarity
    /// - Off-diagonal [i][j]: cross-embedder correlation weight
    pub weights: [[f32; 13]; 13],

    /// Whether to use off-diagonal (cross-embedder) weights
    pub use_correlations: bool,

    /// Aggregation function
    pub aggregation: AggregationType,

    /// Optional per-embedder thresholds
    pub thresholds: Option<[f32; 13]>,
}

#[derive(Clone, Copy, Debug)]
pub enum AggregationType {
    /// Weighted sum: Σ w_i * score_i
    WeightedSum,

    /// Weighted geometric mean: (Π score_i^w_i)
    WeightedGeometric,

    /// Reciprocal Rank Fusion: Σ 1/(k + rank_i)
    RRF { k: f32 },

    /// Maximum across weighted scores
    WeightedMax,

    /// Minimum across weighted scores (for AND-like behavior)
    WeightedMin,
}

impl SearchMatrix {
    /// Aggregate per-embedder scores using this matrix
    pub fn aggregate(&self, scores: &[f32; 13]) -> f32 {
        match self.aggregation {
            AggregationType::WeightedSum => {
                let mut total = 0.0;

                // Diagonal: direct embedder contributions
                for i in 0..13 {
                    total += self.weights[i][i] * scores[i];
                }

                // Off-diagonal: cross-embedder correlations
                if self.use_correlations {
                    for i in 0..13 {
                        for j in (i + 1)..13 {
                            let cross = (scores[i] * scores[j]).sqrt();
                            total += self.weights[i][j] * cross;
                        }
                    }
                }

                total
            }

            AggregationType::WeightedGeometric => {
                let mut product = 1.0f32;
                let mut weight_sum = 0.0f32;

                for i in 0..13 {
                    if self.weights[i][i] > 0.0 {
                        product *= scores[i].powf(self.weights[i][i]);
                        weight_sum += self.weights[i][i];
                    }
                }

                product.powf(1.0 / weight_sum)
            }

            AggregationType::RRF { k } => {
                // Convert scores to ranks, then apply RRF
                let mut indexed: Vec<(usize, f32)> = scores.iter()
                    .enumerate()
                    .map(|(i, &s)| (i, s))
                    .collect();
                indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                let mut rrf = 0.0;
                for (rank, (embedder_idx, _score)) in indexed.iter().enumerate() {
                    rrf += self.weights[*embedder_idx][*embedder_idx] / (k + rank as f32 + 1.0);
                }

                rrf
            }

            AggregationType::WeightedMax => {
                (0..13)
                    .map(|i| self.weights[i][i] * scores[i])
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0)
            }

            AggregationType::WeightedMin => {
                (0..13)
                    .filter(|&i| self.weights[i][i] > 0.0)
                    .map(|i| self.weights[i][i] * scores[i])
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0)
            }
        }
    }
}

impl SearchMatrix {
    /// Identity matrix - equal weight on all embedders
    pub fn identity() -> Self {
        let mut weights = [[0.0; 13]; 13];
        for i in 0..13 {
            weights[i][i] = 1.0 / 13.0;
        }

        Self {
            name: "identity".into(),
            weights,
            use_correlations: false,
            aggregation: AggregationType::WeightedSum,
            thresholds: None,
        }
    }
}
```

### 8.2 Predefined Search Matrices

```rust
/// Library of predefined search matrices
pub struct SearchMatrixLibrary;

impl SearchMatrixLibrary {
    /// Identity matrix - pure apples-to-apples, equal weights
    pub fn identity() -> SearchMatrix {
        SearchMatrix::identity()
    }

    /// Semantic dominance - semantic similarity is primary signal
    pub fn semantic_dominant() -> SearchMatrix {
        let mut m = SearchMatrix::identity();
        m.weights[0][0] = 0.50; // 50% on E1 semantic
        let rest = 0.50 / 12.0;
        for i in 1..13 {
            m.weights[i][i] = rest;
        }
        m.name = "semantic_dominant".into();
        m
    }

    /// Temporal awareness - time patterns matter more
    pub fn temporal_aware() -> SearchMatrix {
        let mut m = SearchMatrix::identity();
        m.weights[1][1] = 0.25; // E2: Temporal Recent
        m.weights[2][2] = 0.15; // E3: Temporal Periodic
        m.weights[3][3] = 0.10; // E4: Temporal Positional
        m.weights[0][0] = 0.20; // E1: Semantic
        let rest = 0.30 / 9.0;
        for i in [4, 5, 6, 7, 8, 9, 10, 11, 12] {
            m.weights[i][i] = rest;
        }
        m.name = "temporal_aware".into();
        m
    }

    /// Knowledge graph - entities and causality
    pub fn knowledge_graph() -> SearchMatrix {
        let mut m = SearchMatrix::identity();
        m.weights[4][4] = 0.30; // E5: Causal
        m.weights[10][10] = 0.25; // E11: Entity/TransE
        m.weights[0][0] = 0.20; // E1: Semantic
        let rest = 0.25 / 10.0;
        for i in [1, 2, 3, 5, 6, 7, 8, 9, 11, 12] {
            m.weights[i][i] = rest;
        }
        m.name = "knowledge_graph".into();
        m
    }

    /// Code-focused search
    pub fn code_focused() -> SearchMatrix {
        let mut m = SearchMatrix::identity();
        m.weights[6][6] = 0.40; // E7: Code (AST)
        m.weights[0][0] = 0.25; // E1: Semantic
        m.weights[11][11] = 0.15; // E12: Late Interaction (precision)
        let rest = 0.20 / 10.0;
        for i in [1, 2, 3, 4, 5, 7, 8, 9, 10, 12] {
            m.weights[i][i] = rest;
        }
        m.name = "code_focused".into();
        m
    }

    /// Precision retrieval - lexical exactness with late interaction
    pub fn precision_retrieval() -> SearchMatrix {
        let mut m = SearchMatrix::identity();
        m.weights[5][5] = 0.25; // E6: Sparse
        m.weights[12][12] = 0.25; // E13: SPLADE
        m.weights[11][11] = 0.25; // E12: Late Interaction
        m.weights[0][0] = 0.15; // E1: Semantic
        let rest = 0.10 / 9.0;
        for i in [1, 2, 3, 4, 6, 7, 8, 9, 10] {
            m.weights[i][i] = rest;
        }
        m.name = "precision_retrieval".into();
        m.aggregation = AggregationType::RRF { k: 60.0 };
        m
    }

    /// Correlation-aware - enables off-diagonal weights
    pub fn correlation_aware() -> SearchMatrix {
        let mut m = SearchMatrix::identity();

        // Start with balanced diagonal
        for i in 0..13 {
            m.weights[i][i] = 0.06; // ~78% on diagonal
        }

        // Add meaningful cross-embedder correlations
        // Semantic-Contextual correlation
        m.weights[0][7] = 0.02; // E1-E8
        m.weights[7][0] = 0.02;

        // Temporal correlations
        m.weights[1][2] = 0.02; // E2-E3
        m.weights[2][1] = 0.02;
        m.weights[2][3] = 0.01; // E3-E4
        m.weights[3][2] = 0.01;

        // Entity-Causal correlation
        m.weights[4][10] = 0.03; // E5-E11
        m.weights[10][4] = 0.03;

        // Sparse correlations
        m.weights[5][12] = 0.02; // E6-E13
        m.weights[12][5] = 0.02;

        m.name = "correlation_aware".into();
        m.use_correlations = true;
        m
    }

    /// Entry-point optimized - higher weight on discovery spaces
    pub fn entry_point_optimized() -> SearchMatrix {
        let mut m = SearchMatrix::identity();

        // Boost spaces that are good entry points
        m.weights[0][0] = 0.20; // E1: Semantic (general)
        m.weights[5][5] = 0.15; // E6: Sparse (keywords)
        m.weights[12][12] = 0.15; // E13: SPLADE (keywords)
        m.weights[4][4] = 0.12; // E5: Causal (relationships)
        m.weights[6][6] = 0.12; // E7: Code (structure)
        m.weights[11][11] = 0.10; // E12: Late Interaction (precision)

        // Lower weight on supporting spaces
        let rest = 0.16 / 7.0;
        for i in [1, 2, 3, 7, 8, 9, 10] {
            m.weights[i][i] = rest;
        }

        m.name = "entry_point_optimized".into();
        m
    }

    /// Recall-maximizing - use RRF aggregation for best recall
    pub fn recall_maximizing() -> SearchMatrix {
        let mut m = SearchMatrix::identity();
        m.aggregation = AggregationType::RRF { k: 60.0 };
        m.name = "recall_maximizing".into();
        m
    }

    /// Precision-maximizing - require high scores across multiple spaces
    pub fn precision_maximizing() -> SearchMatrix {
        let mut m = SearchMatrix::identity();
        m.aggregation = AggregationType::WeightedGeometric;
        m.name = "precision_maximizing".into();
        m
    }
}
```

### 8.3 Use-Case Matrix Selection

| Use Case | Recommended Matrix | Discovery Strategy | Why |
|----------|-------------------|-------------------|-----|
| General semantic search | `semantic_dominant` | UnionAll | Broad semantic matching |
| Finding recent context | `temporal_aware` | UnionAll | Time-sensitive |
| Causal reasoning | `knowledge_graph` | QuorumN(2) | Need entity+causal |
| Code search | `code_focused` | Tiered(E7, E1) | Start with code structure |
| Exact phrase matching | `precision_retrieval` | UnionAll | Lexical precision |
| Cross-domain discovery | `correlation_aware` | UnionAll | Find hidden connections |
| High recall (RAG) | `recall_maximizing` | UnionAll | Don't miss anything |
| High precision (answers) | `precision_maximizing` | QuorumN(3) | Confident matches only |
| Performance-critical | `entry_point_optimized` | Muvera | 8x speedup |

## 9. Search Strategies by Complexity

### 9.1 Single Embedder Search (Fastest)

When you only need to search one dimension:

```rust
/// Single embedder search strategy
pub struct SingleEmbedderSearch {
    /// Which embedder to use
    embedder: Embedder,

    /// Index for this embedder
    index: Arc<dyn EmbedderIndex>,
}

impl SingleEmbedderSearch {
    pub async fn search(
        &self,
        query_embedding: &EmbedderOutput,
        top_k: usize,
    ) -> Result<Vec<(Uuid, f32)>, SearchError> {
        validate_embedder_output(query_embedding, self.embedder)?;
        self.index.search(query_embedding, top_k).await
    }
}

// Use cases:
// - Pure semantic search: E1 (Semantic)
// - Entity lookup: E11 (Entity/TransE)
// - Keyword search: E6 or E13 (SPLADE)
// - Recent memories: E2 (Temporal Recent)
// - Code patterns: E7 (Code/AST)
```

### 9.2 Embedder Group Search (Fast)

Search a predefined subset of embedders:

```rust
/// Group-based search combining specific embedders
pub struct EmbedderGroupSearch {
    /// Which group to use
    group: EmbedderGroup,

    /// Indices for embedders in the group
    indices: Vec<Arc<dyn EmbedderIndex>>,
}

impl EmbedderGroupSearch {
    pub async fn search(
        &self,
        query_array: &TeleologicalArray,
        top_k: usize,
    ) -> Result<Vec<SearchMatch>, SearchError> {
        let active_embedders = self.group.to_mask().iter_enabled().collect::<Vec<_>>();

        // Parallel search across group
        let searches: Vec<_> = active_embedders.iter().map(|&idx| {
            let index = self.indices[idx].clone();
            let embedding = query_array.embeddings[idx].clone();
            async move { index.search(&embedding, top_k * 2).await }
        }).collect();

        let results = futures::future::try_join_all(searches).await?;

        // Union and score with equal weights
        self.aggregate_with_equal_weights(results, active_embedders.len(), top_k).await
    }
}

// Predefined groups:
// - Temporal: E2 + E3 + E4
// - Relational: E5 + E11 (Causal + Entity)
// - Lexical: E6 + E12 + E13 (Sparse + Late + SPLADE)
// - Multimodal: E1 + E10 (Semantic + Multimodal)
```

### 9.3 Full Entry-Point Discovery (Comprehensive)

The complete multi-space search:

```rust
/// Full entry-point discovery with configurable comparison
pub struct FullTeleologicalSearch {
    discovery: EntryPointDiscovery,
    comparison: FullArrayComparison,
}

impl TeleologicalSearchEngine for FullTeleologicalSearch {
    async fn search(&self, query: SearchQuery) -> Result<SearchResults, SearchError> {
        let start = Instant::now();

        // Stage 1: Entry-point discovery across all spaces
        let discovered = self.discovery.discover(
            &query.query_array,
            &query.discovery,
        ).await?;

        // Stage 2: Full teleological comparison
        let matrix = query.comparison.to_matrix();
        let mut scored = self.comparison.compare_candidates(
            &query.query_array,
            &discovered.candidates,
            &matrix,
        ).await?;

        // Stage 3: Filtering and ranking
        if let Some(min_sim) = query.min_similarity {
            scored.retain(|c| c.final_score >= min_sim);
        }

        scored.truncate(query.top_k);

        // Build results
        let matches = scored.into_iter().enumerate().map(|(i, candidate)| {
            SearchMatch {
                array: candidate.array,
                similarity: candidate.final_score,
                embedder_scores: if query.include_breakdown {
                    Some(candidate.embedder_scores)
                } else {
                    None
                },
                discovered_via: candidate.discovered_via,
                correlations: if query.analyze_correlations {
                    Some(CorrelationAnalysis::compute(
                        &query.query_array,
                        &candidate.array,
                        &candidate.embedder_scores,
                    ))
                } else {
                    None
                },
                rank: i + 1,
            }
        }).collect();

        Ok(SearchResults {
            matches,
            query_time_us: start.elapsed().as_micros() as u64,
            candidates_discovered: discovered.candidates.len(),
            candidates_evaluated: discovered.candidates.len(),
            discovery_stats: discovered.stats,
            metadata: SearchMetadata::default(),
        })
    }
}
```

## 10. Correlation Analysis

```rust
/// Analysis of cross-embedder correlation patterns
#[derive(Clone, Debug)]
pub struct CorrelationAnalysis {
    /// 13x13 correlation matrix between embedder similarities
    pub correlation_matrix: [[f32; 13]; 13],

    /// Notable correlation patterns detected
    pub patterns: Vec<CorrelationPattern>,

    /// Overall correlation coherence score
    pub coherence: f32,

    /// Dominant embedder (highest individual contribution)
    pub dominant_embedder: Embedder,

    /// Embedders with anomalous scores (outliers)
    pub outliers: Vec<(Embedder, OutlierType)>,

    /// Which spaces were aligned (high correlation)
    pub aligned_spaces: Vec<(Embedder, Embedder)>,

    /// Which spaces were decorrelated (potential insight)
    pub decorrelated_spaces: Vec<(Embedder, Embedder)>,
}

#[derive(Clone, Debug)]
pub struct CorrelationPattern {
    pub pattern_type: PatternType,
    pub embedders: Vec<Embedder>,
    pub strength: f32,
    pub description: String,
}

#[derive(Clone, Debug)]
pub enum PatternType {
    /// High correlation between embedders
    HighCorrelation,
    /// Surprising decorrelation
    Decorrelated,
    /// One embedder dominates
    SingleDominant,
    /// Multiple embedders agree strongly
    ConsensusHigh,
    /// Embedders disagree
    ConsensusPoor,
    /// Temporal-semantic alignment
    TemporalSemanticAlign,
    /// Entity-causal chain detected
    EntityCausalChain,
    /// Code-semantic mismatch (might indicate technical debt)
    CodeSemanticMismatch,
}

impl CorrelationAnalysis {
    pub fn compute(
        query: &TeleologicalArray,
        candidate: &TeleologicalArray,
        embedder_scores: &[f32; 13],
    ) -> Self {
        // Compute correlation matrix
        let mut correlation_matrix = [[0.0; 13]; 13];
        for i in 0..13 {
            for j in 0..13 {
                correlation_matrix[i][j] = embedder_scores[i] * embedder_scores[j];
            }
        }

        // Detect patterns
        let patterns = Self::detect_patterns(&correlation_matrix, embedder_scores);

        // Compute coherence (how aligned are the embedders)
        let mean: f32 = embedder_scores.iter().sum::<f32>() / 13.0;
        let variance: f32 = embedder_scores.iter()
            .map(|&s| (s - mean).powi(2))
            .sum::<f32>() / 13.0;
        let coherence = 1.0 - variance.sqrt();

        // Find dominant embedder
        let (dominant_idx, _) = embedder_scores.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        // Detect outliers
        let std_dev = variance.sqrt();
        let outliers: Vec<_> = embedder_scores.iter()
            .enumerate()
            .filter_map(|(i, &s)| {
                if s > mean + 2.0 * std_dev {
                    Some((Embedder::all()[i], OutlierType::PositiveOutlier))
                } else if s < mean - 2.0 * std_dev {
                    Some((Embedder::all()[i], OutlierType::NegativeOutlier))
                } else {
                    None
                }
            })
            .collect();

        // Find aligned and decorrelated spaces
        let mut aligned_spaces = Vec::new();
        let mut decorrelated_spaces = Vec::new();

        for i in 0..13 {
            for j in (i + 1)..13 {
                let diff = (embedder_scores[i] - embedder_scores[j]).abs();
                if diff < 0.1 && embedder_scores[i] > 0.7 {
                    aligned_spaces.push((Embedder::all()[i], Embedder::all()[j]));
                } else if diff > 0.5 {
                    decorrelated_spaces.push((Embedder::all()[i], Embedder::all()[j]));
                }
            }
        }

        Self {
            correlation_matrix,
            patterns,
            coherence,
            dominant_embedder: Embedder::all()[dominant_idx],
            outliers,
            aligned_spaces,
            decorrelated_spaces,
        }
    }
}
```

## 11. Query Builder

```rust
/// Fluent builder for search queries
pub struct SearchQueryBuilder {
    query_array: Option<TeleologicalArray>,
    comparison: Option<ComparisonType>,
    discovery: DiscoveryConfig,
    top_k: usize,
    filter: Option<SearchFilter>,
    min_similarity: Option<f32>,
    include_breakdown: bool,
    analyze_correlations: bool,
}

impl SearchQueryBuilder {
    pub fn new() -> Self {
        Self {
            query_array: None,
            comparison: None,
            discovery: DiscoveryConfig::default(),
            top_k: 10,
            filter: None,
            min_similarity: None,
            include_breakdown: false,
            analyze_correlations: false,
        }
    }

    /// Set the query array
    pub fn query(mut self, array: TeleologicalArray) -> Self {
        self.query_array = Some(array);
        self
    }

    /// Use a preset search matrix
    pub fn preset(mut self, preset: &str) -> Self {
        let matrix = match preset {
            "semantic" => SearchMatrixLibrary::semantic_dominant(),
            "temporal" => SearchMatrixLibrary::temporal_aware(),
            "knowledge" => SearchMatrixLibrary::knowledge_graph(),
            "code" => SearchMatrixLibrary::code_focused(),
            "precision" => SearchMatrixLibrary::precision_retrieval(),
            "correlation" => SearchMatrixLibrary::correlation_aware(),
            "entry_point" => SearchMatrixLibrary::entry_point_optimized(),
            "recall" => SearchMatrixLibrary::recall_maximizing(),
            "high_precision" => SearchMatrixLibrary::precision_maximizing(),
            _ => SearchMatrixLibrary::identity(),
        };
        self.comparison = Some(ComparisonType::MatrixStrategy(matrix));
        self
    }

    /// Use custom search matrix
    pub fn matrix(mut self, matrix: SearchMatrix) -> Self {
        self.comparison = Some(ComparisonType::MatrixStrategy(matrix));
        self
    }

    /// Configure entry-point discovery
    pub fn discovery(mut self, config: DiscoveryConfig) -> Self {
        self.discovery = config;
        self
    }

    /// Use MUVERA for fast discovery
    pub fn muvera(mut self, k_sim: usize, dim_proj: usize, r_reps: usize) -> Self {
        self.discovery.strategy = DiscoveryStrategy::Muvera { k_sim, dim_proj, r_reps };
        self
    }

    /// Require matches in at least N spaces
    pub fn quorum(mut self, n: usize) -> Self {
        self.discovery.strategy = DiscoveryStrategy::QuorumN(n);
        self
    }

    /// Set candidates per space for discovery
    pub fn candidates_per_space(mut self, k: usize) -> Self {
        self.discovery.candidates_per_space = k;
        self
    }

    /// Limit to specific embedding spaces
    pub fn only_spaces(mut self, mask: EmbedderMask) -> Self {
        self.discovery.active_spaces = mask;
        self
    }

    /// Set maximum results
    pub fn top_k(mut self, k: usize) -> Self {
        self.top_k = k;
        self
    }

    /// Set minimum similarity threshold
    pub fn min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = Some(threshold);
        self
    }

    /// Include per-embedder breakdown
    pub fn with_breakdown(mut self) -> Self {
        self.include_breakdown = true;
        self
    }

    /// Enable correlation analysis
    pub fn with_correlations(mut self) -> Self {
        self.analyze_correlations = true;
        self
    }

    /// Add time range filter
    pub fn created_after(mut self, time: DateTime<Utc>) -> Self {
        self.filter.get_or_insert_with(SearchFilter::default)
            .created_after = Some(time);
        self
    }

    /// Build the query
    pub fn build(self) -> Result<SearchQuery, QueryError> {
        let query_array = self.query_array.ok_or(QueryError::MissingQueryArray)?;
        let comparison = self.comparison.unwrap_or_else(|| {
            ComparisonType::MatrixStrategy(SearchMatrixLibrary::identity())
        });

        Ok(SearchQuery {
            query_array,
            comparison,
            top_k: self.top_k,
            discovery: self.discovery,
            filter: self.filter,
            min_similarity: self.min_similarity,
            include_breakdown: self.include_breakdown,
            analyze_correlations: self.analyze_correlations,
        })
    }
}
```

## 12. Usage Examples

```rust
// Example 1: Basic semantic search with entry-point discovery
let results = engine.search(
    SearchQueryBuilder::new()
        .query(my_array)
        .preset("semantic")
        .top_k(20)
        .build()?
).await?;

// Example 2: High-precision search requiring matches in 3+ spaces
let results = engine.search(
    SearchQueryBuilder::new()
        .query(my_array)
        .preset("high_precision")
        .quorum(3)
        .top_k(10)
        .with_breakdown()
        .build()?
).await?;

// Example 3: Fast MUVERA search for performance-critical path
let results = engine.search(
    SearchQueryBuilder::new()
        .query(my_array)
        .preset("entry_point")
        .muvera(64, 32, 20) // ~8x speedup
        .top_k(50)
        .build()?
).await?;

// Example 4: Code search with correlation analysis
let results = engine.search(
    SearchQueryBuilder::new()
        .query(my_array)
        .preset("code")
        .only_spaces(EmbedderMask::from([Embedder::Code, Embedder::Semantic, Embedder::LateInteraction]))
        .top_k(15)
        .with_correlations()
        .build()?
).await?;

// Example 5: Knowledge graph traversal
let results = engine.search(
    SearchQueryBuilder::new()
        .query(my_array)
        .preset("knowledge")
        .candidates_per_space(200) // Higher recall for graph queries
        .min_similarity(0.6)
        .with_breakdown()
        .with_correlations()
        .build()?
).await?;
```

## 13. Performance Characteristics

| Operation | Target Latency | Notes |
|-----------|---------------|-------|
| Single-space search (1M memories) | <2ms | HNSW index |
| 13-space parallel discovery | <20ms | All spaces in parallel |
| MUVERA FDE encoding | <1ms | Single-vector encoding |
| MUVERA search | <3ms | 8x faster than full discovery |
| Full array fetch | <5ms | Batch fetch from storage |
| Per-embedder comparison (13x) | <1ms | Parallel computation |
| Matrix aggregation | <0.1ms | Simple arithmetic |
| Correlation analysis | <2ms | Optional |
| **Total (full pipeline)** | **<30ms** | **1M memories** |

### 13.1 Scaling Considerations

- **Candidates per space**: Higher values improve recall but increase comparison cost
- **MUVERA FDE size**: Larger FDEs (more reps/clusters) improve accuracy but use more memory
- **Quorum N**: Higher N reduces candidates but may miss relevant results
- **Active spaces**: Limiting to relevant spaces improves speed

## 14. References

- [MUVERA: Multi-Vector Retrieval via Fixed Dimensional Encodings](https://arxiv.org/abs/2405.19504) - NeurIPS 2024
- [Google Research: Making multi-vector retrieval as fast as single-vector search](https://research.google/blog/muvera-making-multi-vector-retrieval-as-fast-as-single-vector-search/)
- [ParlayANN: Scalable Parallel Graph-Based ANN Search](https://dl.acm.org/doi/10.1145/3627535.3638475)
- [CMU: New Techniques for Parallelism in Nearest Neighbor Search](http://reports-archive.adm.cs.cmu.edu/anon/anon/2025/CMU-CS-25-100.pdf)
- [Weaviate MUVERA Implementation](https://weaviate.io/blog/muvera)
- [Qdrant MUVERA Embeddings](https://qdrant.tech/articles/muvera-embeddings/)

## 15. MCP Integration

See [08-MCP-TOOLS.md](./08-MCP-TOOLS.md) for the updated MCP tool specifications including:
- `search_teleological` - Full entry-point discovery search
- `search_single_space` - Single embedder search
- `search_muvera` - MUVERA-accelerated search
- `configure_search_matrix` - Create custom search matrices
