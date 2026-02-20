//! Default implementation of CrossSpaceSimilarityEngine.
//!
//! This module provides `DefaultCrossSpaceEngine` which implements the
//! `CrossSpaceSimilarityEngine` trait for computing similarity across
//! 13 embedding spaces.
//!
//! # Key Features
//!
//! - Uses existing `AggregationStrategy::aggregate_rrf` (no reimplementation)
//! - Supports all `WeightingStrategy` variants
//! - Thread-safe (`Send + Sync`)
//! - Deterministic output for same inputs
//!
//! # Performance
//!
//! Per constitution.yaml:
//! - Pair similarity: <5ms
//! - Batch 100: <50ms
//! - RRF fusion: <2ms per 1000 candidates

use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

use crate::retrieval::AggregationStrategy;
use crate::types::fingerprint::{
    EmbeddingSlice, SparseVector, TeleologicalFingerprint, NUM_EMBEDDERS,
};

use super::config::{CrossSpaceConfig, MissingSpaceHandling, WeightingStrategy};
use super::engine::CrossSpaceSimilarityEngine;
use super::error::SimilarityError;
use super::explanation::{ScoreInterpretation, SimilarityExplanation, SpaceDetail, SPACE_NAMES};
use super::multi_utl::MultiUtlParams;
use super::result::CrossSpaceSimilarity;

/// Default implementation of the cross-space similarity engine.
///
/// This engine computes similarity by:
/// 1. Computing per-space similarity using cosine similarity for dense vectors
/// 2. Handling sparse vectors (E6, E13) with sparse dot product
/// 3. Handling token-level vectors (E12) with MaxSim
/// 4. Aggregating using the configured strategy (default: RRF k=60)
///
/// # Thread Safety
///
/// This struct is `Send + Sync` and can be shared across threads.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::similarity::{DefaultCrossSpaceEngine, CrossSpaceConfig};
///
/// let engine = DefaultCrossSpaceEngine::new();
/// let config = CrossSpaceConfig::default();
///
/// let result = engine.compute_similarity(&fp1, &fp2, &config).await?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct DefaultCrossSpaceEngine;

impl DefaultCrossSpaceEngine {
    /// Create a new DefaultCrossSpaceEngine.
    #[inline]
    pub fn new() -> Self {
        Self
    }

