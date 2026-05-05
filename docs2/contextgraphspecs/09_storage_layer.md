# Storage Layer

**Source files covered:**
- `crates/context-graph-storage/src/lib.rs` — crate root, re-exports
- `crates/context-graph-storage/src/error.rs` — `StorageError`, `StorageResult`
- `crates/context-graph-storage/src/column_families.rs` — base 11 CFs, option profiles, `TOTAL_COLUMN_FAMILIES = 101` (line 408), `apply_write_buffer_limits`
- `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs` — main `RocksDbTeleologicalStore` struct (line 79), `open_with_config` (line 145), HNSW load/persist/restore, lock inspection, consistency verification
- `crates/context-graph-storage/src/teleological/rocksdb_store/types.rs` — `TeleologicalStoreConfig` (line 155), `TeleologicalStoreError` (line 30)
- `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs` — store/retrieve/update/delete (`SOFT_DELETE_PREFIX = "soft_deleted::"` line 34), hard-delete batch order, GC + post-GC compaction threshold
- `crates/context-graph-storage/src/teleological/rocksdb_store/persistence.rs` — batch ops, flush/checkpoint/compact (`MAX_CHECKPOINTS = 5` line 394), HNSW persistence, topic portfolio (`__latest__` sentinel)
- `crates/context-graph-storage/src/teleological/rocksdb_store/index_ops.rs` — `add_to_indexes` / `remove_from_indexes` (compaction lock semantics, zero-norm skip, E2 sinusoidal recompute)
- `crates/context-graph-storage/src/teleological/rocksdb_store/inverted_index.rs` — E13/E6 RMW with `multi_get_cf` batching, sorted-posting-list invariant + legacy auto-sort
- `crates/context-graph-storage/src/teleological/serialization.rs` — bincode + JSON rules, `TELEOLOGICAL_VERSION = 1`, fingerprint size guard `[5_000, 150_000]` bytes
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs` — `EmbedderIndex` enum (20 variants), `all_hnsw()` returns 16
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs` — `HnswConfig` defaults, `InvertedIndexConfig` (E6/E13 vocab=30,522, max_nnz=1500)
- `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/{types.rs,ops.rs}` — `HnswEmbedderIndex` (usearch wrapper), `MAX_HNSW_VECTORS_PER_INDEX = 2_000_000`, compaction threshold (>25% orphans)
- `crates/context-graph-storage/src/teleological/indexes/registry.rs` — `EmbedderIndexRegistry` (`HashMap<EmbedderIndex, Arc<HnswEmbedderIndex>>`)
- `crates/context-graph-storage/src/teleological/indexes/metrics.rs` — `cosine_similarity` (raw [-1,1]), `distance_to_similarity` (cosine `(2-d)/2`)
- `crates/context-graph-storage/src/teleological/search/token_storage.rs` — `RocksDbTokenStorage` for E12 (`MAX_TOKENS_PER_MEMORY = 512`)
- `crates/context-graph-storage/src/teleological/search/pipeline/traits.rs` — `InMemoryTokenStorage` (`MAX_TOKEN_ENTRIES = 5_000`), `InMemorySpladeIndex` (`MAX_SPLADE_DOCS = 100_000`)
- `crates/context-graph-storage/src/graph_edges/{repository.rs, serialization.rs, types.rs, builder.rs}` — `EdgeRepository`, `BackgroundGraphBuilder`
- `crates/context-graph-storage/src/code/store.rs` — `CodeStore` for the 5 code CFs
- `crates/context-graph-storage/src/dynamicjepa/` — 28 DJ CFs (separate per-domain stores)
- `crates/context-graph-core/src/traits/teleological_memory_store/store.rs` — `TeleologicalMemoryStore` async trait (canonical interface)
- `crates/context-graph-core/src/stubs/teleological_store_stub/mod.rs` — `InMemoryTeleologicalStore` (test-only stub)
- `crates/context-graph-mcp/src/main.rs:564-791` — `PidFileGuard` (PID file + Unix flock)

Cross-references:
- [03_configuration.md](03_configuration.md) — `storage.*`, `index.*`, `TeleologicalStoreConfig` knobs
- [04_database_schema.md](04_database_schema.md) — physical layout (CF list, key encodings, version bytes)
- [05_embedding_subsystem.md](05_embedding_subsystem.md) — embedding semantics consumed by HNSW indexes
- [06_search_and_fusion.md](06_search_and_fusion.md) — search pipeline that drives index_ops/inverted_index reads

## 1. Scope

The `context-graph-storage` crate persists, in one RocksDB instance per `storage.path`:

| Domain | CF group | Module |
| --- | --- | --- |
| Memory primary records | base + teleological + quantized | `teleological/rocksdb_store/` |
| Content + source metadata | `content`, `source_metadata`, `file_index` | `teleological/rocksdb_store/{content,source_metadata,file_index}.rs` |
| Provenance + audit + lifecycle | `audit_log`, `audit_by_target`, `merge_history`, `importance_history`, `tool_call_index`, `consolidation_recommendations`, `embedding_registry` | `teleological/rocksdb_store/{audit_log,provenance_storage}.rs` |
| Causal | `causal_relationships`, `causal_by_source` | `teleological/rocksdb_store/causal_relationships.rs` |
| Constellations | `constellations`, `constellation_by_selector` | `teleological/rocksdb_store/constellation.rs` |
| Contrastive pairs | `contrastive_pairs`, `contrastive_by_kind`, `contrastive_by_anchor` | `teleological/rocksdb_store/contrastive.rs` |
| Typed-edge corpus + LLM validation | `typed_edge_records`, `typed_edge_validations` | `teleological/rocksdb_store/{typed_edge_export,llm_validation}.rs` |
| Training/learning | `training_records`, `learning_events` | `teleological/rocksdb_store/{export,learning}.rs` |
| Learner state (UTL) | `learner_*`, `fingerprints_learner`, `goal_centroids`, `learner_training_datasets` | `teleological/rocksdb_store/{learner,learner_training}.rs` |
| Code | `code_entities`, `code_e7_embeddings`, `code_file_index`, `code_name_index`, `code_signature_index` | `code/store.rs` |
| Graph edges | `embedder_edges`, `typed_edges`, `typed_edges_by_type` | `graph_edges/repository.rs` |
| DynamicJEPA | 28 `dj_*` CFs | `dynamicjepa/` (separate stores per domain) |

