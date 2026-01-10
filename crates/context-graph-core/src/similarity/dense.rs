//! Dense vector similarity functions with optional SIMD acceleration.
//!
//! This module provides core similarity primitives for dense embeddings
//! (E1, E2, E3, E4, E5, E7, E8, E9, E10, E11).
//!
//! # Performance
//!
//! SIMD (AVX2) provides 2-4x speedup on x86_64 for vectors >256 dimensions.
//! Constitution.yaml target: <5ms for pair similarity.
//!
//! # Dense Embedder Dimensions
//!
//! | Embedder | Dimension |
//! |----------|-----------|
//! | E1       | 1024      |
//! | E2       | 512       |
//! | E3       | 512       |
//! | E4       | 512       |
//! | E5       | 768       |
//! | E7       | 1536      |
//! | E8       | 384       |
//! | E9       | 1024      |
//! | E10      | 768       |
//! | E11      | 384       |

use thiserror::Error;

/// Errors from dense vector similarity computation.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum DenseSimilarityError {
    /// Dimension mismatch between vectors.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension (from first vector)
        expected: usize,
        /// Actual dimension (from second vector)
        actual: usize,
    },

    /// Empty vector provided.
    #[error("Empty vector provided")]
    EmptyVector,

    /// Zero magnitude vector - cosine undefined.
    #[error("Zero magnitude vector - cosine undefined")]
    ZeroMagnitude,
}

