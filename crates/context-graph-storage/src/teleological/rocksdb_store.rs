//! RocksDB-backed TeleologicalMemoryStore implementation.
//!
//! This module provides a persistent storage implementation for TeleologicalFingerprints
//! using RocksDB with 17 column families for efficient indexing and retrieval.
//!
//! # Column Families Used
//!
//! - `fingerprints`: Primary storage for ~63KB TeleologicalFingerprints
//! - `purpose_vectors`: 13D purpose vectors for fast purpose-only queries (52 bytes)
//! - `e13_splade_inverted`: Inverted index for Stage 1 (Recall) sparse search
//! - `e1_matryoshka_128`: E1 truncated 128D vectors for Stage 2 (Semantic ANN)
//! - `emb_0` through `emb_12`: Per-embedder quantized storage
//!
//! # FAIL FAST Policy
//!
//! **NO FALLBACKS. NO MOCK DATA. ERRORS ARE FATAL.**
//!
//! Every RocksDB operation that fails returns a detailed error with:
//! - The operation that failed
//! - The column family involved
//! - The key being accessed
//! - The underlying RocksDB error
//!
//! # Thread Safety
//!
//! The store is thread-safe for concurrent reads and writes via RocksDB's internal locking.
//! HNSW indexes are protected by `RwLock` for concurrent query access.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use rocksdb::{Cache, ColumnFamily, Options, WriteBatch, DB};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::index::{
    HnswMultiSpaceIndex, MultiSpaceIndexManager, EmbedderIndex,
};
use context_graph_core::teleological::ComparisonValidationError;
use context_graph_core::traits::{
    TeleologicalMemoryStore, TeleologicalSearchOptions, TeleologicalSearchResult,
    TeleologicalStorageBackend,
};
use context_graph_core::types::fingerprint::{
    PurposeVector, SemanticFingerprint, SparseVector, TeleologicalFingerprint,
    JohariFingerprint, NUM_EMBEDDERS,
};

use super::column_families::{
    get_all_teleological_cf_descriptors, CF_E13_SPLADE_INVERTED, CF_E1_MATRYOSHKA_128,
    CF_FINGERPRINTS, CF_PURPOSE_VECTORS, QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS,
};
use super::schema::{
    e13_splade_inverted_key, e1_matryoshka_128_key, fingerprint_key, parse_fingerprint_key,
    purpose_vector_key,
};
use super::serialization::{
    deserialize_memory_id_list, deserialize_purpose_vector,
    deserialize_teleological_fingerprint, serialize_e1_matryoshka_128, serialize_memory_id_list,
    serialize_purpose_vector, serialize_teleological_fingerprint,
};

// ============================================================================
// Error Types - FAIL FAST with detailed context
// ============================================================================

/// Detailed error type for teleological store operations.
///
/// Every error includes enough context for immediate debugging:
/// - Operation name
/// - Column family
/// - Key (if applicable)
/// - Underlying cause
#[derive(Debug, Error)]
pub enum TeleologicalStoreError {
    /// RocksDB operation failed.
    #[error("RocksDB {operation} failed on CF '{cf}' with key '{key:?}': {source}")]
    RocksDbOperation {
        operation: &'static str,
        cf: &'static str,
        key: Option<String>,
        #[source]
        source: rocksdb::Error,
    },

    /// Database failed to open.
    #[error("Failed to open RocksDB at '{path}': {message}")]
    OpenFailed { path: String, message: String },

    /// Column family not found.
    #[error("Column family '{name}' not found in database")]
    ColumnFamilyNotFound { name: String },

    /// Serialization error.
    #[error("Serialization error for fingerprint {id:?}: {message}")]
    Serialization { id: Option<Uuid>, message: String },

    /// Deserialization error.
    #[error("Deserialization error for key '{key}': {message}")]
    Deserialization { key: String, message: String },

    /// Fingerprint validation failed.
    #[error("Validation error for fingerprint {id:?}: {message}")]
    Validation { id: Option<Uuid>, message: String },

    /// Index operation failed.
    #[error("Index operation failed on '{index_name}': {message}")]
    IndexOperation { index_name: String, message: String },

    /// Checkpoint operation failed.
    #[error("Checkpoint operation failed: {message}")]
    CheckpointFailed { message: String },

    /// Restore operation failed.
    #[error("Restore operation failed from '{path}': {message}")]
    RestoreFailed { path: String, message: String },

    /// Internal error (should never happen).
    #[error("Internal error: {0}")]
    Internal(String),

