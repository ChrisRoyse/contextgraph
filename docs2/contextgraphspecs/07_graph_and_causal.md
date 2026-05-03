# Graph And Causal

**Source files covered:**
- `crates/context-graph-core/src/graph/asymmetric.rs` — E8 asymmetric formula, GraphDirection, ConnectivityContext
- `crates/context-graph-core/src/causal/mod.rs`, `asymmetric.rs`, `chain.rs`, `inference.rs`, `scm.rs` — SCM, E5 asymmetric, transitive chain, abduction
- `crates/context-graph-core/src/graph_linking/{mod,direction,edge_builder,edge_type,embedder_edge,knn_graph,nn_descent,storage_keys,thresholds,typed_edge,weight_projector}.rs`
- `crates/context-graph-core/src/llm_edge_validation/mod.rs` — `LLMEdgeValidation` row schema
- `crates/context-graph-core/src/types/causal_relationship.rs` — `CausalRelationship` (E5 + source-anchored E5 + E1 + E8 + E11)
- `crates/context-graph-storage/src/graph_edges/{repository,builder,serialization,types}.rs` — `EdgeRepository` over RocksDB
- `crates/context-graph-storage/src/teleological/rocksdb_store/{causal_relationships,causal_hnsw_index,anomaly_derivation,llm_validation}.rs`
- `crates/context-graph-causal-agent/src/{lib,llm,scanner,activator,service}.rs` — Hermes-2-Pro causal LLM agent
- `crates/context-graph-graph-agent/src/{lib,llm,scanner,activator,service}.rs` — Hermes-2-Pro graph LLM agent
- `crates/context-graph-mcp/src/handlers/tools/{graph_tools,graph_link_tools,causal_tools,causal_relationship_tools,causal_discovery_tools,maintenance_tools,typed_edges_tools,sequence_tools,entity_tools}.rs`

Cross-references: see [04](04_database_schema.md) for CF layout, [06](06_search_and_fusion.md) for the fusion path that produces edge-derived signals, [08](08_mcp_tools_reference.md) for full tool schemas, [18](18_contrastive_and_typed_edges.md) for typed-edge training records and contrastive pairs.

## 1. Two parallel graphs

The system maintains two superimposed directed graphs over the same memory UUIDs. They are consumed by different tool families.

| Graph | Backing CFs | Built by | Read by |
|---|---|---|---|
| **K-NN + typed-edge graph** | `embedder_edges`, `typed_edges`, `typed_edges_by_type` | Background `EdgeBuilder` (NN-Descent) and LLM agent activators | `get_memory_neighbors`, `get_typed_edges`, `traverse_graph`, `get_unified_neighbors`, `search_connections`, `get_graph_path`, `discover_graph_relationships`, `validate_graph_link`, `validate_low_confidence_edges`, `derive_anomalies_from_edges`, `export_typed_edges_corpus` |
| **Causal SCM** | `causal_relationships`, `causal_by_source` (+ HNSW index file) | `trigger_causal_discovery` (LLM, two modes) | `search_causes`, `search_effects`, `search_causal_relationships`, `get_causal_chain`, `traverse_memory_chain`, `repair_causal_relationships` |

The two graphs share signal: typed edges of kind `CausalChain` are weight-driven by the same E5 dual encoding that anchors `causal_relationships`, and `discover_graph_relationships` persists its LLM verdicts as `GraphConnected` typed edges.

## 2. Stored graph data

| Data | CF | Key | Value | Reference |
|---|---|---|---|---|
| K-NN edges (per embedder, outgoing) | `embedder_edges` | 17B (legacy) `[embedder_id:u8][source:16]` or 33B `[embedder_id:u8][source:16][target:16]` | `Vec<EmbedderEdge>` (similarity, target) | `graph_linking/storage_keys.rs:39`, `graph_edges/repository.rs:81-141` |
| Typed edges (primary) | `typed_edges` | 32B `[source:16][target:16]` | bincode `TypedEdge` | `graph_linking/typed_edge.rs:48`, `graph_edges/repository.rs:217-273` |
| Typed-edge type secondary index | `typed_edges_by_type` | 33B `[edge_type:u8][source:16][target:16]` | target UUID (16B) | `graph_edges/repository.rs:230-248` |
| Typed-edge training rows | `typed_edge_records` | 33B `[source:16][target:16][edge_type:u8]` | bincode `TypedEdgeTrainingRecord` | see [18](18_contrastive_and_typed_edges.md) |
| LLM edge verdicts | `typed_edge_validations` | 33B same composite | `[version:u8][bincode LLMEdgeValidation]` | `llm_edge_validation/mod.rs:35` |
| Causal relationships (primary) | `causal_relationships` | UUID | JSON `CausalRelationship` (E5 dual + source-anchored E5 dual + E1 + E8 dual + E11 + LLM provenance) | `types/causal_relationship.rs:252` |
| Causal-by-source secondary index | `causal_by_source` | source fingerprint UUID | list of causal_relationship UUIDs | `teleological/rocksdb_store/causal_relationships.rs` |
| Causal HNSW index file | `<db>/causal_hnsw/` | (HNSW persisted blob) | dual cause/effect HNSW for `search_causal_e5` | `teleological/rocksdb_store/causal_hnsw_index.rs` |