/// Calculate L2 norm (magnitude) of a vector.
///
/// # Arguments
/// - `v`: The vector to compute norm for
///
/// # Returns
/// The L2 norm (Euclidean length) of the vector.
///
/// # Example
/// ```rust,ignore
/// let v = vec![3.0, 4.0];
/// let norm = l2_norm(&v);
/// assert!((norm - 5.0).abs() < 1e-6);
/// ```
#[inline]
pub fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Normalize a vector to unit length in-place.
///
/// Does nothing if vector has zero magnitude (avoids division by zero).
///
/// # Arguments
/// - `v`: The vector to normalize in-place
///
/// # Example
/// ```rust,ignore
/// let mut v = vec![3.0, 4.0];
/// normalize(&mut v);
/// // v is now [0.6, 0.8]
/// ```
#[inline]
pub fn normalize(v: &mut [f32]) {
    let norm = l2_norm(v);
    if norm > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Internal dot product without validation.
/// Caller must ensure vectors have equal length.
#[inline]
fn dot_product_unchecked(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Calculate dot product between two dense vectors.
///
/// # Arguments
/// - `a`: First vector
/// - `b`: Second vector
///
/// # Returns
/// The dot product of the two vectors.
///
/// # Errors
/// - `DenseSimilarityError::EmptyVector` if either vector is empty
/// - `DenseSimilarityError::DimensionMismatch` if vectors have different lengths
///
/// # Example
/// ```rust,ignore
/// let a = vec![1.0, 2.0, 3.0];
/// let b = vec![4.0, 5.0, 6.0];
/// let dot = dot_product(&a, &b)?; // 32.0
/// ```
#[inline]
pub fn dot_product(a: &[f32], b: &[f32]) -> Result<f32, DenseSimilarityError> {
    if a.is_empty() || b.is_empty() {
        return Err(DenseSimilarityError::EmptyVector);
    }
    if a.len() != b.len() {
        return Err(DenseSimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }
    Ok(dot_product_unchecked(a, b))
}

/// Calculate cosine similarity between two dense vectors.
///
/// Returns value in [-1.0, 1.0] where 1.0 means identical direction,
/// 0.0 means orthogonal, and -1.0 means opposite direction.
///
/// # Arguments
/// - `a`: First vector
/// - `b`: Second vector
///
/// # Returns
/// Cosine similarity clamped to [-1.0, 1.0].
///
/// # Errors
/// - `DenseSimilarityError::EmptyVector` if either vector is empty
/// - `DenseSimilarityError::DimensionMismatch` if vectors have different lengths
/// - `DenseSimilarityError::ZeroMagnitude` if either vector has zero norm
///
/// # Example
/// ```rust,ignore
/// let a = vec![1.0, 0.0];
/// let b = vec![0.0, 1.0];
/// let sim = cosine_similarity(&a, &b)?; // 0.0 (orthogonal)
/// ```
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, DenseSimilarityError> {
    if a.is_empty() || b.is_empty() {
        return Err(DenseSimilarityError::EmptyVector);
    }
    if a.len() != b.len() {
        return Err(DenseSimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }

    let dot = dot_product_unchecked(a, b);
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return Err(DenseSimilarityError::ZeroMagnitude);
    }

    let result = dot / (norm_a * norm_b);
    // Clamp to valid range to handle floating point errors
    Ok(result.clamp(-1.0, 1.0))
}

/// Calculate Euclidean distance between two dense vectors.
///
/// # Arguments
/// - `a`: First vector
/// - `b`: Second vector
///
/// # Returns
/// The Euclidean distance (L2 norm of difference).
///
/// # Errors
/// - `DenseSimilarityError::EmptyVector` if either vector is empty
/// - `DenseSimilarityError::DimensionMismatch` if vectors have different lengths
///
/// # Example
/// ```rust,ignore
/// let a = vec![0.0, 0.0];
/// let b = vec![3.0, 4.0];
/// let dist = euclidean_distance(&a, &b)?; // 5.0
/// ```
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> Result<f32, DenseSimilarityError> {
    if a.is_empty() || b.is_empty() {
        return Err(DenseSimilarityError::EmptyVector);
    }
    if a.len() != b.len() {
        return Err(DenseSimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }
    let sum: f32 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum();
    Ok(sum.sqrt())
}

// ============================================================================
// SIMD IMPLEMENTATION (x86_64 AVX2)
// ============================================================================

/// Minimum vector length for SIMD to be beneficial (overhead vs. gain).
const SIMD_MIN_LENGTH: usize = 32;

/// Calculate cosine similarity using SIMD (AVX2 + FMA) instructions.
///
/// This function provides 2-4x speedup over the scalar implementation
/// for vectors with 256+ dimensions. For small vectors (<32 dims), it
/// falls back to the scalar implementation.
///
/// # Architecture Requirements
/// - x86_64 only
/// - Requires AVX2 and FMA instruction sets (runtime checked)
///
/// # Arguments
/// - `a`: First vector
/// - `b`: Second vector
///
/// # Returns
/// Cosine similarity clamped to [-1.0, 1.0].
///
/// # Errors
/// - `DenseSimilarityError::EmptyVector` if either vector is empty
/// - `DenseSimilarityError::DimensionMismatch` if vectors have different lengths
/// - `DenseSimilarityError::ZeroMagnitude` if either vector has zero norm
///
/// # Example
/// ```rust,ignore
/// #[cfg(target_arch = "x86_64")]
/// {
///     let a: Vec<f32> = (0..1024).map(|i| i as f32 * 0.001).collect();
///     let b: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.001).sin()).collect();
///     let sim = cosine_similarity_simd(&a, &b)?;
/// }
/// ```
#[cfg(target_arch = "x86_64")]
pub fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> Result<f32, DenseSimilarityError> {
    use std::arch::x86_64::*;

    if a.is_empty() || b.is_empty() {
        return Err(DenseSimilarityError::EmptyVector);
    }
    if a.len() != b.len() {
        return Err(DenseSimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }

    // For small vectors, use scalar (SIMD overhead not worth it)
    if a.len() < SIMD_MIN_LENGTH {
        return cosine_similarity(a, b);
    }

    // Check AVX2 support at runtime
    if !is_x86_feature_detected!("avx2") || !is_x86_feature_detected!("fma") {
        return cosine_similarity(a, b);
    }

    // SAFETY: We have verified AVX2 and FMA support above, and we ensure
    // proper bounds checking throughout the implementation.
    unsafe {
        let mut dot_sum = _mm256_setzero_ps();
        let mut norm_a_sum = _mm256_setzero_ps();
        let mut norm_b_sum = _mm256_setzero_ps();

        let chunks = a.len() / 8;
        for i in 0..chunks {
            let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));

            // FMA: dot_sum += va * vb
            dot_sum = _mm256_fmadd_ps(va, vb, dot_sum);
            // FMA: norm_a_sum += va * va
            norm_a_sum = _mm256_fmadd_ps(va, va, norm_a_sum);
            // FMA: norm_b_sum += vb * vb
            norm_b_sum = _mm256_fmadd_ps(vb, vb, norm_b_sum);
        }

        // Horizontal sum: reduce 8 lanes to 1
        let dot = hsum_avx(dot_sum);
        let norm_a_sq = hsum_avx(norm_a_sum);
        let norm_b_sq = hsum_avx(norm_b_sum);

        // Handle remainder with scalar code
        let remainder_start = chunks * 8;
        let mut dot_rem = 0.0f32;
        let mut norm_a_rem = 0.0f32;
        let mut norm_b_rem = 0.0f32;
        for i in remainder_start..a.len() {
            dot_rem += a[i] * b[i];
            norm_a_rem += a[i] * a[i];
            norm_b_rem += b[i] * b[i];
        }

        let total_dot = dot + dot_rem;
        let total_norm_a = (norm_a_sq + norm_a_rem).sqrt();
        let total_norm_b = (norm_b_sq + norm_b_rem).sqrt();

        if total_norm_a < f32::EPSILON || total_norm_b < f32::EPSILON {
            return Err(DenseSimilarityError::ZeroMagnitude);
        }

        let result = total_dot / (total_norm_a * total_norm_b);
        Ok(result.clamp(-1.0, 1.0))
    }
}

/// Horizontal sum of 8 f32 lanes in AVX register.
///
/// # Safety
/// Caller must ensure AVX2 is available.
#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn hsum_avx(v: std::arch::x86_64::__m256) -> f32 {
    use std::arch::x86_64::*;
    // Sum pairs: [a+b, c+d, e+f, g+h, a+b, c+d, e+f, g+h]
    let sum1 = _mm256_hadd_ps(v, v);
    // Sum pairs again: [a+b+c+d, e+f+g+h, a+b+c+d, e+f+g+h, ...]
    let sum2 = _mm256_hadd_ps(sum1, sum1);
    // Extract low and high 128-bit lanes and add
    let low = _mm256_extractf128_ps(sum2, 0);
    let high = _mm256_extractf128_ps(sum2, 1);
    let sum3 = _mm_add_ps(low, high);
    _mm_cvtss_f32(sum3)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let sim = cosine_similarity(&v, &v).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0, got {}",
            sim
        );
        println!(
            "[PASS] Cosine of identical vectors = 1.0: actual = {:.6}",
            sim
        );
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            sim.abs() < 1e-6,
            "Orthogonal vectors should have similarity 0.0, got {}",
            sim
        );
        println!(
            "[PASS] Cosine of orthogonal vectors = 0.0: actual = {:.6}",
            sim
        );
    }

    #[test]
    fn test_cosine_opposite_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim + 1.0).abs() < 1e-6,
            "Opposite vectors should have similarity -1.0, got {}",
            sim
        );
        println!(
            "[PASS] Cosine of opposite vectors = -1.0: actual = {:.6}",
            sim
        );
    }

    #[test]
    fn test_dimension_mismatch_error() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = cosine_similarity(&a, &b);
        assert!(matches!(
            result,
            Err(DenseSimilarityError::DimensionMismatch {
                expected: 2,
                actual: 3
            })
        ));
        println!(
            "[PASS] Dimension mismatch correctly detected: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_empty_vector_error() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 2.0];
        let result = cosine_similarity(&a, &b);
        assert!(matches!(result, Err(DenseSimilarityError::EmptyVector)));
        println!(
            "[PASS] Empty vector correctly detected: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_zero_magnitude_error() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = cosine_similarity(&a, &b);
        assert!(matches!(result, Err(DenseSimilarityError::ZeroMagnitude)));
        println!(
            "[PASS] Zero magnitude correctly detected: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dot = dot_product(&a, &b).unwrap();
        let expected = 1.0 * 4.0 + 2.0 * 5.0 + 3.0 * 6.0; // 32.0
        assert!((dot - expected).abs() < 1e-6);
        println!("[PASS] Dot product = {}, expected = {}", dot, expected);
    }

    #[test]
    fn test_dot_product_empty_vector() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 2.0];
        let result = dot_product(&a, &b);
        assert!(matches!(result, Err(DenseSimilarityError::EmptyVector)));
        println!(
            "[PASS] Dot product empty vector error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_dot_product_dimension_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = dot_product(&a, &b);
        assert!(matches!(
            result,
            Err(DenseSimilarityError::DimensionMismatch { .. })
        ));
        println!(
            "[PASS] Dot product dimension mismatch error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let dist = euclidean_distance(&a, &b).unwrap();
        assert!(
            (dist - 5.0).abs() < 1e-6,
            "Expected distance 5.0, got {}",
            dist
        );
        println!("[PASS] Euclidean distance = {}, expected = 5.0", dist);
    }

    #[test]
    fn test_euclidean_distance_same_point() {
        let a = vec![1.0, 2.0, 3.0];
        let dist = euclidean_distance(&a, &a).unwrap();
        assert!(
            dist.abs() < 1e-6,
            "Distance to self should be 0.0, got {}",
            dist
        );
        println!("[PASS] Euclidean distance to self = {}", dist);
    }

    #[test]
    fn test_euclidean_distance_empty_vector() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 2.0];
        let result = euclidean_distance(&a, &b);
        assert!(matches!(result, Err(DenseSimilarityError::EmptyVector)));
        println!(
            "[PASS] Euclidean distance empty vector error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_euclidean_distance_dimension_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = euclidean_distance(&a, &b);
        assert!(matches!(
            result,
            Err(DenseSimilarityError::DimensionMismatch { .. })
        ));
        println!(
            "[PASS] Euclidean distance dimension mismatch error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_l2_norm() {
        let v = vec![3.0, 4.0];
        let norm = l2_norm(&v);
        assert!((norm - 5.0).abs() < 1e-6);
        println!("[PASS] L2 norm of [3,4] = {}, expected = 5.0", norm);
    }

    #[test]
    fn test_l2_norm_zero_vector() {
        let v = vec![0.0, 0.0, 0.0];
        let norm = l2_norm(&v);
        assert!(norm.abs() < 1e-6);
        println!("[PASS] L2 norm of zero vector = {}", norm);
    }

    #[test]
    fn test_l2_norm_single_element() {
        let v = vec![5.0];
        let norm = l2_norm(&v);
        assert!((norm - 5.0).abs() < 1e-6);
        println!("[PASS] L2 norm of [5] = {}", norm);
    }

    #[test]
    fn test_normalize() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        let norm = l2_norm(&v);
        assert!(
            (norm - 1.0).abs() < 1e-6,
            "Normalized vector should have norm 1.0, got {}",
            norm
        );
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
        println!("[PASS] Normalized [3,4] = [{:.3}, {:.3}]", v[0], v[1]);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let mut v = vec![0.0, 0.0, 0.0];
        let original = v.clone();
        normalize(&mut v);
        // Zero vector should remain unchanged
        assert_eq!(v, original);
        println!("[PASS] Normalize zero vector unchanged");
    }

    #[test]
    fn test_normalize_already_normalized() {
        let mut v = vec![1.0, 0.0];
        normalize(&mut v);
        assert!((v[0] - 1.0).abs() < 1e-6);
        assert!(v[1].abs() < 1e-6);
        println!("[PASS] Already normalized vector unchanged: [{:.3}, {:.3}]", v[0], v[1]);
    }

    #[test]
    fn test_high_dimensional_1024() {
        // Simulate E1_DIM = 1024
        let a: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.001).collect();
        let b: Vec<f32> = (0..1024).map(|i| ((i as f32) * 0.001).sin()).collect();
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (-1.0..=1.0).contains(&sim),
            "Similarity out of range: {}",
            sim
        );
        assert!(!sim.is_nan() && !sim.is_infinite());
        println!("[PASS] 1024D cosine similarity = {:.6}", sim);
    }

    #[test]
    fn test_high_dimensional_512() {
        // Simulate E2/E3/E4_DIM = 512
        let a: Vec<f32> = (0..512).map(|i| (i as f32) * 0.002).collect();
        let b: Vec<f32> = (0..512).map(|i| ((i as f32) * 0.002).cos()).collect();
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((-1.0..=1.0).contains(&sim));
        assert!(!sim.is_nan() && !sim.is_infinite());
        println!("[PASS] 512D cosine similarity = {:.6}", sim);
    }

    #[test]
    fn test_high_dimensional_768() {
        // Simulate E5/E10_DIM = 768
        let a: Vec<f32> = (0..768).map(|i| (i as f32) * 0.001 + 0.1).collect();
        let b: Vec<f32> = (0..768).map(|i| ((i as f32) * 0.001).tan().clamp(-10.0, 10.0)).collect();
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((-1.0..=1.0).contains(&sim));
        assert!(!sim.is_nan() && !sim.is_infinite());
        println!("[PASS] 768D cosine similarity = {:.6}", sim);
    }

    #[test]
    fn test_high_dimensional_1536() {
        // Simulate E7_DIM = 1536
        let a: Vec<f32> = (0..1536).map(|i| (i as f32) * 0.0005).collect();
        let b: Vec<f32> = (0..1536).map(|i| ((i as f32) * 0.0005).exp().min(10.0)).collect();
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((-1.0..=1.0).contains(&sim));
        assert!(!sim.is_nan() && !sim.is_infinite());
        println!("[PASS] 1536D cosine similarity = {:.6}", sim);
    }

    #[test]
    fn test_high_dimensional_384() {
        // Simulate E8/E11_DIM = 384
        let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.003).collect();
        let b: Vec<f32> = (0..384).map(|i| ((i as f32) * 0.003).sin()).collect();
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((-1.0..=1.0).contains(&sim));
        assert!(!sim.is_nan() && !sim.is_infinite());
        println!("[PASS] 384D cosine similarity = {:.6}", sim);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_matches_scalar() {
        // Test with E1 dimensions (1024)
        let a: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.001) + 0.5).collect();
        let b: Vec<f32> = (0..1024).map(|i| ((i as f32) * 0.002).sin()).collect();

        let scalar = cosine_similarity(&a, &b).unwrap();
        let simd = cosine_similarity_simd(&a, &b).unwrap();

        let diff = (scalar - simd).abs();
        assert!(
            diff < 1e-5,
            "SIMD result differs from scalar by {}: scalar={}, simd={}",
            diff,
            scalar,
            simd
        );
        println!(
            "[PASS] SIMD matches scalar within 1e-5: scalar={:.6}, simd={:.6}, diff={:.9}",
            scalar, simd, diff
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_with_different_dimensions() {
        // Test all dense embedder dimensions
        let dimensions = [1024, 512, 768, 1536, 384]; // E1, E2/E3/E4, E5/E10, E7, E8/E11

        for dim in dimensions {
            let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001) + 0.5).collect();
            let b: Vec<f32> = (0..dim).map(|i| ((i as f32) * 0.002).cos()).collect();

            let scalar = cosine_similarity(&a, &b).unwrap();
            let simd = cosine_similarity_simd(&a, &b).unwrap();

            let diff = (scalar - simd).abs();
            assert!(diff < 1e-5, "SIMD differs at dim={}: diff={}", dim, diff);
            println!(
                "[PASS] dim={}: scalar={:.6}, simd={:.6}",
                dim, scalar, simd
            );
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_small_vector_fallback() {
        // Small vectors should use scalar fallback
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![5.0, 4.0, 3.0, 2.0, 1.0];

        let scalar = cosine_similarity(&a, &b).unwrap();
        let simd = cosine_similarity_simd(&a, &b).unwrap();

        // Should be identical since SIMD falls back to scalar for small vectors
        let diff = (scalar - simd).abs();
        assert!(
            diff < 1e-10,
            "Small vector SIMD should match scalar exactly: diff={}",
            diff
        );
        println!(
            "[PASS] Small vector fallback: scalar={:.6}, simd={:.6}",
            scalar, simd
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_remainder_handling() {
        // Test vector with non-multiple-of-8 length
        let dim = 1000; // Not divisible by 8
        let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001) + 0.5).collect();
        let b: Vec<f32> = (0..dim).map(|i| ((i as f32) * 0.002).sin()).collect();

        let scalar = cosine_similarity(&a, &b).unwrap();
        let simd = cosine_similarity_simd(&a, &b).unwrap();

        let diff = (scalar - simd).abs();
        assert!(
            diff < 1e-5,
            "SIMD remainder handling failed: diff={}",
            diff
        );
        println!(
            "[PASS] Remainder handling (dim={}): scalar={:.6}, simd={:.6}",
            dim, scalar, simd
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_identical_vectors() {
        let v: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.001).collect();
        let sim = cosine_similarity_simd(&v, &v).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "SIMD identical vectors should have similarity 1.0, got {}",
            sim
        );
        println!("[PASS] SIMD identical vectors = {:.6}", sim);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_orthogonal_vectors() {
        // Create orthogonal vectors in high dimensions
        let mut a: Vec<f32> = vec![0.0; 128];
        let mut b: Vec<f32> = vec![0.0; 128];
        // First 64 dimensions for a, last 64 for b
        for i in 0..64 {
            a[i] = 1.0;
            b[64 + i] = 1.0;
        }

        let sim = cosine_similarity_simd(&a, &b).unwrap();
        assert!(
            sim.abs() < 1e-5,
            "SIMD orthogonal vectors should have similarity ~0.0, got {}",
            sim
        );
        println!("[PASS] SIMD orthogonal vectors = {:.6}", sim);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_opposite_vectors() {
        let a: Vec<f32> = (0..256).map(|i| (i as f32) * 0.01).collect();
        let b: Vec<f32> = a.iter().map(|x| -x).collect();

        let sim = cosine_similarity_simd(&a, &b).unwrap();
        assert!(
            (sim + 1.0).abs() < 1e-5,
            "SIMD opposite vectors should have similarity -1.0, got {}",
            sim
        );
        println!("[PASS] SIMD opposite vectors = {:.6}", sim);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_dimension_mismatch() {
        let a: Vec<f32> = vec![1.0; 256];
        let b: Vec<f32> = vec![1.0; 512];
        let result = cosine_similarity_simd(&a, &b);
        assert!(matches!(
            result,
            Err(DenseSimilarityError::DimensionMismatch {
                expected: 256,
                actual: 512
            })
        ));
        println!(
            "[PASS] SIMD dimension mismatch detected: {:?}",
            result.err()
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_empty_vector() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![1.0; 256];
        let result = cosine_similarity_simd(&a, &b);
        assert!(matches!(result, Err(DenseSimilarityError::EmptyVector)));
        println!("[PASS] SIMD empty vector detected: {:?}", result.err());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_zero_magnitude() {
        let a: Vec<f32> = vec![0.0; 256];
        let b: Vec<f32> = (0..256).map(|i| i as f32 * 0.01).collect();
        let result = cosine_similarity_simd(&a, &b);
        assert!(matches!(result, Err(DenseSimilarityError::ZeroMagnitude)));
        println!("[PASS] SIMD zero magnitude detected: {:?}", result.err());
    }

    // Edge case tests
    #[test]
    fn test_edge_case_near_max_values() {
        // Test with large values (scaled to avoid overflow)
        let scale = f32::MAX.sqrt() / 100.0;
        let a: Vec<f32> = vec![scale; 100];
        let b: Vec<f32> = vec![scale; 100];

        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-3,
            "Near-max values should give similarity ~1.0, got {}",
            sim
        );
        assert!(!sim.is_nan() && !sim.is_infinite());
        println!("[PASS] Near-max values: similarity = {:.6}", sim);
    }

    #[test]
    fn test_edge_case_near_min_values() {
        // Test with very small values
        let a: Vec<f32> = vec![f32::EPSILON * 10.0; 100];
        let b: Vec<f32> = vec![f32::EPSILON * 10.0; 100];

        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-3,
            "Near-min values should give similarity ~1.0, got {}",
            sim
        );
        assert!(!sim.is_nan() && !sim.is_infinite());
        println!("[PASS] Near-min values: similarity = {:.6}", sim);
    }

    #[test]
    fn test_edge_case_mixed_signs() {
        let a: Vec<f32> = vec![1.0, -1.0, 1.0, -1.0, 1.0];
        let b: Vec<f32> = vec![1.0, 1.0, 1.0, 1.0, 1.0];

        let sim = cosine_similarity(&a, &b).unwrap();
        // Expected: (1-1+1-1+1) / (sqrt(5) * sqrt(5)) = 1/5 = 0.2
        assert!(
            (sim - 0.2).abs() < 1e-6,
            "Mixed signs: expected 0.2, got {}",
            sim
        );
        println!("[PASS] Mixed signs: similarity = {:.6}", sim);
    }

    #[test]
    fn test_edge_case_single_nonzero() {
        let a: Vec<f32> = vec![0.0, 0.0, 1.0, 0.0, 0.0];
        let b: Vec<f32> = vec![0.0, 0.0, 1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Single nonzero: expected 1.0, got {}",
            sim
        );
        println!("[PASS] Single nonzero: similarity = {:.6}", sim);
    }
}
