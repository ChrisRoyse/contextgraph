# Embedder Navigation Gap Analysis & Recommendations

## MCP Tool Audit: Can AI Agents Fully Navigate the 13-Embedder Dataset?

**Date**: 2026-02-07
**Scope**: All 53 MCP tools in `context-graph-mcp`
**Question**: Do the tools give AI agents enough control over embedder weights and search strategies to truly navigate data embedded by the 13-embedder system?

**Short Answer**: No. The current tools provide good domain-specific access to individual embedders but lack the ability to dynamically compose arbitrary embedder combinations with custom weights. AI agents are restricted to ~17 predefined weight profiles and cannot create their own blends at runtime.

---

## Part 1: What AI Agents CAN Do Today

### 1.1 Single-Embedder Access (Good)

| Tool | Embedder | What It Does |
|------|----------|-------------|
| `search_by_embedder` | Any E1-E13 | Search using any single embedder as primary. Has `includeAllScores` flag to see all 13 scores. |
| `search_recent` | E2 | Temporal recency with `temporalWeight`, `decayFunction`, `temporalScale` |
| `search_periodic` | E3 | Periodic patterns with `targetHour`, `targetDayOfWeek`, `periodicWeight` |
| `get_conversation_context` | E4 | Sequence-based with `direction`, `windowSize` |
| `get_session_timeline` | E4 | Ordered session timeline |
| `traverse_memory_chain` | E4 | Multi-hop sequence traversal from anchor |
| `search_causes` / `search_effects` | E5 | Directional causal search |
| `search_causal_relationships` | E5 | Causal similarity search |
| `get_causal_chain` | E5 | Causal chain traversal |
| `search_by_keywords` | E6 | Keyword search with `blendWithSemantic`, `useSpladeExpansion` |
| `search_code` | E7 | Code search with `blendWithSemantic` |
| `search_robust` | E9 | Noise-robust with E9/E1 blind-spot thresholds |
| `search_by_intent` | E10 | Intent-based with `weightProfile` selection |
| `extract_entities` / `search_by_entities` | E11 | Entity extraction and search |
| `get_entity_graph` | E11 | Entity relationship graph |
| `get_memory_neighbors` | Any (by index 0-12) | K-NN in a single embedder space |

**Verdict**: Individual embedder access is comprehensive. Every embedder has at least one dedicated tool.

### 1.2 Multi-Embedder Fusion (Limited)

| Tool | What It Does | Limitation |
|------|-------------|-----------|
| `search_graph` | Multi-space search with `weightProfile` | Only accepts named profiles from a fixed enum of ~10 |
| `get_unified_neighbors` | RRF across all 13 with `weight_profile` | Only accepts named profiles from a fixed enum of ~8 |
| `search_by_intent` | E10 + E1 multiplicative boost | Only accepts named `weightProfile` |
| `compare_embedder_views` | Side-by-side 2-5 embedder rankings | No blending, just comparison |
| `get_embedder_clusters` | HDBSCAN clusters per embedder | Single embedder only |

### 1.3 Named Weight Profiles Available

The system has **17 predefined profiles** (from `weights/mod.rs`):

| Profile | Primary Embedder | Use Case |
|---------|-----------------|----------|
| `semantic_search` | E1 (0.33) | General queries |
| `causal_reasoning` | E5 (0.45) | "Why" questions |
| `code_search` | E7 (0.40) | Programming queries |
| `fact_checking` | E11 (0.40) | Entity/fact queries |
| `intent_search` | E1 (0.55) + E10 boost | Goal/purpose queries |
| `intent_enhanced` | E1 (0.55) + strong E10 boost | Stronger intent focus |
| `graph_reasoning` | E8 (0.40) | Structural/connectivity |
| `temporal_navigation` | E2/E3/E4 (0.22 each) | Time-based queries |
| `sequence_navigation` | E4 (0.55) | Sequence traversal |
| `conversation_history` | E4 (0.35) + E1 (0.30) | Contextual recall |
| `category_weighted` | Constitution-compliant | Balanced by category |
| `typo_tolerant` | E9 (0.15) + E1 (0.30) | Noisy queries |
| `pipeline_stage1_recall` | E13 (0.25) + E6 (0.25) | Stage 1 recall |
| `pipeline_stage2_scoring` | E1 (0.50) | Dense scoring |
| `pipeline_full` | E1 (0.40) | Full pipeline |
| `balanced` | All equal (0.077) | Testing/comparison |

