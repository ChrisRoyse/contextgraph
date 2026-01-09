//! Core database tests: open, close, health check, column families.
//!
//! Tests for RocksDbMemex core functionality, configuration, and error handling.

use tempfile::TempDir;

use super::config::RocksDbConfig;
use super::core::RocksDbMemex;
use super::error::StorageError;
use crate::column_families::cf_names;

// =========================================================================
// Helper Functions
// =========================================================================

fn create_temp_db() -> (TempDir, RocksDbMemex) {
    let tmp = TempDir::new().expect("Failed to create temp dir");
    let db = RocksDbMemex::open(tmp.path()).expect("Failed to open database");
    (tmp, db)
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

    let expected_cfs = [
        "nodes",
        "edges",
        "embeddings",
        "metadata",
        "johari_open",
        "johari_hidden",
        "johari_blind",
        "johari_unknown",
        "temporal",
        "tags",
        "sources",
        "system",
    ];

    for cf in &expected_cfs {
        let result = db.get_cf(cf);
        println!("CF '{}' accessible: {}", cf, result.is_ok());
        assert!(result.is_ok(), "CF '{}' should be accessible", cf);
    }
}

// =========================================================================
// Health Check and Flush Tests
// =========================================================================

#[test]
fn test_health_check_passes() {
    let (_tmp, db) = create_temp_db();
    let result = db.health_check();
    assert!(result.is_ok(), "Health check should pass: {:?}", result);
}

#[test]
fn test_health_check_verifies_all_cfs() {
    println!("=== TEST: health_check() verifies all 12 CFs ===");
    let (_tmp, db) = create_temp_db();

    // Health check should pass on fresh database
    let result = db.health_check();
    assert!(result.is_ok());
    println!(
        "Health check passed: all {} CFs verified",
        cf_names::ALL.len()
    );
}

#[test]
fn test_flush_all_succeeds() {
    let (_tmp, db) = create_temp_db();
    let result = db.flush_all();
    assert!(result.is_ok(), "Flush should succeed: {:?}", result);
}

#[test]
fn test_flush_all_on_empty_db() {
    println!("=== TEST: flush_all() on empty database ===");
    let (_tmp, db) = create_temp_db();

    println!("BEFORE: Flushing empty database");
    let result = db.flush_all();
    println!("AFTER: Flush result = {:?}", result);

    assert!(result.is_ok(), "Flush on empty DB should succeed");
}

// =========================================================================
// Reopen and Persistence Tests
// =========================================================================

#[test]
fn test_reopen_preserves_cfs() {
    println!("=== TEST: Reopen preserves column families ===");
    let tmp = TempDir::new().expect("create temp dir");
    let path = tmp.path().to_path_buf();

    {
        println!("BEFORE: Opening database first time");
        let db = RocksDbMemex::open(&path).expect("first open failed");
        assert!(db.health_check().is_ok());
        println!("AFTER: First open successful, dropping database");
    }

    {
        println!("BEFORE: Reopening database");
        let db = RocksDbMemex::open(&path).expect("reopen failed");
        println!("AFTER: Reopen successful");

        for cf_name in cf_names::ALL {
            let cf = db.get_cf(cf_name);
            assert!(cf.is_ok(), "CF '{}' should exist after reopen", cf_name);
        }
        println!("RESULT: All 12 CFs preserved after reopen");
    }
}

// =========================================================================
// Accessor Tests
// =========================================================================

#[test]
fn test_db_accessor() {
    let (_tmp, db) = create_temp_db();
    let raw_db = db.db();
    let path = raw_db.path();
    assert!(!path.to_string_lossy().is_empty());
}

#[test]
fn test_path_accessor() {
    let tmp = TempDir::new().expect("create temp dir");
    let expected_path = tmp.path().to_string_lossy().to_string();
    let db = RocksDbMemex::open(tmp.path()).expect("open failed");
    assert_eq!(db.path(), expected_path);
}

// =========================================================================
// WAL and Config Tests
// =========================================================================

#[test]
fn test_wal_disabled() {
    println!("=== TEST: WAL can be disabled ===");
    let tmp = TempDir::new().expect("create temp dir");

    let config = RocksDbConfig {
        enable_wal: false,
        ..Default::default()
    };

    println!("BEFORE: Opening with WAL disabled");
    let db = RocksDbMemex::open_with_config(tmp.path(), config).expect("open failed");
    println!("AFTER: Database opened with WAL disabled");

    assert!(db.health_check().is_ok());
}

// =========================================================================
// Edge Case Tests
// =========================================================================

#[test]
fn edge_case_multiple_opens_same_path_fails() {
    println!("=== EDGE CASE 1: Multiple opens on same path ===");
    let tmp = TempDir::new().expect("create temp dir");

    let db1 = RocksDbMemex::open(tmp.path()).expect("first open");
    println!("BEFORE: First database opened at {:?}", tmp.path());

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
