//! Result types for cross-space similarity computation.
//!
//! This module provides the `CrossSpaceSimilarity` struct that contains
//! the computed similarity score and optional diagnostic information.

use crate::types::fingerprint::NUM_EMBEDDERS;
use serde::{Deserialize, Serialize};

/// Result of cross-space similarity computation.
///
/// Contains the aggregated similarity score along with optional breakdown
/// information for debugging and explanation purposes.
///
/// # Score Interpretation
///
/// - `score`: Final normalized similarity in range [0.0, 1.0]
/// - `confidence`: How reliable the score is based on space coverage and variance
/// - `active_spaces`: Bitmask of which embedding spaces contributed
///
/// # Example
///
/// ```rust,ignore
/// let result = engine.compute_similarity(&fp1, &fp2, &config).await?;
///
/// println!("Score: {:.4}", result.score);
/// println!("Confidence: {:.4}", result.confidence);
/// println!("Active spaces: {}/13", result.active_count());
///
/// if let Some(ref scores) = result.space_scores {
///     for (i, score) in scores.iter().enumerate() {
///         if let Some(s) = score {
///             println!("  E{}: {:.4}", i + 1, s);
///         }
///     }
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossSpaceSimilarity {
    /// Final aggregated similarity score in range [0.0, 1.0].
    ///
    /// This is the primary result - normalized to [0.0, 1.0] regardless
    /// of the underlying aggregation strategy.
    pub score: f32,

    /// Raw score before normalization.
    ///
    /// For RRF, this is the sum of 1/(k+rank+1) contributions.
    /// For weighted average, this is the weighted sum before division.
    pub raw_score: f32,

    /// Per-space similarity breakdown (13 spaces).
    ///
    /// Only populated if `config.include_breakdown = true`.
    /// `None` values indicate missing/inactive spaces.
    pub space_scores: Option<[Option<f32>; NUM_EMBEDDERS]>,

    /// Bitmask of which spaces contributed (bits 0-12).
    ///
    /// Use `active_count()` to get the count of active spaces.
    /// Bit i is set if space i contributed to the score.
    pub active_spaces: u16,

    /// Per-space weights used (13D).
    ///
    /// Only populated if `config.include_breakdown = true`.
    /// Shows the actual weights applied to each space.
    pub space_weights: Option<[f32; NUM_EMBEDDERS]>,

    /// Purpose alignment contribution (if purpose weighting enabled).
    ///
    /// Shows how much the purpose vector modulated the final score.
    pub purpose_contribution: Option<f32>,

    /// Confidence in the score [0.0, 1.0].
    ///
    /// Based on:
    /// - Number of active spaces (more = higher confidence)
    /// - Variance of per-space scores (lower = higher confidence)
    /// - Agreement between spaces (higher = higher confidence)
    pub confidence: f32,

    /// RRF-specific normalized score.
    ///
    /// Only populated when using RRF strategy.
    /// Normalized to [0.0, 1.0] based on theoretical maximum RRF score.
    pub rrf_score: Option<f32>,
}

impl CrossSpaceSimilarity {
    /// Count of active embedding spaces that contributed to the score.
    ///
    /// # Returns
    /// Number of bits set in `active_spaces` (0-13).
    #[inline]
    pub fn active_count(&self) -> u32 {
        self.active_spaces.count_ones()
    }

    /// Check if a specific embedding space contributed.
    ///
    /// # Arguments
    /// - `space_idx`: Index of the embedding space (0-12)
    ///
    /// # Returns
    /// `true` if the space was active and contributed to the score.
    #[inline]
    pub fn is_space_active(&self, space_idx: usize) -> bool {
        if space_idx >= NUM_EMBEDDERS {
            return false;
        }
        (self.active_spaces & (1 << space_idx)) != 0
    }

    /// Get the similarity score for a specific space (if available).
    ///
    /// # Arguments
    /// - `space_idx`: Index of the embedding space (0-12)
    ///
    /// # Returns
    /// - `Some(score)` if breakdown is available and space was active
    /// - `None` if breakdown not included or space was inactive
    #[inline]
    pub fn get_space_score(&self, space_idx: usize) -> Option<f32> {
        self.space_scores
            .as_ref()
            .and_then(|scores| {
                if space_idx < NUM_EMBEDDERS {
                    scores[space_idx]
                } else {
                    None
                }
            })
    }

    /// Create a result indicating insufficient active spaces.
    ///
    /// This is used when the minimum space requirement is not met.
    pub fn insufficient_spaces(required: usize, found: u32) -> Self {
        Self {
            score: 0.0,
            raw_score: 0.0,
            space_scores: None,
            active_spaces: 0,
            space_weights: None,
            purpose_contribution: None,
            confidence: 0.0,
            rrf_score: None,
        }
    }

    /// Create a zero result (no similarity).
    pub fn zero() -> Self {
        Self {
            score: 0.0,
            raw_score: 0.0,
            space_scores: None,
            active_spaces: 0,
            space_weights: None,
            purpose_contribution: None,
            confidence: 0.0,
            rrf_score: None,
        }
    }

