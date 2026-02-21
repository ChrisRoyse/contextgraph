//! Cross-component integration tests.

use super::helpers::{test_config, test_handle, test_handle_full, MB};
use crate::warm::memory_pool::WarmMemoryPools;
use crate::warm::registry::{WarmModelRegistry, EMBEDDING_MODEL_IDS, TOTAL_MODEL_COUNT};
use crate::warm::state::WarmModelState;
use crate::warm::validation::{TestInferenceConfig, WarmValidator};

#[test]
fn test_full_warm_loading_pipeline_simulation() {
    // Simulate complete warm loading pipeline
    let config = test_config();
    let mut registry = WarmModelRegistry::new();
    let mut pools = WarmMemoryPools::new(config.clone());
    let validator = WarmValidator::new();

    // Register all 14 models with realistic sizes
    let model_sizes: Vec<(&str, usize)> = EMBEDDING_MODEL_IDS
        .iter()
        .map(|id| (*id, 600 * MB))
        .collect();

    for (model_id, size) in &model_sizes {
        registry.register_model(*model_id, *size, 768).unwrap();
    }

    assert_eq!(registry.model_count(), TOTAL_MODEL_COUNT);

    // Load in order (largest first)
    for model_id in registry.loading_order() {
        // Start loading
        registry.start_loading(&model_id).unwrap();

        let entry = registry.get_entry(&model_id).unwrap();
        let size = entry.expected_bytes;
        let dimension = entry.expected_dimension;

        // Allocate VRAM
        let vram_ptr = 0x1000_0000_u64 + (pools.total_allocated_bytes() as u64);
        pools.allocate_model(&model_id, size, vram_ptr).unwrap();

        // Simulate loading progress
        registry.update_progress(&model_id, 50, size / 2).unwrap();
        registry.update_progress(&model_id, 100, size).unwrap();

        // Validation
        registry.mark_validating(&model_id).unwrap();

        // Simulate inference output
        let output: Vec<f32> = vec![0.1; dimension];
        let config = TestInferenceConfig::for_embedding_model(&model_id, dimension);
        let handle = test_handle_full(vram_ptr, size, 0, 0xABCD);

        let result = validator.validate_model(&config, &handle, &output);
        assert!(result.is_valid(), "Validation failed for {}", model_id);

        // Mark warm
        registry.mark_warm(&model_id, handle).unwrap();
    }

    // Verify final state
    assert!(registry.all_warm());
    assert!(!registry.any_failed());
    assert_eq!(registry.warm_count(), TOTAL_MODEL_COUNT);
    assert!(pools.is_within_budget());

    // Verify all handles are accessible
    for model_id in EMBEDDING_MODEL_IDS {
        assert!(
            registry.get_handle(model_id).is_some(),
            "Missing handle for {}",
            model_id
        );
    }
}

#[test]
fn test_fail_fast_on_first_validation_failure() {
    let mut registry = WarmModelRegistry::new();
    let mut pools = WarmMemoryPools::rtx_5090();
    let validator = WarmValidator::new();

    // Register 3 models
    registry.register_model("E1", 500 * MB, 768).unwrap();
    registry.register_model("E2", 500 * MB, 768).unwrap();
    registry.register_model("E3", 500 * MB, 768).unwrap();

    // Load E1 successfully
    registry.start_loading("E1").unwrap();
    pools.allocate_model("E1", 500 * MB, 0x1000).unwrap();
    registry.update_progress("E1", 100, 500 * MB).unwrap();
    registry.mark_validating("E1").unwrap();

    let output = vec![0.1; 768];
    let config = TestInferenceConfig::for_embedding_model("E1", 768);
    let handle = test_handle(500 * MB);
    let result = validator.validate_model(&config, &handle, &output);
    assert!(result.is_valid());

    registry.mark_warm("E1", handle).unwrap();

    // Load E2 - validation fails (NaN in output)
    registry.start_loading("E2").unwrap();
    pools.allocate_model("E2", 500 * MB, 0x2000).unwrap();
    registry.update_progress("E2", 100, 500 * MB).unwrap();
    registry.mark_validating("E2").unwrap();

    let bad_output = vec![f32::NAN; 768];
    let config = TestInferenceConfig::for_embedding_model("E2", 768);
    let result = validator.validate_model(&config, &test_handle(500 * MB), &bad_output);
    assert!(!result.is_valid());
    assert!(!result.weights_valid);

    // Mark as failed with exit code 103
    registry.mark_failed("E2", 103, "NaN in output").unwrap();

    // FAIL-FAST: Don't continue loading E3
    assert!(registry.any_failed());
    assert!(!registry.all_warm());

    // E3 should still be Pending
    assert!(matches!(
        registry.get_state("E3"),
        Some(WarmModelState::Pending)
    ));
}
