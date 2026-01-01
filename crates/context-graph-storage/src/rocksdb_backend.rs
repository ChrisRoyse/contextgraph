//! RocksDB storage backend implementation.
//!
//! Provides persistent storage using RocksDB with column families
//! for Johari quadrant separation and efficient indexing.
//!
//! # Performance Targets (constitution.yaml)
//! - inject_context: p95 < 25ms, p99 < 50ms
//! - hopfield: < 1ms
//!
//! # Column Families
//! Uses 12 CFs defined in `column_families.rs`:
//! - nodes, edges, embeddings, metadata
//! - johari_open, johari_hidden, johari_blind, johari_unknown
//! - temporal, tags, sources, system

use rocksdb::{Cache, ColumnFamily, Options, DB};
use std::path::Path;
use thiserror::Error;

use crate::column_families::{cf_names, get_column_family_descriptors};

/// Default block cache size: 256MB (per constitution.yaml).
pub const DEFAULT_CACHE_SIZE: usize = 256 * 1024 * 1024;

/// Default maximum open files.
pub const DEFAULT_MAX_OPEN_FILES: i32 = 1000;

/// Storage operation errors.
///
/// These errors cover database lifecycle operations.
/// For serialization errors, see `SerializationError`.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database failed to open.
    #[error("Failed to open database at '{path}': {message}")]
    OpenFailed { path: String, message: String },

    /// Column family not found (should never happen if DB opened correctly).
    #[error("Column family '{name}' not found")]
    ColumnFamilyNotFound { name: String },

    /// Write operation failed.
    #[error("Write failed: {0}")]
    WriteFailed(String),

    /// Read operation failed.
    #[error("Read failed: {0}")]
    ReadFailed(String),

    /// Flush operation failed.
    #[error("Flush failed: {0}")]
    FlushFailed(String),
}

/// Configuration options for RocksDbMemex.
///
/// # Defaults
/// - `max_open_files`: 1000
/// - `block_cache_size`: 256MB (268,435,456 bytes)
/// - `enable_wal`: true (durability)
/// - `create_if_missing`: true (convenience)
#[derive(Debug, Clone)]
pub struct RocksDbConfig {
    /// Maximum open files (default: 1000).
    pub max_open_files: i32,
    /// Block cache size in bytes (default: 256MB).
    pub block_cache_size: usize,
    /// Enable Write-Ahead Logging (default: true).
    pub enable_wal: bool,
    /// Create database if missing (default: true).
    pub create_if_missing: bool,
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            max_open_files: DEFAULT_MAX_OPEN_FILES,
            block_cache_size: DEFAULT_CACHE_SIZE,
            enable_wal: true,
            create_if_missing: true,
        }
    }
}

/// RocksDB-backed storage implementation.
///
/// Provides persistent storage for MemoryNodes and GraphEdges with
/// optimized column families for different access patterns.
///
/// # Thread Safety
/// RocksDB's `DB` type is internally thread-safe for concurrent reads and writes.
/// This struct can be shared across threads via `Arc<RocksDbMemex>`.
///
/// # Column Families
/// Opens all 12 column families defined in `column_families.rs`.
///
/// # Example
/// ```rust,ignore
/// use context_graph_storage::rocksdb_backend::{RocksDbMemex, RocksDbConfig};
/// use tempfile::TempDir;
///
/// let tmp = TempDir::new().unwrap();
/// let db = RocksDbMemex::open(tmp.path()).expect("open failed");
/// assert!(db.health_check().is_ok());
/// ```
pub struct RocksDbMemex {
    /// The RocksDB database instance.
    db: DB,
    /// Shared block cache (kept alive for DB lifetime).
    #[allow(dead_code)]
    cache: Cache,
    /// Database path for reference.
    path: String,
}

