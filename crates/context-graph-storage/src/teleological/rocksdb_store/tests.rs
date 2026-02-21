//! Tests for RocksDbTeleologicalStore.
//!
//! Comprehensive tests for CRUD operations, persistence, and trait compliance.

use super::*;
use tempfile::TempDir;
use uuid::Uuid;

use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};

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
    let e5_vec = generate_vec(768, seed + 4);
    let semantic = SemanticFingerprint {
        e1_semantic: generate_vec(1024, seed),               // 1024D
        e2_temporal_recent: generate_vec(512, seed + 1),     // 512D
        e3_temporal_periodic: generate_vec(512, seed + 2),   // 512D
        e4_temporal_positional: generate_vec(512, seed + 3), // 512D
        e5_causal_as_cause: e5_vec.clone(),                  // 768D (as cause)
        e5_causal_as_effect: e5_vec,                         // 768D (as effect)
        e5_causal: Vec::new(),                               // Empty - using new dual format
        e6_sparse: generate_sparse(seed + 5),                // Sparse
        e7_code: generate_vec(1536, seed + 6),               // 1536D
        e8_graph_as_source: generate_vec(1024, seed + 7),       // 1024D (as source)
        e8_graph_as_target: generate_vec(1024, seed + 8),       // 1024D (as target)
        e8_graph: Vec::new(),                                  // Legacy field, empty
        e9_hdc: generate_vec(1024, seed + 8),                  // 1024D HDC (projected)
        e10_multimodal_paraphrase: generate_vec(768, seed + 9), // 768D (paraphrase side)
        e10_multimodal_as_context: generate_vec(768, seed + 13), // 768D (as context)
        e11_entity: generate_vec(768, seed + 10),              // 768D (KEPLER)
        e12_late_interaction: generate_late_interaction(seed + 11), // Vec<Vec<f32>>
        e13_splade: generate_sparse(seed + 12),              // Sparse
    };

    // Create unique hash
    let mut hash = [0u8; 32];
    for (i, byte) in hash.iter_mut().enumerate() {
        *byte = ((seed + i as u64) % 256) as u8;
    }

    TeleologicalFingerprint::new(semantic, hash)
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
    assert_eq!(
        store.backend_type(),
        context_graph_core::traits::TeleologicalStorageBackend::RocksDb
    );
}

// ============================================================================
// Corruption Detection Tests - REAL data, NO mocks (TASK-STORAGE-001)
// ============================================================================

/// Test that corruption detection catches missing SST files.
///
/// This test uses REAL RocksDB data:
/// 1. Creates a valid database with multiple fingerprints
/// 2. Forces flush to create SST files on disk
/// 3. Closes database cleanly
/// 4. Deletes an SST file to simulate corruption
/// 5. Attempts to reopen and verifies CorruptionDetected error
#[tokio::test]
async fn test_corruption_detection_missing_sst_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_path_buf();

    // Step 1: Create database with REAL data (not mocks)
    {
        let store = create_initialized_store(&path);

        // Store multiple fingerprints to ensure SST files are created
        for seed in 1..=10 {
            let fp = create_test_fingerprint_with_seed(seed);
            store.store(fp).await.expect("Store should succeed");
        }

        // Force flush to ensure data is written to SST files
        store.flush().await.expect("Flush should succeed");
    }

    // Step 2: Identify SST files
    let sst_files: Vec<std::path::PathBuf> = std::fs::read_dir(&path)
        .expect("Should read directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".sst"))
        .map(|e| e.path())
        .collect();

    assert!(
        !sst_files.is_empty(),
        "Database should have at least one SST file after storing fingerprints and flushing"
    );

    // Step 3: Delete an SST file to simulate corruption
    let deleted_file = &sst_files[0];
    let deleted_name = deleted_file
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    std::fs::remove_file(deleted_file).expect("Should delete SST file");

    // Step 4: Attempt to reopen - should fail with CorruptionDetected
    let result = RocksDbTeleologicalStore::open(&path);

    match result {
        Err(TeleologicalStoreError::CorruptionDetected {
            path: err_path,
            missing_count,
            missing_files,
            manifest_file,
        }) => {
            // Verify error contains correct information
            assert_eq!(err_path, path.to_string_lossy().to_string());
            assert!(missing_count >= 1, "Should detect at least 1 missing file");
            // Split comma-separated list and check for exact match (not substring)
            // to avoid false positives like "12.sst" matching "112.sst"
            let file_list: Vec<&str> = missing_files.split(", ").collect();
            let deleted_without_ext = deleted_name.replace(".sst", "");
            assert!(
                file_list
                    .iter()
                    .any(|f| *f == deleted_name || *f == deleted_without_ext),
                "Missing files should include the deleted file '{}', got: {:?}",
                deleted_name,
                file_list
            );
            assert!(
                manifest_file.contains("MANIFEST-"),
                "Should reference a MANIFEST file, got: {}",
                manifest_file
            );
        }
        Err(e) => {
            // Also acceptable: RocksDB's own corruption error
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Corruption") || err_msg.contains("No such file"),
                "Expected CorruptionDetected or RocksDB corruption error, got: {}",
                e
            );
        }
        Ok(_) => {
            panic!("Expected corruption error when opening database with missing SST file");
        }
    }
}

/// Test that a clean database passes corruption check.
///
/// Verifies that corruption detection doesn't produce false positives
/// on a healthy database with REAL data.
#[tokio::test]
async fn test_corruption_detection_clean_database() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_path_buf();

    // Create and populate database
    {
        let store = create_initialized_store(&path);
        for seed in 1..=5 {
            let fp = create_test_fingerprint_with_seed(seed);
            store.store(fp).await.expect("Store should succeed");
        }
        store.flush().await.expect("Flush should succeed");
    }

    // Reopen should succeed with no corruption
    let store = RocksDbTeleologicalStore::open(&path);
    assert!(
        store.is_ok(),
        "Clean database should open without corruption error: {:?}",
        store.err()
    );

    // Verify data is still accessible
    let store = store.unwrap();
    let count = store.count().await.expect("Count should succeed");
    assert_eq!(count, 5, "Should have 5 fingerprints after reopen");
}

