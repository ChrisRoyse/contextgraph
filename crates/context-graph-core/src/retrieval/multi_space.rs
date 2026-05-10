//! Multi-space similarity computation for 13-embedding retrieval.
//!
//! This module implements the core comparison engine that:
//! - Computes similarity scores across all 13 embedding spaces
//! - Determines relevance using ANY() logic (any space above high threshold)
//! - Calculates category-weighted relevance scores
//! - Excludes temporal spaces (E2-E4) from weighted calculations per AP-60
//!
//! # Architecture Rules
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) MUST NOT count toward topic detection
//! - AP-10: No NaN/Infinity in scores

use uuid::Uuid;

use crate::embeddings::category::category_for;
use crate::teleological::Embedder;
use crate::types::fingerprint::SemanticFingerprint;
use crate::weights::E5_CAUSAL_ENABLED;

use super::config::SimilarityThresholds;
use super::distance::{compute_similarity_for_space, compute_similarity_for_space_with_direction};
use super::similarity::{PerSpaceScores, SimilarityResult};

use crate::causal::asymmetric::CausalDirection;

/// Multi-space similarity computation service.
///
/// Provides methods for computing similarity across all 13 embedding spaces,
/// determining relevance, and calculating weighted scores.
///
/// # Weight Handling
///
/// Category weights are obtained directly from `category_for(embedder).topic_weight()`,
/// which ensures consistency with the constitution and avoids weight duplication.
#[derive(Debug, Clone)]
pub struct MultiSpaceSimilarity {
    thresholds: SimilarityThresholds,
}

impl MultiSpaceSimilarity {
    /// Create with custom thresholds.
    pub fn new(thresholds: SimilarityThresholds) -> Self {
        Self { thresholds }
    }

    /// Create with default configuration from spec.
    ///
    /// Uses high_thresholds/low_thresholds from TECH-PHASE3 spec.
    /// Category weights are derived from `category_for(embedder).topic_weight()`.
    pub fn with_defaults() -> Self {
        Self {
            thresholds: SimilarityThresholds::default(),
        }
    }

    /// Compute similarity scores across all 13 embedding spaces.
    ///
    /// Uses the distance calculator to compute per-space similarities.
    pub fn compute_similarity(
        &self,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
    ) -> PerSpaceScores {
        let mut scores = PerSpaceScores::new();

        for embedder in Embedder::all() {
            let sim = compute_similarity_for_space(embedder, query, memory);
            scores.set_score(embedder, sim);
        }

        scores
    }

    /// Compute similarity scores with causal direction for E5.
    ///
    /// Like `compute_similarity()` but uses asymmetric E5 similarity when
    /// a causal direction is provided (per ARCH-15 and AP-77).
    ///
    /// # Arguments
    /// * `query` - Query fingerprint
    /// * `memory` - Memory fingerprint
    /// * `causal_direction` - Detected causal direction of the query
    ///
    /// # Returns
    /// Per-space similarity scores with direction-aware E5 computation
    pub fn compute_similarity_with_direction(
        &self,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
        causal_direction: CausalDirection,
    ) -> PerSpaceScores {
        let mut scores = PerSpaceScores::new();

        for embedder in Embedder::all() {
            let sim = compute_similarity_for_space_with_direction(
                embedder,
                query,
                memory,
                causal_direction,
            );
            scores.set_score(embedder, sim);
        }

        scores
    }

    /// Check if memory is relevant (ANY space above high threshold).
    ///
    /// Returns true if at least one embedding space has a similarity
    /// score above its high threshold.
    pub fn is_relevant(&self, scores: &PerSpaceScores) -> bool {
        for embedder in Embedder::all() {
            if matches!(embedder, Embedder::Causal) && !E5_CAUSAL_ENABLED {
                continue;
            }
            let score = scores.get_score(embedder);
            let threshold = self.thresholds.high.get_threshold(embedder);
            if score > threshold {
                return true;
            }
        }
        false
    }

    /// Get list of embedders where score exceeds high threshold.
    pub fn matching_spaces(&self, scores: &PerSpaceScores) -> Vec<Embedder> {
        let mut matches = Vec::new();

        for embedder in Embedder::all() {
            if matches!(embedder, Embedder::Causal) && !E5_CAUSAL_ENABLED {
                continue;
            }
            let score = scores.get_score(embedder);
            let threshold = self.thresholds.high.get_threshold(embedder);
            if score > threshold {
                matches.push(embedder);
            }
        }

        matches
    }

