# MCP Tools Reference

> Context Graph MCP Server -- JSON-RPC 2.0 tool reference for the 13-embedder semantic memory retrieval system.

**Source files covered:**

- `crates/context-graph-mcp/src/main.rs` -- server entry point, CLI, daemon mode
- `crates/context-graph-mcp/src/lib.rs` -- library root
- `crates/context-graph-mcp/src/protocol.rs` -- JSON-RPC types and error codes
- `crates/context-graph-mcp/src/server/mod.rs` -- `McpServer` struct, stdio transport, lifecycle
- `crates/context-graph-mcp/src/server/transport.rs` -- TCP transport, bounded reads
- `crates/context-graph-mcp/src/server/watchers.rs` -- background tasks (file watcher, code watcher, graph builder)
- `crates/context-graph-mcp/src/tools/` -- tool name constants, definitions, aliases, types
- `crates/context-graph-mcp/src/handlers/` -- tool handler implementations and dispatch
- `crates/context-graph-mcp/src/adapters/` -- `LazyMultiArrayProvider`, `LlmCausalHintProvider`
- `crates/context-graph-mcp/src/weights.rs` -- weight profile resolution

**Cross-references:** See `01_architecture_overview.md` for system architecture, `02_embedder_reference.md` for embedder details, `05_search_strategies.md` for search strategy internals.

---

## Table of Contents

