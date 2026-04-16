# Storage Layer

Source files covered:
- `crates/context-graph-storage/src/lib.rs`
- `crates/context-graph-storage/src/column_families.rs`
- `crates/context-graph-storage/src/error.rs`
- `crates/context-graph-storage/src/indexes.rs`
- `crates/context-graph-storage/src/graph_edges/` (mod.rs, repository.rs, builder.rs, serialization.rs, types.rs)
- `crates/context-graph-storage/src/teleological/` (mod.rs, column_families.rs, serialization.rs, schema.rs)
- `crates/context-graph-storage/src/teleological/rocksdb_store/` (mod.rs, store.rs, types.rs, crud.rs, persistence.rs, index_ops.rs, inverted_index.rs, trait_impl.rs, content.rs, source_metadata.rs, audit_log.rs, causal_relationships.rs, fusion.rs, export.rs, constellation.rs, contrastive.rs)
- `crates/context-graph-storage/src/teleological/quantized/` (mod.rs, trait_def.rs, rocksdb_impl.rs)
- `crates/context-graph-storage/src/code/` (mod.rs, store.rs, error.rs)
- `crates/context-graph-core/src/traits/teleological_memory_store/` (mod.rs, store.rs, backend.rs, options.rs, result.rs, ext.rs)
- `crates/context-graph-core/src/index/` (mod.rs, config.rs, manager.rs, splade_impl.rs)
- `crates/context-graph-core/src/index/hnsw_impl/` (mod.rs, real_hnsw.rs, multi_space.rs, persistence.rs)
- `crates/context-graph-core/src/memory/session.rs`

---

## 1. Storage Architecture Overview

The Context Graph storage layer is built on **RocksDB** for persistent key-value storage and **usearch/hnsw_rs** for in-memory HNSW vector indexes. The architecture spans two crates:

- **`context-graph-core`**: Defines traits (`TeleologicalMemoryStore`, `MultiSpaceIndexManager`, `QuantizedFingerprintStorage`), HNSW index implementations, and SPLADE inverted indexes.
- **`context-graph-storage`**: Provides RocksDB-backed implementations of those traits, column family definitions, graph edge storage, code entity storage, and quantized per-embedder storage.

### High-Level Data Flow

```
store_memory(text)
    |
    v
TeleologicalFingerprint (~63KB)
    |
    +---> CF_FINGERPRINTS (bincode, primary storage)
    +---> CF_CONTENT (UTF-8 text, LZ4 compressed)
    +---> CF_SOURCE_METADATA (JSON)
    +---> CF_E1_MATRYOSHKA_128 (512 bytes, Stage 2 ANN)
    +---> CF_E13_SPLADE_INVERTED (posting lists, Stage 1 recall)
    +---> CF_E6_SPARSE_INVERTED (posting lists, Stage 1 co-pilot)
    +---> CF_E12_LATE_INTERACTION (bincode, MaxSim rerank)
    +---> CF_TOPIC_PROFILES (52 bytes, 13D vector)
    +---> emb_0..emb_12 (quantized per-embedder)
    +---> 15 in-memory HNSW indexes (see Section 6 for full list)
```

### Design Principles

- **Fail fast**: All errors return detailed context (operation, column family, key). No silent fallbacks.
- **Atomic writes**: Multi-CF writes use `WriteBatch` for all-or-nothing semantics.
- **Thread safety**: RocksDB handles internal locking; HNSW indexes use `parking_lot::RwLock`; soft-delete tracking uses `DashMap` for lock-free concurrent reads.
- **Serialization rule**: Bincode for dense vectors (positional format, fast). JSON for provenance, metadata, and audit data (bincode + `skip_serializing_if` = silent corruption).

---

## 2. RocksDB Configuration

### Database Options

| Setting | Value | Rationale |
|---------|-------|-----------|
| Block cache | 256 MB shared LRU | Shared across all 59 CFs per constitution.yaml |
| Max open files | 1,000 | Default in `TeleologicalStoreConfig` |
| WAL | Enabled (default) | Durability; can be disabled for testing |
| Paranoid checks | Enabled | Catches bit rot at ~1-3% perf cost |
| Compression | LZ4 (all data CFs) | Fast compress/decompress; `system` CF uses None |
| Create if missing | true | Both DB and missing CFs auto-created |

Configuration is managed by `TeleologicalStoreConfig`:

```rust
pub struct TeleologicalStoreConfig {
    pub block_cache_size: usize,     // 256 * 1024 * 1024 (256MB)
    pub max_open_files: i32,         // 1000
    pub enable_wal: bool,            // true
    pub create_if_missing: bool,     // true
    pub gc_retention_secs: u64,      // 604800 (7 days)
}
```

### Write Buffer Limits

RocksDB defaults to 64MB write buffer x 2 per CF, which for 59 CFs would consume ~7.6GB. The system applies per-CF limits via `apply_write_buffer_limits()`:

| CF Category | Write Buffer Size | Max Write Buffers |
|-------------|-------------------|-------------------|
| Nodes | 4 MB | 2 |
| Edges | 4 MB | 2 |
| Embeddings | 8 MB | 2 |
| Index CFs (temporal, tags, sources) | 2 MB | 2 |
| System | 2 MB | 2 |
| Embedder edges | 4 MB | 2 |
| Typed edges | 4 MB | 2 |
| Typed edges by type | 2 MB | 2 |

### Per-CF Optimization

| CF Type | Bloom Filter | Prefix Extractor | Block Size | Point Lookup Hint |
|---------|-------------|-------------------|------------|-------------------|
| `nodes` | 10-bit | None | Default | 256MB |
| `edges` | 10-bit | 16 bytes (UUID) | Default | None |
| `embeddings` | None | None | 64 KB | None |
| `metadata`, `temporal`, `tags`, `sources` | 10-bit | 16 bytes (UUID) | Default | None |
| `system` | None | None | Default | None |
| `embedder_edges` | 10-bit | 1 byte (embedder_id) | Default | None |
| `typed_edges` | 10-bit | 16 bytes (source UUID) | Default | None |
| `typed_edges_by_type` | 10-bit | 1 byte (edge_type) | Default | None |

---

## 3. Column Families

The database uses **59 column families** total (`TOTAL_COLUMN_FAMILIES` in `crates/context-graph-storage/src/column_families.rs:396`), organized into five groups: 11 base + 28 teleological + 13 quantized + 5 code + 2 causal.

### 3.1 Base CFs (11)

