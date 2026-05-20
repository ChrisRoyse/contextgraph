//! Performance Tests
//!
//! TEST 5: Batch Operation Performance Test
//! TEST 8: Concurrent Access Test

use std::time::Instant;

use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use tempfile::TempDir;
use uuid::Uuid;

use crate::helpers::{create_initialized_store, create_random_fingerprint};

// =============================================================================
// TEST 5: Batch Operation Performance Test
// =============================================================================

#[tokio::test]
async fn test_batch_store_retrieve_performance() {
    println!("\n=== TEST: Batch Store/Retrieve Performance ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    const BATCH_SIZE: usize = 1000;

    // Generate all fingerprints first (exclude from timing)
    println!("[GENERATING] {} fingerprints...", BATCH_SIZE);
    let generate_start = Instant::now();
    let fingerprints: Vec<TeleologicalFingerprint> = (0..BATCH_SIZE)
        .map(|_| create_random_fingerprint())
        .collect();
    let generate_duration = generate_start.elapsed();
    println!(
        "[GENERATED] {} fingerprints in {:?}",
        BATCH_SIZE, generate_duration
    );

    let ids: Vec<Uuid> = fingerprints.iter().map(|fp| fp.id).collect();

    // Time batch store
    println!(
        "[BEFORE] Store empty, starting batch store of {}",
        BATCH_SIZE
    );
    let store_start = Instant::now();

    let stored_ids = store
        .store_batch(fingerprints)
        .await
        .expect("Batch store failed");

    let store_duration = store_start.elapsed();
    let store_ms = store_duration.as_millis();

    println!(
        "[STORED] {} fingerprints in {:?} ({:.2} fps)",
        BATCH_SIZE,
        store_duration,
        BATCH_SIZE as f64 / store_duration.as_secs_f64()
    );

    assert_eq!(
        stored_ids.len(),
        BATCH_SIZE,
        "Should store all fingerprints"
    );
    // Debug builds are ~3-5x slower than release; use generous limit
    assert!(
        store_ms < 30_000,
        "Batch store should complete in <30s, took {}ms",
        store_ms
    );

    // Time batch retrieve
    let retrieve_start = Instant::now();

    let retrieved = store
        .retrieve_batch(&ids)
        .await
        .expect("Batch retrieve failed");

    let retrieve_duration = retrieve_start.elapsed();
    let retrieve_ms = retrieve_duration.as_millis();

    println!(
        "[RETRIEVED] {} fingerprints in {:?} ({:.2} fps)",
        BATCH_SIZE,
        retrieve_duration,
        BATCH_SIZE as f64 / retrieve_duration.as_secs_f64()
    );

    assert_eq!(
        retrieved.len(),
        BATCH_SIZE,
        "Should retrieve all fingerprints"
    );
    assert!(
        retrieve_ms < 5_000,
        "Batch retrieve should complete in <5s, took {}ms",
        retrieve_ms
    );

    // Verify all retrieved successfully
    let successful = retrieved.iter().filter(|opt| opt.is_some()).count();
    assert_eq!(
        successful, BATCH_SIZE,
        "All fingerprints should be retrievable"
    );

    // Verify data integrity on sample
    for (i, opt) in retrieved.iter().enumerate().take(10) {
        let fp = opt
            .as_ref()
            .unwrap_or_else(|| panic!("Fingerprint {} missing", i));
        assert_eq!(fp.semantic.e1_semantic.len(), 1024, "E1 dimension mismatch");
    }

    // Get final stats
    let count = store.count().await.expect("Count failed");
    let size_bytes = store.storage_size_bytes();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

    println!(
        "[AFTER] Stored {} fingerprints, DB size = {:.2}MB",
        count, size_mb
    );
    println!(
        "[VERIFIED] All {} fingerprints stored and retrievable",
        BATCH_SIZE
    );
    println!("\n=== PASS: Batch Store/Retrieve Performance ===\n");
}

// =============================================================================
// TEST 8: Concurrent Access Test
// =============================================================================

#[tokio::test]
async fn test_concurrent_access() {
    println!("\n=== TEST: Concurrent Access ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = std::sync::Arc::new(create_initialized_store(temp_dir.path()));

    const CONCURRENT_OPS: usize = 100;
    let mut handles = Vec::with_capacity(CONCURRENT_OPS);

    println!(
        "[STARTING] {} concurrent store operations...",
        CONCURRENT_OPS
    );

    for _ in 0..CONCURRENT_OPS {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            let fp = create_random_fingerprint();
            let id = fp.id;
            store_clone
                .store(fp)
                .await
                .expect("Concurrent store failed");
            id
        });
        handles.push(handle);
    }

    // Wait for all stores to complete
    let stored_ids: Vec<Uuid> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.expect("Task panicked"))
        .collect();

    println!("[STORED] {} fingerprints concurrently", stored_ids.len());
    assert_eq!(stored_ids.len(), CONCURRENT_OPS);

    // Verify all stored successfully
    let count = store.count().await.expect("Count failed");
    assert_eq!(
        count, CONCURRENT_OPS,
        "All concurrent stores should succeed"
    );

    // Concurrent retrieves
    let mut retrieve_handles = Vec::with_capacity(CONCURRENT_OPS);
    for &id in &stored_ids {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            store_clone
                .retrieve(id)
                .await
                .expect("Concurrent retrieve failed")
                .is_some()
        });
        retrieve_handles.push(handle);
    }

    let results: Vec<bool> = futures::future::join_all(retrieve_handles)
        .await
        .into_iter()
        .map(|r| r.expect("Task panicked"))
        .collect();

    let all_found = results.iter().all(|&found| found);
    assert!(all_found, "All concurrent retrieves should find data");

    println!(
        "[VERIFIED] {} concurrent operations completed successfully",
        CONCURRENT_OPS * 2
    );
    println!("\n=== PASS: Concurrent Access ===\n");
}
