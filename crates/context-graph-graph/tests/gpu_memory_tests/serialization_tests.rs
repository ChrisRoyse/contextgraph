//! Serialization tests for GPU Memory Manager
//!
//! Tests config serialization, stats serialization, and memory category
//! enum serialization using serde_json.

use context_graph_graph::index::gpu_memory::{
    GpuMemoryConfig, GpuMemoryManager, MemoryCategory, MemoryStats,
};

#[test]
fn test_config_serialization() {
    println!("\n=== TEST: Config Serialization ===");

    let config = GpuMemoryConfig::with_budget(1_000_000)
        .category_budget(MemoryCategory::FaissIndex, 500_000);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&config).expect("Serialization failed");
    println!("Config JSON:\n{}", json);

    // Deserialize back
    let config2: GpuMemoryConfig = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(config.total_budget, config2.total_budget);
    assert_eq!(
        config.category_budgets.get(&MemoryCategory::FaissIndex),
        config2.category_budgets.get(&MemoryCategory::FaissIndex)
    );

    println!("=== PASSED ===\n");
}

#[test]
fn test_stats_serialization() {
    println!("\n=== TEST: Stats Serialization ===");

    let manager = GpuMemoryManager::new(GpuMemoryConfig::with_budget(1_000_000))
        .expect("Manager creation failed");

    let _h = manager
        .allocate(500_000, MemoryCategory::WorkingMemory)
        .expect("Allocation failed");

    let stats = manager.stats();

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&stats).expect("Serialization failed");
    println!("Stats JSON:\n{}", json);

    // Deserialize back
    let stats2: MemoryStats = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(stats.total_allocated, stats2.total_allocated);
    assert_eq!(stats.total_budget, stats2.total_budget);

    println!("=== PASSED ===\n");
}

#[test]
fn test_memory_category_serialization() {
    println!("\n=== TEST: Memory Category Serialization ===");

    let categories = [
        MemoryCategory::FaissIndex,
        MemoryCategory::HyperbolicCoords,
        MemoryCategory::EntailmentCones,
        MemoryCategory::WorkingMemory,
        MemoryCategory::Other,
    ];

    for cat in categories.iter() {
        let json = serde_json::to_string(cat).expect("Serialization failed");
        println!("{:?} -> {}", cat, json);

        let deserialized: MemoryCategory =
            serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(*cat, deserialized);
    }

    println!("=== PASSED ===\n");
}
