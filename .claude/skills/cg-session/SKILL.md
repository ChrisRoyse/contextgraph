---
name: cg-session
description: |
  Navigate session history, timeline, and conversation context using E4 positional embeddings.
  Use for recalling previous discussions, navigating conversation flow, or comparing session states.
  Keywords: session, history, timeline, context, previous, conversation, turn, navigate.
allowed-tools: mcp__context-graph__get_conversation_context,mcp__context-graph__get_session_timeline,mcp__context-graph__traverse_memory_chain,mcp__context-graph__compare_session_states
model: inherit
version: 2.0.0
user-invocable: true
---

# Session Navigator

Navigate conversation history and session timelines using E4 (V_ordering) sequence embeddings.

## Tools Overview

| Tool | Purpose | Required Params |
|------|---------|-----------------|
| `get_conversation_context` | Context around current turn | (none) |
| `get_session_timeline` | Chronological session timeline | (none) |
| `traverse_memory_chain` | Walk memory chain from anchor | `anchorId` (UUID) |
| `compare_session_states` | Compare states at two points | (none) |

## MCP Tool: get_conversation_context

Get memories around the current conversation turn with auto-anchoring.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `direction` | string | No | "before" | `before`, `after`, or `both` |
| `windowSize` | integer | No | 10 | Turns to include (1-50) |
| `sessionOnly` | boolean | No | true | Only current session memories |
| `includeContent` | boolean | No | true | Include full content text |
| `query` | string | No | - | Optional semantic filter alongside sequence ordering |
| `minSimilarity` | number | No | 0.0 | Min similarity when query provided (0.0-1.0) |

## MCP Tool: get_session_timeline

Ordered timeline of all session memories with sequence numbers and position labels.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `sessionId` | string | No | - | Session ID (default: current session) |
| `limit` | integer | No | 50 | Max memories (1-200) |
| `offset` | integer | No | 0 | Skip N memories for pagination |
| `sourceTypes` | array[string] | No | - | Filter by: HookDescription, ClaudeResponse, Manual, MDFileChunk |
| `includeContent` | boolean | No | false | Include full content text |

## MCP Tool: traverse_memory_chain

Navigate through a chain of memories from an anchor point. Multi-hop traversal for tracing topic evolution.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `anchorId` | string (UUID) | Yes | - | Starting memory UUID |
| `direction` | string | No | "backward" | `forward`, `backward`, or `bidirectional` |
| `hops` | integer | No | 5 | Max hops (1-20) |
| `semanticFilter` | string | No | - | Topic filter for related memories only |
| `minSimilarity` | number | No | 0.3 | Min similarity for filtered traversal (0.0-1.0) |
| `includeContent` | boolean | No | true | Include full content text |

## MCP Tool: compare_session_states

Compare memory state at different sequence points in a session.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `beforeSequence` | integer or "start" | No | - | Starting sequence number or "start" |
| `afterSequence` | integer or "current" | No | - | Ending sequence number or "current" |
| `topicFilter` | string | No | - | Focus comparison on specific topic |
| `sessionId` | string | No | - | Session ID (default: current session) |

## Example Usage

| User Request | Tool | Parameters |
|--------------|------|------------|
| "What did we discuss before?" | `get_conversation_context` | `{"direction": "before", "includeContent": true}` |
| "Show session timeline" | `get_session_timeline` | `{"includeContent": true}` |
| "Trace from this memory" | `traverse_memory_chain` | `{"anchorId": "uuid-here", "direction": "backward", "hops": 10}` |
| "What changed this session?" | `compare_session_states` | `{"beforeSequence": "start", "afterSequence": "current"}` |
| "Recent hook outputs" | `get_session_timeline` | `{"sourceTypes": ["HookDescription"], "limit": 20}` |

## Edge Cases

| Condition | Response |
|-----------|----------|
| No session memories | "No memories in current session. Memories are stored via hooks or store_memory." |
| anchorId not found | Error: "Memory {id} not found" |
| No memories in range | "No memories found between the specified sequence points." |
