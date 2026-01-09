//! Statistics tracking tests for GPU Memory Manager
//!
//! Tests memory statistics, usage tracking, usage percentage calculation,
//! and category budget reporting.

use context_graph_graph::index::gpu_memory::{GpuMemoryConfig, GpuMemoryManager, MemoryCategory};

#[test]
fn test_stats_tracking() {
    println!("\n=== TEST: Stats Tracking ===");

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1024 * 1024))
        .expect("Manager creation failed");

    let _h1 = manager
        .allocate(100_000, MemoryCategory::FaissIndex)
        .expect("Allocation failed");
    let _h2 = manager
        .allocate(200_000, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");

    let stats = manager.stats();
    println!("Stats: {:?}", stats);

    assert_eq!(stats.total_allocated, 300_000, "Total should be 300KB");
    assert_eq!(stats.allocation_count, 2, "Should have 2 allocations");
    assert_eq!(
        stats.category_usage.get(&MemoryCategory::FaissIndex),
        Some(&100_000),
        "FaissIndex should show 100KB"
    );
    assert_eq!(
        stats.category_usage.get(&MemoryCategory::WorkingMemory),
        Some(&200_000),
        "WorkingMemory should show 200KB"
    );

    println!("=== PASSED ===\n");
}

#[test]
fn test_memory_stats_usage_percent() {
    println!("\n=== TEST: Memory Stats Usage Percent ===");

    let manager =
        GpuMemoryManager::new(GpuMemoryConfig::with_budget(1000)).expect("Manager creation failed");

    let _h = manager
        .allocate(500, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");

    let stats = manager.stats();
    let percent = stats.usage_percent();

    println!("Usage percent: {}%", percent);
    assert!((percent - 50.0).abs() < 0.1, "Should be approximately 50%");

    println!("=== PASSED ===\n");
}

#[test]
fn test_memory_stats_category_budget() {
    println!("\n=== TEST: Memory Stats Category Budget ===");

    let manager =
        GpuMemoryManager::new(GpuMemoryConfig::default()).expect("Manager creation failed");

    let stats = manager.stats();

    // Verify all categories have budgets
    assert!(stats
        .category_budget
        .contains_key(&MemoryCategory::FaissIndex));
    assert!(stats
        .category_budget
        .contains_key(&MemoryCategory::HyperbolicCoords));
    assert!(stats
        .category_budget
        .contains_key(&MemoryCategory::EntailmentCones));
    assert!(stats
        .category_budget
        .contains_key(&MemoryCategory::WorkingMemory));
    assert!(stats.category_budget.contains_key(&MemoryCategory::Other));

    // Verify default budgets
    assert_eq!(
        stats.category_budget.get(&MemoryCategory::FaissIndex),
        Some(&(8 * 1024 * 1024 * 1024))
    );

    println!("Category budgets verified");
    println!("=== PASSED ===\n");
}