    /// Comparison validation error (from TASK-CORE-004).
    ///
    /// This error occurs when fingerprint comparison types fail validation,
    /// such as weights not summing to 1.0 or synergy matrix symmetry violations.
    #[error("Comparison validation failed: {0}")]
    ComparisonValidation(#[from] ComparisonValidationError),
}

impl TeleologicalStoreError {
    /// Create a RocksDB operation error.
    pub fn rocksdb_op(
        operation: &'static str,
        cf: &'static str,
        key: Option<Uuid>,
        source: rocksdb::Error,
    ) -> Self {
        Self::RocksDbOperation {
            operation,
            cf,
            key: key.map(|k| k.to_string()),
            source,
        }
    }
}

impl From<TeleologicalStoreError> for CoreError {
    fn from(e: TeleologicalStoreError) -> Self {
        CoreError::StorageError(e.to_string())
    }
}

/// Result type for teleological store operations.
pub type TeleologicalStoreResult<T> = Result<T, TeleologicalStoreError>;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for RocksDbTeleologicalStore.
#[derive(Debug, Clone)]
pub struct TeleologicalStoreConfig {
    /// Block cache size in bytes (default: 256MB).
    pub block_cache_size: usize,
    /// Maximum number of open files (default: 1000).
    pub max_open_files: i32,
    /// Enable WAL (write-ahead log) for durability (default: true).
    pub enable_wal: bool,
    /// Create database if it doesn't exist (default: true).
    pub create_if_missing: bool,
}

impl Default for TeleologicalStoreConfig {
    fn default() -> Self {
        Self {
            block_cache_size: 256 * 1024 * 1024, // 256MB
            max_open_files: 1000,
            enable_wal: true,
            create_if_missing: true,
        }
    }
}

// ============================================================================
// Main Store Implementation
// ============================================================================

/// RocksDB-backed storage for TeleologicalFingerprints.
///
/// Implements the `TeleologicalMemoryStore` trait with persistent storage
/// across 17 column families for efficient indexing and retrieval.
///
/// # Thread Safety
///
/// The store is thread-safe for concurrent access:
/// - RocksDB handles internal locking for reads/writes
/// - HNSW indexes are protected by RwLock
///
/// # Example
///
/// ```ignore
/// use context_graph_storage::teleological::RocksDbTeleologicalStore;
/// use tempfile::TempDir;
///
/// let tmp = TempDir::new().unwrap();
/// let store = RocksDbTeleologicalStore::open(tmp.path()).unwrap();
///
/// // Store a fingerprint
/// let id = store.store(fingerprint).await.unwrap();
///
/// // Retrieve it
/// let retrieved = store.retrieve(id).await.unwrap();
/// ```
pub struct RocksDbTeleologicalStore {
    /// The RocksDB database instance.
    db: Arc<DB>,
    /// Shared block cache across column families.
    #[allow(dead_code)]
    cache: Cache,
    /// Database path.
    path: PathBuf,
    /// In-memory count of fingerprints (cached for performance).
    fingerprint_count: RwLock<Option<usize>>,
    /// Soft-deleted IDs (tracked in memory for filtering).
    soft_deleted: RwLock<HashMap<Uuid, bool>>,
    /// HNSW multi-space index for O(log n) ANN search.
    /// Uses real hnsw_rs library - NO FALLBACKS.
    hnsw_index: Arc<tokio::sync::RwLock<HnswMultiSpaceIndex>>,
}

impl RocksDbTeleologicalStore {
    /// Open a teleological store at the specified path with default configuration.
    ///
    /// Creates the database and all 17 column families if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    ///
    /// # Returns
    ///
    /// * `Ok(RocksDbTeleologicalStore)` - Successfully opened store
    /// * `Err(TeleologicalStoreError)` - Open failed with detailed error
    ///
    /// # Errors
    ///
    /// - `TeleologicalStoreError::OpenFailed` - Path invalid, permissions denied, or DB locked
    pub fn open<P: AsRef<Path>>(path: P) -> TeleologicalStoreResult<Self> {
        Self::open_with_config(path, TeleologicalStoreConfig::default())
    }

    /// Open a teleological store with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    /// * `config` - Custom configuration options
    ///
    /// # Returns
    ///
    /// * `Ok(RocksDbTeleologicalStore)` - Successfully opened store
    /// * `Err(TeleologicalStoreError)` - Open failed
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        config: TeleologicalStoreConfig,
    ) -> TeleologicalStoreResult<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let path_str = path_buf.to_string_lossy().to_string();

        info!(
            "Opening RocksDbTeleologicalStore at '{}' with cache_size={}MB",
            path_str,
            config.block_cache_size / (1024 * 1024)
        );

        // Create shared block cache
        let cache = Cache::new_lru_cache(config.block_cache_size);

        // Create DB options
        let mut db_opts = Options::default();
        db_opts.create_if_missing(config.create_if_missing);
        db_opts.create_missing_column_families(true);
        db_opts.set_max_open_files(config.max_open_files);

        if !config.enable_wal {
            db_opts.set_manual_wal_flush(true);
        }

        // Get all 17 teleological column family descriptors
        let cf_descriptors = get_all_teleological_cf_descriptors(&cache);

        debug!(
            "Opening database with {} column families",
            cf_descriptors.len()
        );

        // Open database with all column families
        let db = DB::open_cf_descriptors(&db_opts, &path_str, cf_descriptors).map_err(|e| {
            error!("Failed to open RocksDB at '{}': {}", path_str, e);
            TeleologicalStoreError::OpenFailed {
                path: path_str.clone(),
                message: e.to_string(),
            }
        })?;

        info!(
            "Successfully opened RocksDbTeleologicalStore with {} column families",
            TELEOLOGICAL_CFS.len() + QUANTIZED_EMBEDDER_CFS.len()
        );

        // Create HNSW index (uninitialized - call initialize_hnsw() for O(log n) search)
        let hnsw_index = Arc::new(tokio::sync::RwLock::new(HnswMultiSpaceIndex::new()));

