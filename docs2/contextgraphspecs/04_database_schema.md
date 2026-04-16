# Database Schema

> Source files covered:
> - `crates/context-graph-storage/src/lib.rs`
> - `crates/context-graph-storage/src/column_families.rs`
> - `crates/context-graph-storage/src/indexes.rs`
> - `crates/context-graph-storage/src/teleological/mod.rs`
> - `crates/context-graph-storage/src/teleological/column_families.rs`
> - `crates/context-graph-storage/src/teleological/schema.rs`
> - `crates/context-graph-storage/src/teleological/serialization.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/types.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/content.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/audit_log.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/causal_relationships.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/index_ops.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/persistence.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs`
> - `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs`
> - `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs`
> - `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs`
> - `crates/context-graph-storage/src/teleological/indexes/hnsw_config/distance.rs`
> - `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs`
> - `crates/context-graph-storage/src/teleological/indexes/hnsw_config/functions.rs`
> - `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/types.rs`
> - `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/ops.rs`
> - `crates/context-graph-storage/src/teleological/indexes/registry.rs`
> - `crates/context-graph-storage/src/teleological/quantized/trait_def.rs`
> - `crates/context-graph-storage/src/graph_edges/repository.rs`
> - `crates/context-graph-storage/src/graph_edges/serialization.rs`
> - `crates/context-graph-storage/src/graph_edges/types.rs`
> - `crates/context-graph-storage/src/code/mod.rs`
> - `crates/context-graph-storage/src/code/store.rs`

Cross-references: [03_configuration.md](03_configuration.md) (configuration keys), [09_storage_layer.md](09_storage_layer.md) (RocksDB layer, HNSW, inverted indexes), and embedding/search docs when present.

---

## 1. Connection Management

### 1.1 RocksDB Opening

The database is opened via `RocksDbTeleologicalStore::open()` or `RocksDbTeleologicalStore::open_with_config()`. The process:

1. **Stale lock detection** -- if the database path already exists, `detect_and_remove_stale_lock()` checks for orphaned LOCK files from unclean shutdown and removes them before open.
2. **Shared block cache** -- a single LRU block cache is created (default 256 MB via `Cache::new_lru_cache`) and shared across all 59 column families. This avoids redundant caching.
3. **DB options** are configured:
   - `create_if_missing(true)` (configurable)
   - `create_missing_column_families(true)` -- all 59 CFs are opened on every start
   - `set_max_open_files(1000)` (configurable)
   - `set_paranoid_checks(true)` -- detects bit rot and SST file corruption at ~1-3% perf cost
   - WAL enabled by default (`enable_wal: true`)
4. **Column family descriptors** are assembled via `get_all_column_family_descriptors()` which combines:
   - 11 base CFs from `crates/context-graph-storage/src/column_families.rs` (see `cf_names::ALL`)
   - 46 teleological/quantized/code/causal CFs from `crates/context-graph-storage/src/teleological/column_families.rs`
5. `DB::open_cf_descriptors()` opens the database with all 59 CFs (constant `TOTAL_COLUMN_FAMILIES = 59` in `column_families.rs:396`).
6. **Post-open initialization**:
   - `EmbedderIndexRegistry::new()` creates 15 empty in-memory HNSW indexes.
   - `CausalE11Index::new()` creates the causal relationship HNSW index.
   - Persisted soft-delete markers are loaded from `CF_SYSTEM` (prefix `soft_deleted::`) into a `DashMap<Uuid, i64>`.
   - Total document count is computed from `CF_FINGERPRINTS` iterator (minus soft-deleted) for IDF calculations.

### 1.2 Default Configuration (`TeleologicalStoreConfig`)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `block_cache_size` | 256 MB | Shared LRU block cache across all CFs |
| `max_open_files` | 1000 | RocksDB file descriptor limit |
| `enable_wal` | `true` | Write-ahead log for durability |
| `create_if_missing` | `true` | Auto-create database on first open |
| `gc_retention_secs` | 604800 (7 days) | Soft-delete GC retention period |

### 1.3 Write Buffer Management

All 59 CFs apply `apply_write_buffer_limits()` (`crates/context-graph-storage/src/column_families.rs:33`) to prevent excessive memory consumption. Without limits, the RocksDB default (64 MB write buffer x 2 per CF) would consume ~7.6 GB for 59 CFs. Each CF sets a per-CF write buffer size (2-8 MB) with `max_write_buffer_number = 2`.

### 1.4 Thread Safety

- `Arc<DB>` is shared across threads.
- HNSW indexes use `parking_lot::RwLock` (non-poisonable) for interior mutability.
- Soft-delete set uses `DashMap` for lock-free concurrent reads on the search hot path.
- A `secondary_index_lock` (parking_lot Mutex) serializes inverted index read-modify-write operations to prevent lost-update races.
- A `compaction_lock` (parking_lot RwLock) gates store/delete (read lock) vs. index rebuild (write lock) for consistency.

---

## 2. Schema Versioning Strategy

### 2.1 Version Bytes

Every serialized structure is prefixed with a version byte. Version mismatches produce errors (no automatic migration).

| Structure | Constant | Current Version |
|-----------|----------|-----------------|
| `TeleologicalFingerprint` | `TELEOLOGICAL_VERSION` | `1` |
| Embedder edges (K-NN) | `GRAPH_EDGE_VERSION` | `1` |
| Typed edges | `GRAPH_EDGE_VERSION` | `1` |

### 2.2 Versioning Rules

- **Serialization (write path)**: panics on error because data originates from in-memory structs the system controls.
- **Deserialization (read path)**: returns `Result<T, CoreError>` so a single corrupted record does not crash the server during iteration.
- On version mismatch, deserialization returns an error: "No automatic migration is supported - data must be regenerated."
- Legacy E5/E8 single-vector data is auto-migrated to dual asymmetric format in `deserialize_teleological_fingerprint()` via `migrate_legacy_e5()` / `migrate_legacy_e8()`.

### 2.3 Size Validation

