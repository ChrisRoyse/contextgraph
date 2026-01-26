//! Core RocksDbTeleologicalStore struct and constructor.
//!
//! This module contains the main store struct, its constructor methods,
//! and basic accessor methods. Complex operations are in separate modules:
//! - `index_ops.rs` - HNSW index operations
//! - `inverted_index.rs` - SPLADE inverted index operations

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use bincode;
use rocksdb::{Cache, ColumnFamily, Options, WriteBatch, DB};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::types::fingerprint::TeleologicalFingerprint;

use crate::teleological::column_families::{
    get_all_teleological_cf_descriptors, CF_CONTENT, CF_E12_LATE_INTERACTION, CF_E1_MATRYOSHKA_128,
    CF_FINGERPRINTS, CF_SOURCE_METADATA, QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS,
};
use crate::teleological::indexes::EmbedderIndexRegistry;
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
    pub(crate) db: Arc<DB>,
    /// Shared block cache across column families.
    #[allow(dead_code)]
    pub(crate) cache: Cache,
    /// Database path.
    pub(crate) path: PathBuf,
    /// In-memory count of fingerprints (cached for performance).
    pub(crate) fingerprint_count: RwLock<Option<usize>>,
    /// Soft-deleted IDs (tracked in memory for filtering).
    pub(crate) soft_deleted: RwLock<HashMap<Uuid, bool>>,
    /// Per-embedder index registry with 12 HNSW indexes for O(log n) ANN search.
    /// E6, E12, E13 use different index types (inverted/MaxSim).
    /// NO FALLBACKS - FAIL FAST on invalid operations.
    pub(crate) index_registry: Arc<EmbedderIndexRegistry>,
}

