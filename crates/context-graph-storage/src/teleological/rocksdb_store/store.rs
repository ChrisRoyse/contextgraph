//! Core RocksDbTeleologicalStore struct and constructor.
//!
//! This module contains the main store struct, its constructor methods,
//! and basic accessor methods. Complex operations are in separate modules:
//! - `index_ops.rs` - HNSW index operations
//! - `inverted_index.rs` - SPLADE inverted index operations

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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
use context_graph_core::weights::{E11_ENTITY_ENABLED, E5_CAUSAL_ENABLED};

use crate::column_families::{cf_names, get_all_column_family_descriptors};
use crate::dynamicjepa::column_families::DJ_CF_NAMES;
use crate::teleological::column_families::{
    CAUSAL_CFS, CF_CONTENT, CF_E12_LATE_INTERACTION, CF_E1_MATRYOSHKA_128, CF_FINGERPRINTS,
    CF_SOURCE_METADATA, CODE_CFS, QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS,
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
/// across 134 column families (ContextGraph, DynamicJEPA, and ME-JEPA).
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
    /// Per-embedder index registry with 15 HNSW indexes for O(log n) ANN search.
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
    /// DATA-5 FIX: RwLock protecting HNSW indexes during compaction/rebuild.
    ///
    /// - store/delete acquire **read lock** (concurrent, zero contention)
    /// - rebuild_indexes_from_store acquires **write lock** (exclusive during compaction)
    ///
    /// Compaction is infrequent (~10min or manual) so write lock contention is negligible.
    /// Prevents duplicate/missing entries from concurrent store + rebuild race.
    pub(crate) compaction_lock: RwLock<()>,
}

