# MCP Tools Reference

This document describes every MCP tool the ContextGraph server exposes. The server speaks JSON-RPC 2.0 over stdio (default) or TCP. All tools live under the `tools/list` and `tools/call` JSON-RPC methods.

**Source-of-truth files:**
- `crates/context-graph-mcp/src/tools/mod.rs` — registry assembly
- `crates/context-graph-mcp/src/tools/names.rs` — canonical name constants for dispatch
- `crates/context-graph-mcp/src/tools/aliases.rs` — alias resolution
- `crates/context-graph-mcp/src/tools/types.rs` — `ToolDefinition` shape
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` — registry assembly + count assertions
- `crates/context-graph-mcp/src/tools/definitions/{capability,core,merge,curation,topic,file_watcher,sequence,causal,causal_discovery,keyword,code,graph,robustness,entity,embedder,temporal,graph_link,maintenance,provenance,daemon,training,learning,utl,constellation,contrastive,typed_edges,dynamicjepa,reality}.rs`
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` — the `tool_dispatch!` macro routing
- `crates/context-graph-mcp/src/handlers/tools/*_dtos.rs` — argument shapes for non-trivial handlers
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/{alter,analyze,interact,optimizer,autoresearch,helpers,path_policy,shift_log,schema_validator,lock,errors}.rs` — ccreality reality-loop handler family

**Related docs:**
- [05_embedding_subsystem.md](05_embedding_subsystem.md) — embedder identifiers (E1–E14, E15–E21)
- [06_search_and_fusion.md](06_search_and_fusion.md) — search strategies and weight profiles
- [07_graph_and_causal.md](07_graph_and_causal.md) — typed edges, K-NN, causal direction
- [09_storage_layer.md](09_storage_layer.md) — RocksDB column families written by these tools
- [17_constellation_subsystem.md](17_constellation_subsystem.md) — constellation semantics
- [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) — AnomalyKind taxonomy
- [20_learning_subsystem.md](20_learning_subsystem.md) — Learning-as-UTL
- [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md) — DynamicJEPA (5090jepa) MCP surface
- [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md) — ccreality reality-loop subsystem

---

## 1. Tool registry overview

`get_tool_definitions()` (in `crates/context-graph-mcp/src/tools/definitions/mod.rs:38`) aggregates per-category modules into one `Vec<ToolDefinition>`. The test `tools::definitions::tests::test_total_tool_count_and_no_duplicates` pins the totals:

| Build feature flag | Total tools |
| --- | --- |
| `--features llm` (default) | **145** |
| Without `llm` | **141** |

When the server is launched with `--mode reality-loop` (`tools::is_reality_loop_mode()` returns true), the registry additionally includes the 35 ccreality reality-loop tools defined in `definitions/reality.rs`. The lightweight reality-loop stdio server (`get_reality_loop_tool_definitions()`) only exposes that subset.

LLM-gated tools (require `--features llm`):
- `discover_graph_relationships`, `validate_graph_link` — LLM-based graph relationship discovery
- `trigger_causal_discovery`, `get_causal_discovery_status` — Qwen2.5 causal discovery
- `validate_low_confidence_edges` exists in both builds, but returns `{status:"skipped"}` when LLM disabled

### 1.1 Tool count by category

The following counts come from `tests::test_submodule_counts` in `definitions/mod.rs:172-210`. Reality-loop tools (`definitions/reality.rs`) only appear when `is_reality_loop_mode()` is true.

| Category | Count (with llm) | Count (without llm) | Definition file |
| --- | --- | --- | --- |
| capability | 1 | 1 | `capability.rs` |
| core | 5 | 5 | `core.rs` |
| merge | 1 | 1 | `merge.rs` |
| curation | 2 | 2 | `curation.rs` |
| topic | 4 | 4 | `topic.rs` |
| file_watcher | 4 | 4 | `file_watcher.rs` |
| sequence | 4 | 4 | `sequence.rs` |
| causal | 4 | 4 | `causal.rs` |
| causal_discovery | 2 | 0 | `causal_discovery.rs` |
| keyword | 1 | 1 | `keyword.rs` |
| code | 1 | 1 | `code.rs` |
| graph | 4 | 2 | `graph.rs` |
| robustness | 1 | 1 | `robustness.rs` |
| entity | 6 | 6 | `entity.rs` |
| embedder (cross-cutting) | 7 | 7 | `embedder.rs::definitions()` |
| embedder (E12/E13 standalone) | 2 | 2 | `embedder.rs::standalone_definitions()` |
| temporal | 2 | 2 | `temporal.rs` |
| graph_link | 4 | 4 | `graph_link.rs` |
| maintenance | 1 | 1 | `maintenance.rs` |
| provenance | 3 | 3 | `provenance.rs` |
| daemon | 1 | 1 | `daemon.rs` |
| training | 4 | 4 | `training.rs` |
| learning (Learning-as-UTL) | 14 | 14 | `learning.rs` |
| utl (learner-state) | 18 | 18 | `utl.rs` |
| constellation | 6 | 6 | `constellation.rs` |
| contrastive | 4 | 4 | `contrastive.rs` |
| typed_edges | 4 | 4 | `typed_edges.rs` |
| dynamicjepa | 35 | 35 | `dynamicjepa.rs` |
| reality (only `--mode reality-loop`) | 35 | 35 | `reality.rs` |
| **Total (default mode)** | **145** | **141** | |
| **Total (reality-loop mode)** | **180** | **176** | |

### 1.2 Aliases

Defined in `crates/context-graph-mcp/src/tools/aliases.rs`. `resolve_alias()` runs in `dispatch.rs:73` for every `tools/call`; unknown names pass through unchanged.

| Legacy name | Canonical name | Notes |
| --- | --- | --- |
| `consolidate_memories` | `trigger_consolidation` | renamed for clarity |
| `inject_context` | `store_memory` | merged tools — `rationale` field travels through |

### 1.3 ToolDefinition shape

`crates/context-graph-mcp/src/tools/types.rs`:

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value, // serialized as "inputSchema"
    pub output_schema: Option<serde_json::Value>, // optional "outputSchema"
}
```

Every schema sets `additionalProperties: false` — unknown params are rejected with `INVALID_PARAMS`. Schema integer fields use JSON `integer` (not `number`) to enforce type at the JSON-RPC boundary (e.g., `lastHours`, `lastDays` on `search_graph`).

### 1.4 Schema↔DTO alignment

Each `definitions/<group>.rs` schema is paired with a DTO module under `handlers/tools/<group>_dtos.rs`. Defaults specified in schemas (`default: …`) MUST match handler defaults. The `validate.rs` helpers at `handlers/tools/validate.rs` provide shared `additionalProperties:false` enforcement and numeric clamping utilities.

---

## 2. Protocol and dispatch

Source: `crates/context-graph-mcp/src/protocol.rs` and `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`. The MCP server speaks JSON-RPC 2.0 with `id` of type `Number(i64)` or `String`.

| Method | Handler | Behavior |
| --- | --- | --- |
| `initialize` | `handle_initialize` | MCP session metadata + capabilities |
| `shutdown` | `handle_shutdown` | Triggers `McpServer::shutdown()` graceful path |
| `tools/list` | `handle_tools_list` (`dispatch.rs:40`) | Returns `ToolDefinition[]` |
| `tools/call` | `handle_tools_call` (`dispatch.rs:46`) | Resolves alias, dispatches via `tool_dispatch!` macro |

Per-tool timeout: `config.mcp.request_timeout` is enforced via `tokio::time::timeout` for every request and every batch sub-request. On timeout the server emits `LAYER_TIMEOUT` (-32007) with the original `request.id` (id is parsed BEFORE consuming the payload). Notifications (no `id` field) NEVER receive a response — even on timeout.

Transport modes (`crates/context-graph-mcp/src/server/mod.rs::TransportMode`):
- **Stdio** (default): newline-delimited JSON over stdin/stdout. Supports JSON-RPC batch arrays per spec §6.
- **TCP**: bound on `config.mcp.bind_address:tcp_port`, default `127.0.0.1:3100`. TCP-specific error codes `-32110…-32114`.
- **Daemon mode** (`--daemon`): TCP daemon shared across stdio clients via stdio-to-TCP proxy. Lock file at `/tmp/context-graph-mcp-daemon.lock`.
- **Reality-loop mode** (`--mode reality-loop`): lightweight stdio server that exposes only the 35 `mcp__cgreality__*` tools. Acquires a flock at `~/.cache/cgreality/reality-loop-stdio.lock` to evict stale prior MCPs (SIGTERM 2s → SIGKILL).

Standard error codes:

| Code | Name | Meaning |
| --- | --- | --- |
| -32700 | `PARSE_ERROR` | Malformed JSON line |
| -32600 | `INVALID_REQUEST` | Bad envelope, oversized batch, empty batch |
| -32601 | `TOOL_NOT_FOUND` | Name does not match any dispatch entry |
| -32602 | `INVALID_PARAMS` | Missing/unknown params, schema violation |
| -32007 | `LAYER_TIMEOUT` | Handler exceeded `request_timeout` |

ccreality tools also emit a structured per-error body with `error_code` (e.g., `CCREALITY_OFFICIAL_REPORT_MISSING`), `field_path`, `message`, `source_of_truth`, and `details`. See `handlers/tools/reality_loop/errors.rs`.

---

## 3. Tool inventory (navigation table)

Quick lookup for every tool, its category, and the parameters it requires. Optional parameters are not listed here — see the per-tool sections below.