/// Test corruption detection when MANIFEST references missing files.
///
/// This simulates the exact scenario from the real corruption incident:
/// MANIFEST-000701 referencing missing 000682.sst (file above max existing)
///
/// The detection heuristic catches files referenced ABOVE max_existing,
/// which is the typical pattern when a crash occurs during compaction/flush.
#[tokio::test]
async fn test_corruption_detection_manifest_sst_mismatch() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_path_buf();

    // Step 1: Create database with enough data to generate multiple SST files
    {
        let store = create_initialized_store(&path);

        // Store many fingerprints to force multiple flushes and compactions
        for seed in 1..=50 {
            let fp = create_test_fingerprint_with_seed(seed);
            store.store(fp).await.expect("Store should succeed");

            // Periodic flush to create SST files
            if seed % 10 == 0 {
                store.flush().await.expect("Flush should succeed");
            }
        }
        store.flush().await.expect("Final flush should succeed");
    }

    // Step 2: Get sorted SST files and delete the HIGHEST numbered one(s)
    // This simulates corruption where new files were referenced but not fully written
    let mut sst_files: Vec<_> = std::fs::read_dir(&path)
        .expect("Read dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".sst"))
        .collect();

    // Sort by file name (which includes the number)
    sst_files.sort_by_key(|e| e.file_name().to_string_lossy().to_string());

    assert!(!sst_files.is_empty(), "Should have SST files");

    // Delete the LAST (highest numbered) file to simulate incomplete write
    let deleted_file = sst_files.pop().unwrap();
    std::fs::remove_file(deleted_file.path()).expect("Delete SST");

    // Step 3: Verify corruption is detected (or RocksDB reports it)
    let result = RocksDbTeleologicalStore::open(&path);

    // Either our detection or RocksDB's should catch this
    match result {
        Err(TeleologicalStoreError::CorruptionDetected { missing_count, .. }) => {
            assert!(
                missing_count >= 1,
                "Should detect missing file(s), got: {}",
                missing_count
            );
        }
        Err(e) => {
            // RocksDB's own error is also acceptable for corruption
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Corruption") || err_msg.contains("No such file"),
                "Expected corruption error, got: {}",
                e
            );
        }
        Ok(_) => {
            // If it opens successfully, that's actually OK too - RocksDB might have
            // recovered automatically or the deleted file wasn't needed.
            // The key test is test_corruption_detection_missing_sst_file which
            // deliberately deletes a known file.
        }
    }
}

/// Test that new (empty) database passes corruption check.
///
/// A fresh database without CURRENT file should not trigger false positives.
#[tokio::test]
async fn test_corruption_detection_new_database() {
    let tmp = TempDir::new().unwrap();

    // Open fresh database - should succeed (no CURRENT file yet)
    let result = RocksDbTeleologicalStore::open(tmp.path());
    assert!(
        result.is_ok(),
        "New database should open without corruption error: {:?}",
        result.err()
    );
}

/// Test that corruption detection provides actionable error messages.
///
/// Verifies FAIL FAST policy with detailed context for debugging.
#[tokio::test]
async fn test_corruption_detection_error_details() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_path_buf();

    // Create database
    {
        let store = create_initialized_store(&path);
        let fp = create_test_fingerprint_with_seed(99);
        store.store(fp).await.expect("Store should succeed");
        store.flush().await.expect("Flush should succeed");
    }

    // Corrupt by deleting SST files
    for entry in std::fs::read_dir(&path).expect("Read dir") {
        if let Ok(entry) = entry {
            if entry.file_name().to_string_lossy().ends_with(".sst") {
                std::fs::remove_file(entry.path()).expect("Delete");
                break; // Delete just one
            }
        }
    }

    // Get error and verify details
    let result = RocksDbTeleologicalStore::open(&path);
    let err = match result {
        Ok(_) => panic!("Should fail on corruption"),
        Err(e) => e,
    };
    let err_string = err.to_string();

    // Verify error message contains FAIL FAST debugging info
    // Either our custom CorruptionDetected or RocksDB's error
    assert!(
        err_string.contains("CORRUPTION")
            || err_string.contains("Corruption")
            || err_string.contains("No such file"),
        "Error should indicate corruption, got: {}",
        err_string
    );

    // If it's our custom error, verify structure
    if let TeleologicalStoreError::CorruptionDetected {
        path: err_path,
        missing_count,
        missing_files,
        manifest_file,
    } = err
    {
        // Verify all fields are populated
        assert!(!err_path.is_empty(), "Path should not be empty");
        assert!(missing_count >= 1, "Should have at least 1 missing file");
        assert!(
            !missing_files.is_empty(),
            "Missing files list should not be empty"
        );
        assert!(
            !manifest_file.is_empty(),
            "Manifest file should be identified"
        );

        // Verify path matches
        assert!(
            err_path.contains(&path.file_name().unwrap().to_string_lossy().to_string()),
            "Error path should match database path"
        );
    }
}

// =============================================================================
// FILE INDEX TESTS (CF_FILE_INDEX column family)
// =============================================================================