    /// Compute weighted relevance score using category weights.
    ///
    /// Formula: Sum(category_weight * max(0, score - threshold)) / max_possible
    ///
    /// NOTE: Temporal spaces (E2-E4) have category_weight 0.0 and are excluded.
    /// Uses category_for(embedder).topic_weight() for weights.
    pub fn compute_relevance_score(&self, scores: &PerSpaceScores) -> f32 {
        let mut weighted_sum = 0.0_f32;
        let mut max_possible = 0.0_f32;

        for embedder in Embedder::all() {
            if matches!(embedder, Embedder::Causal) && !E5_CAUSAL_ENABLED {
                continue;
            }
            let category_weight = category_for(embedder).topic_weight();

            // Skip temporal spaces (weight = 0.0) per AP-60
            if category_weight == 0.0 {
                continue;
            }

            let score = scores.get_score(embedder);
            let threshold = self.thresholds.high.get_threshold(embedder);

            // Score above threshold contributes positively
            let contribution = (score - threshold).max(0.0);
            weighted_sum += category_weight * contribution;

            // Maximum possible is if score was 1.0
            max_possible += category_weight * (1.0 - threshold).max(0.0);
        }

        // Normalize to [0.0, 1.0]
        if max_possible > 0.0 {
            (weighted_sum / max_possible).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Compute weighted similarity using category weights (excludes temporal).
    ///
    /// This is a simpler version that sums weighted scores without threshold subtraction.
    /// Result: Sum(category_weight * score) / Sum(category_weight)
    pub fn compute_weighted_similarity(&self, scores: &PerSpaceScores) -> f32 {
        let mut weighted_sum = 0.0_f32;
        let mut total_weight = 0.0_f32;

        for embedder in Embedder::all() {
            if matches!(embedder, Embedder::Causal) && !E5_CAUSAL_ENABLED {
                continue;
            }
            let category_weight = category_for(embedder).topic_weight();

            // Skip temporal spaces (weight = 0.0) per AP-60
            if category_weight == 0.0 {
                continue;
            }

            let score = scores.get_score(embedder);
            weighted_sum += category_weight * score;
            total_weight += category_weight;
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Compute complete SimilarityResult for a memory.
    pub fn compute_full_result(
        &self,
        memory_id: Uuid,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
    ) -> SimilarityResult {
        let scores = self.compute_similarity(query, memory);
        let matching = self.matching_spaces(&scores);
        let relevance = self.compute_relevance_score(&scores);

        SimilarityResult::with_relevance(memory_id, scores, relevance, matching)
    }

    /// Compute complete SimilarityResult with causal direction.
    ///
    /// Like `compute_full_result()` but uses direction-aware E5 similarity
    /// per ARCH-15 and AP-77.
    pub fn compute_full_result_with_direction(
        &self,
        memory_id: Uuid,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
        causal_direction: CausalDirection,
    ) -> SimilarityResult {
        let scores = self.compute_similarity_with_direction(query, memory, causal_direction);
        let matching = self.matching_spaces(&scores);
        let relevance = self.compute_relevance_score(&scores);

        SimilarityResult::with_relevance(memory_id, scores, relevance, matching)
    }

    /// Get reference to thresholds.
    #[inline]
    pub fn thresholds(&self) -> &SimilarityThresholds {
        &self.thresholds
    }

    /// Check if score is below low threshold (for divergence detection).
    #[inline]
    pub fn is_below_low_threshold(&self, embedder: Embedder, score: f32) -> bool {
        score < self.thresholds.low.get_threshold(embedder)
    }
}

/// Batch comparison for multiple memories.
pub fn compute_similarities_batch(
    similarity: &MultiSpaceSimilarity,
    query: &SemanticFingerprint,
    memories: &[(Uuid, SemanticFingerprint)],
) -> Vec<SimilarityResult> {
    memories
        .iter()
        .map(|(id, memory)| similarity.compute_full_result(*id, query, memory))
        .collect()
}

/// Filter to relevant results only.
pub fn filter_relevant(
    similarity: &MultiSpaceSimilarity,
    results: Vec<SimilarityResult>,
) -> Vec<SimilarityResult> {
    results
        .into_iter()
        .filter(|r| similarity.is_relevant(&r.per_space_scores))
        .collect()
}

/// Sort results by relevance score (highest first).
pub fn sort_by_relevance(mut results: Vec<SimilarityResult>) -> Vec<SimilarityResult> {
    results.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_relevant() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // One match
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.80);
        assert!(similarity.is_relevant(&scores));

        // No match
        let mut scores2 = PerSpaceScores::new();
        scores2.set_score(Embedder::Semantic, 0.70);
        assert!(!similarity.is_relevant(&scores2));
    }

    #[test]
    fn test_matching_spaces() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.80);
        scores.set_score(Embedder::Code, 0.85);
        scores.set_score(Embedder::Sparse, 0.30);

        let matches = similarity.matching_spaces(&scores);
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&Embedder::Semantic));
        assert!(matches.contains(&Embedder::Code));
    }

    #[test]
    fn test_relevance_score_higher_with_more_matches() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores_one = PerSpaceScores::new();
        scores_one.set_score(Embedder::Semantic, 0.80);

        let mut scores_two = PerSpaceScores::new();
        scores_two.set_score(Embedder::Semantic, 0.80);
        scores_two.set_score(Embedder::Code, 0.85);

        assert!(
            similarity.compute_relevance_score(&scores_two)
                > similarity.compute_relevance_score(&scores_one)
        );
    }

    #[test]
    fn test_temporal_excluded_from_weighted_similarity() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::TemporalRecent, 0.95);
        scores.set_score(Embedder::TemporalPeriodic, 0.95);
        scores.set_score(Embedder::TemporalPositional, 0.95);

        let weighted = similarity.compute_weighted_similarity(&scores);
        assert!(
            weighted < 0.01,
            "Temporal-only should give near-zero weighted: {}",
            weighted
        );

        let rel = similarity.compute_relevance_score(&scores);
        assert_eq!(rel, 0.0, "Temporal-only should give 0.0 relevance");
    }

    #[test]
    fn test_below_low_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        assert!(similarity.is_below_low_threshold(Embedder::Semantic, 0.25));
        assert!(!similarity.is_below_low_threshold(Embedder::Semantic, 0.35));
    }

    #[test]
    fn test_compute_full_result() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let memory_id = Uuid::new_v4();
        let query = SemanticFingerprint::zeroed();
        let memory = SemanticFingerprint::zeroed();

        let result = similarity.compute_full_result(memory_id, &query, &memory);

        assert_eq!(result.memory_id, memory_id);
        assert_eq!(result.space_count as usize, result.matching_spaces.len());
        assert!(result.relevance_score >= 0.0 && result.relevance_score <= 1.0);
    }

    #[test]
    fn test_sort_by_relevance() {
        let results = vec![
            SimilarityResult::with_relevance(Uuid::new_v4(), PerSpaceScores::new(), 0.3, vec![]),
            SimilarityResult::with_relevance(Uuid::new_v4(), PerSpaceScores::new(), 0.9, vec![]),
            SimilarityResult::with_relevance(Uuid::new_v4(), PerSpaceScores::new(), 0.5, vec![]),
        ];

        let sorted = sort_by_relevance(results);
        assert_eq!(sorted[0].relevance_score, 0.9);
        assert_eq!(sorted[1].relevance_score, 0.5);
        assert_eq!(sorted[2].relevance_score, 0.3);
    }

    #[test]
    fn test_filter_relevant() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores_high = PerSpaceScores::new();
        scores_high.set_score(Embedder::Semantic, 0.85);

        let mut scores_low = PerSpaceScores::new();
        scores_low.set_score(Embedder::Semantic, 0.50);

        let results = vec![
            SimilarityResult::with_relevance(
                Uuid::new_v4(),
                scores_high.clone(),
                0.8,
                vec![Embedder::Semantic],
            ),
            SimilarityResult::with_relevance(Uuid::new_v4(), scores_low.clone(), 0.0, vec![]),
            SimilarityResult::with_relevance(
                Uuid::new_v4(),
                scores_high.clone(),
                0.7,
                vec![Embedder::Semantic],
            ),
        ];

        let filtered = filter_relevant(&similarity, results);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn edge_case_all_zeros() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let scores = PerSpaceScores::new();

        assert!(!similarity.is_relevant(&scores));
        assert_eq!(similarity.matching_spaces(&scores).len(), 0);
        assert_eq!(similarity.compute_relevance_score(&scores), 0.0);
        assert_eq!(similarity.compute_weighted_similarity(&scores), 0.0);
    }

    #[test]
    fn test_direction_only_affects_e5() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        query.e1_semantic = vec![1.0; 1024];
        memory.e1_semantic = vec![1.0; 1024];
        query.e5_causal_as_cause = vec![1.0; 768];
        query.e5_causal_as_effect = vec![0.0; 768];
        memory.e5_causal_as_cause = vec![0.1; 768];
        memory.e5_causal_as_effect = vec![0.9; 768];

        let sym = similarity.compute_similarity(&query, &memory);
        let with_cause =
            similarity.compute_similarity_with_direction(&query, &memory, CausalDirection::Cause);

        for embedder in Embedder::all() {
            if !matches!(embedder, Embedder::Causal) {
                let s = sym.get_score(embedder);
                let c = with_cause.get_score(embedder);
                assert!(
                    (s - c).abs() < 1e-5,
                    "{:?} should be unchanged: sym={}, cause={}",
                    embedder,
                    s,
                    c
                );
            }
        }
    }
}
