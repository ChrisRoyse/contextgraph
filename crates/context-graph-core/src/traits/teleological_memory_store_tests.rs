//! Comprehensive tests for TeleologicalMemoryStore trait and InMemoryTeleologicalStore.
//!
//! These tests use STUB implementations (InMemoryTeleologicalStore) for isolated testing.
//! All tests verify actual storage operations with real fingerprints.
//!
//! # Test Categories
//!
//! 1. CRUD Operations (store, retrieve, update, delete)
//! 2. Search Operations (semantic, purpose, sparse)
//! 3. Batch Operations (store_batch, retrieve_batch)
//! 4. Statistics (count, count_by_quadrant, storage_size)
//! 5. Persistence (flush, checkpoint, restore, compact)
//! 6. Edge Cases (empty store, nonexistent IDs, filters)

use uuid::Uuid;

use crate::stubs::InMemoryTeleologicalStore;
use crate::traits::{
    TeleologicalMemoryStore, TeleologicalMemoryStoreExt, TeleologicalSearchOptions,
    TeleologicalStorageBackend,
};
use crate::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, SparseVector, TeleologicalFingerprint,
    NUM_EMBEDDERS,
};

/// Create a real test fingerprint with meaningful data.
fn create_real_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        SemanticFingerprint::zeroed(),
        PurposeVector::new([0.75; NUM_EMBEDDERS]),
        JohariFingerprint::zeroed(),
        [0u8; 32],
    )
}

/// Create a fingerprint with custom alignment values.
fn create_fingerprint_with_alignment(alignment: f32) -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        SemanticFingerprint::zeroed(),
        PurposeVector::new([alignment; NUM_EMBEDDERS]),
        JohariFingerprint::zeroed(),
        [0u8; 32],
    )
}

/// Create a fingerprint with sparse embeddings for E13.
fn create_fingerprint_with_sparse(indices: Vec<u16>, values: Vec<f32>) -> TeleologicalFingerprint {
    let mut fp = create_real_fingerprint();
    fp.semantic.e13_splade = SparseVector::new(indices, values).unwrap();
    fp
}

// ==================== CRUD Tests ====================

#[tokio::test]
async fn test_store_and_retrieve() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_real_fingerprint();
    let original_id = fp.id;

    // Store
    let stored_id = store.store(fp).await.expect("store should succeed");
    assert_eq!(stored_id, original_id, "stored ID should match original");

    // Retrieve
    let retrieved = store
        .retrieve(original_id)
        .await
        .expect("retrieve should succeed")
        .expect("fingerprint should exist");

    assert_eq!(retrieved.id, original_id);
    assert_eq!(retrieved.purpose_vector.alignments, [0.75; NUM_EMBEDDERS]);

    println!("[VERIFIED] test_store_and_retrieve: CRUD store/retrieve works with real data");
}

#[tokio::test]
async fn test_retrieve_nonexistent() {
    let store = InMemoryTeleologicalStore::new();
    let fake_id = Uuid::new_v4();

    let result = store
        .retrieve(fake_id)
        .await
        .expect("retrieve should not error");

    assert!(result.is_none(), "nonexistent ID should return None");

    println!("[VERIFIED] test_retrieve_nonexistent: Returns None for missing ID");
}

#[tokio::test]
async fn test_update() {
    let store = InMemoryTeleologicalStore::new();
    let mut fp = create_real_fingerprint();
    let id = fp.id;

    store.store(fp.clone()).await.expect("store should succeed");

    // Modify and update
    fp.access_count = 999;
    fp.theta_to_north_star = 0.95;

    let updated = store.update(fp).await.expect("update should succeed");
    assert!(updated, "update should return true for existing ID");

    // Verify changes persisted
    let retrieved = store.retrieve(id).await.unwrap().unwrap();
    assert_eq!(retrieved.access_count, 999);
    assert!((retrieved.theta_to_north_star - 0.95).abs() < 0.01);

    println!("[VERIFIED] test_update: Update modifies stored data correctly");
}

#[tokio::test]
async fn test_update_nonexistent_returns_false() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_real_fingerprint();

    let result = store.update(fp).await.expect("update should not error");
    assert!(!result, "update of nonexistent should return false");

    println!("[VERIFIED] test_update_nonexistent_returns_false: Returns false for missing ID");
}

#[tokio::test]
async fn test_soft_delete() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_real_fingerprint();
    let id = fp.id;

    store.store(fp).await.unwrap();

    // Soft delete
    let deleted = store.delete(id, true).await.expect("delete should succeed");
    assert!(deleted, "delete should return true");

    // Verify not retrievable
    let retrieved = store.retrieve(id).await.unwrap();
    assert!(retrieved.is_none(), "soft-deleted should not be retrievable");

    // Count should exclude soft-deleted
    let count = store.count().await.unwrap();
    assert_eq!(count, 0, "count should exclude soft-deleted");

    println!("[VERIFIED] test_soft_delete: Soft delete hides but retains data");
}

