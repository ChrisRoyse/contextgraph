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

use crate::config::constants::similarity;
use crate::types::fingerprint::NUM_EMBEDDERS;
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
}

impl Default for AggregationStrategy {
    fn default() -> Self {
        // k=60 per constitution.yaml embeddings.similarity.rrf_constant
        Self::RRF {
            k: similarity::RRF_K,
        }
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
            Self::WeightedAverage {
                weights,
                require_all,
            } => {
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
            Self::MaxPooling => matches.iter().map(|(_, sim)| *sim).fold(0.0_f32, f32::max),
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
        // Pre-allocate for total IDs across all ranked lists to avoid reallocations
        let total_ids: usize = ranked_lists.iter().map(|(_, ids)| ids.len()).sum();
        let mut scores: HashMap<Uuid, f32> = HashMap::with_capacity(total_ids);

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
        // Pre-allocate for total IDs across all ranked lists to avoid reallocations
        let total_ids: usize = ranked_lists.iter().map(|(_, ids)| ids.len()).sum();
        let mut scores: HashMap<Uuid, f32> = HashMap::with_capacity(total_ids);

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

    /// Additive RRF with unique contribution tracking.
    ///
    /// Tracks which embedders found each result and identifies unique
    /// contributions (results found by only one embedder - blind spots).
    ///
    /// # Philosophy
    /// Each embedder looks at content from its unique perspective:
    /// - E1 finds semantically similar content
    /// - E11 finds entity-related content E1 missed
    /// - E5 finds causally-related content E1 missed
    /// - etc.
    ///
    /// When an embedder finds something NO OTHER embedder found, that's
    /// a "blind spot discovery" and gets a 10% boost.
    ///
    /// # Arguments
    /// - ranked_lists: Vec of (space_index, Vec<memory_id>) per space
    /// - k: RRF constant (default: 60)
    /// - unique_boost: Boost for blind spot discoveries (default: 0.1 = 10%)
    ///
    /// # Returns
    /// Vec of CombinedResult with full contribution tracking
    pub fn aggregate_rrf_additive(
        ranked_lists: &[(usize, Vec<Uuid>)],
        k: f32,
        unique_boost: f32,
    ) -> Vec<CombinedResult> {
        // Track contributions for each memory
        let mut contributions: HashMap<Uuid, CombinedResultBuilder> = HashMap::new();

        for (space_idx, ranked_ids) in ranked_lists {
            for (rank, memory_id) in ranked_ids.iter().enumerate() {
                let rrf_contribution = 1.0 / (k + (rank as f32) + 1.0);

                let builder = contributions
                    .entry(*memory_id)
                    .or_insert_with(|| CombinedResultBuilder::new(*memory_id));

                builder.add_contribution(*space_idx, rank, rrf_contribution);
            }
        }

        // Convert to results and apply unique boost
        let mut results: Vec<CombinedResult> = contributions
            .into_values()
            .map(|builder| {
                let mut result = builder.build();

                // Apply unique boost if found by only one embedder
                if result.unique_contribution {
                    result.boosted_score = result.rrf_score * (1.0 + unique_boost);
                } else {
                    result.boosted_score = result.rrf_score;
                }

                // Populate insight_annotation per improvement plan 5.5
                if result.unique_contribution {
                    result.insight_annotation = Some(format!(
                        "Blind-spot discovery: Only {} found this result (other embedders missed it)",
                        result.primary_embedder_name()
                    ));
                } else if result.found_by.len() >= 10 {
                    result.insight_annotation = Some(format!(
                        "Strong consensus: {}/{} embedders agree on this result",
                        result.found_by.len(),
                        NUM_EMBEDDERS
                    ));
                }

                result
            })
            .collect();

        // Sort by boosted score descending
        results.sort_by(|a, b| {
            b.boosted_score
                .partial_cmp(&a.boosted_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }
}

/// Result from additive RRF aggregation with contribution tracking.
#[derive(Debug, Clone)]
pub struct CombinedResult {
    /// Memory UUID.
    pub memory_id: Uuid,

    /// Base RRF score (before boost).
    pub rrf_score: f32,

    /// Boosted score (with unique contribution boost).
    pub boosted_score: f32,

    /// Set of embedder indices that found this result.
    pub found_by: std::collections::HashSet<usize>,

    /// The embedder with the highest contribution (lowest rank).
    pub primary_embedder: usize,

    /// Whether this was a unique contribution (found by only one embedder).
    /// These are "blind spot discoveries" - insights other embedders missed.
    pub unique_contribution: bool,

    /// Best rank achieved across all embedders.
    pub best_rank: usize,

    /// Human-readable insight annotation describing the contribution.
    pub insight_annotation: Option<String>,
}

impl CombinedResult {
    /// Get embedder name for the primary embedder.
    pub fn primary_embedder_name(&self) -> &'static str {
        match self.primary_embedder {
            0 => "E1_Semantic",
            1 => "E2_Temporal_Recent",
            2 => "E3_Temporal_Periodic",
            3 => "E4_Temporal_Positional",
            4 => "E5_Causal",
            5 => "E6_Sparse",
            6 => "E7_Code",
            7 => "E8_Graph",
            8 => "E9_HDC",
            9 => "E10_Multimodal",
            10 => "E11_Entity",
            11 => "E12_Late_Interaction",
            12 => "E13_SPLADE",
            13 => "E14_BgeM3Dense",
            _ => "Unknown",
        }
    }

    /// Number of embedders that found this result.
    pub fn embedder_count(&self) -> usize {
        self.found_by.len()
    }
}

/// Builder for CombinedResult to track contributions during aggregation.
struct CombinedResultBuilder {
    memory_id: Uuid,
    rrf_score: f32,
    found_by: std::collections::HashSet<usize>,
    best_rank: usize,
    primary_embedder: usize,
}

impl CombinedResultBuilder {
    fn new(memory_id: Uuid) -> Self {
        Self {
            memory_id,
            rrf_score: 0.0,
            found_by: std::collections::HashSet::new(),
            best_rank: usize::MAX,
            primary_embedder: 0,
        }
    }

    fn add_contribution(&mut self, embedder_idx: usize, rank: usize, contribution: f32) {
        self.rrf_score += contribution;
        self.found_by.insert(embedder_idx);

        // Track the embedder with the best (lowest) rank
        if rank < self.best_rank {
            self.best_rank = rank;
            self.primary_embedder = embedder_idx;
        }
    }

    fn build(self) -> CombinedResult {
        let unique_contribution = self.found_by.len() == 1;

        CombinedResult {
            memory_id: self.memory_id,
            rrf_score: self.rrf_score,
            boosted_score: self.rrf_score, // Will be updated with boost
            found_by: self.found_by,
            primary_embedder: self.primary_embedder,
            unique_contribution,
            best_rank: if self.best_rank == usize::MAX {
                0
            } else {
                self.best_rank
            },
            insight_annotation: None,
        }
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

        println!(
            "[VERIFIED] RRF single list: id1={:.6}, id2={:.6}",
            scores.get(&id1).unwrap(),
            scores.get(&id2).unwrap()
        );
    }

    #[test]
    fn test_rrf_aggregation_multiple_lists() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let ranked_lists = vec![
            (0, vec![id1, id2, id3]),  // Space 0: id1=rank0, id2=rank1, id3=rank2
            (1, vec![id2, id1, id3]),  // Space 1: id2=rank0, id1=rank1, id3=rank2
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

        assert!(
            score1 > score2,
            "id1 should rank higher due to higher weight in space 0"
        );

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
        assert!(
            (score - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            score
        );

        println!("[VERIFIED] WeightedAverage: score={:.4}", score);
    }

    #[test]
    fn test_weighted_average_require_all() {
        let weights = [1.0; NUM_EMBEDDERS];
        let strategy = AggregationStrategy::WeightedAverage {
            weights,
            require_all: true,
        };

        // Only 2 matches, but require_all=true needs all embedders.
        let matches = vec![(0, 0.8), (1, 0.6)];
        let score = strategy.aggregate(&matches);

        assert_eq!(
            score, 0.0,
            "Should return 0 when require_all is true and not all spaces matched"
        );

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
    fn test_rrf_contribution() {
        let contrib = AggregationStrategy::rrf_contribution(0, 60.0);
        assert!((contrib - 1.0 / 61.0).abs() < 0.0001);

        let contrib2 = AggregationStrategy::rrf_contribution(5, 60.0);
        assert!((contrib2 - 1.0 / 66.0).abs() < 0.0001);

        println!(
            "[VERIFIED] rrf_contribution: rank0={:.6}, rank5={:.6}",
            contrib, contrib2
        );
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

    // ========================================================================
    // ADDITIVE RRF TESTS
    // ========================================================================

    #[test]
    fn test_additive_rrf_basic() {
        println!("=== TEST: Additive RRF Basic ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        // id1: found by E1 and E5
        // id2: found only by E11 (unique contribution)
        // id3: found by E1, E5, E7
        let ranked_lists = vec![
            (0, vec![id1, id3]), // E1: id1=rank0, id3=rank1
            (4, vec![id1, id3]), // E5: id1=rank0, id3=rank1
            (6, vec![id3]),      // E7: only id3
            (10, vec![id2]),     // E11: only id2 (unique!)
        ];

        let results = AggregationStrategy::aggregate_rrf_additive(&ranked_lists, 60.0, 0.1);

        println!("Results:");
        for r in &results {
            println!(
                "  {}: rrf={:.4}, boosted={:.4}, unique={}, embedders={:?}",
                r.memory_id, r.rrf_score, r.boosted_score, r.unique_contribution, r.found_by
            );
        }

        // Find id2 (unique contribution)
        let id2_result = results.iter().find(|r| r.memory_id == id2).unwrap();
        assert!(id2_result.unique_contribution);
        assert_eq!(id2_result.embedder_count(), 1);
        assert!(id2_result.found_by.contains(&10)); // E11
        assert!(id2_result.boosted_score > id2_result.rrf_score);

        // Find id1 (not unique)
        let id1_result = results.iter().find(|r| r.memory_id == id1).unwrap();
        assert!(!id1_result.unique_contribution);
        assert_eq!(id1_result.embedder_count(), 2);
        assert!(id1_result.found_by.contains(&0)); // E1
        assert!(id1_result.found_by.contains(&4)); // E5

        println!("[VERIFIED] Additive RRF tracks unique contributions");
    }

    #[test]
    fn test_additive_rrf_unique_boost() {
        println!("=== TEST: Additive RRF Unique Boost ===");

        let id_unique = Uuid::new_v4();
        let id_common = Uuid::new_v4();

        // id_unique: found only by E11 at rank 0
        // id_common: found by E1, E5, E7 all at rank 0
        let ranked_lists = vec![
            (0, vec![id_common]),  // E1
            (4, vec![id_common]),  // E5
            (6, vec![id_common]),  // E7
            (10, vec![id_unique]), // E11 only
        ];

        let results = AggregationStrategy::aggregate_rrf_additive(&ranked_lists, 60.0, 0.1);

        let unique = results.iter().find(|r| r.memory_id == id_unique).unwrap();
        let common = results.iter().find(|r| r.memory_id == id_common).unwrap();

        // unique: rrf=1/61, boosted=1/61*1.1
        // common: rrf=3/61, boosted=3/61
        println!(
            "Unique: rrf={:.4}, boosted={:.4}",
            unique.rrf_score, unique.boosted_score
        );
        println!(
            "Common: rrf={:.4}, boosted={:.4}",
            common.rrf_score, common.boosted_score
        );

        assert!(
            unique.boosted_score > unique.rrf_score,
            "Unique should get boost"
        );
        assert_eq!(
            common.boosted_score, common.rrf_score,
            "Common should not get boost"
        );

        // Verify 10% boost for unique
        let expected_boost = unique.rrf_score * 1.1;
        assert!((unique.boosted_score - expected_boost).abs() < 0.0001);

        println!("[VERIFIED] Unique contributions get 10% boost");
    }

    #[test]
    fn test_additive_rrf_primary_embedder() {
        println!("=== TEST: Additive RRF Primary Embedder ===");

        let id = Uuid::new_v4();

        // id found by E1 at rank 5, E5 at rank 0, E7 at rank 3
        // Primary should be E5 (best rank)
        let ranked_lists = vec![
            (
                0,
                vec![
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    id,
                ],
            ), // E1 rank 5
            (4, vec![id]),                                                 // E5 rank 0
            (6, vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), id]), // E7 rank 3
        ];

        let results = AggregationStrategy::aggregate_rrf_additive(&ranked_lists, 60.0, 0.1);
        let result = results.iter().find(|r| r.memory_id == id).unwrap();

        assert_eq!(
            result.primary_embedder, 4,
            "Primary should be E5 (best rank)"
        );
        assert_eq!(result.best_rank, 0);
        assert_eq!(result.primary_embedder_name(), "E5_Causal");

        println!("[VERIFIED] Primary embedder is one with best rank");
    }

    #[test]
    fn test_additive_rrf_sorted_by_score() {
        println!("=== TEST: Additive RRF Sorted By Score ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let ranked_lists = vec![
            (0, vec![id1, id2, id3]), // E1: id1=rank0, id2=rank1, id3=rank2
            (4, vec![id1, id2]),      // E5: id1=rank0, id2=rank1
            (6, vec![id1]),           // E7: id1=rank0
        ];

        let results = AggregationStrategy::aggregate_rrf_additive(&ranked_lists, 60.0, 0.1);

        // Results should be sorted by boosted_score descending
        for i in 1..results.len() {
            assert!(
                results[i - 1].boosted_score >= results[i].boosted_score,
                "Results should be sorted by score descending"
            );
        }

        // id1 should be first (highest score - found by all 3 at rank 0)
        assert_eq!(results[0].memory_id, id1);

        println!("[VERIFIED] Results sorted by boosted score");
    }
}
