//! Token-level (ColBERT MaxSim) similarity functions.
//!
//! Implements MaxSim scoring for E12 late-interaction embeddings:
//! `score = (1/|Q|) × Σᵢ max_j cos(q_i, d_j)`
//!
//! # Example
//! ```
//! use context_graph_core::similarity::token_level::{max_sim, symmetric_max_sim};
//!
//! // 2 query tokens, 3 doc tokens, each 128D
//! let query: Vec<Vec<f32>> = vec![vec![1.0; 128], vec![0.5; 128]];
//! let document: Vec<Vec<f32>> = vec![vec![1.0; 128], vec![0.0; 128], vec![0.5; 128]];
//!
//! let score = max_sim(&query, &document);
//! assert!(score >= 0.0 && score <= 1.0);
//! ```

use crate::similarity::dense::cosine_similarity;
use crate::types::fingerprint::E12_TOKEN_DIM;

/// Calculate MaxSim score: normalized sum of max similarities per query token.
///
/// For each query token q_i, finds max_j cos(q_i, d_j) across all document tokens,
/// then returns the average: (1/|Q|) × Σᵢ max_j cos(q_i, d_j)
///
/// # Arguments
/// * `query` - Query token embeddings, each must be 128D
/// * `document` - Document token embeddings, each must be 128D
///
/// # Returns
/// MaxSim score in [0.0, 1.0] range
///
/// # Panics
/// Panics if any token embedding dimension != 128
#[inline]
pub fn max_sim(query: &[Vec<f32>], document: &[Vec<f32>]) -> f32 {
    // Handle empty sequences
    if query.is_empty() || document.is_empty() {
        return 0.0;
    }

    // Validate dimensions - FAIL FAST
    for (i, q) in query.iter().enumerate() {
        assert_eq!(
            q.len(),
            E12_TOKEN_DIM,
            "Query token {} has dimension {}, expected {}",
            i,
            q.len(),
            E12_TOKEN_DIM
        );
    }
    for (i, d) in document.iter().enumerate() {
        assert_eq!(
            d.len(),
            E12_TOKEN_DIM,
            "Document token {} has dimension {}, expected {}",
            i,
            d.len(),
            E12_TOKEN_DIM
        );
    }

    let mut total_max_sim = 0.0f32;

    for q_token in query {
        let mut max_sim_for_token = f32::NEG_INFINITY;

        for d_token in document {
            // cosine_similarity returns Result, unwrap since we validated dimensions
            let sim = cosine_similarity(q_token, d_token)
                .expect("Dimension mismatch should have been caught by validation");

            if sim > max_sim_for_token {
                max_sim_for_token = sim;
            }
        }

        // Only add if we found a valid similarity
        if max_sim_for_token.is_finite() {
            total_max_sim += max_sim_for_token;
        }
    }

    // Normalize by query length
    total_max_sim / query.len() as f32
}

/// Calculate symmetric MaxSim: average of both directions.
///
/// `sym_max_sim(A, B) = (max_sim(A, B) + max_sim(B, A)) / 2`
///
/// Useful when neither sequence is clearly the "query" vs "document".
#[inline]
pub fn symmetric_max_sim(a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
    let ab = max_sim(a, b);
    let ba = max_sim(b, a);
    (ab + ba) / 2.0
}