    /// Compute cosine similarity between two dense vectors.
    ///
    /// Uses SIMD (AVX2+FMA) acceleration on x86_64 when available,
    /// providing 2-4x speedup for vectors with 256+ dimensions.
    ///
    /// # Arguments
    /// - `a`: First vector slice
    /// - `b`: Second vector slice
    ///
    /// # Returns
    /// Cosine similarity in range [-1.0, 1.0].
    /// Returns 0.0 if either vector has zero norm or mismatched dimensions.
    ///
    /// # Errors
    /// Does not error; returns 0.0 for degenerate cases.
    #[inline]
    fn cosine_similarity_dense(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError> {
        // Use SIMD-accelerated version on x86_64, with fallback to scalar
        #[cfg(target_arch = "x86_64")]
        {
            super::dense::cosine_similarity_simd(a, b)
                .map_err(|e| SimilarityError::invalid_config(e.to_string()))
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            super::dense::cosine_similarity(a, b)
                .map_err(|e| SimilarityError::invalid_config(e.to_string()))
        }
    }

    /// Compute sparse cosine similarity between two sparse vectors.
    ///
    /// Uses SparseVector::cosine_similarity() for proper normalization.
    /// This ensures E6/E13 sparse embeddings produce scores in [-1, 1] range.
    ///
    /// # Important
    ///
    /// Per ARCH-12 and constitution.yaml, sparse vectors (E6/E13) must use
    /// cosine similarity, NOT raw dot product. Raw dot product is unbounded
    /// and would produce incorrect scores when normalized with (sim + 1) / 2.
    #[inline]
    fn sparse_cosine_similarity(a: &SparseVector, b: &SparseVector) -> f32 {
        // Delegate to SparseVector's proper cosine similarity implementation
        // which normalizes by L2 norms: dot(a,b) / (||a|| * ||b||)
        a.cosine_similarity(b)
    }

    /// Compute MaxSim for token-level embeddings (E12 ColBERT).
    ///
    /// Formula: MaxSim(Q, D) = SUM_i MAX_j (q_i . d_j)
    /// where q_i are query tokens and d_j are document tokens.
    #[inline]
    fn maxsim_token_level(query_tokens: &[Vec<f32>], doc_tokens: &[Vec<f32>]) -> f32 {
        if query_tokens.is_empty() || doc_tokens.is_empty() {
            return 0.0;
        }

        let mut total = 0.0f32;

        for q in query_tokens {
            let mut max_sim = f32::NEG_INFINITY;
            for d in doc_tokens {
                if q.len() == d.len() {
                    let sim = Self::dot_product(q, d);
                    if sim > max_sim {
                        max_sim = sim;
                    }
                }
            }
            if max_sim > f32::NEG_INFINITY {
                total += max_sim;
            }
        }

        // Normalize by number of query tokens
        if !query_tokens.is_empty() {
            total / query_tokens.len() as f32
        } else {
            0.0
        }
    }

    /// Simple dot product for token vectors.
    #[inline]
    fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Compute similarity between two embedding slices.
    ///
    /// Handles all three types: Dense, Sparse, TokenLevel
    ///
    /// # Space-specific handling
    ///
    /// - Dense vectors (E1, E5, E7, E8, E10, E11): Cosine similarity in [-1, 1]
    /// - Sparse vectors (E6, E13): Cosine similarity via SparseVector::cosine_similarity()
    /// - Token-level (E12): MaxSim normalized by query length
    /// - E9 (HDC projected): Returns score directly without additional normalization
    ///   (E9 projected vectors are already in [0, 1] conceptually from Hamming)
    fn compute_slice_similarity(
        slice1: &EmbeddingSlice<'_>,
        slice2: &EmbeddingSlice<'_>,
        space_idx: usize,
    ) -> Result<f32, SimilarityError> {
        match (slice1, slice2) {
            (EmbeddingSlice::Dense(a), EmbeddingSlice::Dense(b)) => {
                Self::cosine_similarity_dense(a, b)
            }
            (EmbeddingSlice::Sparse(a), EmbeddingSlice::Sparse(b)) => {
                // Use proper cosine similarity, not raw dot product
                // This ensures E6/E13 produce scores in [-1, 1] range
                Ok(Self::sparse_cosine_similarity(a, b))
            }
            (EmbeddingSlice::TokenLevel(a), EmbeddingSlice::TokenLevel(b)) => {
                Ok(Self::maxsim_token_level(a, b))
            }
            _ => {
                // Type mismatch - should not happen for same space
                Err(SimilarityError::invalid_config(format!(
                    "Embedding type mismatch in space {}",
                    space_idx
                )))
            }
        }
    }

    /// Check if an embedding slice is populated (non-empty/non-zero).
    fn is_populated(slice: &EmbeddingSlice<'_>) -> bool {
        match slice {
            EmbeddingSlice::Dense(v) => !v.is_empty() && v.iter().any(|&x| x.abs() > f32::EPSILON),
            EmbeddingSlice::Sparse(sv) => !sv.indices.is_empty(),
            EmbeddingSlice::TokenLevel(tokens) => !tokens.is_empty(),
        }
    }

    /// Compute weighted average similarity.
    fn weighted_average(scores: &[Option<f32>], weights: &[f32]) -> (f32, f32) {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;

        for (score_opt, &weight) in scores.iter().zip(weights.iter()) {
            if let Some(score) = score_opt {
                sum += score * weight;
                weight_sum += weight;
            }
        }

        if weight_sum > f32::EPSILON {
            (sum / weight_sum, sum)
        } else {
            (0.0, 0.0)
        }
    }

    /// Compute variance of scores for confidence calculation.
    fn compute_variance(scores: &[Option<f32>]) -> f32 {
        let active_scores: Vec<f32> = scores.iter().filter_map(|s| *s).collect();
        if active_scores.len() < 2 {
            return 0.0;
        }

        let mean: f32 = active_scores.iter().sum::<f32>() / active_scores.len() as f32;
        let variance: f32 = active_scores
            .iter()
            .map(|&s| (s - mean).powi(2))
            .sum::<f32>()
            / active_scores.len() as f32;

        variance
    }
}

#[async_trait]
impl CrossSpaceSimilarityEngine for DefaultCrossSpaceEngine {
    async fn compute_similarity(
        &self,
        fp1: &TeleologicalFingerprint,
        fp2: &TeleologicalFingerprint,
        config: &CrossSpaceConfig,
    ) -> Result<CrossSpaceSimilarity, SimilarityError> {
        let mut space_scores: [Option<f32>; NUM_EMBEDDERS] = [None; NUM_EMBEDDERS];
        let mut active_spaces: u16 = 0;
        let mut active_count: usize = 0;

        // Step 1: Compute per-space similarities
        for (i, score) in space_scores.iter_mut().enumerate() {
            let slice1_opt = fp1.semantic.get_embedding(i);
            let slice2_opt = fp2.semantic.get_embedding(i);

            match (slice1_opt, slice2_opt) {
                (Some(s1), Some(s2)) => {
                    // Both have embeddings - check if populated
                    if Self::is_populated(&s1) && Self::is_populated(&s2) {
                        let sim = Self::compute_slice_similarity(&s1, &s2, i)?;

                        // Clamp to valid range and check for NaN
                        if sim.is_nan() || sim.is_infinite() {
                            return Err(SimilarityError::invalid_numeric(format!(
                                "NaN/Inf in space {} similarity",
                                i
                            )));
                        }

                        // Normalize cosine to [0, 1] range: (cos + 1) / 2
                        let normalized = (sim + 1.0) / 2.0;
                        *score = Some(normalized.clamp(0.0, 1.0));
                        active_spaces |= 1 << i;
                        active_count += 1;
                    }
                }
                _ => {
                    // Handle missing based on config
                    match config.missing_space_handling {
                        MissingSpaceHandling::Skip => {
                            // Already None, skip
                        }
                        MissingSpaceHandling::ZeroFill => {
                            *score = Some(0.0);
                            // Don't count as active
                        }
                        MissingSpaceHandling::AverageFill => {
                            // Will fill later after computing active scores
                        }
                        MissingSpaceHandling::RequireAll => {
                            return Err(SimilarityError::insufficient_spaces(
                                NUM_EMBEDDERS,
                                active_count,
                            ));
                        }
                    }
                }
            }
        }

        // Step 2: Check minimum active spaces
        if active_count < config.min_active_spaces {
            return Err(SimilarityError::insufficient_spaces(
                config.min_active_spaces,
                active_count,
            ));
        }

        // Step 3: Handle AverageFill for missing spaces
        if config.missing_space_handling == MissingSpaceHandling::AverageFill && active_count > 0 {
            let avg: f32 =
                space_scores.iter().filter_map(|s| *s).sum::<f32>() / active_count as f32;

            for score in space_scores.iter_mut() {
                if score.is_none() {
                    *score = Some(avg);
                }
            }
        }

        // Step 4: Get weights based on strategy
        let weights = self.get_weights(&config.weighting_strategy);

        // Note: Topic weighting uses topic profile strengths from the fingerprint.
        // Weights are applied as-is based on the weighting strategy.

        // Step 5: Compute aggregated score
        let (score, raw_score) = Self::weighted_average(&space_scores, &weights);

        // Step 6: Compute confidence
        let variance = Self::compute_variance(&space_scores);
        let confidence = CrossSpaceSimilarity::compute_confidence(active_count as u32, variance);

        // Step 7: Build result
        let mut result = CrossSpaceSimilarity {
            score,
            raw_score,
            space_scores: if config.include_breakdown {
                Some(space_scores)
            } else {
                None
            },
            active_spaces,
            space_weights: if config.include_breakdown {
                Some(weights)
            } else {
                None
            },
            // Purpose contribution is always None now since purpose weighting was removed
            purpose_contribution: None,
            confidence,
            rrf_score: None,
        };

        // Add breakdown if requested
        if config.include_breakdown {
            result.space_scores = Some(space_scores);
            result.space_weights = Some(weights);
        }

        Ok(result)
    }

