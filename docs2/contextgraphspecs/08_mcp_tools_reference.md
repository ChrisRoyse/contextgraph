# MCP Tools Reference

**Source files covered:**
- `crates/context-graph-mcp/src/tools/mod.rs`
- `crates/context-graph-mcp/src/tools/names.rs` — canonical name constants
- `crates/context-graph-mcp/src/tools/aliases.rs` — alias resolution
- `crates/context-graph-mcp/src/tools/types.rs` — `ToolDefinition` shape
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` — registry assembly
- `crates/context-graph-mcp/src/tools/definitions/{capability,core,merge,curation,topic,file_watcher,sequence,causal,causal_discovery,keyword,code,graph,robustness,entity,embedder,temporal,graph_link,maintenance,provenance,daemon,training,learning,utl,constellation,contrastive,typed_edges,dynamicjepa}.rs`
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` — runtime dispatch
- `crates/context-graph-mcp/src/handlers/tools/*.rs` — handler implementations

**Cross-references:**
- See [05_embedding_subsystem.md](05_embedding_subsystem.md) for embedder identifiers (E1–E14, E15–E21).
- See [06_search_and_fusion.md](06_search_and_fusion.md) for search strategies and weight profiles.
- See [07_graph_and_causal.md](07_graph_and_causal.md) for typed edges, K-NN, and causal directions.
- See [09_storage_layer.md](09_storage_layer.md) for column families written by these tools.
- See [17_constellation_subsystem.md](17_constellation_subsystem.md) for constellation semantics.
- See [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) for AnomalyKind taxonomy.
- See [20_learning_subsystem.md](20_learning_subsystem.md) for Learning-as-UTL details.

---

## 1. Tool Registry Overview

`get_tool_definitions()` (in `crates/context-graph-mcp/src/tools/definitions/mod.rs:37`) aggregates per-category modules. Test `tools::definitions::tests::test_total_tool_count_and_no_duplicates` pins the totals:

| Build | Total tools |
| --- | --- |
| With `--features llm` (default) | **145** |
| Without `llm` (LLM-gated tools omitted) | **141** |

