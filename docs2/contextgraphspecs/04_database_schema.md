# Database Schema

**Source files covered:**
- `crates/context-graph-storage/src/column_families.rs`
- `crates/context-graph-storage/src/teleological/column_families.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/**/*.rs`

## 1. Database Type
The project uses RocksDB column families, not SQL tables. All column families are registered at open time by descriptor builders.

## 2. Counts
| Group | Count |
| --- | --- |
| Base | 11 |
| Teleological/learner/training | 41 |
| Quantized embedder | 14 |
| Code | 5 |
| Causal | 2 |
| Total | 73 |

## 3. Every Column Family
| Group | Constant | Name | Source comment |
| --- | --- | --- | --- |
| base | NODES | nodes | Primary node storage column family. |
| base | EDGES | edges | Primary node storage column family. Graph edge storage column family. |
| base | EMBEDDINGS | embeddings | Primary node storage column family. Graph edge storage column family. Embedding vectors column family (1536D × f32 = 6144 bytes per embedding). |
| base | METADATA | metadata | Graph edge storage column family. Embedding vectors column family (1536D × f32 = 6144 bytes per embedding). Node metadata column family. |
| base | TEMPORAL | temporal | Embedding vectors column family (1536D × f32 = 6144 bytes per embedding). Node metadata column family. Temporal index column family for time-based queries. |
| base | TAGS | tags | Node metadata column family. Temporal index column family for time-based queries. Tag index column family. |
| base | SOURCES | sources | Temporal index column family for time-based queries. Tag index column family. Source index column family. |
| base | SYSTEM | system | Tag index column family. Source index column family. System metadata column family (rare access, no compression). |
| base | EMBEDDER_EDGES | embedder_edges | K-NN edges per embedder. Key: [embedder_id: u8][source_uuid: 16 bytes] = 17 bytes Value: serialized Vec<EmbedderEdge> (k neighbors) |
| base | TYPED_EDGES | typed_edges | Multi-relation typed edges. Key: [source_uuid: 16 bytes][target_uuid: 16 bytes] = 32 bytes Value: serialized TypedEdge |
| base | TYPED_EDGES_BY_TYPE | typed_edges_by_type | Typed edges indexed by edge type (secondary index). Key: [edge_type: u8][source_uuid: 16 bytes] = 17 bytes Value: target_uuid (16 bytes) |
| teleological/learner/training | CF_FINGERPRINTS | fingerprints | - SemanticFingerprint (13 embeddings, 15,120 dense dims = ~60KB) - TopicProfile (13D, 52 bytes) - Metadata (timestamps, hash, etc.) |
| teleological/learner/training | CF_TOPIC_PROFILES | topic_profiles |  Stored separately from full fingerprints for fast profile-only queries. Key: UUID (16 bytes) → Value: 13 × f32 = 52 bytes |
| teleological/learner/training | CF_E13_SPLADE_INVERTED | e13_splade_inverted | Key: term_id (u16, 2 bytes) → Value: Vec<Uuid> (memory IDs with that term)  SPLADE vocabulary size: 30,522 terms (per semantic.rs E13_SPLADE_VOCAB) |
| teleological/learner/training | CF_E6_SPARSE_INVERTED | e6_sparse_inverted | # Usage - Stage 1: Co-pilot with E13 for dual sparse recall - Stage 3.5: Tie-breaker when E1 scores are close |
| teleological/learner/training | CF_E1_MATRYOSHKA_128 | e1_matryoshka_128 |  E1 Matryoshka embeddings (1024D) can be truncated to 128D while preserving reasonable accuracy for coarse filtering. |
| teleological/learner/training | CF_CONTENT | content | - Maximum content size: 1MB (enforced at application layer) - Compression: LZ4 (~50% reduction for typical text) - No bloom filter (point lookups only by UUID) |
| teleological/learner/training | CF_SOURCE_METADATA | source_metadata | - LZ4 compression (JSON-like data compresses well) - Bloom filter for fast lookups - Point lookups only by UUID |
| teleological/learner/training | CF_FILE_INDEX | file_index | - LZ4 compression (file paths and UUID lists compress well) - Bloom filter for fast path existence checks - Prefix iteration for path-based queries |
| teleological/learner/training | CF_TOPIC_PORTFOLIO | topic_portfolio | - LZ4 compression (JSON compresses well) - Bloom filter for fast session lookups - Point lookups by session_id or "__latest__" sentinel |
| teleological/learner/training | CF_E12_LATE_INTERACTION | e12_late_interaction |  # FAIL FAST Policy No fallback options - let RocksDB error on open if misconfigured. |
| teleological/learner/training | CF_ENTITY_PROVENANCE | entity_provenance |  # FAIL FAST Policy No fallback options - let RocksDB error on open if misconfigured. |
| teleological/learner/training | CF_AUDIT_LOG | audit_log | - Bloom filter for fast existence checks - Big-endian timestamp prefix ensures chronological iteration - Append-only: NO update or delete operations |
| teleological/learner/training | CF_AUDIT_BY_TARGET | audit_by_target | - Bloom filter for fast target existence checks - 16-byte prefix extractor for UUID-based prefix scans - Append-only: NO update or delete operations |
| teleological/learner/training | CF_MERGE_HISTORY | merge_history | - LZ4 compression (JSON fingerprint data compresses well) - Bloom filter for fast merged_id lookups - PERMANENT: never expires, never deleted |
| teleological/learner/training | CF_IMPORTANCE_HISTORY | importance_history | - LZ4 compression (structured data compresses well) - Bloom filter for fast memory_id lookups - PERMANENT: never expires, never deleted |
| teleological/learner/training | CF_TOOL_CALL_INDEX | tool_call_index | - LZ4 compression - Bloom filter for fast tool_use_id lookups - FAIL FAST: No fallback options |
| teleological/learner/training | CF_CONSOLIDATION_RECOMMENDATIONS | consolidation_recommendations | - LZ4 compression - Bloom filter for fast lookups - FAIL FAST: No fallback options |
| teleological/learner/training | CF_EMBEDDING_REGISTRY | embedding_registry | - Bloom filter for fast fingerprint_id lookups - 16-byte prefix extractor for UUID keys - PERMANENT: never expires, never deleted |
| teleological/learner/training | CF_CUSTOM_WEIGHT_PROFILES | custom_weight_profiles | - LZ4 compression - Bloom filter for fast name lookups - Point lookups by profile name |
| teleological/learner/training | CF_HNSW_GRAPHS | hnsw_graphs | - No bloom filter (few keys, ~30 total) - Large block size (values are large) - Updated periodically + at shutdown |
| teleological/learner/training | CF_TRAINING_RECORDS | training_records | - Bloom filter for fast point lookups by memory UUID - 16-byte prefix extractor for UUID keys - 8MB write buffer (values are MB-sized when full embeddings included) |
| teleological/learner/training | CF_LEARNING_EVENTS | learning_events | - Bloom filter for fast point lookup by event UUID - 16-byte prefix extractor for UUID keys - 4MB write buffer (event records are usually small but may include text) |
| teleological/learner/training | CF_LEARNER_PROFILE | learner_profile |  Key: learner UUID bytes (16). Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerProfile]`. |
| teleological/learner/training | CF_LEARNER_CONSTELLATIONS | learner_constellations |  Key: `[learner_id:16][selector_kind:u8]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode learner constellation payload]`. |
| teleological/learner/training | CF_FINGERPRINTS_LEARNER | fingerprints_learner |  Key: `[learner_id:16][session_ts:u64 big-endian]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerFingerprint]`. |
| teleological/learner/training | CF_LEARNER_M_PER_TRACE | learner_m_per_trace |  Key: `[learner_id:16][trace_id:16]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerMTrace]`. |
| teleological/learner/training | CF_LEARNER_STATE_HISTORY | learner_state_history |  Key: `[learner_id:16][session_ts:u64 big-endian]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerStateVector]`. |
| teleological/learner/training | CF_LEARNER_GOAL_STATES | learner_goal_states |  Key: `[learner_id:16][skill_id:16]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerGoalState]`. |
| teleological/learner/training | CF_LEARNER_RETRIEVAL_LOG | learner_retrieval_log |  Key: `[learner_id:16][trace_id:16][ts:u64 big-endian]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerRetrievalLog]`. |
| teleological/learner/training | CF_LEARNER_K_SLEEP | learner_k_sleep |  Key: `[learner_id:16][session_ts:u64 big-endian]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerKSleep]`. |
| teleological/learner/training | CF_GOAL_CENTROIDS | goal_centroids |  Key: `[skill_id:16][modality:u8]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode GoalCentroid]`. |
| teleological/learner/training | CF_LEARNER_DELTA_LOG | learner_delta_log |  Key: `[learner_id:16][session_ts:u64 big-endian]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerDeltaLog]`. |
| teleological/learner/training | CF_LEARNER_AUDIT | learner_audit |  Key: `[learner_id:16][ts:u64 big-endian][audit_id:16]`. Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerAuditEntry]`. |
| teleological/learner/training | CF_LEARNER_TRAINING_DATASETS | learner_training_datasets |  Key: dataset UUID bytes (16). Value: `[LEARNER_TRAINING_DATASET_VERSION: u8][bincode LearnerTrainingDataset]`. |
| teleological/learner/training | CF_CONSTELLATIONS | constellations | - Bloom filter for fast UUID point lookups - 16-byte prefix extractor for UUID keys - 8MB write buffer (constellation records can reach several hundred KB) |
| teleological/learner/training | CF_CONSTELLATION_BY_SELECTOR | constellation_by_selector | - Bloom filter for fast existence checks - 1-byte prefix extractor (selector kind) for per-kind range scans - 2MB write buffer (secondary index is append-light) |
| teleological/learner/training | CF_CONTRASTIVE_PAIRS | contrastive_pairs | - Bloom filter for fast point lookups by composite key - 32-byte prefix extractor so `[anchor][negative]` stays a stable prefix - 4MB write buffer (pair records are typically 1-5KB each) |
| teleological/learner/training | CF_CONTRASTIVE_BY_KIND | contrastive_by_kind | - Bloom filter for fast existence checks - 1-byte prefix extractor (kind byte) - 2MB write buffer (append-light secondary index) |
| teleological/learner/training | CF_CONTRASTIVE_BY_ANCHOR | contrastive_by_anchor | - Bloom filter for fast anchor existence checks - 16-byte prefix extractor (anchor UUID) - 2MB write buffer |
| teleological/learner/training | CF_TYPED_EDGE_RECORDS | typed_edge_records | - Bloom filter for fast composite-key lookups - 33-byte prefix extractor so the whole composite key is a stable prefix - 4MB write buffer (records are typically 2-10 KB each) |
| teleological/learner/training | CF_TYPED_EDGE_VALIDATIONS | typed_edge_validations | - Bloom filter for fast existence checks - 33-byte prefix extractor - 2MB write buffer (append-light) |
| quantized_embedder | CF_EMB_0 | emb_0 | Column family for E1_Semantic (ModelId=0) quantized embeddings. Quantization: PQ-8 (8 bytes compressed from 1024D). |
| quantized_embedder | CF_EMB_1 | emb_1 | Quantization: PQ-8 (8 bytes compressed from 1024D). Column family for E2_TemporalRecent (ModelId=1) quantized embeddings. Quantization: Float8E4M3 (512 bytes from 512D). |
| quantized_embedder | CF_EMB_2 | emb_2 | Quantization: Float8E4M3 (512 bytes from 512D). Column family for E3_TemporalPeriodic (ModelId=2) quantized embeddings. Quantization: Float8E4M3 (512 bytes from 512D). |
| quantized_embedder | CF_EMB_3 | emb_3 | Quantization: Float8E4M3 (512 bytes from 512D). Column family for E4_TemporalPositional (ModelId=3) quantized embeddings. Quantization: Float8E4M3 (512 bytes from 512D). |
| quantized_embedder | CF_EMB_4 | emb_4 | Quantization: Float8E4M3 (512 bytes from 512D). Column family for E5_Causal (ModelId=4) quantized embeddings. Quantization: PQ-8 (8 bytes compressed from 768D). |
| quantized_embedder | CF_EMB_5 | emb_5 | Quantization: PQ-8 (8 bytes compressed from 768D). Column family for E6_Sparse (ModelId=5) quantized embeddings. Quantization: SparseNative (variable size, indices+values). |
| quantized_embedder | CF_EMB_6 | emb_6 | Quantization: SparseNative (variable size, indices+values). Column family for E7_Code (ModelId=6) quantized embeddings. Quantization: PQ-8 (8 bytes compressed from 1536D). |
| quantized_embedder | CF_EMB_7 | emb_7 | Quantization: PQ-8 (8 bytes compressed from 1536D). Column family for E8_Graph (ModelId=7) quantized embeddings. Quantization: Float8E4M3 (1024 bytes from 1024D). |
| quantized_embedder | CF_EMB_8 | emb_8 | Quantization: Float8E4M3 (1024 bytes from 1024D). Column family for E9_HDC (ModelId=8) quantized embeddings. Quantization: Binary (1250 bytes from 10000D binary vector). |
| quantized_embedder | CF_EMB_9 | emb_9 | Quantization: Binary (1250 bytes from 10000D binary vector). Column family for E10_Multimodal (ModelId=9) quantized embeddings. Quantization: PQ-8 (8 bytes compressed from 768D). |
| quantized_embedder | CF_EMB_10 | emb_10 | Quantization: PQ-8 (8 bytes compressed from 768D). Column family for E11_Entity/Kepler (production slot 10) quantized embeddings. Quantization: PQ-8 (8 bytes compressed from 768D). |
| quantized_embedder | CF_EMB_11 | emb_11 | Quantization: PQ-8 (8 bytes compressed from 768D). Column family for E12_LateInteraction (ModelId=11) quantized embeddings. Quantization: TokenPruning (variable size, ~50% of original). |
| quantized_embedder | CF_EMB_12 | emb_12 | Quantization: TokenPruning (variable size, ~50% of original). Column family for E13_SPLADE (ModelId=12) quantized embeddings. Quantization: SparseNative (variable size, indices+values). |
| quantized_embedder | CF_EMB_13 | emb_13 | Quantization: SparseNative (variable size, indices+values). Column family for E14_BgeM3Dense (ModelId=13) quantized embeddings. Quantization: PQ-8 (8 bytes compressed from 1024D). |
| code | CF_CODE_ENTITIES | code_entities | - LZ4 compression (code text compresses well) - Bloom filter for fast UUID lookups - Point lookups by entity ID |
| code | CF_CODE_E7_EMBEDDINGS | code_e7_embeddings | - LZ4 compression (float arrays compress moderately) - Bloom filter for fast lookups - Each embedding is exactly 6144 bytes |
| code | CF_CODE_FILE_INDEX | code_file_index | - LZ4 compression (paths and UUID lists compress well) - Bloom filter for fast path existence checks - Prefix iteration for path-based queries |
| code | CF_CODE_NAME_INDEX | code_name_index | # Storage Details - LZ4 compression - Prefix scan support for partial name matching |
| code | CF_CODE_SIGNATURE_INDEX | code_signature_index | # Storage Details - No compression (hash keys are random, don't compress) - Bloom filter for fast lookups |
| causal | CF_CAUSAL_RELATIONSHIPS | causal_relationships | - LZ4 compression (text + embeddings compress well) - Bloom filter for fast UUID lookups - Point lookups by causal_relationship_id |
| causal | CF_CAUSAL_BY_SOURCE | causal_by_source | - LZ4 compression (UUID lists compress well) - Bloom filter for fast source existence checks - Point lookups by source fingerprint ID |

## 4. Versioning
| Record | Version |
| --- | --- |
| TrainingRecord | 3 |
| Learner records | 1 |
| LearningEvent | 1 |

## 5. Connection Management And Options
RocksDB option builders configure shared cache, bloom filters, prefix extractors, LZ4 or no compression, and per-CF write buffer limits. MCP `PidFileGuard` writes `<db_path>/mcp.pid` and holds a Unix flock to prevent multiple processes opening the same DB.

## 6. What Is Not Covered
There are no SQL columns, SQL indexes, SQL foreign keys, or SQL migrations in the source.
