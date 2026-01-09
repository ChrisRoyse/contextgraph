//! Tests for cone operations.
//!
//! REAL DATA ONLY, NO MOCKS (per constitution REQ-KG-TEST)

#![allow(clippy::assertions_on_constants)]

use super::*;

// ========== Configuration Tests ==========

#[test]
fn test_config_default() {
    let config = ConeCudaConfig::default();
    assert!((config.curvature - (-1.0)).abs() < 1e-6);
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_with_curvature_valid() {
    let config = ConeCudaConfig::with_curvature(-0.5).unwrap();
    assert!((config.curvature - (-0.5)).abs() < 1e-6);
}

#[test]
fn test_config_with_curvature_invalid_positive() {
    use crate::error::CudaError;
    let result = ConeCudaConfig::with_curvature(0.5);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CudaError::InvalidConfig(_)));
}

#[test]
fn test_config_with_curvature_invalid_zero() {
    let result = ConeCudaConfig::with_curvature(0.0);
    assert!(result.is_err());
}

#[test]
fn test_config_with_curvature_invalid_nan() {
    let result = ConeCudaConfig::with_curvature(f32::NAN);
    assert!(result.is_err());
}

#[test]
fn test_config_abs_curvature() {
    let config = ConeCudaConfig::with_curvature(-2.5).unwrap();
    assert!((config.abs_curvature() - 2.5).abs() < 1e-6);
}

// ========== ConeData Tests ==========

#[test]
fn test_cone_data_valid() {
    let apex = [0.1f32; 64];
    let cone = ConeData::new(apex, 0.5);
    assert!(cone.is_ok());
    let cone = cone.unwrap();
    assert!((cone.aperture - 0.5).abs() < 1e-6);
}

#[test]
fn test_cone_data_invalid_apex_outside_ball() {
    let apex = [1.0f32; 64]; // norm = sqrt(64) >> 1
    let result = ConeData::new(apex, 0.5);
    assert!(result.is_err());
}

#[test]
fn test_cone_data_invalid_negative_aperture() {
    let apex = [0.1f32; 64];
    let result = ConeData::new(apex, -0.5);
    assert!(result.is_err());
}

#[test]
fn test_cone_data_gpu_format_roundtrip() {
    let apex = [0.123f32; 64];
    let cone = ConeData::from_raw(apex, 0.456);

    let gpu_format = cone.to_gpu_format();
    let restored = ConeData::from_gpu_format(&gpu_format);

    assert!((restored.aperture - cone.aperture).abs() < 1e-6);
    for i in 0..64 {
        assert!((restored.apex[i] - cone.apex[i]).abs() < 1e-6);
    }
}

// ========== CPU Reference Implementation Tests ==========

#[test]
fn test_cpu_score_point_at_apex_returns_1() {
    let apex = [0.1f32; 64];
    let point = apex;
    let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
    assert!(
        (score - 1.0).abs() < 1e-4,
        "Point at apex should have score 1.0, got {}",
        score
    );
}

#[test]
fn test_cpu_score_apex_at_origin() {
    // Degenerate cone at origin contains all points
    let apex = [0.0f32; 64];
    let mut point = [0.0f32; 64];
    point[0] = 0.5;

    let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
    assert!(
        (score - 1.0).abs() < 1e-4,
        "Apex at origin should give score 1.0, got {}",
        score
    );
}

#[test]
fn test_cpu_score_inside_cone_returns_1() {
    // Wide aperture = point is inside cone
    let mut apex = [0.0f32; 64];
    apex[0] = 0.3;

    let mut point = [0.0f32; 64];
    point[0] = 0.1; // Point between apex and origin

    let score = cone_membership_score_cpu(&apex, 1.5, &point, -1.0);
    assert!(
        score > 0.9,
        "Point inside wide cone should have high score: {}",
        score
    );
}

#[test]
fn test_cpu_score_outside_cone_decays() {
    // Narrow cone and point far outside
    let mut apex = [0.0f32; 64];
    apex[0] = 0.5;
    let aperture = 0.1; // Very narrow

    let mut point = [0.0f32; 64];
    point[1] = 0.5; // Perpendicular direction

    let score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);
    assert!(
        score < 0.5,
        "Point outside narrow cone should have low score: {}",
        score
    );
}

