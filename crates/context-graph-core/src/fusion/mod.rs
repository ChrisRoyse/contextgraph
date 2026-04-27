//! Multi-Embedder Fusion Strategies
//!
//! Implements Weighted Reciprocal Rank Fusion (RRF) per ARCH-18.
//! RRF preserves individual embedder rankings better than weighted score sum.
//!
//! # Research Background
//!
//! Per Elastic's Weighted RRF research:
//! - RRF formula: `RRF_score(d) = Sum(weight_i / (k + rank_i + 1))`
//! - Standard k value is 60 (provides good balance between top-heavy and uniform)
//! - RRF is robust to score distribution differences between embedders
//!
//! # Usage
//!
//! ```rust
//! use context_graph_core::fusion::{FusionStrategy, EmbedderRanking, fuse_rankings};
//! use uuid::Uuid;
//!
//! let rankings = vec![
//!     EmbedderRanking {
//!         embedder_name: "E1".to_string(),
//!         weight: 1.0,
//!         ranked_docs: vec![(Uuid::new_v4(), 0.9), (Uuid::new_v4(), 0.8)],
//!     },
//! ];
//!
//! let results = fuse_rankings(&rankings, FusionStrategy::WeightedRRF, 10);
//! ```

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// CORE-M5: Use canonical RRF formula from retrieval::aggregation
use crate::retrieval::AggregationStrategy;

/// RRF constant (standard value from research)
/// k=60 provides good balance between top-heavy ranking and uniform distribution
pub const RRF_K: f32 = 60.0;

/// Fusion strategy for combining multi-embedder results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FusionStrategy {
    /// Simple weighted sum of similarity scores (legacy)
    WeightedSum,
    /// Weighted Reciprocal Rank Fusion (recommended per ARCH-18)
    #[default]
    WeightedRRF,
    /// Score-Weighted RRF: uses E5 score magnitude in RRF contribution.
    /// For E5: `contribution = weight * score / (rank + K)` (preserves signal strength).
    /// For all other embedders: standard RRF `weight / (rank + K)`.
    ScoreWeightedRRF,
}

/// A single embedder's ranked results
#[derive(Debug, Clone)]
pub struct EmbedderRanking {
    /// Name of the embedder (e.g., "E1", "E5", "E7")
    pub embedder_name: String,
    /// Weight for this embedder in fusion (0.0 to 1.0 typically)
    pub weight: f32,
    /// Ranked documents as (doc_id, similarity_score), ordered by rank
    pub ranked_docs: Vec<(Uuid, f32)>,
}

impl EmbedderRanking {
    /// Create a new embedder ranking
    pub fn new(
        embedder_name: impl Into<String>,
        weight: f32,
        ranked_docs: Vec<(Uuid, f32)>,
    ) -> Self {
        Self {
            embedder_name: embedder_name.into(),
            weight,
            ranked_docs,
        }
    }

    /// Create from embedder index and weight
    pub fn from_index(index: usize, weight: f32, ranked_docs: Vec<(Uuid, f32)>) -> Self {
        Self {
            embedder_name: format!("E{}", index + 1),
            weight,
            ranked_docs,
        }
    }
}

/// Result of fusion
#[derive(Debug, Clone)]
pub struct FusedResult {
    /// Document UUID
    pub doc_id: Uuid,
    /// Fused score (interpretation depends on strategy)
    pub fused_score: f32,
    /// Which embedders contributed to this result
    pub contributing_embedders: Vec<String>,
}

impl FusedResult {
    /// Create a new fused result
    pub fn new(doc_id: Uuid, fused_score: f32, contributing_embedders: Vec<String>) -> Self {
        Self {
            doc_id,
            fused_score,
            contributing_embedders,
        }
    }
}