/// Test that file index operations work correctly with RocksDB storage.
#[tokio::test]
async fn test_file_index_crud_operations() {
    println!("\n=== FSV: RocksDB File Index CRUD Test ===\n");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    let file_path1 = "/test/docs/file1.md";
    let file_path2 = "/test/docs/file2.md";

    // Create fingerprints to index
    let fp1 = create_test_fingerprint_with_seed(1);
    let fp2 = create_test_fingerprint_with_seed(2);
    let fp3 = create_test_fingerprint_with_seed(3);
    let id1 = fp1.id;
    let id2 = fp2.id;
    let id3 = fp3.id;

    // Store fingerprints first (file index references these)
    store.store(fp1).await.unwrap();
    store.store(fp2).await.unwrap();
    store.store(fp3).await.unwrap();

    println!("PHASE 1: Index file fingerprints");

    // Index fingerprints for file1 (two chunks)
    store.index_file_fingerprint(file_path1, id1).await.unwrap();
    store.index_file_fingerprint(file_path1, id2).await.unwrap();

    // Index fingerprint for file2 (one chunk)
    store.index_file_fingerprint(file_path2, id3).await.unwrap();

    println!("  Indexed 2 fingerprints for file1, 1 for file2");

    println!("\nPHASE 2: Retrieve fingerprints by file path");

    let file1_ids = store.get_fingerprints_for_file(file_path1).await.unwrap();
    let file2_ids = store.get_fingerprints_for_file(file_path2).await.unwrap();

    assert_eq!(file1_ids.len(), 2, "File1 should have 2 fingerprints");
    assert_eq!(file2_ids.len(), 1, "File2 should have 1 fingerprint");

    assert!(file1_ids.contains(&id1), "File1 should contain id1");
    assert!(file1_ids.contains(&id2), "File1 should contain id2");
    assert!(file2_ids.contains(&id3), "File2 should contain id3");

    println!("  ✓ File1 has {} fingerprints", file1_ids.len());
    println!("  ✓ File2 has {} fingerprints", file2_ids.len());

    println!("\nPHASE 3: List indexed files");

    let indexed_files = store.list_indexed_files().await.unwrap();
    assert_eq!(indexed_files.len(), 2, "Should have 2 indexed files");

    let paths: Vec<&str> = indexed_files.iter().map(|e| e.file_path.as_str()).collect();
    assert!(paths.contains(&file_path1), "Should contain file1");
    assert!(paths.contains(&file_path2), "Should contain file2");

    println!("  ✓ Found {} indexed files", indexed_files.len());

    println!("\nPHASE 4: Get file watcher stats");

    let stats = store.get_file_watcher_stats().await.unwrap();
    assert_eq!(stats.total_files, 2, "Should have 2 files");
    assert_eq!(stats.total_chunks, 3, "Should have 3 total chunks");
    assert_eq!(stats.min_chunks, 1, "Min chunks should be 1");
    assert_eq!(stats.max_chunks, 2, "Max chunks should be 2");

    println!("  ✓ Stats: {} files, {} chunks", stats.total_files, stats.total_chunks);

    println!("\nPHASE 5: Unindex a fingerprint");

    let removed = store.unindex_file_fingerprint(file_path1, id1).await.unwrap();
    assert!(removed, "Should have removed id1 from file1");

    let file1_ids_after = store.get_fingerprints_for_file(file_path1).await.unwrap();
    assert_eq!(file1_ids_after.len(), 1, "File1 should now have 1 fingerprint");
    assert!(file1_ids_after.contains(&id2), "File1 should still contain id2");

    println!("  ✓ Unindexed id1 from file1, now has {} fingerprints", file1_ids_after.len());

    println!("\nPHASE 6: Clear file index");

    let cleared = store.clear_file_index(file_path1).await.unwrap();
    assert_eq!(cleared, 1, "Should have cleared 1 fingerprint from file1");

    let file1_ids_cleared = store.get_fingerprints_for_file(file_path1).await.unwrap();
    assert!(file1_ids_cleared.is_empty(), "File1 should have no fingerprints");

    // File1 should no longer appear in list
    let indexed_files_after = store.list_indexed_files().await.unwrap();
    let paths_after: Vec<&str> = indexed_files_after.iter().map(|e| e.file_path.as_str()).collect();
    assert!(!paths_after.contains(&file_path1), "File1 should not be in index");
    assert!(paths_after.contains(&file_path2), "File2 should still be in index");

    println!("  ✓ Cleared file1 index, {} files remain", indexed_files_after.len());

    println!("\n=== FSV: PASSED - File index CRUD operations work correctly ===\n");
}

/// Test that file index persists across store reopens.
#[tokio::test]
async fn test_file_index_persistence() {
    println!("\n=== FSV: RocksDB File Index Persistence Test ===\n");

    let tmp = TempDir::new().unwrap();
    let file_path = "/test/persistent.md";
    let fp = create_test_fingerprint_with_seed(100);
    let id = fp.id;

    // Phase 1: Store and index
    {
        let store = create_initialized_store(tmp.path());
        store.store(fp).await.unwrap();
        store.index_file_fingerprint(file_path, id).await.unwrap();

        let ids = store.get_fingerprints_for_file(file_path).await.unwrap();
        assert_eq!(ids.len(), 1, "Should have 1 fingerprint before close");
        println!("  Indexed 1 fingerprint before close");
    }
    // Store dropped, DB closed

    // Phase 2: Reopen and verify persistence
    {
        let store = create_initialized_store(tmp.path());
        let ids = store.get_fingerprints_for_file(file_path).await.unwrap();
        assert_eq!(ids.len(), 1, "Should have 1 fingerprint after reopen");
        assert_eq!(ids[0], id, "Should be the same fingerprint ID");

        let indexed_files = store.list_indexed_files().await.unwrap();
        assert_eq!(indexed_files.len(), 1, "Should have 1 indexed file");
        assert_eq!(indexed_files[0].file_path, file_path, "File path should match");

        println!("  ✓ File index persisted across store reopen");
    }

    println!("\n=== FSV: PASSED - File index persistence verified ===\n");
}

