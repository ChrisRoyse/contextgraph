//! CPU reference implementation for cone operations.
//!
//! Provides CPU fallback and reference implementations for testing GPU kernel correctness.
//!
//! # CANONICAL Membership Score Formula
//!
//! ```text
//! - If angle <= aperture: score = 1.0
//! - If angle > aperture: score = exp(-2.0 * (angle - aperture))
//! ```
//!
//! # Angle Computation Algorithm
//!
//! ```text
//! 1. tangent = log_map(apex, point) - direction to point in tangent space
//! 2. to_origin = log_map(apex, origin) - cone axis direction (toward origin)
//! 3. cos_angle = dot(tangent, to_origin) / (||tangent|| * ||to_origin||)
//! 4. angle = acos(cos_angle.clamp(-1.0, 1.0))
//!
//! Edge cases that return angle = 0.0 (score = 1.0):
//! - Point at apex (distance < eps)
//! - Apex at origin (norm < eps)
//! - Zero-length tangent or to_origin vectors
//! ```

use super::constants::{CONE_DATA_DIM, CONE_EPS, POINT_DIM};

/// Compute Mobius addition on CPU.
///
/// Formula: (x (+) y) = ((1 + 2c<x,y> + c||y||^2)x + (1 - c||x||^2)y) /
///                      (1 + 2c<x,y> + c^2||x||^2||y||^2)
fn mobius_add_cpu(x: &[f32; 64], y: &[f32; 64], c: f32) -> [f32; 64] {
    let x_norm_sq: f32 = x.iter().map(|v| v * v).sum();
    let y_norm_sq: f32 = y.iter().map(|v| v * v).sum();
    let xy_dot: f32 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();

    let num_coeff_x = 1.0 + 2.0 * c * xy_dot + c * y_norm_sq;
    let num_coeff_y = 1.0 - c * x_norm_sq;
    let denom = 1.0 + 2.0 * c * xy_dot + c * c * x_norm_sq * y_norm_sq;

    let safe_denom = if denom.abs() < CONE_EPS {
        if denom < 0.0 {
            -CONE_EPS
        } else {
            CONE_EPS
        }
    } else {
        denom
    };

    let mut result = [0.0f32; 64];
    for i in 0..64 {
        result[i] = (num_coeff_x * x[i] + num_coeff_y * y[i]) / safe_denom;
    }
    result
}

/// Compute log map on CPU: log_x(y) - tangent vector at x pointing toward y.
///
/// Formula: log_x(y) = (2 / (lambda_x * sqrt(c))) * arctanh(sqrt(c) * ||(-x) + y||) * ((-x) + y) / ||(-x) + y||
fn log_map_cpu(x: &[f32; 64], y: &[f32; 64], c: f32) -> [f32; 64] {
    let sqrt_c = c.sqrt();

    // Compute (-x) for Mobius subtraction
    let mut neg_x = [0.0f32; 64];
    for i in 0..64 {
        neg_x[i] = -x[i];
    }

    // Compute diff = (-x) + y (Mobius addition)
    let diff = mobius_add_cpu(&neg_x, y, c);

    // Compute ||diff||
    let diff_norm_sq: f32 = diff.iter().map(|v| v * v).sum();
    let diff_norm = diff_norm_sq.max(0.0).sqrt();

    // Handle identical points
    if diff_norm < CONE_EPS {
        return [0.0f32; 64];
    }

    // Conformal factor at x: lambda_x = 2 / (1 - c||x||^2)
    let x_norm_sq: f32 = x.iter().map(|v| v * v).sum();
    let denom_lambda = (1.0 - c * x_norm_sq).max(CONE_EPS);
    let lambda_x = 2.0 / denom_lambda;

    // arctanh(sqrt(c) * ||(-x) + y||), clamped
    let arg = (sqrt_c * diff_norm).min(1.0 - CONE_EPS);
    let arctanh_val = arg.atanh();

    // Scale factor
    let scale = (2.0 / (lambda_x * sqrt_c)) * arctanh_val / diff_norm;

    let mut tangent = [0.0f32; 64];
    for i in 0..64 {
        tangent[i] = scale * diff[i];
    }
    tangent
}