| Name | Purpose | Key Format | Value |
|------|---------|------------|-------|
| `nodes` | Primary node storage | UUID (16 bytes) | Node data |
| `edges` | Graph edge storage | UUID (16 bytes) | Edge data |
| `embeddings` | Embedding vectors | UUID (16 bytes) | 1536D x f32 = 6144 bytes |
| `metadata` | Node metadata | UUID (16 bytes) | Metadata |
| `temporal` | Time-based index | timestamp_ms:UUID | Index entry |
| `tags` | Tag index | tag:UUID | Index entry |
| `sources` | Source index | source_uri:UUID | Index entry |
| `system` | System metadata | string key | Various |
| `embedder_edges` | K-NN edges per embedder | [embedder_id:u8][source:16] = 17 bytes | Vec\<EmbedderEdge\> |
| `typed_edges` | Multi-relation typed edges | [source:16][target:16] = 32 bytes | TypedEdge |
| `typed_edges_by_type` | Secondary index by edge type | [edge_type:u8][source:16][target:16] = 33 bytes | target UUID |

### 3.2 Teleological CFs (28)

| Name | Purpose | Key Format | Value |
|------|---------|------------|-------|
| `fingerprints` | Primary ~63KB TeleologicalFingerprints | UUID (16 bytes) | ~63KB bincode |
| `topic_profiles` | 13D topic profiles | UUID (16 bytes) | 52 bytes |
| `e13_splade_inverted` | E13 SPLADE inverted index | term_id (2 bytes) | Vec\<UUID\> |
| `e6_sparse_inverted` | E6 keyword inverted index | term_id (2 bytes) | Vec\<UUID\> |
| `e1_matryoshka_128` | E1 128D truncated vectors | UUID (16 bytes) | 512 bytes |
| `content` | Original text content | UUID (16 bytes) | UTF-8 text (LZ4) |
| `source_metadata` | Provenance metadata | UUID (16 bytes) | JSON (~100-500 bytes) |
| `file_index` | File path to fingerprint IDs | file_path (UTF-8) | FileIndexEntry (bincode) |
| `topic_portfolio` | Session topic portfolios | session_id or "\_\_latest\_\_" | JSON (~1-50KB) |
| `e12_late_interaction` | ColBERT token embeddings | UUID (16 bytes) | bincode (10-256KB) |
| `entity_provenance` | Entity provenance | UUID (16 bytes) | Provenance data |
| `audit_log` | Audit trail records | [timestamp_nanos_be (8B)][uuid (16B)] | AuditRecord JSON |
| `audit_by_target` | Audit by target index | [target:16][timestamp_nanos_be:8] | Audit primary key |
| `merge_history` | Merge provenance | [memory_uuid (16B)][timestamp_nanos_be (8B)] | MergeRecord JSON |
| `importance_history` | Importance change history | [memory_uuid (16B)][timestamp_nanos_be (8B)] | ImportanceChangeRecord JSON |
| `tool_call_index` | Tool call tracking | tool_use_id (UTF-8) | `Vec<Uuid>` (bincode) |
| `consolidation_recommendations` | Consolidation state | UUID (16 bytes) | ConsolidationRecommendation (bincode) |
| `embedding_registry` | Embedder metadata per fingerprint | UUID (16 bytes) | EmbeddingVersionRecord (JSON) |
| `custom_weight_profiles` | User weight profiles | profile name (UTF-8) | `[f32; 13]` (JSON) |
| `hnsw_graphs` | Persisted HNSW graph data | `"graph:{embedder}"` / `"meta:{embedder}"` | usearch bytes + JSON meta |
| `training_records` | Training corpus export bundle (content, 13 embeddings, cross-correlations, edges, neighbors, causal labels) — populated by `export_training_corpus` MCP tool. Persistence in `teleological/rocksdb_store/export.rs`. | UUID (16 bytes) same as `CF_FINGERPRINTS` | `[version: u8 = TRAINING_RECORD_VERSION][bincode TrainingRecord]` (MB-scale) |
| `constellations` | Phase 2 compiled constellation records: per-embedder centroids + spread stats over a caller-selected member set (Topic / Session / Tag / Time / explicit Ids). Dense centroids for E1/E2/E3/E4/E5/E7/E8/E9/E10/E11, sparse top-K for E6/E13, pooled 128D centroid for E12. Persistence in `teleological/rocksdb_store/constellation.rs`. | UUID (16 bytes, fresh per compilation) | `[version: u8 = CONSTELLATION_VERSION][bincode Constellation]` |
| `constellation_by_selector` | Secondary index for "does a constellation exist for this selector?" (Phase 2). Written atomically with the primary record in a single `WriteBatch`. | `[kind: u8][sha256(canonical_form)[..16]: 16B]` = 17 B | Constellation UUID (16 B) |
| `contrastive_pairs` | Phase 3 mined contrastive `(anchor, negative, similarity_profile, AnomalyKind, high/low embedder, disagreement)` triples. Persistence in `teleological/rocksdb_store/contrastive.rs`. | `[anchor: 16B][negative: 16B]` = 32 B | `[version: u8 = CONTRASTIVE_PAIR_VERSION][bincode ContrastivePair]` |
| `contrastive_by_kind` | Secondary index: prefix-scan every pair of a given `AnomalyKind` without reading the primary record. | `[kind: u8][anchor: 16B][neg: 16B]` = 33 B | Empty bytes |
| `contrastive_by_anchor` | Secondary index: "every negative mined for this anchor" in O(1); read-modify-write on store, deduped on read. | Anchor UUID (16 B) | `bincode(Vec<Uuid>)` |

### 3.3 Quantized Embedder CFs (13)

| CF Name | Embedder | Quantization | Approx. Value Size |
|---------|----------|--------------|-------------------|
| `emb_0` | E1 Semantic | PQ-8 | ~8 bytes |
| `emb_1` | E2 Temporal Recent | Float8 | ~512 bytes |
| `emb_2` | E3 Temporal Periodic | Float8 | ~512 bytes |
| `emb_3` | E4 Temporal Positional | Float8 | ~512 bytes |
| `emb_4` | E5 Causal | PQ-8 | ~8 bytes |
| `emb_5` | E6 Sparse | SparseNative | ~2 KB |
| `emb_6` | E7 Code | PQ-8 | ~8 bytes |
| `emb_7` | E8 Graph | Float8 | ~1024 bytes |
| `emb_8` | E9 HDC | Binary | ~1250 bytes |
| `emb_9` | E10 Multimodal | PQ-8 | ~8 bytes |
| `emb_10` | E11 Entity | PQ-8 | ~768 bytes |
| `emb_11` | E12 Late Interaction | TokenPruning | ~2 KB |
| `emb_12` | E13 SPLADE | SparseNative | ~2 KB |