Serialized `TeleologicalFingerprint` size is validated at write time:
- Minimum: 5,000 bytes (`MIN_FINGERPRINT_SIZE`)
- Maximum: 150,000 bytes (`MAX_FINGERPRINT_SIZE`)
- Panics outside this range, indicating missing embeddings or oversized data.

---

## 3. Column Families

The database has exactly **59 column families** (`TOTAL_COLUMN_FAMILIES` in `crates/context-graph-storage/src/column_families.rs:397`) organized into 5 groups:
- 11 base (8 original + 3 graph linking) — defined in `crates/context-graph-storage/src/column_families.rs`
- 28 teleological (20 base teleological + `CF_TRAINING_RECORDS` + 2 constellation CFs + 3 contrastive CFs + 2 typed-edge CFs) — defined in `crates/context-graph-storage/src/teleological/column_families.rs`
- 13 quantized embedder (`emb_0`..`emb_12`)
- 5 code
- 2 causal

The constant `TOTAL_COLUMN_FAMILIES = 59` is enforced by `test_total_column_families_constant` and `test_get_all_column_family_descriptors_returns_correct_count`. The teleological group itself is enumerated by `TELEOLOGICAL_CFS` / `TELEOLOGICAL_CF_COUNT = 28` in `teleological/column_families.rs:512`.

### 3.1 Base Column Families (11)

| CF Name | Key Format | Key Size | Value Format | Value Size | Purpose | Compression | Prefix Extractor | Bloom Filter |
|---------|-----------|----------|-------------|------------|---------|-------------|-----------------|-------------|
| `nodes` | UUID bytes | 16 B | Node data | Variable | Primary node storage | LZ4 | None (point lookups) | 10-bit |
| `edges` | UUID bytes | 16 B | Edge data | Variable | Graph edge storage | LZ4 | 16 B (source UUID) | 10-bit |
| `embeddings` | UUID bytes | 16 B | `[f32; 1536]` | 6,144 B | Embedding vectors | LZ4 | None (sequential) | None |
| `metadata` | UUID bytes | 16 B | Metadata | Variable | Node metadata | LZ4 | 16 B (UUID) | 10-bit |
| `temporal` | `timestamp_ms:UUID` | 24 B | Index entry | Variable | Time-based index | LZ4 | 16 B (UUID) | 10-bit |
| `tags` | `tag:UUID` | Variable | Index entry | Variable | Tag index | LZ4 | 16 B (UUID) | 10-bit |
| `sources` | `source_uri:UUID` | Variable | Index entry | Variable | Source index | LZ4 | 16 B (UUID) | 10-bit |
| `system` | String key | Variable | System metadata | Variable | System metadata, soft-delete markers | None | None | None |
| `embedder_edges` | `[embedder_id: u8][source: 16B]` | 17 B | `Vec<EmbedderEdge>` serialized | Variable | K-NN edges per embedder | LZ4 | 1 B (embedder_id) | 10-bit |
| `typed_edges` | `[source: 16B][target: 16B]` | 32 B | `TypedEdge` via bincode | Variable | Multi-relation typed edges | LZ4 | 16 B (source UUID) | 10-bit |
| `typed_edges_by_type` | `[edge_type: u8][source: 16B][target: 16B]` | 33 B | Target UUID | 16 B | Secondary index by edge type | LZ4 | 1 B (edge_type) | 10-bit |

Block sizes: `nodes` uses 256 MB point lookup hint; `embeddings` uses 64 KB blocks (fits ~10 embeddings per block).

### 3.2 Teleological Column Families (28)