Total: `TOTAL_COLUMN_FAMILIES = 101` (`column_families.rs:408`).

Single-writer enforcement is at the MCP layer via `PidFileGuard` on `<storage.path>/mcp.pid` (see [04_database_schema.md](04_database_schema.md) §11).

## 2. Repository / Store Pattern

The crate uses an "owned `Arc<DB>` + per-domain repository struct" pattern. There is NO single trait that aggregates all repositories; each domain uses its own struct and its own error type.

| Type | Owns | Async? | Pattern |
| --- | --- | --- | --- |
| `RocksDbTeleologicalStore` (`teleological/rocksdb_store/store.rs:79`) | `Arc<DB>`, `Cache`, `EmbedderIndexRegistry`, `CausalE11Index`, soft-delete `DashMap`, locks | yes (`async` methods, often `spawn_blocking`) | Implements `context_graph_core::traits::TeleologicalMemoryStore` (defined in `core/src/traits/teleological_memory_store/store.rs:48`) |
| `InMemoryTeleologicalStore` (`core/src/stubs/teleological_store_stub/mod.rs:64`) | `RwLock<HashMap>` | yes | Same trait; test-only stub |
| `EdgeRepository` (`graph_edges/repository.rs:33`) | `Arc<DB>` (cloned from teleological store) | sync | Owns `embedder_edges`, `typed_edges`, `typed_edges_by_type`; FAIL FAST errors |
| `CodeStore` (`code/store.rs:42`) | `Arc<DB>` | sync | Owns 5 `code_*` CFs |
| `CausalE11Index` (`teleological/rocksdb_store/causal_hnsw_index.rs`) | usearch HNSW | sync | E11 ANN over causal relationships |
| `EmbedderIndexRegistry` (`teleological/indexes/registry.rs:71`) | 16 `HnswEmbedderIndex` instances | sync (internal `parking_lot::RwLock`) | One per HNSW-capable embedder. `get(embedder)` returns `None` for E6/E12/E13; `get_or_panic` panics with FAIL FAST. Asserts `len() == 16` at construction. |

The DB handle is wrapped in `Arc<DB>` so multiple repositories can share it without re-opening.

`TeleologicalMemoryStore` async trait surface (excerpt):

| Method | Purpose |
| --- | --- |
| `store(fp) -> Uuid` | Single store; rejects soft-deleted IDs |
| `retrieve(id) -> Option<TeleologicalFingerprint>` | Returns `None` on soft-delete |
| `update(fp) -> bool` | Replace entire record; ID must exist |
| `delete(id, soft) -> bool` | `soft=true` ⇒ marker only; `soft=false` ⇒ hard delete (see §6) |
| `search_semantic`, `search_text`, `search_sparse`, etc. | Multi-space and pipeline searches |
| `store_embedding_version` | Provenance into `CF_EMBEDDING_REGISTRY` |

## 3. Open Path and Shared Invariants

`RocksDbTeleologicalStore::open_with_config(path, config)` (`store.rs:145`):

1. If `path` exists: `inspect_rocksdb_lock(path)` (`store.rs:472`) — non-blocking `fcntl(F_SETLK, F_WRLCK)` probe, never removes the LOCK file — and `assert_existing_dynamicjepa_column_families(path)` (`store.rs:368`) — FAIL FAST if any of the 28 DJ CFs are missing from an existing DB (no auto-add for DJ).
2. Build shared `Cache::new_lru_cache(config.block_cache_size)` (default 256MB).
3. Build DB `Options`: `create_if_missing=true`, `create_missing_column_families=true`, `set_max_open_files`, `set_paranoid_checks(true)`. WAL toggle from `config.enable_wal` (when false, sets `manual_wal_flush(true)`).
4. `DB::open_cf_descriptors` with all 101 descriptors. Missing-SST and corrupted-MANIFEST errors are translated to `TeleologicalStoreError::CorruptionDetected` with extracted file names by `transform_corruption_error` (`store.rs:576`).
5. Verify all 28 DJ CF handles after open; FAIL with `ColumnFamilyNotFound` if any missing (`store.rs:200-210`).
6. Construct `EmbedderIndexRegistry::new()` (16 HNSW indexes) and `CausalE11Index::new()`.
7. **Bootstrap soft-delete state** from `CF_SYSTEM` keys with `SOFT_DELETE_PREFIX = "soft_deleted::"` (`crud.rs:34`). Each value is an `i64` BE Unix-epoch-millis. Loaded into `soft_deleted: Arc<DashMap<Uuid, i64>>`. Legacy 1-byte markers are accepted with timestamp 0 (immediate GC eligible).
8. `total_doc_count = raw_fingerprints - soft_deleted.len()` for BM25-IDF (`store.rs:281-299`). `raw_fp_count` is captured here and reused by `verify_consistency` to avoid a second O(n) scan.
9. `try_load_hnsw_indexes()` (`store.rs:1038`): for each HNSW index, read `graph:{Embedder}` + `meta:{Embedder}` from `CF_HNSW_GRAPHS` and `restore_from_persisted`. **E2 is always skipped** (`store.rs:1054-1059`) so its sinusoidal-PE vectors are recomputed from `created_at` on rebuild. Returns `Ok(true)` if any index loaded; `Ok(false)` if none; `Err` if any deserialization failed (caller falls back to full rebuild after `clear_all`).
10. If `try_load_hnsw_indexes` returned true and any registered HNSW index is empty (e.g. a newly-enabled embedder with no persisted data), `clear_all()` + `rebuild_indexes_from_store()` to avoid duplicates from partial overlap (`store.rs:316-339`).
11. If `E11_ENTITY_ENABLED`, `rebuild_causal_e11_index` from `CF_CAUSAL_RELATIONSHIPS` (skipped when E11 disabled — KEPLER produces near-identical vectors).
12. `verify_consistency(raw_fp_count)` (`store.rs:406`): cross-check fingerprint counts vs `CF_CONTENT`, HNSW size vs live fingerprint count (10% tolerance, min 5), update `total_doc_count` if drift detected. Logs warnings only; **never blocks startup**.