All keyed by UUID (16 bytes). Target: ~17KB total per fingerprint across all 13 CFs.

### 3.4 Code CFs (5)

| Name | Purpose | Key Format |
|------|---------|------------|
| `code_entities` | CodeEntity storage | UUID (16 bytes) |
| `code_e7_embeddings` | E7 1536D embeddings | UUID (16 bytes) |
| `code_file_index` | File path to entity IDs | file_path (UTF-8) |
| `code_name_index` | Entity name to IDs | name (UTF-8) |
| `code_signature_index` | Signature hash to IDs | hash (UTF-8) |

### 3.5 Causal CFs (2)

| Name | Purpose | Key Format |
|------|---------|------------|
| `causal_relationships` | LLM-generated causal descriptions | UUID (16 bytes) |
| `causal_by_source` | Source fingerprint to causal IDs | source UUID (16 bytes) |

---

## 4. Store Trait Hierarchy

### 4.1 `TeleologicalMemoryStore` (core trait)

Defined in `crates/context-graph-core/src/traits/teleological_memory_store/store.rs`. This is the primary storage interface. It requires `Send + Sync` and provides async methods via `async_trait`.

**CRUD Operations:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `store` | `(TeleologicalFingerprint) -> Uuid` | Store a new fingerprint, returns assigned UUID |
| `retrieve` | `(Uuid) -> Option<TeleologicalFingerprint>` | Retrieve by UUID; returns None if not found or soft-deleted |
| `update` | `(TeleologicalFingerprint) -> bool` | Replace entire fingerprint; returns false if not found |
| `delete` | `(Uuid, soft: bool) -> bool` | Soft-delete (mark) or hard-delete (permanently remove) |

**Search Operations:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `search_semantic` | `(&SemanticFingerprint, options) -> Vec<SearchResult>` | Multi-space semantic search across 13 embedders |
| `search_text` | `(&str, options) -> Vec<SearchResult>` | Full-text search (generates embeddings internally) |
| `search_sparse` | `(&SparseVector, top_k) -> Vec<(Uuid, f32)>` | E13 SPLADE inverted index search |
| `search_e6_sparse` | `(&SparseVector, max_candidates) -> Vec<(Uuid, usize)>` | E6 keyword inverted index search |

**Batch Operations:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `store_batch` | `(Vec<Fingerprint>) -> Vec<Uuid>` | Bulk store with per-item rollback on index failure |
| `retrieve_batch` | `(&[Uuid]) -> Vec<Option<Fingerprint>>` | Bulk retrieve via spawn_blocking |

**Persistence:**

| Method | Description |
|--------|-------------|
| `flush` | Flush all 59 CFs to durable storage |
| `checkpoint` | Create RocksDB checkpoint (point-in-time snapshot) |
| `restore` | Restore from checkpoint (not yet implemented in-place) |
| `compact` | Hard-delete soft-deleted entries + RocksDB compaction |

**Content and Metadata:**

| Method | Description |
|--------|-------------|
| `store_content` / `get_content` / `delete_content` / `get_content_batch` | Original text content CRUD |
| `store_source_metadata` / `get_source_metadata` / `delete_source_metadata` / `get_source_metadata_batch` | Provenance metadata CRUD |
| `find_fingerprints_by_file_path` | Scan source metadata for file path matches |
| `list_indexed_files` / `get_fingerprints_for_file` / `index_file_fingerprint` / `unindex_file_fingerprint` / `clear_file_index` | File index management |

**Clustering and Topics:**

| Method | Description |
|--------|-------------|
| `persist_topic_portfolio` / `load_topic_portfolio` / `load_latest_topic_portfolio` | Session topic persistence |
| `scan_fingerprints_for_clustering` | Return (UUID, [Vec<f32>; 13]) tuples for HDBSCAN |
| `list_fingerprints_unbiased` | Direct CF scan without semantic bias |

**Causal Relationships:**

| Method | Description |
|--------|-------------|
| `store_causal_relationship` / `get_causal_relationship` | CRUD for LLM-generated causal descriptions |
| `get_causal_relationships_by_source` | Get all causal edges from a source fingerprint |
| `search_causal_relationships` | E1-based similarity search over causal descriptions |
| `search_causal_e5` / `search_causal_e5_hybrid` | E5 asymmetric causal search |

### 4.2 `TeleologicalMemoryStoreExt` (extension trait)

Blanket-implemented for all `TeleologicalMemoryStore` types. Provides:

- `exists(Uuid) -> bool` -- check existence via `retrieve`
- `validate_for_storage(&TeleologicalFingerprint)` -- validate all 13 embedder dimensions before store

### 4.3 `TeleologicalStorageBackend` (enum)

Identifies the storage implementation at runtime:

- `InMemory` -- HashMap-based, for testing
- `RocksDb` -- Production persistent storage
- `TimescaleDb` -- Time-series evolution archival
- `Hybrid` -- RocksDB + TimescaleDB combined

### 4.4 `MultiSpaceIndexManager` (index trait)

Defined in `crates/context-graph-core/src/index/manager.rs`. Coordinates dense HNSW indexes and sparse inverted indexes. The concrete `RocksDbTeleologicalStore` wires this trait into the `EmbedderIndexRegistry`, which hosts **15 HNSW indexes** (E1Semantic, E1Matryoshka128, E2-E4 temporal, E5/E5CausalCause/E5CausalEffect, E7-E9, E10/E10MultimodalParaphrase/E10MultimodalContext, E11Entity) plus **2 inverted indexes** (E6 sparse, E13 SPLADE) and **1 MaxSim store** (E12 ColBERT). See Section 6.1 for the authoritative list.

| Method | Description | Performance Target |
|--------|-------------|-------------------|
| `initialize` | Create all indexes | N/A |
| `add_vector` | Insert into specific HNSW index | <1ms per index |
| `add_fingerprint` | Route all embeddings to respective indexes | N/A |
| `search` | Query a specific HNSW index | <10ms per index |
| `search_matryoshka` | Stage 2 128D ANN search | <10ms |
| `search_splade` | Stage 1 SPLADE recall | <5ms |
| `remove` | Remove from all indexes | N/A |
| `persist` / `load` | Save/restore index state | <1s for 100K vectors |

### 4.5 `QuantizedFingerprintStorage` (quantized trait)

Defined in `crates/context-graph-storage/src/teleological/quantized/trait_def.rs`. All 13 embedders MUST be present.

