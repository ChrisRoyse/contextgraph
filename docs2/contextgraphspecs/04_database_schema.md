# Database Schema

> Source files covered:
> - `crates/context-graph-storage/src/lib.rs`
> - `crates/context-graph-storage/src/column_families.rs`
> - `crates/context-graph-storage/src/error.rs`
> - `crates/context-graph-storage/src/indexes.rs`
> - `crates/context-graph-storage/src/teleological/mod.rs`
> - `crates/context-graph-storage/src/teleological/column_families.rs`
> - `crates/context-graph-storage/src/teleological/schema.rs`
> - `crates/context-graph-storage/src/teleological/serialization.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/types.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs`
> - `crates/context-graph-storage/src/graph_edges/repository.rs`
> - `crates/context-graph-storage/src/graph_edges/serialization.rs`
> - `crates/context-graph-storage/src/code/store.rs`

Cross-refs: [03_configuration.md](03_configuration.md) · [09_storage_layer.md](09_storage_layer.md) · [05_embedding_subsystem.md](05_embedding_subsystem.md) · [07_graph_and_causal.md](07_graph_and_causal.md) · [13_teleological_subsystem.md](13_teleological_subsystem.md) · [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

---

## 1. Engine & Connection

**Engine:** RocksDB (via `rust-rocksdb`). Single-node, embedded, LSM-tree. No virtual tables, no extensions, no foreign tables.

### 1.1 DB-Level Options

| Option | Value | Source |
|---|---|---|
| `create_if_missing` | `config.create_if_missing` (default `true`) | `types.rs::TeleologicalStoreConfig` |
| `create_missing_column_families` | `true` | `store.rs::open_with_config` |
| `max_open_files` | `config.max_open_files` (default `1000`) | `types.rs` |
| `paranoid_checks` | `true` (bit-rot + SST corruption detection) | `store.rs` |
| `enable_wal` | `true` (default); `manual_wal_flush(true)` when `false` | `store.rs` |
| Block cache | `Cache::new_lru_cache(block_cache_size)` default **256 MB**, shared across all CFs | `store.rs` |
| GC retention | `gc_retention_secs` default **7 days** (soft-delete hard-purge window) | `types.rs` |

**Stale-lock handling:** `detect_and_remove_stale_lock()` runs before `DB::open_cf_descriptors`. It inspects the on-disk `LOCK` file, cross-checks the referenced PID via `/proc`, and removes the file when the holder is absent or a zombie.

**Corruption transform:** if `open_cf_descriptors` fails with "missing SST" style errors, `transform_corruption_error()` maps it to `TeleologicalStoreError::CorruptionDetected { path, missing_count, missing_files, manifest_file }` — no auto-repair, fail-fast.

### 1.2 Column Family Descriptor Assembly

`get_all_column_family_descriptors(cache)` = `get_column_family_descriptors(cache)` (11 base) ++ `teleological::get_all_cf_descriptors(cache)` (48 teleological group).

| Bundle | Fn | Count |
|---|---|---|
| Base | `column_families::get_column_family_descriptors` | 11 |
| Teleological | `teleological::column_families::get_teleological_cf_descriptors` | 28 |
| Quantized embedders | `teleological::column_families::get_quantized_embedder_cf_descriptors` | 13 |
| Code | `teleological::column_families::get_code_cf_descriptors` | 5 |
| Causal | `teleological::column_families::get_causal_cf_descriptors` | 2 |
| **Total** | `column_families::TOTAL_COLUMN_FAMILIES` | **59** |

All 59 CFs are opened on every start. RocksDB requires every on-disk CF to be registered during `open_cf_descriptors`; missing a CF from the descriptor list would fail the open.

### 1.3 Schema Versioning

Versioning is **per-record**, not database-wide. There is no single `PRAGMA user_version`. Each serialized record type carries a leading `u8` version byte; deserializers reject on mismatch with no automatic migration.

| Version constant | Applies to | Source |
|---|---|---|
| `TELEOLOGICAL_VERSION = 1` | `CF_FINGERPRINTS` bincode payload | `teleological/serialization.rs` |
| `GRAPH_EDGE_VERSION = 1` | `CF_EMBEDDER_EDGES`, `CF_TYPED_EDGES` payloads | `graph_edges/serialization.rs` |
| `TRAINING_RECORD_VERSION` | `CF_TRAINING_RECORDS` bincode payload | `rocksdb_store/export.rs` |
| `CONSTELLATION_VERSION` | `CF_CONSTELLATIONS` bincode payload | `rocksdb_store/constellation.rs` |
| `CONTRASTIVE_PAIR_VERSION` | `CF_CONTRASTIVE_PAIRS` bincode payload | `rocksdb_store/contrastive.rs` |
| `TYPED_EDGE_RECORD_VERSION` | `CF_TYPED_EDGE_RECORDS` bincode payload | `rocksdb_store/typed_edge_export.rs` |
| `LLM_EDGE_VALIDATION_VERSION` | `CF_TYPED_EDGE_VALIDATIONS` bincode payload | `rocksdb_store/llm_validation.rs` |

**Encoding rule:** all structs containing `Option<T>` or `#[serde(skip_serializing_if)]` MUST use JSON (bincode positional format corrupts field order). Fixed-shape structs use bincode.

---

## 2. CF Option Builder Profiles

| Profile fn | CFs | Block size | Compression | Bloom | Prefix | Write buffer |
|---|---|---:|---|---|---|---:|
| `nodes_options` | `nodes` | default | LZ4 | 10 bits | — (point lookup 256MB hint) | 4 MB |
| `edges_options` | `edges` | default | LZ4 | 10 bits | 16 B (UUID) | 4 MB |
| `embeddings_options` | `embeddings` | 64 KB | LZ4 | — | — | 8 MB |
| `index_options` | `metadata`, `temporal`, `tags`, `sources` | default | LZ4 | 10 bits | 16 B | 2 MB |
| `system_options` | `system` | default | **none** | — | — | 2 MB |
| `embedder_edges_options` | `embedder_edges` | default | LZ4 | 10 bits | 1 B (embedder_id) | 4 MB |
| `typed_edges_options` | `typed_edges` | default | LZ4 | 10 bits | 16 B (source UUID) | 4 MB |
| `typed_edges_by_type_options` | `typed_edges_by_type` | default | LZ4 | 10 bits | 1 B (edge_type) | 2 MB |
| `fingerprint_cf_options` | `fingerprints` | 64 KB | LZ4 | 10 bits | — | 8 MB |
| `topic_profile_cf_options` | `topic_profiles` | default | **none** | 10 bits | — (point 64MB) | 2 MB |
| `e13_splade_inverted_cf_options` | `e13_splade_inverted` | default | LZ4 | 10 bits | — | 4 MB |
| `e6_sparse_inverted_cf_options` | `e6_sparse_inverted` | default | LZ4 | 10 bits | — | 4 MB |
| `e1_matryoshka_128_cf_options` | `e1_matryoshka_128` | 4 KB | LZ4 | 10 bits | — | 2 MB |
| `content_cf_options` | `content` | default | LZ4 | — | — (level compaction) | 8 MB |
| `source_metadata_cf_options` | `source_metadata` | default | LZ4 | 10 bits | — (point 32MB) | 2 MB |
| `file_index_cf_options` | `file_index` | default | LZ4 | 10 bits | — | 2 MB |
| `topic_portfolio_cf_options` | `topic_portfolio` | default | LZ4 | 10 bits | — (point 32MB) | 2 MB |
| `e12_late_interaction_cf_options` | `e12_late_interaction` | 32 KB | LZ4 | 10 bits | 16 B | 4 MB |
| `entity_provenance_cf_options` | `entity_provenance` | default | LZ4 | 10 bits | — | 2 MB |
| `audit_log_cf_options` | `audit_log` | default | LZ4 | 10 bits | — (level compaction) | 4 MB |
| `audit_by_target_cf_options` | `audit_by_target` | default | LZ4 | 10 bits | 16 B | 2 MB |
| `merge_history_cf_options` | `merge_history` | default | LZ4 | 10 bits | — (level compaction) | 2 MB |
| `importance_history_cf_options` | `importance_history` | default | LZ4 | 10 bits | — (level compaction) | 2 MB |
| `tool_call_index_cf_options` | `tool_call_index` | default | LZ4 | 10 bits | — | 2 MB |
| `consolidation_recommendations_cf_options` | `consolidation_recommendations` | default | LZ4 | 10 bits | 16 B (point 32MB) | 2 MB |
| `embedding_registry_cf_options` | `embedding_registry` | default | LZ4 | 10 bits | 16 B (point 32MB) | 2 MB |
| `custom_weight_profiles_cf_options` | `custom_weight_profiles` | default | LZ4 | 10 bits | — | 2 MB |
| `hnsw_graphs_cf_options` | `hnsw_graphs` | default | LZ4 | — | — | 8 MB |
| `training_records_cf_options` | `training_records` | 64 KB | LZ4 | 10 bits | 16 B (level compaction) | 8 MB |
| `constellations_cf_options` | `constellations` | 64 KB | LZ4 | 10 bits | 16 B (level compaction) | 8 MB |
| `constellation_by_selector_cf_options` | `constellation_by_selector` | default | LZ4 | 10 bits | 1 B (selector kind) | 2 MB |
| `contrastive_pairs_cf_options` | `contrastive_pairs` | 16 KB | LZ4 | 10 bits | 32 B (anchor+neg, level compaction) | 4 MB |
| `contrastive_by_kind_cf_options` | `contrastive_by_kind` | default | LZ4 | 10 bits | 1 B (anomaly kind) | 2 MB |
| `contrastive_by_anchor_cf_options` | `contrastive_by_anchor` | default | LZ4 | 10 bits | 16 B (anchor UUID) | 2 MB |
| `typed_edge_records_cf_options` | `typed_edge_records` | 16 KB | LZ4 | 10 bits | 33 B (source+target+type, level compaction) | 4 MB |
| `typed_edge_validations_cf_options` | `typed_edge_validations` | default | LZ4 | 10 bits | 33 B (level compaction) | 2 MB |
| `quantized_embedder_cf_options` | `emb_0`..`emb_12` | 4 KB | LZ4 | 10 bits | — (point 64MB) | 2 MB |
| `code_entities_cf_options` | `code_entities` | 8 KB | LZ4 | 10 bits | 16 B (point 64MB) | 4 MB |
| `code_e7_embeddings_cf_options` | `code_e7_embeddings` | 8 KB | LZ4 | 10 bits | 16 B (point 128MB) | 4 MB |
| `code_file_index_cf_options` | `code_file_index` | default | LZ4 | 10 bits | — | 2 MB |
| `code_name_index_cf_options` | `code_name_index` | default | LZ4 | — | — | 2 MB |
| `code_signature_index_cf_options` | `code_signature_index` | default | **none** | 10 bits | — (point 32MB) | 2 MB |
| `causal_relationships_cf_options` | `causal_relationships` | 8 KB | LZ4 | 10 bits | 16 B (point 64MB) | 4 MB |
| `causal_by_source_cf_options` | `causal_by_source` | default | LZ4 | 10 bits | 16 B (point 32MB) | 2 MB |

All writers share `apply_write_buffer_limits(opts, mb)` which sets `write_buffer_size = mb * 1MB` and `max_write_buffer_number = 2`.

---

## 3. Base CFs (11)

Declared in `column_families.rs`. Constants in `column_families::cf_names` (all `&str`). Declared enum order in `cf_names::ALL`.

| # | Constant | Name | Key | Value | Purpose |
|---:|---|---|---|---|---|
| 0 | `cf_names::NODES` | `nodes` | UUID (16 B) | legacy JSON `MemoryNode` | Primary node storage (legacy API path). Unused by active write path but opened for back-compat. |
| 1 | `cf_names::EDGES` | `edges` | UUID (16 B) | legacy JSON `GraphEdge` | Legacy edge storage. |
| 2 | `cf_names::EMBEDDINGS` | `embeddings` | UUID (16 B) | `Vec<f32>` (1536 × 4 = 6144 B) | Legacy single-embedder vectors. |
| 3 | `cf_names::METADATA` | `metadata` | UUID (16 B) | JSON metadata | Legacy per-node metadata. |
| 4 | `cf_names::TEMPORAL` | `temporal` | `[timestamp_ms:8B BE][UUID:16B]` | UUID ref | Time-ordered secondary index. |
| 5 | `cf_names::TAGS` | `tags` | `[tag_bytes][UUID:16B]` | UUID ref | Tag → node index. |
| 6 | `cf_names::SOURCES` | `sources` | `[source_uri_bytes][UUID:16B]` | UUID ref | Source URI → node index. |
| 7 | `cf_names::SYSTEM` | `system` | UTF-8 string | bytes | DB-wide metadata. Holds soft-delete markers under `SOFT_DELETE_PREFIX` (value = 8-byte BE `i64` timestamp ms). |
| 8 | `cf_names::EMBEDDER_EDGES` | `embedder_edges` | `[embedder_id:u8][source:16B]` = 17 B | `[v:u8=1][count:u32 LE][(target:16B, sim:f32 LE)…]` | K-NN edges per embedder (k ≤ 20). |
| 9 | `cf_names::TYPED_EDGES` | `typed_edges` | `[source:16B][target:16B]` = 32 B | `[v:u8=1][bincode TypedEdge]` | Multi-relation typed edges. |
| 10 | `cf_names::TYPED_EDGES_BY_TYPE` | `typed_edges_by_type` | `[edge_type:u8][source:16B][target:16B]` = 33 B | target UUID (16 B) | Secondary index for `(type, source)` prefix scans. |

---

## 4. Teleological CFs (28)

Declared in `teleological/column_families.rs`. Re-exported via `teleological::mod.rs`. Listed in `TELEOLOGICAL_CFS` (count = `TELEOLOGICAL_CF_COUNT = 28`).

### 4.1 Fingerprints, topics, content

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_FINGERPRINTS` | `fingerprints` | UUID (16 B) | `[v:u8=1][bincode TeleologicalFingerprint]` ~55–110 KB (validated 5 KB–150 KB) | Primary memory store. Contains `SemanticFingerprint` (13 embedders), `TopicProfile`, metadata. |
| `CF_TOPIC_PROFILES` | `topic_profiles` | UUID (16 B) | 13 × f32 = 52 B | Fast access to per-memory 13D topic profile without loading full fingerprint. |
| `CF_CONTENT` | `content` | UUID (16 B) | raw UTF-8 (≤ 1 MB, app-enforced) | Original text payload. |
| `CF_SOURCE_METADATA` | `source_metadata` | UUID (16 B) | JSON `SourceMetadata` (~100–500 B) | Provenance: source URI, session_id, file path, tool_use_id. |
| `CF_FILE_INDEX` | `file_index` | UTF-8 file path | bincode `FileIndexEntry { fingerprint_ids: Vec<Uuid>, … }` (~100–2000 B) | File path → UUIDs for file-watcher reconciliation. |
| `CF_TOPIC_PORTFOLIO` | `topic_portfolio` | UTF-8 `session_id` or `__latest__` | JSON `PersistedTopicPortfolio` (~1–50 KB) | Session-scoped topic snapshot. |

### 4.2 Sparse/dense sidecar CFs

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_E13_SPLADE_INVERTED` | `e13_splade_inverted` | `term_id: u16 BE` = 2 B | bincode `Vec<Uuid>` posting list | E13 SPLADE inverted index (vocab 30,522). |
| `CF_E6_SPARSE_INVERTED` | `e6_sparse_inverted` | `term_id: u16 BE` = 2 B | bincode `Vec<Uuid>` posting list | E6 BM25 inverted index (vocab 30,522). |
| `CF_E1_MATRYOSHKA_128` | `e1_matryoshka_128` | UUID (16 B) | `[v:u8=1][128 × f32 = 512 B]` | Truncated E1 128D for coarse ANN. |
| `CF_E12_LATE_INTERACTION` | `e12_late_interaction` | UUID (16 B) | bincode `Vec<Vec<f32>>` (tokens × 128D, 10–25 KB typical, max 256 KB) | E12 ColBERT token-level embeddings for MaxSim. |

### 4.3 HNSW graph persistence

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_HNSW_GRAPHS` | `hnsw_graphs` | UTF-8 `graph:{EmbedderDebugName}` | usearch-serialized bytes (1–100 MB) | Persisted HNSW graph per embedder. |
| `CF_HNSW_GRAPHS` | `hnsw_graphs` | UTF-8 `meta:{EmbedderDebugName}` | JSON `{ id_to_key, key_to_id, next_key, count }` | UUID↔usearch-key bidirectional map. |

### 4.4 Provenance / audit (append-only)

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_AUDIT_LOG` | `audit_log` | `[timestamp_nanos:8B BE][uuid:16B]` = 24 B | JSON `AuditRecord` (~200–2000 B) | Append-only provenance log. |
| `CF_AUDIT_BY_TARGET` | `audit_by_target` | `[target_uuid:16B][timestamp_nanos:8B BE]` = 24 B | primary key of `CF_AUDIT_LOG` (24 B) | Secondary index for "all events on X". |
| `CF_MERGE_HISTORY` | `merge_history` | `[merged_uuid:16B][timestamp_nanos:8B BE]` = 24 B | JSON `MergeRecord` (~500–5000 B) | Permanent merge lineage. |
| `CF_IMPORTANCE_HISTORY` | `importance_history` | `[memory_uuid:16B][timestamp_nanos:8B BE]` = 24 B | JSON `ImportanceChangeRecord` (~100–500 B) | Permanent importance-change log. |
| `CF_EMBEDDING_REGISTRY` | `embedding_registry` | UUID (16 B) | JSON `EmbeddingVersionRecord` (~200–500 B) | Per-fingerprint per-embedder model version tracking. |
| `CF_ENTITY_PROVENANCE` | `entity_provenance` | `[canonical_id_bytes][memory_uuid:16B]` (variable + 16 B) | bincode `EntityProvenance` (~200–2000 B) | **Reserved — not yet populated.** Opened for forward compat. |
| `CF_TOOL_CALL_INDEX` | `tool_call_index` | UTF-8 `tool_use_id` | bincode `Vec<Uuid>` (~16–160 B) | **Reserved — not yet populated.** |
| `CF_CONSOLIDATION_RECOMMENDATIONS` | `consolidation_recommendations` | UUID (16 B) | bincode `ConsolidationRecommendation` (~500–5000 B) | **Reserved — not yet populated.** |

### 4.5 Search & profile config

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_CUSTOM_WEIGHT_PROFILES` | `custom_weight_profiles` | UTF-8 profile name | JSON `[f32; 13]` weights + metadata (~100–200 B) | User-defined search weight profiles. |

### 4.6 Training-data factory CFs (Phase 2/3 + typed edges)

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_TRAINING_RECORDS` | `training_records` | UUID (16 B) | `[v:u8][bincode TrainingRecord]` (tens–hundreds of KB) | One row per memory for training export. |
| `CF_CONSTELLATIONS` | `constellations` | constellation UUID (16 B) | `[v:u8][bincode Constellation]` (~hundreds of KB) | Compiled centroids for a selector (topic/session/tag/ids). |
| `CF_CONSTELLATION_BY_SELECTOR` | `constellation_by_selector` | `[selector_kind:u8][sha256_hash[0..16]]` = 17 B | constellation UUID (16 B) | Selector → constellation ID lookup. |
| `CF_CONTRASTIVE_PAIRS` | `contrastive_pairs` | `[anchor:16B][negative:16B]` = 32 B | `[v:u8][bincode ContrastivePair]` (~1–5 KB) | Mined (anchor, negative, profile) triples. |
| `CF_CONTRASTIVE_BY_KIND` | `contrastive_by_kind` | `[kind:u8][anchor:16B][negative:16B]` = 33 B | empty value (presence-only) | Anomaly-kind index over pairs. |
| `CF_CONTRASTIVE_BY_ANCHOR` | `contrastive_by_anchor` | anchor UUID (16 B) | bincode `Vec<Uuid>` of negatives | Per-anchor negatives roster. |
| `CF_TYPED_EDGE_RECORDS` | `typed_edge_records` | `[source:16B][target:16B][edge_type:u8]` = 33 B | `[v:u8][bincode TypedEdgeTrainingRecord]` (~2–10 KB) | Typed-edge training rows (content + embedder scores + metadata joins). |
| `CF_TYPED_EDGE_VALIDATIONS` | `typed_edge_validations` | `[source:16B][target:16B][edge_type:u8]` = 33 B | `[v:u8][bincode LLMEdgeValidation]` (~200–500 B) | LLM verdicts on typed edges. |

---

## 5. Quantized Embedder CFs (13)

`QUANTIZED_EMBEDDER_CFS`, count = `QUANTIZED_EMBEDDER_CF_COUNT = 13`. All use `quantized_embedder_cf_options`. Reserved — not yet wired into production write path; opened for forward compat.

| Constant | Name | ModelId | Quantization | Value size | Source dim |
|---|---|---:|---|---:|---:|
| `CF_EMB_0`  | `emb_0`  | 0 E1_Semantic | PQ-8 | ~8 B | 1024 |
| `CF_EMB_1`  | `emb_1`  | 1 E2_TemporalRecent | Float8E4M3 | 512 B | 512 |
| `CF_EMB_2`  | `emb_2`  | 2 E3_TemporalPeriodic | Float8E4M3 | 512 B | 512 |
| `CF_EMB_3`  | `emb_3`  | 3 E4_TemporalPositional | Float8E4M3 | 512 B | 512 |
| `CF_EMB_4`  | `emb_4`  | 4 E5_Causal | PQ-8 | ~8 B | 768 |
| `CF_EMB_5`  | `emb_5`  | 5 E6_Sparse | SparseNative | variable (~2 KB) | 30,522 vocab |
| `CF_EMB_6`  | `emb_6`  | 6 E7_Code | PQ-8 | ~8 B | 1536 |
| `CF_EMB_7`  | `emb_7`  | 7 E8_Graph | Float8E4M3 | 1024 B | 1024 |
| `CF_EMB_8`  | `emb_8`  | 8 E9_HDC | Binary | ~1250 B | 10,000 binary |
| `CF_EMB_9`  | `emb_9`  | 9 E10_Multimodal | PQ-8 | ~8 B | 768 |
| `CF_EMB_10` | `emb_10` | 10 E11_Entity | Float8E4M3 | 768 B | 768 |
| `CF_EMB_11` | `emb_11` | 11 E12_LateInteraction | TokenPruning | variable (~2 KB) | 128/token |
| `CF_EMB_12` | `emb_12` | 12 E13_SPLADE | SparseNative | variable (~2 KB) | 30,522 vocab |

Key: UUID (16 B) per row. Value: bincode `QuantizedEmbedding`.

---

## 6. Code CFs (5)

`CODE_CFS`, count = `CODE_CF_COUNT = 5`. Separate from teleological storage — code uses AST-based chunking and E7 (Qodo-Embed-1-1.5B, 1536D) as primary.

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_CODE_ENTITIES` | `code_entities` | UUID (16 B) | bincode `CodeEntity` (~500–5000 B) | Parsed AST entity: lang, kind, signature, span, parent. |
| `CF_CODE_E7_EMBEDDINGS` | `code_e7_embeddings` | UUID (16 B) | `Vec<f32>` (1536 × 4 = 6144 B) | E7 dense code embedding. |
| `CF_CODE_FILE_INDEX` | `code_file_index` | UTF-8 file path | bincode `CodeFileIndexEntry` | File path → entity UUIDs for cleanup. |
| `CF_CODE_NAME_INDEX` | `code_name_index` | UTF-8 entity name | bincode `Vec<Uuid>` | Name prefix search. |
| `CF_CODE_SIGNATURE_INDEX` | `code_signature_index` | SHA-256 hash (32 B) | bincode `Vec<Uuid>` | Signature-hash lookup. |

---

## 7. Causal CFs (2)

`CAUSAL_CFS`, count = `CAUSAL_CF_COUNT = 2`.

| Constant | Name | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_CAUSAL_RELATIONSHIPS` | `causal_relationships` | UUID (16 B) | bincode `CausalRelationship` (~5–15 KB) | LLM-authored cause→effect description with E1 1024D embedding + provenance. |
| `CF_CAUSAL_BY_SOURCE` | `causal_by_source` | source fingerprint UUID (16 B) | bincode `Vec<Uuid>` | "All causal relationships originating at memory X". |

---

## 8. Key Helpers (schema.rs)

All in `teleological/schema.rs`. FAIL-FAST on key length mismatch.

| Fn | In | Out | CF |
|---|---|---|---|
| `fingerprint_key(&Uuid)` | UUID | `[u8; 16]` | `fingerprints` |
| `parse_fingerprint_key(&[u8])` | 16 B | UUID | `fingerprints` |
| `topic_profile_key(&Uuid)` | UUID | `[u8; 16]` | `topic_profiles` |
| `parse_topic_profile_key(&[u8])` | 16 B | UUID | `topic_profiles` |
| `content_key(&Uuid)` | UUID | `[u8; 16]` | `content` |
| `parse_content_key(&[u8])` | 16 B | UUID | `content` |
| `source_metadata_key(&Uuid)` | UUID | `[u8; 16]` | `source_metadata` |
| `e12_late_interaction_key(&Uuid)` | UUID | `[u8; 16]` | `e12_late_interaction` |
| `e1_matryoshka_128_key(&Uuid)` | UUID | `[u8; 16]` | `e1_matryoshka_128` |
| `parse_e1_matryoshka_key(&[u8])` | 16 B | UUID | `e1_matryoshka_128` |
| `e13_splade_inverted_key(u16)` | term_id | `[u8; 2]` BE | `e13_splade_inverted` |
| `parse_e13_splade_key(&[u8])` | 2 B | `u16` | `e13_splade_inverted` |
| `e6_sparse_inverted_key(u16)` | term_id | `[u8; 2]` BE | `e6_sparse_inverted` |
| `parse_e6_sparse_key(&[u8])` | 2 B | `u16` | `e6_sparse_inverted` |
| `session_temporal_key(i64)` | ts_ms | `b"t:" + 8B BE` | `system`/per-session scans |
| `causal_relationship_key(&Uuid)` | UUID | `[u8; 16]` | `causal_relationships` |
| `causal_by_source_key(&Uuid)` | UUID | `[u8; 16]` | `causal_by_source` |

Compound keys constructed inline in repositories:

| Key pattern | Bytes | CF |
|---|---:|---|
| `[embedder_id:u8][source:16B]` | 17 | `embedder_edges` (via `EdgeStorageKey::new` / `embedder_prefix`) |
| `[source:16B][target:16B]` | 32 | `typed_edges` (via `TypedEdgeStorageKey::new`) |
| `[edge_type:u8][source:16B][target:16B]` | 33 | `typed_edges_by_type` |
| `[timestamp_nanos:8B BE][uuid:16B]` | 24 | `audit_log` |
| `[uuid:16B][timestamp_nanos:8B BE]` | 24 | `audit_by_target`, `merge_history`, `importance_history` |
| `[anchor:16B][negative:16B]` | 32 | `contrastive_pairs` |
| `[kind:u8][anchor:16B][negative:16B]` | 33 | `contrastive_by_kind` |
| `[source:16B][target:16B][edge_type:u8]` | 33 | `typed_edge_records`, `typed_edge_validations` |
| `[selector_kind:u8][sha256[..16]]` | 17 | `constellation_by_selector` |
| `graph:{name}` / `meta:{name}` | variable | `hnsw_graphs` |

---

## 9. Encoding Summary

| Encoder | Used for | Notes |
|---|---|---|
| **bincode 1.3** | `TeleologicalFingerprint`, `TypedEdge`, `TrainingRecord`, `Constellation`, `ContrastivePair`, `TypedEdgeTrainingRecord`, `LLMEdgeValidation`, `CodeEntity`, `CausalRelationship`, posting lists `Vec<Uuid>` | Fixed-shape structs only. Never use with `#[serde(skip_serializing_if)]` or optional reordering — positional format silently corrupts. |
| **JSON (serde_json)** | `SourceMetadata`, `AuditRecord`, `MergeRecord`, `ImportanceChangeRecord`, `EmbeddingVersionRecord`, `[f32;13]` weight profiles, HNSW meta map, `PersistedTopicPortfolio` | Anything with optional fields / provenance. |
| **Raw UTF-8** | `content` values, string keys | No framing. |
| **Raw little-endian f32** | `e1_matryoshka_128` value body, `embedder_edges` similarity field | After 1-byte version header. |
| **Big-endian u16 / i64** | `e6/e13 term_id` keys, timestamps in audit/merge/importance/temporal keys | Ensures lexicographic = chronological. |

Version-byte framing: bincode payloads always leading `u8` version; JSON payloads are unframed.

---

## 10. Secondary Indexes (Summary)

| Primary CF | Secondary CF(s) | Maintained by | Rebuild |
|---|---|---|---|
| `fingerprints` | `topic_profiles`, `e1_matryoshka_128`, `e13_splade_inverted`, `e6_sparse_inverted`, `source_metadata`, `content`, `file_index`, `embedder_edges`, `typed_edges`, `typed_edges_by_type` | `store_fingerprint_internal`, `store_typed_edge` | `rebuild_indexes_from_store()` + `BackgroundGraphBuilder` |
| `typed_edges` | `typed_edges_by_type` | `EdgeRepository::store_typed_edge` (WriteBatch) | N/A (derived on insert) |
| `audit_log` | `audit_by_target` | `append_audit_record` (WriteBatch) | N/A |
| `causal_relationships` | `causal_by_source` | `store_causal_relationship` (behind `secondary_index_lock`) | N/A |
| `constellations` | `constellation_by_selector` | `store_constellation` | N/A |
| `contrastive_pairs` | `contrastive_by_kind`, `contrastive_by_anchor` | `store_contrastive_pair` | N/A |
| `file_index` | (none — is itself secondary) | `index_file_fingerprint_async` (behind `secondary_index_lock`) | N/A |

Soft-delete entries live in `CF_SYSTEM` under `SOFT_DELETE_PREFIX`; loaded into `DashMap<Uuid, i64>` at startup before any HNSW rebuild.

---

## 11. Error Hierarchy (Storage Layer)

| Type | Where | Variants / notes |
|---|---|---|
| `StorageError` (`error.rs`) | Public storage API | `OpenFailed`, `ColumnFamilyNotFound`, `WriteFailed`, `ReadFailed`, `FlushFailed`, `NotFound { id }`, `Serialization`, `ValidationFailed`, `IndexCorrupted { index_name, details }`, `Internal`. `From<ValidationError>`, `From<rocksdb::Error>`. |
| `TeleologicalStoreError` (`rocksdb_store/types.rs`) | Teleological store | `RocksDbOperation { operation, cf, key, source }`, `OpenFailed`, `ColumnFamilyNotFound`, `Serialization { id, message }`, `Deserialization`, `Validation`, `IndexOperation`, `CheckpointFailed`, `RestoreFailed`, `StaleLockCleanupFailed`, **`CorruptionDetected { path, missing_count, missing_files, manifest_file }`** (fail-fast, no auto-repair), `Internal`, `ComparisonValidation(#[from])`. Bidirectional `From` with `CoreError`. |
| `GraphEdgeStorageError` (`graph_edges/types.rs`) | K-NN + typed edges | `RocksDb { operation, column_family, source }`, `Serialization`, `Deserialization`, `ColumnFamilyNotFound`, `InvalidEmbedderId` (0–12 only), `InvalidKeyFormat { expected, actual }`, `EdgeNotFound`, `NotInitialized`, `GraphBuildError`. |
| `CodeStorageError` (`code/error.rs`) | Code store | `RocksDb(#[from])`, `Serialization`, `Deserialization`, `NotFound { id }`, `EmbeddingNotFound`, `FileNotFound { path }`, `InvalidDimension { expected, actual }`, `ColumnFamilyNotFound`, `NotInitialized`, `InvalidKey`, `Duplicate`, `Io(#[from])`. |
| `QuantizedStorageError` (`teleological/quantized`) | Quantized trait | Per-embedder storage errors; wraps IO + serialization. |
| `IndexError` (`teleological/indexes/embedder_index.rs`) | Per-embedder HNSW | `DimensionMismatch { embedder, expected, actual }`, `IndexNotFound { embedder }`, `OperationFailed { embedder, message }`, `ReadOnly`, `InvalidVector { message }`. |

`StorageResult<T>`, `TeleologicalStoreResult<T>`, `GraphEdgeStorageResult<T>`, `CodeStorageResult<T>`, `QuantizedStorageResult<T>`, `IndexResult<T>` are `Result<T, *Error>` aliases.

---

## 12. Invariants

- `TOTAL_COLUMN_FAMILIES == 59` (asserted in tests). Any CF change must update the constant and the descriptor assembly.
- Every CF listed in `cf_names::ALL`, `TELEOLOGICAL_CFS`, `QUANTIZED_EMBEDDER_CFS`, `CODE_CFS`, `CAUSAL_CFS` must have a descriptor produced by the matching `get_*_cf_descriptors` function.
- All 15 HNSW-capable embedders (E1, E1-Matryoshka-128, E2, E3, E4, E5, E7, E8, E9, E10, E11 + E5CausalCause, E5CausalEffect, E10MultimodalParaphrase, E10MultimodalContext) are registered in `EmbedderIndexRegistry::new()`. E6, E12, E13 are not HNSW (inverted / MaxSim).
- All writes to inverted indexes, `causal_by_source`, and `file_index` go through `secondary_index_lock` (parking_lot `Mutex<()>`) to prevent lost-update races.
- All HNSW structural changes (rebuild, compaction) take the `compaction_lock` write-lock; per-vector insert/delete take the read-lock.
- `TeleologicalFingerprint` bincode payloads are bounded `[5_000, 150_000]` bytes at serialize time (panic outside range).
- Keys for posting-list / UUID-body CFs are validated at parse time with panic-on-mismatch (fail-fast).
