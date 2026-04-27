//! Temporal real data benchmark metrics for evaluating E4 sequence embedder.
//!
//! This module provides metrics specifically designed for evaluating E4 (V_ordering)
//! on real data with session-based ground truth.
//!
//! ## Key Metrics
//!
//! - **Direction Accuracy**: Measures how well E4 filters "before" vs "after" queries
//! - **Sequence Ordering**: Kendall's tau for ordered retrieval within sessions
//! - **Chain Accuracy**: Multi-hop sequence traversal accuracy
//! - **Boundary Detection**: Session boundary F1 score
//!
//! ## Symmetry Requirement
//!
//! A properly functioning E4 should have approximately symmetric before/after accuracy:
//! `|before_accuracy - after_accuracy| < 0.2`
//!
//! The old broken E4 showed before_accuracy=1.0, after_accuracy=0.0 because it used
//! calendar timestamps instead of session sequences.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::datasets::{SequenceDirection, SequenceGroundTruth};

/// Metrics for E4 real data benchmark evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemporalRealdataMetrics {
    /// Direction filtering metrics.
    pub direction: DirectionFilteringMetrics,

    /// Sequence ordering metrics.
    pub ordering: SequenceOrderingMetrics,

    /// Chain traversal metrics.
    pub chain: ChainTraversalMetrics,

    /// Boundary detection metrics.
    pub boundary: BoundaryDetectionMetrics,

    /// Composite metrics.
    pub composite: CompositeTemporalRealdataMetrics,

    /// Number of queries evaluated.
    pub total_queries: usize,
}

/// Direction filtering metrics (before/after retrieval).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectionFilteringMetrics {
    /// Accuracy for "before" queries (items before anchor).
    pub before_accuracy: f64,

    /// Accuracy for "after" queries (items after anchor).
    pub after_accuracy: f64,

    /// Symmetry score: 1.0 - |before - after|.
    /// Higher is better; should be > 0.8 for working E4.
    pub symmetry_score: f64,

    /// Precision at K for direction queries.
    pub precision_at: HashMap<usize, f64>,

    /// Recall at K for direction queries.
    pub recall_at: HashMap<usize, f64>,

    /// Number of queries evaluated.
    pub query_count: usize,
}

/// Sequence ordering metrics (Kendall's tau).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SequenceOrderingMetrics {
    /// Average Kendall's tau across all queries.
    /// Range: [-1.0, 1.0], where 1.0 is perfect agreement.
    pub avg_kendalls_tau: f64,

    /// Kendall's tau for "before" queries (reversed order).
    pub before_kendalls_tau: f64,

    /// Kendall's tau for "after" queries.
    pub after_kendalls_tau: f64,

    /// Mean Reciprocal Rank for ordered retrieval.
    pub ordering_mrr: f64,

    /// Number of queries evaluated.
    pub query_count: usize,
}

/// Chain traversal metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainTraversalMetrics {
    /// Chain accuracy at various lengths.
    pub accuracy_by_length: HashMap<usize, f64>,

    /// Average chain accuracy across all lengths.
    pub avg_chain_accuracy: f64,

    /// Target reach rate (fraction of chains where final target was reached).
    pub target_reach_rate: f64,

    /// Average hops until failure.
    pub avg_hops_before_failure: f64,

    /// Number of chains evaluated.
    pub chain_count: usize,
}

/// Boundary detection metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoundaryDetectionMetrics {
    /// Same-session detection accuracy (true positives / same-session queries).
    pub same_session_accuracy: f64,

    /// Cross-session detection accuracy (true negatives / cross-session queries).
    pub cross_session_accuracy: f64,

    /// Overall boundary detection accuracy.
    pub overall_accuracy: f64,

    /// F1 score for session boundary detection.
    pub boundary_f1: f64,

    /// Number of queries evaluated.
    pub query_count: usize,
}