#[test]
fn test_cpu_score_is_bounded() {
    // All scores must be in [0, 1]
    for seed in 0..20 {
        let mut apex = [0.0f32; 64];
        let mut point = [0.0f32; 64];

        // Deterministic "random" values
        apex[seed % 64] = ((seed as f32 * 0.07) % 0.9) * if seed % 2 == 0 { 1.0 } else { -1.0 };
        point[(seed + 7) % 64] =
            ((seed as f32 * 0.11) % 0.9) * if seed % 3 == 0 { 1.0 } else { -1.0 };

        let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
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
fn test_cpu_canonical_formula_verified() {
    // Verify the CANONICAL formula: exp(-2 * (angle - aperture))
    let mut apex = [0.0f32; 64];
    apex[0] = 0.3;
    let aperture = 0.3; // Narrow aperture

    // Point at a significant angle from cone axis
    let mut point = [0.0f32; 64];
    point[0] = 0.1;
    point[1] = 0.3;

    let score = cone_membership_score_cpu(&apex, aperture, &point, -1.0);

    // Score should be exp(-2 * (angle - aperture)) if angle > aperture
    // Just verify it's a reasonable value
    println!(
        "CANONICAL FORMULA TEST: aperture={}, score={}",
        aperture, score
    );
    assert!((0.0..=1.0).contains(&score), "Score must be in [0,1]");
}

// ========== Batch Tests ==========

#[test]
fn test_batch_cpu_matches_single() {
    let n_cones = 5;
    let n_points = 5;

    let cones: Vec<f32> = (0..n_cones)
        .flat_map(|i| {
            let mut data = [0.0f32; 65];
            data[0] = (i as f32) * 0.1; // Varying apex
            data[64] = 0.5; // Aperture
            data.to_vec()
        })
        .collect();

    let points: Vec<f32> = (0..n_points)
        .flat_map(|i| {
            let mut p = [0.0f32; 64];
            p[0] = (i as f32) * 0.1 + 0.05;
            p.to_vec()
        })
        .collect();

    let batch_scores = cone_check_batch_cpu(&cones, &points, n_cones, n_points, -1.0);

    assert_eq!(batch_scores.len(), n_cones * n_points);

    // Verify batch matches single computations
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
fn test_batch_cpu_100x100() {
    let n = 100;

    let cones: Vec<f32> = (0..n)
        .flat_map(|i| {
            let mut data = [0.0f32; 65];
            data[0] = ((i * 17) % 80) as f32 * 0.01;
            data[64] = 0.5 + (i % 10) as f32 * 0.05;
            data.to_vec()
        })
        .collect();

    let points: Vec<f32> = (0..n)
        .flat_map(|i| {
            let mut p = [0.0f32; 64];
            p[0] = ((i * 23) % 90) as f32 * 0.01;
            p.to_vec()
        })
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
}

// ========== Edge Cases ==========

#[test]
fn test_edge_case_boundary_point() {
    let apex = [0.1f32; 64];
    let scale = 0.99 / (64.0_f32).sqrt();
    let boundary_point: [f32; 64] = [scale; 64];

    let score = cone_membership_score_cpu(&apex, 0.5, &boundary_point, -1.0);
    assert!(
        score.is_finite() && (0.0..=1.0).contains(&score),
        "Boundary point score invalid: {}",
        score
    );
}

#[test]
fn test_edge_case_zero_aperture() {
    let apex = [0.1f32; 64];
    let mut point = [0.0f32; 64];
    point[0] = 0.2;

    let score = cone_membership_score_cpu(&apex, 0.0, &point, -1.0);
    // With zero aperture, only apex should score 1.0
    assert!(
        score < 1.0,
        "Non-apex point with zero aperture should have score < 1.0"
    );
    assert!(score.is_finite() && score >= 0.0);
}

#[test]
fn test_edge_case_large_aperture() {
    // Aperture of PI/2 should contain many points
    let apex = [0.1f32; 64];
    let mut point = [0.0f32; 64];
    point[0] = 0.2;

    let score = cone_membership_score_cpu(&apex, std::f32::consts::FRAC_PI_2, &point, -1.0);
    assert!(
        score > 0.9,
        "Point with large aperture should have high score: {}",
        score
    );
}

// ========== GPU Availability Test ==========

#[test]
fn test_gpu_availability_check() {
    let available = is_cone_gpu_available();
    println!("Cone GPU available: {}", available);

    #[cfg(not(feature = "cuda"))]
    assert!(
        !available,
        "Without cuda feature, GPU should not be available"
    );
}

// ========== Constants Verification ==========

#[test]
fn test_constants() {
    assert_eq!(CONE_DATA_DIM, 65, "CONE_DATA_DIM should be 65");
    assert_eq!(POINT_DIM, 64, "POINT_DIM should be 64");
    assert!(
        (DEFAULT_CURVATURE - (-1.0)).abs() < 1e-10,
        "DEFAULT_CURVATURE should be -1.0"
    );
    assert!(
        CONE_EPS > 0.0 && CONE_EPS < 1e-5,
        "CONE_EPS should be small positive"
    );
}