| Tool | Category | Required params |
| --- | --- | --- |
| `get_capability_matrix` | capability | — |
| `store_memory` | core | `content` |
| `store_memories` | core | `items` |
| `get_memetic_status` | core | — |
| `search_graph` | core | `query` |
| `trigger_consolidation` | core | — |
| `merge_concepts` | merge | `source_ids`, `target_name`, `rationale` |
| `forget_concept` | curation | `node_id` |
| `boost_importance` | curation | `node_id`, `delta` |
| `get_topic_portfolio` | topic | — |
| `get_topic_stability` | topic | — |
| `detect_topics` | topic | — |
| `get_divergence_alerts` | topic | — |
| `list_watched_files` | file_watcher | — |
| `get_file_watcher_stats` | file_watcher | — |
| `delete_file_content` | file_watcher | `file_path` |
| `reconcile_files` | file_watcher | — |
| `get_conversation_context` | sequence | — |
| `get_session_timeline` | sequence | — |
| `traverse_memory_chain` | sequence | `anchorId` |
| `compare_session_states` | sequence | `beforeSequence`, `afterSequence` |
| `search_causal_relationships` | causal | `query` |
| `search_causes` | causal | `query` |
| `search_effects` | causal | `query` |
| `get_causal_chain` | causal | `anchorId` |
| `trigger_causal_discovery` | causal_discovery (llm) | — |
| `get_causal_discovery_status` | causal_discovery (llm) | — |
| `search_by_keywords` | keyword | `query` |
| `search_code` | code | `query` |
| `search_connections` | graph | `query` |
| `get_graph_path` | graph | `anchorId` |
| `discover_graph_relationships` | graph (llm) | `memory_ids` |
| `validate_graph_link` | graph (llm) | `source_id`, `target_id` |
| `search_robust` | robustness | `query` |
| `extract_entities` | entity | `text` |
| `search_by_entities` | entity | `entities` |
| `infer_relationship` | entity | `headEntity`, `tailEntity` |
| `find_related_entities` | entity | `entity`, `relation` |
| `validate_knowledge` | entity | `subject`, `predicate`, `object` |
| `get_entity_graph` | entity | — |
| `search_by_embedder` | embedder | `embedder`, `query` |
| `get_embedder_clusters` | embedder | `embedder` |
| `compare_embedder_views` | embedder | `query`, `embedders` |
| `list_embedder_indexes` | embedder | — |
| `get_memory_fingerprint` | embedder | `memoryId` |
| `create_weight_profile` | embedder | `name`, `weights` |
| `search_cross_embedder_anomalies` | embedder | `query`, `highEmbedder`, `lowEmbedder` |
| `search_by_tokens` | embedder (E12 standalone) | `query` |
| `search_by_expansion` | embedder (E13 standalone) | `query` |
| `search_recent` | temporal | `query` |
| `search_periodic` | temporal | `query` |
| `get_memory_neighbors` | graph_link | `memory_id` |
| `get_typed_edges` | graph_link | `memory_id` |
| `traverse_graph` | graph_link | `start_memory_id` |
| `get_unified_neighbors` | graph_link | `memory_id` |
| `record_graph_learning_event` | learning | several |
| `resolve_graph_learning_policy` | learning | `policyScope` |
| `repair_causal_relationships` | maintenance | — |
| `get_audit_trail` | provenance | — |
| `get_merge_history` | provenance | `memory_id` |
| `get_provenance_chain` | provenance | `memory_id` |
| `daemon_status` | daemon | — |
| `export_training_corpus` | training | — |
| `list_training_records` | training | — |
| `get_training_record` | training | `memoryId` |
| `count_training_records` | training | — |
| `record_learning_event` | learning | `before`, `after`, `outcome` |
| `list_learning_events` | learning | — |
| `get_learning_event` | learning | `eventId` |
| `count_learning_events` | learning | — |
| `list_learning_signal_embedders` | learning | — |
| `compute_learning_signals` | learning | `before`, `after`, `outcome` |
| `embed_learning_event_signals` | learning | `eventId` |
| `estimate_learning_outcome` | learning | `before`, `candidateAfter` |
| `export_learner_training_dataset` | learning | — |
| `list_learner_training_datasets` | learning | — |
| `get_learner_training_dataset` | learning | `datasetId` |
| `count_learner_training_datasets` | learning | — |
| `register_learner` | utl | `handle` |
| `record_session_observation` | utl | `learnerId`, `sessionTs` |
| `compute_delta_s` | utl | — |
| `compute_delta_c` | utl | — |
| `compute_delta_e` | utl | — |
| `compute_L` | utl | `components` |
| `get_learner_M` | utl | `learnerId`, `traceId` |
| `next_review_for_trace` | utl | `learnerId`, `traceId` |
| `list_learner_embedders` | utl | — |
| `preflight_learner_assets` | utl | — |
| `count_learner_state` | utl | — |
| `get_learner_state` | utl | `learnerId` |
| `record_learner_k_sleep` | utl | several |
| `record_learner_retrieval` | utl | several |
| `upsert_goal_centroid` | utl | `skillId`, `modality`, `vector` |
| `get_goal_distance` | utl | several |
| `compile_learner_constellation` | utl | `learnerId`, `sessionTsList` |
| `resolve_learner_retrieval_policy` | utl | `learnerId`, `sessionTs` |
| `compile_constellation` | constellation | `selector`, `label` |
| `list_constellations` | constellation | — |
| `get_constellation` | constellation | `constellationId` |
| `score_against_constellation` | constellation | `constellationId`, `memoryId` |
| `derive_constellation` | constellation | `operation`, `sourceConstellationId`, `label` |
| `delete_constellation` | constellation | `constellationId` |
| `mine_contrastive_pairs` | contrastive | — |
| `list_contrastive_pairs` | contrastive | — |
| `get_contrastive_pair` | contrastive | `anchorId`, `negativeId` |
| `count_contrastive_pairs` | contrastive | — |
| `export_typed_edges_corpus` | typed_edges | — |
| `derive_anomalies_from_edges` | typed_edges | — |
| `validate_low_confidence_edges` | typed_edges | — |
| `list_typed_edge_records` | typed_edges | — |
| `dynamicjepa_*` (35 tools) | dynamicjepa | see § 18 |
| `reality_*` / `harness_*` / `optimizer_*` / `champion_state_*` / `experiment_registry_*` (35 tools) | reality | see § 19 |

---

## 4. Capability tool

