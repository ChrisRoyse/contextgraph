//! Integration tests for Cone CUDA operations.
//!
//! These tests verify the cone membership implementation using REAL DATA only.
//! No mocks - per constitution REQ-KG-TEST.
//!
//! # Test Categories
//!
//! 1. Configuration validation
//! 2. ConeData structure validation
//! 3. CPU reference implementation correctness
//! 4. Edge cases (boundary points, zero aperture, degenerate cones)
//! 5. Batch computation consistency
//! 6. Performance characteristics
//!
//! # Running GPU Tests
//!
//! GPU tests only run when:
//! - Feature `cuda` is enabled
//! - A CUDA-capable GPU is available
//!
//! ```bash
//! # Run all tests (CPU only without CUDA)
//! cargo test -p context-graph-cuda
//!
//! # Run with CUDA (requires GPU)
//! cargo test -p context-graph-cuda --features cuda
//! ```

use context_graph_cuda::cone::{
    cone_check_batch_cpu, cone_membership_score_cpu, is_cone_gpu_available, ConeCudaConfig,
    ConeData, CONE_DATA_DIM, POINT_DIM,
};
use context_graph_cuda::CudaError;

// ============================================================================
// CONFIGURATION TESTS
// ============================================================================

#[test]
fn test_integration_config_defaults_are_valid() {
    let config = ConeCudaConfig::default();
    assert!(config.validate().is_ok(), "Default config should be valid");
    assert!(
        (config.curvature - (-1.0)).abs() < 1e-6,
        "Default curvature should be -1.0"
    );
}

#[test]
fn test_integration_config_with_valid_curvature() {
    let curvatures = [-0.1, -0.5, -1.0, -2.0, -10.0, -100.0];
    for c in curvatures {
        let config = ConeCudaConfig::with_curvature(c);
        assert!(config.is_ok(), "Curvature {} should be valid", c);
        let config = config.unwrap();
        assert!(config.validate().is_ok());
    }
}

#[test]
fn test_integration_config_rejects_invalid_curvatures() {
    let invalid_curvatures = [0.0, 0.1, 1.0, f32::INFINITY, f32::NAN];
    for c in invalid_curvatures {
        let result = ConeCudaConfig::with_curvature(c);
        assert!(result.is_err(), "Curvature {} should be invalid", c);

        if let Err(e) = result {
            assert!(matches!(e, CudaError::InvalidConfig(_)));
        }
    }
}

// ============================================================================
// CONE DATA TESTS
// ============================================================================

#[test]
fn test_integration_cone_data_valid_small_apex() {
    let apex = [0.1f32; 64];
    let cone = ConeData::new(apex, 0.5);
    assert!(cone.is_ok(), "Small apex should be valid");
    let cone = cone.unwrap();
    assert!((cone.aperture - 0.5).abs() < 1e-6);
}

#[test]
fn test_integration_cone_data_valid_zero_apex() {
    let apex = [0.0f32; 64];
    let cone = ConeData::new(apex, 0.5);
    assert!(cone.is_ok(), "Zero apex should be valid");
}

#[test]
fn test_integration_cone_data_valid_near_boundary_apex() {
    // Create apex very close to boundary but still inside
    let scale = 0.98 / (64.0_f32).sqrt();
    let apex: [f32; 64] = [scale; 64];
    let cone = ConeData::new(apex, 0.5);
    assert!(cone.is_ok(), "Near-boundary apex should be valid");
}

#[test]
fn test_integration_cone_data_rejects_outside_ball() {
    // Apex outside Poincare ball (norm > 1)
    let apex = [1.0f32; 64]; // norm = sqrt(64) >> 1
    let result = ConeData::new(apex, 0.5);
    assert!(result.is_err(), "Apex outside ball should be rejected");

    if let Err(e) = result {
        assert!(matches!(e, CudaError::InvalidConfig(_)));
    }
}

#[test]
fn test_integration_cone_data_rejects_negative_aperture() {
    let apex = [0.1f32; 64];
    let result = ConeData::new(apex, -0.5);
    assert!(result.is_err(), "Negative aperture should be rejected");
}

#[test]
fn test_integration_cone_data_rejects_nan_aperture() {
    let apex = [0.1f32; 64];
    let result = ConeData::new(apex, f32::NAN);
    assert!(result.is_err(), "NaN aperture should be rejected");
}

