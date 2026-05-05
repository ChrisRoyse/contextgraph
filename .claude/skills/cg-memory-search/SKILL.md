---
name: cg-memory-search
description: |
  Search Context Graph memories across 13 embedding spaces with configurable strategies.
  Use for finding specific memories, searching by keywords, or exploring stored knowledge.
  Keywords: search, find, query, lookup, semantic, similar, retrieve, memory.
allowed-tools: mcp__context-graph__search_graph,mcp__context-graph__search_code,mcp__context-graph__search_by_keywords
model: inherit
user-invocable: true
---

# Memory Search

Search stored memories using Context Graph's multi-space retrieval.

## Steps

1. **Identify search intent** from the user's query.
2. **Choose the right tool**:
   - General search: `mcp__context-graph__search_graph`
   - Code search: `mcp__context-graph__search_code`
   - Exact keyword match: `mcp__context-graph__search_by_keywords`
3. **Configure search parameters**:
   - For broad search: strategy "multi_space", topK 20
   - For precise search: strategy "pipeline" (E13 recall -> E1 dense -> E12 rerank)
   - For code: use `search_code` with query containing code patterns
   - For exact terms: use `search_by_keywords` with specific terms
4. **Use weight profiles** for domain-specific search:
   - "semantic_search" for general meaning
   - "code_search" for code patterns
   - "causal_reasoning" for cause/effect
   - "fact_checking" for entity-focused search
5. **Present results** with similarity scores and content.

## search_graph Parameters

- `query` (required): search text
- `topK`: 1-100 (default 10)
- `strategy`: "e1_only" | "multi_space" | "pipeline"
- `weightProfile`: semantic_search | causal_reasoning | code_search | fact_checking | graph_reasoning | temporal_navigation | sequence_navigation | conversation_history | category_weighted | typo_tolerant | pipeline_stage1_recall | pipeline_stage2_scoring | pipeline_full | balanced
- `includeContent`: boolean
- `enableRerank`: boolean (E12 ColBERT reranking)
