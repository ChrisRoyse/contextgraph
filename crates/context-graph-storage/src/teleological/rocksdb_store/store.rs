//! Core RocksDbTeleologicalStore struct and constructor.
//!
//! This module contains the main store struct, its constructor methods,
//! and basic accessor methods. Complex operations are in separate modules:
//! - `index_ops.rs` - HNSW index operations
//! - `inverted_index.rs` - SPLADE inverted index operations

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
// P5: DashMap replaces RwLock<HashMap> for lock-free concurrent soft-delete checks.
// Eliminates RwLock contention on the read-heavy search path (5,000+ lock acquisitions
// under concurrent search load). DashMap uses sharded locks internally for O(1)
// concurrent reads without global lock acquisition.
use dashmap::DashMap;
// MED-11 FIX: parking_lot::RwLock is non-poisonable. One panic no longer
// permanently breaks all subsequent operations via poison cascade.
use parking_lot::RwLock;

use bincode;
use rocksdb::{Cache, ColumnFamily, Options, WriteBatch, DB};
use serde_json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::types::fingerprint::TeleologicalFingerprint;

use crate::column_families::{cf_names, get_all_column_family_descriptors};
use crate::teleological::column_families::{
    CF_CONTENT, CF_E12_LATE_INTERACTION, CF_E1_MATRYOSHKA_128, CF_FINGERPRINTS, CF_SOURCE_METADATA,
    QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS, CODE_CFS, CAUSAL_CFS,
};
use crate::teleological::indexes::EmbedderIndexRegistry;

use super::causal_hnsw_index::CausalE11Index;
use crate::teleological::schema::{
    e12_late_interaction_key, e1_matryoshka_128_key, fingerprint_key,
};
use crate::teleological::serialization::{
    serialize_e1_matryoshka_128, serialize_teleological_fingerprint,
};

use super::types::{TeleologicalStoreConfig, TeleologicalStoreError, TeleologicalStoreResult};

// ============================================================================
// Main Store Struct
// ============================================================================

/// RocksDB-backed storage for TeleologicalFingerprints.
///
/// Implements the `TeleologicalMemoryStore` trait with persistent storage
/// across 51 column families (11 base + 20 teleological + 13 quantized + 5 code + 2 causal).
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
    pub(crate) db: Arc<DB>,
    /// Shared block cache across column families.
    #[allow(dead_code)]
    pub(crate) cache: Cache,
    /// Database path.
    pub(crate) path: PathBuf,
    /// In-memory count of fingerprints (cached for performance).
    pub(crate) fingerprint_count: RwLock<Option<usize>>,
    /// P1: Total document count for IDF calculations (O(1) instead of O(n) iterator scan).
    /// Initialized at startup, incremented on store, decremented on hard delete.
    /// Used by sparse search (E6/E13) BM25-IDF scoring. Approximate is fine for IDF.
    pub(crate) total_doc_count: Arc<AtomicUsize>,
    /// P5: Soft-deleted IDs with deletion timestamps (Unix epoch milliseconds).
    /// Uses DashMap for lock-free concurrent reads (eliminates RwLock contention
    /// under 100+ concurrent searches x 50+ results = 5,000+ lookups).
    /// Wrapped in Arc for cheap cloning into spawn_blocking closures.
    pub(crate) soft_deleted: Arc<DashMap<Uuid, i64>>,
    /// Per-embedder index registry with 12 HNSW indexes for O(log n) ANN search.
    /// E6, E12, E13 use different index types (inverted/MaxSim).
    /// NO FALLBACKS - FAIL FAST on invalid operations.
    pub(crate) index_registry: Arc<EmbedderIndexRegistry>,
    /// E11 HNSW index for causal relationships.
    /// Enables O(log n) entity search instead of O(n) brute-force RocksDB scan.
    /// See causal_hnsw_index.rs for implementation details.
    pub(crate) causal_e11_index: Arc<CausalE11Index>,
    /// STG-03/04/10 FIX: Mutex serializing secondary index read-modify-write operations.
    ///
    /// Protects against lost-update races where concurrent writes to the same
    /// posting list (inverted indexes, causal_by_source, file index) cause one
    /// write to silently overwrite the other's additions.
    ///
    /// Covers:
    /// - E13 SPLADE inverted index (store_fingerprint_internal)
    /// - E6 sparse inverted index (store_fingerprint_internal)
    /// - Causal by-source secondary index (store_causal_relationship)
    /// - File path index (index_file_fingerprint_async)
    ///
    /// Uses parking_lot::Mutex for non-poisonable, fast uncontended locking.
    pub(crate) secondary_index_lock: parking_lot::Mutex<()>,
}