Legacy 17-byte secondary-index keys are still readable: when the iterator hits a 17-byte row the target UUID is taken from the value, when it hits the 33-byte row the target is taken from `key[17..33]` (`graph_edges/repository.rs:386-403`).

## 3. K-NN edges (`EmbedderEdge`)

| Constant | Value | File |
|---|---|---|
| `KNN_K` | 20 | `graph_linking/mod.rs:53` |
| `NN_DESCENT_ITERATIONS` | 8 | `graph_linking/mod.rs:56` |
| `NN_DESCENT_SAMPLE_RATE` | 0.5 | `graph_linking/mod.rs:59` |
| `MIN_KNN_SIMILARITY` | 0.3 | `graph_linking/mod.rs:62` |
| Number of K-NN graphs (one per embedder) | `NUM_EMBEDDERS` = 14 | `graph_linking/typed_edge.rs:13` |

K-NN edges are built once at startup (and incrementally on store) by `EdgeBuilder` (`graph_linking/edge_builder.rs`), persisted via `EdgeRepository.store_embedder_edges`, and read on demand. NO FALLBACKS: if `EdgeRepository` is missing the tools fail closed (`graph_link_tools.rs:80-89`).

## 4. Typed edges (`TypedEdge`)

`TypedEdge` is a single classified relationship between two memories. It carries the full 14-slot embedder score array, an agreement bitset, and a direction.

| Field | Shape | Source |
|---|---|---|
| source / target | UUID (16B each) | `graph_linking/typed_edge.rs:51-53` |
| edge_type | `GraphLinkEdgeType` (u8 repr) | `edge_type.rs:48` |
| weight | f32 in [0.0, 1.0] | `typed_edge.rs:57` |
| direction | `DirectedRelation` (Symmetric / Forward / Backward) | `direction.rs` |
| embedder_scores | `[f32; 14]` (one slot per embedder) | `typed_edge.rs:61` |
| agreement_count | u8 — popcount of `agreeing_embedders` | `typed_edge.rs:63` |
| agreeing_embedders | u16 bitset (bit i = embedder i) | `typed_edge.rs:65` |

### 4.1 Edge-type table

`GraphLinkEdgeType` is a closed 8-variant enum (`edge_type.rs:48-81`):

| Variant | u8 | Asymmetric? | Primary embedder | Default threshold | Description |
|---|---|---|---|---|---|
| `SemanticSimilar` | 0 | no | E1 (idx 0) | 0.75 | High E1 cosine |
| `CodeRelated` | 1 | no | E7 (idx 6) | 0.70 | Code/implementation |
| `EntityShared` | 2 | no | E11 (idx 10) | 0.65 | Shared KEPLER entities |
| `CausalChain` | 3 | **yes** | E5 (idx 4) | 0.60 | Cause→effect (or backward) |
| `GraphConnected` | 4 | **yes** | E8 (idx 7) | 0.60 | Source→target structural link |
| `ParaphraseAligned` | 5 | no | E10 (idx 9) | 0.70 | Same meaning, different words |
| `KeywordOverlap` | 6 | no | E6 (idx 5) or E13 (idx 12) | 0.50 | Sparse-term overlap |
| `MultiAgreement` | 7 | no | none (≥3 agree) | 0.60 | 3+ embedders cross threshold |

### 4.2 Auto-classification

