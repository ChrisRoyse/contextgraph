//! Comprehensive tests for the multi-embedding query executor.
//!
//! All tests use STUB implementations (InMemoryTeleologicalStore, StubMultiArrayProvider).

use super::*;
use crate::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};
use crate::traits::TeleologicalMemoryStore;
use crate::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint, NUM_EMBEDDERS,
};
use crate::weights::E5_CAUSAL_ENABLED;
use std::time::Duration;

/// Create a fingerprint with non-zero semantic embeddings.
fn create_searchable_fingerprint(seed: u8) -> TeleologicalFingerprint {
    let mut semantic = SemanticFingerprint::zeroed();

    for i in 0..semantic.e1_semantic.len().min(1024) {
        semantic.e1_semantic[i] = ((seed as usize + i) % 256) as f32 / 255.0;
    }
    for i in 0..semantic.e2_temporal_recent.len().min(512) {
        semantic.e2_temporal_recent[i] = ((seed as usize * 2 + i) % 256) as f32 / 255.0;
    }
    for i in 0..semantic.e7_code.len().min(1536) {
        semantic.e7_code[i] = ((seed as usize * 3 + i) % 256) as f32 / 255.0;
    }

    semantic.e13_splade = SparseVector::new(
        vec![100u16, 200, 300, (seed as u16).saturating_mul(10)],
        vec![0.5, 0.3, 0.8, 0.6],
    )
    .unwrap_or_else(|_| SparseVector::empty());

    TeleologicalFingerprint::new(semantic, [seed; 32])
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
}

#[tokio::test]
async fn test_executor_creation() {
    let store = InMemoryTeleologicalStore::new();
    let provider = StubMultiArrayProvider::new();
    let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

    let spaces = executor.available_spaces();
    assert_eq!(spaces.len(), NUM_EMBEDDERS);
}

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
}

#[tokio::test]
async fn test_execute_with_data() {
    let (executor, _ids) = create_populated_executor(10).await;

    let query = MultiEmbeddingQuery {
        query_text: "test query".to_string(),
        active_spaces: EmbeddingSpaceMask::SEMANTIC_ONLY,
        final_limit: 5,
        ..Default::default()
    };

    let result = executor.execute(query).await;
    assert!(result.is_ok());

    let result = result.unwrap();
    assert!(
        !result.results.is_empty(),
        "Search with data should return results"
    );
    assert!(result.results.len() <= 5, "Should respect final_limit");
    assert!(result.spaces_searched >= 1);
}

#[tokio::test]
async fn test_rrf_aggregation_formula() {
    let id1 = uuid::Uuid::new_v4();
    let id2 = uuid::Uuid::new_v4();

    let ranked_lists = vec![(0, vec![id1, id2]), (1, vec![id2, id1])];

    let scores = AggregationStrategy::aggregate_rrf(&ranked_lists, 60.0);

    let score1 = scores.get(&id1).unwrap();
    let score2 = scores.get(&id2).unwrap();

    // Both should be approximately equal due to symmetry
    assert!((score1 - score2).abs() < 0.0001);

    // Verify exact formula: 1/61 + 1/62
    let expected = 1.0 / 61.0 + 1.0 / 62.0;
    assert!((score1 - expected).abs() < 0.0001);
}

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
}

#[tokio::test]
async fn test_full_query_flow_with_stub_data() {
    let (executor, stored_ids) = create_populated_executor(20).await;

    let query = MultiEmbeddingQuery {
        query_text: "memory consolidation neural".to_string(),
        active_spaces: EmbeddingSpaceMask::ALL,
        final_limit: 10,
        include_space_breakdown: true,
        ..Default::default()
    };

    let result = executor.execute(query).await.unwrap();

    assert!(
        !result.results.is_empty(),
        "Search across all spaces should return results"
    );
    assert!(result.results.len() <= 10, "Should respect final_limit");
    assert!(result.space_breakdown.is_some());
    assert!(result.spaces_searched > 0);
    assert_eq!(result.spaces_failed, 0);

    for m in &result.results {
        assert!(
            stored_ids.contains(&m.memory_id),
            "Returned ID not in stored set"
        );
        assert!(m.aggregate_score > 0.0);
    }
}

#[tokio::test]
async fn test_embedding_space_mask_all() {
    let mask = EmbeddingSpaceMask::ALL;
    let expected_active = if E5_CAUSAL_ENABLED {
        NUM_EMBEDDERS
    } else {
        NUM_EMBEDDERS - 1
    };
    assert_eq!(mask.active_count(), expected_active);
    assert!(mask.includes_splade());
    assert!(mask.includes_late_interaction());

    for i in 0..NUM_EMBEDDERS {
        if !E5_CAUSAL_ENABLED && i == 4 {
            assert!(!mask.is_active(i), "Retired E5 space should be inactive");
        } else {
            assert!(mask.is_active(i), "Space {} should be active", i);
        }
    }

    // Presets
    assert_eq!(
        EmbeddingSpaceMask::ALL_DENSE.active_count(),
        if E5_CAUSAL_ENABLED { 11 } else { 10 }
    );
    assert_eq!(EmbeddingSpaceMask::SEMANTIC_ONLY.active_count(), 1);
    assert_eq!(EmbeddingSpaceMask::TEXT_CORE.active_count(), 3);
}