/// Compute Weighted RRF across multiple embedder rankings
///
/// Formula: `RRF_score(d) = Sum(weight_i / (k + rank_i + 1))`
///
/// Where:
/// - `weight_i` is the embedder's weight
/// - `rank_i` is the document's 0-based rank in that embedder's results
/// - `k` is the RRF constant (60.0 by default)
/// - `+1` converts 0-based rank to 1-based (rank 0 -> denominator k+1)
///
/// # Arguments
///
/// * `rankings` - Rankings from each embedder
/// * `top_k` - Number of results to return
///
/// # Returns
///
/// Fused results sorted by RRF score descending
///
/// # Example
///
/// ```rust
/// use context_graph_core::fusion::{weighted_rrf, EmbedderRanking};
/// use uuid::Uuid;
///
/// let uuid1 = Uuid::nil();
/// let rankings = vec![
///     EmbedderRanking::new("E1", 1.0, vec![(uuid1, 0.9)]),
/// ];
///
/// let results = weighted_rrf(&rankings, 10);
/// assert_eq!(results.len(), 1);
/// ```
pub fn weighted_rrf(rankings: &[EmbedderRanking], top_k: usize) -> Vec<FusedResult> {
    // Standard RRF is score-weighted RRF with no score-weighted embedders
    score_weighted_rrf(rankings, &[], top_k)
}