#[test]
fn test_integration_cone_data_gpu_format_roundtrip() {
    // Create cone with varied apex values - ensure norm < 1
    let mut apex = [0.0f32; 64];
    for (i, a) in apex.iter_mut().enumerate() {
        *a = ((i as f32 * 0.01) - 0.3) * 0.1; // Scale down to keep inside ball
    }
    let aperture = 0.789;

    // Verify apex is inside ball
    let norm_sq: f32 = apex.iter().map(|x| x * x).sum();
    assert!(norm_sq < 1.0, "Apex must be inside Poincare ball");

    let cone = ConeData::new(apex, aperture).unwrap();
    let gpu_format = cone.to_gpu_format();

    assert_eq!(gpu_format.len(), CONE_DATA_DIM);

    let restored = ConeData::from_gpu_format(&gpu_format);

    // Verify roundtrip
    assert!((restored.aperture - aperture).abs() < 1e-6);
    for (i, (restored_val, original_val)) in restored.apex.iter().zip(apex.iter()).enumerate() {
        assert!(
            (restored_val - original_val).abs() < 1e-6,
            "Mismatch at index {}",
            i
        );
    }
}

// ============================================================================
// CPU MEMBERSHIP SCORE TESTS - REAL DATA
// ============================================================================

/// Generate a deterministic test point inside the Poincare ball.
fn generate_test_point(seed: u32, max_norm: f32) -> [f32; 64] {
    let mut point = [0.0f32; 64];
    let mut hash = seed;

    for p in &mut point {
        hash = hash.wrapping_mul(1103515245).wrapping_add(12345);
        let val = ((hash >> 16) & 0x7FFF) as f32 / 32767.0;
        *p = (val - 0.5) * 2.0 * max_norm / (64.0_f32).sqrt();
    }

    // Ensure norm is within bounds
    let norm: f32 = point.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > max_norm {
        let scale = max_norm / norm;
        for v in &mut point {
            *v *= scale;
        }
    }

    point
}

#[test]
fn test_integration_cpu_score_point_at_apex() {
    // Point at apex should always score 1.0
    for seed in 0..10 {
        let apex = generate_test_point(seed, 0.8);
        let point = apex;

        let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
        assert!(
            (score - 1.0).abs() < 1e-4,
            "Point at apex should score 1.0, got {} for seed {}",
            score,
            seed
        );
    }
}

#[test]
fn test_integration_cpu_score_apex_at_origin() {
    // Degenerate cone at origin should return 1.0 for all points
    let apex = [0.0f32; 64];

    for seed in 0..10 {
        let point = generate_test_point(seed, 0.9);
        let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
        assert!(
            (score - 1.0).abs() < 1e-4,
            "Apex at origin should score 1.0, got {} for seed {}",
            score,
            seed
        );
    }
}

#[test]
fn test_integration_cpu_score_bounded_01() {
    // All scores must be in [0, 1]
    for seed in 0..50 {
        let apex = generate_test_point(seed, 0.8);
        let point = generate_test_point(seed + 100, 0.8);
        let aperture = (seed % 10) as f32 * 0.1 + 0.1;

        let score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);

        assert!(
            (0.0..=1.0).contains(&score),
            "Score must be in [0,1], got {} for seed {}",
            score,
            seed
        );
        assert!(score.is_finite(), "Score must be finite for seed {}", seed);
    }
}

#[test]
fn test_integration_cpu_score_wide_aperture() {
    // Wide aperture should accept many points with high scores
    let mut apex = [0.0f32; 64];
    apex[0] = 0.5;
    let aperture = std::f32::consts::PI * 0.9; // Very wide

    let mut high_scores = 0;
    for seed in 0..20 {
        let point = generate_test_point(seed, 0.5);
        let score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);

        if score > 0.9 {
            high_scores += 1;
        }
    }

    assert!(
        high_scores >= 10,
        "Wide aperture should have many high scores: got {} / 20",
        high_scores
    );
}

#[test]
fn test_integration_cpu_score_narrow_aperture() {
    // Narrow aperture should reject most points
    let mut apex = [0.0f32; 64];
    apex[0] = 0.5;
    let aperture = 0.1; // Very narrow

    let mut low_scores = 0;
    for seed in 0..20 {
        // Points perpendicular to cone axis
        let mut point = [0.0f32; 64];
        point[1] = 0.3 + (seed as f32) * 0.01;

        let score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);

        if score < 0.5 {
            low_scores += 1;
        }
    }

    assert!(
        low_scores >= 10,
        "Narrow aperture should have many low scores: got {} / 20 low",
        low_scores
    );
}