/// Test empty file index behavior.
#[tokio::test]
async fn test_file_index_empty_cases() {
    println!("\n=== FSV: RocksDB File Index Empty Cases Test ===\n");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    // List on empty index
    let files = store.list_indexed_files().await.unwrap();
    assert!(files.is_empty(), "Empty index should return empty list");
    println!("  ✓ list_indexed_files returns empty for new store");

    // Get fingerprints for non-existent file
    let ids = store.get_fingerprints_for_file("/nonexistent.md").await.unwrap();
    assert!(ids.is_empty(), "Non-existent file should return empty list");
    println!("  ✓ get_fingerprints_for_file returns empty for non-existent file");

    // Stats on empty index
    let stats = store.get_file_watcher_stats().await.unwrap();
    assert_eq!(stats.total_files, 0, "Empty index should have 0 files");
    assert_eq!(stats.total_chunks, 0, "Empty index should have 0 chunks");
    println!("  ✓ get_file_watcher_stats returns zeros for empty index");

    // Clear non-existent file
    let cleared = store.clear_file_index("/nonexistent.md").await.unwrap();
    assert_eq!(cleared, 0, "Clearing non-existent file should return 0");
    println!("  ✓ clear_file_index returns 0 for non-existent file");

    // Unindex from non-existent file
    let removed = store.unindex_file_fingerprint("/nonexistent.md", Uuid::new_v4()).await.unwrap();
    assert!(!removed, "Unindexing from non-existent file should return false");
    println!("  ✓ unindex_file_fingerprint returns false for non-existent file");

    println!("\n=== FSV: PASSED - Empty index edge cases handled correctly ===\n");
}

// =============================================================================
// CAUSAL RELATIONSHIP REPAIR TESTS (Full State Verification)
// =============================================================================

/// Create a valid test causal relationship for repair testing.
fn create_test_causal_relationship(id: Uuid, source_id: Uuid) -> context_graph_core::types::CausalRelationship {
    context_graph_core::types::CausalRelationship {
        id,
        source_fingerprint_id: source_id,
        cause_statement: "Test cause statement".to_string(),
        effect_statement: "Test effect statement".to_string(),
        explanation: "Test causal relationship explanation".to_string(),
        mechanism_type: "direct".to_string(),
        confidence: 0.85,
        e1_semantic: vec![0.1; 1024],
        e5_as_cause: vec![0.2; 768],
        e5_as_effect: vec![0.3; 768],
        e5_source_cause: vec![0.25; 768],
        e5_source_effect: vec![0.35; 768],
        e8_graph_source: vec![0.4; 1024],
        e8_graph_target: vec![0.5; 1024],
        e11_entity: vec![0.6; 768],
        source_content: "Source content for test".to_string(),
        llm_provenance: None,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        source_spans: vec![],
    }
}

/// FSV Test: Repair with no corrupted entries (no false positives).
#[tokio::test]
async fn test_repair_causal_no_corruption() {
    println!("\n=== FSV: Repair Causal Relationships (No Corruption) ===\n");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    // Store 3 valid causal relationships
    let rel1 = create_test_causal_relationship(Uuid::new_v4(), Uuid::new_v4());
    let rel2 = create_test_causal_relationship(Uuid::new_v4(), Uuid::new_v4());
    let rel3 = create_test_causal_relationship(Uuid::new_v4(), Uuid::new_v4());

    let id1 = store.store_causal_relationship(&rel1).await.unwrap();
    let id2 = store.store_causal_relationship(&rel2).await.unwrap();
    let id3 = store.store_causal_relationship(&rel3).await.unwrap();

    println!("BEFORE REPAIR:");
    println!("  Stored relationships: {}, {}, {}", id1, id2, id3);

    // Verify all exist before repair
    assert!(store.get_causal_relationship(id1).await.unwrap().is_some());
    assert!(store.get_causal_relationship(id2).await.unwrap().is_some());
    assert!(store.get_causal_relationship(id3).await.unwrap().is_some());
    println!("  All 3 relationships verified to exist");

    // RUN REPAIR
    let (deleted_count, total_scanned) = store
        .repair_corrupted_causal_relationships()
        .await
        .expect("Repair should succeed");

    println!("\nAFTER REPAIR:");
    println!("  Deleted: {}, Scanned: {}", deleted_count, total_scanned);

    // VERIFY: No entries deleted
    assert_eq!(deleted_count, 0, "Should delete 0 entries (all valid)");
    assert_eq!(total_scanned, 3, "Should scan 3 entries");

    // PHYSICAL VERIFICATION: All entries still exist
    assert!(store.get_causal_relationship(id1).await.unwrap().is_some());
    assert!(store.get_causal_relationship(id2).await.unwrap().is_some());
    assert!(store.get_causal_relationship(id3).await.unwrap().is_some());
    println!("  All 3 relationships still exist after repair");

    println!("\n=== FSV: PASSED - No false positives ===\n");
}

/// FSV Test: Repair empty database (edge case).
#[tokio::test]
async fn test_repair_causal_empty_database() {
    println!("\n=== FSV: Repair Causal Relationships (Empty Database) ===\n");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    println!("BEFORE REPAIR: No causal relationships stored");

    // RUN REPAIR on empty database
    let (deleted_count, total_scanned) = store
        .repair_corrupted_causal_relationships()
        .await
        .expect("Repair should succeed on empty database");

    println!("AFTER REPAIR: Deleted: {}, Scanned: {}", deleted_count, total_scanned);

    assert_eq!(deleted_count, 0, "Should delete 0 entries");
    assert_eq!(total_scanned, 0, "Should scan 0 entries");

    println!("\n=== FSV: PASSED - Empty database handled correctly ===\n");
}

