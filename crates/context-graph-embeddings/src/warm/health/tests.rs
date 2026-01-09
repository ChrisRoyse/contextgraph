//! Unit Tests for Health Check Module
//!
//! Comprehensive tests for health status, check results, and the health checker service.

use super::*;
use crate::warm::config::WarmConfig;
use crate::warm::handle::ModelHandle;
use crate::warm::loader::WarmLoader;
use crate::warm::memory_pool::WarmMemoryPools;
use crate::warm::registry::{SharedWarmRegistry, WarmModelRegistry};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

/// Helper to create a test config
fn test_config() -> WarmConfig {
    WarmConfig::default()
}

/// Helper to create a test handle
fn test_handle() -> ModelHandle {
    ModelHandle::new(0x1000_0000, 512 * 1024 * 1024, 0, 0xDEAD_BEEF)
}

#[test]
fn test_health_status_enum_variants() {
    let healthy = WarmHealthStatus::Healthy;
    assert!(healthy.is_healthy());
    assert!(!healthy.is_loading());
    assert!(!healthy.is_unhealthy());
    assert!(!healthy.is_not_initialized());
    assert_eq!(healthy.as_str(), "healthy");

    let loading = WarmHealthStatus::Loading;
    assert!(!loading.is_healthy());
    assert!(loading.is_loading());
    assert!(!loading.is_unhealthy());
    assert!(!loading.is_not_initialized());
    assert_eq!(loading.as_str(), "loading");

    let unhealthy = WarmHealthStatus::Unhealthy;
    assert!(!unhealthy.is_healthy());
    assert!(!unhealthy.is_loading());
    assert!(unhealthy.is_unhealthy());
    assert!(!unhealthy.is_not_initialized());
    assert_eq!(unhealthy.as_str(), "unhealthy");

    let not_init = WarmHealthStatus::NotInitialized;
    assert!(!not_init.is_healthy());
    assert!(!not_init.is_loading());
    assert!(!not_init.is_unhealthy());
    assert!(not_init.is_not_initialized());
    assert_eq!(not_init.as_str(), "not_initialized");
}

#[test]
fn test_health_check_initial_not_initialized() {
    let registry: SharedWarmRegistry = Arc::new(RwLock::new(WarmModelRegistry::new()));
    let pools = WarmMemoryPools::new(test_config());

    let checker = WarmHealthChecker::new(registry, pools);
    let health = checker.check();

    assert_eq!(health.status, WarmHealthStatus::NotInitialized);
    assert_eq!(health.models_total, 0);
    assert_eq!(health.models_warm, 0);
    assert_eq!(health.models_loading, 0);
    assert_eq!(health.models_failed, 0);
    assert!(health.error_messages.is_empty());
    assert_eq!(checker.status(), WarmHealthStatus::NotInitialized);
    assert!(!checker.is_healthy());
}

#[test]
fn test_health_check_loading_state() {
    let mut registry_inner = WarmModelRegistry::new();
    registry_inner
        .register_model("E1_Semantic", 500 * 1024 * 1024, 768)
        .unwrap();
    registry_inner
        .register_model("E2_TemporalRecent", 400 * 1024 * 1024, 768)
        .unwrap();
    registry_inner
        .register_model("E3_TemporalPeriodic", 400 * 1024 * 1024, 768)
        .unwrap();

    registry_inner.start_loading("E1_Semantic").unwrap();
    registry_inner.start_loading("E2_TemporalRecent").unwrap();
    registry_inner.mark_validating("E2_TemporalRecent").unwrap();
    registry_inner
        .mark_warm("E2_TemporalRecent", test_handle())
        .unwrap();

    let registry: SharedWarmRegistry = Arc::new(RwLock::new(registry_inner));
    let pools = WarmMemoryPools::new(test_config());
    let checker = WarmHealthChecker::new(registry, pools);
    let health = checker.check();

    assert_eq!(health.status, WarmHealthStatus::Loading);
    assert_eq!(health.models_total, 3);
    assert_eq!(health.models_warm, 1);
    assert_eq!(health.models_loading, 1);
    assert_eq!(health.models_pending, 1);
    assert_eq!(health.models_failed, 0);
    assert!(health.error_messages.is_empty());
}

#[test]
fn test_health_check_healthy_state() {
    let mut registry_inner = WarmModelRegistry::new();
    let models = ["E1_Semantic", "E2_TemporalRecent", "E3_TemporalPeriodic"];
    for model_id in models {
        registry_inner
            .register_model(model_id, 500 * 1024 * 1024, 768)
            .unwrap();
        registry_inner.start_loading(model_id).unwrap();
        registry_inner.mark_validating(model_id).unwrap();
        registry_inner.mark_warm(model_id, test_handle()).unwrap();
    }

    let registry: SharedWarmRegistry = Arc::new(RwLock::new(registry_inner));
    let pools = WarmMemoryPools::new(test_config());
    let checker = WarmHealthChecker::new(registry, pools);
    let health = checker.check();

    assert_eq!(health.status, WarmHealthStatus::Healthy);
    assert_eq!(health.models_total, 3);
    assert_eq!(health.models_warm, 3);
    assert_eq!(health.models_loading, 0);
    assert_eq!(health.models_failed, 0);
    assert!(health.error_messages.is_empty());
    assert!(checker.is_healthy());
    assert_eq!(checker.status(), WarmHealthStatus::Healthy);
    assert!((health.warm_percentage() - 100.0).abs() < 0.01);
}

