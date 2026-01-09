//! M04-T27 Batch Formula Comparison Tests.
//!
//! Tests for batch vs single formula comparison and exponential decay verification.

use crate::common::fixtures::{generate_entailment_cone, generate_poincare_point};

/// M04-T27: Batch comparison test for statistical validation.
///
/// This test validates that the batch CPU function produces the same results
/// as the single-score CPU function, confirming the canonical formula is
/// applied consistently in both code paths.
#[test]
fn test_m04_t27_batch_formula_comparison() {
    println!("\n=== TEST: M04-T27 Batch Formula Comparison ===");

    use context_graph_cuda::cone::{
        cone_check_batch_cpu, cone_membership_score_cpu, CONE_DATA_DIM,
    };

    let n_cones = 50;
    let n_points = 50;

    println!(
        "  Testing {}x{} = {} membership scores...",
        n_cones,
        n_points,
        n_cones * n_points
    );

    // Generate cones (use storage format directly to ensure consistency)
    let cones_storage: Vec<_> = (0..n_cones)
        .map(|i| generate_entailment_cone(i as u32 * 1000, 0.8, (0.2, 0.8)))
        .collect();

    // Generate points
    let points_storage: Vec<_> = (0..n_points)
        .map(|i| generate_poincare_point(i as u32 * 100 + 50000, 0.9))
        .collect();

    // Prepare batch data for cuda crate batch function
    let cones_flat: Vec<f32> = cones_storage
        .iter()
        .flat_map(|c| {
            let mut data = [0.0f32; CONE_DATA_DIM];
            data[..64].copy_from_slice(&c.apex.coords);
            data[64] = c.aperture;
            data.to_vec()
        })
        .collect();

    let points_flat: Vec<f32> = points_storage
        .iter()
        .flat_map(|p| p.coords.to_vec())
        .collect();

    // Compute batch scores using cuda crate
    let batch_scores = cone_check_batch_cpu(&cones_flat, &points_flat, n_cones, n_points, -1.0);

    // Compare each score: single function vs batch function
    let mut max_diff: f32 = 0.0;
    let mut total_diff: f64 = 0.0;
    let mut mismatches = 0;

    for (i, cone_storage) in cones_storage.iter().enumerate() {
        for (j, point_storage) in points_storage.iter().enumerate() {
            // Single cuda CPU score
            let score_single = cone_membership_score_cpu(
                &cone_storage.apex.coords,
                cone_storage.aperture,
                &point_storage.coords,
                -1.0,
            );

            // Batch cuda CPU score
            let score_batch = batch_scores[i * n_points + j];

            // Check single vs batch cuda CPU
            let diff = (score_single - score_batch).abs();
            max_diff = max_diff.max(diff);
            total_diff += diff as f64;

            if diff > 1e-5 {
                mismatches += 1;
                println!("    WARNING: Single/batch mismatch at [{},{}]: single={:.6}, batch={:.6}, diff={:.6}",
                    i, j, score_single, score_batch, diff);
            }
        }
    }

    let avg_diff = total_diff / (n_cones * n_points) as f64;

    println!("  Results:");
    println!("    Total comparisons: {}", n_cones * n_points);
    println!("    Single/batch mismatches: {}", mismatches);
    println!("    Max diff: {:.2e}", max_diff);
    println!("    Avg diff: {:.2e}", avg_diff);

    // Assert batch is internally consistent
    assert_eq!(mismatches, 0, "Batch function should match single function");

    // Assert batch scores are all valid
    for (idx, &score) in batch_scores.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&score) && score.is_finite(),
            "Invalid batch score at index {}: {}",
            idx,
            score
        );
    }

    println!("=== PASSED: M04-T27 Batch Formula Comparison ===\n");
}

/// M04-T27: Verify exponential decay formula specifically.
#[test]
fn test_m04_t27_exponential_decay_verification() {
    println!("\n=== TEST: M04-T27 Exponential Decay Verification ===");

    use context_graph_cuda::cone::cone_membership_score_cpu;
    use context_graph_graph::config::{ConeConfig, HyperbolicConfig};
    use context_graph_graph::entailment::cones::EntailmentCone;
    use context_graph_graph::hyperbolic::{PoincareBall, PoincarePoint as HyperbolicPoint};

    let ball = PoincareBall::new(HyperbolicConfig::default());
    let cone_config = ConeConfig::default();

    // Create a controlled test where we know angle > aperture
    let mut apex_coords = [0.0f32; 64];
    apex_coords[0] = 0.4;
    let apex = HyperbolicPoint::from_coords(apex_coords);

    let mut cone =
        EntailmentCone::new(apex.clone(), 0, &cone_config).expect("Cone creation should succeed");
    // Set aperture_factor to 1.0 for predictable aperture
    cone.aperture_factor = 1.0;
    let aperture = cone.effective_aperture();

    println!(
        "  Testing exponential decay with aperture = {:.4}",
        aperture
    );

    // Test multiple points at different angles
    for i in 0..5 {
        let mut point_coords = [0.0f32; 64];
        point_coords[0] = 0.2;
        point_coords[1] = 0.1 + (i as f32) * 0.1;

        let point = HyperbolicPoint::from_coords(point_coords);

        let score_graph = cone.membership_score(&point, &ball);
        let score_cuda_cpu = cone_membership_score_cpu(&apex_coords, aperture, &point_coords, -1.0);

        let diff = (score_graph - score_cuda_cpu).abs();
        assert!(
            diff < 1e-4,
            "Decay test {}: implementations differ by {} (graph={:.6}, cuda={:.6})",
            i,
            diff,
            score_graph,
            score_cuda_cpu
        );

        println!(
            "    Point {}: graph={:.6}, cuda_cpu={:.6}, diff={:.2e}",
            i, score_graph, score_cuda_cpu, diff
        );
    }

    println!("  Exponential decay consistency verified OK");
    println!("=== PASSED: M04-T27 Exponential Decay Verification ===\n");
}
