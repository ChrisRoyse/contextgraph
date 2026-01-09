//! Edge Cases and Boundary Condition Tests.
//!
//! Tests for boundary points, large batches, NT weight boundaries,
//! empty graph cases, and fixture determinism.

use context_graph_graph::{
    storage::{NodeId, PoincarePoint},
    Domain, NeurotransmitterWeights,
};

use crate::common::fixtures::{generate_entailment_cone, generate_poincare_point};
use crate::common::helpers::{create_test_storage, measure_latency, verify_storage_state};

/// Test boundary points at Poincare ball edge.
#[test]
fn test_poincare_boundary_points() {
    println!("\n=== TEST: Poincare Boundary Points ===");

    use context_graph_cuda::poincare::poincare_distance_cpu;

    // Point very close to boundary
    let scale = 0.99999 / (64.0_f32).sqrt();
    let mut near_boundary = PoincarePoint::origin();
    for i in 0..64 {
        near_boundary.coords[i] = scale;
    }

    let norm = near_boundary.norm();
    println!("  Near-boundary point norm: {:.6}", norm);
    assert!(norm < 1.0, "Point should be inside ball");
    assert!(norm > 0.999, "Point should be very close to boundary");

    // Distance to origin should be large for boundary points
    let origin = PoincarePoint::origin();
    let dist_to_origin = poincare_distance_cpu(&near_boundary.coords, &origin.coords, -1.0);
    println!("  Distance to origin: {:.6}", dist_to_origin);
    assert!(
        dist_to_origin > 5.0,
        "Boundary points should be far from origin in hyperbolic space"
    );

    // Test two boundary points
    let mut boundary_a = PoincarePoint::origin();
    boundary_a.coords[0] = 0.999;

    let mut boundary_b = PoincarePoint::origin();
    boundary_b.coords[1] = 0.999;

    let boundary_dist = poincare_distance_cpu(&boundary_a.coords, &boundary_b.coords, -1.0);
    println!(
        "  Distance between orthogonal boundary points: {:.6}",
        boundary_dist
    );

    println!("=== PASSED: Poincare Boundary Points ===\n");
}

/// Test large batch operations.
#[test]
fn test_large_batch_operations() {
    println!("\n=== TEST: Large Batch Operations ===");

    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    let batch_size: usize = 10000;

    // Prepare batch
    let (_, prepare_timing) = measure_latency("prepare_10k_points", 1_000_000, || {
        for i in 0..batch_size {
            let point = generate_poincare_point(i as u32, 0.9);
            storage
                .put_hyperbolic(i as NodeId, &point)
                .expect("Put failed");
        }
    });

    println!("  Batch prepare timing passed: {}", prepare_timing.passed);

    // Verify
    let count = storage.hyperbolic_count().expect("Count failed");
    assert_eq!(count, batch_size, "Should have all {} entries", batch_size);

    println!("  Batch size: {}", batch_size);
    println!("=== PASSED: Large Batch Operations ===\n");
}

/// Test NT weights boundary values.
#[test]
fn test_nt_weights_boundaries() {
    println!("\n=== TEST: NT Weights Boundaries ===");

    // Test boundary valid values
    let min_weights = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
    assert!(min_weights.validate(), "Min weights should be valid");

    let max_weights = NeurotransmitterWeights::new(1.0, 1.0, 1.0);
    assert!(max_weights.validate(), "Max weights should be valid");

    // Test mid-range values
    let mid_weights = NeurotransmitterWeights::new(0.5, 0.5, 0.5);
    assert!(mid_weights.validate(), "Mid weights should be valid");

    // Test all domain profiles
    for domain in Domain::all() {
        let domain_weights = NeurotransmitterWeights::for_domain(domain);
        assert!(
            domain_weights.validate(),
            "Domain {:?} weights should be valid",
            domain
        );

        // Verify in [0,1] range
        assert!(
            domain_weights.excitatory >= 0.0 && domain_weights.excitatory <= 1.0,
            "excitatory should be in [0,1]"
        );
        assert!(
            domain_weights.inhibitory >= 0.0 && domain_weights.inhibitory <= 1.0,
            "inhibitory should be in [0,1]"
        );
        assert!(
            domain_weights.modulatory >= 0.0 && domain_weights.modulatory <= 1.0,
            "modulatory should be in [0,1]"
        );

        println!(
            "  {:?}: exc={:.2}, inh={:.2}, mod={:.2}",
            domain, domain_weights.excitatory, domain_weights.inhibitory, domain_weights.modulatory
        );
    }

    println!("=== PASSED: NT Weights Boundaries ===\n");
}

/// Test empty graph edge cases.
#[test]
fn test_empty_graph_edge_cases() {
    println!("\n=== TEST: Empty Graph Edge Cases ===");

    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    // Query non-existent node
    let non_existent = storage.get_hyperbolic(99999).expect("Get should not fail");
    assert!(
        non_existent.is_none(),
        "Non-existent node should return None"
    );

    // Query non-existent cone
    let non_existent_cone = storage.get_cone(99999).expect("Get should not fail");
    assert!(
        non_existent_cone.is_none(),
        "Non-existent cone should return None"
    );

    // Query empty adjacency
    let empty_adj = storage
        .get_adjacency(99999)
        .expect("Get adjacency should not fail");
    assert!(
        empty_adj.is_empty(),
        "Non-existent node should have empty adjacency"
    );

    // Verify counts
    verify_storage_state(&storage, 0, 0, 0).expect("Empty state verification failed");

    println!("=== PASSED: Empty Graph Edge Cases ===\n");
}

/// Test determinism of fixture generation.
#[test]
fn test_fixture_determinism() {
    println!("\n=== TEST: Fixture Determinism ===");

    // Same seed should produce identical results
    let point1 = generate_poincare_point(42, 0.9);
    let point2 = generate_poincare_point(42, 0.9);

    assert_eq!(
        point1.coords, point2.coords,
        "Same seed should produce identical points"
    );

    let cone1 = generate_entailment_cone(123, 0.8, (0.2, 0.6));
    let cone2 = generate_entailment_cone(123, 0.8, (0.2, 0.6));

    assert_eq!(
        cone1.apex.coords, cone2.apex.coords,
        "Same seed should produce identical cones"
    );
    assert_eq!(
        cone1.aperture, cone2.aperture,
        "Same seed should produce identical apertures"
    );

    // Different seeds should produce different results
    let point3 = generate_poincare_point(43, 0.9);
    assert_ne!(
        point1.coords, point3.coords,
        "Different seeds should produce different points"
    );

    println!("=== PASSED: Fixture Determinism ===\n");
}
