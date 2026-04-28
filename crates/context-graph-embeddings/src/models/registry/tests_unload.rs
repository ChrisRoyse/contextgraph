//! Unload and get model tests for ModelRegistry.

use std::sync::Arc;

use crate::error::EmbeddingError;
use crate::types::ModelId;

use super::config::ModelRegistryConfig;
use super::core::ModelRegistry;
use super::tests::TestFactory;

#[tokio::test]
async fn test_unload_model_success() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    registry.load_model(ModelId::Semantic).await.unwrap();

    let result = registry.unload_model(ModelId::Semantic).await;

    assert!(result.is_ok());
    assert!(!registry.is_loaded(ModelId::Semantic).await);
}

#[tokio::test]
async fn test_get_model_lazy_loads() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory.clone()).await.unwrap();

    assert!(!registry.is_loaded(ModelId::Semantic).await);

    let model = registry.get_model(ModelId::Semantic).await.unwrap();

    assert!(registry.is_loaded(ModelId::Semantic).await);
    assert_eq!(model.model_id(), ModelId::Semantic);
    assert_eq!(factory.create_count(), 1);
}

#[tokio::test]
async fn test_unload_refuses_live_external_handle() {
    let factory = Arc::new(TestFactory::new());
    let config = ModelRegistryConfig::default();

    let registry = ModelRegistry::new(config, factory).await.unwrap();
    let model = registry.get_model(ModelId::Semantic).await.unwrap();

    let result = registry.unload_model(ModelId::Semantic).await;
    assert!(matches!(
        result,
        Err(EmbeddingError::ModelInUse {
            model_id: ModelId::Semantic,
            ref_count: 1
        })
    ));

    drop(model);
    registry.unload_model(ModelId::Semantic).await.unwrap();
    assert!(!registry.is_loaded(ModelId::Semantic).await);
}