| CF Name | Key Format | Key Size | Value Format | Value Size | Purpose | Compression | Status |
|---------|-----------|----------|-------------|------------|---------|-------------|--------|
| `fingerprints` | UUID bytes | 16 B | Version byte + bincode `TeleologicalFingerprint` | ~5-150 KB (typical ~63 KB) | Primary fingerprint storage with all 13 embeddings | LZ4 (64 KB blocks) | Active |
| `topic_profiles` | UUID bytes | 16 B | 13 x f32, little-endian | 52 B | Separate 13D topic profiles for fast queries | None | Active |
| `e13_splade_inverted` | `term_id` as u16 big-endian | 2 B | `[count: u32 LE][UUID x N]` | 4 + N x 16 B | E13 SPLADE inverted index for Stage 1 recall | LZ4 | Active |
| `e6_sparse_inverted` | `term_id` as u16 big-endian | 2 B | `[count: u32 LE][UUID x N]` | 4 + N x 16 B | E6 sparse inverted index for dual Stage 1 recall | LZ4 | Active |
| `e1_matryoshka_128` | UUID bytes | 16 B | 128 x f32, little-endian | 512 B | E1 truncated 128D vectors for Stage 2 fast filtering | LZ4 (4 KB blocks) | Active |
| `content` | UUID bytes | 16 B | Raw UTF-8 text | Up to 1 MB | Original content text per fingerprint | LZ4 (level compaction) | Active |
| `source_metadata` | UUID bytes | 16 B | `SourceMetadata` via JSON | ~100-500 B | Provenance information (source type, file path) | LZ4 | Active |
| `file_index` | File path bytes (UTF-8) | Variable | `FileIndexEntry` via bincode | ~100-2,000 B | File path to fingerprint ID mapping | LZ4 | Active |
| `topic_portfolio` | Session ID bytes (UTF-8) or `"__latest__"` | Variable | `PersistedTopicPortfolio` via JSON | ~1-50 KB | Topic portfolio for session continuity | LZ4 | Active |
| `e12_late_interaction` | UUID bytes | 16 B | `Vec<Vec<f32>>` via bincode (variable tokens x 128D) | 512 B - 256 KB (typical ~15 KB) | E12 ColBERT token embeddings for MaxSim Stage 5 | LZ4 (32 KB blocks) | Active |
| `entity_provenance` | `{canonical_id_bytes}_{memory_uuid_bytes}` | Variable + 16 B | `EntityProvenance` via bincode | ~200-2,000 B | Entity extraction provenance | LZ4 | Reserved |
| `audit_log` | `{timestamp_nanos_be}_{uuid_bytes}` | 24 B | `AuditRecord` via JSON | ~200-2,000 B | Append-only audit log (chronological) | LZ4 (level compaction) | Active |
| `audit_by_target` | `{target_uuid_bytes}_{timestamp_nanos_be}` | 24 B | Primary key bytes | 24 B | Audit log secondary index by target entity | LZ4 | Active |
| `merge_history` | `{merged_uuid_bytes}_{timestamp_nanos_be}` | 24 B | `MergeRecord` via JSON | ~500-5,000 B | Permanent merge lineage tracking | LZ4 (level compaction) | Active |
| `importance_history` | `{memory_uuid_bytes}_{timestamp_nanos_be}` | 24 B | `ImportanceChangeRecord` via JSON | ~100-500 B | Permanent importance change auditing | LZ4 (level compaction) | Active |
| `tool_call_index` | `tool_use_id` bytes (UTF-8) | Variable | `Vec<Uuid>` via bincode | ~16-160 B | Tool call to memory mapping | LZ4 | Reserved |
| `consolidation_recommendations` | UUID bytes | 16 B | `ConsolidationRecommendation` via bincode | ~500-5,000 B | Consolidation recommendation persistence | LZ4 | Reserved |
| `embedding_registry` | UUID bytes | 16 B | `EmbeddingVersionRecord` via JSON | ~200-500 B | Embedder model version tracking per fingerprint | LZ4 | Active |
| `custom_weight_profiles` | Profile name bytes (UTF-8) | Variable | `[f32; 13]` via JSON | ~100-200 B | User-created weight profiles for search | LZ4 | Active |
| `hnsw_graphs` | `"graph:{embedder}"` or `"meta:{embedder}"` (UTF-8) | Variable | Graph: usearch serialized bytes; Meta: JSON `{id_to_key, key_to_id, next_key, count}` | 1-100 MB (graph), ~1 KB (meta) | Persisted HNSW indexes for fast startup | LZ4 | Active |
| `training_records` | UUID bytes (same key as `CF_FINGERPRINTS`) | 16 B | `[version: u8 = TRAINING_RECORD_VERSION][bincode-encoded TrainingRecord]` | Variable (MB-sized with full embeddings) | Training corpus export: content + 13 embeddings + 13D topic profile + 78D synergy cross-correlations + 6D group alignments + typed edges + K-NN neighbors + causal labels + topic memberships. Populated by `export_training_corpus` MCP tool. Defined in `teleological/column_families.rs:399`; persistence implemented in `teleological/rocksdb_store/export.rs`. | LZ4 (~30-50% reduction) | Active |
| `constellations` | UUID bytes (fresh per compilation — **not** a member id) | 16 B | `[version: u8 = CONSTELLATION_VERSION][bincode-encoded Constellation]` | ~tens-hundreds KB | Phase 2 compiled constellation records. Aggregates per-embedder centroids + spread stats (L2 mean/stddev, cosine p50/p95, min/max, coverage) over a caller-selected member set; dense centroids for E1/E2/E3/E4/E5/E7/E8/E9/E10/E11, sparse top-K terms for E6/E13, pooled 128D token centroid for E12. Defined in `teleological/column_families.rs:418`; persistence implemented in `teleological/rocksdb_store/constellation.rs`. | LZ4 | Active |
| `constellation_by_selector` | `[selector_kind: u8][sha256(canonical_selector)[..16]: 16B]` | 17 B | Constellation UUID | 16 B | Secondary index: "does a constellation already exist for this Topic / Session / Tag / Time / Ids selector?" — O(1) existence check instead of a linear `CF_CONSTELLATIONS` scan. Canonical selector form produced by `ConstellationSelector::canonical_form()`; kind byte from `ConstellationSelector::kind_byte()`. Defined in `teleological/column_families.rs:436`; written atomically with the primary record in `constellation.rs:store_constellation`. | LZ4 | Active |
| `contrastive_pairs` | `[anchor_uuid: 16B][negative_uuid: 16B]` | 32 B | `[version: u8 = CONTRASTIVE_PAIR_VERSION][bincode-encoded ContrastivePair]` | ~1-5 KB typical | Phase 3 mined contrastive `(anchor, negative, similarity_profile)` triples for contrastive / metric-learning training. Carries classified `AnomalyKind`, high/low embedder indices, and disagreement magnitude. Composite primary key makes `(anchor, negative)` idempotent. Defined in `teleological/column_families.rs:453`; persistence implemented in `teleological/rocksdb_store/contrastive.rs`. | LZ4 | Active |
| `contrastive_by_kind` | `[kind: u8][anchor: 16B][neg: 16B]` | 33 B | Empty bytes | 0 B | Secondary index: prefix-scan enumeration of every pair of a given `AnomalyKind` without reading the primary record. Prefix extractor is 1 byte (kind). Defined in `teleological/column_families.rs:466`. | LZ4 | Active |
| `contrastive_by_anchor` | Anchor UUID | 16 B | `bincode(Vec<Uuid>)` of every mined negative for that anchor | Variable | Secondary index: "list every negative mined for this anchor" in O(1). Written append-style (read-modify-write inside `store_contrastive_pair`) with duplicates deduped lazily on read. Defined in `teleological/column_families.rs:479`. | LZ4 | Active |
| `typed_edge_records` | `[source:16][target:16][edge_type:1]` | 33 B | `[version: u8 = TYPED_EDGE_RECORD_VERSION][bincode TypedEdgeTrainingRecord]` | ~2-10 KB | Phase 4 F1 typed-edge export corpus. Each record captures per-embedder similarity profiles (`[f32; 13]`), source/target content, session metadata, optional `LLMValidationSummary` join, and exporter version. Composite 33-byte key (`typed_edge_keys.rs`) shared with `CF_TYPED_EDGE_VALIDATIONS`. Persistence in `teleological/rocksdb_store/typed_edge_export.rs`. Defined in `teleological/column_families.rs:499`. | LZ4 | Active |
| `typed_edge_validations` | `[source:16][target:16][edge_type:1]` | 33 B | `[version: u8 = LLM_EDGE_VALIDATION_VERSION][bincode LLMEdgeValidation]` | ~0.5-2 KB | Phase 4 F4 LLM verdicts on typed edges. Each record stores `LLMVerdict` (Valid / Invalid / Reclassify), confidence, rationale, `auto_derived_weight`, `validator_version`, and SHA-256 `prompt_hash` for reproducibility. Key layout identical to `CF_TYPED_EDGE_RECORDS`. Persistence in `teleological/rocksdb_store/llm_validation.rs`. Defined in `teleological/column_families.rs:516`. | LZ4 | Active |