1. [Server Architecture](#1-server-architecture)
2. [Request/Response Format](#2-requestresponse-format)
3. [Tool Registration and Routing](#3-tool-registration-and-routing)
4. [Tool Aliases](#4-tool-aliases)
5. [Error Codes](#5-error-codes)
6. [Core Tools](#6-core-tools)
7. [Curation Tools](#7-curation-tools)
8. [Topic Tools](#8-topic-tools)
9. [File Watcher Tools](#9-file-watcher-tools)
10. [Sequence Tools (E4)](#10-sequence-tools-e4)
11. [Causal Tools (E5)](#11-causal-tools-e5)
12. [Causal Discovery Tools (LLM)](#12-causal-discovery-tools-llm)
13. [Keyword Tools (E6)](#13-keyword-tools-e6)
14. [Code Tools (E7)](#14-code-tools-e7)
15. [Graph Tools (E8)](#15-graph-tools-e8)
16. [Robustness Tools (E9)](#16-robustness-tools-e9)
17. [Entity Tools (E11)](#17-entity-tools-e11)
18. [Embedder-First Search Tools](#18-embedder-first-search-tools)
19. [Token/Expansion Search Tools (E12/E13)](#19-tokenexpansion-search-tools-e12e13)
20. [Temporal Tools (E2/E3)](#20-temporal-tools-e2e3)
21. [Graph Linking Tools (K-NN)](#21-graph-linking-tools-k-nn)
22. [Maintenance Tools](#22-maintenance-tools)
23. [Provenance Tools](#23-provenance-tools)
24. [Daemon Tools](#24-daemon-tools)
25. [Training Tools](#25-training-tools)
26. [Constellation Tools](#26-constellation-tools)
27. [Contrastive Pair Tools](#27-contrastive-pair-tools)
28. [Typed-Edge Training Tools](#28-typed-edge-training-tools)

---

## 1. Server Architecture

### Transport Modes

The MCP server supports two transport modes selected via CLI, environment variable, or config file. Priority: CLI > ENV > Config > Default.

| Mode | Description | Use Case |
|------|-------------|----------|
| **stdio** (default) | Standard input/output, newline-delimited JSON-RPC | Process-based MCP clients (Claude Desktop, VS Code) |
| **tcp** | TCP socket on configurable address/port | Network-based clients, remote deployments |

SSE transport is explicitly **not supported**; the server returns a fatal error if requested.

### Daemon Mode

Daemon mode (`--daemon`) allows multiple Claude Code terminals to share a single MCP server process with embedding models loaded once into VRAM.

1. **First terminal**: Starts a headless TCP daemon that loads all 13 embedding models into VRAM (~32 GB on RTX 5090).
2. **Subsequent terminals**: Connect to the existing daemon via a stdio-to-TCP proxy.
3. Default daemon port: `3100` (configurable via `--daemon-port`).

### Direct Mode (stdio)

Without `--daemon`, the server runs as a standalone stdio process. It reads JSON-RPC requests line-by-line from stdin and writes responses to stdout. Each line is one JSON-RPC message.

### CLI Arguments

| Argument | Description |
|----------|-------------|
| `--config <PATH>` | Path to configuration file (TOML) |
| `--transport <MODE>` | `stdio` (default) or `tcp` |
| `--port <PORT>` | TCP port (only with `--transport tcp`) |
| `--bind <ADDRESS>` | TCP bind address (default: `127.0.0.1`) |
| `--warm-first` | Block startup until embedding models are loaded (default behavior) |
| `--no-warm` | Skip blocking warmup; embeddings fail until background load completes |
| `--daemon` | Enable daemon mode (recommended for multi-terminal) |
| `--daemon-port <PORT>` | Daemon TCP port (default: `3100`) |
| `--help`, `-h` | Show help message |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CONTEXT_GRAPH_TRANSPORT` | Transport mode (`stdio` or `tcp`) |
| `CONTEXT_GRAPH_TCP_PORT` | TCP port number |
| `CONTEXT_GRAPH_BIND_ADDRESS` | TCP bind address |
| `CONTEXT_GRAPH_WARM_FIRST` | Set to `"0"` to disable blocking warmup (default: `"1"`) |
| `CONTEXT_GRAPH_DAEMON` | Set to `"1"` to enable daemon mode |
| `CONTEXT_GRAPH_DAEMON_PORT` | Daemon port number (default: `3100`) |
| `RUST_LOG` | Log level (`error`, `warn`, `info`, `debug`, `trace`) |
| `CLAUDE_SESSION_ID` | Fallback session ID for `store_memory` when `sessionId` is not provided |

### Background Tasks

The server runs several background tasks:

- **Soft-delete GC** -- purges soft-deleted records after 7-day retention (runs every 5 minutes)
- **HNSW persistence** -- periodically persists HNSW indexes to disk
- **Background graph builder** -- computes K-NN edges for newly stored memories in batches (~60s cycle)
- **File watcher** -- monitors filesystem changes and indexes file content
- **Code watcher** -- monitors code files for E7 code embedding updates

All background tasks use `tokio::select!` with a watch channel for immediate shutdown signaling.

### Connection Limits

- TCP transport uses a `Semaphore` for limiting concurrent connections.
- Input lines are bounded at **10 MB** (`MAX_LINE_BYTES`) to prevent OOM.
- JSON-RPC batch requests are limited to **100** items (`MAX_BATCH_SIZE`).

---

## 2. Request/Response Format

The server uses **JSON-RPC 2.0** over newline-delimited JSON (NDJSON).

### Request

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "store_memory",
    "arguments": {
      "content": "Rust uses ownership for memory safety"
    }
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `jsonrpc` | string | Yes | Must be `"2.0"` |
| `id` | string, integer, or null | No | Request identifier. Absent = notification (no response). |
| `method` | string | Yes | One of `initialize`, `shutdown`, `notifications/initialized`, `tools/list`, `tools/call` |
| `params` | object | Depends | Required for `tools/call`; optional for others |

### Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "status": "stored", "memory_uuid": "..." }
}
```

Error response:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Missing 'content' parameter"
  }
}
```

### Notification

Requests without an `id` field are notifications. The `notifications/initialized` method is a notification sent by the client after `initialize` completes. Notifications produce no response.

---

## 3. Tool Registration and Routing

### MCP Methods

| Method | Description |
|--------|-------------|
| `initialize` | MCP lifecycle initialization; returns server capabilities |
| `notifications/initialized` | Client notification that initialization is complete |
| `shutdown` | Graceful server shutdown |
| `tools/list` | Returns all registered tool definitions with schemas |
| `tools/call` | Dispatches to a specific tool by name |

All tool access is via `tools/list` and `tools/call`. Direct method calls (e.g., `memory/store`) are **not supported**.

### Tool Call Dispatch

When `tools/call` is received:

1. Extract `name` from `params`.
2. Resolve aliases via `resolve_alias()` (see Section 4).
3. Extract `arguments` from `params` (defaults to `{}`).
4. Dispatch to the corresponding handler method via the `tool_dispatch!` macro.
5. If the tool name is unknown, return error code `-32006` (`TOOL_NOT_FOUND`).

### Tool Count

Defined in `crates/context-graph-mcp/src/tools/names.rs` (71 name constants) and dispatched in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` (81 match arms; the extra arms cover handlers that accept both zero-arg and single-arg forms via the `tool_dispatch!` macro).

- **With `llm` feature**: 75 tools (includes 4 LLM-dependent tools)
- **Without `llm` feature**: 67 tools

The 4 LLM-only tools are: `trigger_causal_discovery`, `get_causal_discovery_status`, `discover_graph_relationships`, `validate_graph_link`.

The 13 tools added since the prior revision (all non-LLM):

| Category | Tools |
|----------|-------|
| Training (§25) | `export_training_corpus`, `list_training_records`, `get_training_record`, `count_training_records` |
| Constellation (§26) | `compile_constellation`, `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation` |
| Contrastive (§27) | `mine_contrastive_pairs`, `list_contrastive_pairs`, `get_contrastive_pair`, `count_contrastive_pairs` |

All 13 downcast `Handlers::teleological_store` to `RocksDbTeleologicalStore`; in-memory and other backends return a tool error ("requires RocksDbTeleologicalStore") because the column families (`CF_TRAINING_RECORDS`, `CF_CONSTELLATIONS` + selector index, `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_ANCHOR` + `CF_CONTRASTIVE_BY_KIND`) are RocksDB-only.

---

## 4. Tool Aliases

Legacy tool names are resolved to their canonical forms transparently.

| Legacy Name | Canonical Name |
|-------------|----------------|
| `consolidate_memories` | `trigger_consolidation` |
| `inject_context` | `store_memory` |

Alias resolution happens before dispatch; clients can use either name.

---

## 5. Error Codes

### Standard JSON-RPC 2.0

| Code | Constant | Description |
|------|----------|-------------|
| `-32700` | `PARSE_ERROR` | Invalid JSON |
| `-32600` | `INVALID_REQUEST` | Invalid JSON-RPC request |
| `-32601` | `METHOD_NOT_FOUND` | Method does not exist |
| `-32602` | `INVALID_PARAMS` | Invalid method parameters |
| `-32603` | `INTERNAL_ERROR` | Internal server error |

### Context Graph Specific

| Code | Constant | Description |
|------|----------|-------------|
| `-32001` | `FEATURE_DISABLED` | Requested feature is not enabled |
| `-32002` | `NODE_NOT_FOUND` | Memory node not found by UUID |
| `-32004` | `STORAGE_ERROR` | Storage backend error |
| `-32005` | `EMBEDDING_ERROR` | Embedding generation error |
| `-32006` | `TOOL_NOT_FOUND` | Unknown tool name |
| `-32007` | `LAYER_TIMEOUT` | Handler timeout exceeded |
| `-32021` | `INSUFFICIENT_MEMORIES` | Not enough memories for topic detection |

### TCP Transport Specific

| Code | Constant | Description |
|------|----------|-------------|
| `-32110` | `TCP_BIND_FAILED` | Address/port unavailable or permission denied |
| `-32111` | `TCP_CONNECTION_ERROR` | Stream read/write failed, client disconnected |
| `-32112` | `TCP_MAX_CONNECTIONS_REACHED` | Maximum concurrent connections reached |
| `-32113` | `TCP_FRAME_ERROR` | Invalid NDJSON framing, message too large |
| `-32114` | `TCP_CLIENT_TIMEOUT` | Request processing exceeded timeout |

---

## 6. Core Tools

### 6.1 `store_memory`

Store a memory node directly in the knowledge graph without UTL processing. Generates all 13 embeddings for the content.

**Side effects:** Writes a `TeleologicalFingerprint` to RocksDB (51 column families); generates 13 embedding vectors; creates audit record; queues fingerprint for background K-NN graph building; if LLM feature enabled, may trigger inline causal discovery.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `content` | string | Yes | -- | Non-empty | The content to store |
| `rationale` | string | No | -- | 1-1024 chars | Why this context is relevant (merged from `inject_context`) |
| `importance` | number | No | `0.5` | `[0.0, 1.0]` | Importance score for the memory |
| `sessionId` | string | No | `CLAUDE_SESSION_ID` env | -- | Session ID for session-scoped storage |
| `operatorId` | string | No | -- | -- | Operator/user ID for audit provenance tracking |

**Return:** JSON object with `status`, `memory_uuid`, `rationale` (if provided), and storage metadata.

**Error conditions:**
- Content is empty or missing
- Rationale length outside 1-1024 range
- Embedding generation failure (models not loaded)
- RocksDB write failure

---

### 6.2 `get_memetic_status`

Get current system status including fingerprint count, number of embedders (13), storage backend and size, and layer status.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| *(none)* | -- | -- | -- | -- | No parameters |

**Return:** JSON object with fingerprint count, embedder count, storage backend info, storage size, and layer status from `LayerStatusProvider`.

---

### 6.3 `search_graph`

Search the knowledge graph using multi-space semantic similarity across 13 embedders. The primary search tool with extensive parameterization.

**Side effects:** None (read-only). May trigger E5 asymmetric reranking for causal queries.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | Non-empty | The search query text |
| `topK` | integer | No | `10` | `1-100` | Maximum results to return |
| `minSimilarity` | number | No | `0.0` | `[0.0, 1.0]` | Minimum similarity threshold |
| `includeContent` | boolean | No | `false` | -- | Include content text in results |
| `strategy` | string | No | `"multi_space"` | `"e1_only"`, `"multi_space"`, `"pipeline"` | Search strategy |
| `weightProfile` | string | No | -- | See below | Named weight profile for multi_space search |
| `customWeights` | object | No | -- | Keys: `E1`-`E13`, values: `[0, 1]` | Per-embedder weights (overrides `weightProfile`) |
| `excludeEmbedders` | array | No | -- | `["E1"..."E13"]` | Embedders to exclude from fusion |
| `useQuantizedPrefilter` | boolean | No | `false` | -- | Use PQ-8 quantized vectors for fast approximate pre-filtering |
| `includeEmbedderBreakdown` | boolean | No | `false` | -- | Include per-embedder score breakdown |
| `enableRerank` | boolean | No | `false` | -- | Enable ColBERT E12 re-ranking (Stage 3) |
| `enableAsymmetricE5` | boolean | No | `true` | -- | Enable asymmetric E5 causal reranking for causal queries |
| `causalDirection` | string | No | `"auto"` | `"auto"`, `"cause"`, `"effect"`, `"none"` | Causal direction mode |
| `enableQueryExpansion` | boolean | No | `false` | -- | Expand causal queries with related terms |
| `temporalWeight` | number | No | `0.0` | `[0.0, 1.0]` | Weight for temporal post-retrieval boost |
| `conversationContext` | object | No | -- | See sub-params | Convenience wrapper for sequence-based retrieval |
| `conversationContext.anchorToCurrentTurn` | boolean | No | `true` | -- | Auto-anchor to current session sequence |
| `conversationContext.turnsBack` | integer | No | `10` | `0-100` | Turns to look back from anchor |
| `conversationContext.turnsForward` | integer | No | `0` | `0-100` | Turns to look forward from anchor |
| `sessionScope` | string | No | `"all"` | `"current"`, `"all"`, `"recent"` | Session scope filter |
| `decayHalfLifeSecs` | integer | No | `86400` | -- | Half-life for exponential temporal decay (seconds) |
| `lastHours` | integer | No | -- | `>= 1` | Filter to last N hours |
| `lastDays` | integer | No | -- | `>= 1` | Filter to last N days |
| `sessionId` | string | No | -- | -- | Filter to specific session ID |
| `periodicBoost` | number | No | -- | `[0.0, 1.0]` | E3 periodic matching boost weight |
| `targetHour` | integer | No | -- | `0-23` | Target hour for periodic matching |
| `targetDayOfWeek` | integer | No | -- | `0-6` (0=Sun) | Target day for periodic matching |
| `sequenceAnchor` | string (UUID) | No | -- | -- | Anchor memory UUID for E4 sequence retrieval |
| `sequenceDirection` | string | No | -- | `"before"`, `"after"`, `"around"`, `"both"` | Direction for sequence retrieval |
| `includeProvenance` | boolean | No | `false` | -- | Include retrieval provenance metadata |
| `decayFunction` | string | No | `"exponential"` | `"linear"`, `"exponential"`, `"step"`, `"none"`, `"no_decay"` | Temporal decay function |
| `temporalScale` | string | No | `"meso"` | `"micro"`, `"meso"`, `"macro"`, `"long"`, `"archival"` | Temporal scale for decay |

**Weight profile values:** `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`.

**Return:** JSON object with `results` array. Each result contains `memory_uuid`, `score`, `content` (if requested), and optionally per-embedder breakdown and provenance metadata.

---

### 6.4 `trigger_consolidation`

Analyze memories for consolidation candidates. Returns pairs that could be merged but does NOT automatically merge them.

**Side effects:** None (read-only analysis).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `strategy` | string | No | `"similarity"` | `"similarity"`, `"temporal"`, `"semantic"` | Consolidation strategy |
| `min_similarity` | number | No | `0.925` | `[0.0, 1.0]` | Minimum similarity threshold (SRC-3 normalized scale) |
| `max_memories` | integer | No | `100` | `1-10000` | Maximum memories to process in one batch |

**Return:** JSON object with consolidation candidates (pairs of similar memories with similarity scores).

---

## 7. Curation Tools

### 7.1 `merge_concepts`

Merge two or more related concept nodes into a unified node with 30-day undo capability.

**Side effects:** Creates merged memory in RocksDB; soft-deletes source memories; writes audit records; returns `reversal_hash` for undo.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `source_ids` | array of UUID strings | Yes | -- | 2-10 UUIDs | UUIDs of concepts to merge |
| `target_name` | string | Yes | -- | 1-256 chars | Name for the merged concept |
| `rationale` | string | Yes | -- | 1-1024 chars | Rationale for merge (required per PRD 0.3) |
| `merge_strategy` | string | No | `"union"` | `"union"`, `"intersection"`, `"weighted_average"` | Merge strategy |
| `force_merge` | boolean | No | `false` | -- | Force merge even if priors conflict |

**Return:** JSON object with merged memory UUID, `reversal_hash`, and merge metadata.

**Error conditions:** Fewer than 2 source IDs; source IDs not found; empty target name.

### 7.2 `forget_concept`

Soft-delete a memory with 30-day recovery window (per SEC-06).

**Side effects:** Marks memory as soft-deleted in RocksDB (persisted, not just in-memory); writes audit record with optional reason.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `node_id` | string (UUID) | Yes | -- | Valid UUID | UUID of the memory to forget |
| `soft_delete` | boolean | No | `true` | -- | Use soft delete with 30-day recovery (per BR-MCP-001) |
| `operator_id` | string | No | -- | -- | Operator ID for provenance tracking |
| `reason` | string | No | -- | -- | Reason for deletion (for audit trail) |

**Return:** JSON object with `deleted_at` timestamp and deletion status.

**Error conditions:** Memory UUID not found.

### 7.3 `boost_importance`

Adjust a memory's importance score by delta. Final value clamped to `[0.0, 1.0]`.

**Side effects:** Updates importance in RocksDB; writes audit record.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `node_id` | string (UUID) | Yes | -- | Valid UUID | UUID of the memory to boost |
| `delta` | number | Yes | -- | `[-1.0, 1.0]` | Importance change value |
| `operator_id` | string | No | -- | -- | Operator ID for provenance tracking |

**Return:** JSON object with `old_importance`, `delta`, and `new_importance` (clamped).

**Error conditions:** Memory UUID not found; delta is NaN or Infinity.

---

## 8. Topic Tools

### 8.1 `get_topic_portfolio`

Get all discovered topics with profiles, stability metrics, and tier info. Topics emerge from weighted multi-space clustering (threshold >= 2.5). Temporal embedders (E2-E4) are excluded.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `format` | string | No | `"standard"` | `"brief"`, `"standard"`, `"verbose"` | Output format |

**Return:** JSON object with topic array; each topic has name, spaces, stability, and tier info.

### 8.2 `get_topic_stability`

Get portfolio-level stability metrics including churn rate, entropy, and phase breakdown.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `hours` | integer | No | `6` | `1-168` | Lookback period in hours |

**Return:** JSON object with churn rate, entropy, phase breakdown, and averages over the lookback period.

### 8.3 `detect_topics`

Force topic detection recalculation using HDBSCAN clustering.

**Side effects:** Recomputes topics in the cluster manager; updates in-memory topic state.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `force` | boolean | No | `false` | -- | Force detection even if recently computed |
| `max_memories` | integer | No | `10000` | `1-50000` | Maximum fingerprints to load for clustering |

**Return:** JSON object with detected topics and cluster statistics.

**Error conditions:** Fewer than 3 memories (minimum for HDBSCAN clustering); error code `-32021`.

### 8.4 `get_divergence_alerts`

Check for divergence from recent activity using semantic embedders only (E1, E6, E7, E10, E12, E13 per AP-62). E5 and E2-E4 are excluded.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `lookback_hours` | integer | No | `2` | `1-48` | Hours to look back for comparison |

**Return:** JSON object with divergence alerts and drift metrics.

---

## 9. File Watcher Tools

### 9.1 `list_watched_files`

List all files that have embeddings in the knowledge graph.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `include_counts` | boolean | No | `true` | -- | Include chunk counts per file |
| `path_filter` | string | No | -- | Glob pattern | Filter paths (e.g., `**/docs/*.md`) |

**Return:** JSON object with file list including paths, chunk counts, and last update times.

### 9.2 `get_file_watcher_stats`

Get statistics about file watcher content.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| *(none)* | -- | -- | -- | -- | No parameters |

**Return:** JSON object with total files, total chunks, average chunks per file, min/max values.

### 9.3 `delete_file_content`

Delete all embeddings for a specific file path.

**Side effects:** Soft-deletes (or hard-deletes) all memory chunks for the file in RocksDB; writes audit records.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `file_path` | string | Yes | -- | Absolute path | Path to the file |
| `soft_delete` | boolean | No | `true` | -- | Use soft delete with 30-day recovery (per SEC-06) |

**Return:** JSON object with deletion count and status.

### 9.4 `reconcile_files`

Find orphaned files (embeddings exist but file is missing from disk) and optionally delete them.

**Side effects:** If `dry_run=false`, deletes orphaned embeddings from RocksDB.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `dry_run` | boolean | No | `true` | -- | If true, only report without deleting |
| `base_path` | string | No | -- | -- | Base path to limit reconciliation scope |

**Return:** JSON object with list of orphaned files and deletion counts (if not dry run).

---

## 10. Sequence Tools (E4)

These tools use E4 (V_ordering) for session-positional retrieval.

### 10.1 `get_conversation_context`

Get memories around the current conversation turn with auto-anchoring.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `direction` | string | No | `"before"` | `"before"`, `"after"`, `"both"` | Search direction |
| `windowSize` | integer | No | `10` | `1-50` | Number of turns in the window |
| `sessionOnly` | boolean | No | `true` | -- | Only include memories from current session |
| `includeContent` | boolean | No | `true` | -- | Include full content text |
| `query` | string | No | -- | -- | Optional semantic filter |
| `minSimilarity` | number | No | `0.0` | `[0.0, 1.0]` | Minimum similarity when query is provided |

**Return:** JSON object with ordered memories around the current turn, with position labels (e.g., "2 turns ago").

### 10.2 `get_session_timeline`

Get an ordered timeline of all session memories with sequence numbers.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `sessionId` | string | No | Current session | -- | Session ID to get timeline for |
| `limit` | integer | No | `50` | `1-200` | Maximum memories to return |
| `offset` | integer | No | `0` | `>= 0` | Skip N memories for pagination |
| `sourceTypes` | array of strings | No | All types | `"HookDescription"`, `"ClaudeResponse"`, `"Manual"`, `"MDFileChunk"` | Filter by source types |
| `includeContent` | boolean | No | `false` | -- | Include full content text |

**Return:** JSON object with ordered timeline entries, each with sequence number and position label.

### 10.3 `traverse_memory_chain`

Navigate through a chain of memories starting from an anchor point. Supports multi-hop traversal.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `anchorId` | string (UUID) | Yes | -- | Valid UUID | Starting memory UUID |
| `direction` | string | No | `"backward"` | `"forward"`, `"backward"`, `"bidirectional"` | Traversal direction |
| `hops` | integer | No | `5` | `1-20` | Maximum hops to traverse |
| `semanticFilter` | string | No | -- | -- | Optional topic filter |
| `minSimilarity` | number | No | `0.3` | `[0.0, 1.0]` | Minimum similarity for filtered traversal |
| `includeContent` | boolean | No | `true` | -- | Include full content text |

**Return:** JSON object with chain of memories in traversal order, with hop numbers and similarity scores.

**Error conditions:** Anchor UUID not found.

### 10.4 `compare_session_states`

Compare memory state at different sequence points in a session.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `beforeSequence` | integer or string | Yes | -- | Integer or `"start"` | Starting sequence number |
| `afterSequence` | integer or string | Yes | -- | Integer or `"current"` | Ending sequence number |
| `sessionId` | string | No | Current session | -- | Session ID to compare within |

**Return:** JSON object with topics, memory counts, and key differences between the two points.

---

## 11. Causal Tools (E5)

These tools use E5 asymmetric embeddings with direction modifiers (cause->effect 1.2x, effect->cause 0.8x per AP-77).

### 11.1 `search_causal_relationships`

Search for causal relationships using E5 asymmetric similarity. Returns LLM-generated descriptions with provenance linking to source memories.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Natural language query about causal relationships |
| `direction` | string | No | `"all"` | `"cause"`, `"effect"`, `"all"` | Filter by causal direction |
| `topK` | integer | No | `10` | `1-100` | Maximum results |
| `includeSource` | boolean | No | `true` | -- | Include original source content |
| `includeProvenance` | boolean | No | `false` | -- | Include retrieval provenance metadata |
| `sourceWeight` | number | No | `0.6` | `[0.0, 1.0]` | Weight for source-anchored embeddings in hybrid search |
| `explanationWeight` | number | No | `0.4` | `[0.0, 1.0]` | Weight for explanation embeddings in hybrid search |
| `multiEmbedder` | boolean | No | `false` | -- | Enable multi-embedder search (E1+E5+E8+E11 consensus) |
| `minConsensus` | number | No | `0.0` | `[0.0, 1.0]` | Minimum consensus threshold for multi-embedder mode |
| `e1Weight` | number | No | `0.30` | `[0.0, 1.0]` | E1 semantic weight in multi-embedder mode |
| `e5Weight` | number | No | `0.35` | `[0.0, 1.0]` | E5 causal weight in multi-embedder mode |
| `e8Weight` | number | No | `0.15` | `[0.0, 1.0]` | E8 graph weight in multi-embedder mode |
| `e11Weight` | number | No | `0.20` | `[0.0, 1.0]` | E11 entity weight in multi-embedder mode |

**Return:** JSON object with causal relationships, each containing LLM-generated 1-3 paragraph descriptions, source memory references, and confidence scores.

### 11.2 `search_causes`

Abductive reasoning to find likely causes of observed effects. Uses 0.8x effect->cause dampening.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | The observed effect to find causes for |
| `topK` | integer | No | `10` | `1-50` | Maximum causes to return |
| `minScore` | number | No | `0.1` | `[0.0, 1.0]` | Minimum abductive score threshold |
| `includeContent` | boolean | No | `false` | -- | Include full content text |
| `filterCausalDirection` | string | No | -- | `"cause"`, `"effect"`, `"unknown"` | Filter by persisted causal direction |
| `searchScope` | string | No | `"memories"` | `"memories"`, `"relationships"`, `"all"` | Search scope |
| `strategy` | string | No | -- | `"e1_only"`, `"multi_space"`, `"pipeline"` | Search strategy |
| `rerankWeight` | number | No | `0.4` | `[0.0, 1.0]` | E12 rerank weight (pipeline strategy only) |

**Return:** JSON object with ranked causes and abductive scores.

### 11.3 `search_effects`

Find effects/consequences of a given cause. Uses 1.2x cause->effect boost.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | The cause to find effects for |
| `topK` | integer | No | `10` | `1-50` | Maximum effects to return |
| `minScore` | number | No | `0.1` | `[0.0, 1.0]` | Minimum predictive score threshold |
| `includeContent` | boolean | No | `false` | -- | Include full content text |
| `filterCausalDirection` | string | No | -- | `"cause"`, `"effect"`, `"unknown"` | Filter by persisted causal direction |
| `searchScope` | string | No | `"memories"` | `"memories"`, `"relationships"`, `"all"` | Search scope |
| `strategy` | string | No | -- | `"e1_only"`, `"multi_space"`, `"pipeline"` | Search strategy |
| `rerankWeight` | number | No | `0.4` | `[0.0, 1.0]` | E12 rerank weight (pipeline strategy only) |

**Return:** JSON object with ranked effects and predictive scores.

### 11.4 `get_causal_chain`

Build and visualize transitive causal chains from an anchor point. Applies hop attenuation (0.9^hop).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `anchorId` | string (UUID) | Yes | -- | Valid UUID | Starting memory UUID |
| `direction` | string | No | `"forward"` | `"forward"`, `"backward"` | Traverse cause->effect or effect->cause |
| `maxHops` | integer | No | `5` | `1-10` | Maximum hops |
| `minSimilarity` | number | No | `0.3` | `[0.0, 1.0]` | Minimum similarity per hop |
| `includeContent` | boolean | No | `false` | -- | Include full content text |

**Return:** JSON object with causal chain nodes in traversal order, with hop-attenuated scores.

**Error conditions:** Anchor UUID not found.

---

## 12. Causal Discovery Tools (LLM)

These tools require the `llm` feature flag. They use Hermes-2-Pro-Mistral-7B (Q5_K_M GGUF, ~5GB VRAM) via `llama-cpp-2` for grammar-constrained (GBNF) JSON inference (causal-agent/src/llm/mod.rs:85, mod.rs:139). The shipping tool description (tools/definitions/causal_discovery.rs:35) still references the legacy "Qwen2.5 via Candle"; that description is stale — the handler is backed by `CausalDiscoveryLLM` built on llama-cpp-2.

### 12.1 `trigger_causal_discovery`

Manually trigger the causal discovery agent to analyze memories for cause-effect relationships.

**Side effects:** LLM inference (Hermes-2-Pro-Mistral-7B Q5_K_M, ~5GB model + ~1GB KV cache ≈ 6GB VRAM within the 9GB budget per causal-agent/src/lib.rs:33); creates E5 asymmetric embeddings via `CausalModel.embed_dual()`; writes causal relationships to RocksDB; creates graph edges.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `mode` | string | No | `"extract"` | `"extract"`, `"pairs"` | `"extract"`: find all cause-effect in each memory; `"pairs"`: find relationships between memories |
| `maxPairs` | integer | No | `50` | `1-200` | Maximum candidate pairs to analyze per run |
| `minConfidence` | number | No | `0.7` | `[0.5, 1.0]` | Minimum LLM confidence to accept a relationship |
| `sessionScope` | string | No | `"all"` | `"current"`, `"all"`, `"recent"` | Scope of memories to analyze |
| `session_id` | string | No | -- | -- | Session ID (used with `sessionScope="current"`) |
| `dryRun` | boolean | No | `false` | -- | Analyze without creating embeddings or edges |

**Return:** JSON object with discovery results: pairs analyzed, relationships found, confidence scores, and timing.

**Error conditions:** LLM feature not enabled; model loading failure; VRAM exhaustion.

### 12.2 `get_causal_discovery_status`

Get status and statistics of the causal discovery agent.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `includeLastResult` | boolean | No | `true` | -- | Include detailed results from last cycle |
| `includeGraphStats` | boolean | No | `true` | -- | Include causal graph statistics |

**Return:** JSON object with agent running state, last cycle results, VRAM usage, and cumulative statistics.

---

## 13. Keyword Tools (E6)

### 13.1 `search_by_keywords`

Find memories matching specific keywords using E6 sparse embeddings. Enhances E1 with keyword-level precision.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Keyword query (phrase or multiple keywords) |
| `topK` | integer | No | `10` | `1-50` | Maximum results |
| `minScore` | number | No | `0.1` | `[0.0, 1.0]` | Minimum blended score threshold |
| `blendWithSemantic` | number | No | `0.3` | `[0.0, 1.0]` | E6 keyword weight in blend (0.0=pure E1, 1.0=pure E6) |
| `useSpladeExpansion` | boolean | No | `true` | -- | Use E13 SPLADE for term expansion (fast->quick) |
| `includeContent` | boolean | No | `false` | -- | Include full content text |
| `strategy` | string | No | `"multi_space"` | `"e1_only"`, `"multi_space"`, `"pipeline"` | Search strategy |

**Return:** JSON object with results ranked by blended keyword and semantic scores.

---

## 14. Code Tools (E7)

### 14.1 `search_code`

Find memories containing code patterns using E7 dense embeddings (1536D Qodo-Embed).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Code query (functionality, patterns, or constructs) |
| `topK` | integer | No | `10` | `1-50` | Maximum results |
| `minScore` | number | No | `0.2` | `[0.0, 1.0]` | Minimum blended score threshold |
| `blendWithSemantic` | number | No | `0.4` | `[0.0, 1.0]` | E7 code weight in blend (0.0=pure E1, 1.0=pure E7) |
| `includeContent` | boolean | No | `false` | -- | Include full content text |
| `searchMode` | string | No | `"hybrid"` | `"hybrid"`, `"e7Only"`, `"e1WithE7Rerank"`, `"pipeline"` | Code search strategy |
| `languageHint` | string | No | -- | `"rust"`, `"python"`, `"javascript"`, `"typescript"`, `"go"`, `"java"`, `"cpp"`, `"sql"` | Programming language hint |

**Return:** JSON object with results including detected language info and relevance scores.

---

## 15. Graph Tools (E8)

### 15.1 `search_connections`

Find memories connected to a given concept using asymmetric E8 similarity (source->target 1.2x, target->source 0.8x).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Concept to find connections for |
| `direction` | string | No | `"both"` | `"source"`, `"target"`, `"both"` | Connection direction |
| `topK` | integer | No | `10` | `1-50` | Maximum connections |
| `minScore` | number | No | `0.1` | `[0.0, 1.0]` | Minimum connection score |
| `includeContent` | boolean | No | `false` | -- | Include full content text |
| `filterGraphDirection` | string | No | -- | `"source"`, `"target"`, `"unknown"` | Filter by persisted direction |
| `includeProvenance` | boolean | No | `false` | -- | Include provenance metadata |

**Return:** JSON object with connected memories, connection scores, and direction information.

### 15.2 `get_graph_path`

Build multi-hop graph paths from an anchor point. Applies hop attenuation (0.9^hop).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `anchorId` | string (UUID) | Yes | -- | Valid UUID | Starting memory UUID |
| `direction` | string | No | `"forward"` | `"forward"`, `"backward"` | source->target or target->source |
| `maxHops` | integer | No | `5` | `1-10` | Maximum hops |
| `minSimilarity` | number | No | `0.3` | `[0.0, 1.0]` | Minimum similarity per hop |
| `includeContent` | boolean | No | `false` | -- | Include full content text |

**Return:** JSON object with graph path nodes and hop-attenuated scores.

### 15.3 `discover_graph_relationships` (LLM only)

Discover graph relationships between memories using LLM analysis (Hermes-2-Pro-Mistral-7B, shared via `Arc<CausalDiscoveryLLM>` with the causal agent per graph-agent/src/lib.rs).

**Side effects:** LLM inference; writes discovered relationships to RocksDB; creates E8 graph edges.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memory_ids` | array of UUID strings | Yes | -- | 2-50 UUIDs | Memory UUIDs to analyze |
| `relationship_types` | array of strings | No | All types | See below | Filter to specific types |
| `min_confidence` | number | No | `0.7` | `[0.0, 1.0]` | Minimum confidence threshold |
| `batch_size` | integer | No | `50` | `1-100` | Maximum candidate pairs to analyze |

**Relationship types:** `contains`, `scoped_by`, `depends_on`, `imports`, `requires`, `references`, `cites`, `interprets`, `distinguishes`, `implements`, `complies_with`, `fulfills`, `extends`, `modifies`, `supersedes`, `overrules`, `calls`, `applies`, `used_by`.

**Return:** JSON object with discovered relationships including confidence scores, categories, and directions.

### 15.4 `validate_graph_link` (LLM only)

Validate a proposed graph link between two memories using LLM analysis.

**Side effects:** LLM inference (read-only validation, no writes).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `source_id` | string (UUID) | Yes | -- | Valid UUID | Source memory UUID |
| `target_id` | string (UUID) | Yes | -- | Valid UUID | Target memory UUID |
| `expected_relationship_type` | string | No | -- | Same as `relationship_types` above | Expected type to validate |

**Return:** JSON object with validation result, detected relationship type, confidence score, category, and direction.

---

## 16. Robustness Tools (E9)

### 16.1 `search_robust`

Find memories using E9 noise-robust structural matching via character trigram hypervectors. Finds matches despite typos, casing variations, and morphological changes.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | Min 3 chars | Query text (typos OK) |
| `topK` | integer | No | `10` | `1-50` | Maximum results |
| `minScore` | number | No | `0.1` | `[0.0, 1.0]` | Minimum blended score |
| `e9DiscoveryThreshold` | number | No | `0.08` | `[0.0, 1.0]` | Minimum E9 score for blind-spot discovery |
| `e1WeaknessThreshold` | number | No | `0.5` | `[0.0, 1.0]` | Maximum E1 score to be considered "missed" |
| `includeContent` | boolean | No | `false` | -- | Include full content text |
| `includeE9Score` | boolean | No | `true` | -- | Include separate E9 and E1 scores |
| `strategy` | string | No | `"multi_space"` | `"e1_only"`, `"multi_space"`, `"pipeline"` | Search strategy |

**Return:** JSON object with results including per-embedder E9/E1 scores and blind-spot annotations.

---

## 17. Entity Tools (E11)

These tools use E11 KEPLER/TransE entity embeddings (768D).

### 17.1 `extract_entities`

Extract and canonicalize entities from text using pattern matching and knowledge base lookup.

**Side effects:** None (read-only text analysis).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `text` | string | Yes | -- | -- | Text to extract entities from |
| `includeUnknown` | boolean | No | `true` | -- | Include entities not in knowledge base |
| `groupByType` | boolean | No | `false` | -- | Group results by entity type |

**Return:** JSON object with extracted entities, each with canonical form, original mention, type, and confidence.

### 17.2 `search_by_entities`

Find memories containing specific entities with entity-aware ranking.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `entities` | array of strings | Yes | -- | -- | Entity names to search for (auto-canonicalized) |
| `entityTypes` | array of strings | No | -- | `"ProgrammingLanguage"`, `"Framework"`, `"Database"`, `"Cloud"`, `"Company"`, `"TechnicalTerm"`, `"Unknown"` | Filter by entity types |
| `matchMode` | string | No | `"any"` | `"any"`, `"all"` | Match any or all entities |
| `topK` | integer | No | `10` | `1-50` | Maximum results |
| `minScore` | number | No | `0.2` | `[0.0, 1.0]` | Minimum similarity threshold |
| `includeContent` | boolean | No | `false` | -- | Include full content |
| `boostExactMatch` | number | No | `1.15` | `[1.0, 3.0]` | Boost multiplier for exact entity matches |
| `strategy` | string | No | -- | `"e1_only"`, `"multi_space"`, `"pipeline"` | Search strategy |

**Return:** JSON object with entity-ranked results and matched entity details.

### 17.3 `infer_relationship`

Infer the relationship between two entities using TransE (r^ = t - h).

**Side effects:** None (read-only computation).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `headEntity` | string | Yes | -- | -- | Subject/head entity |
| `tailEntity` | string | Yes | -- | -- | Object/tail entity |
| `headType` | string | No | -- | `"ProgrammingLanguage"`, `"Framework"`, `"Database"`, `"Cloud"`, `"Company"`, `"TechnicalTerm"` | Type hint for head |
| `tailType` | string | No | -- | Same as `headType` | Type hint for tail |
| `topK` | integer | No | `5` | `1-20` | Number of relation candidates |
| `includeScore` | boolean | No | `true` | -- | Include raw TransE scores |

**Return:** JSON object with ranked relation candidates and TransE scores.

### 17.4 `find_related_entities`

Find entities that have a given relationship to a source entity using TransE.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `entity` | string | Yes | -- | -- | Source entity |
| `relation` | string | Yes | -- | -- | Relationship to search for |
| `direction` | string | No | `"outgoing"` | `"outgoing"`, `"incoming"` | h->t or t<-h |
| `entityType` | string | No | -- | `"ProgrammingLanguage"`, `"Framework"`, `"Database"`, `"Cloud"`, `"Company"`, `"TechnicalTerm"` | Filter by entity type |
| `topK` | integer | No | `10` | `1-50` | Maximum results |
| `minScore` | number | No | -- | `[-10, 0]` | Minimum TransE score (more negative = less strict) |
| `searchMemories` | boolean | No | `true` | -- | Filter to entities found in stored memories |

**Return:** JSON object with related entities and TransE scores.

### 17.5 `validate_knowledge`

Score whether a (subject, predicate, object) knowledge triple is valid using TransE. Score of 0 is perfect match.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `subject` | string | Yes | -- | -- | Subject/head entity |
| `predicate` | string | Yes | -- | -- | Predicate/relation |
| `object` | string | Yes | -- | -- | Object/tail entity |
| `subjectType` | string | No | -- | `"ProgrammingLanguage"`, `"Framework"`, `"Database"`, `"Cloud"`, `"Company"`, `"TechnicalTerm"` | Type hint for subject |
| `objectType` | string | No | -- | Same as `subjectType` | Type hint for object |

**Return:** JSON object with validation result (`"valid"`, `"uncertain"`, `"unlikely"`), TransE score, and optionally supporting/contradicting memories.

### 17.6 `get_entity_graph`

Build and visualize entity relationships discovered in stored memories.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `centerEntity` | string | No | -- | -- | Focal entity to center the graph on |
| `maxNodes` | integer | No | `50` | `1-500` | Maximum number of nodes |
| `entityTypes` | array of strings | No | -- | `"ProgrammingLanguage"`, `"Framework"`, `"Database"`, `"Cloud"`, `"Company"`, `"TechnicalTerm"`, `"Unknown"` | Filter by entity types |
| `minRelationScore` | number | No | `0.3` | `[0.0, 1.0]` | Minimum edge score threshold |
| `includeMemoryCounts` | boolean | No | `true` | -- | Include memory reference counts per node |

**Return:** JSON object with graph nodes and edges, each edge weighted by score and evidence count.

---

## 18. Embedder-First Search Tools

These tools enable searching using any of the 13 embedders as the primary perspective (Constitution v6.3).

### 18.1 `search_by_embedder`

Search using any HNSW-capable embedder as the primary perspective.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `embedder` | string | Yes | -- | `"E1"`, `"E2"`, `"E3"`, `"E4"`, `"E5"`, `"E7"`, `"E8"`, `"E9"`, `"E10"`, `"E11"` | Primary embedder (E6/E12/E13 not supported -- non-HNSW) |
| `query` | string | Yes | -- | -- | Search query |
| `topK` | integer | No | `10` | `1-100` | Maximum results |
| `minSimilarity` | number | No | `0` | `[0.0, 1.0]` | Minimum similarity threshold |
| `includeContent` | boolean | No | `false` | -- | Include full content text |
| `includeAllScores` | boolean | No | `false` | -- | Include scores from all 13 embedders |

**Return:** JSON object with results ranked by the selected embedder's similarity, optionally with all-embedder scores.

### 18.2 `get_embedder_clusters`

Explore clusters of memories in a specific embedder's space.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `embedder` | string | Yes | -- | `"E1"`, `"E2"`, `"E3"`, `"E4"`, `"E5"`, `"E7"`, `"E8"`, `"E9"`, `"E10"`, `"E11"` | Embedder space to cluster |
| `minClusterSize` | integer | No | `3` | `2-50` | Minimum memories per cluster |
| `topClusters` | integer | No | `10` | `1-50` | Maximum clusters to return |
| `includeSamples` | boolean | No | `true` | -- | Include sample memories per cluster |
| `samplesPerCluster` | integer | No | `3` | `1-10` | Number of samples per cluster |

**Return:** JSON object with cluster list, each containing centroid info, member count, and sample memories.

### 18.3 `compare_embedder_views`

Compare how different embedders rank the same query, highlighting agreements and unique finds.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Search query |
| `embedders` | array of strings | Yes | -- | 2-5 items from `"E1"`...`"E11"` (not E6/E12/E13) | Embedders to compare |
| `topK` | integer | No | `5` | `1-20` | Top results per embedder |
| `includeContent` | boolean | No | `false` | -- | Include content text |

**Return:** JSON object with per-embedder rankings, agreement analysis, and unique finds.

### 18.4 `list_embedder_indexes`

List all 13 embedder indexes with statistics.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `includeDetails` | boolean | No | `true` | -- | Include memory usage and query latency stats |

**Return:** JSON object with per-embedder index information: dimension, index type, vector count, size, GPU residency.

### 18.5 `get_memory_fingerprint`

Retrieve per-embedder fingerprint vectors for a specific memory.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memoryId` | string (UUID) | Yes | -- | Valid UUID | Memory to inspect |
| `embedders` | array of strings | No | All 13 | `"E1"`...`"E13"` | Filter to specific embedders |
| `includeVectorNorms` | boolean | No | `true` | -- | Include L2 norm of each vector |
| `includeContent` | boolean | No | `false` | -- | Include memory content text |

**Return:** JSON object with per-embedder presence, dimension, L2 norm, and variant info for asymmetric embedders (E5/E8/E10). Sparse embedders (E6/E13) show non-zero element count.

**Error conditions:** Memory UUID not found.

### 18.6 `create_weight_profile`

Create a named custom embedder weight profile for the current session.

**Side effects:** Stores profile in server memory (session-scoped, not persisted to disk). Maximum 1000 custom profiles.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `name` | string | Yes | -- | 1-64 chars | Profile name (must not conflict with built-in names) |
| `weights` | object | Yes | -- | Keys `E1`-`E13`, values `[0, 1]`, sum ~1.0 | Per-embedder weights |
| `description` | string | No | -- | -- | Optional description |

**Return:** JSON object confirming profile creation with normalized weight values.

**Error conditions:** Name conflicts with built-in profile; weights do not sum to ~1.0; exceeds 1000 custom profiles.

### 18.7 `search_cross_embedder_anomalies`

Find memories that score high in one embedder but low in another (blind-spot detection).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Search query |
| `highEmbedder` | string | Yes | -- | `"E1"`...`"E13"` | Embedder expected to score high |
| `lowEmbedder` | string | Yes | -- | `"E1"`...`"E13"` | Embedder expected to score low |
| `highThreshold` | number | No | `0.5` | `[0.0, 1.0]` | Minimum score in high embedder |
| `lowThreshold` | number | No | `0.3` | `[0.0, 1.0]` | Maximum score in low embedder |
| `topK` | integer | No | `10` | `1-100` | Maximum results |
| `includeContent` | boolean | No | `false` | -- | Include content text |
| `primaryEmbedder` | string | No | `"E1"` | `"E1"`...`"E13"` | Primary embedder for generalized detection |
| `contrastEmbedder` | string | No | `"E9"` | `"E1"`...`"E13"` | Contrast embedder for generalized detection |

**Return:** JSON object with anomalous memories and anomaly scores (high_score - low_score).

---

## 19. Token/Expansion Search Tools (E12/E13)

### 19.1 `search_by_tokens`

Search using E12 ColBERT token-level MaxSim scoring for precise phrase matching.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Query text |
| `topK` | integer | No | `10` | `1-100` | Maximum results |
| `minSimilarity` | number | No | `0.3` | `[0.0, 1.0]` | Minimum similarity threshold |

**Return:** JSON object with results ranked by MaxSim token-level scores.

### 19.2 `search_by_expansion`

Search using E13 SPLADE learned term expansion for enhanced keyword recall.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Query text |
| `topK` | integer | No | `10` | `1-100` | Maximum results |
| `minScore` | number | No | `0.1` | `[0.0, 1.0]` | Minimum score threshold |

**Return:** JSON object with results ranked by SPLADE expansion scores.

---

## 20. Temporal Tools (E2/E3)

Temporal boost is applied POST-retrieval per ARCH-25 (not in similarity fusion).

### 20.1 `search_recent`

Search with E2 temporal freshness boost applied.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Search query text |
| `topK` | integer | No | `10` | `1-100` | Maximum results |
| `temporalWeight` | number | No | `0.3` | `[0.1, 1.0]` | Temporal boost weight (higher = more recency preference) |
| `decayFunction` | string | No | `"exponential"` | `"linear"`, `"exponential"`, `"step"`, `"none"`, `"no_decay"` | Decay function |
| `temporalScale` | string | No | `"meso"` | `"micro"` (1h), `"meso"` (1d), `"macro"` (1w), `"long"` (1mo), `"archival"` (1y) | Time scale |
| `includeContent` | boolean | No | `true` | -- | Include content text |
| `minSimilarity` | number | No | `0.1` | `[0.0, 1.0]` | Minimum semantic similarity before temporal boost |

**Return:** JSON object with temporally boosted results.

### 20.2 `search_periodic`

Search for memories matching periodic time patterns (E3 V_periodicity).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `query` | string | Yes | -- | -- | Search query text |
| `topK` | integer | No | `10` | `1-100` | Maximum results |
| `targetHour` | integer | No | -- | `0-23` | Target hour (auto-detected if `autoDetect=true`) |
| `targetDayOfWeek` | integer | No | -- | `0-6` (0=Sun) | Target day (auto-detected if `autoDetect=true`) |
| `autoDetect` | boolean | No | `false` | -- | Auto-detect target from current time |
| `periodicWeight` | number | No | `0.3` | `[0.1, 1.0]` | Periodic boost weight |
| `includeContent` | boolean | No | `true` | -- | Include content text |
| `minSimilarity` | number | No | `0.1` | `[0.0, 1.0]` | Minimum semantic similarity before periodic boost |

**Return:** JSON object with periodically boosted results.

---

## 21. Graph Linking Tools (K-NN)

These tools navigate the pre-computed K-NN graph and typed edges.

### 21.1 `get_memory_neighbors`

Get K nearest neighbors in a specific embedder space using pre-computed K-NN edges.

**Side effects:** None (read-only). Note: recently stored memories may return 0 neighbors until background graph build runs (~60s).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memory_id` | string (UUID) | Yes | -- | Valid UUID | Memory to find neighbors for |
| `embedder_id` | integer | No | `0` | `0-12` | Embedder space (0=E1, 4=E5, 6=E7, 7=E8, 9=E10, 10=E11) |
| `top_k` | integer | No | `10` | `1-50` | Number of neighbors |
| `min_similarity` | number | No | `0.0` | `[0.0, 1.0]` | Minimum similarity threshold |
| `include_content` | boolean | No | `false` | -- | Include memory content |

**Return:** JSON object with neighbor list sorted by similarity, each with memory UUID, similarity score, and optionally content.

### 21.2 `get_typed_edges`

Get typed edges from a memory. Types are derived from embedder agreement patterns.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memory_id` | string (UUID) | Yes | -- | Valid UUID | Memory to get edges from |
| `edge_type` | string | No | -- | `"semantic_similar"`, `"code_related"`, `"entity_shared"`, `"causal_chain"`, `"graph_connected"`, `"paraphrase_aligned"`, `"keyword_overlap"`, `"multi_agreement"` | Filter by edge type |
| `direction` | string | No | `"outgoing"` | `"outgoing"`, `"incoming"`, `"both"` | Edge direction |
| `min_weight` | number | No | `0.0` | `[0.0, 1.0]` | Minimum edge weight |
| `include_content` | boolean | No | `false` | -- | Include memory content |

**Return:** JSON object with typed edges, each containing target memory UUID, edge type, weight, and direction.

### 21.3 `traverse_graph`

Multi-hop graph traversal following typed edges.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `start_memory_id` | string (UUID) | Yes | -- | Valid UUID | Starting memory |
| `max_hops` | integer | No | `2` | `1-5` | Maximum traversal depth |
| `edge_type` | string | No | -- | Same as `get_typed_edges` edge types | Filter by edge type |
| `min_weight` | number | No | `0.3` | `[0.0, 1.0]` | Minimum edge weight to follow |
| `max_results` | integer | No | `20` | `1-100` | Maximum paths to return |
| `include_content` | boolean | No | `false` | -- | Include memory content |

**Return:** JSON object with traversal paths, each showing the sequence of memories and edges followed.

### 21.4 `get_unified_neighbors`

Find neighbors using Weighted RRF fusion across all 13 embedders. Per ARCH-21: uses RRF, not weighted sum. Per AP-60: temporal embedders (E2-E4) excluded from semantic fusion.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memory_id` | string (UUID) | Yes | -- | Valid UUID | Memory to find neighbors for |
| `weight_profile` | string | No | `"semantic_search"` | Same as `search_graph` `weightProfile` values | Weight profile for RRF fusion |
| `top_k` | integer | No | `10` | `1-50` | Number of neighbors |
| `min_score` | number | No | `0.0` | `[0.0, 1.0]` | Minimum RRF score |
| `include_content` | boolean | No | `false` | -- | Include memory content |
| `include_embedder_breakdown` | boolean | No | `true` | -- | Include per-embedder scores and ranks |
| `custom_weights` | object | No | -- | Keys `E1`-`E13`, values `[0, 1]` | Custom per-embedder weights (overrides profile) |
| `exclude_embedders` | array of strings | No | -- | `"E1"`...`"E13"` | Embedders to exclude |

**Return:** JSON object with unified neighbor list ranked by RRF score, with optional per-embedder breakdown.

---

## 22. Maintenance Tools

### 22.1 `repair_causal_relationships`

Repair corrupted causal relationships by removing entries that fail deserialization. Scans `CF_CAUSAL_RELATIONSHIPS` and deletes truncated or corrupted entries.

**Side effects:** Deletes corrupted entries from RocksDB `CF_CAUSAL_RELATIONSHIPS` column family.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| *(none)* | -- | -- | -- | -- | No parameters |

**Return:** JSON object with `deleted_count` and `total_scanned` statistics.

---

## 23. Provenance Tools

### 23.1 `get_audit_trail`

Query the audit log for a specific memory or time range. Returns chronological audit records.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `target_id` | string (UUID) | No | -- | Valid UUID | Memory UUID to get audit trail for |
| `start_time` | string | No | -- | ISO 8601 timestamp | Time range start |
| `end_time` | string | No | -- | ISO 8601 timestamp | Time range end |
| `limit` | integer | No | `50` | Max `500` | Maximum records to return |

**Return:** JSON object with chronological audit records showing operations (create, merge, delete, boost, etc.).

### 23.2 `get_merge_history`

Show merge lineage and history for a fingerprint.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memory_id` | string (UUID) | Yes | -- | Valid UUID | Memory to get merge history for |
| `include_source_metadata` | boolean | No | `false` | -- | Include source metadata for merged sources |

**Return:** JSON object with merge operations that created or affected this memory, including source IDs, strategy, and operator.

### 23.3 `get_provenance_chain`

Full provenance chain from embedding to source for a memory.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memory_id` | string (UUID) | Yes | -- | Valid UUID | Memory to trace provenance for |
| `include_audit` | boolean | No | `false` | -- | Include audit trail |
| `include_embedding_version` | boolean | No | `false` | -- | Include embedding version info |
| `include_importance_history` | boolean | No | `false` | -- | Include importance change history |
| `include_merge_history` | boolean | No | `false` | -- | Include merge lineage history |
| `depth_full` | boolean | No | `false` | -- | Include all provenance data (audit, embedding, importance, merge) |

**Return:** JSON object with source type, file path, chunk info, operator attribution, causal direction, and creation timestamps. Optionally includes audit, embedding version, importance history, and merge lineage.

---

## 24. Daemon Tools

### 24.1 `daemon_status`

Returns the daemon's health metrics for multi-agent observability (handlers/tools/daemon_tools.rs).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| *(none)* | -- | -- | -- | -- | No parameters |

**Return (TCP daemon mode):** JSON object with:
- `pid` (integer)
- `uptime_secs` (integer)
- `active_connections` (integer)
- `max_connections` (integer)
- `models_state` (object)
- `background_tasks` (object): `{ running: bool, graph_builder: bool }`
- `autoConsolidation` (object): `{ running, lastRunEpochSecs, lastCandidatesFound, lastMergesExecuted, totalMerges, totalCycles }` — populated from `AutoConsolidationStatus` per daemon_tools.rs:114

**Return (stdio mode):** Reduced payload with `pid`, `mode: "stdio"`, a `note` string, and `background_tasks.graph_builder` only. Uptime, connection counts, and auto-consolidation metrics are omitted.

---

## 25. Training Tools

Four tools that materialize fully-labeled training records into the `CF_TRAINING_RECORDS` column family. Schemas: `crates/context-graph-mcp/src/tools/definitions/training.rs:14-174`. Handlers: `crates/context-graph-mcp/src/handlers/tools/training_tools.rs`. Name constants: `crates/context-graph-mcp/src/tools/names.rs:144-150`. Dispatch: `handlers/tools/dispatch.rs:167-171`.

All four handlers downcast `Handlers::teleological_store` to `RocksDbTeleologicalStore`; other backends return the error `"<tool> requires RocksDbTeleologicalStore."`.

Output is RocksDB-only: the CF stores `version_byte + bincode(TrainingRecord)`. No JSON / JSONL / Parquet files are written.

### 25.1 `export_training_corpus`

Iterate stored fingerprints, assemble a `TrainingRecord` per memory (13 embeddings, 13D topic profile, 78 synergy-weighted cross-correlations, 6D group alignments, outgoing typed edges, per-embedder K-NN neighbors, Phase-5 temporal labels, LLM-discovered causal labels, and optionally a Tucker-1 core), and persist each one to `CF_TRAINING_RECORDS`.

**Side effects:** Writes to `CF_TRAINING_RECORDS`. Optionally deletes every existing row in that CF before export (`clearExisting=true`). Reads `CF_TOPIC_PROFILES`, `CF_TYPED_EDGES` (including a one-time full reverse-index sweep when `includeIncomingEdges=true`), and `CF_SOURCE_METADATA`.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `filter` | string | No | `"all"` | `"all"`, `"session"` | `"session"` requires `filterId` |
| `filterId` | string | No | -- | -- | Session ID when `filter="session"`; ignored otherwise |
| `maxMemories` | integer | No | `10000` | `1-1000000` | Hard cap on memories processed per call |
| `includeEmbeddings` | boolean | No | `true` | -- | Include E1/E2/E3/E4/E5/E7/E8/E9/E10/E11 dense vectors (~46KB/record) |
| `includeSparseVectors` | boolean | No | `true` | -- | Include E6 + E13 sparse index/value arrays |
| `includeTokenEmbeddings` | boolean | No | `false` | -- | Include E12 ColBERT token embeddings (~15KB/record) |
| `includeEdges` | boolean | No | `true` | -- | Include outgoing typed edges + per-embedder K-NN neighbors |
| `includeCausalLabels` | boolean | No | `true` | -- | Include `causal_effects` (`causal_causes` stays empty in v1) |
| `includeIncomingEdges` | boolean | No | `false` | -- | Include incoming typed edges; triggers one O(N) reverse-index sweep at export start |
| `includeTemporalLabels` | boolean | No | `true` | -- | Include Phase-5 temporal labels |
| `includeTuckerCore` | boolean | No | `false` | -- | Include Tucker-1 decomposition of the 13x13x1024 interaction tensor at ranks (4, 4, 128); adds ~18KB/record |
| `clearExisting` | boolean | No | `false` | -- | Delete all rows in `CF_TRAINING_RECORDS` before export |

`additionalProperties: false`.

**Return:** `{ status: "exported", exported, skipped_filtered, errors, cleared_before_export, fingerprints_scanned, approx_bytes_written, elapsed_ms, storage: { backend: "rocksdb", column_family: "training_records", format: "version_byte + bincode" }, record_shape: {...}, filter: { mode, filter_id } }` (handler: `training_tools.rs:283-317`).

**Error conditions:** Backend is not `RocksDbTeleologicalStore`; `filter="session"` with empty `filterId`; `list_fingerprints_unbiased` failure; `clear_all_training_records` failure. Per-record build failures are counted into `errors` and the export continues.

### 25.2 `list_training_records`

List UUIDs currently in `CF_TRAINING_RECORDS` with optional per-record shape statistics. Shape mode batches RocksDB reads via `multi_get_training_records`.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `limit` | integer | No | `50` | `1-1000` | Max UUIDs to return |
| `offset` | integer | No | `0` | `>= 0` | Pagination offset |
| `includeShape` | boolean | No | `true` | -- | Include per-record shape summary (content chars, importance, session/source, embedding vector lengths, edge counts, K-NN populated count, causal label counts, `temporal_labels_present` flag). Adds one batched read per page. |

**Return:** `{ total, offset, limit, returned, records: [...] }`. Each record with `includeShape=true` contains: `memory_id`, `content_chars`, `importance`, `session_id`, `source_type`, `topic_profile_len`, `cross_correlations_len`, `group_alignments_len`, `e1_len`, `e5_cause_len`, `e7_len`, `e8_source_len`, `e11_len`, `e6_sparse_nnz`, `e13_sparse_nnz`, `e12_token_count`, `outgoing_edges`, `incoming_edges`, `knn_embedders_populated`, `causal_effects`, `causal_causes`, `temporal_labels_present` (handler: `training_tools.rs:589-649`).

### 25.3 `get_training_record`

Fetch a single `TrainingRecord` by memory UUID. Topic profile, cross-correlations, group alignments, edge / K-NN summaries, and causal labels are always returned; raw vectors and token embeddings are opt-in.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `memoryId` | string (UUID) | Yes | -- | Valid UUID | Memory whose record to fetch |
| `includeVectors` | boolean | No | `false` | -- | Include E1/E2/E3/E4/E5/E7/E8/E9/E10/E11 dense arrays and E6/E13 sparse arrays |
| `includeTokenEmbeddings` | boolean | No | `false` | -- | Include E12 token embeddings |
| `includeEdges` | boolean | No | `true` | -- | Include outgoing/incoming typed edges and K-NN neighbors |

**Return:** Full record JSON when found; `{ memory_id, found: false }` when absent (handler: `training_tools.rs:687-694`).

**Error conditions:** Missing/invalid `memoryId`; backend is not RocksDB; point-get failure.

### 25.4 `count_training_records`

Return the current number of rows in `CF_TRAINING_RECORDS`. Implementation is an O(N) scan via `count_training_records` on the RocksDB store (handler: `training_tools.rs:698-713`).

**Side effects:** None (read-only).

**Parameters:** None (`additionalProperties: false`).

**Return:** `{ count: integer }`.

---

## 26. Constellation Tools

Five tools that compile / read / score / delete "constellation" records (per-embedder centroids plus spread statistics over a caller-selected set of memories). Schemas: `crates/context-graph-mcp/src/tools/definitions/constellation.rs:19-187`. Handlers: `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs`. Name constants: `tools/names.rs:155-164`. Dispatch: `handlers/tools/dispatch.rs:173-177`.

All five handlers require `RocksDbTeleologicalStore`; other backends return `"<tool> requires RocksDbTeleologicalStore"`. Records persist in `CF_CONSTELLATIONS` with a selector-hash secondary index.

### 26.1 `compile_constellation`

Resolve members from a selector, stream them into a `ConstellationAccumulator`, and persist the finalized `Constellation` to `CF_CONSTELLATIONS` + its selector index.

**Side effects:** Writes one row to `CF_CONSTELLATIONS` and one selector-index entry. Reads `CF_TOPIC_PROFILES` per member (hard failure on RocksDB read errors — see `constellation_tools.rs:128-141`). For `selector="topic"`, reads the latest topic portfolio to compute `purity`.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `selector` | string | Yes | -- | `"topic"`, `"session"`, `"tag"`, `"time_range"`, `"explicit_ids"` | Member resolution strategy |
| `label` | string | Yes | -- | 1-256 chars | Human-readable tag |
| `topicId` | string | Conditional | -- | -- | Required when `selector="topic"` |
| `sessionId` | string | Conditional | -- | -- | Required when `selector="session"` |
| `tag` | string | Conditional | -- | -- | Required when `selector="tag"`. **Note:** the handler matches `tag` against `SourceMetadata.tool_name` (`constellation_tools.rs:469-474`) — the only user-controllable tag-like field today. |
| `startIso` | string (RFC-3339) | Conditional | -- | -- | Required when `selector="time_range"`. Inclusive lower bound on `fingerprint.created_at` |
| `endIso` | string (RFC-3339) | Conditional | -- | -- | Required when `selector="time_range"`. Inclusive upper bound |
| `memoryIds` | array of UUID | Conditional | -- | >=1 entry | Required when `selector="explicit_ids"` |
| `rationale` | string | No | -- | -- | Annotation for `explicit_ids`; not part of the selector hash |
| `maxMembers` | integer | No | `50000` | `3-100000` | Cap on accumulator members |
| `rebuildIfExists` | boolean | No | `false` | -- | Recompile even if a constellation already exists for this selector |

`additionalProperties: false`.

**Return:** On success, `{ status: "compiled", constellation_id, label, selector_kind, member_count, coherence, purity, per_embedder_coverage: [f32; 13] }`. When a matching selector already exists and `rebuildIfExists=false`: `{ constellation_id, already_exists: true, message }` (handler: `constellation_tools.rs:81-201`).

**Error conditions:** Missing `selector` / `label`; backend is not RocksDB; unknown selector kind; `time_range` with `endIso < startIso`; empty `memoryIds`; invalid UUID; selector resolves to zero members; `get_topic_profile` or `observe` failure; `TooFewMembers` (< 3 members).

### 26.2 `list_constellations`

Paginated UUID listing with optional per-record shape summary via `multi_get_constellations`.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `limit` | integer | No | `50` | `1-1000` | Max UUIDs per page |
| `offset` | integer | No | `0` | `>= 0` | Pagination offset |
| `includeCentroids` | boolean | No | `false` | -- | Include per-embedder `centroid`, `sparse_top_terms`, `pooled_token_centroid` arrays; when `false`, only array lengths are returned |

**Return:** `{ total, offset, limit, returned, constellations: [...] }`. Each row includes `constellation_id`, `label`, `created_at`, `selector_kind`, `selector`, `member_count`, `coherence`, `purity`, and per-embedder stat blocks (`embedder_index`, `dimension`, `vector_kind`, `mean_l2`, `stddev_l2`, `cosine_spread_p50`, `cosine_spread_p95`, `min_cosine`, `max_cosine`, `coverage`, `mean_token_count`). Rows whose body is missing are returned as `{ constellation_id, missing: true }` (handler: `constellation_tools.rs:205-280`).

### 26.3 `get_constellation`

Point lookup by UUID. `includeCentroids` defaults to `true` (opposite of the list tool).

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `constellationId` | string (UUID) | Yes | -- | Valid UUID | Constellation UUID |
| `includeCentroids` | boolean | No | `true` | -- | Include centroid + `topic_profile_centroid` / `group_alignment_centroid` / `cross_correlation_centroid` arrays and the full `member_ids` list |

**Return:** Full record when found, otherwise `{ constellation_id, found: false }`. When `includeCentroids=false`, the record carries `*_len` / `member_ids_count` fields instead of the full arrays (handler: `constellation_tools.rs:283-320`).

### 26.4 `score_against_constellation`

Fetch the constellation + a candidate memory's fingerprint, then run `score_memory_against_constellation` to compute per-embedder cosine-to-centroid similarities.

**Side effects:** None (read-only). Uses the generic `teleological_store.retrieve()` to fetch the candidate fingerprint (works through any trait-compatible backend, though the constellation lookup still requires RocksDB).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `constellationId` | string (UUID) | Yes | -- | Valid UUID | Target constellation |
| `memoryId` | string (UUID) | Yes | -- | Valid UUID | Candidate memory (must exist in `CF_FINGERPRINTS`) |

**Return:** `{ constellation_id, memory_id, per_embedder_cosine: [f32; 13], combined_score, in_spread_p95 }`. `combined_score` is the unweighted mean over embedders with `coverage > 0`; `in_spread_p95` is `true` iff the candidate is at least as central as the constellation's 95th-percentile E1 member (handler: `constellation_tools.rs:323-387`).

**Error conditions:** Missing / invalid `constellationId` or `memoryId`; constellation not found; memory not found in the fingerprint store; backend is not RocksDB.

### 26.5 `delete_constellation`

Remove a constellation and its selector-index entry. Idempotent.

**Side effects:** Deletes from `CF_CONSTELLATIONS` + selector secondary index.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `constellationId` | string (UUID) | Yes | -- | Valid UUID | Constellation to delete |

**Return:** `{ constellation_id, deleted: bool }` — `deleted=false` when no record existed (handler: `constellation_tools.rs:390-422`).

---

## 27. Contrastive Pair Tools

Four tools that mine, list, read, and count cross-embedder anomaly pairs stored in `CF_CONTRASTIVE_PAIRS` (plus `CF_CONTRASTIVE_BY_ANCHOR` and `CF_CONTRASTIVE_BY_KIND` secondary indexes). Schemas: `crates/context-graph-mcp/src/tools/definitions/contrastive.rs:18-196`. Handlers: `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs`. Name constants: `tools/names.rs:169-176`. Dispatch: `handlers/tools/dispatch.rs:179-182`.

All four handlers require `RocksDbTeleologicalStore`.

The six `AnomalyKind` values used by `kind` / `kinds` filters: `semantic_but_not_causal`, `keyword_but_not_paraphrase`, `code_shape_but_different_intent`, `entity_shared_but_different_structure`, `hdc_robust_but_semantic_different`, `other`.

### 27.1 `mine_contrastive_pairs`

Scan anchors from an unbiased fingerprint sample, score a bounded candidate pool with the full 13-slot similarity profile, classify each qualifying pair into an `AnomalyKind`, and persist the `(anchor, negative, similarity_profile, kind)` triple. Idempotent on the composite `(anchorId, negativeId)` key.

The implementation reproduces the `search_cross_embedder_anomalies` scoring step in-process rather than re-entering the search pipeline (see the "Design note" comment at `contrastive_tools.rs:14-37`).

**Side effects:** Writes to `CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_ANCHOR`, and `CF_CONTRASTIVE_BY_KIND`. Reads `list_fingerprints_unbiased` once, `get_content_batch` once (pool_len IDs), and conditionally `get_source_metadata_batch` when `sessionFilter` is set.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `maxPairs` | integer | No | `1000` | `1-100000` | Hard cap on pairs persisted this run |
| `minDisagreement` | number | No | `0.3` | `[0.0, 1.0]` | Minimum `max(high_sim) - min(low_sim)` to keep a pair |
| `highThreshold` | number | No | `0.6` | `[0.0, 1.0]` | Similarity >= this classifies an embedder as "high" |
| `lowThreshold` | number | No | `0.3` | `[0.0, 1.0]` | Similarity <= this classifies an embedder as "low"; must be `< highThreshold` |
| `kinds` | array of string | No | -- | See `AnomalyKind` list above | Only persist pairs whose classified kind is in this list |
| `sessionFilter` | string | No | -- | -- | Only scan anchors whose `source_metadata.session_id` matches |
| `topKCandidatesPerAnchor` | integer | No | `10` | `1-500` | Size of per-anchor candidate pool retained after disagreement sort |
| `candidatePoolSize` | integer | No | `500` | `10-50000` | Max peers sampled from the store per run (one shared scan) |

`additionalProperties: false`.

**Return:** `{ status: "mined" | "no-op", summary: { pairs_stored, pairs_skipped_below_threshold, pairs_skipped_kind_filter, anchors_scanned, anchors_no_candidate, duration_ms }, config: {...} }`. `status: "no-op"` when the candidate pool has fewer than 2 fingerprints (handler: `contrastive_tools.rs:70-397`).

**Error conditions:** Backend is not RocksDB; `kinds` not an array or contains an unknown kind; `MiningConfig::validate` failure (e.g. `lowThreshold >= highThreshold`); `list_fingerprints_unbiased` failure. Per-pair `store_contrastive_pair` failures are logged and the mining loop continues.

### 27.2 `list_contrastive_pairs`

Paginated `(anchorId, negativeId)` listing with optional kind and/or anchor filter. `includeFull=true` fetches the full record for each row.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `limit` | integer | No | `100` | `1-10000` | Max pairs to return |
| `offset` | integer | No | `0` | `>= 0` | Pagination offset |
| `kind` | string | No | -- | `AnomalyKind` enum | Filter by anomaly kind (uses `CF_CONTRASTIVE_BY_KIND` prefix scan) |
| `anchorId` | string (UUID) | No | -- | Valid UUID | Filter by anchor (uses `CF_CONTRASTIVE_BY_ANCHOR`) |
| `includeFull` | boolean | No | `false` | -- | Fetch the full `ContrastivePair` record for each key |

Routing by filter combination (handler: `contrastive_tools.rs:446-498`):
- `kind + anchorId`: walk the anchor's negatives and filter in-process by kind.
- `kind` only: prefix-scan by kind (over-fetches `offset+limit` and applies paging post-hoc).
- `anchorId` only: `pairs_for_anchor`.
- Neither: `list_contrastive_pair_keys`.

**Return:** `{ total_matching, returned, offset, limit, include_full, kind_filter, anchor_filter, pairs: [...] }`. When `includeFull=false`, each pair is `{ anchor_id, negative_id }`; when `true`, the full record (see §27.3).

### 27.3 `get_contrastive_pair`

Composite-key point lookup by `(anchorId, negativeId)`.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `anchorId` | string (UUID) | Yes | -- | Valid UUID | Anchor memory UUID |
| `negativeId` | string (UUID) | Yes | -- | Valid UUID | Negative memory UUID |

**Return:** Full `ContrastivePair`: `{ found: true, anchor_id, negative_id, anchor_text, negative_text, similarity_profile: [f32; 13], high_embedders, low_embedders, disagreement_magnitude, anomaly_kind, anomaly_kind_byte, mined_at, generator }` or `{ found: false, anchor_id, negative_id }` (handler: `contrastive_tools.rs:550-595`).

### 27.4 `count_contrastive_pairs`

Total or per-kind row count.

**Side effects:** None (read-only).

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `kind` | string | No | -- | `AnomalyKind` enum | Restrict count to one kind via `CF_CONTRASTIVE_BY_KIND` prefix scan |

**Return:** `{ count }` when `kind` is absent; `{ count, kind }` when `kind` is supplied (handler: `contrastive_tools.rs:598-643`).

---

## 28. Typed-Edge Training Tools

Source: `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs`, `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs`

These tools implement the Phase 4 typed-edge training-data factory: F1 exports a labeled corpus of typed edges with per-embedder similarity profiles, F2 derives anomaly pairs from edge scores, F4 validates low-confidence edges via the local LLM, and a listing tool queries the exported corpus.

### 28.1 `export_typed_edges_corpus`

Export typed edges as training records to `CF_TYPED_EDGE_RECORDS`. Each record captures per-embedder similarity profiles, content, source metadata, and optionally joins LLM validation verdicts from `CF_TYPED_EDGE_VALIDATIONS`.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `maxEdges` | integer | No | `10000` | 1 -- 1,000,000 | Maximum edges to export |
| `includeContent` | boolean | No | `true` | -- | Include source/target memory content |
| `includeSourceMetadata` | boolean | No | `true` | -- | Include session_id, source_type, target_type |
| `includeMechanismType` | boolean | No | `true` | -- | For CausalChain edges, join mechanism_type from `CF_CAUSAL_RELATIONSHIPS` |
| `joinLLMValidation` | boolean | No | `true` | -- | Join `LLMValidationSummary` from `CF_TYPED_EDGE_VALIDATIONS` if available |
| `clearExisting` | boolean | No | `false` | -- | Delete all existing rows in `CF_TYPED_EDGE_RECORDS` before export |

**Side effects:** Writes to `CF_TYPED_EDGE_RECORDS`. Reads `CF_TYPED_EDGES`, `CF_CONTENT`, `CF_SOURCE_METADATA`, `CF_CAUSAL_RELATIONSHIPS`, `CF_TYPED_EDGE_VALIDATIONS`.

**Return:** `{ status, exportedCount, errors, clearedCount, duration_ms }` with storage metadata.

### 28.2 `derive_anomalies_from_edges`

Derive contrastive anomaly pairs directly from typed-edge embedder scores without re-scanning the full memory corpus. Produces 5 of 6 `AnomalyKind` variants (Other requires full offline mining). Results stored in `CF_CONTRASTIVE_PAIRS` + secondary indexes.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `highThreshold` | number | No | `0.6` | 0.0 -- 1.0 | Similarity >= this counts as "high" |
| `lowThreshold` | number | No | `0.3` | 0.0 -- 1.0 | Similarity <= this counts as "low" |
| `minDisagreement` | number | No | `0.3` | 0.0 -- 1.0 | Minimum high - low gap to keep a pair |
| `maxPairs` | integer | No | `10000` | 1 -- 1,000,000 | Maximum anomaly pairs to derive |

**Side effects:** Writes to `CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`. Reads `CF_TYPED_EDGES`, `CF_CONTENT`.

**Return:** `AnomalyDerivationSummary` with `{ edges_scanned, pairs_written, skipped_below_threshold, skipped_missing_content, per_kind_counts, duration_ms }`.

### 28.3 `validate_low_confidence_edges`

Send low-confidence typed edges to the local LLM for validation. The LLM produces a verdict (Valid / Invalid / Reclassify) with confidence and rationale. Results stored in `CF_TYPED_EDGE_VALIDATIONS`. LLM-gated: returns `{status: "skipped"}` when LLM is not loaded.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `weightCeiling` | number | No | `0.60` | 0.0 -- 1.0 | Only validate edges with weight < this threshold |
| `maxEdges` | integer | No | `100` | 1 -- 10,000 | Maximum edges to validate per invocation |
| `sessionFilter` | string | No | -- | -- | Restrict to edges involving memories from this session |

**Side effects:** Writes to `CF_TYPED_EDGE_VALIDATIONS`. Reads `CF_TYPED_EDGES`, `CF_CONTENT`. Invokes LLM inference (Hermes-2-Pro-Mistral-7B) per edge.

**Return:** `{ validatedCount, alreadyHadValidation, errors, duration_ms }`. Idempotent: edges with existing validation are skipped.

### 28.4 `list_typed_edge_records`

Paginated listing of exported typed-edge training records from `CF_TYPED_EDGE_RECORDS`.

| Parameter | Type | Required | Default | Valid Values | Description |
|-----------|------|----------|---------|--------------|-------------|
| `limit` | integer | No | `100` | 1 -- 1,000 | Maximum records to return |
| `offset` | integer | No | `0` | >= 0 | Skip this many records (pagination) |
| `includeFull` | boolean | No | `false` | -- | Return full `TypedEdgeTrainingRecord` payloads (true) or keys only (false) |

**Return:** `{ total, offset, limit, returned, records }` with keys `(source_id, target_id, edge_type)` and optionally full record payloads.

---

## Appendix A: Tool Count Summary

| Category | Count | LLM-Only |
|----------|-------|----------|
| Core | 4 | 0 |
| Curation (merge + forget + boost) | 3 | 0 |
| Topic | 4 | 0 |
| File Watcher | 4 | 0 |
| Sequence (E4) | 4 | 0 |
| Causal (E5) | 4 | 0 |
| Causal Discovery (E5 LLM) | 2 | 2 |
| Keyword (E6) | 1 | 0 |
| Code (E7) | 1 | 0 |
| Graph (E8) | 2-4 | 2 |
| Robustness (E9) | 1 | 0 |
| Entity (E11) | 6 | 0 |
| Embedder-First | 7 | 0 |
| Token/Expansion (E12/E13) | 2 | 0 |
| Temporal (E2/E3) | 2 | 0 |
| Graph Linking (K-NN) | 4 | 0 |
| Maintenance | 1 | 0 |
| Provenance | 3 | 0 |
| Daemon | 1 | 0 |
| Training (§25) | 4 | 0 |
| Constellation (§26) | 5 | 0 |
| Contrastive (§27) | 4 | 0 |
| Typed-Edge Training (§28) | 4 | 0 |
| **Total** | **71-75** | **4** |

The Graph (E8) row reads "2-4 | 2" because 2 of its 4 tools (`discover_graph_relationships`, `validate_graph_link`) are LLM-only.

## Appendix B: Embedder Quick Reference

| ID | Name | Dimension | Index Type | Description |
|----|------|-----------|------------|-------------|
| E1 | Semantic | 1024D | HNSW | Dense semantic similarity (foundation) |
| E2 | Temporal Recency | 512D | HNSW | Sinusoidal PE of Unix timestamp |
| E3 | Temporal Periodic | 512D | HNSW | Time-of-day / day-of-week patterns |
| E4 | Temporal Positional | 512D | HNSW | Session sequence ordering |
| E5 | Causal | 768D | HNSW | Asymmetric cause/effect (dual vectors) |
| E6 | Keyword | 30K sparse | Inverted | Exact keyword matching |
| E7 | Code | 1536D | HNSW | Code patterns via Qodo-Embed |
| E8 | Graph | 1024D | HNSW | Asymmetric source/target (dual vectors) |
| E9 | Robustness | 1024D | HNSW | HDC character trigram hypervectors |
| E10 | Paraphrase | 768D | HNSW | Asymmetric doc/query (dual vectors) |
| E11 | Entity | 768D | HNSW | KEPLER/TransE entity embeddings |
| E12 | ColBERT | 128D/token | MaxSim | Token-level late interaction |
| E13 | SPLADE | 30K sparse | Inverted | Learned term expansion |

Asymmetric embedders (E5, E8, E10) store dual vectors. E6, E12, E13 use non-HNSW indexes and do not support `search_by_embedder` or `get_embedder_clusters`.
