//! Aggregation strategies for multi-space search results.
//!
//! This module provides the `AggregationStrategy` enum with implementations
//! for combining results from multiple embedding space searches.
//!
//! # Primary Strategy: RRF (per constitution.yaml)
//!
//! RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1) where k=60
//!
//! # Example
//!
//! ```
//! use context_graph_core::retrieval::AggregationStrategy;
//! use uuid::Uuid;
//!
//! let id1 = Uuid::new_v4();
//! let id2 = Uuid::new_v4();
//!
//! let ranked_lists = vec![
//!     (0, vec![id1, id2]),  // Space 0: id1=rank0, id2=rank1
//!     (1, vec![id2, id1]),  // Space 1: id2=rank0, id1=rank1
//! ];
//!
//! let scores = AggregationStrategy::aggregate_rrf(&ranked_lists, 60.0);
//! assert!(scores.contains_key(&id1));
//! assert!(scores.contains_key(&id2));
//! ```

use crate::types::fingerprint::{PurposeVector, NUM_EMBEDDERS};
use std::collections::HashMap;
use uuid::Uuid;

/// Aggregation strategy for combining multi-space search results.
///
/// # Primary Strategy: RRF (per constitution.yaml)
/// RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)
#[derive(Clone, Debug)]
pub enum AggregationStrategy {
    /// Reciprocal Rank Fusion - PRIMARY STRATEGY.
    /// Formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)
    ///
    /// # Parameters
    /// - k: Ranking constant (default: 60 per RRF literature)
    RRF { k: f32 },

    /// Weighted average of similarities.
    /// Score = Σ(wᵢ × simᵢ) / Σwᵢ
    WeightedAverage {
        weights: [f32; NUM_EMBEDDERS],
        require_all: bool,
    },

    /// Maximum similarity across spaces.
    /// Score = max(simᵢ)
    MaxPooling,

    /// Purpose-weighted aggregation using 13D purpose vector.
    /// Score = Σ(τᵢ × simᵢ) / Στᵢ where τ = purpose_vector.alignments
    PurposeWeighted { purpose_vector: PurposeVector },
}

impl Default for AggregationStrategy {
    fn default() -> Self {
        Self::RRF { k: 60.0 }
    }
}

impl AggregationStrategy {
    /// Aggregate similarity scores (for non-RRF strategies).
    ///
    /// # Arguments
    /// - matches: Vec of (space_index, similarity) pairs
    ///
    /// # Returns
    /// Aggregated similarity score [0.0, 1.0]
    ///
    /// # Panics
    /// Panics if called with RRF strategy (use aggregate_rrf instead)
    pub fn aggregate(&self, matches: &[(usize, f32)]) -> f32 {
        match self {
            Self::RRF { .. } => {
                panic!("RRF requires rank-based input - use aggregate_rrf()");
            }
            Self::WeightedAverage { weights, require_all } => {
                if *require_all && matches.len() < NUM_EMBEDDERS {
                    return 0.0;
                }
                let (sum, weight_sum) = matches
                    .iter()
                    .filter(|(idx, _)| *idx < NUM_EMBEDDERS)
                    .map(|(idx, sim)| (sim * weights[*idx], weights[*idx]))
                    .fold((0.0, 0.0), |(s, w), (sim, wt)| (s + sim, w + wt));
                if weight_sum > f32::EPSILON {
                    sum / weight_sum
                } else {
                    0.0
                }
            }
            Self::MaxPooling => matches
                .iter()
                .map(|(_, sim)| *sim)
                .fold(0.0_f32, f32::max),
            Self::PurposeWeighted { purpose_vector } => {
                let (sum, weight_sum) = matches
                    .iter()
                    .filter(|(idx, _)| *idx < NUM_EMBEDDERS)
                    .map(|(idx, sim)| {
                        let weight = purpose_vector.alignments[*idx];
                        (sim * weight, weight)
                    })
                    .fold((0.0, 0.0), |(s, w), (sim, wt)| (s + sim, w + wt));
                if weight_sum > f32::EPSILON {
                    sum / weight_sum
                } else {
                    0.0
                }
            }
        }
    }

    /// Aggregate using Reciprocal Rank Fusion across ranked lists.
    ///
    /// # Formula
    /// RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)
    ///
    /// # Arguments
    /// - ranked_lists: Vec of (space_index, Vec<memory_id>) per space
    /// - k: RRF constant (default: 60)
    ///
    /// # Returns
    /// HashMap of memory_id -> RRF score
    ///
    /// # Example
    /// ```ignore
    /// // Document d appears at ranks 0, 2, 1 across 3 spaces
    /// // RRF(d) = 1/(60+1) + 1/(60+3) + 1/(60+2) = 1/61 + 1/63 + 1/62 ≈ 0.0492
    /// ```
    pub fn aggregate_rrf(ranked_lists: &[(usize, Vec<Uuid>)], k: f32) -> HashMap<Uuid, f32> {
        let mut scores: HashMap<Uuid, f32> = HashMap::new();

        for (_space_idx, ranked_ids) in ranked_lists {
            for (rank, memory_id) in ranked_ids.iter().enumerate() {
                // RRF: 1 / (k + rank + 1) - rank is 0-indexed
                let rrf_contribution = 1.0 / (k + (rank as f32) + 1.0);
                *scores.entry(*memory_id).or_insert(0.0) += rrf_contribution;
            }
        }

        scores
    }

