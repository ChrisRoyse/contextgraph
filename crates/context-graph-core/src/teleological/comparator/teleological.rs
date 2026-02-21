//! TeleologicalComparator: Core comparator for teleological fingerprints.
//!
//! Routes to correct similarity function per embedder type, applies weights
//! and synergy matrices, returns detailed comparison results.

use std::collections::HashMap;

use crate::similarity::{cosine_similarity, max_sim, DenseSimilarityError};
use crate::teleological::{
    ComparisonValidationResult, Embedder, GroupType, MatrixSearchConfig, SearchStrategy,
    SimilarityBreakdown, NUM_EMBEDDERS,
};
use crate::types::{EmbeddingSlice, SemanticFingerprint};

use super::result::ComparisonResult;
use super::strategies::AggregationStrategies;

/// Compares teleological fingerprints using configurable strategies.
///
/// # Apples-to-Apples Guarantee
///
/// Each embedder's output is compared only with the same embedder's output
/// from another fingerprint. Cross-embedder comparison is FORBIDDEN per ARCH-02.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::teleological::{TeleologicalComparator, MatrixSearchConfig};
/// use context_graph_core::types::SemanticFingerprint;
///
/// let comparator = TeleologicalComparator::new();
/// let result = comparator.compare(&fingerprint_a, &fingerprint_b)?;
/// println!("Overall similarity: {:.4}", result.overall);
/// ```
#[derive(Debug, Clone)]
pub struct TeleologicalComparator {
    pub(crate) config: MatrixSearchConfig,
}

impl Default for TeleologicalComparator {
    fn default() -> Self {
        Self::new()
    }
}

impl AggregationStrategies for TeleologicalComparator {
    fn config(&self) -> &MatrixSearchConfig {
        &self.config
    }
}

impl TeleologicalComparator {
    /// Create with default configuration (Cosine strategy, Full scope).
    pub fn new() -> Self {
        Self {
            config: MatrixSearchConfig::default(),
        }
    }

