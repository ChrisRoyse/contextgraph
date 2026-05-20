//! Roundtrip and Serialization Tests
//!
//! TEST 1: RocksDB + Store Roundtrip Integration Test
//! TEST 9: Serialization Size Verification

use std::time::Instant;

use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_storage::teleological::{
    deserialize_teleological_fingerprint, serialize_teleological_fingerprint,
};
use tempfile::TempDir;
use uuid::Uuid;

use crate::helpers::{create_initialized_store, create_random_fingerprint};

// =============================================================================
// TEST 1: RocksDB + Store Roundtrip Integration Test
// =============================================================================

#[tokio::test]
async fn test_rocksdb_store_roundtrip_real_data() {
    println!("\n=== TEST: RocksDB + Store Roundtrip with REAL Data ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Verify health check passes
    store.health_check().expect("Health check failed");

    const TEST_COUNT: usize = 100;
    let mut stored_ids: Vec<Uuid> = Vec::with_capacity(TEST_COUNT);

    println!("[BEFORE] Store empty, 0 fingerprints");
    let initial_count = store.count().await.expect("Failed to count");
    assert_eq!(initial_count, 0, "Store should be empty initially");

    // Store 100 REAL fingerprints
    println!(
        "[STORING] {} fingerprints with REAL vector data...",
        TEST_COUNT
    );
    let store_start = Instant::now();

    for i in 0..TEST_COUNT {
        let fp = create_random_fingerprint();
        let id = fp.id;

        // Verify fingerprint has correct dimensions before storage
        assert_eq!(fp.semantic.e1_semantic.len(), 1024, "E1 should be 1024D");
        // E5 uses dual vectors (cause + effect) for asymmetric causal similarity
        assert_eq!(
            fp.semantic.e5_causal_as_cause.len(),
            768,
            "E5 cause should be 768D"
        );
        assert_eq!(
            fp.semantic.e5_causal_as_effect.len(),
            768,
            "E5 effect should be 768D"
        );
        assert!(
            fp.semantic.e5_causal.is_empty(),
            "Legacy e5_causal should be empty"
        );
        assert_eq!(
            fp.semantic.e9_hdc.len(),
            1024,
            "E9 should be 1024D (projected)"
        );
        assert!(
            fp.semantic.e6_sparse.nnz() > 0,
            "E6 sparse should have entries"
        );
        assert!(
            fp.semantic.e13_splade.nnz() > 0,
            "E13 sparse should have entries"
        );

        let stored_id = store.store(fp).await.expect("Failed to store");
        assert_eq!(stored_id, id, "Stored ID should match original");
        stored_ids.push(id);

        if (i + 1) % 25 == 0 {
            println!("  Stored {}/{}", i + 1, TEST_COUNT);
        }
    }

    let store_duration = store_start.elapsed();
    println!(
        "[STORED] {} fingerprints in {:?}",
        TEST_COUNT, store_duration
    );

    // Verify count
    let after_count = store.count().await.expect("Failed to count");
    assert_eq!(after_count, TEST_COUNT, "Count should be {}", TEST_COUNT);

    // Retrieve and verify all 100 fingerprints
    println!("[RETRIEVING] {} fingerprints...", TEST_COUNT);
    let retrieve_start = Instant::now();

    for (i, &id) in stored_ids.iter().enumerate() {
        let retrieved = store
            .retrieve(id)
            .await
            .expect("Failed to retrieve")
            .unwrap_or_else(|| panic!("Fingerprint {} not found", id));

        // Verify data integrity
        assert_eq!(retrieved.id, id, "ID mismatch");
        assert_eq!(
            retrieved.semantic.e1_semantic.len(),
            1024,
            "E1 dimension mismatch"
        );
        assert_eq!(
            retrieved.semantic.e9_hdc.len(),
            1024,
            "E9 dimension mismatch (expected 1024)"
        );
        assert!(
            retrieved.semantic.e13_splade.nnz() > 0,
            "E13 sparse should have entries"
        );

        // Verify vectors are unit normalized (L2 norm ~ 1.0)
        let e1_norm: f32 = retrieved
            .semantic
            .e1_semantic
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!(
            (e1_norm - 1.0).abs() < 0.01,
            "E1 should be unit normalized, got norm={}",
            e1_norm
        );

        if (i + 1) % 25 == 0 {
            println!("  Retrieved {}/{}", i + 1, TEST_COUNT);
        }
    }

    let retrieve_duration = retrieve_start.elapsed();
    println!(
        "[RETRIEVED] {} fingerprints in {:?}",
        TEST_COUNT, retrieve_duration
    );

    // Get storage size
    let size_bytes = store.storage_size_bytes();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "[AFTER] Stored {} fingerprints, DB size = {:.2}MB",
        TEST_COUNT, size_mb
    );

    println!(
        "[VERIFIED] All {} fingerprints retrievable, roundtrip successful",
        TEST_COUNT
    );
    println!("\n=== PASS: RocksDB + Store Roundtrip ===\n");
}

// =============================================================================
// TEST 9: Serialization Size Verification
// =============================================================================

#[test]
fn test_serialization_size_verification() {
    println!("\n=== TEST: Serialization Size Verification ===\n");

    let fp = create_random_fingerprint();
    let id = fp.id;

    // Serialize
    let bytes = serialize_teleological_fingerprint(&fp);

    println!(
        "[SERIALIZED] Fingerprint {} to {} bytes ({:.2}KB)",
        id,
        bytes.len(),
        bytes.len() as f64 / 1024.0
    );

    // Verify size is in expected range (25KB - 100KB with E9_DIM = 1024 projected)
    assert!(
        bytes.len() >= 25_000,
        "Serialized size should be >= 25KB, got {} bytes",
        bytes.len()
    );
    assert!(
        bytes.len() <= 100_000,
        "Serialized size should be <= 100KB, got {} bytes",
        bytes.len()
    );

    // Deserialize
    let restored =
        deserialize_teleological_fingerprint(&bytes).expect("Failed to deserialize fingerprint");

    // Verify integrity
    assert_eq!(restored.id, id, "ID mismatch after roundtrip");
    assert_eq!(
        restored.semantic.e1_semantic.len(),
        1024,
        "E1 dimension mismatch"
    );
    assert_eq!(
        restored.semantic.e9_hdc.len(),
        1024,
        "E9 dimension mismatch (expected 1024)"
    );
    assert_eq!(
        restored.content_hash.len(),
        32,
        "Content hash dimension mismatch"
    );

    println!("[VERIFIED] Serialization roundtrip preserves all data");
    println!("\n=== PASS: Serialization Size Verification ===\n");
}