    /// Aggregate RRF with per-space weighting.
    ///
    /// # Formula
    /// RRF_weighted(d) = Σᵢ wᵢ/(k + rankᵢ(d) + 1)
    pub fn aggregate_rrf_weighted(
        ranked_lists: &[(usize, Vec<Uuid>)],
        k: f32,
        weights: &[f32; NUM_EMBEDDERS],
    ) -> HashMap<Uuid, f32> {
        let mut scores: HashMap<Uuid, f32> = HashMap::new();

        for (space_idx, ranked_ids) in ranked_lists {
            let weight = if *space_idx < NUM_EMBEDDERS {
                weights[*space_idx]
            } else {
                1.0
            };

            for (rank, memory_id) in ranked_ids.iter().enumerate() {
                let rrf_contribution = weight / (k + (rank as f32) + 1.0);
                *scores.entry(*memory_id).or_insert(0.0) += rrf_contribution;
            }
        }

        scores
    }

    /// Compute RRF contribution for a single rank.
    #[inline]
    pub fn rrf_contribution(rank: usize, k: f32) -> f32 {
        1.0 / (k + (rank as f32) + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_aggregation_single_list() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let ranked_lists = vec![(0, vec![id1, id2])];

        let scores = AggregationStrategy::aggregate_rrf(&ranked_lists, 60.0);

        // id1 at rank 0: 1/(60+1) = 1/61
        // id2 at rank 1: 1/(60+2) = 1/62
        let expected_id1 = 1.0 / 61.0;
        let expected_id2 = 1.0 / 62.0;

        assert!((*scores.get(&id1).unwrap() - expected_id1).abs() < 0.0001);
        assert!((*scores.get(&id2).unwrap() - expected_id2).abs() < 0.0001);

        println!("[VERIFIED] RRF single list: id1={:.6}, id2={:.6}",
            scores.get(&id1).unwrap(), scores.get(&id2).unwrap());
    }

    #[test]
    fn test_rrf_aggregation_multiple_lists() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let ranked_lists = vec![
            (0, vec![id1, id2, id3]), // Space 0: id1=rank0, id2=rank1, id3=rank2
            (1, vec![id2, id1, id3]), // Space 1: id2=rank0, id1=rank1, id3=rank2
            (12, vec![id1, id3, id2]), // Space 12 (SPLADE): id1=rank0, id3=rank1, id2=rank2
        ];

        let scores = AggregationStrategy::aggregate_rrf(&ranked_lists, 60.0);

        // id1 appears at ranks 0, 1, 0 -> 1/61 + 1/62 + 1/61 ≈ 0.0489
        // id2 appears at ranks 1, 0, 2 -> 1/62 + 1/61 + 1/63 ≈ 0.0484
        // id3 appears at ranks 2, 2, 1 -> 1/63 + 1/63 + 1/62 ≈ 0.0479

        let score1 = scores.get(&id1).unwrap();
        let score2 = scores.get(&id2).unwrap();
        let score3 = scores.get(&id3).unwrap();

        assert!(score1 > score2, "id1 should rank higher than id2");
        assert!(score2 > score3, "id2 should rank higher than id3");

        // Verify exact RRF formula
        let expected_id1 = 1.0 / 61.0 + 1.0 / 62.0 + 1.0 / 61.0;
        assert!(
            (score1 - expected_id1).abs() < 0.0001,
            "RRF for id1: expected {}, got {}",
            expected_id1,
            score1
        );

        println!(
            "[VERIFIED] RRF multiple lists: id1={:.4}, id2={:.4}, id3={:.4}",
            score1, score2, score3
        );
    }

    #[test]
    fn test_rrf_weighted() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let ranked_lists = vec![
            (0, vec![id1, id2]), // Space 0 weight=2.0
            (1, vec![id2, id1]), // Space 1 weight=1.0
        ];

        let mut weights = [1.0; NUM_EMBEDDERS];
        weights[0] = 2.0; // Double weight for space 0

        let scores = AggregationStrategy::aggregate_rrf_weighted(&ranked_lists, 60.0, &weights);

        // id1: 2.0/61 + 1.0/62 ≈ 0.0489
        // id2: 2.0/62 + 1.0/61 ≈ 0.0486

        let score1 = scores.get(&id1).unwrap();
        let score2 = scores.get(&id2).unwrap();

