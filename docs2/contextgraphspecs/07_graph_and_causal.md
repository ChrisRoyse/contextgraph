# Graph and Causal Subsystem

Source files covered:

- `crates/context-graph-core/src/graph/{mod.rs,asymmetric.rs}`
- `crates/context-graph-core/src/graph_linking/{mod.rs,direction.rs,edge_builder.rs,edge_type.rs,embedder_edge.rs,error.rs,knn_graph.rs,nn_descent.rs,service.rs,storage_keys.rs,thresholds.rs,typed_edge.rs,weight_projector.rs}`
- `crates/context-graph-core/src/causal/{mod.rs,asymmetric.rs,chain.rs,inference.rs,scm.rs}`
- `crates/context-graph-core/src/llm_edge_validation/mod.rs`
- `crates/context-graph-core/src/typed_edge_export/mod.rs` (LLMVerdict)
- `crates/context-graph-causal-agent/src/{lib.rs,service/mod.rs,llm/{mod.rs,prompt.rs},scanner/mod.rs,activator/mod.rs,types/mod.rs}`
- `crates/context-graph-graph-agent/src/{lib.rs,service/mod.rs,llm/{mod.rs,prompt.rs},scanner/mod.rs,activator/mod.rs,types/mod.rs}`
- `crates/context-graph-storage/src/graph_edges/{mod.rs,builder.rs,repository.rs,serialization.rs,types.rs}`
- `crates/context-graph-mcp/src/handlers/tools/{graph_tools.rs,graph_link_tools.rs,causal_tools.rs,causal_discovery_tools.rs,causal_relationship_tools.rs}`