// ============================================================================
// Constructor and Open Methods
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Open a teleological store at the specified path with default configuration.
    ///
    /// Creates the database and all 51 column families if they don't exist.
    /// **Automatically detects and removes stale lock files.**
    pub fn open<P: AsRef<Path>>(path: P) -> TeleologicalStoreResult<Self> {
        Self::open_with_config(path, TeleologicalStoreConfig::default())
    }

    /// Open a teleological store with custom configuration.
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
        if path_buf.exists() {
            match Self::detect_and_remove_stale_lock(&path_buf) {
                Ok(true) => {
                    info!("Removed stale lock at '{}', proceeding with open", path_str);
                }
                Ok(false) => {
                    debug!("No stale lock detected at '{}'", path_str);
                }
                Err(e) => {
                    error!(
                        "FAIL FAST: Stale lock cleanup failed at '{}': {}",
                        path_str, e
                    );
                    return Err(e);
                }
            }

            // NOTE: We let RocksDB detect corruption during open rather than pre-checking.
            // RocksDB knows exactly which files it needs, avoiding false positives.
            // If open fails with corruption, we transform the error below.
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

        // Get ALL column families (51 total: 11 base + 20 teleological + 13 quantized + 5 code + 2 causal)
        // This includes the graph edge CFs (embedder_edges, typed_edges, typed_edges_by_type)
        // required for K-NN graph-based retrieval. NO FALLBACKS - database must have all CFs.
        let cf_descriptors = get_all_column_family_descriptors(&cache);

        debug!(
            "Opening database with {} column families",
            cf_descriptors.len()
        );

        // Open database with all column families
        let db = DB::open_cf_descriptors(&db_opts, &path_str, cf_descriptors).map_err(|e| {
            error!("Failed to open RocksDB at '{}': {}", path_str, e);
            Self::transform_corruption_error(&path_str, e)
        })?;

        // Create per-embedder index registry (15 HNSW indexes)
        let index_registry = Arc::new(EmbedderIndexRegistry::new());

        // Create E11 HNSW index for causal relationships
        let causal_e11_index = Arc::new(CausalE11Index::new());

        info!(
            "Successfully opened RocksDbTeleologicalStore with {} column families and {} per-embedder indexes",
            TELEOLOGICAL_CFS.len() + QUANTIZED_EMBEDDER_CFS.len(),
            index_registry.len()
        );

        let db_arc = Arc::new(db);

        // SEC-06-FIX: Load persisted soft-delete markers from CF_SYSTEM BEFORE index rebuild.
        // Without this, soft-deleted memories get added back to HNSW indexes on restart.
        let soft_deleted = {
            use super::crud::SOFT_DELETE_PREFIX;

            // P5: Use DashMap for lock-free concurrent reads on the search hotpath
            let map: DashMap<Uuid, i64> = DashMap::new();
            if let Some(cf_system) = db_arc.cf_handle(crate::column_families::cf_names::SYSTEM) {
                let iter = db_arc.prefix_iterator_cf(cf_system, SOFT_DELETE_PREFIX.as_bytes());
                for item in iter {
                    match item {
                        Ok((key, value)) => {
                            let key_str = String::from_utf8_lossy(&key);
                            if let Some(uuid_str) = key_str.strip_prefix(SOFT_DELETE_PREFIX) {
                                if let Ok(id) = uuid::Uuid::parse_str(uuid_str) {
                                    // Parse timestamp from 8-byte big-endian i64.
                                    // Legacy `b"1"` markers (1 byte) get timestamp 0
                                    // (immediately eligible for GC).
                                    let ts = if value.len() == 8 {
                                        i64::from_be_bytes(value[..8].try_into().unwrap())
                                    } else {
                                        warn!(
                                            "Legacy soft-delete marker for {} ({}B value), \
                                             treating as timestamp 0 (immediate GC eligible)",
                                            id, value.len()
                                        );
                                        0
                                    };
                                    map.insert(id, ts);
                                }
                            } else {
                                // Prefix iterator went past our prefix -- stop
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Error reading soft-delete markers: {}", e);
                            break;
                        }
                    }
                }
            }
            if !map.is_empty() {
                info!("Loaded {} persisted soft-delete markers from CF_SYSTEM", map.len());
            }
            Arc::new(map)
        };

        // P1: Count total documents for O(1) IDF lookups in sparse search.
        // This replaces the O(n) full-iterator scan that was the #1 scaling bottleneck.
        let total_doc_count = {
            let cf_fp = db_arc.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
                TeleologicalStoreError::ColumnFamilyNotFound {
                    name: CF_FINGERPRINTS.to_string(),
                }
            })?;
            let raw_count = db_arc.iterator_cf(cf_fp, rocksdb::IteratorMode::Start).count();
            // DAT-1: Subtract soft-deleted entries so total_doc_count reflects
            // only live documents. IDF calculations use this count as denominator,
            // and inflating it with soft-deleted entries biases term weights downward.
            let count = raw_count.saturating_sub(soft_deleted.len());
            info!(
                "P1: Initialized total_doc_count = {} from CF_FINGERPRINTS (raw={}, soft_deleted={})",
                count, raw_count, soft_deleted.len()
            );
            Arc::new(AtomicUsize::new(count))
        };

        let store = Self {
            db: db_arc,
            cache,
            path: path_buf,
            fingerprint_count: RwLock::new(None),
            total_doc_count,
            soft_deleted,
            index_registry,
            causal_e11_index,
            secondary_index_lock: parking_lot::Mutex::new(()),
        };

        // Try fast path: load HNSW indexes from CF_HNSW_GRAPHS (persisted graphs).
        // Falls back to O(n) rebuild from CF_FINGERPRINTS if no persisted data or errors.
        match store.try_load_hnsw_indexes() {
            Ok(true) => {
                debug!("HNSW indexes loaded from persisted CF_HNSW_GRAPHS");
            }
            Ok(false) => {
                // No persisted HNSW data — full rebuild from fingerprints
                store.rebuild_indexes_from_store()?;
            }
            Err(e) => {
                warn!(error = %e, "HNSW restore from disk failed — falling back to O(n) rebuild. If this persists, investigate HNSW data corruption.");
                store.rebuild_indexes_from_store()?;
            }
        }

        // Rebuild E11 HNSW index from existing causal relationships
        store.rebuild_causal_e11_index()?;

        Ok(store)
    }

    /// Detect and remove stale RocksDB lock files.
    fn detect_and_remove_stale_lock<P: AsRef<Path>>(path: P) -> TeleologicalStoreResult<bool> {
        let lock_path = path.as_ref().join("LOCK");
        let lock_path_str = lock_path.to_string_lossy().to_string();

        if !lock_path.exists() {
            debug!(
                "No LOCK file at '{}' - database not previously opened",
                lock_path_str
            );
            return Ok(false);
        }

        info!(
            "LOCK file exists at '{}' - checking if stale",
            lock_path_str
        );

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;

            let file = match fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&lock_path)
            {
                Ok(f) => f,
                Err(e) => {
                    warn!("Cannot open LOCK file for inspection: {}", e);
                    return Self::try_remove_lock_file(&lock_path, &lock_path_str);
                }
            };

            let fd = file.as_raw_fd();
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };

            if result == 0 {
                info!(
                    "Acquired exclusive lock - LOCK file at '{}' is STALE",
                    lock_path_str
                );
                unsafe { libc::flock(fd, libc::LOCK_UN) };
                drop(file);
                Self::try_remove_lock_file(&lock_path, &lock_path_str)
            } else {
                let errno = std::io::Error::last_os_error();
                if errno.raw_os_error() == Some(libc::EWOULDBLOCK) {
                    info!(
                        "LOCK file at '{}' is held by another process - NOT stale",
                        lock_path_str
                    );
                    Ok(false)
                } else {
                    warn!("flock() failed with unexpected error: {}", errno);
                    Self::try_remove_lock_file(&lock_path, &lock_path_str)
                }
            }
        }

        #[cfg(windows)]
        {
            // L11 FIX: On Windows, we cannot safely probe for live lock holders (no flock).
            // Log a warning instead of blindly removing — could kill a live process's lock.
            warn!(
                "LOCK file at '{}' found on Windows — cannot verify if held by live process. \
                 Attempting removal (may fail if locked by another process).",
                lock_path_str
            );
            Self::try_remove_lock_file(&lock_path, &lock_path_str)
        }

        #[cfg(not(any(unix, windows)))]
        {
            warn!(
                "LOCK file at '{}' found on unknown OS — attempting removal",
                lock_path_str
            );
            Self::try_remove_lock_file(&lock_path, &lock_path_str)
        }
    }

    /// Attempt to remove a stale lock file.
    fn try_remove_lock_file(
        lock_path: &Path,
        lock_path_str: &str,
    ) -> TeleologicalStoreResult<bool> {
        match fs::remove_file(lock_path) {
            Ok(()) => {
                info!(
                    "Successfully removed stale LOCK file at '{}'",
                    lock_path_str
                );
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
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
                        Manual intervention required: remove '{}'",
                        e, lock_path_str
                    ),
                })
            }
        }
    }

    /// Transform RocksDB errors into detailed, user-friendly error types.
    ///
    /// When RocksDB's open fails with corruption-related errors, this function
    /// transforms them into `CorruptionDetected` errors with:
    /// - Database path
    /// - Missing file details (extracted from error message)
    /// - Recovery options
    ///
    /// This approach is more reliable than pre-checking because RocksDB knows
    /// exactly which files it needs, avoiding false positives.
    fn transform_corruption_error(path: &str, error: rocksdb::Error) -> TeleologicalStoreError {
        let err_msg = error.to_string();

        // Check for corruption patterns in the error message
        let is_corruption = err_msg.contains("Corruption")
            || (err_msg.contains("No such file or directory") && err_msg.contains(".sst"))
            || (err_msg.contains("MANIFEST")
                && (err_msg.contains("corrupted") || err_msg.contains("missing")));

        if !is_corruption {
            return TeleologicalStoreError::OpenFailed {
                path: path.to_string(),
                message: err_msg,
            };
        }

        // Extract missing SST file from error message
        // Pattern: "While open a file for random read: /path/000682.sst: No such file"
        // NOTE: Uses '/' as path separator since RocksDB errors use Unix-style paths
        // even on Windows. This is a best-effort extraction for debugging context.
        let mut missing_files = Vec::new();
        for part in err_msg.split_whitespace() {
            if part.ends_with(".sst") || part.ends_with(".sst:") {
                if let Some(filename) = part.trim_end_matches(':').rsplit('/').next() {
                    missing_files.push(filename.to_string());
                }
            }
        }

        // Get MANIFEST file from error message if present
        let manifest_file = err_msg
            .split_whitespace()
            .find(|s| s.contains("MANIFEST-"))
            .map(|s| {
                s.trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
                    .to_string()
            })
            .unwrap_or_else(|| "unknown".to_string());

        let missing_files_str = if missing_files.is_empty() {
            "see error details".to_string()
        } else {
            missing_files.join(", ")
        };

        error!(
            "FAIL FAST: CORRUPTION DETECTED at '{}' - RocksDB reports: {}",
            path, err_msg
        );
        error!(
            "Recovery options: (1) Delete database and restore from backup, \
             (2) Use 'ldb repair' tool (may lose data), \
             (3) Restore from snapshot"
        );

        TeleologicalStoreError::CorruptionDetected {
            path: path.to_string(),
            missing_count: missing_files.len().max(1),
            missing_files: missing_files_str,
            manifest_file,
        }
    }

}