| Method | Description |
|--------|-------------|
| `store_quantized_fingerprint` | Atomic write of all 13 embedders via WriteBatch |
| `load_quantized_fingerprint` | Load all 13 embedders with version check |
| `load_embedder` | Lazy load single embedder (0-12) |
| `delete_quantized_fingerprint` | Atomic delete of all 13 embedders |
| `exists_quantized_fingerprint` | Check emb_0 CF (optimization) |
| `quantization_router` | Access encode/decode operations |

---

## 5. Memory Store Operations

### 5.1 Store

When `store(fingerprint)` is called on `RocksDbTeleologicalStore`:

1. **Soft-delete guard**: Rejects stores for soft-deleted IDs (prevents invisible phantom data).
2. **RocksDB write** (`store_fingerprint_internal`): Atomic `WriteBatch` across:
   - `CF_FINGERPRINTS` -- full fingerprint (bincode, ~63KB)
   - `CF_E1_MATRYOSHKA_128` -- truncated 128D vector (512 bytes)
   - `CF_E13_SPLADE_INVERTED` -- posting list updates (under `secondary_index_lock`)
   - `CF_E6_SPARSE_INVERTED` -- posting list updates (under `secondary_index_lock`)
   - `CF_E12_LATE_INTERACTION` -- ColBERT tokens (bincode)
   - `CF_TOPIC_PROFILES` -- 13D topic vector (52 bytes)
   - `emb_0`..`emb_12` -- quantized per-embedder data
3. **HNSW index insert** (`add_to_indexes`): Inserts into all 15 HNSW-capable indexes (see Section 6.1). Acquires read lock on `compaction_lock`. Skips zero-norm vectors. Skips E11Entity when `E11_ENTITY_ENABLED == false` (the current default).
4. **IDF counter**: Increments `total_doc_count` atomic counter.
5. **Rollback**: If HNSW indexing fails, performs full hard-delete from all CFs.

### 5.2 Retrieve

1. Checks `soft_deleted` DashMap (lock-free O(1)).
2. Reads from `CF_FINGERPRINTS` via point lookup.
3. Deserializes with version check (bincode).

### 5.3 Search

Three search strategies are available (see `SearchStrategy` enum):

- **E1Only** (default): Single E1 HNSW index lookup. Fastest.
- **MultiSpace**: Weighted fusion of semantic embedders (E1, E5, E7, E8, E10, E11). Temporal embedders (E2-E4) have weight 0.0 in scoring but participate in HNSW retrieval when weight > 0 in profile. Score fusion via Weighted Reciprocal Rank Fusion (RRF).
- **Pipeline**: 3-stage retrieval: Stage 1 (E13 SPLADE + E1 recall) -> Stage 2 (multi-space scoring) -> Stage 3 (optional E12 ColBERT MaxSim rerank).

Score normalization options: `None`, `MinMax` (default), `ZScore`, `Convex`.

### 5.4 Update

Replaces the entire fingerprint. Internally performs delete-then-store to ensure index consistency.

### 5.5 Delete (Soft vs Hard)

**Soft delete** (`delete(id, soft=true)`):
- Adds ID + timestamp to `soft_deleted` DashMap.
- Persists marker to `CF_SYSTEM` with key `"soft_deleted::{uuid}"` and 8-byte big-endian timestamp value.
- Data remains in all CFs but is filtered from all queries and search results.
- Hard-deleted during compaction after `gc_retention_secs` (default: 7 days).

**Hard delete** (`delete(id, soft=false)`):
- Removes from all CFs: `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_SOURCE_METADATA`, `CF_E1_MATRYOSHKA_128`, `CF_E12_LATE_INTERACTION`, `CF_TOPIC_PROFILES`, all `emb_*` CFs.
- Removes from inverted indexes (E6, E13 posting lists).
- Removes from all HNSW indexes.
- Removes soft-delete marker if present.
- Decrements `total_doc_count`.

---

## 6. HNSW Index Management

### 6.1 Index Architecture

The system maintains **15 HNSW indexes** via the `usearch` library, managed by `EmbedderIndexRegistry` (see `crates/context-graph-storage/src/teleological/indexes/registry.rs`). The `EmbedderIndex` enum that names each index is defined in `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs:37`. The counts break down as: 11 primary (E1, E1Matryoshka128, E2-E5, E7-E11) plus 4 asymmetric variants (E5CausalCause, E5CausalEffect, E10MultimodalParaphrase, E10MultimodalContext).

| HNSW Index | Embedder | Dimension | Distance Metric | Purpose |
|------------|----------|-----------|-----------------|---------|
| E1Semantic | E1 | 1024D | Cosine | Primary semantic search |
| E1Matryoshka128 | E1 (truncated) | 128D | Cosine | Stage 2 fast ANN (M=32, ef_c=256, ef_s=128) |
| E2TemporalRecent | E2 | 512D | Cosine | Temporal recency (sinusoidal PE of absolute Unix timestamp) |
| E3TemporalPeriodic | E3 | 512D | Cosine | Fourier periodic encoding |
| E4TemporalPositional | E4 | 512D | Cosine | Sinusoidal PE of session turn index |
| E5Causal | E5 | 768D | AsymmetricCosine | Legacy active causal vector |
| E5CausalCause | E5 cause vector | 768D | AsymmetricCosine | Cause-direction retrieval (ARCH-15) |
| E5CausalEffect | E5 effect vector | 768D | AsymmetricCosine | Effect-direction retrieval (ARCH-15) |
| E7Code | E7 | 1536D | Cosine | Code pattern search |
| E8Graph | E8 | 1024D | Cosine | Graph relationship search |
| E9HDC | E9 | 1024D | Cosine | Typo-robust HDC search |
| E10Multimodal | E10 | 768D | AsymmetricCosine | Legacy active multimodal vector |
| E10MultimodalParaphrase | E10 paraphrase | 768D | AsymmetricCosine | Paraphrase-direction retrieval (ARCH-15) |
| E10MultimodalContext | E10 context | 768D | AsymmetricCosine | Context-direction retrieval (ARCH-15) |
| E11Entity | E11 (KEPLER) | 768D | Cosine | Entity search. **Disabled by default** via `E11_ENTITY_ENABLED = false` in `crates/context-graph-core/src/weights/mod.rs:45`. |

Embedders E6 (Sparse), E12 (ColBERT/MaxSim), and E13 (SPLADE) do **not** use HNSW. They use inverted indexes (see Section 7) or direct MaxSim computation.

Separate from the registry, a dedicated **CausalE11Index** (`crates/context-graph-storage/src/teleological/rocksdb_store/causal_hnsw_index.rs`) wraps a 768D E11 HNSW over LLM-generated causal relationships with `INITIAL_CAPACITY = 1024` growing by doubling.