    /// Create with specific configuration.
    pub fn with_config(config: MatrixSearchConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &MatrixSearchConfig {
        &self.config
    }

    /// Compare two fingerprints using configured strategy.
    ///
    /// # Errors
    ///
    /// Returns error if weights are invalid (FAIL FAST per constitution.yaml).
    pub fn compare(
        &self,
        a: &SemanticFingerprint,
        b: &SemanticFingerprint,
    ) -> ComparisonValidationResult<ComparisonResult> {
        self.compare_with_strategy(a, b, self.config.strategy)
    }

    /// Compare with explicit strategy override.
    pub fn compare_with_strategy(
        &self,
        a: &SemanticFingerprint,
        b: &SemanticFingerprint,
        strategy: SearchStrategy,
    ) -> ComparisonValidationResult<ComparisonResult> {
        // FAIL FAST: Validate weights before any computation
        self.config.weights.validate()?;

        let mut result = ComparisonResult::new(strategy);

        // Compare each embedder (apples-to-apples)
        for idx in 0..NUM_EMBEDDERS {
            let a_slice = a.get_embedding(idx);
            let b_slice = b.get_embedding(idx);

            if let (Some(a_emb), Some(b_emb)) = (a_slice, b_slice) {
                result.per_embedder[idx] = self.compare_embedder_slices(&a_emb, &b_emb);
            }
        }

        // Aggregate scores according to strategy
        result.overall = self.aggregate(&result.per_embedder, strategy);

        // Compute coherence measure
        result.coherence = compute_coherence(&result.per_embedder);

        // Find dominant embedder
        result.dominant_embedder = find_dominant_embedder(&result.per_embedder);

        // Generate breakdown if requested
        if self.config.compute_breakdown {
            result.breakdown = Some(generate_breakdown(&result, strategy));
        }

        Ok(result)
    }

    /// Compare a single embedder pair using the correct similarity function.
    ///
    /// Routes based on EmbeddingSlice variant:
    /// - Dense: cosine_similarity → (raw + 1) / 2 normalization
    /// - Sparse: jaccard_similarity (index overlap, per constitution E6 jaccard_threshold)
    /// - TokenLevel: max_sim
    fn compare_embedder_slices(
        &self,
        a: &EmbeddingSlice<'_>,
        b: &EmbeddingSlice<'_>,
    ) -> Option<f32> {
        match (a, b) {
            // Dense embeddings (E1-E5, E7-E11): cosine similarity
            (EmbeddingSlice::Dense(a_dense), EmbeddingSlice::Dense(b_dense)) => {
                // Skip empty vectors
                if a_dense.is_empty() || b_dense.is_empty() {
                    return None;
                }
                // cosine_similarity returns Result, handle error by returning None
                match cosine_similarity(a_dense, b_dense) {
                    Ok(sim) => Some((sim + 1.0) / 2.0),
                    Err(DenseSimilarityError::DimensionMismatch { .. }) => {
                        // Dimension mismatch between same embedder type - should not happen
                        // with valid fingerprints, but handle gracefully
                        None
                    }
                    Err(DenseSimilarityError::EmptyVector) => None,
                    Err(DenseSimilarityError::ZeroMagnitude) => {
                        // Zero vector - treat as no similarity
                        Some(0.0)
                    }
                }
            }

            // Sparse embeddings (E6, E13): Jaccard similarity (per constitution E6 jaccard_threshold)
            (EmbeddingSlice::Sparse(a_sparse), EmbeddingSlice::Sparse(b_sparse)) => {
                // Skip empty sparse vectors (no meaningful comparison possible)
                if a_sparse.nnz() == 0 || b_sparse.nnz() == 0 {
                    return None;
                }
                Some(a_sparse.jaccard_similarity(b_sparse))
            }

            // Token-level embeddings (E12): ColBERT MaxSim
            (EmbeddingSlice::TokenLevel(a_tokens), EmbeddingSlice::TokenLevel(b_tokens)) => {
                // Skip empty token sequences (no meaningful comparison possible)
                if a_tokens.is_empty() || b_tokens.is_empty() {
                    return None;
                }
                let sim = max_sim(a_tokens, b_tokens);
                Some(f32::clamp(sim, 0.0, 1.0))
            }

            // Type mismatch - should never happen with valid fingerprints
            // This would indicate a bug in SemanticFingerprint.get_embedding()
            _ => None,
        }
    }
}

/// Compute coherence measure across embedders.
/// Higher coherence = more consistent scores = more confident result.
pub(crate) fn compute_coherence(scores: &[Option<f32>; NUM_EMBEDDERS]) -> Option<f32> {
    let valid_scores: Vec<f32> = scores.iter().filter_map(|&s| s).collect();
    if valid_scores.len() < 2 {
        return None; // Need at least 2 scores for coherence
    }

    let n = valid_scores.len() as f32;
    let mean: f32 = valid_scores.iter().sum::<f32>() / n;

    if mean < f32::EPSILON {
        return Some(0.0); // All zeros = no coherence information
    }

    let variance: f32 = valid_scores
        .iter()
        .map(|&s| (s - mean).powi(2))
        .sum::<f32>()
        / n;
    let std_dev = variance.sqrt();
    let cov = std_dev / mean;

    // Coherence = 1 / (1 + CoV)
    Some(1.0 / (1.0 + cov))
}

/// Find the embedder with the highest similarity score.
pub(crate) fn find_dominant_embedder(scores: &[Option<f32>; NUM_EMBEDDERS]) -> Option<Embedder> {
    let mut max_score = f32::NEG_INFINITY;
    let mut max_idx = None;

    for (idx, score) in scores.iter().enumerate() {
        if let Some(s) = score {
            if *s > max_score {
                max_score = *s;
                max_idx = Some(idx);
            }
        }
    }

    max_idx.and_then(Embedder::from_index)
}

/// Generate detailed breakdown for the comparison.
pub(crate) fn generate_breakdown(
    result: &ComparisonResult,
    strategy: SearchStrategy,
) -> SimilarityBreakdown {
    let mut breakdown = SimilarityBreakdown {
        overall: result.overall,
        topic_profile: result.overall, // Simplified: use overall as topic
        cross_correlations: 0.0,
        group_alignments: 0.0,
        per_group: HashMap::new(),
        per_embedder_topic: [0.0; NUM_EMBEDDERS],
        top_correlation_pairs: Vec::new(),
        strategy_used: strategy,
    };

    // Fill per-embedder scores
    for (idx, score) in result.per_embedder.iter().enumerate() {
        breakdown.per_embedder_topic[idx] = score.unwrap_or(0.0);
    }

    // Calculate group scores
    let group_map: [(GroupType, &[usize]); 6] = [
        (GroupType::Factual, &[0, 10, 11, 12]),
        (GroupType::Temporal, &[1, 2, 3]),
        (GroupType::Causal, &[4, 6]),
        (GroupType::Relational, &[7, 8]),
        (GroupType::Qualitative, &[9]),
        (GroupType::Implementation, &[5]),
    ];

    for (group_type, indices) in group_map {
        let group_scores: Vec<f32> = indices
            .iter()
            .filter_map(|&i| result.per_embedder[i])
            .collect();

        if !group_scores.is_empty() {
            let avg = group_scores.iter().sum::<f32>() / group_scores.len() as f32;
            breakdown.per_group.insert(group_type, avg);
        }
    }

    // Calculate group alignments average
    if !breakdown.per_group.is_empty() {
        breakdown.group_alignments =
            breakdown.per_group.values().sum::<f32>() / breakdown.per_group.len() as f32;
    }

    breakdown
}
