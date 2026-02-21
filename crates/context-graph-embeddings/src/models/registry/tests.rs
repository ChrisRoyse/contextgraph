//! Tests for ModelRegistry.
//!
//! Comprehensive test suite covering configuration, registry creation,
//! initialization, load/unload operations, concurrency, and edge cases.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::{get_memory_estimate, EmbeddingModel, ModelFactory, SingleModelConfig};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::config::ModelRegistryConfig;
use super::core::ModelRegistry;

// =========================================================================
// Test Factory Implementation
// =========================================================================

/// Test implementation of EmbeddingModel for registry testing.
pub(super) struct TestModel {
    model_id: ModelId,
    initialized: AtomicBool,
}

impl TestModel {
    pub fn new(model_id: ModelId) -> Self {
        Self {
            model_id,
            initialized: AtomicBool::new(true),
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingModel for TestModel {
    fn model_id(&self) -> ModelId {
        self.model_id
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id,
            });
        }

        self.validate_input(input)?;

        let dim = self.dimension();
        let vector: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001).sin()).collect();
        Ok(ModelEmbedding::new(self.model_id, vector, 100))
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }
}

/// Test factory that creates TestModel instances.
pub(super) struct TestFactory {
    /// Count of create_model calls for testing
    create_count: AtomicU64,
}

impl TestFactory {
    pub fn new() -> Self {
        Self {
            create_count: AtomicU64::new(0),
        }
    }