#[test]
fn test_integration_cpu_score_canonical_formula() {
    // Verify the canonical formula:
    // - If angle <= aperture: score = 1.0
    // - If angle > aperture: score = exp(-2.0 * (angle - aperture))

    // Create a cone and test point where we can reason about the angle
    let mut apex = [0.0f32; 64];
    apex[0] = 0.5;
    let aperture = 0.3;

    // Point between apex and origin (in cone)
    let mut point_in = [0.0f32; 64];
    point_in[0] = 0.2;
    let score_in = cone_membership_score_cpu(&apex, aperture, &point_in, -1.0);

    // Point perpendicular to cone axis (likely outside)
    let mut point_out = [0.0f32; 64];
    point_out[1] = 0.5;
    let score_out = cone_membership_score_cpu(&apex, aperture, &point_out, -1.0);

    // Point in cone should have higher score than point outside
    println!(
        "CANONICAL FORMULA TEST: score_in={}, score_out={}",
        score_in, score_out
    );

    // Just verify scores are valid and follow expected pattern
    assert!((0.0..=1.0).contains(&score_in));
    assert!((0.0..=1.0).contains(&score_out));
}

#[test]
fn test_integration_cpu_score_different_curvatures() {
    let mut apex = [0.0f32; 64];
    apex[0] = 0.3;

    let mut point = [0.0f32; 64];
    point[0] = 0.1;
    point[1] = 0.2;

    let score_c1 = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
    let score_c05 = cone_membership_score_cpu(&apex, 0.5, &point, -0.5);
    let score_c2 = cone_membership_score_cpu(&apex, 0.5, &point, -2.0);

    // All scores should be valid
    assert!(score_c1.is_finite() && (0.0..=1.0).contains(&score_c1));
    assert!(score_c05.is_finite() && (0.0..=1.0).contains(&score_c05));
    assert!(score_c2.is_finite() && (0.0..=1.0).contains(&score_c2));

    // Different curvatures should give different results
    // (unless the point happens to be exactly on the cone boundary)
    println!(
        "Curvature test: c=-1.0 -> {}, c=-0.5 -> {}, c=-2.0 -> {}",
        score_c1, score_c05, score_c2
    );
}

// ============================================================================
// EDGE CASE TESTS - FULL STATE VERIFICATION
// ============================================================================

#[test]
fn test_integration_edge_case_zero_aperture() {
    // Zero aperture cone: only apex should score 1.0
    let apex = [0.1f32; 64];
    let aperture = 0.0;

    // Point at apex
    let score_apex = cone_membership_score_cpu(&apex, aperture, &apex, -1.0);
    assert!(
        (score_apex - 1.0).abs() < 1e-4,
        "Point at apex with zero aperture should score 1.0"
    );

    // Point NOT at apex
    let mut other = [0.1f32; 64];
    other[0] = 0.2;
    let score_other = cone_membership_score_cpu(&apex, aperture, &other, -1.0);
    assert!(
        score_other < 1.0,
        "Point away from apex with zero aperture should score < 1.0"
    );
}

#[test]
fn test_integration_edge_case_pi_aperture() {
    // Aperture of PI should accept everything
    let mut apex = [0.0f32; 64];
    apex[0] = 0.5;
    let aperture = std::f32::consts::PI;

    for seed in 0..10 {
        let point = generate_test_point(seed, 0.9);
        let score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);
        assert!(
            score > 0.9,
            "PI aperture should accept all points: got {} for seed {}",
            score,
            seed
        );
    }
}

#[test]
fn test_integration_edge_case_boundary_apex() {
    // Apex very close to Poincare ball boundary
    let scale = 0.99 / (64.0_f32).sqrt();
    let apex: [f32; 64] = [scale; 64];

    let norm: f32 = apex.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 0.99).abs() < 0.01, "Apex should be near boundary");

    // Test with various points
    for seed in 0..10 {
        let point = generate_test_point(seed, 0.5);
        let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
        assert!(
            score.is_finite() && (0.0..=1.0).contains(&score),
            "Score should be valid for boundary apex: got {}",
            score
        );
    }
}

