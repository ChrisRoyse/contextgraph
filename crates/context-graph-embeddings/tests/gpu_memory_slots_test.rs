//! Integration tests for GPU memory slot management (TASK-CORE-011).
//!
//! Tests the ModelSlotManager for managing 13 embedding model slots
//! within the 8GB budget constraint.
//!
//! # Full State Verification
//!
//! These tests verify:
//! 1. Source of Truth: ModelSlotManager internal state
//! 2. Edge Cases: Budget boundary, over-budget rejection, LRU selection
//! 3. Evidence of Success: Printed before/after state

use context_graph_core::teleological::embedder::Embedder;
use context_graph_embeddings::gpu::{
    GpuMemoryPool, MemoryPressure, ModelSlotManager, MODEL_BUDGET_BYTES,
};
use std::thread::sleep;
use std::time::Duration;

/// Verify the MODEL_BUDGET_BYTES constant is exactly 8GB
#[test]
fn test_model_budget_constant_is_8gb() {
    const EXPECTED_8GB: usize = 8 * 1024 * 1024 * 1024;
    assert_eq!(MODEL_BUDGET_BYTES, EXPECTED_8GB);
    println!("MODEL_BUDGET_BYTES = {} bytes = 8GB", MODEL_BUDGET_BYTES);
    println!("[PASS] MODEL_BUDGET_BYTES constant is 8GB");
}

/// Full State Verification: Edge Case 1 - Budget Boundary
#[test]
fn test_fsv_edge_case_1_budget_boundary() {
    println!("\n=== FULL STATE VERIFICATION: EDGE CASE 1 ===");
    println!("Testing: Budget boundary - allocate exactly at budget");

    // BEFORE state
    let mut manager = ModelSlotManager::with_budget(1000);
    println!(
        "BEFORE: allocated={}, available={}, pressure={:?}",
        manager.allocated(),
        manager.available(),
        manager.pressure_level()
    );
    assert_eq!(manager.allocated(), 0);
    assert_eq!(manager.available(), 1000);

    // ACTION: Allocate exactly at budget
    let result = manager.allocate_slot(Embedder::Semantic, 1000);
    println!("ACTION: allocate_slot(Semantic, 1000) => {:?}", result);
    assert!(result.is_ok(), "Allocation at exact budget should succeed");

    // AFTER state
    println!(
        "AFTER: allocated={}, available={}, pressure={:?}",
        manager.allocated(),
        manager.available(),
        manager.pressure_level()
    );
    assert_eq!(manager.allocated(), 1000);
    assert_eq!(manager.available(), 0);

    println!("[PASS] Edge Case 1: Budget boundary respected - exact allocation succeeded");
}

/// Full State Verification: Edge Case 2 - Over-Budget Rejection
#[test]
fn test_fsv_edge_case_2_over_budget_rejection() {
    println!("\n=== FULL STATE VERIFICATION: EDGE CASE 2 ===");
    println!("Testing: Over-budget allocation rejection");

    // BEFORE state
    let mut manager = ModelSlotManager::with_budget(1000);
    manager
        .allocate_slot(Embedder::Semantic, 900)
        .expect("initial allocation should succeed");
    println!(
        "BEFORE: allocated={}, available={}, pressure={:?}",
        manager.allocated(),
        manager.available(),
        manager.pressure_level()
    );
    assert_eq!(manager.allocated(), 900);

    // ACTION: Try to exceed budget
    let result = manager.allocate_slot(Embedder::TemporalRecent, 200);
    println!("ACTION: allocate_slot(TemporalRecent, 200) => {:?}", result);
    assert!(result.is_err(), "Over-budget allocation should fail");

    // AFTER state (should be unchanged)
    println!(
        "AFTER: allocated={}, available={}, pressure={:?}",
        manager.allocated(),
        manager.available(),
        manager.pressure_level()
    );
    assert_eq!(manager.allocated(), 900); // Unchanged

    println!("[PASS] Edge Case 2: Over-budget allocation rejected, state unchanged");
}