Shared in-memory state on the store struct (`store.rs:79`):

| Field | Type | Purpose |
| --- | --- | --- |
| `db` | `Arc<DB>` | Shared RocksDB handle |
| `cache` | `Cache` | Shared 256MB LRU block cache |
| `path` | `PathBuf` | Storage path (also where `mcp.pid` lives) |
| `fingerprint_count` | `parking_lot::RwLock<Option<usize>>` | Cached count, invalidated on writes (parking_lot, non-poisonable) |
| `total_doc_count` | `Arc<AtomicUsize>` | O(1) for IDF; `fetch_update` with checked subtraction (no underflow) |
| `soft_deleted` | `Arc<DashMap<Uuid, i64>>` | Lock-free concurrent soft-delete set keyed by UUID, value = Unix-millis |
| `index_registry` | `Arc<EmbedderIndexRegistry>` | 16 HNSW indexes (asserts `len() == 16`) |
| `causal_e11_index` | `Arc<CausalE11Index>` | E11 ANN over `causal_relationships` |
| `secondary_index_lock` | `parking_lot::Mutex<()>` | Serializes RMW on E13/E6 inverted indexes, `causal_by_source`, `file_index` |
| `compaction_lock` | `parking_lot::RwLock<()>` | Read-locked by store/delete, write-locked by HNSW rebuild/compaction |

## 4. Atomicity and Batching

### 4.1 Single-fingerprint store

`store_async` (`crud.rs:50`) and `store_fingerprint_internal` (`store.rs:1144`):

1. Reject if `is_soft_deleted(id)` (FAIL FAST per MED-13 — `crud.rs:60-72`).
2. Acquire `secondary_index_lock` mutex (STG-04 — held across the whole RMW + write).
3. Build `WriteBatch` containing: `CF_FINGERPRINTS` put, `CF_E1_MATRYOSHKA_128` put (truncated first 128 dims of E1), E13 SPLADE inverted-index updates (RMW via `multi_get_cf` + sorted insert + `serialize_memory_id_list`), E6 sparse inverted-index updates (when `fp.e6_sparse` present), `CF_E12_LATE_INTERACTION` put (when non-empty).
4. **`CF_TOPIC_PROFILES` is NOT written here** — topic profiles are computed asynchronously by the clustering pipeline (`detect_topics`, `get_topic_portfolio`) and written separately. Hard-delete still cleans up `CF_TOPIC_PROFILES` defensively (`crud.rs:365-366`).
5. **`CF_CONTENT` and `CF_SOURCE_METADATA` are NOT in this batch either** — they have separate writers in `content.rs` / `source_metadata.rs` modules.
6. `db.write(batch)` is the atomicity boundary. RocksDB applies the batch atomically across all CFs.
7. Drop `secondary_index_lock` (STOR-M2: only after batch commit is durable).
8. THEN `add_to_indexes(&fp)` (`index_ops.rs:25`) acquires `compaction_lock.read()` and inserts into all HNSW-capable indexes. Zero-norm vectors are skipped (warn for non-temporal embedders, debug for E2/E3/E4 legacy data). Skips E11 entirely when `E11_ENTITY_ENABLED=false`.
9. Increment `total_doc_count` only when this is a new insert (`count_as_new=true`).
10. If `add_to_indexes` fails, rollback via `delete_async(id, false)` (hard delete cleans ALL CFs, not just `CF_FINGERPRINTS`) (`crud.rs:88-109`).

### 4.2 Batch store (`store_batch_async`, `persistence.rs:48-152`)

- Iterates fingerprints; for each, calls `store_fingerprint_internal(&fp, true)` (sync I/O).
- Rejects soft-deleted IDs (STOR-5).
- On HNSW failure (`add_to_indexes`), attempts **rollback** by `delete_async(id, false)` (hard delete) for that single fingerprint and continues with the next.
- Returns the list of successfully stored UUIDs.
- HIGH-7: only fails entirely (returns Err) if ALL fingerprints failed; otherwise returns partial success with logged failures.

### 4.3 Update

`update_async` (`crud.rs:147`) performs a full replacement:

1. Read old raw bytes for rollback safety; deserialize old fingerprint.
2. Under `secondary_index_lock`: write batch removing OLD E13 + E6 terms, then `db.write(batch)` (STG-04).
3. `remove_from_indexes(id)` (HNSW maps).
4. `store_fingerprint_internal(&fingerprint, false)` (`count_as_new=false` — update, not new doc).
5. `add_to_indexes(&fingerprint)`.
6. On failure at step 5: under `secondary_index_lock`, remove the NEW fingerprint's E13/E6 terms (M2: prevents orphaned new postings after rollback), then restore old fingerprint via `store_fingerprint_internal(&old_fp, false)` and `add_to_indexes(&old_fp)`. Failures during rollback are logged at error level — the fingerprint is in a partially-recovered state requiring manual cleanup.

STOR-7 NOTE: There is a brief transient window between removing old inverted-index terms and adding new ones where a concurrent reader may see the fingerprint missing from inverted indexes but still present in HNSW. Self-healing on completion; accepted trade-off vs holding `secondary_index_lock` across the entire operation.

## 5. HNSW Lifecycle

### 5.1 Index registry

`EmbedderIndexRegistry::new()` constructs 16 `HnswEmbedderIndex` instances at startup (`indexes/registry.rs:87` asserts `len() == 16`). The list of HNSW-capable embedders is `EmbedderIndex::all_hnsw()` in `indexes/hnsw_config/embedder.rs:186`.

| HNSW index | Dim | Metric (from `indexes/metrics.rs`) |
| --- | --- | --- |
| E1Semantic | 1024 | Cosine |
| E1Matryoshka128 | 128 | Cosine |
| E2/E3/E4 (TemporalRecent / Periodic / Positional) | 512 each | Cosine |
| E5Causal, E5CausalCause, E5CausalEffect | 768 each | Asymmetric cosine |
| E7Code | 1536 | Cosine |
| E8Graph | 1024 | Cosine |
| E9HDC | 1024 | Cosine |
| E10Multimodal, E10MultimodalParaphrase, E10MultimodalContext | 768 each | Cosine |
| E11Entity (Kepler) | 768 | Cosine |
| E14BgeM3Dense | 1024 | Cosine |

E6 / E12 / E13 are NOT HNSW and live in their own CFs/structures (inverted index for E6/E13, MaxSim for E12).

### 5.2 Add/remove

- `add_to_indexes(&fp)` (called after RocksDB commit): inserts each per-embedder vector into its HNSW index. Read lock on `compaction_lock`.
- `remove_from_indexes(id)` (called from hard-delete after RocksDB batch commit): best-effort; failures are logged but do not fail the delete (orphans are filtered at search time).

### 5.3 Persistence to `CF_HNSW_GRAPHS`

| Key | Value | Notes |
| --- | --- | --- |
| `graph:{embedder_name}` | usearch `save_to_buffer` bytes (1-100MB) | LZ4 compressed by RocksDB; ~30-50% reduction |
| `meta:{embedder_name}` | JSON `{ mappings: Vec<(Uuid, u64)>, next_key: u64 }` (`HnswPersistMetadata`, `hnsw_impl/types.rs:368`) | UUID↔usearch-key mapping + next monotonic key |

Persistence is **periodic + on shutdown** (`persist_hnsw_indexes`, `store.rs:914`), not on every write. Empty indexes are skipped. On open, `try_load_hnsw_indexes()` attempts to restore each index. **Critical caveat:** it returns `Ok(true)` if ANY index loads — newly-enabled embedders end up with empty HNSW until the post-load empty-index detection at `store.rs:324-338` triggers `clear_all() + rebuild_indexes_from_store()`.

### 5.4 Rebuild

`rebuild_indexes_from_store` (`store.rs:686`):
- Takes WRITE side of `compaction_lock` — blocks all concurrent store/delete (DATA-5).
- Iterates `CF_FINGERPRINTS`, skipping soft-deleted IDs.
- Recomputes E2 vector via `compute_decay_embedding(fp.created_at, None, &DEFAULT_DECAY_RATES)` (sinusoidal PE) — fixes degenerate legacy E2 vectors (delta≈0 decay).
- Inserts via `add_to_indexes_unlocked` (compaction lock already held).
- FAIL FAST: any add error fails the rebuild (no partial success).

The fast-path `try_load_hnsw_indexes` always skips persisted E2 HNSW (`store.rs:1054-1059`) so E2 is recomputed every startup until the comment-flagged removal once all deployments have run sinusoidal rebuild.

`compact_hnsw_if_needed` (`store.rs:981`) checks each index's `removed_count() / usearch_size() > 25%` orphan ratio (`HnswEmbedderIndex::needs_compaction`, `hnsw_impl/types.rs:211`). If any index needs compaction, calls `rebuild_indexes_from_store()` which clears + rebuilds, eliminating orphans. CausalE11Index also checked when E11 enabled.

### 5.5 ID mapping

usearch operates on `u64` keys, not UUIDs. Each `HnswEmbedderIndex` (`hnsw_impl/types.rs:76`) maintains:

| Field | Type | Purpose |
| --- | --- | --- |
| `index` | `parking_lot::RwLock<usearch::Index>` | Underlying HNSW graph |
| `id_to_key` | `parking_lot::RwLock<HashMap<Uuid, u64>>` | UUID → usearch key |
| `key_to_id` | `parking_lot::RwLock<HashMap<u64, Uuid>>` | usearch key → UUID (search results) |
| `next_key` | `parking_lot::RwLock<u64>` | Monotonically increasing next key |
| `removed_count` | `AtomicUsize` | Orphans (removed from maps but still in usearch graph) |

`remove(id)` (`hnsw_impl/ops.rs:95`) deletes from both maps and increments `removed_count`. usearch itself does NOT support vector deletion — orphans accumulate until compaction rebuild reclaims them. Search filters via `key_to_id.get(key)` post-lookup.