`TypedEdge::from_scores` (`typed_edge.rs:145-197`):
1. Build agreement bitset by `score >= per_embedder_threshold`. **Temporal embedders E2/E3/E4 (idx 1..=3) are unconditionally excluded per AP-60** (`typed_edge.rs:158-161`).
2. If `agreement_count >= 3` → `MultiAgreement`.
3. Otherwise priority: E7 → E11 → E5 → E8 → E10 → E6/E13 → E1 (`detect_edge_type`, `typed_edge.rs:200-241`).
4. Weight: for `MultiAgreement` = mean of agreeing scores; otherwise = primary embedder's score, clamped to [0, 1] (`typed_edge.rs:244-275`).
5. Direction: forced to `Symmetric` for symmetric types; for asymmetric types `Symmetric` is upgraded to `Forward`.

`TypedEdge::new` (`typed_edge.rs:88-131`) is the validated constructor: rejects weight outside [0, 1], rejects asymmetric type with `Symmetric` direction, rejects `agreement_count != popcount(agreeing_embedders)`.

## 5. Asymmetric similarity

### 5.1 E5 causal (`causal/asymmetric.rs`)

Direction modifiers per AP-77:

| Direction | Multiplier | Constant |
|---|---|---|
| cause → effect | 1.2 | `direction_mod::CAUSE_TO_EFFECT` |
| effect → cause | 0.8 | `direction_mod::EFFECT_TO_CAUSE` |
| same direction | 1.0 | `direction_mod::SAME_DIRECTION` |
| unknown either side | 1.0 | `direction_mod::UNKNOWN` |

Formula: `sim = base_cos × direction_mod` (intervention-overlap term `(0.7 + 0.3 × overlap)` is **disabled**, set to 1.0 — see comment `causal/asymmetric.rs:16-21`).

`CausalDirection::{Cause, Effect, Unknown}` MUST be set on the search options when E5 participates in fusion — otherwise AP-77 forces E5's contribution to 0.0 (note in `causal_tools.rs` and lessons memory).

`infer_direction_from_fingerprint` (`causal/asymmetric.rs:120-149`): compares **component variance** (not L2 norm) of the E5 dual vectors. >5% variance gap classifies as `Cause` or `Effect`, otherwise `Unknown`. Both vectors are unit-normalized by the model so L2 norms are useless.

#### Causal content gate

Post-training E5 thresholds (`causal/asymmetric.rs:58-73`):

| Threshold | Value | Effect |
|---|---|---|
| `CAUSAL_THRESHOLD` | 0.04 | Score ≥ this → `× CAUSAL_BOOST` |
| `NON_CAUSAL_THRESHOLD` | 0.008 | Score ≤ this → `× NON_CAUSAL_DEMOTION` |
| `CAUSAL_BOOST` | 1.10 | Multiplier on causal-query results that pass the gate |
| `NON_CAUSAL_DEMOTION` | 0.85 | Multiplier on causal-query results that fail the gate |

Between the two thresholds the score passes through unchanged (ambiguous zone). Applied via `apply_causal_gate(score, e5_score, is_causal_query)` from `search_causes`/`search_effects` after the abduction/prediction blend.

### 5.2 E8 graph (`graph/asymmetric.rs`)

Same shape but with structural roles:

| Direction | Multiplier |
|---|---|
| source → target | 1.2 |
| target → source | 0.8 |
| same / unknown | 1.0 |

Formula: `sim = base_cos × direction_mod × (0.7 + 0.3 × connectivity_overlap)` (`graph/asymmetric.rs:311-332`). The connectivity-overlap factor is **active** for E8 (unlike E5). `ConnectivityContext` blends entity Jaccard, entity containment, relationship-type Jaccard, module bonus, and depth bonus into [0, 1].

`detect_graph_query_intent` (`graph/asymmetric.rs:520-604`) inspects the natural-language query for source-seeking phrases ("what imports", "callers of", "depends on") vs target-seeking phrases ("dependencies of", "what does X import"); ties default to `Source`.

### 5.3 E10 paraphrase

E10's projections (`paraphrase` vs `context`) follow the same 1.2 / 0.8 pattern when set explicitly. Stored asymmetric pairs use the dual `e10_*_paraphrase` and `e10_*_context` slots on `SemanticFingerprint`.

## 6. Structural Causal Model (`causal/scm.rs`)

`CausalGraph` is an in-memory directed graph for transient SCM operations, separate from the persisted `causal_relationships` rows.

| Constant | Value | File |
|---|---|---|
| `MAX_CAUSAL_NODES` | 500_000 | `scm.rs:195` |
| `MAX_CAUSAL_EDGES` | 2_000_000 | `scm.rs:198` |

