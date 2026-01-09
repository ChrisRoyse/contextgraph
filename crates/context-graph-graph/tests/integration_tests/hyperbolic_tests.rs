//! Hyperbolic Geometry Tests (Poincare Ball).
//!
//! Tests for Poincare point operations, invariants, and distance properties.

use context_graph_graph::storage::PoincarePoint;

use crate::common::fixtures::{generate_poincare_point, POINCARE_MAX_NORM};

/// Test Poincare point operations and invariants.
#[test]
fn test_poincare_point_invariants() {
    println!("\n=== TEST: Poincare Point Invariants ===");

    // Test origin
    let origin = PoincarePoint::origin();
    let origin_norm = origin.norm();
    assert!(origin_norm.abs() < 1e-6, "Origin norm should be 0");

    // Test point generation respects max_norm
    for seed in 0..100 {
        let point = generate_poincare_point(seed, 0.9);
        let norm = point.norm();

        assert!(
            norm <= 0.9 + 1e-5,
            "Point norm {} exceeds max 0.9 for seed {}",
            norm,
            seed
        );
        assert!(norm >= 0.0, "Point norm cannot be negative");
    }

    // Test point validity (inside unit ball)
    for seed in 0..50 {
        let point = generate_poincare_point(seed, POINCARE_MAX_NORM);
        assert!(
            point.norm() < 1.0,
            "Point with max_norm {} should be inside unit ball",
            POINCARE_MAX_NORM
        );
    }

    // Test boundary points
    let mut boundary_point = PoincarePoint::origin();
    boundary_point.coords[0] = 0.99999;
    assert!(
        boundary_point.norm() < 1.0,
        "Boundary point should be inside ball"
    );

    // Just outside boundary should be invalid
    let mut outside_point = PoincarePoint::origin();
    outside_point.coords[0] = 1.0;
    assert!(
        outside_point.norm() >= 1.0,
        "Point on boundary should not be inside ball"
    );

    println!("=== PASSED: Poincare Point Invariants ===\n");
}

/// Test Poincare distance properties (CPU reference).
#[test]
fn test_poincare_distance_properties() {
    println!("\n=== TEST: Poincare Distance Properties ===");

    use context_graph_cuda::poincare::poincare_distance_cpu;

    // Reflexivity: d(x, x) = 0
    for seed in 0..10 {
        let point = generate_poincare_point(seed, 0.9);
        let dist = poincare_distance_cpu(&point.coords, &point.coords, -1.0);
        assert!(dist.abs() < 1e-5, "Reflexivity violated: d(x,x) = {}", dist);
    }

    // Symmetry: d(x, y) = d(y, x)
    for seed in 0..10 {
        let x = generate_poincare_point(seed, 0.9);
        let y = generate_poincare_point(seed + 100, 0.9);

        let d_xy = poincare_distance_cpu(&x.coords, &y.coords, -1.0);
        let d_yx = poincare_distance_cpu(&y.coords, &x.coords, -1.0);

        assert!(
            (d_xy - d_yx).abs() < 1e-5,
            "Symmetry violated: d(x,y)={}, d(y,x)={}",
            d_xy,
            d_yx
        );
    }

    // Triangle inequality: d(x, z) <= d(x, y) + d(y, z)
    for seed in 0..10 {
        let x = generate_poincare_point(seed, 0.5);
        let y = generate_poincare_point(seed + 100, 0.5);
        let z = generate_poincare_point(seed + 200, 0.5);

        let d_xy = poincare_distance_cpu(&x.coords, &y.coords, -1.0);
        let d_yz = poincare_distance_cpu(&y.coords, &z.coords, -1.0);
        let d_xz = poincare_distance_cpu(&x.coords, &z.coords, -1.0);

        assert!(
            d_xz <= d_xy + d_yz + 1e-4,
            "Triangle inequality violated: d(x,z)={} > d(x,y)+d(y,z)={}",
            d_xz,
            d_xy + d_yz
        );
    }

    // Non-negativity
    for seed in 0..20 {
        let x = generate_poincare_point(seed, 0.9);
        let y = generate_poincare_point(seed + 100, 0.9);
        let dist = poincare_distance_cpu(&x.coords, &y.coords, -1.0);
        assert!(dist >= 0.0, "Distance cannot be negative: {}", dist);
    }

    println!("=== PASSED: Poincare Distance Properties ===\n");
}
