---
id: "M05-T09"
title: "Implement KL Divergence Computation"
description: |
  Implement kl_divergence(p, q, epsilon) function for distribution comparison.
  Formula: KL(P || Q) = sum_i P(i) * log(P(i) / Q(i))
  Include softmax_normalize(values, temperature) for probability distribution.
  Include cosine_similarity(a, b) for embedding comparison.
  Performance target: <1ms for 1536-dimensional vectors.
layer: "logic"
status: "pending"
priority: "critical"
estimated_hours: 2
sequence: 9
depends_on:
  - "M05-T02"
spec_refs:
  - "TECH-UTL-005 Section 5"
  - "SPEC-UTL-005 Section 3.2.1"
files_to_create:
  - path: "crates/context-graph-utl/src/surprise/kl_divergence.rs"
    description: "KL divergence, softmax normalization, and cosine similarity functions"
files_to_modify:
  - path: "crates/context-graph-utl/src/surprise/mod.rs"
    description: "Add kl_divergence module and re-exports"
  - path: "crates/context-graph-utl/src/lib.rs"
    description: "Re-export surprise module"
test_file: "crates/context-graph-utl/tests/surprise_tests.rs"
---

## Overview

The KL divergence (Kullback-Leibler divergence) is a fundamental information-theoretic measure used in the UTL surprise computation. It quantifies how one probability distribution differs from a reference distribution, which is crucial for detecting novel/surprising information.

## Mathematical Foundation

### KL Divergence

The Kullback-Leibler divergence from Q to P is:

```
KL(P || Q) = sum_i P(i) * log(P(i) / Q(i))
```

Properties:
- Non-negative: KL(P || Q) >= 0
- KL(P || Q) = 0 if and only if P = Q
- Asymmetric: KL(P || Q) != KL(Q || P) in general

### Softmax Normalization

Converts arbitrary values to a probability distribution:

```
softmax(x_i) = exp(x_i / temperature) / sum_j exp(x_j / temperature)
```

### Cosine Similarity

Measures angular similarity between vectors:

```
cosine_sim(a, b) = (a . b) / (||a|| * ||b||)
```

Range: [-1, 1] where 1 = identical, 0 = orthogonal, -1 = opposite

## Implementation Requirements

### File: `crates/context-graph-utl/src/surprise/kl_divergence.rs`

```rust
//! KL divergence and related mathematical functions for surprise computation.
//!
//! # Overview
//!
//! This module provides core mathematical functions for computing surprise:
//!
//! - `kl_divergence`: Kullback-Leibler divergence between distributions
//! - `softmax_normalize`: Convert values to probability distribution
//! - `cosine_similarity`: Angular similarity between vectors
//! - `cosine_distance`: Distance metric derived from cosine similarity
//!
//! # Performance
//!
//! All functions are optimized for high-dimensional vectors:
//! - <1ms for 1536-dimensional vectors
//! - SIMD-friendly sequential operations
//!
//! # Constitution Reference
//!
//! - TECH-UTL-005 Section 5: Surprise computation
//! - SPEC-UTL-005 Section 3.2.1: KL divergence formula

use crate::config::KlConfig;

/// Default epsilon for numerical stability.
pub const DEFAULT_EPSILON: f32 = 1e-10;

/// Default temperature for softmax normalization.
pub const DEFAULT_TEMPERATURE: f32 = 1.0;

/// Compute KL divergence between two probability distributions.
///
/// Formula: KL(P || Q) = sum_i P(i) * log(P(i) / Q(i))
///
/// # Arguments
///
/// * `p` - First distribution (typically observed)
/// * `q` - Second distribution (typically expected/reference)
/// * `epsilon` - Small value for numerical stability
///
/// # Returns
///
/// Non-negative KL divergence value. Returns 0 for identical distributions.
///
/// # Properties
///
/// - KL(P || Q) >= 0 for all P, Q
/// - KL(P || Q) = 0 iff P = Q
/// - Asymmetric: KL(P || Q) != KL(Q || P) in general
///
/// # Numerical Stability
///
/// The function clamps values to avoid log(0) using epsilon parameter.
///
/// # Performance
///
/// - O(n) time complexity
/// - <1ms for 1536-dimensional vectors
/// - SIMD-friendly sequential iteration
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::kl_divergence;
///
/// let p = vec![0.5, 0.5];
/// let q = vec![0.5, 0.5];
/// let kl = kl_divergence(&p, &q, 1e-10);
/// assert!(kl.abs() < 1e-6); // Identical distributions have KL = 0
/// ```
pub fn kl_divergence(p: &[f32], q: &[f32], epsilon: f32) -> f32 {
    debug_assert_eq!(p.len(), q.len(), "Distributions must have same length");

    if p.len() != q.len() {
        return f32::NAN;
    }

    if p.is_empty() {
        return 0.0;
    }

    let mut divergence = 0.0f32;

    for (p_i, q_i) in p.iter().zip(q.iter()) {
        // Skip if p_i is effectively zero (no contribution)
        if *p_i > epsilon {
            // Clamp q_i to avoid log(0)
            let q_safe = q_i.max(epsilon);
            divergence += p_i * (p_i / q_safe).ln();
        }
    }

    // Ensure non-negative (handle floating-point errors)
    divergence.max(0.0)
}