`insert(id, vector)` rejects when `key_to_id.len() >= MAX_HNSW_VECTORS_PER_INDEX (2_000_000)` (H1 fix). Doubles capacity when usearch's `size() >= capacity()`. Updates re-use the UUID slot but leave the old vector orphaned (no usearch deletion).

## 6. Soft-Delete and Hard-Delete

### 6.1 Soft-delete (`crud.rs:284-331`)

1. Insert `(id, now_millis)` into the in-memory `soft_deleted: DashMap`.
2. Write `("soft_deleted::{uuid}", now_millis_be_bytes)` to `CF_SYSTEM` so the marker survives restart (`SEC-06-FIX`).
3. Decrement `total_doc_count` via `fetch_update` with checked subtraction (prevents `usize::MAX` underflow that corrupts BM25-IDF).
4. Invalidate the `fingerprint_count` cache.

The fingerprint record itself remains in `CF_FINGERPRINTS`. It is filtered out at every read site (`is_soft_deleted` check). Search results, K-NN traversal, content reads, and audit reads all gate on this set.

### 6.2 Hard-delete order (`crud.rs:332-475`)

The order is intentional and tested:

1. Capture `was_soft_deleted = is_soft_deleted(id)` so we know whether to decrement `total_doc_count` later (avoid double-decrement when GC runs `delete(id, false)` on an already-soft-deleted ID).
2. Acquire `_index_guard = secondary_index_lock.lock()`.
3. Read existing fingerprint bytes from `CF_FINGERPRINTS`. If absent, return `Ok(false)`.
4. Try `deserialize_teleological_fingerprint(bytes)`; if it fails, log and proceed without inverted-index cleanup (orphans are harmless on read).
5. Build `WriteBatch` with deletes from: `CF_FINGERPRINTS`, `CF_TOPIC_PROFILES`, `CF_E1_MATRYOSHKA_128`, `CF_E13_SPLADE_INVERTED` (term-by-term, cleaned via `remove_from_splade_inverted_index`), `CF_E6_SPARSE_INVERTED` (when present), `CF_CONTENT`, `CF_E12_LATE_INTERACTION`, `CF_SOURCE_METADATA` (DAT-7), `CF_EMBEDDING_REGISTRY` (current-state provenance — must not survive hard-delete), every `CF_EMB_*` (DAT-7; no-op until quantized write path is wired), and the `CF_SYSTEM` soft-delete marker key.
6. **Commit RocksDB batch FIRST** (`db.write(batch)`, `crud.rs:434`). This is the atomicity boundary. Rationale (FIX-M5): if RocksDB succeeds and HNSW fails, search returns a non-existent UUID that is filtered post-fetch (orphan, harmless). If HNSW were first and RocksDB failed, data would be visible but un-findable (worse).
7. **Drop `_index_guard` AFTER batch commit** is durable (STOR-M2: prevents lost-update if a concurrent store re-adds postings while batch is in flight).
8. **Best-effort HNSW removal** via `remove_from_indexes(id)` (`crud.rs:442-444`). Failure logs a warning (`Hard-delete: HNSW index removal failed (orphan will be filtered at search time)`) but does NOT fail the delete.
9. Invalidate `fingerprint_count` cache.
10. If NOT `was_soft_deleted`, decrement `total_doc_count` via checked `fetch_update(|c| if c > 0 { Some(c-1) } else { None })` (STOR-H1 underflow prevention; M1 FIX prevents double-decrement after soft-delete + GC hard-delete).

KNOWN ORPHANS (acceptable, by design):
- `CF_FILE_INDEX`: no reverse lookup `(fingerprint_id → file_path)` so cleanup is skipped on hard-delete (`crud.rs:412-414`). `get_fingerprints_for_file` returns stale UUIDs that fail retrieval (filtered).
- HNSW indexes: usearch does not support vector deletion. Vectors stay in graph, removed only from `id_to_key`/`key_to_id` maps. Reclaimed on next compaction (>25% orphan threshold).
- E6/E13 inverted indexes when fingerprint deserialization fails at step 4: orphan terms point to a deleted ID and are filtered post-deserialize at search time.

### 6.3 Soft-delete GC (`crud.rs:493-617`)

`gc_soft_deleted(retention_secs) -> usize`:
- Cutoff = `now_millis - retention_secs * 1000`.
- Snapshot expired IDs from the `DashMap` (lock-free).
- For each expired entry, call `delete_async(id, false)` (hard delete). Per-entry failures are logged and skipped (entry stays in `soft_deleted` for retry next cycle).
- After GC: `soft_deleted.shrink_to_fit()` releases peak DashMap capacity (M9 FIX).
- After GC: when `deleted >= GC_COMPACTION_THRESHOLD = 10` (`crud.rs:557`), trigger `db.compact_range_cf` on the 6 primary CFs (`CF_FINGERPRINTS`, `CF_SOURCE_METADATA`, `CF_TOPIC_PROFILES`, `CF_E12_LATE_INTERACTION`, `CF_E13_SPLADE_INVERTED`, `CF_E1_MATRYOSHKA_128`) plus all 14 quantized embedder CFs (M7 FIX). Below threshold, compaction is skipped to avoid I/O churn.
- Returns the number of entries hard-deleted.
- `TeleologicalStoreConfig.gc_retention_secs` defaults to 7 days (`types.rs:180`); `0` means GC immediately on next run.

`compact_async` (`persistence.rs:466`) does the same hard-delete-soft-deleted-then-compact-all-CFs pass but synchronously across the full CF set (`cf_names::ALL` + teleological + quantized + code + causal + DJ). Failed hard-deletes remain in `soft_deleted` for the next compaction (MED-14: do NOT call `clear()`).

