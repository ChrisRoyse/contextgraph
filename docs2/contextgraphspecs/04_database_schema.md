# Database Schema

**Source files covered:**
- `crates/context-graph-storage/src/column_families.rs`
- `crates/context-graph-storage/src/teleological/column_families.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/**/*.rs`

## 1. Database Type
RocksDB column families (no SQL). All CFs registered at open time. Single-writer enforced via `<db_path>/mcp.pid` Unix flock.

## 2. CF Counts
| Group | Count |
| --- | --- |
| Base | 11 |
| Teleological / learner / training | 41 |
| Quantized embedder (CF_EMB_0..CF_EMB_13) | 14 |
| Code | 5 |
| Causal | 2 |
| **Total** | **73** |

## 3. Base CFs (11)
| Name | Key | Purpose |
| --- | --- | --- |
| `nodes` | UUID | Memory node primary record |
| `edges` | composite | Generic graph edge storage |
| `embeddings` | UUID | 1536D × f32 (~6KB) embedding vectors (legacy/code path) |
| `metadata` | UUID | Node metadata |
| `temporal` | timestamp | Time-based query index |
| `tags` | tag prefix | Tag → UUID list |
| `sources` | source prefix | Source → UUID list |
| `system` | sentinel keys | Rare metadata (no compression) |
| `embedder_edges` | `[embedder_id:u8][source:16]` (legacy) / `[embedder_id:u8][source:16][target:16]` (33B) | K-NN edges per embedder |
| `typed_edges` | `[source:16][target:16]` | Multi-relation typed edges |
| `typed_edges_by_type` | `[edge_type:u8][source:16][target:16]` (33B) | Edge-type secondary index |

## 4. Teleological / Learner / Training CFs (41)
| Name | Notes |
| --- | --- |
| `fingerprints` | UUID → SemanticFingerprint (14 embeddings) + TopicProfile + metadata (~60KB) |
| `topic_profiles` | UUID → 13 × f32 (52B) |
| `e13_splade_inverted` | term_id (u16) → Vec<UUID>; SPLADE 30522 vocab |
| `e6_sparse_inverted` | term_id → Vec<UUID>; sparse 30522 vocab |
| `e1_matryoshka_128` | UUID → 128D truncated E1 (coarse filter) |
| `content` | UUID → utf8 (max 1MB, LZ4) |
| `source_metadata` | UUID → bincode SourceMetadata |
| `file_index` | path prefix → UUID list |
| `topic_portfolio` | session_id or `__latest__` → portfolio JSON |
| `e12_late_interaction` | UUID → 128D-per-token list |
| `entity_provenance` | UUID → EntityLink + TransE score |
| `audit_log` | `[ts:u64 BE][op_id:16]` → AuditRecord (append-only, big-endian timestamp prefix) |
| `audit_by_target` | `[target:16][ts:u64 BE]` → audit_log key (16-byte prefix extractor) |
| `merge_history` | merged_id → JSON merge lineage (PERMANENT) |
| `importance_history` | memory_id → importance change log (PERMANENT) |
| `tool_call_index` | tool_use_id → memory UUIDs |
| `consolidation_recommendations` | rec_id → rec |
| `embedding_registry` | fingerprint_id → model+version per embedder slot (PERMANENT, 16-byte prefix) |
| `custom_weight_profiles` | profile name → weights |
| `hnsw_graphs` | per-index serialized HNSW (~30 keys, large blocks) |
| `training_records` | UUID → TrainingRecord v3 (8MB write buffer) |
| `learning_events` | event UUID → LearningEvent |
| `learner_profile` | learner_id → LearnerProfile |
| `learner_constellations` | `[learner_id:16][selector_kind:u8]` → constellation |
| `fingerprints_learner` | `[learner_id:16][session_ts:u64 BE]` → LearnerFingerprint |
| `learner_m_per_trace` | `[learner_id:16][trace_id:16]` → LearnerMTrace |
| `learner_state_history` | `[learner_id:16][session_ts:u64 BE]` → LearnerStateVector |
| `learner_goal_states` | `[learner_id:16][skill_id:16]` → LearnerGoalState |
| `learner_retrieval_log` | `[learner_id:16][trace_id:16][ts:u64 BE]` → LearnerRetrievalLog |
| `learner_k_sleep` | `[learner_id:16][session_ts:u64 BE]` → LearnerKSleep |
| `goal_centroids` | `[skill_id:16][modality:u8]` → GoalCentroid |
| `learner_delta_log` | `[learner_id:16][session_ts:u64 BE]` → LearnerDeltaLog |
| `learner_audit` | `[learner_id:16][ts:u64 BE][audit_id:16]` → LearnerAuditEntry |
| `learner_training_datasets` | dataset_id → row-major f32 matrix (versioned) |
| `constellations` | UUID → Constellation (can reach hundreds of KB) |
| `constellation_by_selector` | `[selector_kind:u8][SHA256(canonical):16]` → constellation UUID |
| `contrastive_pairs` | `[anchor:16][negative:16]` (32B) → ContrastivePair |
| `contrastive_by_kind` | `[kind:u8] + composite` |
| `contrastive_by_anchor` | `[anchor:16] + composite` |
| `typed_edge_records` | `[source:16][target:16][type:u8]` (33B) → TypedEdgeTrainingRecord |
| `typed_edge_validations` | same composite → LLMEdgeValidation |

