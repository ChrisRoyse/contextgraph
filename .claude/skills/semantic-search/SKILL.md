---
name: semantic-search
description: Search the knowledge graph using multi-space retrieval across 13 embedding spaces. Returns memories ranked by weighted similarity. Keywords - search, find, query, lookup, semantic, similar, retrieve.
allowed-tools: mcp__context-graph__search_graph,mcp__context-graph__store_memory
model: inherit
version: 2.0.0
user-invocable: true
---
# Semantic Search

Search the knowledge graph using the 13-embedding multi-space retrieval system.

## MCP Tool: search_graph

### Required Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | The search query text |

### Optional Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `topK` | integer | 10 | Results to return (1-100) |
| `minSimilarity` | number | 0.0 | Minimum similarity threshold (0.0-1.0) |
| `includeContent` | boolean | false | Include full content text in results |
| `strategy` | string | "multi_space" | Search strategy: `e1_only`, `multi_space`, `pipeline` |
| `weightProfile` | string | - | One of 14 profiles: semantic_search, causal_reasoning, code_search, fact_checking, graph_reasoning, temporal_navigation, sequence_navigation, conversation_history, category_weighted, typo_tolerant, pipeline_stage1_recall, pipeline_stage2_scoring, pipeline_full, balanced |
| `customWeights` | object | - | Per-embedder weights {"E1": 0.4, "E5": 0.3, ...}. Must sum to ~1.0. Overrides weightProfile. |
| `excludeEmbedders` | array | - | Embedders to exclude: ["E2", "E3", "E4"]. Zeroed + renormalized. |
| `includeEmbedderBreakdown` | boolean | false | Include per-embedder scores and contribution breakdown |
| `causalDirection` | string | "auto" | Causal direction: auto, cause, effect, none |
| `enableRerank` | boolean | false | Enable ColBERT E12 re-ranking |
| `sessionScope` | string | "all" | Session scope: current, all, recent |
| `lastHours` | number | - | Filter to last N hours |
| `lastDays` | number | - | Filter to last N days |
| `includeProvenance` | boolean | false | Include retrieval provenance metadata |

### Response Structure

```json
{
  "results": [
    {
      "fingerprintId": "550e8400-e29b-41d4-a716-446655440000",
      "similarity": 0.87,
      "dominantEmbedder": "E7",
      "content": "Full text here if includeContent=true"
    }
  ],
  "count": 5,
  "searchTransparency": {
    "activeEmbedders": ["E1", "E5", "E7", "E8", "E10", "E11"],
    "strategy": "multi_space",
    "weightProfile": "semantic_search"
  }
}
```

## Search Strategies

| Strategy | Description | When to Use |
|----------|-------------|-------------|
| `e1_only` | Fast E1 semantic only | Simple keyword-like queries |
| `multi_space` | E1+E5+E7+E8+E10+E11 via Weighted RRF (default) | Most queries - best overall quality |
| `pipeline` | E13 recall -> E1 dense -> E12 rerank | Maximum precision needed |

## Embedder Categories

| Category | Embedders | Weight | Purpose |
|----------|-----------|--------|---------|
| SEMANTIC | E1, E5, E6, E7, E10, E12, E13 | 1.0 | Primary meaning, causality, code, intent |
| RELATIONAL | E8, E11 | 0.5 | Graph connectivity, named entities |
| STRUCTURAL | E9 | 0.5 | Format/structure patterns |
| TEMPORAL | E2, E3, E4 | 0.0 | Post-retrieval only (not for ranking) |

### Embedder Reference

- **E1 (V_meaning)**: General semantic meaning - 1024D dense
- **E2 (V_freshness)**: Temporal recency - 512D (post-retrieval only)
- **E3 (V_periodicity)**: Time-of-day patterns - 512D (post-retrieval only)
- **E4 (V_ordering)**: Sequence position - 512D (post-retrieval only)
- **E5 (V_causality)**: Cause-effect relationships - 768D asymmetric
- **E6 (V_selectivity)**: Sparse keyword matching - ~30K sparse
- **E7 (V_correctness)**: Code patterns - 1536D dense
- **E8 (V_connectivity)**: Graph structure - 1024D asymmetric
- **E9 (V_robustness)**: Noise-robust structure - 1024D HDC
- **E10 (V_multimodality)**: Paraphrase detection - 768D asymmetric
- **E11 (V_factuality)**: Named entity knowledge - 768D KEPLER/TransE
- **E12 (V_precision)**: Exact phrase reranking - 128D/token ColBERT
- **E13 (V_keyword_precision)**: Term expansion - ~30K SPLADE sparse

## Usage Instructions

### Basic Search
```
search_graph({ "query": "authentication middleware" })
```

### Code Search (weight profile)
```
search_graph({
  "query": "error handling patterns",
  "weightProfile": "code_search",
  "includeContent": true
})
```

### Causal Search
```
search_graph({
  "query": "Why did we choose PostgreSQL?",
  "weightProfile": "causal_reasoning",
  "causalDirection": "cause",
  "includeContent": true
})
```

### Custom Weights
```
search_graph({
  "query": "authentication implementation",
  "strategy": "multi_space",
  "customWeights": {"E1": 0.3, "E7": 0.4, "E11": 0.2, "E5": 0.1},
  "includeEmbedderBreakdown": true
})
```

### Pipeline (Maximum Precision)
```
search_graph({
  "query": "exact error message text",
  "strategy": "pipeline",
  "topK": 5,
  "includeContent": true
})
```

## Edge Cases

| Condition | Response |
|-----------|----------|
| No query provided | Error: "Missing required parameter: query" |
| Empty results | "No memories match your search. Try broader terms or lower minSimilarity." |
| High minSimilarity filters everything | "No results above threshold. Consider lowering minSimilarity." |
| Empty knowledge graph | "Knowledge graph is empty. Use store_memory to add content first." |
| All results low similarity (<0.3) | Note: "Results have low relevance. Consider rephrasing." |

## Related Tools

- **store_memory**: Store new memories in the knowledge graph
- **get_memetic_status**: Check graph health and metrics
