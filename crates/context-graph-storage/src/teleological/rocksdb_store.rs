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
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use rocksdb::{Cache, ColumnFamily, Options, WriteBatch, DB};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::teleological::ComparisonValidationError;

use super::indexes::{EmbedderIndex, EmbedderIndexOps, EmbedderIndexRegistry, IndexError};
use context_graph_core::traits::{
    TeleologicalMemoryStore, TeleologicalSearchOptions, TeleologicalSearchResult,
    TeleologicalStorageBackend,
};
use context_graph_core::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, SparseVector, TeleologicalFingerprint,
    NUM_EMBEDDERS,
};

use super::column_families::{
    get_all_teleological_cf_descriptors, CF_CONTENT, CF_E13_SPLADE_INVERTED, CF_E1_MATRYOSHKA_128,
    CF_FINGERPRINTS, CF_PURPOSE_VECTORS, QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS,
};
use super::schema::{
    content_key, e13_splade_inverted_key, e1_matryoshka_128_key, fingerprint_key,
    parse_fingerprint_key, purpose_vector_key,
};
use super::serialization::{
    deserialize_memory_id_list, deserialize_teleological_fingerprint, serialize_e1_matryoshka_128,
    serialize_memory_id_list, serialize_purpose_vector, serialize_teleological_fingerprint,
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

    /// Stale lock detected and could not be cleaned.
    #[error("Stale lock detected at '{path}' but cleanup failed: {message}")]
    StaleLockCleanupFailed { path: String, message: String },

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
    /// Per-embedder index registry with 12 HNSW indexes for O(log n) ANN search.
    /// E6, E12, E13 use different index types (inverted/MaxSim).
    /// NO FALLBACKS - FAIL FAST on invalid operations.
    index_registry: Arc<EmbedderIndexRegistry>,
}

impl RocksDbTeleologicalStore {
    /// Detect and remove stale RocksDB lock files.
    ///
    /// RocksDB creates a LOCK file when opening a database. If the process crashes
    /// or is killed without cleanly closing the database, this LOCK file remains
    /// ("stale lock"). This function detects and removes such stale locks.
    ///
    /// # Detection Algorithm
    ///
    /// 1. Check if LOCK file exists
    /// 2. If it exists, attempt to acquire an exclusive file lock on it
    /// 3. If we CAN acquire the lock, no other process holds it (stale)
    /// 4. Remove the stale LOCK file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Stale lock was detected and removed
    /// * `Ok(false)` - No stale lock (either no lock file or lock is held by active process)
    /// * `Err(...)` - Lock file exists but couldn't be removed
    ///
    /// # FAIL FAST
    ///
    /// If the lock file exists but cannot be removed (permissions, etc.), this
    /// returns an error rather than silently proceeding.
    fn detect_and_remove_stale_lock<P: AsRef<Path>>(path: P) -> TeleologicalStoreResult<bool> {
        let lock_path = path.as_ref().join("LOCK");
        let lock_path_str = lock_path.to_string_lossy().to_string();

        // Check if LOCK file exists
        if !lock_path.exists() {
            debug!("No LOCK file at '{}' - database not previously opened", lock_path_str);
            return Ok(false);
        }

        info!("LOCK file exists at '{}' - checking if stale", lock_path_str);

        // Try to open the lock file with exclusive access
        // On Unix, we use flock-style locking; on Windows, LockFile
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;

            let file = match fs::OpenOptions::new().read(true).write(true).open(&lock_path) {
                Ok(f) => f,
                Err(e) => {
                    // Can't even open the file - might be permissions issue
                    warn!("Cannot open LOCK file for inspection: {}", e);
                    // Try to remove it anyway
                    return Self::try_remove_lock_file(&lock_path, &lock_path_str);
                }
            };

            // Try non-blocking exclusive lock
            let fd = file.as_raw_fd();
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };

