# Teleological Subsystem

**Source files covered:**
- `crates/context-graph-core/src/teleological/{mod,types,embedder,groups,resolution,vector,meaning,synergy_matrix,matrix_search,comparison_error,comparator,tucker}.rs`
- `crates/context-graph-core/src/teleological/profile/{mod,task_type,fusion_strategy,metrics,teleological}.rs`
- `crates/context-graph-core/src/teleological/services/{mod,correlation_extractor,fusion_engine,group_aggregator,meaning_pipeline,multi_space_retriever,profile_manager,synergy_service,tucker_decomposer}.rs`
- `crates/context-graph-core/src/types/fingerprint/teleological/types.rs` — `TELEOLOGICAL_VECTOR_DIM = 13`, `TeleologicalFingerprint`
- `crates/context-graph-core/src/learner/mod.rs` — learner-state UTL primitives
- `crates/context-graph-storage/src/teleological/mod.rs` — public re-exports
- `crates/context-graph-storage/src/teleological/column_families.rs` — 41 teleological CFs, 14 quantized embedder CFs, 5 code CFs, 2 causal CFs
- `crates/context-graph-storage/src/teleological/schema.rs`, `serialization.rs`, `indexes/`, `quantized/`, `search/`, `tests/`
- `crates/context-graph-storage/src/teleological/rocksdb_store/{mod,store,types,crud,fusion,search,index_ops,inverted_index,causal_hnsw_index,causal_relationships,constellation,content,contrastive,export,file_index,helpers,learner,learner_training,learning,llm_validation,persistence,provenance_storage,source_metadata,trait_impl,typed_edge_export,typed_edge_keys,versioned_bincode,anomaly_derivation,audit_log}.rs`
- `crates/context-graph-mcp/src/handlers/tools/{utl_tools,capability_tools,typed_edges_tools,constellation_tools}.rs`

Cross-references: [04](04_database_schema.md) for the full CF catalog (101 CFs total at DB open), [05](05_embedding_subsystem.md) for the 14-embedder fingerprint shape, [06](06_search_and_fusion.md) for the four search strategies, [09](09_storage_layer.md) for the lower RocksDB layer, [12](12_clustering_and_topics.md) for HDBSCAN topic detection, [17](17_constellation_subsystem.md) for constellation aggregation, [18](18_contrastive_and_typed_edges.md) for anomaly derivation outputs, [20](20_learning_subsystem.md) for learner training datasets.

## 1. What "teleological" means in this codebase

The "teleological subsystem" is the union of two superimposed concerns that share the prefix and storage path:

| Layer | Scope | Persistence root | Reader tools |
|---|---|---|---|
| **Content teleology** | Cross-embedder synergy / fusion / topic profile / meaning extraction over content `TeleologicalFingerprint` rows | `CF_FINGERPRINTS`, `CF_TOPIC_PROFILES`, `CF_TOPIC_PORTFOLIO`, `CF_E1_MATRYOSHKA_128`, `CF_E6_SPARSE_INVERTED`, `CF_E12_LATE_INTERACTION`, `CF_E13_SPLADE_INVERTED` | `search_graph`, `get_topic_portfolio`, `get_topic_stability`, `compile_constellation`, `score_against_constellation` |
| **Learner-state UTL (E15–E21)** | Per-learner observation envelopes, state vectors, goal centroids, retrieval logs, audits | `CF_LEARNER_PROFILE`, `CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_STATE_HISTORY`, `CF_GOAL_CENTROIDS`, `CF_LEARNER_GOAL_STATES`, `CF_LEARNER_CONSTELLATIONS`, … | `register_learner`, `record_session_observation`, `upsert_goal_centroid`, `get_goal_distance`, `count_learner_state`, `get_learner_state`, `compile_learner_constellation`, `resolve_learner_retrieval_policy` |

The **content layer** is described in detail across [05](05_embedding_subsystem.md), [06](06_search_and_fusion.md), [12](12_clustering_and_topics.md), [17](17_constellation_subsystem.md). This document concentrates on the catalog of CFs, the 13-dim teleological signature vector, the search variants, and the learner UTL layer that turns the system from a passive store into a goal-oriented memory.

## 2. The 13-dim teleological signature vector

`crates/context-graph-core/src/types/fingerprint/teleological/types.rs:12`

```text
pub const TELEOLOGICAL_VECTOR_DIM: usize = 13;
```

`TeleologicalFingerprint::teleological_vector() -> [f32; 13]` (`types.rs:105-122`) yields a compact "shape" of the memory across embedding spaces — used as an ultra-fast first-pass candidate filter before the full multi-embedder search.

| Idx | Source | Embedder | Aggregator |
|---|---|---|---|
| 0 | `s.e1_semantic` | E1 Semantic | `l2_norm` |
| 1 | `s.e2_temporal_recent` | E2 Temporal Recent | `l2_norm` |
| 2 | `s.e3_temporal_periodic` | E3 Temporal Periodic | `l2_norm` |
| 3 | `s.e4_temporal_positional` | E4 Temporal Positional | `l2_norm` |
| 4 | `s.e5_active_vector()` | E5 Causal (active side, cause/effect via `e5_active_vector`) | `l2_norm` |
| 5 | `s.e6_sparse` | E6 Sparse Lexical | `sparse_norm` = `indices.len() / 1000` |
| 6 | `s.e7_code` | E7 Code | `l2_norm` |
| 7 | `s.e8_active_vector()` | E8 Graph (active side, source/target) | `l2_norm` |
| 8 | `s.e9_hdc` | E9 HDC | `l2_norm` |
| 9 | `s.e10_active_vector()` | E10 Multimodal (active side, paraphrase/context) | `l2_norm` |
| 10 | `s.e11_entity` | E11 Entity (Kepler) | `l2_norm` |
| 11 | `s.e12_late_interaction` | E12 ColBERT | `token_avg_norm` (mean of per-token L2 norms; 0 when no tokens) |
| 12 | `s.e13_splade` | E13 SPLADE | `sparse_norm` |

