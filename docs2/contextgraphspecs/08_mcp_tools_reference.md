# 08 — MCP Tools Reference

Per-tool reference for the Context Graph MCP server. All tools are exposed via JSON-RPC 2.0 `tools/list` and `tools/call`. Every schema sets `additionalProperties: false` — unknown parameters are rejected.

**Source files covered:**

- `crates/context-graph-mcp/src/protocol.rs` — JSON-RPC 2.0 types + error codes
- `crates/context-graph-mcp/src/tools/names.rs` — tool name constants
- `crates/context-graph-mcp/src/tools/aliases.rs` — legacy-name resolution
- `crates/context-graph-mcp/src/tools/definitions/*.rs` — per-category schemas
- `crates/context-graph-mcp/src/handlers/tools/*.rs` — per-category handlers
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` — name → handler routing
- `crates/context-graph-core/src/*` — DTOs consumed by handlers

**Deep-dive cross-refs:** `05_embedding_subsystem.md` (13 embedders), `06_search_and_fusion.md` (strategies, RRF, pipelines), `07_graph_and_causal.md` (typed edges, causal), `12_clustering_and_topics.md` (topic tools), `16_training_subsystem.md` (training corpus), `17_constellation_subsystem.md` (constellations), `18_contrastive_and_typed_edges.md` (contrastive/typed-edge tools).

---

## 1. Summary

Tool constants in `names.rs`: **75** (`grep -cE "pub const [A-Z_]+: &str"`).

Build-time feature-gated: `causal_discovery` (2 tools) and the LLM variants of `graph` (2 of 4) require `--features llm`. Without `llm`, server exposes **71**; with `llm`, **75**.

| Category | File | Count | Tools |
|----------|------|------:|-------|
| core | `core.rs` | 4 | `store_memory`, `get_memetic_status`, `search_graph`, `trigger_consolidation` |
| merge | `merge.rs` | 1 | `merge_concepts` |
| curation | `curation.rs` | 2 | `forget_concept`, `boost_importance` |
| topic | `topic.rs` | 4 | `get_topic_portfolio`, `get_topic_stability`, `detect_topics`, `get_divergence_alerts` |
| file_watcher | `file_watcher.rs` | 4 | `list_watched_files`, `get_file_watcher_stats`, `delete_file_content`, `reconcile_files` |
| sequence | `sequence.rs` | 4 | `get_conversation_context`, `get_session_timeline`, `traverse_memory_chain`, `compare_session_states` |
| causal | `causal.rs` | 4 | `search_causal_relationships`, `search_causes`, `search_effects`, `get_causal_chain` |
| causal_discovery (llm) | `causal_discovery.rs` | 2 | `trigger_causal_discovery`, `get_causal_discovery_status` |
| keyword | `keyword.rs` | 1 | `search_by_keywords` |
| code | `code.rs` | 1 | `search_code` |
| graph | `graph.rs` | 2 + 2 (llm) | `search_connections`, `get_graph_path`, `discover_graph_relationships` (llm), `validate_graph_link` (llm) |
| robustness | `robustness.rs` | 1 | `search_robust` |
| entity | `entity.rs` | 6 | `extract_entities`, `search_by_entities`, `infer_relationship`, `find_related_entities`, `validate_knowledge`, `get_entity_graph` |
| embedder | `embedder.rs` | 7 + 2 standalone | `search_by_embedder`, `get_embedder_clusters`, `compare_embedder_views`, `list_embedder_indexes`, `get_memory_fingerprint`, `create_weight_profile`, `search_cross_embedder_anomalies`, `search_by_tokens`, `search_by_expansion` |
| temporal | `temporal.rs` | 2 | `search_recent`, `search_periodic` |
| graph_link | `graph_link.rs` | 4 | `get_memory_neighbors`, `get_typed_edges`, `traverse_graph`, `get_unified_neighbors` |
| maintenance | `maintenance.rs` | 1 | `repair_causal_relationships` |
| provenance | `provenance.rs` | 3 | `get_audit_trail`, `get_merge_history`, `get_provenance_chain` |
| daemon | `daemon.rs` | 1 | `daemon_status` |
| training | `training.rs` | 4 | `export_training_corpus`, `list_training_records`, `get_training_record`, `count_training_records` |
| constellation | `constellation.rs` | 5 | `compile_constellation`, `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation` |
| contrastive | `contrastive.rs` | 4 | `mine_contrastive_pairs`, `list_contrastive_pairs`, `get_contrastive_pair`, `count_contrastive_pairs` |
| typed_edges | `typed_edges.rs` | 4 | `export_typed_edges_corpus`, `derive_anomalies_from_edges`, `validate_low_confidence_edges`, `list_typed_edge_records` |
| **Total (llm on)** | | **75** | |

---

## 2. Transport and Protocol

### Methods

| Method | Purpose |
|--------|---------|
| `initialize` | Handshake, returns server info + `capabilities.tools` |
| `shutdown` | Graceful shutdown request |
| `tools/list` | Returns `{ tools: [ToolDefinition, ...] }` (75 entries with llm) |
| `tools/call` | Invokes a tool: `{ name: "...", arguments: {...} }` |

### Request / Response

JSON-RPC 2.0, one message per line (NDJSON) on stdio and TCP.

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search_graph","arguments":{"query":"rust"}}}
```

- `id` may be string, number, or `null`. Absent `id` = **notification** → no response emitted (transport-level suppression).
- Success response: `{"jsonrpc":"2.0","id":...,"result":{...}}`.
- Error response: `{"jsonrpc":"2.0","id":...,"error":{"code":i32,"message":"..."}}`.
- All tool schemas are objects with `"additionalProperties": false`. Extra fields → `INVALID_PARAMS`.

### Error codes (`protocol::error_codes`)

| Code | Constant | Meaning |
|-----:|----------|---------|
| -32700 | `PARSE_ERROR` | Malformed JSON |
| -32600 | `INVALID_REQUEST` | Not a valid JSON-RPC 2.0 request |
| -32601 | `METHOD_NOT_FOUND` | Unknown method (e.g. not `tools/call`) |
| -32602 | `INVALID_PARAMS` | Missing/invalid tool arguments |
| -32603 | `INTERNAL_ERROR` | Unhandled runtime error |
| -32001 | `FEATURE_DISABLED` | Feature not compiled in |
| -32002 | `NODE_NOT_FOUND` | Memory UUID not in store |
| -32004 | `STORAGE_ERROR` | RocksDB / HNSW I/O failure |
| -32005 | `EMBEDDING_ERROR` | Embedder inference failed |
| -32006 | `TOOL_NOT_FOUND` | Unknown tool name |
| -32007 | `LAYER_TIMEOUT` | Downstream layer timed out |
| -32021 | `INSUFFICIENT_MEMORIES` | Too few memories for operation |
| -32110..-32114 | `TCP_*` | Transport-layer errors |

### Aliases (backward compat)

| Legacy name | Canonical |
|-------------|-----------|
| `consolidate_memories` | `trigger_consolidation` |
| `inject_context` | `store_memory` |

Resolved in `tools::aliases::resolve_alias` before dispatch.

### Conventions

- Parameter names are **camelCase** except in `curation`, `merge`, `file_watcher`, `maintenance`, `provenance`, `graph_link`, and `typed_edges` which use **snake_case** — the exact casing in each schema is authoritative. The per-tool tables below use the on-wire casing.
- All cosine-derived scores are normalized to `[0, 1]` via `(raw+1)/2` (SRC-3).
- Embedder IDs: `E1` (semantic 1024D), `E2/E3/E4` (temporal 512D), `E5` (causal 768D, asymmetric), `E6` (sparse 30K keyword), `E7` (code 1536D), `E8` (graph 1024D, asymmetric), `E9` (HDC 1024D), `E10` (paraphrase 1024D, asymmetric), `E11` (entity 768D), `E12` (ColBERT 128D/token MaxSim), `E13` (SPLADE 30K sparse).

---

## 3. Core tools (`definitions/core.rs`)

### 3.1 `store_memory`

Store a memory node directly in the knowledge graph. `inject_context` resolves here via alias; providing `rationale` triggers the same validation and response shape.

| Param | Type | Req | Default | Range | Description |
|-------|------|:--:|---------|-------|-------------|
| `content` | string | yes | | | Text to store |
| `rationale` | string | no | | 1..=1024 chars | Why storing; returned verbatim when set |
| `importance` | number | no | 0.5 | [0, 1] | Memory importance |
| `sessionId` | string | no | env `CLAUDE_SESSION_ID` | | Session scope |
| `operatorId` | string | no | | | Audit provenance |

**Returns:** `{ memory_id, fingerprint_id, embedding_count, session_id, rationale? }`.
**Side effects:** writes `CF_FINGERPRINTS`, all 13 embedders persisted, adds to every HNSW index it is valid for, writes `CF_AUDIT_LOG` (`create`), triggers K-NN graph build eventually, invalidates topic cache.
**Errors:** `INVALID_PARAMS` (missing content, out-of-range importance), `EMBEDDING_ERROR`, `STORAGE_ERROR`.

### 3.2 `get_memetic_status`

System status snapshot (no params). **Returns:** `{ fingerprint_count, embedder_count (13), storage_backend, storage_size_bytes, layer_status }`. Read-only.

### 3.3 `search_graph`

Primary multi-space search. Covers E1-foundation, RRF multi-space fusion, pipeline (E13→E1→E12), asymmetric E5 reranking, temporal boosts (E2/E3/E4), session/time filters, and weight profiles. Schema is the richest in the system.

| Param | Type | Req | Default | Range / Enum | Description |
|-------|------|:--:|---------|--------------|-------------|
| `query` | string | yes | | | Query text |
| `topK` | integer | no | 10 | [1, 100] | Result cap |
| `minSimilarity` | number | no | 0.0 | [0, 1] | Score floor |
| `includeContent` | boolean | no | false | | Include raw content |
| `strategy` | string | no | `multi_space` | `e1_only` \| `multi_space` \| `pipeline` | Strategy selector |
| `weightProfile` | string | no | | see enum below | Named profile |
| `customWeights` | object | no | | `E1..E13` ∈ [0,1], ≈sum-to-1 | Per-embedder weight override |
| `excludeEmbedders` | string[] | no | | `E1..E13` | Zero-out + renormalize |
| `useQuantizedPrefilter` | boolean | no | false | | PQ-8 prefilter |
| `includeEmbedderBreakdown` | boolean | no | false | | Per-embedder contributions |
| `enableRerank` | boolean | no | false | | E12 ColBERT stage 3 |
| `enableAsymmetricE5` | boolean | no | true | | Direction-aware E5 |
| `causalDirection` | string | no | `auto` | `auto` \| `cause` \| `effect` \| `none` | E5 direction override |
| `enableQueryExpansion` | boolean | no | false | | Causal query expansion |
| `temporalWeight` | number | no | 0.0 | [0, 1] | Post-retrieval temporal boost |
| `conversationContext` | object | no | | `{anchorToCurrentTurn, turnsBack 0..=100 =10, turnsForward 0..=100 =0}` | Session-anchored window |
| `sessionScope` | string | no | `all` | `current` \| `all` \| `recent` | Session filter |
| `decayHalfLifeSecs` | integer | no | 86400 | | Exponential decay half-life |
| `lastHours` | integer | no | | ≥1 | Window filter |
| `lastDays` | integer | no | | ≥1 | Window filter |
| `sessionId` | string | no | | | Session filter |
| `periodicBoost` | number | no | | [0, 1] | E3 periodic weight |
| `targetHour` | integer | no | | [0, 23] | For `periodicBoost` |
| `targetDayOfWeek` | integer | no | | [0, 6] (0=Sun) | For `periodicBoost` |
| `sequenceAnchor` | uuid | no | | | E4 anchor |
| `sequenceDirection` | string | no | | `before` \| `after` \| `around` \| `both` | E4 direction |
| `includeProvenance` | boolean | no | false | | Retrieval provenance block |
| `decayFunction` | string | no | `exponential` | `linear` \| `exponential` \| `step` \| `none` \| `no_decay` | |
| `temporalScale` | string | no | `meso` | `micro` \| `meso` \| `macro` \| `long` \| `archival` | |

**Weight profile enum:** `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`.

**Returns:** `{ results: [{memory_id, score, content?, embedder_breakdown?, provenance?}], strategy, weights_used }`.
**Side effects:** read-only.
**Errors:** `INVALID_PARAMS` (missing `query`, weight sums out of range, bad enum), `EMBEDDING_ERROR`, `LAYER_TIMEOUT`.

### 3.4 `trigger_consolidation`

Identify merge candidates; does **not** auto-merge. Feed candidates to `merge_concepts`.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `strategy` | string | no | `similarity` | `similarity` \| `temporal` \| `semantic` |
| `min_similarity` | number | no | 0.925 | [0, 1] (SRC-3 normalized) |
| `max_memories` | integer | no | 100 | [1, 10000] |

**Returns:** `{ candidates: [{memory_a, memory_b, similarity, strategy}] }`. Read-only.

---

## 4. Merge tools (`definitions/merge.rs`)

### 4.1 `merge_concepts`

Union / intersection / weighted-average merge. Produces a reversal hash (30-day recovery, SEC-06).

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `source_ids` | uuid[] | yes | | 2..=10 |
| `target_name` | string | yes | | 1..=256 chars |
| `merge_strategy` | string | no | `union` | `union` \| `intersection` \| `weighted_average` |
| `rationale` | string | yes | | 1..=1024 chars |
| `force_merge` | boolean | no | false | | bypass prior-conflict check |

**Returns:** `{ merged_memory_id, reversal_hash, strategy, source_ids }`.
**Side effects:** new fingerprint in `CF_FINGERPRINTS`, sources soft-deleted, merge record in `CF_MERGE_HISTORY`, audit row, HNSW updates (add merged, remove sources).
**Errors:** `INVALID_PARAMS`, `NODE_NOT_FOUND`, `STORAGE_ERROR`.

---

## 5. Curation tools (`definitions/curation.rs`)

### 5.1 `forget_concept`

| Param | Type | Req | Default | Description |
|-------|------|:--:|---------|-------------|
| `node_id` | uuid | yes | | Memory to delete |
| `soft_delete` | boolean | no | true | SEC-06 30-day recovery |
| `operator_id` | string | no | | Audit operator |
| `reason` | string | no | | Audit reason |

**Returns:** `{ deleted_at, soft_delete, node_id }`.
**Side effects:** tombstone in `CF_SOFT_DELETES` (soft) or full removal from `CF_FINGERPRINTS`+all HNSW indexes (hard); audit row in `CF_AUDIT_LOG`.
**Errors:** `NODE_NOT_FOUND`, `STORAGE_ERROR`.

### 5.2 `boost_importance`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `node_id` | uuid | yes | | |
| `delta` | number | yes | | [-1, 1] |
| `operator_id` | string | no | | |

Handler clamps final `importance` to `[0, 1]` (BR-MCP-002). **Returns:** `{ node_id, old, delta, new }`. Writes `CF_FINGERPRINTS` + audit row.

---

## 6. Topic tools (`definitions/topic.rs`)

### 6.1 `get_topic_portfolio`

| Param | Type | Req | Default | Enum |
|-------|------|:--:|---------|------|
| `format` | string | no | `standard` | `brief` \| `standard` \| `verbose` |

**Returns:** `{ topics: [{id, label, weighted_agreement, stability, tier, members?, centroid?}] }`. Read-only (serves cached portfolio).

### 6.2 `get_topic_stability`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `hours` | integer | no | 6 | [1, 168] |

**Returns:** `{ churn_rate, entropy, phase_breakdown, lookback_hours }`. Read-only.

### 6.3 `detect_topics`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `force` | boolean | no | false | |
| `max_memories` | integer | no | 10000 | [1, 50000] |

Triggers HDBSCAN (min_cluster_size=3, weighted_agreement ≥ 2.5, E2/E3/E4 excluded per AP-60).
**Returns:** `{ topics_detected, noise_count, duration_ms }`.
**Side effects:** rewrites in-memory topic portfolio; persists cluster assignments to `CF_TOPICS`.
**Errors:** `INSUFFICIENT_MEMORIES` when corpus < 3.

### 6.4 `get_divergence_alerts`

Uses semantic embedders only (E1/E6/E7/E10/E12/E13 per AP-62; E5 excluded per AP-77; E2-E4 per AP-63).

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `lookback_hours` | integer | no | 2 | [1, 48] |

**Returns:** `{ alerts: [{topic_id, divergence_score, drift_vector}] }`. Read-only.

---

## 7. File watcher tools (`definitions/file_watcher.rs`)

### 7.1 `list_watched_files`

| Param | Type | Req | Default | Description |
|-------|------|:--:|---------|-------------|
| `include_counts` | boolean | no | true | Include chunk counts |
| `path_filter` | string | no | | Glob pattern |

**Returns:** `{ files: [{path, chunk_count, last_updated}] }`.

### 7.2 `get_file_watcher_stats`

No params. **Returns:** `{ total_files, total_chunks, avg_chunks_per_file, min, max }`.

### 7.3 `delete_file_content`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `file_path` | string | yes | |
| `soft_delete` | boolean | no | true |

**Returns:** `{ deleted_chunks, soft_delete, file_path }`. Writes soft-delete tombstones or removes fingerprints+HNSW entries for every chunk of the file.

### 7.4 `reconcile_files`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `dry_run` | boolean | no | true |
| `base_path` | string | no | |

**Returns:** `{ orphans: [...], deleted_count, dry_run }`. When `dry_run=false`, deletes orphaned chunk embeddings for files no longer on disk.

---

## 8. Sequence tools (E4) (`definitions/sequence.rs`)

### 8.1 `get_conversation_context`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `direction` | string | no | `before` | `before` \| `after` \| `both` |
| `windowSize` | integer | no | 10 | [1, 50] |
| `sessionOnly` | boolean | no | true | |
| `includeContent` | boolean | no | true | |
| `query` | string | no | | Optional semantic filter |
| `minSimilarity` | number | no | 0.0 | [0, 1] |

Auto-anchors to current session sequence. **Returns:** `{ context: [{memory_id, sequence, position_label, similarity?}] }`. Read-only.

### 8.2 `get_session_timeline`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `sessionId` | string | no | current | |
| `limit` | integer | no | 50 | [1, 200] |
| `offset` | integer | no | 0 | ≥0 |
| `sourceTypes` | string[] | no | all | `HookDescription` \| `ClaudeResponse` \| `Manual` \| `MDFileChunk` |
| `includeContent` | boolean | no | false | |

**Returns:** `{ timeline: [...], session_id, total }`. Read-only.

### 8.3 `traverse_memory_chain`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `anchorId` | uuid | yes | | |
| `direction` | string | no | `backward` | `forward` \| `backward` \| `bidirectional` |
| `hops` | integer | no | 5 | [1, 20] |
| `semanticFilter` | string | no | | |
| `minSimilarity` | number | no | 0.3 | [0, 1] |
| `includeContent` | boolean | no | true | |

**Returns:** `{ chain: [{memory_id, hop, similarity}], hops_traversed }`. Read-only.

### 8.4 `compare_session_states`

| Param | Type | Req | Default | Notes |
|-------|------|:--:|---------|-------|
| `beforeSequence` | integer \| string | yes | | integer or `"start"` |
| `afterSequence` | integer \| string | yes | | integer or `"current"` |
| `sessionId` | string | no | current | |

**Returns:** `{ topics_added, topics_removed, memory_count_delta, key_differences }`. Read-only.

---

## 9. Causal tools (E5) (`definitions/causal.rs`)

All four apply asymmetric E5 (cause→effect ×1.2, effect→cause ×0.8 per AP-77).

### 9.1 `search_causal_relationships`

Searches LLM-generated 1–3 paragraph causal descriptions with full provenance.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | |
| `direction` | string | no | `all` | `cause` \| `effect` \| `all` |
| `topK` | integer | no | 10 | [1, 100] |
| `includeSource` | boolean | no | true | |
| `includeProvenance` | boolean | no | false | |
| `sourceWeight` | number | no | 0.6 | [0, 1] |
| `explanationWeight` | number | no | 0.4 | [0, 1] |
| `multiEmbedder` | boolean | no | false | Requires `direction` ∈ `cause`/`effect` |
| `minConsensus` | number | no | 0.0 | [0, 1] |
| `e1Weight` | number | no | 0.30 | [0, 1] |
| `e5Weight` | number | no | 0.35 | [0, 1] |
| `e8Weight` | number | no | 0.15 | [0, 1] |
| `e11Weight` | number | no | 0.20 | [0, 1] |

**Returns:** `{ relationships: [{description, source_memory_id, effect_memory_id, causal_score, provenance?}] }`. Read-only; hits `CF_CAUSAL_RELATIONSHIPS`.

### 9.2 `search_causes`

Abductive (effect → cause) via E5 with 0.8× dampening.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | Observed effect |
| `topK` | integer | no | 10 | [1, 50] |
| `minScore` | number | no | 0.1 | [0, 1] |
| `includeContent` | boolean | no | false | |
| `filterCausalDirection` | string | no | | `cause` \| `effect` \| `unknown` |
| `searchScope` | string | no | `memories` | `memories` \| `relationships` \| `all` |
| `strategy` | string | no | | `e1_only` \| `multi_space` \| `pipeline` |
| `rerankWeight` | number | no | 0.4 | [0, 1] — pipeline only |

### 9.3 `search_effects`

Predictive (cause → effect) via E5 with 1.2× boost. Identical schema to `search_causes`.

### 9.4 `get_causal_chain`

Transitive chain with 0.9^hop attenuation.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `anchorId` | uuid | yes | | |
| `direction` | string | no | `forward` | `forward` \| `backward` |
| `maxHops` | integer | no | 5 | [1, 10] |
| `minSimilarity` | number | no | 0.3 | [0, 1] |
| `includeContent` | boolean | no | false | |

**Returns:** `{ chain: [{hop, memory_id, score, direction}], terminal_reason }`.

---

## 10. Causal discovery tools (`definitions/causal_discovery.rs`, feature = `llm`)

Uses Qwen2.5 via Candle/CUDA with GBNF-constrained decoding. Confirmed pairs are embedded through `CausalModel::embed_dual` into E5 cause/effect.

### 10.1 `trigger_causal_discovery`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `mode` | string | no | `extract` | `extract` (within-memory) \| `pairs` (cross-memory) |
| `maxPairs` | integer | no | 50 | [1, 200] |
| `minConfidence` | number | no | 0.7 | [0.5, 1.0] |
| `sessionScope` | string | no | `all` | `current` \| `all` \| `recent` |
| `session_id` | string | no | | Required with `sessionScope=current` for precise filter |
| `dryRun` | boolean | no | false | Skip writes |

**Side effects:** writes `CF_CAUSAL_RELATIONSHIPS`, typed edges of kind `causal_chain`, audit rows; LLM inference (VRAM ≈ 6 GB FP16 for 3B, 14 GB for 7B).
**Errors:** `FEATURE_DISABLED` without `llm`, `INTERNAL_ERROR` on LLM failure.

### 10.2 `get_causal_discovery_status`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `includeLastResult` | boolean | no | true |
| `includeGraphStats` | boolean | no | true |

**Returns:** `{ running, last_cycle, vram_bytes, stats: {pairs_analyzed, relationships_found, ...}, graph_stats? }`.

---

## 11. Keyword tools (`definitions/keyword.rs`)

### 11.1 `search_by_keywords`

E6 sparse keyword matching; optionally SPLADE-expanded.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | |
| `topK` | integer | no | 10 | [1, 50] |
| `minScore` | number | no | 0.1 | [0, 1] |
| `blendWithSemantic` | number | no | 0.3 | [0, 1] (E6 weight; 1.0 = pure E6) |
| `useSpladeExpansion` | boolean | no | true | E13 term expansion |
| `includeContent` | boolean | no | false | |
| `strategy` | string | no | `multi_space` | `e1_only` \| `multi_space` \| `pipeline` |

Read-only; hits sparse inverted index (E6) and, if expansion on, E13.

---

## 12. Code tools (`definitions/code.rs`)

### 12.1 `search_code`

E7 Qodo-Embed 1536D dense.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | |
| `topK` | integer | no | 10 | [1, 50] |
| `minScore` | number | no | 0.2 | [0, 1] |
| `blendWithSemantic` | number | no | 0.4 | [0, 1] (E7 weight) |
| `includeContent` | boolean | no | false | |
| `searchMode` | string | no | `hybrid` | `hybrid` \| `e7Only` \| `e1WithE7Rerank` \| `pipeline` |
| `languageHint` | string | no | | `rust` \| `python` \| `javascript` \| `typescript` \| `go` \| `java` \| `cpp` \| `sql` |

**Returns:** `{ results, detected_language }`. Read-only.

---

## 13. Graph tools (`definitions/graph.rs`)

### 13.1 `search_connections`

E8 asymmetric (source→target ×1.2, target→source ×0.8).

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | |
| `direction` | string | no | `both` | `source` \| `target` \| `both` |
| `topK` | integer | no | 10 | [1, 50] |
| `minScore` | number | no | 0.1 | [0, 1] |
| `includeContent` | boolean | no | false | |
| `filterGraphDirection` | string | no | | `source` \| `target` \| `unknown` |
| `includeProvenance` | boolean | no | false | |

### 13.2 `get_graph_path`

Multi-hop E8 traversal, 0.9^hop attenuation.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `anchorId` | uuid | yes | | |
| `direction` | string | no | `forward` | `forward` \| `backward` |
| `maxHops` | integer | no | 5 | [1, 10] |
| `minSimilarity` | number | no | 0.3 | [0, 1] |
| `includeContent` | boolean | no | false | |

### 13.3 `discover_graph_relationships` (llm)

LLM relationship classification over candidate pairs. 19 relation types across 4 domains.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `memory_ids` | uuid[] | yes | | 2..=50 |
| `relationship_types` | string[] | no | all | see enum below |
| `min_confidence` | number | no | 0.7 | [0, 1] |
| `batch_size` | integer | no | 50 | [1, 100] |

**Relationship enum:** `contains`, `scoped_by`, `depends_on`, `imports`, `requires`, `references`, `cites`, `interprets`, `distinguishes`, `implements`, `complies_with`, `fulfills`, `extends`, `modifies`, `supersedes`, `overrules`, `calls`, `applies`, `used_by`.

**Side effects:** writes typed edges to `CF_TYPED_EDGES` (kind `graph_connected`), audit rows, LLM inference.

### 13.4 `validate_graph_link` (llm)

Single-pair validation.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `source_id` | uuid | yes | | |
| `target_id` | uuid | yes | | |
| `expected_relationship_type` | string | no | | (same 19-value enum) |

**Returns:** `{ valid, confidence, detected_relationship_type, category, direction, rationale }`.

---

## 14. Robustness tools (`definitions/robustness.rs`)

### 14.1 `search_robust`

E9 HDC trigram hypervectors (1024D projected); highlights E1 blind spots.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | ≥3 chars (trigram floor) |
| `topK` | integer | no | 10 | [1, 50] |
| `minScore` | number | no | 0.1 | [0, 1] |
| `e9DiscoveryThreshold` | number | no | 0.08 | [0, 1] — E9 floor for "discovery" flag |
| `e1WeaknessThreshold` | number | no | 0.5 | [0, 1] — E1 ceiling for "missed" flag |
| `includeContent` | boolean | no | false | |
| `includeE9Score` | boolean | no | true | |
| `strategy` | string | no | `multi_space` | `e1_only` \| `multi_space` \| `pipeline` |

A result is tagged "E9 discovery" when `e9_score ≥ e9DiscoveryThreshold AND e1_score < e1WeaknessThreshold`.

---

## 15. Entity tools (E11) (`definitions/entity.rs`)

Entity type enum (reused below): `ProgrammingLanguage`, `Framework`, `Database`, `Cloud`, `Company`, `TechnicalTerm`, `Unknown`.

### 15.1 `extract_entities`

| Param | Type | Req | Default | Description |
|-------|------|:--:|---------|-------------|
| `text` | string | yes | | |
| `includeUnknown` | boolean | no | true | Include heuristic hits |
| `groupByType` | boolean | no | false | |

**Returns:** `{ entities: [{surface, canonical, type, confidence}] }`. Read-only; canonicalization resolves e.g. `k8s`→`kubernetes`.

### 15.2 `search_by_entities`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `entities` | string[] | yes | | Canonicalized on input |
| `entityTypes` | string[] | no | | entity-type enum |
| `matchMode` | string | no | `any` | `any` \| `all` |
| `topK` | integer | no | 10 | [1, 50] |
| `minScore` | number | no | 0.2 | [0, 1] |
| `includeContent` | boolean | no | false | |
| `boostExactMatch` | number | no | 1.15 | [1.0, 3.0] |
| `strategy` | string | no | `e1_only` | `e1_only` \| `multi_space` \| `pipeline` |

### 15.3 `infer_relationship`

TransE: `r̂ = t − h`, match to known relations.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `headEntity` | string | yes | | |
| `tailEntity` | string | yes | | |
| `headType` / `tailType` | string | no | | entity-type enum (no `Unknown`) |
| `topK` | integer | no | 5 | [1, 20] |
| `includeScore` | boolean | no | true | |

### 15.4 `find_related_entities`

TransE: outgoing `t̂ = h + r`, incoming `ĥ = t − r`.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `entity` | string | yes | | |
| `relation` | string | yes | | |
| `direction` | string | no | `outgoing` | `outgoing` \| `incoming` |
| `entityType` | string | no | | entity-type enum |
| `topK` | integer | no | 10 | [1, 50] |
| `minScore` | number | no | | [-10, 0] (log-dist) |
| `searchMemories` | boolean | no | true | Restrict to entities seen in corpus |

### 15.5 `validate_knowledge`

TransE triple score `score = −‖h + r − t‖₂`.

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `subject`, `predicate`, `object` | string | yes | |
| `subjectType`, `objectType` | string | no | entity-type enum (no `Unknown`) |

**Returns:** `{ score, verdict: "valid" \| "uncertain" \| "unlikely", supporting_memories?, contradicting_memories? }`.

### 15.6 `get_entity_graph`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `centerEntity` | string | no | | Focal node |
| `maxNodes` | integer | no | 50 | [1, 500] |
| `entityTypes` | string[] | no | | entity-type enum |
| `minRelationScore` | number | no | 0.3 | [0, 1] |
| `includeMemoryCounts` | boolean | no | true | |

**Returns:** `{ nodes, edges, center }`. Read-only.

---

## 16. Embedder-first search tools (`definitions/embedder.rs`)

Embedder enum restricted to HNSW-backed only (`E1..=E5, E7..=E11`) for search/cluster/compare. Fingerprint/anomaly tools accept all 13.

### 16.1 `search_by_embedder`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `embedder` | string | yes | | HNSW-only (10 values) |
| `query` | string | yes | | |
| `topK` | integer | no | 10 | [1, 100] |
| `minSimilarity` | number | no | 0 | [0, 1] |
| `includeContent` | boolean | no | false | |
| `includeAllScores` | boolean | no | false | All-13 breakdown |

### 16.2 `get_embedder_clusters`

HDBSCAN on one embedder's HNSW space.

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `embedder` | string | yes | | HNSW-only enum |
| `minClusterSize` | integer | no | 3 | [2, 50] |
| `topClusters` | integer | no | 10 | [1, 50] |
| `includeSamples` | boolean | no | true | |
| `samplesPerCluster` | integer | no | 3 | [1, 10] |

### 16.3 `compare_embedder_views`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `query` | string | yes | | |
| `embedders` | string[] | yes | | 2..=5; HNSW-only enum |
| `topK` | integer | no | 5 | [1, 20] |
| `includeContent` | boolean | no | false | |

**Returns:** `{ rankings: {E1: [...], E11: [...]}, agreement, unique_per_embedder }`.

### 16.4 `list_embedder_indexes`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `includeDetails` | boolean | no | true |

**Returns:** `{ indexes: [{embedder, dim, type, vector_count, size_bytes, gpu_resident, latency_p50?}] }`. Read-only.

### 16.5 `get_memory_fingerprint`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `memoryId` | uuid | yes | | |
| `embedders` | string[] | no | all 13 | `E1..=E13` |
| `includeVectorNorms` | boolean | no | true | |
| `includeContent` | boolean | no | false | |

**Returns:** per-embedder `{dim, norm, present, non_zero_count? (sparse), asymmetric_variants?}`. Asymmetric embedders (E5, E8, E10) include both directional vectors.

### 16.6 `create_weight_profile`

Session-scoped custom profile.

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `name` | string | yes | | 1..=64 chars; must not collide with built-in |
| `weights` | object | yes | | `E1..E13 ∈ [0,1]`, ≈sum-to-1 |
| `description` | string | no | | |

**Returns:** `{ name, weights, session_id }`. Side effect: session-local in-memory store; consumed by `search_graph` and `get_unified_neighbors`.

### 16.7 `search_cross_embedder_anomalies`

Finds `high(X) ∧ low(Y)` memories.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | |
| `highEmbedder` | string | yes | | `E1..E13` |
| `lowEmbedder` | string | yes | | `E1..E13` |
| `highThreshold` | number | no | 0.5 | [0, 1] |
| `lowThreshold` | number | no | 0.3 | [0, 1] |
| `topK` | integer | no | 10 | [1, 100] |
| `includeContent` | boolean | no | false | |
| `primaryEmbedder` | string | no | `E1` | `E1..E13` |
| `contrastEmbedder` | string | no | `E9` | `E1..E13` |

### 16.8 `search_by_tokens` (standalone)

E12 ColBERT MaxSim.

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `query` | string | yes | | |
| `topK` | integer | no | 10 | [1, 100] |
| `minSimilarity` | number | no | 0.3 | [0, 1] |

### 16.9 `search_by_expansion` (standalone)

E13 SPLADE.

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `query` | string | yes | | |
| `topK` | integer | no | 10 | [1, 100] |
| `minScore` | number | no | 0.1 | [0, 1] |

---

## 17. Temporal tools (E2/E3) (`definitions/temporal.rs`)

Post-retrieval boost per ARCH-25.

### 17.1 `search_recent`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `query` | string | yes | | |
| `topK` | integer | no | 10 | [1, 100] |
| `temporalWeight` | number | no | 0.3 | [0.1, 1.0] |
| `decayFunction` | string | no | `exponential` | `linear` \| `exponential` \| `step` \| `none` \| `no_decay` |
| `temporalScale` | string | no | `meso` | `micro` (1h) \| `meso` (1d) \| `macro` (1w) \| `long` (1mo) \| `archival` (1y) |
| `includeContent` | boolean | no | true | |
| `minSimilarity` | number | no | 0.1 | [0, 1] |

### 17.2 `search_periodic`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `query` | string | yes | | |
| `topK` | integer | no | 10 | [1, 100] |
| `targetHour` | integer | no | now-hour if `autoDetect` | [0, 23] |
| `targetDayOfWeek` | integer | no | now-dow if `autoDetect` | [0, 6] |
| `autoDetect` | boolean | no | false | |
| `periodicWeight` | number | no | 0.3 | [0.1, 1.0] |
| `includeContent` | boolean | no | true | |
| `minSimilarity` | number | no | 0.1 | [0, 1] |

---

## 18. Graph linking tools (K-NN) (`definitions/graph_link.rs`)

### 18.1 `get_memory_neighbors`

Pre-computed per-embedder K-NN edges. Returns 0 for recently stored memories until the background builder runs (~60 s). Use `get_unified_neighbors` for immediate results.

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `memory_id` | uuid | yes | | |
| `embedder_id` | integer | no | 0 | [0, 12] (0=E1, 4=E5, 6=E7, 7=E8, 9=E10, 10=E11) |
| `top_k` | integer | no | 10 | [1, 50] |
| `min_similarity` | number | no | 0.0 | [0, 1] |
| `include_content` | boolean | no | false | |

### 18.2 `get_typed_edges`

Edge types (from embedder-agreement patterns): `semantic_similar`, `code_related`, `entity_shared`, `causal_chain`, `graph_connected`, `paraphrase_aligned`, `keyword_overlap`, `multi_agreement`.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `memory_id` | uuid | yes | | |
| `edge_type` | string | no | | 8-value enum above |
| `direction` | string | no | `outgoing` | `outgoing` \| `incoming` \| `both` |
| `min_weight` | number | no | 0.0 | [0, 1] |
| `include_content` | boolean | no | false | |

`target_id` in the response is the peer for both directions (source for incoming, target for outgoing).

### 18.3 `traverse_graph`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `start_memory_id` | uuid | yes | | |
| `max_hops` | integer | no | 2 | [1, 5] |
| `edge_type` | string | no | | typed-edge enum |
| `min_weight` | number | no | 0.3 | [0, 1] |
| `max_results` | integer | no | 20 | [1, 100] |
| `include_content` | boolean | no | false | |

### 18.4 `get_unified_neighbors`

Weighted RRF across all 13 embedders (ARCH-21). Temporal embedders excluded from semantic fusion per AP-60.

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `memory_id` | uuid | yes | | |
| `weight_profile` | string | no | `semantic_search` | (same 14 values as `search_graph`) |
| `top_k` | integer | no | 10 | [1, 50] |
| `min_score` | number | no | 0.0 | [0, 1] |
| `include_content` | boolean | no | false | |
| `include_embedder_breakdown` | boolean | no | true | |
| `custom_weights` | object | no | | `E1..E13 ∈ [0, 1]` |
| `exclude_embedders` | string[] | no | | `E1..E13` |

---

## 19. Maintenance tools (`definitions/maintenance.rs`)

### 19.1 `repair_causal_relationships`

No params. Scans `CF_CAUSAL_RELATIONSHIPS` and deletes entries that fail deserialization. **Returns:** `{ deleted_count, total_scanned }`. Side effect: RocksDB deletes; idempotent.

---

## 20. Provenance tools (`definitions/provenance.rs`)

### 20.1 `get_audit_trail`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `target_id` | uuid | no | | |
| `start_time` | ISO-8601 | no | | |
| `end_time` | ISO-8601 | no | | |
| `limit` | integer | no | 50 | ≤500 |

**Returns:** `{ records: [{timestamp, op, target_id, operator_id, details}] }` sorted chronologically. Reads `CF_AUDIT_LOG`.

### 20.2 `get_merge_history`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `memory_id` | uuid | yes | |
| `include_source_metadata` | boolean | no | false |

**Returns:** `{ merges: [{merge_id, source_ids, target_id, strategy, operator, reversal_hash, timestamp}] }`.

### 20.3 `get_provenance_chain`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `memory_id` | uuid | yes | |
| `include_audit` | boolean | no | false |
| `include_embedding_version` | boolean | no | false |
| `include_importance_history` | boolean | no | false |
| `include_merge_history` | boolean | no | false |
| `depth_full` | boolean | no | false (expands all four `include_*`) |

**Returns:** `{ source: {...}, operator_attribution, causal_direction, created_at, audit?, embedding_version?, importance_history?, merge_history? }`.

---

## 21. Daemon tools (`definitions/daemon.rs`)

### 21.1 `daemon_status`

No params. **Returns:** `{ pid, uptime_secs, active_connections, model_state, background_tasks: {gc, hnsw_persist, graph_builder} }`. Used to verify multi-agent setup.

---

## 22. Training tools (`definitions/training.rs`)

Writes to `CF_TRAINING_RECORDS` (bincode + version byte). No external file output.

### 22.1 `export_training_corpus`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `filter` | string | no | `all` | `all` \| `session` |
| `filterId` | string | no | | Required with `filter=session` |
| `maxMemories` | integer | no | 10000 | [1, 1000000] |
| `includeEmbeddings` | boolean | no | true | Dense E1/E2/E3/E4/E5/E7/E8/E9/E10/E11 |
| `includeSparseVectors` | boolean | no | true | E6, E13 |
| `includeTokenEmbeddings` | boolean | no | false | E12 (≈15 KB/record) |
| `includeEdges` | boolean | no | true | Outgoing typed edges + per-embedder K-NN |
| `includeCausalLabels` | boolean | no | true | `causal_effects` only |
| `includeIncomingEdges` | boolean | no | false | Requires O(N) reverse scan |
| `includeTemporalLabels` | boolean | no | true | Hour-of-day, dow, age, session position |
| `includeTuckerCore` | boolean | no | false | Tucker-1 ranks (4,4,128), CPU |
| `clearExisting` | boolean | no | false | Wipe CF before export |

**Returns:** `{ exported_count, skipped_count, bytes_written, clear_existing }`.

### 22.2 `list_training_records`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `limit` | integer | no | 50 | [1, 1000] |
| `offset` | integer | no | 0 | ≥0 |
| `includeShape` | boolean | no | true | Adds 1 read/UUID |

### 22.3 `get_training_record`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `memoryId` | uuid | yes | |
| `includeVectors` | boolean | no | false |
| `includeTokenEmbeddings` | boolean | no | false |
| `includeEdges` | boolean | no | true |

### 22.4 `count_training_records`

No params. **Returns:** `{ count }`.

---

## 23. Constellation tools (`definitions/constellation.rs`)

See `17_constellation_subsystem.md` for math (per-embedder centroids, 95th-percentile spread, coherence, purity).

### 23.1 `compile_constellation`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `selector` | string | yes | | `topic` \| `session` \| `tag` \| `time_range` \| `explicit_ids` |
| `topicId` | string | cond (selector=topic) | | |
| `sessionId` | string | cond (selector=session) | | |
| `tag` | string | cond (selector=tag) | | |
| `startIso` / `endIso` | date-time | cond (selector=time_range) | | RFC-3339 |
| `memoryIds` | uuid[] | cond (selector=explicit_ids) | | |
| `rationale` | string | no | | Annotation for `explicit_ids`; not hashed |
| `label` | string | yes | | 1..=256 chars |
| `maxMembers` | integer | no | 50000 | [3, 100000] |
| `rebuildIfExists` | boolean | no | false | |

**Returns:** `{ constellation_id, member_count, shape_summary }`.
**Errors:** `TooFewMembers` (< 3), `TooManyMembers` (> `maxMembers`).
**Side effects:** row in `CF_CONSTELLATIONS` + selector index.

### 23.2 `list_constellations`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `limit` | integer | no | 50 | [1, 1000] |
| `offset` | integer | no | 0 | ≥0 |
| `includeCentroids` | boolean | no | false | |

### 23.3 `get_constellation`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `constellationId` | uuid | yes | |
| `includeCentroids` | boolean | no | true |

### 23.4 `score_against_constellation`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `constellationId` | uuid | yes | |
| `memoryId` | uuid | yes | Must exist in `CF_FINGERPRINTS` |

**Returns:** `{ per_embedder_similarity, combined_score, in_spread_p95 }`.

### 23.5 `delete_constellation`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `constellationId` | uuid | yes | |

**Returns:** `{ deleted: bool }`. Idempotent; removes selector-index entry.

---

## 24. Contrastive pair tools (`definitions/contrastive.rs`)

AnomalyKind enum (`snake_case`): `semantic_but_not_causal`, `keyword_but_not_paraphrase`, `code_shape_but_different_intent`, `entity_shared_but_different_structure`, `hdc_robust_but_semantic_different`, `other`.

### 24.1 `mine_contrastive_pairs`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `maxPairs` | integer | no | 1000 | [1, 100000] |
| `minDisagreement` | number | no | 0.3 | [0, 1] |
| `highThreshold` | number | no | 0.6 | [0, 1] |
| `lowThreshold` | number | no | 0.3 | [0, 1]; must be < `highThreshold` |
| `kinds` | string[] | no | all | AnomalyKind enum |
| `sessionFilter` | string | no | | |
| `topKCandidatesPerAnchor` | integer | no | 10 | [1, 500] |
| `candidatePoolSize` | integer | no | 500 | [10, 50000] |

**Side effects:** upserts rows in `CF_CONTRASTIVE_PAIRS` (+ by-kind and by-anchor indexes). Idempotent on `(anchor_id, negative_id)`.

### 24.2 `list_contrastive_pairs`

| Param | Type | Req | Default | Range / Enum |
|-------|------|:--:|---------|--------------|
| `limit` | integer | no | 100 | [1, 10000] |
| `offset` | integer | no | 0 | ≥0 |
| `kind` | string | no | | AnomalyKind enum |
| `anchorId` | uuid | no | | |
| `includeFull` | boolean | no | false | Fetch full record |

### 24.3 `get_contrastive_pair`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `anchorId` | uuid | yes | |
| `negativeId` | uuid | yes | |

### 24.4 `count_contrastive_pairs`

| Param | Type | Req | Default |
|-------|------|:--:|---------|
| `kind` | string | no | AnomalyKind enum (omitted = total) |

---

## 25. Typed-edge training tools (`definitions/typed_edges.rs`)

Backed by `CF_TYPED_EDGE_RECORDS` (export), `CF_CONTRASTIVE_PAIRS` (derive), `CF_TYPED_EDGE_VALIDATIONS` (validate).

### 25.1 `export_typed_edges_corpus`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `maxEdges` | integer | no | 10000 | [1, 1000000] |
| `includeContent` | boolean | no | true | Source/target content |
| `includeSourceMetadata` | boolean | no | true | session_id, source_type |
| `includeMechanismType` | boolean | no | true | Joins `CF_CAUSAL_RELATIONSHIPS` for `causal_chain` edges |
| `joinLLMValidation` | boolean | no | true | Attach `LLMValidationSummary` when present |
| `clearExisting` | boolean | no | false | |

Idempotent on `(source, target, edge_type)`.

### 25.2 `derive_anomalies_from_edges`

Classifies typed edges into five AnomalyKinds (the sixth, `other`, is not derivable here) and persists to `CF_CONTRASTIVE_PAIRS` with `generator="typed_edge_anomaly_derivation_v1"`.

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `highThreshold` | number | no | 0.60 | [0, 1] |
| `lowThreshold` | number | no | 0.30 | [0, 1] |
| `minDisagreement` | number | no | 0.30 | [0, 1] |
| `maxPairs` | integer | no | 10000 | [1, 1000000] |
| `kinds` | string[] | no | | **Not yet supported** — providing returns `INVALID_PARAMS` |

### 25.3 `validate_low_confidence_edges` (requires llm)

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `weightCeiling` | number | no | 0.60 | [0, 1] — only edges below are eligible |
| `maxEdges` | integer | no | 100 | [1, 10000] |
| `sessionFilter` | string | no | | |

Skips edges that already have a validation row. Without `llm` feature → returns `{ status: "skipped", reason }` (not an error).

### 25.4 `list_typed_edge_records`

| Param | Type | Req | Default | Range |
|-------|------|:--:|---------|-------|
| `limit` | integer | no | 100 | [1, 1000] |
| `offset` | integer | no | 0 | ≥0 |
| `includeFull` | boolean | no | false | Hydrate full `TypedEdgeTrainingRecord` |

---

## Appendix A — Cross-references

- Embedder dimensions, active vectors, asymmetric rules: `05_embedding_subsystem.md`.
- Strategies (`e1_only`, `multi_space`, `pipeline`), RRF fusion, gate thresholds: `06_search_and_fusion.md`.
- Typed-edge derivation, K-NN graph build schedule, causal chain math: `07_graph_and_causal.md`.
- Topic detection (HDBSCAN, weighted-agreement threshold, AP-60/62/63/77): `12_clustering_and_topics.md`.
- Training record shape and Tucker-1 core: `16_training_subsystem.md`.
- Constellation selector hash and scoring: `17_constellation_subsystem.md`.
- Contrastive anomaly classifier and typed-edge LLM validation: `18_contrastive_and_typed_edges.md`.
- RocksDB CFs referenced above: `04_database_schema.md`.
