//! Distance metrics for similarity computation.
//!
//! # FAIL FAST. NO FALLBACKS.
//!
//! Invalid inputs panic immediately. No silent defaults.
//!
//! # F-026 Sherlock investigation 2026-05-19
//!
//! Panics are fail-closed but they CRASH the MCP server process. Per the
//! audit findings, the MCP server must return a structured error response,
//! not abort. This module now exposes `try_*` siblings of every panicking
//! function (`try_recommended_metric`, `try_compute_distance`,
//! `try_distance_to_similarity`, `try_cosine_similarity`) that return
//! `Result<_, MetricError>` instead of panicking. New callers MUST use the
//! `try_*` variants; legacy callers (and the existing `#[should_panic]`
//! tests) continue to use the panicking variants.

use super::hnsw_config::{DistanceMetric, EmbedderIndex};

/// Structured errors for the Result-returning metric helpers (F-026).
///
/// Per F-026 (Sherlock investigation 2026-05-19): the legacy `panic!`
/// helpers crash the MCP server process on bad input. These structured
/// errors let the server return a JSON-RPC error instead of aborting.
///
/// All variants use `SCREAMING_SNAKE_CASE` codes in their `Display`
/// implementation so operators can grep logs for the failure mode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MetricError {
    #[error("METRIC_INDEX_OUT_OF_RANGE: embedder_idx={embedder_idx} valid_max=12")]
    IndexOutOfRange { embedder_idx: usize },
    #[error("METRIC_INVERTED_INDEX_EMBEDDER: embedder={embedder:?} uses inverted index, not vector distance")]
    InvertedIndexEmbedder { embedder: EmbedderIndex },
    #[error("METRIC_DIMENSION_MISMATCH: left={left} right={right}")]
    DimensionMismatch { left: usize, right: usize },
    #[error("METRIC_EMPTY_VECTORS")]
    EmptyVectors,
    #[error("METRIC_MAXSIM_REQUIRES_TOKEN_LEVEL: cannot use MaxSim at vector level")]
    MaxSimRequiresTokenLevel,
}

/// Get recommended distance metric for embedder by 0-12 index.
///
/// # Panics
///
/// - Panics with "INDEX ERROR" if `embedder_idx >= 13`
/// - Panics with "METRIC ERROR" if called for E6 (index 5) or E13 (index 12) - they use inverted index
///
/// # Returns
///
/// - `DistanceMetric::AsymmetricCosine` for E5 (index 4)
/// - `DistanceMetric::MaxSim` for E12 (index 11)
/// - `DistanceMetric::Cosine` for all others
pub fn recommended_metric(embedder_idx: usize) -> DistanceMetric {
    let embedder = EmbedderIndex::from_index(embedder_idx);
    match embedder {
        EmbedderIndex::E5Causal => DistanceMetric::AsymmetricCosine,
        EmbedderIndex::E6Sparse | EmbedderIndex::E13Splade => {
            panic!("METRIC ERROR: E6/E13 use inverted index, not vector distance")
        }
        EmbedderIndex::E12LateInteraction => DistanceMetric::MaxSim,
        _ => DistanceMetric::Cosine,
    }
}

/// Compute distance between two vectors using specified metric.
///
/// # Panics
///
/// - Panics with "METRIC ERROR" if vectors have different lengths
/// - Panics with "METRIC ERROR" if vectors are empty
/// - Panics with "METRIC ERROR" if MaxSim is used (requires token-level computation)
///
/// # Returns
///
/// Distance value where lower = more similar:
/// - Cosine: [0, 2] where 0 = identical
/// - DotProduct: negated (more negative = more similar)
/// - Euclidean: [0, inf) where 0 = identical
/// - AsymmetricCosine: same as Cosine (asymmetry handled at query time)
pub fn compute_distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
    if a.len() != b.len() {
        panic!(
            "METRIC ERROR: vector length mismatch: {} vs {}",
            a.len(),
            b.len()
        );
    }
    if a.is_empty() {
        panic!("METRIC ERROR: empty vectors");
    }

    match metric {
        DistanceMetric::Cosine | DistanceMetric::AsymmetricCosine => {
            let mut dot = 0.0f32;
            let mut norm_a = 0.0f32;
            let mut norm_b = 0.0f32;

            for i in 0..a.len() {
                dot += a[i] * b[i];
                norm_a += a[i] * a[i];
                norm_b += b[i] * b[i];
            }

            let norm_a = norm_a.sqrt();
            let norm_b = norm_b.sqrt();

            if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
                return 1.0; // Maximum distance for zero vectors
            }

            1.0 - (dot / (norm_a * norm_b))
        }
        DistanceMetric::DotProduct => {
            let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            -dot // Negate so lower = more similar
        }
        DistanceMetric::Euclidean => a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt(),
        DistanceMetric::MaxSim => {
            panic!("METRIC ERROR: MaxSim requires token-level computation, not vector distance")
        }
    }
}