## 7. Encoding Rules

Per project convention (`teleological/serialization.rs:7-13`, `code/store.rs:38-40`):

| Data shape | Format | Why |
| --- | --- | --- |
| `TeleologicalFingerprint` (dense vectors, no skip_serializing_if) | bincode | Positional layout; small |
| `SemanticFingerprint` | bincode | Same |
| `Vec<f32>` embeddings (`code_e7_embeddings`, etc.) | raw `Vec<f32>` | Smallest for fixed dimension |
| `SourceMetadata`, `MergeRecord`, `ImportanceChangeRecord`, `EmbeddingVersionRecord`, `AuditRecord` | JSON | Contains optional fields with `#[serde(skip_serializing_if = ...)]`, which BREAKS bincode. JSON is mandatory here. |
| `CodeEntity`, `CodeFileIndexEntry`, `Vec<UUID>` indexes (code) | JSON | Metadata, not dense vectors |
| `PersistedTopicPortfolio`, `[f32; 14]` weight profiles | JSON | Human-readable, evolvable |
| `LearningEvent`, `LearnerProfile`, all learner records | bincode (with `[v:u8]` prefix) | Dense, no skip_serializing_if |
| `TrainingRecord`, `LearnerTrainingDataset` | bincode (with `[v:u8]` prefix) | Same |
| `Constellation`, `ContrastivePair`, `TypedEdgeTrainingRecord`, `LLMEdgeValidation` | bincode (with `[v:u8]` prefix) | Same |
| `CausalRelationship` | bincode | Text + dual E5 vectors |
| HNSW graphs in `CF_HNSW_GRAPHS` | usearch native bytes (graph) + JSON (meta) | usearch dictates graph format |

**Hard rule:** `bincode + #[serde(skip_serializing_if = ...)] = SILENT DATA CORRUPTION.` Bincode is positional; skipped fields shift subsequent field offsets between writer and reader, corrupting all fields after the skip. Optional/provenance records MUST use JSON column families.

`TeleologicalFingerprint` size guard (`serialization.rs:44,56`):
- `MIN_FINGERPRINT_SIZE = 5_000` (5KB) — below this means missing dense embeddings.
- `MAX_FINGERPRINT_SIZE = 150_000` (150KB) — above this means oversized sparse/token storage.
- Outside the range ⇒ `panic!` (FAIL FAST per STG-01: serialization is on the write path of in-memory data we control).
- Deserialization returns `CoreError::SerializationError` rather than panicking, so one corrupted record cannot crash an iteration.

## 8. Errors

### 8.1 `StorageError` (`error.rs`)

| Variant | Trigger |
| --- | --- |
| `OpenFailed { path, message }` | DB open failure (transformed for missing-SST corruption) |
| `ColumnFamilyNotFound { name }` | Missing CF handle after open |
| `WriteFailed(String)` | RocksDB write error |
| `ReadFailed(String)` | RocksDB read error |
| `FlushFailed(String)` | RocksDB flush error |
| `NotFound { id }` | Constructed via `not_found_node`, `not_found_edge`, `not_found_embedding` |
| `Serialization(String)` | bincode/JSON failure (downstream of `From<ValidationError>`) |
| `ValidationFailed(String)` | `ValidationError` (e.g. `InvalidEmbeddingDimension`) coerced via `From` |
| `IndexCorrupted { index_name, details }` | Detected during scan (e.g. `temporal` UUID parse failure) |
| `Internal(String)` | Catch-all incl. `From<rocksdb::Error>` |

### 8.2 `TeleologicalStoreError` (`teleological/rocksdb_store/types.rs:30-115`)

| Variant | Notes |
| --- | --- |
| `RocksDbOperation { operation, cf, key, source }` | Detailed per-op error with CF and key context |
| `OpenFailed { path, message }` | DB open |
| `ColumnFamilyNotFound { name }` | CF handle missing |
| `Serialization { id, message }` | Per-fingerprint serialization |
| `Deserialization { key, message }` | Per-key deserialization |
| `Validation { id, message }` | Fingerprint validation |
| `IndexOperation { index_name, message }` | HNSW/inverted index failure |
| `CheckpointFailed { message }` | Checkpoint creation |
| `RestoreFailed { path, message }` | Restore from checkpoint |
| `StaleLockCleanupFailed { path, message }` | RocksDB LOCK reclamation |
| `CorruptionDetected { path, missing_count, missing_files, manifest_file }` | MANIFEST references missing SSTs; FAIL FAST, no auto-recovery |
| `Internal(String)` | Catch-all |
| `ComparisonValidation(ComparisonValidationError)` | Synergy / weight-sum violations from core types |

`TeleologicalStoreError` is convertible into `CoreError` via `From`. `CoreError` is convertible back via `From` for ergonomic `?` use across crate boundaries.

### 8.3 `GraphEdgeStorageError` (`graph_edges/types.rs`), `CodeStorageError` (`code/error.rs`), `QuantizedStorageError`

Per-domain typed errors with similar shape: `RocksDb { source, cf }`, `Serialization`, `Deserialization`, `NotFound`. Distinct from `StorageError` to keep domain failures separable.

## 9. Read-After-Write Consistency

