//! Tests for WarmModelRegistry with all 12 models.

use super::helpers::{test_handle, MB};
use crate::warm::error::WarmError;
use crate::warm::registry::{WarmModelRegistry, EMBEDDING_MODEL_IDS, TOTAL_MODEL_COUNT};
use crate::warm::state::WarmModelState;

use std::sync::{Arc, RwLock};

#[test]
fn test_new_registry_is_empty() {
    let registry = WarmModelRegistry::new();
    assert_eq!(registry.model_count(), 0);
    assert!(!registry.all_warm());
    assert!(!registry.any_failed());
}

#[test]
fn test_embedding_model_ids_count() {
    assert_eq!(EMBEDDING_MODEL_IDS.len(), 13);
    assert_eq!(TOTAL_MODEL_COUNT, 13);
}

#[test]
fn test_register_all_12_models() {
    let mut registry = WarmModelRegistry::new();

    for (i, model_id) in EMBEDDING_MODEL_IDS.iter().enumerate() {
        registry
            .register_model(*model_id, (i + 1) * 100 * MB, 768)
            .unwrap();
    }

    assert_eq!(registry.model_count(), TOTAL_MODEL_COUNT);
}

#[test]
fn test_register_duplicate_fails() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * MB, 768)
        .unwrap();

    let err = registry
        .register_model("E1_Semantic", 256 * MB, 512)
        .unwrap_err();

    match err {
        WarmError::ModelAlreadyRegistered { model_id } => {
            assert_eq!(model_id, "E1_Semantic");
        }
        _ => panic!("Expected ModelAlreadyRegistered error"),
    }
}

#[test]
fn test_full_state_transition_pending_to_warm() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * MB, 768)
        .unwrap();

    // Pending -> Loading
    assert!(matches!(
        registry.get_state("E1_Semantic"),
        Some(WarmModelState::Pending)
    ));

    registry.start_loading("E1_Semantic").unwrap();
    assert!(matches!(
        registry.get_state("E1_Semantic"),
        Some(WarmModelState::Loading { .. })
    ));

    // Update progress
    registry
        .update_progress("E1_Semantic", 50, 256 * MB)
        .unwrap();
    if let Some(WarmModelState::Loading {
        progress_percent,
        bytes_loaded,
    }) = registry.get_state("E1_Semantic")
    {
        assert_eq!(progress_percent, 50);
        assert_eq!(bytes_loaded, 256 * MB);
    }

    // Loading -> Validating
    registry.mark_validating("E1_Semantic").unwrap();
    assert!(matches!(
        registry.get_state("E1_Semantic"),
        Some(WarmModelState::Validating)
    ));

    // Validating -> Warm
    registry
        .mark_warm("E1_Semantic", test_handle(512 * MB))
        .unwrap();
    assert!(matches!(
        registry.get_state("E1_Semantic"),
        Some(WarmModelState::Warm)
    ));
    assert!(registry.get_handle("E1_Semantic").is_some());
}

#[test]
fn test_invalid_transition_pending_to_validating() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * MB, 768)
        .unwrap();

    let err = registry.mark_validating("E1_Semantic").unwrap_err();
    assert!(matches!(err, WarmError::ModelValidationFailed { .. }));
}

#[test]
fn test_invalid_transition_pending_to_warm() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * MB, 768)
        .unwrap();

    let err = registry
        .mark_warm("E1_Semantic", test_handle(512 * MB))
        .unwrap_err();
    assert!(matches!(err, WarmError::ModelValidationFailed { .. }));
}

#[test]
fn test_mark_failed_from_loading() {
    let mut registry = WarmModelRegistry::new();
    registry
        .register_model("E1_Semantic", 512 * MB, 768)
        .unwrap();
    registry.start_loading("E1_Semantic").unwrap();

    registry
        .mark_failed("E1_Semantic", 102, "CUDA allocation failed")
        .unwrap();

    match registry.get_state("E1_Semantic") {
        Some(WarmModelState::Failed {
            error_code,
            error_message,
        }) => {
            assert_eq!(error_code, 102);
            assert_eq!(error_message, "CUDA allocation failed");
        }
        _ => panic!("Expected Failed state"),
    }
}

#[test]
fn test_loading_order_largest_first() {
    let mut registry = WarmModelRegistry::new();

    registry.register_model("Small", 100 * MB, 768).unwrap();
    registry.register_model("Large", 500 * MB, 768).unwrap();
    registry.register_model("Medium", 250 * MB, 768).unwrap();

    let order = registry.loading_order();
    assert_eq!(order[0], "Large");
    assert_eq!(order[1], "Medium");
    assert_eq!(order[2], "Small");
}

#[test]
fn test_all_warm_complete() {
    let mut registry = WarmModelRegistry::new();
    registry.register_model("E1", 256 * MB, 768).unwrap();
    registry.register_model("E2", 256 * MB, 768).unwrap();

    for model_id in ["E1", "E2"] {
        registry.start_loading(model_id).unwrap();
        registry.mark_validating(model_id).unwrap();
        registry.mark_warm(model_id, test_handle(256 * MB)).unwrap();
    }

    assert!(registry.all_warm());
    assert_eq!(registry.warm_count(), 2);
}

#[test]
fn test_any_failed_detection() {
    let mut registry = WarmModelRegistry::new();
    registry.register_model("E1", 256 * MB, 768).unwrap();
    registry.register_model("E2", 256 * MB, 768).unwrap();

    registry.start_loading("E1").unwrap();
    registry.mark_failed("E1", 102, "Failed").unwrap();

    assert!(registry.any_failed());
    assert!(!registry.all_warm());

    let failed = registry.failed_entries();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, "E1");
    assert_eq!(failed[0].1, 102);
}

#[test]
fn test_operations_on_unregistered_model() {
    let mut registry = WarmModelRegistry::new();

    assert!(matches!(
        registry.start_loading("NonExistent"),
        Err(WarmError::ModelNotRegistered { .. })
    ));
    assert!(matches!(
        registry.update_progress("NonExistent", 50, 1000),
        Err(WarmError::ModelNotRegistered { .. })
    ));
    assert!(matches!(
        registry.mark_validating("NonExistent"),
        Err(WarmError::ModelNotRegistered { .. })
    ));
    assert!(matches!(
        registry.mark_warm("NonExistent", test_handle(1000)),
        Err(WarmError::ModelNotRegistered { .. })
    ));
}

#[test]
fn test_shared_registry_thread_safety() {
    type SharedRegistry = Arc<RwLock<WarmModelRegistry>>;
    let registry: SharedRegistry = Arc::new(RwLock::new(WarmModelRegistry::new()));

    // Write access
    {
        let mut reg = registry.write().unwrap();
        reg.register_model("E1_Semantic", 512 * MB, 768).unwrap();
    }

    // Read access
    {
        let reg = registry.read().unwrap();
        assert_eq!(reg.model_count(), 1);
    }

    // Clone and access
    let registry_clone = Arc::clone(&registry);
    {
        let reg = registry_clone.read().unwrap();
        assert!(reg.get_state("E1_Semantic").is_some());
    }
}
