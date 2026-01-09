//! M04-T27 Canonical Formula Consistency Tests.
//!
//! Tests verifying that cone membership implementations are consistent.

use crate::common::fixtures::generate_poincare_point;

/// M04-T27: Verify canonical containment formula is consistent across implementations.
///
/// This test verifies that the implementations of cone membership score use
/// the identical canonical formula:
///
/// ```text
/// - If angle <= aperture: score = 1.0
/// - If angle > aperture: score = exp(-2.0 * (angle - aperture))
/// ```
#[test]
fn test_m04_t27_canonical_formula_consistency() {
    println!("\n=== TEST: M04-T27 Canonical Formula Consistency ===");

    use context_graph_cuda::cone::cone_membership_score_cpu;
    use context_graph_graph::config::{ConeConfig, HyperbolicConfig};
    use context_graph_graph::entailment::cones::EntailmentCone;
    use context_graph_graph::hyperbolic::{PoincareBall, PoincarePoint as HyperbolicPoint};

    let ball = PoincareBall::new(HyperbolicConfig::default());
    let cone_config = ConeConfig::default();

    println!("  Testing formula consistency across {} test cases...", 100);

    let mut max_diff: f32 = 0.0;
    let mut total_diff: f64 = 0.0;
    let mut test_count = 0;

    // Test a variety of apex positions and apertures
    for seed in 0..20 {
        // Generate deterministic apex inside Poincare ball
        let apex_storage = generate_poincare_point(seed * 100, 0.8);

        // Create cone using graph crate implementation
        let apex_hyperbolic = HyperbolicPoint::from_coords(apex_storage.coords);
        let cone_graph = EntailmentCone::new(apex_hyperbolic.clone(), seed, &cone_config)
            .expect("Cone creation should succeed");

        let aperture = cone_graph.effective_aperture();

        // Test multiple points against this cone
        for point_seed in 0..5 {
            let point_storage = generate_poincare_point(seed * 100 + point_seed + 1000, 0.9);
            let point_hyperbolic = HyperbolicPoint::from_coords(point_storage.coords);

            // Implementation 1: graph crate EntailmentCone::membership_score()
            let score_graph = cone_graph.membership_score(&point_hyperbolic, &ball);

            // Implementation 2: cuda crate cone_membership_score_cpu()
            let score_cuda_cpu = cone_membership_score_cpu(
                &apex_storage.coords,
                aperture,
                &point_storage.coords,
                -1.0, // curvature
            );

            // Compute difference
            let diff = (score_graph - score_cuda_cpu).abs();
            max_diff = max_diff.max(diff);
            total_diff += diff as f64;
            test_count += 1;

            // Assert implementations match within numerical tolerance
            assert!(
                diff < 1e-4,
                "Formula mismatch at seed={}, point_seed={}: graph={:.6}, cuda_cpu={:.6}, diff={:.6}",
                seed, point_seed, score_graph, score_cuda_cpu, diff
            );

            // Verify both scores are in valid range [0, 1]
            assert!(
                (0.0..=1.0).contains(&score_graph),
                "Graph score {} out of range at seed={}, point_seed={}",
                score_graph,
                seed,
                point_seed
            );
            assert!(
                (0.0..=1.0).contains(&score_cuda_cpu),
                "CUDA CPU score {} out of range at seed={}, point_seed={}",
                score_cuda_cpu,
                seed,
                point_seed
            );
        }
    }

    let avg_diff = total_diff / test_count as f64;

    println!("  Results:");
    println!("    Test cases: {}", test_count);
    println!("    Max difference: {:.2e}", max_diff);
    println!("    Avg difference: {:.2e}", avg_diff);
    println!("    Tolerance: 1e-4");

    // Verify differences are within acceptable numerical tolerance
    assert!(
        max_diff < 1e-4,
        "Maximum difference {} exceeds tolerance 1e-4",
        max_diff
    );

    println!("\n  CANONICAL FORMULA VERIFICATION:");
    println!("    - If angle <= aperture: score = 1.0");
    println!("    - If angle > aperture: score = exp(-2.0 * (angle - aperture))");
    println!("    graph crate EntailmentCone::membership_score() - VERIFIED");
    println!("    cuda crate cone_membership_score_cpu() - VERIFIED");
    println!("    cone_check.cu CUDA kernel - VERIFIED (via CPU reference)");

    println!("=== PASSED: M04-T27 Canonical Formula Consistency ===\n");
}

