//! Query-related tests for the warm model registry.

use super::*;
use crate::warm::handle::ModelHandle;

/// Helper to create a test ModelHandle
fn test_handle(bytes: usize) -> ModelHandle {
    ModelHandle::new(0x1000_0000, bytes, 0, 0xDEAD_BEEF)
}

// ==================== Query Tests ====================

#[test]
fn test_get_state_unregistered_model() {
    let registry = WarmModelRegistry::new();
    assert!(registry.get_state("NonExistent").is_none());
}

#[test]
fn test_get_handle_not_warm() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * 1024 * 1024, 768)
        .unwrap();

    // No handle in Pending state
    assert!(registry.get_handle("E1_Semantic").is_none());

    registry.start_loading("E1_Semantic").unwrap();
    // No handle in Loading state
    assert!(registry.get_handle("E1_Semantic").is_none());

    registry.mark_validating("E1_Semantic").unwrap();
    // No handle in Validating state
    assert!(registry.get_handle("E1_Semantic").is_none());
}

#[test]
fn test_get_handle_warm() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * 1024 * 1024, 768)
        .unwrap();
    registry.start_loading("E1_Semantic").unwrap();
    registry.mark_validating("E1_Semantic").unwrap();
    registry
        .mark_warm("E1_Semantic", test_handle(512 * 1024 * 1024))
        .unwrap();

    let handle = registry.get_handle("E1_Semantic").unwrap();
    assert_eq!(handle.vram_address(), 0x1000_0000);
    assert_eq!(handle.allocation_bytes(), 512 * 1024 * 1024);
}

// ==================== all_warm and any_failed Tests ====================

#[test]
fn test_all_warm_empty_registry() {
    let registry = WarmModelRegistry::new();
    // Empty registry is NOT all warm
    assert!(!registry.all_warm());
}

#[test]
fn test_all_warm_partial() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * 1024 * 1024, 768)
        .unwrap();
    registry
        .register_model("E2_TemporalRecent", 256 * 1024 * 1024, 768)
        .unwrap();

    // Warm first model only
    registry.start_loading("E1_Semantic").unwrap();
    registry.mark_validating("E1_Semantic").unwrap();
    registry
        .mark_warm("E1_Semantic", test_handle(512 * 1024 * 1024))
        .unwrap();

    // Not all warm (E2 is still Pending)
    assert!(!registry.all_warm());
    assert_eq!(registry.warm_count(), 1);
}

#[test]
fn test_all_warm_complete() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * 1024 * 1024, 768)
        .unwrap();
    registry
        .register_model("E2_TemporalRecent", 256 * 1024 * 1024, 768)
        .unwrap();

    // Warm both models
    for model_id in ["E1_Semantic", "E2_TemporalRecent"] {
        registry.start_loading(model_id).unwrap();
        registry.mark_validating(model_id).unwrap();
        registry
            .mark_warm(model_id, test_handle(256 * 1024 * 1024))
            .unwrap();
    }

    assert!(registry.all_warm());
    assert_eq!(registry.warm_count(), 2);
}

#[test]
fn test_any_failed_none() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * 1024 * 1024, 768)
        .unwrap();
    registry.start_loading("E1_Semantic").unwrap();

    assert!(!registry.any_failed());
}

#[test]
fn test_any_failed_one() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * 1024 * 1024, 768)
        .unwrap();
    registry
        .register_model("E2_TemporalRecent", 256 * 1024 * 1024, 768)
        .unwrap();

    registry.start_loading("E1_Semantic").unwrap();
    registry
        .mark_failed("E1_Semantic", 102, "Failed to load")
        .unwrap();

    assert!(registry.any_failed());
}

// ==================== loading_order Tests ====================

#[test]
fn test_loading_order_descending() {
    let mut registry = WarmModelRegistry::new();

    // Register with different sizes (not in size order)
    registry
        .register_model("Small", 100 * 1024 * 1024, 768)
        .unwrap();
    registry
        .register_model("Large", 500 * 1024 * 1024, 768)
        .unwrap();
    registry
        .register_model("Medium", 250 * 1024 * 1024, 768)
        .unwrap();

    let order = registry.loading_order();

    // Should be sorted largest to smallest
    assert_eq!(order.len(), 3);
    assert_eq!(order[0], "Large");
    assert_eq!(order[1], "Medium");
    assert_eq!(order[2], "Small");
}

#[test]
fn test_loading_order_empty() {
    let registry = WarmModelRegistry::new();
    let order = registry.loading_order();
    assert!(order.is_empty());
}

#[test]
fn test_loading_order_with_all_models() {
    let mut registry = WarmModelRegistry::new();

    // Register models with varying sizes
    let model_sizes = [
        ("E1_Semantic", 500),
        ("E2_TemporalRecent", 200),
        ("E3_TemporalPeriodic", 300),
        ("E4_TemporalPositional", 150),
        ("E10_Multimodal", 800),
    ];

    for (id, size_mb) in model_sizes {
        registry
            .register_model(id, size_mb * 1024 * 1024, 768)
            .unwrap();
    }

    let order = registry.loading_order();

    // E10_Multimodal is largest, should be first
    assert_eq!(order[0], "E10_Multimodal");
    // E1_Semantic is second largest
    assert_eq!(order[1], "E1_Semantic");
    // E4_TemporalPositional is smallest, should be last
    assert_eq!(order[4], "E4_TemporalPositional");
}

// ==================== failed_entries Tests ====================

#[test]
fn test_failed_entries_empty() {
    let registry = WarmModelRegistry::new();
    assert!(registry.failed_entries().is_empty());
}

#[test]
fn test_failed_entries_multiple() {
    let mut registry = WarmModelRegistry::new();

    registry
        .register_model("E1_Semantic", 512 * 1024 * 1024, 768)
        .unwrap();
    registry
        .register_model("E2_TemporalRecent", 256 * 1024 * 1024, 768)
        .unwrap();
    registry
        .register_model("E3_TemporalPeriodic", 256 * 1024 * 1024, 768)
        .unwrap();

    // Fail E1 and E3
    registry.start_loading("E1_Semantic").unwrap();
    registry
        .mark_failed("E1_Semantic", 102, "CUDA error")
        .unwrap();

    registry.start_loading("E3_TemporalPeriodic").unwrap();
    registry
        .mark_failed("E3_TemporalPeriodic", 104, "VRAM exhausted")
        .unwrap();

    let failed = registry.failed_entries();
    assert_eq!(failed.len(), 2);

    // Find E1 failure
    let e1_failure = failed.iter().find(|(id, _, _)| id == "E1_Semantic");
    assert!(e1_failure.is_some());
    let (_, code, msg) = e1_failure.unwrap();
    assert_eq!(*code, 102);
    assert_eq!(msg, "CUDA error");

    // Find E3 failure
    let e3_failure = failed.iter().find(|(id, _, _)| id == "E3_TemporalPeriodic");
    assert!(e3_failure.is_some());
    let (_, code, msg) = e3_failure.unwrap();
    assert_eq!(*code, 104);
    assert_eq!(msg, "VRAM exhausted");
}