`names.rs` (line 1–344) defines all canonical name constants. `dispatch.rs` (line 87–263 in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`) wires every name to a handler method. `tool_dispatch!` (line 24) is the macro that generates the `match` arms.

LLM-gated tools (require `--features llm`):
- `discover_graph_relationships`, `validate_graph_link` — graph relationship LLM
- `trigger_causal_discovery`, `get_causal_discovery_status` — Qwen2.5 causal discovery
- (`validate_low_confidence_edges` — the tool exists but returns `{status:"skipped"}` when LLM disabled)

### 1.1 Tool count by category

The following counts come from `tests::test_submodule_counts` in `definitions/mod.rs:159`.

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
| embedder (cross-cutting) | 7 | 7 | `embedder.rs` (`definitions()`) |
| embedder (standalone E12/E13) | 2 | 2 | `embedder.rs` (`standalone_definitions()`) |
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
| **Total** | **145** | **141** | |

### 1.2 Aliases

Defined in `crates/context-graph-mcp/src/tools/aliases.rs:31`:

| Legacy name | Canonical name | Notes |
| --- | --- | --- |
| `consolidate_memories` | `trigger_consolidation` | renamed for clarity |
| `inject_context` | `store_memory` | merged tools — `rationale` field travels through |

`resolve_alias()` runs in `dispatch.rs:73` for every `tools/call`. Unknown names pass through unchanged.

### 1.3 ToolDefinition shape

`crates/context-graph-mcp/src/tools/types.rs:8` defines:

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value, // serialized as "inputSchema"
    pub output_schema: Option<serde_json::Value>, // optional "outputSchema"
}
```

Every schema sets `additionalProperties: false` (Audit #9 L5 — unknown params return `InvalidParams`). Schema integer fields use JSON `integer` (not `number`) to enforce type at the JSON-RPC boundary (Audit #9 H2 — e.g., `lastHours`, `lastDays` on `search_graph`).

### 1.4 Schema↔DTO alignment

Each `definitions/<group>.rs` schema is paired with a DTO module under `handlers/tools/<group>_dtos.rs`. Defaults specified in schemas (`default: …`) MUST match handler defaults (Audit #5 fix — `decayFunction` default was previously `"linear"` in the handler vs `"exponential"` in the schema). The `validate.rs` helpers at `handlers/tools/validate.rs` provide shared `additionalProperties:false` enforcement and numeric clamping utilities.

---

## 2. Protocol Methods

Source: `crates/context-graph-mcp/src/protocol.rs`. The MCP server speaks JSON-RPC 2.0 with `id` of type `Number(i64)` or `String`.

| Method | Handler | Behavior |
| --- | --- | --- |
| `initialize` | `handle_initialize` | MCP session metadata + capabilities |
| `shutdown` | `handle_shutdown` | Triggers `McpServer::shutdown()` graceful path |
| `tools/list` | `handle_tools_list` (`dispatch.rs:40`) | Returns `ToolDefinition[]` (145 with llm, 141 without) |
| `tools/call` | `handle_tools_call` (`dispatch.rs:46`) | Resolves alias (`aliases::resolve_alias`), dispatches via `tool_dispatch!` macro (`dispatch.rs:24`) |

Per-tool timeout: `config.mcp.request_timeout` is enforced via `tokio::time::timeout` for every request and every batch sub-request. On timeout: emits `LAYER_TIMEOUT` (-32007) with the original `request.id` (MCP-H2 fix — id is parsed BEFORE consuming the payload). Notifications (no `id` field per JSON-RPC 2.0 §4.1, distinguished from `id: null` per Audit #10 SRV-M2) NEVER receive a response — even on timeout.

Transport modes (`crates/context-graph-mcp/src/server/mod.rs:37` `TransportMode`):
- `Stdio` (default) — newline-delimited JSON over stdin/stdout. Supports JSON-RPC batch arrays (`[{...},{...}]`) per spec §6 (INFRA-H3 fix). Capped via `MAX_BATCH_SIZE` (`server::transport`) and `MAX_LINE_BYTES` per-line (AGT-04).
- `Tcp` — bound on `config.mcp.bind_address:tcp_port`, default `127.0.0.1:3100`, `max_connections=32` semaphore-gated. TCP-specific error codes `-32110…-32114` (see `server/transport.rs`). Selected by `--transport tcp`, `CONTEXT_GRAPH_TRANSPORT=tcp`, or `config.mcp.transport="tcp"`. **`sse` transport is rejected** (`determine_transport_mode` at `main.rs:283`).
- Daemon mode (`--daemon`): TCP daemon shared across stdio clients via stdio-to-TCP proxy. Lock file at `/tmp/context-graph-mcp-daemon.lock`; stale-lock detection uses `flock` + `kill(pid, 0)` + `/proc/pid/status` zombie check.

Errors:
- `INVALID_PARAMS` (-32602) — missing/unknown params, schema violation
- `TOOL_NOT_FOUND` (-32601) — name does not match any dispatch entry
- `LAYER_TIMEOUT` (-32007) — handler exceeded `config.mcp.request_timeout`
- `INVALID_REQUEST` (-32600) — bad JSON-RPC envelope, oversized batch, empty batch
- `PARSE_ERROR` (-32700) — malformed JSON line

---

## 3. Tool Inventory (Navigation Table)

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
| `record_graph_learning_event` | learning | `policyScope`, `sourceMemoryId`, `selectedNeighborIds`, `beforeRank`, `afterRank`, `outcome` |
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
| `record_learner_k_sleep` | utl | `learnerId`, `sessionTs`, `slowWaveMinutes` |
| `record_learner_retrieval` | utl | `learnerId`, `traceId`, `ts`, `correct`, `score` |
| `upsert_goal_centroid` | utl | `skillId`, `modality`, `vector` |
| `get_goal_distance` | utl | `learnerId`, `skillId`, `sessionTs`, `modality` |
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
| `dynamicjepa_register_domain_pack` | dynamicjepa | `dbPath`, `file` |
| `dynamicjepa_list_domain_packs` | dynamicjepa | `dbPath` |
| `dynamicjepa_get_domain_pack` | dynamicjepa | `dbPath`, `id` |
| `dynamicjepa_ingest_event` | dynamicjepa | `dbPath`, `domain`, `adapter`, `file` |
| `dynamicjepa_run_adapter` | dynamicjepa | `dbPath`, `eventId` |
| `dynamicjepa_materialize_panel` | dynamicjepa | `dbPath` |
| `dynamicjepa_get_panel` | dynamicjepa | `dbPath`, `panelId` |
| `dynamicjepa_list_instrument_readings` | dynamicjepa | `dbPath`, `eventId` |
| `dynamicjepa_create_binding` | dynamicjepa | `dbPath`, `leftCf`, `leftKey`, `rightCf`, `rightKey` |
| `dynamicjepa_list_bindings` | dynamicjepa | `dbPath` |
| `dynamicjepa_compile_trajectories` | dynamicjepa | `dbPath`, `domain` |
| `dynamicjepa_get_trajectory` | dynamicjepa | `dbPath`, `id` |
| `dynamicjepa_list_trajectories` | dynamicjepa | `dbPath`, `domain` |
| `dynamicjepa_compile_dataset` | dynamicjepa | `dbPath`, `domain` |
| `dynamicjepa_get_dataset_shard` | dynamicjepa | `dbPath`, `datasetId`, `shardId` |
| `dynamicjepa_inspect_dataset_row` | dynamicjepa | `dbPath`, `datasetId`, `shardId`, `row` |
| `dynamicjepa_train` | dynamicjepa | `dbPath`, `datasetId`, `config`, `artifactRoot` |
| `dynamicjepa_get_training_run` | dynamicjepa | `dbPath`, `id` |
| `dynamicjepa_get_artifact` | dynamicjepa | `dbPath`, `id` |
| `dynamicjepa_predict` | dynamicjepa | `dbPath`, `artifactId`, `panelId`, `actionId` |
| `dynamicjepa_plan` | dynamicjepa | `dbPath`, `artifactId`, `panelId`, `skillId` |
| `dynamicjepa_record_surprise` | dynamicjepa | `dbPath`, `predictionId`, `observedOutcomeId`, `observedPanelId` |
| `dynamicjepa_build_constellation` | dynamicjepa | `dbPath`, `domain` |
| `dynamicjepa_list_constellations` | dynamicjepa | `dbPath` |
| `dynamicjepa_get_constellation` | dynamicjepa | `dbPath`, `domain`, `modality` |
| `dynamicjepa_calibrate_threshold` | dynamicjepa | `dbPath`, `domain` |
| `dynamicjepa_recalibrate_threshold` | dynamicjepa | `dbPath`, `domain`, `supersedes`, `reason` |
| `dynamicjepa_compute_mc_ratio` | dynamicjepa | `dbPath`, `domain`, `outputDir` |
| `dynamicjepa_audit_pairwise_mi` | dynamicjepa | `dbPath`, `domain`, `outputDir` |
| `dynamicjepa_cross_domain_transfer` | dynamicjepa | `outputRoot` |
| `dynamicjepa_get_prediction` | dynamicjepa | `dbPath`, `id` |
| `dynamicjepa_get_plan_trace` | dynamicjepa | `dbPath`, `id` |
| `dynamicjepa_get_surprise` | dynamicjepa | `dbPath`, `id` |
| `dynamicjepa_inspect_counts` | dynamicjepa | `dbPath` |
| `dynamicjepa_inspect_cf` | dynamicjepa | `dbPath`, `cf` |

---

## 4. Capability Tools

### `get_capability_matrix`
**Description:** Return the MCP capability matrix: every E1–E14 content embedder, E15–E21 learner-state slots, tool groups, source-of-truth RocksDB CFs, and (optional) live runtime counts.
**Source:** `crates/context-graph-mcp/src/tools/definitions/capability.rs:11`; handler `crates/context-graph-mcp/src/handlers/tools/capability_tools.rs:25`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `includeRuntimeState` | boolean |  | `true` | Read live counts from RocksDB and daemon state. Fails closed when live state cannot be read. |
| `includeToolSchemas` | boolean |  | `false` | Inline every tool's full inputSchema. Off by default to keep responses compact. |

**Output schema:** `{version, mcp, embedders, capabilities, runtime?}`
**Side effects:** none (read-only).

---

## 5. Core Tools

### `store_memory`
**Description:** Store a single memory directly into the knowledge graph without UTL processing.
**Source:** `crates/context-graph-mcp/src/tools/definitions/core.rs:17`; handler `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs:179`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `content` | string | yes | — | 1–10000 chars |
| `rationale` | string |  | — | 1–1024 chars; when provided, mirrored in response |
| `importance` | number |  | `0.5` | `[0.0, 1.0]` clamped |
| `sessionId` | string |  | env `CLAUDE_SESSION_ID` | scopes the memory to a session |
| `operatorId` | string |  | — | tracked in audit |

**Side effects:** writes `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_SOURCE_METADATA`, all `CF_EMB_*` indexes, `CF_AUDIT_LOG`. Triggers HNSW insertion across 11 dense embedders + sparse insertion for E6/E13 + token insertion for E12.

### `store_memories`
**Description:** All-or-error batch ingestion. Each item runs the same 14-content-embedder pipeline. On any required write/readback failure, every fingerprint produced in the batch is hard-deleted before returning.
**Source:** `core.rs:55`; handler `memory_tools.rs:614`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `items` | array | yes | — | 1–64 items, each `{content, rationale?, importance?, sessionId?, operatorId?}` |
| `sessionId` | string |  | — | default for items missing `sessionId` |
| `operatorId` | string |  | — | default for items missing `operatorId` |
| `causalHints` | boolean |  | `false` | request LLM E5 causal hints before embedding |

**Output schema:** `{requestedCount, storedCount, embedderCount, items}`.
**Side effects:** same as `store_memory`, batched; rolls back fingerprints on partial failure.

### `get_memetic_status`
**Description:** System status — fingerprint count, 14 embedder count, storage backend/size, layer status.
**Source:** `core.rs:127`; handler `crates/context-graph-mcp/src/handlers/tools/status_tools.rs:21`.
**Params:** none. **Side effects:** none.

### `search_graph`
**Description:** Multi-space semantic search across 14 embedders. Auto-applies asymmetric E5 (cause↔effect) for causal queries, E10 for paraphrase, E8 for graph. Supports temporal (E2/E3/E4), state-conditioned, and rerank pipelines.
**Source:** `core.rs:140`; handler `memory_tools.rs:1267`.

| Param | Type | Req | Default | Range / Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | search text |
| `topK` | integer |  | `10` | 1–100 |
| `minSimilarity` | number |  | `0.0` | `[0,1]` |
| `includeContent` | boolean |  | `false` | include content text in results |
| `strategy` | string |  | `multi_space` | `e1_only` \| `multi_space` \| `pipeline` |
| `weightProfile` | string |  | — | one of 20 named profiles (see [06_search_and_fusion.md](06_search_and_fusion.md) §profiles); includes `temporal_navigation`, `sequence_navigation`, `conversation_history`, `multilingual_search`, `long_context`, `translation_finder`, `affect_repair`, `affect_priming`, `affect_neutral`, etc. |
| `stateConditionedProfile` | boolean |  | `false` | resolve weight profile via `CF_LEARNER_STATE_HISTORY` for `learnerId`+`learnerSessionTs` |
| `learnerId` | uuid |  | — | with `stateConditionedProfile=true` |
| `learnerSessionTs` | integer |  | — | with `stateConditionedProfile=true` |
| `customWeights` | object (E1–E14) |  | — | overrides `weightProfile`; values 0–1, sum ≈ 1.0 |
| `excludeEmbedders` | array<E1..E14> |  | — | weight set to 0; remainder renormalized |
| `useQuantizedPrefilter` | boolean |  | `false` | PQ-8 prefilter |
| `includeEmbedderBreakdown` | boolean |  | `false` | per-embedder scores per result |
| `enableRerank` | boolean |  | `false` | enable Stage 3 ColBERT (E12) rerank |
| `enableAsymmetricE5` | boolean |  | `true` | enable cause↔effect direction modifiers |
| `causalDirection` | string |  | `auto` | `auto` \| `cause` \| `effect` \| `none` |
| `enableQueryExpansion` | boolean |  | `false` | expand causal queries |
| `temporalWeight` | number |  | `0.0` | `[0,1]` post-retrieval boost |
| `conversationContext` | object |  | — | `{anchorToCurrentTurn?, turnsBack?, turnsForward?}` |
| `sessionScope` | string |  | `all` | `current` \| `all` \| `recent` |
| `decayHalfLifeSecs` | integer |  | `86400` | exponential half-life |
| `lastHours` | integer |  | — | filter to last N hours |
| `lastDays` | integer |  | — | filter to last N days |
| `sessionId` | string |  | — | exact session filter |
| `periodicBoost` | number |  | — | E3 boost weight |
| `targetHour` | integer |  | — | 0–23 |
| `targetDayOfWeek` | integer |  | — | 0=Sun, 6=Sat |
| `sequenceAnchor` | uuid |  | — | E4 anchor |
| `sequenceDirection` | string |  | — | `before` \| `after` \| `around` \| `both` |
| `includeProvenance` | boolean |  | `false` | retrieval provenance metadata |
| `decayFunction` | string |  | `exponential` | `linear` \| `exponential` \| `step` \| `none` \| `no_decay` |
| `temporalScale` | string |  | `meso` | `micro` \| `meso` \| `macro` \| `long` \| `archival` |

**Side effects:** none (read-only).

### `trigger_consolidation`
**Description:** Identify merge candidates. **Does NOT auto-merge.** Use `merge_concepts` to execute.
**Source:** `core.rs:376`; handler `crates/context-graph-mcp/src/handlers/tools/consolidation.rs:248`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `strategy` | string |  | `similarity` | `similarity` \| `temporal` \| `semantic` |
| `min_similarity` | number |  | `0.925` | SRC-3 normalized `[0,1]` |
| `max_memories` | integer |  | `100` | 1–10000 |

**Alias:** `consolidate_memories`. **Side effects:** none (returns candidate pairs).

---

## 6. Merge & Curation

### `merge_concepts`
**Description:** Merge ≥2 concept nodes into a unified node with reversal_hash for 30-day undo.
**Source:** `crates/context-graph-mcp/src/tools/definitions/merge.rs:16`; handler in `curation_tools.rs` / `memory_tools.rs`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `source_ids` | array<uuid> | yes | — | 2–10 UUIDs |
| `target_name` | string | yes | — | 1–256 chars |
| `merge_strategy` | string |  | `union` | `union` \| `intersection` \| `weighted_average` |
| `rationale` | string | yes | — | 1–1024 chars (PRD 0.3) |
| `force_merge` | boolean |  | `false` | bypass prior-conflict guard |

**Side effects:** writes target into `CF_FINGERPRINTS`, `CF_MERGE_HISTORY`, `CF_AUDIT_LOG`; sources soft-deleted; reversal hash retained 30 days.

### `forget_concept`
**Description:** Soft-delete with 30-day recovery (SEC-06) or permanent delete.
**Source:** `crates/context-graph-mcp/src/tools/definitions/curation.rs:21`; handler `crates/context-graph-mcp/src/handlers/tools/curation_tools.rs:38`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `node_id` | uuid | yes | — | memory UUID |
| `soft_delete` | boolean |  | `true` | `false` = permanent delete |
| `operator_id` | string |  | — | provenance |
| `reason` | string |  | — | audit text |

**Side effects:** writes tombstone in `CF_SYSTEM` + `CF_AUDIT_LOG`; soft-delete blocks fingerprint from search.

### `boost_importance`
**Description:** Adjust importance by `delta`; final value clamped `[0,1]` (BR-MCP-002).
**Source:** `curation.rs:53`; handler `curation_tools.rs:163`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `node_id` | uuid | yes | — | — |
| `delta` | number | yes | — | `[-1.0, 1.0]` |
| `operator_id` | string |  | — | provenance |

**Side effects:** updates `CF_FINGERPRINTS.importance`, appends `CF_IMPORTANCE_HISTORY`, `CF_AUDIT_LOG`.

---

## 7. Topic Tools

### `get_topic_portfolio`
**Description:** All discovered topics with profiles + stability + tier info. Topics emerge from weighted multi-space clustering (`weighted_agreement >= 2.5`); E2–E4 excluded (AP-60).
**Source:** `topic.rs:20`; handler `topic_tools.rs:192`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `format` | string |  | `standard` | `brief` \| `standard` \| `verbose` |

### `get_topic_stability`
**Description:** Portfolio-level churn rate, entropy, phase breakdown.
**Source:** `topic.rs:39`; handler `topic_tools.rs:367`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `hours` | integer |  | `6` | 1–168 |

### `detect_topics`
**Description:** Force HDBSCAN recalculation; minimum 3 memories.
**Source:** `topic.rs:57`; handler `topic_tools.rs:441`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `force` | boolean |  | `false` | bypass recent-cache check |
| `max_memories` | integer |  | `10000` | 1–50000 |

**Side effects:** writes `CF_TOPICS`, `CF_TOPIC_MEMBERSHIPS`.

### `get_divergence_alerts`
**Description:** Divergence vs recent activity using SEMANTIC embedders only (E1, E6, E7, E10, E12, E13). E5 excluded (AP-77), E2–E4 excluded (AP-63).
**Source:** `topic.rs:82`; handler `topic_tools.rs:684`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `lookback_hours` | integer |  | `2` | 1–48 |

---

## 8. File Watcher Tools

Source: `crates/context-graph-mcp/src/tools/definitions/file_watcher.rs`; handler `crates/context-graph-mcp/src/handlers/tools/file_watcher_tools.rs`.

### `list_watched_files`
**Description:** Files with embeddings + chunk counts + last update.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `include_counts` | boolean |  | `true` | per-file chunk counts |
| `path_filter` | string |  | — | glob pattern (e.g., `**/docs/*.md`) |

### `get_file_watcher_stats`
**Description:** Aggregate stats — total files, total chunks, avg/min/max.
**Params:** none.

### `delete_file_content`
**Description:** Delete embeddings for a path; soft-delete supported (SEC-06).

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `file_path` | string | yes | — | absolute path |
| `soft_delete` | boolean |  | `true` | 30-day recovery |

**Side effects:** removes fingerprints + HNSW entries; writes `CF_AUDIT_LOG` rows.

### `reconcile_files`
**Description:** Find orphaned embeddings (file missing on disk) and optionally delete.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `dry_run` | boolean |  | `true` | preview only |
| `base_path` | string |  | — | scope to a subtree |

**Side effects:** soft-deletes orphans when `dry_run=false`.

---

## 9. Sequence Tools (E4 V_ordering)

Source: `sequence.rs`; handler `sequence_tools.rs`.

### `get_conversation_context`
**Description:** Memories around the current turn with auto-anchoring.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `direction` | string |  | `before` | `before` \| `after` \| `both` |
| `windowSize` | integer |  | `10` | 1–50 |
| `sessionOnly` | boolean |  | `true` | scope to current session |
| `includeContent` | boolean |  | `true` | full text |
| `query` | string |  | — | optional semantic filter |
| `minSimilarity` | number |  | `0.0` | `[0,1]` |

### `get_session_timeline`
**Description:** Ordered timeline; sorted by `session_sequence` ascending; emits human-readable position labels.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `sessionId` | string |  | current | session UUID |
| `limit` | integer |  | `50` | 1–200 |
| `offset` | integer |  | `0` | ≥0 |
| `sourceTypes` | array<string> |  | all | `HookDescription` \| `ClaudeResponse` \| `Manual` \| `MDFileChunk` |
| `includeContent` | boolean |  | `false` | full text |

### `traverse_memory_chain`
**Description:** Multi-hop chain navigation from an anchor.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `anchorId` | uuid | yes | — | starting memory |
| `direction` | string |  | `backward` | `forward` \| `backward` \| `bidirectional` |
| `hops` | integer |  | `5` | 1–20 |
| `semanticFilter` | string |  | — | optional topic filter |
| `minSimilarity` | number |  | `0.3` | `[0,1]` |
| `includeContent` | boolean |  | `true` | full text |

### `compare_session_states`
**Description:** Compare memory state at two sequence points.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `beforeSequence` | int \| string | yes | — | integer or `"start"` |
| `afterSequence` | int \| string | yes | — | integer or `"current"` |
| `sessionId` | string |  | current | scope |

---

## 10. Causal Tools (E5)

Source: `causal.rs`; handler `causal_tools.rs` and `causal_relationship_tools.rs`.

### `search_causal_relationships`
**Description:** E5 asymmetric similarity + LLM 1–3 paragraph descriptions with provenance to source memories.
**Source:** `causal.rs:16`; handler `causal_relationship_tools.rs:74`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | natural-language causal query |
| `direction` | string |  | `all` | `cause` \| `effect` \| `all` |
| `topK` | integer |  | `10` | 1–100 |
| `includeSource` | boolean |  | `true` | include original source content |
| `includeProvenance` | boolean |  | `false` | retrieval provenance metadata |
| `sourceWeight` | number |  | `0.6` | `[0,1]` source-anchored embedding weight |
| `explanationWeight` | number |  | `0.4` | `[0,1]` explanation embedding weight |
| `multiEmbedder` | boolean |  | `false` | E1+E5+E8+E11 consensus (requires direction `cause`/`effect`) |
| `minConsensus` | number |  | `0.0` | `[0,1]` |
| `e1Weight` | number |  | `0.30` | `[0,1]` consensus weight |
| `e5Weight` | number |  | `0.35` | `[0,1]` |
| `e8Weight` | number |  | `0.15` | `[0,1]` |
| `e11Weight` | number |  | `0.20` | `[0,1]` |

### `search_causes`
**Description:** Abductive — find causes of effects with 0.8× effect→cause dampening (AP-77).
**Source:** `causal.rs:111`; handler `causal_tools.rs:59`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | observed effect |
| `topK` | integer |  | `10` | 1–50 |
| `minScore` | number |  | `0.1` | `[0,1]` |
| `includeContent` | boolean |  | `false` | — |
| `filterCausalDirection` | string |  | — | `cause` \| `effect` \| `unknown` |
| `searchScope` | string |  | `memories` | `memories` \| `relationships` \| `all` |
| `strategy` | string |  | — | `e1_only` \| `multi_space` \| `pipeline` |
| `rerankWeight` | number |  | `0.4` | `[0,1]`; only for `pipeline` |

### `search_effects`
**Description:** Predictive — find effects of causes with 1.2× cause→effect boost.
**Source:** `causal.rs:172`; handler `causal_tools.rs:444`. **Same params as `search_causes`** with `query` describing the cause.

### `get_causal_chain`
**Description:** Transitive causal chain from anchor; hop attenuation 0.9^hop.
**Source:** `causal.rs:232`; handler `causal_tools.rs:813`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `anchorId` | uuid | yes | — | starting memory |
| `direction` | string |  | `forward` | `forward` \| `backward` |
| `maxHops` | integer |  | `5` | 1–10 |
| `minSimilarity` | number |  | `0.3` | `[0,1]` |
| `includeContent` | boolean |  | `false` | — |

---

## 11. Causal Discovery Tools (LLM)

LLM-gated; require `--features llm`. Source: `causal_discovery.rs`; handler `causal_discovery_tools.rs`.

### `trigger_causal_discovery`
**Description:** Run the Qwen2.5 causal discovery agent. FP16 on RTX 5090. ~6GB VRAM (3B), ~14GB (7B). Confirmed relationships call `CausalModel.embed_dual()` to produce asymmetric E5 vectors.
**Source:** `causal_discovery.rs:32`; handler `causal_discovery_tools.rs:85`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `mode` | string |  | `extract` | `extract` (within-memory) \| `pairs` (between memories) |
| `maxPairs` | integer |  | `50` | 1–200 |
| `minConfidence` | number |  | `0.7` | `[0.5, 1.0]` |
| `sessionScope` | string |  | `all` | `current` \| `all` \| `recent` |
| `session_id` | string |  | — | with `sessionScope=current` for exact match |
| `dryRun` | boolean |  | `false` | analyze without writing edges/embeddings |

**Side effects:** writes `CF_CAUSAL_RELATIONSHIPS`, `CF_CAUSAL_BY_SOURCE`; updates E5 cause/effect HNSW indexes.

### `get_causal_discovery_status`
**Description:** Agent status, last cycle stats, VRAM, cumulative counts.
**Source:** `causal_discovery.rs:84`; handler `causal_discovery_tools.rs:485`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `includeLastResult` | boolean |  | `true` | include last cycle's analyzed pairs |
| `includeGraphStats` | boolean |  | `true` | causal graph node/edge counts |

---

## 12. Keyword / Code / Robustness

### `search_by_keywords` (E6)
Source: `keyword.rs:23`; handler `keyword_tools.rs:59`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | phrase or keywords |
| `topK` | integer |  | `10` | 1–50 |
| `minScore` | number |  | `0.1` | `[0,1]` |
| `blendWithSemantic` | number |  | `0.3` | `[0,1]` (0=pure E1, 1=pure E6) |
| `useSpladeExpansion` | boolean |  | `true` | E13 SPLADE expansion |
| `includeContent` | boolean |  | `false` | — |
| `strategy` | string |  | `multi_space` | `e1_only` \| `multi_space` \| `pipeline` |

### `search_code` (E7)
Source: `code.rs:23`; handler `code_tools.rs:62`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | code description or pattern |
| `topK` | integer |  | `10` | 1–50 |
| `minScore` | number |  | `0.2` | `[0,1]` |
| `blendWithSemantic` | number |  | `0.4` | `[0,1]` |
| `includeContent` | boolean |  | `false` | — |
| `searchMode` | string |  | `hybrid` | `hybrid` \| `e7Only` \| `e1WithE7Rerank` \| `pipeline` |
| `languageHint` | string |  | — | `rust` \| `python` \| `javascript` \| `typescript` \| `go` \| `java` \| `cpp` \| `sql` |

### `search_robust` (E9 HDC)
Source: `robustness.rs:34`; handler `robustness_tools.rs:73`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | minLength 3 (trigram requirement) |
| `topK` | integer |  | `10` | 1–50 |
| `minScore` | number |  | `0.1` | `[0,1]` |
| `e9DiscoveryThreshold` | number |  | `0.08` | `[0,1]` E9 score to mark as 'E9 discovery' |
| `e1WeaknessThreshold` | number |  | `0.5` | `[0,1]` max E1 to qualify as missed |
| `includeContent` | boolean |  | `false` | — |
| `includeE9Score` | boolean |  | `true` | per-result E9/E1 split |
| `strategy` | string |  | `multi_space` | `e1_only` \| `multi_space` \| `pipeline` |

---

## 13. Entity Tools (E11 / TransE)

Source: `entity.rs`; handler `entity_tools.rs`.

### `extract_entities`
Source: `entity.rs:24`; handler `entity_tools.rs:622`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `text` | string | yes | — | text to canonicalize |
| `includeUnknown` | boolean |  | `true` | include heuristic-detected entities |
| `groupByType` | boolean |  | `false` | group by entity type |

### `search_by_entities`
Source: `entity.rs:53`; handler `entity_tools.rs:760`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `entities` | array<string> | yes | — | names; canonicalized |
| `entityTypes` | array<enum> |  | — | `ProgrammingLanguage` \| `Framework` \| `Database` \| `Cloud` \| `Company` \| `TechnicalTerm` \| `Unknown` |
| `matchMode` | string |  | `any` | `any` \| `all` |
| `topK` | integer |  | `10` | 1–50 |
| `minScore` | number |  | `0.2` | `[0,1]` |
| `includeContent` | boolean |  | `false` | — |
| `boostExactMatch` | number |  | `1.15` | `[1.0, 3.0]` |
| `strategy` | string |  | — | `e1_only` \| `multi_space` \| `pipeline` |

### `infer_relationship` (TransE r̂ = t − h)
Source: `entity.rs:118`; handler `entity_tools.rs:1158`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `headEntity` | string | yes | — | subject |
| `tailEntity` | string | yes | — | object |
| `headType` | enum |  | — | type hint |
| `tailType` | enum |  | — | type hint |
| `topK` | integer |  | `5` | 1–20 |
| `includeScore` | boolean |  | `true` | raw TransE scores |

### `find_related_entities`
Source: `entity.rs:163`; handler `entity_tools.rs:1350`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `entity` | string | yes | — | source entity |
| `relation` | string | yes | — | predicate name |
| `direction` | string |  | `outgoing` | `outgoing` (h→t) \| `incoming` (t←h) |
| `entityType` | enum |  | — | restrict result type |
| `topK` | integer |  | `10` | 1–50 |
| `minScore` | number |  | — | `[-10, 0]` (TransE distance, more negative = looser) |
| `searchMemories` | boolean |  | `true` | filter to entities found in stored memories |

### `validate_knowledge`
Source: `entity.rs:215`; handler `entity_tools.rs:1581`. Computes `score = -||h + r - t||₂`. Returns `valid` / `uncertain` / `unlikely`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `subject` | string | yes | — | head |
| `predicate` | string | yes | — | relation |
| `object` | string | yes | — | tail |
| `subjectType` | enum |  | — | type hint |
| `objectType` | enum |  | — | type hint |

### `get_entity_graph`
Source: `entity.rs:252`; handler `entity_tools.rs:1875`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `centerEntity` | string |  | — | focal entity |
| `maxNodes` | integer |  | `50` | 1–500 |
| `entityTypes` | array<enum> |  | all | filter |
| `minRelationScore` | number |  | `0.3` | `[0,1]` |
| `includeMemoryCounts` | boolean |  | `true` | reference counts per node |

---

## 14. Embedder Tools

Source: `embedder.rs`; handler `embedder_tools.rs`. The cross-cutting embedder list excludes E6/E12/E13 (non-HNSW) — they use `search_by_tokens`/`search_by_expansion` standalone. E14 is BGE-M3 Dense (1024-D, multilingual, 8192-token context).

### `search_by_embedder`
Source: `embedder.rs:37`; handler `embedder_tools.rs:71`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `embedder` | enum | yes | — | `E1`–`E14` excluding `E6`,`E12`,`E13` (11 values) |
| `query` | string | yes | — | — |
| `topK` | integer |  | `10` | 1–100 |
| `minSimilarity` | number |  | `0` | `[0,1]` |
| `includeContent` | boolean |  | `false` | — |
| `includeAllScores` | boolean |  | `false` | per-embedder scores in each result |

### `get_embedder_clusters`
Source: `embedder.rs:91`; handler `embedder_tools.rs:322`. Same embedder enum (11 values).

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `embedder` | enum | yes | — | E1–E14 (11 HNSW-capable) |
| `minClusterSize` | integer |  | `3` | 2–50 |
| `topClusters` | integer |  | `10` | 1–50 |
| `includeSamples` | boolean |  | `true` | sample memories |
| `samplesPerCluster` | integer |  | `3` | 1–10 |

### `compare_embedder_views`
Source: `embedder.rs:137`; handler `embedder_tools.rs:543`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | — |
| `embedders` | array<enum> | yes | — | 2–5 embedders, same 11-value enum |
| `topK` | integer |  | `5` | 1–20 |
| `includeContent` | boolean |  | `false` | — |

### `list_embedder_indexes`
Source: `embedder.rs:178`; handler `embedder_tools.rs:804`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `includeDetails` | boolean |  | `true` | memory usage + latency |

### `get_memory_fingerprint`
Source: `embedder.rs:196`; handler `embedder_tools.rs:941`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memoryId` | uuid | yes | — | — |
| `embedders` | array<E1..E14> |  | all 14 | filter |
| `includeVectorNorms` | boolean |  | `true` | L2 norms |
| `includeContent` | boolean |  | `false` | — |

### `create_weight_profile`
Source: `embedder.rs:236`; handler `embedder_tools.rs:1242`. Session-scoped; rejects built-in profile names.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `name` | string | yes | — | 1–64 chars, must not collide with built-ins |
| `weights` | object | yes | — | E1–E14 numbers in `[0,1]`, sum ≈ 1.0 |
| `description` | string |  | — | free text |

### `search_cross_embedder_anomalies`
Source: `embedder.rs:282`; handler `embedder_tools.rs:1362`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | — |
| `highEmbedder` | enum<E1..E14> | yes | — | expected HIGH |
| `lowEmbedder` | enum<E1..E14> | yes | — | expected LOW |
| `highThreshold` | number |  | `0.5` | `[0,1]` |
| `lowThreshold` | number |  | `0.3` | `[0,1]` |
| `topK` | integer |  | `10` | 1–100 |
| `includeContent` | boolean |  | `false` | — |
| `primaryEmbedder` | enum<E1..E14> |  | `E1` | for generalized blind-spot mode |
| `contrastEmbedder` | enum<E1..E14> |  | `E9` | for generalized blind-spot mode |

### `search_by_tokens` (E12 ColBERT MaxSim, standalone)
Source: `embedder.rs:354`; handler `embedder_tools.rs:1576`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | — |
| `topK` | integer |  | `10` | 1–100 |
| `minSimilarity` | number |  | `0.3` | `[0,1]` |

### `search_by_expansion` (E13 SPLADE, standalone)
Source: `embedder.rs:368`; handler `embedder_tools.rs:1642`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | — |
| `topK` | integer |  | `10` | 1–100 |
| `minScore` | number |  | `0.1` | `[0,1]` |

---

## 15. Temporal Tools

Source: `temporal.rs`; handler `temporal_tools.rs`. Both apply boost POST-retrieval (ARCH-25), not in similarity fusion.

### `search_recent` (E2 freshness)
Source: `temporal.rs:18`; handler `temporal_tools.rs:34`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | — |
| `topK` | integer |  | `10` | 1–100 |
| `temporalWeight` | number |  | `0.3` | `[0.1, 1.0]` |
| `decayFunction` | string |  | `exponential` | `linear` \| `exponential` \| `step` \| `none` \| `no_decay` |
| `temporalScale` | string |  | `meso` | `micro` (1h) \| `meso` (1d) \| `macro` (1w) \| `long` (1mo) \| `archival` (1y) |
| `includeContent` | boolean |  | `true` | — |
| `minSimilarity` | number |  | `0.1` | `[0,1]` |

### `search_periodic` (E3 V_periodicity)
Source: `temporal.rs:76`; handler `temporal_tools.rs:203`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | — |
| `topK` | integer |  | `10` | 1–100 |
| `targetHour` | integer |  | — | 0–23 |
| `targetDayOfWeek` | integer |  | — | 0=Sun, 6=Sat |
| `autoDetect` | boolean |  | `false` | use current time if targets omitted |
| `periodicWeight` | number |  | `0.3` | `[0.1, 1.0]` |
| `includeContent` | boolean |  | `true` | — |
| `minSimilarity` | number |  | `0.1` | `[0,1]` |

---

## 16. Graph Tools

### `search_connections` (E8 asymmetric)
Source: `graph.rs:20`; handler `graph_tools.rs:81`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `query` | string | yes | — | — |
| `direction` | string |  | `both` | `source` \| `target` \| `both` |
| `topK` | integer |  | `10` | 1–50 |
| `minScore` | number |  | `0.1` | `[0,1]` |
| `includeContent` | boolean |  | `false` | — |
| `filterGraphDirection` | string |  | — | `source` \| `target` \| `unknown` |
| `includeProvenance` | boolean |  | `false` | — |

### `get_graph_path`
Source: `graph.rs:74`; handler `graph_tools.rs:400`. Hop attenuation 0.9^hop.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `anchorId` | uuid | yes | — | starting node |
| `direction` | string |  | `forward` | `forward` \| `backward` |
| `maxHops` | integer |  | `5` | 1–10 |
| `minSimilarity` | number |  | `0.3` | `[0,1]` |
| `includeContent` | boolean |  | `false` | — |

### `discover_graph_relationships` (LLM)
Source: `graph.rs:125`; handler `graph_tools.rs:679`. 20 relationship types across Code, Legal, Academic, General domains.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `memory_ids` | array<uuid> | yes | — | 2–50 |
| `relationship_types` | array<enum> |  | all | 19 enum values incl. `contains`, `imports`, `cites`, `interprets`, `implements`, `extends`, `supersedes`, `calls`, `applies`, `used_by`, etc. |
| `min_confidence` | number |  | `0.7` | `[0,1]` |
| `batch_size` | integer |  | `50` | 1–100 |

**Side effects:** writes typed edges into `CF_TYPED_EDGES` + reverse indexes.

### `validate_graph_link` (LLM)
Source: `graph.rs:180`; handler `graph_tools.rs:1014`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `source_id` | uuid | yes | — | "points to" memory |
| `target_id` | uuid | yes | — | "pointed to" memory |
| `expected_relationship_type` | enum |  | — | one of 19 relationship enums |

---

## 17. Graph-Link Tools (K-NN + Typed Edges)

Source: `graph_link.rs`; handler `graph_link_tools.rs`.

### `get_memory_neighbors`
Source: `graph_link.rs:22`; handler `graph_link_tools.rs:72`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `memory_id` | uuid | yes | — | — |
| `embedder_id` | integer |  | `0` | 0–12 (0=E1, 4=E5, 6=E7, 7=E8, 9=E10, 10=E11) |
| `top_k` | integer |  | `10` | 1–50 |
| `min_similarity` | number |  | `0.0` | `[0,1]` |
| `include_content` | boolean |  | `false` | — |

NOTE: returns 0 results until the background K-NN graph builder runs (~60s after store).

### `get_typed_edges`
Source: `graph_link.rs:70`; handler `graph_link_tools.rs:343`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | uuid | yes | — | — |
| `edge_type` | enum |  | — | `semantic_similar` \| `code_related` \| `entity_shared` \| `causal_chain` \| `graph_connected` \| `paraphrase_aligned` \| `keyword_overlap` \| `multi_agreement` |
| `direction` | enum |  | `outgoing` | `outgoing` \| `incoming` \| `both` |
| `min_weight` | number |  | `0.0` | `[0,1]` |
| `include_content` | boolean |  | `false` | — |

### `traverse_graph`
Source: `graph_link.rs:123`; handler `graph_link_tools.rs:648`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `start_memory_id` | uuid | yes | — | — |
| `max_hops` | integer |  | `2` | 1–5 |
| `edge_type` | enum |  | — | same 8 edge types |
| `min_weight` | number |  | `0.3` | `[0,1]` |
| `max_results` | integer |  | `20` | 1–100 |
| `include_content` | boolean |  | `false` | — |

### `get_unified_neighbors`
Source: `graph_link.rs:184`; handler `graph_link_tools.rs:948`. Weighted RRF fusion across all 14 embedders (ARCH-21). Temporal embedders (E2-E4) excluded by default (AP-60).

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `memory_id` | uuid | yes | — | — |
| `weight_profile` | enum |  | `semantic_search` | 20 named profiles (same set as `search_graph`) |
| `top_k` | integer |  | `10` | 1–50 |
| `min_score` | number |  | `0.0` | `[0,1]` RRF score |
| `include_content` | boolean |  | `false` | — |
| `include_embedder_breakdown` | boolean |  | `true` | per-embedder scores + ranks |
| `custom_weights` | object<E1..E14> |  | — | overrides profile |
| `exclude_embedders` | array<E1..E14> |  | — | renormalize remainder |
| `learning_policy` | object |  | — | `{policy_scope*, min_evidence?, max_scan?, learning_rate?, include_evidence?}` — fails if no matching graph LearningEvents |

---

## 18. Graph-Learning Tools

Source: `learning.rs` (within `definitions/learning.rs`); handler `crates/context-graph-mcp/src/handlers/tools/graph_learning_tools.rs`.

### `record_graph_learning_event`
Source: `learning.rs:259`; handler `graph_learning_tools.rs:217`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `eventId` | uuid |  | generated | — |
| `policyScope` | string | yes | — | stored as `taskId=graph_learning:<scope>` |
| `graphTool` | enum |  | `get_unified_neighbors` | `get_unified_neighbors` \| `get_memory_neighbors` \| `traverse_graph` |
| `sourceMemoryId` | uuid | yes | — | — |
| `selectedNeighborIds` | array<uuid> | yes | — | minItems 1 |
| `rejectedNeighborIds` | array<uuid> |  | `[]` | — |
| `weightProfile` | string |  | `semantic_search` | — |
| `beforeRank` | integer | yes | — | minimum 1 |
| `afterRank` | integer | yes | — | minimum 1 |
| `query` | string |  | `""` | — |
| `sessionId` | string |  | — | — |
| `responseId` | string |  | — | — |
| `outcome` | object | yes | — | `{label*, utilityDelta*, correctionRequired?, reuseObserved?}` |
| `outcomeReason` | string |  | — | — |

**Side effects:** writes a row into `CF_LEARNING_EVENTS` then reads it back for verification. Rejects when graph evidence is missing.

### `resolve_graph_learning_policy`
Source: `learning.rs:298`; handler `graph_learning_tools.rs:435`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `policyScope` | string | yes | — | — |
| `sourceMemoryId` | uuid |  | — | optional filter |
| `baseWeightProfile` | string |  | `semantic_search` | — |
| `minEvidence` | integer |  | `1` | 1–10000 |
| `maxScan` | integer |  | `10000` | 1–200000 |
| `learningRate` | number |  | `0.35` | `[0,1]` |
| `includeEvidence` | boolean |  | `true` | — |

---

## 19. Maintenance / Provenance / Daemon

### `repair_causal_relationships`
Source: `maintenance.rs:14`; handler `maintenance_tools.rs:16`.
**Description:** Scan `CF_CAUSAL_RELATIONSHIPS` and delete entries that fail deserialization. Returns `(deleted_count, total_scanned)`.
**Params:** none.
**Side effects:** deletes corrupt rows.

### `get_audit_trail`
Source: `provenance.rs:13`; handler `provenance_tools.rs:35`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `target_id` | uuid |  | — | optional memory filter |
| `start_time` | string (ISO 8601) |  | — | RFC-3339 lower bound |
| `end_time` | string (ISO 8601) |  | — | RFC-3339 upper bound |
| `limit` | integer |  | `50` | max 500 |

### `get_merge_history`
Source: `provenance.rs:40`; handler `provenance_tools.rs:144`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | uuid | yes | — | — |
| `include_source_metadata` | boolean |  | `false` | — |

### `get_provenance_chain`
Source: `provenance.rs:60`; handler `provenance_tools.rs:269`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memory_id` | uuid | yes | — | — |
| `include_audit` | boolean |  | `false` | — |
| `include_embedding_version` | boolean |  | `false` | — |
| `include_importance_history` | boolean |  | `false` | — |
| `include_merge_history` | boolean |  | `false` | — |
| `depth_full` | boolean |  | `false` | shortcut for all-true |

### `daemon_status`
Source: `daemon.rs:11`; handler `daemon_tools.rs:54`.
**Description:** Active connection count, model state, GC/HNSW-persist/graph-builder background task state, uptime, PID.
**Params:** none.

---

## 20. Training Tools

Source: `training.rs`; handler `training_tools.rs`.

### `export_training_corpus`
Source: `training.rs:16`; handler `training_tools.rs:63`. Each record bundles content, all 14 embedding vectors, 14D topic profile, 91D synergy cross-correlations, 6D group alignments, outgoing typed edges, K-NN, LLM causal labels, optional Tucker-1 decomposition.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `filter` | enum |  | `all` | `all` \| `session` |
| `filterId` | string |  | — | session id when `filter=session` |
| `maxMemories` | integer |  | `10000` | 1–1000000 |
| `includeEmbeddings` | boolean |  | `true` | dense E1/E2/E3/E4/E5/E7/E8/E9/E10/E11/E14 |
| `includeSparseVectors` | boolean |  | `true` | E6 + E13 |
| `includeTokenEmbeddings` | boolean |  | `false` | E12 (~15KB/record) |
| `includeEdges` | boolean |  | `true` | outgoing + per-embedder K-NN |
| `includeCausalLabels` | boolean |  | `true` | LLM causal_effects (causal_causes empty in v1) |
| `includeIncomingEdges` | boolean |  | `false` | requires O(N) reverse-index scan |
| `includeTemporalLabels` | boolean |  | `true` | hour, dow, periodic bucket, age, seq pos, E2/E3/E4 norms |
| `includeTuckerCore` | boolean |  | `false` | Phase-4 Tucker-1 (4,4,128) of 14×14×1024 tensor |
| `clearExisting` | boolean |  | `false` | wipe `CF_TRAINING_RECORDS` first |

**Side effects:** writes `CF_TRAINING_RECORDS` (binary bincode + version byte).

### `list_training_records`
Source: `training.rs:99`; handler `training_tools.rs:559`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | `50` | 1–1000 |
| `offset` | integer |  | `0` | ≥0 |
| `includeShape` | boolean |  | `true` | — |

### `get_training_record`
Source: `training.rs:130`; handler `training_tools.rs:660`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `memoryId` | uuid | yes | — | — |
| `includeVectors` | boolean |  | `false` | dense + sparse |
| `includeTokenEmbeddings` | boolean |  | `false` | E12 |
| `includeEdges` | boolean |  | `true` | outgoing + incoming + K-NN |

### `count_training_records`
Source: `training.rs:164`; handler `training_tools.rs:705`. **Params:** none.

---

## 21. Learning-as-UTL Tools

Source: `learning.rs`; handler `learning_tools.rs`. These are event-level UTL embedders (5 baseline signals: `delta_e`, `surprise`, `coherence`, `consolidation`, `transfer`), distinct from the E1–E14 content embedders.

### `record_learning_event`
Source: `learning.rs:9`; handler `learning_tools.rs:188`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `eventId` | uuid |  | generated | — |
| `memoryIds` | array<uuid> |  | `[]` | — |
| `sessionId` | string |  | — | — |
| `responseId` | string |  | — | — |
| `taskId` | string |  | — | — |
| `query` | string |  | `""` | — |
| `retrievedContext` | string |  | `""` | — |
| `assistantResponse` | string |  | `""` | — |
| `before` | object | yes | — | `{topicProfile[14], crossCorrelations[91], retrievalRank?, embedderScores[14]?, contradictionPressure?, integrationConfidence?, recurrenceCount?, stabilityScore?, domain?, successfulTransferCount?}` |
| `after` | object | yes | — | same shape as `before` |
| `outcome` | object | yes | — | `{label*: useful\|neutral\|harmful\|no_learning, utilityDelta?, correctionRequired?, reuseObserved?}` |

**Side effects:** writes `CF_LEARNING_EVENTS`; signal embedders run server-side.

### `list_learning_events`
Source: `learning.rs:64`; handler `learning_tools.rs:306`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | `50` | 1–1000 |
| `offset` | integer |  | `0` | ≥0 |
| `includeFeatures` | boolean |  | `true` | — |

### `get_learning_event`
Source: `learning.rs:78`; handler `learning_tools.rs:383`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `eventId` | uuid | yes | — | — |
| `includeText` | boolean |  | `true` | — |
| `includeSignals` | boolean |  | `true` | — |

### `count_learning_events`
Source: `learning.rs:92`; handler `learning_tools.rs:423`. **Params:** none.

### `list_learning_signal_embedders`
Source: `learning.rs:102`; handler `learning_tools.rs:443`. **Params:** none.

### `compute_learning_signals`
Source: `learning.rs:114`; handler `learning_tools.rs:464`. Inline compute, no storage write.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `before` | object | yes | — | state |
| `after` | object | yes | — | state |
| `outcome` | object | yes | — | label etc. |
| `signalIds` | array<enum> |  | all 5 | `delta_e` \| `surprise` \| `coherence` \| `consolidation` \| `transfer` |
| `includeFeatures` | boolean |  | `true` | — |

### `embed_learning_event_signals`
Source: `learning.rs:170`; handler `learning_tools.rs:528`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `eventId` | uuid | yes | — | persisted event |
| `signalIds` | array<enum> |  | all 5 | — |
| `includePersisted` | boolean |  | `true` | compare freshly-computed vs persisted |

### `estimate_learning_outcome`
Source: `learning.rs:194`; handler `learning_tools.rs:628`. Nearest-neighbor case-based reasoning over `CF_LEARNING_EVENTS`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `before` | object | yes | — | candidate before-state |
| `candidateAfter` | object | yes | — | candidate after-state |
| `maxNeighbors` | integer |  | `5` | 1–100 |
| `maxScan` | integer |  | `10000` | 1–200000 |
| `minSimilarity` | number |  | `0.0` | `[-1, 1]` |
| `taskId` | string |  | — | exact filter |
| `domain` | string |  | — | before/after domain filter |
| `includeNeighbors` | boolean |  | `true` | — |

### `export_learner_training_dataset`
Source: `learning.rs:318`; handler `learning_tools.rs:810`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `datasetId` | uuid |  | generated | — |
| `task` | enum |  | `reward_model` | `reward_model` \| `reranker` \| `embedder_contrastive` \| `diagnostic_classifier` \| `scheduler` \| `personal_physiology` |
| `maxRows` | integer |  | `10000` | 1–1000000 |
| `clearExisting` | boolean |  | `false` | — |

**Side effects:** writes `CF_LEARNER_TRAINING_DATASETS` (versioned row-major f32 matrix).

### `list_learner_training_datasets`
Source: `learning.rs:337`; handler `learning_tools.rs:916`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | `50` | 1–1000 |
| `offset` | integer |  | `0` | ≥0 |
| `includeShape` | boolean |  | `true` | — |

### `get_learner_training_dataset`
Source: `learning.rs:351`; handler `learning_tools.rs:993`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `datasetId` | uuid | yes | — | — |
| `includeMatrix` | boolean |  | `false` | full f32 buffer |
| `previewRows` | integer |  | `3` | 0–100 |

### `count_learner_training_datasets`
Source: `learning.rs:365`; handler `learning_tools.rs:1033`. **Params:** none.

---

## 22. UTL Learner-State Tools

Source: `utl.rs`; handler `utl_tools.rs`. These tools cover the Phase-0/Phase-1 measurement substrate plus state-conditioned retrieval policy. They write to learner-specific column families (`CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_STATE_HISTORY`, `CF_LEARNER_DELTA_LOG`, `CF_LEARNER_K_SLEEP`, `CF_LEARNER_RETRIEVAL_LOG`, `CF_LEARNER_M_PER_TRACE`, `CF_GOAL_CENTROIDS`, `CF_LEARNER_PROFILE`).

### `register_learner`
Source: `utl.rs:10`; handler `utl_tools.rs:270`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid |  | generated | — |
| `handle` | string | yes | — | display handle |
| `consentState` | string |  | `consented-local-first` | — |
| `modalitiesEnabled` | array<string> |  | `["affect_text","self_report"]` | — |
| `calibrationSessionTs` | integer |  | — | ≥0 |

### `record_session_observation`
Source: `utl.rs:26`; handler `utl_tools.rs:328`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `sessionTs` | integer | yes | — | unix seconds |
| `modality` | string |  | `affect_text` | — |
| `rawText` | string |  | `""` | — |
| `consentState` | string |  | `consented-local-first` | — |
| `preprocessingVersion` | string |  | `phase0-preprocess-v1` | — |
| `embedderVersion` | string |  | `phase0-deterministic-v1` | — |
| `thresholdVersion` | string |  | `thresholds-default-pending-calibration-v1` | — |
| `stateVector` | array<number> |  | — | learner state vector |
| `components` | object |  | — | `{plasticityWindow*, hrvCoherence*, valence*, arousal*, stressFloor*, kSleep?}` |
| `embeddings` | array |  | `[]` | `[{modality*, vector*, scalar?}]` |
| `signals` | array |  | `[]` | `[{modality*, text?, samples?, features?, sampleRateHz?, channels?}]` |

### `compute_delta_s`
Source: `utl.rs:88`; handler `utl_tools.rs:578`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `predictedText` | string |  | `""` | — |
| `actualText` | string |  | `""` | — |
| `simulatedText` | string |  | — | optional |
| `explorationRate` | number |  | `0.0` | `[0,1]` |
| `gamma` | number |  | `0.7` | `[0,1]` |

### `compute_delta_c`
Source: `utl.rs:103`; handler `utl_tools.rs:599`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `recentScores` | array<number> |  | `[]` | — |
| `hrvCoherence` | number |  | `0.0` | `[0,1]` |
| `panelAgreement` | number |  | `0.0` | `[0,1]` |
| `contradiction` | number |  | `0.0` | `[0,1]` |
| `gradientScale` | number |  | — | ≥0 |

### `compute_delta_e`
Source: `utl.rs:119`; handler `utl_tools.rs:620`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid |  | — | resolves stored components |
| `sessionTs` | integer |  | — | with `learnerId` |
| `components` | object |  | — | inline override |

### `compute_L`
Source: `utl.rs:148`; handler `utl_tools.rs:657`. L = ΔS · ΔC · ΔE.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid |  | — | required when `persist=true` |
| `sessionTs` | integer |  | — | required when `persist=true` |
| `traceId` | uuid |  | — | required when `persist=true` |
| `predictedText` | string |  | `""` | — |
| `actualText` | string |  | `""` | — |
| `simulatedText` | string |  | — | — |
| `explorationRate` | number |  | `0.0` | `[0,1]` |
| `recentScores` | array<number> |  | `[]` | — |
| `hrvCoherence` | number |  | `0.0` | `[0,1]` |
| `panelAgreement` | number |  | `0.0` | `[0,1]` |
| `contradiction` | number |  | `0.0` | `[0,1]` |
| `components` | object | yes | — | physiology subset |
| `persist` | boolean |  | `false` | write delta log + M trace |
| `retrievalCorrect` | boolean |  | — | for M trace |

### `get_learner_M`
Source: `utl.rs:188`; handler `utl_tools.rs:804`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `traceId` | uuid | yes | — | — |

### `next_review_for_trace`
Source: `utl.rs:201`; handler `utl_tools.rs:829`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `traceId` | uuid | yes | — | — |

### `list_learner_embedders`
Source: `utl.rs:214`; handler `utl_tools.rs:867`. **Params:** none. Returns the canonical E1–E21 matrix.

### `preflight_learner_assets`
Source: `utl.rs:224`; handler `utl_tools.rs:907`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `modelsRoot` | string |  | `models` | path |
| `calibrationRoot` | string |  | `data/utl_calibration` | path |
| `allowMissing` | boolean |  | `false` | when false, fails closed on missing assets |

### `count_learner_state`
Source: `utl.rs:238`; handler `utl_tools.rs:940`. **Params:** none.

### `get_learner_state`
Source: `utl.rs:248`; handler `utl_tools.rs:968`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `sessionTs` | integer |  | — | optional fingerprint/state fetch |
| `traceId` | uuid |  | — | optional M trace fetch |
| `includeVectors` | boolean |  | `false` | — |

### `record_learner_k_sleep`
Source: `utl.rs:263`; handler `utl_tools.rs:1056`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `sessionTs` | integer | yes | — | ≥0 |
| `slowWaveMinutes` | integer | yes | — | 0–1440 |
| `k` | number |  | — | `[0, 5]` |

### `record_learner_retrieval`
Source: `utl.rs:278`; handler `utl_tools.rs:1109`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `traceId` | uuid | yes | — | — |
| `ts` | integer | yes | — | ≥0 |
| `sessionTs` | integer |  | — | resolves stored state when set |
| `correct` | boolean | yes | — | retrieval outcome |
| `score` | number | yes | — | `[0,1]` |
| `stateVector` | array<number> |  | — | inline override |
| `components` | object |  | — | inline override |

### `upsert_goal_centroid`
Source: `utl.rs:312`; handler `utl_tools.rs:1191`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `skillId` | uuid | yes | — | — |
| `modality` | string | yes | — | — |
| `vector` | array<number> | yes | — | minItems 1 |

### `get_goal_distance`
Source: `utl.rs:326`; handler `utl_tools.rs:1233`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `skillId` | uuid | yes | — | — |
| `sessionTs` | integer | yes | — | ≥0 |
| `modality` | string | yes | — | — |
| `persistGoalState` | boolean |  | `false` | snapshot the comparison |

### `compile_learner_constellation`
Source: `utl.rs:342`; handler `utl_tools.rs:1327`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `sessionTsList` | array<integer> | yes | — | minItems 1 |
| `selectorKind` | integer |  | `1` | 0–255 |
| `label` | string |  | `regulated-baseline` | — |

### `resolve_learner_retrieval_policy`
Source: `utl.rs:357`; handler `utl_tools.rs:1404`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `learnerId` | uuid | yes | — | — |
| `sessionTs` | integer | yes | — | ≥0 |
| `baseWeightProfile` | enum |  | `semantic_search` | one of 20 named profiles |
| `includeWeights` | boolean |  | `true` | resolved E1–E14 weights |

---

## 23. Constellation Tools

Source: `constellation.rs`; handler `constellation_tools.rs`. Constellations are per-embedder centroids + spread statistics over a member set. See [17_constellation_subsystem.md](17_constellation_subsystem.md).

### `compile_constellation`
Source: `constellation.rs:23`; handler `constellation_tools.rs:42`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `selector` | enum | yes | — | `topic` \| `session` \| `tag` \| `time_range` \| `explicit_ids` |
| `topicId` | string |  | — | required for `selector=topic` |
| `sessionId` | string |  | — | required for `selector=session` |
| `tag` | string |  | — | required for `selector=tag` |
| `startIso` | RFC-3339 |  | — | required for `selector=time_range` |
| `endIso` | RFC-3339 |  | — | required for `selector=time_range` |
| `memoryIds` | array<uuid> |  | — | required for `selector=explicit_ids` |
| `rationale` | string |  | — | annotation for `explicit_ids` (NOT part of selector hash) |
| `label` | string | yes | — | 1–256 chars |
| `maxMembers` | integer |  | `50000` | 3–100000 (`TooManyMembers` if exceeded) |
| `rebuildIfExists` | boolean |  | `false` | recompile vs return existing |

**Side effects:** writes `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR`. Fails with `TooFewMembers` when fewer than 3 members.

### `list_constellations`
Source: `constellation.rs:93`; handler `constellation_tools.rs:203`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | `50` | 1–1000 |
| `offset` | integer |  | `0` | ≥0 |
| `includeCentroids` | boolean |  | `false` | per-embedder centroids |

### `get_constellation`
Source: `constellation.rs:124`; handler `constellation_tools.rs:271`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `constellationId` | uuid | yes | — | — |
| `includeCentroids` | boolean |  | `true` | per-embedder centroid + sparse_top_terms + pooled_token_centroid |

### `score_against_constellation`
Source: `constellation.rs:148`; handler `constellation_tools.rs:305`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `constellationId` | uuid | yes | — | — |
| `memoryId` | uuid | yes | — | candidate memory (must be in `CF_FINGERPRINTS`) |

Returns per-embedder cosine, unweighted `combined_score`, `in_spread_p95` flag.

### `derive_constellation`
Source: `constellation.rs:172`; handler `constellation_tools.rs:405`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `operation` | enum | yes | — | `interpolate` \| `add` \| `difference` \| `anti_pole` |
| `sourceConstellationId` | uuid | yes | — | — |
| `otherConstellationId` | uuid |  | — | for binary ops |
| `alpha` | number |  | `0.5` | `[0,1]` for `interpolate` |
| `label` | string | yes | — | 1–256 chars |
| `selector` | enum |  | — | candidate pool for `anti_pole` |
| `topicId`/`sessionId`/`tag`/`startIso`/`endIso`/`memoryIds`/`rationale` |  |  | — | as in `compile_constellation` |
| `maxCandidates` | integer |  | `10000` | 3–200000 |
| `selectedMembers` | integer |  | `50` | 3–100000 |

**Side effects:** writes `CF_CONSTELLATIONS`; performs RocksDB readback verification.

### `delete_constellation`
Source: `constellation.rs:267`; handler `constellation_tools.rs:367`. Idempotent.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `constellationId` | uuid | yes | — | returns `{deleted: bool}` |

---

## 24. Contrastive Pair Tools

Source: `contrastive.rs`; handler `contrastive_tools.rs`. AnomalyKind enum: `semantic_but_not_causal`, `keyword_but_not_paraphrase`, `code_shape_but_different_intent`, `entity_shared_but_different_structure`, `hdc_robust_but_semantic_different`, `other`.

### `mine_contrastive_pairs`
Source: `contrastive.rs:20`; handler `contrastive_tools.rs:70`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `maxPairs` | integer |  | `1000` | 1–100000 |
| `minDisagreement` | number |  | `0.3` | `[0,1]` |
| `highThreshold` | number |  | `0.6` | `[0,1]` |
| `lowThreshold` | number |  | `0.3` | `[0,1]`, must be `<` highThreshold |
| `kinds` | array<enum> |  | all | filter |
| `sessionFilter` | string |  | — | — |
| `topKCandidatesPerAnchor` | integer |  | `10` | 1–500 |
| `candidatePoolSize` | integer |  | `500` | 10–50000 |

**Side effects:** writes `CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`.

### `list_contrastive_pairs`
Source: `contrastive.rs:99`; handler `contrastive_tools.rs:381`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | `100` | 1–10000 |
| `offset` | integer |  | `0` | ≥0 |
| `kind` | enum |  | — | AnomalyKind |
| `anchorId` | uuid |  | — | — |
| `includeFull` | boolean |  | `false` | full record (heavier) |

### `get_contrastive_pair`
Source: `contrastive.rs:148`; handler `contrastive_tools.rs:524`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `anchorId` | uuid | yes | — | — |
| `negativeId` | uuid | yes | — | — |

### `count_contrastive_pairs`
Source: `contrastive.rs:171`; handler `contrastive_tools.rs:569`.

| Param | Type | Req | Default | Description |
| --- | --- | :-: | --- | --- |
| `kind` | enum |  | — | optional AnomalyKind filter |

---

## 25. Typed-Edge Training Tools

Source: `typed_edges.rs`; handler `typed_edges_tools.rs`. See [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

### `export_typed_edges_corpus`
Source: `typed_edges.rs:25`; handler `typed_edges_tools.rs:62`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `maxEdges` | integer |  | `10000` | 1–1000000 |
| `includeContent` | boolean |  | `true` | — |
| `includeSourceMetadata` | boolean |  | `true` | session_id + source_type |
| `includeMechanismType` | boolean |  | `true` | join `CF_CAUSAL_RELATIONSHIPS` for `CausalChain` edges |
| `joinLLMValidation` | boolean |  | `true` | join `CF_TYPED_EDGE_VALIDATIONS` |
| `clearExisting` | boolean |  | `false` | — |

**Side effects:** writes `CF_TYPED_EDGE_RECORDS` (idempotent on `(source, target, edge_type)` key).

### `derive_anomalies_from_edges`
Source: `typed_edges.rs:78`; handler `typed_edges_tools.rs:338`. Generator stamp: `typed_edge_anomaly_derivation_v1`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `highThreshold` | number |  | `0.60` | `[0,1]` |
| `lowThreshold` | number |  | `0.30` | `[0,1]` |
| `minDisagreement` | number |  | `0.30` | `[0,1]` |
| `maxPairs` | integer |  | `10000` | 1–1000000 |
| `kinds` | array<string> |  | — | NOT YET SUPPORTED — passing returns InvalidParams |

**Side effects:** writes `CF_CONTRASTIVE_PAIRS` via the contrastive write path.

### `validate_low_confidence_edges`
Source: `typed_edges.rs:130`; handler `typed_edges_tools.rs:448`. LLM-gated; returns `{status:"skipped"}` when LLM disabled.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `weightCeiling` | number |  | `0.60` | `[0,1]` |
| `maxEdges` | integer |  | `100` | 1–10000 |
| `sessionFilter` | string |  | — | restricts by source's session_id |

**Side effects:** writes `CF_TYPED_EDGE_VALIDATIONS` (idempotent; skips edges that already have a row).

### `list_typed_edge_records`
Source: `typed_edges.rs:166`; handler `typed_edges_tools.rs:747`.

| Param | Type | Req | Default | Range |
| --- | --- | :-: | --- | --- |
| `limit` | integer |  | `100` | 1–1000 |
| `offset` | integer |  | `0` | ≥0 |
| `includeFull` | boolean |  | `false` | hydrate full record |

---

## 26. DynamicJEPA Tools (5090jepa Phase 9)

Source: `dynamicjepa.rs` (35 tools); handler `dynamicjepa_tools.rs`. Every tool requires a `dbPath` parameter pointing to the DynamicJEPA RocksDB data directory (except `dynamicjepa_cross_domain_transfer`, which writes its own output root). All schemas use `additionalProperties: false`.

### Domain-pack registry
| Tool | Required | Optional | Description |
| --- | --- | --- | --- |
| `dynamicjepa_register_domain_pack` (`dynamicjepa.rs:11`, handler `dynamicjepa_tools.rs:26`) | `dbPath`, `file` | — | Registers a strict domain-pack TOML. Writes to physical RocksDB. |
| `dynamicjepa_list_domain_packs` (`dynamicjepa.rs:17`, handler line 47) | `dbPath` | `limit`, `offset` | Paginated list. |
| `dynamicjepa_get_domain_pack` (`dynamicjepa.rs:23`, handler line 69) | `dbPath`, `id` | `domainVersion` (default `1.0.0`) | Read one pack. |

### Event ingestion + adapters
| Tool | Required | Optional | Description |
| --- | --- | --- | --- |
| `dynamicjepa_ingest_event` (handler line 91) | `dbPath`, `domain`, `adapter`, `file` | — | Ingest JSONL fixture; runs adapter. |
| `dynamicjepa_run_adapter` (handler line 114) | `dbPath`, `eventId` | — | Re-run adapter for persisted event. |
| `dynamicjepa_materialize_panel` (handler line 135) | `dbPath` | `transitionId`, `allPending`, `domain` | Materialize one panel or all pending for `domain`. |
| `dynamicjepa_get_panel` (handler line 158) | `dbPath`, `panelId` | `includeReadings` | Read one panel. |
| `dynamicjepa_list_instrument_readings` (handler line 180) | `dbPath`, `eventId` | — | List readings for one raw event. |

### Bindings
| Tool | Required | Optional |
| --- | --- | --- |
| `dynamicjepa_create_binding` (handler line 201) | `dbPath`, `leftCf`, `leftKey`, `rightCf`, `rightKey` | `method` (`explicit_mapping`), `kind` (`event_to_trajectory`), `score` (1.0; `[0,1]`) |
| `dynamicjepa_list_bindings` (handler line 228) | `dbPath` | `entity`, `limit`, `offset` |

### Trajectories + datasets
| Tool | Required | Optional | Description |
| --- | --- | --- | --- |
| `dynamicjepa_compile_trajectories` (handler line 251) | `dbPath`, `domain` | `policy` (`by_domain_session`) | Compile panels + transitions into trajectories. |
| `dynamicjepa_get_trajectory` (handler line 273) | `dbPath`, `id` | — | Read one trajectory. |
| `dynamicjepa_list_trajectories` (handler line 294) | `dbPath`, `domain` | `limit`, `offset` | Paginated list. |
| `dynamicjepa_compile_dataset` (handler line 317) | `dbPath`, `domain` | `policy` (`one_step`), `split` (`train`) | Compile dataset shards. |
| `dynamicjepa_get_dataset_shard` (handler line 340) | `dbPath`, `datasetId`, `shardId` | — | Read one shard. |
| `dynamicjepa_inspect_dataset_row` (handler line 362) | `dbPath`, `datasetId`, `shardId`, `row` | — | Inspect one decoded row. |

### Training & artifacts (CUDA-only)
| Tool | Required | Optional |
| --- | --- | --- |
| `dynamicjepa_train` (handler line 385) | `dbPath`, `datasetId`, `config`, `artifactRoot` | — |
| `dynamicjepa_get_training_run` (handler line 407) | `dbPath`, `id` | — |
| `dynamicjepa_get_artifact` (handler line 428) | `dbPath`, `id` | `verifyFiles` |

### Predictions, plans, surprise
| Tool | Required | Optional |
| --- | --- | --- |
| `dynamicjepa_predict` (handler line 450) | `dbPath`, `artifactId`, `panelId`, `actionId` | — |
| `dynamicjepa_plan` (handler line 473) | `dbPath`, `artifactId`, `panelId`, `skillId` | — |
| `dynamicjepa_record_surprise` (handler line 495) | `dbPath`, `predictionId`, `observedOutcomeId`, `observedPanelId` | — |

### Constellations
| Tool | Required | Optional |
| --- | --- | --- |
| `dynamicjepa_build_constellation` (handler line 518) | `dbPath`, `domain` | `domainVersion`, `subject`, `sourceEventSelector`, `builtByRunId` |
| `dynamicjepa_list_constellations` (handler line 543) | `dbPath` | `domain`, `subject`, `limit`, `offset` |
| `dynamicjepa_get_constellation` (handler line 567) | `dbPath`, `domain`, `modality` | `domainVersion`, `subject` |

### Calibration & audit
| Tool | Required | Optional |
| --- | --- | --- |
| `dynamicjepa_calibrate_threshold` (handler line 591) | `dbPath`, `domain` | `domainVersion`, `subject`, `modality`, `calibrationEventSelector`, `percentile` (0–100) |
| `dynamicjepa_recalibrate_threshold` (handler line 617) | `dbPath`, `domain`, `supersedes`, `reason` | `domainVersion`, `subject`, `modality`, `calibrationEventSelector`, `percentile` |
| `dynamicjepa_compute_mc_ratio` (handler line 645) | `dbPath`, `domain`, `outputDir` | `domainVersion` |
| `dynamicjepa_audit_pairwise_mi` (handler line 668) | `dbPath`, `domain`, `outputDir` | `domainVersion`, `sampleSize` (≥50), `estimator` (`ksg`), `ksgK` (≥1), `bootstrapIters`, `seed` |
| `dynamicjepa_cross_domain_transfer` (handler line 696) | `outputRoot` | `seeds` (default `[42,43,44,45,46]`), `sourceEvents`, `targetEvents`, `bootstrapIters`, `trainEpochs`, `batchSize`, `maxSecondsPerTraining`, `learningRate` (>0), `stoppingTarget` (>0) |

### Read-only inspection
| Tool | Required | Optional |
| --- | --- | --- |
| `dynamicjepa_get_prediction` (handler line 725) | `dbPath`, `id` | — |
| `dynamicjepa_get_plan_trace` (handler line 746) | `dbPath`, `id` | `includePredictions`, `includeGuards` |
| `dynamicjepa_get_surprise` (handler line 769) | `dbPath`, `id` | — |
| `dynamicjepa_inspect_counts` (handler line 790) | `dbPath` | — |
| `dynamicjepa_inspect_cf` (handler line 811) | `dbPath`, `cf` | `keyHex`, `limit`, `offset` |

---

## 27. Common Side Effects Summary

| Tool group | Column families written |
| --- | --- |
| `store_memory(_ies)` | `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_SOURCE_METADATA`, all `CF_EMB_*`, `CF_AUDIT_LOG` |
| `forget_concept` | tombstone in `CF_SYSTEM`, `CF_AUDIT_LOG` |
| `merge_concepts` | `CF_FINGERPRINTS` (target), `CF_MERGE_HISTORY`, `CF_AUDIT_LOG`; soft-delete on sources |
| `boost_importance` | `CF_FINGERPRINTS`, `CF_IMPORTANCE_HISTORY`, `CF_AUDIT_LOG` |
| `delete_file_content` / `reconcile_files` | per-file fingerprint deletion + HNSW removal + `CF_AUDIT_LOG` |
| `detect_topics` | `CF_TOPICS`, `CF_TOPIC_MEMBERSHIPS` |
| `trigger_causal_discovery` | `CF_CAUSAL_RELATIONSHIPS`, `CF_CAUSAL_BY_SOURCE`; updates E5 cause/effect HNSW |
| `discover_graph_relationships` / `validate_graph_link` | `CF_TYPED_EDGES` (+ reverse indexes) |
| `repair_causal_relationships` | deletes corrupt rows from `CF_CAUSAL_RELATIONSHIPS` |
| `export_training_corpus` | `CF_TRAINING_RECORDS` (binary bincode) |
| `record_learning_event` / graph-learning event | `CF_LEARNING_EVENTS` |
| `record_session_observation` | `CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_STATE_HISTORY` |
| `compute_L` (persist=true) | `CF_LEARNER_DELTA_LOG`, `CF_LEARNER_M_PER_TRACE` |
| `record_learner_k_sleep` | `CF_LEARNER_K_SLEEP` |
| `record_learner_retrieval` | `CF_LEARNER_RETRIEVAL_LOG` |
| `upsert_goal_centroid` | `CF_GOAL_CENTROIDS` |
| `compile_constellation` / `derive_constellation` | `CF_CONSTELLATIONS`, `CF_CONSTELLATION_BY_SELECTOR` |
| `delete_constellation` | deletes from above |
| `mine_contrastive_pairs` / `derive_anomalies_from_edges` | `CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR` |
| `validate_low_confidence_edges` | `CF_TYPED_EDGE_VALIDATIONS` |
| `export_typed_edges_corpus` | `CF_TYPED_EDGE_RECORDS` |
| `export_learner_training_dataset` | `CF_LEARNER_TRAINING_DATASETS` |
| DynamicJEPA writers | DynamicJEPA-namespaced CFs in the supplied `dbPath` |

For full CF schema see [09_storage_layer.md](09_storage_layer.md).

---

## 28. Common Error Conditions

| Condition | Error | Where |
| --- | --- | --- |
| Unknown tool name | `TOOL_NOT_FOUND` (-32601) | `dispatch.rs:30` |
| Missing `params` | `INVALID_PARAMS` | `dispatch.rs:54` |
| Missing `name` field | `INVALID_PARAMS` | `dispatch.rs:65` |
| Unknown property in args (`additionalProperties: false`) | `InvalidParams` | per-handler validation |
| Missing required field | `InvalidParams` | per-handler validation |
| Out-of-range numeric/integer | `InvalidParams` | per-handler validation |
| Soft-deleted memory referenced | `NotFound` | per-handler |
| LLM-gated tool without `--features llm` | `Unsupported` or `{status:"skipped"}` | `causal_discovery.rs`, `graph.rs`, `typed_edges.rs:130` |
| Constellation member count below 3 | `TooFewMembers` | `constellation_tools.rs` |
| Constellation member count above `maxMembers` | `TooManyMembers` | `constellation_tools.rs` |
| Graph-learning resolve without persisted evidence | hard error (no fallback) | `graph_learning_tools.rs:435` |
| `derive_anomalies_from_edges` with `kinds` filter | `InvalidParams` (not yet supported) | `typed_edges_tools.rs:338` |

---

## 29. AnomalyKinds (used by contrastive + typed_edges)

- `semantic_but_not_causal` — high E1 but low E5
- `keyword_but_not_paraphrase` — high E6 but low E10
- `code_shape_but_different_intent` — high E7 but low E1
- `entity_shared_but_different_structure` — high E11 but low E8
- `hdc_robust_but_semantic_different` — high E9 but low E1
- `other` — sentinel for unmatched configurations

---

## 30. Supported Weight Profiles

Used by `search_graph.weightProfile`, `get_unified_neighbors.weight_profile`, and `resolve_learner_retrieval_policy.baseWeightProfile`:

`semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`, `multilingual_search`, `long_context`, `translation_finder`, `affect_repair`, `affect_priming`, `affect_neutral`.

For the resolved E1–E14 weights of each profile see [06_search_and_fusion.md](06_search_and_fusion.md) §weight-profiles. For learner state-conditioned profiles (`affect_*`) see [20_learning_subsystem.md](20_learning_subsystem.md).