/// Convert distance to similarity in [0.0, 1.0] range.
///
/// # Panics
///
/// Panics with "METRIC ERROR" if MaxSim is used.
///
/// # Conversion formulas
///
/// - Cosine: `(2 - distance) / 2` clamped to [0, 1]
/// - DotProduct: sigmoid of negated distance
/// - Euclidean: exponential decay `exp(-distance)`
pub fn distance_to_similarity(distance: f32, metric: DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::Cosine | DistanceMetric::AsymmetricCosine => {
            // Cosine distance is in [0, 2], convert to similarity [0, 1]
            ((2.0 - distance) / 2.0).clamp(0.0, 1.0)
        }
        DistanceMetric::DotProduct => {
            // Dot product is negated, so un-negate and sigmoid
            let dp = -distance;
            1.0 / (1.0 + (-dp).exp())
        }
        DistanceMetric::Euclidean => {
            // Euclidean: use exponential decay
            (-distance).exp()
        }
        DistanceMetric::MaxSim => {
            panic!("METRIC ERROR: MaxSim similarity computed at token level")
        }
    }
}

/// Compute cosine similarity directly (not distance).
///
/// Returns value in [-1, 1] where 1 = identical.
///
/// # Panics
///
/// - Panics with "METRIC ERROR" if vectors have different lengths
/// - Panics with "METRIC ERROR" if vectors are empty
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        panic!(
            "METRIC ERROR: vector length mismatch: {} vs {}",
            a.len(),
            b.len()
        );
    }
    if a.is_empty() {
        panic!("METRIC ERROR: empty vectors");
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator < f32::EPSILON {
        // STOR-L6: Zero-norm vectors should be rejected at insertion time (STOR-M1).
        // If we reach here, something bypassed validation — log for diagnostics.
        tracing::warn!(
            "Zero-norm vector detected in similarity computation — should have been rejected at insertion"
        );
        return 0.0;
    }

    dot / denominator
}

// ============================================================================
// F-026 Result-returning siblings (Sherlock investigation 2026-05-19)
//
// These functions mirror the panicking ones above but return
// `Result<_, MetricError>` so the MCP server can surface a JSON-RPC error
// instead of crashing on bad input. New callers MUST use these.
// ============================================================================

/// Result-returning variant of `recommended_metric` (F-026).
pub fn try_recommended_metric(embedder_idx: usize) -> Result<DistanceMetric, MetricError> {
    if embedder_idx >= 13 {
        return Err(MetricError::IndexOutOfRange { embedder_idx });
    }
    let embedder = EmbedderIndex::from_index(embedder_idx);
    match embedder {
        EmbedderIndex::E5Causal => Ok(DistanceMetric::AsymmetricCosine),
        EmbedderIndex::E6Sparse | EmbedderIndex::E13Splade => {
            Err(MetricError::InvertedIndexEmbedder { embedder })
        }
        EmbedderIndex::E12LateInteraction => Ok(DistanceMetric::MaxSim),
        _ => Ok(DistanceMetric::Cosine),
    }
}

/// Result-returning variant of `compute_distance` (F-026).
///
/// Returns `MetricError::DimensionMismatch` / `EmptyVectors` /
/// `MaxSimRequiresTokenLevel` instead of panicking.
pub fn try_compute_distance(
    a: &[f32],
    b: &[f32],
    metric: DistanceMetric,
) -> Result<f32, MetricError> {
    if a.len() != b.len() {
        return Err(MetricError::DimensionMismatch {
            left: a.len(),
            right: b.len(),
        });
    }
    if a.is_empty() {
        return Err(MetricError::EmptyVectors);
    }

    match metric {
        DistanceMetric::Cosine | DistanceMetric::AsymmetricCosine => {
            let mut dot = 0.0f32;
            let mut norm_a = 0.0f32;
            let mut norm_b = 0.0f32;
            for i in 0..a.len() {
                dot += a[i] * b[i];
                norm_a += a[i] * a[i];
                norm_b += b[i] * b[i];
            }
            let norm_a = norm_a.sqrt();
            let norm_b = norm_b.sqrt();
            if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
                return Ok(1.0); // Maximum distance for zero vectors
            }
            Ok(1.0 - (dot / (norm_a * norm_b)))
        }
        DistanceMetric::DotProduct => {
            let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            Ok(-dot)
        }
        DistanceMetric::Euclidean => Ok(a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()),
        DistanceMetric::MaxSim => Err(MetricError::MaxSimRequiresTokenLevel),
    }
}

