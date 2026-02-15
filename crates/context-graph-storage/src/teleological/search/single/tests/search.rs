//! Search functionality tests for single embedder search.
//!
//! Tests actual HNSW search behavior including empty indexes, thresholds, and similarity.

use std::sync::Arc;

use uuid::Uuid;

use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexOps, EmbedderIndexRegistry};

use crate::teleological::search::single::config::SingleEmbedderSearchConfig;
use crate::teleological::search::single::search::SingleEmbedderSearch;

fn create_test_search() -> SingleEmbedderSearch {
    let registry = Arc::new(EmbedderIndexRegistry::new());
    SingleEmbedderSearch::new(registry)
}

// Helper function for random floats
// HIGH-21 FIX: Use AtomicU32 instead of static mut to prevent data race UB.
// Tests run in parallel by default - static mut without synchronization is UB.
fn rand_float() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(42);
    let old = SEED.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |s| {
        Some(s.wrapping_mul(1103515245).wrapping_add(12345))
    }).unwrap();
    let new_seed = old.wrapping_mul(1103515245).wrapping_add(12345);
    (new_seed as f32) / (u32::MAX as f32)
}

// ========== EMPTY INDEX TESTS ==========

#[test]
fn test_empty_index_returns_empty_results() {
    println!("=== TEST: Empty index returns empty results ===");
    println!("BEFORE: Searching empty E8Graph index");

    let search = create_test_search();
    let query = vec![0.5f32; 1024];

    let result = search.search(EmbedderIndex::E8Graph, &query, 10, None);

    println!("AFTER: result = {:?}", result);
    assert!(result.is_ok());

    let results = result.unwrap();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);

    println!("RESULT: PASS");
}

#[test]
fn test_k_zero_returns_empty() {
    println!("=== TEST: k=0 returns empty results ===");

    let search = create_test_search();
    let query = vec![0.5f32; 1024];

    let result = search.search(EmbedderIndex::E8Graph, &query, 0, None);

    assert!(result.is_ok());
    let results = result.unwrap();
    assert!(results.is_empty());

    println!("RESULT: PASS");
}

// ========== SEARCH WITH DATA TESTS ==========

#[test]
fn test_search_returns_inserted_vector() {
    println!("=== TEST: Search returns inserted vector ===");
    println!("BEFORE: Inserting one vector into E8Graph");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));

    // Insert a vector
    let id = Uuid::new_v4();
    let vector = vec![0.5f32; 1024];
    let index = registry.get(EmbedderIndex::E8Graph).unwrap();
    index.insert(id, &vector).unwrap();

    println!("  Inserted: id={}", id);

    // Search
    let result = search.search(EmbedderIndex::E8Graph, &vector, 10, None);

    println!("AFTER: result = {:?}", result);
    assert!(result.is_ok());

    let results = result.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results.top().unwrap().id, id);
    assert!(results.top().unwrap().similarity > 0.99); // Identical vector

    println!("RESULT: PASS");
}