### `get_capability_matrix`
**Description:** Return the MCP capability matrix: every E1–E14 content embedder, E15–E21 learner-state slots, tool groups, source-of-truth RocksDB CFs, and (optional) live runtime counts.
**Source:** `tools/definitions/capability.rs:11`; handler `handlers/tools/capability_tools.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `includeRuntimeState` | boolean |  | `true` | Read live counts from RocksDB and daemon state. Fails closed when live state cannot be read. |
| `includeToolSchemas` | boolean |  | `false` | Inline every tool's full inputSchema. Off by default to keep responses compact. |

**Output schema:** `{version, mcp, embedders, capabilities, runtime?}`
**Side effects:** none (read-only).

---

## 5. Core tools

### `store_memory`
**Description:** Store a single memory directly into the knowledge graph without UTL processing.
**Source:** `tools/definitions/core.rs:17`; handler `handlers/tools/memory_tools.rs`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `content` | string | yes | — | 1–10000 chars |
| `rationale` | string |  | — | 1–1024 chars; when provided, mirrored in response (legacy `inject_context` alias) |
| `importance` | number |  | `0.5` | clamped to `[0.0, 1.0]` |
| `sessionId` | string |  | env `CLAUDE_SESSION_ID` | scopes the memory to a session |
| `operatorId` | string |  | — | tracked in audit log |

**Side effects:** writes `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_SOURCE_METADATA`, all `CF_EMB_*` indexes, `CF_AUDIT_LOG`. Triggers HNSW insertion across 11 dense embedders + sparse insertion for E6/E13 + token insertion for E12.

### `store_memories`
**Description:** Store a validated batch of memories through the same 14-content-embedder pipeline. Batch is **all-or-error**: if any fingerprint/content/source-metadata/embedding/audit readback fails, every fingerprint stored in this batch is hard-deleted and the tool returns an error.
**Source:** `tools/definitions/core.rs:55`; DTOs `handlers/tools/memory_tools.rs`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `items` | array | yes | — | 1–64 items, each `{content, rationale?, importance?, sessionId?, operatorId?}` |
| `sessionId` | string |  | — | default for items that omit it |
| `operatorId` | string |  | — | default for items that omit it |
| `causalHints` | boolean |  | `false` | When true, ask the LLM causal-hint provider for E5 hints before batch embedding |

**Output schema:** `{requestedCount, storedCount, embedderCount, items[]}` (declared on the tool definition).
**Side effects:** identical to `store_memory` but multiplied by `items.len()`. On rollback, all just-inserted UUIDs are hard-deleted from every CF.

### `get_memetic_status`
**Description:** Return current system status: fingerprint count, number of embedders (14), storage backend, total bytes on disk, layer status from `LayerStatusProvider`.
**Source:** `tools/definitions/core.rs:128`; handler `handlers/tools/status_tools.rs`.

| Param | Type | Req | Default | Description |

| (no parameters) | | | | |

**Side effects:** none (read-only).

### `search_graph`
**Description:** Multi-space semantic search across all 14 embedders. Default profile is `multi_space` weighted RRF; `pipeline` runs E13 recall → E1 dense → E12 rerank; `e1_only` is the fast E1-dense path. Supports temporal weighting (E2/E3/E4), causal direction reranking (E5 asymmetric), session scope, decay function, ColBERT rerank.
**Source:** `tools/definitions/core.rs:140`; DTOs `handlers/tools/memory_tools.rs::SearchGraphRequest`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `topK` | integer |  | `10` | 1–100 |
| `minSimilarity` | number |  | `0.0` | `[0.0, 1.0]` |
| `includeContent` | boolean |  | `false` | inline result content text |
| `strategy` | string |  | `multi_space` | enum: `e1_only`, `multi_space`, `pipeline` |
| `weightProfile` | string |  | profile-dependent | enum (see §5.4 below) |
| `stateConditionedProfile` | boolean |  | `false` | route `weightProfile` through learner-state-conditioned policy (requires `learnerId`+`learnerSessionTs`) |
| `learnerId` | string (uuid) |  | — | required when `stateConditionedProfile=true` |
| `learnerSessionTs` | integer |  | — | persisted learner session timestamp (≥0) |
| `customWeights` | object |  | — | per-embedder weights `E1`..`E14`, each `[0,1]`, must sum to ~1.0 |
| `excludeEmbedders` | array |  | — | embedders to drop from fusion (renormalize remainder) |
| `useQuantizedPrefilter` | boolean |  | `false` | PQ-8 prefilter for fast approximate recall |
| `includeEmbedderBreakdown` | boolean |  | `false` | per-embedder contribution per result |
| `enableRerank` | boolean |  | `false` | E12 ColBERT rerank stage |
| `enableAsymmetricE5` | boolean |  | `true` | apply E5 asymmetric reranking when query looks causal |
| `causalDirection` | string |  | `auto` | enum: `auto`, `cause`, `effect`, `none` |
| `enableQueryExpansion` | boolean |  | `false` | expand causal queries with related terms |
| `temporalWeight` | number |  | `0.0` | post-retrieval temporal boost weight `[0,1]` |
| `conversationContext` | object |  | — | `{anchorToCurrentTurn?, turnsBack?, turnsForward?}` |
| `sessionScope` | string |  | `all` | enum: `current`, `all`, `recent` |
| `decayFunction` | string |  | `exponential` | enum: `linear`, `exponential`, `step`, `none`, `no_decay` |
| `decayHalfLifeSecs` | integer |  | `86400` | half-life for exponential decay |
| `temporalScale` | string |  | `meso` | enum: `micro`, `meso`, `macro`, `long`, `archival` |
| `lastHours` | integer |  | — | hard window filter (≥1) |
| `lastDays` | integer |  | — | hard window filter (≥1) |
| `sessionId` | string |  | — | filter to one session |
| `periodicBoost` | number |  | — | weight for E3 periodic boost |
| `targetHour` | integer |  | — | 0–23, used with `periodicBoost` |
| `targetDayOfWeek` | integer |  | — | 0=Sun…6=Sat, used with `periodicBoost` |
| `sequenceAnchor` | string (uuid) |  | — | E4 anchor memory |
| `sequenceDirection` | string |  | — | enum: `before`, `after`, `around`, `both` |
| `includeProvenance` | boolean |  | `false` | include retrieval provenance metadata |

**Weight profiles** (`weightProfile` enum, 19 values):
`semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`, `multilingual_search`, `long_context`, `translation_finder`, `affect_repair`, `affect_priming`, `affect_neutral`.

**Side effects:** none (read-only). Reads `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_HNSW_*`, `CF_INVERTED_*`, `CF_TOKEN_*`.

### `trigger_consolidation`
**Description:** Analyze memories for consolidation candidates. Returns pairs that **could** be merged but does not auto-merge. Use `merge_concepts` to execute.
**Source:** `tools/definitions/core.rs:376`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `strategy` | string |  | `similarity` | enum: `similarity`, `temporal`, `semantic` |
| `min_similarity` | number |  | `0.925` | normalized cosine threshold `[0,1]` |
| `max_memories` | integer |  | `100` | 1–10000 fingerprints to consider |

**Side effects:** none (read-only — only proposes candidates).

---

## 6. Merge tool

### `merge_concepts`
**Description:** Merge 2–10 concept fingerprints into a single unified node. Returns `reversal_hash` for 30-day undo.
**Source:** `tools/definitions/merge.rs:16`; handler `handlers/tools/memory_tools.rs`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `source_ids` | array | yes | — | 2–10 UUIDs |
| `target_name` | string | yes | — | 1–256 chars |
| `merge_strategy` | string |  | `union` | enum: `union`, `intersection`, `weighted_average` |
| `rationale` | string | yes | — | 1–1024 chars (PRD 0.3 mandate) |
| `force_merge` | boolean |  | `false` | ignore prior conflicts |

**Side effects:** writes a new fingerprint, soft-deletes (status: superseded) the sources, writes `CF_MERGE_HISTORY`, `CF_AUDIT_LOG`. Updates HNSW indexes.

---

## 7. Curation tools

### `forget_concept`
**Description:** Soft-delete a memory with 30-day recovery window (per SEC-06). Set `soft_delete=false` for permanent deletion.
**Source:** `tools/definitions/curation.rs:20`; DTOs `handlers/tools/curation_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `node_id` | string (uuid) | yes | — | memory to forget |
| `soft_delete` | boolean |  | `true` | when false, hard-deletes (irrecoverable) |
| `operator_id` | string |  | — | tracked in audit |
| `reason` | string |  | — | tracked in audit |

**Side effects:** soft path writes `deleted_at` to `CF_SOURCE_METADATA` and appends `CF_AUDIT_LOG`. Hard path removes the fingerprint from every `CF_EMB_*`/`CF_HNSW_*`/sparse/token CF, then deletes from `CF_FINGERPRINTS` (RocksDB-first ordering).

### `boost_importance`
**Description:** Adjust a memory's importance score by `delta`. Final value is clamped to `[0.0, 1.0]` (BR-MCP-002). Response includes old, delta, and new.
**Source:** `tools/definitions/curation.rs:53`; DTOs `handlers/tools/curation_dtos.rs`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `node_id` | string (uuid) | yes | — | memory UUID |
| `delta` | number | yes | — | `[-1.0, 1.0]` |
| `operator_id` | string |  | — | tracked in audit |

**Side effects:** updates `CF_FINGERPRINTS::importance`, appends `CF_AUDIT_LOG`.

---

## 8. Topic tools

### `get_topic_portfolio`
**Description:** All discovered topics with profiles, stability metrics, and tier info. Topics emerge from weighted multi-space clustering (threshold ≥ 2.5; AP-60 excludes E2-E4).
**Source:** `tools/definitions/topic.rs:20`; DTOs `handlers/tools/topic_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `format` | string |  | `standard` | enum: `brief`, `standard`, `verbose` |

**Side effects:** none (read-only `CF_TOPICS`).

### `get_topic_stability`
**Description:** Portfolio-level churn rate, entropy, phase breakdown.
**Source:** `tools/definitions/topic.rs:39`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `hours` | integer |  | `6` | lookback 1–168 |

### `detect_topics`
**Description:** Force topic detection recalculation using HDBSCAN. Requires ≥3 memories. Threshold `weighted_agreement >= 2.5`.
**Source:** `tools/definitions/topic.rs:57`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `force` | boolean |  | `false` | ignore recent computation |
| `max_memories` | integer |  | `10000` | 1–50000 |

**Side effects:** writes `CF_TOPICS`, `CF_TOPIC_MEMBERSHIP`.

### `get_divergence_alerts`
**Description:** Check for divergence from recent activity using **semantic** embedders only (E1, E6, E7, E10, E12, E13 per AP-62). E5 is excluded because it returns 0.0 without a CausalDirection (AP-77). Temporal embedders excluded per AP-63.
**Source:** `tools/definitions/topic.rs:82`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `lookback_hours` | integer |  | `2` | 1–48 |

---

## 9. File watcher tools

### `list_watched_files`
**Description:** All files that have embeddings from the file watcher.
**Source:** `tools/definitions/file_watcher.rs:20`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `include_counts` | boolean |  | `true` | include chunk counts |
| `path_filter` | string |  | — | glob pattern |

### `get_file_watcher_stats`
**Description:** Total files, total chunks, avg/min/max chunks per file.
**Source:** `tools/definitions/file_watcher.rs:42`.

(no parameters)

### `delete_file_content`
**Description:** Delete all embeddings for a file. Soft-delete by default (SEC-06).
**Source:** `tools/definitions/file_watcher.rs:54`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `file_path` | string | yes | — | absolute path |
| `soft_delete` | boolean |  | `true` | 30-day recovery |

**Side effects:** soft sets `deleted_at`; hard removes from every CF.

### `reconcile_files`
**Description:** Find orphaned files (DB has chunks, disk doesn't have file) and optionally delete them.
**Source:** `tools/definitions/file_watcher.rs:76`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `dry_run` | boolean |  | `true` | preview only |
| `base_path` | string |  | — | optional scope |

---

## 10. Sequence tools (E4)

E4 is the V_ordering embedder for conversation/session sequence position.

### `get_conversation_context`
**Description:** Memories around the current conversation turn, auto-anchored. Uses E4.
**Source:** `tools/definitions/sequence.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `direction` | string |  | `before` | enum: `before`, `after`, `both` |
| `windowSize` | integer |  | `10` | 1–50 |
| `sessionOnly` | boolean |  | `true` | restrict to current session |
| `includeContent` | boolean |  | `true` | inline content |

### `get_session_timeline`
**Description:** Ordered timeline by `session_sequence` ascending. Includes labels like "2 turns ago".
**Source:** `tools/definitions/sequence.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `sessionId` | string |  | current | session to scope |
| `limit` | integer |  | `50` | 1–200 |
| `offset` | integer |  | `0` | pagination |
| `sourceTypes` | array |  | — | enum: `HookDescription`, `ClaudeResponse`, `Manual`, `MDFileChunk` |

### `traverse_memory_chain`
**Description:** Multi-hop traversal from an anchor.
**Source:** `tools/definitions/sequence.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `anchorId` | string (uuid) | yes | — | starting memory |
| `direction` | string |  | `backward` | enum: `forward`, `backward`, `bidirectional` |
| `hops` | integer |  | `5` | 1–20 |
| `semanticFilter` | string |  | — | optional topic filter |
| `minSimilarity` | number |  | — | similarity floor |