/// Compute single cone membership score on CPU.
///
/// CANONICAL FORMULA:
/// - If angle <= aperture: 1.0
/// - If angle > aperture: exp(-2.0 * (angle - aperture))
///
/// # Arguments
///
/// * `apex` - Cone apex (64-element array)
/// * `aperture` - Effective aperture in radians
/// * `point` - Point to test (64-element array)
/// * `curvature` - Poincare ball curvature (must be negative)
///
/// # Returns
///
/// Membership score in [0, 1].
///
/// # Example
///
/// ```
/// use context_graph_cuda::cone::cone_membership_score_cpu;
///
/// let apex = [0.1f32; 64];
/// let point = apex.clone();  // Same as apex
/// let score = cone_membership_score_cpu(&apex, 0.5, &point, -1.0);
/// assert!((score - 1.0).abs() < 1e-4);  // Point at apex = score 1.0
/// ```
pub fn cone_membership_score_cpu(
    apex: &[f32; 64],
    aperture: f32,
    point: &[f32; 64],
    curvature: f32,
) -> f32 {
    let c = curvature.abs();

    // Edge case: apex at origin (degenerate cone contains all)
    let apex_norm_sq: f32 = apex.iter().map(|x| x * x).sum();
    if apex_norm_sq < CONE_EPS * CONE_EPS {
        return 1.0;
    }

    // Edge case: point at apex
    let diff_sq: f32 = apex
        .iter()
        .zip(point.iter())
        .map(|(a, p)| (a - p) * (a - p))
        .sum();
    if diff_sq < CONE_EPS * CONE_EPS {
        return 1.0;
    }

    // Compute tangent from apex to point
    let tangent = log_map_cpu(apex, point, c);

    // Compute tangent from apex to origin (cone axis)
    let origin = [0.0f32; 64];
    let to_origin = log_map_cpu(apex, &origin, c);

    // Compute norms
    let tangent_norm_sq: f32 = tangent.iter().map(|x| x * x).sum();
    let to_origin_norm_sq: f32 = to_origin.iter().map(|x| x * x).sum();

    let tangent_norm = tangent_norm_sq.sqrt();
    let to_origin_norm = to_origin_norm_sq.sqrt();

    // Edge case: degenerate tangent vectors
    if tangent_norm < CONE_EPS || to_origin_norm < CONE_EPS {
        return 1.0;
    }

    // Compute angle via dot product
    let dot: f32 = tangent
        .iter()
        .zip(to_origin.iter())
        .map(|(a, b)| a * b)
        .sum();
    let cos_angle = (dot / (tangent_norm * to_origin_norm)).clamp(-1.0, 1.0);
    let angle = cos_angle.acos();

    // CANONICAL FORMULA
    if angle <= aperture {
        1.0
    } else {
        (-2.0 * (angle - aperture)).exp()
    }
}

/// Compute batch cone membership scores on CPU.
///
/// Reference implementation for testing GPU kernel correctness.
///
/// # Arguments
///
/// * `cones` - Cone data, flattened \[n_cones * 65\]
/// * `points` - Point vectors, flattened \[n_points * 64\]
/// * `n_cones` - Number of cones
/// * `n_points` - Number of points
/// * `curvature` - Poincare ball curvature (must be negative)
///
/// # Returns
///
/// Score matrix, flattened \[n_cones * n_points\], row-major.
///
/// # Panics
///
/// Panics if input arrays have incorrect sizes.
pub fn cone_check_batch_cpu(
    cones: &[f32],
    points: &[f32],
    n_cones: usize,
    n_points: usize,
    curvature: f32,
) -> Vec<f32> {
    assert_eq!(
        cones.len(),
        n_cones * CONE_DATA_DIM,
        "Invalid cones size: expected {}, got {}",
        n_cones * CONE_DATA_DIM,
        cones.len()
    );
    assert_eq!(
        points.len(),
        n_points * POINT_DIM,
        "Invalid points size: expected {}, got {}",
        n_points * POINT_DIM,
        points.len()
    );

    let mut scores = vec![0.0f32; n_cones * n_points];

    for i in 0..n_cones {
        let cone_start = i * CONE_DATA_DIM;
        let apex: &[f32; 64] = cones[cone_start..cone_start + 64].try_into().unwrap();
        let aperture = cones[cone_start + 64];

        for j in 0..n_points {
            let pt_start = j * POINT_DIM;
            let point: &[f32; 64] = points[pt_start..pt_start + 64].try_into().unwrap();

            scores[i * n_points + j] = cone_membership_score_cpu(apex, aperture, point, curvature);
        }
    }

    scores
}