    pub fn create_count(&self) -> u64 {
        self.create_count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl ModelFactory for TestFactory {
    fn create_model(
        &self,
        model_id: ModelId,
        config: &SingleModelConfig,
    ) -> EmbeddingResult<Box<dyn EmbeddingModel>> {
        config.validate()?;

        if !self.supports_model(model_id) {
            return Err(EmbeddingError::ModelNotFound { model_id });
        }

        self.create_count.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(TestModel::new(model_id)))
    }

    fn supported_models(&self) -> &[ModelId] {
        ModelId::all()
    }

    fn estimate_memory(&self, model_id: ModelId) -> usize {
        get_memory_estimate(model_id)
    }
}

// =========================================================================
// CONFIG TESTS (5 tests)
// =========================================================================

#[test]
fn test_config_default() {
    let config = ModelRegistryConfig::default();
    assert_eq!(config.max_concurrent_loads, 4);
    assert_eq!(config.memory_budget_bytes, 32_000_000_000);
    assert!(config.preload_models.is_empty());
    assert!(!config.enable_debug_logging);
}

#[test]
fn test_config_rtx_5090() {
    let config = ModelRegistryConfig::rtx_5090();
    assert_eq!(config.memory_budget_bytes, 32_000_000_000);
}

#[test]
fn test_config_rtx_4090() {
    let config = ModelRegistryConfig::rtx_4090();
    assert_eq!(config.memory_budget_bytes, 24_000_000_000);
}

#[test]
fn test_config_validate_success() {
    let config = ModelRegistryConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validate_zero_concurrent_loads_fails() {
    let config = ModelRegistryConfig {
        max_concurrent_loads: 0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::ConfigError { message }) => {
            assert!(message.contains("max_concurrent_loads"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_config_validate_zero_budget_fails() {
    let config = ModelRegistryConfig {
        memory_budget_bytes: 0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::ConfigError { message }) => {
            assert!(message.contains("memory_budget_bytes"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_config_validate_duplicate_preload_fails() {
    let config = ModelRegistryConfig {
        preload_models: vec![ModelId::Semantic, ModelId::Code, ModelId::Semantic],
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::ConfigError { message }) => {
            assert!(message.contains("duplicate"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

// =========================================================================
// REGISTRY CREATION TESTS (3 tests)
// =========================================================================

#[tokio::test]
async fn test_registry_new_success() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let result = ModelRegistry::new(config, factory).await;
    assert!(result.is_ok());

    let registry = result.unwrap();
    assert_eq!(registry.loaded_count().await, 0);
    assert_eq!(registry.total_memory_usage().await, 0);
}

#[tokio::test]
async fn test_registry_new_with_invalid_config_fails() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig {
        max_concurrent_loads: 0,
        ..Default::default()
    };

    let result = ModelRegistry::new(config, factory).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_registry_new_creates_locks_for_all_models() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    // Verify loading_locks has entries for all 14 models
    for model_id in ModelId::all() {
        assert!(
            registry.loading_locks.contains_key(model_id),
            "Missing lock for {:?}",
            model_id
        );
    }
}

// =========================================================================
// INITIALIZE TESTS (4 tests)
// =========================================================================

#[tokio::test]
async fn test_initialize_with_no_preload() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    let result = registry.initialize().await;

    assert!(result.is_ok());
    assert_eq!(registry.loaded_count().await, 0);
}

#[tokio::test]
async fn test_initialize_preloads_models() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig {
        preload_models: vec![ModelId::Semantic, ModelId::Code],
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory.clone()).await.unwrap();
    registry.initialize().await.unwrap();

    assert_eq!(registry.loaded_count().await, 2);
    assert!(registry.is_loaded(ModelId::Semantic).await);
    assert!(registry.is_loaded(ModelId::Code).await);
    assert_eq!(factory.create_count(), 2);
}

#[tokio::test]
async fn test_initialize_updates_stats() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig {
        preload_models: vec![ModelId::Graph, ModelId::Entity],
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    registry.initialize().await.unwrap();

    let stats = registry.stats().await;
    assert_eq!(stats.load_count, 2);
    assert_eq!(stats.loaded_count, 2);
}

#[tokio::test]
async fn test_initialize_fails_if_memory_exceeded() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig {
        // Budget only allows one model
        memory_budget_bytes: 200_000_000,
        preload_models: vec![ModelId::Semantic, ModelId::Contextual], // Both > 200MB
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    let result = registry.initialize().await;

    assert!(result.is_err());
    // First model may have loaded before failure
    assert!(registry.loaded_count().await <= 1);
}

// =========================================================================
// LOAD MODEL TESTS (6 tests)
// =========================================================================

#[tokio::test]
async fn test_load_model_success() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    let result = registry.load_model(ModelId::Semantic).await;

    assert!(result.is_ok());
    assert!(registry.is_loaded(ModelId::Semantic).await);
}

#[tokio::test]
async fn test_load_model_updates_memory() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    registry.load_model(ModelId::Semantic).await.unwrap();

    let expected_memory = get_memory_estimate(ModelId::Semantic);
    assert_eq!(registry.total_memory_usage().await, expected_memory);
}

#[tokio::test]
async fn test_load_model_updates_stats() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    registry.load_model(ModelId::Code).await.unwrap();

    let stats = registry.stats().await;
    assert_eq!(stats.load_count, 1);
    assert_eq!(stats.loaded_count, 1);
}

#[tokio::test]
async fn test_load_model_fails_when_budget_exceeded() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig {
        memory_budget_bytes: 100_000_000, // 100MB - smaller than Semantic
        ..Default::default()
    };

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    let result = registry.load_model(ModelId::Semantic).await;

    assert!(result.is_err());
    match result {
        Err(EmbeddingError::MemoryBudgetExceeded { .. }) => {}
        _ => panic!("Expected MemoryBudgetExceeded error"),
    }

    assert!(!registry.is_loaded(ModelId::Semantic).await);
    assert_eq!(registry.total_memory_usage().await, 0);

    let stats = registry.stats().await;
    assert_eq!(stats.load_failures, 1);
}

#[tokio::test]
async fn test_load_already_loaded_model_is_cache_hit() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory.clone()).await.unwrap();
    registry.load_model(ModelId::Semantic).await.unwrap();

    // Load again
    registry.load_model(ModelId::Semantic).await.unwrap();

    // Should only create once
    assert_eq!(factory.create_count(), 1);

    let stats = registry.stats().await;
    assert_eq!(stats.load_count, 1);
    assert_eq!(stats.cache_hits, 1);
}

#[tokio::test]
async fn test_load_multiple_models() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();

    // Load 5 models
    registry.load_model(ModelId::Semantic).await.unwrap();
    registry.load_model(ModelId::Code).await.unwrap();
    registry.load_model(ModelId::Graph).await.unwrap();
    registry.load_model(ModelId::Entity).await.unwrap();
    registry.load_model(ModelId::Hdc).await.unwrap();

    assert_eq!(registry.loaded_count().await, 5);

    let loaded = registry.loaded_models().await;
    assert!(loaded.contains(&ModelId::Semantic));
    assert!(loaded.contains(&ModelId::Code));
    assert!(loaded.contains(&ModelId::Graph));
    assert!(loaded.contains(&ModelId::Entity));
    assert!(loaded.contains(&ModelId::Hdc));
}