/// Full State Verification: Edge Case 3 - LRU Eviction Selection
#[test]
fn test_fsv_edge_case_3_lru_eviction_selection() {
    println!("\n=== FULL STATE VERIFICATION: EDGE CASE 3 ===");
    println!("Testing: LRU candidate correctly identified");

    let mut manager = ModelSlotManager::with_budget(10000);

    // Allocate in order with delays
    manager
        .allocate_slot(Embedder::Semantic, 1000)
        .expect("allocate E1");
    println!("Allocated Semantic (E1) at t=0");
    sleep(Duration::from_millis(15));

    manager
        .allocate_slot(Embedder::TemporalRecent, 1000)
        .expect("allocate E2");
    println!("Allocated TemporalRecent (E2) at t=15ms");
    sleep(Duration::from_millis(15));

    manager
        .allocate_slot(Embedder::TemporalPeriodic, 1000)
        .expect("allocate E3");
    println!("Allocated TemporalPeriodic (E3) at t=30ms");

    // BEFORE touch
    let lru_before = manager.get_lru_candidate();
    println!(
        "BEFORE touch: LRU candidate = {:?} (expected Semantic)",
        lru_before
    );
    assert_eq!(
        lru_before,
        Some(Embedder::Semantic),
        "Before touch, Semantic should be LRU"
    );

    // ACTION: Touch Semantic to make it recent
    manager.touch(&Embedder::Semantic);
    println!("ACTION: touch(Semantic)");

    // AFTER touch
    let lru_after = manager.get_lru_candidate();
    println!(
        "AFTER touch: LRU candidate = {:?} (expected TemporalRecent)",
        lru_after
    );
    assert_eq!(
        lru_after,
        Some(Embedder::TemporalRecent),
        "After touch, TemporalRecent should be LRU"
    );

    println!("[PASS] Edge Case 3: LRU candidate correctly identified");
}

