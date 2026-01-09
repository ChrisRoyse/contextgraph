//! Storage Lifecycle Tests.
//!
//! Tests for storage creation, migration, batch operations, and basic CRUD.

use context_graph_graph::storage::{LegacyGraphEdge, NodeId, PoincarePoint, SCHEMA_VERSION};

use crate::common::fixtures::generate_entailment_cone;
use crate::common::fixtures::generate_poincare_point;
use crate::common::helpers::{
    create_test_storage, measure_latency, verify_storage_state, StateLog,
};

/// Test storage creation, migration, and basic CRUD operations.
#[test]
fn test_storage_lifecycle_complete() {
    println!("\n=== TEST: Storage Lifecycle ===");

    // Create storage
    let log = StateLog::new("storage", "uninitialized");
    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");
    log.after("created");

    // Verify initial state
    verify_storage_state(&storage, 0, 0, 0).expect("Initial state verification failed");

    // Apply migrations
    let schema_before = storage.get_schema_version().expect("Get schema failed");
    assert_eq!(schema_before, 0, "New DB should have version 0");

    let schema_after = storage.apply_migrations().expect("Migration failed");
    assert_eq!(
        schema_after, SCHEMA_VERSION,
        "Should migrate to current version"
    );

    // Add hyperbolic point
    let point = generate_poincare_point(42, 0.9);
    storage
        .put_hyperbolic(1, &point)
        .expect("Put hyperbolic failed");

    // Add entailment cone
    let cone = generate_entailment_cone(42, 0.8, (0.2, 0.6));
    storage.put_cone(1, &cone).expect("Put cone failed");

    // Add adjacency
    storage
        .add_edge(
            1,
            LegacyGraphEdge {
                target: 2,
                edge_type: 1,
            },
        )
        .expect("Add edge failed");

    // Verify state after additions
    verify_storage_state(&storage, 1, 1, 1).expect("Post-add state verification failed");

    // Read back and verify hyperbolic point
    let retrieved_point = storage
        .get_hyperbolic(1)
        .expect("Get hyperbolic failed")
        .expect("Point should exist");
    assert_eq!(point.coords, retrieved_point.coords, "Points should match");

    // Read back and verify cone
    let retrieved_cone = storage
        .get_cone(1)
        .expect("Get cone failed")
        .expect("Cone should exist");
    assert!(
        (cone.aperture - retrieved_cone.aperture).abs() < 1e-6,
        "Apertures should match"
    );

    // Delete and verify
    storage
        .delete_hyperbolic(1)
        .expect("Delete hyperbolic failed");
    storage.delete_cone(1).expect("Delete cone failed");

    verify_storage_state(&storage, 0, 0, 1).expect("Post-delete state verification failed");

    println!("=== PASSED: Storage Lifecycle ===\n");
}

/// Test batch write operations with timing.
#[test]
fn test_storage_batch_operations() {
    println!("\n=== TEST: Storage Batch Operations ===");

    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    let batch_size = 1000;
    let points: Vec<PoincarePoint> = (0..batch_size)
        .map(|i| generate_poincare_point(i, 0.9))
        .collect();

    // Time batch insert
    let (_, timing) = measure_latency("batch_insert_1000_points", 100_000, || {
        for (i, point) in points.iter().enumerate() {
            storage
                .put_hyperbolic(i as NodeId, point)
                .expect("Put failed");
        }
    });

    assert!(
        timing.passed,
        "Batch insert should complete within NFR target"
    );

    // Verify count
    let count = storage.hyperbolic_count().expect("Count failed");
    assert_eq!(
        count, batch_size as usize,
        "Should have all {} entries",
        batch_size
    );

    // Time batch read
    let (_, read_timing) = measure_latency("batch_read_1000_points", 50_000, || {
        for i in 0..batch_size {
            let _ = storage.get_hyperbolic(i as NodeId).expect("Get failed");
        }
    });

    assert!(
        read_timing.passed,
        "Batch read should complete within NFR target"
    );

    println!("=== PASSED: Storage Batch Operations ===\n");
}