// ============================================================================
// Column Family Accessors
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Get a column family handle by name.
    pub(crate) fn get_cf(&self, name: &str) -> TeleologicalStoreResult<&ColumnFamily> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| TeleologicalStoreError::ColumnFamilyNotFound {
                name: name.to_string(),
            })
    }

    /// Get the content column family handle (FAIL FAST on missing).
    #[inline]
    pub(crate) fn cf_content(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONTENT)
            .expect("CF_CONTENT must exist - database initialization failed")
    }

    /// Get the source_metadata column family handle (FAIL FAST on missing).
    #[inline]
    pub(crate) fn cf_source_metadata(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_SOURCE_METADATA)
            .expect("CF_SOURCE_METADATA must exist - database initialization failed")
    }
}

// ============================================================================
// Index Rebuilding
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Rebuild all HNSW indexes from existing RocksDB fingerprints.
    ///
    /// This is called during `open()` to restore indexes on restart.
    /// The HNSW indexes are in-memory and need to be rebuilt from RocksDB data.
    ///
    /// # FAIL FAST: Errors during rebuild cause store open to fail.
    ///
    /// # Returns
    ///
    /// Ok(()) if rebuilding succeeds, Err if any fingerprint fails to add.
    fn rebuild_indexes_from_store(&self) -> TeleologicalStoreResult<()> {
        use crate::teleological::column_families::CF_FINGERPRINTS;
        use crate::teleological::schema::parse_fingerprint_key;
        use crate::teleological::serialization::deserialize_teleological_fingerprint;

        let start = std::time::Instant::now();

        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut success_count = 0;
        let mut error_count = 0;

        for item in iter {
            let (key, value) = item.map_err(|e| {
                error!("FAIL FAST: RocksDB iteration failed during index rebuild: {}", e);
                TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
            })?;

            // Parse fingerprint ID from key
            let id = parse_fingerprint_key(&key);

            // Skip soft-deleted fingerprints
            if self.is_soft_deleted(&id) {
                continue;
            }

            // Deserialize fingerprint - skip corrupted records
            let fp = match deserialize_teleological_fingerprint(&value) {
                Ok(fp) => fp,
                Err(e) => {
                    warn!(
                        "Skipping corrupted fingerprint {} during index rebuild: {}",
                        id, e
                    );
                    error_count += 1;
                    continue;
                }
            };

            // Add to HNSW indexes - FAIL FAST on error
            match self.add_to_indexes(&fp) {
                Ok(()) => {
                    success_count += 1;
                }
                Err(e) => {
                    error!(
                        "FAIL FAST: Failed to add fingerprint {} to indexes during rebuild: {}",
                        id, e
                    );
                    error_count += 1;
                    // Continue to count all errors, but we'll fail at the end
                }
            }
        }

        let elapsed = start.elapsed();

        if error_count > 0 {
            error!(
                "FAIL FAST: Index rebuild failed with {} errors out of {} fingerprints in {:?}",
                error_count, success_count + error_count, elapsed
            );
            return Err(TeleologicalStoreError::Internal(format!(
                "Index rebuild failed: {} fingerprints could not be added to indexes",
                error_count
            )));
        }

        if success_count > 0 {
            info!(
                "Rebuilt HNSW indexes: {} fingerprints added to {} indexes in {:?}",
                success_count,
                self.index_registry.len(),
                elapsed
            );
        } else {
            debug!("No fingerprints to rebuild indexes from (empty store)");
        }

        Ok(())
    }

    /// Rebuild E11 HNSW index from existing causal relationships in RocksDB.
    ///
    /// This is called during `open()` to restore the causal E11 index on restart.
    /// The HNSW index is in-memory and needs to be rebuilt from RocksDB data.
    ///
    /// # Performance
    ///
    /// This scans all causal relationships and inserts those with E11 embeddings
    /// into the HNSW index. For large collections, this may take a few seconds.
    ///
    /// # Returns
    ///
    /// Ok(()) if rebuilding succeeds, Err if any critical operation fails.
    pub(crate) fn rebuild_causal_e11_index(&self) -> TeleologicalStoreResult<()> {
        use crate::teleological::column_families::CF_CAUSAL_RELATIONSHIPS;
        use context_graph_core::types::CausalRelationship;

        let start = std::time::Instant::now();

        let cf = self
            .db
            .cf_handle(CF_CAUSAL_RELATIONSHIPS)
            .ok_or_else(|| TeleologicalStoreError::ColumnFamilyNotFound {
                name: CF_CAUSAL_RELATIONSHIPS.to_string(),
            })?;

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut success_count = 0;
        let mut skip_count = 0;
        let mut error_count = 0;

        for item in iter {
            let (_key, value) = match item {
                Ok((k, v)) => (k, v),
                Err(e) => {
                    error!(
                        "FAIL FAST: RocksDB iteration failed during causal E11 index rebuild: {}",
                        e
                    );
                    error_count += 1;
                    continue;
                }
            };

            // Deserialize causal relationship (JSON format)
            let relationship: CausalRelationship = match serde_json::from_slice(&value) {
                Ok(r) => r,
                Err(e) => {
                    let key_id = (_key.len() == 16)
                        .then(|| Uuid::from_slice(&_key).ok())
                        .flatten();
                    error!(
                        "Failed to deserialize causal relationship {:?} during E11 index rebuild: {}",
                        key_id, e
                    );
                    error_count += 1;
                    continue;
                }
            };

            // Only index relationships with E11 embeddings
            if !relationship.has_entity_embedding() {
                skip_count += 1;
                continue;
            }

            // Add to HNSW index
            match self
                .causal_e11_index
                .insert(relationship.id, relationship.e11_embedding())
            {
                Ok(()) => {
                    success_count += 1;
                }
                Err(e) => {
                    error!(
                        "Failed to add causal relationship {} to E11 HNSW index: {}",
                        relationship.id, e
                    );
                    error_count += 1;
                }
            }
        }

        let elapsed = start.elapsed();

        if error_count > 0 {
            error!(
                "FAIL FAST: Causal E11 index rebuild failed with {} errors (indexed: {}, skipped: {}) in {:?}",
                error_count, success_count, skip_count, elapsed
            );
            return Err(TeleologicalStoreError::Internal(format!(
                "Causal E11 index rebuild failed: {} relationships could not be processed",
                error_count
            )));
        } else if success_count > 0 {
            info!(
                "Rebuilt causal E11 HNSW index: {} relationships indexed, {} skipped (no E11) in {:?}",
                success_count, skip_count, elapsed
            );
        } else {
            debug!("No causal relationships with E11 embeddings to rebuild (indexed: 0, skipped: {})", skip_count);
        }

        Ok(())
    }
}

