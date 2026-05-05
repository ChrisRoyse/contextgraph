---
name: context-inject
description: |
  Inject relevant memories into the current context using Context Graph's 13-embedder search.
  Use when starting a new task, investigating a problem, or needing background context.
  Keywords: context, inject, memory, retrieve, background, recall.
allowed-tools: mcp__context-graph__search_graph,mcp__context-graph__search_causes,mcp__context-graph__search_effects
model: inherit
user-invocable: true
---

# Context Inject

Retrieve and inject relevant memories from Context Graph into the current conversation.

## R15: Hook-Injected Context Awareness

The `user_prompt_submit` hook already injects relevant memories into each conversation turn via the system-reminder. Before making MCP calls, check if the hook-injected context (visible in the conversation as `<context-graph-injection>`) already answers the user's question. Only call `search_graph` if additional context is needed beyond what hooks already provided.

## Steps

1. **Check hook-injected context first** — look for existing `<context-graph-injection>` in the conversation.
2. **Determine the query** from the user's request or current task context.
2. **Search for relevant memories** using `mcp__context-graph__search_graph`:
   - Set `query` to the user's topic or question
   - Set `topK` to 10 (or higher for broad searches)
   - Set `includeContent` to true
   - Set `strategy` to "multi_space" for best recall across all embedder spaces
   - Set `includeEmbedderBreakdown` to true to see which embedders contributed
3. **If the query involves causality** (why, caused, because), also call:
   - `mcp__context-graph__search_causes` with the query
   - `mcp__context-graph__search_effects` with the query
4. **Present the results** as relevant context, noting similarity scores and dominant embedders.

## Parameters Reference

`search_graph` accepts:
- `query` (required): search text
- `topK`: 1-100 (default 10)
- `minSimilarity`: 0-1 (default 0.0)
- `includeContent`: boolean (default false)
- `strategy`: "e1_only" | "multi_space" | "pipeline"
- `weightProfile`: one of 14 profiles
- `includeEmbedderBreakdown`: boolean
- `causalDirection`: "auto" | "cause" | "effect" | "none"
