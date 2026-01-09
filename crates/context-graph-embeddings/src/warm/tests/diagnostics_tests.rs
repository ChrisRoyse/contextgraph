//! Tests for WarmDiagnostics JSON reporting.

use super::helpers::{test_config, GB, MB};
use crate::warm::error::WarmError;
use crate::warm::memory_pool::WarmMemoryPools;
use crate::warm::registry::WarmModelRegistry;

#[test]
fn test_diagnostic_error_info_collection() {
    // Collect error information for diagnostics
    let errors: Vec<WarmError> = vec![
        WarmError::ModelFileMissing {
            model_id: "E1_Semantic".to_string(),
            path: "/models/semantic.bin".to_string(),
        },
        WarmError::VramInsufficientTotal {
            required_bytes: 32 * GB,
            available_bytes: 24 * GB,
            required_gb: 32.0,
            available_gb: 24.0,
            model_breakdown: vec![("E1".to_string(), 800 * MB), ("E2".to_string(), 600 * MB)],
        },
    ];

    for err in &errors {
        // Verify we can extract diagnostic info
        assert!(!err.category().is_empty());
        assert!(!err.error_code().is_empty());
        assert!(err.exit_code() > 0);
    }
}

#[test]
fn test_diagnostic_registry_snapshot() {
    let mut registry = WarmModelRegistry::new();

    registry.register_model("E1", 800 * MB, 768).unwrap();
    registry.register_model("E2", 600 * MB, 768).unwrap();

    registry.start_loading("E1").unwrap();
    registry.update_progress("E1", 50, 400 * MB).unwrap();

    // Collect diagnostic snapshot
    let model_count = registry.model_count();
    let warm_count = registry.warm_count();
    let any_failed = registry.any_failed();
    let loading_order = registry.loading_order();

    assert_eq!(model_count, 2);
    assert_eq!(warm_count, 0);
    assert!(!any_failed);
    assert_eq!(loading_order[0], "E1"); // Largest first
}

#[test]
fn test_diagnostic_memory_pool_snapshot() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_model("E1", 10 * GB, 0x1000).unwrap();
    pools.allocate_working(2 * GB).unwrap();

    // Collect diagnostic snapshot
    let model_capacity = pools.model_pool_capacity();
    let model_allocated = pools.total_allocated_bytes();
    let model_utilization = pools.model_pool_utilization();
    let working_capacity = pools.working_pool_capacity();
    let working_available = pools.available_working_bytes();
    let within_budget = pools.is_within_budget();

    assert_eq!(model_capacity, 24 * GB);
    assert_eq!(model_allocated, 10 * GB + 2 * GB);
    assert!(model_utilization > 0.4 && model_utilization < 0.5);
    assert_eq!(working_capacity, 8 * GB);
    assert_eq!(working_available, 6 * GB);
    assert!(within_budget);
}

#[test]
fn test_diagnostic_failure_report() {
    let mut registry = WarmModelRegistry::new();

    registry.register_model("E1", 800 * MB, 768).unwrap();
    registry.register_model("E2", 600 * MB, 768).unwrap();

    registry.start_loading("E1").unwrap();
    registry
        .mark_failed("E1", 102, "Checksum mismatch")
        .unwrap();

    registry.start_loading("E2").unwrap();
    registry.mark_failed("E2", 104, "VRAM exhausted").unwrap();

    // Collect failure report
    let failures = registry.failed_entries();

    assert_eq!(failures.len(), 2);

    // Verify we can build a diagnostic report
    for (model_id, error_code, error_message) in &failures {
        assert!(!model_id.is_empty());
        assert!(*error_code >= 100);
        assert!(!error_message.is_empty());
    }
}

#[test]
fn test_diagnostic_model_allocation_details() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_model("E1", GB, 0x1000_0000).unwrap();
    pools.allocate_model("E2", 2 * GB, 0x5000_0000).unwrap();

    let allocations = pools.list_model_allocations();

    assert_eq!(allocations.len(), 2);

    for alloc in allocations {
        // Verify diagnostic info is available
        assert!(!alloc.model_id.is_empty());
        assert!(alloc.vram_ptr > 0);
        assert!(alloc.size_bytes > 0);
        assert!(alloc.age_secs() >= 0.0);
    }
}

#[test]
fn test_diagnostic_config_dump() {
    let config = test_config();

    // Collect config diagnostic info
    let vram_budget = config.vram_budget_bytes;
    let vram_headroom = config.vram_headroom_bytes;
    let total_required = config.total_vram_required();
    let cuda_device = config.cuda_device_id;
    let quantization = config.quantization.as_str();
    let max_load_time = config.max_load_time_per_model_ms;

    assert_eq!(vram_budget, 24 * GB);
    assert_eq!(vram_headroom, 8 * GB);
    assert_eq!(total_required, 32 * GB);
    assert_eq!(cuda_device, 0);
    assert_eq!(quantization, "FP16");
    assert_eq!(max_load_time, 30_000);
}
