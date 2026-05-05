---
name: cg-code-search
description: |
  Search code patterns using E7 Qodo-Embed (1536D) code embeddings.
  Use for finding functions, patterns, signatures, or code anomalies.
  Keywords: code, search, function, signature, pattern, AST, Qodo.
allowed-tools: mcp__context-graph__search_code,mcp__context-graph__search_cross_embedder_anomalies
model: inherit
version: 2.0.0
user-invocable: true
---

# Code Search

Search indexed code using E7 (V_correctness) 1536D dense code embeddings via Qodo-Embed.

## Steps

1. **Search code**: `mcp__context-graph__search_code` with query
2. **Find anomalies**: `mcp__context-graph__search_cross_embedder_anomalies` to find code that scores high on E7 but low on E1

## MCP Tool: search_code

### Required Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | Code query — describe functionality, patterns, or specific constructs |

### Optional Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `topK` | integer | 10 | Results to return (1-50) |
| `minScore` | number | 0.2 | Minimum blended score threshold (0.0-1.0) |
| `blendWithSemantic` | number | 0.4 | E7 code weight in blend (0.0-1.0). 0.0=pure E1 semantic, 1.0=pure E7 code |
| `includeContent` | boolean | false | Include full content text in results |
| `searchMode` | string | "semantic" | Strategy: `e7Only`, `pipeline`, `semantic` |
| `languageHint` | string | - | Language hint: rust, python, javascript, typescript, go, java, cpp, sql |
| `includeAstContext` | boolean | false | Include AST context (scope chain, entity type) if available |

### Search Modes

| Mode | Description | When to Use |
|------|-------------|-------------|
| `semantic` | Hybrid E1+E7 blend (default) | General code queries with semantic context |
| `e7Only` | Pure E7 code embeddings | Strictly code-pattern matching |
| `pipeline` | Full E13->E1->E7->E12 pipeline | Maximum precision code search |

### Response Structure

```json
{
  "results": [
    {
      "fingerprintId": "uuid",
      "score": 0.85,
      "detectedLanguage": "rust",
      "content": "fn example() { ... }"
    }
  ],
  "count": 5,
  "searchMode": "semantic"
}
```

## MCP Tool: search_cross_embedder_anomalies

Find memories where one embedder scores high but another scores low — reveals blind spots.

### Required Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | Search query |
| `highEmbedder` | string | Embedder expected to score HIGH (E1-E13, e.g., "E7") |
| `lowEmbedder` | string | Embedder expected to score LOW (E1-E13, e.g., "E1") |

### Optional Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `topK` | integer | 10 | Results to return (1-100) |
| `highThreshold` | number | 0.5 | Minimum score in highEmbedder (0.0-1.0) |
| `lowThreshold` | number | 0.3 | Maximum score in lowEmbedder (0.0-1.0) |
| `includeContent` | boolean | false | Include content text in results |

## Example Usage

| User Request | Tool | Parameters |
|--------------|------|------------|
| "Find error handling patterns" | `search_code` | `{"query": "error handling", "includeContent": true}` |
| "Rust async implementations" | `search_code` | `{"query": "async fn", "languageHint": "rust", "searchMode": "e7Only", "includeContent": true}` |
| "Find code E1 misses" | `search_cross_embedder_anomalies` | `{"query": "code patterns", "highEmbedder": "E7", "lowEmbedder": "E1"}` |
| "Precise function lookup" | `search_code` | `{"query": "authentication middleware", "searchMode": "pipeline", "includeContent": true}` |

## Edge Cases

| Condition | Response |
|-----------|----------|
| No code memories stored | "No code patterns found. Store code content first with store_memory." |
| No results above minScore | "No code matches above threshold. Try lowering minScore or broadening query." |
| Invalid searchMode | Error: searchMode must be one of: e7Only, pipeline, semantic |