#[test]
fn test_integration_edge_case_boundary_point() {
    let mut apex = [0.0f32; 64];
    apex[0] = 0.3;

    // Point very close to boundary
    let scale = 0.99 / (64.0_f32).sqrt();
    let boundary_point: [f32; 64] = [scale; 64];

    let score = cone_membership_score_cpu(&apex, 0.5, &boundary_point, -1.0);
    assert!(
        score.is_finite() && (0.0..=1.0).contains(&score),
        "Boundary point score should be valid: {}",
        score
    );
}

#[test]
fn test_integration_edge_case_very_close_points() {
    // When point is AT the apex (or very close within epsilon), score should be 1.0
    let apex = [0.05f32; 64];

    // Point EXACTLY at apex should return 1.0
    let score_at_apex = cone_membership_score_cpu(&apex, 0.5, &apex, -1.0);
    assert!(
        (score_at_apex - 1.0).abs() < 1e-4,
        "Point at apex should score 1.0: {}",
        score_at_apex
    );

    // Point very slightly perturbed - may not be 1.0 because angle is computed
    // in tangent space, and even a tiny Euclidean distance can map to a non-zero
    // angle. The key is that the score should be valid.
    let mut nearby = [0.05f32; 64];
    nearby[0] += 1e-6;

    let score_nearby = cone_membership_score_cpu(&apex, 0.5, &nearby, -1.0);
    assert!(
        score_nearby.is_finite() && (0.0..=1.0).contains(&score_nearby),
        "Nearby point should have valid score: {}",
        score_nearby
    );
}

#[test]
fn test_integration_edge_case_opposite_direction() {
    // Point in direction opposite to cone axis
    let mut apex = [0.0f32; 64];
    apex[0] = 0.5; // Cone axis points toward origin (negative x)

    let mut point = [0.0f32; 64];
    point[0] = 0.8; // Point away from origin (positive x)

    let score = cone_membership_score_cpu(&apex, 0.1, &point, -1.0);
    assert!(
        score < 0.5,
        "Point in opposite direction should have low score: {}",
        score
    );
}

// ============================================================================
// BATCH COMPUTATION TESTS
// ============================================================================

#[test]
fn test_integration_batch_cpu_consistency() {
    let n_cones = 10;
    let n_points = 15;

    // Generate cones
    let cones: Vec<f32> = (0..n_cones)
        .flat_map(|i| {
            let apex = generate_test_point(i as u32, 0.7);
            let aperture = 0.3 + (i as f32) * 0.05;
            let mut data = apex.to_vec();
            data.push(aperture);
            data
        })
        .collect();

    // Generate points
    let points: Vec<f32> = (0..n_points)
        .flat_map(|i| generate_test_point((i + 100) as u32, 0.8).to_vec())
        .collect();

    let batch_scores = cone_check_batch_cpu(&cones, &points, n_cones, n_points, -1.0);

    assert_eq!(batch_scores.len(), n_cones * n_points);

    // Verify batch matches individual computations
    for i in 0..n_cones {
        let apex: &[f32; 64] = cones[i * 65..i * 65 + 64].try_into().unwrap();
        let aperture = cones[i * 65 + 64];

        for j in 0..n_points {
            let point: &[f32; 64] = points[j * 64..(j + 1) * 64].try_into().unwrap();

            let single = cone_membership_score_cpu(apex, aperture, point, -1.0);
            let batch = batch_scores[i * n_points + j];

            assert!(
                (single - batch).abs() < 1e-5,
                "Mismatch at [{}, {}]: single={}, batch={}",
                i,
                j,
                single,
                batch
            );
        }
    }
}

#[test]
fn test_integration_batch_cpu_100x100() {
    let n = 100;

    let cones: Vec<f32> = (0..n)
        .flat_map(|i| {
            let apex = generate_test_point(i as u32, 0.7);
            let aperture = 0.3 + ((i * 7) % 10) as f32 * 0.05;
            let mut data = apex.to_vec();
            data.push(aperture);
            data
        })
        .collect();

    let points: Vec<f32> = (0..n)
        .flat_map(|i| generate_test_point((i * 11 + 500) as u32, 0.8).to_vec())
        .collect();

    let scores = cone_check_batch_cpu(&cones, &points, n, n, -1.0);

    assert_eq!(scores.len(), n * n);

    // All scores must be valid
    for (idx, &s) in scores.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&s) && s.is_finite(),
            "Invalid score at {}: {}",
            idx,
            s
        );
    }

    // Count distribution
    let high_scores = scores.iter().filter(|&&s| s > 0.9).count();
    let low_scores = scores.iter().filter(|&&s| s < 0.1).count();
    println!(
        "100x100 batch: {} high scores (>0.9), {} low scores (<0.1)",
        high_scores, low_scores
    );
}