The vector is **13-D**, not 14-D, even though `NUM_EMBEDDERS = 14` (E14 BGE-M3 Dense is tracked separately in training records since its dense norm overlaps E1's information content). `sparse_norm` divides by 1000 to keep magnitudes comparable to dense L2 norms — typical SPLADE has 200-1500 active terms, so it yields `0.2-1.5`.

For asymmetric embedders (E5, E8, E10), the **active side** is selected via `e{N}_active_vector()` helpers: cause/source/paraphrase, with legacy fallback when only one side is populated.

## 3. The `TeleologicalFingerprint` record

`crates/context-graph-core/src/types/fingerprint/teleological/types.rs:32-89`. Stored in `CF_FINGERPRINTS` keyed by 16-byte UUID; values are ~46 KB (~63 KB when E6 sparse is present).

| Field | Type | Notes |
|---|---|---|
| `id` | `Uuid` | UUID v4 |
| `semantic` | `SemanticFingerprint` | 14 embedding slots (E1-E14, dual cause/effect for E5, dual source/target for E8, dual paraphrase/context for E10) |
| `content_hash` | `[u8; 32]` | SHA-256 of source content |
| `created_at`, `last_updated` | `DateTime<Utc>` | wall-clock |
| `access_count` | `u64` | bumped on retrieval |
| `importance` | `f32` clamped `[0, 1]` | default `0.5`; consumed by consolidation, `boost_importance`, dream phases |
| `last_accessed_at` | `DateTime<Utc>` | **`#[serde(skip)]`** — bincode is positional, so this lives in memory only and resets to `Utc::now()` on deserialization |
| `e6_sparse` | `Option<SparseVector>` | `#[serde(default)]` — original sparse vector for inverted-index recall and exact term matching; ~235 active terms typical (~1.4 KB) |

Why `serde(skip)`: bincode is positional. Adding a new field to the bincode layout would break deserialization of all existing data. For persistent access tracking, a separate CF would be needed.

## 4. Constants

### 4.1 Content layer

| Constant | Value | File |
|---|---|---|
| `TELEOLOGICAL_VECTOR_DIM` | 13 | `types/fingerprint/teleological/types.rs:12` |
| `NUM_EMBEDDERS` | 14 | `teleological/types.rs:217` |
| `EMBEDDING_DIM` (canonical fusion-tensor depth) | 1024 | `teleological/types.rs:214` |
| `SYNERGY_DIM` | 14 | `teleological/synergy_matrix.rs` (one row/col per embedder) |
| `CROSS_CORRELATION_COUNT` | 91 | = C(14, 2) |
| Tucker default ranks | (4, 4, 128) | `teleological/types.rs:94` (`TuckerCore::DEFAULT_RANKS`) |
| `MAX_WEIGHTED_AGREEMENT` | 9.5 | sum of topic weights, used by topic-confidence reporting |
| `TELEOLOGICAL_VERSION` (serialization) | bumped on layout change | `teleological/serialization.rs` |

### 4.2 Learner layer (`learner/mod.rs`)

| Constant | Value | File |
|---|---|---|
| `LEARNER_RECORD_VERSION` | 1 | `mod.rs:17` |
| `LEARNER_MODALITY_COUNT` | 7 | `mod.rs:20` |
| `LEARNER_BASELINE_SELECTOR_REGULATED` | 1 | `mod.rs:23` |
| `MAX_LEARNER_STATE_VECTOR_VALUES` | 16_384 | `mod.rs:26` |
| `MAX_MODALITY_EMBEDDINGS_PER_OBSERVATION` | 16 | `mod.rs:29` |
| `MAX_RECENT_SCORE_WINDOW` | 1024 | `mod.rs:32` |
| `MAX_LEARNER_LABEL_CHARS` | 256 | `mod.rs:35` |

## 5. Column-family catalog

`crates/context-graph-storage/src/teleological/column_families.rs` declares **four families of CFs** that all open behind the same `RocksDbTeleologicalStore`:

| Family | Count | Constant | Source |
|---|---|---|---|
| Teleological | 41 | `TELEOLOGICAL_CF_COUNT` | `column_families.rs:657` |
| Quantized embedders | 14 | `QUANTIZED_EMBEDDER_CF_COUNT` | `column_families.rs:746` |
| Code | 5 | `CODE_CF_COUNT` | `column_families.rs:1887` |
| Causal | 2 | `CAUSAL_CF_COUNT` | `column_families.rs:2057` |

`get_all_teleological_cf_descriptors(cache)` returns `TELEOLOGICAL_CF_COUNT + QUANTIZED_EMBEDDER_CF_COUNT = 55` descriptors (`column_families.rs:1798`). Code and causal CFs are added separately via `get_code_cf_descriptors` and `get_causal_cf_descriptors`. The default DB has 101 CFs total at open (per the doc-comment on `RocksDbTeleologicalStore::open` in `rocksdb_store/store.rs:137`).

### 5.1 The 41 teleological CFs (`TELEOLOGICAL_CFS` array, `column_families.rs:612-654`)

| # | CF name | Purpose | Key | Value | Source |
|---|---|---|---|---|---|
| 1 | `fingerprints` | Primary `TeleologicalFingerprint` rows | UUID (16) | bincode TF (~46-63 KB) | `:70` |
| 2 | `topic_profiles` | 13D topic-profile vectors per memory | UUID | 13 × f32 = 52 B | `:76` |
| 3 | `e13_splade_inverted` | SPLADE inverted index for Stage 1 sparse recall | term_id (u16, 2 B) | `Vec<Uuid>` | `:84` |
| 4 | `e6_sparse_inverted` | E6 sparse inverted index (e6upgrade.md) | term_id (u16) | `Vec<Uuid>` | `:97` |
| 5 | `e1_matryoshka_128` | E1 truncated 128D vectors | UUID | 128 × f32 = 512 B | `:106` |
| 6 | `content` | Original UTF-8 text | UUID | text up to 1 MB, LZ4 | `:122` |
| 7 | `source_metadata` | Provenance (file paths, etc.) | UUID | JSON, ~100-500 B | `:136` |
| 8 | `file_index` | file_path → `FileIndexEntry { Vec<Uuid> }` | UTF-8 path | bincode | `:150` |
| 9 | `topic_portfolio` | Persisted `PersistedTopicPortfolio` for session continuity | session_id or `__latest__` | JSON | `:164` |
| 10 | `e12_late_interaction` | E12 ColBERT token embeddings | UUID | `Vec<Vec<f32>>` bincode (typical 10-25 KB, max 256 KB) | `:190` |
| 11 | `entity_provenance` | Reserved — not yet populated | `{canonical_id}_{uuid}` | bincode `EntityProvenance` | `:213` |
| 12 | `audit_log` | Append-only `AuditRecord` trail | `[ts_nanos_BE:8][uuid:16]` (24 B) | JSON | `:235` |
| 13 | `audit_by_target` | Audit secondary index by target memory | `[uuid:16][ts_nanos_BE:8]` | primary key bytes | `:250` |
| 14 | `merge_history` | Permanent merge lineage (Phase 4) | `[merged_uuid:16][ts_nanos_BE:8]` | JSON `MergeRecord` | `:272` |
| 15 | `importance_history` | Permanent importance change history | `[uuid:16][ts_nanos_BE:8]` | JSON | `:286` |
| 16 | `tool_call_index` | Reserved — `tool_use_id` → `Vec<Uuid>` | tool_use_id | bincode | `:306` |
| 17 | `consolidation_recommendations` | Reserved | — | — | `:325` |
| 18 | `embedding_registry` | Embedder registry / version tracking | — | — | `:344` |
| 19 | `custom_weight_profiles` | Weight-profile JSON storage | profile name | JSON | `:362` |
| 20 | `hnsw_graphs` | Persisted usearch HNSW index blobs | `graph:{embedder}` / `meta:{embedder}` | bytes / JSON | `:377` |
| 21 | `training_records` | Training data export rows | UUID | `[ver][bincode]` | `:399` |
| 22 | `learning_events` | Learning event log | UUID | bincode | `:416` |
| 23 | `learner_profile` | `LearnerProfile` records | learner UUID (16) | bincode | `:422` |
| 24 | `learner_constellations` | Per-learner regulated-state baselines | `[learner:16][selector_kind:u8]` (17 B) | bincode | `:428`; cross-ref [17](17_constellation_subsystem.md) |
| 25 | `fingerprints_learner` | `LearnerFingerprint` per session | `[learner:16][session_ts_BE:8]` | bincode | `:434` |
| 26 | `learner_m_per_trace` | Per-trace M(t) records | `[learner:16][trace:16]` | bincode | `:440` |
| 27 | `learner_state_history` | `LearnerStateVector` history | `[learner:16][session_ts_BE:8]` | bincode | `:446` |
| 28 | `learner_goal_states` | `LearnerGoalState` per (learner, skill) | `[learner:16][skill:16]` | bincode | `:452` |
| 29 | `learner_retrieval_log` | Retrieval-practice outcomes | `[learner:16][trace:16][ts_BE:8]` | bincode | `:458` |
| 30 | `learner_k_sleep` | Sleep-pressure scalar history | `[learner:16][session_ts_BE:8]` | bincode | `:464` |
| 31 | `goal_centroids` | Per-(skill, modality) expert centroids | `[skill:16][modality:u8]` | bincode | `:470` |
| 32 | `learner_delta_log` | Per-session ΔS/ΔC/ΔE | `[learner:16][session_ts_BE:8]` | bincode | `:476` |
| 33 | `learner_audit` | Per-learner audit trail | `[learner:16][ts_BE:8][audit:16]` | bincode | `:482` |
| 34 | `learner_training_datasets` | Compiled training matrices per learner | dataset UUID | bincode | `:492` |
| 35 | `constellations` | `Constellation` records (Phase 2) | UUID (16) | `[CONSTELLATION_VERSION:u8][bincode]` | `:511`; see [17](17_constellation_subsystem.md) |
| 36 | `constellation_by_selector` | Selector → constellation UUID secondary index | `[kind:u8][sha256(canonical)[..16]]` (17 B) | UUID (16) | `:529` |
| 37 | `contrastive_pairs` | Mined `(anchor, negative)` rows | `[anchor:16][neg:16]` (32 B) | bincode | `:546` |
| 38 | `contrastive_by_kind` | Anomaly-kind index | `[kind:u8][anchor:16][neg:16]` (33 B) | empty marker | `:559` |
| 39 | `contrastive_by_anchor` | Anchor → `Vec<Uuid>` index | anchor UUID (16) | append-bincode | `:572` |
| 40 | `typed_edge_records` | Per-typed-edge training records (F1) | `[source:16][target:16][edge_type:u8]` (33 B) | `[ver][bincode TypedEdgeTrainingRecord]` | `:592` |
| 41 | `typed_edge_validations` | LLM-validated typed-edge verdicts (F4) | same 33-byte composite | `[ver][bincode LLMEdgeValidation]` | `:609` |

### 5.2 The 14 quantized embedder CFs (`column_families.rs:728-743`)

Per-embedder per-UUID quantized vector storage. Reserved for forward compatibility — not yet wired into the production write path. Each value is a bincode-encoded `QuantizedEmbedding` from `context-graph-embeddings`.

| CF | ModelId | Embedder | Quantization | Approx size |
|---|---|---|---|---|
| `emb_0` | 0 | E1_Semantic (1024D) | PQ-8 | ~8 B |
| `emb_1` | 1 | E2_TemporalRecent (512D) | Float8E4M3 | 512 B |
| `emb_2` | 2 | E3_TemporalPeriodic (512D) | Float8E4M3 | 512 B |
| `emb_3` | 3 | E4_TemporalPositional (512D) | Float8E4M3 | 512 B |
| `emb_4` | 4 | E5_Causal (768D) | PQ-8 | ~8 B |
| `emb_5` | 5 | E6_Sparse | SparseNative | variable (~2 KB) |
| `emb_6` | 6 | E7_Code (1536D) | PQ-8 | ~8 B |
| `emb_7` | 7 | E8_Graph (1024D) | Float8E4M3 | 1024 B |
| `emb_8` | 8 | E9_HDC (1024D) | Binary | ~1250 B |
| `emb_9` | 9 | E10_Multimodal (768D) | PQ-8 | ~8 B |
| `emb_10` | 10 | E11_Entity / Kepler (768D) | PQ-8 | ~768 B |
| `emb_11` | 11 | E12_LateInteraction | TokenPruning | variable (~2 KB) |
| `emb_12` | 12 | E13_SPLADE | SparseNative | variable (~2 KB) |
| `emb_13` | 13 | E14_BgeM3Dense (1024D) | PQ-8 | ~8 B |

### 5.3 PQ-8 / Float8 metadata

`QuantizedEmbedding` (`context-graph-embeddings/src/quantization/types.rs:124-201`) carries a `QuantizationMetadata` enum that records the parameters needed to dequantize:

| Variant | Fields | Reconstruction |
|---|---|---|
| `PQ8 { codebook_id: u32, num_subvectors: u8 = 8 }` | look up trained centroids from `PQ8Codebook { embedding_dim, num_subvectors=8, num_centroids=256, centroids[8][256][D/8] }` | reassemble via centroid lookup |
| `Float8 { scale: f32, bias: f32 }` | per-vector min/max captured at quantize time and stored as `(scale, bias)` | `original = quantized * scale + bias` |
| `Binary { threshold: f32 }` | bit = 1 iff value ≥ threshold | sign-restore |
| `Sparse { vocab_size, nnz }` | indices+values stored as bytes | direct reconstruction |
| `TokenPruning { original_tokens, kept_tokens, threshold }` | top-50% by importance | partial reconstruction |

Critical lesson from prior audits: **PQ-8 must store actual min/max** (lesson in `tasks/lessons.md`: "hardcoding `[-1, 1]` corrupts dequantization when data has different range"); for asymmetric embedders both vectors must be quantized independently — cloning cause→effect destroys the dual-vector signal entirely.

### 5.4 The 5 code CFs (`CODE_CFS`, `column_families.rs:1878-1884`)

| CF | Purpose | Key | Value |
|---|---|---|---|
| `code_entities` | AST-extracted code entities (functions, structs, traits, …) | UUID | bincode `CodeEntity` (~500-5000 B) |
| `code_e7_embeddings` | E7 (Qodo-Embed-1.5B) embeddings — code's primary embedder, **not E1** | UUID | `Vec<f32>` 1536D = 6144 B |
| `code_file_index` | file_path → entity UUIDs | UTF-8 path | bincode `CodeFileIndexEntry` |
| `code_name_index` | entity name → `Vec<Uuid>` | UTF-8 name | bincode |
| `code_signature_index` | signature SHA256 → `Vec<Uuid>` | 32-byte hash | bincode |

### 5.5 The 2 causal CFs (`CAUSAL_CFS`, `column_families.rs:2054`)

| CF | Purpose | Key | Value |
|---|---|---|---|
| `causal_relationships` | LLM-generated causal description rows with E1 embedding + provenance | UUID | bincode `CausalRelationship` (~5-15 KB) |
| `causal_by_source` | source fingerprint → `Vec<Uuid>` of causal relationships | source UUID | bincode `Vec<Uuid>` |

## 6. The 15 HNSW indexes

`teleological/indexes/` exposes one `HnswConfig` per embedder index. The set contains 15 entries — not 11 or 12 — because E1 is mirrored at full and Matryoshka-128 dimensions, E5 is asymmetric (separate cause/effect HNSWs), and E10 is asymmetric (paraphrase/context):

E1, E1Matryoshka128, E2, E3, E4, E5 + E5CausalCause + E5CausalEffect, E7, E8, E9, E10 + E10MultimodalParaphrase + E10MultimodalContext, E11.

E6 (sparse), E12 (token MaxSim), E13 (sparse) are NOT HNSW-indexed — they use inverted-index / late-interaction primitives instead. See `teleological/search/mod.rs:14-32` for the full enumeration.

## 7. Search variants in the teleological store

`teleological/search/` declares the four canonical strategies and their configuration types. They are wired together by the MCP `search_graph` handler and consumed by the fusion engine in `services/fusion_engine.rs`.

| Module | Type | Purpose |
|---|---|---|
| `search/single/` | `SingleEmbedderSearch`, `SingleEmbedderSearchConfig`, `SingleEmbedderSearchResults`, `EmbedderSearchHit` | Stage 2/3 of the pipeline; HNSW lookup for one embedder |
| `search/multi/` | `MultiEmbedderSearch`, `MultiSearchBuilder`, `NormalizationStrategy`, `AggregationStrategy`, `AggregatedHit`, `PerEmbedderResults` | Parallel search across all 11 HNSW embedders + score normalization + aggregation |
| `search/pipeline/` | `RetrievalPipeline`, `PipelineBuilder`, `PipelineStage`, `PipelineConfig`, `PipelineCandidate`, `StageConfig`, `StageResult`, `TokenStorage`, `SpladeIndex`, in-memory test impls | The 5-stage `pipeline` strategy: E13 SPLADE → E1 dense → E12 MaxSim |
| `search/matrix/` | `MatrixStrategySearch`, `MatrixSearchBuilder`, `SearchMatrix`, `MatrixSearchResults`, `MatrixAnalysis`, `CorrelationAnalysis`, `CorrelationPattern` | 14×14 cross-embedder correlation search and analysis |
| `search/maxsim.rs` | `MaxSimScorer`, `compute_maxsim_direct`, `cosine_similarity_128d`, `E12_TOKEN_DIM` | E12 ColBERT MaxSim reranker |
| `search/temporal_boost.rs` | temporal boost helpers | E2/E3/E4 score uplift |
| `search/token_storage.rs` | `RocksDbTokenStorage` (CF_E12_LATE_INTERACTION reader), `MAX_TOKENS_PER_MEMORY` | Production E12 token store |

Strategies tie back to fusion through the `SearchStrategy` enum in `teleological/matrix_search.rs` and the `weighted_rrf_fusion_with_scores` helper in `rocksdb_store/fusion.rs` (`RRF_K` constant).

Per the embedding-subsystem and search docs:

- `e1_only` — single E1 HNSW (`SingleEmbedderSearch`).
- `multi_space` (default) — `MultiEmbedderSearch` over the 11 HNSW embedders, with E2/E3/E4 weight-gated and E11 disabled by default. See `MEMORY.md` "MultiSpace active" note.
- `pipeline` — `RetrievalPipeline` running E13 → E1 → E12 stages.
- `embedder_first` — embedder-prioritized routing (`embedder_first` matrix path).

`MaxSim cosine` returns raw `[-1, 1]` — callers that mix into fusion must apply `(raw + 1.0) / 2.0` (SRC-3). See [11](11_cuda_acceleration.md) §9.1 for the same invariant on GPU-batched cosines.

## 8. Audit log, merge history, importance history, stale-lock detection

### 8.1 Audit log (Phase 1.1 provenance)

`audit_log` and `audit_by_target` are **append-only** by design (no update or delete) — `column_families.rs:222-250`. Primary key is `[ts_nanos_BE:8][uuid:16]`, ensuring chronological iteration. Secondary index keyed by `[target_uuid:16][ts_nanos_BE:8]` enables "show me all operations on memory X". Records (`AuditRecord`) are JSON. The whole subsystem is implemented in `rocksdb_store/audit_log.rs`.

### 8.2 Merge history (Phase 4)

`merge_history` is **permanent** — never expires, never deleted (`column_families.rs:259-272`). Stores `MergeRecord` JSON entries keyed by `[merged_uuid:16][ts_nanos_BE:8]`. Reversal records (separate, 30-day TTL) live elsewhere; merge history stays for full lineage tracking forever.

### 8.3 Importance history (Phase 4)

`importance_history` is also **permanent** (`column_families.rs:274-286`). Every change to `TeleologicalFingerprint.importance` writes an `ImportanceChangeRecord` JSON row keyed by `[memory_uuid:16][ts_nanos_BE:8]`. Important lesson: **importance clamping must happen at handler level** — the constructor clamp is bypassed by serde deserialization, so the MCP handler `boost_importance` enforces `[0, 1]` before write.

Importance must also persist soft-delete state to RocksDB; the in-memory HashMap is cleared on restart. Use a `CF_SYSTEM` prefix key for soft-delete flags.

### 8.4 Stale-lock detection (`rocksdb_store/store.rs`)

`open_with_config` (`store.rs:145-198`) inspects RocksDB `LOCK` files for diagnostic purposes only — it **never removes them**. RocksDB owns its own lock protocol and will fail the open if another process owns the lock.

`inspect_rocksdb_lock` (`store.rs:474-559`) probes the LOCK with `fcntl` to determine whether the lock is currently held:

- LOCK file missing → "database not previously opened"; proceed.
- LOCK exists and `fcntl(F_SETLK, F_UNLCK)` reports it is held by another PID → log a warning that RocksDB open will fail if the owner is still active; leave the LOCK in place.
- LOCK exists but is not held → log and leave in place for RocksDB.
- Cannot open the LOCK / unsupported OS → log and leave in place.

After the open succeeds, the function asserts that all `DJ_CF_NAMES` (DynamicJEPA CFs) are present (`store.rs:200-210`); a missing CF is a hard `TeleologicalStoreError::ColumnFamilyNotFound`. The store also enables `paranoid_checks` (`set_paranoid_checks(true)`) at ~1-3% perf cost to catch bit rot and SST corruption on open.

The MCP daemon stale-lock workflow (separate from store-level inspection) uses PID + port liveness checks; see lessons in `MEMORY.md`:
- "Alive process ≠ serving process" — `kill(pid, 0) == 0` only proves a process-table entry exists, not that it's serving.
- "`kill(pid,0)` returns 0 for zombies" — after `SIGKILL`, the process may be a zombie (flock released) while `kill` still reports "alive". Check `/proc/pid/status` for `State:\tZ` to detect this.

## 9. Learner-state UTL layer (E15-E21)

`crates/context-graph-core/src/learner/mod.rs` defines the per-learner observation envelopes, state vectors, goal centroids, retrieval logs, and audits.

### 9.1 Modalities

`LearnerModality` (`learner/mod.rs:39-80`) maps to embedder slots E15-E21 in the planning sense (no on-disk per-embedder CFs yet):

| Variant | Slot | String form |
|---|---|---|
| `AffectSpeech` | E15 | `affect_speech` |
| `AffectFace` | E16 | `affect_face` |
| `AffectText` | E17 | `affect_text` |
| `Ppg` | E18 | `ppg` |
| `Eda` | E19 | `eda` |
| `Eeg` | E20 | `eeg` |
| `EegArtifactRobust` | (E20-prime) | `eeg_artifact_robust` |
| `SelfReport` | external | `self_report` (rejected as a centroid modality, see §11) |

`LearnerModality::parse(&str)` is the canonical string→enum entry-point used by every UTL tool.

### 9.2 Core learner records

All persisted via `RocksDbTeleologicalStore` (`storage/teleological/rocksdb_store/learner.rs`). Every record has a `validate()` returning `CoreError::ValidationError`.

| Type | Fields (selected) | CF | Key | File |
|---|---|---|---|---|
| `ObservationEnvelope` | `observation_id`, `learner_id`, `session_ts`, `modality`, `consent_state`, `raw_sha256`, `preprocessing_version`, `embedder_version`, `threshold_version`, `parent_observation_ids`, `created_at` | (in-fingerprint) | — | `learner/mod.rs:84-140` |
| `ComputationEnvelope` | `computation_id`, `learner_id`, `session_ts`, `parent_observation_ids`, `threshold_version`, `output_sha256`, `created_at` | (in-fingerprint) | — | `learner/mod.rs:144-181` |
| `LearnerProfile` | `learner_id`, `handle`, `consent_state`, `modalities_enabled`, `calibration_session_ts`, `created_at` | `learner_profile` | learner_id | `learner/mod.rs:185-225` |
| `ModalityEmbedding` | `modality`, `vector`, `scalar`, `source_observation_id` | (in-fingerprint) | — | `learner/mod.rs:229-244` |
| `LearnerStateComponents` | `plasticity_window`, `hrv_coherence`, `valence`, `arousal`, `stress_floor`, `k_sleep` (range [0, 5]) | (embedded) | — | `learner/mod.rs:248-272` |
| `LearnerStateVector` | `learner_id`, `session_ts`, `values`, `components`, `context: BTreeMap<String, String>` | `learner_state_history` | `[learner_id:16][session_ts:u64 BE]` | `learner/mod.rs:276-294` |
| `LearnerFingerprint` | `learner_id`, `session_ts`, `observation_envelopes`, `modality_embeddings`, `state_vector` | `fingerprints_learner` | same composite | `learner/mod.rs:298-343` |
| `LearnerDeltaLog` | per-session ΔS / ΔC / ΔE breakdown rows | `learner_delta_log` | `[learner_id:16][session_ts:u64 BE]` | `learner/mod.rs` (DeltaLog block) |
| `LearnerMTrace` | per-trace M(L) memory trace | `learner_m_per_trace` | `[learner_id:16][trace_id:16]` | `learner/mod.rs` |
| `LearnerKSleep` | recovery / sleep-pressure scalar history | `learner_k_sleep` | `[learner_id:16][session_ts:u64 BE]` | `learner/mod.rs` |
| `LearnerRetrievalLog` | per-trace retrieval outcomes (correct, score, state vector) | `learner_retrieval_log` | `[learner_id:16][trace_id:16][ts:u64 BE]` | `learner/mod.rs` |
| `LearnerGoalState` | `learner_id`, `skill_id`, `state_vector` | `learner_goal_states` | `[learner_id:16][skill_id:16]` | `learner/mod.rs:603-615` |
| `GoalCentroid` | `skill_id`, `modality`, `vector` (per-skill expert-answer centroid) | `goal_centroids` | `[skill_id:16][modality:u8]` | `learner/mod.rs:712-724` |
| `LearnerModalityCentroid` | `modality`, `vector`, `scalar_mean`, `sample_count` | (in-constellation) | — | `learner/mod.rs:618-640` |
| `LearnerConstellation` | regulated-state baseline; `selector_kind`, modality centroids, `state_centroid`, `session_ts_start/end` | `learner_constellations` | `[learner_id:16][selector_kind:u8]` | `learner/mod.rs:643-710`; cross-ref [17](17_constellation_subsystem.md) |
| `LearnerAuditEntry` | `audit_id`, `learner_id`, `ts`, `action`, `target_cf`, `result_sha256`, `parent_audit_id` | `learner_audit` | `[learner_id:16][ts:u64 BE][audit_id:16]` | `learner/mod.rs:727-745` |
| `LearnerTrainingDataset` | row-major f32 matrix exported per learner | `learner_training_datasets` | dataset_id | see [20](20_learning_subsystem.md) |

All learner CF row keys preserve `[learner_id]` as a 16-byte prefix so prefix scans yield single-learner views. `LearnerStateVector::values` is the freeform learner-state vector (capacity capped at 16 384 f32) used for goal-distance computation.

## 10. Goal centroids and goal-distance

`GoalCentroid` (`learner/mod.rs:714-724`) is a per-skill expert-answer centroid stored in `CF_GOAL_CENTROIDS` keyed by `[skill_id:16][modality:u8]`. It is **per-modality** so the same skill can hold different centroids for, e.g., affect-text vs. EEG.

### 10.1 `upsert_goal_centroid`

Handler: `utl_tools.rs:1191`. Args (`GoalCentroidArgs`, `utl_tools.rs:232`):

| Field | JSON key | Type | Required |
|---|---|---|---|
| skill_id | `skillId` | UUID | yes |
| modality | `modality` | string (one of E15-E21 strings) | yes |
| vector | `vector` | f32[] | yes |

Validation: `GoalCentroid::validate` runs `validate_vector` (non-empty, finite, length-bounded). Persists via `RocksDbTeleologicalStore::store_goal_centroid` (`learner.rs:236`). `count_goal_centroids` (`learner.rs:299`) is exposed via `get_capability_matrix.runtime.learnerState.goalCentroids`.

### 10.2 `get_goal_distance`

Handler: `utl_tools.rs:1233`. Args (`GoalDistanceArgs`, `utl_tools.rs:240`):

| Field | JSON key | Type | Required | Default |
|---|---|---|---|---|
| learner_id | `learnerId` | UUID | yes | — |
| skill_id | `skillId` | UUID | yes | — |
| session_ts | `sessionTs` | u64 | yes | — |
| modality | `modality` | string | yes | — |
| persist_goal_state | `persistGoalState` | bool | no | `false` |

Algorithm (`utl_tools.rs:1248-1322`):
1. Load `GoalCentroid` for `(skill_id, modality)` → 404 if missing.
2. Load `LearnerFingerprint` for `(learner_id, session_ts)` → 404 if missing.
3. Locate the `ModalityEmbedding` matching `modality` inside the fingerprint → error if absent.
4. Validate dimensions match.
5. Compute three metrics:

   | Metric | Formula | Source |
   |---|---|---|
   | `cosine_similarity` | `dot / (|a|·|b|)` | `cosine_similarity(...)` helper |
   | `cosine_distance` | `1 - cosine_similarity` | derived |
   | `euclidean_distance` | `sqrt(Σ(a_i - b_i)²)` | helper |
   | `transfer_t_lambda_1` | UTL transfer T at λ=1.0 | `compute_transfer_t(emb, centroid, 1.0)` |

6. If `persistGoalState=true`: write `LearnerGoalState { learner_id, skill_id, state_vector: fingerprint.state_vector }` to `CF_LEARNER_GOAL_STATES` and append a `LearnerAuditEntry` with `action="get_goal_distance"`, `target_cf="learner_goal_states"` (`utl_tools.rs:1286-1308`).

The `source_of_truth` field on the response is the canonical list `["goal_centroids", "fingerprints_learner", "learner_goal_states", "learner_audit"]`.

## 11. UTL tool catalogue (17 tools)

`crates/context-graph-mcp/src/tools/definitions/utl.rs` registers exactly 17 learner-state tool definitions. Names below are the canonical MCP names; handlers live in `handlers/tools/utl_tools.rs`.

| Tool | Args (camelCase) | Persists to / reads | Notes |
|---|---|---|---|
| `register_learner` | `learnerId?`, `handle`, `consentState`, `modalitiesEnabled`, `calibrationSessionTs?` | `learner_profile`, `learner_audit` | rejects unknown modality strings; emits audit row |
| `record_session_observation` | `learnerId`, `sessionTs`, `signals?` or `components{plasticityWindow, hrvCoherence, valence, arousal, stressFloor, kSleep}`, `modalityEmbeddings`, `mode`, `consentState`, … | `fingerprints_learner`, `learner_state_history`, `learner_audit` | refuses writes when `consentState=revoked`; supports `mode ∈ {signals, components, both}` |
| `compute_delta_s` | predicted/actual/simulated text, `explorationRate`, `gamma?` | (pure / optional persist via `record_*`) | bag-of-words cosine proxy for ΔS |
| `compute_delta_c` | learner_id, session window | `learner_delta_log` | ΔC retention/recall metric |
| `compute_delta_e` | learner_id, session window | `learner_delta_log` | ΔE engagement metric |
| `compute_L` | learner_id, trace_id | (pure) | M(L) computation result |
| `get_learner_M` | learner_id, trace_id | reads `learner_m_per_trace` | M trace lookup |
| `next_review_for_trace` | learner_id, trace_id | reads `learner_m_per_trace` + recent retrievals | spaced-repetition next-review timestamp |
| `list_learner_embedders` | none | runtime model registry | lists E15-E21 specs from `learner_embedder_specs` |
| `preflight_learner_assets` | `modelsRoot`, `calibrationRoot` | filesystem | reports model/calibration availability per modality |
| `count_learner_state` | none | per-CF counts | aggregates `count_learner_*` from `learner.rs:263-307` |
| `get_learner_state` | `learnerId`, `sessionTs` | reads `learner_state_history` | returns `LearnerStateVector` or 404 |
| `record_learner_k_sleep` | `learnerId`, `sessionTs`, value | `learner_k_sleep`, `learner_audit` | sleep-pressure scalar |
| `record_learner_retrieval` | `learnerId`, `traceId`, `ts`, `correct`, `score`, `stateVector?`, `components?`, `sessionTs?` | `learner_retrieval_log`, `learner_audit` | feeds ΔE / spaced-repetition |
| `upsert_goal_centroid` | `skillId`, `modality`, `vector` | `goal_centroids` | see §10.1 |
| `get_goal_distance` | `learnerId`, `skillId`, `sessionTs`, `modality`, `persistGoalState?` | `goal_centroids`, `fingerprints_learner`, optionally `learner_goal_states`, `learner_audit` | see §10.2 |
| `compile_learner_constellation` | `learnerId`, `sessionTsList[]`, `selectorKind?`, `label?` | `learner_constellations` | aggregates per-modality centroids; see [17](17_constellation_subsystem.md) §7 |
| `resolve_learner_retrieval_policy` | `learnerId`, `sessionTs`, `baseWeightProfile?`, `includeWeights?` | reads multiple learner CFs | returns a per-modality retrieval-weight vector |

Plus the cross-cutting `get_capability_matrix` (in `capability_tools.rs`) which exposes per-CF live counts for the entire learner subsystem.

### 11.1 `get_capability_matrix.runtime.learnerState`

Live counters (`capability_tools.rs:146-158`) exposing the size of every learner CF:

```
learnerProfile, learnerConstellations, fingerprintsLearner,
learnerMPerTrace, learnerStateHistory, learnerGoalStates,
learnerRetrievalLog, learnerKSleep, goalCentroids,
learnerDeltaLog, learnerAudit
```

Each value comes from a `count_learner_*` method on `RocksDbTeleologicalStore` (`learner.rs:263-307`). This is the **only** sanctioned way to size the learner store at runtime — it is used as the source-of-truth probe for `record_session_observation` workflows.

## 12. Anomaly derivation from edges

`derive_anomalies_from_edges` (handler `typed_edges_tools.rs:338`, store `rocksdb_store/anomaly_derivation.rs`) is the bridge from the **typed-edge graph** ([07](07_graph_and_causal.md)) into **contrastive training corpora** ([18](18_contrastive_and_typed_edges.md)). It belongs here because the classifier is a pure function over the teleological signature attached to each edge.

### 12.1 `AnomalyDerivationConfig` defaults

Mirrored from the offline contrastive-pair miner so both pipelines stay comparable (`anomaly_derivation.rs:31-55`).

| Field | Default |
|---|---|
| `high_threshold` | 0.60 |
| `low_threshold` | 0.30 |
| `min_disagreement` | 0.30 |
| `max_pairs` | 10_000 |

### 12.2 `classify_edge_as_anomaly` rules

Pure function over `TypedEdge.embedder_scores` (`anomaly_derivation.rs:203-279`). Embedder index map: E1=0, E5=4, E6=5, E7=6, E8=7, E9=8, E10=9, E11=10, E13=12.

| `edge_type` | Pattern | Resulting `AnomalyKind` | High slots | Low slots |
|---|---|---|---|---|
| `SemanticSimilar` | E1 high, E5 low | `SemanticButNotCausal` | [0] | [4] |
| `KeywordOverlap` | (E6 ∨ E13) high, E10 low | `KeywordButNotParaphrase` | [5] or [12] | [9] |
| `CodeRelated` | E7 high, E1 low | `CodeShapeButDifferentIntent` | [6] | [0] |
| `EntityShared` | E11 high, E8 low | `EntitySharedButDifferentStructure` | [10] | [7] |
| (any other type) | E9 high, E1 low | `HdcRobustButSemanticDifferent` (cross-type fallback) | [8] | [0] |

Rows below `min_disagreement` are dropped. Source/target with missing content are skipped (so anchors and negatives always have full text). Surviving rows are written via the standard `store_contrastive_pair` write path with `generator = "typed_edge_anomaly_derivation_v1"` — the secondary indexes (`CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`) stay in sync.

### 12.3 `AnomalyDerivationSummary`

Returned to the MCP client (`anomaly_derivation.rs:60-77`):

| Field | Meaning |
|---|---|
| `edges_scanned` | Number of `CF_TYPED_EDGES` rows walked |
| `pairs_written` | Pairs persisted to `CF_CONTRASTIVE_PAIRS` |
| `skipped_below_threshold` | Classified but dropped on `min_disagreement` |
| `skipped_missing_content` | Classified but dropped due to missing source/target text |
| `per_kind_counts` | `HashMap<AnomalyKind, usize>` |
| `duration_ms` | Wall-clock |

Idempotent on `(anchor, negative)` — re-derivation overwrites prior rows.

## 13. Synergy / fusion / meaning machinery (content layer)

| Component | File | Purpose |
|---|---|---|
| `SynergyMatrix` | `teleological/synergy_matrix.rs` | 14×14 cross-embedding synergy weights (`SYNERGY_DIM=14`); `with_base_synergies()` is the canonical instantiation used by every constellation compile |
| `GroupAlignments` / `GroupType` | `teleological/groups.rs:266+` | 6D (Factual, Temporal, Causal, Relational, Qualitative, Implementation); `dominant_group` / `weakest_group` over alignments |
| `MultiResolutionHierarchy` | `teleological/resolution.rs` | 14D → 6D group → 3D core (What/How/Why) → 1D scalar; `quick_score()` = level-3 |
| `TeleologicalVector` | `teleological/vector.rs` | Fused per-task representation; constructed from synergy + topic profile |
| `MeaningExtractionConfig`, `ExtractedMeaning`, `FusionMethod`, `NuanceDimension` | `teleological/meaning.rs` | Configuration for `services/meaning_pipeline.rs` |
| `TaskType` (8 variants) | `teleological/profile/task_type.rs:19-78` | `CodeSearch`, `SemanticSearch`, `TemporalSearch`, `CausalSearch`, `FactualSearch`, `SocialSearch`, `AbstractSearch`, `General` — each lists its primary embedder indices |
| `FusionStrategy` | `teleological/profile/fusion_strategy.rs` | enumerated fusion strategies |
| `TeleologicalProfile` | `teleological/profile/teleological.rs` | Task-specific weight + fusion config |
| `MatrixSearchConfig`, `SearchStrategy`, `SimilarityBreakdown`, `ComprehensiveComparison` | `teleological/matrix_search.rs` | Cross-correlation search across all 14 embedders; `embedder_names()` is the canonical name table |
| `TucKerCompressor` (`CpuTuckerCompressor`) | `teleological/tucker.rs` | Streaming HOSVD compression to `TuckerCore`; default ranks (4, 4, 128) |
| `TeleologicalComparator` / `BatchComparator` / `ComparisonResult` / `ComparisonValidationError` | `teleological/comparator.rs`, `comparison_error.rs` | Validation + multi-fingerprint comparison entry-point |
| `services/*.rs` | — | One module per service: correlation extractor, fusion engine, group aggregator, meaning pipeline, multi-space retriever, profile manager, synergy service, tucker decomposer |

## 14. Validation invariants

The learner layer is validation-heavy and FAIL FAST:

- `validate_vector(vec, field)` rejects empty, non-finite values, or length > `MAX_LEARNER_STATE_VECTOR_VALUES`.
- `validate_unit(value, field)` enforces `[0, 1]`.
- `validate_signed_unit(value, field)` enforces `[-1, 1]`.
- `validate_label(s, field)` enforces non-empty + length ≤ `MAX_LEARNER_LABEL_CHARS`.
- `validate_hash(s, field)` enforces lowercase hex of expected length.
- `LearnerProfile::validate` caps modalities at `LEARNER_MODALITY_COUNT + 1` (the +1 is `SelfReport`, allowed in profile but not as a centroid).
- `LearnerConstellation::validate` rejects `SelfReport` centroids and duplicate modalities.
- `LearnerStateComponents::validate` enforces `k_sleep` finite in [0, 5]; valence/arousal in [-1, 1]; everything else in [0, 1].
- All envelope fields (`raw_sha256`, `output_sha256`) must be SHA-256 hex.
- All store writes wrap mutations in a `RocksDB WriteBatch` for atomicity and emit a parent `LearnerAuditEntry` (lineage via `parent_audit_id`).

## 15. Cross-cutting lessons

- **bincode + `skip_serializing_if` is BROKEN** — bincode is positional; use JSON for any record that ever has optional fields. `TeleologicalFingerprint.last_accessed_at` works around this with `#[serde(skip)]` + `#[serde(default = ...)]`.
- **Adding a trait method requires updating ALL impl blocks** — the RocksDB store and the InMemory test stub both implement the same teleological trait family.
- **Schema defaults must match handler defaults** — `decayFunction` schema said `"exponential"` but the handler defaulted to `linear`; align them in both places.
- **MCP `additionalProperties: false` blocks unlisted params** — handler params NOT in the schema are silently dropped. Always audit schema↔DTO alignment.
- **Hard-delete order matters** — HNSW removal before RocksDB commit = data invisible if the RocksDB write fails. Always persist to disk first, then remove from index.
- **Background-task compaction can race with concurrent inserts** — HNSW rebuild + insert may create duplicate usearch entries; the `compaction_lock: RwLock<()>` on `RocksDbTeleologicalStore` (`store.rs:120-127`) gives compaction an exclusive write lock and stores/deletes a concurrent read lock.

## 16. What is NOT covered here

- The 13×13 cross-correlation centroid math used in `Constellation` — see [17](17_constellation_subsystem.md).
- The compile / score / derive operations on constellations — see [17](17_constellation_subsystem.md).
- ΔS / ΔC / ΔE math, M-trace, and spaced-repetition formulas — see [20](20_learning_subsystem.md).
- CAMD-DPO consumption of contrastive pairs derived in §12 — see [19](19_camd_dpo_subsystem.md).
- The four search strategies in detail and their weight profiles — see [06](06_search_and_fusion.md).
- GPU acceleration for batched cosine and HDBSCAN — see [11](11_cuda_acceleration.md).
