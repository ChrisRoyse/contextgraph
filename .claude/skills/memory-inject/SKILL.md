---
name: memory-inject
description: "Retrieve and inject contextual memories for current task. Uses search_graph for semantic retrieval across 13 embedding spaces. Supports compact/standard/verbose formats with token budgeting. Keywords: memory, context, inject, retrieve, recall, background."
allowed-tools: mcp__context-graph__search_graph,mcp__context-graph__store_memory
model: haiku
version: 2.0.0
user-invocable: true
---
# Memory Inject

Retrieve relevant memories from the knowledge graph and format them for context injection.

## Overview

This skill searches the knowledge graph using `search_graph` to find semantically similar memories. Results are ranked by relevance across the 13 embedding spaces (weighted by category: SEMANTIC=1.0, RELATIONAL=0.5, STRUCTURAL=0.5, TEMPORAL=0.0 for metadata only).

**Primary Tool**: `search_graph` (retrieval)
**Secondary Tool**: `store_memory` (storage - use only when explicitly saving new context)

## Instructions

When the user needs context, background, or wants to recall previous work:

**R15: Check hook context first** — The `user_prompt_submit` hook already injects relevant memories via `<context-graph-injection>`. Before calling `search_graph`, check if the hook-injected context already contains relevant results. Only make MCP calls when hook context is insufficient or the user explicitly requests deeper search.

### Basic Retrieval

1. Extract the query from user input or current task context
2. Call `search_graph` with appropriate parameters:
   ```json
   {
     "query": "<user query or task context>",
     "topK": 10,
     "minSimilarity": 0.3,
     "includeContent": true
   }
   ```
3. Format results based on requested verbosity
4. Track token usage and truncate if budget exceeded

### Verbosity Levels

| Level | Token Budget | Content |
|-------|--------------|---------|
| compact | ~300 tokens | IDs + relevance scores only |
| standard | ~800 tokens | Full content + source + dominant embedder |
| verbose | ~1200 tokens | Add per-embedder similarity scores (includeEmbedderBreakdown=true) |

## MCP Tools

### search_graph (Primary)

Retrieves memories by semantic similarity.

**Parameters**:
- `query` (required): Search query text
- `topK` (optional): Max results 1-100 (default: 10)
- `minSimilarity` (optional): Threshold 0.0-1.0 (default: 0.0)
- `includeContent` (optional): Include content text (default: false)
- `strategy` (optional): e1_only, multi_space, pipeline (default: multi_space)
- `weightProfile` (optional): One of 14 profiles (e.g., semantic_search, code_search)
- `includeEmbedderBreakdown` (optional): Per-embedder scores (default: false)

**Returns**:
```json
{
  "results": [
    {
      "fingerprintId": "uuid",
      "similarity": 0.85,
      "dominantEmbedder": "E1",
      "content": "memory content (if includeContent=true)"
    }
  ],
  "count": 5,
  "searchTransparency": {
    "activeEmbedders": ["E1", "E5", "E7", "E8", "E10", "E11"],
    "strategy": "multi_space"
  }
}
```

### store_memory (Secondary - Storage Only)

Store new context into the knowledge graph. Use ONLY when user explicitly wants to save something.

**Parameters**:
- `content` (required): Content to store
- `rationale` (optional): Why this should be stored (1-1024 chars)
- `importance` (optional): Score 0.0-1.0 (default: 0.5)
- `sessionId` (optional): Session ID for scoped storage
- `operatorId` (optional): Operator ID for audit provenance

**Returns**:
```json
{
  "fingerprintId": "uuid",
  "embedderCount": 13,
  "embeddingLatencyMs": 150
}
```

## Output Formats

### Compact Format
```
Relevant Memories (N found):
- [0.XX] 550e8400... (E1)
- [0.XX] 6fa45921... (E7)
[N results, ~M tokens]
```

### Standard Format
```
Retrieved Memories (N found):

1. [similarity: 0.XX, via E7]
   ID: 550e8400-e29b-41d4-a716-446655440000
   The JWT middleware validates tokens...

2. [similarity: 0.XX, via E1]
   ID: 6fa45921-7c3e-4a8b-b5f2-123456789abc
   User authentication flow requires...

Token usage: M/budget
```

### Verbose Format
Standard format plus per-embedder similarity breakdown:
```
1. [similarity: 0.XX, via E7]
   ID: 550e8400-e29b-41d4-a716-446655440000
   The JWT middleware validates tokens...
   Embedder Breakdown:
     E1: 0.XX | E5: 0.XX | E7: 0.XX | E8: 0.XX
     E10: 0.XX | E11: 0.XX
     [E2-E4 temporal excluded from ranking]
```

## Edge Cases

| Condition | Response |
|-----------|----------|
| No query provided | "Please provide a search query to retrieve relevant memories." |
| No memories found | "No relevant memories found for '[query]'. The knowledge graph may not have related content yet." |
| Zero results above threshold | "No memories exceeded the similarity threshold (0.XX). Try lowering minSimilarity or broadening your query." |
| Token budget exceeded | "[N results truncated to fit budget. Showing top M most relevant.]" |
| Empty knowledge graph | "Knowledge graph is empty. No memories have been stored yet." |

## Example Usage

| User Request | Action | Parameters |
|--------------|--------|------------|
| "What do we know about authentication?" | `search_graph` | `{"query": "authentication", "topK": 10, "includeContent": true}` |
| "Get code context for the API" | `search_graph` | `{"query": "API implementation", "weightProfile": "code_search", "includeContent": true}` |
| "Brief overview of recent work" | `search_graph` | `{"query": "recent work tasks", "topK": 5, "includeContent": true}` |
| "Detailed context with scores" | `search_graph` | `{"query": "...", "topK": 15, "includeContent": true, "includeEmbedderBreakdown": true}` |
| "Save this finding for later" | `store_memory` | `{"content": "...", "rationale": "..."}` |

## Constraints

| Do NOT | Reason |
|--------|--------|
| Use deprecated `inject_context` tool | It was merged into `store_memory` |
| Guess memory content | Only show actual results from search |
| Exceed token budget silently | Always note truncation when budget exceeded |
| Include temporal embedders in relevance | E2-E4 have weight = 0.0 (metadata only) |