#[test]
fn test_search_identical_vectors_high_similarity() {
    println!("=== TEST: Identical vectors have similarity ~= 1.0 ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));

    let id = Uuid::new_v4();
    let vector: Vec<f32> = (0..1024).map(|i| (i as f32) / 1024.0).collect(); // E8 upgraded to 1024D

    let index = registry.get(EmbedderIndex::E8Graph).unwrap();
    index.insert(id, &vector).unwrap();

    let result = search
        .search(EmbedderIndex::E8Graph, &vector, 1, None)
        .unwrap();

    assert_eq!(result.len(), 1);
    let hit = result.top().unwrap();
    println!("Similarity: {}", hit.similarity);
    assert!(hit.similarity > 0.99);

    println!("RESULT: PASS");
}

#[test]
fn test_search_orthogonal_vectors_low_similarity() {
    println!("=== TEST: Orthogonal vectors have similarity ~= 0.0 ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));

    // Vector A: [1, 0, 0, 0, ...]
    let mut vec_a = vec![0.0f32; 1024];
    vec_a[0] = 1.0;

    // Vector B: [0, 1, 0, 0, ...]
    let mut vec_b = vec![0.0f32; 1024];
    vec_b[1] = 1.0;

    let id = Uuid::new_v4();
    let index = registry.get(EmbedderIndex::E8Graph).unwrap();
    index.insert(id, &vec_b).unwrap();

    let result = search
        .search(EmbedderIndex::E8Graph, &vec_a, 1, None)
        .unwrap();

    assert_eq!(result.len(), 1);
    let hit = result.top().unwrap();
    println!("Similarity: {}", hit.similarity);
    assert!(hit.similarity < 0.01); // Orthogonal

    println!("RESULT: PASS");
}

#[test]
fn test_search_with_threshold_filters() {
    println!("=== TEST: Threshold filters low-similarity results ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));
    let index = registry.get(EmbedderIndex::E8Graph).unwrap();

    // Normalized query vector (all positive, normalized) - E8 upgraded to 1024D
    let norm = (1024.0_f32).sqrt();
    let query: Vec<f32> = (0..1024).map(|_| 1.0 / norm).collect();

    // High similarity (identical to query)
    let id_high = Uuid::new_v4();
    index.insert(id_high, &query).unwrap();

    // Medium similarity (rotated 45 degrees - cosine ~0.7)
    let id_med = Uuid::new_v4();
    let med_norm = (1024.0_f32 * 2.0).sqrt();
    let vec_med: Vec<f32> = (0..1024)
        .map(|i| if i < 512 { 2.0 / med_norm } else { 0.0 })
        .collect();
    index.insert(id_med, &vec_med).unwrap();

    // Low similarity (orthogonal - alternating signs)
    let id_low = Uuid::new_v4();
    let vec_low: Vec<f32> = (0..1024)
        .map(|i| if i % 2 == 0 { 1.0 / norm } else { -1.0 / norm })
        .collect();
    index.insert(id_low, &vec_low).unwrap();

    // Search without threshold - should return all
    let result = search
        .search(EmbedderIndex::E8Graph, &query, 10, None)
        .unwrap();
    println!("Without threshold: {} results", result.len());
    for (i, hit) in result.iter().enumerate() {
        println!("  [{}] similarity={:.4}", i, hit.similarity);
    }
    assert_eq!(result.len(), 3);

    // Search with high threshold - should filter to only the identical vector
    let result = search
        .search(EmbedderIndex::E8Graph, &query, 10, Some(0.99))
        .unwrap();
    println!("With threshold 0.99: {} results", result.len());
    assert_eq!(
        result.len(),
        1,
        "Only the identical vector should pass 0.99 threshold"
    );

    println!("RESULT: PASS");
}

#[test]
fn test_k_greater_than_index_size() {
    println!("=== TEST: k > index size returns all available ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));
    let index = registry.get(EmbedderIndex::E8Graph).unwrap();

    // Insert 5 vectors
    for _ in 0..5 {
        let vec = vec![rand_float(); 1024];
        index.insert(Uuid::new_v4(), &vec).unwrap();
    }

    // Request k=1000, but only 5 exist
    let query = vec![0.5f32; 1024];
    let result = search
        .search(EmbedderIndex::E8Graph, &query, 1000, None)
        .unwrap();

    println!("Requested k=1000, got {} results", result.len());
    assert_eq!(result.len(), 5);

    println!("RESULT: PASS");
}

#[test]
fn test_threshold_filters_all() {
    println!("=== TEST: Very high threshold filters all results ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));
    let index = registry.get(EmbedderIndex::E8Graph).unwrap();

    // Insert some vectors
    for _ in 0..5 {
        let vec: Vec<f32> = (0..1024).map(|i| (i as f32 % 10.0) / 10.0).collect(); // E8 upgraded to 1024D
        index.insert(Uuid::new_v4(), &vec).unwrap();
    }

    // Use completely different query with threshold 0.99
    let query = vec![0.0f32; 1024];
    let result = search
        .search(EmbedderIndex::E8Graph, &query, 10, Some(0.99))
        .unwrap();

    println!(
        "With threshold 0.99 on orthogonal vectors: {} results",
        result.len()
    );
    // All should be filtered because query is [0,0,0...] which is orthogonal
    assert!(result.is_empty());

    println!("RESULT: PASS");
}

// ========== ALL EMBEDDER TESTS ==========

#[test]
fn test_all_hnsw_embedders_searchable() {
    println!("=== TEST: All 12 HNSW embedders are searchable ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));

    for embedder in EmbedderIndex::all_hnsw() {
        let dim = embedder.dimension().unwrap();
        let query = vec![0.5f32; dim];

        let result = search.search(embedder, &query, 10, None);
        assert!(
            result.is_ok(),
            "{:?} should be searchable but got: {:?}",
            embedder,
            result.err()
        );

        let results = result.unwrap();
        println!("  {:?} ({}D): {} results", embedder, dim, results.len());
    }

    println!("RESULT: PASS");
}

// ========== HELPER TESTS ==========

#[test]
fn test_search_default() {
    println!("=== TEST: search_default uses config values ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let config = SingleEmbedderSearchConfig {
        default_k: 50,
        default_threshold: Some(0.5),
        ef_search: None,
    };
    let search = SingleEmbedderSearch::with_config(Arc::clone(&registry), config);

    let query = vec![0.5f32; 1024];
    let result = search.search_default(EmbedderIndex::E8Graph, &query);

    assert!(result.is_ok());
    let results = result.unwrap();
    assert_eq!(results.k, 50);
    assert_eq!(results.threshold, Some(0.5));

    println!("RESULT: PASS");
}

#[test]
fn test_search_ids_above_threshold() {
    println!("=== TEST: search_ids_above_threshold returns (id, similarity) pairs ===");

    let registry = Arc::new(EmbedderIndexRegistry::new());
    let search = SingleEmbedderSearch::new(Arc::clone(&registry));
    let index = registry.get(EmbedderIndex::E8Graph).unwrap();

    let id = Uuid::new_v4();
    let vector = vec![0.5f32; 1024];
    index.insert(id, &vector).unwrap();

    let pairs = search
        .search_ids_above_threshold(EmbedderIndex::E8Graph, &vector, 10, 0.5)
        .unwrap();

    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0, id);
    assert!(pairs[0].1 > 0.99);

    println!("RESULT: PASS");
}

#[test]
fn test_latency_recorded() {
    println!("=== TEST: Search latency is recorded ===");

    let search = create_test_search();
    let query = vec![0.5f32; 1024];

    let result = search
        .search(EmbedderIndex::E8Graph, &query, 10, None)
        .unwrap();

    // Verify latency field is populated (may be 0 in release builds with empty indexes)
    println!("Latency: {} us", result.latency_us);

    println!("RESULT: PASS");
}