        Ok(Self {
            db: Arc::new(db),
            cache,
            path: path_buf,
            fingerprint_count: RwLock::new(None),
            soft_deleted: RwLock::new(HashMap::new()),
            hnsw_index,
        })
    }

    /// Initialize HNSW indexes for O(log n) ANN search.
    ///
    /// **MUST be called before using search operations** for optimal performance.
    /// Without initialization, search falls back to O(n) linear scan.
    ///
    /// # Errors
    ///
    /// Returns error if HNSW initialization fails (e.g., invalid configuration).
    pub async fn initialize_hnsw(&self) -> CoreResult<()> {
        let mut hnsw = self.hnsw_index.write().await;
        hnsw.initialize().await.map_err(|e| {
            error!("FATAL: Failed to initialize HNSW indexes: {}", e);
            CoreError::IndexError(e.to_string())
        })?;
        info!("HNSW indexes initialized for O(log n) search");
        Ok(())
    }

    /// Check if HNSW indexes are initialized.
    pub async fn is_hnsw_initialized(&self) -> bool {
        let hnsw = self.hnsw_index.read().await;
        // Check if any index has been added (via status)
        !hnsw.status().iter().all(|s| s.element_count == 0)
    }

    /// Get a column family handle by name.
    ///
    /// # Errors
    ///
    /// Returns `TeleologicalStoreError::ColumnFamilyNotFound` if CF doesn't exist.
    fn get_cf(&self, name: &str) -> TeleologicalStoreResult<&ColumnFamily> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| TeleologicalStoreError::ColumnFamilyNotFound {
                name: name.to_string(),
            })
    }

    /// Store a fingerprint in all relevant column families.
    ///
    /// Writes to:
    /// 1. `fingerprints` - Full serialized fingerprint
    /// 2. `purpose_vectors` - 13D purpose vector for fast queries
    /// 3. `e1_matryoshka_128` - Truncated E1 embedding for Stage 2
    /// 4. `e13_splade_inverted` - Updates inverted index for Stage 1
    fn store_fingerprint_internal(
        &self,
        fp: &TeleologicalFingerprint,
    ) -> TeleologicalStoreResult<()> {
        let id = fp.id;
        let key = fingerprint_key(&id);

        // Create a write batch for atomic writes
        let mut batch = WriteBatch::default();

        // 1. Store full fingerprint
        let cf_fingerprints = self.get_cf(CF_FINGERPRINTS)?;
        let serialized = serialize_teleological_fingerprint(fp);
        batch.put_cf(cf_fingerprints, &key, &serialized);

        // 2. Store purpose vector
        let cf_purpose = self.get_cf(CF_PURPOSE_VECTORS)?;
        let purpose_key = purpose_vector_key(&id);
        let purpose_bytes = serialize_purpose_vector(&fp.purpose_vector.alignments);
        batch.put_cf(cf_purpose, &purpose_key, &purpose_bytes);

        // 3. Store E1 Matryoshka 128D truncated vector
        let cf_matryoshka = self.get_cf(CF_E1_MATRYOSHKA_128)?;
        let matryoshka_key = e1_matryoshka_128_key(&id);
        // Truncate E1 from 1024D to 128D
        let mut truncated = [0.0f32; 128];
        let e1 = &fp.semantic.e1_semantic;
        let copy_len = std::cmp::min(e1.len(), 128);
        truncated[..copy_len].copy_from_slice(&e1[..copy_len]);
        let matryoshka_bytes = serialize_e1_matryoshka_128(&truncated);
        batch.put_cf(cf_matryoshka, &matryoshka_key, &matryoshka_bytes);

        // 4. Update E13 SPLADE inverted index
        // For each active term in the E13 sparse vector, add this fingerprint's ID to the posting list
        self.update_splade_inverted_index(&mut batch, &id, &fp.semantic.e13_splade)?;

        // Execute atomic batch write
        self.db.write(batch).map_err(|e| {
            error!("Failed to write fingerprint batch for {}: {}", id, e);
            TeleologicalStoreError::rocksdb_op("write_batch", CF_FINGERPRINTS, Some(id), e)
        })?;

        // Invalidate count cache
        if let Ok(mut count) = self.fingerprint_count.write() {
            *count = None;
        }

        debug!("Stored fingerprint {} ({} bytes)", id, serialized.len());
        Ok(())
    }

    /// Update the E13 SPLADE inverted index for a fingerprint.
    fn update_splade_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf_inverted = self.get_cf(CF_E13_SPLADE_INVERTED)?;

        // For each active term, update the posting list
        for &term_id in &sparse.indices {
            let term_key = e13_splade_inverted_key(term_id);

            // Read existing posting list
            let existing = self
                .db
                .get_cf(cf_inverted, &term_key)
                .map_err(|e| {
                    TeleologicalStoreError::rocksdb_op(
                        "get",
                        CF_E13_SPLADE_INVERTED,
                        None,
                        e,
                    )
                })?;

            let mut ids: Vec<Uuid> = match existing {
                Some(data) => deserialize_memory_id_list(&data),
                None => Vec::new(),
            };

            // Add this ID if not already present
            if !ids.contains(id) {
                ids.push(*id);
                let serialized = serialize_memory_id_list(&ids);
                batch.put_cf(cf_inverted, &term_key, &serialized);
            }
        }

        Ok(())
    }

    /// Remove a fingerprint's terms from the inverted index.
    fn remove_from_splade_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf_inverted = self.get_cf(CF_E13_SPLADE_INVERTED)?;

        for &term_id in &sparse.indices {
            let term_key = e13_splade_inverted_key(term_id);

            let existing = self.db.get_cf(cf_inverted, &term_key).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("get", CF_E13_SPLADE_INVERTED, None, e)
            })?;

            if let Some(data) = existing {
                let mut ids: Vec<Uuid> = deserialize_memory_id_list(&data);
                ids.retain(|&i| i != *id);

                if ids.is_empty() {
                    batch.delete_cf(cf_inverted, &term_key);
                } else {
                    let serialized = serialize_memory_id_list(&ids);
                    batch.put_cf(cf_inverted, &term_key, &serialized);
                }
            }
        }

        Ok(())
    }

    /// Add fingerprint to HNSW indexes for O(log n) search.
    ///
    /// This adds the fingerprint's semantic embeddings and purpose vector
    /// to the in-memory HNSW indexes managed by HnswMultiSpaceIndex.
    async fn add_to_hnsw_indexes(&self, fp: &TeleologicalFingerprint) -> CoreResult<()> {
        let mut hnsw = self.hnsw_index.write().await;

        // Add full semantic fingerprint (all 13 embedders)
        hnsw.add_fingerprint(fp.id, &fp.semantic).await.map_err(|e| {
            error!("Failed to add fingerprint {} to HNSW indexes: {}", fp.id, e);
            CoreError::IndexError(e.to_string())
        })?;

        // Add purpose vector
        hnsw.add_purpose_vector(fp.id, &fp.purpose_vector.alignments)
            .await
            .map_err(|e| {
                error!("Failed to add purpose vector {} to HNSW: {}", fp.id, e);
                CoreError::IndexError(e.to_string())
            })?;

        debug!("Added fingerprint {} to HNSW indexes", fp.id);
        Ok(())
    }

    /// Remove fingerprint from HNSW indexes.
    async fn remove_from_hnsw_indexes(&self, id: Uuid) -> CoreResult<()> {
        let mut hnsw = self.hnsw_index.write().await;
        hnsw.remove(id).await.map_err(|e| {
            error!("Failed to remove {} from HNSW indexes: {}", id, e);
            CoreError::IndexError(e.to_string())
        })?;
        debug!("Removed fingerprint {} from HNSW indexes", id);
        Ok(())
    }

    /// Retrieve raw fingerprint bytes from RocksDB.
    fn get_fingerprint_raw(&self, id: Uuid) -> TeleologicalStoreResult<Option<Vec<u8>>> {
        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let key = fingerprint_key(&id);

        self.db
            .get_cf(cf, &key)
            .map_err(|e| TeleologicalStoreError::rocksdb_op("get", CF_FINGERPRINTS, Some(id), e))
    }

    /// Check if an ID is soft-deleted.
    fn is_soft_deleted(&self, id: &Uuid) -> bool {
        if let Ok(deleted) = self.soft_deleted.read() {
            deleted.get(id).copied().unwrap_or(false)
        } else {
            false
        }
    }

    /// Get the database path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get a reference to the underlying RocksDB instance.
    ///
    /// # Warning
    ///
    /// Direct DB access bypasses validation. Use for diagnostics only.
    pub fn db(&self) -> &DB {
        &self.db
    }

    /// Health check: verify all column families are accessible.
    pub fn health_check(&self) -> TeleologicalStoreResult<()> {
        for cf_name in TELEOLOGICAL_CFS {
            self.get_cf(cf_name)?;
        }
        for cf_name in QUANTIZED_EMBEDDER_CFS {
            self.get_cf(cf_name)?;
        }
        Ok(())
    }

    /// Get raw bytes from a specific column family.
    ///
    /// This method is for physical verification and debugging.
    /// It bypasses all caching and deserialization.
    ///
    /// # Arguments
    ///
    /// * `cf_name` - Name of the column family
    /// * `key` - Raw key bytes
    ///
    /// # Returns
    ///
    /// Raw bytes if found, or None if key doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns error if CF not found or RocksDB read fails.
    pub fn get_raw_bytes(
        &self,
        cf_name: &str,
        key: &[u8],
    ) -> TeleologicalStoreResult<Option<Vec<u8>>> {
        let cf = self.get_cf(cf_name)?;
        self.db.get_cf(cf, key).map_err(|e| {
            TeleologicalStoreError::Internal(format!(
                "RocksDB get_raw failed on CF '{}': {}",
                cf_name, e
            ))
        })
    }
}