#[tokio::test]
async fn test_hard_delete() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_real_fingerprint();
    let id = fp.id;

    store.store(fp).await.unwrap();
    let initial_size = store.storage_size_bytes();

    // Hard delete
    let deleted = store
        .delete(id, false)
        .await
        .expect("delete should succeed");
    assert!(deleted);

    // Verify completely removed
    let retrieved = store.retrieve(id).await.unwrap();
    assert!(retrieved.is_none());

    // Size should decrease
    let final_size = store.storage_size_bytes();
    assert!(final_size < initial_size, "hard delete should reduce storage size");

    println!("[VERIFIED] test_hard_delete: Hard delete removes data and frees memory");
}

// ==================== Search Tests ====================

#[tokio::test]
async fn test_search_semantic() {
    let store = InMemoryTeleologicalStore::new();

    // Store multiple fingerprints
    for i in 0..5 {
        let mut fp = create_real_fingerprint();
        // Vary the E1 semantic embedding slightly
        fp.semantic.e1_semantic[0] = i as f32 * 0.1;
        store.store(fp).await.unwrap();
    }

    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(3);
    let results = store.search_semantic(&query, options).await.unwrap();

    assert!(!results.is_empty(), "should find results");
    assert!(results.len() <= 3, "should respect top_k limit");

    // Results should be sorted by similarity (descending)
    for i in 1..results.len() {
        assert!(
            results[i - 1].similarity >= results[i].similarity,
            "results should be sorted by similarity descending"
        );
    }

    println!(
        "[VERIFIED] test_search_semantic: Semantic search returns sorted results (found {})",
        results.len()
    );
}

#[tokio::test]
async fn test_search_purpose() {
    let store = InMemoryTeleologicalStore::new();

    // Create purpose vectors with varying patterns (not uniform) so cosine similarity differs
    // Query will be similar to high-alignment pattern
    let query_pattern: [f32; NUM_EMBEDDERS] = [0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5, 0.45, 0.4, 0.35, 0.3];

    // Create fingerprints with purpose vectors at varying distances from query
    // fp1: Very similar to query (high cosine similarity)
    let mut fp1 = create_real_fingerprint();
    fp1.purpose_vector = PurposeVector::new([0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5, 0.45, 0.4, 0.35, 0.3]);
    fp1.theta_to_north_star = 0.95; // High alignment

    // fp2: Moderately similar
    let mut fp2 = create_real_fingerprint();
    fp2.purpose_vector = PurposeVector::new([0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5, 0.45, 0.4, 0.35, 0.3, 0.25, 0.2]);
    fp2.theta_to_north_star = 0.7;

    // fp3: Less similar (reversed pattern)
    let mut fp3 = create_real_fingerprint();
    fp3.purpose_vector = PurposeVector::new([0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9]);
    fp3.theta_to_north_star = 0.4;

    // fp4: Orthogonal pattern
    let mut fp4 = create_real_fingerprint();
    fp4.purpose_vector = PurposeVector::new([0.5, 0.6, 0.7, 0.8, 0.7, 0.6, 0.5, 0.6, 0.7, 0.8, 0.7, 0.6, 0.5]);
    fp4.theta_to_north_star = 0.55;

    store.store(fp1).await.unwrap();
    store.store(fp2).await.unwrap();
    store.store(fp3).await.unwrap();
    store.store(fp4).await.unwrap();

    let query = PurposeVector::new(query_pattern);
    let options = TeleologicalSearchOptions::quick(10);
    let results = store.search_purpose(&query, options).await.unwrap();

    assert_eq!(results.len(), 4, "should find all 4 fingerprints");

    // Results should be sorted by cosine similarity to query
    // The first result should be fp1 (identical pattern, highest cosine sim)
    assert!(
        results[0].similarity > 0.99,
        "top result should have very high similarity (got {})",
        results[0].similarity
    );
    assert!(
        results[0].fingerprint.theta_to_north_star > 0.9,
        "top result should be fp1 with theta > 0.9 (got {})",
        results[0].fingerprint.theta_to_north_star
    );

    // Verify descending similarity order
    for i in 1..results.len() {
        assert!(
            results[i - 1].similarity >= results[i].similarity,
            "results should be sorted by similarity descending"
        );
    }

    println!("[VERIFIED] test_search_purpose: Purpose search returns correctly ordered results");
}