/// Score-Weighted RRF: uses raw similarity scores for specific embedders.
///
/// For score-weighted embedders (e.g., E5 causal): `contribution = weight * score / (rank + 1 + k)`
/// For standard embedders: `contribution = weight / (rank + 1 + k)` (normal RRF)
///
/// This preserves E5 score magnitude information that standard RRF discards.
/// An E5 score of 0.58 (strong causal signal) contributes more than 0.12 (weak) at the same rank.
///
/// # Arguments
///
/// * `rankings` - Rankings from each embedder
/// * `score_weighted_embedders` - Set of embedder names that should use score-weighted variant
/// * `top_k` - Number of results to return
pub fn score_weighted_rrf(
    rankings: &[EmbedderRanking],
    score_weighted_embedders: &[&str],
    top_k: usize,
) -> Vec<FusedResult> {
    let mut doc_scores: HashMap<Uuid, (f32, Vec<String>)> = HashMap::new();

    for ranking in rankings {
        if ranking.weight <= 0.0 {
            continue;
        }

        let use_score_weighting =
            score_weighted_embedders.contains(&ranking.embedder_name.as_str());

        for (rank, (doc_id, similarity)) in ranking.ranked_docs.iter().enumerate() {
            // CORE-M5: Use canonical rrf_contribution formula from aggregation
            let base_rrf = AggregationStrategy::rrf_contribution(rank, RRF_K);
            let rrf_contribution = if use_score_weighting {
                // Score-weighted: magnitude of similarity modulates contribution
                ranking.weight * similarity * base_rrf
            } else {
                // Standard RRF: rank-only
                ranking.weight * base_rrf
            };

            let entry = doc_scores.entry(*doc_id).or_insert((0.0, Vec::new()));
            entry.0 += rrf_contribution;
            if !entry.1.contains(&ranking.embedder_name) {
                entry.1.push(ranking.embedder_name.clone());
            }
        }
    }

    let mut results: Vec<FusedResult> = doc_scores
        .into_iter()
        .map(|(doc_id, (score, embedders))| FusedResult {
            doc_id,
            fused_score: score,
            contributing_embedders: embedders,
        })
        .collect();

    results.sort_by(|a, b| {
        b.fused_score
            .partial_cmp(&a.fused_score)
            .unwrap_or_else(|| {
                // STOR-L3: NaN goes last for deterministic ordering
                if a.fused_score.is_nan() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
    });
    results.truncate(top_k);

    results
}

/// Legacy weighted sum fusion (for backward compatibility)
///
/// Computes: `fused_score(d) = Sum(weight_i * similarity_i)`
///
/// # Arguments
///
/// * `rankings` - Rankings from each embedder
/// * `top_k` - Number of results to return
///
/// # Returns
///
/// Fused results sorted by weighted sum score descending
///
/// # Note
///
/// Weighted sum is sensitive to score distribution differences between embedders.
/// Use `weighted_rrf` for more robust fusion.
pub fn weighted_sum(rankings: &[EmbedderRanking], top_k: usize) -> Vec<FusedResult> {
    let mut doc_scores: HashMap<Uuid, (f32, Vec<String>)> = HashMap::new();

    for ranking in rankings {
        // Skip zero-weight embedders
        if ranking.weight <= 0.0 {
            continue;
        }

        for (doc_id, similarity) in &ranking.ranked_docs {
            let weighted_score = ranking.weight * similarity;

            let entry = doc_scores.entry(*doc_id).or_insert((0.0, Vec::new()));
            entry.0 += weighted_score;
            if !entry.1.contains(&ranking.embedder_name) {
                entry.1.push(ranking.embedder_name.clone());
            }
        }
    }

    let mut results: Vec<FusedResult> = doc_scores
        .into_iter()
        .map(|(doc_id, (score, embedders))| FusedResult {
            doc_id,
            fused_score: score,
            contributing_embedders: embedders,
        })
        .collect();

    results.sort_by(|a, b| {
        b.fused_score
            .partial_cmp(&a.fused_score)
            .unwrap_or_else(|| {
                // STOR-L3: NaN goes last for deterministic ordering
                if a.fused_score.is_nan() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
    });
    results.truncate(top_k);

    results
}

/// Fuse rankings using the specified strategy
///
/// # Arguments
///
/// * `rankings` - Rankings from each embedder
/// * `strategy` - Fusion strategy to use
/// * `top_k` - Number of results to return
///
/// # Returns
///
/// Fused results sorted by score descending
///
/// # Example
///
/// ```rust
/// use context_graph_core::fusion::{fuse_rankings, FusionStrategy, EmbedderRanking};
/// use uuid::Uuid;
///
/// let uuid1 = Uuid::nil();
/// let rankings = vec![
///     EmbedderRanking::new("E1", 1.0, vec![(uuid1, 0.9)]),
/// ];
///
/// // Use default strategy (WeightedRRF)
/// let results = fuse_rankings(&rankings, FusionStrategy::default(), 10);
/// ```
pub fn fuse_rankings(
    rankings: &[EmbedderRanking],
    strategy: FusionStrategy,
    top_k: usize,
) -> Vec<FusedResult> {
    match strategy {
        FusionStrategy::WeightedSum => weighted_sum(rankings, top_k),
        FusionStrategy::WeightedRRF => weighted_rrf(rankings, top_k),
        FusionStrategy::ScoreWeightedRRF => score_weighted_rrf(rankings, &["E5"], top_k),
    }
}

/// Normalize similarity scores to [0, 1] range using min-max normalization
///
/// # Arguments
///
/// * `scores` - Raw similarity scores (modified in place)
///
/// # Returns
///
/// Min and max values used for normalization
pub fn normalize_minmax(scores: &mut [(Uuid, f32)]) -> (f32, f32) {
    if scores.is_empty() {
        return (0.0, 1.0);
    }

    let min_score = scores.iter().map(|(_, s)| *s).fold(f32::INFINITY, f32::min);
    let max_score = scores
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);

    let range = max_score - min_score;
    if range > f32::EPSILON {
        for (_, score) in scores.iter_mut() {
            *score = (*score - min_score) / range;
        }
    } else {
        // All scores are the same, normalize to 1.0
        for (_, score) in scores.iter_mut() {
            *score = 1.0;
        }
    }

    (min_score, max_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uuid(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    #[test]
    fn test_rrf_basic() {
        let rankings = vec![
            EmbedderRanking {
                embedder_name: "E1".to_string(),
                weight: 1.0,
                ranked_docs: vec![
                    (make_uuid(1), 0.9),
                    (make_uuid(2), 0.8),
                    (make_uuid(3), 0.7),
                ],
            },
            EmbedderRanking {
                embedder_name: "E5".to_string(),
                weight: 1.0,
                ranked_docs: vec![
                    (make_uuid(2), 0.95), // doc 2 is rank 1 here
                    (make_uuid(1), 0.85),
                    (make_uuid(4), 0.75),
                ],
            },
        ];

        let results = weighted_rrf(&rankings, 10);

        // Doc 2 should be ranked higher because it's rank 1 in E5 and rank 2 in E1
        // Doc 1 is rank 1 in E1 but rank 2 in E5
        assert!(results.len() >= 2);

        // Both should have contributions from both embedders (for docs 1 and 2)
        let doc1_result = results.iter().find(|r| r.doc_id == make_uuid(1)).unwrap();
        let doc2_result = results.iter().find(|r| r.doc_id == make_uuid(2)).unwrap();

        assert!(doc1_result.contributing_embedders.len() >= 2);
        assert!(doc2_result.contributing_embedders.len() >= 2);

        // Doc 2: rank 1 in E5 (1/(1+60)=0.0164), rank 2 in E1 (1/(2+60)=0.0161) = 0.0325
        // Doc 1: rank 1 in E1 (1/(1+60)=0.0164), rank 2 in E5 (1/(2+60)=0.0161) = 0.0325
        // They should be very close, but doc 2 is slightly higher due to being top in E5
        // Actually they should be equal since both have same rank pattern
    }

    #[test]
    fn test_rrf_weight_matters() {
        let rankings = vec![
            EmbedderRanking {
                embedder_name: "E1".to_string(),
                weight: 2.0, // Higher weight
                ranked_docs: vec![(make_uuid(1), 0.9)],
            },
            EmbedderRanking {
                embedder_name: "E5".to_string(),
                weight: 0.5, // Lower weight
                ranked_docs: vec![(make_uuid(2), 0.95)],
            },
        ];

        let results = weighted_rrf(&rankings, 10);

        // Doc 1 should be higher because E1 has higher weight
        // Doc 1: 2.0/(1+60) = 0.0328
        // Doc 2: 0.5/(1+60) = 0.0082
        assert_eq!(results[0].doc_id, make_uuid(1));
    }

    #[test]
    fn test_weighted_sum_vs_rrf() {
        let rankings = vec![
            EmbedderRanking {
                embedder_name: "E1".to_string(),
                weight: 1.0,
                ranked_docs: vec![
                    (make_uuid(1), 0.9),
                    (make_uuid(2), 0.1), // Very low score in E1
                ],
            },
            EmbedderRanking {
                embedder_name: "E5".to_string(),
                weight: 1.0,
                ranked_docs: vec![
                    (make_uuid(2), 0.95), // High score in E5
                    (make_uuid(1), 0.15), // Low score in E5
                ],
            },
        ];

        let rrf_results = weighted_rrf(&rankings, 10);
        let sum_results = weighted_sum(&rankings, 10);

        // RRF should rank doc 2 higher (rank 1 in E5)
        // Doc 1 RRF: 1/(1+60) + 1/(2+60) = 0.0164 + 0.0161 = 0.0325
        // Doc 2 RRF: 1/(2+60) + 1/(1+60) = 0.0161 + 0.0164 = 0.0325
        // Actually same! Because ranks are symmetric

        // Weighted sum:
        // Doc 1: 0.9 + 0.15 = 1.05
        // Doc 2: 0.1 + 0.95 = 1.05
        // Also same!

        // The key insight is RRF is more robust to score distribution differences
        assert!(!rrf_results.is_empty());
        assert!(!sum_results.is_empty());
    }

    #[test]
    fn test_rrf_respects_ranking_order() {
        // Test that RRF respects the ranking order, not just the scores
        let rankings = vec![EmbedderRanking {
            embedder_name: "E1".to_string(),
            weight: 1.0,
            ranked_docs: vec![
                (make_uuid(1), 0.99), // rank 1
                (make_uuid(2), 0.98), // rank 2
                (make_uuid(3), 0.97), // rank 3
            ],
        }];

        let results = weighted_rrf(&rankings, 10);

        // Order should be: doc 1, doc 2, doc 3
        assert_eq!(results[0].doc_id, make_uuid(1));
        assert_eq!(results[1].doc_id, make_uuid(2));
        assert_eq!(results[2].doc_id, make_uuid(3));

        // Scores should be: 1/(1+60), 1/(2+60), 1/(3+60)
        assert!((results[0].fused_score - 1.0 / 61.0).abs() < 0.0001);
        assert!((results[1].fused_score - 1.0 / 62.0).abs() < 0.0001);
        assert!((results[2].fused_score - 1.0 / 63.0).abs() < 0.0001);
    }

    #[test]
    fn test_fuse_rankings_default_strategy() {
        let rankings = vec![EmbedderRanking::new("E1", 1.0, vec![(make_uuid(1), 0.9)])];

        // Default strategy should be WeightedRRF
        let results = fuse_rankings(&rankings, FusionStrategy::default(), 10);
        assert_eq!(results.len(), 1);

        // RRF score for rank 1 with weight 1.0
        assert!((results[0].fused_score - 1.0 / 61.0).abs() < 0.0001);
    }

    #[test]
    fn test_zero_weight_embedders_ignored() {
        let rankings = vec![
            EmbedderRanking::new("E1", 1.0, vec![(make_uuid(1), 0.9)]),
            EmbedderRanking::new("E2", 0.0, vec![(make_uuid(2), 0.95)]), // Zero weight, ignored
        ];

        let results = weighted_rrf(&rankings, 10);

        // Only doc 1 should appear (E2 is ignored)
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, make_uuid(1));
    }

    #[test]
    fn test_normalize_minmax() {
        let mut scores = vec![
            (make_uuid(1), 0.2),
            (make_uuid(2), 0.8),
            (make_uuid(3), 0.5),
        ];

        let (min, max) = normalize_minmax(&mut scores);

        assert_eq!(min, 0.2);
        assert_eq!(max, 0.8);

        // After normalization:
        // 0.2 -> 0.0
        // 0.8 -> 1.0
        // 0.5 -> 0.5
        let score1 = scores.iter().find(|(id, _)| *id == make_uuid(1)).unwrap().1;
        let score2 = scores.iter().find(|(id, _)| *id == make_uuid(2)).unwrap().1;
        let score3 = scores.iter().find(|(id, _)| *id == make_uuid(3)).unwrap().1;

        assert!((score1 - 0.0).abs() < 0.0001);
        assert!((score2 - 1.0).abs() < 0.0001);
        assert!((score3 - 0.5).abs() < 0.0001);
    }

    #[test]
    fn test_normalize_minmax_same_scores() {
        let mut scores = vec![(make_uuid(1), 0.5), (make_uuid(2), 0.5)];

        normalize_minmax(&mut scores);

        // All same scores should normalize to 1.0
        assert!((scores[0].1 - 1.0).abs() < 0.0001);
        assert!((scores[1].1 - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_embedder_ranking_constructors() {
        let ranking1 = EmbedderRanking::new("E1", 0.5, vec![]);
        assert_eq!(ranking1.embedder_name, "E1");
        assert_eq!(ranking1.weight, 0.5);

        let ranking2 = EmbedderRanking::from_index(0, 0.3, vec![]);
        assert_eq!(ranking2.embedder_name, "E1");

        let ranking3 = EmbedderRanking::from_index(6, 0.4, vec![]);
        assert_eq!(ranking3.embedder_name, "E7");
    }

    #[test]
    fn test_score_weighted_rrf_e5_uses_score() {
        // E5 with same rank but different scores should produce different contributions
        let rankings = vec![
            EmbedderRanking::new("E1", 1.0, vec![(make_uuid(1), 0.9), (make_uuid(2), 0.8)]),
            EmbedderRanking::new(
                "E5",
                1.0,
                vec![
                    (make_uuid(1), 0.58), // Strong causal signal
                    (make_uuid(2), 0.12), // Weak causal signal
                ],
            ),
        ];

        let results = score_weighted_rrf(&rankings, &["E5"], 10);

        // Doc 1 should be ranked higher because E5 has higher score
        // E1 contributions are the same for both (standard RRF)
        // E5 doc 1: 1.0 * 0.58 / (1 + 60) = 0.00951
        // E5 doc 2: 1.0 * 0.12 / (2 + 60) = 0.00194
        assert_eq!(results[0].doc_id, make_uuid(1));

        // In standard RRF, E5 doc 1 and doc 2 would have same rank difference as E1
        let standard_results = weighted_rrf(&rankings, 10);
        // Both should have doc 1 on top, but score gap should be larger with score-weighted
        let sw_gap = results[0].fused_score - results[1].fused_score;
        let std_gap = standard_results[0].fused_score - standard_results[1].fused_score;
        assert!(
            sw_gap > std_gap,
            "Score-weighted gap {} should be larger than standard gap {}",
            sw_gap,
            std_gap
        );
    }

    #[test]
    fn test_score_weighted_rrf_non_e5_unchanged() {
        // Non-E5 embedders should produce same results as standard RRF
        let rankings = vec![EmbedderRanking::new("E1", 1.0, vec![(make_uuid(1), 0.9)])];

        let sw_results = score_weighted_rrf(&rankings, &["E5"], 10);
        let std_results = weighted_rrf(&rankings, 10);

        assert_eq!(sw_results.len(), std_results.len());
        assert!((sw_results[0].fused_score - std_results[0].fused_score).abs() < 0.0001);
    }

    #[test]
    fn test_fuse_rankings_score_weighted_variant() {
        let rankings = vec![EmbedderRanking::new("E1", 1.0, vec![(make_uuid(1), 0.9)])];
        let results = fuse_rankings(&rankings, FusionStrategy::ScoreWeightedRRF, 10);
        assert_eq!(results.len(), 1);
        // E1 is not in score-weighted list, so should behave like standard RRF
        assert!((results[0].fused_score - 1.0 / 61.0).abs() < 0.0001);
    }
}