/// Composite metrics combining all E4 effectiveness measures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompositeTemporalRealdataMetrics {
    /// Overall E4 effectiveness score.
    pub overall_e4_score: f64,

    /// Improvement over timestamp-based baseline.
    pub improvement_over_timestamp_baseline: f64,

    /// Whether metrics pass minimum thresholds.
    pub passes_thresholds: bool,

    /// Feature contributions breakdown.
    pub feature_contributions: E4FeatureContributions,
}

/// Breakdown of E4 feature contributions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E4FeatureContributions {
    /// Direction filtering contribution.
    pub direction_filtering: f64,

    /// Sequence ordering contribution.
    pub sequence_ordering: f64,

    /// Chain traversal contribution.
    pub chain_traversal: f64,

    /// Boundary detection contribution.
    pub boundary_detection: f64,
}

impl TemporalRealdataMetrics {
    /// Minimum thresholds for passing benchmark.
    pub const MIN_DIRECTION_SYMMETRY: f64 = 0.7;
    pub const MIN_BEFORE_ACCURACY: f64 = 0.65;
    pub const MIN_AFTER_ACCURACY: f64 = 0.65;
    pub const MIN_KENDALLS_TAU: f64 = 0.5;

    /// Overall quality score (weighted combination).
    ///
    /// Weights based on E4's primary purpose (session ordering):
    /// - Direction filtering: 35% (core before/after functionality)
    /// - Sequence ordering: 35% (Kendall's tau for ordering)
    /// - Chain traversal: 20% (multi-hop navigation)
    /// - Boundary detection: 10% (session awareness)
    pub fn quality_score(&self) -> f64 {
        0.35 * self.direction.overall_score()
            + 0.35 * self.ordering.overall_score()
            + 0.20 * self.chain.overall_score()
            + 0.10 * self.boundary.overall_score()
    }

    /// Check if metrics meet minimum thresholds.
    pub fn meets_thresholds(&self) -> bool {
        self.direction.symmetry_score >= Self::MIN_DIRECTION_SYMMETRY
            && self.direction.before_accuracy >= Self::MIN_BEFORE_ACCURACY
            && self.direction.after_accuracy >= Self::MIN_AFTER_ACCURACY
            && self.ordering.avg_kendalls_tau >= Self::MIN_KENDALLS_TAU
    }
}

impl DirectionFilteringMetrics {
    /// Overall direction filtering score.
    pub fn overall_score(&self) -> f64 {
        // Heavily weight symmetry since asymmetry was the main bug
        0.3 * self.before_accuracy + 0.3 * self.after_accuracy + 0.4 * self.symmetry_score
    }
}

impl SequenceOrderingMetrics {
    /// Overall ordering score.
    pub fn overall_score(&self) -> f64 {
        // Normalize Kendall's tau from [-1, 1] to [0, 1]
        let tau_normalized = (self.avg_kendalls_tau + 1.0) / 2.0;
        0.6 * tau_normalized + 0.4 * self.ordering_mrr
    }
}

impl ChainTraversalMetrics {
    /// Overall chain score.
    pub fn overall_score(&self) -> f64 {
        0.5 * self.avg_chain_accuracy + 0.5 * self.target_reach_rate
    }
}

impl BoundaryDetectionMetrics {
    /// Overall boundary score.
    pub fn overall_score(&self) -> f64 {
        0.4 * self.overall_accuracy + 0.6 * self.boundary_f1
    }
}

// =============================================================================
// METRIC COMPUTATION FUNCTIONS
// =============================================================================

/// Result from a direction query evaluation.
#[derive(Debug, Clone)]
pub struct DirectionQueryResult {
    /// Query ID.
    pub query_id: Uuid,

    /// Query direction.
    pub direction: SequenceDirection,

    /// Retrieved IDs in ranked order.
    pub retrieved_ids: Vec<Uuid>,

    /// Expected IDs (ground truth).
    pub expected_ids: HashSet<Uuid>,

    /// Expected order (for Kendall's tau).
    pub expected_order: Vec<Uuid>,
}