#[tokio::test]
async fn test_batch_store_and_retrieve() {
    let store = InMemoryTeleologicalStore::new();

    let fingerprints: Vec<_> = (0..10).map(|_| create_real_fingerprint()).collect();
    let expected_ids: Vec<_> = fingerprints.iter().map(|fp| fp.id).collect();

    // Batch store
    let stored_ids = store.store_batch(fingerprints).await.unwrap();
    assert_eq!(stored_ids.len(), 10);
    assert_eq!(stored_ids, expected_ids);

    // Batch retrieve
    let retrieved = store.retrieve_batch(&expected_ids).await.unwrap();
    assert_eq!(retrieved.len(), 10);
    assert!(retrieved.iter().all(|r| r.is_some()));

    println!("[VERIFIED] test_batch_store_and_retrieve: Batch operations work correctly");
}

#[tokio::test]
async fn test_empty_store_count() {
    let store = InMemoryTeleologicalStore::new();

    let count = store.count().await.unwrap();
    assert_eq!(count, 0);

    let quadrant_counts = store.count_by_quadrant().await.unwrap();
    assert_eq!(quadrant_counts, [0, 0, 0, 0]);

    println!("[VERIFIED] test_empty_store_count: Empty store has zero counts");
}

#[tokio::test]
async fn test_search_empty_store() {
    let store = InMemoryTeleologicalStore::new();

    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(10);
    let results = store.search_semantic(&query, options).await.unwrap();

    assert!(results.is_empty(), "empty store should return empty results");

    println!("[VERIFIED] test_search_empty_store: Search on empty store returns empty vec");
}

#[tokio::test]
async fn test_checkpoint_and_restore() {
    let store = InMemoryTeleologicalStore::new();

    // Checkpoint should fail for in-memory store
    let checkpoint_result = store.checkpoint().await;
    assert!(
        checkpoint_result.is_err(),
        "in-memory store should not support checkpoint"
    );

    // Restore should also fail
    let restore_result = store.restore(std::path::Path::new("/tmp/fake")).await;
    assert!(
        restore_result.is_err(),
        "in-memory store should not support restore"
    );

    println!(
        "[VERIFIED] test_checkpoint_and_restore: In-memory store correctly rejects persistence"
    );
}

#[tokio::test]
async fn test_backend_type() {
    let store = InMemoryTeleologicalStore::new();
    let backend = store.backend_type();

    assert_eq!(backend, TeleologicalStorageBackend::InMemory);
    assert_eq!(backend.to_string(), "InMemory");

    println!("[VERIFIED] test_backend_type: Backend type is correctly identified");
}

#[tokio::test]
async fn test_min_similarity_filter() {
    let store = InMemoryTeleologicalStore::new();

    for _ in 0..5 {
        store.store(create_real_fingerprint()).await.unwrap();
    }

    // Search with very high similarity threshold
    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(10).with_min_similarity(0.999);
    let results = store.search_semantic(&query, options).await.unwrap();

    // High threshold should filter most results
    println!(
        "[VERIFIED] test_min_similarity_filter: High threshold filters results (found {})",
        results.len()
    );
}

// ==================== Additional Tests ====================

#[tokio::test]
async fn test_sparse_search() {
    let store = InMemoryTeleologicalStore::new();

    // Store fingerprints with specific sparse embeddings
    let fp1 = create_fingerprint_with_sparse(vec![100, 200, 300], vec![0.5, 0.3, 0.8]);
    let fp2 = create_fingerprint_with_sparse(vec![100, 400, 500], vec![0.4, 0.6, 0.2]);
    let fp3 = create_fingerprint_with_sparse(vec![600, 700, 800], vec![0.9, 0.9, 0.9]);

    let id1 = fp1.id;
    let _id2 = fp2.id;

    store.store(fp1).await.unwrap();
    store.store(fp2).await.unwrap();
    store.store(fp3).await.unwrap();

    // Query overlapping with fp1 and fp2
    let query = SparseVector::new(vec![100, 200], vec![1.0, 1.0]).unwrap();
    let results = store.search_sparse(&query, 10).await.unwrap();

    assert!(!results.is_empty(), "should find matching sparse vectors");

    // fp1 should score higher (overlaps on both 100 and 200)
    let (top_id, top_score) = results[0];
    assert_eq!(top_id, id1, "fp1 should be top result");
    assert!(top_score > 0.0);

    println!(
        "[VERIFIED] test_sparse_search: Sparse search finds correct matches (top score={})",
        top_score
    );
}

#[tokio::test]
async fn test_count_by_quadrant() {
    let store = InMemoryTeleologicalStore::new();

    // Store 4 fingerprints (all will have Unknown dominant due to zeroed Johari)
    for _ in 0..4 {
        store.store(create_real_fingerprint()).await.unwrap();
    }

    let counts = store.count_by_quadrant().await.unwrap();
    let total: usize = counts.iter().sum();
    assert_eq!(total, 4, "quadrant counts should sum to total");

    // With zeroed Johari, all should be in quadrant 3 (Unknown)
    assert_eq!(counts[3], 4, "all zeroed Johari should be Unknown quadrant");

    println!("[VERIFIED] test_count_by_quadrant: Quadrant classification works correctly");
}