/// Compute KL divergence using configuration settings.
///
/// # Arguments
///
/// * `p` - First distribution
/// * `q` - Second distribution
/// * `config` - KL computation configuration
///
/// # Returns
///
/// KL divergence clamped to [0, max_kl_value].
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::kl_divergence_with_config;
/// use context_graph_utl::config::KlConfig;
///
/// let config = KlConfig::default();
/// let p = vec![0.7, 0.3];
/// let q = vec![0.3, 0.7];
/// let kl = kl_divergence_with_config(&p, &q, &config);
/// assert!(kl <= config.max_kl_value);
/// ```
pub fn kl_divergence_with_config(p: &[f32], q: &[f32], config: &KlConfig) -> f32 {
    let raw_kl = kl_divergence(p, q, config.epsilon);
    raw_kl.min(config.max_kl_value)
}

/// Normalize KL divergence to [0, 1] range.
///
/// # Arguments
///
/// * `kl` - Raw KL divergence value
/// * `max_kl` - Maximum KL value for normalization
///
/// # Returns
///
/// Normalized value in [0, 1].
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::normalize_kl;
///
/// let normalized = normalize_kl(5.0, 10.0);
/// assert_eq!(normalized, 0.5);
/// ```
#[inline]
pub fn normalize_kl(kl: f32, max_kl: f32) -> f32 {
    if max_kl <= 0.0 {
        return 0.0;
    }
    (kl / max_kl).clamp(0.0, 1.0)
}

/// Apply softmax normalization to convert values to probability distribution.
///
/// Formula: softmax(x_i) = exp(x_i / T) / sum_j exp(x_j / T)
///
/// # Arguments
///
/// * `values` - Input values (can be any real numbers)
/// * `temperature` - Temperature parameter (higher = more uniform, lower = more peaked)
///
/// # Returns
///
/// Probability distribution summing to 1.0.
///
/// # Numerical Stability
///
/// Uses log-sum-exp trick to avoid overflow:
/// 1. Subtract max value before exp
/// 2. Compute normalized probabilities
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::softmax_normalize;
///
/// let values = vec![1.0, 2.0, 3.0];
/// let probs = softmax_normalize(&values, 1.0);
/// let sum: f32 = probs.iter().sum();
/// assert!((sum - 1.0).abs() < 1e-6);
/// ```
pub fn softmax_normalize(values: &[f32], temperature: f32) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }

    let temp = if temperature.abs() < 1e-10 {
        1e-10 // Avoid division by zero
    } else {
        temperature
    };

    // Find max for numerical stability (log-sum-exp trick)
    let max_val = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    // Compute exp((x - max) / T) for stability
    let exp_values: Vec<f32> = values
        .iter()
        .map(|x| ((x - max_val) / temp).exp())
        .collect();

    // Sum for normalization
    let sum: f32 = exp_values.iter().sum();

    // Avoid division by zero
    if sum < 1e-10 {
        // Return uniform distribution
        let uniform = 1.0 / values.len() as f32;
        return vec![uniform; values.len()];
    }

    // Normalize
    exp_values.iter().map(|x| x / sum).collect()
}