| Operation | Consistency |
| --- | --- |
| RocksDB single-CF read/write | Sequentially consistent within a single `DB` handle |
| RocksDB `WriteBatch` | Atomic across CFs (all-or-nothing) |
| `db.write(batch)` then read | Read sees the write (RocksDB MVCC) |
| HNSW read after `add_to_indexes` | Eventually visible (usearch internal lock; reads after add_to_indexes return success will see the new vector) |
| Inverted index read after RMW under `secondary_index_lock` | Sequentially consistent within the lock scope |
| Soft-delete marker survives restart | Yes — persisted in `CF_SYSTEM`; loaded on open into `DashMap` |
| `total_doc_count` after restart | Recomputed: `raw_fingerprint_count - persisted_soft_deleted.len()` |
| Crash after RocksDB commit but before HNSW add | HNSW lacks the entry (orphan in RocksDB; not findable until rebuild) |
| Crash after HNSW remove but before RocksDB delete commit | Cannot happen — RocksDB commit is the FIRST step of hard-delete |

## 10. Backup and Snapshots

Implemented in `teleological/rocksdb_store/persistence.rs` (and trait surface in `core/src/traits/teleological_memory_store/store.rs`):

| API | Backing mechanism | Notes |
| --- | --- | --- |
| `flush_async()` | `db.flush_cf(cf)` for every CF in the union of {base, teleological, quantized, code, causal, DJ} | Forces memtable → SST; uses `spawn_blocking` |
| `compact_async()` | (1) Hard-delete every soft-deleted entry from RocksDB; (2) `db.compact_range_cf(cf, None, None)` for every CF | Infrequent; failed hard-deletes retained in `soft_deleted` for retry |
| `checkpoint() / checkpoint_sync()` | RocksDB `Checkpoint::create_checkpoint(<path>/checkpoints/checkpoint_{ts})` (hard-linked SST files into a sibling dir) | Auto-rotated to keep `MAX_CHECKPOINTS = 5` (`persistence.rs:394`); `TeleologicalStoreError::CheckpointFailed` on error |
| `restore_async(path)` | NOT IMPLEMENTED: returns `RestoreFailed` with instruction to restart the application pointing at the checkpoint path | In-place restore would require stopping background tasks and reopening the DB |
| Topic portfolio persistence | `persist_topic_portfolio_async(session_id, portfolio)` writes JSON to `CF_TOPIC_PORTFOLIO` under both `session_id` and `__latest__` (`LATEST_PORTFOLIO_KEY = b"__latest__"`, `persistence.rs:561`) | Loaded on session start via `load_topic_portfolio_async` or `load_latest_topic_portfolio_async` |
| HNSW persistence | `persist_hnsw_indexes()` writes `graph:{e}` + `meta:{e}` to `CF_HNSW_GRAPHS` for every non-empty index | Periodic + on shutdown; empty indexes skipped |

There is NO automatic backup, NO incremental backup, NO replication, NO snapshot rotation policy beyond auto-pruning the `<path>/checkpoints/` directory to the most recent 5.

## 11. Versioning and Migration

Schema evolution is **wipe-and-rebuild**. Every record type carries a one-byte version prefix:

| Record | Version constant | CF |
| --- | --- | --- |
| TeleologicalFingerprint | `TELEOLOGICAL_VERSION = 1` | `CF_FINGERPRINTS` |
| TrainingRecord | `TRAINING_RECORD_VERSION = 3` | `CF_TRAINING_RECORDS` |
| Constellation | `CONSTELLATION_VERSION = 2` | `CF_CONSTELLATIONS` |
| ContrastivePair | `CONTRASTIVE_PAIR_VERSION = 1` | `CF_CONTRASTIVE_PAIRS` |
| TypedEdgeTrainingRecord | `TYPED_EDGE_RECORD_VERSION = 1` | `CF_TYPED_EDGE_RECORDS` |
| LLMEdgeValidation | `LLM_EDGE_VALIDATION_VERSION = 1` | `CF_TYPED_EDGE_VALIDATIONS` |
| LearningEvent | `LEARNING_EVENT_VERSION = 1` | `CF_LEARNING_EVENTS` |
| Learner records (all 11 learner CFs) | `LEARNER_RECORD_VERSION = 1` | per-learner CF set |
| LearnerTrainingDataset | `LEARNER_TRAINING_DATASET_VERSION = 1` | `CF_LEARNER_TRAINING_DATASETS` |
| DJ AuditRecord | `DJ_AUDIT_RECORD_VERSION = 2` | `dj_audit_log` |

Mismatched version bytes return a `SerializationError` describing the mismatch and stating "No automatic migration is supported - data must be regenerated." Adding a CF: auto-creates on next open thanks to `create_missing_column_families = true`. Removing a CF: requires deleting the DB (RocksDB requires opening every existing CF).

## 11.4 CodeStore (`code/store.rs`)

`CodeStore` (`code/store.rs:61`) wraps `Arc<DB>` and operates on the 5 code CFs:

| CF | Key | Value | Format |
| --- | --- | --- | --- |
| `code_entities` | UUID | `CodeEntity` | JSON (metadata) |
| `code_e7_embeddings` | UUID | `Vec<f32>` 1536D = 6144B | bincode (dense vectors) |
| `code_file_index` | file_path bytes (UTF-8) | `CodeFileIndexEntry` | JSON |
| `code_name_index` | entity_name bytes (UTF-8) | `Vec<UUID>` | JSON |
| `code_signature_index` | `SHA256(signature)` 32B | `Vec<UUID>` | JSON |

Constitution rule applied here verbatim (`code/store.rs:38-40`): "JSON for provenance/metadata/audit. Bincode for dense vectors only." Each entity's full `SemanticFingerprint` (all 14 embedders) is stored alongside the entity for multi-space code search; the dense fingerprint stays bincode while the entity wrapper is JSON.