// ============================================================================
// Constructor and Open Methods
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Open a teleological store at the specified path with default configuration.
    ///
    /// Creates the database and all canonical column families if they don't exist.
    /// Existing RocksDB LOCK files are inspected for diagnostics only and are
    /// never removed by the store opener.
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

        // LOCK DIAGNOSTICS: Never remove RocksDB LOCK files here. RocksDB owns
        // its lock protocol and will fail the open if another process owns it.
        if path_buf.exists() {
            Self::inspect_rocksdb_lock(&path_buf)?;
            Self::assert_existing_dynamicjepa_column_families(&path_buf)?;

            // NOTE: We let RocksDB detect corruption during open rather than pre-checking.
            // RocksDB knows exactly which files it needs, avoiding false positives.
            // If open fails with corruption, we transform the error below.
        }

        // Create shared block cache
        let cache = Cache::new_lru_cache(config.block_cache_size);

        // Create DB options
        let mut db_opts = Options::default();
        db_opts.create_if_missing(config.create_if_missing);
        db_opts.create_missing_column_families(config.create_missing_column_families);
        db_opts.set_max_open_files(config.max_open_files);

        // CORRUPTION-RESILIENCE: Enable paranoid checks for early corruption detection.
        // Catches bit rot and SST file corruption at slight performance cost (~1-3%).
        db_opts.set_paranoid_checks(true);

        if !config.enable_wal {
            db_opts.set_manual_wal_flush(true);
        }

        // Get ALL column families, including DynamicJEPA. NO FALLBACKS - database must have all CFs.
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

        for cf_name in DJ_CF_NAMES {
            if db.cf_handle(cf_name).is_none() {
                error!(
                    cf = *cf_name,
                    "FAIL FAST: DynamicJEPA column family missing after RocksDB open"
                );
                return Err(TeleologicalStoreError::ColumnFamilyNotFound {
                    name: (*cf_name).to_string(),
                });
            }
        }

        // Create per-embedder index registry (15 HNSW indexes)
        let index_registry = Arc::new(EmbedderIndexRegistry::new());

        // Create E11 HNSW index for causal relationships
        let causal_e11_index = Arc::new(CausalE11Index::new());

        info!(
            "Successfully opened RocksDbTeleologicalStore with {} DynamicJEPA CFs and {} per-embedder indexes",
            DJ_CF_NAMES.len(),
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
                                            id,
                                            value.len()
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
                info!(
                    "Loaded {} persisted soft-delete markers from CF_SYSTEM",
                    map.len()
                );
            }
            Arc::new(map)
        };

        // P1: Count total documents for O(1) IDF lookups in sparse search.
        // This replaces the O(n) full-iterator scan that was the #1 scaling bottleneck.
        // raw_fp_count is also used by verify_consistency() to avoid redundant O(n) scan.
        let (total_doc_count, raw_fp_count) = {
            let cf_fp = db_arc.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
                TeleologicalStoreError::ColumnFamilyNotFound {
                    name: CF_FINGERPRINTS.to_string(),
                }
            })?;
            let raw_count = db_arc
                .iterator_cf(cf_fp, rocksdb::IteratorMode::Start)
                .count();
            // DAT-1: Subtract soft-deleted entries so total_doc_count reflects
            // only live documents. IDF calculations use this count as denominator,
            // and inflating it with soft-deleted entries biases term weights downward.
            let count = raw_count.saturating_sub(soft_deleted.len());
            info!(
                "P1: Initialized total_doc_count = {} from CF_FINGERPRINTS (raw={}, soft_deleted={})",
                count, raw_count, soft_deleted.len()
            );
            (Arc::new(AtomicUsize::new(count)), raw_count)
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
            compaction_lock: RwLock::new(()),
        };

        // Try fast path: load HNSW indexes from CF_HNSW_GRAPHS (persisted graphs).
        // Falls back to O(n) rebuild from CF_FINGERPRINTS if no persisted data or errors.
        match store.try_load_hnsw_indexes() {
            Ok(true) => {
                debug!("HNSW indexes loaded from persisted CF_HNSW_GRAPHS");
                // Check for newly-enabled HNSW indexes that have no persisted data.
                // This happens when embedders are promoted to HNSW (e.g., E2/E3/E4
                // temporal embedders after first-class fusion integration). The fast
                // path loads existing graphs but skips indexes with no CF_HNSW_GRAPHS
                // entry. Detect empty indexes and trigger a full rebuild if needed.
                if raw_fp_count > 0 {
                    use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexOps};
                    let has_empty = store.index_registry.iter().any(|(embedder, index)| {
                        if !E5_CAUSAL_ENABLED
                            && matches!(
                                *embedder,
                                EmbedderIndex::E5Causal
                                    | EmbedderIndex::E5CausalCause
                                    | EmbedderIndex::E5CausalEffect
                            )
                        {
                            return false;
                        }
                        if !E11_ENTITY_ENABLED && *embedder == EmbedderIndex::E11Entity {
                            return false;
                        }
                        index.len() == 0
                    });
                    if has_empty {
                        info!("Detected empty HNSW indexes after fast-path restore — clearing all and rebuilding from fingerprints");
                        // Clear ALL indexes to prevent duplicates from fast-path + rebuild overlap.
                        store.index_registry.clear_all();
                        store.rebuild_indexes_from_store()?;
                    }
                }
            }
            Ok(false) => {
                // No persisted HNSW data — full rebuild from fingerprints
                store.rebuild_indexes_from_store()?;
            }
            Err(e) => {
                warn!(error = %e, "HNSW restore from disk failed — falling back to O(n) rebuild. If this persists, investigate HNSW data corruption.");
                // STOR-3 FIX: Clear partially-loaded indexes before rebuild
                // to prevent duplicate/orphaned vectors from the failed partial load.
                store.index_registry.clear_all();
                store.rebuild_indexes_from_store()?;
            }
        }

        // Rebuild E11 HNSW index from existing causal relationships.
        // Skip when E11 is disabled — KEPLER produces near-identical vectors (0.96-0.98 cosine),
        // so the rebuild is wasted I/O scanning all causal relationships for nothing.
        if E11_ENTITY_ENABLED {
            store.rebuild_causal_e11_index()?;
        }

        // Startup consistency verification — detect post-crash data inconsistencies.
        // Logs warnings only; does NOT block startup.
        // Pass raw_count from P1 initialization to avoid redundant O(n) scan.
        store.verify_consistency(raw_fp_count);

        Ok(store)
    }

    fn assert_existing_dynamicjepa_column_families(path: &Path) -> TeleologicalStoreResult<()> {
        if !path.join("CURRENT").exists() {
            return Ok(());
        }
        let opts = Options::default();
        let existing =
            DB::list_cf(&opts, path).map_err(|err| TeleologicalStoreError::OpenFailed {
                path: path.display().to_string(),
                message: format!("failed to list RocksDB column families before open: {err}"),
            })?;
        let existing = existing.iter().map(String::as_str).collect::<Vec<_>>();
        let missing = DJ_CF_NAMES
            .iter()
            .copied()
            .filter(|name| !existing.contains(name))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(TeleologicalStoreError::OpenFailed {
                path: path.display().to_string(),
                message: format!(
                    "existing RocksDB is missing DynamicJEPA v3 column families: {}; expected {} DynamicJEPA CFs. Rebuild the DB from a v3 bundle instead of relying on create_missing_column_families.",
                    missing.join(", "),
                    DJ_CF_NAMES.len()
                ),
            });
        }
        Ok(())
    }

    /// Verify data consistency across column families and indexes.
    ///
    /// Called at startup after HNSW rebuild. Checks:
    /// 1. CF_FINGERPRINTS count vs CF_CONTENT count
    /// 2. HNSW index sizes vs live fingerprint count
    /// 3. Soft-delete count cross-reference
    ///
    /// Logs warnings for any inconsistencies detected.
    /// Does NOT block startup — operators investigate if warnings appear.
    fn verify_consistency(&self, raw_fp_count: usize) {
        use crate::teleological::indexes::EmbedderIndexOps;

        let start = std::time::Instant::now();

        // raw_fp_count is passed from open_with_config() to avoid redundant O(n) scan.
        let soft_deleted_count = self.soft_deleted.len();
        let live_fp_count = raw_fp_count.saturating_sub(soft_deleted_count);

        // 2. Count entries in CF_CONTENT
        let content_count = match self.get_cf(CF_CONTENT) {
            Ok(cf) => self
                .db
                .iterator_cf(cf, rocksdb::IteratorMode::Start)
                .count(),
            Err(_) => 0,
        };

        // Content count should be <= fingerprint count (not all fingerprints have content stored separately)
        // But content_count should never EXCEED fingerprint count
        if content_count > raw_fp_count {
            warn!(
                "CONSISTENCY WARNING: CF_CONTENT({}) > CF_FINGERPRINTS({}): {} orphaned content entries. \
                 This may indicate incomplete deletion during a previous crash.",
                content_count, raw_fp_count, content_count - raw_fp_count
            );
        }

        // 3. Check HNSW index sizes vs live fingerprint count.
        // E2/E3/E4 HNSW indexes are now populated (temporal first-class fusion).
        // Skip retired/disabled slots so consistency checks reflect active topology.
        for (embedder, index) in self.index_registry.iter() {
            use crate::teleological::indexes::EmbedderIndex;
            if !E5_CAUSAL_ENABLED
                && matches!(
                    *embedder,
                    EmbedderIndex::E5Causal
                        | EmbedderIndex::E5CausalCause
                        | EmbedderIndex::E5CausalEffect
                )
            {
                continue;
            }
            if !E11_ENTITY_ENABLED && *embedder == EmbedderIndex::E11Entity {
                continue;
            }
            let hnsw_count = index.len();
            // Allow 10% tolerance (removed entries are tracked but not physically deleted from usearch)
            let tolerance = (live_fp_count / 10).max(5);
            if hnsw_count.abs_diff(live_fp_count) > tolerance {
                warn!(
                    "CONSISTENCY WARNING: HNSW {:?} has {} entries but {} live fingerprints \
                     (tolerance={}). Mismatch may indicate stale index data.",
                    embedder, hnsw_count, live_fp_count, tolerance
                );
            }
        }

        // 4. Verify total_doc_count matches live fingerprint count
        let cached_doc_count = self.total_doc_count.load(Ordering::Relaxed);
        if cached_doc_count != live_fp_count {
            warn!(
                "CONSISTENCY WARNING: total_doc_count({}) != live_fp_count({}). \
                 Updating total_doc_count to match actual count.",
                cached_doc_count, live_fp_count
            );
            self.total_doc_count.store(live_fp_count, Ordering::SeqCst);
        }

        let elapsed = start.elapsed();
        info!(
            "Startup consistency check complete in {:?}: {} live fingerprints, {} soft-deleted, {} content entries",
            elapsed, live_fp_count, soft_deleted_count, content_count
        );
    }

    /// Inspect RocksDB lock files without removing them.
    fn inspect_rocksdb_lock<P: AsRef<Path>>(path: P) -> TeleologicalStoreResult<()> {
        let lock_path = path.as_ref().join("LOCK");
        let lock_path_str = lock_path.to_string_lossy().to_string();

        if !lock_path.exists() {
            debug!(
                "No LOCK file at '{}' - database not previously opened",
                lock_path_str
            );
            return Ok(());
        }

        info!(
            "LOCK file exists at '{}' - inspecting advisory lock state without removal",
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
                    warn!(
                        "Cannot open LOCK file for inspection; leaving it in place: {}",
                        e
                    );
                    return Ok(());
                }
            };

            let fd = file.as_raw_fd();
            let mut probe = libc::flock {
                l_type: libc::F_WRLCK as libc::c_short,
                l_whence: libc::SEEK_SET as libc::c_short,
                l_start: 0,
                l_len: 0,
                l_pid: 0,
            };
            let result = unsafe { libc::fcntl(fd, libc::F_SETLK, &probe) };

            if result == 0 {
                info!(
                    "LOCK file at '{}' is not currently held; leaving it in place for RocksDB",
                    lock_path_str
                );
                probe.l_type = libc::F_UNLCK as libc::c_short;
                let _ = unsafe { libc::fcntl(fd, libc::F_SETLK, &probe) };
                Ok(())
            } else {
                let errno = std::io::Error::last_os_error();
                if matches!(errno.raw_os_error(), Some(code) if code == libc::EACCES || code == libc::EAGAIN)
                {
                    info!(
                        "LOCK file at '{}' is held by another process; RocksDB open will fail if the owner is still active",
                        lock_path_str
                    );
                } else {
                    warn!(
                        "fcntl lock probe for '{}' failed with unexpected error {}; leaving LOCK in place",
                        lock_path_str, errno
                    );
                }
                Ok(())
            }
        }

        #[cfg(windows)]
        {
            // On Windows, we cannot safely probe for live lock holders here.
            // Leave the file in place and let RocksDB's own lock open path decide.
            warn!(
                "LOCK file at '{}' found on Windows; leaving it in place for RocksDB",
                lock_path_str
            );
            Ok(())
        }

        #[cfg(not(any(unix, windows)))]
        {
            warn!(
                "LOCK file at '{}' found on unknown OS; leaving it in place for RocksDB",
                lock_path_str
            );
            Ok(())
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
        use context_graph_embeddings::models::custom::compute_decay_embedding;
        use context_graph_embeddings::models::custom::DEFAULT_DECAY_RATES;

        // DATA-5 FIX: Acquire write lock — blocks all concurrent store/delete
        // until rebuild is complete. Prevents duplicate/missing entries.
        let _guard = self.compaction_lock.write();

        let start = std::time::Instant::now();

        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut success_count = 0;
        let mut error_count = 0;
        let mut e2_recomputed = 0;

        for item in iter {
            let (key, value) = item.map_err(|e| {
                error!(
                    "FAIL FAST: RocksDB iteration failed during index rebuild: {}",
                    e
                );
                TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
            })?;

            // Parse fingerprint ID from key
            let id = parse_fingerprint_key(&key);

            // Skip soft-deleted fingerprints
            if self.is_soft_deleted(&id) {
                continue;
            }

            // Deserialize fingerprint - skip corrupted records
            let mut fp = match deserialize_teleological_fingerprint(&value) {
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

            // Recompute E2 vector from created_at timestamp using sinusoidal encoding.
            // Stored E2 vectors from before the sinusoidal fix are degenerate (all identical,
            // computed with delta≈0 decay). Recomputing ensures the E2 HNSW index contains
            // distinct vectors that reflect actual creation timestamps.
            // This is a fast CPU computation (~1μs per fingerprint).
            let recomputed_e2 = compute_decay_embedding(fp.created_at, None, &DEFAULT_DECAY_RATES);
            fp.semantic.e2_temporal_recent = recomputed_e2;
            e2_recomputed += 1;

            // Add to HNSW indexes — use unlocked variant since we hold write lock
            match self.add_to_indexes_unlocked(&fp) {
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
                error_count,
                success_count + error_count,
                elapsed
            );
            return Err(TeleologicalStoreError::Internal(format!(
                "Index rebuild failed: {} fingerprints could not be added to indexes",
                error_count
            )));
        }

        if success_count > 0 {
            info!(
                "Rebuilt HNSW indexes: {} fingerprints added to {} indexes in {:?} ({} E2 vectors recomputed)",
                success_count,
                self.index_registry.len(),
                elapsed,
                e2_recomputed
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

        let cf = self.db.cf_handle(CF_CAUSAL_RELATIONSHIPS).ok_or_else(|| {
            TeleologicalStoreError::ColumnFamilyNotFound {
                name: CF_CAUSAL_RELATIONSHIPS.to_string(),
            }
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
            debug!(
                "No causal relationships with E11 embeddings to rebuild (indexed: 0, skipped: {})",
                skip_count
            );
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
        // DATA-5 FIX: Race condition eliminated by compaction_lock in rebuild_indexes_from_store.
        // rebuild acquires write lock; concurrent store/delete blocked until complete.
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

        // STOR-4 FIX: Also check CausalE11Index for compaction (skip when E11 disabled)
        if E11_ENTITY_ENABLED && self.causal_e11_index.needs_compaction() {
            let removed = self.causal_e11_index.removed_count();
            let total = self.causal_e11_index.len();
            info!(
                "HNSW compaction needed for CausalE11Index: {removed} orphaned / {total} total ({:.0}%)",
                if total > 0 { (removed as f64 / total as f64) * 100.0 } else { 0.0 }
            );
            any_needs_compaction = true;
        }

        if any_needs_compaction {
            info!(
                "HNSW compaction starting — write lock will block concurrent store/delete \
                 until rebuild completes (DATA-5 fix)"
            );
            info!("H1 FIX: Rebuilding all HNSW indexes from CF_FINGERPRINTS to eliminate orphaned vectors");
            self.rebuild_indexes_from_store()?;
            // Reset all removed counts after successful rebuild
            for (_embedder, index) in self.index_registry.iter() {
                index.reset_removed_count();
            }

            // STOR-4 FIX: Also rebuild and reset CausalE11Index (skip when E11 disabled)
            if E11_ENTITY_ENABLED {
                self.causal_e11_index.clear();
                self.rebuild_causal_e11_index()?;
                self.causal_e11_index.reset_removed_count();
            }

            info!("HNSW compaction complete — all orphaned vectors eliminated (including CausalE11Index)");
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
            // Skip E2 persisted HNSW — E2 vectors are recomputed from created_at
            // timestamps during rebuild to use sinusoidal encoding. Persisted E2
            // graphs may contain degenerate vectors from the old decay encoding.
            // NOTE: This forces a full O(n) rebuild on every startup (the empty E2
            // triggers the has_empty detection path). Once all deployments have run
            // the sinusoidal rebuild at least once, this skip can be removed.
            if *embedder == crate::teleological::indexes::EmbedderIndex::E2TemporalRecent {
                debug!(
                    "Skipping persisted E2 HNSW — will recompute from timestamps during rebuild"
                );
                continue;
            }

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
    ///
    /// HIGH-6 FIX: `count_as_new` controls whether total_doc_count is incremented.
    ///   - `true` for new inserts (store_async, store_batch_async)
    ///   - `false` for updates and rollbacks (update_async rollback)
    pub(crate) fn store_fingerprint_internal(
        &self,
        fp: &TeleologicalFingerprint,
        count_as_new: bool,
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

        // 2. Topic profiles are NOT computed at store time. They are computed asynchronously
        // by the topic detection/clustering system (detect_topics, get_topic_portfolio) and
        // stored separately. CF_TOPIC_PROFILES is written by the clustering pipeline, not here.
        // The hard-delete path in crud.rs still deletes from CF_TOPIC_PROFILES defensively
        // to clean up any profiles that were computed for a fingerprint being deleted.

        // 3. Store E1 Matryoshka 128D truncated vector
        // Note: CF_E1_MATRYOSHKA_128 is written for recovery/rebuild purposes.
        // Pipeline Stage 2 reads from the in-memory HNSW index, not this CF directly.
        // This CF serves as durable backup for index reconstruction.
        let cf_matryoshka = self.get_cf(CF_E1_MATRYOSHKA_128)?;
        let matryoshka_key = e1_matryoshka_128_key(&id);
        let mut truncated = [0.0f32; 128];
        let e1 = &fp.semantic.e1_semantic;
        assert!(
            e1.len() >= 128,
            "[E_STOR_MATRYOSHKA_001] E1 semantic vector has {} dims, need >= 128 for Matryoshka truncation. \
             This indicates a corrupted or incomplete fingerprint.",
            e1.len()
        );
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

        // Invalidate count cache; only increment doc count for genuinely new documents
        *self.fingerprint_count.write() = None;
        if count_as_new {
            self.total_doc_count.fetch_add(1, Ordering::Relaxed);
        }

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

    /// Health check: verify ALL column families are accessible.
    pub fn health_check(&self) -> TeleologicalStoreResult<()> {
        let all_cf_arrays: &[&[&str]] = &[
            cf_names::ALL,
            TELEOLOGICAL_CFS,
            QUANTIZED_EMBEDDER_CFS,
            CODE_CFS,
            CAUSAL_CFS,
            DJ_CF_NAMES,
        ];

        for cf_names_arr in all_cf_arrays {
            for cf_name in *cf_names_arr {
                self.get_cf(cf_name)?;
            }
        }
        Ok(())
    }
}
