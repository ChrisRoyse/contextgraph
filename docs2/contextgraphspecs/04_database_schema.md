# Database Schema

**Source files covered:**
- `crates/context-graph-storage/src/lib.rs` — crate root, `TOTAL_COLUMN_FAMILIES` re-export
- `crates/context-graph-storage/src/column_families.rs` — base 11 CFs, option profiles, `get_all_column_family_descriptors` (line 387), `TOTAL_COLUMN_FAMILIES = 101` (line 408)
- `crates/context-graph-storage/src/teleological/column_families.rs` — 41 teleological + 14 quantized embedder + 5 code + 2 causal CFs (`TELEOLOGICAL_CF_COUNT = 41` line 657, `QUANTIZED_EMBEDDER_CF_COUNT = 14` line 746, `CODE_CF_COUNT = 5` line 1887, `CAUSAL_CF_COUNT = 2` line 2057)
- `crates/context-graph-storage/src/dynamicjepa/column_families.rs` — 28 DynamicJEPA CFs (`DJ_CF_NAMES` line 43, `DJ_CF_COUNT = DJ_CF_NAMES.len()` line 74)
- `crates/context-graph-storage/src/dynamicjepa/keys.rs` — fixed-byte key encoders for DJ CFs
- `crates/context-graph-storage/src/dynamicjepa/encode.rs` — DJ versioned bincode wire format (`DYNAMIC_JEPA_STORAGE_VALUE_VERSION = 1`, line 9)
- `crates/context-graph-storage/src/dynamicjepa/audit.rs` — `DJ_AUDIT_RECORD_VERSION = 2` (line 7)
- `crates/context-graph-storage/src/teleological/serialization.rs` — `TELEOLOGICAL_VERSION = 1` (line 35), `MIN_FINGERPRINT_SIZE = 5_000` (line 44), `MAX_FINGERPRINT_SIZE = 150_000` (line 56)
- `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs` — DB open path (`open_with_config` line 145), single-writer setup, soft-delete persistence, lock inspection
- `crates/context-graph-storage/src/teleological/rocksdb_store/types.rs` — `TeleologicalStoreConfig` defaults (line 172), `TeleologicalStoreError` enum (line 31)
- `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs` — `SOFT_DELETE_PREFIX = "soft_deleted::"` (line 34), hard-delete batch order, GC compaction threshold (`GC_COMPACTION_THRESHOLD = 10`, line 557)
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs` — `EmbedderIndex::all_hnsw()` (line 186, 16 indexes including E14)
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs` — `NUM_EMBEDDERS = 14` (line 48), `TOPIC_PROFILE_DIM = 14` (line 54), per-embedder dimension constants (lines 6-45)
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs` — `HnswConfig::default_for_dimension` (line 89; M=16, ef_construction=200, ef_search=100), `HnswConfig::matryoshka_128d` (line 96; M=32, ef_construction=256, ef_search=128)
- `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/types.rs` — `MAX_HNSW_VECTORS_PER_INDEX = 2_000_000` (line 18)
- `crates/context-graph-storage/src/teleological/indexes/registry.rs` — `EmbedderIndexRegistry::new()` (line 89, builds 16 HNSW indexes)
- `crates/context-graph-storage/src/graph_edges/repository.rs` — typed-edges-by-type 33-byte secondary key construction (lines 235-243, 301-305, 508-514)
- `crates/context-graph-mcp/src/main.rs:578-797` — `PidFileGuard` (PID file + Unix `flock`)
- `crates/context-graph-core/src/{training,learner,learning,constellation,contrastive,typed_edge_export,llm_edge_validation,learner_training}/mod.rs` — record version constants

Cross-references:
- [03_configuration.md](03_configuration.md) — config knobs that govern open path (`storage.path`, block cache, GC retention)
- [05_embedding_subsystem.md](05_embedding_subsystem.md) — semantics of E1-E14 embedders that map to `CF_EMB_*` and HNSW indexes
- [06_search_and_fusion.md](06_search_and_fusion.md) — search pipeline that drives index_ops/inverted_index reads
- [07_graph_and_causal.md](07_graph_and_causal.md) — typed-edge / causal CF semantics
- [09_storage_layer.md](09_storage_layer.md) — runtime behavior, repository pattern, atomicity, HNSW lifecycle
- [13_teleological_subsystem.md](13_teleological_subsystem.md) — how `TeleologicalFingerprint` is shaped, which CFs persist which fields
- [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md) — DynamicJEPA semantics and CF use

## 1. Database Type and Open Sequence

RocksDB column families (no SQL, no foreign keys, no triggers, no migrations).

`RocksDbTeleologicalStore::open_with_config` (`teleological/rocksdb_store/store.rs:145`) performs:

1. Pre-open lock inspection: `inspect_rocksdb_lock(path)` (read-only `fcntl(F_SETLK, F_WRLCK)` probe of the RocksDB `LOCK` file — never removed; `store.rs:472`) and `assert_existing_dynamicjepa_column_families(path)` (FAIL FAST if any of the 28 DJ CFs are missing from an existing DB; `store.rs:368`).
2. Build shared `Cache::new_lru_cache(config.block_cache_size)` (default 256MB).
3. Construct DB `Options`: `create_if_missing(true)`, `create_missing_column_families(true)`, `set_max_open_files(config.max_open_files)`, `set_paranoid_checks(true)`. If `enable_wal=false`, `set_manual_wal_flush(true)`.
4. `cf_descriptors = get_all_column_family_descriptors(&cache)` — returns the canonical `TOTAL_COLUMN_FAMILIES = 101` descriptors (`column_families.rs:387/408`).
5. `DB::open_cf_descriptors(&db_opts, &path_str, cf_descriptors)`. On failure, `transform_corruption_error` (`store.rs:576`) converts missing-SST and corrupted-MANIFEST errors into `TeleologicalStoreError::CorruptionDetected` with extracted file names.
6. Verify every `DJ_CF_NAMES` handle is present after open; otherwise fail with `ColumnFamilyNotFound` (`store.rs:200-210`).
7. Construct `EmbedderIndexRegistry::new()` (16 HNSW indexes, see §10) and `CausalE11Index::new()` (E11 ANN over causal relationships).
8. Load persisted soft-delete markers from `CF_SYSTEM` (`SOFT_DELETE_PREFIX = "soft_deleted::"` from `crud.rs:34`) into an `Arc<DashMap<Uuid, i64>>`. Legacy 1-byte markers are accepted with timestamp 0 (immediately GC-eligible) (`store.rs:228-276`).
9. Initialize `total_doc_count = raw_fingerprint_count - soft_deleted.len()` for BM25-IDF using `Arc<AtomicUsize>` (`store.rs:281-299`).
10. `try_load_hnsw_indexes()`: restore from `CF_HNSW_GRAPHS`. Returns `Ok(true)` if **any** index loads. If true and any registered HNSW index is empty (e.g. newly-enabled embedder), `clear_all()` + `rebuild_indexes_from_store()` (`store.rs:316-351`). E2 persisted graph is **always skipped** to force sinusoidal-PE recompute from `created_at` (`store.rs:1054-1059`).
11. If `E11_ENTITY_ENABLED`, `rebuild_causal_e11_index()` from `CF_CAUSAL_RELATIONSHIPS`.
12. `verify_consistency(raw_fp_count)` cross-checks `CF_FINGERPRINTS` vs `CF_CONTENT` count, HNSW size vs live fingerprint count (10% tolerance, min 5), and `total_doc_count` consistency. Logs warnings only — never blocks startup.

Single-writer enforcement via `PidFileGuard` (PID file + Unix `flock(LOCK_EX | LOCK_NB)`) is owned by the MCP main, NOT by the storage crate (see §11).

## 2. CF Counts (canonical: `TOTAL_COLUMN_FAMILIES = 101`)

Defined at `crates/context-graph-storage/src/column_families.rs:408`.

| Group | Count | Source | Constant |
| --- | --- | --- | --- |
| Base | 11 | `column_families.rs::cf_names::ALL` | `cf_names::ALL.len() == 11` |
| Teleological | 41 | `teleological/column_families.rs::TELEOLOGICAL_CFS` | `TELEOLOGICAL_CF_COUNT = 41` |
| Quantized embedder (`emb_0`..`emb_13`) | 14 | `teleological/column_families.rs::QUANTIZED_EMBEDDER_CFS` | `QUANTIZED_EMBEDDER_CF_COUNT = 14` |
| Code | 5 | `teleological/column_families.rs::CODE_CFS` | `CODE_CF_COUNT = 5` |
| Causal | 2 | `teleological/column_families.rs::CAUSAL_CFS` | `CAUSAL_CF_COUNT = 2` |
| DynamicJEPA | 28 | `dynamicjepa/column_families.rs::DJ_CF_NAMES` | `DJ_CF_COUNT == 28` |
| **Total** | **101** | `column_families.rs::TOTAL_COLUMN_FAMILIES` | |

`get_all_column_family_descriptors(cache)` (`column_families.rs:387`) returns these in order:

1. 11 base
2. 41 teleological + 14 quantized + 5 code + 2 causal (= 62, supplied by `teleological::get_all_cf_descriptors`)
3. 28 DJ (supplied by `dynamicjepa::column_families::dj_cf_descriptors`)

## 3. Base CFs (11)

Source: `crates/context-graph-storage/src/column_families.rs`. Constants in `cf_names` module (lines 39-97).

| Name | Key encoding | Value encoding | Option profile | Purpose |
| --- | --- | --- | --- | --- |
| `nodes` | UUID (16B) | (legacy `MemoryNode` bincode) | `nodes_options` (bloom 10b, LZ4, point-lookup 256MB hint, 4MB WB) | Primary node record (legacy MemoryStore path; not used by current teleological store) |
| `edges` | composite `[source:16][...]` | (legacy edge bincode) | `edges_options` (bloom, LZ4, 16B prefix, 4MB WB) | Generic graph edge storage (legacy) |
| `embeddings` | UUID (16B) | `Vec<f32>` 1536D = 6144B | `embeddings_options` (64KB blocks, LZ4, 8MB WB) | Embedding vectors (legacy code path) |
| `metadata` | UUID (16B) | `MemoryNode` metadata (bincode) | `index_options` (bloom, LZ4, 16B prefix, 2MB WB) | Node metadata (legacy) |
| `temporal` | `[ts_ms:?][uuid:16]` | UUID | `index_options` | Temporal index for range scans |
| `tags` | `[tag_bytes][uuid:16]` | UUID | `index_options` | Tag → UUID list |
| `sources` | `[source_uri][uuid:16]` | UUID | `index_options` | Source URI → UUID list |
| `system` | sentinel string keys (e.g. `"soft_deleted::{uuid}"`, `"cursor::{key}"`) | varies (`i64 BE` 8B for soft-delete TS; opaque bytes for cursors) | `system_options` (NO compression, NO bloom, no prefix, 2MB WB) | Rare metadata; SOFT-DELETE markers + processing cursors persisted here |
| `embedder_edges` | `[embedder_id:u8][source:16]` = 17B (`EdgeStorageKey::new`) | `[v:u8 = 1][count:u32 LE][N × ([target:16][sim:f32 LE])]` (`graph_edges/serialization.rs:30`) | `embedder_edges_options` (bloom, LZ4, 1B prefix, 4MB WB) | K-NN edges per embedder; one posting list per `(embedder, source)` |
| `typed_edges` | `[source:16][target:16]` (32B) | `[v:u8 = 1][bincode TypedEdge]` (`graph_edges/serialization.rs:157`) | `typed_edges_options` (bloom, LZ4, 16B prefix, 4MB WB) | Multi-relation typed edges (primary store) |
| `typed_edges_by_type` | `[edge_type:u8][source:16][target:16]` (33B; legacy 17B `[type][source]` still readable on iterate) | `target_uuid` (16B) | `typed_edges_by_type_options` (bloom, LZ4, 1B prefix, 2MB WB) | Edge-type secondary index, queried by 17B prefix `[type][source]` |

Notes on the 33-byte typed-edges-by-type secondary key (`graph_edges/repository.rs:235-243`, `301-305`, `508-514`): the legacy 17-byte key `[type][source]` (with the target stored only in the value) silently overwrote earlier edges when the same `(type, source)` had multiple targets. The current encoding writes the full `[type][source][target]` (33B) key with `target_uuid` (16B) duplicated as the value. `get_typed_edges_by_type` iterates with the 17-byte `[type][source]` prefix and parses the target from key bytes 17..33 when available, otherwise from the value (legacy compat) (`repository.rs:344-424`).

Direction-aware typed-edge queries (`crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs:397-510`):
- `outgoing` / default: secondary index `get_typed_edges_by_type(source, type)` (fast, 17B prefix scan).
- `incoming`: full CF scan via `get_typed_edges_to(target)` and post-filter by edge type (O(n) over `CF_TYPED_EDGES`; no reverse index).
- `both`: union of the above.

## 4. Teleological CFs (41)

Source: `crates/context-graph-storage/src/teleological/column_families.rs:611-654` lists `TELEOLOGICAL_CFS`. Each entry below cites the `pub const CF_*` declaration line.

| Name | Key | Value | Notes / Source |
| --- | --- | --- | --- |
| `fingerprints` | UUID (16B) | `[v:u8 = 1][bincode TeleologicalFingerprint]` (~50KB; range 5KB–150KB) | `CF_FINGERPRINTS` (l.70). `TELEOLOGICAL_VERSION = 1` (`serialization.rs:35`). Min/max enforced (`serialization.rs:44,56`); panic on write outside range, `Err` on read. |
| `topic_profiles` | UUID (16B) | `14 × f32` = 56B (LE, `serialize_topic_profile`) | `CF_TOPIC_PROFILES` (l.76). `TOPIC_PROFILE_DIM = 14` (constants.rs:54). No compression (56B too small). Written by clustering pipeline only — NOT by `store_fingerprint_internal`. |
| `e13_splade_inverted` | `term_id: u16` (2B) | `Vec<UUID>` (bincode, sorted) | `CF_E13_SPLADE_INVERTED` (l.84). SPLADE 30,522-term BERT vocab. |
| `e6_sparse_inverted` | `term_id: u16` (2B) | `Vec<UUID>` (bincode, sorted) | `CF_E6_SPARSE_INVERTED` (l.97). E6 30,522 vocab; ~235 active terms/doc (0.77% sparsity). |
| `e1_matryoshka_128` | UUID (16B) | `128 × f32` = 512B (LE, `serialize_e1_matryoshka_128`) | `CF_E1_MATRYOSHKA_128` (l.106). E1 truncated to first 128 dims for Stage 2 fast filter. Durable backup; runtime path reads from in-memory HNSW. |
| `content` | UUID (16B) | UTF-8 (max 1MB, app-enforced) | `CF_CONTENT` (l.122). LZ4 only. |
| `source_metadata` | UUID (16B) | `SourceMetadata` JSON (~100-500B) | `CF_SOURCE_METADATA` (l.136). JSON, NOT bincode (per skip_serializing_if rule). |
| `file_index` | file_path bytes (UTF-8, var) | `FileIndexEntry` bincode (`Vec<UUID> + metadata`) | `CF_FILE_INDEX` (l.150). |
| `topic_portfolio` | `session_id` bytes OR `"__latest__"` | `PersistedTopicPortfolio` JSON (~1-50KB) | `CF_TOPIC_PORTFOLIO` (l.164). |
| `e12_late_interaction` | UUID (16B) | `Vec<Vec<f32>>` bincode (10-256KB) | `CF_E12_LATE_INTERACTION` (l.190). Per-token 128D ColBERT. |
| `entity_provenance` | `[canonical_id_bytes][uuid:16]` (var + 16B) | `EntityProvenance` bincode | `CF_ENTITY_PROVENANCE` (l.213). RESERVED (not yet populated). |
| `audit_log` | `[ts_nanos:u64 BE][op_id:16]` (24B) | `AuditRecord` JSON | `CF_AUDIT_LOG` (l.235). APPEND-ONLY. |
| `audit_by_target` | `[target:16][ts_nanos:u64 BE]` (24B) | primary key bytes (24B back-ref) | `CF_AUDIT_BY_TARGET` (l.250). 16B prefix extractor. APPEND-ONLY. |
| `merge_history` | `[merged_uuid:16][ts_nanos:u64 BE]` (24B) | `MergeRecord` JSON (~500-5KB) | `CF_MERGE_HISTORY` (l.272). PERMANENT (never expires). |
| `importance_history` | `[memory_uuid:16][ts_nanos:u64 BE]` (24B) | `ImportanceChangeRecord` JSON | `CF_IMPORTANCE_HISTORY` (l.286). PERMANENT. |
| `tool_call_index` | `tool_use_id` bytes (UTF-8) | `Vec<UUID>` bincode | `CF_TOOL_CALL_INDEX` (l.306). RESERVED. |
| `consolidation_recommendations` | `recommendation_uuid:16` | `ConsolidationRecommendation` bincode | `CF_CONSOLIDATION_RECOMMENDATIONS` (l.325). RESERVED. |
| `embedding_registry` | `fingerprint_uuid:16` | `EmbeddingVersionRecord` JSON | `CF_EMBEDDING_REGISTRY` (l.344). Current-state provenance, 16B prefix. Removed on hard-delete. |
| `custom_weight_profiles` | `profile_name` bytes (UTF-8) | `[f32; 14]` JSON (~100-200B) | `CF_CUSTOM_WEIGHT_PROFILES` (l.362). One profile per name; cap 1000. |
| `hnsw_graphs` | `graph:{embedder}` OR `meta:{embedder}` (UTF-8) | usearch serialized bytes (1-100MB) OR JSON `{ mappings, next_key }` | `CF_HNSW_GRAPHS` (l.377). NO bloom (few keys). 8MB WB. |
| `training_records` | UUID (16B) | `[v:u8 = 3][bincode TrainingRecord]` | `CF_TRAINING_RECORDS` (l.399). `TRAINING_RECORD_VERSION = 3` (core/training/mod.rs:67). 8MB WB. |
| `learning_events` | event UUID (16B) | `[v:u8 = 1][bincode LearningEvent]` | `CF_LEARNING_EVENTS` (l.416). `LEARNING_EVENT_VERSION = 1` (core/learning/mod.rs:18). |
| `learner_profile` | `learner_uuid:16` | `[v:u8 = 1][bincode LearnerProfile]` | `CF_LEARNER_PROFILE` (l.422). `LEARNER_RECORD_VERSION = 1` (core/learner/mod.rs:17). |
| `learner_constellations` | `[learner_id:16][selector_kind:u8]` (17B) | `[v:u8 = 1][bincode payload]` | `CF_LEARNER_CONSTELLATIONS` (l.428). |
| `fingerprints_learner` | `[learner_id:16][session_ts:u64 BE]` (24B) | `[v:u8 = 1][bincode LearnerFingerprint]` | `CF_FINGERPRINTS_LEARNER` (l.434). E15-E21 per-session. |
| `learner_m_per_trace` | `[learner_id:16][trace_id:16]` (32B) | `[v:u8 = 1][bincode LearnerMTrace]` | `CF_LEARNER_M_PER_TRACE` (l.440). |
| `learner_state_history` | `[learner_id:16][session_ts:u64 BE]` (24B) | `[v:u8 = 1][bincode LearnerStateVector]` | `CF_LEARNER_STATE_HISTORY` (l.446). |
| `learner_goal_states` | `[learner_id:16][skill_id:16]` (32B) | `[v:u8 = 1][bincode LearnerGoalState]` | `CF_LEARNER_GOAL_STATES` (l.452). |
| `learner_retrieval_log` | `[learner_id:16][trace_id:16][ts:u64 BE]` (40B) | `[v:u8 = 1][bincode LearnerRetrievalLog]` | `CF_LEARNER_RETRIEVAL_LOG` (l.458). |
| `learner_k_sleep` | `[learner_id:16][session_ts:u64 BE]` (24B) | `[v:u8 = 1][bincode LearnerKSleep]` | `CF_LEARNER_K_SLEEP` (l.464). |
| `goal_centroids` | `[skill_id:16][modality:u8]` (17B) | `[v:u8 = 1][bincode GoalCentroid]` | `CF_GOAL_CENTROIDS` (l.470). |
| `learner_delta_log` | `[learner_id:16][session_ts:u64 BE]` (24B) | `[v:u8 = 1][bincode LearnerDeltaLog]` | `CF_LEARNER_DELTA_LOG` (l.476). |
| `learner_audit` | `[learner_id:16][ts:u64 BE][audit_id:16]` (40B) | `[v:u8 = 1][bincode LearnerAuditEntry]` | `CF_LEARNER_AUDIT` (l.482). |
| `learner_training_datasets` | dataset UUID (16B) | `[v:u8 = 1][bincode LearnerTrainingDataset]` row-major `f32` matrix + provenance | `CF_LEARNER_TRAINING_DATASETS` (l.492). `LEARNER_TRAINING_DATASET_VERSION = 1`. |
| `constellations` | constellation UUID (16B) | `[v:u8 = 2][bincode Constellation]` | `CF_CONSTELLATIONS` (l.511). `CONSTELLATION_VERSION = 2`. Centroids may be 100s of KB. |
| `constellation_by_selector` | `[selector_kind:u8][SHA256(canonical_form)[..16]]` (17B) | constellation UUID (16B) | `CF_CONSTELLATION_BY_SELECTOR` (l.529). 1B prefix. |
| `contrastive_pairs` | `[anchor:16][negative:16]` (32B) | `[v:u8 = 1][bincode ContrastivePair]` | `CF_CONTRASTIVE_PAIRS` (l.546). 32B prefix; `CONTRASTIVE_PAIR_VERSION = 1`. |
| `contrastive_by_kind` | `[kind:u8][anchor:16][negative:16]` (33B) | empty | `CF_CONTRASTIVE_BY_KIND` (l.559). 1B prefix. |
| `contrastive_by_anchor` | `anchor:16` | `Vec<UUID>` bincode (deduped lazily) | `CF_CONTRASTIVE_BY_ANCHOR` (l.572). |
| `typed_edge_records` | `[source:16][target:16][edge_type:u8]` (33B) | `[v:u8 = 1][bincode TypedEdgeTrainingRecord]` | `CF_TYPED_EDGE_RECORDS` (l.592). `TYPED_EDGE_RECORD_VERSION = 1`. 33B prefix. |
| `typed_edge_validations` | `[source:16][target:16][edge_type:u8]` (33B) | `[v:u8 = 1][bincode LLMEdgeValidation]` | `CF_TYPED_EDGE_VALIDATIONS` (l.609). `LLM_EDGE_VALIDATION_VERSION = 1`. |

## 5. Quantized Embedder CFs (14, `emb_0`..`emb_13`)

Source: `crates/context-graph-storage/src/teleological/column_families.rs:670-746`. All share `quantized_embedder_cf_options` (4KB blocks, LZ4, 64MB point-lookup hint, 2MB WB). Key = UUID (16B); value = `QuantizedEmbedding` bincode. **Reserved — not yet wired into production write path; deletes pass through but no-op.**

| CF | ModelId | Embedder | Source dim | Quantization | Stored size |
| --- | --- | --- | --- | --- | --- |
| `emb_0` | 0 | E1_Semantic | 1024D | PQ-8 | 8B |
| `emb_1` | 1 | E2_TemporalRecent | 512D | Float8E4M3 | 512B |
| `emb_2` | 2 | E3_TemporalPeriodic | 512D | Float8E4M3 | 512B |
| `emb_3` | 3 | E4_TemporalPositional | 512D | Float8E4M3 | 512B |
| `emb_4` | 4 | E5_Causal (dual cause/effect) | 768D × 2 | PQ-8 | 8B (per side; both vectors quantized) |
| `emb_5` | 5 | E6_Sparse | 30522 vocab | SparseNative | variable |
| `emb_6` | 6 | E7_Code | 1536D | PQ-8 | 8B |
| `emb_7` | 7 | E8_Graph (dual source/target) | 1024D × 2 | Float8E4M3 | 1024B (per side) |
| `emb_8` | 8 | E9_HDC | 10000-bit binary | Binary | 1250B |
| `emb_9` | 9 | E10_Multimodal (dual paraphrase/context) | 768D × 2 | PQ-8 | 8B (per side) |
| `emb_10` | 10 | E11_Entity (Kepler) | 768D | PQ-8 | 8B |
| `emb_11` | 11 | E12_LateInteraction | per-token 128D | TokenPruning | ~50% |
| `emb_12` | 12 | E13_SPLADE | 30522 vocab | SparseNative | variable |
| `emb_13` | 13 | E14_BgeM3Dense | 1024D | PQ-8 | 8B |

## 6. Code CFs (5)

Source: `crates/context-graph-storage/src/teleological/column_families.rs:1825-1887`.

| Name | Key | Value | Options |
| --- | --- | --- | --- |
| `code_entities` | UUID (16B) | `CodeEntity` JSON (~500-5000B) | LZ4, 8KB blocks, 16B prefix, 64MB point-lookup hint, 4MB WB |
| `code_e7_embeddings` | UUID (16B) | `Vec<f32>` 1536D = 6144B | LZ4, 8KB blocks, 16B prefix, 128MB point-lookup hint, 4MB WB |
| `code_file_index` | file_path (UTF-8) | `CodeFileIndexEntry` JSON | LZ4, bloom, 2MB WB |
| `code_name_index` | entity_name (UTF-8) | `Vec<UUID>` JSON | LZ4, no bloom (prefix scans), 2MB WB |
| `code_signature_index` | `signature_hash: SHA256` (32B) | `Vec<UUID>` JSON | NO compression, bloom, 32MB point-lookup hint, 2MB WB |

CodeEntity / CodeFileIndexEntry / index lists use JSON ("metadata, JSON; vectors, bincode"). Each entity also stores a full `SemanticFingerprint` (kept by code subsystem) in bincode for multi-space code search via the same 14-embedder structure used by text.

## 7. Causal CFs (2)

Source: `crates/context-graph-storage/src/teleological/column_families.rs:2038-2099`.

| Name | Key | Value | Options |
| --- | --- | --- | --- |
| `causal_relationships` | UUID (16B) | `CausalRelationship` bincode (~5-15KB; text + dual E5 + LLMProvenance) | LZ4, 8KB blocks, bloom, 16B prefix, 64MB point-lookup hint, 4MB WB |
| `causal_by_source` | source fingerprint UUID (16B) | `Vec<UUID>` bincode | LZ4, bloom, 16B prefix, 2MB WB |

## 8. DynamicJEPA CFs (28)

Source: `crates/context-graph-storage/src/dynamicjepa/column_families.rs`. All use LZ4 + level compaction. Five reusable option profiles defined in this file:

- `dj_uuid_point_cf_options` — 8KB blocks, 4MB WB, 16B prefix, bloom (point lookups by UUID).
- `dj_composite_index_cf_options` — 8KB, 4MB WB, 16B prefix, bloom.
- `dj_secondary_index_cf_options` — 8KB, 1MB WB, 16B prefix, NO bloom.
- `dj_append_only_cf_options(prefix_len)` — 8KB, 4MB WB, configurable prefix, bloom.
- `dj_large_blob_cf_options` — 64KB, 8MB WB, 16B prefix, bloom.

Wire format: every value is `[record_version: u8][bincode payload]` via `encode_record`/`decode_record` (`dynamicjepa/encode.rs`); `DYNAMIC_JEPA_STORAGE_VALUE_VERSION = 1` (line 9). Mismatched version returns `DynamicJepaError::Codec` and explicitly does NOT migrate. The audit row uses `DJ_AUDIT_RECORD_VERSION = 2` (`audit.rs:7`).

| Name | Profile | Key encoder (`dynamicjepa/keys.rs`) | Key length | Description |
| --- | --- | --- | --- | --- |
| `dj_domain_packs` | uuid_point | `domain_pack_key(id, version)` | 16 | Domain-pack registry primary store |
| `dj_domain_pack_by_nv` | composite_index | `name_version_key(id, version)` | 32 | Lookup by `(name, version)` composite (BLAKE3-128 of name and version concatenated) |
| `dj_instrument_registry` | composite_index | `instrument_registry_key(domain_uuid, id)` | 32 | Per-pack instrument metadata |
| `dj_adapter_registry` | composite_index | `adapter_registry_key(domain_uuid, id)` | 32 | Per-pack adapter metadata |
| `dj_raw_events` | uuid_point | `uuid_key(event_id)` | 16 | Raw event ingestion |
| `dj_normalized_states` | uuid_point | `uuid_key(state_id)` | 16 | Normalized state snapshots |
| `dj_actions` | uuid_point | `uuid_key(action_id)` | 16 | Action records |
| `dj_outcomes` | uuid_point | `uuid_key(outcome_id)` | 16 | Outcome records |
| `dj_transitions` | uuid_point | `uuid_key(transition_id)` | 16 | State-action-outcome transitions |
| `dj_adapter_runs` | append_only(16) | `uuid_key(run_id)` | 16 | Append-only adapter execution log |
| `dj_instrument_readings` | composite_index | `instrument_reading_key(event_id, instrument_id)` | 32 | Instrument readings keyed by `(event, instrument)` |
| `dj_latent_panels` | large_blob | UUID (16B) | 16 | Per-panel latent vectors (large blobs) |
| `dj_pairwise_readings` | append_only(16) | `pairwise_reading_key(event, j, k)` | 24 | Pairwise reading log |
| `dj_constellations` | composite_index | `constellation_key(domain, subject, modality)` | 36 | Domain-pack constellations |
| `dj_threshold_calibrations` | composite_index | `threshold_calibration_key(domain, subject, modality, supersede_seq)` | 40 | Calibration tables |
| `dj_bindings` | uuid_point | `uuid_key(binding_id)` | 16 | Binding records |
| `dj_bindings_by_entity` | secondary_index | `binding_entity_key(entity_ref, binding_id)` | 32 | Reverse index by entity (no bloom) |
| `dj_trajectories` | uuid_point | UUID (16B) | 16 | Trajectory primary store |
| `dj_dataset_shards` | large_blob | `dataset_shard_key(dataset, shard)` | 32 | Dataset matrix shards |
| `dj_training_runs` | append_only(16) | `uuid_key(run_id)` | 16 | Training-run audit log |
| `dj_model_artifacts` | uuid_point | `uuid_key(artifact_id)` | 16 | Model artifact metadata |
| `dj_predictions` | append_only(16) | UUID (16B) | 16 | Prediction log |
| `dj_skill_policies` | uuid_point | UUID (16B) | 16 | Skill policy records |
| `dj_plan_traces` | append_only(16) | UUID (16B) | 16 | Plan trace log |
| `dj_guard_decisions` | append_only(16) | UUID (16B) | 16 | Guard decision log |
| `dj_surprise_events` | append_only(16) | UUID (16B) | 16 | Surprise event log |
| `dj_verification_runs` | append_only(16) | UUID (16B) | 16 | Verification run log |
| `dj_audit_log` | append_only(8) | `audit_key(ts_nanos_be, audit_id)` | 24 | DJ audit log; values prefixed by `DJ_AUDIT_RECORD_VERSION = 2` |

## 9. Schema Versioning (per-record version bytes)

There is no global `SCHEMA_VERSION` constant. Each record type has its own one-byte version prefix; deserialization rejects mismatches with a hard error (no migration support, see §13).

| Constant | Value | Source |
| --- | --- | --- |
| `TELEOLOGICAL_VERSION` | `1` | `teleological/serialization.rs:35` |
| `TRAINING_RECORD_VERSION` | `3` | `core/training/mod.rs:67` |
| `CONSTELLATION_VERSION` | `2` | `core/constellation/mod.rs:55` |
| `CONTRASTIVE_PAIR_VERSION` | `1` | `core/contrastive/mod.rs:49` |
| `TYPED_EDGE_RECORD_VERSION` | `1` | `core/typed_edge_export/mod.rs:32` |
| `LLM_EDGE_VALIDATION_VERSION` | `1` | `core/llm_edge_validation/mod.rs:25` |
| `LEARNING_EVENT_VERSION` | `1` | `core/learning/mod.rs:18` |
| `LEARNER_RECORD_VERSION` | `1` | `core/learner/mod.rs:17` (covers all 11 learner CFs) |
| `LEARNER_TRAINING_DATASET_VERSION` | `1` | `core/learner_training.rs:20` |
| `DYNAMIC_JEPA_STORAGE_VALUE_VERSION` | `1` | `dynamicjepa/encode.rs:9` (DJ payload bincode default) |
| `DJ_AUDIT_RECORD_VERSION` | `2` | `dynamicjepa/audit.rs:7` |

Phase-13 privacy/retention/erasure (`ledger_record_kind = privacy_decision`) records are written by the BlackBox CLI subsystem (`crates/context-graph-cli/src/commands/blackbox.rs`) into the BlackBox tenant ledger, NOT into a column family of this RocksDB schema. The BlackBox spec is in `docs/blackboxfix/` and is out of scope here.

## 10. Indexes

### 10.1 HNSW (16 indexes via `EmbedderIndexRegistry`)

`EmbedderIndex::all_hnsw()` (`teleological/indexes/hnsw_config/embedder.rs:186`) lists exactly 16 entries; `EmbedderIndexRegistry::new()` instantiates one per entry (`indexes/registry.rs:89`, asserted to 16 by tests).

| # | EmbedderIndex variant | Dim | Distance metric (see `metrics.rs`) |
| --- | --- | --- | --- |
| 1 | `E1Semantic` | 1024 | Cosine |
| 2 | `E1Matryoshka128` | 128 | Cosine (truncated E1) |
| 3 | `E2TemporalRecent` | 512 | Cosine |
| 4 | `E3TemporalPeriodic` | 512 | Cosine |
| 5 | `E4TemporalPositional` | 512 | Cosine |
| 6 | `E5Causal` | 768 | AsymmetricCosine (legacy active vector) |
| 7 | `E5CausalCause` | 768 | AsymmetricCosine (cause vector) |
| 8 | `E5CausalEffect` | 768 | AsymmetricCosine (effect vector) |
| 9 | `E7Code` | 1536 | Cosine |
| 10 | `E8Graph` | 1024 | Cosine (dual src/tgt stored, single index) |
| 11 | `E9HDC` | 1024 | Cosine (projected from 10K-bit) |
| 12 | `E10Multimodal` | 768 | AsymmetricCosine (legacy active vector) |
| 13 | `E10MultimodalParaphrase` | 768 | AsymmetricCosine (paraphrase side) |
| 14 | `E10MultimodalContext` | 768 | AsymmetricCosine (context side) |
| 15 | `E11Entity` | 768 | Cosine (KEPLER) |
| 16 | `E14BgeM3Dense` | 1024 | Cosine (BGE-M3 / XLM-RoBERTa-Large CLS) |

Non-HNSW (other index types):

| Embedder | Index type | CF |
| --- | --- | --- |
| `E6Sparse` | inverted index (BERT vocab 30,522 terms) | `e6_sparse_inverted` |
| `E12LateInteraction` | ColBERT MaxSim (token-level, no ANN) | `e12_late_interaction` |
| `E13Splade` | inverted index (BM25 over SPLADE 30,522 vocab) | `e13_splade_inverted` |

HNSW graphs persist into `CF_HNSW_GRAPHS` (key formats `graph:{embedder}` and `meta:{embedder}`). Persistence is periodic + on shutdown. See [09_storage_layer.md](09_storage_layer.md) §5 for the lifecycle.

### 10.2 Secondary indexes (RocksDB-only, posting lists)

| Index | Backing CF | Read-modify-write protected by |
| --- | --- | --- |
| Tag → UUID list | `tags` | implicit via `WriteBatch` |
| Source URI → UUID list | `sources` | implicit |
| Temporal | `temporal` | implicit |
| File path → UUID list | `file_index` | `secondary_index_lock` |
| Causal source → relationships | `causal_by_source` | `secondary_index_lock` |
| E13 SPLADE inverted | `e13_splade_inverted` | `secondary_index_lock` |
| E6 sparse inverted | `e6_sparse_inverted` | `secondary_index_lock` |
| Audit by target | `audit_by_target` | append-only |
| Constellation by selector | `constellation_by_selector` | n/a |
| Contrastive by kind | `contrastive_by_kind` | n/a |
| Contrastive by anchor | `contrastive_by_anchor` | n/a |
| Typed edges by type | `typed_edges_by_type` | direct write (atomic batch with `typed_edges`) |
| Code file index | `code_file_index` | direct write |
| Code name index | `code_name_index` | direct write |
| Code signature index | `code_signature_index` | direct write |

### 10.3 Composite Key Encodings (canonical reference)

| CF | Composite key | Total bytes | Prefix extractor |
| --- | --- | --- | --- |
| `embedder_edges` | `[embedder_id:u8][source:16]` | 17 | 1B (embedder_id) — value is the full posting list for this `(embedder, source)` |
| `typed_edges` | `[source:16][target:16]` | 32 | 16B (source UUID) |
| `typed_edges_by_type` (current) | `[edge_type:u8][source:16][target:16]` | 33 | 1B |
| `typed_edges_by_type` (legacy) | `[edge_type:u8][source:16]` | 17 | 1B |
| `audit_log` | `[ts_nanos:u64 BE][op_id:16]` | 24 | none (timestamp ordering) |
| `audit_by_target` | `[target:16][ts_nanos:u64 BE]` | 24 | 16B |
| `merge_history` | `[merged:16][ts_nanos:u64 BE]` | 24 | 16B |
| `importance_history` | `[memory:16][ts_nanos:u64 BE]` | 24 | 16B |
| `learner_constellations` | `[learner:16][selector_kind:u8]` | 17 | 16B |
| `fingerprints_learner` | `[learner:16][session_ts:u64 BE]` | 24 | 16B |
| `learner_m_per_trace` | `[learner:16][trace:16]` | 32 | 16B |
| `learner_state_history` | `[learner:16][session_ts:u64 BE]` | 24 | 16B |
| `learner_goal_states` | `[learner:16][skill:16]` | 32 | 16B |
| `learner_retrieval_log` | `[learner:16][trace:16][ts:u64 BE]` | 40 | 16B |
| `learner_k_sleep` | `[learner:16][session_ts:u64 BE]` | 24 | 16B |
| `goal_centroids` | `[skill:16][modality:u8]` | 17 | 16B |
| `learner_delta_log` | `[learner:16][session_ts:u64 BE]` | 24 | 16B |
| `learner_audit` | `[learner:16][ts:u64 BE][audit:16]` | 40 | 16B |
| `constellation_by_selector` | `[selector_kind:u8][SHA256_first16:16]` | 17 | 1B |
| `contrastive_pairs` | `[anchor:16][negative:16]` | 32 | 32B |
| `contrastive_by_kind` | `[kind:u8][anchor:16][negative:16]` | 33 | 1B |
| `typed_edge_records` | `[source:16][target:16][edge_type:u8]` | 33 | 33B |
| `typed_edge_validations` | `[source:16][target:16][edge_type:u8]` | 33 | 33B |
| `dj_domain_pack_by_nv` | `[blake3_128(name)][blake3_128(version)]` | 32 | 16B |
| `dj_instrument_registry` | `[domain_uuid:16][blake3_128(instrument_id)]` | 32 | 16B |
| `dj_pairwise_readings` | `[event:16][j:u32 LE][k:u32 LE]` | 24 | 16B |
| `dj_constellations` | `[domain:16][SHA256_first16(subject)][modality:u32 LE]` | 36 | 16B |
| `dj_threshold_calibrations` | `[constellation_key:36][supersede_seq:u32 LE]` | 40 | 16B |
| `dj_dataset_shards` | `[dataset:16][shard:16]` | 32 | 16B |
| `dj_audit_log` | `[ts_nanos:u64 BE][audit_id:16]` | 24 | 8B |

### 10.4 Soft-delete prefix in `CF_SYSTEM`

Source: `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs:32-39`.

| Element | Value |
| --- | --- |
| Prefix | `"soft_deleted::"` |
| Full key | `soft_deleted::{uuid_string}` (UTF-8) |
| Value | `i64` Unix epoch milliseconds, big-endian (8B) |
| GC retention | `TeleologicalStoreConfig.gc_retention_secs = 7 * 24 * 3600` (default 7 days, see `types.rs:180`) |
| Loaded at startup | yes; `total_doc_count` initialized to `raw - soft_deleted.len()` for IDF |
| Post-GC compaction | triggered when `>= GC_COMPACTION_THRESHOLD = 10` deletions in a single GC pass (`crud.rs:557`); compacts 6 primary CFs + all 14 quantized CFs |

## 11. Connection Management

`PidFileGuard` (`crates/context-graph-mcp/src/main.rs:578-797`):

- Writes the current PID to `<storage.path>/mcp.pid`.
- Holds `flock(LOCK_EX | LOCK_NB)` on Unix; on non-Unix, NO lock is acquired and an `E_PID_NO_LOCK` warning is logged (multiple instances WILL corrupt RocksDB).
- On contention (`EWOULDBLOCK`), reads the holder PID and runs:
  - If PID file is unreadable ⇒ refuse to start (`E_PID_READ_FAIL`) — cannot determine if holder is alive.
  - `kill(pid, 0) != 0` ⇒ holder process gone, lock is stale, reclaim.
  - Else, read `/proc/{pid}/status` for `State:\tZ` (zombie) or `State:\tX` (dead) ⇒ stale, reclaim.
  - Otherwise refuse to start with explicit kill instructions.
- On reclaim: drop the file, `sleep(100ms)`, reopen with `truncate(true)`, retry `flock`. On success, write current PID (with logged warnings on seek/truncate/write/flush failures) and return guard.
- On `Drop`, removes the PID file (best-effort; logs `E_PID_CLEANUP` warning on failure).
- RocksDB's own `LOCK` file is owned by RocksDB; `RocksDbTeleologicalStore::open_with_config` only inspects it via a non-blocking `fcntl(F_SETLK, F_WRLCK)` probe for diagnostics and never removes it (`store.rs:472-564`).
- Daemon-mode HTTP health check (`is_daemon_healthy_on_port`, `main.rs:955+`): a live PID does not prove the daemon is *responsive* — daemon mode also runs a JSON-RPC `tools/list` round-trip on the configured port and only treats the lock as legitimate when both checks pass. SIGTERM(2s grace) → SIGKILL eviction is run on a stale-but-alive holder.

`TeleologicalStoreConfig` defaults (`types.rs:172-183`):

| Field | Default | Purpose |
| --- | --- | --- |
| `block_cache_size` | `256 * 1024 * 1024` (256MB) | Shared block cache across all CFs |
| `max_open_files` | `1000` | RocksDB `set_max_open_files` |
| `enable_wal` | `true` | When false, sets `manual_wal_flush(true)` |
| `create_if_missing` | `true` | DB-level option |
| `create_missing_column_families` | `true` | DB-level option |
| `gc_retention_secs` | `7 * 24 * 3600` (7 days) | Soft-delete GC threshold |

In-memory caps (storage-side; not config-driven):

| Constant | Value | Source | Purpose |
| --- | --- | --- | --- |
| `MAX_HNSW_VECTORS_PER_INDEX` | `2_000_000` | `teleological/indexes/hnsw_impl/types.rs:18` | Per-HNSW-index hard cap (insert returns error when reached); ~16GB total at 1024D × f32 |
| `MAX_TOKENS_PER_MEMORY` | `512` | `teleological/search/token_storage.rs:41` | E12 ColBERT max tokens per memory; rejected at insert |
| `MAX_TOKEN_ENTRIES` | `5_000` | `teleological/search/pipeline/traits.rs:32` | `InMemoryTokenStorage` LRU cap |
| `MAX_SPLADE_DOCS` | `100_000` | `teleological/search/pipeline/traits.rs:158` | `InMemorySpladeIndex` LRU cap |
| `MAX_CHECKPOINTS` | `5` | `teleological/rocksdb_store/persistence.rs:394` | Auto-rotate `<path>/checkpoints/` retaining most recent 5 |
| `GC_COMPACTION_THRESHOLD` | `10` | `teleological/rocksdb_store/crud.rs:557` | Number of GC hard-deletes before triggering RocksDB `compact_range_cf` on 6 primary CFs + 14 quantized CFs |

## 12. RocksDB Option Profiles (Summary)

Derived from `crates/context-graph-storage/src/column_families.rs` and `teleological/column_families.rs`.

| Profile | Used by | Compression | Bloom | Prefix | Write buffer |
| --- | --- | --- | --- | --- | --- |
| `nodes_options` | `nodes`, fingerprint-shape CFs (also via `fingerprint_cf_options`) | LZ4 | 10 bits | none (point-lookup 256MB hint) | 4MB |
| `edges_options` | `edges` | LZ4 | 10 bits | 16B (source UUID) | 4MB |
| `embeddings_options` | `embeddings` | LZ4 | (cache index/filter) | none | 8MB (64KB blocks) |
| `index_options` | `metadata`, `temporal`, `tags`, `sources` | LZ4 | 10 bits | 16B | 2MB |
| `system_options` | `system` | NONE | none | none | 2MB |
| `embedder_edges_options` | `embedder_edges` | LZ4 | 10 bits | 1B | 4MB |
| `typed_edges_options` | `typed_edges` | LZ4 | 10 bits | 16B | 4MB |
| `typed_edges_by_type_options` | `typed_edges_by_type` | LZ4 | 10 bits | 1B | 2MB |
| `fingerprint_cf_options` | `fingerprints` | LZ4 | 10 bits | none (64KB blocks) | 8MB |
| `topic_profile_cf_options` | `topic_profiles` | NONE (56B) | 10 bits (point-lookup 64MB hint) | none | 2MB |
| `e13_splade_inverted_cf_options` / `e6_sparse_inverted_cf_options` | both inverted CFs | LZ4 | 10 bits | none | 4MB |
| `e1_matryoshka_128_cf_options` | `e1_matryoshka_128` | LZ4 | 10 bits | none (4KB blocks) | 2MB |
| `uuid_prefixed_cf_options(block_size, wb)` | `training_records`, `learning_events`, all `learner_*`, `learner_training_datasets`, `goal_centroids`, `fingerprints_learner` | LZ4 | 10 bits | 16B (level compaction) | parameterized |
| `hnsw_graphs_cf_options` | `hnsw_graphs` | LZ4 | NONE (few keys) | none | 8MB |
| `custom_weight_profiles_cf_options` | `custom_weight_profiles` | LZ4 | 10 bits | none | 2MB |
| `code_entities_cf_options` | `code_entities` | LZ4 | 10 bits (8KB blocks, 64MB point-lookup) | 16B | 4MB |
| `code_e7_embeddings_cf_options` | `code_e7_embeddings` | LZ4 | 10 bits (8KB blocks, 128MB point-lookup) | 16B | 4MB |
| `code_file_index_cf_options` | `code_file_index` | LZ4 | 10 bits | none | 2MB |
| `code_name_index_cf_options` | `code_name_index` | LZ4 | NONE | none | 2MB |
| `code_signature_index_cf_options` | `code_signature_index` | NONE | 10 bits (32MB point-lookup) | none | 2MB |
| `causal_relationships_cf_options` | `causal_relationships` | LZ4 | 10 bits (8KB blocks, 64MB point-lookup) | 16B | 4MB |
| `causal_by_source_cf_options` | `causal_by_source` | LZ4 | 10 bits | 16B | 2MB |
| DJ profiles | DynamicJEPA CFs | LZ4 (level compaction) | varies | varies | varies (see §8) |

Per-CF write-buffer limits are enforced by `apply_write_buffer_limits` (`column_families.rs:33`): `set_write_buffer_size(N * 1024 * 1024)` and `set_max_write_buffer_number(2)`.

## 13. Migrations

There are NO migrations. Per project policy ("pre-production, synthetic data only"):

- Each record type has a single accepted version (§9). Deserialization with a different version byte returns `CoreError::SerializationError("Version mismatch ... No automatic migration is supported - data must be regenerated")` (e.g. `serialization.rs:147-158`); DJ payloads return `DynamicJepaError::Codec` with the same "wipe and rebuild" remediation (`dynamicjepa/encode.rs:55`).
- Schema evolution = wipe DB and rebuild.
- New CFs added to `get_all_column_family_descriptors` are auto-created on next open thanks to `create_missing_column_families = true`. Removing a CF requires deleting the DB (RocksDB requires opening every previously-existing CF). `assert_existing_dynamicjepa_column_families` (`store.rs:368`) makes this explicit for the DJ family: it FAILs FAST if any of the 28 DJ CFs is missing from an existing DB rather than relying on the auto-create flag.

## 14. What Is NOT in This Schema

- BlackBox / privacy-retention / erasure ledger records (`docs/blackboxfix/`): they live in tenant SQLite ledgers, not in the RocksDB CFs above.
- `[graph]` (`max_edges_per_node`, `default_edge_weight`) values from `config/default.toml`: not modeled in the current Rust `Config`.
- 5090jepa demo packs (`counter_world`, `gridworld_5x5`): these are ML model packs, not CFs. They are loaded via the `dj_domain_packs` CF when ingested.
- HNSW M / ef-construction parameters per index: hard-coded in `teleological/indexes/hnsw_config/functions.rs`, not in `IndexConfig`.
