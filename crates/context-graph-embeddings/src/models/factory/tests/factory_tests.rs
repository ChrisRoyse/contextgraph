//! Factory construction and model path tests.

use std::path::PathBuf;

use crate::config::GpuConfig;
use crate::models::factory::DefaultModelFactory;
use crate::traits::ModelFactory;
use crate::types::ModelId;

// =========================================================================
// FACTORY CONSTRUCTION TESTS (4 tests)
// =========================================================================

#[test]
fn test_factory_new() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    assert_eq!(factory.models_dir(), &PathBuf::from("./models"));
    assert!(factory.gpu_config().enabled);
}

#[test]
fn test_factory_with_cpu_config() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::cpu_only());
    assert!(!factory.gpu_config().enabled);
}

#[test]
fn test_factory_with_rtx_5090_config() {
    let factory =
        DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::rtx_5090_optimized());
    assert!(factory.gpu_config().enabled);
    assert!(factory.gpu_config().green_contexts);
    assert!(factory.gpu_config().gds_enabled);
}

#[test]
fn test_factory_clone() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let cloned = factory.clone();
    assert_eq!(factory.models_dir(), cloned.models_dir());
}

// =========================================================================
// MODEL PATH TESTS (5 tests)
// =========================================================================

#[test]
fn test_get_model_path_pretrained() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    // Pretrained models should have paths
    assert!(factory.get_model_path(ModelId::Semantic).is_some());
    assert!(factory.get_model_path(ModelId::Causal).is_some());
    assert!(factory.get_model_path(ModelId::Sparse).is_some());
    assert!(factory.get_model_path(ModelId::Code).is_some());
    assert!(factory.get_model_path(ModelId::Graph).is_some());
    assert!(factory.get_model_path(ModelId::Contextual).is_some());
    assert!(factory.get_model_path(ModelId::Entity).is_some());
    assert!(factory.get_model_path(ModelId::LateInteraction).is_some());
}

#[test]
fn test_get_model_path_custom_returns_none() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    // Custom models should return None
    assert!(factory.get_model_path(ModelId::TemporalRecent).is_none());
    assert!(factory.get_model_path(ModelId::TemporalPeriodic).is_none());
    assert!(factory
        .get_model_path(ModelId::TemporalPositional)
        .is_none());
    assert!(factory.get_model_path(ModelId::Hdc).is_none());
}

#[test]
fn test_get_model_path_semantic() {
    let factory = DefaultModelFactory::new(PathBuf::from("/data/models"), GpuConfig::default());
    let path = factory.get_model_path(ModelId::Semantic).unwrap();
    assert_eq!(path, PathBuf::from("/data/models/semantic"));
}

#[test]
fn test_get_model_path_colbert() {
    let factory = DefaultModelFactory::new(PathBuf::from("/data/models"), GpuConfig::default());
    let path = factory.get_model_path(ModelId::LateInteraction).unwrap();
    assert_eq!(path, PathBuf::from("/data/models/late-interaction"));
}

#[test]
fn test_get_model_path_contextual() {
    let factory = DefaultModelFactory::new(PathBuf::from("/data/models"), GpuConfig::default());
    let path = factory.get_model_path(ModelId::Contextual).unwrap();
    assert_eq!(path, PathBuf::from("/data/models/contextual"));
}

// =========================================================================
// SUPPORTED MODELS TESTS (4 tests)
// =========================================================================

#[test]
fn test_supported_models_count() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    assert_eq!(factory.supported_models().len(), 14);
}

#[test]
fn test_supported_models_contains_all() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    for model_id in ModelId::all() {
        assert!(
            factory.supports_model(*model_id),
            "Factory should support {:?}",
            model_id
        );
    }
}

#[test]
fn test_supports_model_semantic() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    assert!(factory.supports_model(ModelId::Semantic));
}

#[test]
fn test_supports_model_late_interaction() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    assert!(factory.supports_model(ModelId::LateInteraction));
}

// =========================================================================
// EDGE CASE TESTS (4 tests)
// =========================================================================

#[test]
fn test_factory_with_empty_path() {
    let factory = DefaultModelFactory::new(PathBuf::from(""), GpuConfig::default());
    let config = crate::traits::SingleModelConfig::default();

    // Should still work for custom models
    let result = factory.create_model(ModelId::TemporalRecent, &config);
    assert!(result.is_ok());
}

#[test]
fn test_factory_with_absolute_path() {
    let factory = DefaultModelFactory::new(
        PathBuf::from("/absolute/path/to/models"),
        GpuConfig::default(),
    );

    let path = factory.get_model_path(ModelId::Semantic).unwrap();
    assert!(path.starts_with("/absolute/path"));
}

#[test]
fn test_factory_with_relative_path() {
    let factory =
        DefaultModelFactory::new(PathBuf::from("./relative/models"), GpuConfig::default());

    let path = factory.get_model_path(ModelId::Semantic).unwrap();
    assert!(path.starts_with("./relative"));
}

#[test]
fn test_factory_debug_impl() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let debug_str = format!("{:?}", factory);
    assert!(debug_str.contains("DefaultModelFactory"));
    assert!(debug_str.contains("./models"));
}
