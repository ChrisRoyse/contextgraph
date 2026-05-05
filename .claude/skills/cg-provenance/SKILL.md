---
name: cg-provenance
description: |
  Audit memory operations, trace merge history, and inspect provenance chains.
  Use for understanding how memories were created, merged, or modified.
  Keywords: provenance, audit, trail, merge, history, chain, lineage, tracking.
allowed-tools: mcp__context-graph__get_audit_trail,mcp__context-graph__get_merge_history,mcp__context-graph__get_provenance_chain
model: inherit
version: 2.0.0
user-invocable: true
---

# Provenance Auditor

Audit and trace memory provenance in Context Graph. Inspect the full lifecycle of any memory.

## Tools Overview

| Tool | Purpose | Required Params |
|------|---------|-----------------|
| `get_audit_trail` | Query audit log by target or time | (none) |
| `get_merge_history` | Show merge lineage for a memory | `memory_id` |
| `get_provenance_chain` | Full provenance from embedding to source | `memory_id` |

## MCP Tool: get_audit_trail

Query the audit log for a specific memory or time range. Returns chronological records of all operations (create, merge, delete, boost, etc.).

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `target_id` | string | No | - | UUID of memory to get audit trail for |
| `start_time` | string | No | - | ISO 8601 timestamp for range start (e.g., '2024-01-01T00:00:00Z') |
| `end_time` | string | No | - | ISO 8601 timestamp for range end |
| `limit` | integer | No | 50 | Max records to return (max: 500) |

At least one of `target_id` or `start_time`/`end_time` should be provided.

## MCP Tool: get_merge_history

Show merge lineage and history for a fingerprint. Returns all merge operations that created or affected this memory.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `memory_id` | string | Yes | - | UUID of the memory |
| `include_source_metadata` | boolean | No | false | Include source metadata for merged sources |

## MCP Tool: get_provenance_chain

Full provenance chain from embedding to source. Shows source type, file path, chunk info, operator attribution, causal direction, and creation timestamps.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `memory_id` | string | Yes | - | UUID of the memory |
| `include_audit` | boolean | No | false | Include audit trail in provenance chain |
| `include_embedding_version` | boolean | No | false | Include embedding version info |

## Example Usage

| User Request | Tool | Parameters |
|--------------|------|------------|
| "What happened to this memory?" | `get_audit_trail` | `{"target_id": "uuid-here"}` |
| "Show today's audit log" | `get_audit_trail` | `{"start_time": "2026-02-16T00:00:00Z", "limit": 100}` |
| "Was this memory merged from others?" | `get_merge_history` | `{"memory_id": "uuid-here", "include_source_metadata": true}` |
| "Full provenance for this memory" | `get_provenance_chain` | `{"memory_id": "uuid-here", "include_audit": true, "include_embedding_version": true}` |

## Edge Cases

| Condition | Response |
|-----------|----------|
| No audit records found | "No audit records found for the specified target or time range." |
| Memory has no merge history | "Memory was not created via merge — it was stored directly." |
| memory_id not found | Error: "Memory {id} not found" |