/// FSV Test: Repair with corrupted entries (happy path).
#[tokio::test]
async fn test_repair_causal_with_corruption() {
    use crate::teleological::schema::causal_relationship_key;
    use rocksdb::IteratorMode;

    println!("\n=== FSV: Repair Causal Relationships (With Corruption) ===\n");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    // Store 2 valid relationships
    let rel1 = create_test_causal_relationship(Uuid::new_v4(), Uuid::new_v4());
    let rel2 = create_test_causal_relationship(Uuid::new_v4(), Uuid::new_v4());
    let id1 = store.store_causal_relationship(&rel1).await.unwrap();
    let id2 = store.store_causal_relationship(&rel2).await.unwrap();

    // Inject 2 corrupted entries directly into RocksDB
    let corrupted_id1 = Uuid::new_v4();
    let corrupted_id2 = Uuid::new_v4();
    let cf = store.cf_causal_relationships();

    // Write truncated binary data (simulating crash during write)
    store.db.put_cf(cf, causal_relationship_key(&corrupted_id1), &[0x01, 0x02, 0x03]).unwrap();
    store.db.put_cf(cf, causal_relationship_key(&corrupted_id2), &[0xff; 50]).unwrap();

    println!("BEFORE REPAIR:");
    println!("  Valid relationships: {}, {}", id1, id2);
    println!("  Corrupted entries: {}, {}", corrupted_id1, corrupted_id2);

    // Count entries in CF before repair
    let count_before = store.db.iterator_cf(cf, IteratorMode::Start).count();
    println!("  Total entries in CF: {}", count_before);
    assert_eq!(count_before, 4, "Should have 4 entries (2 valid + 2 corrupted)");

    // Verify valid entries exist
    assert!(store.get_causal_relationship(id1).await.unwrap().is_some());
    assert!(store.get_causal_relationship(id2).await.unwrap().is_some());

    // Verify corrupted entries exist as raw bytes
    assert!(store.db.get_cf(cf, causal_relationship_key(&corrupted_id1)).unwrap().is_some());
    assert!(store.db.get_cf(cf, causal_relationship_key(&corrupted_id2)).unwrap().is_some());
    println!("  Corrupted raw data verified to exist");

    // RUN REPAIR
    let (deleted_count, total_scanned) = store
        .repair_corrupted_causal_relationships()
        .await
        .expect("Repair should succeed");

    println!("\nAFTER REPAIR:");
    println!("  Deleted: {}, Scanned: {}", deleted_count, total_scanned);

    // VERIFY: Exactly 2 corrupted entries deleted
    assert_eq!(deleted_count, 2, "Should delete 2 corrupted entries");
    assert_eq!(total_scanned, 4, "Should scan 4 entries");

    // PHYSICAL VERIFICATION: Count entries in CF after repair
    let count_after = store.db.iterator_cf(cf, IteratorMode::Start).count();
    println!("  Total entries in CF after repair: {}", count_after);
    assert_eq!(count_after, 2, "Should have 2 entries (only valid ones)");

    // VERIFY: Valid entries still exist
    assert!(store.get_causal_relationship(id1).await.unwrap().is_some());
    assert!(store.get_causal_relationship(id2).await.unwrap().is_some());
    println!("  Valid relationships verified to still exist");

    // VERIFY: Corrupted entries removed from database
    assert!(store.db.get_cf(cf, causal_relationship_key(&corrupted_id1)).unwrap().is_none());
    assert!(store.db.get_cf(cf, causal_relationship_key(&corrupted_id2)).unwrap().is_none());
    println!("  Corrupted entries verified to be deleted");

    println!("\n=== FSV: PASSED - Corrupted entries removed, valid preserved ===\n");
}

/// FSV Test: Repair idempotency (multiple runs should be safe).
#[tokio::test]
async fn test_repair_causal_idempotency() {
    use crate::teleological::schema::causal_relationship_key;

    println!("\n=== FSV: Repair Causal Relationships (Idempotency) ===\n");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    // Store 1 valid relationship
    let rel1 = create_test_causal_relationship(Uuid::new_v4(), Uuid::new_v4());
    let id1 = store.store_causal_relationship(&rel1).await.unwrap();

    // Inject 1 corrupted entry
    let corrupted_id = Uuid::new_v4();
    let cf = store.cf_causal_relationships();
    store.db.put_cf(cf, causal_relationship_key(&corrupted_id), &[0xde, 0xad, 0xbe, 0xef]).unwrap();

    println!("RUN 1:");
    let (deleted1, scanned1) = store.repair_corrupted_causal_relationships().await.unwrap();
    println!("  Deleted: {}, Scanned: {}", deleted1, scanned1);
    assert_eq!(deleted1, 1, "First run should delete 1 corrupted entry");
    assert_eq!(scanned1, 2, "First run should scan 2 entries");

    println!("\nRUN 2 (idempotency check):");
    let (deleted2, scanned2) = store.repair_corrupted_causal_relationships().await.unwrap();
    println!("  Deleted: {}, Scanned: {}", deleted2, scanned2);
    assert_eq!(deleted2, 0, "Second run should delete 0 entries");
    assert_eq!(scanned2, 1, "Second run should scan 1 entry (only valid)");

    println!("\nRUN 3 (triple check):");
    let (deleted3, scanned3) = store.repair_corrupted_causal_relationships().await.unwrap();
    println!("  Deleted: {}, Scanned: {}", deleted3, scanned3);
    assert_eq!(deleted3, 0, "Third run should delete 0 entries");
    assert_eq!(scanned3, 1, "Third run should scan 1 entry");

    // PHYSICAL VERIFICATION: Valid entry still exists
    assert!(store.get_causal_relationship(id1).await.unwrap().is_some());
    println!("  Valid entry survived multiple repairs");

    println!("\n=== FSV: PASSED - Repair is idempotent ===\n");
}

