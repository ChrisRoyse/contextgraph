//! SIMD-specific tests for dense vector similarity functions.
//! These tests are only compiled on x86_64 architecture.

use super::super::*;

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
    let dimensions = [1024, 512, 768, 1536]; // E1/E8/E9, E2/E3/E4, E5/E10/E11, E7

    for dim in dimensions {
        let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001) + 0.5).collect();
        let b: Vec<f32> = (0..dim).map(|i| ((i as f32) * 0.002).cos()).collect();

        let scalar = cosine_similarity(&a, &b).unwrap();
        let simd = cosine_similarity_simd(&a, &b).unwrap();

        let diff = (scalar - simd).abs();
        assert!(diff < 1e-5, "SIMD differs at dim={}: diff={}", dim, diff);
        println!("[PASS] dim={}: scalar={:.6}, simd={:.6}", dim, scalar, simd);
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
    assert!(diff < 1e-5, "SIMD remainder handling failed: diff={}", diff);
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