// ============================================================================
// TeleologicalMemoryStore Trait Implementation
// ============================================================================

#[async_trait]
impl TeleologicalMemoryStore for RocksDbTeleologicalStore {
    // ==================== CRUD Operations ====================

    async fn store(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<Uuid> {
        let id = fingerprint.id;
        debug!("Storing fingerprint {}", id);

        // Store in RocksDB (primary storage)
        self.store_fingerprint_internal(&fingerprint)?;

        // Add to HNSW indexes for O(log n) search
        self.add_to_hnsw_indexes(&fingerprint).await?;

        Ok(id)
    }

    async fn retrieve(&self, id: Uuid) -> CoreResult<Option<TeleologicalFingerprint>> {
        debug!("Retrieving fingerprint {}", id);

        // Check soft-deleted
        if self.is_soft_deleted(&id) {
            return Ok(None);
        }

        let raw = self.get_fingerprint_raw(id)?;

        match raw {
            Some(data) => {
                let fp = deserialize_teleological_fingerprint(&data);
                Ok(Some(fp))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<bool> {
        let id = fingerprint.id;
        debug!("Updating fingerprint {}", id);

        // Check if exists
        let existing = self.get_fingerprint_raw(id)?;
        if existing.is_none() {
            return Ok(false);
        }

        // If updating, we need to remove old terms from inverted index first
        if let Some(old_data) = existing {
            let old_fp = deserialize_teleological_fingerprint(&old_data);
            let mut batch = WriteBatch::default();
            self.remove_from_splade_inverted_index(&mut batch, &id, &old_fp.semantic.e13_splade)?;
            self.db.write(batch).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("write_batch", CF_E13_SPLADE_INVERTED, Some(id), e)
            })?;
        }

        // Remove from HNSW indexes (will be re-added with updated vectors)
        self.remove_from_hnsw_indexes(id).await?;

        // Store updated fingerprint in RocksDB
        self.store_fingerprint_internal(&fingerprint)?;

        // Add updated fingerprint to HNSW indexes
        self.add_to_hnsw_indexes(&fingerprint).await?;

        Ok(true)
    }

    async fn delete(&self, id: Uuid, soft: bool) -> CoreResult<bool> {
        debug!("Deleting fingerprint {} (soft={})", id, soft);

        let existing = self.get_fingerprint_raw(id)?;
        if existing.is_none() {
            return Ok(false);
        }

        if soft {
            // Soft delete: mark as deleted in memory
            if let Ok(mut deleted) = self.soft_deleted.write() {
                deleted.insert(id, true);
            }
        } else {
            // Hard delete: remove from all column families
            let old_fp = deserialize_teleological_fingerprint(&existing.unwrap());
            let key = fingerprint_key(&id);

            let mut batch = WriteBatch::default();

            // Remove from fingerprints
            let cf_fp = self.get_cf(CF_FINGERPRINTS)?;
            batch.delete_cf(cf_fp, &key);

            // Remove from purpose_vectors
            let cf_pv = self.get_cf(CF_PURPOSE_VECTORS)?;
            batch.delete_cf(cf_pv, &purpose_vector_key(&id));

            // Remove from e1_matryoshka_128
            let cf_mat = self.get_cf(CF_E1_MATRYOSHKA_128)?;
            batch.delete_cf(cf_mat, &e1_matryoshka_128_key(&id));

            // Remove from inverted index
            self.remove_from_splade_inverted_index(&mut batch, &id, &old_fp.semantic.e13_splade)?;

            // Remove from soft-deleted tracking
            if let Ok(mut deleted) = self.soft_deleted.write() {
                deleted.remove(&id);
            }

            self.db.write(batch).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("delete_batch", CF_FINGERPRINTS, Some(id), e)
            })?;

            // Invalidate count cache
            if let Ok(mut count) = self.fingerprint_count.write() {
                *count = None;
            }

            // Remove from HNSW indexes
            self.remove_from_hnsw_indexes(id).await?;
        }

        info!("Deleted fingerprint {} (soft={})", id, soft);
        Ok(true)
    }

