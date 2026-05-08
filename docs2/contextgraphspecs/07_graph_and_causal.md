# Graph and Causal

**Source files covered:**
- `crates/context-graph-core/src/graph_linking/{mod,direction,edge_builder,edge_type,embedder_edge,knn_graph,nn_descent,storage_keys,thresholds,typed_edge,weight_projector,service,error}.rs`
- `crates/context-graph-core/src/graph/{mod,asymmetric}.rs` — E8 asymmetric formula, `GraphDirection`, `ConnectivityContext`, `compute_e8_asymmetric_fingerprint_similarity`, `compute_e8_asymmetric_full`, `detect_graph_query_intent`
- `crates/context-graph-core/src/causal/{mod,asymmetric,chain,inference,scm}.rs` — SCM, E5 asymmetric, transitive chain, abduction
- `crates/context-graph-core/src/llm_edge_validation/mod.rs` — `LLMEdgeValidation` row schema, `LLM_EDGE_VALIDATION_VERSION = 1`
- `crates/context-graph-core/src/types/causal_relationship.rs` — `CausalRelationship` (E5 + source-anchored E5 + E1 + E8 + E11), `LLMProvenance`, `CausalSourceSpan`
- `crates/context-graph-storage/src/graph_edges/{mod,repository,builder,serialization,types}.rs` — `EdgeRepository`, `BackgroundGraphBuilder`, `GraphBuilderConfig`, `BuilderStats`
- `crates/context-graph-storage/src/teleological/rocksdb_store/{causal_relationships,causal_hnsw_index,anomaly_derivation,llm_validation,typed_edge_export,typed_edge_keys}.rs`
- `crates/context-graph-causal-agent/src/{lib,error,llm/{mod,prompt},scanner/mod,activator/mod,service/mod,types/mod}.rs` — Hermes-2-Pro causal LLM agent
- `crates/context-graph-graph-agent/src/{lib,stubs,error,llm/{mod,prompt},scanner/mod,activator/mod,service/mod,types/mod}.rs` — Hermes-2-Pro graph LLM agent (shared LLM)
- `crates/context-graph-mcp/src/handlers/tools/{graph_tools,graph_link_tools,causal_tools,causal_relationship_tools,causal_discovery_tools,maintenance_tools,typed_edges_tools,sequence_tools,entity_tools}.rs`
- `models/hermes-2-pro/{causal_analysis.gbnf,graph_relationship.gbnf,validation.gbnf}` — grammar files

Cross-links: [04_database_schema.md](04_database_schema.md) for CF layout, [05_embedding_subsystem.md](05_embedding_subsystem.md) for embedder dimensions and dual-vector contracts, [06_search_and_fusion.md](06_search_and_fusion.md) for the fusion path that produces edge-derived signals, [09_clustering_topics_and_temporal.md](09_clustering_topics_and_temporal.md) for goal/topic centroids consumed by edge anomaly derivation, [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) for typed-edge training records and contrastive pairs.

---

## 1. Two parallel graphs over the same UUIDs

The system maintains two superimposed directed graphs over memory UUIDs. They are consumed by different tool families.

| Graph | Backing CFs | Built by | Read by |
| --- | --- | --- | --- |
| **K-NN + typed-edge graph** | `embedder_edges`, `typed_edges`, `typed_edges_by_type` | `BackgroundGraphBuilder` (NN-Descent) and `GraphDiscoveryService` activator | `get_memory_neighbors`, `get_typed_edges`, `traverse_graph`, `get_unified_neighbors`, `search_connections`, `get_graph_path`, `discover_graph_relationships`, `validate_graph_link`, `validate_low_confidence_edges`, `derive_anomalies_from_edges`, `export_typed_edges_corpus` |
| **Causal SCM** | `causal_relationships`, `causal_by_source` (+ HNSW index file at `<db>/causal_hnsw/`) | `trigger_causal_discovery` → `E5EmbedderActivator` → in-memory `CausalGraph` + RocksDB persist | `search_causes`, `search_effects`, `search_causal_relationships`, `get_causal_chain`, `traverse_memory_chain`, `repair_causal_relationships` |

The graphs share signal: typed edges of kind `CausalChain` are weight-driven by the same E5 dual encoding that anchors `causal_relationships`, and `discover_graph_relationships` persists its LLM verdicts as `GraphConnected` typed edges.

---

## 2. Stored graph data

| Data | CF | Key | Value | Reference |
| --- | --- | --- | --- | --- |
| K-NN edges (per embedder) | `embedder_edges` | 17B `[embedder_id:u8][source:16]` (`EdgeStorageKey`) | bincode `Vec<EmbedderEdge>` (similarity, target) | `graph_linking/storage_keys.rs:39`, `graph_edges/repository.rs:81-141` |
| Typed edges (primary) | `typed_edges` | 32B `[source:16][target:16]` (`TypedEdgeStorageKey`) | bincode `TypedEdge` | `graph_linking/storage_keys.rs:138`, `graph_edges/repository.rs:217-248` |
| Typed-edge type secondary index | `typed_edges_by_type` | 33B `[edge_type:u8][source:16][target:16]` (legacy 17B `[edge_type:u8][source:16]` still readable) | target UUID (16B) | `graph_edges/repository.rs:230-248`, `:386-403` |
| Typed-edge training rows | `typed_edge_records` | 33B `[source:16][target:16][edge_type:u8]` (`typed_edge_record_key`) | `[TYPED_EDGE_RECORD_VERSION:u8][bincode TypedEdgeTrainingRecord]` | `typed_edge_keys.rs:14`, `typed_edge_export.rs` — see [18](18_contrastive_and_typed_edges.md) |
| LLM edge verdicts | `typed_edge_validations` | 33B same shape as records | `[LLM_EDGE_VALIDATION_VERSION=1:u8][bincode LLMEdgeValidation]` | `llm_edge_validation/mod.rs:25,35`, `llm_validation.rs` |
| Causal relationships (primary) | `causal_relationships` | UUID | JSON `CausalRelationship` (E5 dual + source-anchored E5 dual + E1 + E8 dual + E11 + LLM provenance) | `types/causal_relationship.rs:252` |
| Causal-by-source secondary index | `causal_by_source` | source fingerprint UUID | list of causal_relationship UUIDs | `causal_relationships.rs` |
| Causal HNSW index file | `<db>/causal_hnsw/` | persisted blob | dual cause/effect HNSW for `search_causal_e5` over `causal_relationships`; also `CausalE11Index` (768D KEPLER) | `causal_hnsw_index.rs:75` |