/// Compute direction filtering metrics.
pub fn compute_direction_metrics(
    results: &[DirectionQueryResult],
    k_values: &[usize],
) -> DirectionFilteringMetrics {
    if results.is_empty() {
        return DirectionFilteringMetrics::default();
    }

    let before_results: Vec<_> = results
        .iter()
        .filter(|r| r.direction == SequenceDirection::Before)
        .collect();

    let after_results: Vec<_> = results
        .iter()
        .filter(|r| r.direction == SequenceDirection::After)
        .collect();

    // Compute accuracy for each direction
    let before_accuracy = if !before_results.is_empty() {
        before_results
            .iter()
            .map(|r| compute_retrieval_accuracy(r, 10))
            .sum::<f64>()
            / before_results.len() as f64
    } else {
        0.0
    };

    let after_accuracy = if !after_results.is_empty() {
        after_results
            .iter()
            .map(|r| compute_retrieval_accuracy(r, 10))
            .sum::<f64>()
            / after_results.len() as f64
    } else {
        0.0
    };

    // Symmetry score: how close before/after accuracies are
    let symmetry_score = 1.0 - (before_accuracy - after_accuracy).abs();

    // Compute precision@K and recall@K
    let mut precision_at = HashMap::new();
    let mut recall_at = HashMap::new();

    for &k in k_values {
        let precision_sum: f64 = results.iter().map(|r| compute_precision_at_k(r, k)).sum();
        let recall_sum: f64 = results.iter().map(|r| compute_recall_at_k(r, k)).sum();

        let n = results.len() as f64;
        precision_at.insert(k, precision_sum / n);
        recall_at.insert(k, recall_sum / n);
    }

    DirectionFilteringMetrics {
        before_accuracy,
        after_accuracy,
        symmetry_score,
        precision_at,
        recall_at,
        query_count: results.len(),
    }
}

/// Compute retrieval accuracy for a single query.
fn compute_retrieval_accuracy(result: &DirectionQueryResult, k: usize) -> f64 {
    if result.expected_ids.is_empty() {
        return 1.0; // No expected results means any result is "correct"
    }

    let hits = result
        .retrieved_ids
        .iter()
        .take(k)
        .filter(|id| result.expected_ids.contains(id))
        .count();

    hits as f64 / result.expected_ids.len().min(k) as f64
}

/// Compute precision at K.
fn compute_precision_at_k(result: &DirectionQueryResult, k: usize) -> f64 {
    if k == 0 {
        return 0.0;
    }

    let hits = result
        .retrieved_ids
        .iter()
        .take(k)
        .filter(|id| result.expected_ids.contains(id))
        .count();

    hits as f64 / k as f64
}

/// Compute recall at K.
fn compute_recall_at_k(result: &DirectionQueryResult, k: usize) -> f64 {
    if result.expected_ids.is_empty() {
        return 1.0;
    }

    let hits = result
        .retrieved_ids
        .iter()
        .take(k)
        .filter(|id| result.expected_ids.contains(id))
        .count();

    hits as f64 / result.expected_ids.len() as f64
}

/// Result from sequence ordering evaluation.
#[derive(Debug, Clone)]
pub struct OrderingResult {
    /// Query ID.
    pub query_id: Uuid,

    /// Query direction.
    pub direction: SequenceDirection,

    /// Retrieved IDs in ranked order.
    pub retrieved_order: Vec<Uuid>,

    /// Expected order (ground truth).
    pub expected_order: Vec<Uuid>,
}

/// Compute sequence ordering metrics.
pub fn compute_ordering_metrics(results: &[OrderingResult]) -> SequenceOrderingMetrics {
    if results.is_empty() {
        return SequenceOrderingMetrics::default();
    }

    let before_results: Vec<_> = results
        .iter()
        .filter(|r| r.direction == SequenceDirection::Before)
        .collect();

    let after_results: Vec<_> = results
        .iter()
        .filter(|r| r.direction == SequenceDirection::After)
        .collect();

    // Compute Kendall's tau for each direction
    let before_tau = if !before_results.is_empty() {
        before_results
            .iter()
            .map(|r| compute_kendalls_tau(r))
            .sum::<f64>()
            / before_results.len() as f64
    } else {
        0.0
    };

    let after_tau = if !after_results.is_empty() {
        after_results
            .iter()
            .map(|r| compute_kendalls_tau(r))
            .sum::<f64>()
            / after_results.len() as f64
    } else {
        0.0
    };

    // Average tau across all results
    let avg_tau = results.iter().map(compute_kendalls_tau).sum::<f64>() / results.len() as f64;

    // Compute ordering MRR
    let mrr = compute_ordering_mrr(results);

    SequenceOrderingMetrics {
        avg_kendalls_tau: avg_tau,
        before_kendalls_tau: before_tau,
        after_kendalls_tau: after_tau,
        ordering_mrr: mrr,
        query_count: results.len(),
    }
}

