//! Edge case tests for GPU Memory Manager
//!
//! Tests boundary conditions: zero-size allocations, maximum allocations,
//! minimal budgets, and invalid configurations.

#![allow(clippy::field_reassign_with_default)]

use context_graph_graph::index::gpu_memory::{GpuMemoryConfig, GpuMemoryManager, MemoryCategory};

#[test]
fn test_edge_case_zero_size_allocation() {
    println!("\n=== EDGE CASE: Zero Size Allocation ===");
    println!("BEFORE: manager.used() = {}", 0);

    let manager =
        GpuMemoryManager::new(GpuMemoryConfig::with_budget(1024)).expect("Manager creation failed");

    // Zero-size allocation is allowed (no-op effectively)
    let h = manager
        .allocate(0, MemoryCategory::WorkingMemory)
        .expect("Zero allocation should succeed");

    println!("AFTER: manager.used() = {}", manager.used());
    assert_eq!(manager.used(), 0, "Zero allocation should not change usage");
    assert_eq!(h.size(), 0, "Handle size should be 0");

    drop(h);
    assert_eq!(manager.used(), 0, "Should still be 0 after drop");

    println!("=== PASSED ===\n");
}

#[test]
fn test_edge_case_max_allocation() {
    println!("\n=== EDGE CASE: Maximum Allocation ===");

    let budget: usize = 1024 * 1024; // 1MB
    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(budget))
        .expect("Manager creation failed");

    println!("BEFORE: Allocating exact budget size");
    let h = manager
        .allocate(budget, MemoryCategory::WorkingMemory)
        .expect("Exact budget allocation should succeed");
    println!(
        "AFTER: manager.used() = {}, manager.available() = {}",
        manager.used(),
        manager.available()
    );

    assert_eq!(manager.used(), budget);
    assert_eq!(manager.available(), 0);

    // Any additional allocation should fail
    let result = manager.allocate(1, MemoryCategory::Other);
    assert!(result.is_err(), "Should fail when budget exhausted");

    drop(h);
    println!("=== PASSED ===\n");
}

#[test]
fn test_edge_case_empty_inputs() {
    println!("\n=== EDGE CASE: Boundary Conditions ===");

    // Test config with minimal budget
    let manager =
        GpuMemoryManager::new(GpuMemoryConfig::with_budget(1)).expect("Should allow 1 byte budget");

    let h = manager
        .allocate(1, MemoryCategory::Other)
        .expect("Single byte allocation should succeed");
    assert_eq!(manager.used(), 1);
    assert_eq!(manager.available(), 0);

    let result = manager.allocate(1, MemoryCategory::Other);
    assert!(result.is_err());

    drop(h);
    println!("=== PASSED ===\n");
}

#[test]
fn test_edge_case_invalid_config() {
    println!("\n=== EDGE CASE: Invalid Config ===");

    // Zero budget should fail
    let result = GpuMemoryManager::new(GpuMemoryConfig::with_budget(0));
    assert!(result.is_err(), "Zero budget should fail validation");

    // Invalid threshold should fail
    let mut config = GpuMemoryConfig::default();
    config.low_memory_threshold = 0.0;
    let result = GpuMemoryManager::new(config);
    assert!(result.is_err(), "Zero threshold should fail validation");

    println!("=== PASSED ===\n");
}