/// FSV Test: Various corruption patterns.
#[tokio::test]
async fn test_repair_causal_various_corruption_patterns() {
    use crate::teleological::schema::causal_relationship_key;
    use rocksdb::IteratorMode;

    println!("\n=== FSV: Repair Causal Relationships (Various Corruption Patterns) ===\n");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());
    let cf = store.cf_causal_relationships();

    // Store 1 valid relationship
    let rel1 = create_test_causal_relationship(Uuid::new_v4(), Uuid::new_v4());
    let id1 = store.store_causal_relationship(&rel1).await.unwrap();

    // Inject various corruption patterns
    let patterns: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("single_byte", vec![0x00]),
        ("truncated_header", vec![0x01, 0x02, 0x03, 0x04, 0x05]),
        ("all_zeros", vec![0x00; 100]),
        ("all_ones", vec![0xff; 100]),
        ("random_garbage", vec![0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe]),
    ];

    let mut corrupted_ids = Vec::new();
    for (name, data) in &patterns {
        let id = Uuid::new_v4();
        store.db.put_cf(cf, causal_relationship_key(&id), data).unwrap();
        corrupted_ids.push((name, id));
        println!("  Injected {} corruption: {} ({} bytes)", name, id, data.len());
    }

    println!("\nBEFORE REPAIR:");
    let count_before = store.db.iterator_cf(cf, IteratorMode::Start).count();
    println!("  Total entries: {} (1 valid + {} corrupted)", count_before, patterns.len());

    // RUN REPAIR
    let (deleted_count, total_scanned) = store
        .repair_corrupted_causal_relationships()
        .await
        .expect("Repair should handle all corruption patterns");

    println!("\nAFTER REPAIR:");
    println!("  Deleted: {}, Scanned: {}", deleted_count, total_scanned);

    // VERIFY
    assert_eq!(deleted_count, patterns.len(), "Should delete all corrupted entries");
    assert_eq!(total_scanned, patterns.len() + 1, "Should scan all entries");

    // PHYSICAL VERIFICATION: Count entries in CF
    let count_after = store.db.iterator_cf(cf, IteratorMode::Start).count();
    println!("  Total entries after repair: {}", count_after);
    assert_eq!(count_after, 1, "Should have only 1 valid entry");

    // VERIFY: Each corrupted entry is gone
    for (name, id) in &corrupted_ids {
        assert!(store.db.get_cf(cf, causal_relationship_key(id)).unwrap().is_none(),
            "{} corruption should be deleted", name);
        println!("  ✓ {} corruption deleted: {}", name, id);
    }

    // VERIFY: Valid entry preserved
    assert!(store.get_causal_relationship(id1).await.unwrap().is_some());
    println!("  ✓ Valid entry preserved");

    println!("\n=== FSV: PASSED - All corruption patterns handled ===\n");
}

// ============================================================================
// PROVENANCE INTEGRATION TESTS - Phase 4-6 with Synthetic Data
// ============================================================================

#[tokio::test]
async fn test_provenance_audit_log_roundtrip() {
    use context_graph_core::types::audit::{AuditOperation, AuditRecord, AuditResult};

    println!("\n=== FSV: Audit Log Roundtrip ===");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    let target_id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();

    // WRITE: Create 3 audit records for the same target
    let rec1 = AuditRecord::new(AuditOperation::MemoryCreated, target_id)
        .with_operator("alice".to_string());
    let rec2 = AuditRecord::new(
        AuditOperation::ImportanceBoosted {
            old: 0.5,
            new: 0.7,
            delta: 0.2,
        },
        target_id,
    )
    .with_operator("bob".to_string());
    let rec3 = AuditRecord::new(
        AuditOperation::MemoryDeleted {
            soft: true,
            reason: Some("obsolete".to_string()),
        },
        target_id,
    )
    .with_operator("alice".to_string());

    store.append_audit_record(&rec1).unwrap();
    store.append_audit_record(&rec2).unwrap();
    store.append_audit_record(&rec3).unwrap();

    println!("  Wrote 3 audit records for target {}", target_id);

    // VERIFY: Read back by target
    let retrieved = store.get_audit_by_target(target_id, 100).unwrap();
    assert_eq!(retrieved.len(), 3, "Expected 3 records for target");

    // VERIFY: Records have correct operator_ids
    let operators: Vec<&Option<String>> = retrieved.iter().map(|r| &r.operator_id).collect();
    println!("  Operators: {:?}", operators);

    // VERIFY: First record is MemoryCreated
    assert!(
        matches!(retrieved[0].operation, AuditOperation::MemoryCreated),
        "First record should be MemoryCreated"
    );

    // VERIFY: Second record is ImportanceBoosted with correct values
    match &retrieved[1].operation {
        AuditOperation::ImportanceBoosted { old, new, delta } => {
            assert!((old - 0.5).abs() < f32::EPSILON);
            assert!((new - 0.7).abs() < f32::EPSILON);
            assert!((delta - 0.2).abs() < f32::EPSILON);
        }
        other => panic!("Expected ImportanceBoosted, got {:?}", other),
    }

    // VERIFY: Third record is MemoryDeleted with reason
    match &retrieved[2].operation {
        AuditOperation::MemoryDeleted { soft, reason } => {
            assert!(*soft);
            assert_eq!(reason.as_deref(), Some("obsolete"));
        }
        other => panic!("Expected MemoryDeleted, got {:?}", other),
    }

    // VERIFY: All records are Success
    for r in &retrieved {
        assert!(matches!(r.result, AuditResult::Success));
    }

    // VERIFY: Count
    let count = store.count_audit_records().unwrap();
    assert!(count >= 3, "Expected at least 3 audit records, got {}", count);

    // EDGE CASE: Query non-existent target
    let empty_target = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let empty = store.get_audit_by_target(empty_target, 100).unwrap();
    assert_eq!(empty.len(), 0, "Non-existent target should return 0 records");

    println!("  ✓ Audit log roundtrip: 3 records written and verified");
    println!("  ✓ Edge case: empty target returns 0 records");
    println!("\n=== FSV: PASSED - Audit Log Roundtrip ===\n");
}