/// Test: Allocate All 13 Embedders (from task spec)
#[test]
fn test_allocate_all_13_embedders_within_budget() {
    println!("\n=== TEST: ALLOCATE ALL 13 EMBEDDERS ===");

    // Approximate sizes for quantized models (from task spec)
    let model_sizes: [(Embedder, usize, &str); 13] = [
        (
            Embedder::Semantic,
            200_000_000,
            "E1: 200MB (1024D e5-large)",
        ),
        (Embedder::TemporalRecent, 100_000_000, "E2: 100MB (512D)"),
        (Embedder::TemporalPeriodic, 100_000_000, "E3: 100MB (512D)"),
        (
            Embedder::TemporalPositional,
            100_000_000,
            "E4: 100MB (512D)",
        ),
        (Embedder::Causal, 150_000_000, "E5: 150MB (768D nomic-embed)"),
        (Embedder::Sparse, 50_000_000, "E6: 50MB sparse SPLADE"),
        (Embedder::Code, 300_000_000, "E7: 300MB (1536D Qodo)"),
        (Embedder::Graph, 150_000_000, "E8: 150MB (1024D e5-large-v2)"),
        (Embedder::Hdc, 50_000_000, "E9: 50MB (1024D projected)"),
        (Embedder::Multimodal, 150_000_000, "E10: 150MB (768D CLIP)"),
        (Embedder::Entity, 150_000_000, "E11: 150MB (768D KEPLER)"),
        (
            Embedder::LateInteraction,
            200_000_000,
            "E12: 200MB (128D/token ColBERT)",
        ),
        (
            Embedder::KeywordSplade,
            50_000_000,
            "E13: 50MB sparse SPLADE v3",
        ),
    ];

    let mut manager = ModelSlotManager::new(); // 8GB budget
    let mut total_expected = 0usize;

    println!("Budget: {}GB", manager.budget() / (1024 * 1024 * 1024));
    println!("\nAllocating models:");

    for (embedder, size, desc) in model_sizes.iter() {
        let result = manager.allocate_slot(*embedder, *size);
        println!("  {} => {:?}", desc, result);
        assert!(
            result.is_ok(),
            "Failed to allocate {:?}: {:?}",
            embedder,
            result
        );
        total_expected += size;
    }

    // VERIFICATION
    println!("\n--- VERIFICATION ---");
    println!("Total slots: {}", manager.slot_count());
    println!("Loaded count: {}", manager.loaded_count());
    println!(
        "Total allocated: {} bytes ({:.2}GB)",
        manager.allocated(),
        manager.allocated() as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!(
        "Available: {} bytes ({:.2}GB)",
        manager.available(),
        manager.available() as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!(
        "Utilization: {:.2}%",
        (manager.allocated() as f64 / manager.budget() as f64) * 100.0
    );
    println!("Pressure level: {:?}", manager.pressure_level());

    assert_eq!(manager.loaded_count(), 13);
    assert_eq!(manager.allocated(), total_expected);
    assert_eq!(
        manager.pressure_level(),
        MemoryPressure::Low,
        "13 models should fit comfortably in 8GB budget"
    );

    println!("\n[PASS] All 13 embedders allocated within 8GB budget");
}

/// Test: Memory Pressure Reaches Critical
#[test]
fn test_memory_pressure_reaches_critical() {
    println!("\n=== TEST: MEMORY PRESSURE THRESHOLD ===");

    let mut manager = ModelSlotManager::with_budget(1000);

    // 96% utilization should be Critical
    manager
        .allocate_slot(Embedder::Semantic, 960)
        .expect("allocation should succeed");

    println!("Allocated 960/1000 bytes (96%)");
    println!("Pressure level: {:?}", manager.pressure_level());
    println!("Should evict: {}", manager.pressure_level().should_evict());

    assert_eq!(manager.pressure_level(), MemoryPressure::Critical);
    assert!(manager.pressure_level().should_evict());

    println!("[PASS] pressure_level() returns Critical at 96% utilization");
}

/// Test: GpuMemoryPool pressure_level integration
#[test]
fn test_gpu_memory_pool_pressure_level() {
    println!("\n=== TEST: GpuMemoryPool PRESSURE LEVEL ===");

    let pool = GpuMemoryPool::new(1000);

    // Low pressure
    pool.allocate("test1", 400).expect("allocate 40%");
    assert_eq!(pool.pressure_level().unwrap(), MemoryPressure::Low);
    println!("40% utilization: {:?}", pool.pressure_level().unwrap());

    // Add more to get Medium
    pool.allocate("test2", 200).expect("allocate +20%");
    assert_eq!(pool.pressure_level().unwrap(), MemoryPressure::Medium);
    println!("60% utilization: {:?}", pool.pressure_level().unwrap());

    // Add more to get High
    pool.allocate("test3", 250).expect("allocate +25%");
    assert_eq!(pool.pressure_level().unwrap(), MemoryPressure::High);
    println!("85% utilization: {:?}", pool.pressure_level().unwrap());

    // Add more to get Critical
    pool.allocate("test4", 110).expect("allocate +11%");
    assert_eq!(pool.pressure_level().unwrap(), MemoryPressure::Critical);
    assert!(pool.should_evict().unwrap());
    println!("96% utilization: {:?}", pool.pressure_level().unwrap());

    println!("[PASS] GpuMemoryPool pressure_level() works correctly");
}

/// Test: Eviction with automatic recovery
#[test]
fn test_eviction_with_automatic_recovery() {
    println!("\n=== TEST: LRU EVICTION ===");

    let mut manager = ModelSlotManager::with_budget(1000);

    // Fill up
    manager
        .allocate_slot(Embedder::Semantic, 600)
        .expect("allocate E1");
    sleep(Duration::from_millis(10));
    manager
        .allocate_slot(Embedder::TemporalRecent, 300)
        .expect("allocate E2");

    println!(
        "Before eviction: allocated={}, available={}",
        manager.allocated(),
        manager.available()
    );
    println!(
        "Slots: Semantic={}, TemporalRecent={}",
        manager.has_slot(&Embedder::Semantic),
        manager.has_slot(&Embedder::TemporalRecent)
    );

    // Try to allocate 500 more - should trigger eviction
    let result = manager.allocate_with_eviction(Embedder::Code, 500);
    println!("allocate_with_eviction(Code, 500) => {:?}", result);

    assert!(result.is_ok());
    let evicted = result.unwrap();
    println!("Evicted embedders: {:?}", evicted);

    assert!(evicted.contains(&Embedder::Semantic));
    assert!(manager.has_slot(&Embedder::Code));
    assert!(!manager.has_slot(&Embedder::Semantic));

    println!(
        "After eviction: allocated={}, available={}",
        manager.allocated(),
        manager.available()
    );
    println!(
        "Slots: Semantic={}, TemporalRecent={}, Code={}",
        manager.has_slot(&Embedder::Semantic),
        manager.has_slot(&Embedder::TemporalRecent),
        manager.has_slot(&Embedder::Code)
    );

    println!("[PASS] LRU eviction works correctly");
}

/// Test: MemoryPressure ordering
#[test]
fn test_memory_pressure_ordering() {
    assert!(MemoryPressure::Low < MemoryPressure::Medium);
    assert!(MemoryPressure::Medium < MemoryPressure::High);
    assert!(MemoryPressure::High < MemoryPressure::Critical);

    // Test from_utilization at boundaries
    assert_eq!(MemoryPressure::from_utilization(0.0), MemoryPressure::Low);
    assert_eq!(MemoryPressure::from_utilization(49.9), MemoryPressure::Low);
    assert_eq!(
        MemoryPressure::from_utilization(50.0),
        MemoryPressure::Medium
    );
    assert_eq!(
        MemoryPressure::from_utilization(79.9),
        MemoryPressure::Medium
    );
    assert_eq!(MemoryPressure::from_utilization(80.0), MemoryPressure::High);
    assert_eq!(MemoryPressure::from_utilization(94.9), MemoryPressure::High);
    assert_eq!(
        MemoryPressure::from_utilization(95.0),
        MemoryPressure::Critical
    );
    assert_eq!(
        MemoryPressure::from_utilization(100.0),
        MemoryPressure::Critical
    );

    println!("[PASS] MemoryPressure ordering and thresholds correct");
}

/// Test: Source of Truth Verification - Internal State Access
#[test]
fn test_source_of_truth_internal_state() {
    println!("\n=== SOURCE OF TRUTH: INTERNAL STATE ===");

    let mut manager = ModelSlotManager::with_budget(5000);

    // Allocate multiple slots
    manager.allocate_slot(Embedder::Semantic, 1000).expect("E1");
    manager
        .allocate_slot(Embedder::TemporalRecent, 500)
        .expect("E2");
    manager.allocate_slot(Embedder::Code, 1500).expect("E7");

    // READ from Source of Truth
    println!("--- Source of Truth Read ---");
    println!("  total_allocated (internal): {}", manager.allocated());
    println!("  slot_count (HashMap size): {}", manager.slot_count());
    println!("  loaded_count (filter loaded): {}", manager.loaded_count());
    println!("  pressure_level(): {:?}", manager.pressure_level());

    // Verify state consistency
    assert_eq!(manager.allocated(), 1000 + 500 + 1500);
    assert_eq!(manager.slot_count(), 3);
    assert_eq!(manager.loaded_count(), 3);
    assert_eq!(manager.available(), 5000 - 3000);

    // Verify each slot exists
    assert!(manager.has_slot(&Embedder::Semantic));
    assert!(manager.has_slot(&Embedder::TemporalRecent));
    assert!(manager.has_slot(&Embedder::Code));
    assert!(!manager.has_slot(&Embedder::Causal));

    // Verify slot sizes via get_slot
    assert_eq!(
        manager.get_slot(&Embedder::Semantic).map(|s| s.size_bytes),
        Some(1000)
    );
    assert_eq!(
        manager
            .get_slot(&Embedder::TemporalRecent)
            .map(|s| s.size_bytes),
        Some(500)
    );
    assert_eq!(
        manager.get_slot(&Embedder::Code).map(|s| s.size_bytes),
        Some(1500)
    );

    println!("[PASS] Source of Truth verification complete");
}

/// Test: Slot update (re-allocation)
#[test]
fn test_slot_update_reallocation() {
    println!("\n=== TEST: SLOT UPDATE (RE-ALLOCATION) ===");

    let mut manager = ModelSlotManager::with_budget(2000);

    // Initial allocation
    manager
        .allocate_slot(Embedder::Semantic, 500)
        .expect("initial");
    println!(
        "After initial: allocated={}, slot_size={}",
        manager.allocated(),
        manager.get_slot(&Embedder::Semantic).unwrap().size_bytes
    );
    assert_eq!(manager.allocated(), 500);

    // Update to larger
    manager
        .allocate_slot(Embedder::Semantic, 800)
        .expect("update larger");
    println!(
        "After update larger: allocated={}, slot_size={}",
        manager.allocated(),
        manager.get_slot(&Embedder::Semantic).unwrap().size_bytes
    );
    assert_eq!(manager.allocated(), 800);

    // Update to smaller
    manager
        .allocate_slot(Embedder::Semantic, 300)
        .expect("update smaller");
    println!(
        "After update smaller: allocated={}, slot_size={}",
        manager.allocated(),
        manager.get_slot(&Embedder::Semantic).unwrap().size_bytes
    );
    assert_eq!(manager.allocated(), 300);

    // Only one slot should exist
    assert_eq!(manager.slot_count(), 1);

    println!("[PASS] Slot update (re-allocation) works correctly");
}

/// Test: Empty manager edge cases
#[test]
fn test_empty_manager_edge_cases() {
    let manager = ModelSlotManager::with_budget(1000);

    assert_eq!(manager.allocated(), 0);
    assert_eq!(manager.available(), 1000);
    assert_eq!(manager.slot_count(), 0);
    assert_eq!(manager.loaded_count(), 0);
    assert_eq!(manager.pressure_level(), MemoryPressure::Low);
    assert!(manager.get_lru_candidate().is_none());
    assert!(!manager.has_slot(&Embedder::Semantic));

    println!("[PASS] Empty manager edge cases handled correctly");
}

/// Test: Deallocate all slots
#[test]
fn test_deallocate_all_slots() {
    let mut manager = ModelSlotManager::with_budget(5000);

    // Allocate several
    manager.allocate_slot(Embedder::Semantic, 1000).unwrap();
    manager.allocate_slot(Embedder::Code, 1000).unwrap();
    manager.allocate_slot(Embedder::Causal, 1000).unwrap();

    assert_eq!(manager.allocated(), 3000);
    assert_eq!(manager.slot_count(), 3);

    // Deallocate all
    let freed1 = manager.deallocate_slot(&Embedder::Semantic);
    let freed2 = manager.deallocate_slot(&Embedder::Code);
    let freed3 = manager.deallocate_slot(&Embedder::Causal);

    assert_eq!(freed1, 1000);
    assert_eq!(freed2, 1000);
    assert_eq!(freed3, 1000);
    assert_eq!(manager.allocated(), 0);
    assert_eq!(manager.slot_count(), 0);

    println!("[PASS] Deallocate all slots works correctly");
}