    async fn compute_batch(
        &self,
        query: &TeleologicalFingerprint,
        candidates: &[TeleologicalFingerprint],
        config: &CrossSpaceConfig,
    ) -> Result<Vec<CrossSpaceSimilarity>, SimilarityError> {
        let mut results = Vec::with_capacity(candidates.len());

        for (idx, candidate) in candidates.iter().enumerate() {
            match self.compute_similarity(query, candidate, config).await {
                Ok(sim) => results.push(sim),
                Err(e) => return Err(SimilarityError::batch_error(idx, e)),
            }
        }

        Ok(results)
    }

    fn compute_rrf_from_ranks(
        &self,
        ranked_lists: &[(usize, Vec<Uuid>)],
        k: f32,
    ) -> HashMap<Uuid, f32> {
        // REUSE existing implementation - DO NOT reimplement
        AggregationStrategy::aggregate_rrf(ranked_lists, k)
    }

    async fn compute_multi_utl(&self, params: &MultiUtlParams) -> f32 {
        // Delegate to MultiUtlParams::compute()
        params.compute()
    }

    fn explain(&self, result: &CrossSpaceSimilarity) -> SimilarityExplanation {
        let interpretation = ScoreInterpretation::from_score(result.score);
        let active_count = result.active_count();

        // Build space details if breakdown available
        let mut space_details: [Option<SpaceDetail>; NUM_EMBEDDERS] =
            [const { None }; NUM_EMBEDDERS];
        let mut key_factors = Vec::new();

        if let (Some(ref scores), Some(ref weights)) = (&result.space_scores, &result.space_weights)
        {
            for (i, (score_opt, &weight)) in scores.iter().zip(weights.iter()).enumerate() {
                if let Some(score) = score_opt {
                    let contribution = score * weight;
                    let space_interp = ScoreInterpretation::from_score(*score);

                    space_details[i] = Some(SpaceDetail {
                        space_idx: i,
                        space_name: SPACE_NAMES[i].to_string(),
                        score: *score,
                        weight,
                        contribution,
                        interpretation: format!(
                            "{} ({:.2} × {:.3} = {:.3})",
                            space_interp.label(),
                            score,
                            weight,
                            contribution
                        ),
                    });

                    // Track key factors
                    if *score >= 0.7 && weight >= 0.05 {
                        key_factors.push(format!("+ Strong {} ({:.2})", SPACE_NAMES[i], score));
                    } else if *score < 0.3 && weight >= 0.05 {
                        key_factors.push(format!("- Weak {} ({:.2})", SPACE_NAMES[i], score));
                    }
                }
            }
        }

        let summary = format!(
            "{} similarity ({:.2}) across {}/{} spaces (confidence: {:.0}%)",
            interpretation.label(),
            result.score,
            active_count,
            NUM_EMBEDDERS,
            result.confidence * 100.0
        );

        let confidence_explanation = if result.confidence >= 0.8 {
            format!(
                "High confidence: {} active spaces with consistent scores",
                active_count
            )
        } else if result.confidence >= 0.5 {
            "Moderate confidence: reasonable coverage with some variance".to_string()
        } else {
            format!(
                "Low confidence: only {} spaces active or high variance",
                active_count
            )
        };

        SimilarityExplanation {
            summary,
            space_details,
            score_interpretation: interpretation,
            key_factors,
            confidence_explanation,
            recommendations: Vec::new(),
        }
    }