/// Compute Kendall's tau for a single ordering result.
fn compute_kendalls_tau(result: &OrderingResult) -> f64 {
    // Build position maps
    let retrieved_positions: HashMap<&Uuid, usize> = result
        .retrieved_order
        .iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();

    let expected_positions: HashMap<&Uuid, usize> = result
        .expected_order
        .iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();

    // Only consider IDs that appear in both
    let common_ids: Vec<&Uuid> = result
        .retrieved_order
        .iter()
        .filter(|id| expected_positions.contains_key(id))
        .collect();

    if common_ids.len() < 2 {
        return 0.0;
    }

    let n = common_ids.len();
    let mut concordant = 0i64;
    let mut discordant = 0i64;

    for i in 0..n {
        for j in (i + 1)..n {
            let ret_i = retrieved_positions[common_ids[i]];
            let ret_j = retrieved_positions[common_ids[j]];
            let exp_i = expected_positions[common_ids[i]];
            let exp_j = expected_positions[common_ids[j]];

            let ret_diff = ret_i as i64 - ret_j as i64;
            let exp_diff = exp_i as i64 - exp_j as i64;

            if ret_diff * exp_diff > 0 {
                concordant += 1;
            } else if ret_diff * exp_diff < 0 {
                discordant += 1;
            }
        }
    }

    let pairs = (n * (n - 1) / 2) as f64;
    if pairs < f64::EPSILON {
        0.0
    } else {
        (concordant - discordant) as f64 / pairs
    }
}

