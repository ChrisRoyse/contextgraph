---
name: session-context
description: "Explore session history, timeline, and context using E4 (V_ordering) sequence tools. Retrieve memories by conversation position, navigate chains, compare states. Keywords: session, history, timeline, context, previous, before, after, conversation, turn."
allowed-tools: Read,Glob,Grep
model: haiku
version: 1.0.0
user-invocable: true
---
# Session Context

Navigate and explore session history using E4 (V_ordering) sequence-based tools.

## Overview

This skill provides first-class conversational context capabilities:
- Get memories around the current conversation turn
- View ordered session timeline with position labels
- Navigate memory chains from any anchor point
- Compare session states at different points

**Research Basis**: MemoriesDB (2025), Memoria Framework, TG-RAG, Episodic Memory RAG

## Primary MCP Tools

### get_conversation_context

Get memories around the current conversation turn with auto-anchoring.

**Parameters**:
- `direction` (optional): "before" | "after" | "both" (default: "before")
- `windowSize` (optional): 1-50 turns to include (default: 10)
- `sessionOnly` (optional): Filter to current session (default: true)
- `includeContent` (optional): Include full text (default: true)
- `query` (optional): Semantic filter
- `minSimilarity` (optional): 0.0-1.0 threshold (default: 0.0)

**Example**:
```json
{
  "direction": "before",
  "windowSize": 5,
  "includeContent": true
}
```

**Response includes**:
- Memory content
- Sequence info with position labels ("2 turns ago", "previous turn")
- Source type (HookDescription, ClaudeResponse, etc.)

### get_session_timeline

Get an ordered timeline of all session memories.

**Parameters**:
- `sessionId` (optional): Session ID (default: current session)
- `limit` (optional): 1-200 results (default: 50)
- `offset` (optional): Pagination offset (default: 0)
- `sourceTypes` (optional): Filter by source type array
- `includeContent` (optional): Include full text (default: false)

**Example**:
```json
{
  "limit": 20,
  "sourceTypes": ["HookDescription", "ClaudeResponse"],
  "includeContent": true
}
```

**Response includes**:
- Timeline sorted by sequence (ascending)
- Position labels for each memory
- Current sequence number

### traverse_memory_chain

Navigate through a chain of memories from an anchor point.

**Parameters**:
- `anchorId` (required): UUID of starting memory
- `direction` (optional): "forward" | "backward" | "bidirectional" (default: "backward")
- `hops` (optional): 1-20 max hops (default: 5)
- `semanticFilter` (optional): Topic filter
- `minSimilarity` (optional): 0.0-1.0 threshold (default: 0.3)
- `includeContent` (optional): Include full text (default: true)

**Example**:
```json
{
  "anchorId": "550e8400-e29b-41d4-a716-446655440000",
  "direction": "backward",
  "hops": 10,
  "semanticFilter": "authentication"
}
```

### compare_session_states

Compare memory state at different sequence points.

**Parameters**:
- `beforeSequence` (optional): Start point (number or "start")
- `afterSequence` (optional): End point (number or "current")
- `topicFilter` (optional): Focus topic
- `sessionId` (optional): Session to compare within

**Example**:
```json
{
  "beforeSequence": "start",
  "afterSequence": "current"
}
```

**Response includes**:
- Memory counts in each window
- Source type breakdown
- Added memories count

## Usage Patterns

### "What did we discuss earlier?"

1. Use `get_conversation_context` with direction="before"
2. Results include position labels ("2 turns ago")

```json
{
  "direction": "before",
  "windowSize": 10,
  "includeContent": true
}
```

### "Show me the session timeline"

1. Use `get_session_timeline` with appropriate limit
2. Results sorted by sequence ascending

```json
{
  "limit": 50,
  "includeContent": false
}
```

### "What happened between X and now?"

1. Use `compare_session_states` with sequence range

```json
{
  "beforeSequence": 5,
  "afterSequence": "current"
}
```

### "Trace back from this memory"

1. Get memory UUID from previous search
2. Use `traverse_memory_chain` with backward direction

```json
{
  "anchorId": "<uuid>",
  "direction": "backward",
  "hops": 10
}
```

## Weight Profiles Used

| Tool | Profile | E4 Weight | E1 Weight |
|------|---------|-----------|-----------|
| get_conversation_context | conversation_history | 35% | 30% |
| get_session_timeline | sequence_navigation | 55% | 20% |
| traverse_memory_chain | sequence_navigation | 55% | 20% |
| compare_session_states | sequence_navigation | 55% | 20% |

## Position Labels

| Distance | Label |
|----------|-------|
| 0 | "current turn" |
| 1 | "previous turn" / "next turn" |
| N | "N turns ago" / "N turns ahead" |

## Constraints

| Do NOT | Reason |
|--------|--------|
| Use E2-E4 for topic similarity | Temporal embedders measure TIME, not TOPIC |
| Ignore sequenceInfo in results | Contains position labels for user clarity |
| Use this for general search | Use `/memory-inject` or `search_graph` instead |
| Assume memories exist | Session may be new with few memories |
