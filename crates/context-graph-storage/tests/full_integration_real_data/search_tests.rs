//! Search Accuracy Tests
//!
//! TEST 6: Search Accuracy Test

use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::SemanticFingerprint;
use tempfile::TempDir;

use crate::helpers::{create_initialized_store, create_random_fingerprint};

// =============================================================================
// TEST 6: Search Accuracy Test
// =============================================================================

#[tokio::test]
async fn test_search_returns_correct_results() {
    println!("\n=== TEST: Search Returns Correct Results ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Store 50 random fingerprints
    println!("[SETUP] Storing 50 fingerprints...");
    for _ in 0..50 {
        let fp = create_random_fingerprint();
        store.store(fp).await.expect("Failed to store");
    }

    // Store one fingerprint with known E1 vector for searching
    let mut known_fp = create_random_fingerprint();
    // Set E1 to a known pattern: first 100 elements = 1/sqrt(100), rest = 0
    let norm = 100.0_f32.sqrt();
    known_fp.semantic.e1_semantic = vec![0.0; 1024];
    for i in 0..100 {
        known_fp.semantic.e1_semantic[i] = 1.0 / norm;
    }
    let known_id = known_fp.id;

    store
        .store(known_fp.clone())
        .await
        .expect("Failed to store known fp");
    println!("[SETUP] Stored known fingerprint {}", known_id);

    // Create a query that should match the known fingerprint well
    let mut query_semantic = SemanticFingerprint::zeroed();
    for i in 0..100 {
        query_semantic.e1_semantic[i] = 1.0 / norm;
    }

    // Search
    let options = context_graph_core::traits::TeleologicalSearchOptions {
        top_k: 10,
        min_similarity: 0.0,
        include_deleted: false,
        embedder_indices: vec![],
        semantic_query: None,   // No semantic query for this test
        include_content: false, // TASK-CONTENT-005
        ..Default::default()
    };

    let results = store
        .search_semantic(&query_semantic, options)
        .await
        .expect("Search failed");

    println!("[SEARCH] Returned {} results", results.len());
    assert!(!results.is_empty(), "Search should return results");

    // The known fingerprint should be in top results with high similarity
    let found = results.iter().find(|r| r.fingerprint.id == known_id);
    assert!(
        found.is_some(),
        "Known fingerprint should be in search results"
    );

    let known_result = found.unwrap();
    println!(
        "[FOUND] Known fingerprint at similarity {:.4}",
        known_result.similarity
    );
    assert!(
        known_result.similarity > 0.9,
        "Known fingerprint should have high similarity (> 0.9), got {}",
        known_result.similarity
    );

    // Results should be sorted by similarity descending
    for i in 1..results.len() {
        assert!(
            results[i - 1].similarity >= results[i].similarity,
            "Results should be sorted by similarity descending"
        );
    }

    println!("[VERIFIED] Search returns correct results in correct order");
    println!("\n=== PASS: Search Returns Correct Results ===\n");
}