### 6.2 HNSW Configuration

Default HNSW parameters per embedder (from `HnswConfig::default_for_dimension` in `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs:89`):

| Parameter | Default Value | Matryoshka 128D (`HnswConfig::matryoshka_128d`) |
|-----------|---------------|-----------------|
| M (links per node) | 16 | 32 |
| ef_construction | 200 | 256 |
| ef_search | 100 | 128 |

**Hard cap**: `MAX_HNSW_VECTORS_PER_INDEX = 2,000,000` (`crates/context-graph-storage/src/teleological/indexes/hnsw_impl/types.rs:18`). Inserts beyond this limit are rejected; updates are not.

**Initial capacity** (for `CausalE11Index`): `INITIAL_CAPACITY = 1024`, grows by doubling when exhausted (`causal_hnsw_index.rs:50`).

**Validation** (panics in `HnswConfig::new`): `m >= 2`, `ef_construction >= m`, `ef_search >= 1`, `dimension >= 1`.

Estimated memory per vector: `dimension * 4 + M * 2 * 4` bytes (from `HnswConfig::estimated_memory_per_vector`).

### 6.3 Index Lifecycle

**Startup**: On `open_with_config()`:
1. Attempt fast-path load from `CF_HNSW_GRAPHS` (persisted graphs).
2. If no persisted data or load fails, clear all indexes and rebuild from `CF_FINGERPRINTS` (O(n) scan).
3. Detect newly-enabled embedders with empty indexes after fast-path restore and trigger rebuild if needed.
4. Run consistency verification (HNSW sizes vs live fingerprint count, 10% tolerance).

**Insert**: `add_to_indexes()` acquires compaction_lock read lock, inserts into all 15 HNSW indexes. Zero-norm vectors are skipped (temporal embedders may have legacy zero data). NaN/Inf vectors are rejected. E11 is skipped when `E11_ENTITY_ENABLED == false` (the current default).

**Delete**: Removes vectors from all HNSW indexes. Note that usearch tracks removed entries but does not physically delete them.

**Compaction**: `compact_hnsw_if_needed()` rebuilds HNSW indexes when removed entry count exceeds a threshold. Acquires compaction_lock write lock (exclusive, blocks all concurrent store/delete).

**Persistence**: HNSW graphs are persisted to `CF_HNSW_GRAPHS` for fast startup recovery. Target: <1s for 100K vectors.

---

## 7. Inverted Indexes for Sparse Vectors (E6/E13)

Two inverted indexes support sparse vector retrieval:

### 7.1 E13 SPLADE Inverted Index (`CF_E13_SPLADE_INVERTED`)

- **Vocabulary**: 30,522 terms (BERT tokenizer)
- **Max non-zero entries per vector**: 1,500
- **Scoring**: BM25+SPLADE hybrid with IDF weighting
- **Key format**: `term_id` (u16, 2 bytes)
- **Value format**: Sorted `Vec<Uuid>` posting list (bincode)
- **Used in**: Stage 1 recall of the retrieval pipeline

BM25 scoring formula:
```
score(d, q) = Sum_t IDF(t) * TF(t, d) * q_weight(t)
IDF(t) = ln((N - df + 0.5) / (df + 0.5) + 1)
```

The in-memory `SpladeInvertedIndex` (in `context-graph-core`) also provides standalone search. The RocksDB implementation stores posting lists persistently.