// ============================================================================
// Constructor and Open Methods
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Open a teleological store at the specified path with default configuration.
    ///
    /// Creates the database and all 17 column families if they don't exist.
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

        // Get all 17 teleological column family descriptors
        let cf_descriptors = get_all_teleological_cf_descriptors(&cache);

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

        info!(
            "Successfully opened RocksDbTeleologicalStore with {} column families and {} per-embedder indexes",
            TELEOLOGICAL_CFS.len() + QUANTIZED_EMBEDDER_CFS.len(),
            index_registry.len()
        );

        let store = Self {
            db: Arc::new(db),
            cache,
            path: path_buf,
            fingerprint_count: RwLock::new(None),
            soft_deleted: RwLock::new(HashMap::new()),
            index_registry,
        };

        // CRITICAL: Rebuild HNSW indexes from existing RocksDB fingerprints
        // Without this, indexes are empty on every restart and multi-space search fails!
        store.rebuild_indexes_from_store()?;

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
            Self::try_remove_lock_file(&lock_path, &lock_path_str)
        }

        #[cfg(not(any(unix, windows)))]
        {
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

    /// Detect database corruption by verifying MANIFEST references only existing SST files.
    ///
    /// NOTE: This function is currently unused. We rely on RocksDB's own
    /// corruption detection and transform its errors via `transform_corruption_error`.
    /// Keeping this for potential future use or manual corruption checks.
    #[allow(dead_code)]
    fn detect_database_corruption<P: AsRef<Path>>(path: P) -> TeleologicalStoreResult<()> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy().to_string();

        // Step 1: Read CURRENT file to get MANIFEST filename
        let current_path = path_ref.join("CURRENT");
        if !current_path.exists() {
            debug!(
                "No CURRENT file at '{}' - new database, skipping corruption check",
                path_str
            );
            return Ok(());
        }

        let manifest_name = match fs::read_to_string(&current_path) {
            Ok(content) => content.trim().to_string(),
            Err(e) => {
                warn!(
                    "Cannot read CURRENT file at '{}': {} - skipping corruption check",
                    path_str, e
                );
                return Ok(());
            }
        };

        let manifest_path = path_ref.join(&manifest_name);
        if !manifest_path.exists() {
            error!(
                "FAIL FAST: CURRENT file references '{}' but MANIFEST does not exist at '{}'",
                manifest_name,
                manifest_path.display()
            );
            return Err(TeleologicalStoreError::CorruptionDetected {
                path: path_str,
                missing_count: 1,
                missing_files: manifest_name.clone(),
                manifest_file: manifest_name,
            });
        }

        info!("Checking database integrity: MANIFEST={}", manifest_name);

        // Step 2: Scan MANIFEST for SST file references
        // RocksDB stores file numbers in the MANIFEST. We look for patterns that
        // indicate SST file numbers (6-digit numbers that would form .sst files)
        let manifest_bytes = match fs::read(&manifest_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!(
                    "FAIL FAST: Cannot read MANIFEST '{}': {}",
                    manifest_path.display(),
                    e
                );
                return Err(TeleologicalStoreError::OpenFailed {
                    path: path_str,
                    message: format!("Cannot read MANIFEST '{}': {}", manifest_name, e),
                });
            }
        };

        // Extract potential SST file numbers from MANIFEST
        // RocksDB's VersionEdit uses variable-length encoding, but SST file numbers
        // often appear as recognizable patterns. We scan for likely file numbers.
        let referenced_sst_files = Self::extract_sst_references_from_manifest(&manifest_bytes);

        // Step 3: Get existing SST files
        let existing_sst_files: std::collections::HashSet<u64> = match fs::read_dir(path_ref) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.ends_with(".sst") {
                        // Extract number from "000XXX.sst" format
                        name.strip_suffix(".sst")
                            .and_then(|s| s.parse::<u64>().ok())
                    } else {
                        None
                    }
                })
                .collect(),
            Err(e) => {
                warn!("Cannot enumerate SST files at '{}': {}", path_str, e);
                return Ok(()); // Can't verify, let RocksDB handle it
            }
        };

        debug!(
            "Found {} existing SST files, MANIFEST references {} potential files",
            existing_sst_files.len(),
            referenced_sst_files.len()
        );

        // Step 4: Find missing SST files with smart filtering
        //
        // The varint heuristic finds many false positives (field tags, sizes, etc.).
        // To avoid false positives, we only flag files as missing if:
        // 1. There are existing SST files (so we have a reference point)
        // 2. The missing file number is ABOVE max_existing but within a reasonable buffer
        //
        // This catches the REAL corruption case:
        // - MANIFEST references file 682
        // - But only files up to 649 exist
        // - Difference: file 682 was being written when shutdown occurred
        //
        // This AVOIDS false positives:
        // - MANIFEST contains many varint numbers (field tags, sizes, etc.)
        // - These might decode to numbers like 14, 16, 18 within existing file range
        // - But they're not real missing files
        //
        // If NO existing files, let RocksDB handle it - can't reliably detect corruption.
        if existing_sst_files.is_empty() {
            debug!("No existing SST files - skipping corruption check (new or empty DB)");
            return Ok(());
        }

        let max_existing = *existing_sst_files.iter().max().unwrap_or(&0);
        // Only flag files ABOVE max_existing but within reasonable buffer
        // This catches: referenced file 682 when max existing is 649
        // Buffer of 100 allows for files that might have been legitimately deleted
        let upper_bound = max_existing + 100;

        let missing_files: Vec<u64> = referenced_sst_files
            .iter()
            .filter(|num| {
                // Only consider files ABOVE max existing (corruption from incomplete writes)
                // and within a reasonable upper bound (to filter out very high false positives)
                **num > max_existing && **num <= upper_bound &&
                // And not already existing (sanity check)
                !existing_sst_files.contains(num)
            })
            .cloned()
            .collect();

        if !missing_files.is_empty() {
            let missing_file_names: Vec<String> = missing_files
                .iter()
                .map(|num| format!("{:06}.sst", num))
                .collect();

            error!(
                "FAIL FAST: CORRUPTION DETECTED at '{}' - MANIFEST '{}' references {} missing SST files: [{}]",
                path_str,
                manifest_name,
                missing_files.len(),
                missing_file_names.join(", ")
            );
            error!(
                "Recovery options: (1) Delete database and restore from backup, \
                 (2) Use 'ldb repair' tool (may lose data), \
                 (3) Restore from snapshot"
            );

            return Err(TeleologicalStoreError::CorruptionDetected {
                path: path_str,
                missing_count: missing_files.len(),
                missing_files: missing_file_names.join(", "),
                manifest_file: manifest_name,
            });
        }

        info!(
            "Database integrity check passed: {} SST files verified",
            existing_sst_files.len()
        );
        Ok(())
    }

    /// Extract SST file number references from MANIFEST binary data.
    ///
    /// RocksDB's MANIFEST uses a log-structured format with VersionEdit records.
    /// Each record contains file additions/deletions encoded as varint file numbers.
    ///
    /// This function uses a targeted approach focusing on varint-encoded numbers,
    /// which is how RocksDB actually stores file numbers in the MANIFEST.
    ///
    /// Note: Currently unused - we rely on RocksDB's own corruption detection.
    #[allow(dead_code)]
    fn extract_sst_references_from_manifest(
        manifest_bytes: &[u8],
    ) -> std::collections::HashSet<u64> {
        let mut referenced_files = std::collections::HashSet::new();

        // RocksDB uses protobuf-style variable-length integer encoding (varint).
        // File numbers are encoded as varints in VersionEdit records.
        // We scan for varint-encoded numbers in the reasonable file number range.

        let mut i = 0;
        while i < manifest_bytes.len() {
            if let Some((num, bytes_read)) = Self::decode_varint(&manifest_bytes[i..]) {
                // Filter to plausible SST file numbers:
                // - Greater than 0 (file numbers start at 1)
                // - Less than 100,000 (reasonable upper bound for most databases)
                //   Most databases won't have more than 100k SST files
                // - Varint encoding means small numbers use fewer bytes,
                //   so this catches the most common patterns
                if num > 0 && num < 100_000 {
                    referenced_files.insert(num);
                }
                // Move forward by at least 1 byte to avoid infinite loops
                i += bytes_read.max(1);
            } else {
                i += 1;
            }
        }

        referenced_files
    }

    /// Decode a variable-length integer (varint) from bytes.
    ///
    /// Returns the decoded number and number of bytes consumed.
    /// Note: Currently unused - we rely on RocksDB's own corruption detection.
    #[allow(dead_code)]
    fn decode_varint(bytes: &[u8]) -> Option<(u64, usize)> {
        if bytes.is_empty() {
            return None;
        }

        let mut result: u64 = 0;
        let mut shift = 0;

        for (i, &byte) in bytes.iter().enumerate() {
            if i >= 10 {
                // Varint too long (max 10 bytes for u64)
                return None;
            }

            result |= ((byte & 0x7F) as u64) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                // End of varint
                return Some((result, i + 1));
            }
        }

        None // Incomplete varint
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

    /// Get the e12_late_interaction column family handle (FAIL FAST on missing).
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn cf_e12_late_interaction(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_E12_LATE_INTERACTION)
            .expect("CF_E12_LATE_INTERACTION must exist - database initialization failed")
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

            // Deserialize fingerprint
            let fp = deserialize_teleological_fingerprint(&value);

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

    /// Retrieve raw fingerprint bytes from RocksDB.
    pub(crate) fn get_fingerprint_raw(&self, id: Uuid) -> TeleologicalStoreResult<Option<Vec<u8>>> {
        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let key = fingerprint_key(&id);

        self.db
            .get_cf(cf, key)
            .map_err(|e| TeleologicalStoreError::rocksdb_op("get", CF_FINGERPRINTS, Some(id), e))
    }

    /// Check if an ID is soft-deleted.
    pub(crate) fn is_soft_deleted(&self, id: &Uuid) -> bool {
        if let Ok(deleted) = self.soft_deleted.read() {
            deleted.get(id).copied().unwrap_or(false)
        } else {
            false
        }
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