Notes:
- "Reserved" CFs are opened for forward compatibility but not yet populated in the production write path.
- Audit log and merge/importance history CFs are **append-only** -- records are never updated or deleted.
- Merge history and importance history are **permanent** -- they never expire, unlike reversal records (30-day TTL).
- `training_records`, `constellations`, `contrastive_pairs`, `typed_edge_records`, and `typed_edge_validations` all use the canonical `[version_byte][bincode]` layout. Deserialization rejects mismatched versions with `CoreError::SerializationError`; there is no automatic migration (records must be re-exported / re-compiled / re-mined / re-validated).
- `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` are updated together inside a single `WriteBatch` (`store_constellation`). `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_KIND` + `CF_CONTRASTIVE_BY_ANCHOR` are updated together inside a single `WriteBatch` (`store_contrastive_pair`). `CF_TYPED_EDGE_RECORDS` and `CF_TYPED_EDGE_VALIDATIONS` share the same 33-byte composite key format (`typed_edge_keys.rs`) but are written independently.

### 3.3 Quantized Embedder Column Families (13)

One CF per embedder (ModelId index 0-12). All use UUID (16 bytes) keys. Quantized storage is reserved but not yet wired into production writes.

| CF Name | Embedder | Quantization | Key Size | Approx Value Size |
|---------|----------|-------------|----------|-------------------|
| `emb_0` | E1 Semantic (1024D) | PQ-8 | 16 B | ~8 B |
| `emb_1` | E2 Temporal Recent (512D) | Float8E4M3 | 16 B | ~512 B |
| `emb_2` | E3 Temporal Periodic (512D) | Float8E4M3 | 16 B | ~512 B |
| `emb_3` | E4 Temporal Positional (512D) | Float8E4M3 | 16 B | ~512 B |
| `emb_4` | E5 Causal (768D) | PQ-8 | 16 B | ~8 B |
| `emb_5` | E6 Sparse (30,522 vocab) | SparseNative | 16 B | ~2 KB |
| `emb_6` | E7 Code (1536D) | PQ-8 | 16 B | ~8 B |
| `emb_7` | E8 Graph (1024D) | Float8E4M3 | 16 B | ~1,024 B |
| `emb_8` | E9 HDC (10,000-bit) | Binary | 16 B | ~1,250 B |
| `emb_9` | E10 Multimodal (768D) | PQ-8 | 16 B | ~8 B |
| `emb_10` | E11 Entity (768D) | Float8E4M3 | 16 B | ~768 B |
| `emb_11` | E12 Late Interaction (128D/token) | TokenPruning | 16 B | ~2 KB |
| `emb_12` | E13 SPLADE (30,522 vocab) | SparseNative | 16 B | ~2 KB |

All quantized CFs share identical RocksDB options: 4 KB block size, LZ4 compression, bloom filter, 64 MB point lookup hint, 2 MB write buffer.

The `QuantizedFingerprintStorage` trait requires atomic writes of all 13 embedders via `WriteBatch`. Stores verify `fingerprint.embeddings.len() == 13` and panic otherwise.

### 3.4 Code Column Families (5)

Separate storage for code entities. Code uses AST-based chunking and E7 (Qodo-Embed-1-1.5B, 1536D) as the primary embedder.

| CF Name | Key Format | Key Size | Value Format | Value Size | Purpose |
|---------|-----------|----------|-------------|------------|---------|
| `code_entities` | UUID bytes | 16 B | `CodeEntity` via JSON | ~500-5,000 B | Code entity metadata (language, signature, line numbers) |
| `code_e7_embeddings` | UUID bytes | 16 B | Full `SemanticFingerprint` via bincode | ~63 KB (all 13 embeddings) | Complete 13-embedding fingerprint for multi-space code search |
| `code_file_index` | File path bytes (UTF-8) | Variable | `CodeFileIndexEntry` via bincode | Variable | File path to code entity ID mapping |
| `code_name_index` | Entity name bytes (UTF-8) | Variable | `Vec<Uuid>` via bincode | Variable | Entity name to entity ID secondary index |
| `code_signature_index` | Signature hash (SHA-256) | 32 B | `Vec<Uuid>` via bincode | Variable | Signature hash to entity ID secondary index |

Note: `code_e7_embeddings` stores the **full 13-embedder** `SemanticFingerprint` (not just E7), enabling multi-space search over code entities.

### 3.5 Causal Column Families (2)

Store LLM-generated causal relationship descriptions with provenance.

| CF Name | Key Format | Key Size | Value Format | Value Size | Purpose |
|---------|-----------|----------|-------------|------------|---------|
| `causal_relationships` | UUID bytes | 16 B | `CausalRelationship` via JSON | ~5-15 KB | Primary causal relationship storage (description + E1 embedding + provenance) |
| `causal_by_source` | Source fingerprint UUID bytes | 16 B | `Vec<Uuid>` via bincode | Variable | Secondary index: source fingerprint to causal relationship IDs |

Max serialized size per causal relationship: 100 KB (`MAX_CAUSAL_RELATIONSHIP_SIZE`).

---

## 4. Secondary Indexes