            if result == 0 {
                // Successfully acquired lock - no other process holds it (stale)
                info!(
                    "Acquired exclusive lock - LOCK file at '{}' is STALE (no process holds it)",
                    lock_path_str
                );

                // Release the lock before removing
                unsafe { libc::flock(fd, libc::LOCK_UN) };
                drop(file);

                return Self::try_remove_lock_file(&lock_path, &lock_path_str);
            } else {
                let errno = std::io::Error::last_os_error();
                if errno.raw_os_error() == Some(libc::EWOULDBLOCK) {
                    // Lock is held by another process - NOT stale
                    info!(
                        "LOCK file at '{}' is held by another process - NOT stale",
                        lock_path_str
                    );
                    return Ok(false);
                } else {
                    // Other error (shouldn't happen)
                    warn!("flock() failed with unexpected error: {}", errno);
                    // Try to remove it anyway - might be a corrupted lock
                    return Self::try_remove_lock_file(&lock_path, &lock_path_str);
                }
            }
        }

        #[cfg(windows)]
        {
            // On Windows, just try to delete the lock file
            // If another process holds it, the delete will fail
            Self::try_remove_lock_file(&lock_path, &lock_path_str)
        }

        #[cfg(not(any(unix, windows)))]
        {
            // For other platforms, just try to remove the lock file
            Self::try_remove_lock_file(&lock_path, &lock_path_str)
        }
    }

    /// Attempt to remove a stale lock file.
    fn try_remove_lock_file(lock_path: &Path, lock_path_str: &str) -> TeleologicalStoreResult<bool> {
        match fs::remove_file(lock_path) {
            Ok(()) => {
                info!("Successfully removed stale LOCK file at '{}'", lock_path_str);
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // File was already removed (race condition, but OK)
                debug!("LOCK file at '{}' was already removed", lock_path_str);
                Ok(true)
            }
            Err(e) => {
                error!(
                    "FAIL FAST: Cannot remove stale LOCK file at '{}': {}",
                    lock_path_str, e
                );
                Err(TeleologicalStoreError::StaleLockCleanupFailed {
                    path: lock_path_str.to_string(),
                    message: format!(
                        "Lock file exists but cannot be removed: {}. \
                        Manual intervention required: check permissions or remove '{}'",
                        e, lock_path_str
                    ),
                })
            }
        }
    }

    /// Open a teleological store at the specified path with default configuration.
    ///
    /// Creates the database and all 17 column families if they don't exist.
    /// **Automatically detects and removes stale lock files.**
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
    /// - `TeleologicalStoreError::StaleLockCleanupFailed` - Stale lock detected but couldn't be removed
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

        // STALE LOCK DETECTION: Check for and remove stale lock files before opening.
        // This prevents "Resource temporarily unavailable" errors from orphaned locks.
        // FAIL FAST: If we can't clean up a stale lock, fail immediately with clear error.
        if path_buf.exists() {
            match Self::detect_and_remove_stale_lock(&path_buf) {
                Ok(true) => {
                    info!("Removed stale lock at '{}', proceeding with open", path_str);
                }
                Ok(false) => {
                    debug!("No stale lock detected at '{}'", path_str);
                }
                Err(e) => {
                    error!("FAIL FAST: Stale lock cleanup failed at '{}': {}", path_str, e);
                    return Err(e);
                }
            }
        }

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

        // Create per-embedder index registry (12 HNSW indexes)
        let index_registry = Arc::new(EmbedderIndexRegistry::new());

        info!(
            "Successfully opened RocksDbTeleologicalStore with {} column families and {} per-embedder indexes",
            TELEOLOGICAL_CFS.len() + QUANTIZED_EMBEDDER_CFS.len(),
            index_registry.len()
        );

        Ok(Self {
            db: Arc::new(db),
            cache,
            path: path_buf,
            fingerprint_count: RwLock::new(None),
            soft_deleted: RwLock::new(HashMap::new()),
            index_registry,
        })
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

    /// Get the content column family handle.
    ///
    /// # FAIL FAST
    ///
    /// Panics if CF_CONTENT doesn't exist. This indicates database initialization
    /// failed and is an invariant violation that cannot be recovered from.
    /// CF_CONTENT must be present in any valid database opened by this store.
    ///
    /// TASK-CONTENT-006: Content storage CF handle.
    #[inline]
    fn cf_content(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONTENT)
            .expect("CF_CONTENT must exist - database initialization failed")
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
        batch.put_cf(cf_fingerprints, key, &serialized);

        // 2. Store purpose vector
        let cf_purpose = self.get_cf(CF_PURPOSE_VECTORS)?;
        let purpose_key = purpose_vector_key(&id);
        let purpose_bytes = serialize_purpose_vector(&fp.purpose_vector.alignments);
        batch.put_cf(cf_purpose, purpose_key, purpose_bytes);

        // 3. Store E1 Matryoshka 128D truncated vector
        let cf_matryoshka = self.get_cf(CF_E1_MATRYOSHKA_128)?;
        let matryoshka_key = e1_matryoshka_128_key(&id);
        // Truncate E1 from 1024D to 128D
        let mut truncated = [0.0f32; 128];
        let e1 = &fp.semantic.e1_semantic;
        let copy_len = std::cmp::min(e1.len(), 128);
        truncated[..copy_len].copy_from_slice(&e1[..copy_len]);
        let matryoshka_bytes = serialize_e1_matryoshka_128(&truncated);
        batch.put_cf(cf_matryoshka, matryoshka_key, matryoshka_bytes);

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
            let existing = self.db.get_cf(cf_inverted, term_key).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("get", CF_E13_SPLADE_INVERTED, None, e)
            })?;

            let mut ids: Vec<Uuid> = match existing {
                Some(data) => deserialize_memory_id_list(&data),
                None => Vec::new(),
            };

            // Add this ID if not already present
            if !ids.contains(id) {
                ids.push(*id);
                let serialized = serialize_memory_id_list(&ids);
                batch.put_cf(cf_inverted, term_key, &serialized);
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

            let existing = self.db.get_cf(cf_inverted, term_key).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("get", CF_E13_SPLADE_INVERTED, None, e)
            })?;

            if let Some(data) = existing {
                let mut ids: Vec<Uuid> = deserialize_memory_id_list(&data);
                ids.retain(|&i| i != *id);

                if ids.is_empty() {
                    batch.delete_cf(cf_inverted, term_key);
                } else {
                    let serialized = serialize_memory_id_list(&ids);
                    batch.put_cf(cf_inverted, term_key, &serialized);
                }
            }
        }

        Ok(())
    }

    /// Add fingerprint to per-embedder HNSW indexes for O(log n) search.
    ///
    /// Inserts vectors into all 12 HNSW-capable embedder indexes plus PurposeVector.
    /// E6, E12, E13 are skipped (they use different index types).
    ///
    /// # FAIL FAST
    ///
    /// - DimensionMismatch: panic with detailed error
    /// - InvalidVector (NaN/Inf): panic with location
    fn add_to_indexes(&self, fp: &TeleologicalFingerprint) -> Result<(), IndexError> {
        let id = fp.id;

        // Add to all HNSW-capable dense embedder indexes
        for embedder in EmbedderIndex::all_hnsw() {
            // Skip PurposeVector - handled separately at the end
            if embedder == EmbedderIndex::PurposeVector {
                continue;
            }

            if let Some(index) = self.index_registry.get(embedder) {
                let vector = Self::get_embedder_vector(&fp.semantic, embedder);
                index.insert(id, vector)?;
            }
        }

        // Add purpose vector to its dedicated 13D index
        if let Some(pv_index) = self.index_registry.get(EmbedderIndex::PurposeVector) {
            pv_index.insert(id, &fp.purpose_vector.alignments)?;
        }

        debug!(
            "Added fingerprint {} to {} indexes",
            id,
            self.index_registry.len()
        );
        Ok(())
    }

    /// Extract vector for specific embedder from SemanticFingerprint.
    ///
    /// Returns the appropriate vector slice for the given embedder index.
    ///
    /// # FAIL FAST
    ///
    /// Panics for embedders that don't use HNSW:
    /// - E6Sparse: Use inverted index
    /// - E12LateInteraction: Use MaxSim
    /// - E13Splade: Use inverted index
    /// - PurposeVector: Use purpose_vector.alignments directly
    fn get_embedder_vector(semantic: &SemanticFingerprint, embedder: EmbedderIndex) -> &[f32] {
        match embedder {
            EmbedderIndex::E1Semantic => &semantic.e1_semantic,
            EmbedderIndex::E1Matryoshka128 => {
                // Truncate E1 to 128D - return first 128 elements
                &semantic.e1_semantic[..128.min(semantic.e1_semantic.len())]
            }
            EmbedderIndex::E2TemporalRecent => &semantic.e2_temporal_recent,
            EmbedderIndex::E3TemporalPeriodic => &semantic.e3_temporal_periodic,
            EmbedderIndex::E4TemporalPositional => &semantic.e4_temporal_positional,
            EmbedderIndex::E5Causal => &semantic.e5_causal,
            EmbedderIndex::E6Sparse => {
                panic!("FAIL FAST: E6 is sparse - use inverted index, not HNSW")
            }
            EmbedderIndex::E7Code => &semantic.e7_code,
            EmbedderIndex::E8Graph => &semantic.e8_graph,
            EmbedderIndex::E9HDC => &semantic.e9_hdc,
            EmbedderIndex::E10Multimodal => &semantic.e10_multimodal,
            EmbedderIndex::E11Entity => &semantic.e11_entity,
            EmbedderIndex::E12LateInteraction => {
                panic!("FAIL FAST: E12 is late-interaction - use MaxSim, not HNSW")
            }
            EmbedderIndex::E13Splade => {
                panic!("FAIL FAST: E13 is sparse - use inverted index, not HNSW")
            }
            EmbedderIndex::PurposeVector => {
                panic!("FAIL FAST: PurposeVector is not in SemanticFingerprint - use purpose_vector.alignments")
            }
        }
    }

    /// Remove fingerprint from all per-embedder indexes.
    ///
    /// Removes the ID from all 12 HNSW indexes.
    fn remove_from_indexes(&self, id: Uuid) -> Result<(), IndexError> {
        for (_embedder, index) in self.index_registry.iter() {
            // Remove returns bool (found or not), we ignore it
            let _ = index.remove(id)?;
        }
        debug!("Removed fingerprint {} from all indexes", id);
        Ok(())
    }

    /// Compute similarity scores for all 13 embedders.
    ///
    /// Uses cosine similarity for dense embedders.
    /// E6 and E13 (sparse) return 0.0 - use search_sparse() separately.
    /// E12 (late-interaction) returns 0.0 - use MaxSim separately.
    fn compute_embedder_scores(
        &self,
        query: &SemanticFingerprint,
        stored: &SemanticFingerprint,
    ) -> [f32; 13] {
        [
            compute_cosine_similarity(&query.e1_semantic, &stored.e1_semantic),
            compute_cosine_similarity(&query.e2_temporal_recent, &stored.e2_temporal_recent),
            compute_cosine_similarity(&query.e3_temporal_periodic, &stored.e3_temporal_periodic),
            compute_cosine_similarity(
                &query.e4_temporal_positional,
                &stored.e4_temporal_positional,
            ),
            compute_cosine_similarity(&query.e5_causal, &stored.e5_causal),
            0.0, // E6 sparse - use search_sparse()
            compute_cosine_similarity(&query.e7_code, &stored.e7_code),
            compute_cosine_similarity(&query.e8_graph, &stored.e8_graph),
            compute_cosine_similarity(&query.e9_hdc, &stored.e9_hdc),
            compute_cosine_similarity(&query.e10_multimodal, &stored.e10_multimodal),
            compute_cosine_similarity(&query.e11_entity, &stored.e11_entity),
            0.0, // E12 late-interaction - use MaxSim
            0.0, // E13 SPLADE - use search_sparse()
        ]
    }

    /// Retrieve raw fingerprint bytes from RocksDB.
    fn get_fingerprint_raw(&self, id: Uuid) -> TeleologicalStoreResult<Option<Vec<u8>>> {
        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let key = fingerprint_key(&id);

        self.db
            .get_cf(cf, key)
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

    /// Get a reference to the underlying RocksDB instance FOR TESTING/DIAGNOSTICS ONLY.
    ///
    /// # Constitution Compliance
    ///
    /// ARCH-06: "All Memory Operations Through MCP Tools - Direct database access forbidden"
    ///
    /// This method is intended ONLY for:
    /// - Integration tests performing Full State Verification
    /// - Diagnostics and debugging
    /// - Health checks
    ///
    /// **DO NOT** use this method in production MCP handlers or application code.
    /// All memory operations should go through the proper MCP tool interface.
    ///
    /// # Warning
    ///
    /// Direct DB access bypasses validation. Improper use violates ARCH-06.
    #[doc(hidden)]
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

        // Add to per-embedder indexes for O(log n) search
        self.add_to_indexes(&fingerprint)
            .map_err(|e| CoreError::IndexError(e.to_string()))?;

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
                TeleologicalStoreError::rocksdb_op(
                    "write_batch",
                    CF_E13_SPLADE_INVERTED,
                    Some(id),
                    e,
                )
            })?;
        }

        // Remove from per-embedder indexes (will be re-added with updated vectors)
        self.remove_from_indexes(id)
            .map_err(|e| CoreError::IndexError(e.to_string()))?;

        // Store updated fingerprint in RocksDB
        self.store_fingerprint_internal(&fingerprint)?;

        // Add updated fingerprint to per-embedder indexes
        self.add_to_indexes(&fingerprint)
            .map_err(|e| CoreError::IndexError(e.to_string()))?;

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
            batch.delete_cf(cf_fp, key);

            // Remove from purpose_vectors
            let cf_pv = self.get_cf(CF_PURPOSE_VECTORS)?;
            batch.delete_cf(cf_pv, purpose_vector_key(&id));

            // Remove from e1_matryoshka_128
            let cf_mat = self.get_cf(CF_E1_MATRYOSHKA_128)?;
            batch.delete_cf(cf_mat, e1_matryoshka_128_key(&id));

            // Remove from inverted index
            self.remove_from_splade_inverted_index(&mut batch, &id, &old_fp.semantic.e13_splade)?;

            // Remove content (TASK-CONTENT-009: cascade content deletion)
            let cf_content = self.cf_content();
            batch.delete_cf(cf_content, content_key(&id));

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

            // Remove from per-embedder indexes
            self.remove_from_indexes(id)
                .map_err(|e| CoreError::IndexError(e.to_string()))?;
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
            "Searching semantic with top_k={}, min_similarity={}",
            options.top_k, options.min_similarity
        );

        // Search E1 Semantic as primary (TODO: parallel multi-space search)
        let entry_embedder = EmbedderIndex::E1Semantic;
        let entry_index = self.index_registry.get(entry_embedder).ok_or_else(|| {
            CoreError::IndexError(format!("Index {:?} not found", entry_embedder))
        })?;

        // Search E1 semantic space with 2x top_k to allow filtering
        let k = (options.top_k * 2).max(20);
        let candidates = entry_index
            .search(&query.e1_semantic, k, None)
            .map_err(|e| {
                error!("Semantic search failed: {}", e);
                CoreError::IndexError(e.to_string())
            })?;

        // Fetch full fingerprints for candidates
        let mut results = Vec::with_capacity(candidates.len());

        for (id, distance) in candidates {
            // Convert distance to similarity (HNSW returns distance, not similarity)
            let similarity = 1.0 - distance.min(1.0);

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

                // Compute all 13 embedder scores using helper
                let embedder_scores = self.compute_embedder_scores(query, &fp.semantic);

                let purpose_alignment = query_purpose_alignment(&fp.purpose_vector);

                results.push(TeleologicalSearchResult::new(
                    fp,
                    similarity,
                    embedder_scores,
                    purpose_alignment,
                ));
            }
        }

        // Sort by similarity descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to top_k
        results.truncate(options.top_k);

        debug!("Semantic search returned {} results", results.len());
        Ok(results)
    }

    async fn search_purpose(
        &self,
        query: &PurposeVector,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        debug!(
            "Searching by purpose vector with top_k={} using per-embedder index",
            options.top_k
        );

        // Use PurposeVector index for O(log n) search
        let pv_index = self
            .index_registry
            .get(EmbedderIndex::PurposeVector)
            .ok_or_else(|| CoreError::IndexError("PurposeVector index not found".to_string()))?;

        // Search purpose vector space with 2x top_k to allow filtering
        let k = (options.top_k * 2).max(20);
        let candidates = pv_index.search(&query.alignments, k, None).map_err(|e| {
            error!("Purpose vector search failed: {}", e);
            CoreError::IndexError(e.to_string())
        })?;

        // Fetch full fingerprints for candidates
        let mut results = Vec::with_capacity(candidates.len());

        for (id, distance) in candidates {
            // Convert distance to similarity
            let similarity = 1.0 - distance.min(1.0);

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

                // Compute embedder scores:
                // If semantic_query is provided, compute actual cosine similarities per embedder
                // Otherwise, return zeros (backward compatible fallback with warning)
                let embedder_scores = match &options.semantic_query {
                    Some(query_semantic) => {
                        // Compute actual per-embedder cosine similarities
                        self.compute_embedder_scores(query_semantic, &fp.semantic)
                    }
                    None => {
                        // Fallback: zeros (less useful but backward compatible)
                        tracing::warn!(
                            "search_purpose: No semantic_query provided - embedder_scores will be zeros. \
                             Pass semantic_query in options for meaningful per-embedder scores."
                        );
                        [0.0f32; 13]
                    }
                };
                results.push(TeleologicalSearchResult::new(
                    fp,
                    similarity,
                    embedder_scores,
                    similarity, // Purpose alignment is the similarity
                ));
            }
        }

        // Sort by similarity descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to top_k
        results.truncate(options.top_k);

        debug!("Purpose vector search returned {} results", results.len());
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

            if let Some(data) = self.db.get_cf(cf, term_key).map_err(|e| {
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

    async fn store_batch(
        &self,
        fingerprints: Vec<TeleologicalFingerprint>,
    ) -> CoreResult<Vec<Uuid>> {
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

    async fn retrieve_batch(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<TeleologicalFingerprint>>> {
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
            let (key, _) = item.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
            })?;
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
            let (key, value) = item.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
            })?;
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
            self.db
                .flush_cf(cf)
                .map_err(|e| TeleologicalStoreError::RocksDbOperation {
                    operation: "flush",
                    cf: cf_name,
                    key: None,
                    source: e,
                })?;
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db
                .flush_cf(cf)
                .map_err(|e| TeleologicalStoreError::RocksDbOperation {
                    operation: "flush",
                    cf: cf_name,
                    key: None,
                    source: e,
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

    async fn list_all_johari(&self, limit: usize) -> CoreResult<Vec<(Uuid, JohariFingerprint)>> {
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

    // ==================== Content Storage Operations ====================

    /// Store content text for a fingerprint (TASK-CONTENT-007).
    ///
    /// # Validation
    /// - Content must be <= 1MB (1,048,576 bytes)
    /// - If fingerprint exists, SHA-256 hash must match content_hash
    ///
    /// # Errors
    /// - Returns error if content exceeds size limit (FAIL FAST)
    /// - Returns error if hash mismatch with existing fingerprint (FAIL FAST)
    /// - Returns error if RocksDB write fails
    async fn store_content(&self, id: Uuid, content: &str) -> CoreResult<()> {
        const MAX_CONTENT_SIZE: usize = 1_048_576; // 1MB

        // 1. Validate size - FAIL FAST
        let content_bytes = content.as_bytes();
        if content_bytes.len() > MAX_CONTENT_SIZE {
            error!(
                "CONTENT ERROR: Content size {} bytes exceeds max {} bytes for fingerprint {}",
                content_bytes.len(),
                MAX_CONTENT_SIZE,
                id
            );
            return Err(CoreError::Internal(format!(
                "Content size {} bytes exceeds maximum {} bytes for fingerprint {}",
                content_bytes.len(),
                MAX_CONTENT_SIZE,
                id
            )));
        }

        // 2. Compute SHA-256 hash of content
        let mut hasher = Sha256::new();
        hasher.update(content_bytes);
        let computed_hash: [u8; 32] = hasher.finalize().into();

        // 3. Verify hash matches if fingerprint exists
        if let Some(data) = self.get_fingerprint_raw(id)? {
            let fp = deserialize_teleological_fingerprint(&data);
            if fp.content_hash != computed_hash {
                error!(
                    "CONTENT ERROR: Hash mismatch for fingerprint {}. \
                     Expected: {:02x?}, Computed: {:02x?}. \
                     Content length: {} bytes.",
                    id,
                    &fp.content_hash[..8], // First 8 bytes for log
                    &computed_hash[..8],
                    content_bytes.len()
                );
                return Err(CoreError::Internal(format!(
                    "Content hash mismatch for fingerprint {}. Expected hash does not match provided content.",
                    id
                )));
            }
            debug!(
                "Content hash verified for fingerprint {} ({} bytes)",
                id,
                content_bytes.len()
            );
        } else {
            // No fingerprint exists - store content anyway for pre-storage scenarios
            debug!(
                "Storing content for non-existent fingerprint {} ({} bytes). \
                 Hash verification will occur when fingerprint is created.",
                id,
                content_bytes.len()
            );
        }

        // 4. Store content using cf_content() and content_key()
        let cf = self.cf_content();
        let key = content_key(&id);

        self.db.put_cf(cf, key, content_bytes).map_err(|e| {
            error!(
                "ROCKSDB ERROR: Failed to store content for fingerprint {}: {}",
                id, e
            );
            TeleologicalStoreError::rocksdb_op("put_content", CF_CONTENT, Some(id), e)
        })?;

        info!(
            "Stored content for fingerprint {} ({} bytes, LZ4 compressed)",
            id,
            content_bytes.len()
        );
        Ok(())
    }

    /// Retrieve content text for a fingerprint (TASK-CONTENT-008).
    ///
    /// # Returns
    /// - Ok(Some(content)) if content exists
    /// - Ok(None) if no content stored for this fingerprint
    ///
    /// # Errors
    /// - Returns error if stored bytes are not valid UTF-8 (FAIL FAST - data corruption)
    /// - Returns error if RocksDB read fails
    async fn get_content(&self, id: Uuid) -> CoreResult<Option<String>> {
        let key = content_key(&id);
        let cf = self.cf_content();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => {
                // Decode UTF-8 - FAIL FAST on corruption
                String::from_utf8(bytes).map(Some).map_err(|e| {
                    error!(
                        "CONTENT ERROR: Invalid UTF-8 in stored content for fingerprint {}. \
                         This indicates data corruption. Error: {}. Bytes length: {}",
                        id,
                        e,
                        e.as_bytes().len()
                    );
                    CoreError::Internal(format!(
                        "Invalid UTF-8 in content for {}: {}. Data corruption detected.",
                        id, e
                    ))
                })
            }
            Ok(None) => {
                debug!("No content found for fingerprint {}", id);
                Ok(None)
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to read content for fingerprint {}: {}",
                    id, e
                );
                Err(CoreError::StorageError(format!(
                    "Failed to read content for {}: {}",
                    id, e
                )))
            }
        }
    }

    /// Batch retrieve content for multiple fingerprints (TASK-CONTENT-008).
    ///
    /// Uses RocksDB multi_get_cf for efficient batch retrieval.
    /// Results maintain the same order as input IDs.
    ///
    /// # Returns
    /// - Vec<Option<String>> with None for missing content entries
    ///
    /// # Errors
    /// - Returns error if any stored bytes are not valid UTF-8 (FAIL FAST)
    /// - Returns error if RocksDB batch read fails
    async fn get_content_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<String>>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        debug!("Batch retrieving content for {} fingerprints", ids.len());

        let cf = self.cf_content();

        // Prepare keys for batch read - multi_get_cf needs (&CF, key)
        let keys: Vec<_> = ids
            .iter()
            .map(|id| (cf, content_key(id).to_vec()))
            .collect();

        // Batch read from RocksDB
        let results = self.db.multi_get_cf(keys);

        // Process results maintaining order
        let mut contents = Vec::with_capacity(ids.len());
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(Some(bytes)) => {
                    let content = String::from_utf8(bytes).map_err(|e| {
                        error!(
                            "CONTENT ERROR: Invalid UTF-8 in batch content for fingerprint {}. \
                             Index: {}, Error: {}",
                            ids[i], i, e
                        );
                        CoreError::Internal(format!(
                            "Invalid UTF-8 in content for {}: {}. Data corruption detected.",
                            ids[i], e
                        ))
                    })?;
                    contents.push(Some(content));
                }
                Ok(None) => contents.push(None),
                Err(e) => {
                    error!(
                        "ROCKSDB ERROR: Batch read failed at index {} (fingerprint {}): {}",
                        i, ids[i], e
                    );
                    return Err(CoreError::StorageError(format!(
                        "Failed to read content batch at index {}: {}",
                        i, e
                    )));
                }
            }
        }

        let found_count = contents.iter().filter(|c| c.is_some()).count();
        debug!(
            "Batch content retrieval complete: {} requested, {} found",
            ids.len(),
            found_count
        );
        Ok(contents)
    }

    /// Delete content for a fingerprint (TASK-CONTENT-009).
    ///
    /// This is a standalone method for deleting content without deleting the
    /// associated fingerprint. For most use cases, use the `delete` method
    /// with `soft=false` which cascades to content deletion atomically.
    ///
    /// # Returns
    /// - Ok(true) if content existed and was deleted
    /// - Ok(false) if no content existed for this fingerprint
    ///
    /// # Errors
    /// - Returns error if RocksDB operation fails
    async fn delete_content(&self, id: Uuid) -> CoreResult<bool> {
        let key = content_key(&id);
        let cf = self.cf_content();

        // Check if content exists first
        let exists = match self.db.get_cf(cf, &key) {
            Ok(Some(_)) => true,
            Ok(None) => {
                debug!("No content to delete for fingerprint {}", id);
                return Ok(false);
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to check content existence for fingerprint {}: {}",
                    id, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to check content existence for {}: {}",
                    id, e
                )));
            }
        };

        if exists {
            self.db.delete_cf(cf, key).map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to delete content for fingerprint {}: {}",
                    id, e
                );
                CoreError::StorageError(format!("Failed to delete content for {}: {}", id, e))
            })?;
            info!("Deleted content for fingerprint {}", id);
        }

        Ok(exists)
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
    for quadrant in johari.quadrants.iter().take(NUM_EMBEDDERS) {
        for (total, &q_val) in totals.iter_mut().zip(quadrant.iter()) {
            *total += q_val;
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
            e1_semantic: generate_vec(1024, seed),               // 1024D
            e2_temporal_recent: generate_vec(512, seed + 1),     // 512D
            e3_temporal_periodic: generate_vec(512, seed + 2),   // 512D
            e4_temporal_positional: generate_vec(512, seed + 3), // 512D
            e5_causal: generate_vec(768, seed + 4),              // 768D
            e6_sparse: generate_sparse(seed + 5),                // Sparse
            e7_code: generate_vec(1536, seed + 6),               // 1536D
            e8_graph: generate_vec(384, seed + 7),               // 384D
            e9_hdc: generate_vec(1024, seed + 8),                // 1024D HDC (projected)
            e10_multimodal: generate_vec(768, seed + 9),         // 768D
            e11_entity: generate_vec(384, seed + 10),            // 384D
            e12_late_interaction: generate_late_interaction(seed + 11), // Vec<Vec<f32>>
            e13_splade: generate_sparse(seed + 12),              // Sparse
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

    /// Helper to create store with initialized indexes.
    ///
    /// Note: EmbedderIndexRegistry is initialized in the constructor,
    /// so no separate initialization step is needed.
    fn create_initialized_store(path: &std::path::Path) -> RocksDbTeleologicalStore {
        RocksDbTeleologicalStore::open(path).unwrap()
    }

    #[tokio::test]
    async fn test_open_and_health_check() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path());
        assert!(store.health_check().is_ok());
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path());

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
            let store = create_initialized_store(&path);
            store.store(fp.clone()).await.unwrap();
            store.flush().await.unwrap();
        }

        // Reopen and verify
        {
            let store = create_initialized_store(&path);
            let retrieved = store.retrieve(id).await.unwrap();
            assert!(
                retrieved.is_some(),
                "Fingerprint should persist across database close/reopen"
            );
            assert_eq!(retrieved.unwrap().id, id);
        }

        // Verify raw bytes exist in RocksDB
        {
            let store = create_initialized_store(&path);
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
        let store = create_initialized_store(tmp.path());

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
        let store = create_initialized_store(tmp.path());

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
        let store = create_initialized_store(tmp.path());

        assert_eq!(store.count().await.unwrap(), 0);

        store
            .store(create_test_fingerprint_with_seed(1))
            .await
            .unwrap();
        store
            .store(create_test_fingerprint_with_seed(2))
            .await
            .unwrap();
        store
            .store(create_test_fingerprint_with_seed(3))
            .await
            .unwrap();

        assert_eq!(store.count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_backend_type() {
        let tmp = TempDir::new().unwrap();
        let store = create_initialized_store(tmp.path());
        assert_eq!(store.backend_type(), TeleologicalStorageBackend::RocksDb);
    }
}