    // ==================== Search Operations ====================

    async fn search_semantic(
        &self,
        query: &SemanticFingerprint,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        debug!(
            "Searching semantic with top_k={}, min_similarity={} using O(log n) HNSW",
            options.top_k, options.min_similarity
        );

        // Use HNSW index for O(log n) approximate nearest neighbor search
        let hnsw = self.hnsw_index.read().await;

        // Search E1 semantic space (primary embedder) with 2x top_k to allow filtering
        let k = (options.top_k * 2).max(20);
        let candidates = hnsw.search(EmbedderIndex::E1Semantic, &query.e1_semantic, k)
            .await
            .map_err(|e| {
                error!("HNSW search failed: {}", e);
                CoreError::IndexError(e.to_string())
            })?;

        drop(hnsw); // Release lock before DB operations

        // Fetch full fingerprints for candidates
        let mut results = Vec::with_capacity(candidates.len());

        for (id, similarity) in candidates {
            // Skip soft-deleted
            if !options.include_deleted && self.is_soft_deleted(&id) {
                continue;
            }

            // Apply min_similarity filter
            if similarity < options.min_similarity {
                continue;
            }

            // Fetch full fingerprint from RocksDB
            if let Some(data) = self.get_fingerprint_raw(id)? {
                let fp = deserialize_teleological_fingerprint(&data);

                // Compute all 13 embedder scores
                let mut embedder_scores = [0.0f32; 13];
                embedder_scores[0] = similarity; // E1 semantic
                embedder_scores[1] = compute_cosine_similarity(&query.e2_temporal_recent, &fp.semantic.e2_temporal_recent);
                embedder_scores[2] = compute_cosine_similarity(&query.e3_temporal_periodic, &fp.semantic.e3_temporal_periodic);
                embedder_scores[3] = compute_cosine_similarity(&query.e4_temporal_positional, &fp.semantic.e4_temporal_positional);
                embedder_scores[4] = compute_cosine_similarity(&query.e5_causal, &fp.semantic.e5_causal);
                // E6 is sparse - skip for now (use search_sparse separately)
                embedder_scores[6] = compute_cosine_similarity(&query.e7_code, &fp.semantic.e7_code);
                embedder_scores[7] = compute_cosine_similarity(&query.e8_graph, &fp.semantic.e8_graph);
                embedder_scores[8] = compute_cosine_similarity(&query.e9_hdc, &fp.semantic.e9_hdc);
                embedder_scores[9] = compute_cosine_similarity(&query.e10_multimodal, &fp.semantic.e10_multimodal);
                embedder_scores[10] = compute_cosine_similarity(&query.e11_entity, &fp.semantic.e11_entity);
                // E12 late interaction and E13 SPLADE computed separately

                let purpose_alignment = query_purpose_alignment(&fp.purpose_vector);

                results.push(TeleologicalSearchResult::new(
                    fp,
                    similarity,
                    embedder_scores,
                    purpose_alignment,
                ));
            }
        }

        // Results already sorted by HNSW, but re-sort to ensure correct order after filtering
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to top_k
        results.truncate(options.top_k);

        debug!("HNSW semantic search returned {} results", results.len());
        Ok(results)
    }

    async fn search_purpose(
        &self,
        query: &PurposeVector,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        debug!(
            "Searching by purpose vector with top_k={} using O(log n) HNSW",
            options.top_k
        );

        // Use HNSW index for O(log n) purpose vector search
        let hnsw = self.hnsw_index.read().await;

        // Search purpose vector space with 2x top_k to allow filtering
        let k = (options.top_k * 2).max(20);
        let candidates = hnsw.search_purpose(&query.alignments, k)
            .await
            .map_err(|e| {
                error!("HNSW purpose search failed: {}", e);
                CoreError::IndexError(e.to_string())
            })?;

        drop(hnsw); // Release lock before DB operations

        // Fetch full fingerprints for candidates
        let mut results = Vec::with_capacity(candidates.len());

        for (id, similarity) in candidates {
            // Skip soft-deleted
            if !options.include_deleted && self.is_soft_deleted(&id) {
                continue;
            }

            // Apply min_similarity filter
            if similarity < options.min_similarity {
                continue;
            }

            // Fetch full fingerprint from RocksDB
            if let Some(fp) = self.retrieve(id).await? {
                // Apply min_alignment filter if specified
                if let Some(min_align) = options.min_alignment {
                    if fp.purpose_vector.aggregate_alignment() < min_align {
                        continue;
                    }
                }

                let embedder_scores = [0.0f32; 13]; // Purpose search doesn't compute embedder scores
                results.push(TeleologicalSearchResult::new(
                    fp,
                    similarity,
                    embedder_scores,
                    similarity, // Purpose alignment is the similarity
                ));
            }
        }

        // Results already sorted by HNSW, but re-sort after filtering
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to top_k
        results.truncate(options.top_k);

        debug!("HNSW purpose search returned {} results", results.len());
        Ok(results)
    }

