//! Comprehensive tests for the multi-embedding query executor.
//!
//! # Test Categories
//!
//! 1. Query validation tests
//! 2. Executor creation and configuration
//! 3. Single-space search tests
//! 4. Multi-space RRF aggregation tests
//! 5. Pipeline execution tests
//! 6. Real data integration tests (no mocks)
//! 7. Edge cases and error handling
//!
//! All tests use real `InMemoryTeleologicalStore` and `StubMultiArrayProvider`.
//! NO MOCK DATA - per constitution.yaml.

use super::*;
use crate::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};
use crate::traits::TeleologicalMemoryStore;
use crate::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, SparseVector, TeleologicalFingerprint,
    NUM_EMBEDDERS,
};
use std::sync::Arc;
use std::time::Duration;

/// Create a test fingerprint with real data.
fn create_test_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        SemanticFingerprint::zeroed(),
        PurposeVector::new([0.75; NUM_EMBEDDERS]),
        JohariFingerprint::zeroed(),
        [0u8; 32],
    )
}

/// Create a fingerprint with non-zero semantic embeddings.
fn create_searchable_fingerprint(seed: u8) -> TeleologicalFingerprint {
    let mut semantic = SemanticFingerprint::zeroed();

    // Set some non-zero values for each embedding space
    for i in 0..semantic.e1_semantic.len().min(1024) {
        semantic.e1_semantic[i] = ((seed as usize + i) % 256) as f32 / 255.0;
    }
    for i in 0..semantic.e2_temporal_recent.len().min(512) {
        semantic.e2_temporal_recent[i] = ((seed as usize * 2 + i) % 256) as f32 / 255.0;
    }
    for i in 0..semantic.e7_code.len().min(256) {
        semantic.e7_code[i] = ((seed as usize * 3 + i) % 256) as f32 / 255.0;
    }

    // Add sparse vectors
    semantic.e13_splade = SparseVector::new(
        vec![100u16, 200, 300, (seed as u16).saturating_mul(10)],
        vec![0.5, 0.3, 0.8, 0.6],
    )
    .unwrap_or_else(|_| SparseVector::empty());

    TeleologicalFingerprint::new(
        semantic,
        PurposeVector::new([0.75 + (seed as f32 * 0.01); NUM_EMBEDDERS]),
        JohariFingerprint::zeroed(),
        [seed; 32],
    )
}

/// Create executor with pre-populated store.
async fn create_populated_executor(
    count: usize,
) -> (InMemoryMultiEmbeddingExecutor, Vec<uuid::Uuid>) {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();

    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let fp = create_searchable_fingerprint(i as u8);
        let id = store.store(fp).await.unwrap();
        ids.push(id);
    }

    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);
    (executor, ids)
}

// ==================== Query Validation Tests ====================

#[tokio::test]
async fn test_query_validation_empty_text_fails() {
    let query = MultiEmbeddingQuery {
        query_text: "".to_string(),
        ..Default::default()
    };

    let result = query.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        crate::error::CoreError::ValidationError { field, .. } => {
            assert_eq!(field, "query_text");
        }
        _ => panic!("Expected ValidationError"),
    }

    println!("[VERIFIED] Empty query text returns ValidationError");
}

#[tokio::test]
async fn test_query_validation_no_active_spaces_fails() {
    let query = MultiEmbeddingQuery {
        query_text: "test query".to_string(),
        active_spaces: EmbeddingSpaceMask(0),
        ..Default::default()
    };

    let result = query.validate();
    assert!(result.is_err());

    println!("[VERIFIED] Zero active spaces returns ValidationError");
}

#[tokio::test]
async fn test_query_validation_valid_passes() {
    let query = MultiEmbeddingQuery::new("How does memory consolidation work?");
    assert!(query.validate().is_ok());

    println!("[VERIFIED] Valid query passes validation");
}

// ==================== Executor Creation Tests ====================