    fn get_weights(&self, strategy: &WeightingStrategy) -> [f32; NUM_EMBEDDERS] {
        match strategy {
            WeightingStrategy::Uniform => WeightingStrategy::uniform_weights(),
            WeightingStrategy::Static(weights) => *weights,
            WeightingStrategy::TopicAligned => {
                // TopicAligned is not yet implemented — returns uniform weights.
                // TODO: Implement topic-profile-based weight modulation.
                WeightingStrategy::uniform_weights()
            }
            WeightingStrategy::RRF { .. } => {
                // RRF fusion is applied in the search pipeline, not here.
                // This engine uses uniform weights for RRF explanation purposes.
                WeightingStrategy::uniform_weights()
            }
            WeightingStrategy::TopicWeightedRRF { .. } => WeightingStrategy::uniform_weights(),
            WeightingStrategy::LateInteraction => {
                // Emphasize E12 for late interaction
                let mut weights = [0.05; NUM_EMBEDDERS];
                weights[11] = 0.40; // E12 ColBERT
                weights[0] = 0.20; // E1 Semantic
                weights
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = DefaultCrossSpaceEngine::cosine_similarity_dense(&a, &b).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0"
        );
        println!("[PASS] Identical vectors: sim = {}", sim);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = DefaultCrossSpaceEngine::cosine_similarity_dense(&a, &b).unwrap();
        assert!(
            sim.abs() < 1e-6,
            "Orthogonal vectors should have similarity 0.0"
        );
        println!("[PASS] Orthogonal vectors: sim = {}", sim);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = DefaultCrossSpaceEngine::cosine_similarity_dense(&a, &b).unwrap();
        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "Opposite vectors should have similarity -1.0"
        );
        println!("[PASS] Opposite vectors: sim = {}", sim);
    }

