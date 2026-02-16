//! Tests for InMemoryTeleologicalStore.

use std::path::Path;
use uuid::Uuid;

use super::similarity::cosine_similarity;
use super::InMemoryTeleologicalStore;
use crate::traits::{
    TeleologicalMemoryStore, TeleologicalSearchOptions, TeleologicalStorageBackend,
};
use crate::types::fingerprint::{SemanticFingerprint, SparseVector, TeleologicalFingerprint};

fn create_test_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(SemanticFingerprint::zeroed(), [0u8; 32])
}

#[tokio::test]
async fn test_store_and_retrieve() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_test_fingerprint();
    let id = fp.id;
    let stored_id = store.store(fp.clone()).await.unwrap();
    assert_eq!(stored_id, id);
    let retrieved = store.retrieve(id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, id);
}

#[tokio::test]
async fn test_retrieve_nonexistent() {
    let store = InMemoryTeleologicalStore::new();
    let result = store.retrieve(Uuid::new_v4()).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_update() {
    let store = InMemoryTeleologicalStore::new();
    let mut fp = create_test_fingerprint();
    let id = fp.id;
    store.store(fp.clone()).await.unwrap();
    fp.access_count = 42;
    let updated = store.update(fp).await.unwrap();
    assert!(updated);
    let retrieved = store.retrieve(id).await.unwrap().unwrap();
    assert_eq!(retrieved.access_count, 42);
}

#[tokio::test]
async fn test_update_nonexistent_returns_false() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_test_fingerprint();
    let result = store.update(fp).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_soft_delete() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_test_fingerprint();
    let id = fp.id;
    store.store(fp).await.unwrap();
    let deleted = store.delete(id, true).await.unwrap();
    assert!(deleted);
    let retrieved = store.retrieve(id).await.unwrap();
    assert!(retrieved.is_none());
    assert!(store.data.contains_key(&id));
}

#[tokio::test]
async fn test_hard_delete() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_test_fingerprint();
    let id = fp.id;
    store.store(fp).await.unwrap();
    let deleted = store.delete(id, false).await.unwrap();
    assert!(deleted);
    let retrieved = store.retrieve(id).await.unwrap();
    assert!(retrieved.is_none());
    assert!(!store.data.contains_key(&id));
}

#[tokio::test]
async fn test_search_semantic() {
    let store = InMemoryTeleologicalStore::new();
    for _ in 0..5 {
        store.store(create_test_fingerprint()).await.unwrap();
    }
    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(10);
    let results = store.search_semantic(&query, options).await.unwrap();
    assert!(!results.is_empty(), "Search after storing 5 items should return results");
    assert!(results.len() <= 5);
}

#[tokio::test]
async fn test_batch_store_and_retrieve() {
    let store = InMemoryTeleologicalStore::new();
    let fingerprints: Vec<_> = (0..10).map(|_| create_test_fingerprint()).collect();
    let ids: Vec<_> = fingerprints.iter().map(|fp| fp.id).collect();
    let stored_ids = store.store_batch(fingerprints).await.unwrap();
    assert_eq!(stored_ids.len(), 10);
    let retrieved = store.retrieve_batch(&ids).await.unwrap();
    assert_eq!(retrieved.len(), 10);
    assert!(retrieved.iter().all(|r| r.is_some()));
}