**Not exposed to MCP**: `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`, `sequence_navigation`, `conversation_history`, `temporal_navigation` - these profiles exist in `weights/mod.rs` but are NOT listed in the `search_graph` or `get_unified_neighbors` enum constraints.

---

## Part 2: Critical Gaps

### Gap 1: No Custom Weight Arrays

**The core limitation.** AI agents cannot pass `[f32; 13]` custom weight arrays. They must select from named profiles.

```
WHAT THE AI WANTS TO DO:
  "Search with E4=0.4, E7=0.3, E1=0.2, E5=0.1"

WHAT THE AI CAN DO:
  "Search with weightProfile='sequence_navigation'"  (closest preset, but E4=0.55 not 0.4)
```

The backend already has `validate_weights()` which accepts and validates any `[f32; 13]` array. The infrastructure exists - it just isn't exposed via MCP.

### Gap 2: No Runtime Weight Profile Creation

AI agents cannot create, name, and reuse custom weight profiles. A code-analysis agent might want to create a "code_with_entities" profile (E7=0.35, E11=0.30, E1=0.20, E6=0.15) and reuse it across many queries in a session.

### Gap 3: Incomplete Profile Enum in MCP Tools

The `search_graph` tool only exposes ~10 of the 17 profiles:
```
Exposed:     semantic_search, causal_reasoning, code_search, fact_checking,
             temporal_navigation, category_weighted, sequence_navigation,
             conversation_history, intent_search, intent_enhanced

NOT exposed: typo_tolerant, graph_reasoning, pipeline_stage1_recall,
             pipeline_stage2_scoring, pipeline_full, balanced
```

`get_unified_neighbors` only exposes 8 profiles and is missing `temporal_navigation`, `sequence_navigation`, `conversation_history`, `typo_tolerant`, and all pipeline profiles.

### Gap 4: No Composite Multi-Embedder Search