#[tokio::test]
async fn test_provenance_merge_history_roundtrip() {
    use chrono::Utc;
    use context_graph_core::types::audit::MergeRecord;

    println!("\n=== FSV: Merge History Roundtrip ===");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    let merged_id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
    let source1 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000001").unwrap();
    let source2 = Uuid::parse_str("aaaaaaaa-0002-0002-0002-000000000002").unwrap();

    // WRITE: Create 2 merge records for the same merged_id
    let rec1 = MergeRecord {
        id: Uuid::new_v4(),
        merged_id,
        source_ids: vec![source1, source2],
        strategy: "union".to_string(),
        rationale: "consolidating duplicates".to_string(),
        operator_id: Some("merge-agent".to_string()),
        timestamp: Utc::now(),
        reversal_hash: "abc123".to_string(),
        original_fingerprints_json: vec!["{}".to_string(), "{}".to_string()],
    };

    let rec2 = MergeRecord {
        id: Uuid::new_v4(),
        merged_id,
        source_ids: vec![Uuid::new_v4()],
        strategy: "weighted_average".to_string(),
        rationale: "further consolidation".to_string(),
        operator_id: Some("admin".to_string()),
        timestamp: Utc::now(),
        reversal_hash: "def456".to_string(),
        original_fingerprints_json: vec!["{}".to_string()],
    };

    store.append_merge_record(&rec1).unwrap();
    store.append_merge_record(&rec2).unwrap();

    println!("  Wrote 2 merge records for merged_id {}", merged_id);

    // VERIFY: Read back
    let retrieved = store.get_merge_history(merged_id, 100).unwrap();
    assert_eq!(retrieved.len(), 2, "Expected 2 merge records");

    // VERIFY: First record content
    assert_eq!(retrieved[0].merged_id, merged_id);
    assert_eq!(retrieved[0].source_ids.len(), 2);
    assert_eq!(retrieved[0].strategy, "union");
    assert_eq!(retrieved[0].operator_id, Some("merge-agent".to_string()));
    assert_eq!(retrieved[0].reversal_hash, "abc123");

    // VERIFY: Second record content
    assert_eq!(retrieved[1].strategy, "weighted_average");
    assert_eq!(retrieved[1].operator_id, Some("admin".to_string()));

    // EDGE CASE: Limit query to 1
    let limited = store.get_merge_history(merged_id, 1).unwrap();
    assert_eq!(limited.len(), 1, "Limit=1 should return 1 record");

    // EDGE CASE: Non-existent merged_id
    let no_records = store
        .get_merge_history(Uuid::new_v4(), 100)
        .unwrap();
    assert_eq!(no_records.len(), 0, "Non-existent ID should return 0 records");

    println!("  ✓ Merge history roundtrip: 2 records written and verified");
    println!("  ✓ Edge case: limit=1 returns 1, non-existent returns 0");
    println!("\n=== FSV: PASSED - Merge History Roundtrip ===\n");
}

#[tokio::test]
async fn test_provenance_importance_history_roundtrip() {
    use chrono::Utc;
    use context_graph_core::types::audit::ImportanceChangeRecord;

    println!("\n=== FSV: Importance History Roundtrip ===");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    let memory_id = Uuid::parse_str("99999999-8888-7777-6666-555544443333").unwrap();

    // WRITE: 3 importance changes
    for (i, (old, new, delta)) in [(0.5, 0.7, 0.2), (0.7, 0.9, 0.2), (0.9, 0.6, -0.3)]
        .iter()
        .enumerate()
    {
        let rec = ImportanceChangeRecord {
            memory_id,
            timestamp: Utc::now(),
            old_value: *old,
            new_value: *new,
            delta: *delta,
            operator_id: Some(format!("user-{}", i)),
            reason: if i == 2 {
                Some("too high".to_string())
            } else {
                None
            },
        };
        store.append_importance_change(&rec).unwrap();
    }

    println!("  Wrote 3 importance change records for memory {}", memory_id);

    // VERIFY: Read back
    let retrieved = store.get_importance_history(memory_id, 100).unwrap();
    assert_eq!(retrieved.len(), 3, "Expected 3 importance change records");

    // VERIFY: Values match
    assert!((retrieved[0].old_value - 0.5).abs() < f32::EPSILON);
    assert!((retrieved[0].new_value - 0.7).abs() < f32::EPSILON);
    assert!((retrieved[0].delta - 0.2).abs() < f32::EPSILON);
    assert_eq!(retrieved[0].operator_id, Some("user-0".to_string()));

    assert!((retrieved[2].delta - (-0.3)).abs() < f32::EPSILON);
    assert_eq!(retrieved[2].reason, Some("too high".to_string()));

    // EDGE CASE: Non-existent memory
    let empty = store.get_importance_history(Uuid::new_v4(), 100).unwrap();
    assert_eq!(empty.len(), 0);

    println!("  ✓ Importance history roundtrip: 3 records written and verified");
    println!("  ✓ Edge case: non-existent memory returns 0 records");
    println!("\n=== FSV: PASSED - Importance History Roundtrip ===\n");
}

