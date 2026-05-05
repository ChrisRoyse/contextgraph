---
name: cg-causal
description: |
  Explore cause-effect relationships in Context Graph using E5 causal embeddings.
  Use for root cause analysis, understanding why something happened, or tracing effects.
  Keywords: causal, cause, effect, why, root cause, chain, reasoning.
allowed-tools: mcp__context-graph__search_causes,mcp__context-graph__search_effects,mcp__context-graph__get_causal_chain,mcp__context-graph__search_causal_relationships
model: inherit
version: 2.0.0
user-invocable: true
---

# Causal Reasoning

Explore cause-effect relationships using E5 (V_causality) asymmetric embeddings.

E5 uses separate cause/effect encodings per ARCH-15, with direction modifiers per AP-77:
- cause->effect: 1.2x boost
- effect->cause: 0.8x dampening

## Tools Overview

| Tool | Purpose | Required Params |
|------|---------|-----------------|
| `search_causes` | Find likely causes of an effect (abductive) | `query` |
| `search_effects` | Find effects of a cause (predictive) | `query` |
| `get_causal_chain` | Traverse causal chains from anchor | `anchorId` (UUID) |
| `search_causal_relationships` | Search LLM-generated causal descriptions | `query` |

## MCP Tool: search_causes

Abductive reasoning — find likely causes of observed effects. Uses 0.8x effect->cause dampening.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | Yes | - | The observed effect to find causes for |
| `topK` | integer | No | 10 | Results to return (1-50) |
| `minScore` | number | No | 0.1 | Minimum abductive score (0.0-1.0) |
| `includeContent` | boolean | No | false | Include full content text |
| `filterCausalDirection` | string | No | - | Filter by: `cause`, `effect`, `unknown` |
| `searchScope` | string | No | "memories" | Scope: `memories`, `relationships`, `all` |
| `strategy` | string | No | - | Strategy: `multi_space` or `pipeline` |
| `rerankWeight` | number | No | 0.4 | E12 rerank weight (0.0-1.0, pipeline only) |

## MCP Tool: search_effects

Predictive reasoning — find effects of a given cause. Uses 1.2x cause->effect boost.

### Parameters

Same as `search_causes` — `query` (required), `topK`, `minScore`, `includeContent`, `filterCausalDirection`, `searchScope`, `strategy`, `rerankWeight`.

## MCP Tool: get_causal_chain

Build transitive causal chains from an anchor point with hop attenuation (0.9^hop).

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `anchorId` | string (UUID) | Yes | - | UUID of the starting memory |
| `direction` | string | No | "forward" | `forward` (cause->effect) or `backward` (effect->cause) |
| `maxHops` | integer | No | 5 | Maximum hops (1-10) |
| `minSimilarity` | number | No | 0.3 | Minimum similarity per hop (0.0-1.0) |
| `includeContent` | boolean | No | false | Include full content text |

## MCP Tool: search_causal_relationships

Search LLM-generated causal descriptions with full provenance.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | Yes | - | Natural language causal query |
| `direction` | string | No | "all" | Filter: `cause`, `effect`, `all` |
| `topK` | integer | No | 10 | Results to return (1-100) |
| `includeSource` | boolean | No | true | Include original source content |
| `includeProvenance` | boolean | No | false | Include retrieval provenance metadata |
| `sourceWeight` | number | No | 0.6 | Source embedding weight in hybrid search (0.0-1.0) |
| `explanationWeight` | number | No | 0.4 | Explanation embedding weight (0.0-1.0) |
| `multiEmbedder` | boolean | No | false | Use E1+E5+E8+E11 consensus scoring (requires direction=cause or effect) |

## Example Usage

| User Request | Tool | Parameters |
|--------------|------|------------|
| "Why did the build fail?" | `search_causes` | `{"query": "build failure", "includeContent": true}` |
| "What happens if we remove caching?" | `search_effects` | `{"query": "removing cache layer", "includeContent": true}` |
| "Trace causal chain from this memory" | `get_causal_chain` | `{"anchorId": "uuid-here", "direction": "forward", "maxHops": 5}` |
| "What causes memory leaks?" | `search_causal_relationships` | `{"query": "memory leaks", "direction": "cause", "includeSource": true}` |

## Edge Cases

| Condition | Response |
|-----------|----------|
| No causal relationships stored | "No causal data found. Causal relationships are discovered by the LLM tools." |
| anchorId not found | Error: "Memory {id} not found" |
| Chain terminates early | Chain ends when no hop exceeds minSimilarity threshold |
| multiEmbedder with direction=all | Error: multiEmbedder requires direction to be 'cause' or 'effect' |