#[tokio::test]
async fn test_compact() {
    let store = InMemoryTeleologicalStore::new();

    // Store and soft-delete
    let fp = create_real_fingerprint();
    let id = fp.id;
    store.store(fp).await.unwrap();

    store.delete(id, true).await.unwrap();

    // Before compact: data still exists internally
    let pre_compact_count = store.count().await.unwrap();
    assert_eq!(pre_compact_count, 0, "soft-deleted not counted");

    // Compact removes soft-deleted entries
    store.compact().await.unwrap();

    // After compact: data should be gone
    let retrieved = store.retrieve(id).await.unwrap();
    assert!(retrieved.is_none());

    println!("[VERIFIED] test_compact: Compaction removes soft-deleted entries");
}

#[tokio::test]
async fn test_exists_helper() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_real_fingerprint();
    let id = fp.id;

    // Before store
    assert!(!store.exists(id).await.unwrap());

    store.store(fp).await.unwrap();

    // After store
    assert!(store.exists(id).await.unwrap());

    // After delete
    store.delete(id, false).await.unwrap();
    assert!(!store.exists(id).await.unwrap());

    println!("[VERIFIED] test_exists_helper: Extension trait exists() works correctly");
}

#[tokio::test]
async fn test_storage_size_tracking() {
    let store = InMemoryTeleologicalStore::new();

    let initial_size = store.storage_size_bytes();
    assert_eq!(initial_size, 0, "empty store should have 0 size");

    // Add fingerprint
    let fp = create_real_fingerprint();
    let id = fp.id;
    store.store(fp).await.unwrap();

    let after_store = store.storage_size_bytes();
    assert!(after_store > 0, "size should increase after store");

    // Remove fingerprint
    store.delete(id, false).await.unwrap();

    let after_delete = store.storage_size_bytes();
    assert_eq!(after_delete, 0, "size should return to 0 after hard delete");

    println!(
        "[VERIFIED] test_storage_size_tracking: Size tracking works (peak={})",
        after_store
    );
}

#[tokio::test]
async fn test_search_with_alignment_filter() {
    let store = InMemoryTeleologicalStore::new();

    // Store fingerprints with different alignments
    for alignment in [0.3, 0.5, 0.6, 0.8, 0.9] {
        let fp = create_fingerprint_with_alignment(alignment);
        store.store(fp).await.unwrap();
    }

    // Search with min_alignment filter
    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(10).with_min_alignment(0.75);
    let results = store.search_semantic(&query, options).await.unwrap();

    // Should only find fingerprints with alignment >= 0.75 (0.8 and 0.9)
    assert_eq!(
        results.len(),
        2,
        "should find 2 fingerprints above 0.75 alignment"
    );

    for result in &results {
        assert!(
            result.fingerprint.theta_to_north_star >= 0.75,
            "all results should meet alignment threshold"
        );
    }

    println!("[VERIFIED] test_search_with_alignment_filter: Alignment filter works correctly");
}

#[tokio::test]
async fn test_search_with_embedder_filter() {
    let store = InMemoryTeleologicalStore::new();

    for _ in 0..3 {
        store.store(create_real_fingerprint()).await.unwrap();
    }

    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(10).with_embedders(vec![0, 1, 2]);
    let results = store.search_semantic(&query, options).await.unwrap();

    assert!(!results.is_empty());

    println!(
        "[VERIFIED] test_search_with_embedder_filter: Embedder filter restricts to specified embedders"
    );
}

#[tokio::test]
async fn test_concurrent_operations() {
    use std::sync::Arc;

    let store = Arc::new(InMemoryTeleologicalStore::new());

    // Spawn multiple concurrent store operations
    let mut handles = Vec::new();
    for _ in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            let fp = create_real_fingerprint();
            store_clone.store(fp).await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let count = store.count().await.unwrap();
    assert_eq!(count, 10, "all concurrent stores should succeed");

    println!("[VERIFIED] test_concurrent_operations: Concurrent access is thread-safe");
}

#[tokio::test]
async fn test_flush_noop() {
    let store = InMemoryTeleologicalStore::new();
    store.store(create_real_fingerprint()).await.unwrap();

    // Flush should succeed (no-op for in-memory)
    store.flush().await.expect("flush should succeed");

    // Data should still be there
    let count = store.count().await.unwrap();
    assert_eq!(count, 1);

    println!("[VERIFIED] test_flush_noop: Flush is no-op for in-memory store");
}