// ============================================================================
// HNSW Index Persistence (CF_HNSW_GRAPHS)
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Persist all HNSW indexes to CF_HNSW_GRAPHS.
    ///
    /// Stores each index as two key-value pairs:
    /// - `graph:{EmbedderDebugName}` → usearch serialized graph bytes
    /// - `meta:{EmbedderDebugName}` → JSON of UUID↔key mappings + next_key
    ///
    /// Empty indexes are skipped. Uses a WriteBatch for atomicity.
    pub fn persist_hnsw_indexes(&self) -> TeleologicalStoreResult<()> {
        use crate::teleological::column_families::CF_HNSW_GRAPHS;

        let start = std::time::Instant::now();
        let cf = self.get_cf(CF_HNSW_GRAPHS)?;

        let mut batch = rocksdb::WriteBatch::default();
        let mut persisted = 0usize;
        let mut skipped = 0usize;

        for (embedder, index) in self.index_registry.iter() {
            let name = format!("{:?}", embedder);

            let graph_data = match index.serialize_graph() {
                Ok(Some(data)) => data,
                Ok(None) => {
                    skipped += 1;
                    continue;
                }
                Err(e) => {
                    error!("Failed to serialize HNSW graph for {}: {}", name, e);
                    return Err(TeleologicalStoreError::Internal(format!(
                        "HNSW graph serialization failed for {}: {}",
                        name, e
                    )));
                }
            };

            let meta_data = match index.serialize_metadata() {
                Some(data) => data,
                None => {
                    skipped += 1;
                    continue;
                }
            };

            let graph_key = format!("graph:{}", name);
            let meta_key = format!("meta:{}", name);

            batch.put_cf(cf, graph_key.as_bytes(), &graph_data);
            batch.put_cf(cf, meta_key.as_bytes(), &meta_data);
            persisted += 1;
        }

        if persisted > 0 {
            self.db.write(batch).map_err(|e| {
                error!("Failed to write HNSW index batch to RocksDB: {}", e);
                TeleologicalStoreError::rocksdb_op("write_batch", CF_HNSW_GRAPHS, None, e)
            })?;
        }

        let elapsed = start.elapsed();
        if persisted > 0 {
            info!(
                "Persisted {} HNSW indexes to CF_HNSW_GRAPHS ({} empty/skipped) in {:?}",
                persisted, skipped, elapsed
            );
        }

        Ok(())
    }

    /// H1/M9 FIX: Check if any HNSW index needs compaction and rebuild if so.
    ///
    /// Compaction is triggered when > 25% of vectors in any index are orphaned
    /// (removed from UUID maps but still consuming memory in usearch graph).
    /// This rebuilds ALL indexes from CF_FINGERPRINTS, eliminating all orphans.
    pub fn compact_hnsw_if_needed(&self) -> TeleologicalStoreResult<()> {
        // DATA-5: RACE CONDITION WARNING
        // HNSW compaction rebuilds indexes from CF_FINGERPRINTS while concurrent
        // store/delete operations may be modifying the same indexes. This can cause:
        // 1. Duplicate usearch entries if an insert happens during rebuild
        // 2. Missing entries if an insert completes after rebuild reads CF_FINGERPRINTS
        //    but before rebuild finishes writing the new index
        // This is an acknowledged design concern — a full fix would require a
        // read-write lock around all HNSW operations during compaction. For now,
        // operators should schedule compaction during low-traffic periods.
        let mut any_needs_compaction = false;
        for (embedder, index) in self.index_registry.iter() {
            if index.needs_compaction() {
                let removed = index.removed_count();
                let total = index.usearch_size();
                info!(
                    "HNSW compaction needed for {:?}: {removed} orphaned / {total} total ({:.0}%)",
                    embedder,
                    (removed as f64 / total as f64) * 100.0
                );
                any_needs_compaction = true;
            }
        }

        if any_needs_compaction {
            // DATA-5: Warn about race condition only when compaction actually runs
            warn!(
                "DATA-5: HNSW compaction starting — concurrent inserts/deletes may race \
                 with index rebuild. Schedule compaction during low-traffic periods."
            );
            info!("H1 FIX: Rebuilding all HNSW indexes from CF_FINGERPRINTS to eliminate orphaned vectors");
            self.rebuild_indexes_from_store()?;
            // Reset all removed counts after successful rebuild
            for (_embedder, index) in self.index_registry.iter() {
                index.reset_removed_count();
            }
            info!("HNSW compaction complete — all orphaned vectors eliminated");
        }

        Ok(())
    }

    /// Try to load HNSW indexes from CF_HNSW_GRAPHS.
    ///
    /// Returns `true` if at least one index was restored, `false` if CF is empty.
    /// On any deserialization error, returns `Err` so caller can fall back to rebuild.
    fn try_load_hnsw_indexes(&self) -> TeleologicalStoreResult<bool> {
        use crate::teleological::column_families::CF_HNSW_GRAPHS;

        let start = std::time::Instant::now();
        let cf = self.get_cf(CF_HNSW_GRAPHS)?;

        let mut restored = 0usize;
        let mut errors = Vec::new();

        for (embedder, index) in self.index_registry.iter() {
            let name = format!("{:?}", embedder);
            let graph_key = format!("graph:{}", name);
            let meta_key = format!("meta:{}", name);

            // Read both graph and metadata
            let graph_data = match self.db.get_cf(cf, graph_key.as_bytes()) {
                Ok(Some(data)) => data,
                Ok(None) => continue, // No persisted data for this index
                Err(e) => {
                    errors.push(format!("{}: graph read failed: {}", name, e));
                    continue;
                }
            };

            let meta_data = match self.db.get_cf(cf, meta_key.as_bytes()) {
                Ok(Some(data)) => data,
                Ok(None) => {
                    errors.push(format!("{}: graph exists but metadata missing", name));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("{}: metadata read failed: {}", name, e));
                    continue;
                }
            };

            // Restore the index
            match index.restore_from_persisted(&graph_data, &meta_data) {
                Ok(count) => {
                    debug!("Restored HNSW index for {} ({} vectors)", name, count);
                    restored += 1;
                }
                Err(e) => {
                    errors.push(format!("{}: restore failed: {}", name, e));
                }
            }
        }

        let elapsed = start.elapsed();

        if !errors.is_empty() {
            warn!(
                "HNSW index restore had {} errors (restored {}): {:?}",
                errors.len(),
                restored,
                errors
            );
            // Return error so caller falls back to full rebuild
            return Err(TeleologicalStoreError::Internal(format!(
                "HNSW index restore failed for {} indexes: {}",
                errors.len(),
                errors.join("; ")
            )));
        }

        if restored > 0 {
            info!(
                "Restored {} HNSW indexes from CF_HNSW_GRAPHS in {:?}",
                restored, elapsed
            );
        }

        Ok(restored > 0)
    }
}

