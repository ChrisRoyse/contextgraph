//! Weighted Reciprocal Rank Fusion (RRF) for multi-embedder search.
//!
//! Implements the fusion strategy per ARCH-21: "Multi-space fusion uses Weighted RRF,
//! NOT weighted sum."
//!
//! # RRF Formula
//!
//! For each result `r` with rank `k` in result set `i`:
//! ```text
//! score(r) = Σ (weight_i / (K + rank_i(r)))
//! ```
//!
//! Where:
//! - `K` is the RRF constant (default: 60, per constitution)
//! - `weight_i` is the embedder weight
//! - `rank_i(r)` is the rank of result `r` in result set `i` (0-indexed)
//!
//! # Why RRF Instead of Weighted Sum?
//!
//! RRF is more robust than weighted sum because:
//! 1. **Rank-based**: Uses rank positions, not raw scores, avoiding score miscalibration
//! 2. **Scale-invariant**: Different embedders can have different score distributions
//! 3. **Consensus-favoring**: Results appearing in multiple lists get boosted
//!
//! # Example
//!
//! ```ignore
//! let results = weighted_rrf_fusion(
//!     vec![
//!         (e1_results, 0.30),  // E1 semantic
//!         (e5_results, 0.35),  // E5 causal (highest weight)
//!         (e8_results, 0.15),  // E8 graph
//!         (e11_results, 0.20), // E11 entity
//!     ],
//!     60, // K constant
//! );
//! ```

use std::collections::HashMap;
use uuid::Uuid;

/// RRF constant K (per constitution.yaml).
///
/// This determines how quickly rank importance decays:
/// - Rank 0: 1/(60+0) = 0.0167
/// - Rank 1: 1/(60+1) = 0.0164
/// - Rank 10: 1/(60+10) = 0.0143
/// - Rank 100: 1/(60+100) = 0.00625
pub const RRF_K: f32 = 60.0;

/// Weighted Reciprocal Rank Fusion (RRF) for combining multiple result sets.
///
/// Fuses results from multiple embedders using rank-based scoring with
/// per-embedder weights. This is the preferred fusion method per ARCH-21.
///
/// # Arguments
///
/// * `result_sets` - Vector of (results, weight) tuples where:
///   - `results`: Vec<(Uuid, f32)> sorted by similarity descending
///   - `weight`: Embedder weight (should sum to ~1.0 across all sets)
/// * `k` - RRF constant (typically 60)
///
/// # Returns
///
/// Vector of (Uuid, RRF score) sorted by RRF score descending.
///
/// # Formula
///
/// For each result `r`:
/// ```text
/// rrf_score(r) = Σ (weight_i / (k + rank_i(r)))
/// ```
///
/// # Example
///
/// ```ignore
/// let fused = weighted_rrf_fusion(
///     vec![
///         (vec![(id1, 0.9), (id2, 0.8)], 0.35),  // E5 causal
///         (vec![(id2, 0.7), (id1, 0.6)], 0.30),  // E1 semantic
///     ],
///     60,
/// );
/// // id2 appears high in both lists, likely scores higher than id1
/// ```
pub fn weighted_rrf_fusion(
    result_sets: Vec<(Vec<(Uuid, f32)>, f32)>,
    k: i32,
) -> Vec<(Uuid, f32)> {
    let mut scores: HashMap<Uuid, f32> = HashMap::new();

    for (results, weight) in result_sets {
        for (rank, (id, _similarity)) in results.iter().enumerate() {
            // RRF formula: score = weight / (k + rank + 1)
            // Note: rank is 0-indexed, so we add 1 to match standard RRF definition
            let rrf_contribution = weight / (k as f32 + rank as f32 + 1.0);
            *scores.entry(*id).or_default() += rrf_contribution;
        }
    }

    // Sort by RRF score descending
    let mut sorted: Vec<_> = scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    sorted
}

/// RRF fusion with per-embedder score tracking.
///
/// Similar to `weighted_rrf_fusion` but also tracks individual embedder
/// contributions for consensus analysis.
///
/// # Returns
///
/// Vector of (Uuid, RRF score, per-embedder scores) sorted by RRF score descending.
pub fn weighted_rrf_fusion_with_scores(
    result_sets: Vec<(Vec<(Uuid, f32)>, f32, &str)>, // (results, weight, embedder_name)
    k: i32,
) -> Vec<(Uuid, f32, HashMap<String, f32>)> {
    let mut scores: HashMap<Uuid, f32> = HashMap::new();
    let mut per_embedder: HashMap<Uuid, HashMap<String, f32>> = HashMap::new();

    for (results, weight, embedder_name) in result_sets {
        for (rank, (id, similarity)) in results.iter().enumerate() {
            // RRF contribution
            let rrf_contribution = weight / (k as f32 + rank as f32 + 1.0);
            *scores.entry(*id).or_default() += rrf_contribution;

            // Track per-embedder similarity score
            per_embedder
                .entry(*id)
                .or_default()
                .insert(embedder_name.to_string(), *similarity);
        }
    }

    // Sort by RRF score descending
    let mut sorted: Vec<_> = scores
        .into_iter()
        .map(|(id, score)| {
            let embedder_scores = per_embedder.remove(&id).unwrap_or_default();
            (id, score, embedder_scores)
        })
        .collect();

    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    sorted
}