#[test]
fn test_health_check_unhealthy_state() {
    let mut registry_inner = WarmModelRegistry::new();
    registry_inner
        .register_model("E1_Semantic", 500 * 1024 * 1024, 768)
        .unwrap();
    registry_inner
        .register_model("E2_TemporalRecent", 400 * 1024 * 1024, 768)
        .unwrap();

    registry_inner.start_loading("E1_Semantic").unwrap();
    registry_inner.mark_validating("E1_Semantic").unwrap();
    registry_inner
        .mark_warm("E1_Semantic", test_handle())
        .unwrap();

    registry_inner.start_loading("E2_TemporalRecent").unwrap();
    registry_inner
        .mark_failed("E2_TemporalRecent", 102, "CUDA allocation failed")
        .unwrap();

    let registry: SharedWarmRegistry = Arc::new(RwLock::new(registry_inner));
    let pools = WarmMemoryPools::new(test_config());
    let checker = WarmHealthChecker::new(registry, pools);
    let health = checker.check();

    assert_eq!(health.status, WarmHealthStatus::Unhealthy);
    assert_eq!(health.models_total, 2);
    assert_eq!(health.models_warm, 1);
    assert_eq!(health.models_failed, 1);
    assert_eq!(health.error_messages.len(), 1);
    assert!(health.error_messages[0].contains("E2_TemporalRecent"));
    assert!(health.error_messages[0].contains("CUDA allocation failed"));
    assert!(!checker.is_healthy());
    assert_eq!(checker.status(), WarmHealthStatus::Unhealthy);
}

#[test]
fn test_health_checker_from_loader() {
    let config = test_config();
    let loader = WarmLoader::new(config).expect("Failed to create loader");
    let checker = WarmHealthChecker::from_loader(&loader);
    let health = checker.check();

    assert_eq!(health.status, WarmHealthStatus::Loading);
    assert!(health.models_total > 0);
    assert_eq!(health.models_warm, 0);
    assert!(health.models_pending > 0);
    assert!(health.uptime.is_some());
    assert!(checker.uptime() >= Duration::ZERO);
}

#[test]
fn test_uptime_tracking() {
    let registry: SharedWarmRegistry = Arc::new(RwLock::new(WarmModelRegistry::new()));
    let pools = WarmMemoryPools::new(test_config());
    let checker = WarmHealthChecker::new(registry, pools);

    let uptime1 = checker.uptime();
    assert!(uptime1.as_millis() < 100);

    thread::sleep(Duration::from_millis(10));

    let uptime2 = checker.uptime();
    assert!(uptime2 > uptime1);

    let health = checker.check();
    assert!(health.uptime.is_some());
    assert!(health.uptime.unwrap() >= uptime1);
}

#[test]
fn test_error_messages_populated() {
    let mut registry_inner = WarmModelRegistry::new();
    registry_inner
        .register_model("E1_Semantic", 500 * 1024 * 1024, 768)
        .unwrap();
    registry_inner
        .register_model("E2_TemporalRecent", 400 * 1024 * 1024, 768)
        .unwrap();
    registry_inner
        .register_model("E3_TemporalPeriodic", 400 * 1024 * 1024, 768)
        .unwrap();

    registry_inner.start_loading("E1_Semantic").unwrap();
    registry_inner
        .mark_failed("E1_Semantic", 104, "Insufficient VRAM")
        .unwrap();
    registry_inner.start_loading("E2_TemporalRecent").unwrap();
    registry_inner
        .mark_failed("E2_TemporalRecent", 103, "NaN in weights")
        .unwrap();

    let registry: SharedWarmRegistry = Arc::new(RwLock::new(registry_inner));
    let pools = WarmMemoryPools::new(test_config());
    let checker = WarmHealthChecker::new(registry, pools);
    let health = checker.check();

    assert_eq!(health.models_failed, 2);
    assert_eq!(health.error_messages.len(), 2);

    let all_messages = health.error_messages.join("\n");
    assert!(all_messages.contains("E1_Semantic"));
    assert!(all_messages.contains("Insufficient VRAM"));
    assert!(all_messages.contains("104"));
    assert!(all_messages.contains("E2_TemporalRecent"));
    assert!(all_messages.contains("NaN in weights"));
    assert!(all_messages.contains("103"));
}
