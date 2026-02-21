//! Model creation tests for all 14 models.

use std::path::PathBuf;

use crate::config::GpuConfig;
use crate::error::EmbeddingError;
use crate::models::factory::DefaultModelFactory;
use crate::traits::{ModelFactory, SingleModelConfig};
use crate::types::ModelId;

// =========================================================================
// CREATE MODEL TESTS - CUSTOM MODELS (5 tests)
// =========================================================================

#[test]
fn test_create_temporal_recent() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::TemporalRecent, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::TemporalRecent);
    assert!(model.is_initialized()); // Custom models are immediately ready
}

#[test]
fn test_create_temporal_periodic() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::TemporalPeriodic, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::TemporalPeriodic);
    assert!(model.is_initialized());
}

#[test]
fn test_create_temporal_positional() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::TemporalPositional, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::TemporalPositional);
    assert!(model.is_initialized());
}

#[test]
fn test_create_hdc() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Hdc, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Hdc);
    assert!(model.is_initialized());
}

#[test]
fn test_create_all_custom_models() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let custom_models = [
        ModelId::TemporalRecent,
        ModelId::TemporalPeriodic,
        ModelId::TemporalPositional,
        ModelId::Hdc,
    ];

    for model_id in custom_models {
        let result = factory.create_model(model_id, &config);
        assert!(
            result.is_ok(),
            "Failed to create {:?}: {:?}",
            model_id,
            result.err()
        );
        let model = result.unwrap();
        assert_eq!(model.model_id(), model_id);
        assert!(model.is_initialized());
    }
}

// =========================================================================
// CREATE MODEL TESTS - PRETRAINED MODELS (8 tests)
// =========================================================================

#[test]
fn test_create_semantic() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Semantic, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Semantic);
    assert!(!model.is_initialized()); // Pretrained models need load()
}

#[test]
fn test_create_causal() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Causal, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Causal);
    assert!(!model.is_initialized());
}

#[test]
fn test_create_sparse() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Sparse, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Sparse);
    assert!(!model.is_initialized());
}

#[test]
fn test_create_code() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Code, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Code);
    assert!(!model.is_initialized());
}

#[test]
fn test_create_graph() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Graph, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Graph);
    assert!(!model.is_initialized());
}

#[test]
fn test_create_multimodal() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Contextual, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Contextual);
    assert!(!model.is_initialized());
}

#[test]
fn test_create_entity() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::Entity, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::Entity);
    assert!(!model.is_initialized());
}

#[test]
fn test_create_late_interaction() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    let result = factory.create_model(ModelId::LateInteraction, &config);
    assert!(result.is_ok(), "Failed: {:?}", result.err());

    let model = result.unwrap();
    assert_eq!(model.model_id(), ModelId::LateInteraction);
    assert!(!model.is_initialized());
}

// =========================================================================
// CREATE ALL 13 MODELS TEST (1 comprehensive test)
// =========================================================================

#[test]
fn test_create_all_13_models() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    for model_id in ModelId::all() {
        let result = factory.create_model(*model_id, &config);
        assert!(
            result.is_ok(),
            "Failed to create {:?}: {:?}",
            model_id,
            result.err()
        );

        let model = result.unwrap();
        assert_eq!(
            model.model_id(),
            *model_id,
            "Model ID mismatch for {:?}",
            model_id
        );
    }
}

// =========================================================================
// CONFIG VALIDATION TESTS (4 tests)
// =========================================================================

#[test]
fn test_create_with_invalid_config_fails() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig {
        max_batch_size: 0, // Invalid
        ..Default::default()
    };

    let result = factory.create_model(ModelId::Semantic, &config);
    assert!(result.is_err());

    match result {
        Err(EmbeddingError::ConfigError { message }) => {
            assert!(message.contains("max_batch_size"));
        }
        other => panic!("Expected ConfigError, got {:?}", other.err()),
    }
}

#[test]
fn test_create_with_cpu_config() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::cpu_only());
    let config = SingleModelConfig::cpu();

    let result = factory.create_model(ModelId::TemporalRecent, &config);
    assert!(result.is_ok());
}

#[test]
fn test_create_with_cuda_fp16_config() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::cuda_fp16();

    let result = factory.create_model(ModelId::Semantic, &config);
    assert!(result.is_ok());
}

#[test]
fn test_create_with_custom_batch_size() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig {
        max_batch_size: 64,
        ..Default::default()
    };

    let result = factory.create_model(ModelId::Code, &config);
    assert!(result.is_ok());
}