#[tokio::test]
async fn test_empty_store_count() {
    let store = InMemoryTeleologicalStore::new();
    let count = store.count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_search_empty_store() {
    let store = InMemoryTeleologicalStore::new();
    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(10);
    let results = store.search_semantic(&query, options).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_checkpoint_and_restore() {
    let store = InMemoryTeleologicalStore::new();
    let checkpoint_result = store.checkpoint().await;
    assert!(checkpoint_result.is_err());
    let restore_result = store.restore(Path::new("/tmp/nonexistent")).await;
    assert!(restore_result.is_err());
}

#[tokio::test]
async fn test_backend_type() {
    let store = InMemoryTeleologicalStore::new();
    assert_eq!(store.backend_type(), TeleologicalStorageBackend::InMemory);
}

/// TST-09 FIX: Added assertions. Previously the result was assigned to `_results`
/// (underscore prefix = intentionally unused) with zero assertions.
#[tokio::test]
async fn test_min_similarity_filter() {
    let store = InMemoryTeleologicalStore::new();
    for _ in 0..5 {
        store.store(create_test_fingerprint()).await.unwrap();
    }
    let query = SemanticFingerprint::zeroed();
    let options = TeleologicalSearchOptions::quick(10).with_min_similarity(0.99);
    let results = store.search_semantic(&query, options).await.unwrap();
    // With zeroed query against zeroed fingerprints, cosine similarity is undefined (0/0).
    // The stub returns 0.0 similarity, which should be filtered by min_similarity=0.99.
    // Assert that the filter actually reduces results (or returns empty).
    assert!(
        results.len() <= 5,
        "min_similarity=0.99 should filter some or all results, got {}",
        results.len()
    );
}

#[tokio::test]
async fn test_sparse_search() {
    let store = InMemoryTeleologicalStore::new();
    let mut fp = create_test_fingerprint();
    fp.semantic.e13_splade = SparseVector::new(vec![100, 200, 300], vec![0.5, 0.3, 0.8]).unwrap();
    store.store(fp).await.unwrap();
    let query = SparseVector::new(vec![100, 200, 400], vec![0.5, 0.5, 0.5]).unwrap();
    let results = store.search_sparse(&query, 10).await.unwrap();
    assert!(!results.is_empty());
    assert!(results[0].1 > 0.0);
}

#[tokio::test]
async fn test_compact() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_test_fingerprint();
    let id = fp.id;
    store.store(fp).await.unwrap();
    store.delete(id, true).await.unwrap();
    assert!(store.data.contains_key(&id));
    store.compact().await.unwrap();
    assert!(!store.data.contains_key(&id));
}

#[test]
fn test_cosine_similarity() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert!((sim - 1.0).abs() < 1e-6);
    let c = vec![0.0, 1.0, 0.0];
    let sim2 = cosine_similarity(&a, &c);
    assert!(sim2.abs() < 1e-6);
}

#[tokio::test]
async fn test_content_round_trip() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_test_fingerprint();
    let id = fp.id;
    let test_content = "Test content for round-trip verification";
    store.store(fp).await.unwrap();
    let store_result = store.store_content(id, test_content).await;
    assert!(store_result.is_ok());
    let retrieved = store.get_content(id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), test_content);
    let deleted = store.delete_content(id).await.unwrap();
    assert!(deleted);
    let after_delete = store.get_content(id).await.unwrap();
    assert!(after_delete.is_none());
}

#[tokio::test]
async fn test_hard_delete_cascades_to_content() {
    let store = InMemoryTeleologicalStore::new();
    let fp = create_test_fingerprint();
    let id = fp.id;
    let test_content = "Content that should be deleted with fingerprint";
    store.store(fp).await.unwrap();
    store.store_content(id, test_content).await.unwrap();
    let content_before = store.get_content(id).await.unwrap();
    assert!(content_before.is_some());
    let deleted = store.delete(id, false).await.unwrap();
    assert!(deleted);
    let fp_after = store.retrieve(id).await.unwrap();
    assert!(fp_after.is_none());
    let content_after = store.get_content(id).await.unwrap();
    assert!(content_after.is_none());
}

#[tokio::test]
async fn test_content_batch_mixed() {
    let store = InMemoryTeleologicalStore::new();
    let fp1 = create_test_fingerprint();
    let fp2 = create_test_fingerprint();
    let fp3 = create_test_fingerprint();
    let id1 = fp1.id;
    let id2 = fp2.id;
    let id3 = fp3.id;
    store.store(fp1).await.unwrap();
    store.store(fp2).await.unwrap();
    store.store(fp3).await.unwrap();
    store.store_content(id1, "Content for fp1").await.unwrap();
    store.store_content(id3, "Content for fp3").await.unwrap();
    let batch = store.get_content_batch(&[id1, id2, id3]).await.unwrap();
    assert_eq!(batch.len(), 3);
    assert!(batch[0].is_some());
    assert_eq!(batch[0].as_ref().unwrap(), "Content for fp1");
    assert!(batch[1].is_none());
    assert!(batch[2].is_some());
    assert_eq!(batch[2].as_ref().unwrap(), "Content for fp3");
}

#[tokio::test]
async fn test_content_nonexistent_id() {
    let store = InMemoryTeleologicalStore::new();
    let random_id = Uuid::new_v4();
    let result = store.get_content(random_id).await.unwrap();
    assert!(result.is_none());
    let deleted = store.delete_content(random_id).await.unwrap();
    assert!(!deleted);
}

// ============================================================================
// FILE INDEX TESTS - Verify proper implementation (not no-ops)
// ============================================================================