The `typed_edges_by_type` legacy 17-byte secondary-index keys are still readable: when the iterator hits a 17-byte row the target UUID is taken from the value, when it hits the 33-byte row the target is taken from `key[17..33]` (`graph_edges/repository.rs:386-403`). `typed_edge_records` and `typed_edge_validations` share the byte-for-byte identical 33-byte composite key defined in one shared module so a future migration touches one place (`typed_edge_keys.rs:14`).

---

## 3. K-NN graph (`EmbedderEdge`)

| Constant | Value | File |
| --- | --- | --- |
| `KNN_K` | 20 | `graph_linking/mod.rs:53` |
| `NN_DESCENT_ITERATIONS` | 8 | `graph_linking/mod.rs:56` |
| `NN_DESCENT_SAMPLE_RATE` | 0.5 | `graph_linking/mod.rs:59` |
| `MIN_KNN_SIMILARITY` | 0.3 | `graph_linking/mod.rs:62` |
| Number of K-NN graphs | `NUM_EMBEDDERS = 14` | `graph_linking/typed_edge.rs:13`, `weight_projector.rs:19` |

K-NN edges are built by `BackgroundGraphBuilder` (`graph_edges/builder.rs:156`) and persisted via `EdgeRepository.store_embedder_edges`. Tools fail closed when the repository is missing (`graph_link_tools.rs:80-89`).

### 3.1 `BackgroundGraphBuilder` (`graph_edges/builder.rs`)

Background K-NN service that batches fingerprints from the teleological store, runs NN-Descent per embedder, then drives `EdgeBuilder` to derive typed edges.

| `GraphBuilderConfig` field | Default | Notes |
| --- | --- | --- |
| `batch_interval_secs` | 60 | Time between batch ticks |
| `max_batch_size` | 1000 | Hard ceiling per tick |
| `min_batch_size` | 10 | Defer until queue reaches this |
| `active_embedders` | `[0, 4, 6, 7, 9, 10]` (E1, E5, E7, E8, E10, E11) | Subset built into K-NN graphs by default |
| `k` | 20 | K-NN neighbours per node |
| `min_weighted_agreement` | 2.5 | Topic threshold per ARCH-09 |
| `process_on_max` | true | Process immediately when `max_batch_size` reached |

`BackgroundGraphBuilder` exposes `enqueue(fingerprint_id)` (called from `store_memory`), an internal `tokio` task driven by `running: AtomicBool`, plus `BatchBuildResult`, `RebuildResult`, and `BuilderStats { total_processed, batches_processed, total_knn_edges, total_typed_edges, queue_size, last_batch_ms }`. M8 fix: queue is capped at 100,000 entries — at 50,000 a warn fires; over 100,000 the oldest 50,000 are drained.

### 3.2 `EdgeBuilderConfig` (`graph_linking/edge_builder.rs:49`)

| Field | Default | Notes |
| --- | --- | --- |
| `min_weighted_agreement` | 2.5 | Below this no `TypedEdge` is emitted |
| `embedder_thresholds` | `DEFAULT_EMBEDDER_THRESHOLDS` | Per-embedder agreement; values below |
| `edge_thresholds` | `DEFAULT_THRESHOLDS` | Per-type minimum weights |
| `include_temporal` | `false` | AP-60: temporal embedders (E2-E4) NEVER count toward edge-type detection |
| `learned_projection` | `None` | Optional `Arc<dyn WeightProjector>` for learned weight head; falls back to constitution-based weighted-agreement when `None` |

`DEFAULT_EMBEDDER_THRESHOLDS` (`edge_builder.rs:31`): E1=0.75, E2=E3=E4=0.70, E5=0.60, E6=0.50, E7=0.70, E8=0.60, E9=0.60, E10=0.70, E11=0.65, E12=0.60, E13=0.50, E14=0.70.

---

## 4. Typed edges — `GraphLinkEdgeType` (8 variants) vs LLM `RelationshipType` (20 values)

There are **two distinct relationship taxonomies**. Don't conflate them.

### 4.1 `GraphLinkEdgeType` — closed 8-variant enum (the storage taxonomy)

`crates/context-graph-core/src/graph_linking/edge_type.rs:48` defines the on-disk edge-type tag stored as `u8` in `typed_edges`. Every K-NN/typed-edge derivation maps onto exactly one of these 8.

| u8 | Variant | Asymmetric? | Primary embedder | Default threshold | Description |
| --- | --- | --- | --- | --- | --- |
| 0 | `SemanticSimilar` | no | E1 (idx 0) | 0.75 | High E1 cosine |
| 1 | `CodeRelated` | no | E7 (idx 6) | 0.70 | Code/implementation |
| 2 | `EntityShared` | no | E11 (idx 10) | 0.65 | Shared KEPLER entities |
| 3 | `CausalChain` | **yes** | E5 (idx 4) | 0.60 | Cause→effect (or backward) |
| 4 | `GraphConnected` | **yes** | E8 (idx 7) | 0.60 | Source→target structural link |
| 5 | `ParaphraseAligned` | no | E10 (idx 9) | 0.70 | Same meaning, different words |
| 6 | `KeywordOverlap` | no | E6 (idx 5) or E13 (idx 12) | 0.50 | Sparse-term overlap |
| 7 | `MultiAgreement` | no | none (≥3 agree) | 0.60 | 3+ embedders cross threshold |

`GraphLinkEdgeType::is_asymmetric()` is true only for `CausalChain` and `GraphConnected` (`edge_type.rs:93-95`). `Display` returns the snake-case form. `from_u8(8..=255)` returns `None` so corrupted tags fail closed.

### 4.2 `RelationshipType` — open 20-value enum (the LLM extraction taxonomy)