`CausalNode { id, name, domain }` and `CausalEdge { source, target, strength, mechanism, cause_embedding, effect_embedding, is_bidirectional, reverse_embeddings, llm_confidence, mechanism_type, embedding_hint_provenance }` (`scm.rs:30-110`).

Indices kept on every mutation: `effects_index: HashMap<Uuid, Vec<usize>>` (outgoing edge offsets per cause), `causes_index` (incoming offsets per effect). `add_node` / `add_edge` return `false` once caps are hit. `remove_node` retains and rebuilds; `find_path` is BFS.

## 7. Causal chain reasoning (`causal/chain.rs`)

| Constant | Value | File |
|---|---|---|
| `HOP_ATTENUATION` | 0.9 | `chain.rs:39` |
| `MIN_CHAIN_SCORE` | 0.1 | `chain.rs:42` |
| `MAX_CHAIN_LENGTH` | 10 | `chain.rs:45` |

`build_causal_chain`, `compute_chain_score{,_raw}`, `score_causal_chain{,_attenuated}`, `rank_causes_by_abduction{,_raw}` (re-exported from `causal/mod.rs:35-40`). Per-hop score multiplied by `HOP_ATTENUATION ^ hop_index`.

`AbductionResult { cause_id, score, raw_similarity }` is the standard return shape used by `search_causes`. `search_effects` uses the symmetric `rank_effects_by_prediction`.

## 8. MCP tool catalogue

