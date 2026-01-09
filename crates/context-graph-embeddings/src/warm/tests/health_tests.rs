//! Tests for WarmHealthChecker status monitoring.

use super::helpers::{test_handle, MB};
use crate::warm::registry::WarmModelRegistry;
use crate::warm::state::WarmModelState;

#[test]
fn test_health_status_from_registry_all_warm() {
    let mut registry = WarmModelRegistry::new();
    registry.register_model("E1", 256 * MB, 768).unwrap();
    registry.register_model("E2", 256 * MB, 768).unwrap();

    for model_id in ["E1", "E2"] {
        registry.start_loading(model_id).unwrap();
        registry.mark_validating(model_id).unwrap();
        registry.mark_warm(model_id, test_handle(256 * MB)).unwrap();
    }

    // Healthy: all models warm
    assert!(registry.all_warm());
    assert!(!registry.any_failed());
    assert_eq!(registry.warm_count(), 2);
}

#[test]
fn test_health_status_from_registry_loading() {
    let mut registry = WarmModelRegistry::new();
    registry.register_model("E1", 256 * MB, 768).unwrap();
    registry.register_model("E2", 256 * MB, 768).unwrap();

    // E1 is warm, E2 is still loading
    registry.start_loading("E1").unwrap();
    registry.mark_validating("E1").unwrap();
    registry.mark_warm("E1", test_handle(256 * MB)).unwrap();

    registry.start_loading("E2").unwrap();
    registry.update_progress("E2", 50, 128 * MB).unwrap();

    // Loading: not all warm, none failed
    assert!(!registry.all_warm());
    assert!(!registry.any_failed());

    // Check E2 is in loading state
    match registry.get_state("E2") {
        Some(WarmModelState::Loading {
            progress_percent,
            bytes_loaded,
        }) => {
            assert_eq!(progress_percent, 50);
            assert_eq!(bytes_loaded, 128 * MB);
        }
        _ => panic!("Expected Loading state"),
    }
}

#[test]
fn test_health_status_from_registry_unhealthy() {
    let mut registry = WarmModelRegistry::new();
    registry.register_model("E1", 256 * MB, 768).unwrap();

    registry.start_loading("E1").unwrap();
    registry.mark_failed("E1", 102, "Load failed").unwrap();

    // Unhealthy: has failures
    assert!(registry.any_failed());
    assert!(!registry.all_warm());

    let failures = registry.failed_entries();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, "E1");
    assert_eq!(failures[0].1, 102);
}

#[test]
fn test_health_status_from_registry_not_initialized() {
    let registry = WarmModelRegistry::new();

    // NotInitialized: no models registered
    assert_eq!(registry.model_count(), 0);
    assert!(!registry.all_warm());
    assert!(!registry.any_failed());
}

#[test]
fn test_health_check_warm_count_progression() {
    let mut registry = WarmModelRegistry::new();

    // Register 5 models
    for i in 1..=5 {
        registry
            .register_model(format!("E{}", i), 100 * MB, 768)
            .unwrap();
    }

    assert_eq!(registry.warm_count(), 0);

    // Warm them one by one
    for i in 1..=5 {
        let model_id = format!("E{}", i);
        registry.start_loading(&model_id).unwrap();
        registry.mark_validating(&model_id).unwrap();
        registry
            .mark_warm(&model_id, test_handle(100 * MB))
            .unwrap();

        assert_eq!(registry.warm_count(), i);
    }

    assert!(registry.all_warm());
}
