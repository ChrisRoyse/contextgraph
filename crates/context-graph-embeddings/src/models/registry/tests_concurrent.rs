//! Concurrency tests for ModelRegistry.

use std::sync::Arc;

use crate::traits::get_memory_estimate;
use crate::types::ModelId;

use super::config::ModelRegistryConfig;
use super::core::ModelRegistry;
use super::tests::TestFactory;

// =========================================================================
// CONCURRENCY TESTS (3 tests)
// =========================================================================

#[tokio::test]
async fn test_concurrent_get_same_model_loads_once() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = Arc::new(ModelRegistry::new(config, factory.clone()).await.unwrap());

    // Spawn 100 concurrent get_model calls
    let handles: Vec<_> = (0..100)
        .map(|_| {
            let r = Arc::clone(&registry);
            tokio::spawn(async move { r.get_model(ModelId::Semantic).await })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Should only have created the model once
    assert_eq!(factory.create_count(), 1);

    let stats = registry.stats().await;
    assert_eq!(stats.load_count, 1);
    // 99 cache hits (first one triggers load)
    assert_eq!(stats.cache_hits, 99);
}

#[tokio::test]
async fn test_concurrent_load_different_models() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = Arc::new(ModelRegistry::new(config, factory).await.unwrap());

    let models = [
        ModelId::Semantic,
        ModelId::Code,
        ModelId::Graph,
        ModelId::Entity,
        ModelId::Hdc,
    ];

    let handles: Vec<_> = models
        .iter()
        .map(|model_id| {
            let r = Arc::clone(&registry);
            let mid = *model_id;
            tokio::spawn(async move { r.load_model(mid).await })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    assert_eq!(registry.loaded_count().await, 5);
}

#[tokio::test]
async fn test_concurrent_load_unload() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = Arc::new(ModelRegistry::new(config, factory).await.unwrap());

    // First load
    registry.load_model(ModelId::Semantic).await.unwrap();

    // Concurrent operations
    let r1 = Arc::clone(&registry);
    let r2 = Arc::clone(&registry);

    let handle1 = tokio::spawn(async move { r1.get_model(ModelId::Semantic).await });

    let handle2 = tokio::spawn(async move {
        // Small delay to ensure get_model starts first
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        r2.unload_model(ModelId::Semantic).await
    });

    let result1 = handle1.await.unwrap();
    let result2 = handle2.await.unwrap();

    // Both should succeed - get_model gets Arc before unload
    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

// =========================================================================
// MEMORY TRACKING TESTS (3 tests)
// =========================================================================

#[tokio::test]
async fn test_memory_tracking_accurate() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    registry.load_model(ModelId::Semantic).await.unwrap();
    registry.load_model(ModelId::Code).await.unwrap();

    let expected = get_memory_estimate(ModelId::Semantic) + get_memory_estimate(ModelId::Code);
    assert_eq!(registry.total_memory_usage().await, expected);
}

#[tokio::test]
async fn test_remaining_memory_accurate() {
    let factory = Arc::new(TestFactory::new());
    let budget = 5_000_000_000; // 5GB
    let config = ModelRegistryConfig {
        memory_budget_bytes: budget,
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    registry.load_model(ModelId::Graph).await.unwrap();

    let expected_remaining = budget - get_memory_estimate(ModelId::Graph);
    assert_eq!(registry.remaining_memory().await, expected_remaining);
}

#[tokio::test]
async fn test_load_all_14_models_memory_matches() {
    let factory = Arc::new(TestFactory::new());
    let total: usize = crate::traits::MEMORY_ESTIMATES
        .iter()
        .map(|(_, m)| *m)
        .sum();
    let config = ModelRegistryConfig {
        memory_budget_bytes: total + 1_000_000,
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    for model_id in ModelId::all() {
        registry.load_model(*model_id).await.unwrap();
    }

    assert_eq!(registry.loaded_count().await, 14);
    assert_eq!(registry.total_memory_usage().await, total);
}
