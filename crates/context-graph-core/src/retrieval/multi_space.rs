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

use super::config::SimilarityThresholds;
use super::distance::compute_similarity_for_space;
use super::similarity::{PerSpaceScores, SimilarityResult};

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

    /// Check if memory is relevant (ANY space above high threshold).
    ///
    /// Returns true if at least one embedding space has a similarity
    /// score above its high threshold.
    pub fn is_relevant(&self, scores: &PerSpaceScores) -> bool {
        for embedder in Embedder::all() {
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
    use crate::retrieval::config::{high_thresholds, low_thresholds};

    // =========================================================================
    // is_relevant Tests
    // =========================================================================

    #[test]
    fn test_is_relevant_one_match() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.80); // Above 0.75 threshold
        scores.set_score(Embedder::Code, 0.50); // Below 0.80 threshold

        assert!(similarity.is_relevant(&scores));
        println!("[PASS] is_relevant returns true with one matching space");
    }

    #[test]
    fn test_is_relevant_no_match() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.70); // Below 0.75 threshold
        scores.set_score(Embedder::Code, 0.50); // Below 0.80 threshold

        assert!(!similarity.is_relevant(&scores));
        println!("[PASS] is_relevant returns false with no matching spaces");
    }

    #[test]
    fn test_is_relevant_all_temporal_high() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // High temporal scores but all other spaces low
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::TemporalRecent, 0.95);
        scores.set_score(Embedder::TemporalPeriodic, 0.95);
        scores.set_score(Embedder::TemporalPositional, 0.95);

        // Temporal spaces DO count for is_relevant (threshold check is not weighted)
        // But they have weight 0.0 for weighted_similarity
        assert!(similarity.is_relevant(&scores)); // 0.95 > 0.70 threshold
        println!("[PASS] Temporal spaces above threshold count for is_relevant");
    }

    // =========================================================================
    // matching_spaces Tests
    // =========================================================================

    #[test]
    fn test_matching_spaces() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.80); // Match (> 0.75)
        scores.set_score(Embedder::Code, 0.85); // Match (> 0.80)
        scores.set_score(Embedder::Sparse, 0.30); // No match (< 0.60)

        let matches = similarity.matching_spaces(&scores);
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&Embedder::Semantic));
        assert!(matches.contains(&Embedder::Code));
        println!("[PASS] matching_spaces returns correct embedder list");
    }

    // =========================================================================
    // compute_relevance_score Tests
    // =========================================================================

    #[test]
    fn test_relevance_score_higher_with_more_matches() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores_one = PerSpaceScores::new();
        scores_one.set_score(Embedder::Semantic, 0.80);

        let mut scores_two = PerSpaceScores::new();
        scores_two.set_score(Embedder::Semantic, 0.80);
        scores_two.set_score(Embedder::Code, 0.85);

        let rel_one = similarity.compute_relevance_score(&scores_one);
        let rel_two = similarity.compute_relevance_score(&scores_two);

        assert!(
            rel_two > rel_one,
            "rel_two {} should be > rel_one {}",
            rel_two,
            rel_one
        );
        println!(
            "[PASS] More matches = higher relevance: {} > {}",
            rel_two, rel_one
        );
    }

    #[test]
    fn test_relevance_score_normalized() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // Maximum possible scores
        let mut scores = PerSpaceScores::new();
        for embedder in Embedder::all() {
            scores.set_score(embedder, 1.0);
        }

        let rel = similarity.compute_relevance_score(&scores);
        assert!(
            rel >= 0.0 && rel <= 1.0,
            "Relevance {} out of [0,1] range",
            rel
        );
        println!("[PASS] Relevance score {} is normalized to [0,1]", rel);
    }

    #[test]
    fn test_relevance_score_zero_when_all_below_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // All scores below their high thresholds
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.50); // Below 0.75
        scores.set_score(Embedder::Code, 0.60); // Below 0.80
        scores.set_score(Embedder::Causal, 0.50); // Below 0.70

        let rel = similarity.compute_relevance_score(&scores);
        assert_eq!(rel, 0.0, "All below threshold should give 0.0 relevance");
        println!("[PASS] All below threshold = relevance 0.0");
    }

    // =========================================================================
    // Temporal Exclusion Tests (AP-60)
    // =========================================================================

    #[test]
    fn test_temporal_excluded_from_weighted_similarity() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // High temporal scores only
        let mut scores_temporal = PerSpaceScores::new();
        scores_temporal.set_score(Embedder::TemporalRecent, 0.95);
        scores_temporal.set_score(Embedder::TemporalPeriodic, 0.95);
        scores_temporal.set_score(Embedder::TemporalPositional, 0.95);

        let weighted = similarity.compute_weighted_similarity(&scores_temporal);
        // All semantic/relational/structural are 0.0, temporal excluded
        assert!(
            weighted < 0.01,
            "Temporal-only scores should give near-zero weighted: {}",
            weighted
        );
        println!(
            "[PASS] AP-60: Temporal excluded from weighted_similarity: {}",
            weighted
        );
    }

    #[test]
    fn test_temporal_excluded_from_relevance_score() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // High temporal scores only
        let mut scores_temporal = PerSpaceScores::new();
        scores_temporal.set_score(Embedder::TemporalRecent, 0.95);
        scores_temporal.set_score(Embedder::TemporalPeriodic, 0.95);
        scores_temporal.set_score(Embedder::TemporalPositional, 0.95);

        let rel = similarity.compute_relevance_score(&scores_temporal);
        assert_eq!(rel, 0.0, "Temporal-only should give 0.0 relevance");
        println!("[PASS] AP-60: Temporal excluded from relevance_score");
    }

    // =========================================================================
    // Category Weight Tests
    // =========================================================================

    #[test]
    fn test_semantic_contributes_full_weight() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.90);
        scores.set_score(Embedder::Code, 0.90);

        let weighted = similarity.compute_weighted_similarity(&scores);
        // Two semantic spaces at 0.90 should give positive result
        assert!(
            weighted > 0.0,
            "Semantic spaces should contribute: {}",
            weighted
        );
        println!("[PASS] Semantic spaces contribute full weight: {}", weighted);
    }

    #[test]
    fn test_relational_contributes_half_weight() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // One semantic at 1.0, one relational at 1.0
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 1.0); // weight 1.0
        scores.set_score(Embedder::Emotional, 1.0); // weight 0.5 (relational)

        let weighted = similarity.compute_weighted_similarity(&scores);
        // Sum(w*s) / Sum(w) for all non-temporal spaces
        // weighted_sum = 1.0*1.0 + 0.5*1.0 = 1.5
        // total_weight = 8.5 (all non-temporal weights)
        // result = 1.5 / 8.5 = 0.176...
        assert!(weighted > 0.0 && weighted <= 1.0);
        println!(
            "[PASS] Relational contributes 0.5 weight, result: {}",
            weighted
        );
    }

    // =========================================================================
    // is_below_low_threshold Tests
    // =========================================================================

    #[test]
    fn test_below_low_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // E1 low threshold is 0.30
        assert!(similarity.is_below_low_threshold(Embedder::Semantic, 0.25));
        assert!(!similarity.is_below_low_threshold(Embedder::Semantic, 0.35));
        println!("[PASS] is_below_low_threshold works correctly");
    }

    // =========================================================================
    // compute_full_result Tests
    // =========================================================================

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
        println!("[PASS] compute_full_result builds valid SimilarityResult");
    }

    // =========================================================================
    // Batch and Sort Tests
    // =========================================================================

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
        println!("[PASS] sort_by_relevance orders highest first");
    }

    #[test]
    fn test_compute_similarities_batch() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let query = SemanticFingerprint::zeroed();

        let memories = vec![
            (Uuid::new_v4(), SemanticFingerprint::zeroed()),
            (Uuid::new_v4(), SemanticFingerprint::zeroed()),
            (Uuid::new_v4(), SemanticFingerprint::zeroed()),
        ];

        let results = compute_similarities_batch(&similarity, &query, &memories);

        assert_eq!(results.len(), 3);
        for result in &results {
            assert!(result.relevance_score >= 0.0 && result.relevance_score <= 1.0);
        }
        println!("[PASS] compute_similarities_batch processes all memories");
    }

    #[test]
    fn test_filter_relevant() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // Create results with varying scores
        let mut scores_high = PerSpaceScores::new();
        scores_high.set_score(Embedder::Semantic, 0.85); // Above 0.75

        let mut scores_low = PerSpaceScores::new();
        scores_low.set_score(Embedder::Semantic, 0.50); // Below 0.75

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
        println!("[PASS] filter_relevant keeps only relevant results");
    }

    // =========================================================================
    // Threshold Value Tests
    // =========================================================================

    #[test]
    fn test_threshold_values_from_spec() {
        let high = high_thresholds();
        let low = low_thresholds();

        // Verify key spec values
        assert_eq!(high.get_threshold(Embedder::Semantic), 0.75);
        assert_eq!(high.get_threshold(Embedder::Code), 0.80);
        assert_eq!(high.get_threshold(Embedder::Sparse), 0.60);

        assert_eq!(low.get_threshold(Embedder::Semantic), 0.30);
        assert_eq!(low.get_threshold(Embedder::Causal), 0.25);
        assert_eq!(low.get_threshold(Embedder::Code), 0.35);

        println!("[PASS] Threshold values match spec");
    }

    #[test]
    fn test_all_high_greater_than_low() {
        let high = high_thresholds();
        let low = low_thresholds();

        for embedder in Embedder::all() {
            let h = high.get_threshold(embedder);
            let l = low.get_threshold(embedder);
            assert!(h > l, "{:?}: high {} must be > low {}", embedder, h, l);
        }
        println!("[PASS] All high thresholds > low thresholds");
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn edge_case_all_zeros() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let scores = PerSpaceScores::new(); // All zeros

        assert!(!similarity.is_relevant(&scores));
        assert_eq!(similarity.matching_spaces(&scores).len(), 0);
        assert_eq!(similarity.compute_relevance_score(&scores), 0.0);
        assert_eq!(similarity.compute_weighted_similarity(&scores), 0.0);
        println!("[PASS] Edge case: all zeros handled correctly");
    }

    #[test]
    fn edge_case_all_ones() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        for embedder in Embedder::all() {
            scores.set_score(embedder, 1.0);
        }

        assert!(similarity.is_relevant(&scores));
        let matching = similarity.matching_spaces(&scores);
        assert_eq!(matching.len(), 13); // All spaces match when score = 1.0

        let rel = similarity.compute_relevance_score(&scores);
        let weighted = similarity.compute_weighted_similarity(&scores);

        assert!(rel > 0.99 && rel <= 1.0, "rel={}", rel);
        assert!(weighted > 0.99 && weighted <= 1.0, "weighted={}", weighted);
        println!("[PASS] Edge case: all ones = max relevance");
    }

    #[test]
    fn edge_case_exactly_at_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.75); // Exactly at threshold

        // > threshold, not >=, so 0.75 should NOT match
        assert!(!similarity.is_relevant(&scores));
        assert_eq!(similarity.matching_spaces(&scores).len(), 0);
        println!("[PASS] Edge case: exactly at threshold = not relevant (> not >=)");
    }

    #[test]
    fn edge_case_just_above_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.76); // Just above threshold

        assert!(similarity.is_relevant(&scores));
        assert_eq!(similarity.matching_spaces(&scores).len(), 1);
        println!("[PASS] Edge case: just above threshold = relevant");
    }

    #[test]
    fn edge_case_max_weighted_agreement_consistency() {
        use crate::embeddings::category::max_weighted_agreement;

        // Verify max_weighted_agreement matches the constitution
        let max_wa = max_weighted_agreement();
        assert!(
            (max_wa - 8.5).abs() < f32::EPSILON,
            "max_weighted_agreement should be 8.5, got {}",
            max_wa
        );
        println!("[PASS] max_weighted_agreement = 8.5 (matches constitution)");
    }

    // =========================================================================
    // Constitution Compliance Tests
    // =========================================================================

    #[test]
    fn test_arch09_topic_threshold() {
        use crate::embeddings::category::topic_threshold;

        // ARCH-09: Topic threshold is weighted_agreement >= 2.5
        assert!(
            (topic_threshold() - 2.5).abs() < f32::EPSILON,
            "Topic threshold must be 2.5"
        );
        println!("[PASS] ARCH-09: topic_threshold = 2.5");
    }

    #[test]
    fn test_ap60_temporal_weight_zero() {
        // AP-60: Temporal embedders (E2-E4) MUST NOT count toward topic detection
        for embedder in [
            Embedder::TemporalRecent,
            Embedder::TemporalPeriodic,
            Embedder::TemporalPositional,
        ] {
            let weight = category_for(embedder).topic_weight();
            assert_eq!(
                weight, 0.0,
                "AP-60 violation: {:?} has non-zero weight {}",
                embedder, weight
            );
        }
        println!("[PASS] AP-60: All temporal embedders have weight 0.0");
    }

    #[test]
    fn test_category_weights_from_category_for() {
        // Verify category_for returns correct weights per constitution
        // Semantic: 1.0, Temporal: 0.0, Relational: 0.5, Structural: 0.5
        for embedder in Embedder::all() {
            let weight = category_for(embedder).topic_weight();
            let category = category_for(embedder);

            let expected = match category {
                crate::embeddings::category::EmbedderCategory::Semantic => 1.0,
                crate::embeddings::category::EmbedderCategory::Temporal => 0.0,
                crate::embeddings::category::EmbedderCategory::Relational => 0.5,
                crate::embeddings::category::EmbedderCategory::Structural => 0.5,
            };

            assert!(
                (weight - expected).abs() < f32::EPSILON,
                "{:?}: weight {} != expected {}",
                embedder,
                weight,
                expected
            );
        }
        println!("[PASS] All category weights match constitution");
    }
}