        assert!(score1 > score2, "id1 should rank higher due to higher weight in space 0");

        println!(
            "[VERIFIED] RRF weighted: id1={:.4}, id2={:.4}",
            score1, score2
        );
    }

    #[test]
    fn test_weighted_average() {
        let mut weights = [0.0; NUM_EMBEDDERS];
        weights[0] = 1.0; // E1 weight = 1.0
        weights[1] = 0.5; // E2 weight = 0.5

        let strategy = AggregationStrategy::WeightedAverage {
            weights,
            require_all: false,
        };

        let matches = vec![(0, 0.8), (1, 0.6)];
        let score = strategy.aggregate(&matches);

        // (0.8 * 1.0 + 0.6 * 0.5) / (1.0 + 0.5) = 1.1 / 1.5 = 0.7333...
        let expected = 1.1 / 1.5;
        assert!((score - expected).abs() < 0.001, "Expected {}, got {}", expected, score);

        println!("[VERIFIED] WeightedAverage: score={:.4}", score);
    }

    #[test]
    fn test_weighted_average_require_all() {
        let weights = [1.0; NUM_EMBEDDERS];
        let strategy = AggregationStrategy::WeightedAverage {
            weights,
            require_all: true,
        };

        // Only 2 matches, but require_all=true needs all 13
        let matches = vec![(0, 0.8), (1, 0.6)];
        let score = strategy.aggregate(&matches);

        assert_eq!(score, 0.0, "Should return 0 when require_all is true and not all spaces matched");

        println!("[VERIFIED] WeightedAverage require_all: score=0.0 when incomplete");
    }

    #[test]
    fn test_max_pooling() {
        let strategy = AggregationStrategy::MaxPooling;
        let matches = vec![(0, 0.8), (1, 0.6), (2, 0.9)];
        let score = strategy.aggregate(&matches);

        assert!((score - 0.9).abs() < 0.001);

        println!("[VERIFIED] MaxPooling: max={:.4}", score);
    }

    #[test]
    fn test_max_pooling_empty() {
        let strategy = AggregationStrategy::MaxPooling;
        let matches: Vec<(usize, f32)> = vec![];
        let score = strategy.aggregate(&matches);

        assert_eq!(score, 0.0);

        println!("[VERIFIED] MaxPooling empty: score=0.0");
    }

    #[test]
    fn test_purpose_weighted() {
        let purpose = PurposeVector::new([0.5; NUM_EMBEDDERS]);
        let strategy = AggregationStrategy::PurposeWeighted {
            purpose_vector: purpose,
        };

        let matches = vec![(0, 0.8), (1, 0.6)];
        let score = strategy.aggregate(&matches);

        // With equal weights: (0.8 * 0.5 + 0.6 * 0.5) / (0.5 + 0.5) = 0.7
        assert!((score - 0.7).abs() < 0.001);

        println!("[VERIFIED] PurposeWeighted: score={:.4}", score);
    }

    #[test]
    fn test_purpose_weighted_varied() {
        let mut alignments = [0.5; NUM_EMBEDDERS];
        alignments[0] = 0.9; // High alignment for space 0
        alignments[1] = 0.1; // Low alignment for space 1

        let purpose = PurposeVector::new(alignments);
        let strategy = AggregationStrategy::PurposeWeighted {
            purpose_vector: purpose,
        };

        let matches = vec![(0, 0.8), (1, 0.8)];
        let score = strategy.aggregate(&matches);

        // (0.8 * 0.9 + 0.8 * 0.1) / (0.9 + 0.1) = (0.72 + 0.08) / 1.0 = 0.8
        assert!((score - 0.8).abs() < 0.001);

        println!("[VERIFIED] PurposeWeighted varied: score={:.4}", score);
    }

    #[test]
    fn test_rrf_contribution() {
        let contrib = AggregationStrategy::rrf_contribution(0, 60.0);
        assert!((contrib - 1.0 / 61.0).abs() < 0.0001);

        let contrib2 = AggregationStrategy::rrf_contribution(5, 60.0);
        assert!((contrib2 - 1.0 / 66.0).abs() < 0.0001);

        println!("[VERIFIED] rrf_contribution: rank0={:.6}, rank5={:.6}", contrib, contrib2);
    }

    #[test]
    #[should_panic(expected = "RRF requires rank-based input")]
    fn test_rrf_aggregate_panics() {
        let strategy = AggregationStrategy::RRF { k: 60.0 };
        let matches = vec![(0, 0.8)];
        strategy.aggregate(&matches); // Should panic
    }

    #[test]
    fn test_default_is_rrf() {
        let strategy = AggregationStrategy::default();
        match strategy {
            AggregationStrategy::RRF { k } => {
                assert!((k - 60.0).abs() < 0.001);
            }
            _ => panic!("Default should be RRF"),
        }

        println!("[VERIFIED] Default strategy is RRF with k=60");
    }
}