Full schemas in [08](08_mcp_tools_reference.md). Function dispatch in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`.

### 8.1 K-NN / typed-edge readers

| Tool | Handler | Behavior |
|---|---|---|
| `get_memory_neighbors` | `graph_link_tools.rs:72` | K-NN neighbors in one embedder space; reads `EdgeRepository.get_embedder_edges`; fails closed if zero edges |
| `get_typed_edges` | `graph_link_tools.rs:343` | Outgoing / incoming / both typed edges; optional `edge_type` filter; for **incoming + type filter** uses full incoming scan + post-filter (BUG-1 fix); peer UUID returned in `target_id` (BUG-2 fix) |
| `get_unified_neighbors` | `graph_link_tools.rs:948` | Weighted RRF fusion across the 11 semantic embedders (E2/E3/E4 excluded per AP-60); custom weights / exclusions supported; optional `learning_policy` overrides weights from `LearningEvent` history |
| `traverse_graph` | `graph_link_tools.rs:648` | BFS over typed edges; cumulative weight = `prod(edge.weight)`; default `max_hops=2`, `max_results=20`, `min_weight=0.3` |
| `get_neighbors` | (alias of `get_memory_neighbors`) | — |

### 8.2 E8 graph reasoning (asymmetric)

| Tool | Handler | Behavior |
|---|---|---|
| `search_connections` | `graph_tools.rs:81` | Embed query → search via `graph_reasoning` weight profile (5× over-fetch) → score with `compute_e8_asymmetric_fingerprint_similarity` → apply 1.2 / 0.8 modifier |
| `get_graph_path` | `graph_tools.rs:400` | Multi-hop E8 path; per-hop attenuation `0.9^hop` |

`SOURCE_DIRECTION_MODIFIER` and `TARGET_DIRECTION_MODIFIER` constants in `graph_dtos.rs` (1.2 / 0.8).

### 8.3 Causal reasoning (E5 asymmetric)

| Tool | Handler | Behavior |
|---|---|---|
| `search_causes` | `causal_tools.rs:59` | Effect→cause abduction. `search_scope ∈ {memories, relationships, all}`: memories path uses `rank_causes_by_abduction` (80 % E1 + 20 % E5 blend) + causal gate; relationships path queries `search_causal_e5(effect_vec, true, k)` over `causal_relationships`. Applies `ABDUCTIVE_DAMPENING = 0.8` |
| `search_effects` | `causal_tools.rs:444` | Cause→effect prediction; mirrors `search_causes`; applies `PREDICTIVE_BOOST = 1.2` |
| `get_causal_chain` | `causal_tools.rs:813` | Iterative SCM-style traversal: each hop runs `search_semantic` with `causal_reasoning` profile, picks best unvisited candidate by asymmetric E5 sim above `min_similarity` (raw, pre-modifier — MCP-M3 fix), ranks with direction modifier |
| `search_causal_relationships` | `causal_relationship_tools.rs:74` | Direct semantic / asymmetric search over `CF_CAUSAL_RELATIONSHIPS`. `direction ∈ {cause, effect, all}` maps to E5-as-cause / E5-as-effect / E1 fallback. `multiEmbedder=true` enables 4-embedder consensus (E1 0.30 + E5 0.35 + E8 0.15 + E11 0.20) |
| `traverse_memory_chain` | `sequence_tools.rs:601` | Walks `SequenceOptions` (E4 ordering) chain with optional semantic filter; not a causal-graph tool but commonly chained with the above |

### 8.4 LLM-driven discovery / validation (require `llm` feature)

| Tool | Handler | Underlying agent | Persists to |
|---|---|---|---|
| `trigger_causal_discovery` | `causal_discovery_tools.rs:85` | `CausalDiscoveryService` (extract mode) or `GraphDiscoveryService` (pairs mode), both backed by Hermes-2-Pro-Mistral-7B-Q5_K_M GGUF (~6 GB VRAM) | `causal_relationships`, audit log, optionally typed edges |
| `get_causal_discovery_status` | same file | Reports cycle metrics + cursor | — |
| `discover_graph_relationships` | `graph_tools.rs:679` | `GraphDiscoveryService` (shares LLM with causal-agent via `Arc<CausalDiscoveryLLM>`) | `typed_edges` as `GraphLinkEdgeType::GraphConnected`; audit |
| `validate_graph_link` | `graph_tools.rs:1014` | Single-pair LLM call | Returns verdict, **does not** persist |
| `validate_low_confidence_edges` | `typed_edges_tools.rs:448` | Iterates typed edges with `weight < ceiling`, runs LLM verdict | `typed_edge_validations` (versioned, see §9) |
| `infer_relationship` | `entity_tools.rs:1158` | TransE on E11 KEPLER (no LLM) | — (returns relation candidates) |
| `validate_knowledge` | `entity_tools.rs:1581` | E11 / TransE plausibility check | — |
| `repair_causal_relationships` | `maintenance_tools.rs:16` | Scans `causal_relationships`, deletes rows that fail JSON deserialization, emits `AuditOperation::CausalRelationshipRepaired` | mutates `causal_relationships` |
| `derive_anomalies_from_edges` | `typed_edges_tools.rs:338` | Pure classifier over `typed_edges`; no LLM | `contrastive_pairs` (+ `by_kind` / `by_anchor`) |

## 9. LLM agents

Both crates depend on `llama-cpp-2` with CUDA, GBNF grammar-constrained JSON, and ChatML prompt format. The LLM is loaded **once** and shared between `CausalDiscoveryService` and `GraphDiscoveryService` via `Arc<CausalDiscoveryLLM>` (`graph-agent/lib.rs:52-66`).

| Crate | Lib root | Key types | Use |
|---|---|---|---|
| `context-graph-causal-agent` | `lib.rs` | `CausalDiscoveryLLM`, `CausalDiscoveryService`, `CausalDiscoveryConfig`, `MemoryScanner`, `E5EmbedderActivator`, `MemoryForAnalysis`, `CausalAnalysisResult`, `CausalCandidate`, `CausalLinkDirection` | Extract or pair-discover causal relationships; activate E5 embedder for confirmed pairs |
| `context-graph-graph-agent` | `lib.rs` | `GraphRelationshipLLM`, `GraphDiscoveryService`, `GraphDiscoveryConfig`, `MemoryScanner`, `E8Activator`, `MemoryForGraphAnalysis`, `RelationshipType`, `RelationshipCategory`, `GraphLinkDirection`, `ContentDomain` | Detect 19 structural relationship types across 4 content domains; persist as typed edges |

### 9.1 Graph-agent relationship taxonomy (`graph-agent/types`)

19 detectable relationship types + `None`, grouped into 6 categories (`graph-agent/lib.rs:31-39`):

| Category | Types |
|---|---|
| Containment | `contains`, `scoped_by` |
| Dependency | `depends_on`, `imports`, `requires` |
| Reference | `references`, `cites`, `interprets`, `distinguishes` |
| Implementation | `implements`, `complies_with`, `fulfills` |
| Extension | `extends`, `modifies`, `supersedes`, `overrules` |
| Invocation | `calls`, `applies`, `used_by` |

`ContentDomain ∈ {Code, Legal, Academic, General}` selects the prompt template.

### 9.2 VRAM and dimensions

| Component | Quant | VRAM | Notes |
|---|---|---|---|
| Hermes-2-Pro-Mistral-7B GGUF | Q5_K_M | ~5 GB | shared by both agents |
| KV cache (4096 ctx) | — | ~1 GB | |
| Total LLM | — | **~6 GB** | within 9 GB envelope |
| E8 embedding | — | shares with E1 (e5-large-v2 1024D) | upgraded from MiniLM 384D, see `graph-agent/lib.rs:14-19` |
| `E8_DIMENSION` | 1024 | — | `graph-agent/lib.rs:102` |

## 10. LLM edge validation (`LLMEdgeValidation`)

Schema (`llm_edge_validation/mod.rs:35`):

| Field | Type | Source |
|---|---|---|
| `validated_at` | `DateTime<Utc>` | wall-clock at LLM call |
| `verdict` | `LLMVerdict ∈ {Valid, Invalid, Reclassify}` | `typed_edge_export::LLMVerdict` |
| `confidence` | f32 in [0, 1] | LLM output |
| `rationale` | String, 1–3 sentences | LLM output (grammar-constrained) |
| `auto_derived_weight` | f32 | snapshot of `TypedEdge.weight` at validation time |
| `validator_version` | String, e.g. `"Hermes-2-Pro-Mistral-7B-Q5_K_M@2026-04-14"` | from `GraphDiscoveryConfig` |
| `prompt_hash` | `[u8; 32]` SHA-256 of the actual prompt | drift detection across template revisions |

Wire format: `[LLM_EDGE_VALIDATION_VERSION = 1][bincode]`. Deserializer rejects mismatched version with `CoreError::SerializationError` — no migration.

## 11. Provenance for inferred edges

Every LLM-derived edge gains an audit trail:

- **Causal relationships** carry `LLMProvenance { model_id, prompt_template_hash, temperature, tokens_consumed, generation_time_ms, grammar_type, ... }` (`types/causal_relationship.rs:54+`) and `CausalSourceSpan` for character-level highlight (`types/causal_relationship.rs:138+`).
- **Typed edges** persisted by `discover_graph_relationships` are followed by an `AuditOperation` row pointing at source/target UUIDs (`graph_tools.rs:854+`).
- **Edge validations** carry the `validator_version` + `prompt_hash` fields above.

## 12. Causal relationship row (`CausalRelationship`)

`types/causal_relationship.rs:252-360` — every row carries the full multi-embedder fan-out so downstream search modes don't need a re-embed:

| Block | Fields | Dim |
|---|---|---|
| Statements | `cause_statement`, `effect_statement`, `explanation` | text |
| E5 dual (explanation) | `e5_as_cause`, `e5_as_effect` | 768 |
| E5 dual (source-anchored) | `e5_source_cause`, `e5_source_effect` (anti-clustering) | 768 |
| E1 | `e1_semantic` | 1024 |
| E8 dual | `e8_graph_source`, `e8_graph_target` | 1024 |
| E11 | `e11_entity` | 768 |
| Metadata | `confidence`, `mechanism_type`, `source_content`, `source_fingerprint_id`, `created_at`, `source_spans` | — |

`mechanism_type ∈ { "direct", "mediated", "feedback", "temporal" }`. Source-anchored embeddings exist to prevent E5/E8 explanation clustering — distinct sources sharing prompt structure stay separable.

## 13. Recent K-NN / typed-edge bug fixes

| ID | Severity | File | Fix |
|---|---|---|---|
| BUG-1 | HIGH | `graph_link_tools.rs:407-475` | Type filter now direction-aware: incoming + type filter falls back to full incoming scan + post-filter (secondary index is keyed by source) |
| BUG-2 | HIGH | `graph_link_tools.rs:553-559` | `target_id` in response is now the **peer** node — `edge.source()` for incoming, `edge.target()` for outgoing (was always self UUID for incoming) |
| BUG-3 | MEDIUM | `graph_edges/repository.rs:230-248` | Secondary index key extended to 33B `[type][source][target]` to disambiguate same-type edges from same source; legacy 17B keys still readable via `key.len() < 33` branch |

## 14. What is NOT covered here

- Embedder weight profiles consumed by `get_unified_neighbors` and the asymmetric search tools — see [06](06_search_and_fusion.md).
- Constellation aggregation over per-goal evidence — see [17](17_constellation_subsystem.md).
- Goal centroids and anomaly derivation as feedback into the teleological store — see [13](13_teleological_subsystem.md).
- Typed-edge training records / contrastive pair mining schemas — see [18](18_contrastive_and_typed_edges.md).
