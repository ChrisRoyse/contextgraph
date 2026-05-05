---
name: cg-blind-spot
description: |
  Detect blind spots where enhancer embedders find things E1 semantic misses.
  Use E9 HDC for typo-robust search or compare embedder views for disagreements.
  Keywords: blind spot, robust, typo, noise, compare, embedder, disagreement.
allowed-tools: mcp__context-graph__search_robust,mcp__context-graph__compare_embedder_views
model: inherit
user-invocable: true
---

# Blind Spot Detective

Find memories that standard semantic search (E1) misses but specialized embedders find.

## Steps

1. **Typo/noise robust search**: `mcp__context-graph__search_robust` with query (uses E9 HDC)
2. **Compare embedder views**: `mcp__context-graph__compare_embedder_views` to see where embedders disagree

## Tool Parameters

- `search_robust`: query (required), topK, minScore
- `compare_embedder_views`: query (required), topK, embedders (array of embedder names)