impl RocksDbMemex {
    /// Open a RocksDB database at the specified path with default configuration.
    ///
    /// Creates the database and all 12 column families if they don't exist.
    ///
    /// # Arguments
    /// * `path` - Path to the database directory
    ///
    /// # Returns
    /// * `Ok(RocksDbMemex)` - Successfully opened database
    /// * `Err(StorageError::OpenFailed)` - Database could not be opened
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    /// use tempfile::TempDir;
    ///
    /// let tmp = TempDir::new().unwrap();
    /// let db = RocksDbMemex::open(tmp.path()).expect("open failed");
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        Self::open_with_config(path, RocksDbConfig::default())
    }

    /// Open a RocksDB database with custom configuration.
    ///
    /// # Arguments
    /// * `path` - Path to the database directory
    /// * `config` - Custom configuration options
    ///
    /// # Returns
    /// * `Ok(RocksDbMemex)` - Successfully opened database
    /// * `Err(StorageError::OpenFailed)` - Database could not be opened
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        config: RocksDbConfig,
    ) -> Result<Self, StorageError> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Create shared block cache
        let cache = Cache::new_lru_cache(config.block_cache_size);

        // Create DB options
        let mut db_opts = Options::default();
        db_opts.create_if_missing(config.create_if_missing);
        db_opts.create_missing_column_families(true);
        db_opts.set_max_open_files(config.max_open_files);

        // WAL configuration
        if !config.enable_wal {
            db_opts.set_manual_wal_flush(true);
        }

        // Get column family descriptors with optimized options
        let cf_descriptors = get_column_family_descriptors(&cache);

        // Open database with all column families
        let db = DB::open_cf_descriptors(&db_opts, &path_str, cf_descriptors).map_err(|e| {
            StorageError::OpenFailed {
                path: path_str.clone(),
                message: e.to_string(),
            }
        })?;

        Ok(Self {
            db,
            cache,
            path: path_str,
        })
    }

    /// Get a reference to a column family by name.
    ///
    /// # Arguments
    /// * `name` - Column family name (use `cf_names::*` constants)
    ///
    /// # Returns
    /// * `Ok(&ColumnFamily)` - Reference to the column family
    /// * `Err(StorageError::ColumnFamilyNotFound)` - CF doesn't exist
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    /// use context_graph_storage::column_families::cf_names;
    ///
    /// let cf = db.get_cf(cf_names::NODES)?;
    /// ```
    pub fn get_cf(&self, name: &str) -> Result<&ColumnFamily, StorageError> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound {
                name: name.to_string(),
            })
    }

    /// Get the database path.
    ///
    /// # Returns
    /// The path where the database is stored.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Check if the database is healthy.
    ///
    /// Verifies all 12 column families are accessible.
    ///
    /// # Returns
    /// * `Ok(())` - All CFs accessible
    /// * `Err(StorageError::ColumnFamilyNotFound)` - A CF is missing
    pub fn health_check(&self) -> Result<(), StorageError> {
        for cf_name in cf_names::ALL {
            self.get_cf(cf_name)?;
        }
        Ok(())
    }

    /// Flush all column families to disk.
    ///
    /// Forces all buffered writes to be persisted.
    ///
    /// # Returns
    /// * `Ok(())` - All CFs flushed successfully
    /// * `Err(StorageError::FlushFailed)` - Flush operation failed
    pub fn flush_all(&self) -> Result<(), StorageError> {
        for cf_name in cf_names::ALL {
            let cf = self.get_cf(cf_name)?;
            self.db
                .flush_cf(cf)
                .map_err(|e| StorageError::FlushFailed(e.to_string()))?;
        }
        Ok(())
    }

    /// Get a reference to the underlying RocksDB instance.
    ///
    /// Use this for advanced operations not covered by the high-level API.
    /// Be careful not to violate data invariants.
    pub fn db(&self) -> &DB {
        &self.db
    }
}