/// Apply softmax normalization in-place.
///
/// More efficient than `softmax_normalize` when output buffer is pre-allocated.
///
/// # Arguments
///
/// * `values` - Input values (modified in-place)
/// * `temperature` - Temperature parameter
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::softmax_normalize_inplace;
///
/// let mut values = vec![1.0, 2.0, 3.0];
/// softmax_normalize_inplace(&mut values, 1.0);
/// let sum: f32 = values.iter().sum();
/// assert!((sum - 1.0).abs() < 1e-6);
/// ```
pub fn softmax_normalize_inplace(values: &mut [f32], temperature: f32) {
    if values.is_empty() {
        return;
    }

    let temp = if temperature.abs() < 1e-10 {
        1e-10
    } else {
        temperature
    };

    // Find max for numerical stability
    let max_val = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    // Apply exp((x - max) / T)
    let mut sum = 0.0f32;
    for v in values.iter_mut() {
        *v = ((*v - max_val) / temp).exp();
        sum += *v;
    }

    // Normalize
    if sum < 1e-10 {
        let uniform = 1.0 / values.len() as f32;
        for v in values.iter_mut() {
            *v = uniform;
        }
    } else {
        for v in values.iter_mut() {
            *v /= sum;
        }
    }
}

/// Compute cosine similarity between two vectors.
///
/// Formula: cos(a, b) = (a . b) / (||a|| * ||b||)
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
///
/// Similarity in [-1, 1] where:
/// - 1.0 = identical direction
/// - 0.0 = orthogonal
/// - -1.0 = opposite direction
///
/// Returns 0.0 for zero vectors.
///
/// # Performance
///
/// - O(n) time complexity
/// - Single pass through both vectors
/// - <1ms for 1536-dimensional vectors
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::cosine_similarity;
///
/// let a = vec![1.0, 0.0, 0.0];
/// let b = vec![1.0, 0.0, 0.0];
/// assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
///
/// let c = vec![0.0, 1.0, 0.0];
/// assert!(cosine_similarity(&a, &c).abs() < 1e-6); // Orthogonal
/// ```
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vectors must have same length");

    if a.len() != b.len() {
        return 0.0;
    }

    if a.is_empty() {
        return 0.0;
    }

    // Single pass: compute dot product and norms simultaneously
    let mut dot_product = 0.0f32;
    let mut norm_a_sq = 0.0f32;
    let mut norm_b_sq = 0.0f32;

    for (a_i, b_i) in a.iter().zip(b.iter()) {
        dot_product += a_i * b_i;
        norm_a_sq += a_i * a_i;
        norm_b_sq += b_i * b_i;
    }

    let norm_product = (norm_a_sq * norm_b_sq).sqrt();

    if norm_product < 1e-10 {
        return 0.0;
    }

    // Clamp to handle floating-point errors
    (dot_product / norm_product).clamp(-1.0, 1.0)
}

/// Compute cosine distance between two vectors.
///
/// Formula: distance = 1 - cosine_similarity(a, b)
///
/// # Returns
///
/// Distance in [0, 2] where:
/// - 0.0 = identical direction
/// - 1.0 = orthogonal
/// - 2.0 = opposite direction
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::cosine_distance;
///
/// let a = vec![1.0, 0.0];
/// let b = vec![1.0, 0.0];
/// assert!(cosine_distance(&a, &b).abs() < 1e-6); // Identical
///
/// let c = vec![0.0, 1.0];
/// assert!((cosine_distance(&a, &c) - 1.0).abs() < 1e-6); // Orthogonal
/// ```
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_similarity(a, b)
}