### `compare_session_states`
**Description:** Compare memory state at two sequence points in a session.
**Source:** `tools/definitions/sequence.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `beforeSequence` | int|string | yes | — | sequence number or `"start"` |
| `afterSequence` | int|string | yes | — | sequence number or `"current"` |
| `sessionId` | string |  | current | session to scope |

---

## 11. Causal tools (E5)

E5 is the asymmetric causal embedder. Direction modifiers: cause→effect 1.2x, effect→cause 0.8x.

### `search_causal_relationships`
**Description:** Search causal relationships by description similarity with provenance. Returns LLM-generated descriptions linked to source memories.
**Source:** `tools/definitions/causal.rs`; DTOs `handlers/tools/causal_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | description text |
| `direction` | string |  | — | enum: `cause`, `effect` |
| `top_k` | integer |  | `10` | results |
| `min_similarity` | number |  | — | floor |
| `include_provenance` | boolean |  | `true` | include source memory IDs |

### `search_causes`
**Description:** Find memories that caused something matching the query. Applies asymmetric E5 reranking.
**Source:** `tools/definitions/causal.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | what happened |
| `top_k` | integer |  | `10` | results |
| `min_similarity` | number |  | — | floor |

### `search_effects`
**Description:** Find memories that resulted from a cause matching the query.
**Source:** `tools/definitions/causal.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | the cause |
| `top_k` | integer |  | `10` | results |
| `min_similarity` | number |  | — | floor |

### `get_causal_chain`
**Description:** Traverse a chain of causal relationships starting from an anchor.
**Source:** `tools/definitions/causal.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `anchorId` | string (uuid) | yes | — | starting memory |
| `direction` | string |  | `forward` | causal direction |
| `hops` | integer |  | `5` | max depth |
| `min_strength` | number |  | — | edge weight floor |

---

## 12. Causal discovery tools (LLM-gated)

These tools are only present with `--features llm`. They use Qwen2.5-7B for causal analysis with GBNF grammar constraints.

### `trigger_causal_discovery`
**Description:** Manually trigger the causal discovery agent on memories.
**Source:** `tools/definitions/causal_discovery.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `mode` | string |  | `extract` | enum: `extract` (find new), `validate` (check existing) |
| `max_memories` | integer |  | — | scope cap |
| `min_score` | number |  | — | confidence floor |
| `session_scope` | string |  | `current` | enum: `current`, `all` |

**Side effects:** writes `CF_CAUSAL_RELATIONSHIPS`, may write `CF_AUDIT_LOG`.

### `get_causal_discovery_status`
**Description:** Status and statistics of the causal discovery agent.
**Source:** `tools/definitions/causal_discovery.rs`.

(no parameters)

---

## 13. Single-purpose search tools

### `search_by_keywords` (E6)
**Description:** Sparse keyword inverted-index search.
**Source:** `tools/definitions/keyword.rs`; DTOs `handlers/tools/keyword_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | keyword query |
| `top_k` | integer |  | `10` | results |
| `min_similarity` | number |  | — | floor |
| `boost_factor` | number |  | — | E6 weight in fusion |
| `include_content` | boolean |  | `false` | inline content |
| `multi_space` | boolean |  | — | mix into multi-space fusion |
| `weight_profile` | string |  | — | profile override |

### `search_code` (E7)
**Description:** Code-aware search using Qodo-Embed (1536D).
**Source:** `tools/definitions/code.rs`; DTOs `handlers/tools/code_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | code or signature query |
| `top_k` | integer |  | `10` | results |
| `language` | string |  | — | hint (`rust`, `python`, …) |
| `mode` | string |  | `hybrid` | enum: `hybrid` (E7+E1) and similar |
| `include_content` | boolean |  | `false` | inline content |

### `search_robust` (E9)
**Description:** Typo-tolerant search via E9 HDC. Surfaces results with high E9 + low E1 (blind-spot detection).
**Source:** `tools/definitions/robustness.rs`; DTOs `handlers/tools/robustness_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | possibly noisy query |
| `top_k` | integer |  | `10` | results |
| `min_similarity` | number |  | — | floor |
| `min_e9` | number |  | — | E9 floor |
| `max_e1` | number |  | — | E1 ceiling for blind-spot mode |
| `compare_e1` | boolean |  | — | return both E1 and E9 scores |

---

## 14. Graph tools (E8)

### `search_connections`
**Description:** Search the graph by E8 structural similarity (imports, dependencies, references).
**Source:** `tools/definitions/graph.rs`; DTOs `handlers/tools/graph_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | concept or symbol |
| `direction` | string |  | — | enum: `source`, `target`, both |
| `top_k` | integer |  | `10` | results |
| `min_similarity` | number |  | — | floor |
| `include_content` | boolean |  | `false` | inline content |

### `get_graph_path`
**Description:** Shortest path between two memories through the graph.
**Source:** `tools/definitions/graph.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `anchorId` | string (uuid) | yes | — | start |
| `targetId` | string (uuid) | yes | — | end |
| `max_hops` | integer |  | `5` | depth cap |
| `min_strength` | number |  | — | edge weight floor |

### `discover_graph_relationships` (LLM)
**Description:** Use LLM analysis (graph-agent + CausalDiscoveryLLM) to discover relationships among a set of memories.
**Source:** `tools/definitions/graph.rs` (LLM-gated).

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_ids` | array | yes | — | UUIDs to analyze |
| `min_confidence` | number |  | — | confidence floor |

### `validate_graph_link` (LLM)
**Description:** Validate one proposed graph link via LLM.
**Source:** `tools/definitions/graph.rs` (LLM-gated).

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `source_id` | string (uuid) | yes | — | source memory |
| `target_id` | string (uuid) | yes | — | target memory |
| `relation` | string |  | — | proposed relation type |

---

## 15. Entity tools (E11)

E11 is the KEPLER/TransE entity embedder (768D). Asymmetric for head/tail entities.

### `extract_entities`
**Description:** Extract named entities from raw text.
**Source:** `tools/definitions/entity.rs`; DTOs `handlers/tools/entity_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `text` | string | yes | — | input text |
| `include_types` | boolean |  | `true` | tag with entity types |
| `include_offsets` | boolean |  | `false` | character offsets |

### `search_by_entities`
**Description:** Search the graph by entity match.
**Source:** `tools/definitions/entity.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `entities` | array | yes | — | one or more entity strings |
| `top_k` | integer |  | `10` | results |
| `mode` | string |  | — | match strategy |

### `infer_relationship`
**Description:** TransE-style relation inference: given head and tail entities, predict relations.
**Source:** `tools/definitions/entity.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `headEntity` | string | yes | — | subject |
| `tailEntity` | string | yes | — | object |
| `top_k` | integer |  | — | top relations |

### `find_related_entities`
**Description:** Given an entity and a relation, find related entities via TransE arithmetic.
**Source:** `tools/definitions/entity.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `entity` | string | yes | — | anchor |
| `relation` | string | yes | — | relation type |
| `direction` | string |  | — | head→tail or tail→head |
| `top_k` | integer |  | — | results |

### `validate_knowledge`
**Description:** Validate a `(subject, predicate, object)` triple via the knowledge graph.
**Source:** `tools/definitions/entity.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `subject` | string | yes | — | head |
| `predicate` | string | yes | — | relation |
| `object` | string | yes | — | tail |

### `get_entity_graph`
**Description:** Return a structured entity graph (nodes + edges).
**Source:** `tools/definitions/entity.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `seed_entities` | array |  | — | seed set |
| `max_nodes` | integer |  | — | cap |
| `max_hops` | integer |  | — | depth cap |

---

## 16. Embedder-first search tools

7 cross-cutting tools (`embedder::definitions()`) plus 2 standalone (E12, E13).

### `search_by_embedder`
**Description:** Search using a specific embedder as the primary perspective. Each embedder sees the graph differently.
**Source:** `tools/definitions/embedder.rs`; DTOs `handlers/tools/embedder_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `embedder` | string | yes | — | enum: E1..E14 |
| `query` | string | yes | — | search text |
| `top_k` | integer |  | `10` | results |
| `min_similarity` | number |  | — | floor |
| `include_content` | boolean |  | `false` | inline content |
| `include_provenance` | boolean |  | `false` | retrieval provenance |

### `get_embedder_clusters`
**Description:** HDBSCAN clusters within a single embedder's index.
**Source:** `tools/definitions/embedder.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `embedder` | string | yes | — | embedder name |
| `min_cluster_size` | integer |  | — | clustering param |
| `max_clusters` | integer |  | — | cap |

### `compare_embedder_views`
**Description:** Run the same query against multiple embedders, return per-embedder rankings + agreement metrics.
**Source:** `tools/definitions/embedder.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `embedders` | array | yes | — | embedders to compare |
| `top_k` | integer |  | — | results per embedder |

### `list_embedder_indexes`
**Description:** List all loaded embedder indexes (HNSW, sparse, token) with shapes and counts.
**Source:** `tools/definitions/embedder.rs`.

(no parameters)

### `get_memory_fingerprint`
**Description:** Full fingerprint for one memory: embeddings, scores, source metadata.
**Source:** `tools/definitions/embedder.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memoryId` | string (uuid) | yes | — | memory |
| `include_vectors` | boolean |  | `false` | inline raw vectors |

### `create_weight_profile`
**Description:** Create a session-scoped custom weight profile for reuse.
**Source:** `tools/definitions/embedder.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `name` | string | yes | — | profile name |
| `weights` | object | yes | — | `{E1, E2, …}` each `[0,1]` |
| `description` | string |  | — | docs |

**Side effects:** session-scoped — not persisted to RocksDB.