**In-memory bounds** (audit #14/#15 memory safety fixes):
- `InMemorySpladeIndex` capacity: `MAX_SPLADE_DOCS = 100,000` (FIFO eviction of oldest when full). Source: `crates/context-graph-storage/src/teleological/search/pipeline/traits.rs:160`.
- `InMemoryTokenStorage` (E12 ColBERT token cache) capacity: `MAX_TOKEN_ENTRIES = 5,000` (FIFO eviction; each entry is ~262 KB). Source: `crates/context-graph-storage/src/teleological/search/pipeline/traits.rs:32`.
- `SparseInvertedIndex` (standalone legacy path) max documents: `DEFAULT_MAX_SPARSE_DOCS = 500,000`. Source: `crates/context-graph-core/src/retrieval/sparse_index.rs:46`.
- SPLADE model file load cap: `MAX_LOAD_FILE_SIZE = 1,073,741,824` bytes (1 GB). Source: `crates/context-graph-core/src/index/splade_impl.rs:252`.

### 7.2 E6 Sparse Inverted Index (`CF_E6_SPARSE_INVERTED`)

- **Vocabulary**: 30,522 terms (same BERT tokenizer)
- **Max non-zero entries per vector**: 1,500
- **Scoring**: Term overlap count (no BM25)
- **Purpose**: Exact keyword matching that E1 semantic search misses due to embedding averaging

### 7.3 Posting List Management

Both inverted indexes share the same update/remove logic (extracted into generic helpers):

- **Update**: Read-modify-write under `secondary_index_lock` mutex to prevent lost-update races. Uses `multi_get_cf` for batch reads. Posting lists are kept sorted by UUID for O(log n) binary search.
- **Remove**: Binary search for UUID in sorted posting list, remove if found.
- **Legacy handling**: Unsorted posting lists (from before the sort fix) are sorted on first access and written back.

---

## 8. Code Store

The `CodeStore` (`crates/context-graph-storage/src/code/store.rs`) provides separate storage for code entities, distinct from text memories.

### Architecture

- Uses 5 dedicated column families (see Section 3.4).
- Each code entity stores the full 13-embedder `SemanticFingerprint` (ARCH-01 compliance).
- E7 (Qodo-Embed-1-1.5B, 1536D) is the primary code embedder.
- Secondary indexes for name and signature provide O(1) lookups.
- File-level tracking enables efficient change detection and cleanup.

### Serialization

| Data Type | Format |
|-----------|--------|
| `CodeEntity`, `CodeFileIndexEntry`, `Vec<Uuid>` indexes | JSON |
| `SemanticFingerprint` | bincode |

### Key Constants

- `E7_CODE_DIM = 1536` (Qodo-Embed-1-1.5B output dimension)

### Operations

| Operation | Description |
|-----------|-------------|
| Store entity | Writes to `code_entities` (JSON) + `code_e7_embeddings` (bincode) + indexes |
| Search by name | Prefix scan on `code_name_index` |
| Search by signature | Hash lookup on `code_signature_index` |
| Search by file | Prefix scan on `code_file_index` |
| Delete by file | Clear all entities for a file path |

---

## 9. Teleological Store (`RocksDbTeleologicalStore`)

The primary production store implementation, located in `crates/context-graph-storage/src/teleological/rocksdb_store/`.

### Internal Structure

```rust
pub struct RocksDbTeleologicalStore {
    db: Arc<DB>,
    cache: Cache,
    path: PathBuf,
    fingerprint_count: RwLock<Option<usize>>,   // Cached count
    total_doc_count: Arc<AtomicUsize>,           // O(1) IDF lookups
    soft_deleted: Arc<DashMap<Uuid, i64>>,       // Lock-free concurrent reads
    index_registry: Arc<EmbedderIndexRegistry>,  // 15 HNSW indexes (see Section 6.1)
    causal_e11_index: Arc<CausalE11Index>,       // Separate E11 HNSW for causal relationships
    secondary_index_lock: parking_lot::Mutex<()>,// Protects posting list RMW
    compaction_lock: RwLock<()>,                 // Protects HNSW during rebuild
}
```

### Startup Sequence

1. Detect and remove stale lock files (Unix: `flock` probe; Windows: logged warning).
2. Create shared 256MB LRU block cache.
3. Open RocksDB with all 59 CF descriptors via `get_all_column_family_descriptors()`. Transform corruption errors.
4. Load persisted soft-delete markers from `CF_SYSTEM` (before index rebuild).
5. Count total documents for O(1) IDF lookups.
6. Load HNSW indexes (fast-path from `CF_HNSW_GRAPHS`, fallback to O(n) rebuild).
7. Rebuild E11 HNSW index from causal relationships (when enabled).
8. Run consistency verification (non-blocking, logs warnings only).

### Module Decomposition

| Module | Responsibility |
|--------|---------------|
| `store.rs` | Struct definition, constructors, open/close, stale lock detection |
| `crud.rs` | Store, retrieve, update, delete operations |
| `search.rs` | Semantic, text, sparse search implementations |
| `index_ops.rs` | HNSW index add/remove operations |
| `inverted_index.rs` | E6/E13 posting list update/remove |
| `persistence.rs` | Batch ops, statistics, flush/checkpoint/compact, topic portfolio |
| `content.rs` | Content text CRUD |
| `source_metadata.rs` | Provenance metadata CRUD |
| `audit_log.rs` | Audit trail append/query |
| `causal_relationships.rs` | Causal relationship CRUD |
| `fusion.rs` | Weighted RRF fusion, consensus computation |
| `trait_impl.rs` | Thin async wrapper delegating to internal methods |
| `types.rs` | Error types, config, result aliases |
| `export.rs` | Training-record persistence (`CF_TRAINING_RECORDS`) for `export_training_corpus` MCP tool: versioned encode/decode + topic-profile reader |
| `constellation.rs` | Phase 2 constellation persistence: atomic primary + selector-index writes across `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` |
| `contrastive.rs` | Phase 3 contrastive-pair persistence: atomic primary + by-kind + by-anchor writes across `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_KIND` + `CF_CONTRASTIVE_BY_ANCHOR` |

---

## 10. Graph Edge Storage

Located in `crates/context-graph-storage/src/graph_edges/`. Provides persistent storage for knowledge graph linking.

### 10.1 K-NN Edges (`embedder_edges` CF)

Each of 13 embedders maintains its own K-NN graph. The `EdgeRepository` provides:

| Method | Description |
|--------|-------------|
| `store_embedder_edges(embedder_id, source, edges)` | Store k=20 neighbors for a node |
| `get_embedder_edges(embedder_id, source)` | Get neighbors (empty vec if none) |
| `delete_embedder_edges(embedder_id, source)` | Remove neighbor list |
| `iter_embedder_edges(embedder_id)` | Iterate all nodes for an embedder |
| `is_empty()` | Check if any K-NN edges exist |

Key format: `[embedder_id: u8][source_uuid: 16 bytes]` = 17 bytes.

Serialization uses a custom compact format:
```
[version: u8][count: u32][edges: (target_uuid: 16 bytes, similarity: f32)...]
```

### 10.2 Typed Edges (`typed_edges` CF)

Multi-relation edges derived from embedder agreement patterns, with 8 edge types.

| Method | Description |
|--------|-------------|
| `store_typed_edge(edge)` | Store edge + update secondary index (atomic WriteBatch) |
| `get_typed_edge(source, target)` | Point lookup by source+target |
| `get_typed_edges_from(source)` | All outgoing edges from a node (prefix scan) |
| `get_typed_edges_to(target)` | All incoming edges to a node (full CF scan, O(n)) |
| `get_typed_edges_by_type(source, edge_type)` | Filtered by type via secondary index |
| `delete_typed_edge(source, target)` | Delete from primary + secondary index |
| `store_typed_edges_batch(edges)` | Batch write via single WriteBatch |

**Primary key**: `[source: 16 bytes][target: 16 bytes]` = 32 bytes.

**Secondary index key** (`typed_edges_by_type`): `[edge_type: u8][source: 16 bytes][target: 16 bytes]` = 33 bytes. Target is included in the key to prevent overwrites when the same source has multiple edges of the same type.

**Serialization**: TypedEdge uses bincode with a version prefix byte.

### 10.3 Background Graph Builder

`BackgroundGraphBuilder` provides asynchronous K-NN graph construction:

| Parameter | Default Value |
|-----------|---------------|
| `batch_interval_secs` | 60 |
| `max_batch_size` | 1,000 |
| `min_batch_size` | 10 |
| `active_embedders` | [0, 4, 6, 7, 9, 10] (E1, E5, E7, E8, E10, E11) |
| `k` (neighbors per node) | 20 |
| `min_weighted_agreement` | 2.5 |
| `queue overflow limit` | 100,000 (drains to 50,000 on overflow) |

The builder:
1. Queues fingerprint IDs via `enqueue()` (non-blocking).
2. Periodically drains the queue in batches.
3. Runs NN-Descent per active embedder to build K-NN graphs.
4. Creates typed edges based on embedder agreement.
5. Persists to `EdgeRepository` and emits audit records.

`rebuild_all()` performs a full rebuild by scanning all fingerprints from the teleological store.

---

## 11. Training / Constellation / Contrastive Storage

Three additional persistence paths live in `crates/context-graph-storage/src/teleological/rocksdb_store/`. All three share the same canonical wire format (`[version: u8][bincode payload]`) and the same failure policy: deserialization rejects mismatched version bytes with `CoreError::SerializationError` — there is no automatic migration.

### 11.1 Training Records (`export.rs`)

Backs the `export_training_corpus` MCP tool. Each record bundles the memory's content, all 13 embedding vectors (symmetric plus asymmetric pairs for E5 and E10), the 13D topic profile, 78D synergy-weighted cross-correlations, 6D group alignments, typed edges, K-NN neighbors, causal labels, and topic memberships.

| Operation | CF | Notes |
|-----------|----|-------|
| `store_training_record(id, &record)` | `CF_TRAINING_RECORDS` | Idempotent per UUID — re-exports overwrite prior records |
| `get_training_record(id)` | `CF_TRAINING_RECORDS` | Returns `None` on miss; version mismatch is an error |
| `multi_get_training_records(ids)` | `CF_TRAINING_RECORDS` | Batch read; undecodable rows become `None` slots, RocksDB errors bubble up |
| `list_training_record_ids()` | `CF_TRAINING_RECORDS` | Full CF scan; non-UUID keys are logged and skipped |
| `count_training_records()` | `CF_TRAINING_RECORDS` | O(n) iterator count |
| `delete_training_record(id)` | `CF_TRAINING_RECORDS` | Returns `true` when a record actually existed |
| `clear_all_training_records()` | `CF_TRAINING_RECORDS` | Caller-initiated reset (used by the `clearExisting=true` MCP path and tests) |
| `get_topic_profile(id)` | `CF_TOPIC_PROFILES` | Reader used during export to pull the canonical 13 × f32 = 52-byte profile; rejects malformed rows |

Wire format helpers `encode_training_record` / `decode_training_record` are exported publicly so downstream CLI exporters emit byte-for-byte identical payloads to what lands in `CF_TRAINING_RECORDS`.

### 11.2 Constellations (`constellation.rs`)

Phase 2 compiler persistence. Each constellation aggregates per-embedder centroids and spread statistics (L2 mean/stddev, cosine p50/p95, min/max, coverage) over a caller-selected member set (Topic / Session / Tag / Time / explicit Ids). A secondary index at `CF_CONSTELLATION_BY_SELECTOR` — key `[selector_kind: u8][sha256(canonical_form)[..16]: 16B]` — maps selectors to the compiled UUID so "does one already exist?" is an O(1) point lookup.

| Operation | CFs touched | Atomicity |
|-----------|-------------|-----------|
| `store_constellation(&c)` | `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` | Single `WriteBatch`; re-stores overwrite both records |
| `get_constellation(id)` | `CF_CONSTELLATIONS` | Point lookup by UUID |
| `list_constellation_ids()` | `CF_CONSTELLATIONS` | Full CF scan; non-UUID keys are logged and skipped |
| `count_constellations()` | `CF_CONSTELLATIONS` | O(n) iterator count |
| `multi_get_constellations(ids)` | `CF_CONSTELLATIONS` | Batch read; undecodable rows become `None` |
| `delete_constellation(id)` | `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` (conditional) | Single `WriteBatch`; the selector-index entry is only deleted when it still points at `id`, so a newer constellation sharing the selector is never clobbered |
| `find_constellation_by_selector(&selector)` | `CF_CONSTELLATION_BY_SELECTOR` | O(1) selector → UUID lookup |

The secondary-index key builder (`selector_index_key`) produces a 17-byte key; distinct `ConstellationSelector::kind_byte()` values guarantee different keys for selectors that would otherwise hash the same.

### 11.3 Contrastive Pairs (`contrastive.rs`)

Phase 3 miner persistence. Each record bundles an `(anchor, negative, similarity_profile)` triple with classified `AnomalyKind`, high/low embedder indices, and disagreement magnitude. Three CFs cooperate:

| CF | Key | Value | Purpose |
|----|-----|-------|---------|
| `CF_CONTRASTIVE_PAIRS` | `[anchor:16B][neg:16B]` (32 B) | `[version][bincode ContrastivePair]` | Primary record; composite key enforces `(anchor, negative)` idempotency |
| `CF_CONTRASTIVE_BY_KIND` | `[kind:u8][anchor:16B][neg:16B]` (33 B) | Empty | Per-anomaly-kind prefix scan |
| `CF_CONTRASTIVE_BY_ANCHOR` | Anchor UUID (16 B) | `bincode(Vec<Uuid>)` of negatives | "All negatives mined for this anchor" |

`store_contrastive_pair` and `delete_contrastive_pair` write all three CFs in a single `WriteBatch`. The anchor→Vec row is read outside the batch and rewritten inside; concurrent writers can race, but the 32-byte composite primary key is the correctness anchor, and duplicates in `CF_CONTRASTIVE_BY_ANCHOR` are deduplicated lazily on read.

---

## 12. Quantized Storage (PQ-8)

Located in `crates/context-graph-storage/src/teleological/quantized/`.

### Architecture

Each of the 13 embedders has a dedicated column family (`emb_0`..`emb_12`) for quantized embedding storage. The quantization method varies per embedder:

| Quantization | Embedders | Storage Savings |
|-------------|-----------|-----------------|
| PQ-8 (Product Quantization) | E1, E5, E7, E10, E11 | ~128x compression |
| Float8 | E2, E3, E4, E8 | ~4x compression |
| Binary | E9 (HDC) | ~32x compression |
| SparseNative | E6, E13 | Native sparse format |
| TokenPruning | E12 (ColBERT) | Prune least-important tokens |

### Operations

All operations enforce the all-13-embedders invariant:

| Method | Description |
|--------|-------------|
| `store_quantized_fingerprint` | Atomic `WriteBatch` across all 13 `emb_*` CFs. Panics if < 13 embedders. |
| `load_quantized_fingerprint` | Reads all 13 CFs, verifies version. Returns complete `StoredQuantizedFingerprint`. |
| `load_embedder` | Lazy single-embedder load. Panics if `embedder_idx >= 13`. |
| `delete_quantized_fingerprint` | Atomic delete from all 13 CFs. |
| `exists_quantized_fingerprint` | Checks only `emb_0` (optimization: if emb_0 exists, all should). |

### Serialization

Per-embedder quantized embeddings use bincode with a key format of `[uuid: 16 bytes]`. The `QuantizationRouter` provides encode/decode operations for each quantization method.

---

## 13. Batch Operations

Batch operations are critical for performance. The storage layer provides batching at multiple levels:

### RocksDB WriteBatch

Used internally for atomic multi-CF writes:
- `store_fingerprint_internal` writes to 8+ CFs in a single `WriteBatch`.
- `store_typed_edge` writes primary + secondary index atomically.
- `store_quantized_fingerprint` writes all 13 embedder CFs atomically.

### Store-Level Batching

| Method | Behavior |
|--------|----------|
| `store_batch` | Processes each fingerprint individually with per-item rollback on failure. Returns only successfully stored IDs. Returns error only if ALL fail. |
| `retrieve_batch` | Uses `spawn_blocking` for I/O on Tokio's blocking thread pool. Filters soft-deleted entries. |
| `get_content_batch` | Bulk content retrieval. |
| `get_source_metadata_batch` | Bulk metadata retrieval. |

### Graph Edge Batching

| Method | Behavior |
|--------|----------|
| `store_typed_edges_batch` | Single `WriteBatch` for all primary + secondary index entries. |

---

## 14. Compaction and Garbage Collection

### Soft-Delete GC

The `compact()` method performs garbage collection:

1. Collects all soft-deleted IDs from the `DashMap`.
2. Hard-deletes each entry via `delete_async(id, false)` -- removes from all CFs and HNSW indexes.
3. Tracks success/failure per entry. Failed entries remain in `soft_deleted` for retry on next cycle.
4. Does NOT clear the DashMap globally (prevents MED-15 race where new soft-deletes are lost).

GC retention period: `gc_retention_secs` (default 7 days, 604800 seconds). Set to 0 for immediate GC.

### RocksDB Compaction

After GC, `compact_range_cf` is called on all 59 column families with full-range compaction (no key bounds). This reclaims disk space from deleted entries.

### HNSW Compaction

`compact_hnsw_if_needed()` rebuilds HNSW indexes when too many removed entries accumulate. Acquires exclusive write lock on `compaction_lock`, blocking all concurrent store/delete operations during rebuild.

---

## 15. Backup and Snapshot Capabilities

### Checkpoints

RocksDB checkpoints provide point-in-time snapshots:

| Aspect | Detail |
|--------|--------|
| Path | `{db_path}/checkpoints/checkpoint_{YYYYMMDD_HHMMSS_fff}` |
| Max checkpoints | 5 (auto-cleanup of oldest) |
| Method | `RocksDB::Checkpoint::create_checkpoint` (hardlinks SST files) |
| Space cost | Minimal (hardlinks share data with live DB) |
| Restore | Not yet implemented in-place; requires restart with checkpoint path |

### Persistence (HNSW)

HNSW indexes are persisted to `CF_HNSW_GRAPHS` for fast startup recovery. On startup:
1. Try fast-path load from persisted data.
2. Fall back to O(n) rebuild from `CF_FINGERPRINTS` if load fails.
3. Clear partially-loaded indexes before rebuild to prevent duplicates.

---

## 16. Session Management at Storage Level

### Session Types

Sessions are defined in `crates/context-graph-core/src/memory/session.rs`:

```rust
pub enum SessionStatus {
    Active,      // Currently capturing memories
    Completed,   // Ended normally via SessionEnd hook
    Abandoned,   // Ended without proper closure
}
```

Sessions use an in-memory `SessionCache`, not RocksDB (invalid DB path does not cause failure).

### Topic Portfolio Persistence

Session-to-session topic continuity is stored in `CF_TOPIC_PORTFOLIO`:

| Key | Value |
|-----|-------|
| `{session_id}` | `PersistedTopicPortfolio` (JSON) |
| `"__latest__"` | Most recent portfolio (fallback sentinel) |

The `persist_topic_portfolio` method stores under both the session-specific key and `"__latest__"` for cross-session restoration.

### Session Filtering

Session filtering is NOT implemented in the storage layer. It is applied as a post-filter via `source_metadata.session_id` in the search path. The `TemporalSearchOptions` struct supports `session_id: Option<String>` for this purpose.

---

## 17. Serialization: Bincode vs JSON per CF

The storage layer follows a strict serialization policy per the constitution:

| Format | Used For | CFs |
|--------|----------|-----|
| **Bincode** | Dense vectors, fingerprints, embeddings | `fingerprints`, `e1_matryoshka_128`, `e12_late_interaction`, `topic_profiles`, `emb_0`..`emb_12`, graph edge data, `file_index` |
| **JSON** | Provenance, metadata, audit, profiles | `source_metadata`, `content` (raw UTF-8), `topic_portfolio`, `audit_log`, `audit_by_target`, `merge_history`, `importance_history`, `custom_weight_profiles`, `code_entities` |

### Bincode Constraints

- Version prefix byte on all serialized types (`TELEOLOGICAL_VERSION = 1`, `GRAPH_EDGE_VERSION = 1`).
- `skip_serializing_if` MUST NOT be used with bincode (positional format causes silent field corruption).
- Fingerprint size bounds: min 5KB, max 150KB (panics on violation).
- Serialization (write path) panics on error; deserialization (read path) returns `Result` (one corrupted record must not crash the server).

### Custom Binary Formats

- **EmbedderEdge**: Compact `[version:u8][count:u32][(target:16B, similarity:f32)...]` format (20 bytes per edge).
- **Inverted index posting lists**: Sorted `Vec<Uuid>` via bincode.
- **Secondary index keys**: Fixed-size byte arrays for prefix scanning.

---

## 18. Error Handling

### Error Hierarchy

| Error Type | Crate | Used By |
|-----------|-------|---------|
| `StorageError` | context-graph-storage | Base column family operations |
| `TeleologicalStoreError` | context-graph-storage | RocksDB teleological store |
| `GraphEdgeStorageError` | context-graph-storage | Graph edge operations |
| `CodeStorageError` | context-graph-storage | Code entity operations |
| `QuantizedStorageError` | context-graph-storage | Quantized fingerprint operations |
| `IndexError` | context-graph-core | HNSW and inverted index operations |
| `CoreError` | context-graph-core | Cross-cutting (all errors convert to this) |

All storage errors include context: operation name, column family, key, and underlying cause. The system follows a **fail-fast** policy with no silent fallbacks or mock data.

### Corruption Detection

The store detects corruption at multiple levels:
- `paranoid_checks = true` in RocksDB options.
- `CorruptionDetected` error variant with MANIFEST/SST file details.
- Consistency verification at startup (non-blocking warnings).
- Version mismatch detection on deserialization.

---

## Cross-References

- **Configuration keys, env vars, defaults**: See [03_configuration.md](03_configuration.md).
- **RocksDB column families, schema versioning**: See [04_database_schema.md](04_database_schema.md).
- **Embedder architecture**: See `07_embedding_pipeline.md` for the 13-embedder system (E1-E13) when present.
- **Search strategies**: See `08_search_retrieval.md` for the 5-stage retrieval pipeline when present.
- **Graph linking**: See the graph linking architecture document for NN-Descent, edge type detection, and embedder agreement patterns.
- **Topic detection**: See the clustering documentation for HDBSCAN integration with `scan_fingerprints_for_clustering`.
- **Causal relationships**: See the causal discovery documentation for LLM-generated causal descriptions and E5 asymmetric search.
