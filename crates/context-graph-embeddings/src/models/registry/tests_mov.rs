//! Manual output verification tests for ModelRegistry.

use std::sync::Arc;

use crate::traits::ModelFactory;
use crate::types::{ModelId, ModelInput};

use super::config::ModelRegistryConfig;
use super::core::ModelRegistry;
use super::tests::TestFactory;

// =========================================================================
// MANUAL OUTPUT VERIFICATION TESTS (4 tests)
// =========================================================================

#[tokio::test]
async fn test_mov_1_model_instance_valid() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    registry.load_model(ModelId::Semantic).await.unwrap();

    let model = registry.get_model(ModelId::Semantic).await.unwrap();

    // Verify Arc has at least 2 refs (registry + this)
    assert!(Arc::strong_count(&model) >= 2);

    // Verify model is functional
    let input = ModelInput::text("Test").unwrap();
    let embedding = model.embed(&input).await.unwrap();
    assert_eq!(embedding.model_id, ModelId::Semantic);
}

#[tokio::test]
async fn test_mov_2_memory_tracker_consistency() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory.clone()).await.unwrap();

    // Load several models
    registry.load_model(ModelId::Semantic).await.unwrap();
    registry.load_model(ModelId::Code).await.unwrap();
    registry.load_model(ModelId::Graph).await.unwrap();

    // Calculate expected from factory estimates
    let loaded = registry.loaded_models().await;
    let expected: usize = loaded.iter().map(|id| factory.estimate_memory(*id)).sum();

    let actual = registry.total_memory_usage().await;

    // Verify within 1% tolerance
    let diff = expected.abs_diff(actual);
    let tolerance = expected / 100;
    assert!(
        diff <= tolerance,
        "Memory mismatch: expected {}, actual {}, diff {}, tolerance {}",
        expected,
        actual,
        diff,
        tolerance
    );
}

#[tokio::test]
async fn test_mov_3_statistics_accuracy() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    // Perform N loads
    let n = 3;
    registry.load_model(ModelId::Semantic).await.unwrap();
    registry.load_model(ModelId::Code).await.unwrap();
    registry.load_model(ModelId::Graph).await.unwrap();

    // Perform M unloads
    let m = 1;
    registry.unload_model(ModelId::Code).await.unwrap();

    // Perform P cache hits
    let p = 2;
    let _ = registry.get_model(ModelId::Semantic).await.unwrap();
    let _ = registry.get_model(ModelId::Graph).await.unwrap();

    let stats = registry.stats().await;
    assert_eq!(stats.load_count, n as u64);
    assert_eq!(stats.unload_count, m as u64);
    assert_eq!(stats.cache_hits, p as u64);
}

#[tokio::test]
async fn test_mov_4_thread_safety_under_load() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = Arc::new(ModelRegistry::new(config, factory).await.unwrap());

    // Pre-load some models
    registry.load_model(ModelId::Semantic).await.unwrap();
    registry.load_model(ModelId::Code).await.unwrap();

    // Run 100 concurrent operations (mix of load/unload/get)
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let r = Arc::clone(&registry);
            tokio::spawn(async move {
                match i % 10 {
                    0..=6 => {
                        // 70% get operations
                        let model_id = if i % 2 == 0 {
                            ModelId::Semantic
                        } else {
                            ModelId::Code
                        };
                        let _ = r.get_model(model_id).await;
                    }
                    7..=8 => {
                        // 20% load operations (may fail if already loaded)
                        let _ = r.load_model(ModelId::Graph).await;
                    }
                    _ => {
                        // 10% stats operations
                        let _ = r.stats().await;
                    }
                }
            })
        })
        .collect();

    // All operations should complete without panic or deadlock
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok(), "Task panicked or failed to complete");
    }

    // Registry should still be functional
    let stats = registry.stats().await;
    assert!(stats.loaded_count >= 2); // At least the pre-loaded models
}