/// Result-returning variant of `distance_to_similarity` (F-026).
pub fn try_distance_to_similarity(
    distance: f32,
    metric: DistanceMetric,
) -> Result<f32, MetricError> {
    match metric {
        DistanceMetric::Cosine | DistanceMetric::AsymmetricCosine => {
            Ok(((2.0 - distance) / 2.0).clamp(0.0, 1.0))
        }
        DistanceMetric::DotProduct => {
            let dp = -distance;
            Ok(1.0 / (1.0 + (-dp).exp()))
        }
        DistanceMetric::Euclidean => Ok((-distance).exp()),
        DistanceMetric::MaxSim => Err(MetricError::MaxSimRequiresTokenLevel),
    }
}

/// Result-returning variant of `cosine_similarity` (F-026).
///
/// Returns `MetricError::DimensionMismatch` / `EmptyVectors` instead of
/// panicking, so the MCP server can surface a JSON-RPC error rather than
/// crashing the process.
pub fn try_cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, MetricError> {
    if a.len() != b.len() {
        return Err(MetricError::DimensionMismatch {
            left: a.len(),
            right: b.len(),
        });
    }
    if a.is_empty() {
        return Err(MetricError::EmptyVectors);
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator < f32::EPSILON {
        // Same zero-norm semantics as the panicking sibling — diagnostic
        // log + 0.0 return (NOT an error variant). Zero-norm is a valid
        // runtime state (already rejected at insertion per STOR-M1); a
        // dimension mismatch is the structural bug we surface as Err.
        tracing::warn!(
            "Zero-norm vector detected in try_cosine_similarity — should have been rejected at insertion"
        );
        return Ok(0.0);
    }

    Ok(dot / denominator)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // recommended_metric Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_recommended_metric_e1_cosine() {
        println!("=== TEST: E1 (idx 0) returns Cosine ===");
        println!("BEFORE: recommended_metric(0)");

        let metric = recommended_metric(0);
        println!("AFTER: {:?}", metric);

        assert_eq!(metric, DistanceMetric::Cosine);
        println!("RESULT: PASS - E1 uses Cosine");
    }

    #[test]
    fn test_recommended_metric_e5_asymmetric() {
        println!("=== TEST: E5 (idx 4) returns AsymmetricCosine ===");
        println!("BEFORE: recommended_metric(4)");

        let metric = recommended_metric(4);
        println!("AFTER: {:?}", metric);

        assert_eq!(metric, DistanceMetric::AsymmetricCosine);
        println!("RESULT: PASS - E5 Causal uses AsymmetricCosine");
    }

    #[test]
    fn test_recommended_metric_e12_maxsim() {
        println!("=== TEST: E12 (idx 11) returns MaxSim ===");
        println!("BEFORE: recommended_metric(11)");

        let metric = recommended_metric(11);
        println!("AFTER: {:?}", metric);

        assert_eq!(metric, DistanceMetric::MaxSim);
        println!("RESULT: PASS - E12 LateInteraction uses MaxSim");
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_recommended_metric_e6() {
        println!("=== TEST: E6 (idx 5) should panic ===");
        println!("BEFORE: recommended_metric(5)");
        let _ = recommended_metric(5);
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_recommended_metric_e13() {
        println!("=== TEST: E13 (idx 12) should panic ===");
        println!("BEFORE: recommended_metric(12)");
        let _ = recommended_metric(12);
    }

    // -------------------------------------------------------------------------
    // compute_distance Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_compute_distance_cosine_identical() {
        println!("=== TEST: Cosine distance 0 for identical vectors ===");
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let distance = compute_distance(&a, &b, DistanceMetric::Cosine);
        println!("AFTER: distance={}", distance);

        assert!(
            distance.abs() < 1e-6,
            "Identical vectors should have distance 0"
        );
        println!("RESULT: PASS");
    }

    #[test]
    fn test_compute_distance_cosine_orthogonal() {
        println!("=== TEST: Cosine distance 1 for orthogonal vectors ===");
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let distance = compute_distance(&a, &b, DistanceMetric::Cosine);
        println!("AFTER: distance={}", distance);

        assert!(
            (distance - 1.0).abs() < 1e-6,
            "Orthogonal vectors should have distance 1"
        );
        println!("RESULT: PASS");
    }

    #[test]
    fn test_compute_distance_euclidean() {
        println!("=== TEST: Euclidean distance sqrt(2) for [1,0] vs [0,1] ===");
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let distance = compute_distance(&a, &b, DistanceMetric::Euclidean);
        println!("AFTER: distance={}", distance);

        let expected = 2.0f32.sqrt();
        assert!(
            (distance - expected).abs() < 1e-6,
            "Expected sqrt(2)={}, got {}",
            expected,
            distance
        );
        println!("RESULT: PASS");
    }

    #[test]
    fn test_compute_distance_dot_product() {
        println!("=== TEST: DotProduct negation ===");
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let distance = compute_distance(&a, &b, DistanceMetric::DotProduct);
        // dot = 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        // negated = -32
        println!("AFTER: distance={}", distance);

        assert!(
            (distance - (-32.0)).abs() < 1e-6,
            "Expected -32.0, got {}",
            distance
        );
        println!("RESULT: PASS - dot product negated correctly");
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_compute_distance_length_mismatch() {
        println!("=== TEST: Length mismatch should panic ===");
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        println!("BEFORE: a.len()={}, b.len()={}", a.len(), b.len());
        let _ = compute_distance(&a, &b, DistanceMetric::Cosine);
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_compute_distance_empty() {
        println!("=== TEST: Empty vectors should panic ===");
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        println!("BEFORE: empty vectors");
        let _ = compute_distance(&a, &b, DistanceMetric::Cosine);
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_compute_distance_maxsim() {
        println!("=== TEST: MaxSim should panic in compute_distance ===");
        let a = vec![1.0, 2.0];
        let b = vec![3.0, 4.0];
        println!("BEFORE: attempting MaxSim distance");
        let _ = compute_distance(&a, &b, DistanceMetric::MaxSim);
    }

    // -------------------------------------------------------------------------
    // distance_to_similarity Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_distance_to_similarity_cosine() {
        println!("=== TEST: Cosine distance to similarity ===");

        // Distance 0 -> similarity 1
        let sim_0 = distance_to_similarity(0.0, DistanceMetric::Cosine);
        println!("BEFORE: distance=0.0");
        println!("AFTER: similarity={}", sim_0);
        assert!(
            (sim_0 - 1.0).abs() < 1e-6,
            "Distance 0 should give similarity 1"
        );

        // Distance 2 -> similarity 0
        let sim_2 = distance_to_similarity(2.0, DistanceMetric::Cosine);
        println!("BEFORE: distance=2.0");
        println!("AFTER: similarity={}", sim_2);
        assert!(sim_2.abs() < 1e-6, "Distance 2 should give similarity 0");

        // Distance 1 -> similarity 0.5
        let sim_1 = distance_to_similarity(1.0, DistanceMetric::Cosine);
        println!("BEFORE: distance=1.0");
        println!("AFTER: similarity={}", sim_1);
        assert!(
            (sim_1 - 0.5).abs() < 1e-6,
            "Distance 1 should give similarity 0.5"
        );

        println!("RESULT: PASS");
    }

    #[test]
    fn test_distance_to_similarity_euclidean() {
        println!("=== TEST: Euclidean distance to similarity (exp decay) ===");

        // Distance 0 -> similarity 1 (exp(0) = 1)
        let sim_0 = distance_to_similarity(0.0, DistanceMetric::Euclidean);
        println!("BEFORE: distance=0.0");
        println!("AFTER: similarity={}", sim_0);
        assert!(
            (sim_0 - 1.0).abs() < 1e-6,
            "Distance 0 should give similarity 1"
        );

        // Distance 1 -> similarity ~0.368 (exp(-1))
        let sim_1 = distance_to_similarity(1.0, DistanceMetric::Euclidean);
        println!("BEFORE: distance=1.0");
        println!("AFTER: similarity={}", sim_1);
        let expected = (-1.0f32).exp();
        assert!(
            (sim_1 - expected).abs() < 1e-6,
            "Expected {}, got {}",
            expected,
            sim_1
        );

        println!("RESULT: PASS");
    }

    // -------------------------------------------------------------------------
    // cosine_similarity Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_cosine_similarity_identical() {
        println!("=== TEST: Cosine similarity 1.0 for identical normalized ===");
        // Normalized vector
        let inv_sqrt2 = 1.0 / 2.0f32.sqrt();
        let a = vec![inv_sqrt2, inv_sqrt2];
        let b = vec![inv_sqrt2, inv_sqrt2];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let sim = cosine_similarity(&a, &b);
        println!("AFTER: similarity={}", sim);

        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0"
        );
        println!("RESULT: PASS");
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        println!("=== EDGE CASE: Zero vector returns 0.0 similarity ===");
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let sim = cosine_similarity(&a, &b);
        println!("AFTER: similarity={}", sim);

        assert!(sim.abs() < 1e-6, "Zero vector should have similarity 0.0");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        println!("=== TEST: Opposite vectors have similarity -1 ===");
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let sim = cosine_similarity(&a, &b);
        println!("AFTER: similarity={}", sim);

        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "Opposite vectors should have similarity -1"
        );
        println!("RESULT: PASS");
    }

    // -------------------------------------------------------------------------
    // Edge Case and Boundary Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_recommended_metric_all_valid_indices() {
        println!("=== EDGE CASE: All valid indices 0-12 ===");
        // Expected: 0=Cosine, 1=Cosine, 2=Cosine, 3=Cosine, 4=Asymmetric,
        // 5=panic(inverted), 6=Cosine, 7=Cosine, 8=Cosine, 9=Cosine, 10=Cosine,
        // 11=MaxSim, 12=panic(inverted)

        let expected = vec![
            (0, DistanceMetric::Cosine),
            (1, DistanceMetric::Cosine),
            (2, DistanceMetric::Cosine),
            (3, DistanceMetric::Cosine),
            (4, DistanceMetric::AsymmetricCosine),
            // 5 = E6 (inverted - skip)
            (6, DistanceMetric::Cosine),
            (7, DistanceMetric::Cosine),
            (8, DistanceMetric::Cosine),
            (9, DistanceMetric::Cosine),
            (10, DistanceMetric::Cosine),
            (11, DistanceMetric::MaxSim),
            // 12 = E13 (inverted - skip)
        ];

        for (idx, expected_metric) in expected {
            println!("BEFORE: recommended_metric({})", idx);
            let metric = recommended_metric(idx);
            println!("AFTER: {:?}", metric);
            assert_eq!(metric, expected_metric, "Index {} mismatch", idx);
        }

        println!("RESULT: PASS");
    }

    #[test]
    fn test_compute_distance_asymmetric_cosine() {
        println!("=== TEST: AsymmetricCosine behaves like Cosine at vector level ===");
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        println!("BEFORE: a={:?}, b={:?}", a, b);

        let cosine_dist = compute_distance(&a, &b, DistanceMetric::Cosine);
        let asymmetric_dist = compute_distance(&a, &b, DistanceMetric::AsymmetricCosine);
        println!(
            "AFTER: cosine={}, asymmetric={}",
            cosine_dist, asymmetric_dist
        );

        assert!(
            (cosine_dist - asymmetric_dist).abs() < 1e-6,
            "AsymmetricCosine should behave like Cosine at vector level"
        );
        println!("RESULT: PASS");
    }

    #[test]
    fn test_distance_to_similarity_dot_product() {
        println!("=== TEST: DotProduct distance to similarity (sigmoid) ===");

        // Large negative distance (high positive dot product) -> high similarity
        let sim_high = distance_to_similarity(-10.0, DistanceMetric::DotProduct);
        println!("BEFORE: distance=-10.0 (high positive dot)");
        println!("AFTER: similarity={}", sim_high);
        assert!(
            sim_high > 0.99,
            "High positive dot should give near 1 similarity"
        );

        // Zero distance -> similarity 0.5 (sigmoid(0) = 0.5)
        let sim_zero = distance_to_similarity(0.0, DistanceMetric::DotProduct);
        println!("BEFORE: distance=0.0");
        println!("AFTER: similarity={}", sim_zero);
        assert!(
            (sim_zero - 0.5).abs() < 1e-6,
            "Zero dot should give similarity 0.5"
        );

        println!("RESULT: PASS");
    }

    #[test]
    fn test_cosine_similarity_high_dimensional() {
        println!("=== EDGE CASE: High-dimensional vectors (1024D) ===");
        let dim = 1024;
        let a: Vec<f32> = (0..dim).map(|i| (i as f32) / 100.0).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32) / 100.0 + 0.1).collect();
        println!("BEFORE: 1024D vectors, slight offset");

        let sim = cosine_similarity(&a, &b);
        println!("AFTER: similarity={}", sim);

        // Should be very close to 1 since vectors are nearly parallel
        assert!(
            sim > 0.99,
            "Nearly parallel vectors should have high similarity"
        );
        println!("RESULT: PASS");
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_distance_to_similarity_maxsim() {
        println!("=== TEST: MaxSim should panic in distance_to_similarity ===");
        let _ = distance_to_similarity(0.5, DistanceMetric::MaxSim);
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_cosine_similarity_length_mismatch() {
        println!("=== TEST: Length mismatch should panic in cosine_similarity ===");
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let _ = cosine_similarity(&a, &b);
    }

    #[test]
    #[should_panic(expected = "METRIC ERROR")]
    fn test_panic_cosine_similarity_empty() {
        println!("=== TEST: Empty vectors should panic in cosine_similarity ===");
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let _ = cosine_similarity(&a, &b);
    }

    // -------------------------------------------------------------------------
    // Verification Log Test
    // -------------------------------------------------------------------------

    // =========================================================================
    // F-026 REGRESSION TESTS (Sherlock investigation 2026-05-19)
    //
    // These tests assert that the `try_*` siblings return structured errors
    // instead of panicking. If anyone replaces them with the panicking
    // helpers, these tests fail because they expect Result.
    //
    // Uses deterministic Gaussian samples via SplitMix64+Box-Muller per
    // tier-compression test convention — NOT sin waves.
    // =========================================================================

    fn f026_splitmix64(seed: u64) -> u64 {
        let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn f026_gaussian(seed: u64) -> f32 {
        let r1 = f026_splitmix64(seed.wrapping_mul(2)) >> 11;
        let r2 = f026_splitmix64(seed.wrapping_mul(2).wrapping_add(1)) >> 11;
        let u1 = (r1 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
        let u2 = (r2 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;
        let z = r * theta.cos();
        z.tanh() as f32
    }

    fn f026_vec(n: usize, seed: u64) -> Vec<f32> {
        (0..n)
            .map(|i| f026_gaussian(seed.wrapping_add(i as u64)))
            .collect()
    }

    #[test]
    fn test_f026_try_recommended_metric_oob_returns_err() {
        match try_recommended_metric(13) {
            Err(MetricError::IndexOutOfRange { embedder_idx }) => {
                assert_eq!(embedder_idx, 13);
            }
            other => panic!("expected IndexOutOfRange, got {:?}", other),
        }
        match try_recommended_metric(99) {
            Err(MetricError::IndexOutOfRange { embedder_idx }) => {
                assert_eq!(embedder_idx, 99);
            }
            other => panic!("expected IndexOutOfRange, got {:?}", other),
        }
    }

    #[test]
    fn test_f026_try_recommended_metric_inverted_returns_err() {
        match try_recommended_metric(5) {
            Err(MetricError::InvertedIndexEmbedder { embedder }) => {
                assert_eq!(embedder, EmbedderIndex::E6Sparse);
            }
            other => panic!("expected InvertedIndexEmbedder, got {:?}", other),
        }
        match try_recommended_metric(12) {
            Err(MetricError::InvertedIndexEmbedder { embedder }) => {
                assert_eq!(embedder, EmbedderIndex::E13Splade);
            }
            other => panic!("expected InvertedIndexEmbedder, got {:?}", other),
        }
    }

    #[test]
    fn test_f026_try_recommended_metric_happy_path() {
        assert_eq!(try_recommended_metric(0).unwrap(), DistanceMetric::Cosine);
        assert_eq!(
            try_recommended_metric(4).unwrap(),
            DistanceMetric::AsymmetricCosine
        );
        assert_eq!(try_recommended_metric(11).unwrap(), DistanceMetric::MaxSim);
    }

    #[test]
    fn test_f026_try_compute_distance_dim_mismatch_returns_err() {
        let a = f026_vec(64, 0x111);
        let b = f026_vec(128, 0x222);
        match try_compute_distance(&a, &b, DistanceMetric::Cosine) {
            Err(MetricError::DimensionMismatch { left, right }) => {
                assert_eq!(left, 64);
                assert_eq!(right, 128);
            }
            other => panic!("expected DimensionMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_f026_try_compute_distance_empty_returns_err() {
        let result = try_compute_distance(&[], &[], DistanceMetric::Cosine);
        assert_eq!(result, Err(MetricError::EmptyVectors));
    }

    #[test]
    fn test_f026_try_compute_distance_maxsim_returns_err() {
        let a = f026_vec(64, 0xAAA);
        let b = f026_vec(64, 0xBBB);
        let result = try_compute_distance(&a, &b, DistanceMetric::MaxSim);
        assert_eq!(result, Err(MetricError::MaxSimRequiresTokenLevel));
    }

    #[test]
    fn test_f026_try_compute_distance_happy_path_matches_panic_helper() {
        let a = f026_vec(128, 0x5_a_d_b_e_e_f);
        let b = f026_vec(128, 0x5_a_d_b_e_e_f);
        let try_val =
            try_compute_distance(&a, &b, DistanceMetric::Cosine).expect("identical inputs valid");
        let panic_val = compute_distance(&a, &b, DistanceMetric::Cosine);
        assert!(
            (try_val - panic_val).abs() < 1e-5,
            "try={try_val} panic={panic_val}"
        );
    }

    #[test]
    fn test_f026_try_cosine_similarity_dim_mismatch_returns_err() {
        let a = f026_vec(32, 0x1234);
        let b = f026_vec(64, 0x1234);
        match try_cosine_similarity(&a, &b) {
            Err(MetricError::DimensionMismatch { left, right }) => {
                assert_eq!(left, 32);
                assert_eq!(right, 64);
            }
            other => panic!("expected DimensionMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_f026_try_cosine_similarity_empty_returns_err() {
        let result = try_cosine_similarity(&[], &[]);
        assert_eq!(result, Err(MetricError::EmptyVectors));
    }

    #[test]
    fn test_f026_try_cosine_similarity_zero_norm_returns_zero() {
        // Zero-norm is a valid runtime state (per STOR-M1 rejection at
        // insertion); return 0.0 not an error variant.
        let zero = vec![0.0_f32; 128];
        let nonzero = f026_vec(128, 0xC0DE);
        let result = try_cosine_similarity(&zero, &nonzero);
        assert_eq!(result, Ok(0.0));
    }

    #[test]
    fn test_f026_try_cosine_similarity_happy_path() {
        let a = f026_vec(256, 0x99);
        let b = a.clone();
        let result = try_cosine_similarity(&a, &b).expect("identical inputs valid");
        assert!(
            (result - 1.0).abs() < 1e-5,
            "identical cosine ≈ 1.0, got {result}"
        );
    }

    #[test]
    fn test_f026_try_distance_to_similarity_maxsim_returns_err() {
        let result = try_distance_to_similarity(0.5, DistanceMetric::MaxSim);
        assert_eq!(result, Err(MetricError::MaxSimRequiresTokenLevel));
    }

    #[test]
    fn test_f026_metric_error_variants_use_screaming_snake_case() {
        let oob = MetricError::IndexOutOfRange { embedder_idx: 99 };
        let mismatch = MetricError::DimensionMismatch { left: 1, right: 2 };
        let empty = MetricError::EmptyVectors;
        let inverted = MetricError::InvertedIndexEmbedder {
            embedder: EmbedderIndex::E6Sparse,
        };
        let maxsim = MetricError::MaxSimRequiresTokenLevel;
        assert!(format!("{oob}").contains("METRIC_INDEX_OUT_OF_RANGE"));
        assert!(format!("{mismatch}").contains("METRIC_DIMENSION_MISMATCH"));
        assert!(format!("{empty}").contains("METRIC_EMPTY_VECTORS"));
        assert!(format!("{inverted}").contains("METRIC_INVERTED_INDEX_EMBEDDER"));
        assert!(format!("{maxsim}").contains("METRIC_MAXSIM_REQUIRES_TOKEN_LEVEL"));
    }
}