There is no single tool that lets the AI say: "Search using E4 sequence + E7 code with these specific blend ratios." The closest options are:
- `search_graph` with a named profile (but can't customize weights)
- `search_by_embedder` (but only one embedder at a time)
- `compare_embedder_views` (side-by-side, no fusion)

### Gap 5: No Per-Embedder Score Introspection for Existing Memories

The `search_by_embedder` tool has `includeAllScores` but only on search results. There is no tool to ask: "Show me the full 13-embedder fingerprint scores for memory UUID X" without searching first.

### Gap 6: No Search Result Provenance (Which Embedder Found This?)

When `search_graph` returns results via multi-space RRF, the AI cannot see which embedders contributed most to each result's ranking. The `get_unified_neighbors` tool has `include_embedder_breakdown` which is good, but `search_graph` does not expose per-embedder contribution details.

### Gap 7: No Dynamic Post-Retrieval Boost Composition

Temporal tools (`search_recent`, `search_periodic`) apply a single temporal boost. But what if the AI wants: "Apply E2 recency decay + E3 periodic boost + E5 causal direction boost together on the same query"? There's no way to compose multiple post-retrieval boosts.

### Gap 8: No Embedder Mask / Exclusion

The AI cannot say "Search using all embedders EXCEPT E2 and E3." The `EmbedderMask` (u16 bitmask) exists in core but is not exposed via any MCP tool.

### Gap 9: No Cross-Embedder Anomaly Detection

There's no tool to ask: "Find memories where E7 (code) thinks they're very similar but E1 (semantic) thinks they're very different." This would reveal interesting structural relationships. The `search_robust` tool does this for E9 vs E1, but the concept isn't generalized.

---

## Part 3: Recommended New Tools

### Priority 1 (High Impact, Enables True Navigation)

#### Tool 1: `search_custom_weights` - Custom Weight Array Search

The most important missing tool. Let the AI pass arbitrary `[f32; 13]` weights.

```json
{
  "name": "search_custom_weights",
  "params": {
    "query": "string (required)",
    "weights": {
      "type": "object",
      "description": "Custom weights for each embedder. Must sum to ~1.0.",
      "properties": {
        "E1": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E2": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E3": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E4": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E5": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E6": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E7": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E8": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E9": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E10": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E11": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E12": { "type": "number", "min": 0, "max": 1, "default": 0 },
        "E13": { "type": "number", "min": 0, "max": 1, "default": 0 }
      }
    },
    "topK": "integer",
    "includeContent": "boolean",
    "includeEmbedderBreakdown": "boolean"
  }
}
```

**Backend impact**: Minimal. The `multi_space.rs` already accepts `[f32; 13]`. Just bypass `get_weight_profile()` and pass the array directly after `validate_weights()`.

#### Tool 2: `get_memory_fingerprint` - Full Fingerprint Introspection

Let the AI inspect a memory's full 13-embedder representation.

```json
{
  "name": "get_memory_fingerprint",
  "params": {
    "memory_id": "uuid (required)",
    "embedders": ["E1", "E5", "E7"],  // optional filter, default: all 13
    "includeVectorNorms": "boolean",
    "includeContent": "boolean"
  }
}
```

Returns per-embedder: dimension, vector norm, index presence, asymmetric variant info (e.g., E5 has cause/effect vectors). Gives the AI a full picture of how the system "sees" a memory.

#### Tool 3: `search_with_explanation` - Explainable Multi-Space Search

Wraps `search_graph` with per-result attribution showing which embedders contributed most.

```json
{
  "name": "search_with_explanation",
  "params": {
    "query": "string (required)",
    "weightProfile": "string (optional)",
    "weights": "object (optional, custom [f32;13])",
    "topK": "integer",
    "includeContent": "boolean"
  },
  "returns": {
    "results": [{
      "id": "uuid",
      "score": 0.85,
      "embedderContributions": {
        "E1": { "rank": 2, "score": 0.91, "contribution": 0.35 },
        "E7": { "rank": 1, "score": 0.94, "contribution": 0.28 },
        "E5": { "rank": 5, "score": 0.72, "contribution": 0.15 }
      },
      "dominantEmbedder": "E7",
      "agreementLevel": "high"  // how many embedders agree
    }]
  }
}
```

This is how the AI understands what the system is "thinking" about each result.

### Priority 2 (Better Navigation Control)

#### Tool 4: `create_weight_profile` - Session-Scoped Custom Profiles

Let the AI create named profiles that persist for the session.

```json
{
  "name": "create_weight_profile",
  "params": {
    "name": "string (required, e.g. 'my_code_entity_blend')",
    "weights": { "E1": 0.2, "E7": 0.35, "E11": 0.30, "E6": 0.15 },
    "description": "string (optional)",
    "sessionScoped": "boolean (default: true)"
  }
}
```

Once created, the AI can use `"weightProfile": "my_code_entity_blend"` in `search_graph`, `get_unified_neighbors`, etc. Profiles are session-scoped by default (disappear when the connection closes).

#### Tool 5: `search_cross_embedder_anomalies` - Generalized Blind-Spot Detection

Generalize `search_robust`'s E9-vs-E1 blind-spot concept to any embedder pair.

```json
{
  "name": "search_cross_embedder_anomalies",
  "params": {
    "query": "string (required)",
    "highEmbedder": "E7",   // embedder where score should be HIGH
    "lowEmbedder": "E1",    // embedder where score should be LOW
    "highThreshold": 0.7,
    "lowThreshold": 0.5,
    "topK": 10
  }
}
```

Use cases:
- E7 high + E1 low = code patterns invisible to semantic search
- E11 high + E7 low = entity relationships without code context
- E5 high + E8 low = causal chains without structural links
- E4 high + E1 low = sequential proximity without semantic similarity

#### Tool 6: `compose_post_retrieval_boosts` - Multi-Boost Composition

Let the AI apply multiple post-retrieval boosts simultaneously.

```json
{
  "name": "compose_post_retrieval_boosts",
  "params": {
    "query": "string (required)",
    "baseProfile": "semantic_search",
    "boosts": [
      { "type": "recency", "weight": 0.3, "decay": "exponential", "scale": "meso" },
      { "type": "periodic", "weight": 0.2, "targetHour": 14 },
      { "type": "causal", "direction": "cause", "weight": 0.2 },
      { "type": "sequence", "sessionOnly": true, "weight": 0.1 }
    ],
    "topK": 10
  }
}
```

This is the most powerful navigation tool: start with a semantic base, then layer on temporal, causal, and sequence boosts simultaneously. Currently the AI must choose ONE boost strategy.

### Priority 3 (Advanced Navigation Patterns)

#### Tool 7: `navigate_embedder_landscape` - Iterative Exploration

An exploration tool for understanding the "shape" of data in a specific embedder space.

```json
{
  "name": "navigate_embedder_landscape",
  "params": {
    "embedder": "E5",
    "startQuery": "string (optional)",
    "startMemory": "uuid (optional)",
    "steps": [
      { "direction": "toward", "target": "string or uuid" },
      { "direction": "orthogonal_to", "target": "string" },
      { "direction": "away_from", "target": "string" }
    ],
    "stepSize": 0.3,
    "topKPerStep": 5
  }
}
```

This lets the AI "walk" through the embedding space: "Start from this memory, move toward 'authentication', then move orthogonal to 'security' (to find auth-related things that aren't security)."

#### Tool 8: `get_embedder_agreement_map` - Multi-Embedder Agreement View

For a given memory or query, show which embedder pairs agree/disagree on what's related.

```json
{
  "name": "get_embedder_agreement_map",
  "params": {
    "memory_id": "uuid (or query string)",
    "topK": 10,
    "embedders": ["E1", "E5", "E7", "E11"]  // optional subset
  },
  "returns": {
    "agreementMatrix": {
      "E1_E5": { "overlap": 0.6, "sharedResults": 6, "uniqueToE1": 4, "uniqueToE5": 4 },
      "E1_E7": { "overlap": 0.3, "sharedResults": 3, "uniqueToE1": 7, "uniqueToE7": 7 }
    },
    "universalResults": ["uuid1", "uuid2"],   // found by ALL selected embedders
    "uniqueFinds": {
      "E5_only": ["uuid3"],  // found ONLY by E5
      "E7_only": ["uuid4"]   // found ONLY by E7
    }
  }
}
```

#### Tool 9: `adaptive_search` - Let The System Choose Weights

Instead of the AI choosing weights, let the system analyze the query and automatically select the optimal weight blend.

```json
{
  "name": "adaptive_search",
  "params": {
    "query": "string (required)",
    "topK": 10,
    "explainStrategy": true  // show why these weights were chosen
  },
  "returns": {
    "results": [...],
    "strategyExplanation": {
      "detectedIntent": "causal_code_query",
      "weightsUsed": { "E1": 0.15, "E5": 0.35, "E7": 0.30, "E6": 0.10, "E11": 0.10 },
      "reasoning": "Query contains 'why does' (causal signal) + 'function' (code signal)"
    }
  }
}
```

This uses query classification (already partially implemented via `intentMode: "auto"` and `causalDirection: "auto"`) but makes the strategy selection fully transparent to the AI.

---

## Part 4: Quick Wins (No New Tools Required)

These can be done by updating existing tool schemas:

### 4.1 Add `customWeights` Parameter to `search_graph`

Add an optional `customWeights` object alongside the existing `weightProfile` enum. When both are provided, `customWeights` takes precedence.

```json
"customWeights": {
  "type": "object",
  "description": "Custom per-embedder weights (overrides weightProfile). Must sum to ~1.0.",
  "properties": {
    "E1": { "type": "number" }, "E2": { "type": "number" },
    ... // all 13
  }
}
```

**Estimated effort**: Small. Add parsing in the search_graph handler, call `validate_weights()`, pass to multi-space search.

### 4.2 Expose All 17 Profiles in Existing Enums

The `search_graph` and `get_unified_neighbors` tool schemas should list ALL 17 profiles, not just subsets. Currently `graph_reasoning`, `typo_tolerant`, `balanced`, and pipeline profiles are defined in `weights/mod.rs` but not listed in the MCP tool enums.

**Estimated effort**: Trivial. Update the `"enum"` arrays in tool definition JSON schemas.

### 4.3 Add `includeEmbedderBreakdown` to `search_graph`

The `get_unified_neighbors` tool already has `include_embedder_breakdown`. Add the same flag to `search_graph` when using `multi_space` strategy, so the AI can see per-embedder contributions on every search.

**Estimated effort**: Small. The per-embedder scores are already computed in `multi_space.rs` during RRF - just include them in the response.

### 4.4 Add `excludeEmbedders` Parameter to Multi-Space Tools

Use the existing `EmbedderMask` infrastructure to let the AI exclude specific embedders:

```json
"excludeEmbedders": {
  "type": "array",
  "items": { "enum": ["E1","E2",...,"E13"] },
  "description": "Embedders to exclude from fusion (their weight becomes 0)"
}
```

**Estimated effort**: Small. Zero out the excluded indices in the weight array before RRF.

---

## Part 5: Navigation Strategy Patterns

These are usage patterns an AI agent could employ if the recommended tools were implemented:

### Pattern 1: Progressive Narrowing

```
1. adaptive_search("authentication bug in login") -> broad results + strategy explanation
2. search_custom_weights(same query, boost E7 to 0.5) -> focus on code matches
3. get_memory_fingerprint(best_result_id) -> see all 13 scores
4. search_cross_embedder_anomalies(highEmbedder=E5, lowEmbedder=E1) -> find causal chains E1 missed
```

### Pattern 2: Multi-Perspective Exploration

```
1. compare_embedder_views(query, [E1, E5, E7, E11]) -> see different perspectives
2. get_embedder_agreement_map(best_result_id) -> see where embedders agree/disagree
3. search_by_embedder(embedder=E5, query) -> dive into E5's unique finds
4. traverse_graph(start=unique_find, edge_type=causal_chain) -> follow the causal thread
```

### Pattern 3: Temporal + Semantic Composition

```
1. compose_post_retrieval_boosts(query, boosts=[recency + periodic + sequence])
   -> "What code was I working on at 3pm yesterday?"
   -> Combines: E1 semantic match + E2 recency decay + E3 3pm boost + E4 session position
```

### Pattern 4: Custom Profile Session

```
1. create_weight_profile("legal_entity_focus", {E1: 0.15, E11: 0.40, E5: 0.20, E6: 0.15, E8: 0.10})
2. search_graph(query="contract dispute", weightProfile="legal_entity_focus")
3. search_graph(query="liability clause", weightProfile="legal_entity_focus")
4. search_graph(query="settlement terms", weightProfile="legal_entity_focus")
   -> Consistent entity-heavy searches across all queries in this analysis
```

---

## Part 6: Implementation Priority Matrix

| Tool/Change | Impact | Effort | Priority |
|------------|--------|--------|----------|
| Add `customWeights` to `search_graph` | **Critical** | Small | **P0** |
| Expose all 17 profiles in enums | High | Trivial | **P0** |
| Add `includeEmbedderBreakdown` to `search_graph` | High | Small | **P0** |
| `get_memory_fingerprint` | High | Small | **P1** |
| `search_with_explanation` | High | Medium | **P1** |
| Add `excludeEmbedders` param | Medium | Small | **P1** |
| `create_weight_profile` | High | Medium | **P2** |
| `search_cross_embedder_anomalies` | Medium | Medium | **P2** |
| `compose_post_retrieval_boosts` | High | Large | **P2** |
| `adaptive_search` | High | Large | **P3** |
| `navigate_embedder_landscape` | Medium | Large | **P3** |
| `get_embedder_agreement_map` | Medium | Medium | **P3** |

---

## Part 7: Architecture Notes

### Why `validate_weights()` Makes Custom Weights Safe

The existing `validate_weights()` function in `weights/mod.rs` already:
1. Validates each weight is in `[0.0, 1.0]`
2. Validates the array sums to `~1.0` (tolerance 0.01)
3. Returns detailed error messages

This means accepting custom `[f32; 13]` from MCP is safe as long as validation is applied before use.

### Pipeline-Stage Embedders (E12, E13) In Custom Weights

E12 (ColBERT) and E13 (SPLADE) operate in specific pipeline stages, not in standard similarity fusion. Custom weights should:
- Allow E12/E13 weights but document that they only apply in `pipeline` strategy
- In `multi_space` strategy, silently treat E12/E13 weights as 0 and renormalize

### Temporal Embedder Constraints (E2-E4)

Per AP-60 and AP-71, temporal embedders should not participate in semantic fusion. The system should:
- Allow E2-E4 weights in custom weight arrays
- Warn (but don't reject) when E2-E4 weights are >0 in a non-temporal context
- Document that high E2-E4 weights may reduce topical relevance

### Asymmetric Embedders (E5, E8, E10)

E5 (causal), E8 (graph), and E10 (intent) have dual vectors (cause/effect, source/target, intent/context). Custom weight tools should:
- Accept an optional `direction` parameter for asymmetric embedders
- Default to the symmetric/auto behavior when direction is not specified
- Document the 1.2x/0.8x directional modifiers

---

## Part 8: Summary

The Context Graph's 13-embedder architecture is powerful but underexposed. The AI has 53 MCP tools but cannot:

1. **Compose custom embedder weight blends** (the single most impactful gap)
2. **Inspect a memory's full multi-embedder fingerprint**
3. **See which embedders contributed to each search result**
4. **Create session-scoped weight profiles for specialized analysis**
5. **Combine multiple post-retrieval boosts simultaneously**
6. **Detect cross-embedder anomalies beyond E9-vs-E1**

The P0 fixes (add `customWeights` param, expose all profiles, add embedder breakdowns) require minimal backend changes and would immediately unlock true embedder navigation. The P1-P3 tools build progressively more sophisticated navigation capabilities.

The goal: transform the AI from a consumer of predefined search strategies into a navigator that dynamically adjusts its 13-dimensional perspective based on what it's looking for.