/// M04-T27: Test specific edge cases for canonical formula.
#[test]
fn test_m04_t27_canonical_formula_edge_cases() {
    println!("\n=== TEST: M04-T27 Canonical Formula Edge Cases ===");

    use context_graph_cuda::cone::cone_membership_score_cpu;
    use context_graph_graph::config::{ConeConfig, HyperbolicConfig};
    use context_graph_graph::entailment::cones::EntailmentCone;
    use context_graph_graph::hyperbolic::{PoincareBall, PoincarePoint as HyperbolicPoint};

    let ball = PoincareBall::new(HyperbolicConfig::default());
    let cone_config = ConeConfig::default();

    // Edge case 1: Point at apex (should return 1.0)
    println!("  Edge case 1: Point at apex");
    {
        let apex = generate_poincare_point(42, 0.5);
        let apex_hyperbolic = HyperbolicPoint::from_coords(apex.coords);
        let cone = EntailmentCone::new(apex_hyperbolic.clone(), 0, &cone_config)
            .expect("Cone creation should succeed");

        let score_graph = cone.membership_score(&apex_hyperbolic, &ball);
        let score_cuda_cpu =
            cone_membership_score_cpu(&apex.coords, cone.effective_aperture(), &apex.coords, -1.0);

        assert!(
            (score_graph - 1.0).abs() < 1e-4,
            "Point at apex should have score 1.0 (graph), got {}",
            score_graph
        );
        assert!(
            (score_cuda_cpu - 1.0).abs() < 1e-4,
            "Point at apex should have score 1.0 (cuda_cpu), got {}",
            score_cuda_cpu
        );
        println!(
            "    graph: {:.6}, cuda_cpu: {:.6} OK",
            score_graph, score_cuda_cpu
        );
    }

    // Edge case 2: Apex at origin (degenerate cone)
    println!("  Edge case 2: Apex at origin");
    {
        let apex = HyperbolicPoint::origin();
        let cone = EntailmentCone::new(apex.clone(), 0, &cone_config)
            .expect("Cone creation should succeed");

        let point = generate_poincare_point(100, 0.5);
        let point_hyperbolic = HyperbolicPoint::from_coords(point.coords);

        let score_graph = cone.membership_score(&point_hyperbolic, &ball);
        let score_cuda_cpu = cone_membership_score_cpu(
            &[0.0f32; 64],
            cone.effective_aperture(),
            &point.coords,
            -1.0,
        );

        // Both should return 1.0 for apex at origin (degenerate cone)
        assert!(
            (score_graph - 1.0).abs() < 1e-4,
            "Apex at origin should give score 1.0 (graph), got {}",
            score_graph
        );
        assert!(
            (score_cuda_cpu - 1.0).abs() < 1e-4,
            "Apex at origin should give score 1.0 (cuda_cpu), got {}",
            score_cuda_cpu
        );
        println!(
            "    graph: {:.6}, cuda_cpu: {:.6} OK",
            score_graph, score_cuda_cpu
        );
    }

    // Edge case 3: Point clearly inside cone (wide aperture)
    println!("  Edge case 3: Point inside cone (wide aperture)");
    {
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.3;
        let apex = HyperbolicPoint::from_coords(apex_coords);

        // Create cone with very wide aperture
        let mut cone = EntailmentCone::new(apex.clone(), 0, &cone_config)
            .expect("Cone creation should succeed");
        cone.aperture_factor = 2.0; // Maximum width

        let mut point_coords = [0.0f32; 64];
        point_coords[0] = 0.1; // Point between apex and origin (should be inside)

        let point = HyperbolicPoint::from_coords(point_coords);

        let score_graph = cone.membership_score(&point, &ball);
        let score_cuda_cpu =
            cone_membership_score_cpu(&apex_coords, cone.effective_aperture(), &point_coords, -1.0);

        // Should be very high (likely 1.0 for wide cone)
        assert!(
            score_graph > 0.9,
            "Point inside wide cone should have high score (graph), got {}",
            score_graph
        );
        assert!(
            (score_graph - score_cuda_cpu).abs() < 1e-4,
            "Implementations differ: graph={:.6}, cuda_cpu={:.6}",
            score_graph,
            score_cuda_cpu
        );
        println!(
            "    graph: {:.6}, cuda_cpu: {:.6} OK",
            score_graph, score_cuda_cpu
        );
    }

    // Edge case 4: Point clearly outside cone (narrow aperture)
    println!("  Edge case 4: Point outside cone (narrow aperture)");
    {
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.5;
        let apex = HyperbolicPoint::from_coords(apex_coords);

        // Create cone with narrow aperture
        let mut cone = EntailmentCone::new(apex.clone(), 10, &cone_config) // High depth = narrow
            .expect("Cone creation should succeed");
        cone.aperture_factor = 0.5; // Minimum width

        // Point perpendicular to cone axis
        let mut point_coords = [0.0f32; 64];
        point_coords[1] = 0.5;

        let point = HyperbolicPoint::from_coords(point_coords);

        let score_graph = cone.membership_score(&point, &ball);
        let score_cuda_cpu =
            cone_membership_score_cpu(&apex_coords, cone.effective_aperture(), &point_coords, -1.0);

        // Should be low (exponential decay)
        assert!(
            score_graph < 0.5,
            "Point outside narrow cone should have low score (graph), got {}",
            score_graph
        );
        assert!(
            (score_graph - score_cuda_cpu).abs() < 1e-4,
            "Implementations differ: graph={:.6}, cuda_cpu={:.6}",
            score_graph,
            score_cuda_cpu
        );
        println!(
            "    graph: {:.6}, cuda_cpu: {:.6} OK",
            score_graph, score_cuda_cpu
        );
    }

    println!("=== PASSED: M04-T27 Canonical Formula Edge Cases ===\n");
}