// DB is automatically closed when RocksDbMemex is dropped (RocksDB's Drop impl)

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Helper Functions
    // =========================================================================

    fn create_temp_db() -> (TempDir, RocksDbMemex) {
        let tmp = TempDir::new().expect("Failed to create temp dir");
        let db = RocksDbMemex::open(tmp.path()).expect("Failed to open database");
        (tmp, db)
    }

    // =========================================================================
    // RocksDbConfig Tests
    // =========================================================================

    #[test]
    fn test_config_default_values() {
        let config = RocksDbConfig::default();
        assert_eq!(config.max_open_files, 1000);
        assert_eq!(config.block_cache_size, 256 * 1024 * 1024);
        assert!(config.enable_wal);
        assert!(config.create_if_missing);
    }

    #[test]
    fn test_config_custom_values() {
        let config = RocksDbConfig {
            max_open_files: 500,
            block_cache_size: 128 * 1024 * 1024,
            enable_wal: false,
            create_if_missing: false,
        };
        assert_eq!(config.max_open_files, 500);
        assert_eq!(config.block_cache_size, 128 * 1024 * 1024);
        assert!(!config.enable_wal);
        assert!(!config.create_if_missing);
    }

    #[test]
    fn test_config_clone() {
        let config = RocksDbConfig::default();
        let cloned = config.clone();
        assert_eq!(config.max_open_files, cloned.max_open_files);
    }

    #[test]
    fn test_config_debug() {
        let config = RocksDbConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("RocksDbConfig"));
        assert!(debug.contains("max_open_files"));
    }

    // =========================================================================
    // StorageError Tests
    // =========================================================================

    #[test]
    fn test_error_open_failed() {
        let error = StorageError::OpenFailed {
            path: "/tmp/test".to_string(),
            message: "permission denied".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("/tmp/test"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn test_error_column_family_not_found() {
        let error = StorageError::ColumnFamilyNotFound {
            name: "unknown_cf".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("unknown_cf"));
    }

    #[test]
    fn test_error_write_failed() {
        let error = StorageError::WriteFailed("disk full".to_string());
        assert!(error.to_string().contains("disk full"));
    }

    #[test]
    fn test_error_read_failed() {
        let error = StorageError::ReadFailed("io error".to_string());
        assert!(error.to_string().contains("io error"));
    }

    #[test]
    fn test_error_flush_failed() {
        let error = StorageError::FlushFailed("sync failed".to_string());
        assert!(error.to_string().contains("sync failed"));
    }

    #[test]
    fn test_error_debug() {
        let error = StorageError::WriteFailed("test".to_string());
        let debug = format!("{:?}", error);
        assert!(debug.contains("WriteFailed"));
    }

    // =========================================================================
    // Database Open/Close Tests
    // =========================================================================

    #[test]
    fn test_open_creates_database() {
        println!("=== TEST: open() creates database ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path();

        println!("BEFORE: Database path = {:?}", path);
        println!("BEFORE: Path exists = {}", path.exists());

        let db = RocksDbMemex::open(path).expect("open failed");

        println!("AFTER: Database opened successfully");
        println!("AFTER: db.path() = {}", db.path());

        assert!(path.exists(), "Database directory should exist");
        assert_eq!(db.path(), path.to_string_lossy());
    }

    #[test]
    fn test_open_with_custom_config() {
        println!("=== TEST: open_with_config() custom settings ===");
        let tmp = TempDir::new().expect("create temp dir");

        let config = RocksDbConfig {
            max_open_files: 100,
            block_cache_size: 64 * 1024 * 1024, // 64MB
            enable_wal: true,
            create_if_missing: true,
        };

        println!("BEFORE: Custom config = {:?}", config);

        let db = RocksDbMemex::open_with_config(tmp.path(), config).expect("open failed");

        println!("AFTER: Database opened with custom config");
        assert!(db.health_check().is_ok());
    }

    #[test]
    fn test_open_invalid_path_fails() {
        // Try to open in a non-existent path without create_if_missing
        let config = RocksDbConfig {
            create_if_missing: false,
            ..Default::default()
        };

        let result = RocksDbMemex::open_with_config("/nonexistent/path/db", config);
        assert!(result.is_err());

        if let Err(StorageError::OpenFailed { path, message }) = result {
            assert!(path.contains("nonexistent"));
            assert!(!message.is_empty());
        }
    }

    // =========================================================================
    // Column Family Tests
    // =========================================================================

    #[test]
    fn test_get_cf_returns_valid_handle() {
        let (_tmp, db) = create_temp_db();

        for cf_name in cf_names::ALL {
            let cf = db.get_cf(cf_name);
            assert!(cf.is_ok(), "CF '{}' should exist", cf_name);
        }
    }

    #[test]
    fn test_get_cf_unknown_returns_error() {
        let (_tmp, db) = create_temp_db();

        let result = db.get_cf("nonexistent_cf");
        assert!(result.is_err());

        if let Err(StorageError::ColumnFamilyNotFound { name }) = result {
            assert_eq!(name, "nonexistent_cf");
        } else {
            panic!("Expected ColumnFamilyNotFound error");
        }
    }

    #[test]
    fn test_all_12_cfs_accessible() {
        println!("=== TEST: All 12 column families accessible ===");
        let (_tmp, db) = create_temp_db();

        println!("BEFORE: Checking {} column families", cf_names::ALL.len());

        for (i, cf_name) in cf_names::ALL.iter().enumerate() {
            let cf = db
                .get_cf(cf_name)
                .unwrap_or_else(|_| panic!("CF {} should exist", cf_name));
            println!("  CF {}: '{}' -> handle obtained", i, cf_name);
            // CF handle is valid (non-null pointer internally)
            let _ = cf;
        }

        println!("AFTER: All 12 CFs verified accessible");
    }

    // =========================================================================
    // Health Check Tests
    // =========================================================================

    #[test]
    fn test_health_check_passes() {
        let (_tmp, db) = create_temp_db();
        let result = db.health_check();
        assert!(result.is_ok(), "Health check should pass: {:?}", result);
    }

    #[test]
    fn test_health_check_verifies_all_cfs() {
        println!("=== TEST: health_check verifies all CFs ===");
        let (_tmp, db) = create_temp_db();

        println!("BEFORE: Running health check");
        let result = db.health_check();
        println!("AFTER: Health check result = {:?}", result);

        assert!(result.is_ok());
    }

    // =========================================================================
    // Flush Tests
    // =========================================================================

    #[test]
    fn test_flush_all_succeeds() {
        let (_tmp, db) = create_temp_db();
        let result = db.flush_all();
        assert!(result.is_ok(), "Flush should succeed: {:?}", result);
    }

    #[test]
    fn test_flush_all_on_empty_db() {
        println!("=== TEST: flush_all on empty database ===");
        let (_tmp, db) = create_temp_db();

        println!("BEFORE: Flushing empty database");
        let result = db.flush_all();
        println!("AFTER: Flush result = {:?}", result);

        assert!(result.is_ok());
    }

    // =========================================================================
    // Reopen Tests
    // =========================================================================

    #[test]
    fn test_reopen_preserves_cfs() {
        println!("=== TEST: Reopen preserves column families ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path().to_path_buf();

        // Open first time
        {
            println!("BEFORE: Opening database first time");
            let db = RocksDbMemex::open(&path).expect("first open failed");
            assert!(db.health_check().is_ok());
            println!("AFTER: First open successful, dropping database");
        } // db dropped here

        // Reopen
        {
            println!("BEFORE: Reopening database");
            let db = RocksDbMemex::open(&path).expect("reopen failed");
            println!("AFTER: Reopen successful");

            // Verify all CFs still exist
            for cf_name in cf_names::ALL {
                let cf = db.get_cf(cf_name);
                assert!(cf.is_ok(), "CF '{}' should exist after reopen", cf_name);
            }
            println!("RESULT: All 12 CFs preserved after reopen");
        }
    }

    // =========================================================================
    // Edge Case Tests (REQUIRED - print before/after state)
    // =========================================================================

    #[test]
    fn edge_case_multiple_opens_same_path_fails() {
        println!("=== EDGE CASE 1: Multiple opens on same path ===");
        let tmp = TempDir::new().expect("create temp dir");

        let db1 = RocksDbMemex::open(tmp.path()).expect("first open");
        println!("BEFORE: First database opened at {:?}", tmp.path());

        // Second open should fail (RocksDB lock)
        let result = RocksDbMemex::open(tmp.path());
        println!("AFTER: Second open attempt result = {:?}", result.is_err());

        assert!(result.is_err(), "Second open should fail due to lock");
        drop(db1);
        println!("RESULT: PASS - RocksDB prevents concurrent opens");
    }

    #[test]
    fn edge_case_minimum_cache_size() {
        println!("=== EDGE CASE 2: Minimum cache size (1MB) ===");
        let tmp = TempDir::new().expect("create temp dir");

        let config = RocksDbConfig {
            block_cache_size: 1024 * 1024, // 1MB
            ..Default::default()
        };

        println!("BEFORE: Opening with 1MB cache");
        let db = RocksDbMemex::open_with_config(tmp.path(), config).expect("open failed");
        println!("AFTER: Database opened with minimal cache");

        assert!(db.health_check().is_ok());
        println!("RESULT: PASS - Works with minimum cache");
    }

    #[test]
    fn edge_case_path_with_spaces() {
        println!("=== EDGE CASE 3: Path with spaces ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path_with_spaces = tmp.path().join("path with spaces");
        std::fs::create_dir_all(&path_with_spaces).expect("create dir");

        println!(
            "BEFORE: Opening at path with spaces: {:?}",
            path_with_spaces
        );
        let db = RocksDbMemex::open(&path_with_spaces).expect("open failed");
        println!("AFTER: Database opened successfully");

        assert!(db.health_check().is_ok());
        assert!(db.path().contains("path with spaces"));
        println!("RESULT: PASS - Path with spaces handled correctly");
    }

    // =========================================================================
    // db() accessor test
    // =========================================================================

    #[test]
    fn test_db_accessor() {
        let (_tmp, db) = create_temp_db();
        let raw_db = db.db();
        // Just verify we can access properties
        let path = raw_db.path();
        assert!(!path.to_string_lossy().is_empty());
    }

    // =========================================================================
    // Path accessor test
    // =========================================================================

    #[test]
    fn test_path_accessor() {
        let tmp = TempDir::new().expect("create temp dir");
        let expected_path = tmp.path().to_string_lossy().to_string();
        let db = RocksDbMemex::open(tmp.path()).expect("open failed");
        assert_eq!(db.path(), expected_path);
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_default_cache_size_constant() {
        assert_eq!(DEFAULT_CACHE_SIZE, 256 * 1024 * 1024);
        assert_eq!(DEFAULT_CACHE_SIZE, 268_435_456); // 256MB in bytes
    }

    #[test]
    fn test_default_max_open_files_constant() {
        assert_eq!(DEFAULT_MAX_OPEN_FILES, 1000);
    }

    // =========================================================================
    // Integration-style Tests
    // =========================================================================

    #[test]
    fn test_full_lifecycle() {
        println!("=== TEST: Full database lifecycle ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path().to_path_buf();

        // 1. Open
        println!("STEP 1: Opening database");
        let db = RocksDbMemex::open(&path).expect("open failed");

        // 2. Health check
        println!("STEP 2: Health check");
        assert!(db.health_check().is_ok());

        // 3. Access all CFs
        println!("STEP 3: Access all 12 CFs");
        for cf_name in cf_names::ALL {
            let _ = db.get_cf(cf_name).expect("CF should exist");
        }

        // 4. Flush
        println!("STEP 4: Flush all CFs");
        assert!(db.flush_all().is_ok());

        // 5. Verify path
        println!("STEP 5: Verify path");
        assert_eq!(db.path(), path.to_string_lossy());

        // 6. Get raw DB reference
        println!("STEP 6: Get raw DB reference");
        let _ = db.db();

        // 7. Drop (implicit close)
        println!("STEP 7: Drop database");
        drop(db);

        // 8. Reopen to verify data persistence
        println!("STEP 8: Reopen and verify");
        let db2 = RocksDbMemex::open(&path).expect("reopen failed");
        assert!(db2.health_check().is_ok());

        println!("RESULT: PASS - Full lifecycle completed successfully");
    }

    #[test]
    fn test_wal_disabled() {
        println!("=== TEST: WAL disabled configuration ===");
        let tmp = TempDir::new().expect("create temp dir");

        let config = RocksDbConfig {
            enable_wal: false,
            ..Default::default()
        };

        println!("BEFORE: Opening with WAL disabled");
        let db = RocksDbMemex::open_with_config(tmp.path(), config).expect("open failed");
        println!("AFTER: Database opened with WAL disabled");

        assert!(db.health_check().is_ok());
        println!("RESULT: PASS - Database works with WAL disabled");
    }

    #[test]
    fn test_database_files_created() {
        println!("=== TEST: Database files created on disk ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path();

        println!("BEFORE: Directory contents: {:?}", std::fs::read_dir(path).map(|r| r.count()));

        let db = RocksDbMemex::open(path).expect("open failed");
        db.flush_all().expect("flush failed");

        println!("AFTER: Directory contents:");
        for entry in std::fs::read_dir(path).expect("read dir") {
            if let Ok(e) = entry {
                println!("  - {:?}", e.file_name());
            }
        }

        // Verify RocksDB files exist
        let has_sst_or_manifest = std::fs::read_dir(path)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .any(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.contains("MANIFEST") || name.contains("CURRENT") || name.contains("LOG")
            });

        assert!(has_sst_or_manifest, "RocksDB control files should exist");
        println!("RESULT: PASS - Database files verified on disk");

        drop(db);
    }
}
