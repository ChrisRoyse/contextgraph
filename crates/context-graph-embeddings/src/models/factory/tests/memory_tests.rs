//! Memory estimation and thread safety tests.

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::GpuConfig;
use crate::models::factory::DefaultModelFactory;
use crate::traits::{ModelFactory, QuantizationMode};
use crate::types::ModelId;

// =========================================================================
// MEMORY ESTIMATE TESTS (5 tests)
// =========================================================================

#[test]
fn test_estimate_memory_all_nonzero() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    for model_id in ModelId::all() {
        let estimate = factory.estimate_memory(*model_id);
        assert!(
            estimate > 0,
            "Memory estimate for {:?} should be > 0",
            model_id
        );
    }
}

#[test]
fn test_estimate_memory_semantic_largest() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    // Semantic and Graph (both e5-large-v2) are the largest at 1.4GB
    let semantic = factory.estimate_memory(ModelId::Semantic);
    for model_id in ModelId::all() {
        assert!(
            semantic >= factory.estimate_memory(*model_id),
            "Semantic should be >= {:?}",
            model_id
        );
    }
}

#[test]
fn test_estimate_memory_temporal_smallest() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    let temporal_recent = factory.estimate_memory(ModelId::TemporalRecent);
    let temporal_periodic = factory.estimate_memory(ModelId::TemporalPeriodic);
    let temporal_positional = factory.estimate_memory(ModelId::TemporalPositional);

    // All three temporal models have the same (smallest) size
    assert_eq!(temporal_recent, temporal_periodic);
    assert_eq!(temporal_periodic, temporal_positional);
}

#[test]
fn test_estimate_memory_quantized() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    let base = factory.estimate_memory(ModelId::Semantic);
    let fp16 = factory.estimate_memory_quantized(ModelId::Semantic, QuantizationMode::Fp16);
    let int8 = factory.estimate_memory_quantized(ModelId::Semantic, QuantizationMode::Int8);

    assert_eq!(fp16, (base as f32 * 0.5) as usize);
    assert_eq!(int8, (base as f32 * 0.25) as usize);
}

#[test]
fn test_estimate_memory_semantic() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());

    // Semantic model should be ~1.4 GB
    let estimate = factory.estimate_memory(ModelId::Semantic);
    assert!(estimate > 1_000_000_000, "Semantic should be > 1 GB");
    assert!(estimate < 2_000_000_000, "Semantic should be < 2 GB");
}

// =========================================================================
// THREAD SAFETY TESTS (3 tests)
// =========================================================================

#[test]
fn test_factory_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<DefaultModelFactory>();
}

#[test]
fn test_factory_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<DefaultModelFactory>();
}

#[test]
fn test_factory_in_arc() {
    let factory: Arc<dyn ModelFactory> = Arc::new(DefaultModelFactory::new(
        PathBuf::from("./models"),
        GpuConfig::default(),
    ));

    assert_eq!(factory.supported_models().len(), 14);
    assert!(factory.supports_model(ModelId::Semantic));
}
