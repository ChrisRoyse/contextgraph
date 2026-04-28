//! Tests for ModelRegistry.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::{EmbeddingModel, ModelFactory, SingleModelConfig};
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
            initialized: AtomicBool::new(false),
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

    async fn load(&self) -> EmbeddingResult<()> {
        self.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn unload(&self) -> EmbeddingResult<()> {
        self.initialized.store(false, Ordering::SeqCst);
        Ok(())
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
        crate::traits::get_memory_estimate(model_id)
    }
}

// =========================================================================
// REGISTRY TESTS
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
async fn test_load_model_success() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    let result = registry.load_model(ModelId::Semantic).await;

    assert!(result.is_ok());
    assert!(registry.is_loaded(ModelId::Semantic).await);
    assert!(registry
        .get_cached_model(ModelId::Semantic)
        .await
        .unwrap()
        .is_initialized());
}
