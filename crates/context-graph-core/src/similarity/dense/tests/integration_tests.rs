//! Integration and edge case tests for dense vector similarity functions.
//! Includes high-dimensional tests and numerical edge cases.

use super::super::*;

// =============================================================================
// High-Dimensional Tests (Simulating Real Embedding Dimensions)
// =============================================================================

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
    let b: Vec<f32> = (0..768)
        .map(|i| ((i as f32) * 0.001).tan().clamp(-10.0, 10.0))
        .collect();
    let sim = cosine_similarity(&a, &b).unwrap();
    assert!((-1.0..=1.0).contains(&sim));
    assert!(!sim.is_nan() && !sim.is_infinite());
    println!("[PASS] 768D cosine similarity = {:.6}", sim);
}

#[test]
fn test_high_dimensional_1536() {
    // Simulate E7_DIM = 1536
    let a: Vec<f32> = (0..1536).map(|i| (i as f32) * 0.0005).collect();
    let b: Vec<f32> = (0..1536)
        .map(|i| ((i as f32) * 0.0005).exp().min(10.0))
        .collect();
    let sim = cosine_similarity(&a, &b).unwrap();
    assert!((-1.0..=1.0).contains(&sim));
    assert!(!sim.is_nan() && !sim.is_infinite());
    println!("[PASS] 1536D cosine similarity = {:.6}", sim);
}

#[test]
fn test_high_dimensional_384() {
    // Test 384D vectors (generic dimension test, not tied to any specific embedder)
    let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.003).collect();
    let b: Vec<f32> = (0..384).map(|i| ((i as f32) * 0.003).sin()).collect();
    let sim = cosine_similarity(&a, &b).unwrap();
    assert!((-1.0..=1.0).contains(&sim));
    assert!(!sim.is_nan() && !sim.is_infinite());
    println!("[PASS] 384D cosine similarity = {:.6}", sim);
}

// =============================================================================
// Numerical Edge Case Tests
// =============================================================================

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
