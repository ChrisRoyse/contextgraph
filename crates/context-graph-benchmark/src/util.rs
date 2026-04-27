//! Shared utility functions for benchmarking.

use std::cmp::Ordering;
use uuid::Uuid;

/// Compute cosine similarity between two vectors, normalized to [0, 1].
///
/// Normalized to [0, 1] to match production (SRC-3 normalization):
/// `(raw + 1.0) / 2.0` where raw is in [-1, 1].
///
/// Returns 0.0 if vectors have different lengths or either has zero norm.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let raw = cosine_similarity_raw(a, b);
    // Normalized to [0, 1] to match production (SRC-3 normalization)
    (raw + 1.0) / 2.0
}

/// Compute raw cosine similarity between two vectors, returning [-1, 1].
///
/// Use this when you need the raw cosine value (e.g., for comparison with
/// external tools that expect the standard mathematical cosine range).
///
/// Returns 0.0 if vectors have different lengths or either has zero norm.
pub fn cosine_similarity_raw(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Compute cosine similarity between two vectors with f64 precision, normalized to [0, 1].
///
/// Normalized to [0, 1] to match production (SRC-3 normalization).
/// Uses f64 arithmetic internally for reduced floating-point accumulation error
/// on high-dimensional vectors.
///
/// Returns 0.0 if vectors have different lengths or either has zero norm.
pub fn cosine_similarity_f64(a: &[f32], b: &[f32]) -> f64 {
    let raw = cosine_similarity_raw_f64(a, b);
    // Normalized to [0, 1] to match production (SRC-3 normalization)
    (raw + 1.0) / 2.0
}

/// Compute raw cosine similarity between two vectors with f64 precision, returning [-1, 1].
///
/// Returns 0.0 if vectors have different lengths or either has zero norm.
pub fn cosine_similarity_raw_f64(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Sort comparator for (Uuid, f32) pairs by similarity descending.
///
/// Uses UUID as tiebreaker for deterministic ordering when similarities are equal.
pub fn similarity_sort_desc(a: &(Uuid, f32), b: &(Uuid, f32)) -> Ordering {
    match b.1.partial_cmp(&a.1) {
        Some(Ordering::Equal) | None => a.0.cmp(&b.0),
        Some(ord) => ord,
    }
}

// To run benchmark tests: cargo test -p context-graph-benchmark --features benchmark-tests
#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        // Identical vectors: raw=1.0, normalized=(1+1)/2=1.0
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];
        // Orthogonal vectors: raw=0.0, normalized=(0+1)/2=0.5
        assert!((cosine_similarity(&a, &c) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        // Opposite vectors: raw=-1.0, normalized=(-1+1)/2=0.0
        assert!(cosine_similarity(&a, &b).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        // Different lengths: raw=0.0, normalized=(0+1)/2=0.5
        assert_eq!(cosine_similarity(&a, &b), 0.5);
    }

    #[test]
    fn test_cosine_similarity_raw_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity_raw(&a, &b) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_raw_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity_raw(&a, &c).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_f64_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity_f64(&a, &b) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_sort_desc() {
        let id1 = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let id2 = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

        let mut items = vec![(id2, 0.5), (id1, 0.8), (id1, 0.5)];
        items.sort_by(similarity_sort_desc);

        // Should be: (id1, 0.8), (id1, 0.5), (id2, 0.5) - highest sim first, then UUID tiebreaker
        assert_eq!(items[0].0, id1);
        assert_eq!(items[0].1, 0.8);
    }
}