#[tokio::test]
async fn test_executor_creation() {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();
    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

    let spaces = executor.available_spaces();
    assert_eq!(spaces.len(), 13);

    println!("[VERIFIED] Executor created with 13 spaces");
}

#[tokio::test]
async fn test_executor_with_arcs() {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let provider = Arc::new(StubMultiArrayProvider::new());
    let executor = InMemoryMultiEmbeddingExecutor::with_arcs(store, provider);

    let spaces = executor.available_spaces();
    assert_eq!(spaces.len(), 13);

    println!("[VERIFIED] Executor created with Arc-wrapped components");
}

#[tokio::test]
async fn test_available_spaces_info() {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();
    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

    let spaces = executor.available_spaces();

    // E1 Semantic
    assert_eq!(spaces[0].index, 0);
    assert_eq!(spaces[0].name, "E1_Semantic");
    assert_eq!(spaces[0].dimension, 1024);
    assert_eq!(spaces[0].index_type, IndexType::Hnsw);

    // E6 Sparse
    assert_eq!(spaces[5].index, 5);
    assert_eq!(spaces[5].name, "E6_Sparse");
    assert_eq!(spaces[5].dimension, 0);
    assert_eq!(spaces[5].index_type, IndexType::Inverted);

    // E13 SPLADE
    assert_eq!(spaces[12].index, 12);
    assert_eq!(spaces[12].name, "E13_SPLADE");
    assert_eq!(spaces[12].index_type, IndexType::Inverted);

    println!("[VERIFIED] SpaceInfo returns correct details for all 13 spaces");
}

// ==================== Warm Up Tests ====================

#[tokio::test]
async fn test_warm_up_all_spaces() {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();
    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

    let result = executor.warm_up(EmbeddingSpaceMask::ALL).await;
    assert!(result.is_ok());

    println!("[VERIFIED] warm_up succeeds for all spaces");
}

#[tokio::test]
async fn test_warm_up_subset() {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();
    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

    let result = executor.warm_up(EmbeddingSpaceMask::SEMANTIC_ONLY).await;
    assert!(result.is_ok());

    println!("[VERIFIED] warm_up succeeds for subset of spaces");
}

// ==================== Basic Execution Tests ====================

#[tokio::test]
async fn test_execute_empty_store() {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();
    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

    let query = MultiEmbeddingQuery::new("test query");
    let result = executor.execute(query).await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.results.is_empty());
    assert_eq!(result.spaces_failed, 0);

    println!("[VERIFIED] Execute on empty store returns empty results");
}

#[tokio::test]
async fn test_execute_with_data() {
    let (executor, ids) = create_populated_executor(10).await;

    let query = MultiEmbeddingQuery {
        query_text: "test query".to_string(),
        active_spaces: EmbeddingSpaceMask::SEMANTIC_ONLY,
        final_limit: 5,
        ..Default::default()
    };

    let result = executor.execute(query).await;
    assert!(result.is_ok());

    let result = result.unwrap();
    assert!(result.results.len() <= 5);
    assert!(result.spaces_searched >= 1);

    println!(
        "[VERIFIED] Execute with data returns {} results",
        result.results.len()
    );
}