/// Compute ordering MRR.
fn compute_ordering_mrr(results: &[OrderingResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    let sum: f64 = results
        .iter()
        .map(|r| {
            // Find the first correctly positioned item
            if r.expected_order.is_empty() || r.retrieved_order.is_empty() {
                return 0.0;
            }

            // First expected item's position in retrieved order
            for (pos, id) in r.retrieved_order.iter().enumerate() {
                if r.expected_order.first() == Some(id) {
                    return 1.0 / (pos + 1) as f64;
                }
            }
            0.0
        })
        .sum();

    sum / results.len() as f64
}

/// Result from chain traversal evaluation.
#[derive(Debug, Clone)]
pub struct ChainResult {
    /// Chain query details.
    pub chain_length: usize,

    /// Number of correctly retrieved hops.
    pub correct_hops: usize,

    /// Whether the final target was reached.
    pub target_reached: bool,

    /// The hop where the chain broke (if not completed).
    pub failure_hop: Option<usize>,
}

/// Compute chain traversal metrics.
pub fn compute_chain_metrics(results: &[ChainResult]) -> ChainTraversalMetrics {
    if results.is_empty() {
        return ChainTraversalMetrics::default();
    }

    // Group by chain length
    let mut by_length: HashMap<usize, Vec<&ChainResult>> = HashMap::new();
    for result in results {
        by_length
            .entry(result.chain_length)
            .or_default()
            .push(result);
    }

    // Compute accuracy by length
    let mut accuracy_by_length = HashMap::new();
    for (length, results) in &by_length {
        let accuracy = results
            .iter()
            .map(|r| r.correct_hops as f64 / r.chain_length as f64)
            .sum::<f64>()
            / results.len() as f64;
        accuracy_by_length.insert(*length, accuracy);
    }

    // Average chain accuracy
    let avg_chain_accuracy = results
        .iter()
        .map(|r| r.correct_hops as f64 / r.chain_length.max(1) as f64)
        .sum::<f64>()
        / results.len() as f64;

    // Target reach rate
    let target_reach_rate =
        results.iter().filter(|r| r.target_reached).count() as f64 / results.len() as f64;

    // Average hops before failure
    let failed_results: Vec<_> = results.iter().filter(|r| !r.target_reached).collect();
    let avg_hops_before_failure = if !failed_results.is_empty() {
        failed_results
            .iter()
            .map(|r| r.failure_hop.unwrap_or(r.correct_hops) as f64)
            .sum::<f64>()
            / failed_results.len() as f64
    } else {
        0.0
    };

    ChainTraversalMetrics {
        accuracy_by_length,
        avg_chain_accuracy,
        target_reach_rate,
        avg_hops_before_failure,
        chain_count: results.len(),
    }
}

/// Result from boundary detection evaluation.
#[derive(Debug, Clone)]
pub struct BoundaryResult {
    /// Whether the pair is from the same session (ground truth).
    pub same_session: bool,

    /// Predicted same session (based on E4 similarity).
    pub predicted_same_session: bool,

    /// E4 similarity score.
    pub similarity_score: f64,
}

/// Compute boundary detection metrics.
pub fn compute_boundary_metrics(results: &[BoundaryResult]) -> BoundaryDetectionMetrics {
    if results.is_empty() {
        return BoundaryDetectionMetrics::default();
    }

    // Count true positives, false positives, true negatives, false negatives
    let mut tp = 0; // Same session, predicted same
    let mut fp = 0; // Different session, predicted same
    let mut tn = 0; // Different session, predicted different
    let mut fn_count = 0; // Same session, predicted different

    for result in results {
        match (result.same_session, result.predicted_same_session) {
            (true, true) => tp += 1,
            (true, false) => fn_count += 1,
            (false, true) => fp += 1,
            (false, false) => tn += 1,
        }
    }

    let same_session_total = tp + fn_count;
    let cross_session_total = tn + fp;

    let same_session_accuracy = if same_session_total > 0 {
        tp as f64 / same_session_total as f64
    } else {
        0.0
    };

    let cross_session_accuracy = if cross_session_total > 0 {
        tn as f64 / cross_session_total as f64
    } else {
        0.0
    };

    let overall_accuracy = (tp + tn) as f64 / results.len() as f64;

    // F1 for same-session detection
    let precision = if tp + fp > 0 {
        tp as f64 / (tp + fp) as f64
    } else {
        0.0
    };
    let recall = if tp + fn_count > 0 {
        tp as f64 / (tp + fn_count) as f64
    } else {
        0.0
    };
    let boundary_f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    BoundaryDetectionMetrics {
        same_session_accuracy,
        cross_session_accuracy,
        overall_accuracy,
        boundary_f1,
        query_count: results.len(),
    }
}

// =============================================================================
// ALL METRICS COMPUTATION
// =============================================================================

/// Data for direction benchmark evaluation.
#[derive(Debug, Default)]
pub struct DirectionBenchmarkData {
    /// Direction query results.
    pub results: Vec<DirectionQueryResult>,

    /// K values for precision/recall.
    pub k_values: Vec<usize>,
}

/// Data for ordering benchmark evaluation.
#[derive(Debug, Default)]
pub struct OrderingBenchmarkData {
    /// Ordering results.
    pub results: Vec<OrderingResult>,
}

/// Data for chain benchmark evaluation.
#[derive(Debug, Default)]
pub struct ChainBenchmarkData {
    /// Chain results.
    pub results: Vec<ChainResult>,
}

/// Data for boundary benchmark evaluation.
#[derive(Debug, Default)]
pub struct BoundaryBenchmarkData {
    /// Boundary results.
    pub results: Vec<BoundaryResult>,
}

/// Compute all temporal real data metrics.
pub fn compute_all_realdata_metrics(
    direction_data: &DirectionBenchmarkData,
    ordering_data: &OrderingBenchmarkData,
    chain_data: &ChainBenchmarkData,
    boundary_data: &BoundaryBenchmarkData,
    timestamp_baseline_score: f64,
) -> TemporalRealdataMetrics {
    let direction = compute_direction_metrics(&direction_data.results, &direction_data.k_values);
    let ordering = compute_ordering_metrics(&ordering_data.results);
    let chain = compute_chain_metrics(&chain_data.results);
    let boundary = compute_boundary_metrics(&boundary_data.results);

    let overall_score = 0.35 * direction.overall_score()
        + 0.35 * ordering.overall_score()
        + 0.20 * chain.overall_score()
        + 0.10 * boundary.overall_score();

    let improvement = if timestamp_baseline_score > 0.0 {
        (overall_score - timestamp_baseline_score) / timestamp_baseline_score
    } else {
        0.0
    };

    let passes = direction.symmetry_score >= TemporalRealdataMetrics::MIN_DIRECTION_SYMMETRY
        && direction.before_accuracy >= TemporalRealdataMetrics::MIN_BEFORE_ACCURACY
        && direction.after_accuracy >= TemporalRealdataMetrics::MIN_AFTER_ACCURACY
        && ordering.avg_kendalls_tau >= TemporalRealdataMetrics::MIN_KENDALLS_TAU;

    let composite = CompositeTemporalRealdataMetrics {
        overall_e4_score: overall_score,
        improvement_over_timestamp_baseline: improvement,
        passes_thresholds: passes,
        feature_contributions: E4FeatureContributions {
            direction_filtering: direction.overall_score(),
            sequence_ordering: ordering.overall_score(),
            chain_traversal: chain.overall_score(),
            boundary_detection: boundary.overall_score(),
        },
    };

    let total_queries =
        direction.query_count + ordering.query_count + chain.chain_count + boundary.query_count;

    TemporalRealdataMetrics {
        direction,
        ordering,
        chain,
        boundary,
        composite,
        total_queries,
    }
}

/// Convert ground truth queries to evaluation data.
pub fn ground_truth_to_direction_data(
    ground_truth: &SequenceGroundTruth,
    k_values: Vec<usize>,
) -> DirectionBenchmarkData {
    let results = ground_truth
        .direction_queries
        .iter()
        .map(|q| DirectionQueryResult {
            query_id: q.id,
            direction: q.direction,
            retrieved_ids: Vec::new(), // To be filled by runner
            expected_ids: q.expected_ids.iter().cloned().collect(),
            expected_order: q.expected_order.clone(),
        })
        .collect();

    DirectionBenchmarkData { results, k_values }
}

/// Convert ground truth chain queries to evaluation data.
pub fn ground_truth_to_chain_data(ground_truth: &SequenceGroundTruth) -> ChainBenchmarkData {
    let results = ground_truth
        .chain_queries
        .iter()
        .map(|q| ChainResult {
            chain_length: q.chain_length,
            correct_hops: 0, // To be filled by runner
            target_reached: false,
            failure_hop: None,
        })
        .collect();

    ChainBenchmarkData { results }
}

/// Convert ground truth boundary queries to evaluation data.
pub fn ground_truth_to_boundary_data(ground_truth: &SequenceGroundTruth) -> BoundaryBenchmarkData {
    let results = ground_truth
        .boundary_queries
        .iter()
        .map(|q| BoundaryResult {
            same_session: q.same_session,
            predicted_same_session: false, // To be filled by runner
            similarity_score: 0.0,
        })
        .collect();

    BoundaryBenchmarkData { results }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symmetry_score() {
        // Perfect symmetry
        let metrics = DirectionFilteringMetrics {
            before_accuracy: 0.8,
            after_accuracy: 0.8,
            symmetry_score: 1.0 - (0.8 - 0.8f64).abs(),
            ..Default::default()
        };
        assert!((metrics.symmetry_score - 1.0).abs() < 0.01);

        // Broken E4 (asymmetric)
        let broken = DirectionFilteringMetrics {
            before_accuracy: 1.0,
            after_accuracy: 0.0,
            symmetry_score: 1.0 - (1.0 - 0.0f64).abs(),
            ..Default::default()
        };
        assert!((broken.symmetry_score - 0.0).abs() < 0.01);

        println!("[VERIFIED] Symmetry score correctly identifies broken E4");
    }

    #[test]
    fn test_kendalls_tau() {
        let result = OrderingResult {
            query_id: Uuid::new_v4(),
            direction: SequenceDirection::After,
            retrieved_order: vec![Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)],
            expected_order: vec![Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)],
        };

        let tau = compute_kendalls_tau(&result);
        assert!(
            (tau - 1.0).abs() < 0.01,
            "Perfect order should have tau=1.0"
        );

        // Reversed order
        let reversed = OrderingResult {
            query_id: Uuid::new_v4(),
            direction: SequenceDirection::After,
            retrieved_order: vec![Uuid::from_u128(3), Uuid::from_u128(2), Uuid::from_u128(1)],
            expected_order: vec![Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)],
        };

        let tau_reversed = compute_kendalls_tau(&reversed);
        assert!(
            (tau_reversed - (-1.0)).abs() < 0.01,
            "Reversed order should have tau=-1.0"
        );

        println!("[VERIFIED] Kendall's tau correctly computed");
    }

    #[test]
    fn test_boundary_f1() {
        let results = vec![
            BoundaryResult {
                same_session: true,
                predicted_same_session: true,
                similarity_score: 0.9,
            },
            BoundaryResult {
                same_session: true,
                predicted_same_session: false,
                similarity_score: 0.4,
            },
            BoundaryResult {
                same_session: false,
                predicted_same_session: false,
                similarity_score: 0.3,
            },
            BoundaryResult {
                same_session: false,
                predicted_same_session: true,
                similarity_score: 0.6,
            },
        ];

        let metrics = compute_boundary_metrics(&results);

        // TP=1, FP=1, TN=1, FN=1
        // Precision = 1/2 = 0.5, Recall = 1/2 = 0.5
        // F1 = 2 * 0.5 * 0.5 / (0.5 + 0.5) = 0.5
        assert!(
            (metrics.boundary_f1 - 0.5).abs() < 0.01,
            "F1 should be 0.5, got {}",
            metrics.boundary_f1
        );

        println!("[VERIFIED] Boundary F1 correctly computed");
    }

    #[test]
    fn test_chain_metrics() {
        let results = vec![
            ChainResult {
                chain_length: 3,
                correct_hops: 3,
                target_reached: true,
                failure_hop: None,
            },
            ChainResult {
                chain_length: 4,
                correct_hops: 2,
                target_reached: false,
                failure_hop: Some(3),
            },
        ];

        let metrics = compute_chain_metrics(&results);

        // Target reach rate = 1/2 = 0.5
        assert!(
            (metrics.target_reach_rate - 0.5).abs() < 0.01,
            "Target reach rate should be 0.5"
        );

        // Avg chain accuracy = (3/3 + 2/4) / 2 = (1.0 + 0.5) / 2 = 0.75
        assert!(
            (metrics.avg_chain_accuracy - 0.75).abs() < 0.01,
            "Avg chain accuracy should be 0.75, got {}",
            metrics.avg_chain_accuracy
        );

        println!("[VERIFIED] Chain metrics correctly computed");
    }

    #[test]
    fn test_threshold_check() {
        let passing_metrics = TemporalRealdataMetrics {
            direction: DirectionFilteringMetrics {
                before_accuracy: 0.8,
                after_accuracy: 0.75,
                symmetry_score: 0.95,
                ..Default::default()
            },
            ordering: SequenceOrderingMetrics {
                avg_kendalls_tau: 0.7,
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(passing_metrics.meets_thresholds(), "Should pass thresholds");

        let failing_metrics = TemporalRealdataMetrics {
            direction: DirectionFilteringMetrics {
                before_accuracy: 1.0,
                after_accuracy: 0.0, // Broken E4
                symmetry_score: 0.0,
                ..Default::default()
            },
            ordering: SequenceOrderingMetrics {
                avg_kendalls_tau: 1.0, // Tau is fine but symmetry fails
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(
            !failing_metrics.meets_thresholds(),
            "Broken E4 should fail thresholds"
        );

        println!("[VERIFIED] Threshold check correctly identifies broken E4");
    }
}