### `search_cross_embedder_anomalies`
**Description:** Memories that score high in `highEmbedder` but low in `lowEmbedder`. Useful for blind-spot mining.
**Source:** `tools/definitions/embedder.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `highEmbedder` | string | yes | — | embedder A |
| `lowEmbedder` | string | yes | — | embedder B |
| `min_high` | number |  | — | A score floor |
| `max_low` | number |  | — | B score ceiling |
| `top_k` | integer |  | — | results |

### `search_by_tokens` (E12 standalone)
**Description:** ColBERT MaxSim per-token retrieval (E12).
**Source:** `tools/definitions/embedder.rs::standalone_definitions()`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `top_k` | integer |  | — | results |
| `min_score` | number |  | — | floor |

### `search_by_expansion` (E13 standalone)
**Description:** SPLADE sparse expansion-based recall (E13).
**Source:** `tools/definitions/embedder.rs::standalone_definitions()`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `top_k` | integer |  | — | results |
| `min_score` | number |  | — | floor |
| `expansion_terms` | integer |  | — | top-k expansion vocabulary |

---

## 17. Temporal tools (E2/E3)

### `search_recent`
**Description:** Recency-weighted search via E2 (sinusoidal positional encoding of unix timestamp).
**Source:** `tools/definitions/temporal.rs`; DTOs `handlers/tools/temporal_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `top_k` | integer |  | `10` | results |
| `decayFunction` | string |  | `exponential` | enum: `linear`, `exponential`, `step`, `none` |
| `decayHalfLifeSecs` | integer |  | `86400` | half-life seconds |
| `temporal_weight` | number |  | — | E2 weight in fusion |

### `search_periodic`
**Description:** Match periodic patterns via E3.
**Source:** `tools/definitions/temporal.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `targetHour` | integer |  | — | 0–23 |
| `targetDayOfWeek` | integer |  | — | 0=Sun…6=Sat |
| `periodicBoost` | number |  | — | E3 weight |
| `top_k` | integer |  | — | results |

---

## 18. Graph linking tools (K-NN navigation)

### `get_memory_neighbors`
**Description:** K-nearest-neighbours of a memory via the unified K-NN index.
**Source:** `tools/definitions/graph_link.rs`; DTOs `handlers/tools/graph_link_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | string (uuid) | yes | — | anchor |
| `k` | integer |  | `10` | neighbours |
| `min_similarity` | number |  | — | floor |
| `include_content` | boolean |  | `false` | inline content |

### `get_typed_edges`
**Description:** Typed edges (cause/effect, supports/contradicts, etc.) for a memory. Direction-aware (incoming/outgoing/both).
**Source:** `tools/definitions/graph_link.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | string (uuid) | yes | — | anchor |
| `edge_type` | string |  | — | filter by type |
| `direction` | string |  | `both` | enum: `incoming`, `outgoing`, `both` |
| `min_strength` | number |  | — | weight floor |
| `limit` | integer |  | — | cap |

### `traverse_graph`
**Description:** Multi-hop traversal of typed edges from a starting memory.
**Source:** `tools/definitions/graph_link.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `start_memory_id` | string (uuid) | yes | — | starting memory |
| `direction` | string |  | — | enum: `incoming`, `outgoing`, `both` |
| `max_hops` | integer |  | `3` | depth cap |
| `edge_types` | array |  | — | filter |
| `min_strength` | number |  | — | weight floor |

### `get_unified_neighbors`
**Description:** Multi-space K-NN via Weighted RRF across all 14 embedders. Per ARCH-21: fusion via RRF, not weighted sum. Per AP-60: temporal embedders excluded from semantic fusion.
**Source:** `tools/definitions/graph_link.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | string (uuid) | yes | — | anchor |
| `k` | integer |  | `10` | neighbours |
| `weight_profile` | string |  | — | profile override |
| `custom_weights` | object |  | — | per-embedder weights |
| `include_breakdown` | boolean |  | `false` | per-embedder per-result scores |

---

## 19. Maintenance tool

### `repair_causal_relationships`
**Description:** Scan `CF_CAUSAL_RELATIONSHIPS`, delete entries that fail deserialization (truncated or corrupted bincode rows).
**Source:** `tools/definitions/maintenance.rs`.

(no parameters)

**Side effects:** prunes `CF_CAUSAL_RELATIONSHIPS`. Read+write.

---

## 20. Provenance tools

### `get_audit_trail`
**Description:** Audit log entries for a memory or time window.
**Source:** `tools/definitions/provenance.rs`; DTOs `handlers/tools/provenance_dtos.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | string (uuid) |  | — | filter to one memory |
| `operator_id` | string |  | — | filter to one operator |
| `event_type` | string |  | — | filter to one event |
| `since` | integer |  | — | unix timestamp lower bound |
| `until` | integer |  | — | unix timestamp upper bound |
| `limit` | integer |  | — | cap |

### `get_merge_history`
**Description:** Merge lineage for a fingerprint — chain of merges that produced or affected this memory. Supports 30-day reversal.
**Source:** `tools/definitions/provenance.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | string (uuid) | yes | — | memory |
| `include_reversed` | boolean |  | — | include reversed merges |

### `get_provenance_chain`
**Description:** Full chain from embeddings back to source for a memory.
**Source:** `tools/definitions/provenance.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | string (uuid) | yes | — | memory |
| `depth` | integer |  | — | trace depth |

---

## 21. Daemon tool

### `daemon_status`
**Description:** Daemon health metrics: active connections, model state, background tasks, lock holder.
**Source:** `tools/definitions/daemon.rs`.

(no parameters)

---

## 22. Training tools

Persist training data to `CF_TRAINING_RECORDS`.

### `export_training_corpus`
**Description:** Export memories as fully-labeled `TrainingRecord`s persisted to `CF_TRAINING_RECORDS`. Output is RocksDB-only (no external formats).
**Source:** `tools/definitions/training.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `selector` | string |  | `all` | enum: `all`, scope subset |
| `min_importance` | number |  | — | floor |
| `session_id` | string |  | — | scope to one session |
| `max_records` | integer |  | — | cap |
| `include_vectors` | boolean |  | — | persist embeddings |
| `include_audit` | boolean |  | — | include audit history |
| `include_e1_only` | boolean |  | — | restrict to E1 vector |

**Side effects:** writes `CF_TRAINING_RECORDS`.

### `list_training_records`
**Description:** List UUIDs in `CF_TRAINING_RECORDS` with shape stats.
**Source:** `tools/definitions/training.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | — | cap |
| `offset` | integer |  | — | pagination |

### `get_training_record`
**Description:** Fetch one record by memory UUID.
**Source:** `tools/definitions/training.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memoryId` | string (uuid) | yes | — | record id |
| `omit_vectors` | boolean |  | — | strip heavy vectors |

### `count_training_records`
**Description:** Count rows in `CF_TRAINING_RECORDS`.
**Source:** `tools/definitions/training.rs`.

(no parameters)

---

## 23. Learning-as-UTL tools (14)

Persists Universal Theory of Learning events to `CF_LEARNING_EVENTS` and derived rows. Detail in [20_learning_subsystem.md](20_learning_subsystem.md).

### `record_learning_event`
**Description:** Persist one before/after learning event to `CF_LEARNING_EVENTS`.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `before` | object | yes | — | learner state before |
| `after` | object | yes | — | learner state after |
| `outcome` | object | yes | — | outcome details |
| `learner_id` | string |  | — | optional learner |
| `signal_embeddings` | boolean |  | — | also embed signals |

**Side effects:** writes `CF_LEARNING_EVENTS`, optionally `CF_LEARNING_SIGNAL_VECTORS`.

### `list_learning_events`
**Description:** List UUIDs in `CF_LEARNING_EVENTS`.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | — | cap |
| `offset` | integer |  | — | pagination |

### `get_learning_event`
**Description:** Fetch one Learning-as-UTL event by UUID.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `eventId` | string (uuid) | yes | — | event id |

### `count_learning_events`
**Description:** Count rows in `CF_LEARNING_EVENTS`.
**Source:** `tools/definitions/learning.rs`.

(no parameters)

### `list_learning_signal_embedders`
**Description:** List deterministic Learning-as-UTL signal embedders (the curated set used to embed before/after states).
**Source:** `tools/definitions/learning.rs`.

(no parameters)

### `compute_learning_signals`
**Description:** Compute deterministic Learning-as-UTL features and signal embeddings from an inline transition.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `before` | object | yes | — | state before |
| `after` | object | yes | — | state after |
| `outcome` | object | yes | — | outcome |

### `embed_learning_event_signals`
**Description:** Embed one persisted LearningEvent with deterministic signal embedders.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `eventId` | string (uuid) | yes | — | event id |

**Side effects:** writes `CF_LEARNING_SIGNAL_VECTORS`.

### `estimate_learning_outcome`
**Description:** Predict a candidate transition's outcome from nearest persisted LearningEvent cases.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `before` | object | yes | — | candidate before-state |
| `candidateAfter` | object | yes | — | candidate after-state |
| `top_k` | integer |  | — | NN cases |

### `record_graph_learning_event`
**Description:** Persist a graph-navigation outcome as a LearningEvent backed by real EdgeRepository evidence.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `policyScope` | string | yes | — | scope (`global`, `session`, ...) |
| `sourceMemoryId` | string (uuid) | yes | — | anchor |
| `selectedNeighborIds` | array | yes | — | the neighbours the user selected |
| `beforeRank` | array | yes | — | ranking before |
| `afterRank` | array | yes | — | ranking after |
| `outcome` | object | yes | — | outcome details |

### `resolve_graph_learning_policy`
**Description:** Resolve a learned graph ranking policy from persisted graph LearningEvents.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `policyScope` | string | yes | — | scope |

### `export_learner_training_dataset`
**Description:** Compile learner/learning CF rows into a matrix-shaped RocksDB dataset.
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `selector` | string |  | — | scope |
| `description` | string |  | — | metadata |
| `include_signals` | boolean |  | — | include signal vectors |

**Side effects:** writes a learner-training-dataset row.

### `list_learner_training_datasets`
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | — | cap |
| `offset` | integer |  | — | pagination |