// ============================================================================
// Core Storage Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Store a fingerprint in all relevant column families.
    ///
    /// Writes to:
    /// 1. `fingerprints` - Full serialized fingerprint
    /// 2. `topic_profiles` - 13D topic profile
    /// 3. `e1_matryoshka_128` - Truncated E1 embedding for Stage 2
    /// 4. `e13_splade_inverted` - Updates inverted index for Stage 1
    /// 5. `e12_late_interaction` - ColBERT token embeddings for Stage 5
    pub(crate) fn store_fingerprint_internal(
        &self,
        fp: &TeleologicalFingerprint,
    ) -> TeleologicalStoreResult<()> {
        let id = fp.id;
        let key = fingerprint_key(&id);

        // STG-04 FIX: Hold secondary_index_lock for the entire read-modify-write cycle.
        // Without this, concurrent calls to store_fingerprint_internal that share
        // inverted index terms (E13/E6) can race on the read-then-write of posting
        // lists, causing one caller's ID to be silently dropped from the index.
        let _index_guard = self.secondary_index_lock.lock();

        let mut batch = WriteBatch::default();

        // 1. Store full fingerprint
        let cf_fingerprints = self.get_cf(CF_FINGERPRINTS)?;
        let serialized = serialize_teleological_fingerprint(fp);
        batch.put_cf(cf_fingerprints, key, &serialized);

        // 2. Topic profile storage is done via CF_TOPIC_PROFILES when topic_profile is set on fingerprint

        // 3. Store E1 Matryoshka 128D truncated vector
        let cf_matryoshka = self.get_cf(CF_E1_MATRYOSHKA_128)?;
        let matryoshka_key = e1_matryoshka_128_key(&id);
        let mut truncated = [0.0f32; 128];
        let e1 = &fp.semantic.e1_semantic;
        let copy_len = std::cmp::min(e1.len(), 128);
        truncated[..copy_len].copy_from_slice(&e1[..copy_len]);
        let matryoshka_bytes = serialize_e1_matryoshka_128(&truncated);
        batch.put_cf(cf_matryoshka, matryoshka_key, matryoshka_bytes);

        // 4. Update E13 SPLADE inverted index
        self.update_splade_inverted_index(&mut batch, &id, &fp.semantic.e13_splade)?;

        // 4b. Update E6 sparse inverted index (if e6_sparse is present)
        // Per e6upgrade.md: E6 sparse enables Stage 1 dual recall and Stage 3.5 tie-breaker
        if let Some(e6_sparse) = &fp.e6_sparse {
            self.update_e6_sparse_inverted_index(&mut batch, &id, e6_sparse)?;
            debug!(
                "Updated E6 sparse inverted index for fingerprint {} ({} active terms)",
                id,
                e6_sparse.indices.len()
            );
        }

        // 5. Store E12 late interaction tokens
        if !fp.semantic.e12_late_interaction.is_empty() {
            let cf_e12 = self.get_cf(CF_E12_LATE_INTERACTION)?;
            let e12_key = e12_late_interaction_key(&id);
            let e12_bytes = bincode::serialize(&fp.semantic.e12_late_interaction).map_err(|e| {
                error!(
                    "Failed to serialize E12 tokens for fingerprint {}: {}",
                    id, e
                );
                TeleologicalStoreError::Serialization {
                    id: Some(id),
                    message: format!("E12 token serialization failed: {}", e),
                }
            })?;
            batch.put_cf(cf_e12, e12_key, &e12_bytes);
            debug!(
                "Stored {} E12 tokens ({} bytes) for fingerprint {}",
                fp.semantic.e12_late_interaction.len(),
                e12_bytes.len(),
                id
            );
        }

        // Execute atomic batch write (still under lock)
        self.db.write(batch).map_err(|e| {
            error!("Failed to write fingerprint batch for {}: {}", id, e);
            TeleologicalStoreError::rocksdb_op("write_batch", CF_FINGERPRINTS, Some(id), e)
        })?;

        // Lock released here via drop(_index_guard)

        // Invalidate count cache and increment total doc count for IDF
        *self.fingerprint_count.write() = None;
        self.total_doc_count.fetch_add(1, Ordering::Relaxed);

        debug!("Stored fingerprint {} ({} bytes)", id, serialized.len());
        Ok(())
    }

    /// Retrieve raw fingerprint bytes from RocksDB.
    pub(crate) fn get_fingerprint_raw(&self, id: Uuid) -> TeleologicalStoreResult<Option<Vec<u8>>> {
        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let key = fingerprint_key(&id);

        self.db
            .get_cf(cf, key)
            .map_err(|e| TeleologicalStoreError::rocksdb_op("get", CF_FINGERPRINTS, Some(id), e))
    }

    /// Check if an ID is soft-deleted.
    ///
    /// P5: Uses DashMap for lock-free concurrent reads (no global RwLock contention).
    pub(crate) fn is_soft_deleted(&self, id: &Uuid) -> bool {
        self.soft_deleted.contains_key(id)
    }
}

