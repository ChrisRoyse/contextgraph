//! Cross-Space Similarity Engine trait definition.
//!
//! This module defines the `CrossSpaceSimilarityEngine` trait for computing
//! unified similarity scores across 13 embedding spaces.

use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::fingerprint::{TeleologicalFingerprint, NUM_EMBEDDERS};
use super::config::{CrossSpaceConfig, WeightingStrategy};
use super::error::SimilarityError;
use super::explanation::SimilarityExplanation;
use super::multi_utl::MultiUtlParams;
use super::result::CrossSpaceSimilarity;

/// Engine for computing cross-space similarity across 13 embedding spaces.
///
/// # Primary Strategy: RRF
///
/// Per constitution.yaml, RRF (Reciprocal Rank Fusion) with k=60 is the
/// primary aggregation strategy:
///
/// ```text
/// RRF(d) = SUM_i 1/(k + rank_i(d) + 1) where k=60
/// ```
///
/// # Performance Requirements
///
/// From constitution.yaml:
/// - Pair similarity: **<5ms**
/// - Batch 100: **<50ms**
/// - RRF fusion: **<2ms** per 1000 candidates
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` for concurrent access.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::similarity::{
///     CrossSpaceSimilarityEngine, DefaultCrossSpaceEngine, CrossSpaceConfig,
/// };
///
/// let engine = DefaultCrossSpaceEngine::new();
/// let config = CrossSpaceConfig::default();
///
/// let result = engine.compute_similarity(&fp1, &fp2, &config).await?;
/// println!("Similarity: {:.4}", result.score);
/// ```
#[async_trait]
pub trait CrossSpaceSimilarityEngine: Send + Sync {
    /// Compute similarity between two teleological fingerprints.
    ///
    /// # Arguments
    ///
    /// - `fp1`: First fingerprint (typically query)
    /// - `fp2`: Second fingerprint (typically candidate)
    /// - `config`: Computation configuration
    ///
    /// # Returns
    ///
    /// `CrossSpaceSimilarity` with aggregated score and optional breakdown.
    ///
    /// # Errors
    ///
    /// - `SimilarityError::InsufficientSpaces` if fewer than `min_active_spaces`
    /// - `SimilarityError::DimensionMismatch` if embeddings have wrong dimensions
    /// - `SimilarityError::ZeroNormVector` if either vector has zero norm
    ///
    /// # Performance
    ///
    /// `Constraint: <5ms per pair`
    async fn compute_similarity(
        &self,
        fp1: &TeleologicalFingerprint,
        fp2: &TeleologicalFingerprint,
        config: &CrossSpaceConfig,
    ) -> Result<CrossSpaceSimilarity, SimilarityError>;

    /// Compute similarity for a batch of candidates.
    ///
    /// More efficient than calling `compute_similarity` in a loop due to
    /// potential SIMD/parallelization optimizations.
    ///
    /// # Arguments
    ///
    /// - `query`: Query fingerprint
    /// - `candidates`: Slice of candidate fingerprints
    /// - `config`: Computation configuration
    ///
    /// # Returns
    ///
    /// Vector of similarity results, one per candidate.
    ///
    /// # Errors
    ///
    /// Returns `SimilarityError::BatchError` if any individual computation fails,
    /// wrapping the original error with the candidate index.
    ///
    /// # Performance
    ///
    /// `Constraint: <50ms for 100 candidates`
    async fn compute_batch(
        &self,
        query: &TeleologicalFingerprint,
        candidates: &[TeleologicalFingerprint],
        config: &CrossSpaceConfig,
    ) -> Result<Vec<CrossSpaceSimilarity>, SimilarityError>;

    /// Compute RRF (Reciprocal Rank Fusion) from pre-ranked lists.
    ///
    /// Use this when you have pre-computed per-space ranked results from
    /// HNSW indexes or other ANN search.
    ///
    /// # Formula
    ///
    /// ```text
    /// RRF(d) = SUM_i 1/(k + rank_i(d) + 1)
    /// ```
    ///
    /// # Arguments
    ///
    /// - `ranked_lists`: Vector of (space_index, sorted_memory_ids)
    /// - `k`: RRF constant (default 60.0 per constitution.yaml)
    ///
    /// # Returns
    ///
    /// HashMap of memory_id -> RRF score.
    ///
    /// # Note
    ///
    /// This method MUST use the existing `AggregationStrategy::aggregate_rrf`
    /// implementation from `crate::retrieval::aggregation`. Do NOT reimplement.
    ///
    /// # Performance
    ///
    /// `Constraint: <2ms per 1000 candidates`
    fn compute_rrf_from_ranks(
        &self,
        ranked_lists: &[(usize, Vec<Uuid>)],
        k: f32,
    ) -> HashMap<Uuid, f32>;

    /// Compute Multi-UTL score for advanced semantic learning.
    ///
    /// # Formula
    ///
    /// From constitution.yaml:
    /// ```text
    /// L_multi = sigmoid(2.0 * (SUM_i tau_i * lambda_S * Delta_S_i) *
    ///                          (SUM_j tau_j * lambda_C * Delta_C_j) *
    ///                          w_e * cos(phi))
    /// ```
    ///
    /// # Arguments
    ///
    /// - `params`: Multi-UTL parameters including deltas, weights, and phase
    ///
    /// # Returns
    ///
    /// Learning score in range (0.0, 1.0) via sigmoid transformation.
    async fn compute_multi_utl(&self, params: &MultiUtlParams) -> f32;

    /// Generate human-readable explanation of a similarity result.
    ///
    /// # Arguments
    ///
    /// - `result`: A previously computed similarity result
    ///
    /// # Returns
    ///
    /// `SimilarityExplanation` with summary, space details, and recommendations.
    fn explain(&self, result: &CrossSpaceSimilarity) -> SimilarityExplanation;

    /// Get weights for a given weighting strategy.
    ///
    /// # Arguments
    ///
    /// - `strategy`: The weighting strategy to get weights for
    ///
    /// # Returns
    ///
    /// Array of 13 weights, one per embedding space.
    /// For non-weight-based strategies (like RRF), returns uniform weights.
    fn get_weights(&self, strategy: &WeightingStrategy) -> [f32; NUM_EMBEDDERS];
}
