//! Basic functionality tests for GPU Memory Manager
//!
//! Tests core allocation, deallocation, budget enforcement, thread safety,
//! and RTX 5090 configuration.

use context_graph_graph::error::GraphError;
use context_graph_graph::index::gpu_memory::{GpuMemoryConfig, GpuMemoryManager, MemoryCategory};

#[test]
fn test_allocation_and_free() {
    println!("\n=== TEST: Allocation and Free ===");
    println!("BEFORE: Creating manager with 1MB budget");

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1024 * 1024))
        .expect("Manager creation failed");

    println!("AFTER: Manager created");
    assert_eq!(manager.used(), 0, "Initial used must be 0");
    assert_eq!(
        manager.available(),
        1024 * 1024,
        "Initial available must be full budget"
    );

    // Allocate
    println!("BEFORE: Allocating 512KB");
    let handle = manager
        .allocate(512 * 1024, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");
    println!("AFTER: Allocation successful, id={}", handle.id());

    // Verify state
    assert_eq!(manager.used(), 512 * 1024, "Used should be 512KB");
    assert_eq!(manager.available(), 512 * 1024, "Available should be 512KB");
    assert_eq!(handle.size(), 512 * 1024, "Handle size should be 512KB");

    // Free via drop
    println!("BEFORE: Dropping handle");
    drop(handle);
    println!("AFTER: Handle dropped");

    // Verify freed
    assert_eq!(manager.used(), 0, "Used should be 0 after free");
    assert_eq!(
        manager.available(),
        1024 * 1024,
        "Available should be restored"
    );

    println!("=== PASSED ===\n");
}

#[test]
fn test_budget_enforcement() {
    println!("\n=== TEST: Budget Enforcement ===");

    let manager =
        GpuMemoryManager::new(GpuMemoryConfig::with_budget(1024)).expect("Manager creation failed");

    // First allocation within budget
    println!("BEFORE: Allocating 512 bytes (within budget)");
    let _h1 = manager
        .allocate(512, MemoryCategory::WorkingMemory)
        .expect("First allocation should succeed");
    println!("AFTER: First allocation succeeded, used={}", manager.used());

    // Second allocation exceeds budget
    println!("BEFORE: Attempting 1024 byte allocation (exceeds remaining)");
    let result = manager.allocate(1024, MemoryCategory::WorkingMemory);
    println!("AFTER: Result = {:?}", result.is_err());

    assert!(result.is_err(), "Over-budget allocation must fail");
    match result {
        Err(GraphError::GpuResourceAllocation(msg)) => {
            println!("Error message: {}", msg);
            assert!(
                msg.contains("exceed"),
                "Error should mention exceeding budget"
            );
        }
        _ => panic!("Expected GpuResourceAllocation error"),
    }

    println!("=== PASSED ===\n");
}

#[test]
fn test_thread_safety() {
    println!("\n=== TEST: Thread Safety ===");

    use std::thread;

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1024 * 1024 * 100))
        .expect("Manager creation failed");

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let m = manager.clone();
            thread::spawn(move || {
                println!("Thread {} starting allocation", i);
                let _h = m
                    .allocate(1024 * 1024, MemoryCategory::WorkingMemory)
                    .expect("Concurrent allocation should succeed");
                thread::sleep(std::time::Duration::from_millis(10));
                println!("Thread {} completing", i);
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread should not panic");
    }

    // All allocations freed after threads complete
    assert_eq!(manager.used(), 0, "All memory should be freed");

    println!("=== PASSED ===\n");
}

#[test]
fn test_rtx_5090_config() {
    println!("\n=== TEST: RTX 5090 Config ===");

    let manager = GpuMemoryManager::rtx_5090().expect("RTX 5090 config failed");

    assert_eq!(
        manager.budget(),
        24 * 1024 * 1024 * 1024,
        "Budget should be 24GB"
    );

    let stats = manager.stats();
    assert_eq!(stats.total_budget, 24 * 1024 * 1024 * 1024);

    println!("=== PASSED ===\n");
}

#[test]
fn test_try_allocate() {
    println!("\n=== TEST: try_allocate ===");

    let manager =
        GpuMemoryManager::new(GpuMemoryConfig::with_budget(1024)).expect("Manager creation failed");

    // Success case
    let h = manager.try_allocate(512, MemoryCategory::WorkingMemory);
    assert!(h.is_some(), "Should succeed with Some");

    // Failure case (returns None, not error)
    let h2 = manager.try_allocate(1024, MemoryCategory::WorkingMemory);
    assert!(h2.is_none(), "Should return None on failure");

    println!("=== PASSED ===\n");
}

#[test]
fn test_peak_usage_tracking() {
    println!("\n=== TEST: Peak Usage Tracking ===");

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1024 * 1024))
        .expect("Manager creation failed");

    let h1 = manager
        .allocate(256 * 1024, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");
    let h2 = manager
        .allocate(256 * 1024, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");

    assert_eq!(
        manager.stats().peak_usage,
        512 * 1024,
        "Peak should be 512KB"
    );

    drop(h2);
    drop(h1);

    // Peak should remain even after freeing
    assert_eq!(
        manager.stats().peak_usage,
        512 * 1024,
        "Peak should persist"
    );
    assert_eq!(manager.used(), 0, "Current usage should be 0");

    println!("=== PASSED ===\n");
}

#[test]
fn test_manager_clone() {
    println!("\n=== TEST: Manager Clone ===");

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1_000_000))
        .expect("Manager creation failed");

    let _h = manager
        .allocate(500_000, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");

    // Clone manager
    let manager2 = manager.clone();

    // Both should see the same state
    assert_eq!(manager.used(), manager2.used());
    assert_eq!(manager.budget(), manager2.budget());

    // Allocate on clone, original should see it
    let _h2 = manager2
        .allocate(100_000, MemoryCategory::FaissIndex)
        .expect("Allocation on clone failed");

    assert_eq!(manager.used(), 600_000);
    assert_eq!(manager2.used(), 600_000);

    println!("=== PASSED ===\n");
}

#[test]
fn test_debug_format() {
    println!("\n=== TEST: Debug Format ===");

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1_000_000))
        .expect("Manager creation failed");

    let _h = manager
        .allocate(500_000, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");

    let debug_str = format!("{:?}", manager);
    println!("Debug output: {}", debug_str);

    assert!(debug_str.contains("GpuMemoryManager"));
    assert!(debug_str.contains("used"));
    assert!(debug_str.contains("budget"));

    println!("=== PASSED ===\n");
}