/// Compute consensus score from per-embedder results.
///
/// Counts how many embedders have this result in their top-K.
///
/// # Arguments
///
/// * `id` - Result ID to check
/// * `result_sets` - Result sets from each embedder
/// * `top_k` - How many top results to consider per embedder
///
/// # Returns
///
/// Fraction of embedders that have this result in their top-K [0.0, 1.0]
pub fn compute_consensus(id: Uuid, result_sets: &[(Vec<(Uuid, f32)>, f32)], top_k: usize) -> f32 {
    let total_embedders = result_sets.len();
    if total_embedders == 0 {
        return 0.0;
    }

    let mut count = 0;
    for (results, _weight) in result_sets {
        if results.iter().take(top_k).any(|(rid, _)| *rid == id) {
            count += 1;
        }
    }

    count as f32 / total_embedders as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_results(ids: Vec<u128>) -> Vec<(Uuid, f32)> {
        ids.into_iter()
            .enumerate()
            .map(|(rank, id)| {
                let similarity = 1.0 - (rank as f32 * 0.1); // Decreasing similarity
                (Uuid::from_u128(id), similarity)
            })
            .collect()
    }

    #[test]
    fn test_rrf_single_embedder() {
        let results = make_results(vec![1, 2, 3]);
        let fused = weighted_rrf_fusion(vec![(results, 1.0)], 60);

        assert_eq!(fused.len(), 3);
        assert_eq!(fused[0].0, Uuid::from_u128(1)); // Rank 0 should be first
        assert_eq!(fused[1].0, Uuid::from_u128(2)); // Rank 1 should be second
        assert_eq!(fused[2].0, Uuid::from_u128(3)); // Rank 2 should be third
    }

    #[test]
    fn test_rrf_multiple_embedders_consensus() {
        // ID 2 appears high in both lists, ID 1 only high in first
        let e1_results = make_results(vec![1, 2, 3]); // ID 1 rank 0, ID 2 rank 1
        let e5_results = make_results(vec![2, 3, 1]); // ID 2 rank 0, ID 1 rank 2

        let fused = weighted_rrf_fusion(
            vec![(e1_results, 0.5), (e5_results, 0.5)],
            60,
        );

        // ID 2 should rank higher due to appearing high in both lists
        assert_eq!(fused[0].0, Uuid::from_u128(2));
    }

    #[test]
    fn test_rrf_weighted_embedders() {
        // E5 has 2x weight of E1
        let e1_results = make_results(vec![1, 2]); // ID 1 rank 0 (E1)
        let e5_results = make_results(vec![2, 1]); // ID 2 rank 0 (E5)

        let fused = weighted_rrf_fusion(
            vec![(e1_results, 0.33), (e5_results, 0.67)], // E5 has 2x weight
            60,
        );

        // ID 2 should win due to higher E5 weight
        assert_eq!(fused[0].0, Uuid::from_u128(2));
    }

    #[test]
    fn test_consensus_all_agree() {
        let id = Uuid::from_u128(1);
        let result_sets = vec![
            (vec![(id, 0.9), (Uuid::from_u128(2), 0.5)], 0.25),
            (vec![(id, 0.8), (Uuid::from_u128(3), 0.4)], 0.25),
            (vec![(id, 0.7), (Uuid::from_u128(4), 0.3)], 0.25),
            (vec![(id, 0.6), (Uuid::from_u128(5), 0.2)], 0.25),
        ];

        let consensus = compute_consensus(id, &result_sets, 5);
        assert_eq!(consensus, 1.0); // All 4 embedders have ID in top 5
    }

    #[test]
    fn test_consensus_none_agree() {
        let id = Uuid::from_u128(100); // ID not in any result set
        let result_sets = vec![
            (vec![(Uuid::from_u128(1), 0.9)], 0.5),
            (vec![(Uuid::from_u128(2), 0.8)], 0.5),
        ];

        let consensus = compute_consensus(id, &result_sets, 5);
        assert_eq!(consensus, 0.0);
    }

    #[test]
    fn test_consensus_partial() {
        let id = Uuid::from_u128(1);
        let result_sets = vec![
            (vec![(id, 0.9)], 0.25),                      // Has ID
            (vec![(Uuid::from_u128(2), 0.8)], 0.25),      // Doesn't have ID
            (vec![(id, 0.7)], 0.25),                      // Has ID
            (vec![(Uuid::from_u128(3), 0.6)], 0.25),      // Doesn't have ID
        ];

        let consensus = compute_consensus(id, &result_sets, 5);
        assert_eq!(consensus, 0.5); // 2 out of 4 embedders
    }
}
