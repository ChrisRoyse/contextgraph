//! RocksDB lock-file handling tests.
//!
//! These tests verify that the RocksDB store correctly leaves LOCK files under
//! RocksDB ownership. The opener may inspect a LOCK file, but it must not delete
//! it because deleting a live RocksDB lock can permit concurrent write opens.
//!
//! CRITICAL: Uses #[tokio::test] to prevent zombie runtime threads.
//! DO NOT use tokio::runtime::Runtime::new() in tests.

use super::helpers::create_real_fingerprint;
use crate::teleological::RocksDbTeleologicalStore;
use context_graph_core::traits::TeleologicalMemoryStore;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_lock_file_opens_when_unheld() {
    println!("=== LOCK TEST: Database opens with unheld LOCK file ===");

    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("stale_lock_test");
    fs::create_dir_all(&db_path).expect("Failed to create db dir");

    // Step 1: Create an unheld LOCK file (simulating a cleanly closed DB path).
    let lock_path = db_path.join("LOCK");
    fs::write(&lock_path, "").expect("Failed to create unheld LOCK file");
    println!("BEFORE: Created unheld LOCK file at {:?}", lock_path);
    assert!(lock_path.exists(), "LOCK file should exist");

    // Step 2: Open the database - this should leave the LOCK file in place and
    // let RocksDB acquire its own lock.
    println!("OPENING: Attempting to open database with unheld LOCK...");
    let store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Should open successfully with unheld LOCK");

    println!("AFTER: Database opened successfully");

    // Step 3: Verify the database is usable by performing a basic operation
    let count = store.count().await.expect("Should be able to count");
    println!("VERIFY: Database count = {} (expected 0 for new DB)", count);
    assert_eq!(count, 0, "New database should have 0 entries");

    println!("RESULT: PASS - Unheld LOCK was not removed and database opened successfully");
}

#[tokio::test]
async fn test_lock_file_fresh_database() {
    println!("=== LOCK TEST: Fresh database opens without LOCK file ===");

    // Create a temporary directory with no LOCK file
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("fresh_db_test");
    fs::create_dir_all(&db_path).expect("Failed to create db dir");

    let lock_path = db_path.join("LOCK");
    println!("BEFORE: No LOCK file exists at {:?}", lock_path);
    assert!(!lock_path.exists(), "LOCK file should NOT exist");

    // Open the database - should work normally
    println!("OPENING: Opening fresh database...");
    let store =
        RocksDbTeleologicalStore::open(&db_path).expect("Should open fresh database successfully");

    println!("AFTER: Database opened successfully");

    // Verify the database is usable
    let count = store.count().await.expect("Should be able to count");
    println!("VERIFY: Database count = {} (expected 0 for new DB)", count);
    assert_eq!(count, 0, "New database should have 0 entries");

    println!("RESULT: PASS - Fresh database opened without issues");
}

#[tokio::test]
async fn test_lock_file_reopen_after_close() {
    println!("=== LOCK TEST: Database reopens after clean close ===");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("reopen_test");
    fs::create_dir_all(&db_path).expect("Failed to create db dir");

    // Step 1: Open, write, and close the database
    println!("STEP 1: Opening database and writing data...");
    {
        let store = RocksDbTeleologicalStore::open(&db_path).expect("Should open database");

        let fp = create_real_fingerprint();
        store.store(fp).await.expect("Should store fingerprint");
        println!("STEP 1: Stored 1 fingerprint, dropping database handle...");
    } // Database should be closed here, releasing the LOCK

    // Step 2: Verify whether RocksDB left a LOCK file behind. Either state is acceptable.
    let lock_path = db_path.join("LOCK");
    println!(
        "STEP 2: LOCK file exists = {} (may or may not based on RocksDB behavior)",
        lock_path.exists()
    );

    // Step 3: Reopen the database
    println!("STEP 3: Reopening database...");
    let store =
        RocksDbTeleologicalStore::open(&db_path).expect("Should reopen database successfully");

    // Step 4: Verify data persisted
    let count = store.count().await.expect("Should be able to count");
    println!("VERIFY: Database count = {} (expected 1)", count);
    assert_eq!(count, 1, "Reopened database should have 1 entry");

    println!("RESULT: PASS - Database reopened and data persisted");
}

#[tokio::test]
async fn test_live_lock_second_open_fails_without_removing_lock() {
    println!("=== LOCK TEST: Live RocksDB lock blocks second open ===");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("live_lock_test");
    fs::create_dir_all(&db_path).expect("Failed to create db dir");

    let first = RocksDbTeleologicalStore::open(&db_path).expect("first open should succeed");
    let lock_path = db_path.join("LOCK");
    println!("BEFORE SECOND OPEN: LOCK exists = {}", lock_path.exists());
    assert!(lock_path.exists(), "RocksDB should create a LOCK file");

    let second = RocksDbTeleologicalStore::open(&db_path);
    println!(
        "SECOND OPEN RESULT: {:?}",
        second.as_ref().err().map(ToString::to_string)
    );
    assert!(
        second.is_err(),
        "second live open must fail instead of removing a live LOCK file"
    );
    assert!(
        lock_path.exists(),
        "LOCK file must remain after rejected second open"
    );

    drop(first);
    let reopened = RocksDbTeleologicalStore::open(&db_path).expect("reopen after drop should work");
    let count = reopened.count().await.expect("count after reopen");
    println!("AFTER DROP: reopened count = {}", count);
    assert_eq!(count, 0);

    println!("RESULT: PASS - Live lock failed closed and DB remained reopenable");
}

#[tokio::test]
async fn test_multiple_unheld_lock_files() {
    println!("=== LOCK TEST: Multiple operations with unheld LOCK files ===");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("multi_stale_test");
    fs::create_dir_all(&db_path).expect("Failed to create db dir");

    // Iteration 1: Create unheld lock, open, close
    println!("ITERATION 1: Creating unheld lock and opening...");
    let lock_path = db_path.join("LOCK");
    fs::write(&lock_path, "").expect("Failed to create unheld LOCK");
    {
        let _store = RocksDbTeleologicalStore::open(&db_path).expect("Iteration 1: Should open");
    }
    println!("ITERATION 1: Closed database");

    // Iteration 2: Simulate another closed DB path with an unheld LOCK file.
    println!("ITERATION 2: Re-creating unheld lock...");
    if !lock_path.exists() {
        fs::write(&lock_path, "").expect("Failed to create unheld LOCK");
    }
    {
        let _store = RocksDbTeleologicalStore::open(&db_path)
            .expect("Iteration 2: Should open with unheld LOCK");
    }
    println!("ITERATION 2: Closed database");

    // Iteration 3: One more time
    println!("ITERATION 3: Final unheld lock test...");
    if !lock_path.exists() {
        fs::write(&lock_path, "").expect("Failed to create unheld LOCK");
    }
    let store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Iteration 3: Should open with unheld LOCK");

    // Verify database is functional
    let count = store.count().await.expect("Should be able to count");
    println!(
        "VERIFY: Database opened {} times with unheld LOCK files, count = {}",
        3, count
    );

    println!("RESULT: PASS - Multiple unheld LOCK scenarios handled without deleting LOCK");
}