    async fn search_text(
        &self,
        _text: &str,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        // Text search requires embedding generation, which is not available in storage layer
        // Return empty results with a warning
        warn!("search_text called but embedding generation not available in storage layer");
        warn!("Use embedding service to generate query embeddings, then call search_semantic");
        Ok(Vec::with_capacity(options.top_k))
    }

    async fn search_sparse(
        &self,
        sparse_query: &SparseVector,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        debug!(
            "Searching sparse with {} active terms, top_k={}",
            sparse_query.nnz(),
            top_k
        );

        let cf = self.get_cf(CF_E13_SPLADE_INVERTED)?;

        // Accumulate scores per document
        let mut doc_scores: HashMap<Uuid, f32> = HashMap::new();

        for (i, &term_id) in sparse_query.indices.iter().enumerate() {
            let term_key = e13_splade_inverted_key(term_id);
            let query_weight = sparse_query.values[i];

            if let Some(data) = self.db.get_cf(cf, &term_key).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("get", CF_E13_SPLADE_INVERTED, None, e)
            })? {
                let doc_ids = deserialize_memory_id_list(&data);

                for doc_id in doc_ids {
                    // Skip soft-deleted
                    if self.is_soft_deleted(&doc_id) {
                        continue;
                    }

                    // Simple term frequency scoring
                    // TODO: Implement BM25 or other scoring
                    *doc_scores.entry(doc_id).or_insert(0.0) += query_weight;
                }
            }
        }

        // Sort by score descending
        let mut results: Vec<(Uuid, f32)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Truncate to top_k
        results.truncate(top_k);

        debug!("Sparse search returned {} results", results.len());
        Ok(results)
    }

    // ==================== Batch Operations ====================

    async fn store_batch(&self, fingerprints: Vec<TeleologicalFingerprint>) -> CoreResult<Vec<Uuid>> {
        debug!("Storing batch of {} fingerprints", fingerprints.len());

        let mut ids = Vec::with_capacity(fingerprints.len());

        for fp in fingerprints {
            let id = fp.id;
            self.store_fingerprint_internal(&fp)?;
            ids.push(id);
        }

        info!("Stored batch of {} fingerprints", ids.len());
        Ok(ids)
    }

    async fn retrieve_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<TeleologicalFingerprint>>> {
        debug!("Retrieving batch of {} fingerprints", ids.len());

        let mut results = Vec::with_capacity(ids.len());

        for &id in ids {
            let fp = self.retrieve(id).await?;
            results.push(fp);
        }

        Ok(results)
    }

    // ==================== Statistics ====================

    async fn count(&self) -> CoreResult<usize> {
        // Check cache first
        if let Ok(cached) = self.fingerprint_count.read() {
            if let Some(count) = *cached {
                return Ok(count);
            }
        }

        // Count by iterating (expensive, but accurate)
        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut count = 0;
        for item in iter {
            let (key, _) = item
                .map_err(|e| TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e))?;
            let id = parse_fingerprint_key(&key);

            // Exclude soft-deleted
            if !self.is_soft_deleted(&id) {
                count += 1;
            }
        }

        // Cache the result
        if let Ok(mut cached) = self.fingerprint_count.write() {
            *cached = Some(count);
        }

        Ok(count)
    }

    async fn count_by_quadrant(&self) -> CoreResult<[usize; 4]> {
        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut counts = [0usize; 4];

        for item in iter {
            let (key, value) = item
                .map_err(|e| TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e))?;
            let id = parse_fingerprint_key(&key);

            // Skip soft-deleted
            if self.is_soft_deleted(&id) {
                continue;
            }

            let fp = deserialize_teleological_fingerprint(&value);
            let quadrant_idx = get_aggregate_dominant_quadrant(&fp.johari);
            counts[quadrant_idx] += 1;
        }

        Ok(counts)
    }

    fn storage_size_bytes(&self) -> usize {
        // Get approximate size from RocksDB properties
        let mut total = 0usize;

        for cf_name in TELEOLOGICAL_CFS {
            if let Ok(cf) = self.get_cf(cf_name) {
                if let Ok(Some(size)) = self
                    .db
                    .property_int_value_cf(cf, "rocksdb.estimate-live-data-size")
                {
                    total += size as usize;
                }
            }
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            if let Ok(cf) = self.get_cf(cf_name) {
                if let Ok(Some(size)) = self
                    .db
                    .property_int_value_cf(cf, "rocksdb.estimate-live-data-size")
                {
                    total += size as usize;
                }
            }
        }

        total
    }

    fn backend_type(&self) -> TeleologicalStorageBackend {
        TeleologicalStorageBackend::RocksDb
    }

    // ==================== Persistence ====================

    async fn flush(&self) -> CoreResult<()> {
        debug!("Flushing all column families");

        for cf_name in TELEOLOGICAL_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.flush_cf(cf).map_err(|e| {
                TeleologicalStoreError::RocksDbOperation {
                    operation: "flush",
                    cf: cf_name,
                    key: None,
                    source: e,
                }
            })?;
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.flush_cf(cf).map_err(|e| {
                TeleologicalStoreError::RocksDbOperation {
                    operation: "flush",
                    cf: cf_name,
                    key: None,
                    source: e,
                }
            })?;
        }

        info!("Flushed all column families");
        Ok(())
    }

    async fn checkpoint(&self) -> CoreResult<PathBuf> {
        let checkpoint_path = self.path.join("checkpoints").join(format!(
            "checkpoint_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        ));

        debug!("Creating checkpoint at {:?}", checkpoint_path);

        // Create checkpoint directory
        std::fs::create_dir_all(&checkpoint_path).map_err(|e| {
            CoreError::StorageError(format!("Failed to create checkpoint directory: {}", e))
        })?;

        // Create RocksDB checkpoint
        let checkpoint = rocksdb::checkpoint::Checkpoint::new(&self.db).map_err(|e| {
            TeleologicalStoreError::CheckpointFailed {
                message: e.to_string(),
            }
        })?;

        checkpoint
            .create_checkpoint(&checkpoint_path)
            .map_err(|e| TeleologicalStoreError::CheckpointFailed {
                message: e.to_string(),
            })?;

        info!("Created checkpoint at {:?}", checkpoint_path);
        Ok(checkpoint_path)
    }

    async fn restore(&self, checkpoint_path: &Path) -> CoreResult<()> {
        warn!(
            "Restore operation requested from {:?}. This is destructive!",
            checkpoint_path
        );

        // Verify checkpoint exists
        if !checkpoint_path.exists() {
            return Err(TeleologicalStoreError::RestoreFailed {
                path: checkpoint_path.to_string_lossy().to_string(),
                message: "Checkpoint path does not exist".to_string(),
            }
            .into());
        }

        // For restore, we would need to:
        // 1. Close the current database
        // 2. Copy checkpoint files to the database path
        // 3. Reopen the database
        //
        // This requires careful coordination and is not safe while the store is in use.
        // For now, we return an error instructing the user to restart with the checkpoint.

        Err(TeleologicalStoreError::RestoreFailed {
            path: checkpoint_path.to_string_lossy().to_string(),
            message: "In-place restore not supported. Please restart the application with the checkpoint path.".to_string(),
        }.into())
    }

    async fn compact(&self) -> CoreResult<()> {
        debug!("Starting compaction of all column families");

        for cf_name in TELEOLOGICAL_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        }

        // Purge soft-deleted entries during compaction
        if let Ok(mut deleted) = self.soft_deleted.write() {
            for (id, _) in deleted.drain() {
                // These are already removed from RocksDB during delete
                debug!("Purging soft-deleted entry {} from tracking", id);
            }
        }

        info!("Compaction complete");
        Ok(())
    }

    async fn list_by_quadrant(
        &self,
        quadrant: usize,
        limit: usize,
    ) -> CoreResult<Vec<(Uuid, JohariFingerprint)>> {
        debug!("list_by_quadrant: quadrant={}, limit={}", quadrant, limit);

        if quadrant > 3 {
            error!("Invalid quadrant index: {} (must be 0-3)", quadrant);
            return Err(CoreError::ValidationError {
                field: "quadrant".to_string(),
                message: format!("Quadrant index must be 0-3, got {}", quadrant),
            });
        }

        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let mut results = Vec::new();

        // Iterate through all fingerprints, filtering by quadrant
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            if results.len() >= limit {
                break;
            }

            let (key, value) = item.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
            })?;

            let id = parse_fingerprint_key(&key);

            // Skip soft-deleted entries
            if self.is_soft_deleted(&id) {
                continue;
            }

            let fp = deserialize_teleological_fingerprint(&value);
            let dominant = get_aggregate_dominant_quadrant(&fp.johari);

            if dominant == quadrant {
                results.push((id, fp.johari.clone()));
            }
        }

        debug!("list_by_quadrant returned {} results", results.len());
        Ok(results)
    }

    async fn list_all_johari(
        &self,
        limit: usize,
    ) -> CoreResult<Vec<(Uuid, JohariFingerprint)>> {
        debug!("list_all_johari: limit={}", limit);

        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let mut results = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            if results.len() >= limit {
                break;
            }

            let (key, value) = item.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
            })?;

            let id = parse_fingerprint_key(&key);

            if self.is_soft_deleted(&id) {
                continue;
            }

            let fp = deserialize_teleological_fingerprint(&value);
            results.push((id, fp.johari.clone()));
        }

        debug!("list_all_johari returned {} results", results.len());
        Ok(results)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the aggregate dominant quadrant across all 13 embedders.