| Index Name | CF | Key Format | Key Size | Value | Purpose |
|------------|-----|-----------|----------|-------|---------|
| Audit by target | `audit_by_target` | `{target_uuid_bytes}_{timestamp_nanos_be}` | 24 B | Primary key (24 B) pointing back to `audit_log` | "Show all operations on memory X" |
| Typed edges by type | `typed_edges_by_type` | `[edge_type: u8][source: 16B][target: 16B]` | 33 B | Target UUID (16 B) | "All edges of type T from source S" |
| Causal by source | `causal_by_source` | Source fingerprint UUID bytes | 16 B | `Vec<Uuid>` (causal relationship IDs) | "All causal relationships from memory X" |
| File index | `file_index` | File path bytes (UTF-8) | Variable | `FileIndexEntry` (Vec<Uuid> + metadata) | "All fingerprints from file F" |
| Code file index | `code_file_index` | File path bytes (UTF-8) | Variable | `CodeFileIndexEntry` | "All code entities from file F" |
| Code name index | `code_name_index` | Entity name bytes (UTF-8) | Variable | `Vec<Uuid>` | "Find function/struct named X" |
| Code signature index | `code_signature_index` | SHA-256 hash | 32 B | `Vec<Uuid>` | "Find functions with signature pattern" |
| E13 SPLADE inverted | `e13_splade_inverted` | term_id (u16 BE) | 2 B | `Vec<Uuid>` (posting list) | Stage 1 sparse recall |
| E6 sparse inverted | `e6_sparse_inverted` | term_id (u16 BE) | 2 B | `Vec<Uuid>` (posting list) | Dual Stage 1 recall, Stage 3.5 tie-breaker |
| Constellation by selector | `constellation_by_selector` | `[selector_kind: u8][sha256(canonical)[..16]]` | 17 B | Constellation UUID (16 B) | "Does a constellation already exist for Topic/Session/Tag/Time/Ids selector S?" |
| Contrastive by kind | `contrastive_by_kind` | `[kind: u8][anchor: 16B][neg: 16B]` | 33 B | Empty bytes | Prefix-scan every contrastive pair of a given `AnomalyKind` |
| Contrastive by anchor | `contrastive_by_anchor` | Anchor UUID | 16 B | `bincode(Vec<Uuid>)` of negatives | "List every negative mined for anchor A" |
| Soft-delete markers | `system` | `"soft_deleted::{uuid}"` (string) | Variable | Timestamp i64 BE (8 B) or legacy `b"1"` (1 B) | Track soft-deleted memories |

### 4.1 Secondary Index Consistency

A `secondary_index_lock` (parking_lot Mutex) serializes all read-modify-write operations on posting lists (E13 SPLADE inverted, E6 sparse inverted, causal by-source, file index). Without this, concurrent writes to the same posting list would silently overwrite each other's additions.

### 4.2 Typed Edges Secondary Index Key Evolution

The `typed_edges_by_type` key was extended from 17 bytes (`[type(1)][source(16)]`) to 33 bytes (`[type(1)][source(16)][target(16)]`) to prevent overwrites when the same source has multiple edges of the same type. Deserialization handles both legacy 17-byte and current 33-byte key formats.

### 4.3 Constellation & Contrastive Index Atomicity

Both feature groups bundle their primary record and every secondary index entry into a single `WriteBatch`:

| Write operation | CFs touched in one batch |
|-----------------|--------------------------|
| `store_constellation(c)` | `CF_CONSTELLATIONS` (primary) + `CF_CONSTELLATION_BY_SELECTOR` (secondary) |
| `delete_constellation(id)` | `CF_CONSTELLATIONS` (primary) + `CF_CONSTELLATION_BY_SELECTOR` (only when the entry still points at `id` — avoids clobbering a newer constellation that shares the selector) |
| `store_contrastive_pair(pair)` | `CF_CONTRASTIVE_PAIRS` (primary) + `CF_CONTRASTIVE_BY_KIND` (secondary) + `CF_CONTRASTIVE_BY_ANCHOR` (secondary, RMW) |
| `delete_contrastive_pair(anchor, negative)` | `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_KIND` + `CF_CONTRASTIVE_BY_ANCHOR` (rewrites the anchor's `Vec<Uuid>` with the negative removed) |

For `CF_CONTRASTIVE_BY_ANCHOR`, the `Vec<Uuid>` is read outside the batch and appended inside the batch. The composite primary key `[anchor][neg]` guarantees correctness under concurrent writers; anchor-list duplicates are deduplicated on read, not on write.

---

## 5. HNSW Index Configuration and Management

### 5.1 Index Registry

`EmbedderIndexRegistry` manages **15 in-memory HNSW indexes** using the `usearch` library. Three embedders are excluded because they use different index types:

- **E6 Sparse** -- inverted index (not dense)
- **E12 Late Interaction** -- MaxSim token-level scoring (not single-vector)
- **E13 SPLADE** -- inverted index with BM25 (not dense)

The 15 HNSW indexes are:

| Index | Embedder | Dimension | Distance Metric | M | ef_construction | ef_search |
|-------|----------|-----------|----------------|---|----------------|-----------|
| E1Semantic | E1 Semantic | 1,024 | Cosine | 16 | 200 | 100 |
| E1Matryoshka128 | E1 truncated | 128 | Cosine | 32 | 256 | 128 |
| E2TemporalRecent | E2 Temporal Recent | 512 | Cosine | 16 | 200 | 100 |
| E3TemporalPeriodic | E3 Temporal Periodic | 512 | Cosine | 16 | 200 | 100 |
| E4TemporalPositional | E4 Temporal Positional | 512 | Cosine | 16 | 200 | 100 |
| E5Causal | E5 Causal (legacy active) | 768 | AsymmetricCosine | 16 | 200 | 100 |
| E5CausalCause | E5 cause vector | 768 | AsymmetricCosine | 16 | 200 | 100 |
| E5CausalEffect | E5 effect vector | 768 | AsymmetricCosine | 16 | 200 | 100 |
| E7Code | E7 Code | 1,536 | Cosine | 16 | 200 | 100 |
| E8Graph | E8 Graph | 1,024 | Cosine | 16 | 200 | 100 |
| E9HDC | E9 HDC | 1,024 | Cosine | 16 | 200 | 100 |
| E10Multimodal | E10 (legacy active) | 768 | AsymmetricCosine | 16 | 200 | 100 |
| E10MultimodalParaphrase | E10 paraphrase vector | 768 | AsymmetricCosine | 16 | 200 | 100 |
| E10MultimodalContext | E10 context vector | 768 | AsymmetricCosine | 16 | 200 | 100 |
| E11Entity | E11 Entity (KEPLER) | 768 | Cosine | 16 | 200 | 100 |

The E1Matryoshka128 index uses higher connectivity (M=32) and search parameters for Stage 2 fast filtering.

### 5.2 HNSW Internal Data Structures

Each `HnswEmbedderIndex` contains:

```
HnswEmbedderIndex {
    embedder: EmbedderIndex,
    config: HnswConfig,
    index: RwLock<usearch::Index>,      // O(log n) HNSW graph
    id_to_key: RwLock<HashMap<Uuid, u64>>,  // UUID -> usearch key
    key_to_id: RwLock<HashMap<u64, Uuid>>,  // usearch key -> UUID
    next_key: RwLock<u64>,              // monotonically increasing key counter
    removed_count: AtomicUsize,         // orphaned entries for compaction trigger
}
```

### 5.3 Capacity and Limits

- **MAX_HNSW_VECTORS_PER_INDEX**: 2,000,000 vectors. At 1024D x 4 bytes = 4 KB per vector, 2M vectors = ~8 GB raw data + HNSW graph overhead (~16 GB total). Inserts (not updates) are rejected at capacity.
- **Auto-resize**: when `index.size() >= index.capacity()`, capacity is doubled (minimum 1,024).
- **Estimated memory per vector**: `dimension * 4 + M * 2 * 4` bytes (vector data + link data).

### 5.4 HNSW Persistence (CF `hnsw_graphs`)

HNSW indexes are persisted to RocksDB in the `hnsw_graphs` column family to avoid full O(n) rebuild on startup:

- **Graph key**: `"graph:{embedder_name}"` -- stores the usearch serialized binary graph (1-100 MB per index).
- **Meta key**: `"meta:{embedder_name}"` -- stores JSON with `id_to_key`, `key_to_id`, `next_key`, and `count` mappings.

On startup, persisted indexes are loaded. If loading fails or indexes are absent, a full rebuild from `CF_FINGERPRINTS` is performed.

### 5.5 Distance Metrics

| Metric | usearch Mapping | Use Cases |
|--------|----------------|-----------|
| `Cosine` | `MetricKind::Cos` | E1, E2, E3, E4, E7, E8, E9, E11, E1Matryoshka128 |
| `AsymmetricCosine` | `MetricKind::Cos` (asymmetry at query time) | E5 (all variants), E10 (all variants) |
| `DotProduct` | `MetricKind::IP` | Not currently used |
| `Euclidean` | `MetricKind::L2sq` | Not currently used |
| `MaxSim` | N/A (not HNSW-compatible) | E12 ColBERT late interaction |

### 5.6 Inverted Index Configuration

Two embedders use inverted indexes instead of HNSW:

| Embedder | Vocab Size | Max Non-Zero | BM25 | CF |
|----------|-----------|-------------|------|-----|
| E6 Sparse | 30,522 (BERT) | 1,500 | No | `e6_sparse_inverted` |
| E13 SPLADE | 30,522 (BERT) | 1,500 | Yes | `e13_splade_inverted` |

---

## 6. Key Encoding Schemes

### 6.1 UUID Keys (16 bytes)

The majority of CFs use raw UUID bytes (`Uuid::as_bytes()`, big-endian) as the key. This applies to: `fingerprints`, `topic_profiles`, `e1_matryoshka_128`, `content`, `source_metadata`, `e12_late_interaction`, `embedding_registry`, `consolidation_recommendations`, `causal_relationships`, `causal_by_source`, `training_records`, `constellations`, `contrastive_by_anchor`, all quantized embedder CFs (`emb_0` through `emb_12`), `code_entities`, `code_e7_embeddings`.

Key functions:
- `fingerprint_key(id: &Uuid) -> [u8; 16]`
- `topic_profile_key(id: &Uuid) -> [u8; 16]`
- `content_key(id: &Uuid) -> [u8; 16]`
- `source_metadata_key(id: &Uuid) -> [u8; 16]`
- `e1_matryoshka_128_key(id: &Uuid) -> [u8; 16]`
- `e12_late_interaction_key(id: &Uuid) -> [u8; 16]`
- `causal_relationship_key(id: &Uuid) -> [u8; 16]`
- `causal_by_source_key(source_id: &Uuid) -> [u8; 16]`

### 6.2 Term ID Keys (2 bytes)

Inverted indexes use 2-byte big-endian u16 term IDs:
- `e13_splade_inverted_key(term_id: u16) -> [u8; 2]`
- `e6_sparse_inverted_key(term_id: u16) -> [u8; 2]`

Big-endian encoding ensures lexicographic ordering matches numeric ordering.

### 6.3 Composite Keys

#### Embedder Edges (17 bytes)
```
[embedder_id: u8][source_uuid: 16 bytes]
```
Prefix extractor: 1 byte (embedder_id) for per-embedder iteration.

#### Typed Edges (32 bytes)
```
[source_uuid: 16 bytes][target_uuid: 16 bytes]
```
Prefix extractor: 16 bytes (source UUID) for per-source iteration.

#### Typed Edges By Type Secondary Index (33 bytes)
```
[edge_type: u8][source_uuid: 16 bytes][target_uuid: 16 bytes]
```
Prefix extractor: 1 byte (edge_type). Queries use 17-byte prefix (`[type][source]`) to find all targets.

#### Constellation Selector Secondary Index (17 bytes)
```
[selector_kind: u8][sha256(canonical_selector)[..16]: 16 bytes]
```
Prefix extractor: 1 byte (selector kind) for per-kind range scans. Selector canonical form comes from `ConstellationSelector::canonical_form()`; kind byte from `ConstellationSelector::kind_byte()` (e.g. `Topic=0`, `Session=1`). Implemented in `teleological/rocksdb_store/constellation.rs::selector_index_key`.

#### Contrastive Pair Primary Key (32 bytes)
```
[anchor_uuid: 16 bytes][negative_uuid: 16 bytes]
```
Prefix extractor: 32 bytes so the whole composite stays a stable prefix. Guarantees idempotency per `(anchor, negative)`.

#### Contrastive By Kind Secondary Index (33 bytes)
```
[kind: u8][anchor_uuid: 16 bytes][negative_uuid: 16 bytes]
```
Prefix extractor: 1 byte (anomaly-kind). Enables O(log N) prefix scan of every pair of a given `AnomalyKind`.

#### Audit Log Primary Key (24 bytes)
```
[timestamp_nanos_be: 8 bytes][uuid_bytes: 16 bytes]
```
Big-endian timestamp ensures chronological iteration order.

#### Audit By Target Key (24 bytes)
```
[target_uuid_bytes: 16 bytes][timestamp_nanos_be: 8 bytes]
```
UUID prefix enables per-entity queries.

#### Merge History / Importance History Key (24 bytes)
```
[memory_uuid_bytes: 16 bytes][timestamp_nanos_be: 8 bytes]
```

#### Session Temporal Key (10 bytes)
```
[b"t:": 2 bytes][timestamp_ms_be: 8 bytes]
```
Used for temporal range queries within sessions.

### 6.4 String Keys

Several CFs use variable-length UTF-8 string keys:
- `system`: arbitrary string keys (e.g., `"soft_deleted::{uuid}"`)
- `file_index`: file path bytes
- `topic_portfolio`: session ID or `"__latest__"`
- `tool_call_index`: tool_use_id string
- `custom_weight_profiles`: profile name string
- `hnsw_graphs`: `"graph:{embedder}"` or `"meta:{embedder}"`
- `code_file_index`: file path bytes
- `code_name_index`: entity name bytes

---

## 7. Data Lifecycle

### 7.1 Create (Store)

When `store_async()` is called:

1. **Soft-delete check** -- rejects stores for soft-deleted IDs (MED-13 fix). Caller must hard-delete first or use a new ID.
2. **RocksDB write** via `store_fingerprint_internal()` using `WriteBatch` for atomicity across multiple CFs:
   - `CF_FINGERPRINTS`: version byte + bincode TeleologicalFingerprint
   - `CF_TOPIC_PROFILES`: 52-byte topic profile vector
   - `CF_E1_MATRYOSHKA_128`: 512-byte truncated E1 vector
   - `CF_E12_LATE_INTERACTION`: bincode-serialized token embeddings
   - `CF_E13_SPLADE_INVERTED`: posting list updates (under `secondary_index_lock`)
   - `CF_E6_SPARSE_INVERTED`: posting list updates (under `secondary_index_lock`)
   - `CF_SOURCE_METADATA`: JSON-serialized metadata (if present)
   - All 13 quantized embedder CFs (`emb_0` through `emb_12`)
3. **HNSW index insertion** via `add_to_indexes()`:
   - Acquires read lock on `compaction_lock`
   - Inserts into all 15 HNSW-capable embedder indexes
   - Skips zero-norm vectors (legacy data or possible corruption)
   - Skips E11 when `E11_ENTITY_ENABLED == false`
4. **Rollback on index failure** -- if HNSW insertion fails, the RocksDB write is rolled back via hard-delete from all CFs.
5. **Document count increment** -- `total_doc_count` is atomically incremented for IDF calculations.

### 7.2 Update

When `update_async()` is called:

1. Retrieve old fingerprint from RocksDB (for inverted index cleanup and rollback).
2. Remove old terms from E13 SPLADE and E6 sparse inverted indexes (under `secondary_index_lock`).
3. Remove old vectors from HNSW indexes.
4. Write new fingerprint via `store_fingerprint_internal()` (as update, does not increment doc count).
5. Add new vectors to HNSW indexes.
6. On HNSW failure, rollback restores the old fingerprint from captured data.

There is a brief transient inconsistency window between removing old inverted index terms and adding new ones. A concurrent reader may miss the fingerprint from inverted indexes during update. This is self-healing on completion.

### 7.3 Soft Delete

Soft delete marks a memory as invisible without removing data:

1. Add UUID to in-memory `soft_deleted` DashMap with timestamp.
2. Persist marker to `CF_SYSTEM` with key `"soft_deleted::{uuid}"` and value = timestamp as 8-byte big-endian i64.
3. All queries check `is_soft_deleted()` and skip matching IDs.
4. HNSW index entry is removed immediately so it does not appear in ANN search results.
5. `total_doc_count` is decremented.

### 7.4 Hard Delete

Hard delete permanently removes all data across all CFs:

1. Delete from `CF_FINGERPRINTS`
2. Delete from `CF_TOPIC_PROFILES`
3. Delete from `CF_E1_MATRYOSHKA_128`
4. Delete from `CF_E12_LATE_INTERACTION`
5. Remove from E13 SPLADE and E6 sparse inverted indexes
6. Delete from `CF_SOURCE_METADATA`
7. Delete from `CF_CONTENT`
8. Delete from all 13 quantized embedder CFs
9. Remove from HNSW indexes
10. Remove soft-delete marker from `CF_SYSTEM` (if present)
11. Remove from in-memory `soft_deleted` map

Order: RocksDB writes commit before HNSW removal. If RocksDB write fails, HNSW removal is skipped (data remains visible). This prevents the inverse inconsistency where HNSW removal succeeds but RocksDB commit fails.

### 7.5 Garbage Collection

Soft-deleted entries older than `gc_retention_secs` (default: 7 days) are permanently hard-deleted by the GC process. Legacy markers with 1-byte values (pre-timestamp format) are treated as timestamp 0 and are immediately GC-eligible.

### 7.6 HNSW Index Compaction

HNSW indexes accumulate orphaned vectors over time because usearch does not support true deletion -- removed vectors remain in the graph but are excluded from results via the `key_to_id` mapping. When the ratio of removed vectors to total vectors exceeds a threshold, the index is rebuilt from scratch:

1. Acquire write lock on `compaction_lock` (blocks all concurrent store/delete operations).
2. Iterate all fingerprints in `CF_FINGERPRINTS`.
3. Rebuild each HNSW index from the stored vectors.
4. Release write lock.

### 7.7 Append-Only Data

The following CFs are append-only with no update or delete support:
- `audit_log` / `audit_by_target` -- immutable audit trail
- `merge_history` -- permanent merge lineage (never expires)
- `importance_history` -- permanent importance change records (never expires)

---

## 8. Serialization Formats

### 8.1 Format Selection Rules

The project follows a strict rule documented in the constitution: **"JSON for provenance/metadata/audit. Bincode for dense vectors only."**

Additionally: **"bincode + skip_serializing_if = SILENT DATA CORRUPTION."** bincode is a positional format that does not encode field names. If any field uses `#[serde(skip_serializing_if = "...")]`, field count changes between serialization and deserialization, silently corrupting all subsequent fields. All optional/provenance fields must use JSON.

### 8.2 Bincode Usage

| CF | Data Type | Serialization | Notes |
|----|----------|---------------|-------|
| `fingerprints` | `TeleologicalFingerprint` | Version byte (1B) + bincode | Dense vectors dominate; ~63 KB typical |
| `code_e7_embeddings` | `SemanticFingerprint` | bincode | Dense vector storage for code entities |
| `e12_late_interaction` | `Vec<Vec<f32>>` | bincode | Token-level float arrays |
| `file_index` | `FileIndexEntry` | bincode | Vec<Uuid> + metadata |
| `code_file_index` | `CodeFileIndexEntry` | bincode | Code-specific file mapping |
| `code_name_index` | `Vec<Uuid>` | bincode | Entity ID lists |
| `code_signature_index` | `Vec<Uuid>` | bincode | Entity ID lists |
| `causal_by_source` | `Vec<Uuid>` | bincode | Relationship ID lists |
| `tool_call_index` | `Vec<Uuid>` | bincode | Memory ID lists |
| `consolidation_recommendations` | `ConsolidationRecommendation` | bincode | Structured data |
| `typed_edges` | `TypedEdge` | Version byte (1B) + bincode | Complex edge structure |
| `training_records` | `TrainingRecord` | Version byte (1B, `TRAINING_RECORD_VERSION`) + bincode | MB-scale per-memory training bundle; encode/decode in `teleological/rocksdb_store/export.rs::{encode_training_record, decode_training_record}` |
| `constellations` | `Constellation` | Version byte (1B, `CONSTELLATION_VERSION`) + bincode | Per-embedder centroids + spread stats; encode/decode in `teleological/rocksdb_store/constellation.rs::{encode_constellation, decode_constellation}` |
| `contrastive_pairs` | `ContrastivePair` | Version byte (1B, `CONTRASTIVE_PAIR_VERSION`) + bincode | `(anchor, negative, similarity_profile, AnomalyKind, …)`; encode/decode in `teleological/rocksdb_store/contrastive.rs::{encode_pair, decode_pair}` |
| `contrastive_by_anchor` | `Vec<Uuid>` | bincode | Deduplicated-on-read list of every negative mined for an anchor |
| All quantized CFs | `QuantizedEmbedding` | bincode | Per-embedder quantized vectors |

### 8.3 JSON Usage

| CF | Data Type | Notes |
|----|----------|-------|
| `source_metadata` | `SourceMetadata` | Provenance data with optional fields |
| `audit_log` | `AuditRecord` | Contains `serde_json::Value` parameters -- bincode cannot deserialize this |
| `merge_history` | `MergeRecord` | Provenance data with optional fields |
| `importance_history` | `ImportanceChangeRecord` | Structured audit data |
| `topic_portfolio` | `PersistedTopicPortfolio` | Session state with variable structure |
| `custom_weight_profiles` | `[f32; 13]` | Simple but uses JSON for human readability |
| `embedding_registry` | `EmbeddingVersionRecord` | Version metadata |
| `causal_relationships` | `CausalRelationship` | Provenance + text descriptions |
| `hnsw_graphs` (meta keys) | ID mapping metadata | JSON `{id_to_key, key_to_id, next_key, count}` |
| `code_entities` | `CodeEntity` | Metadata, not dense vectors |

### 8.4 Custom Binary Formats

| CF | Format | Description |
|----|--------|-------------|
| `topic_profiles` | Raw f32 little-endian | 13 x f32 = 52 bytes, no version byte or wrapper |
| `e1_matryoshka_128` | Raw f32 little-endian | 128 x f32 = 512 bytes, no version byte or wrapper |
| `e13_splade_inverted` | `[count: u32 LE][UUID bytes x N]` | Custom compact posting list format |
| `e6_sparse_inverted` | `[count: u32 LE][UUID bytes x N]` | Same format as E13 |
| `embedder_edges` | `[version: u8][count: u32 LE][{target: 16B, sim: f32} x N]` | 20 bytes per edge entry |
| `hnsw_graphs` (graph keys) | usearch native format | Library-specific serialized HNSW graph |

### 8.5 Embedder Dimension Constants

| Constant | Value | Used By |
|----------|-------|---------|
| `E1_DIM` | 1,024 | E1 Semantic (e5-large-v2) |
| `E1_MATRYOSHKA_DIM` | 128 | E1 truncated for Stage 2 |
| `E2_DIM` | 512 | E2 Temporal Recent |
| `E3_DIM` | 512 | E3 Temporal Periodic (Fourier) |
| `E4_DIM` | 512 | E4 Temporal Positional (sinusoidal PE) |
| `E5_DIM` | 768 | E5 Causal (Longformer SCM) |
| `E6_SPARSE_VOCAB` | 30,522 | E6 Sparse (BERT vocabulary) |
| `E7_DIM` | 1,536 | E7 Code (Qodo-Embed-1-1.5B) |
| `E8_DIM` | 1,024 | E8 Graph (e5-large-v2) |
| `E9_DIM` | 1,024 | E9 HDC (projected from 10K-bit) |
| `E10_DIM` | 768 | E10 Multimodal (CLIP) |
| `E11_DIM` | 768 | E11 Entity (KEPLER RoBERTa-base + TransE) |
| `E12_TOKEN_DIM` | 128 | E12 Late Interaction (ColBERT, per-token) |
| `E13_SPLADE_VOCAB` | 30,522 | E13 SPLADE (BERT vocabulary) |
| `NUM_EMBEDDERS` | 13 | Total core embedder count (E1-E13) |
| `TOPIC_PROFILE_DIM` | 13 | One weight per embedder |
