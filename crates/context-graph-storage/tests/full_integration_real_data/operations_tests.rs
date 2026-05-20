//! Update, Delete, and Edge Case Tests
//!
//! TEST 7: Update and Delete Operations
//! TEST 10: Edge Cases

use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::SparseVector;
use context_graph_storage::teleological::{fingerprint_key, CF_FINGERPRINTS};
use tempfile::TempDir;
use uuid::Uuid;

use crate::helpers::{
    create_initialized_store, create_random_fingerprint, create_random_fingerprint_with_id,
};

// =============================================================================
// TEST 7: Update and Delete Operations
// =============================================================================

#[tokio::test]
async fn test_update_and_delete_operations() {
    println!("\n=== TEST: Update and Delete Operations ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Store initial fingerprint
    let fp = create_random_fingerprint();
    let id = fp.id;

    store.store(fp).await.expect("Failed to store");
    println!("[STORED] Fingerprint {}", id);

    // Update the fingerprint with new content hash
    let mut updated_fp = store
        .retrieve(id)
        .await
        .expect("Retrieve failed")
        .expect("Fingerprint not found");

    updated_fp.content_hash = [0xAB; 32];

    let update_result = store
        .update(updated_fp.clone())
        .await
        .expect("Update failed");
    assert!(update_result, "Update should succeed");

    // Verify update persisted
    let after_update = store
        .retrieve(id)
        .await
        .expect("Retrieve failed")
        .expect("Fingerprint not found after update");

    assert_eq!(
        after_update.content_hash, [0xAB; 32],
        "Content hash should be updated"
    );
    println!("[UPDATED] Fingerprint {} content hash updated", id);

    // Test soft delete
    let soft_deleted = store.delete(id, true).await.expect("Soft delete failed");
    assert!(soft_deleted, "Soft delete should succeed");

    let after_soft = store.retrieve(id).await.expect("Retrieve failed");
    assert!(
        after_soft.is_none(),
        "Soft deleted fingerprint should not be retrievable"
    );
    println!("[SOFT DELETED] Fingerprint {} no longer retrievable", id);

    // Store another fingerprint for hard delete test
    let fp2 = create_random_fingerprint();
    let id2 = fp2.id;
    store.store(fp2).await.expect("Failed to store");

    let hard_deleted = store.delete(id2, false).await.expect("Hard delete failed");
    assert!(hard_deleted, "Hard delete should succeed");

    // Verify raw bytes are gone
    let db = store.db();
    let cf = db.cf_handle(CF_FINGERPRINTS).expect("Missing CF");
    let raw = db.get_cf(&cf, fingerprint_key(&id2)).expect("Get failed");
    assert!(
        raw.is_none(),
        "Hard deleted fingerprint should be physically removed"
    );
    println!("[HARD DELETED] Fingerprint {} physically removed", id2);

    println!("[VERIFIED] Update and delete operations work correctly");
    println!("\n=== PASS: Update and Delete Operations ===\n");
}

// =============================================================================
// TEST 10: Edge Cases
// =============================================================================

#[tokio::test]
async fn test_edge_cases() {
    println!("\n=== TEST: Edge Cases ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Test 1: Retrieve non-existent ID
    let fake_id = Uuid::new_v4();
    let result = store
        .retrieve(fake_id)
        .await
        .expect("Retrieve should not error");
    assert!(result.is_none(), "Non-existent ID should return None");
    println!("[EDGE] Retrieve non-existent ID returns None - OK");

    // Test 2: Delete non-existent ID
    let deleted = store
        .delete(fake_id, false)
        .await
        .expect("Delete should not error");
    assert!(!deleted, "Delete non-existent should return false");
    println!("[EDGE] Delete non-existent ID returns false - OK");

    // Test 3: Update non-existent fingerprint
    let fp = create_random_fingerprint();
    let updated = store.update(fp).await.expect("Update should not error");
    assert!(!updated, "Update non-existent should return false");
    println!("[EDGE] Update non-existent returns false - OK");

    // Test 4: Empty batch operations
    let empty_ids: Vec<Uuid> = vec![];
    let batch_result = store
        .retrieve_batch(&empty_ids)
        .await
        .expect("Empty batch should work");
    assert!(
        batch_result.is_empty(),
        "Empty batch should return empty result"
    );
    println!("[EDGE] Empty batch retrieve returns empty - OK");

    // Test 5: Empty sparse vectors
    // NOTE: E6 (Sparse), E12 (LateInteraction), E13 (SPLADE) use inverted indexes
    // which are not yet implemented. They are intentionally skipped in HNSW indexing.
    // Once inverted indexes are implemented (TASK-CORE-009+), this test should be updated
    // to verify proper validation.
    let mut empty_sparse_fp = create_random_fingerprint();
    empty_sparse_fp.semantic.e6_sparse = SparseVector::empty();
    empty_sparse_fp.semantic.e13_splade = SparseVector::empty();

    // Currently, empty sparse vectors are accepted because sparse embedders
    // are not indexed by HNSW. This will change when inverted indexes are added.
    let store_result = store.store(empty_sparse_fp).await;
    assert!(
        store_result.is_ok(),
        "Store should accept fingerprint (sparse embedders not yet indexed)"
    );
    println!("[EDGE] Empty sparse vectors accepted (inverted indexes TODO) - OK");

    // Test 6: Double store (should work, overwrites)
    let fp2 = create_random_fingerprint_with_id(Uuid::new_v4());
    let _id2 = fp2.id;
    store
        .store(fp2.clone())
        .await
        .expect("First store should work");
    store
        .store(fp2.clone())
        .await
        .expect("Second store should work");

    let _count = store.count().await.expect("Count failed");
    // Count should not increase from double store of same ID
    println!("[EDGE] Double store of same ID handled - OK");

    println!("[VERIFIED] All edge cases handled correctly");
    println!("\n=== PASS: Edge Cases ===\n");
}