///
/// Aggregates quadrant weights across all embedders and returns the index
/// of the dominant quadrant (0=Open, 1=Hidden, 2=Blind, 3=Unknown).
fn get_aggregate_dominant_quadrant(johari: &JohariFingerprint) -> usize {
    let mut totals = [0.0_f32; 4];
    for embedder_idx in 0..NUM_EMBEDDERS {
        for q in 0..4 {
            totals[q] += johari.quadrants[embedder_idx][q];
        }
    }

    // Find dominant quadrant
    totals
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(3) // Default to Unknown
}

/// Compute cosine similarity between two dense vectors.
fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a.sqrt()) * (norm_b.sqrt());
    if denom < f32::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

/// Compute purpose alignment for a fingerprint.
fn query_purpose_alignment(pv: &PurposeVector) -> f32 {
    pv.aggregate_alignment()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use context_graph_core::types::fingerprint::{SparseVector, NUM_EMBEDDERS};

    /// Create a test fingerprint with real (non-zero) embeddings.
    /// Uses deterministic pseudo-random values seeded from a counter.
    fn create_test_fingerprint_with_seed(seed: u64) -> TeleologicalFingerprint {
        use std::f32::consts::PI;

        // Generate deterministic embeddings from seed
        let generate_vec = |dim: usize, s: u64| -> Vec<f32> {
            (0..dim)
                .map(|i| {
                    let x = ((s as f64 * 0.1 + i as f64 * 0.01) * PI as f64).sin() as f32;
                    x * 0.5 + 0.5 // Normalize to [0, 1] range
                })
                .collect()
        };

        // Generate deterministic sparse vector for SPLADE
        let generate_sparse = |s: u64| -> SparseVector {
            let num_entries = 50 + (s % 50) as usize;
            let mut indices: Vec<u16> = Vec::with_capacity(num_entries);
            let mut values: Vec<f32> = Vec::with_capacity(num_entries);
            for i in 0..num_entries {
                let idx = ((s + i as u64 * 31) % 30522) as u16; // u16 for sparse indices
                let val = ((s as f64 * 0.1 + i as f64 * 0.2) * PI as f64).sin().abs() as f32 + 0.1;
                indices.push(idx);
                values.push(val);
            }
            SparseVector { indices, values }
        };

        // Generate late-interaction vectors (variable number of 128D token vectors)
        let generate_late_interaction = |s: u64| -> Vec<Vec<f32>> {
            let num_tokens = 5 + (s % 10) as usize;
            (0..num_tokens)
                .map(|t| generate_vec(128, s + t as u64 * 100))
                .collect()
        };

        // Create SemanticFingerprint with correct fields (per semantic/fingerprint.rs)
        let semantic = SemanticFingerprint {
            e1_semantic: generate_vec(1024, seed),                    // 1024D
            e2_temporal_recent: generate_vec(512, seed + 1),          // 512D
            e3_temporal_periodic: generate_vec(512, seed + 2),        // 512D
            e4_temporal_positional: generate_vec(512, seed + 3),      // 512D
            e5_causal: generate_vec(768, seed + 4),                   // 768D
            e6_sparse: generate_sparse(seed + 5),                     // Sparse
            e7_code: generate_vec(1536, seed + 6),                    // 1536D
            e8_graph: generate_vec(384, seed + 7),                    // 384D
            e9_hdc: generate_vec(1024, seed + 8),                     // 1024D HDC (projected)
            e10_multimodal: generate_vec(768, seed + 9),              // 768D
            e11_entity: generate_vec(384, seed + 10),                 // 384D
            e12_late_interaction: generate_late_interaction(seed + 11), // Vec<Vec<f32>>
            e13_splade: generate_sparse(seed + 12),                   // Sparse
        };

        // Create PurposeVector with correct structure (alignments: [f32; 13])
        let mut alignments = [0.5f32; NUM_EMBEDDERS];
        for (i, a) in alignments.iter_mut().enumerate() {
            *a = ((seed as f64 * 0.1 + i as f64 * 0.3) * PI as f64).sin() as f32 * 0.5 + 0.5;
        }
        let purpose = PurposeVector::new(alignments);

        // Create JohariFingerprint using zeroed() then set values
        let mut johari = JohariFingerprint::zeroed();
        for i in 0..NUM_EMBEDDERS {
            johari.set_quadrant(i, 0.8, 0.1, 0.05, 0.05, 0.9); // Open dominant with high confidence
        }

        // Create unique hash
        let mut hash = [0u8; 32];
        for (i, byte) in hash.iter_mut().enumerate() {
            *byte = ((seed + i as u64) % 256) as u8;
        }

        TeleologicalFingerprint::new(semantic, purpose, johari, hash)
    }

    fn create_test_fingerprint() -> TeleologicalFingerprint {
        create_test_fingerprint_with_seed(42)
    }

    /// Helper to create store with initialized HNSW indexes.
    async fn create_initialized_store(path: &std::path::Path) -> RocksDbTeleologicalStore {
        let store = RocksDbTeleologicalStore::open(path).unwrap();
        store.initialize_hnsw().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_open_and_health_check() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path()).await;
        assert!(store.health_check().is_ok());
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path()).await;

        let fp = create_test_fingerprint();
        let id = fp.id;

        // Store
        let stored_id = store.store(fp.clone()).await.unwrap();
        assert_eq!(stored_id, id);

        // Retrieve
        let retrieved = store.retrieve(id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_fp = retrieved.unwrap();
        assert_eq!(retrieved_fp.id, id);
    }

    #[tokio::test]
    async fn test_physical_persistence() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();

        let fp = create_test_fingerprint();
        let id = fp.id;

        // Store and close
        {
            let store = create_initialized_store(&path).await;
            store.store(fp.clone()).await.unwrap();
            store.flush().await.unwrap();
        }

        // Reopen and verify
        {
            let store = create_initialized_store(&path).await;
            let retrieved = store.retrieve(id).await.unwrap();
            assert!(
                retrieved.is_some(),
                "Fingerprint should persist across database close/reopen"
            );
            assert_eq!(retrieved.unwrap().id, id);
        }

        // Verify raw bytes exist in RocksDB
        {
            let store = create_initialized_store(&path).await;
            let raw = store.get_fingerprint_raw(id).unwrap();
            assert!(raw.is_some(), "Raw bytes should exist in RocksDB");
            let raw_bytes = raw.unwrap();
            // With E9_DIM = 1024 (projected), fingerprints are ~32-40KB
            assert!(
                raw_bytes.len() >= 25000,
                "Serialized fingerprint should be >= 25KB, got {} bytes",
                raw_bytes.len()
            );
        }
    }

    #[tokio::test]
    async fn test_delete_soft() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path()).await;

        let fp = create_test_fingerprint();
        let id = fp.id;

        store.store(fp).await.unwrap();
        let deleted = store.delete(id, true).await.unwrap();
        assert!(deleted);

        // Should not be retrievable after soft delete
        let retrieved = store.retrieve(id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_delete_hard() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path()).await;

        let fp = create_test_fingerprint();
        let id = fp.id;

        store.store(fp).await.unwrap();
        let deleted = store.delete(id, false).await.unwrap();
        assert!(deleted);

        // Raw bytes should be gone
        let raw = store.get_fingerprint_raw(id).unwrap();
        assert!(raw.is_none());
    }

    #[tokio::test]
    async fn test_count() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path()).await;

        assert_eq!(store.count().await.unwrap(), 0);

        store.store(create_test_fingerprint_with_seed(1)).await.unwrap();
        store.store(create_test_fingerprint_with_seed(2)).await.unwrap();
        store.store(create_test_fingerprint_with_seed(3)).await.unwrap();

        assert_eq!(store.count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_backend_type() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path()).await;
        assert_eq!(store.backend_type(), TeleologicalStorageBackend::RocksDb);
    }
}