Cross-references: [04_database_schema.md](04_database_schema.md), [05_embedding_subsystem.md](05_embedding_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

---

## 1. Graph Data Model

### 1.1 Nodes

Nodes are memories — each identified by a `uuid::Uuid` v4 assigned at `store_memory` time. Node state lives in the teleological store; the graph subsystem only indexes UUIDs and stores edge payloads. See [04_database_schema.md](04_database_schema.md) for memory-side CFs.

### 1.2 Edge Classes

Two disjoint edge classes are persisted:

| Class | Rust type | CF | Meaning |
|-------|-----------|----|---------|
| Per-embedder K-NN | `EmbedderEdge` | `CF_EMBEDDER_EDGES` (`embedder_edges`) | K nearest neighbors of a source node in one embedder's vector space |
| Multi-relation typed | `TypedEdge` | `CF_TYPED_EDGES` (`typed_edges`) | Classified link derived from embedder-agreement patterns; 8 variants |

Secondary index: `CF_TYPED_EDGES_BY_TYPE` (`typed_edges_by_type`) for fast `(edge_type, source) -> targets` lookup.

### 1.3 `EmbedderEdge` (per-embedder K-NN)

| Field | Type | Constraint |
|-------|------|------------|
| `source` | `Uuid` | Required |
| `target` | `Uuid` | Required |
| `embedder_id` | `u8` | 0..=12 |
| `similarity` | `f32` | `[-1.0, 1.0]` |
| `direction` | `DirectedRelation` | `Symmetric` for all embedders except E5/E8 |

Construction rules:

- `EmbedderEdge::new()` rejects E5 (id=4) and E8 (id=7) with `EdgeError::SymmetricCosineViolation`; callers must use `EmbedderEdge::with_direction()` for asymmetric embedders.
- Temporal embedders E2–E4 (ids 1..=3) are flagged by `is_temporal()` and excluded from edge-type detection.

### 1.4 `TypedEdge` (multi-relation)

| Field | Type | Notes |
|-------|------|-------|
| `source` | `Uuid` | — |
| `target` | `Uuid` | — |
| `edge_type` | `GraphLinkEdgeType` | u8 enum, 8 variants |
| `weight` | `f32` | `[0.0, 1.0]` |
| `direction` | `DirectedRelation` | `Symmetric` / `Forward` / `Backward` |
| `embedder_scores` | `[f32; 13]` | Per-embedder raw similarity |
| `agreement_count` | `u8` | Must equal `agreeing_embedders.count_ones()` |
| `agreeing_embedders` | `u16` | Bitset; bit 0 = E1, bit 12 = E13 |

Constructor invariants (enforced by `TypedEdge::new`):

| Check | Error on violation |
|-------|--------------------|
| `weight ∈ [0,1]` | `InvalidSimilarityScore` |
| Asymmetric `edge_type` + `Symmetric` direction | `DirectionRequired` |
| `agreement_count != popcount(agreeing_embedders)` | `AgreementCountMismatch` |

### 1.5 `DirectedRelation`

| Variant | u8 | `similarity_modifier()` | Use |
|---------|----|-----------|-----|
| `Symmetric` | 0 | 1.0 | Default; all embedders except E5/E8 |
| `Forward` | 1 | 1.2 | E5: cause→effect; E8: source→target |
| `Backward` | 2 | 0.8 | E5: effect→cause; E8: target→source |

`reverse()` swaps `Forward` ↔ `Backward`; `Symmetric` is fixed.

---

## 2. K-NN Graph Construction

### 2.1 Algorithm

`NnDescent` (in `graph_linking/nn_descent.rs`) builds approximate K-NN per embedder using the "neighbor-of-neighbor" iterative improvement algorithm.

| Constant | Default | Module |
|----------|---------|--------|
| `KNN_K` | 20 | `graph_linking` |
| `NN_DESCENT_ITERATIONS` | 8 | `graph_linking` |
| `NN_DESCENT_SAMPLE_RATE` (ρ) | 0.5 | `graph_linking` |
| `MIN_KNN_SIMILARITY` | 0.3 | `graph_linking` |
| `early_termination_threshold` | 0.001 | `NnDescentConfig` |

`NnDescentConfig` accepts an optional seed for reproducibility and supports asymmetric embedders via `build_asymmetric_knn`.

### 2.2 In-memory structure

`KnnGraph` (adjacency `HashMap<Uuid, Vec<EmbedderEdge>>` + edge counter):

- `add_edge(edge)` — capacity-bounded eviction: if at `k` capacity, replaces the lowest-similarity neighbor iff the new similarity is strictly greater; updates in place if `(source,target)` already present and new similarity is higher.
- `get_neighbors(node) -> &[EmbedderEdge]` — unsorted.
- `get_neighbors_sorted(node) -> Vec<EmbedderEdge>` — descending by similarity.
- `stats() -> KnnGraphStats` — embedder_id, k, node/edge counts, min/max/avg neighbors, min/max/avg similarity.

### 2.3 Per-embedder indexing

K-NN graphs are built only for semantic (non-temporal) embedders. `GraphLinkServiceConfig.active_embedders` default: `[0, 4, 6, 7, 9, 10]` = E1, E5, E7, E8, E10, E11. K-NN results are persisted via `EdgeRepository.store_embedder_edges(embedder_id, source, &[EmbedderEdge])`; lookups at query time go through `get_embedder_edges(embedder_id, source)`. K-NN neighbors are **not** carried as a field on `SemanticFingerprint` — they live exclusively in `CF_EMBEDDER_EDGES`.

---

## 3. Typed-Edge Taxonomy

`GraphLinkEdgeType` (`repr(u8)`, 8 variants, `Self::COUNT = 8`):

| u8 | Variant | Display string | Primary embedder | Default threshold | Asymmetric | Meaning |
|----|---------|----------------|------------------|-------------------|------------|---------|
| 0 | `SemanticSimilar` | `semantic_similar` | E1 (idx 0) | 0.75 | No | Broad semantic meaning match |
| 1 | `CodeRelated` | `code_related` | E7 (idx 6) | 0.70 | No | Shared code patterns / signatures |
| 2 | `EntityShared` | `entity_shared` | E11 (idx 10) | 0.65 | No | Shared named entities (KEPLER) |
| 3 | `CausalChain` | `causal_chain` | E5 (idx 4) | 0.60 | **Yes** | Cause→effect relationship |
| 4 | `GraphConnected` | `graph_connected` | E8 (idx 7) | 0.60 | **Yes** | Source→target graph connectivity |
| 5 | `ParaphraseAligned` | `paraphrase_aligned` | E10 (idx 9) | 0.70 | No | Same meaning, different surface form |
| 6 | `KeywordOverlap` | `keyword_overlap` | E6 (idx 5) / E13 (idx 12) | 0.50 | No | Lexical / sparse overlap |
| 7 | `MultiAgreement` | `multi_agreement` | 3+ embedders | 0.60 | No | Multiple perspectives confirm |

Helpers: `is_asymmetric()` (true for variants 3, 4), `primary_embedder_index()` (None for `MultiAgreement`), `asymmetric_variants()`, `symmetric_variants()`, `from_u8(u8) -> Option<Self>`. Serde encodes as snake_case.

### 3.1 Edge-type auto-detection

`TypedEdge::from_scores(source, target, scores, thresholds, direction)` in `typed_edge.rs`:

1. Build `agreeing_embedders` bitset: for each `i ∈ 0..13`, skip `i ∈ 1..=3` (temporal excluded), set bit `i` iff `scores[i] >= thresholds[i]`.
2. If `agreement_count >= 3` → `MultiAgreement`; otherwise priority order (first matching wins):
   E7 → `CodeRelated`; E11 → `EntityShared`; E5 → `CausalChain`; E8 → `GraphConnected`; E10 → `ParaphraseAligned`; E6 or E13 → `KeywordOverlap`; fallback → `SemanticSimilar`.
3. Weight: `MultiAgreement` → mean of agreeing scores; otherwise primary-embedder score clamped to `[0,1]`.
4. Direction coerced: asymmetric types default to `Forward` if caller passed `Symmetric`; symmetric types forced to `Symmetric`.

### 3.2 `EdgeThresholds`

Struct in `thresholds.rs` with fields per edge type (see defaults above) plus `multi_agreement_min_embedders: u8` (default 3). `DEFAULT_THRESHOLDS` is the baseline; callers may pass custom thresholds via builder pattern.

`DEFAULT_EMBEDDER_THRESHOLDS: [f32; 13]` (in `edge_builder.rs`) — per-embedder thresholds used during typed-edge derivation: `[0.75, 0.70, 0.70, 0.70, 0.60, 0.50, 0.70, 0.60, 0.60, 0.70, 0.65, 0.60, 0.50]`.

---

## 4. Edge Storage Layout

All key formats are fixed-width, big-endian byte layouts in RocksDB.

### 4.1 `CF_EMBEDDER_EDGES` key — `EdgeStorageKey`

| Offset | Bytes | Field |
|--------|-------|-------|
| 0 | 1 | `embedder_id` (u8, 0..=12) |
| 1..17 | 16 | `source_uuid` |

Total: **17 bytes**. Value: `Vec<EmbedderEdge>` (bincode). Prefix scan by 1-byte embedder id yields all sources. Prefix scan by 17 bytes yields the single-source entry.

### 4.2 `CF_TYPED_EDGES` key — `TypedEdgeStorageKey`

| Offset | Bytes | Field |
|--------|-------|-------|
| 0..16 | 16 | `source_uuid` |
| 16..32 | 16 | `target_uuid` |

Total: **32 bytes**. Value: single `TypedEdge` (bincode). Source-prefix scan (16 bytes) yields all outgoing edges.

### 4.3 `CF_TYPED_EDGES_BY_TYPE` secondary-index key

| Offset | Bytes | Field |
|--------|-------|-------|
| 0 | 1 | `edge_type as u8` |
| 1..17 | 16 | `source_uuid` |
| 17..33 | 16 | `target_uuid` |

Total: **33 bytes**. Value: `target_uuid` bytes (redundant with key for fast iteration). The 16-byte target suffix in the key prevents key collisions when the same source has multiple edges of the same type to different targets — without it, a single source could only have one edge per type. Prefix scan with 17 bytes `(edge_type, source)` returns all targets for that pair.

### 4.4 Incoming-edge lookups (target-side)

There is no symmetric `target`-keyed secondary index. `EdgeRepository::get_typed_edges_to(target)` performs a **full CF scan** of `CF_TYPED_EDGES` and filters deserialized edges by `edge.target() == target` — O(n) in total edges. Called by `get_typed_edges(direction="incoming")` and `get_typed_edges(direction="both")`. For bulk export, `iter_all_typed_edges()` returns every edge once so callers can build their own reverse index.

---

## 5. `EdgeRepository` API

`context_graph_storage::graph_edges::EdgeRepository` (constructor panics if any of the three CFs is missing):

| Method | Purpose | Cost |
|--------|---------|------|
| `store_embedder_edges(embedder_id, source, &[EmbedderEdge])` | Replace K-NN for `(embedder_id, source)` | O(k) |
| `get_embedder_edges(embedder_id, source) -> Vec<EmbedderEdge>` | Point lookup | O(k) |
| `delete_embedder_edges(embedder_id, source)` | Point delete | O(1) |
| `iter_embedder_edges(embedder_id) -> Iter` | Prefix scan per embedder | O(nodes) |
| `store_typed_edge(&TypedEdge)` | WriteBatch: primary + secondary | O(1) |
| `store_typed_edges_batch(&[TypedEdge])` | Single `WriteBatch` | O(batch) |
| `get_typed_edge(source, target) -> Option<TypedEdge>` | Point lookup | O(1) |
| `delete_typed_edge(source, target)` | Batch delete primary + secondary | O(1) |
| `get_typed_edges_from(source) -> Vec<TypedEdge>` | Source-prefix scan | O(out-degree) |
| `get_typed_edges_to(target) -> Vec<TypedEdge>` | Full scan + filter | O(total edges) |
| `get_typed_edges_by_type(source, edge_type) -> Vec<TypedEdge>` | Secondary-index prefix + primary fetch | O(out-degree of type) |
| `iter_all_typed_edges() -> Vec<TypedEdge>` | Full CF materialise | O(total edges) |
| `is_empty()` / `get_stats() -> GraphEdgeStats` | Diagnostics | O(1) / O(total) |

Invalid `embedder_id` (>12) returns `GraphEdgeStorageError::InvalidEmbedderId`. All errors use fail-fast propagation; no silent fallbacks.

---

## 6. Graph Traversal

BFS implementation lives in the MCP handler `call_traverse_graph` (`graph_link_tools.rs`) — there is no separate traversal crate. Characteristics:

| Aspect | Value |
|--------|-------|
| Algorithm | BFS with visited-set |
| Queue item | `(Uuid, hop_level, cumulative_weight, path, edge_types, edge_weights)` |
| Depth limit | `max_hops` (MCP param, 1..=5, default 2) |
| Edge-type filter | Optional `edge_type` param → calls `get_typed_edges_by_type(current, type)` |
| Weight filter | `min_weight` (default 0.3); below-threshold edges skipped |
| Result cap | `max_results` (1..=100, default 20); BFS breaks after the current depth layer once the cap is exceeded |
| Cumulative weight | Multiplicative product of edge weights along the path |
| Self-skip | `neighbor_id == current_id` or already-visited → skipped |

Response fields: `nodes[]` (with `hop_level`, `edge_type_from_parent`, `edge_weight_from_parent`, `cumulative_weight`), `paths[]` (path UUIDs, `total_weight`, `hop_count`, per-hop `edge_types` and `edge_weights`), `unique_nodes_visited`, `path_count`, `metadata.truncated`, `metadata.edges_evaluated`, `metadata.edges_filtered_by_weight`.

---

## 7. Neighbor and Typed-Edge Tools

### 7.1 `get_memory_neighbors`

Per-embedder K-NN lookup. Params: `memory_id` (UUID), `embedder_id` (0..=12, default 0=E1), `top_k` (1..=50, default 10), `min_similarity` (default 0.0), `include_content`. Path: `EdgeRepository::get_embedder_edges` → filter by `min_similarity` → take `top_k` → optional hydrate content + source via `TeleologicalMemoryStore::get_source_metadata_batch` / `get_content_batch`. Returns zero-result with diagnostic when `EdgeRepository.is_empty()`.

### 7.2 `get_typed_edges`

Direction + type filter combine as follows (`graph_link_tools.rs::call_get_typed_edges`):

| `direction` | `edge_type` filter | Repository path |
|-------------|--------------------|-----------------|
| `outgoing` (default) | set | `get_typed_edges_by_type(memory, type)` |
| `outgoing` | unset | `get_typed_edges_from(memory)` |
| `incoming` | set | `get_typed_edges_to(memory)` → filter by type |
| `incoming` | unset | `get_typed_edges_to(memory)` |
| `both` | set | outgoing (via secondary index) + incoming (full scan + type filter) |
| `both` | unset | `get_typed_edges_from` + `get_typed_edges_to` |

Peer-ID resolution: each returned edge's exposed `target_id` is the node at the **other end** relative to the queried memory — if `edge.source() == queried_memory`, the peer is `edge.target()` (outgoing); otherwise the peer is `edge.source()` (incoming). The `direction` field on each result is recomputed per edge to reflect actual orientation, not the query filter.

Weight filter: `min_weight` default 0.0. Contributing embedders are reconstructed from `edge.agreeing_embedders()` bitset.

### 7.3 `get_unified_neighbors`

Weighted RRF fusion across semantic embedders only (E2–E4 excluded per temporal policy). For each embedder in the active profile, `get_embedder_edges` gives a ranked list; score = `Σ weight_i / (rank_i + RRF_K)`. `RRF_K` is defined in `graph_link_dtos.rs`. Weights come from `weight_profile` (default `"semantic_search"`) or `custom_weights` (13-keyed map, name validation against `EMBEDDER_NAMES` enforced — `AP-NAV-01` fail-fast on unknown names). Returns fused `neighbors[]` with optional per-embedder breakdown and `AgreementSummary`.

---

## 8. Causal Discovery Service

### 8.1 Crate layout

`context-graph-causal-agent` (`cfg(feature = "llm")` for LLM paths):

| Module | Purpose |
|--------|---------|
| `llm` | `CausalDiscoveryLLM` — llama-cpp-2 wrapper, grammar-constrained inference |
| `llm/prompt` | `CausalPromptBuilder` — ChatML prompts (pair / single-text / multi-relationship) |
| `scanner` | `MemoryScanner` — heuristic candidate-pair finder |
| `activator` | `E5EmbedderActivator` — persists confirmed relationships, updates `CausalGraph` |
| `service` | `CausalDiscoveryService` — background loop, cursor persistence |
| `types` | `CausalAnalysisResult`, `CausalCandidate`, `CausalHint`, etc. |

### 8.2 LLM backend

| Field | Default | Notes |
|-------|---------|-------|
| `model_path` | `models/hermes-2-pro/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf` | Loaded via `llama-cpp-2` `LlamaModel` |
| `context_size` | 4096 | KV cache sized accordingly |
| `temperature` | 0.0 | Deterministic |
| `max_tokens` | 512 | Supports 1–3 paragraph descriptions |
| `n_gpu_layers` | `u32::MAX` | Full offload (RTX 5090 target, ~5 GB model + ~1 GB KV) |
| `batch_size` | 2048 | Sized for few-shot prompts |
| `use_few_shot` | true | Adds ~200 tokens, improves direction detection |
| `causal_grammar_path` | `models/hermes-2-pro/causal_analysis.gbnf` | GBNF constraint |
| `graph_grammar_path` | `models/hermes-2-pro/graph_relationship.gbnf` | GBNF constraint |
| `validation_grammar_path` | `models/hermes-2-pro/validation.gbnf` | GBNF constraint |

`GrammarType`: `Causal` / `Graph` / `Validation` / `SingleText` / `MultiRelationship`. GBNF constraints guarantee structurally-valid JSON from the decoder.

`CausalPromptBuilder` formats prompts using Hermes-2-Pro ChatML. Three system prompts live inline in `llm/prompt.rs`:

- `default_system_prompt()` — pair causal analysis (two memories).
- `default_single_text_system_prompt()` — single-text causal-nature classification feeding `CausalHint` during `store_memory`.
- `default_multi_relationship_system_prompt()` — extract all cause→effect pairs from one memory.

`max_content_length` clamps each memory's content to 1500 chars before substitution.

### 8.3 Service loop

`CausalDiscoveryService` holds the LLM (`Arc<CausalDiscoveryLLM>`), a `MemoryScanner` (`RwLock`), an `E5EmbedderActivator`, a `CausalGraph` (`Arc<RwLock>`), and a background task `JoinHandle` (stored so panics aren't silently lost).

`CausalDiscoveryConfig` defaults:

| Field | Default |
|-------|---------|
| `interval` | 120 s |
| `batch_size` | 100 pairs/cycle |
| `min_confidence` | 0.7 |
| `skip_analyzed` | true |
| `enable_background` | false (opt-in) |
| `min_interval` | 30 s |
| `max_interval` | 600 s |
| `max_consecutive_errors` | 3 |

Env overrides (`with_env_overrides`): `CAUSAL_DISCOVERY_ENABLED`, `CAUSAL_DISCOVERY_INTERVAL_SECS`, `CAUSAL_DISCOVERY_BATCH_SIZE`, `CAUSAL_DISCOVERY_MIN_CONFIDENCE`.

Cursor `DiscoveryCursor { last_timestamp, last_fingerprint_id, cycles_completed, total_relationships }` is JSON-serialized to CF_SYSTEM at key `causal_discovery_cursor`. Parse failure starts fresh (no error).

`CycleMetrics`: `cycle_number`, `memories_harvested`, `relationships_discovered`, `relationships_rejected`, `total_duration`.

### 8.4 Scanner

`MemoryScanner` generates `CausalCandidate { cause_memory_id, cause_content, effect_memory_id, effect_content, initial_score, earlier_timestamp, later_timestamp }` using heuristics: E1 semantic similarity band (`[similarity_threshold, max_similarity]` = `[0.5, 0.9]`), temporal ordering, causal-marker presence. `ScannerConfig` defaults: `max_candidates=100`, `min_time_gap=10s`, `max_time_gap=24h`, `same_session_only=false`, `min_initial_score=0.3`.

### 8.5 Activator

`E5EmbedderActivator` requires an `Arc<CausalModel>` (fails with error if absent). For each confirmed `CausalAnalysisResult`:

1. Generate `DirectionalEmbeddings` via `CausalModel.embed_as_cause()` / `embed_as_effect()`.
2. Insert `CausalNode` entries into `CausalGraph` if missing (`auto_create_nodes=true`).
3. Add `CausalEdge` with `cause_embedding`, `effect_embedding`, `llm_confidence`, `mechanism_type`, and optional `reverse_embeddings` for bidirectional.

Config: `min_confidence=0.6`, `update_existing=true`, `auto_create_nodes=true`. Stats: `processed`, `embeddings_generated`, `edges_created`, `skipped_low_confidence`, `skipped_existing`, `errors`.

### 8.6 Trigger entry point

`trigger_causal_discovery` MCP tool (`causal_discovery_tools.rs`):

| Param | Default | Range |
|-------|---------|-------|
| `mode` | `"extract"` | `"extract"` (multi-relationship) \| `"pairs"` (pair analysis) |
| `maxPairs` | 50 | 1..=200 |
| `minConfidence` | 0.7 | 0.5..=1.0 |
| `sessionScope` | `"all"` | `"current"` (last 10 files) \| `"recent"` (last 50) \| `"all"` |
| `dryRun` | false | Analyze without persisting |

`get_causal_discovery_status` returns the current `ServiceStatus` and `DiscoveryCycleResult`.

---

## 9. Causal Data Model

### 9.1 `CausalLinkDirection` (agent-level, LLM output)

| Variant | Serde | Meaning |
|---------|-------|---------|
| `ACausesB` | `a_causes_b` | A → B |
| `BCausesA` | `b_causes_a` | B → A |
| `Bidirectional` | `bidirectional` | A ↔ B |
| `NoCausalLink` | `no_causal_link` | Reject |

### 9.2 `CausalDirection` (similarity-level)

Defined in `core/src/causal/asymmetric.rs`. Four variants used across the similarity stack: `Cause`, `Effect`, `Unknown` (default). Direction is inferred from fingerprint via `infer_direction_from_fingerprint()` comparing `component_variance(e5_as_cause)` vs `component_variance(e5_as_effect)` with a 5% separation threshold.

(Note: `CausalRelationship` search paths additionally use an extended enum with `Bidirectional` and `NonCausal` in the MCP layer — see `search_causal_relationships`.)

### 9.3 `CausalNode`

```
id: Uuid
name: String
domain: String   // e.g. "physics", "economics"
```

### 9.4 `CausalEdge`

| Field | Type | Notes |
|-------|------|-------|
| `source` | `Uuid` | Cause node id |
| `target` | `Uuid` | Effect node id |
| `strength` | `f32` | Clamped to `[0,1]` at construction |
| `mechanism` | `String` | Free-text description |
| `cause_embedding` | `Option<Vec<f32>>` | 768D E5 as cause |
| `effect_embedding` | `Option<Vec<f32>>` | 768D E5 as effect |
| `is_bidirectional` | `bool` | Feedback loop flag |
| `reverse_embeddings` | `Option<(Vec<f32>, Vec<f32>)>` | `(b_cause, a_effect)` for bidirectional |
| `llm_confidence` | `f32` | Raw LLM assessment, separate from `strength` |
| `mechanism_type` | `Option<String>` | `"direct"`, `"mediated"`, `"feedback"`, `"temporal"` |
| `embedding_hint_provenance` | `Option<EmbeddingHintProvenance>` | From `store_memory` hint path |

### 9.5 `CausalGraph` (SCM)

`HashMap<Uuid, CausalNode>` + adjacency `Vec<CausalEdge>`. Operations: `add_node`, `remove_node`, `add_edge`, `get_effects(node)`, `get_causes(node)`, path-finding helpers used by causal chain tools. Guarded by `MAX_NODES=500_000`, `MAX_EDGES=2_000_000`.

### 9.6 `CausalRelationship` (persisted LLM output)

Stored in `CF_CAUSAL_RELATIONSHIPS` (primary, by UUID) with secondary index `CF_CAUSAL_BY_SOURCE` (by source memory id). Key fields:

| Field | Type | Notes |
|-------|------|-------|
| `id` | `Uuid` | Relationship UUID |
| `cause_statement` | `String` | Brief cause |
| `effect_statement` | `String` | Brief effect |
| `explanation` | `String` | LLM 1–3 paragraph narrative |
| `e5_as_cause`, `e5_as_effect` | `Vec<f32>` (768D) | Explanation-anchored dual vectors |
| `e5_source_cause`, `e5_source_effect` | `Vec<f32>` (768D) | Source-anchored dual vectors (prevents explanation clustering) |
| `e1_semantic` | `Vec<f32>` (1024D) | Fallback semantic |
| `e8_graph_source`, `e8_graph_target` | `Vec<f32>` (1024D) | Graph-structure dual |
| `source_memory_id` | `Uuid` | Provenance link to originating memory |
| `confidence` | `f32` | LLM confidence |
| `mechanism_type` | `String` | `"direct"` \| `"mediated"` \| `"feedback"` \| `"temporal"` |
| `llm_provenance` | `LLMProvenance` | Model / quantization / prompt-hash |
| `source_spans` | `Vec<CausalSourceSpan>` | Character offsets + text excerpt |

See [05_embedding_subsystem.md](05_embedding_subsystem.md) for E5/E8/E1 dimensions and [04_database_schema.md](04_database_schema.md) for CF listing.

---

## 10. Causal Chain Reasoning

`causal/chain.rs` provides transitive scoring:

| Constant | Value | Purpose |
|----------|-------|---------|
| `HOP_ATTENUATION` | 0.9 | Multiplicative decay per hop |
| `MIN_CHAIN_SCORE` | 0.1 | Discard noise chains |
| `MAX_CHAIN_LENGTH` | 10 | Prevent combinatorial blow-up |

`CausalHop { base_similarity, from_direction, to_direction }` → `hop_score()` applies `compute_asymmetric_similarity_simple` using the direction modifiers from §9.2.

`compute_chain_score(&[CausalHop]) -> f32`: Σ attenuated product:
```
chain = Π (hop_score_i · ATTENUATION^i)
```

`build_causal_chain`, `score_causal_chain`, `score_causal_chain_attenuated`, `rank_causes_by_abduction` are the public entry points. `AbductionResult` ranks candidate causes for an observed effect.

---

## 11. Causal Direction Modifiers (E5)

`causal/asymmetric.rs::direction_mod`:

| Constant | Value | Pair |
|----------|-------|------|
| `CAUSE_TO_EFFECT` | 1.2 | query=Cause, result=Effect |
| `EFFECT_TO_CAUSE` | 0.8 | query=Effect, result=Cause |
| `SAME_DIRECTION` | 1.0 | Cause↔Cause, Effect↔Effect |
| `UNKNOWN` | 1.0 | Either side unknown |

Applied through `CausalDirection::direction_modifier(query_dir, result_dir)` and baked into `compute_e5_asymmetric_fingerprint_similarity()`.

### 11.1 Causal gate

Post-training calibrated thresholds (`causal_gate` module):

| Constant | Value | Interpretation |
|----------|-------|---------------|
| `CAUSAL_THRESHOLD` | 0.04 | E5 ≥ this → definitely causal |
| `NON_CAUSAL_THRESHOLD` | 0.008 | E5 ≤ this → definitely non-causal |
| `CAUSAL_BOOST` | 1.10 | Multiplier applied when gate passes |
| `NON_CAUSAL_DEMOTION` | 0.85 | Multiplier applied when gate fails |

`apply_causal_gate(score, e5_score, is_causal_query)` — no-op if query is not classified causal; ambiguous zone between thresholds leaves score unchanged.

### 11.2 Intervention overlap (disabled)

The `(0.7 + 0.3 × overlap)` intervention-overlap multiplier is set to `1.0` (disabled) based on empirical negative correlation on benchmark data. `InterventionContext` + `overlap_with()` remain in the API for future re-enablement; current production path does not consume them.

---

## 12. Graph Asymmetric Similarity (E8)

`graph/asymmetric.rs` mirrors the E5 structure for the E8 graph embedder:

| Component | Notes |
|-----------|-------|
| `GraphDirection` | `Source` / `Target` / `Unknown` |
| `SOURCE→TARGET` | 1.2× boost (E8 forward) |
| `TARGET→SOURCE` | 0.8× dampening (E8 backward) |
| `ConnectivityContext` | Optional structural overlap (symmetric to intervention overlap) |
| `compute_e8_asymmetric_fingerprint_similarity` | Primary similarity entry |
| `detect_graph_query_intent` | Heuristic classifier for inbound queries |

E8 uses 1024D `e8_graph_as_source` / `e8_graph_as_target` dual vectors on `SemanticFingerprint` (shares the e5-large-v2 model with E1 for zero extra VRAM).

---

## 13. Graph Discovery Service (LLM-powered)

### 13.1 Crate layout

`context-graph-graph-agent` shares the Hermes-2-Pro LLM via `Arc<CausalDiscoveryLLM>` with the causal agent (no extra VRAM).

| Module | Purpose |
|--------|---------|
| `llm` | `GraphRelationshipLLM` — uses graph-grammar GBNF |
| `llm/prompt` | `GraphPromptBuilder` — content-domain-aware prompts |
| `scanner` | `MemoryScanner` for graph candidates |
| `activator` | `E8Activator` — persists to `GraphStorage` with 1024D E8 embeddings |
| `service` | `GraphDiscoveryService` — optional background loop |
| `types` | `GraphAnalysisResult`, `GraphCandidate`, `MemoryForGraphAnalysis`, etc. |

`E8_DIMENSION = 1024` (constant).

### 13.2 Content domains

`ContentDomain` (serde snake_case): `Code`, `Legal`, `Academic`, `General` (default). Determines relationship-type relevance and informs the LLM system prompt.

### 13.3 Relationship categories & types

`RelationshipCategory` (6 categories) and `RelationshipType` (20 variants: 19 + `None`):

| Category | Types |
|----------|-------|
| Containment | `Contains`, `ScopedBy` |
| Dependency | `DependsOn`, `Imports`, `Requires` |
| Reference | `References`, `Cites`, `Interprets`, `Distinguishes` |
| Implementation | `Implements`, `CompliesWith`, `Fulfills` |
| Extension | `Extends`, `Modifies`, `Supersedes`, `Overrules` |
| Invocation | `Calls`, `Applies`, `UsedBy` |
| — | `None` (no relationship) |

### 13.4 `GraphLinkDirection`

| Variant | Serde | Meaning |
|---------|-------|---------|
| `AConnectsB` | `a_connects_b` | A → B (outgoing from A) |
| `BConnectsA` | `b_connects_a` | B → A |
| `Bidirectional` | `bidirectional` | Mutual reference |
| `NoConnection` | `no_connection` | Reject (default) |

`from_str` accepts aliases: `"a->b"`, `"forward"`, `"source"` → `AConnectsB`; `"b->a"`, `"reverse"`, `"target"` → `BConnectsA`; `"both"`, `"mutual"`, `"symmetric"`, `"a<->b"` → `Bidirectional`.

### 13.5 MCP entry points

- `discover_graph_relationships` (`graph_tools.rs::call_discover_graph_relationships`, llm-feature-gated) — runs the graph-agent scanner + LLM over memories, returns `DiscoveredRelationship[]` with `DiscoveryMetadata`.
- `validate_graph_link` (`call_validate_graph_link`) — LLM validates a proposed `(source, target, edge_type)` triple; outputs verdict + rationale.

---

## 14. LLM Edge Validation

`context_graph_core::llm_edge_validation::LLMEdgeValidation` persists per-edge LLM verdicts in `CF_TYPED_EDGE_VALIDATIONS`. Key format: **33 bytes** `[source(16)][target(16)][edge_type(1)]`. Value: `[LLM_EDGE_VALIDATION_VERSION(1) = 1][bincode LLMEdgeValidation]`.

### 14.1 `LLMEdgeValidation` fields

| Field | Type | Notes |
|-------|------|-------|
| `validated_at` | `DateTime<Utc>` | LLM call wall-clock |
| `verdict` | `LLMVerdict` | `Valid` \| `Invalid` \| `Reclassify { new_edge_type: u8 }` |
| `confidence` | `f32` | `[0,1]` |
| `rationale` | `String` | 1–3 sentence grammar-constrained justification |
| `auto_derived_weight` | `f32` | Weight at validation time (drift-audit hook) |
| `validator_version` | `String` | e.g. `"Hermes-2-Pro-Mistral-7B-Q5_K_M@2026-04-14"` |
| `prompt_hash` | `[u8; 32]` | SHA-256 of the exact prompt sent |

### 14.2 `LLMVerdict`

Bincode-serialized enum. Adding variants requires bumping `LLM_EDGE_VALIDATION_VERSION` and `TYPED_EDGE_RECORD_VERSION`. On decode, version-mismatch returns `CoreError::SerializationError` (no automatic migration).

| Variant | Payload | Meaning |
|---------|---------|---------|
| `Valid` | — | Auto-derived edge confirmed |
| `Invalid` | — | Auto-derived edge rejected |
| `Reclassify` | `{ new_edge_type: u8 }` | Link exists but under a different `GraphLinkEdgeType` (0..=7) |

`as_str()` returns snake-case labels `"valid"` / `"invalid"` / `"reclassify"`.

### 14.3 Companion record CF

`CF_TYPED_EDGE_RECORDS` (defined in teleological column families) stores the auto-derived corpus row for each edge — the raw ensemble output — so a validation row can be joined back to the derivation for training / audit. See [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

### 14.4 Invocation entry

MCP tool `validate_low_confidence_edges` walks `CF_TYPED_EDGE_RECORDS`, selects edges below a weight threshold, invokes the LLM validator, and persists results into `CF_TYPED_EDGE_VALIDATIONS`. `validate_graph_link` validates a single proposed edge directly.

---

## 15. Causal Chain and Causal Search Tools

### 15.1 `search_causes` (`causal_tools.rs`)

Abductive retrieval: "what likely caused X?"

1. Embed query (E1 + E5 dual per scope).
2. Search memories with `SearchStrategy::MultiSpace` + weight profile `"semantic_search"` + 5× over-fetch.
3. Blend scores: **80 % E1 + 20 % E5**, apply **0.8× `ABDUCTIVE_DAMPENING`** (effect→cause direction).
4. Filter by `minScore`, return top-`topK`.

Params: `query`, `topK` (1..=50, default 10), `minScore` (default 0.1), `includeContent`, `filterCausalDirection`, `search_scope` (`"memories"` / `"relationships"` / `"all"`).

### 15.2 `search_effects`

Symmetric to `search_causes` but using cause→effect direction (1.2× boost).

### 15.3 `get_causal_chain`

Builds a transitive chain from a starting memory using `chain::build_causal_chain`, returns `CausalChainHop[]` with per-hop `base_similarity`, `direction`, `hop_score`, and aggregated `chain_score`.

### 15.4 `search_causal_relationships` (`causal_relationship_tools.rs`)

Semantic search over `CF_CAUSAL_RELATIONSHIPS` with direction-aware E5 paths:

| Param | Default | Range |
|-------|---------|-------|
| `query` | required | non-empty |
| `direction` | `"all"` | `"cause"` \| `"effect"` \| `"all"` |
| `topK` | 10 | 1..=100 |
| `includeSource` | true | — |
| `multiEmbedder` | false | Enables E1+E5+E8+E11 fusion |

Direction maps: `"cause"` → query as effect, search cause vectors; `"effect"` → query as cause, search effect vectors; `"all"` → E1 semantic fallback.

Default weights when `multiEmbedder=true`: E1=0.30, E5=0.35, E8=0.15, E11=0.20. Source-anchored vectors (`e5_source_cause`, `e5_source_effect`) are weighted 0.6 / explanation 0.4 to prevent LLM-explanation clustering across distinct source documents.

### 15.5 `get_causal_chain` + `search_connections` + `get_graph_path`

- `search_connections` (`graph_tools.rs`) — E8 asymmetric search; params: `query`, `direction` (`"source"` \| `"target"` \| `"both"`), `topK`, `minScore`, `includeContent`, `includeProvenance`. Uses `graph_reasoning` weight profile with 5× over-fetch; applies E8 direction modifier.
- `get_graph_path` — multi-hop path visualisation using E8 per-hop attenuation.
- `get_causal_chain` — exposes §10 chain reasoning as an MCP tool.

---

## 16. E5 Integration with Search

At search time, `embedder_first`, `multi_space`, and `pipeline` strategies all route through `compute_e5_asymmetric_fingerprint_similarity` for the E5 slot when direction hints are present. Query-side direction is sourced from:

1. Explicit param (`filterCausalDirection`) in causal tools.
2. `detect_causal_query_intent(query)` heuristic classifier (causal markers in query text).
3. Fingerprint inference via `infer_direction_from_fingerprint` on stored memories (component-variance comparison).

Embedder-score slot for E5 is only populated when `CausalDirection` is set; otherwise scores default to 0.0 and E5 contributes nothing to fusion weight. See [05_embedding_subsystem.md](05_embedding_subsystem.md) §E5 and [06_search_and_fusion.md](06_search_and_fusion.md) for fusion details.

---

## 17. Background Activators & Scanners

### 17.1 Threading model

| Component | Runtime | Concurrency control |
|-----------|---------|---------------------|
| `CausalDiscoveryService` | Tokio task; `JoinHandle` retained | `AtomicBool running`, `mpsc::Sender<()>` shutdown |
| `CausalDiscoveryLLM` state | Sync (`parking_lot::RwLock<LlmState>`) | `unsafe impl Send+Sync` — llama.cpp C++ layer is internally mutex-guarded |
| `MemoryScanner` | Sync, held inside service under `RwLock` | Single-writer per service |
| `E5EmbedderActivator` | `Arc`-shared; stateless per call apart from `CausalGraph` `RwLock` | — |
| `CausalGraph` | `Arc<parking_lot::RwLock<CausalGraph>>` | Shared between activator and service |
| `GraphDiscoveryService` | Same pattern as causal; shares `Arc<CausalDiscoveryLLM>` | — |

### 17.2 Shutdown

`shutdown_tx: RwLock<Option<mpsc::Sender<()>>>` drains in-flight cycle; `running` atomic flips to false. Stored `join_handle` is awaited so background task panics surface as errors rather than silently disappearing.

### 17.3 Worker pools

The LLM is single-instance: both causal and graph agents share one `Arc<CausalDiscoveryLLM>` to avoid duplicate VRAM allocation. K-NN construction and edge-builder passes run on the main tokio runtime with fork-join parallelism inside `NnDescent::build()` (synchronous, CPU-bound). There is no dedicated worker-pool abstraction — parallelism comes from tokio's default multi-threaded scheduler.