// ============================================================================
// Public API Methods
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Get the database path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get a reference to the underlying RocksDB instance FOR TESTING/DIAGNOSTICS ONLY.
    #[doc(hidden)]
    pub fn db(&self) -> &DB {
        &self.db
    }

    /// Get an Arc reference to the underlying RocksDB instance.
    ///
    /// Use for creating EdgeRepository sharing the same database.
    /// The EdgeRepository requires access to graph edge column families.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_storage::graph_edges::EdgeRepository;
    ///
    /// let store = RocksDbTeleologicalStore::open("path/to/db")?;
    /// let db_arc = store.db_arc();
    /// let edge_repo = EdgeRepository::new(db_arc);
    /// ```
    pub fn db_arc(&self) -> Arc<DB> {
        Arc::clone(&self.db)
    }

    /// Invalidate the fingerprint count cache.
    ///
    /// Useful for benchmarking to force re-counting on each call.
    /// In normal operation, the cache is automatically invalidated
    /// when fingerprints are stored or deleted.
    pub fn invalidate_count_cache(&self) {
        *self.fingerprint_count.write() = None;
    }

    /// Health check: verify ALL 51 column families are accessible.
    pub fn health_check(&self) -> TeleologicalStoreResult<()> {
        let all_cf_arrays: &[&[&str]] = &[
            cf_names::ALL,
            TELEOLOGICAL_CFS,
            QUANTIZED_EMBEDDER_CFS,
            CODE_CFS,
            CAUSAL_CFS,
        ];

        for cf_names_arr in all_cf_arrays {
            for cf_name in *cf_names_arr {
                self.get_cf(cf_name)?;
            }
        }
        Ok(())
    }

    /// Get raw bytes from a specific column family (for debugging).
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