## 5. Quantized Embedder CFs (14)
Per-embedder compressed storage. Key = UUID (16B); value = quantized payload.

| Name | ModelId | Method | Stored size |
| --- | --- | --- | --- |
| `emb_0` | E1_Semantic | PQ-8 | 8B (from 1024D) |
| `emb_1` | E2_TemporalRecent | Float8E4M3 | 512B (from 512D) |
| `emb_2` | E3_TemporalPeriodic | Float8E4M3 | 512B |
| `emb_3` | E4_TemporalPositional | Float8E4M3 | 512B |
| `emb_4` | E5_Causal | PQ-8 | 8B (from 768D, dual cause/effect) |
| `emb_5` | E6_Sparse | SparseNative | variable |
| `emb_6` | E7_Code | PQ-8 | 8B (from 1536D) |
| `emb_7` | E8_Graph | Float8E4M3 | 1024B (dual source/target) |
| `emb_8` | E9_HDC | Binary | 1250B (from 10000-bit binary) |
| `emb_9` | E10_Multimodal | PQ-8 | 8B (from 768D, dual paraphrase/context) |
| `emb_10` | E11_Entity (Kepler) | PQ-8 | 8B (from 768D) |
| `emb_11` | E12_LateInteraction | TokenPruning | ~50% original |
| `emb_12` | E13_SPLADE | SparseNative | variable |
| `emb_13` | E14_BgeM3Dense | PQ-8 | 8B (from 1024D) |

## 6. Code CFs (5)
| Name | Purpose |
| --- | --- |
| `code_entities` | UUID → CodeEntity (text + AST chunk metadata) |
| `code_e7_embeddings` | UUID → 1536D × f32 (6144B) |
| `code_file_index` | path prefix → UUID list |
| `code_name_index` | name prefix → UUID list (partial-name match) |
| `code_signature_index` | hash → UUID (no compression) |

## 7. Causal CFs (2)
| Name | Purpose |
| --- | --- |
| `causal_relationships` | causal_relationship_id → CausalRelationship (text + dual E5 + LLMProvenance) |
| `causal_by_source` | source fingerprint UUID → list of causal_relationship_ids |

## 8. RocksDB Option Profiles
| Profile | Compression | Bloom | Prefix | Write buffer |
| --- | --- | --- | --- | --- |
| nodes / fingerprints | LZ4 | yes | none | 4MB |
| edges / typed_edges / embedder_edges | LZ4 | yes | 16/17/33 | 4MB |
| embeddings (per-embedder) | LZ4 | yes | none | 8MB |
| index (sparse/file/audit_by_target/etc.) | LZ4 | yes | 16 | 2MB |
| system | none | no | none | small |
| append_only (audit_log/merge/importance/embedding_registry) | LZ4 | yes | timestamp / target | 2-4MB |

## 9. Versioning
| Record | Version |
| --- | --- |
| TrainingRecord | 3 |
| LearnerRecord (all learner CFs) | 1 |
| LearningEvent | 1 |
| LearnerTrainingDataset | 1 |

## 10. Connection Management
RocksDB option builders configure shared block cache, bloom filters, prefix extractors, LZ4 compression, per-CF write buffer limits. `PidFileGuard` writes `<db_path>/mcp.pid` and holds a Unix flock to enforce single-writer.

## 11. What Is NOT Covered
No SQL columns, foreign keys, triggers, or migrations. Schema evolution is wipe-and-rebuild (pre-production policy).