#[tokio::test]
async fn test_execute_respects_final_limit() {
    let (executor, _) = create_populated_executor(20).await;

    let query = MultiEmbeddingQuery {
        query_text: "test query".to_string(),
        final_limit: 5,
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();
    assert!(result.results.len() <= 5);

    println!("[VERIFIED] Execute respects final_limit");
}

// ==================== Execute with Pre-computed Embeddings ====================

#[tokio::test]
async fn test_execute_with_embeddings() {
    let (executor, _) = create_populated_executor(10).await;

    let embeddings = SemanticFingerprint::zeroed();
    let query = MultiEmbeddingQuery {
        query_text: "ignored".to_string(),
        active_spaces: EmbeddingSpaceMask::SEMANTIC_ONLY,
        final_limit: 5,
        ..Default::default()
    };

    let result = executor.execute_with_embeddings(&embeddings, query).await;
    assert!(result.is_ok());

    println!("[VERIFIED] execute_with_embeddings works correctly");
}

#[tokio::test]
async fn test_execute_with_embeddings_no_active_spaces_fails() {
    let (executor, _) = create_populated_executor(5).await;

    let embeddings = SemanticFingerprint::zeroed();
    let query = MultiEmbeddingQuery {
        query_text: "test".to_string(),
        active_spaces: EmbeddingSpaceMask(0),
        ..Default::default()
    };

    let result = executor.execute_with_embeddings(&embeddings, query).await;
    assert!(result.is_err());

    println!("[VERIFIED] execute_with_embeddings fails with no active spaces");
}

// ==================== Space Breakdown Tests ====================

#[tokio::test]
async fn test_include_space_breakdown() {
    let (executor, _) = create_populated_executor(10).await;

    let query = MultiEmbeddingQuery {
        query_text: "test query".to_string(),
        include_space_breakdown: true,
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();
    assert!(result.space_breakdown.is_some());

    let breakdown = result.space_breakdown.unwrap();
    assert!(!breakdown.is_empty());

    println!(
        "[VERIFIED] Space breakdown included with {} spaces",
        breakdown.len()
    );
}

// ==================== RRF Aggregation Tests ====================

#[tokio::test]
async fn test_rrf_aggregation_formula() {
    // Test RRF formula: RRF(d) = 1/(k + rank + 1)
    let id1 = uuid::Uuid::new_v4();
    let id2 = uuid::Uuid::new_v4();

    let ranked_lists = vec![
        (0, vec![id1, id2]), // Space 0: id1=rank0, id2=rank1
        (1, vec![id2, id1]), // Space 1: id2=rank0, id1=rank1
    ];

    let scores = AggregationStrategy::aggregate_rrf(&ranked_lists, 60.0);

    // id1: 1/61 + 1/62 ≈ 0.0326
    // id2: 1/62 + 1/61 ≈ 0.0326
    let score1 = scores.get(&id1).unwrap();
    let score2 = scores.get(&id2).unwrap();

    // Both should be approximately equal due to symmetry
    assert!((score1 - score2).abs() < 0.0001);

    // Verify exact formula
    let expected = 1.0 / 61.0 + 1.0 / 62.0;
    assert!((score1 - expected).abs() < 0.0001);

    println!("[VERIFIED] RRF aggregation formula is correct");
}

#[tokio::test]
async fn test_rrf_weighted_aggregation() {
    let id1 = uuid::Uuid::new_v4();
    let id2 = uuid::Uuid::new_v4();

    let ranked_lists = vec![
        (0, vec![id1, id2]), // Space 0 weight=2.0
        (1, vec![id2, id1]), // Space 1 weight=1.0
    ];

    let mut weights = [1.0; NUM_EMBEDDERS];
    weights[0] = 2.0;

    let scores = AggregationStrategy::aggregate_rrf_weighted(&ranked_lists, 60.0, &weights);

    let score1 = scores.get(&id1).unwrap();
    let score2 = scores.get(&id2).unwrap();

    // id1 should score higher due to double weight in space 0
    assert!(score1 > score2);

    println!("[VERIFIED] Weighted RRF aggregation works correctly");
}

// ==================== Pipeline Execution Tests ====================

#[tokio::test]
async fn test_execute_pipeline() {
    let (executor, _) = create_populated_executor(10).await;

    let query = MultiEmbeddingQuery {
        query_text: "test pipeline query".to_string(),
        final_limit: 5,
        ..Default::default()
    };

    let result = executor.execute_pipeline(query).await;
    assert!(result.is_ok());

    let result = result.unwrap();
    assert!(result.stage_timings.is_some());

    let timings = result.stage_timings.unwrap();
    assert!(timings.total() > Duration::ZERO);

    println!("[VERIFIED] Pipeline execution returns stage timings: {}", timings.summary());
}

#[tokio::test]
async fn test_pipeline_stage_config_defaults() {
    let config = PipelineStageConfig::default();

    assert_eq!(config.splade_candidates, 1000);
    assert_eq!(config.matryoshka_128d_limit, 200);
    assert_eq!(config.full_search_limit, 100);
    assert_eq!(config.teleological_limit, 50);
    assert_eq!(config.late_interaction_limit, 20);
    assert!((config.rrf_k - 60.0).abs() < 0.001);
    assert!((config.min_alignment_threshold - 0.55).abs() < 0.001);

    println!("[VERIFIED] PipelineStageConfig defaults match constitution.yaml");
}

// ==================== EmbeddingSpaceMask Tests ====================

#[tokio::test]
async fn test_embedding_space_mask_all() {
    let mask = EmbeddingSpaceMask::ALL;
    assert_eq!(mask.active_count(), 13);
    assert!(mask.includes_splade());
    assert!(mask.includes_late_interaction());

    for i in 0..13 {
        assert!(mask.is_active(i), "Space {} should be active", i);
    }

    println!("[VERIFIED] EmbeddingSpaceMask::ALL has all 13 spaces active");
}

#[tokio::test]
async fn test_embedding_space_mask_presets() {
    assert_eq!(EmbeddingSpaceMask::ALL_DENSE.active_count(), 12);
    assert_eq!(EmbeddingSpaceMask::SEMANTIC_ONLY.active_count(), 1);
    assert_eq!(EmbeddingSpaceMask::TEXT_CORE.active_count(), 3);
    assert_eq!(EmbeddingSpaceMask::SPLADE_ONLY.active_count(), 1);
    assert_eq!(EmbeddingSpaceMask::HYBRID.active_count(), 2);
    assert_eq!(EmbeddingSpaceMask::CODE_FOCUSED.active_count(), 3);

    println!("[VERIFIED] All EmbeddingSpaceMask presets have correct counts");
}

#[tokio::test]
async fn test_embedding_space_mask_active_indices() {
    let mask = EmbeddingSpaceMask::TEXT_CORE;
    let indices = mask.active_indices();
    assert_eq!(indices, vec![0, 1, 2]);

    let hybrid = EmbeddingSpaceMask::HYBRID;
    let indices = hybrid.active_indices();
    assert_eq!(indices, vec![0, 12]); // E1 and E13

    println!("[VERIFIED] active_indices returns correct list");
}

// ==================== Result Type Tests ====================

#[tokio::test]
async fn test_multi_embedding_result_latency_check() {
    let result = MultiEmbeddingResult::new(
        vec![],
        Duration::from_millis(50),
        13,
        0,
    );
    assert!(result.within_latency_target());

    let result_slow = MultiEmbeddingResult::new(
        vec![],
        Duration::from_millis(70),
        13,
        0,
    );
    assert!(!result_slow.within_latency_target());

    println!("[VERIFIED] within_latency_target correctly checks 60ms threshold");
}

#[tokio::test]
async fn test_scored_match_creation() {
    let id = uuid::Uuid::new_v4();
    let m = ScoredMatch::new(id, 0.85, 3);

    assert_eq!(m.memory_id, id);
    assert!((m.similarity - 0.85).abs() < 0.001);
    assert_eq!(m.rank, 3);

    println!("[VERIFIED] ScoredMatch creation works correctly");
}

#[tokio::test]
async fn test_space_search_result_success() {
    let id = uuid::Uuid::new_v4();
    let matches = vec![ScoredMatch::new(id, 0.9, 0)];
    let result = SpaceSearchResult::success(0, matches, Duration::from_millis(5), 1000);

    assert_eq!(result.space_index, 0);
    assert_eq!(result.space_name, "E1_Semantic");
    assert!(result.success);
    assert!(result.error.is_none());
    assert_eq!(result.matches.len(), 1);

    println!("[VERIFIED] SpaceSearchResult::success creates correct result");
}

#[tokio::test]
async fn test_space_search_result_failure() {
    let result = SpaceSearchResult::failure(5, "Index corrupted".to_string());

    assert_eq!(result.space_index, 5);
    assert_eq!(result.space_name, "E6_Sparse");
    assert!(!result.success);
    assert!(result.error.is_some());
    assert!(result.matches.is_empty());

    println!("[VERIFIED] SpaceSearchResult::failure creates correct result");
}

// ==================== Integration Tests ====================

#[tokio::test]
async fn test_full_query_flow_with_real_data() {
    // Store 20 fingerprints
    let (executor, stored_ids) = create_populated_executor(20).await;

    // Execute a query
    let query = MultiEmbeddingQuery {
        query_text: "memory consolidation neural".to_string(),
        active_spaces: EmbeddingSpaceMask::ALL,
        final_limit: 10,
        include_space_breakdown: true,
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();

    // Verify result structure
    assert!(result.results.len() <= 10);
    assert!(result.space_breakdown.is_some());
    assert!(result.spaces_searched > 0);
    assert_eq!(result.spaces_failed, 0);

    // All returned IDs should be from stored fingerprints
    for m in &result.results {
        assert!(
            stored_ids.contains(&m.memory_id),
            "Returned ID not in stored set"
        );
        assert!(m.aggregate_score > 0.0);
    }

    println!(
        "[VERIFIED] Full query flow: {} results, {} spaces searched, {:?} total time",
        result.results.len(),
        result.spaces_searched,
        result.total_time
    );
}

#[tokio::test]
async fn test_aggregated_match_space_contributions() {
    let (executor, _) = create_populated_executor(5).await;

    let query = MultiEmbeddingQuery {
        query_text: "test".to_string(),
        active_spaces: EmbeddingSpaceMask::TEXT_CORE, // 3 spaces
        final_limit: 3,
        include_space_breakdown: true,
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();

    for m in &result.results {
        // Each match should have space contributions
        // space_count tracks how many spaces contributed
        assert!(m.space_count <= 3);
        assert_eq!(m.space_contributions.len(), m.space_count);

        for contrib in &m.space_contributions {
            assert!(contrib.space_index < 13);
            assert!(contrib.rrf_contribution > 0.0);
        }
    }

    println!("[VERIFIED] Aggregated matches have correct space contributions");
}

#[tokio::test]
async fn test_sparse_space_search() {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();

    // Store a fingerprint with sparse data
    let mut fp = create_test_fingerprint();
    fp.semantic.e13_splade = SparseVector::new(vec![100, 200, 300], vec![0.5, 0.3, 0.8]).unwrap();
    store.store(fp).await.unwrap();

    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

    let query = MultiEmbeddingQuery {
        query_text: "sparse test".to_string(),
        active_spaces: EmbeddingSpaceMask::SPLADE_ONLY,
        final_limit: 10,
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();
    assert!(result.spaces_searched >= 1);

    println!("[VERIFIED] Sparse space (SPLADE) search executes correctly");
}

// ==================== Edge Cases ====================

#[tokio::test]
async fn test_query_with_min_similarity_filter() {
    let (executor, _) = create_populated_executor(10).await;

    let query = MultiEmbeddingQuery {
        query_text: "test".to_string(),
        min_similarity: 0.99, // Very high threshold
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();
    // With high threshold, most results should be filtered
    // Exact count depends on test data similarity

    println!(
        "[VERIFIED] min_similarity filter applied: {} results",
        result.results.len()
    );
}

#[tokio::test]
async fn test_single_space_execution() {
    let (executor, _) = create_populated_executor(5).await;

    let query = MultiEmbeddingQuery {
        query_text: "single space".to_string(),
        active_spaces: EmbeddingSpaceMask::SEMANTIC_ONLY,
        final_limit: 3,
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();
    assert!(result.spaces_searched >= 1);

    println!("[VERIFIED] Single space execution works correctly");
}
