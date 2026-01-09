//! Tests for WarmLoader orchestration logic (integration with Registry + Pools).

use super::helpers::{test_handle, GB, MB};
use crate::warm::error::WarmError;
use crate::warm::memory_pool::WarmMemoryPools;
use crate::warm::registry::{WarmModelRegistry, EMBEDDING_MODEL_IDS, TOTAL_MODEL_COUNT};

#[test]
fn test_loader_orchestration_simulation() {
    // Simulate what WarmLoader does: register, allocate, transition
    let mut registry = WarmModelRegistry::new();
    let mut pools = WarmMemoryPools::rtx_5090();

    let models = [("E1_Semantic", 800 * MB), ("E2_Temporal", 600 * MB)];

    // Register models
    for (model_id, size) in models {
        registry.register_model(model_id, size, 768).unwrap();
    }

    // Get loading order (largest first)
    let order = registry.loading_order();
    assert_eq!(order[0], "E1_Semantic"); // 800MB > 600MB

    // Simulate loading each model
    for model_id in &order {
        // Start loading
        registry.start_loading(model_id).unwrap();

        // Get expected size
        let entry = registry.get_entry(model_id).unwrap();
        let size = entry.expected_bytes;

        // Allocate VRAM
        let vram_ptr = 0x1000_0000 + (size as u64);
        pools.allocate_model(model_id, size, vram_ptr).unwrap();

        // Update progress
        registry.update_progress(model_id, 100, size).unwrap();

        // Mark validating then warm
        registry.mark_validating(model_id).unwrap();
        registry.mark_warm(model_id, test_handle(size)).unwrap();
    }

    assert!(registry.all_warm());
    assert!(pools.is_within_budget());
}

#[test]
fn test_loader_fail_fast_on_vram_exhaustion() {
    let mut registry = WarmModelRegistry::new();
    let mut pools = WarmMemoryPools::rtx_5090();

    // Register models that exceed VRAM budget
    registry.register_model("Huge1", 20 * GB, 768).unwrap();
    registry.register_model("Huge2", 10 * GB, 768).unwrap(); // Total 30GB > 24GB

    let order = registry.loading_order();

    // Load first model
    let model_id = &order[0];
    registry.start_loading(model_id).unwrap();
    let size = registry.get_entry(model_id).unwrap().expected_bytes;
    pools.allocate_model(model_id, size, 0x1000).unwrap();
    registry.update_progress(model_id, 100, size).unwrap();
    registry.mark_validating(model_id).unwrap();
    registry.mark_warm(model_id, test_handle(size)).unwrap();

    // Try to load second model - should fail
    let model_id = &order[1];
    registry.start_loading(model_id).unwrap();
    let size = registry.get_entry(model_id).unwrap().expected_bytes;

    let result = pools.allocate_model(model_id, size, 0x2000);
    assert!(matches!(
        result,
        Err(WarmError::VramAllocationFailed { .. })
    ));

    // Mark as failed in registry
    registry
        .mark_failed(model_id, 104, "VRAM exhausted")
        .unwrap();

    assert!(registry.any_failed());
    assert!(!registry.all_warm());
}

#[test]
fn test_loader_all_12_models_fit_in_24gb() {
    let mut registry = WarmModelRegistry::new();
    let mut pools = WarmMemoryPools::rtx_5090();

    // Each model ~1.5GB (12 * 1.5GB = 18GB < 24GB)
    let model_size = (1536) * MB;

    for model_id in EMBEDDING_MODEL_IDS {
        registry.register_model(model_id, model_size, 768).unwrap();
    }

    assert_eq!(registry.model_count(), TOTAL_MODEL_COUNT);

    // Load all models
    for model_id in registry.loading_order() {
        registry.start_loading(&model_id).unwrap();
        let size = registry.get_entry(&model_id).unwrap().expected_bytes;
        pools
            .allocate_model(&model_id, size, 0x1000 + size as u64)
            .unwrap();
        registry.update_progress(&model_id, 100, size).unwrap();
        registry.mark_validating(&model_id).unwrap();
        registry.mark_warm(&model_id, test_handle(size)).unwrap();
    }

    assert!(registry.all_warm());
    assert_eq!(registry.warm_count(), TOTAL_MODEL_COUNT);
    assert!(pools.is_within_budget());
}