`CodeStore::open(path)` opens a standalone RocksDB at the given path with a 64MB block cache (vs. the 256MB shared cache in the main store). Inside the main DB, `CodeStore::new(db_arc)` shares the existing `Arc<DB>` and reuses the main 256MB cache.

## 11.5 Graph Edges (`EdgeRepository`)

`EdgeRepository` (`graph_edges/repository.rs:33`) wraps the same `Arc<DB>` as `RocksDbTeleologicalStore` (via `db_arc()`) and owns three CFs:

| CF | Operation surface |
| --- | --- |
| `embedder_edges` | `store_embedder_edges(emb_id, source, edges)` / `get_embedder_edges` / `delete_embedder_edges` / `iter_embedder_edges(emb_id)`; `embedder_id` validated against `NUM_EMBEDDERS=14` |
| `typed_edges` | `store_typed_edge(edge)` / `get_typed_edge(source, target)` / `get_typed_edges_from(source)` (16B prefix scan) / `get_typed_edges_to(target)` (full O(n) CF scan, no reverse index) / `iter_all_typed_edges()` / `delete_typed_edge` / `store_typed_edges_batch` |
| `typed_edges_by_type` | Secondary index — written atomically alongside `typed_edges` via the same `WriteBatch` (`repository.rs:241-247`) |

Typed-edge secondary index encoding (post-fix; legacy 17-byte key still readable):
- Primary key (`typed_edges`): `[source:16][target:16]` = 32 bytes; value = `[v:u8 = 1][bincode TypedEdge]`.
- Secondary key (`typed_edges_by_type`): `[edge_type:u8][source:16][target:16]` = 33 bytes; value = `target_uuid` (16B). The 17B-suffix form `[type][source]` (with target only in the value) silently lost edges when the same `(type, source)` had multiple targets — fixed by writing the full 33-byte key.
- Read path: `get_typed_edges_by_type(source, type)` iterates with the 17-byte prefix `[type][source]`, reading the target UUID from key bytes 17..33 when available, otherwise from the value (legacy fallback). It then reads each edge from the primary `typed_edges` CF (`repository.rs:344-424`).
- Direction-aware lookup at the MCP layer (`graph_link_tools.rs:397-510`): outgoing uses the secondary index; incoming uses `get_typed_edges_to` + post-filter (full scan); both unions the two.

`BackgroundGraphBuilder` (`graph_edges/builder.rs:156`):
- Default `GraphBuilderConfig`: `batch_interval_secs = 60`, `max_batch_size = 1000`, `active_embedders = [0, 4, 6, 7, 9, 10]` (E1, E5, E7, E8, E10, E11), `process_on_max = true` (line 96-104).
- `process_batch()` drains the queue (up to `max_batch_size`), computes K-NN for each active embedder via in-memory HNSW search, writes posting lists to `embedder_edges`, and emits an audit record on completion.
- `rebuild_all()` recomputes K-NN for every fingerprint across active embedders (used after large ingestion or HNSW compaction).

## 12. Concurrency Notes

- `parking_lot` is used everywhere (`RwLock`, `Mutex`) to avoid poison cascades — one panic no longer breaks all subsequent operations (HIGH-17, MED-11 fix).
- `DashMap` for `soft_deleted` instead of `RwLock<HashMap>` (P5 fix) eliminates contention on the read-heavy search path (5,000+ lookups under load).
- `secondary_index_lock` (mutex) serializes RMW on E13/E6 inverted indexes, `causal_by_source`, and `file_index` to prevent lost-update races (STG-03/04/10 fix). Held across the entire RMW + `db.write(batch)` so concurrent writers cannot read stale posting lists.
- `compaction_lock` (RwLock) gates HNSW rebuild against concurrent stores: rebuild takes write-lock, store/delete take read-lock (DATA-5 fix). Compaction is infrequent (~10min or manual) so write-lock contention is negligible.
- Background batch ops (`store_batch_async`, `retrieve_batch_async`, `count_async`, `flush_async`, `scan_fingerprints_for_clustering_async`, `list_fingerprints_unbiased_async`) use `tokio::task::spawn_blocking` to keep RocksDB I/O off the async runtime. `Arc::clone(&self.db)` and `Arc::clone(&self.soft_deleted)` are passed into the closure — never the raw maps (cloning a `DashMap` would defeat the lock-free guarantee).
- `parking_lot::RwLockReadGuard` is `!Send`; call sites must drop the guard before any `.await` inside `tokio::spawn`.
- Posting-list invariant (`inverted_index.rs:25-34`): E6/E13 posting lists are stored sorted by UUID for binary-search RMW. Legacy unsorted lists are sorted in-place on first read and written back to fix the stored order.
- Inverted-index RMW uses `db.multi_get_cf` to batch all term-key reads (`inverted_index.rs:52, 87, 208`), avoiding per-term sequential `db.get_cf` round-trips (Audit-10 STOR-M3).

## 13. What Is NOT Covered Here

- Embedding model loading / VRAM allocation — see [05_embedding_subsystem.md](05_embedding_subsystem.md) and `crates/context-graph-embeddings/`.
- Search / fusion algorithms — see [06_search_and_fusion.md](06_search_and_fusion.md).
- Causal discovery LLM provenance — see [07_graph_and_causal.md](07_graph_and_causal.md).
- Topic clustering thresholds — see [12_clustering_and_topics.md](12_clustering_and_topics.md).
- Per-CF block size / write-buffer / prefix tuning rationale — see [04_database_schema.md](04_database_schema.md) §12.
- BlackBox privacy/retention/erasure ledger writes (Phase 13): live in tenant SQLite ledgers under `crates/context-graph-cli/src/commands/blackbox.rs`, not in this RocksDB schema.