    #[test]
    fn test_sparse_cosine_similarity() {
        let a = SparseVector {
            indices: vec![1, 3, 5],
            values: vec![0.5, 0.3, 0.2],
        };
        let b = SparseVector {
            indices: vec![1, 4, 5],
            values: vec![0.4, 0.6, 0.1],
        };
        // Intersection at indices 1 and 5: 0.5*0.4 + 0.2*0.1 = 0.2 + 0.02 = 0.22 (dot product)
        // ||a|| = sqrt(0.25 + 0.09 + 0.04) = sqrt(0.38) ≈ 0.6164
        // ||b|| = sqrt(0.16 + 0.36 + 0.01) = sqrt(0.53) ≈ 0.7280
        // cosine = 0.22 / (0.6164 * 0.7280) ≈ 0.490
        let cos = DefaultCrossSpaceEngine::sparse_cosine_similarity(&a, &b);
        assert!(
            (cos - 0.490).abs() < 0.01,
            "Sparse cosine similarity mismatch: {}",
            cos
        );
        println!("[PASS] Sparse cosine similarity: {}", cos);
    }

    #[test]
    fn test_maxsim_token_level() {
        let query = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let doc = vec![vec![0.8, 0.2], vec![0.1, 0.9]];
        let sim = DefaultCrossSpaceEngine::maxsim_token_level(&query, &doc);
        // q[0] best match: doc[0] = 0.8*1 + 0.2*0 = 0.8
        // q[1] best match: doc[1] = 0.1*0 + 0.9*1 = 0.9
        // Average: (0.8 + 0.9) / 2 = 0.85
        assert!((sim - 0.85).abs() < 1e-6, "MaxSim mismatch: {}", sim);
        println!("[PASS] MaxSim token level: {}", sim);
    }

    #[test]
    fn test_get_weights_uniform() {
        let engine = DefaultCrossSpaceEngine::new();
        let weights = engine.get_weights(&WeightingStrategy::Uniform);
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-5,
            "Uniform weights should sum to 1.0"
        );
        println!("[PASS] Uniform weights sum: {}", sum);
    }

    #[test]
    fn test_compute_variance() {
        let scores: [Option<f32>; 13] = [
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
            Some(0.5),
        ];
        let variance = DefaultCrossSpaceEngine::compute_variance(&scores);
        assert!(
            variance.abs() < 1e-6,
            "Constant scores should have 0 variance"
        );
        println!("[PASS] Constant scores variance: {}", variance);
    }

}
