//! Statistics and edge case tests for ModelRegistry.

use std::sync::Arc;

use crate::error::EmbeddingError;
use crate::traits::get_memory_estimate;
use crate::types::ModelId;

use super::config::ModelRegistryConfig;
use super::core::ModelRegistry;
use super::tests::TestFactory;

// =========================================================================
// STATISTICS TESTS (3 tests)
// =========================================================================

#[tokio::test]
async fn test_stats_initial_zeros() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    let stats = registry.stats().await;

    assert_eq!(stats.loaded_count, 0);
    assert_eq!(stats.total_memory_bytes, 0);
    assert_eq!(stats.load_count, 0);
    assert_eq!(stats.unload_count, 0);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.load_failures, 0);
}

#[tokio::test]
async fn test_stats_accurate_after_operations() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    // Load 3 models
    registry.load_model(ModelId::Semantic).await.unwrap();
    registry.load_model(ModelId::Code).await.unwrap();
    registry.load_model(ModelId::Graph).await.unwrap();

    // Unload 1
    registry.unload_model(ModelId::Code).await.unwrap();

    // Get (cache hit)
    let _ = registry.get_model(ModelId::Semantic).await.unwrap();

    let stats = registry.stats().await;
    assert_eq!(stats.load_count, 3);
    assert_eq!(stats.unload_count, 1);
    assert_eq!(stats.loaded_count, 2);
    assert_eq!(stats.cache_hits, 1);
}

#[tokio::test]
async fn test_stats_tracks_failures() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig {
        memory_budget_bytes: 100_000_000, // Too small
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    // Try to load models that exceed budget
    let _ = registry.load_model(ModelId::Semantic).await;
    let _ = registry.load_model(ModelId::Multimodal).await;

    let stats = registry.stats().await;
    assert_eq!(stats.load_failures, 2);
}

// =========================================================================
// EDGE CASE: EDGE-1 - Empty Registry Operations (1 test)
// =========================================================================

#[tokio::test]
async fn test_edge_1_unload_from_empty_registry() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    // Precondition: Registry is empty
    assert_eq!(registry.loaded_count().await, 0);

    // Action: Try to unload
    let result = registry.unload_model(ModelId::Semantic).await;

    // Expected: EmbeddingError::ModelNotLoaded
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::ModelNotLoaded { model_id }) => {
            assert_eq!(model_id, ModelId::Semantic);
        }
        _ => panic!("Expected ModelNotLoaded error"),
    }

    // Postcondition: stats unchanged, models empty
    let stats = registry.stats().await;
    assert_eq!(stats.load_failures, 0); // unload failure != load failure
    assert!(registry.loaded_models().await.is_empty());
}

// =========================================================================
// EDGE CASE: EDGE-2 - Memory Budget Exceeded (1 test)
// =========================================================================

#[tokio::test]
async fn test_edge_2_memory_budget_exceeded() {
    let factory = Arc::new(TestFactory::new());
    // 1GB budget - smaller than LateInteraction (450MB) but we'll test with Multimodal (1.6GB)
    let config = ModelRegistryConfig {
        memory_budget_bytes: 1_000_000_000,
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    // Precondition: Confirm budget
    assert_eq!(registry.memory_budget(), 1_000_000_000);

    // Action: Try to load Multimodal (1.6GB)
    let result = registry.load_model(ModelId::Multimodal).await;

    // Expected: EmbeddingError::MemoryBudgetExceeded
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::MemoryBudgetExceeded {
            requested_bytes,
            available_bytes,
            budget_bytes,
        }) => {
            assert_eq!(requested_bytes, get_memory_estimate(ModelId::Multimodal));
            assert_eq!(available_bytes, 1_000_000_000);
            assert_eq!(budget_bytes, 1_000_000_000);
        }
        _ => panic!("Expected MemoryBudgetExceeded error"),
    }

    // Postcondition: No memory allocated, no model in HashMap
    assert_eq!(registry.total_memory_usage().await, 0);
    assert!(!registry.is_loaded(ModelId::Multimodal).await);

    let stats = registry.stats().await;
    assert_eq!(stats.load_failures, 1);
}

// =========================================================================
// EDGE CASE: EDGE-3 - Concurrent Load Race (1 test)
// =========================================================================

#[tokio::test]
async fn test_edge_3_concurrent_load_race() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig {
        enable_debug_logging: true,
        ..Default::default()
    };

    let registry = Arc::new(ModelRegistry::new(config, factory.clone()).await.unwrap());

    // Precondition: Model not loaded
    assert!(!registry.is_loaded(ModelId::Semantic).await);

    // Action: Spawn 100 concurrent get_model calls
    let handles: Vec<_> = (0..100)
        .map(|_| {
            let r = Arc::clone(&registry);
            tokio::spawn(async move { r.get_model(ModelId::Semantic).await })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Expected: Exactly 1 load, 99 cache hits
    assert_eq!(factory.create_count(), 1);

    let stats = registry.stats().await;
    assert_eq!(stats.load_count, 1);
    assert_eq!(stats.cache_hits, 99);

    // Single allocation in MemoryTracker
    assert_eq!(
        registry.total_memory_usage().await,
        get_memory_estimate(ModelId::Semantic)
    );
}