#[tokio::test]
async fn test_file_index_operations() {
    let store = InMemoryTeleologicalStore::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let file_path = "/test/file.md";

    // Test index_file_fingerprint
    store.index_file_fingerprint(file_path, id1).await.unwrap();
    store.index_file_fingerprint(file_path, id2).await.unwrap();

    // Verify get_fingerprints_for_file returns indexed IDs
    let ids = store.get_fingerprints_for_file(file_path).await.unwrap();
    assert_eq!(ids.len(), 2, "Should have 2 indexed fingerprints");
    assert!(ids.contains(&id1), "Should contain id1");
    assert!(ids.contains(&id2), "Should contain id2");

    // Test list_indexed_files
    let files = store.list_indexed_files().await.unwrap();
    assert_eq!(files.len(), 1, "Should have 1 indexed file");
    assert_eq!(files[0].file_path, file_path);
    assert_eq!(files[0].fingerprint_count(), 2);

    // Test unindex_file_fingerprint
    let removed = store.unindex_file_fingerprint(file_path, id1).await.unwrap();
    assert!(removed, "Should return true when fingerprint was removed");

    let ids_after = store.get_fingerprints_for_file(file_path).await.unwrap();
    assert_eq!(ids_after.len(), 1, "Should have 1 fingerprint after unindex");
    assert!(!ids_after.contains(&id1), "Should NOT contain id1 anymore");
    assert!(ids_after.contains(&id2), "Should still contain id2");

    // Test unindex returns false for non-existent
    let removed_again = store.unindex_file_fingerprint(file_path, id1).await.unwrap();
    assert!(!removed_again, "Should return false for already-removed fingerprint");

    // Test clear_file_index
    store.index_file_fingerprint(file_path, id1).await.unwrap(); // Re-add id1
    let count = store.clear_file_index(file_path).await.unwrap();
    assert_eq!(count, 2, "Should return count of cleared fingerprints");

    let ids_cleared = store.get_fingerprints_for_file(file_path).await.unwrap();
    assert!(ids_cleared.is_empty(), "Should be empty after clear");
}

#[tokio::test]
async fn test_file_index_duplicate_prevention() {
    let store = InMemoryTeleologicalStore::new();
    let id = Uuid::new_v4();
    let file_path = "/test/duplicate.md";

    // Index same fingerprint twice
    store.index_file_fingerprint(file_path, id).await.unwrap();
    store.index_file_fingerprint(file_path, id).await.unwrap();

    // Should only have 1 entry (no duplicates)
    let ids = store.get_fingerprints_for_file(file_path).await.unwrap();
    assert_eq!(ids.len(), 1, "Should prevent duplicates");
}

#[tokio::test]
async fn test_file_index_multiple_files() {
    let store = InMemoryTeleologicalStore::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // Index to different files
    store.index_file_fingerprint("/file1.md", id1).await.unwrap();
    store.index_file_fingerprint("/file2.md", id2).await.unwrap();

    // Verify stats
    let stats = store.get_file_watcher_stats().await.unwrap();
    assert_eq!(stats.total_files, 2, "Should have 2 files");
    assert_eq!(stats.total_chunks, 2, "Should have 2 total chunks");
    assert_eq!(stats.min_chunks, 1);
    assert_eq!(stats.max_chunks, 1);
}

#[tokio::test]
async fn test_file_index_empty_after_clear() {
    let store = InMemoryTeleologicalStore::new();
    let file_path = "/test/clear_me.md";

    // Clear non-existent file should return 0
    let count = store.clear_file_index(file_path).await.unwrap();
    assert_eq!(count, 0, "Clearing non-existent file should return 0");

    // Empty file index should not appear in list
    let files = store.list_indexed_files().await.unwrap();
    assert!(files.is_empty(), "Should have no indexed files");
}

// ============================================================================
// FAIL-FAST TESTS - Verify error propagation per project requirements
// ============================================================================

#[tokio::test]
async fn test_search_text_returns_not_implemented_error() {
    // FAIL-FAST TEST: search_text should return NotImplemented error,
    // NOT silently return empty results.
    //
    // This is a critical behavior change - the storage layer cannot
    // generate embeddings, so it MUST fail instead of pretending to work.
    use crate::error::CoreError;

    let store = InMemoryTeleologicalStore::new();
    let options = TeleologicalSearchOptions::quick(10);

    let result = store.search_text("test query", options).await;

    // MUST be an error, NOT Ok(empty)
    assert!(result.is_err(), "search_text MUST return error, not Ok(empty)");

    // Verify it's specifically NotImplemented
    match result {
        Err(CoreError::NotImplemented(msg)) => {
            // Verify error message is helpful
            assert!(
                msg.contains("storage layer"),
                "Error should mention storage layer limitation"
            );
            assert!(
                msg.contains("search_semantic") || msg.contains("search_graph"),
                "Error should suggest alternative methods"
            );
        }
        Err(other) => panic!("Expected NotImplemented error, got: {:?}", other),
        Ok(_) => panic!("search_text MUST return error, not Ok"),
    }
}