/// Compute normalized cosine distance in [0, 1] range.
///
/// Formula: normalized = (1 - cosine_similarity) / 2
///
/// # Returns
///
/// Distance in [0, 1] where:
/// - 0.0 = identical direction
/// - 0.5 = orthogonal
/// - 1.0 = opposite direction
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::normalized_cosine_distance;
///
/// let a = vec![1.0, 0.0];
/// let b = vec![-1.0, 0.0];
/// assert!((normalized_cosine_distance(&a, &b) - 1.0).abs() < 1e-6);
/// ```
#[inline]
pub fn normalized_cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    (1.0 - cosine_similarity(a, b)) / 2.0
}

/// Compute centroid (mean) of a set of vectors.
///
/// # Arguments
///
/// * `vectors` - Slice of vectors (all must have same dimension)
///
/// # Returns
///
/// Centroid vector, or empty vector if input is empty.
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::compute_centroid;
///
/// let vectors = vec![
///     vec![1.0, 0.0],
///     vec![0.0, 1.0],
/// ];
/// let centroid = compute_centroid(&vectors);
/// assert!((centroid[0] - 0.5).abs() < 1e-6);
/// assert!((centroid[1] - 0.5).abs() < 1e-6);
/// ```
pub fn compute_centroid(vectors: &[Vec<f32>]) -> Vec<f32> {
    if vectors.is_empty() {
        return Vec::new();
    }

    let dim = vectors[0].len();
    if dim == 0 {
        return Vec::new();
    }

    let mut centroid = vec![0.0f32; dim];
    let count = vectors.len() as f32;

    for vec in vectors {
        debug_assert_eq!(vec.len(), dim, "All vectors must have same dimension");
        for (i, v) in vec.iter().enumerate() {
            if i < dim {
                centroid[i] += v;
            }
        }
    }

    for c in centroid.iter_mut() {
        *c /= count;
    }

    centroid
}

/// Compute weighted centroid with importance weights.
///
/// # Arguments
///
/// * `vectors` - Slice of vectors
/// * `weights` - Importance weights for each vector
///
/// # Returns
///
/// Weighted centroid vector.
///
/// # Example
///
/// ```
/// use context_graph_utl::surprise::compute_weighted_centroid;
///
/// let vectors = vec![
///     vec![1.0, 0.0],
///     vec![0.0, 1.0],
/// ];
/// let weights = vec![0.75, 0.25]; // First vector weighted more
/// let centroid = compute_weighted_centroid(&vectors, &weights);
/// assert!(centroid[0] > 0.5); // Closer to first vector
/// ```
pub fn compute_weighted_centroid(vectors: &[Vec<f32>], weights: &[f32]) -> Vec<f32> {
    if vectors.is_empty() || weights.is_empty() {
        return Vec::new();
    }

    let dim = vectors[0].len();
    if dim == 0 {
        return Vec::new();
    }

    let mut centroid = vec![0.0f32; dim];
    let mut weight_sum = 0.0f32;

    for (vec, weight) in vectors.iter().zip(weights.iter()) {
        let w = weight.max(0.0); // Ensure non-negative weights
        weight_sum += w;
        for (i, v) in vec.iter().enumerate() {
            if i < dim {
                centroid[i] += v * w;
            }
        }
    }

    if weight_sum > 1e-10 {
        for c in centroid.iter_mut() {
            *c /= weight_sum;
        }
    }

    centroid
}