/// Calculate MaxSim with early termination for approximate matching.
///
/// Stops processing document tokens for a query token once similarity
/// exceeds `min_score_threshold`. This provides speedup when we only
/// need to know if similarity is "good enough".
///
/// # Arguments
/// * `query` - Query token embeddings
/// * `document` - Document token embeddings
/// * `min_score_threshold` - Stop searching once this threshold is exceeded
///
/// # Returns
/// Approximate MaxSim score (may be slightly lower than exact)
#[inline]
pub fn approximate_max_sim(
    query: &[Vec<f32>],
    document: &[Vec<f32>],
    min_score_threshold: f32,
) -> f32 {
    if query.is_empty() || document.is_empty() {
        return 0.0;
    }

    // Validate dimensions - FAIL FAST
    for (i, q) in query.iter().enumerate() {
        assert_eq!(
            q.len(),
            E12_TOKEN_DIM,
            "Query token {} has dimension {}, expected {}",
            i,
            q.len(),
            E12_TOKEN_DIM
        );
    }
    for (i, d) in document.iter().enumerate() {
        assert_eq!(
            d.len(),
            E12_TOKEN_DIM,
            "Document token {} has dimension {}, expected {}",
            i,
            d.len(),
            E12_TOKEN_DIM
        );
    }

    let mut total = 0.0f32;

    for q_token in query {
        let mut max_sim = f32::NEG_INFINITY;

        for d_token in document {
            let sim = cosine_similarity(q_token, d_token)
                .expect("Dimension mismatch should have been caught");

            if sim > max_sim {
                max_sim = sim;
                // Early termination if threshold exceeded
                if max_sim >= min_score_threshold {
                    break;
                }
            }
        }

        if max_sim.is_finite() {
            total += max_sim;
        }
    }

    total / query.len() as f32
}

/// Result of token alignment analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenAlignment {
    /// Index of the query token
    pub query_token_idx: usize,
    /// Index of the best-matching document token
    pub doc_token_idx: usize,
    /// Cosine similarity between the matched tokens
    pub similarity: f32,
}