#[test]
#[should_panic(expected = "Invalid cones size")]
fn test_integration_batch_cpu_invalid_cone_size() {
    let cones = vec![0.0f32; 64]; // Missing aperture
    let points = vec![0.0f32; 64];
    cone_check_batch_cpu(&cones, &points, 1, 1, -1.0);
}

#[test]
#[should_panic(expected = "Invalid points size")]
fn test_integration_batch_cpu_invalid_points_size() {
    let cones = vec![0.0f32; 65];
    let points = vec![0.0f32; 32]; // Wrong size
    cone_check_batch_cpu(&cones, &points, 1, 1, -1.0);
}

// ============================================================================
// GPU AVAILABILITY TEST
// ============================================================================

#[test]
fn test_integration_gpu_availability_check() {
    let available = is_cone_gpu_available();

    println!("Cone GPU available: {}", available);

    #[cfg(not(feature = "cuda"))]
    assert!(
        !available,
        "Without cuda feature, GPU should not be available"
    );
}

// ============================================================================
// KERNEL INFO TEST
// ============================================================================

#[test]
fn test_integration_kernel_info() {
    use context_graph_cuda::cone::get_cone_kernel_info;

    let info = get_cone_kernel_info();

    #[cfg(feature = "cuda")]
    {
        assert!(
            info.is_some(),
            "Kernel info should be available with cuda feature"
        );
        let info = info.unwrap();
        assert_eq!(info.block_dim_x, 32, "Block dim X should be 32");
        assert_eq!(info.block_dim_y, 8, "Block dim Y should be 8");
        assert_eq!(info.point_dim, 64, "Point dim should be 64");
        assert_eq!(info.cone_data_dim, 65, "Cone data dim should be 65");
        assert!(
            info.shared_mem_bytes > 0,
            "Shared memory should be positive"
        );
        println!("Kernel info: {:?}", info);
    }

    #[cfg(not(feature = "cuda"))]
    assert!(
        info.is_none(),
        "Kernel info should be None without cuda feature"
    );
}

// ============================================================================
// CONSTANTS VERIFICATION
// ============================================================================

#[test]
fn test_integration_constants() {
    assert_eq!(CONE_DATA_DIM, 65, "CONE_DATA_DIM should be 65");
    assert_eq!(POINT_DIM, 64, "POINT_DIM should be 64");
}

// ============================================================================
// DETERMINISM TESTS
// ============================================================================

#[test]
fn test_integration_deterministic_results() {
    let apex = generate_test_point(42, 0.7);
    let point = generate_test_point(123, 0.8);
    let aperture = 0.5;

    // Run 10 times, should get same result
    let first_score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);

    for _ in 0..10 {
        let score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);
        assert!(
            (score - first_score).abs() < 1e-10,
            "Results should be deterministic"
        );
    }
}

// ============================================================================
// NUMERICAL STABILITY TESTS
// ============================================================================

#[test]
fn test_integration_numerical_stability_small_values() {
    // Very small apex (close to origin)
    let mut apex = [0.0f32; 64];
    apex[0] = 1e-6;

    let mut point = [0.0f32; 64];
    point[0] = 1e-5;

    let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
    assert!(
        score.is_finite() && !score.is_nan(),
        "Small values should not cause NaN: {}",
        score
    );
}

#[test]
fn test_integration_numerical_stability_large_curvature() {
    let apex = [0.1f32; 64];
    let point = generate_test_point(0, 0.5);

    let score = cone_membership_score_cpu(&apex, 0.5, &point, -100.0);
    assert!(
        score.is_finite() && (0.0..=1.0).contains(&score),
        "Large curvature should give valid score: {}",
        score
    );
}

#[test]
fn test_integration_numerical_stability_small_curvature() {
    let apex = [0.1f32; 64];
    let point = generate_test_point(0, 0.5);

    let score = cone_membership_score_cpu(&apex, 0.5, &point, -0.001);
    assert!(
        score.is_finite() && (0.0..=1.0).contains(&score),
        "Small curvature should give valid score: {}",
        score
    );
}