### `get_learner_training_dataset`
**Source:** `tools/definitions/learning.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `datasetId` | string (uuid) | yes | — | dataset id |
| `include_rows` | boolean |  | — | hydrate rows |

### `count_learner_training_datasets`
**Source:** `tools/definitions/learning.rs`.

(no parameters)

---

## 24. UTL learner-state tools (18)

Phase-0/Phase-1 measurement substrate + retrieval policy. Detail in [20_learning_subsystem.md](20_learning_subsystem.md).

### `register_learner`
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `handle` | string | yes | — | learner identifier |
| `metadata` | object |  | — | learner metadata |

**Side effects:** writes `CF_LEARNER_PROFILE`.

### `record_session_observation`
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `sessionTs` | integer | yes | — | session timestamp |
| `observation` | object |  | — | observation payload |
| `state_vector` | array |  | — | state vector |

**Side effects:** writes `CF_LEARNER_STATE_HISTORY`.

### `compute_delta_s`
**Description:** Compute the UTL Δ_S surprise signal.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) |  | — | learner |
| `before` | object |  | — | state before |
| `after` | object |  | — | state after |

### `compute_delta_c`
**Description:** Compute the UTL Δ_C coherence signal.
**Source:** `tools/definitions/utl.rs`. Same shape as `compute_delta_s`.

### `compute_delta_e`
**Description:** Compute the UTL Δ_E embodied-state signal.
**Source:** `tools/definitions/utl.rs`. Same shape as `compute_delta_s`.

### `compute_L`
**Description:** Compute UTL L = Δ_S × Δ_C × Δ_E.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `components` | object | yes | — | `{delta_s, delta_c, delta_e}` |

### `get_learner_M`
**Description:** Read a persisted learner M(t) trace.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `traceId` | string (uuid) | yes | — | trace id |

### `next_review_for_trace`
**Description:** Compute next-review timestamp for a learner trace (spaced repetition).
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `traceId` | string (uuid) | yes | — | trace id |

### `list_learner_embedders`
**Description:** List the canonical E1–E21 content + learner-state embedder matrix.
**Source:** `tools/definitions/utl.rs`. (no parameters)

### `preflight_learner_assets`
**Description:** Inspect cached E15–E21 model assets and real calibration datasets.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `verify` | boolean |  | — | also recompute hashes |

### `count_learner_state`
**Description:** Count all learner-state RocksDB column families.
**Source:** `tools/definitions/utl.rs`. (no parameters)

### `get_learner_state`
**Description:** Read persisted learner profile/session-state/delta rows.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `sessionTs` | integer |  | — | scope to one session |

### `record_learner_k_sleep`
**Description:** Persist a sleep-derived k(τ) row.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `sessionTs` | integer | yes | — | session |
| `slowWaveMinutes` | number | yes | — | sleep duration |

### `record_learner_retrieval`
**Description:** Persist a retrieval-practice row tied to a learner trace.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `traceId` | string (uuid) | yes | — | trace |
| `ts` | integer | yes | — | retrieval time |
| `correct` | boolean | yes | — | correctness |
| `score` | number | yes | — | score |

### `upsert_goal_centroid`
**Description:** Persist an expert goal centroid for a skill and learner modality.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `skillId` | string (uuid) | yes | — | skill |
| `modality` | string | yes | — | learner modality |
| `vector` | array | yes | — | centroid vector |

### `get_goal_distance`
**Description:** Compare a learner session embedding to a persisted goal centroid.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `skillId` | string (uuid) | yes | — | skill |
| `sessionTs` | integer | yes | — | session |
| `modality` | string | yes | — | modality |

### `compile_learner_constellation`
**Description:** Compile and persist a learner regulated-state baseline constellation.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `sessionTsList` | array | yes | — | session timestamps |
| `label` | string |  | — | label |

### `resolve_learner_retrieval_policy`
**Description:** Resolve the state-conditioned retrieval weight profile for a persisted learner session.
**Source:** `tools/definitions/utl.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | string (uuid) | yes | — | learner |
| `sessionTs` | integer | yes | — | session |

---

## 25. Constellation tools (Phase 2)

Constellations are per-embedder centroids + spread stats over a selected set of memories. Detail in [17_constellation_subsystem.md](17_constellation_subsystem.md).

### `compile_constellation`
**Description:** Compile a constellation from caller-selected memories and persist to `CF_CONSTELLATIONS`.
**Source:** `tools/definitions/constellation.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `selector` | object|array | yes | — | selection rule (memory IDs, query, filter) |
| `label` | string | yes | — | label |
| `description` | string |  | — | docs |

**Side effects:** writes `CF_CONSTELLATIONS` and the selector index.

### `list_constellations`
**Source:** `tools/definitions/constellation.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | — | cap |
| `offset` | integer |  | — | pagination |
| `include_shape` | boolean |  | — | include shape stats |

### `get_constellation`
**Source:** `tools/definitions/constellation.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `constellationId` | string (uuid) | yes | — | id |
| `include_centroids` | boolean |  | — | inline centroid arrays |

### `score_against_constellation`
**Source:** `tools/definitions/constellation.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `constellationId` | string (uuid) | yes | — | id |
| `memoryId` | string (uuid) | yes | — | candidate memory |

### `derive_constellation`
**Description:** Persist a derived constellation through interpolation/add/difference or anti-pole mining from real candidate memories.
**Source:** `tools/definitions/constellation.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `operation` | string | yes | — | enum: `interpolate`, `add`, `difference`, `antipole` |
| `sourceConstellationId` | string (uuid) | yes | — | base constellation |
| `label` | string | yes | — | derived label |
| `parameter` | object |  | — | operation-specific args |

### `delete_constellation`
**Source:** `tools/definitions/constellation.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `constellationId` | string (uuid) | yes | — | id |

**Side effects:** deletes from `CF_CONSTELLATIONS` and removes selector-index entry.

---

## 26. Contrastive pair tools (Phase 3)