`crates/context-graph-graph-agent/src/types/mod.rs:278` defines the relationship the Hermes-2-Pro graph LLM is allowed to emit (constrained by `graph_relationship.gbnf`). The LLM picks one; the activator records it in graph-agent state and persists the edge as `GraphLinkEdgeType::GraphConnected` (the storage taxonomy doesn't subdivide structural links). 19 detectable types + `None`, grouped into 6 categories.

| Category | Types |
| --- | --- |
| Containment | `contains`, `scoped_by` |
| Dependency | `depends_on`, `imports`, `requires` |
| Reference | `references`, `cites`, `interprets`, `distinguishes` |
| Implementation | `implements`, `complies_with`, `fulfills` |
| Extension | `extends`, `modifies`, `supersedes`, `overrules` |
| Invocation | `calls`, `applies`, `used_by` |
| (none) | `none` |

`ContentDomain ∈ {Code, Legal, Academic, General}` selects domain-specific marker dictionaries (`DomainMarkers::CODE`, `LEGAL`, `ACADEMIC`) and prompt template bias. `DomainMarkers::detect_domain` requires ≥ 2 markers and Code must strictly beat both Legal and Academic to be selected (Code is the default fallback so requires stronger evidence — `graph-agent/types/mod.rs:797-829`).

### 4.3 `TypedEdge` structure

`crates/context-graph-core/src/graph_linking/typed_edge.rs:48`:

| Field | Shape | Source |
| --- | --- | --- |
| source / target | UUID (16B each) | `typed_edge.rs:51-53` |
| edge_type | `GraphLinkEdgeType` (u8 repr) | `:55` |
| weight | f32 in [0.0, 1.0] | `:57` |
| direction | `DirectedRelation` (Symmetric / Forward / Backward) | `:59` (`direction.rs:42`) |
| embedder_scores | `[f32; 14]` (one slot per embedder, SRC-3 [0,1]) | `:61` |
| agreement_count | u8 — popcount of `agreeing_embedders` | `:63` |
| agreeing_embedders | u16 bitset (bit i = embedder i) | `:65` |

### 4.4 Auto-classification — `TypedEdge::from_scores`

`typed_edge.rs:145-197`:

1. Build agreement bitset by `score >= per_embedder_threshold`. Temporal embedders E2/E3/E4 (idx 1..=3) are unconditionally excluded per AP-60 (`typed_edge.rs:158-161`).
2. If `agreement_count >= 3` → `MultiAgreement`.
3. Otherwise priority order (`detect_edge_type`, `:200-241`): E7 → E11 → E5 → E8 → E10 → E6/E13 → E1.
4. Weight: `MultiAgreement` = mean of agreeing scores; otherwise = primary embedder's score, clamped to [0, 1] (`calculate_weight`, `:244-275`).
5. Direction: forced to `Symmetric` for symmetric types; for asymmetric types `Symmetric` is upgraded to `Forward`.

`TypedEdge::new` (`:88-131`) is the validated constructor: rejects weight outside [0, 1], rejects asymmetric type with `Symmetric` direction, rejects `agreement_count != popcount(agreeing_embedders)`.

---

## 5. Asymmetric similarity

### 5.1 E5 causal (`crates/context-graph-core/src/causal/asymmetric.rs`)

Direction modifiers per AP-77:

| Direction | Multiplier | Constant |
| --- | --- | --- |
| cause → effect | 1.2 | `direction_mod::CAUSE_TO_EFFECT` |
| effect → cause | 0.8 | `direction_mod::EFFECT_TO_CAUSE` |
| same direction | 1.0 | `direction_mod::SAME_DIRECTION` |
| unknown either side | 1.0 | `direction_mod::UNKNOWN` |

Formula: `sim = base_cos × direction_mod`. The intervention-overlap term `(0.7 + 0.3 × overlap)` is **disabled** (set to 1.0) — benchmark analysis showed -15.9% correlation. See `compute_asymmetric_similarity` (`asymmetric.rs:383-410`) and `compute_e5_asymmetric_full` (`causal/mod.rs:32`).

`CausalDirection ∈ {Cause, Effect, Unknown}` (`asymmetric.rs:168`) MUST be set on the search options when E5 participates in fusion — otherwise AP-77 forces E5's contribution to the `-1.0` no-signal sentinel.

`infer_direction_from_fingerprint` (`asymmetric.rs:120-149`): compares **component variance** (not L2 norm) of the E5 dual vectors. >5% variance gap classifies as `Cause` or `Effect`, otherwise `Unknown`. Both vectors are unit-normalized by the model so L2 norms are useless.

#### Causal content gate

Post-training E5 thresholds (`asymmetric.rs:58-73` — see also [06 §6](06_search_and_fusion.md)):

| Threshold | Value | Effect |
| --- | --- | --- |
| `CAUSAL_THRESHOLD` | 0.04 | Score ≥ this → `× CAUSAL_BOOST` |
| `NON_CAUSAL_THRESHOLD` | 0.008 | Score ≤ this → `× NON_CAUSAL_DEMOTION` |
| `CAUSAL_BOOST` | 1.10 | Multiplier on causal-query results that pass the gate |
| `NON_CAUSAL_DEMOTION` | 0.85 | Multiplier on causal-query results that fail the gate |

Between the two thresholds, the score passes through unchanged (ambiguous zone). Applied via `apply_causal_gate(score, e5_score, is_causal_query)` from `search_causes` / `search_effects` after the abduction/prediction blend.

### 5.2 E8 graph (`crates/context-graph-core/src/graph/asymmetric.rs`)

Same shape with structural roles (`GraphDirection ∈ {Source, Target, Unknown}`, `asymmetric.rs:59`):

| Direction | Multiplier | Constant |
| --- | --- | --- |
| source → target | 1.2 | `direction_mod::SOURCE_TO_TARGET` |
| target → source | 0.8 | `direction_mod::TARGET_TO_SOURCE` |
| same / unknown | 1.0 | `direction_mod::SAME_DIRECTION` / `UNKNOWN` |

Formula: `sim = base_cos × direction_mod × (0.7 + 0.3 × connectivity_overlap)` (`graph/asymmetric.rs:311-332`). The connectivity-overlap factor is **active** for E8 (unlike E5). `ConnectivityContext` blends entity Jaccard, entity containment, relationship-type Jaccard, module bonus, and depth bonus into [0, 1] (`graph/asymmetric.rs:120-259`).

Public APIs: `compute_e8_asymmetric_fingerprint_similarity(query, doc, query_is_source)` (`asymmetric.rs:428`), `compute_e8_asymmetric_full(query, doc, query_is_source)` (`asymmetric.rs:469`), `adjust_batch_graph_similarities`, `compute_graph_asymmetric_similarity` (re-exported from `graph/mod.rs:21`).

`detect_graph_query_intent` (`graph/asymmetric.rs:520-604`) inspects the natural-language query for source-seeking phrases ("what imports", "callers of", "depends on") vs target-seeking phrases ("dependencies of", "what does X import"); ties default to `Source`.

### 5.3 E10 paraphrase

E10's projections (`paraphrase` vs `context`) follow the same 1.2 / 0.8 pattern when set explicitly. Stored asymmetric pairs use the dual `e10_*_paraphrase` and `e10_*_context` slots on `SemanticFingerprint`.

---

## 6. Structural Causal Model — `CausalGraph` (`causal/scm.rs`)

In-memory directed graph for transient SCM operations, separate from the persisted `causal_relationships` rows. `E5EmbedderActivator` writes into both: it appends to the in-memory graph (for traversal/inference) and (when given a store) into the RocksDB CFs (for durable retrieval).

| Constant | Value | File |
| --- | --- | --- |
| `MAX_CAUSAL_NODES` | 500_000 | `scm.rs:195` |
| `MAX_CAUSAL_EDGES` | 2_000_000 | `scm.rs:198` |

`CausalNode { id, name, domain }` and `CausalEdge { source, target, strength, mechanism, cause_embedding, effect_embedding, is_bidirectional, reverse_embeddings, llm_confidence, mechanism_type, embedding_hint_provenance }` (`scm.rs:30-110`).

Indices kept on every mutation: `effects_index: HashMap<Uuid, Vec<usize>>` (outgoing edge offsets per cause), `causes_index` (incoming offsets per effect). `add_node` / `add_edge` return `false` once caps are hit. `remove_node` retains and rebuilds indices via `rebuild_indices()`; `find_path` is BFS over outgoing edges.

---

## 7. Causal chain reasoning (`causal/chain.rs`)

| Constant | Value | File |
| --- | --- | --- |
| `HOP_ATTENUATION` | 0.9 | `chain.rs:39` |
| `MIN_CHAIN_SCORE` | 0.1 | `chain.rs:42` |
| `MAX_CHAIN_LENGTH` | 10 | `chain.rs:45` |

Re-exports: `build_causal_chain`, `compute_chain_score{,_raw}`, `score_causal_chain{,_attenuated}`, `rank_causes_by_abduction{,_raw}`, `rank_effects_by_prediction{,_raw}` (from `causal/mod.rs:35-40`). Per-hop score multiplied by `HOP_ATTENUATION ^ hop_index`.

`AbductionResult { cause_id, score, raw_similarity }` is the standard return shape used by `search_causes`. `search_effects` uses the symmetric `PredictionResult`.

---

## 8. MCP tool catalogue

Full schemas in [08](08_mcp_tools_reference.md). Function dispatch in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`.

### 8.1 K-NN / typed-edge readers

| Tool | Handler | Behavior |
| --- | --- | --- |
| `get_memory_neighbors` | `graph_link_tools.rs:72` | K-NN neighbors in one embedder space; reads `EdgeRepository.get_embedder_edges`; fails closed if zero edges |
| `get_typed_edges` | `graph_link_tools.rs:343` | Outgoing / incoming / both typed edges; optional `edge_type` filter; for **incoming + type filter** uses full incoming scan + post-filter (BUG-1 fix at `:411-419`); peer UUID returned in `target_id` (BUG-2 fix at `:553-559`) |
| `get_unified_neighbors` | `graph_link_tools.rs:948` | Weighted RRF fusion across the 11 semantic embedders (E2/E3/E4 excluded per AP-60); custom weights / exclusions supported; optional `learning_policy` overrides weights from `LearningEvent` history |
| `traverse_graph` | `graph_link_tools.rs:648` | BFS over typed edges; cumulative weight = `prod(edge.weight)`; default `max_hops=2`, `max_results=20`, `min_weight=0.3` |
| `get_neighbors` | (alias of `get_memory_neighbors`) | — |

### 8.2 E8 graph reasoning (asymmetric)

| Tool | Handler | Behavior |
| --- | --- | --- |
| `search_connections` | `graph_tools.rs:81` | Embed query → search via `graph_reasoning` weight profile (5× over-fetch) → score with `compute_e8_asymmetric_fingerprint_similarity` → apply 1.2 / 0.8 modifier |
| `get_graph_path` | `graph_tools.rs:400` | Multi-hop E8 path; per-hop attenuation `0.9^hop` |

### 8.3 Causal reasoning (E5 asymmetric)

| Tool | Handler | Behavior |
| --- | --- | --- |
| `search_causes` | `causal_tools.rs:59` | Effect→cause abduction. `search_scope ∈ {memories, relationships, all}`: memories path uses `rank_causes_by_abduction` (80 % E1 + 20 % E5 blend) + causal gate; relationships path queries `search_causal_e5(effect_vec, true, k)` over `causal_relationships`. Applies `ABDUCTIVE_DAMPENING = 0.8` |
| `search_effects` | `causal_tools.rs:444` | Cause→effect prediction; mirrors `search_causes`; applies `PREDICTIVE_BOOST = 1.2` |
| `get_causal_chain` | `causal_tools.rs:813` | Iterative SCM-style traversal: each hop runs `search_semantic` with `causal_reasoning` profile, picks best unvisited candidate by asymmetric E5 sim above `min_similarity` (raw, pre-modifier — MCP-M3 fix), ranks with direction modifier |
| `search_causal_relationships` | `causal_relationship_tools.rs:74` | Direct semantic / asymmetric search over `CF_CAUSAL_RELATIONSHIPS`. `direction ∈ {cause, effect, all}` maps to E5-as-cause / E5-as-effect / E1 fallback. `multiEmbedder=true` enables 4-embedder consensus (E1 0.30 + E5 0.35 + E8 0.15 + E11 0.20) |
| `traverse_memory_chain` | `sequence_tools.rs:601` | Walks `SequenceOptions` (E4 ordering) chain with optional semantic filter; not a causal-graph tool but commonly chained with the above |

### 8.4 LLM-driven discovery / validation (require `llm` feature)

| Tool | Handler | Underlying agent | Persists to |
| --- | --- | --- | --- |
| `trigger_causal_discovery` | `causal_discovery_tools.rs:85` | `CausalDiscoveryService` (extract mode) or `GraphDiscoveryService` (pairs mode), both backed by Hermes-2-Pro-Mistral-7B-Q5_K_M GGUF | `causal_relationships` (+ `causal_by_source`), in-memory `CausalGraph`, audit log, optionally typed edges |
| `get_causal_discovery_status` | same file | Reports cycle metrics + cursor | — |
| `discover_graph_relationships` | `graph_tools.rs:679` | `GraphDiscoveryService` (shares LLM with causal-agent via `Arc<CausalDiscoveryLLM>`) | `typed_edges` as `GraphLinkEdgeType::GraphConnected`; audit |
| `validate_graph_link` | `graph_tools.rs:1014` | Single-pair LLM call via `GraphRelationshipLLM::analyze_relationship` | Returns verdict, **does not** persist |
| `validate_low_confidence_edges` | `typed_edges_tools.rs:448` | Iterates typed edges with `weight < ceiling`, runs LLM verdict | `typed_edge_validations` (versioned, see §10) |
| `infer_relationship` | `entity_tools.rs:1158` | TransE on E11 KEPLER (no LLM) | — (returns relation candidates) |
| `validate_knowledge` | `entity_tools.rs:1581` | E11 / TransE plausibility check | — |
| `repair_causal_relationships` | `maintenance_tools.rs:16` | Scans `causal_relationships`, deletes rows that fail JSON deserialization, emits `AuditOperation::CausalRelationshipRepaired` | mutates `causal_relationships` |
| `derive_anomalies_from_edges` | `typed_edges_tools.rs:338` | Pure classifier over `typed_edges`; no LLM | `contrastive_pairs` (+ `by_kind` / `by_anchor`) — see [18](18_contrastive_and_typed_edges.md) |

---

## 9. LLM agents — Hermes-2-Pro-Mistral-7B Q5_K_M GGUF

Both crates depend on `llama-cpp-2` with CUDA, GBNF grammar-constrained JSON, and ChatML prompt format. The LLM is loaded **once** and shared between `CausalDiscoveryService` and `GraphDiscoveryService` via `Arc<CausalDiscoveryLLM>` (`graph-agent/lib.rs:52-66`).

### 9.1 Crates

| Crate | Lib root | Key types | Use |
| --- | --- | --- | --- |
| `context-graph-causal-agent` | `lib.rs` | `CausalDiscoveryLLM`, `CausalDiscoveryService`, `CausalDiscoveryConfig`, `CycleMetrics`, `DiscoveryCursor`, `MemoryScanner`, `ScannerConfig`, `E5EmbedderActivator`, `ActivatorConfig`, `MemoryForAnalysis`, `CausalAnalysisResult`, `CausalCandidate`, `CausalLinkDirection`, `MechanismType`, `SourceSpan`, `SpanType`, `ExtractedCausalRelationship`, `MultiRelationshipResult`, `CausalHint`, `CausalDirectionHint`, `DirectionalEmbeddings`, `CausalMarkers`, `LlmConfig`, `GrammarType` | Pair-classify causal relationships, single-text classify causal nature, multi-relationship extraction; activate E5 embedder for confirmed pairs |
| `context-graph-graph-agent` | `lib.rs` | `GraphRelationshipLLM`, `GraphPromptBuilder`, `GraphDiscoveryService`, `GraphDiscoveryConfig`, `MemoryScanner`, `ScannerConfig`, `E8Activator`, `ActivatorConfig`, `ActivationStats`, `GraphEdge`, `GraphStorage`, `MemoryForGraphAnalysis`, `GraphAnalysisResult`, `GraphLinkDirection`, `RelationshipType`, `RelationshipCategory`, `ContentDomain`, `DomainMarkers`, `GraphMarkers`, `GraphCandidate`, `GraphAgentError`, `E8_DIMENSION = 1024` | Detect 19 structural relationship types across 4 content domains; persist as typed edges |

### 9.2 LLM feature gate

Both crates compile-gate the heavyweight LLM machinery behind a `llm` Cargo feature:

- `causal-agent`: `pub mod llm;`, `pub mod service;` are `#[cfg(feature = "llm")]`. Without the feature, callers get the types and `MemoryScanner` / `E5EmbedderActivator` / `ServiceStatus` / `DiscoveryCycleResult` (`lib.rs:54-77`), but no model loading.
- `graph-agent`: same pattern (`lib.rs:68-96`); additionally `pub mod stubs;` is `#[cfg(all(feature = "test-utils", feature = "llm"))]`.
- `graph-agent/stubs.rs::create_stub_graph_discovery_service()` returns an `Arc<GraphDiscoveryService>` with an unloaded LLM whose paths are intentionally non-existent. Calls to `run_discovery_cycle` / `llm()` return `Err(LlmNotInitialized)` — fail-fast guard, not a fake mock.

### 9.3 GBNF grammars

The LLM's output is grammar-constrained for 100 % valid JSON. Grammar files load from disk; per-type embedded fallbacks exist if missing (CA-H1 fix at `causal-agent/llm/mod.rs:229-249`).

| `GrammarType` | File | Schema |
| --- | --- | --- |
| `Causal` | `models/hermes-2-pro/causal_analysis.gbnf` | `{causal_link: bool, direction: A_causes_B|B_causes_A|bidirectional|none, confidence: 0..1, mechanism: str, mechanism_type: direct|mediated|feedback|temporal}` |
| `Graph` | `models/hermes-2-pro/graph_relationship.gbnf` | `{has_connection: bool, direction: a_to_b|b_to_a|bidirectional|none, relationship_type: <19 enum>|none, category: containment|dependency|reference|implementation|extension|invocation, domain: code|legal|academic|general, confidence: 0..1, description: str}` |
| `Validation` | `models/hermes-2-pro/validation.gbnf` | `{valid: bool, confidence: 0..1, explanation: str}` |
| `SingleText` | embedded `default_single_text_grammar` (`causal-agent/llm/mod.rs:308-325`) | `{is_causal, direction: cause|effect|neutral, confidence, key_phrases: [str], description, asymmetry_strength, cause_entities: [{start,end,label}], effect_entities: [...]}` — used by `analyze_single_text` for E5 hint generation |
| `MultiRelationship` | embedded `default_multi_relationship_grammar` (`causal-agent/llm/mod.rs:332-356`) | `{relationships: [{cause, effect, explanation, confidence, mechanism_type, source_spans: [{start_char,end_char,text_excerpt,span_type: full|cause|effect}]}], has_causal_content: bool}` — used by `extract_causal_relationships` |

### 9.4 ChatML prompt format

Hermes-2-Pro-Mistral uses ChatML (`causal-agent/llm/prompt.rs:60-82`):

```text
<|im_start|>system
{system_prompt}
<|im_end|>
<|im_start|>user
Compare:
A: {truncated_a}
B: {truncated_b}
<|im_end|>
<|im_start|>assistant
```

Three system prompts are kept on `CausalPromptBuilder`: `system_prompt` (pair causal analysis), `single_text_system_prompt` (single-text classification for E5 hints), `multi_relationship_system_prompt` (extract every cause-effect pair from one document).

### 9.5 LLM config and provenance

`LlmConfig` defaults (`causal-agent/llm/mod.rs:83-98`): `model_path = "models/hermes-2-pro/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"`, `context_size = 4096`, `temperature = 0.0` (deterministic), `max_tokens = 512`, `n_gpu_layers = u32::MAX` (full offload), `batch_size = 2048`, `use_few_shot = true`. Each generation also returns a `LLMProvenance` row containing `model_name`, `quantization` (parsed from filename), `temperature`, `max_tokens`, SHA-256 `prompt_template_hash` of the grammar file (or embedded grammar string), `grammar_type`, `tokens_consumed`, `generation_time_ms` (`llm/mod.rs:1242-1326`).

### 9.6 VRAM and dimensions

| Component | Quant | VRAM | Notes |
| --- | --- | --- | --- |
| Hermes-2-Pro-Mistral-7B GGUF | Q5_K_M | ~5 GB | shared by both agents |
| KV cache (4096 ctx) | — | ~1 GB | `estimated_vram_mb = 5000 + (ctx/1024)*256` (`llm/mod.rs:1329-1332`) |
| Total LLM | — | **~6 GB** | within 9 GB envelope |
| E8 embedding | — | shares with E1 (e5-large-v2 1024D) | upgraded from MiniLM 384D |
| `E8_DIMENSION` | 1024 | — | `graph-agent/lib.rs:102` |

### 9.7 Causal pipeline

```
Memories ──► MemoryScanner ──► CausalCandidates ──► CausalDiscoveryLLM ──► E5EmbedderActivator ──► CausalGraph + CausalRelationship
            (E1 clustering   (sorted by score,    (analyze_causal_         (CausalModel.embed_dual    (in-memory SCM +
             + temporal +     deduped against     relationship,             with CausalHintGuidance,   RocksDB persist
             marker score)    analyzed_pairs)     causal_analysis.gbnf)     direction-aware)           via TeleologicalStore)
```

**Stage 1 — `MemoryScanner` (`causal-agent/scanner/mod.rs`):**

| `ScannerConfig` field | Default | Purpose |
| --- | --- | --- |
| `similarity_threshold` | 0.5 | Min E1 cosine to cluster two memories |
| `max_similarity` | 0.9 | Skip near-duplicates |
| `max_candidates` | 100 | Top-N per scan |
| `min_time_gap` | 10 sec | Drop same-event pairs |
| `max_time_gap` | 24 hrs | Causal pairs are usually proximate |
| `same_session_only` | false | Restrict to one session |
| `min_initial_score` | 0.3 | Heuristic-score cutoff |

Scoring (`scanner/mod.rs:251-285`): causal-marker count × 0.15, +0.1 temporal ordering, +0.1 same session, +0.05 each at len>100 and len>300, +0.1 if 0.5 ≤ E1 sim ≤ 0.8. AGT-01 fix: `analyzed_pairs` is only marked AFTER successful LLM analysis (callers invoke `mark_analyzed`); previously a failing pair would be permanently skipped.

**Stage 2 — `CausalDiscoveryLLM` (`causal-agent/llm/mod.rs`):** four entry points sharing one ChatML pipeline:

| Entry point | Grammar | Use |
| --- | --- | --- |
| `analyze_causal_relationship(a, b)` | `Causal` | Pair classification |
| `analyze_batch(pairs)` | `Causal` | Batch wrapper; HIGH-5 fix: returns `Vec<Result<...>>` not silently substituting `default()` |
| `analyze_single_text(content)` | `SingleText` | E5 hint generation for a single memory at store time; CLI-4 fix: returns `Err` when fallback regex extracts zero fields |
| `extract_causal_relationships(content)` | `MultiRelationship` | Extract every distinct cause-effect pair from one document, with character-offset `source_spans`; `fix_source_span_excerpt` realigns LLM-paraphrased excerpts to actual source bytes (`llm/mod.rs:1135-1155`) |

The shared LLM also exposes `generate_text(prompt, grammar_type)` so the graph agent can drive the same llama-cpp instance with the `Graph` or `Validation` grammar (CA-M1 fix at `llm/mod.rs:975-981`).

**Stage 3 — `E5EmbedderActivator` (`causal-agent/activator/mod.rs`):** confirmed pair → E5 dual embedding + edge insertion.

| `ActivatorConfig` field | Default | Purpose |
| --- | --- | --- |
| `min_confidence` | 0.6 | Skip low-confidence relationships |
| `update_existing` | true | Overwrite vs skip pre-existing edges |
| `auto_create_nodes` | true | Auto-add `CausalNode` rows when missing |

Embedding generation (`activator/mod.rs:285-396`): **production fail-fast** — without an injected `Arc<CausalModel>` the activator returns `ConfigError("CausalModel required")`. With a model, calls `embed_dual_guided(content, &CausalHintGuidance)` per side; for `Bidirectional` returns four vectors in a `DirectionalEmbeddings`. `CausalHintGuidance` is built from the LLM's `CausalAnalysisResult` — `asymmetry_strength = confidence.clamp(0.5, 1.0)` for forward/backward, `0.2` for bidirectional, key phrases extracted from `mechanism` text.

`add_graph_edge` (`activator/mod.rs:399-468`): writes to `CausalGraph` based on direction; `Bidirectional` writes both directions with `confidence × 0.8` and forward/backward annotations on the mechanism text. `truncate_name` is UTF-8 safe (AGT-06 fix at `activator/mod.rs:498-516`).

**Stage 4 — `CausalDiscoveryService` (`causal-agent/service/mod.rs`):** orchestrator + persistence + adaptive interval.

| `CausalDiscoveryConfig` field | Default | Purpose |
| --- | --- | --- |
| `interval` | 120 s | Initial sleep between cycles |
| `batch_size` | 100 | Pairs analyzed per cycle |
| `min_confidence` | 0.7 | LLM confidence threshold |
| `skip_analyzed` | true | Use scanner's `analyzed_pairs` cache |
| `enable_background` | **false** (explicit opt-in via env `CAUSAL_DISCOVERY_ENABLED=true`) | Whether `start()` actually spawns a loop |
| `min_interval` | 30 s | Adaptive floor |
| `max_interval` | 600 s | Adaptive ceiling |
| `max_consecutive_errors` | 3 | After this, pause for 1 hour |

`run_discovery_cycle(memories, store)` returns `DiscoveryCycleResult { candidates_found, relationships_confirmed, relationships_rejected, embeddings_generated, edges_created, errors, error_messages, started_at, completed_at, duration }`.

Background loop (`service/mod.rs:471-573`):
- `start(store)` requires `enable_background=true`; loads model, spawns a `tokio::task` (handle stored in `join_handle` per CRIT-05), persists `DiscoveryCursor { last_timestamp, last_fingerprint_id, cycles_completed, total_relationships }` to `CF_SYSTEM` via `store_processing_cursor(CURSOR_KEY = "causal_discovery_cursor", ...)`.
- Each tick: `harvest_memories` validates `embeddings[0].len() == 1024` (LOW-7 fix), runs a discovery cycle, emits `AuditOperation::RelationshipDiscovered`, saves cursor (non-fatal if it fails per ARCH-PROV-01).
- Adaptive interval (`compute_next_interval`): empty harvest → 600 s; harvested but no causal → 300 s; ≤ 5 discoveries → 120 s; > 5 → 30 s. Clamped to `[min_interval, max_interval]`.
- On `consecutive_errors >= max_consecutive_errors` the loop pauses for 3600 s; otherwise exponential backoff `60 × 2^(errors-1)`.
- `stop()` uses CRIT-04 (clone sender out of parking_lot before `.await`) and awaits the JoinHandle with a 10-second timeout; panics are logged.

`ServiceStatus ∈ {Stopped, Starting, Running, Stopping}` (re-exported from `context_graph_core::types::ServiceStatus`).

### 9.8 Graph pipeline

```
Memories ──► MemoryScanner ──► GraphCandidates ──► GraphRelationshipLLM ──► E8Activator ──► GraphStorage
            (E1 clustering    (suspected_types     (analyze_relationship,    (GraphModel.embed_dual    (in-memory; production
             + domain          via GraphMarkers     graph_relationship.gbnf,  separately for source     persists via
             markers + path    + DomainMarkers)    20 types × 4 domains)      and target — GA-H1 fix)   discover_graph_relationships
             similarity)                                                                                tool path → typed_edges)
```

**`MemoryScanner` (`graph-agent/scanner/mod.rs`):** independent of causal-agent. `ScannerConfig` defaults: `similarity_threshold=0.3`, `max_similarity=0.95`, `max_candidates=100`, `min_initial_score=0.2`, `same_session_only=false`, `prioritize_code=true`. Scoring (`:219-269`): graph-marker counts (cap 0.4), +0.15 same session, +0.15 if either looks like code, +0.10 related file paths (shared parent or 2+ common path segments), +0.05/+0.05 at length 100/300, +0.10 if 0.4 ≤ E1 ≤ 0.8. `cosine_similarity` returns **raw [-1, 1]** (intentional — full range is meaningful for clustering vs anti-correlation; thresholds calibrated for this range).

**`GraphRelationshipLLM` (`graph-agent/llm/mod.rs`):** wraps `Arc<CausalDiscoveryLLM>`. `analyze_relationship(a, b)` runs `Graph` grammar then parses `{has_connection, direction, relationship_type, category, domain, confidence, description}` with `LLMProvenance` attached. `validate_relationship(a, b, expected_type)` runs `Validation` grammar and returns `(valid, confidence, explanation)`.

**`E8Activator` (`graph-agent/activator/mod.rs`):**

| `ActivatorConfig` field | Default | Purpose |
| --- | --- | --- |
| `min_confidence` | 0.6 | Threshold for activation |
| `update_existing` | true | Allow re-emitting edges |
| `bidirectional_multiplier` | 0.8 | Confidence × this for both directions of `Bidirectional` |

`activate_relationship` (`:273-332`) runs in three phases under separated locks (parking_lot is !Send so guards drop before await): validate / record, write graph edges synchronously, then **embed source and target SEPARATELY via `GraphModel::embed_dual` per side — GA-H1 fix** (previously concatenated, destroying directional differentiation). Without an injected `Arc<GraphModel>` returns `ConfigError("GraphModel required")`. `GraphStorage` is a simple in-memory `Vec<GraphEdge>` for the in-process pipeline; production persists via `discover_graph_relationships` → `EdgeRepository.store_typed_edge` with `edge_type=GraphConnected`.

**`GraphDiscoveryService` (`graph-agent/service/mod.rs`):**

| `GraphDiscoveryConfig` field | Default | Purpose |
| --- | --- | --- |
| `interval` | 1 hr | (only for the stub loop) |
| `batch_size` | 50 | Candidates per `run_discovery_cycle` |
| `min_confidence` | 0.7 | LLM threshold |
| `skip_analyzed` | true | Use scanner cache |

GA-M1 (`service/mod.rs:175-194`): `is_running()` returns **always `false`** because the background loop is a no-op stub; only the lifecycle bit `is_loop_spawned()` reflects whether `start()` has been called. The actual discovery flow is the on-demand `discover_graph_relationships` MCP tool which calls `run_discovery_cycle` directly. The spawned tick emits `warn!("Graph discovery cycle tick: NOT IMPLEMENTED")` per CRIT-02.

### 9.9 `validate_graph_link` flow

`graph_tools.rs:1014` consumes `(memory_a_id, memory_b_id, expected_type?)`. Path:
1. Fetch both memories' content (fail-closed if either missing).
2. Acquire `GraphDiscoveryService` from handlers; call `service.llm().analyze_relationship(a, b)` (or `validate_relationship` if expected type provided).
3. Return verdict, confidence, direction, relationship_type, category, domain, description, plus `LLMProvenance`. **Does NOT persist** to `typed_edges` — caller must rerun `discover_graph_relationships` to materialise the edge.

---

## 10. LLM edge validation — `LLMEdgeValidation`

Schema (`llm_edge_validation/mod.rs:35`):

| Field | Type | Source |
| --- | --- | --- |
| `validated_at` | `DateTime<Utc>` | wall-clock at LLM call |
| `verdict` | `LLMVerdict ∈ {Valid, Invalid, Reclassify { new_edge_type: u8 }}` | `typed_edge_export::LLMVerdict` |
| `confidence` | f32 in [0, 1] | LLM output |
| `rationale` | String, 1–3 sentences | LLM output (grammar-constrained) |
| `auto_derived_weight` | f32 | snapshot of `TypedEdge.weight` at validation time |
| `validator_version` | String, e.g. `"Hermes-2-Pro-Mistral-7B-Q5_K_M@2026-04-14"` | from `GraphDiscoveryConfig` |
| `prompt_hash` | `[u8; 32]` SHA-256 of the actual prompt | drift detection across template revisions |

Wire format: `[LLM_EDGE_VALIDATION_VERSION = 1][bincode]` (`llm_edge_validation/mod.rs:25`). Deserializer rejects mismatched version with `CoreError::SerializationError` — no migration. Same 33-byte composite key as `typed_edge_records` (`typed_edge_keys.rs:14`).

---

## 11. Provenance for inferred edges

Every LLM-derived edge gains an audit trail:

- **Causal relationships** carry `LLMProvenance { model_name, model_version, quantization, temperature, max_tokens, prompt_template_hash (SHA-256), grammar_type, tokens_consumed, generation_time_ms }` (`types/causal_relationship.rs:54-84`) and `CausalSourceSpan` for character-level highlight (`:138+`).
- **Typed edges** persisted by `discover_graph_relationships` are followed by an `AuditOperation` row pointing at source/target UUIDs (`graph_tools.rs:854+`).
- **Edge validations** carry `validator_version` + `prompt_hash` per row.

---

## 12. Causal relationship row — `CausalRelationship`

`types/causal_relationship.rs:252-364` — every row carries the full multi-embedder fan-out so downstream search modes don't need a re-embed:

| Block | Fields | Dim |
| --- | --- | --- |
| Statements | `cause_statement`, `effect_statement`, `explanation` | text |
| E5 dual (explanation) | `e5_as_cause`, `e5_as_effect` | 768 (`E5_DIM`) |
| E5 dual (source-anchored) | `e5_source_cause`, `e5_source_effect` (anti-clustering) | 768 |
| E1 | `e1_semantic` | 1024 (`E1_DIM`) |
| E8 dual | `e8_graph_source`, `e8_graph_target` | 1024 (`E8_DIM`) |
| E11 | `e11_entity` | 768 (`E11_DIM`) |
| Metadata | `confidence`, `mechanism_type`, `source_content`, `source_fingerprint_id`, `created_at`, `source_spans`, `llm_provenance` | — |

`mechanism_type ∈ { "direct", "mediated", "feedback", "temporal" }`. Source-anchored embeddings exist to prevent E5/E8 explanation clustering — distinct sources sharing prompt structure stay separable.

---

## 13. Edge repository APIs

`crates/context-graph-storage/src/graph_edges/repository.rs` — `EdgeRepository` is a thin RocksDB CRUD layer over the three CFs `EMBEDDER_EDGES`, `TYPED_EDGES`, `TYPED_EDGES_BY_TYPE`. The `new(db)` constructor panics if any required CF is missing (no silent fallback — production ContextGraph either has them or fails to start).

| Method | Behavior |
| --- | --- |
| `store_embedder_edges(embedder_id, source, edges)` | Replaces all K-NN edges for `(embedder, source)`; rejects `embedder_id >= NUM_EMBEDDERS` with `InvalidEmbedderId` |
| `get_embedder_edges(embedder_id, source)` | Returns `Vec<EmbedderEdge>` or empty |
| `delete_embedder_edges(embedder_id, source)` | Removes the row |
| `store_typed_edge(edge)` | Upserts `TypedEdge` and writes the `typed_edges_by_type` secondary-index row in one batch |
| `get_typed_edge(source, target)` | Single-row lookup |
| `get_typed_edges_from(source)` | Prefix scan over `typed_edges` for outgoing edges |
| `get_typed_edges_to(target)` | Honest O(n) scan (HIGH-11 fix — no fake secondary index for target lookups) |
| `get_typed_edges_by_type(edge_type)` | Prefix scan over `typed_edges_by_type` |
| `delete_typed_edge(source, target)` | Removes both primary and secondary rows |
| `iterate_*` | Streaming `DBIteratorWithThreadMode` for export tooling |

All operations follow the FAIL FAST principle — errors surface, never swallowed. Single writer in production is the `BackgroundGraphBuilder` worker; the LLM activators take a separate write path through `discover_graph_relationships`.

---

## 14. Recent K-NN / typed-edge bug fixes

| ID | Severity | File | Fix |
| --- | --- | --- | --- |
| BUG-1 | HIGH | `graph_link_tools.rs:407-475` | Type filter now direction-aware: incoming + type filter falls back to full incoming scan + post-filter (secondary index is keyed by source) |
| BUG-2 | HIGH | `graph_link_tools.rs:553-559` | `target_id` in response is now the **peer** node — `edge.source()` for incoming, `edge.target()` for outgoing (was always self UUID for incoming) |
| BUG-3 | MEDIUM | `graph_edges/repository.rs:230-248`, `:386-403` | Secondary index key extended to 33B `[type][source][target]` to disambiguate same-type edges from same source; legacy 17B keys still readable via `key.len() < 33` branch |
| GA-H1 | HIGH | `graph-agent/activator/mod.rs:321-326` | E8 source / target now embedded SEPARATELY (was concatenated, destroying directional differentiation) |
| GA-M1 / CRIT-02 | MEDIUM | `graph-agent/service/mod.rs:175-194`, `:418-427` | `is_running()` returns `false` (background loop is stub); spawn emits `warn!("NOT IMPLEMENTED")` |
| AGT-01 | — | `causal-agent/scanner/mod.rs:239-243` | `mark_analyzed` only AFTER successful LLM analysis — failing pairs no longer permanently skipped |
| AGT-06 | — | `causal-agent/activator/mod.rs:498-516` | UTF-8 safe `truncate_name` (was byte-slicing, panicked on multi-byte chars) |
| CA-H1 | — | `causal-agent/llm/mod.rs:229-249` | Type-specific embedded grammar fallbacks (was: every grammar type fell back to causal grammar) |
| CA-M1 | — | `causal-agent/llm/mod.rs:975-981` | `generate_text` accepts `GrammarType` argument (was hardcoded to `Causal`) |
| CA-M2 | — | `causal-agent/service/mod.rs:407-410` | Empty `Vec` for E1 placeholder (was 4 KB zero allocation per relationship) |
| CRIT-04 | — | `causal-agent/service/mod.rs:797-804` | `stop()` clones shutdown sender before `.await` (parking_lot guard is !Send) |
| CRIT-05 | — | `causal-agent/service/mod.rs:184-186`, `:809-819`; `graph-agent/service/mod.rs:88-90`, `:452-463` | Background JoinHandle stored, awaited on stop with 10 s timeout, panics logged |
| CLI-4 | — | `causal-agent/llm/mod.rs:638-715` | Single-text fallback returns `Err` when zero fields extracted (was `Ok(default)` indistinguishable from genuine negative) |
| CLI-5 | — | `causal-agent/llm/mod.rs:884-956` | Multi-relationship fallback warns it cannot extract `source_spans` from regex |
| HIGH-5 | — | `causal-agent/llm/mod.rs:491-514` | `analyze_batch` returns `Vec<Result<...>>` not silently substituting `default()` |
| HIGH-11 | — | `graph_edges/repository.rs:431-451` | `get_typed_edges_to` is now an honest O(n) scan (no fake secondary-index for target lookups) |
| LOW-7 | — | `causal-agent/service/mod.rs:684-697` | `harvest_memories` validates `embeddings[0].len() == 1024` (was assumed without bounds check) |

---

## 15. What is NOT covered here

- Embedder weight profiles consumed by `get_unified_neighbors` and the asymmetric search tools — see [06](06_search_and_fusion.md).
- Goal centroids and anomaly derivation as feedback into the teleological store — see [09](09_clustering_topics_and_temporal.md).
- Typed-edge training records and contrastive pair mining schemas — see [18](18_contrastive_and_typed_edges.md).
