//! Category-related tests for GPU Memory Manager
//!
//! Tests category budgets, low memory detection, category availability,
//! multiple allocations across different categories, and allocation handle properties.

use context_graph_graph::index::gpu_memory::{GpuMemoryConfig, GpuMemoryManager, MemoryCategory};

#[test]
fn test_category_budget() {
    println!("\n=== TEST: Category Budget ===");

    let config = GpuMemoryConfig::default().category_budget(MemoryCategory::FaissIndex, 1024);

    let manager = GpuMemoryManager::new(config).expect("Manager creation failed");

    // Within category budget
    println!("BEFORE: Allocating 512 bytes to FaissIndex (budget=1024)");
    let _h1 = manager
        .allocate(512, MemoryCategory::FaissIndex)
        .expect("Within category budget should succeed");
    println!(
        "AFTER: FaissIndex used={}",
        manager
            .stats()
            .category_usage
            .get(&MemoryCategory::FaissIndex)
            .unwrap_or(&0)
    );

    // Exceeds category budget
    println!("BEFORE: Attempting 1024 more bytes to FaissIndex");
    let result = manager.allocate(1024, MemoryCategory::FaissIndex);
    assert!(result.is_err(), "Should fail - exceeds category budget");

    // Different category still works
    println!("BEFORE: Allocating 1024 bytes to WorkingMemory");
    let _h2 = manager
        .allocate(1024, MemoryCategory::WorkingMemory)
        .expect("Different category should succeed");
    println!("AFTER: WorkingMemory allocation succeeded");

    println!("=== PASSED ===\n");
}

#[test]
fn test_low_memory_detection() {
    println!("\n=== TEST: Low Memory Detection ===");

    let mut config = GpuMemoryConfig::with_budget(1000);
    config.low_memory_threshold = 0.8; // 80%

    let manager = GpuMemoryManager::new(config).expect("Manager creation failed");

    // Below threshold
    let _h1 = manager
        .allocate(700, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");
    assert!(
        !manager.is_low_memory(),
        "70% usage should not be low memory"
    );

    // Above threshold
    let _h2 = manager
        .allocate(200, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");
    assert!(manager.is_low_memory(), "90% usage should be low memory");

    println!("=== PASSED ===\n");
}

#[test]
fn test_category_available() {
    println!("\n=== TEST: Category Available ===");

    let config = GpuMemoryConfig::default().category_budget(MemoryCategory::FaissIndex, 1000);

    let manager = GpuMemoryManager::new(config).expect("Manager creation failed");

    // Initial available
    assert_eq!(manager.category_available(MemoryCategory::FaissIndex), 1000);

    // After allocation
    let _h = manager
        .allocate(400, MemoryCategory::FaissIndex)
        .expect("Allocation failed");
    assert_eq!(manager.category_available(MemoryCategory::FaissIndex), 600);

    println!("=== PASSED ===\n");
}

#[test]
fn test_multiple_allocations_different_categories() {
    println!("\n=== TEST: Multiple Allocations Different Categories ===");

    let config = GpuMemoryConfig::with_budget(10_000_000)
        .category_budget(MemoryCategory::FaissIndex, 1_000_000)
        .category_budget(MemoryCategory::HyperbolicCoords, 2_000_000)
        .category_budget(MemoryCategory::EntailmentCones, 3_000_000);

    let manager = GpuMemoryManager::new(config).expect("Manager creation failed");

    println!("BEFORE: Allocating across multiple categories");

    let h1 = manager
        .allocate(500_000, MemoryCategory::FaissIndex)
        .expect("FaissIndex allocation failed");
    let h2 = manager
        .allocate(1_000_000, MemoryCategory::HyperbolicCoords)
        .expect("HyperbolicCoords allocation failed");
    let h3 = manager
        .allocate(1_500_000, MemoryCategory::EntailmentCones)
        .expect("EntailmentCones allocation failed");

    let stats = manager.stats();
    println!("AFTER: total_allocated={}", stats.total_allocated);
    println!(
        "  FaissIndex: {:?}",
        stats.category_usage.get(&MemoryCategory::FaissIndex)
    );
    println!(
        "  HyperbolicCoords: {:?}",
        stats.category_usage.get(&MemoryCategory::HyperbolicCoords)
    );
    println!(
        "  EntailmentCones: {:?}",
        stats.category_usage.get(&MemoryCategory::EntailmentCones)
    );

    assert_eq!(stats.total_allocated, 3_000_000);
    assert_eq!(stats.allocation_count, 3);

    // Verify per-category
    assert_eq!(
        stats.category_usage.get(&MemoryCategory::FaissIndex),
        Some(&500_000)
    );
    assert_eq!(
        stats.category_usage.get(&MemoryCategory::HyperbolicCoords),
        Some(&1_000_000)
    );
    assert_eq!(
        stats.category_usage.get(&MemoryCategory::EntailmentCones),
        Some(&1_500_000)
    );

    // Drop one and verify
    drop(h2);
    let stats = manager.stats();
    assert_eq!(stats.total_allocated, 2_000_000);
    assert_eq!(stats.allocation_count, 2);
    assert_eq!(
        stats.category_usage.get(&MemoryCategory::HyperbolicCoords),
        Some(&0)
    );

    drop(h1);
    drop(h3);

    assert_eq!(manager.used(), 0);
    assert_eq!(manager.stats().allocation_count, 0);

    println!("=== PASSED ===\n");
}

#[test]
fn test_allocation_handle_properties() {
    println!("\n=== TEST: Allocation Handle Properties ===");

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1_000_000))
        .expect("Manager creation failed");

    let h1 = manager
        .allocate(1000, MemoryCategory::FaissIndex)
        .expect("Allocation failed");
    let h2 = manager
        .allocate(2000, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");

    // Verify handle properties
    assert_eq!(h1.size(), 1000);
    assert_eq!(h1.category(), MemoryCategory::FaissIndex);

    assert_eq!(h2.size(), 2000);
    assert_eq!(h2.category(), MemoryCategory::WorkingMemory);

    // IDs should be different
    assert_ne!(h1.id(), h2.id());

    println!(
        "Handle 1: id={}, size={}, category={:?}",
        h1.id(),
        h1.size(),
        h1.category()
    );
    println!(
        "Handle 2: id={}, size={}, category={:?}",
        h2.id(),
        h2.size(),
        h2.category()
    );

    println!("=== PASSED ===\n");
}