// ============================================================================
// TESTS - REAL DATA ONLY, NO MOCKS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    // ========== KL DIVERGENCE TESTS ==========

    #[test]
    fn test_kl_divergence_identical_distributions() {
        let p = vec![0.25, 0.25, 0.25, 0.25];
        let q = vec![0.25, 0.25, 0.25, 0.25];
        let kl = kl_divergence(&p, &q, 1e-10);
        assert!(kl.abs() < EPSILON, "KL of identical distributions should be 0, got {}", kl);
    }

    #[test]
    fn test_kl_divergence_non_negative() {
        let p = vec![0.9, 0.1];
        let q = vec![0.1, 0.9];
        let kl = kl_divergence(&p, &q, 1e-10);
        assert!(kl >= 0.0, "KL divergence must be non-negative");
    }

    #[test]
    fn test_kl_divergence_asymmetric() {
        let p = vec![0.9, 0.1];
        let q = vec![0.5, 0.5];
        let kl_pq = kl_divergence(&p, &q, 1e-10);
        let kl_qp = kl_divergence(&q, &p, 1e-10);
        assert!((kl_pq - kl_qp).abs() > EPSILON, "KL should be asymmetric");
    }

    #[test]
    fn test_kl_divergence_empty() {
        let p: Vec<f32> = vec![];
        let q: Vec<f32> = vec![];
        let kl = kl_divergence(&p, &q, 1e-10);
        assert_eq!(kl, 0.0);
    }

    #[test]
    fn test_kl_divergence_mismatched_length() {
        let p = vec![0.5, 0.5];
        let q = vec![0.33, 0.33, 0.34];
        let kl = kl_divergence(&p, &q, 1e-10);
        assert!(kl.is_nan());
    }

    #[test]
    fn test_kl_divergence_with_zeros() {
        let p = vec![0.0, 1.0];
        let q = vec![0.5, 0.5];
        let kl = kl_divergence(&p, &q, 1e-10);
        assert!(kl.is_finite(), "Should handle zeros in p");
    }

    #[test]
    fn test_kl_divergence_peaked_vs_uniform() {
        let peaked = vec![0.9, 0.05, 0.05];
        let uniform = vec![0.33, 0.33, 0.34];
        let kl = kl_divergence(&peaked, &uniform, 1e-10);
        assert!(kl > 0.5, "Peaked vs uniform should have high KL: {}", kl);
    }

    #[test]
    fn test_normalize_kl() {
        assert_eq!(normalize_kl(5.0, 10.0), 0.5);
        assert_eq!(normalize_kl(0.0, 10.0), 0.0);
        assert_eq!(normalize_kl(10.0, 10.0), 1.0);
        assert_eq!(normalize_kl(15.0, 10.0), 1.0); // Clamped
        assert_eq!(normalize_kl(5.0, 0.0), 0.0); // Edge case
    }

    // ========== SOFTMAX TESTS ==========

    #[test]
    fn test_softmax_sums_to_one() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let probs = softmax_normalize(&values, 1.0);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < EPSILON, "Softmax should sum to 1, got {}", sum);
    }

    #[test]
    fn test_softmax_all_non_negative() {
        let values = vec![-5.0, -2.0, 0.0, 2.0, 5.0];
        let probs = softmax_normalize(&values, 1.0);
        assert!(probs.iter().all(|&p| p >= 0.0), "All probabilities should be non-negative");
    }

    #[test]
    fn test_softmax_preserves_order() {
        let values = vec![1.0, 2.0, 3.0];
        let probs = softmax_normalize(&values, 1.0);
        assert!(probs[0] < probs[1], "Larger input should have larger probability");
        assert!(probs[1] < probs[2], "Larger input should have larger probability");
    }

    #[test]
    fn test_softmax_temperature_effect() {
        let values = vec![1.0, 2.0, 3.0];

        // Low temperature = more peaked
        let probs_low = softmax_normalize(&values, 0.1);
        // High temperature = more uniform
        let probs_high = softmax_normalize(&values, 10.0);

        // Variance should be higher for low temperature
        let variance_low: f32 = probs_low.iter().map(|p| (p - 0.33).powi(2)).sum();
        let variance_high: f32 = probs_high.iter().map(|p| (p - 0.33).powi(2)).sum();
        assert!(variance_low > variance_high, "Low temperature should be more peaked");
    }

    #[test]
    fn test_softmax_empty() {
        let values: Vec<f32> = vec![];
        let probs = softmax_normalize(&values, 1.0);
        assert!(probs.is_empty());
    }

    #[test]
    fn test_softmax_single() {
        let values = vec![5.0];
        let probs = softmax_normalize(&values, 1.0);
        assert_eq!(probs.len(), 1);
        assert!((probs[0] - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_softmax_inplace() {
        let mut values = vec![1.0, 2.0, 3.0];
        softmax_normalize_inplace(&mut values, 1.0);
        let sum: f32 = values.iter().sum();
        assert!((sum - 1.0).abs() < EPSILON);
    }

    // ========== COSINE SIMILARITY TESTS ==========

    #[test]
    fn test_cosine_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < EPSILON, "Identical vectors should have similarity 1");
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < EPSILON, "Orthogonal vectors should have similarity 0");
    }

    #[test]
    fn test_cosine_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < EPSILON, "Opposite vectors should have similarity -1");
    }

    #[test]
    fn test_cosine_symmetric() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let sim_ab = cosine_similarity(&a, &b);
        let sim_ba = cosine_similarity(&b, &a);
        assert!((sim_ab - sim_ba).abs() < EPSILON, "Cosine similarity should be symmetric");
    }

    #[test]
    fn test_cosine_range() {
        let a = vec![1.0, -2.0, 3.0, -4.0];
        let b = vec![-1.0, 2.0, -3.0, 4.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim >= -1.0 && sim <= 1.0, "Similarity should be in [-1, 1]");
    }

    #[test]
    fn test_cosine_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Zero vector should have similarity 0");
    }

    #[test]
    fn test_cosine_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        assert!(cosine_distance(&a, &b).abs() < EPSILON);

        let c = vec![0.0, 1.0];
        assert!((cosine_distance(&a, &c) - 1.0).abs() < EPSILON);

        let d = vec![-1.0, 0.0];
        assert!((cosine_distance(&a, &d) - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_normalized_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        assert!(normalized_cosine_distance(&a, &b).abs() < EPSILON);

        let c = vec![0.0, 1.0];
        assert!((normalized_cosine_distance(&a, &c) - 0.5).abs() < EPSILON);

        let d = vec![-1.0, 0.0];
        assert!((normalized_cosine_distance(&a, &d) - 1.0).abs() < EPSILON);
    }

    // ========== CENTROID TESTS ==========

    #[test]
    fn test_centroid_basic() {
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
        ];
        let centroid = compute_centroid(&vectors);
        assert!((centroid[0] - 0.5).abs() < EPSILON);
        assert!((centroid[1] - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_centroid_single_vector() {
        let vectors = vec![vec![1.0, 2.0, 3.0]];
        let centroid = compute_centroid(&vectors);
        assert_eq!(centroid, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_centroid_empty() {
        let vectors: Vec<Vec<f32>> = vec![];
        let centroid = compute_centroid(&vectors);
        assert!(centroid.is_empty());
    }

    #[test]
    fn test_weighted_centroid() {
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
        ];
        let weights = vec![0.75, 0.25];
        let centroid = compute_weighted_centroid(&vectors, &weights);
        assert!((centroid[0] - 0.75).abs() < EPSILON);
        assert!((centroid[1] - 0.25).abs() < EPSILON);
    }

    #[test]
    fn test_weighted_centroid_equal_weights() {
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
        ];
        let weights = vec![1.0, 1.0];
        let centroid = compute_weighted_centroid(&vectors, &weights);
        assert!((centroid[0] - 0.5).abs() < EPSILON);
        assert!((centroid[1] - 0.5).abs() < EPSILON);
    }

    // ========== PERFORMANCE TESTS ==========

    #[test]
    fn test_high_dimensional_cosine() {
        // 1536-dimensional vectors (OpenAI embedding size)
        let a: Vec<f32> = (0..1536).map(|i| (i as f32).sin()).collect();
        let b: Vec<f32> = (0..1536).map(|i| (i as f32).cos()).collect();

        let start = std::time::Instant::now();
        let _ = cosine_similarity(&a, &b);
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() < 1, "Should complete in <1ms, took {:?}", elapsed);
    }

    #[test]
    fn test_high_dimensional_softmax() {
        let values: Vec<f32> = (0..1536).map(|i| (i as f32) / 100.0).collect();

        let start = std::time::Instant::now();
        let probs = softmax_normalize(&values, 1.0);
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() < 1, "Should complete in <1ms, took {:?}", elapsed);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_high_dimensional_kl() {
        let p: Vec<f32> = (0..1536).map(|_| 1.0 / 1536.0).collect();
        let q: Vec<f32> = (0..1536).map(|_| 1.0 / 1536.0).collect();

        let start = std::time::Instant::now();
        let _ = kl_divergence(&p, &q, 1e-10);
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() < 1, "Should complete in <1ms, took {:?}", elapsed);
    }
}
```

## Acceptance Criteria

### Function Signatures (MUST MATCH EXACTLY)

- [ ] `kl_divergence(p: &[f32], q: &[f32], epsilon: f32) -> f32`
- [ ] `kl_divergence_with_config(p: &[f32], q: &[f32], config: &KlConfig) -> f32`
- [ ] `normalize_kl(kl: f32, max_kl: f32) -> f32`
- [ ] `softmax_normalize(values: &[f32], temperature: f32) -> Vec<f32>`
- [ ] `softmax_normalize_inplace(values: &mut [f32], temperature: f32)`
- [ ] `cosine_similarity(a: &[f32], b: &[f32]) -> f32`
- [ ] `cosine_distance(a: &[f32], b: &[f32]) -> f32`
- [ ] `normalized_cosine_distance(a: &[f32], b: &[f32]) -> f32`
- [ ] `compute_centroid(vectors: &[Vec<f32>]) -> Vec<f32>`
- [ ] `compute_weighted_centroid(vectors: &[Vec<f32>], weights: &[f32]) -> Vec<f32>`

### Mathematical Properties

- [ ] KL divergence is non-negative
- [ ] KL(P || P) = 0 for identical distributions
- [ ] Softmax output sums to 1.0
- [ ] Softmax preserves ordering
- [ ] Cosine similarity is in [-1, 1]
- [ ] Cosine similarity is symmetric

### Performance Requirements

- [ ] <1ms for 1536-dimensional vectors (all functions)
- [ ] Numerical stability with epsilon parameter
- [ ] No NaN/Infinity for valid inputs

## Verification Commands

```bash
# 1. Build the crate
cargo build -p context-graph-utl

# 2. Run surprise tests
cargo test -p context-graph-utl surprise -- --nocapture

# 3. Run specific tests
cargo test -p context-graph-utl test_kl_divergence_identical
cargo test -p context-graph-utl test_cosine_similarity
cargo test -p context-graph-utl test_high_dimensional

# 4. Run clippy
cargo clippy -p context-graph-utl -- -D warnings

# 5. Run doc tests
cargo test -p context-graph-utl --doc
```

## Dependencies

- M05-T02: SurpriseConfig (for KlConfig dependency)

**Note**: The KlConfig struct should be defined in M05-T02 or can be stubbed initially.

## Notes for Implementer

1. Use the log-sum-exp trick for numerical stability in softmax
2. Clamp cosine similarity to handle floating-point errors
3. Tests are co-located in `#[cfg(test)]` module per constitution
4. Performance tests verify <1ms for 1536D vectors
5. The epsilon parameter prevents log(0) and division by zero

---

*Task Version: 1.0.0*
*Created: 2026-01-04*
*Module: 05 - UTL Integration*