    /// Create a perfect similarity result (identical fingerprints).
    pub fn perfect() -> Self {
        Self {
            score: 1.0,
            raw_score: 1.0,
            space_scores: Some([Some(1.0); NUM_EMBEDDERS]),
            active_spaces: (1 << NUM_EMBEDDERS) - 1, // All 13 bits set
            space_weights: Some([1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS]),
            purpose_contribution: None,
            confidence: 1.0,
            rrf_score: None,
        }
    }

    /// Builder pattern: set breakdown scores.
    pub fn with_breakdown(
        mut self,
        space_scores: [Option<f32>; NUM_EMBEDDERS],
        space_weights: [f32; NUM_EMBEDDERS],
    ) -> Self {
        self.space_scores = Some(space_scores);
        self.space_weights = Some(space_weights);
        self
    }

    /// Builder pattern: set RRF score.
    pub fn with_rrf_score(mut self, rrf_score: f32) -> Self {
        self.rrf_score = Some(rrf_score);
        self
    }

    /// Builder pattern: set purpose contribution.
    pub fn with_purpose_contribution(mut self, contribution: f32) -> Self {
        self.purpose_contribution = Some(contribution);
        self
    }

    /// Compute confidence based on active spaces and score variance.
    ///
    /// # Formula
    /// confidence = (active_count / NUM_EMBEDDERS) * (1 - normalized_variance)
    pub fn compute_confidence(
        active_count: u32,
        scores_variance: f32,
    ) -> f32 {
        let coverage = active_count as f32 / NUM_EMBEDDERS as f32;
        // Variance of [0, 1] scores has max ~0.25, so normalize by 0.25
        let variance_factor = 1.0 - (scores_variance / 0.25).min(1.0);
        (coverage * variance_factor).clamp(0.0, 1.0)
    }
}

impl Default for CrossSpaceSimilarity {
    fn default() -> Self {
        Self::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_count_empty() {
        let result = CrossSpaceSimilarity::zero();
        assert_eq!(result.active_count(), 0);
        println!("[PASS] Zero result has 0 active spaces");
    }

    #[test]
    fn test_active_count_all() {
        let result = CrossSpaceSimilarity::perfect();
        assert_eq!(result.active_count(), 13);
        println!("[PASS] Perfect result has 13 active spaces");
    }

    #[test]
    fn test_is_space_active() {
        let mut result = CrossSpaceSimilarity::zero();
        result.active_spaces = 0b0000000001101; // Spaces 0, 2, 3 active

        assert!(result.is_space_active(0));
        assert!(!result.is_space_active(1));
        assert!(result.is_space_active(2));
        assert!(result.is_space_active(3));
        assert!(!result.is_space_active(4));
        assert!(!result.is_space_active(13)); // Out of bounds

        println!(
            "[PASS] is_space_active correctly identifies active spaces: {}",
            result.active_count()
        );
    }

    #[test]
    fn test_get_space_score() {
        let mut result = CrossSpaceSimilarity::zero();
        result.space_scores = Some([
            Some(0.9),
            None,
            Some(0.8),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ]);

        assert_eq!(result.get_space_score(0), Some(0.9));
        assert_eq!(result.get_space_score(1), None);
        assert_eq!(result.get_space_score(2), Some(0.8));
        assert_eq!(result.get_space_score(13), None); // Out of bounds

        println!("[PASS] get_space_score returns correct values");
    }

    #[test]
    fn test_compute_confidence() {
        // Full coverage, no variance = max confidence
        let conf1 = CrossSpaceSimilarity::compute_confidence(13, 0.0);
        assert!((conf1 - 1.0).abs() < 1e-6);

        // Half coverage, no variance
        let conf2 = CrossSpaceSimilarity::compute_confidence(6, 0.0);
        assert!((conf2 - 6.0 / 13.0).abs() < 1e-6);

        // Full coverage, max variance
        let conf3 = CrossSpaceSimilarity::compute_confidence(13, 0.25);
        assert!(conf3 < 0.1);

        println!(
            "[PASS] Confidence: full/no_var={:.4}, half/no_var={:.4}, full/max_var={:.4}",
            conf1, conf2, conf3
        );
    }

    #[test]
    fn test_builder_pattern() {
        let result = CrossSpaceSimilarity {
            score: 0.85,
            raw_score: 0.85,
            space_scores: None,
            active_spaces: 0b1111111111111,
            space_weights: None,
            purpose_contribution: None,
            confidence: 0.9,
            rrf_score: None,
        }
        .with_rrf_score(0.045)
        .with_purpose_contribution(0.1);

        assert_eq!(result.rrf_score, Some(0.045));
        assert_eq!(result.purpose_contribution, Some(0.1));
        println!("[PASS] Builder pattern sets values correctly");
    }

    #[test]
    fn test_score_in_valid_range() {
        let result = CrossSpaceSimilarity::perfect();
        assert!(result.score >= 0.0 && result.score <= 1.0);
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);

        let zero = CrossSpaceSimilarity::zero();
        assert!(zero.score >= 0.0 && zero.score <= 1.0);

        println!("[PASS] All scores in valid [0.0, 1.0] range");
    }
}