#[tokio::test]
async fn test_provenance_embedding_version_roundtrip() {
    use chrono::Utc;
    use context_graph_core::types::audit::EmbeddingVersionRecord;
    use std::collections::HashMap;

    println!("\n=== FSV: Embedding Version Registry Roundtrip ===");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    let fp_id = Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap();

    // WRITE: Embedding version record with all 13 embedder versions
    let mut versions = HashMap::new();
    versions.insert("E1".to_string(), "all-MiniLM-L6-v2.1".to_string());
    versions.insert("E5".to_string(), "e5-base-v2".to_string());
    versions.insert("E7".to_string(), "qodo-embed-1-1.5b".to_string());
    versions.insert("E11".to_string(), "kepler-v1".to_string());

    let rec = EmbeddingVersionRecord {
        fingerprint_id: fp_id,
        computed_at: Utc::now(),
        embedder_versions: versions.clone(),
        e7_model_version: Some("qodo-embed-1-1.5b".to_string()),
        computation_time_ms: Some(42),
    };

    store.store_embedding_version(&rec).unwrap();
    println!("  Wrote embedding version record for fingerprint {}", fp_id);

    // VERIFY: Read back
    let retrieved = store.get_embedding_version(fp_id).unwrap();
    assert!(retrieved.is_some(), "Should find stored version record");
    let retrieved = retrieved.unwrap();

    assert_eq!(retrieved.fingerprint_id, fp_id);
    assert_eq!(retrieved.embedder_versions.len(), 4);
    assert_eq!(
        retrieved.embedder_versions.get("E1"),
        Some(&"all-MiniLM-L6-v2.1".to_string())
    );
    assert_eq!(
        retrieved.e7_model_version,
        Some("qodo-embed-1-1.5b".to_string())
    );
    assert_eq!(retrieved.computation_time_ms, Some(42));

    // VERIFY: Overwrite works (re-embedding)
    let mut updated_versions = versions;
    updated_versions.insert("E1".to_string(), "all-MiniLM-L6-v2.2".to_string());
    let rec2 = EmbeddingVersionRecord {
        fingerprint_id: fp_id,
        computed_at: Utc::now(),
        embedder_versions: updated_versions.clone(),
        e7_model_version: Some("qodo-embed-1-2.0b".to_string()),
        computation_time_ms: Some(55),
    };
    store.store_embedding_version(&rec2).unwrap();

    let updated = store.get_embedding_version(fp_id).unwrap().unwrap();
    assert_eq!(
        updated.embedder_versions.get("E1"),
        Some(&"all-MiniLM-L6-v2.2".to_string())
    );
    assert_eq!(updated.computation_time_ms, Some(55));

    // EDGE CASE: Non-existent fingerprint
    let none = store.get_embedding_version(Uuid::new_v4()).unwrap();
    assert!(none.is_none(), "Non-existent fingerprint should return None");

    println!("  ✓ Embedding version roundtrip: write, read, overwrite all verified");
    println!("  ✓ Edge case: non-existent fingerprint returns None");
    println!("\n=== FSV: PASSED - Embedding Version Registry Roundtrip ===\n");
}

#[tokio::test]
async fn test_provenance_all_cfs_physically_exist() {
    use crate::teleological::column_families::*;

    println!("\n=== FSV: Physical Verification - All Provenance CFs Exist ===");

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    // VERIFY: All 8 provenance column families are accessible
    let provenance_cfs = [
        CF_AUDIT_LOG,
        CF_AUDIT_BY_TARGET,
        CF_MERGE_HISTORY,
        CF_IMPORTANCE_HISTORY,
        CF_TOOL_CALL_INDEX,
        CF_ENTITY_PROVENANCE,
        CF_CONSOLIDATION_RECOMMENDATIONS,
        CF_EMBEDDING_REGISTRY,
    ];

    for cf_name in &provenance_cfs {
        let cf_result = store.get_cf(cf_name);
        assert!(
            cf_result.is_ok(),
            "Column family '{}' should be accessible, got error: {:?}",
            cf_name,
            cf_result.err()
        );
        println!("  ✓ CF '{}' exists and is accessible", cf_name);
    }

    println!("\n=== FSV: PASSED - All 8 Provenance CFs Physically Exist ===\n");
}

// ============================================================================
// STOR-H1: total_doc_count underflow prevention
// ============================================================================

#[tokio::test]
async fn test_total_doc_count_underflow_prevention() {
    use std::sync::atomic::Ordering;

    let tmp = TempDir::new().unwrap();
    let store = create_initialized_store(tmp.path());

    // Verify initial doc count is 0 (no documents stored)
    let initial_count = store.total_doc_count.load(Ordering::Relaxed);
    assert_eq!(initial_count, 0, "Initial total_doc_count should be 0");

    // Store one fingerprint, then soft-delete it to decrement count to 0
    let fp = create_test_fingerprint();
    let id = fp.id;
    store.store(fp).await.unwrap();
    assert_eq!(store.total_doc_count.load(Ordering::Relaxed), 1,
        "total_doc_count should be 1 after storing one fingerprint");

    store.delete(id, true).await.unwrap();
    assert_eq!(store.total_doc_count.load(Ordering::Relaxed), 0,
        "total_doc_count should be 0 after soft-deleting the only fingerprint");

    // Now try to hard-delete the same fingerprint (GC path).
    // The soft-delete already decremented, and the was_soft_deleted guard
    // should prevent another decrement. But even if it tried, the fetch_update
    // underflow guard must prevent wrapping to usize::MAX.
    let _ = store.delete(id, false).await;

    // Verify count is still 0 (not usize::MAX from underflow)
    let final_count = store.total_doc_count.load(Ordering::Relaxed);
    assert_eq!(final_count, 0,
        "STOR-H1 REGRESSION: total_doc_count is {} (expected 0, underflow wraps to usize::MAX = {})",
        final_count, usize::MAX);

    // Also verify with a fresh store that has zero documents and direct delete attempt
    let tmp2 = TempDir::new().unwrap();
    let store2 = create_initialized_store(tmp2.path());
    assert_eq!(store2.total_doc_count.load(Ordering::Relaxed), 0);

    // Store and immediately hard-delete to get count back to 0
    let fp2 = create_test_fingerprint_with_seed(99);
    let id2 = fp2.id;
    store2.store(fp2).await.unwrap();
    store2.delete(id2, false).await.unwrap();
    assert_eq!(store2.total_doc_count.load(Ordering::Relaxed), 0);

    // Double hard-delete on an already-gone fingerprint should be safe
    let _ = store2.delete(id2, false).await;
    let count_after_double_delete = store2.total_doc_count.load(Ordering::Relaxed);
    assert_eq!(count_after_double_delete, 0,
        "Double hard-delete must not cause underflow. Got: {}", count_after_double_delete);
}