Detail in [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

### `mine_contrastive_pairs`
**Description:** Mine cross-embedder anomaly pairs from the corpus and persist to `CF_CONTRASTIVE_PAIRS`.
**Source:** `tools/definitions/contrastive.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `kind` | string |  | — | filter to one anomaly kind |
| `max_pairs` | integer |  | — | cap |
| `min_high` | number |  | — | high-embedder floor |
| `max_low` | number |  | — | low-embedder ceiling |

**Side effects:** writes `CF_CONTRASTIVE_PAIRS`.

### `list_contrastive_pairs`
**Source:** `tools/definitions/contrastive.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | — | cap |
| `offset` | integer |  | — | pagination |
| `kind` | string |  | — | filter |
| `anchor_id` | string (uuid) |  | — | filter |

### `get_contrastive_pair`
**Source:** `tools/definitions/contrastive.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `anchorId` | string (uuid) | yes | — | anchor |
| `negativeId` | string (uuid) | yes | — | negative |

### `count_contrastive_pairs`
**Source:** `tools/definitions/contrastive.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `kind` | string |  | — | filter |

---

## 27. Typed-edge training tools (Phase 4)

### `export_typed_edges_corpus`
**Description:** Export typed edges as `TypedEdgeTrainingRecord`s persisted to `CF_TYPED_EDGE_RECORDS`.
**Source:** `tools/definitions/typed_edges.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `min_strength` | number |  | — | weight floor |
| `edge_types` | array |  | — | filter |
| `max_records` | integer |  | — | cap |

**Side effects:** writes `CF_TYPED_EDGE_RECORDS`.

### `derive_anomalies_from_edges`
**Description:** Derive anomaly pairs directly from typed-edge classifications and persist to `CF_CONTRASTIVE_PAIRS` (F2).
**Source:** `tools/definitions/typed_edges.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `min_confidence` | number |  | — | floor |
| `max_pairs` | integer |  | — | cap |

**Side effects:** writes `CF_CONTRASTIVE_PAIRS`.

### `validate_low_confidence_edges`
**Description:** Run LLM validation on typed edges below the configured weight ceiling and persist verdicts to `CF_TYPED_EDGE_VALIDATIONS` (F4). Returns `{status:"skipped"}` without `--features llm`.
**Source:** `tools/definitions/typed_edges.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `weight_ceiling` | number |  | — | ceiling |
| `max_edges` | integer |  | — | cap |

**Side effects:** writes `CF_TYPED_EDGE_VALIDATIONS` (LLM build only).

### `list_typed_edge_records`
**Source:** `tools/definitions/typed_edges.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | — | cap |
| `offset` | integer |  | — | pagination |
| `hydrate` | boolean |  | — | include full records |

---

## 28. DynamicJEPA tools (35)

Surface the DynamicJEPA (5090jepa) world-model pipeline. Schemas live in `tools/definitions/dynamicjepa.rs`; DTOs in `handlers/tools/dynamicjepa_dtos.rs`. Every tool requires `dbPath` (path to the DynamicJEPA RocksDB data directory). See [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md) for end-to-end semantics.

### 28.1 Domain pack registry

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_register_domain_pack` | `dbPath`, `file` | — | writes domain-pack CFs |
| `dynamicjepa_list_domain_packs` | `dbPath` | `limit`, `offset` | none |
| `dynamicjepa_get_domain_pack` | `dbPath`, `id` | `domainVersion` (default `"1.0.0"`) | none |

### 28.2 Event ingestion + adapters

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_ingest_event` | `dbPath`, `domain`, `adapter`, `file` | — | writes raw events + adapter outputs |
| `dynamicjepa_run_adapter` | `dbPath`, `eventId` | — | runs adapter; writes instrument readings |
| `dynamicjepa_list_instrument_readings` | `dbPath`, `eventId` | — | none |

### 28.3 Latent panels

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_materialize_panel` | `dbPath` | `transitionId`, `allPending` (default `false`), `domain` | writes panels |
| `dynamicjepa_get_panel` | `dbPath`, `panelId` | `includeReadings` (default `false`) | none |

### 28.4 Bindings

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_create_binding` | `dbPath`, `leftCf`, `leftKey`, `rightCf`, `rightKey` | `method` (default `"explicit_mapping"`), `kind` (default `"event_to_trajectory"`), `score` (default `1.0`) | writes binding |
| `dynamicjepa_list_bindings` | `dbPath` | `entity`, `limit`, `offset` | none |

### 28.5 Trajectories + datasets

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_compile_trajectories` | `dbPath`, `domain` | `policy` (default `"by_domain_session"`) | writes trajectories |
| `dynamicjepa_get_trajectory` | `dbPath`, `id` | — | none |
| `dynamicjepa_list_trajectories` | `dbPath`, `domain` | `limit`, `offset` | none |
| `dynamicjepa_compile_dataset` | `dbPath`, `domain` | `policy` (default `"one_step"`), `split` (default `"train"`) | writes dataset shards |
| `dynamicjepa_get_dataset_shard` | `dbPath`, `datasetId`, `shardId` | — | none |
| `dynamicjepa_inspect_dataset_row` | `dbPath`, `datasetId`, `shardId`, `row` | — | none |

### 28.6 Training

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_train` | `dbPath`, `datasetId`, `config`, `artifactRoot` | — | trains tiny CUDA artifact; writes training run + artifact rows |
| `dynamicjepa_get_training_run` | `dbPath`, `id` | — | none |
| `dynamicjepa_get_artifact` | `dbPath`, `id` | `verifyFiles` (default `false`) | none |

### 28.7 Prediction + planning + surprise

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_predict` | `dbPath`, `artifactId`, `panelId`, `actionId` | — | writes prediction |
| `dynamicjepa_plan` | `dbPath`, `artifactId`, `panelId`, `skillId` | `candidateActionJson` | writes plan trace |
| `dynamicjepa_record_surprise` | `dbPath`, `predictionId`, `observedOutcomeId`, `observedPanelId` | — | writes surprise event when threshold breached |
| `dynamicjepa_get_prediction` | `dbPath`, `id` | — | none |
| `dynamicjepa_get_plan_trace` | `dbPath`, `id` | `includePredictions` (default `false`), `includeGuards` (default `false`) | none |
| `dynamicjepa_get_surprise` | `dbPath`, `id` | — | none |

### 28.8 Constellations + thresholds

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_build_constellation` | `dbPath`, `domain` | `domainVersion` (default `"1.0.0"`), `subject` (default `"global"`), `sourceEventSelector`, `builtByRunId` | writes immutable constellation centroids |
| `dynamicjepa_list_constellations` | `dbPath` | `domain`, `subject`, `limit`, `offset` | none |
| `dynamicjepa_get_constellation` | `dbPath`, `domain`, `modality` | `domainVersion`, `subject` | none |
| `dynamicjepa_calibrate_threshold` | `dbPath`, `domain` | `domainVersion`, `subject`, `modality` (default `"all"`), `calibrationEventSelector`, `percentile` | writes calibration row |
| `dynamicjepa_recalibrate_threshold` | `dbPath`, `domain`, `supersedes`, `reason` | `domainVersion`, `subject`, `modality`, `calibrationEventSelector`, `percentile` | supersedes prior calibration |

### 28.9 Audits + paper evidence

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_compute_mc_ratio` | `dbPath`, `domain`, `outputDir` | `domainVersion` | writes `table_mc_ratio.csv` + plot artifacts |
| `dynamicjepa_audit_pairwise_mi` | `dbPath`, `domain`, `outputDir` | `domainVersion`, `sampleSize` (default 50; ≤1000), `estimator` (default `"ksg"`), `ksgK` (default 1; ≤5), `bootstrapIters` (default 1; ≤1000), `seed` (default 0; ≤20260501) | writes pairwise MI artifacts |
| `dynamicjepa_cross_domain_transfer` | `outputRoot` | `seeds` (default `[42,43,44,45,46]`), `sourceEvents` (default 20), `targetEvents` (default 20), `bootstrapIters` (default 1), `trainEpochs` (default 1), `batchSize` (default 1), `maxSecondsPerTraining` (default 1), `learningRate` (default 0.001), `stoppingTarget` (default 0.20) | runs counter_world→gridworld transfer pilot; writes `transfer_results.json`, paper table, plots, bridge artifacts |

### 28.10 Inspection

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `dynamicjepa_inspect_counts` | `dbPath` | — | none |
| `dynamicjepa_inspect_cf` | `dbPath`, `cf` | `keyHex`, `limit`, `offset` | none |

---

## 29. Reality-loop tools (35; mode-gated)

The `mcp__cgreality__*` family is exposed by the lightweight reality-loop stdio MCP server (launched with `--mode reality-loop`) and by the standard server when `tools::is_reality_loop_mode()` is true. Schemas live in `tools/definitions/reality.rs`; handlers in `handlers/tools/reality_loop/`. See [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md) for the subsystem description.

All reality tools return error bodies with `error_code`, `field_path`, `message`, `source_of_truth`, and `details` (`reality_loop/errors.rs`). Reality reads dispatch to `analyze.rs`; harness writes dispatch to `alter.rs`; runs and queries dispatch to `interact.rs`; optimizer artifacts dispatch to `optimizer.rs`; champion/experiment registry to `autoresearch.rs`; shift logging to `shift_log.rs`.

### 29.1 Reality query tools (analyze.rs)

Read-only artifact queries against the active runtime root.

| Tool | Required params | Optional params | Returns |
| --- | --- | --- | --- |
| `reality_latest_root` | — | — | `{runtime_root, active_run_id, active_task_id, active_target_instance, success_predicate, last_attempt_number, source_of_truth}` |
| `reality_attempt_summary` | `run_id`, `attempt` | `compact` (default `false`) | attempt-summary.json (compactable) |
| `reality_official_report` | `run_id`, `attempt` | — | `official-success-evidence.json` or `official-failure-evidence.json` |
| `reality_problem_packet` | `run_id` | `compact` (default `false`) | `problem-reality-packet.json` |
| `reality_signal` | `run_id`, `attempt` | `compact` (default `false`) | `reality-signal-packet.json` |
| `dynamicjepa_reality_for_attempt` | `run_id`, `attempt` | — | `dynamicjepa_reality` block + SQLite verification |
| `reality_failure` | `run_id`, `attempt` | — | compact failure evidence |
| `reality_trigger_decision` | `run_id`, `attempt` | — | `trigger-decision.json` |
| `reality_harness_transitions` | `run_id`, `attempt` | `round` (`integer\|null`) | one or all transition artifacts under `harness-transitions/round-NN.json` |
| `reality_compare_attempts` | `run_id`, `attempt_a`, `attempt_b` | — | per-attempt summaries with diff |
| `reality_audit_trail` | `entity_id` | `limit` (1–500, default 50) | ContextGraph audit trail rows |
| `reality_replay_artifact` | `path` | — | any file artifact + SHA-256 readback |

**Side effects:** all read-only.
**Cross-cutting behavior:** every reality query tool invariant-checks the active runtime root via `read_active_runtime_root()` and the active target instance via `read_active_target_instance()`. Errors include `CCREALITY_ACTIVE_RUNTIME_ROOT_MISSING`, `CCREALITY_ACTIVE_TARGET_INSTANCE_MISSING`, `CCREALITY_OFFICIAL_REPORT_MISSING`, `CCREALITY_PROBLEM_PACKET_MISSING`, `CCREALITY_DYNAMICJEPA_REALITY_*` (8 sub-codes when DynamicJEPA reality block fails verification).

### 29.2 Reality run tools (interact.rs)

Spawn or replay reality-loop runs.

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `reality_run_attempt` | `task_manifest` | `runtime_root`, `max_attempts` (default 1), `official_eval` (default `true`), `model`, `inner_model_path`, `inner_context_tokens`, `inner_max_new_tokens` | spawns `reality-loop` engine subprocess; writes attempt artifacts |
| `reality_run_smoke` | — | `runtime_root`, `label`, `session_id` | spawns synthetic smoke; writes attempt artifacts |
| `reality_run_official_eval` | `run_id`, `attempt` | — | runs official SWE-bench Docker evaluator; writes `official-*.json` |
| `reality_run_targeted_test` | `run_id`, `attempt`, `test_id` | `timeout_secs` (1–1800) | runs one targeted test; writes targeted-test artifact |
| `reality_recompile_reward` | `run_id`, `attempt` | — | replays signal packet under current reward spec |
| `reality_query_ledger` | `run_id`, `where` | `limit` (1–500) | constrained read-only BlackBox ledger query |

### 29.3 Harness write tools (alter.rs)

Governed file edits and command runs. Each shells out to `target/release/reality-loop harness <subcommand>` (the MCP→CLI shim, post-2026-05-07). The path-policy gate was removed 2026-05-07; the only path checks remaining are empty-string and control-character rejection.

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `harness_open_window` | `path`, `start_line`, `end_line` | — | reads file window, returns SHA-256 |
| `harness_apply_line_window_edit` | `path`, `start_line`, `end_line`, `observed_sha256`, `replace` | `session_id`, `tool_use_id` | SHA-guarded line-window edit; writes file; appends shift log |
| `harness_run_command` | `command` | `timeout_secs` (1–1800), `session_id` | runs allowlisted command in project root; appends shift log |
| `harness_git_diff` | — | `path` (`string\|null`), `base` | runs `git diff` |
| `harness_git_status` | — | — | runs `git status --porcelain` |
| `harness_verify_state` | — | `scope` (default `reality_loop_only`; enum `reality_loop_only`/`full`) | runs governed `cargo fmt`/`check`/`test` |

**Cross-cutting behavior:** every harness write goes through `EditLock` (`reality_loop/lock.rs`) — single-writer per project tree. SHA mismatches return `CCREALITY_OBSERVED_SHA256_MISMATCH`. Shift records are appended to `<runtime_root>/shift-logs/<session_id>.jsonl` via `shift_log::append_shift`.

### 29.4 Optimizer record tools (optimizer.rs)

The Stop-hook-enforced trio that closes the loop on every optimizer turn.

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `optimizer_record_decision` | `run_id`, `attempt`, `policy`, `should_run`, `claude_session_id`, `claude_model` | `reasons` | writes `decision-turn-NN.json`, `trigger-decision.json`; appends ledger; appends shift log |
| `optimizer_record_recommendation` | (validated by `RecommendationValidator`; see schema below) | — | writes `recommendation-turn-NN.json`; appends ledger; appends shift log |
| `optimizer_record_harness_transition` | `run_id`, `attempt`, `file_path`, `before_sha256`, `after_sha256`, `lines_added`, `lines_removed` | `tool_use_id`, `cargo_check`, `git_diff_stat`, `session_id` | writes `harness-transitions/round-NN.json`; appends ledger; appends shift log |

**`optimizer_record_recommendation` schema:** validated against `crates/context-graph-mcp/src/handlers/tools/reality_loop/schema_validator.rs::RecommendationValidator`. The schema is loaded at handler startup. Required fields include `run_id`, `attempt`, `turn_number`, `claude_session_id`, plus the recommendation body (`status`, `confidence`, `next_inner_prompt_delta`, `harness_changes`, `remaining_risks`, etc.).

**Recommendation tolerance:** `coerce_stringified_args(args)` in `optimizer.rs::optimizer_record_recommendation` re-types JSON-stringified ints/arrays/booleans before validation (Phase 13 T13.2 — for clients that flatten typed values to strings before transport).

### 29.5 Shift log tools (shift_log.rs)

Per-session reality-shift log: every alter call has a before/after SHA delta appended to `<runtime_root>/shift-logs/<session_id>.jsonl`.

| Tool | Required params | Optional params | Returns |
| --- | --- | --- | --- |
| `reality_shift_log` | `session_id` | `limit` (1–500), `since_shift_id` (`string\|null`) | recent shift records |
| `reality_shift_compare_to_my_view` | `session_id`, `files` | — | per-file `{external_drift_detected, current_sha256, last_seen_sha256}` |

**External drift detection:** `reality_shift_compare_to_my_view` is the canonical way to check if files changed outside the session's shift log. If `external_drift_detected: true`, the optimizer must re-perceive (`harness_open_window`) before any further alter.

### 29.6 Champion state and experiment registry (autoresearch.rs)

Phase 15 autoresearch engine. Tracks (model, task) champions and harness-change experiments at `<run_root>/reality-optimizer/{champion-state.json, experiment-registry.json, attempts.jsonl}`.

| Tool | Required params | Optional params | Side effects |
| --- | --- | --- | --- |
| `champion_state_get` | — | `model`, `task` | none |
| `champion_state_promote` | `model`, `task`, `justification`, `claude_session_id` | — | writes `champion-state.json` (requires `official_resolved=true`) |
| `attempts_history_query` | — | `model`, `instance_id`, `limit` (1–500, default 100) | none |
| `experiment_registry_list` | — | `status_filter` (`pending`/`kept`/`discarded`/`escalated`/`promoted`), `limit` (1–500, default 100) | none |
| `experiment_registry_get` | `experiment_id` | — | none |
| `experiment_registry_propose` | `harness_change_summary`, `claude_session_id` | `harness_change_recommendation_path`, `harness_change_shift_ids`, `files_changed`, `before_attempt_count` | appends experiment registry |
| `experiment_registry_update_outcome` | `experiment_id`, `outcome` (enum: `pending`/`kept`/`discarded`/`escalated`/`promoted`), `claude_session_id` | `reasoning` | updates experiment outcome |

**Champion promotion:** `champion_state_promote` requires the latest attempt for `(model, task)` to have `official_resolved=true` in the BlackBox ledger; otherwise it returns `CCREALITY_CHAMPION_PROMOTION_REJECTED`.

### 29.7 Reality-loop artifact layout

Source-of-truth file paths for reality artifacts:

```
<runtime_root>/<run_id>/
  problem-reality-packet.json
  reality-signal-packet.json
<runtime_root>/<run_id>/<task_id>/qwen3.6-27b/attempts/attempt-NNNNNN/
  attempt-summary.json
  agent-tool-transcript.json
  validation.json
  inner-model-readback.json
  inner-harness-response.json
  reality-signal-packet.json
  trigger-decision.json
  agent-tools/round-NN-MM-<tool>/
  harness-transitions/round-NN.json
  official-evidence/<instance>.json
  official-success-evidence.json | official-failure-evidence.json
  claude-code-optimizer/
    recommendation-turn-NN.json
    decision-turn-NN.json
    commands/<unix>-<command>.json
<runtime_root>/shift-logs/<session_id>.jsonl
<runtime_root>/reality-optimizer/
  champion-state.json
  experiment-registry.json
  attempts.jsonl
```

All file paths returned to the client carry a `source_of_truth: "file:<absolute path>"` marker (or `sqlite:` / `git:` for ledger and git artifacts).

---

## 30. Conventions

### 30.1 Identifiers
- All memory IDs are RFC-4122 UUIDv7 strings.
- Run IDs (reality-loop) are deterministic strings minted by the engine; treated as opaque.
- Session IDs are caller-provided strings (typically `CLAUDE_SESSION_ID`).
- Attempt numbers are zero-based integers per `(run_id, task_id)` pair.

### 30.2 Time
- All timestamps are integer unix seconds unless noted otherwise.
- Decay half-life on `search_graph` is `decayHalfLifeSecs` (default 86400 = 1 day).

### 30.3 Numeric ranges
- Importance, similarity, weight components all use `[0.0, 1.0]` (cosine normalized via SRC-3 `(raw+1)/2` for embedders that return `[-1, 1]`).
- `boost_importance.delta` is the only signed numeric param: `[-1.0, 1.0]`.

### 30.4 Strict additionalProperties
Every input schema sets `additionalProperties: false`. Unknown fields are rejected with `INVALID_PARAMS`. Two exceptions: `optimizer_record_decision` and `optimizer_record_recommendation` permit additional fields (the recommendation validator carries the strict shape).

### 30.5 LLM gating
`#[cfg(feature = "llm")]` guards `discover_graph_relationships`, `validate_graph_link`, `trigger_causal_discovery`, `get_causal_discovery_status`. Counts in §1.1 reflect both builds. `validate_low_confidence_edges` is always present but no-ops in non-LLM builds.

### 30.6 Reality-loop mode gating
`tools::is_reality_loop_mode()` reads the `--mode reality-loop` flag (or `CONTEXT_GRAPH_REALITY_MODE=1` env var). When true, the standard server registers all 35 reality tools alongside the default 145/141 surface; when false, only the lightweight `--mode reality-loop` server exposes them.

### 30.7 Session-scoped state
`create_weight_profile` is session-scoped and is **not** persisted to RocksDB. Custom weight profiles must be re-created when the MCP session restarts.

### 30.8 Soft-delete vs hard-delete
- `forget_concept`: soft by default (30-day recovery via `deleted_at` in `CF_SOURCE_METADATA`).
- `delete_file_content`: soft by default (same mechanism).
- `delete_constellation`: hard delete only.
- Hard-delete order: every `CF_EMB_*`/`CF_HNSW_*`/sparse/token CF first, then `CF_FINGERPRINTS` (RocksDB-first ordering, audit-13 lessons).

### 30.9 Source-of-truth markers
Every reality-loop response carries `source_of_truth` as `file:<absolute path>` (or `sqlite:` / `git:`). Clients should treat these as the canonical reference when summarizing or quoting state.

---

## 31. Adding a new tool

Procedure (also documented in `dispatch.rs:5-9`):

1. Add the tool name constant to `crates/context-graph-mcp/src/tools/names.rs`.
2. Append a `ToolDefinition::new(...)` entry to the appropriate `definitions/<group>.rs` (or create a new submodule and wire it in `definitions/mod.rs`).
3. Update the `test_total_tool_count_and_no_duplicates` and `test_submodule_counts` assertions in `definitions/mod.rs`.
4. Add the handler method `call_X(id, args)` to the relevant `handlers/tools/*_tools.rs` (and DTO struct in `*_dtos.rs` if non-trivial).
5. Add one line to the `tool_dispatch!` invocation in `dispatch.rs:87`.
6. Update §1.1, §3, and the per-category section in this document.

For LLM-gated tools, place the `ToolDefinition::new(...)` inside `#[cfg(feature = "llm")]` and update `test_submodule_counts` accordingly.

For reality-loop tools, add the tool to `tools/names.rs` (under `// ========== CGREALITY REALITY-LOOP TOOLS ==========`), to `definitions/reality.rs`, and to `dispatch.rs` (after `// ccreality reality-loop tools`). The lightweight `--mode reality-loop` registry uses `definitions/reality.rs::definitions()` directly.

---

## 32. Schema↔documentation invariants

To keep this reference honest:

- The **count tables in §1.1** must equal the assertions in `definitions/mod.rs::tests::test_submodule_counts` and `test_total_tool_count_and_no_duplicates`.
- Every tool listed in **§3 Tool inventory** must have a `tool_dispatch!` entry in `dispatch.rs`.
- Every required parameter listed for a tool in **§4–§29** must appear in `required: [...]` in the corresponding `definitions/<group>.rs` schema.
- Defaults documented in **§4–§29** must match the `default:` field in the schema AND the handler default (audit lesson — schema-handler default mismatches caused ranking drift).
- Side effects listed in **§4–§29** must match what the corresponding `*_tools.rs` handler actually writes (verify against `tracing` calls and CF writes during code review).

If you change any of the above, update this doc in the same PR.