/// Get detailed token alignments for interpretability.
///
/// Returns one `TokenAlignment` per query token, showing which document
/// token had the highest similarity.
///
/// # Example
/// ```
/// use context_graph_core::similarity::token_level::token_alignments;
///
/// let query = vec![vec![1.0; 128]];
/// let document = vec![vec![1.0; 128], vec![0.0; 128]];
///
/// let alignments = token_alignments(&query, &document);
/// assert_eq!(alignments.len(), 1);
/// assert_eq!(alignments[0].doc_token_idx, 0); // First doc token matches best
/// ```
#[inline]
pub fn token_alignments(
    query: &[Vec<f32>],
    document: &[Vec<f32>],
) -> Vec<TokenAlignment> {
    if query.is_empty() || document.is_empty() {
        return Vec::new();
    }

    // Validate dimensions - FAIL FAST
    for (i, q) in query.iter().enumerate() {
        assert_eq!(
            q.len(),
            E12_TOKEN_DIM,
            "Query token {} has dimension {}, expected {}",
            i,
            q.len(),
            E12_TOKEN_DIM
        );
    }
    for (i, d) in document.iter().enumerate() {
        assert_eq!(
            d.len(),
            E12_TOKEN_DIM,
            "Document token {} has dimension {}, expected {}",
            i,
            d.len(),
            E12_TOKEN_DIM
        );
    }

    let mut alignments = Vec::with_capacity(query.len());

    for (q_idx, q_token) in query.iter().enumerate() {
        let mut best_d_idx = 0;
        let mut best_sim = f32::NEG_INFINITY;

        for (d_idx, d_token) in document.iter().enumerate() {
            let sim = cosine_similarity(q_token, d_token)
                .expect("Dimension mismatch should have been caught");

            if sim > best_sim {
                best_sim = sim;
                best_d_idx = d_idx;
            }
        }

        alignments.push(TokenAlignment {
            query_token_idx: q_idx,
            doc_token_idx: best_d_idx,
            similarity: if best_sim.is_finite() { best_sim } else { 0.0 },
        });
    }

    alignments
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create normalized 128D token embedding
    fn make_token(values: &[f32]) -> Vec<f32> {
        let mut token = vec![0.0; E12_TOKEN_DIM];
        for (i, &v) in values.iter().enumerate() {
            if i < E12_TOKEN_DIM {
                token[i] = v;
            }
        }
        // Normalize
        let norm: f32 = token.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut token {
                *x /= norm;
            }
        }
        token
    }

    // ============================================
    // SYNTHETIC TEST DATA WITH KNOWN OUTPUTS
    // ============================================

    #[test]
    fn test_maxsim_identical_tokens() {
        // SYNTHETIC DATA: Two identical unit vectors
        // Expected: MaxSim = 1.0 (perfect match)
        let token = make_token(&[1.0, 0.0, 0.0]);
        let query = vec![token.clone()];
        let document = vec![token];

        let score = max_sim(&query, &document);

        assert!(
            (score - 1.0).abs() < 1e-5,
            "Identical tokens should have MaxSim = 1.0, got {}",
            score
        );
    }

    #[test]
    fn test_maxsim_orthogonal_tokens() {
        // SYNTHETIC DATA: Orthogonal unit vectors
        // Expected: MaxSim = 0.0 (no similarity)
        let q_token = make_token(&[1.0, 0.0, 0.0]);
        let d_token = make_token(&[0.0, 1.0, 0.0]);

        let query = vec![q_token];
        let document = vec![d_token];

        let score = max_sim(&query, &document);

        assert!(
            score.abs() < 1e-5,
            "Orthogonal tokens should have MaxSim = 0.0, got {}",
            score
        );
    }

    #[test]
    fn test_maxsim_known_calculation() {
        // SYNTHETIC DATA: 2 query tokens, 2 doc tokens with known cosines
        // q0 = [1, 0, 0, ...] (normalized)
        // q1 = [0, 1, 0, ...] (normalized)
        // d0 = [1, 0, 0, ...] (normalized) -> cos(q0, d0) = 1.0, cos(q1, d0) = 0.0
        // d1 = [0.707, 0.707, 0, ...] (normalized) -> cos(q0, d1) ≈ 0.707, cos(q1, d1) ≈ 0.707

        let q0 = make_token(&[1.0, 0.0]);
        let q1 = make_token(&[0.0, 1.0]);
        let d0 = make_token(&[1.0, 0.0]);
        let d1 = make_token(&[0.707, 0.707]);

        let query = vec![q0, q1];
        let document = vec![d0, d1];

        let score = max_sim(&query, &document);

        // For q0: max(cos(q0,d0)=1.0, cos(q0,d1)≈0.707) = 1.0
        // For q1: max(cos(q1,d0)=0.0, cos(q1,d1)≈0.707) ≈ 0.707
        // MaxSim = (1.0 + 0.707) / 2 ≈ 0.854
        let expected = (1.0 + 0.707) / 2.0;

        assert!(
            (score - expected).abs() < 0.05,
            "Expected MaxSim ≈ {}, got {}",
            expected,
            score
        );
    }

    #[test]
    fn test_symmetric_maxsim() {
        // SYNTHETIC DATA: Asymmetric token sets
        let a = vec![make_token(&[1.0, 0.0])];
        let b = vec![make_token(&[0.707, 0.707]), make_token(&[0.0, 1.0])];

        let sym = symmetric_max_sim(&a, &b);
        let ab = max_sim(&a, &b);
        let ba = max_sim(&b, &a);

        let expected = (ab + ba) / 2.0;

        assert!(
            (sym - expected).abs() < 1e-5,
            "Symmetric MaxSim should be average of both directions"
        );
    }

    #[test]
    fn test_approximate_within_tolerance() {
        // SYNTHETIC DATA: Test that approximate is within 5% of exact
        // Use values starting from 1.0 to avoid zero-magnitude vectors
        let query: Vec<Vec<f32>> = (0..5).map(|i| make_token(&[(i as f32 + 1.0), (5.0 - i as f32 + 1.0)])).collect();
        let document: Vec<Vec<f32>> = (0..10).map(|i| make_token(&[(i as f32 + 1.0), (i as f32 + 1.0)])).collect();

        let exact = max_sim(&query, &document);
        let approx = approximate_max_sim(&query, &document, 0.8);

        let diff = (exact - approx).abs();
        let tolerance = exact * 0.05;

        assert!(
            diff <= tolerance || approx >= exact * 0.95,
            "Approximate {} should be within 5% of exact {}, diff = {}",
            approx,
            exact,
            diff
        );
    }

    #[test]
    fn test_token_alignments_correctness() {
        // SYNTHETIC DATA: Known alignment
        // q0 should align with d0 (identical)
        // q1 should align with d2 (most similar)
        let q0 = make_token(&[1.0, 0.0, 0.0]);
        let q1 = make_token(&[0.0, 0.0, 1.0]);

        let d0 = make_token(&[1.0, 0.0, 0.0]); // Identical to q0
        let d1 = make_token(&[0.0, 1.0, 0.0]); // Orthogonal to both
        let d2 = make_token(&[0.0, 0.0, 1.0]); // Identical to q1

        let query = vec![q0, q1];
        let document = vec![d0, d1, d2];

        let alignments = token_alignments(&query, &document);

        assert_eq!(alignments.len(), 2);
        assert_eq!(alignments[0].query_token_idx, 0);
        assert_eq!(alignments[0].doc_token_idx, 0, "q0 should align with d0");
        assert!((alignments[0].similarity - 1.0).abs() < 1e-5);

        assert_eq!(alignments[1].query_token_idx, 1);
        assert_eq!(alignments[1].doc_token_idx, 2, "q1 should align with d2");
        assert!((alignments[1].similarity - 1.0).abs() < 1e-5);
    }

    // ============================================
    // EDGE CASES - FAIL FAST BEHAVIOR
    // ============================================

    #[test]
    fn test_empty_query_returns_zero() {
        let query: Vec<Vec<f32>> = vec![];
        let document = vec![make_token(&[1.0])];

        let score = max_sim(&query, &document);
        assert_eq!(score, 0.0, "Empty query should return 0.0");
    }

    #[test]
    fn test_empty_document_returns_zero() {
        let query = vec![make_token(&[1.0])];
        let document: Vec<Vec<f32>> = vec![];

        let score = max_sim(&query, &document);
        assert_eq!(score, 0.0, "Empty document should return 0.0");
    }

    #[test]
    fn test_empty_alignments() {
        let query: Vec<Vec<f32>> = vec![];
        let document = vec![make_token(&[1.0])];

        let alignments = token_alignments(&query, &document);
        assert!(alignments.is_empty(), "Empty query should return no alignments");
    }

    #[test]
    #[should_panic(expected = "dimension")]
    fn test_wrong_dimension_panics() {
        // FAIL FAST: Wrong dimension should panic
        let query = vec![vec![1.0; 64]]; // Wrong dimension!
        let document = vec![vec![1.0; E12_TOKEN_DIM]];

        let _ = max_sim(&query, &document);
    }

    #[test]
    #[should_panic(expected = "dimension")]
    fn test_mismatched_dimensions_panics() {
        // FAIL FAST: Mismatched dimensions should panic
        let query = vec![vec![1.0; E12_TOKEN_DIM]];
        let document = vec![vec![1.0; 256]]; // Wrong dimension!

        let _ = max_sim(&query, &document);
    }

    // ============================================
    // SCORE RANGE VALIDATION
    // ============================================

    #[test]
    fn test_score_range() {
        // Generate random-ish tokens and verify score is in [0, 1]
        let query: Vec<Vec<f32>> = (0..3)
            .map(|i| make_token(&[(i as f32 + 1.0), (3.0 - i as f32)]))
            .collect();
        let document: Vec<Vec<f32>> = (0..5)
            .map(|i| make_token(&[(i as f32), (i as f32 + 1.0)]))
            .collect();

        let score = max_sim(&query, &document);

        assert!(
            (0.0..=1.0).contains(&score),
            "MaxSim score {} should be in [0.0, 1.0]",
            score
        );
    }
}
