//! Tests for WarmMemoryPools dual-pool architecture.

use super::helpers::{GB, MB};
use crate::warm::error::WarmError;
use crate::warm::memory_pool::WarmMemoryPools;

#[test]
fn test_rtx_5090_factory_capacities() {
    let pools = WarmMemoryPools::rtx_5090();

    assert_eq!(pools.model_pool_capacity(), 24 * GB);
    assert_eq!(pools.working_pool_capacity(), 8 * GB);
    assert_eq!(pools.total_capacity(), 32 * GB);
}

#[test]
fn test_model_allocation_and_tracking() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools
        .allocate_model("E1_Semantic", 800 * MB, 0x1000)
        .unwrap();

    let alloc = pools.get_model_allocation("E1_Semantic").unwrap();
    assert_eq!(alloc.size_bytes, 800 * MB);
    assert_eq!(alloc.vram_ptr, 0x1000);
    assert_eq!(pools.total_allocated_bytes(), 800 * MB);
    assert_eq!(pools.available_model_bytes(), 24 * GB - 800 * MB);
}

#[test]
fn test_model_deallocation() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools
        .allocate_model("E1_Semantic", 800 * MB, 0x1000)
        .unwrap();
    pools.free_model("E1_Semantic").unwrap();

    assert!(pools.get_model_allocation("E1_Semantic").is_none());
    assert_eq!(pools.total_allocated_bytes(), 0);
}

#[test]
fn test_budget_enforcement_model_pool() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_model("model1", 20 * GB, 0x1000).unwrap();
    pools.allocate_model("model2", 3 * GB, 0x2000).unwrap();

    // This should fail (exceeds 24GB capacity)
    let result = pools.allocate_model("model3", 2 * GB, 0x3000);
    assert!(matches!(
        result,
        Err(WarmError::VramAllocationFailed { .. })
    ));
}

#[test]
fn test_working_memory_allocation_and_free() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_working(GB).unwrap();
    assert_eq!(pools.available_working_bytes(), 7 * GB);

    pools.free_working(GB).unwrap();
    assert_eq!(pools.available_working_bytes(), 8 * GB);
}

#[test]
fn test_working_memory_exhaustion() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_working(8 * GB).unwrap();

    let result = pools.allocate_working(1);
    assert!(matches!(
        result,
        Err(WarmError::WorkingMemoryExhausted { .. })
    ));
}

#[test]
fn test_is_within_budget() {
    let mut pools = WarmMemoryPools::rtx_5090();
    assert!(pools.is_within_budget());

    pools.allocate_model("model1", 20 * GB, 0x1000).unwrap();
    pools.allocate_working(6 * GB).unwrap();
    assert!(pools.is_within_budget());

    // Fill to capacity
    pools.allocate_model("model2", 4 * GB, 0x2000).unwrap();
    pools.allocate_working(2 * GB).unwrap();
    assert!(pools.is_within_budget());
}

#[test]
fn test_duplicate_model_allocation_fails() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_model("E1_Semantic", GB, 0x1000).unwrap();

    let result = pools.allocate_model("E1_Semantic", GB, 0x2000);
    match result {
        Err(WarmError::ModelAlreadyRegistered { model_id }) => {
            assert_eq!(model_id, "E1_Semantic");
        }
        _ => panic!("Expected ModelAlreadyRegistered error"),
    }
}

#[test]
fn test_list_model_allocations() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_model("model1", GB, 0x1000).unwrap();
    pools.allocate_model("model2", 2 * GB, 0x2000).unwrap();
    pools.allocate_model("model3", 3 * GB, 0x3000).unwrap();

    let allocations = pools.list_model_allocations();
    assert_eq!(allocations.len(), 3);

    let ids: Vec<_> = allocations.iter().map(|a| a.model_id.as_str()).collect();
    assert!(ids.contains(&"model1"));
    assert!(ids.contains(&"model2"));
    assert!(ids.contains(&"model3"));
}

#[test]
fn test_utilization_metrics() {
    let mut pools = WarmMemoryPools::rtx_5090();

    assert_eq!(pools.model_pool_utilization(), 0.0);
    assert_eq!(pools.working_pool_utilization(), 0.0);

    pools.allocate_model("model1", 12 * GB, 0x1000).unwrap();
    pools.allocate_working(4 * GB).unwrap();

    assert!((pools.model_pool_utilization() - 0.5).abs() < 0.001);
    assert!((pools.working_pool_utilization() - 0.5).abs() < 0.001);
}

#[test]
fn test_reset_working_pool() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_working(5 * GB).unwrap();
    assert_eq!(pools.available_working_bytes(), 3 * GB);

    pools.reset_working_pool();
    assert_eq!(pools.available_working_bytes(), 8 * GB);
}

#[test]
fn test_working_memory_over_free() {
    let mut pools = WarmMemoryPools::rtx_5090();

    pools.allocate_working(GB).unwrap();
    pools.free_working(10 * GB).unwrap(); // Free more than allocated

    assert_eq!(pools.available_working_bytes(), 8 * GB);
}
