//! Causal benchmark metrics for evaluating E5 embedder effectiveness.
//!
//! This module provides comprehensive metrics for evaluating causal retrieval:
//!
//! - **Direction Detection**: Accuracy, precision, recall for cause vs effect classification
//! - **Asymmetric Retrieval**: MRR with direction awareness, asymmetry ratio
//! - **Causal Reasoning**: COPA-style accuracy, chain traversal, Kendall's tau
//!
//! ## Key Concepts
//!
//! - **Direction Modifier**: 1.2 for cause→effect, 0.8 for effect→cause
//! - **Asymmetry Ratio**: Should be ~1.5 (1.2/0.8) when working correctly
//! - **Intervention Overlap**: Jaccard similarity of intervened variables
//!
//! ## Formula (per Constitution)
//!
//! ```text
//! sim = base_cos × direction_mod × (0.7 + 0.3 × intervention_overlap)
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export CausalDirection from core for convenience
pub use context_graph_core::causal::asymmetric::CausalDirection;

/// Causal metrics for E5 embedder evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CausalMetrics {
    /// Direction detection metrics.
    pub direction: DirectionDetectionMetrics,

    /// Asymmetric retrieval metrics.
    pub asymmetric: AsymmetricRetrievalMetrics,

    /// Causal reasoning metrics.
    pub reasoning: CausalReasoningMetrics,

    /// Composite metrics.
    pub composite: CompositeCausalMetrics,

    /// Number of queries used to compute these metrics.
    pub query_count: usize,
}

/// Direction detection metrics: P/R/F1 for cause vs effect classification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectionDetectionMetrics {
    /// Overall accuracy (correct classifications / total).
    pub accuracy: f64,

    /// Precision for detecting "cause" queries.
    pub cause_precision: f64,

    /// Recall for detecting "cause" queries.
    pub cause_recall: f64,

    /// Precision for detecting "effect" queries.
    pub effect_precision: f64,

    /// Recall for detecting "effect" queries.
    pub effect_recall: f64,

    /// F1 score for direction detection overall.
    pub direction_f1: f64,

    /// Confusion matrix for detailed analysis.
    pub confusion_matrix: DirectionConfusionMatrix,
}

/// Confusion matrix for direction detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectionConfusionMatrix {
    /// True Cause predicted as Cause.
    pub true_cause_pred_cause: usize,

    /// True Cause predicted as Effect.
    pub true_cause_pred_effect: usize,

    /// True Cause predicted as Unknown.
    pub true_cause_pred_unknown: usize,

    /// True Effect predicted as Cause.
    pub true_effect_pred_cause: usize,

    /// True Effect predicted as Effect.
    pub true_effect_pred_effect: usize,

    /// True Effect predicted as Unknown.
    pub true_effect_pred_unknown: usize,

    /// True Unknown predicted as Cause.
    pub true_unknown_pred_cause: usize,

    /// True Unknown predicted as Effect.
    pub true_unknown_pred_effect: usize,

    /// True Unknown predicted as Unknown.
    pub true_unknown_pred_unknown: usize,
}

/// Asymmetric retrieval metrics: measures 1.2/0.8 modifier effectiveness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AsymmetricRetrievalMetrics {
    /// MRR for cause→effect queries.
    pub cause_to_effect_mrr: f64,

    /// MRR for effect→cause queries.
    pub effect_to_cause_mrr: f64,

    /// Asymmetry ratio (should be ~1.5).
    /// Computed as cause_to_effect_mrr / effect_to_cause_mrr when normalized.
    pub asymmetry_ratio: f64,

    /// Average rank improvement vs symmetric baseline.
    pub avg_rank_improvement: f64,

    /// Direction modifier effectiveness score [0.0-1.0].
    pub direction_modifier_effectiveness: f64,

    /// Correlation between intervention overlap and retrieval quality.
    pub intervention_overlap_correlation: f64,

    /// MRR at various K values for cause→effect.
    pub cause_to_effect_mrr_at: HashMap<usize, f64>,

    /// MRR at various K values for effect→cause.
    pub effect_to_cause_mrr_at: HashMap<usize, f64>,
}

/// Causal reasoning metrics: COPA-style + chain traversal.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CausalReasoningMetrics {
    /// COPA accuracy (Choice of Plausible Alternatives).
    pub copa_accuracy: f64,

    /// Chain traversal accuracy (following multi-hop causal chains).
    pub chain_traversal_accuracy: f64,

    /// Kendall's tau for causal ordering (cause should rank before effect).
    pub causal_ordering_tau: f64,

    /// Counterfactual reasoning accuracy.
    pub counterfactual_accuracy: f64,

    /// Accuracy breakdown by question type.
    pub accuracy_by_type: HashMap<String, f64>,
}

/// Composite metrics combining all causal effectiveness measures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompositeCausalMetrics {
    /// Overall causal score (weighted combination).
    pub overall_causal_score: f64,

    /// Improvement over symmetric baseline (no direction modifiers).
    pub improvement_over_symmetric: f64,

    /// E5 contribution (improvement from adding E5 to retrieval).
    pub e5_contribution: f64,

    /// Feature contributions breakdown.
    pub feature_contributions: CausalFeatureContributions,
}

/// Breakdown of causal feature contributions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CausalFeatureContributions {
    /// Direction detection contribution [0.0-1.0].
    pub direction_detection: f64,

    /// Asymmetric similarity contribution [0.0-1.0].
    pub asymmetric_similarity: f64,

    /// Intervention overlap contribution [0.0-1.0].
    pub intervention_overlap: f64,

    /// Causal reasoning contribution [0.0-1.0].
    pub causal_reasoning: f64,
}

// =============================================================================
// E5 BENCHMARK-SPECIFIC METRICS (per E5 Causal Embedder Benchmark Enhancement Plan)
// =============================================================================

/// Comparison between symmetric and asymmetric E5 retrieval.
///
/// Per ARCH-18, AP-77: E5 uses asymmetric similarity with direction modifiers.
/// This structure captures how much better asymmetric retrieval performs vs symmetric.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SymmetricVsAsymmetricComparison {
    /// MRR using symmetric cosine similarity (no direction modifiers).
    pub symmetric_mrr: f64,
    /// MRR using asymmetric similarity (with 1.2x/0.8x modifiers).
    pub asymmetric_mrr: f64,
    /// Percentage improvement: (asymmetric - symmetric) / symmetric * 100.
    pub mrr_improvement_pct: f64,
    /// Average rank improvement over symmetric baseline.
    pub avg_rank_improvement: f64,
    /// Percentage of queries where asymmetric ranked better than symmetric.
    pub asymmetric_wins_pct: f64,
}

impl SymmetricVsAsymmetricComparison {
    /// Check if asymmetric retrieval shows meaningful improvement.
    /// Target: >10% MRR improvement per the plan.
    pub fn is_improvement_significant(&self) -> bool {
        self.mrr_improvement_pct > 10.0
    }
}

/// MRR breakdown by query direction.
///
/// Per ARCH-18: Cause→effect queries should get 1.2x boost, effect→cause 0.8x dampening.
/// The expected cause/effect MRR ratio is ~1.5 (1.2/0.8).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectionMRRBreakdown {
    /// MRR for cause-direction queries.
    pub cause_mrr: f64,
    /// MRR for effect-direction queries.
    pub effect_mrr: f64,
    /// MRR for unknown-direction queries.
    pub unknown_mrr: f64,
    /// Ratio of cause MRR to effect MRR (should be ~1.5).
    pub cause_to_effect_ratio: f64,
    /// Whether the ratio is in target range [1.3, 1.7].
    pub ratio_in_target_range: bool,
}

impl DirectionMRRBreakdown {
    /// Compute ratio and check if in target range.
    pub fn compute_ratio(&mut self) {
        if self.effect_mrr > 0.0 {
            self.cause_to_effect_ratio = self.cause_mrr / self.effect_mrr;
        } else {
            self.cause_to_effect_ratio = 0.0;
        }
        // Target range: 1.3-1.7 (around 1.5 = 1.2/0.8)
        self.ratio_in_target_range =
            self.cause_to_effect_ratio >= 1.3 && self.cause_to_effect_ratio <= 1.7;
    }
}

/// E5 vector verification metrics.
///
/// Per ARCH-18, AP-77: E5 cause/effect vectors must be distinct with minimum
/// 0.3 cosine distance to enable proper asymmetric similarity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E5VectorVerificationMetrics {
    /// Number of documents with distinct cause/effect vectors (distance >= 0.3).
    pub docs_with_distinct_vectors: usize,
    /// Total documents checked.
    pub total_docs_checked: usize,
    /// Percentage of docs with distinct vectors.
    pub distinct_vector_pct: f64,
    /// Average cosine distance between cause and effect vectors.
    pub avg_cause_effect_distance: f64,
    /// Minimum cosine distance observed.
    pub min_cause_effect_distance: f64,
    /// Number of threshold violations (distance < 0.3).
    pub threshold_violations: usize,
}

impl E5VectorVerificationMetrics {
    /// Check if all vectors meet the distance threshold.
    pub fn all_vectors_valid(&self) -> bool {
        self.threshold_violations == 0
    }

    /// The minimum required distance per ARCH-18.
    pub const MIN_DISTANCE_THRESHOLD: f64 = 0.3;
}

/// Direction distribution verification metrics.
///
/// Per the plan: Query causal directions should be distributed 40% Cause, 40% Effect, 20% Unknown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectionDistributionMetrics {
    /// Number of cause-direction queries.
    pub cause_query_count: usize,
    /// Number of effect-direction queries.
    pub effect_query_count: usize,
    /// Number of unknown-direction queries.
    pub unknown_query_count: usize,
    /// Actual percentage of cause queries.
    pub actual_cause_pct: f64,
    /// Actual percentage of effect queries.
    pub actual_effect_pct: f64,
    /// Actual percentage of unknown queries.
    pub actual_unknown_pct: f64,
    /// Whether the distribution is within tolerance (±5%) of target 40/40/20.
    pub distribution_valid: bool,
}

impl DirectionDistributionMetrics {
    /// Target percentages for the 40/40/20 distribution.
    pub const TARGET_CAUSE_PCT: f64 = 40.0;
    pub const TARGET_EFFECT_PCT: f64 = 40.0;
    pub const TARGET_UNKNOWN_PCT: f64 = 20.0;
    pub const TOLERANCE_PCT: f64 = 5.0;

    /// Compute percentages and check if distribution is valid.
    pub fn compute_distribution(&mut self) {
        let total = self.cause_query_count + self.effect_query_count + self.unknown_query_count;
        if total > 0 {
            let total_f = total as f64;
            self.actual_cause_pct = (self.cause_query_count as f64 / total_f) * 100.0;
            self.actual_effect_pct = (self.effect_query_count as f64 / total_f) * 100.0;
            self.actual_unknown_pct = (self.unknown_query_count as f64 / total_f) * 100.0;

            // Check if within tolerance of 40/40/20 target
            let cause_ok =
                (self.actual_cause_pct - Self::TARGET_CAUSE_PCT).abs() <= Self::TOLERANCE_PCT;
            let effect_ok =
                (self.actual_effect_pct - Self::TARGET_EFFECT_PCT).abs() <= Self::TOLERANCE_PCT;
            let unknown_ok =
                (self.actual_unknown_pct - Self::TARGET_UNKNOWN_PCT).abs() <= Self::TOLERANCE_PCT;

            self.distribution_valid = cause_ok && effect_ok && unknown_ok;
        }
    }
}

/// E5-specific impact analysis combining all E5 benchmark metrics.
///
/// This is the top-level structure for E5 causal embedder benchmark results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E5ImpactAnalysis {
    /// E5 vector verification (cause/effect distinctness).
    pub vector_verification: E5VectorVerificationMetrics,
    /// Query direction distribution (40/40/20 target).
    pub direction_distribution: DirectionDistributionMetrics,
    /// Symmetric vs asymmetric retrieval comparison.
    pub asymmetric_comparison: SymmetricVsAsymmetricComparison,
    /// Per-direction MRR breakdown.
    pub direction_mrr: DirectionMRRBreakdown,
    /// Observed asymmetry ratio (from direction_mrr.cause_to_effect_ratio).
    pub observed_asymmetry_ratio: f64,
}

impl E5ImpactAnalysis {
    /// Check overall E5 compliance per Constitution ARCH-18, AP-77.
    pub fn verify_compliance(&self) -> E5ConstitutionalCompliance {
        E5ConstitutionalCompliance {
            // ARCH-18: E5 uses asymmetric similarity (not just cosine)
            arch18_asymmetric: self.asymmetric_comparison.asymmetric_mrr
                != self.asymmetric_comparison.symmetric_mrr,
            // AP-77: E5 does NOT use symmetric cosine alone (asymmetric should be better)
            ap77_no_symmetric: self.asymmetric_comparison.mrr_improvement_pct > 0.0,
            // Direction modifiers correct: cause MRR > effect MRR (because 1.2x > 0.8x)
            direction_modifiers_correct: self.direction_mrr.cause_mrr
                > self.direction_mrr.effect_mrr,
            // Asymmetry ratio in valid range (1.3-1.7, target 1.5)
            asymmetry_ratio_valid: (self.observed_asymmetry_ratio - 1.5).abs() <= 0.2,
        }
    }
}

/// E5 constitutional compliance verification results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E5ConstitutionalCompliance {
    /// ARCH-18: E5 uses asymmetric similarity (verified by MRR difference).
    pub arch18_asymmetric: bool,
    /// AP-77: E5 does NOT use symmetric cosine alone (asymmetric performs better).
    pub ap77_no_symmetric: bool,
    /// Direction modifiers applied correctly (1.2x cause→effect, 0.8x effect→cause).
    pub direction_modifiers_correct: bool,
    /// Asymmetry ratio is in valid range (1.3-1.7).
    pub asymmetry_ratio_valid: bool,
}

impl E5ConstitutionalCompliance {
    /// Check if all E5 compliance rules pass.
    pub fn all_pass(&self) -> bool {
        self.arch18_asymmetric
            && self.ap77_no_symmetric
            && self.direction_modifiers_correct
            && self.asymmetry_ratio_valid
    }
}

impl CausalMetrics {
    /// Overall causal quality score (weighted combination).
    ///
    /// Weights based on importance to causal retrieval:
    /// - Direction detection: 25% (essential for asymmetric matching)
    /// - Asymmetric retrieval: 35% (core E5 functionality)
    /// - Causal reasoning: 40% (end-to-end effectiveness)
    pub fn quality_score(&self) -> f64 {
        0.25 * self.direction.overall_score()
            + 0.35 * self.asymmetric.overall_score()
            + 0.40 * self.reasoning.overall_score()
    }

    /// Check if metrics meet minimum thresholds.
    pub fn meets_thresholds(
        &self,
        min_direction_accuracy: f64,
        min_asymmetry_ratio: f64,
        min_copa_accuracy: f64,
    ) -> bool {
        self.direction.accuracy >= min_direction_accuracy
            && self.asymmetric.asymmetry_ratio >= min_asymmetry_ratio
            && self.reasoning.copa_accuracy >= min_copa_accuracy
    }
}

impl DirectionDetectionMetrics {
    /// Overall direction detection score.
    pub fn overall_score(&self) -> f64 {
        // Weight: 50% accuracy, 50% F1 (to balance precision/recall)
        0.5 * self.accuracy + 0.5 * self.direction_f1
    }
}

impl AsymmetricRetrievalMetrics {
    /// Overall asymmetric retrieval score.
    pub fn overall_score(&self) -> f64 {
        // Normalize asymmetry ratio: 1.5 is ideal
        let asymmetry_score = 1.0 - ((self.asymmetry_ratio - 1.5).abs() / 1.5).min(1.0);

        0.3 * self.cause_to_effect_mrr
            + 0.2 * self.effect_to_cause_mrr
            + 0.25 * asymmetry_score
            + 0.25 * self.direction_modifier_effectiveness
    }
}

impl CausalReasoningMetrics {
    /// Overall reasoning score.
    pub fn overall_score(&self) -> f64 {
        // Normalize Kendall's tau from [-1, 1] to [0, 1]
        let tau_normalized = (self.causal_ordering_tau + 1.0) / 2.0;

        0.35 * self.copa_accuracy
            + 0.25 * self.chain_traversal_accuracy
            + 0.25 * tau_normalized
            + 0.15 * self.counterfactual_accuracy
    }
}

// =============================================================================
// METRIC COMPUTATION FUNCTIONS
// =============================================================================

/// Compute direction detection metrics from predictions vs ground truth.
///
/// # Arguments
/// * `predictions` - Predicted directions from `detect_causal_query_intent()`
/// * `ground_truth` - Actual directions
///
/// # Returns
/// DirectionDetectionMetrics with accuracy, precision, recall, F1
pub fn compute_direction_detection_metrics(
    predictions: &[CausalDirection],
    ground_truth: &[CausalDirection],
) -> DirectionDetectionMetrics {
    if predictions.len() != ground_truth.len() {
        panic!(
            "Predictions and ground truth must have same length: {} vs {}",
            predictions.len(),
            ground_truth.len()
        );
    }

    if predictions.is_empty() {
        return DirectionDetectionMetrics::default();
    }

    let mut confusion = DirectionConfusionMatrix::default();

    for (pred, actual) in predictions.iter().zip(ground_truth.iter()) {
        match (actual, pred) {
            (CausalDirection::Cause, CausalDirection::Cause) => {
                confusion.true_cause_pred_cause += 1
            }
            (CausalDirection::Cause, CausalDirection::Effect) => {
                confusion.true_cause_pred_effect += 1
            }
            (CausalDirection::Cause, CausalDirection::Unknown) => {
                confusion.true_cause_pred_unknown += 1
            }
            (CausalDirection::Effect, CausalDirection::Cause) => {
                confusion.true_effect_pred_cause += 1
            }
            (CausalDirection::Effect, CausalDirection::Effect) => {
                confusion.true_effect_pred_effect += 1
            }
            (CausalDirection::Effect, CausalDirection::Unknown) => {
                confusion.true_effect_pred_unknown += 1
            }
            (CausalDirection::Unknown, CausalDirection::Cause) => {
                confusion.true_unknown_pred_cause += 1
            }
            (CausalDirection::Unknown, CausalDirection::Effect) => {
                confusion.true_unknown_pred_effect += 1
            }
            (CausalDirection::Unknown, CausalDirection::Unknown) => {
                confusion.true_unknown_pred_unknown += 1
            }
        }
    }

    // Calculate accuracy (only for Cause/Effect, Unknown doesn't count as correct/incorrect)
    let total_cause_effect = confusion.true_cause_pred_cause
        + confusion.true_cause_pred_effect
        + confusion.true_cause_pred_unknown
        + confusion.true_effect_pred_cause
        + confusion.true_effect_pred_effect
        + confusion.true_effect_pred_unknown;

    let correct_cause_effect = confusion.true_cause_pred_cause + confusion.true_effect_pred_effect;

    let accuracy = if total_cause_effect > 0 {
        correct_cause_effect as f64 / total_cause_effect as f64
    } else {
        0.0
    };

    // Cause precision: true_cause_pred_cause / total_pred_cause
    let total_pred_cause = confusion.true_cause_pred_cause
        + confusion.true_effect_pred_cause
        + confusion.true_unknown_pred_cause;
    let cause_precision = if total_pred_cause > 0 {
        confusion.true_cause_pred_cause as f64 / total_pred_cause as f64
    } else {
        0.0
    };

    // Cause recall: true_cause_pred_cause / total_actual_cause
    let total_actual_cause = confusion.true_cause_pred_cause
        + confusion.true_cause_pred_effect
        + confusion.true_cause_pred_unknown;
    let cause_recall = if total_actual_cause > 0 {
        confusion.true_cause_pred_cause as f64 / total_actual_cause as f64
    } else {
        0.0
    };

    // Effect precision: true_effect_pred_effect / total_pred_effect
    let total_pred_effect = confusion.true_cause_pred_effect
        + confusion.true_effect_pred_effect
        + confusion.true_unknown_pred_effect;
    let effect_precision = if total_pred_effect > 0 {
        confusion.true_effect_pred_effect as f64 / total_pred_effect as f64
    } else {
        0.0
    };

    // Effect recall: true_effect_pred_effect / total_actual_effect
    let total_actual_effect = confusion.true_effect_pred_cause
        + confusion.true_effect_pred_effect
        + confusion.true_effect_pred_unknown;
    let effect_recall = if total_actual_effect > 0 {
        confusion.true_effect_pred_effect as f64 / total_actual_effect as f64
    } else {
        0.0
    };

    // Macro F1: average of cause F1 and effect F1
    let cause_f1 = if cause_precision + cause_recall > 0.0 {
        2.0 * cause_precision * cause_recall / (cause_precision + cause_recall)
    } else {
        0.0
    };

    let effect_f1 = if effect_precision + effect_recall > 0.0 {
        2.0 * effect_precision * effect_recall / (effect_precision + effect_recall)
    } else {
        0.0
    };

    let direction_f1 = (cause_f1 + effect_f1) / 2.0;

    DirectionDetectionMetrics {
        accuracy,
        cause_precision,
        cause_recall,
        effect_precision,
        effect_recall,
        direction_f1,
        confusion_matrix: confusion,
    }
}

/// Result from an asymmetric retrieval query.
#[derive(Debug, Clone)]
pub struct AsymmetricRetrievalResult {
    /// Query direction.
    pub query_direction: CausalDirection,

    /// Ranks with asymmetric scoring.
    pub asymmetric_ranks: Vec<usize>,

    /// Ranks with symmetric (baseline) scoring.
    pub symmetric_ranks: Vec<usize>,

    /// Relevant document indices.
    pub relevant_indices: Vec<usize>,

    /// Intervention overlap scores.
    pub intervention_overlaps: Vec<f32>,

    /// Asymmetric score for the target (relevant) document.
    /// Used to compute score-based asymmetry ratio.
    pub target_asymmetric_score: f32,

    /// Symmetric score for the target (relevant) document.
    /// Used to compute score-based asymmetry ratio.
    pub target_symmetric_score: f32,
}

/// Compute asymmetric retrieval metrics.
///
/// # Arguments
/// * `results` - Results from asymmetric retrieval benchmark
/// * `k_values` - K values for MRR@K computation
///
/// # Returns
/// AsymmetricRetrievalMetrics with MRR, asymmetry ratio, rank improvement
pub fn compute_asymmetric_retrieval_metrics(
    results: &[AsymmetricRetrievalResult],
    k_values: &[usize],
) -> AsymmetricRetrievalMetrics {
    if results.is_empty() {
        return AsymmetricRetrievalMetrics::default();
    }

    let cause_to_effect: Vec<_> = results
        .iter()
        .filter(|r| r.query_direction == CausalDirection::Cause)
        .collect();

    let effect_to_cause: Vec<_> = results
        .iter()
        .filter(|r| r.query_direction == CausalDirection::Effect)
        .collect();

    // Compute MRR for cause→effect
    let cause_to_effect_mrr = compute_mrr(&cause_to_effect);

    // Compute MRR for effect→cause
    let effect_to_cause_mrr = compute_mrr(&effect_to_cause);

    // Compute MRR@K for both directions
    let mut cause_to_effect_mrr_at = HashMap::new();
    let mut effect_to_cause_mrr_at = HashMap::new();

    for &k in k_values {
        cause_to_effect_mrr_at.insert(k, compute_mrr_at_k(&cause_to_effect, k));
        effect_to_cause_mrr_at.insert(k, compute_mrr_at_k(&effect_to_cause, k));
    }

    // Asymmetry ratio: computed from actual scores, not MRR
    // Should be ~1.5 (1.2/0.8) when direction modifiers work correctly
    // This measures how much higher cause→effect scores are vs effect→cause scores
    let asymmetry_ratio = compute_score_asymmetry_ratio(&cause_to_effect, &effect_to_cause);

    // Compute rank improvement vs symmetric baseline
    let avg_rank_improvement = compute_avg_rank_improvement(results);

    // Direction modifier effectiveness: how close to 1.5 asymmetry
    let direction_modifier_effectiveness = 1.0 - ((asymmetry_ratio - 1.5).abs() / 1.5).min(1.0);

    // Intervention overlap correlation
    let intervention_overlap_correlation = compute_overlap_correlation(results);

    AsymmetricRetrievalMetrics {
        cause_to_effect_mrr,
        effect_to_cause_mrr,
        asymmetry_ratio,
        avg_rank_improvement,
        direction_modifier_effectiveness,
        intervention_overlap_correlation,
        cause_to_effect_mrr_at,
        effect_to_cause_mrr_at,
    }
}

/// Compute MRR from retrieval results.
fn compute_mrr(results: &[&AsymmetricRetrievalResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    let sum: f64 = results
        .iter()
        .map(|r| {
            // Find first relevant document in asymmetric ranks
            for (pos, &rank_idx) in r.asymmetric_ranks.iter().enumerate() {
                if r.relevant_indices.contains(&rank_idx) {
                    return 1.0 / (pos + 1) as f64;
                }
            }
            0.0
        })
        .sum();

    sum / results.len() as f64
}

/// Compute MRR@K from retrieval results.
fn compute_mrr_at_k(results: &[&AsymmetricRetrievalResult], k: usize) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    let sum: f64 = results
        .iter()
        .map(|r| {
            for (pos, &rank_idx) in r.asymmetric_ranks.iter().take(k).enumerate() {
                if r.relevant_indices.contains(&rank_idx) {
                    return 1.0 / (pos + 1) as f64;
                }
            }
            0.0
        })
        .sum();

    sum / results.len() as f64
}

/// Compute asymmetry ratio based on actual scores.
///
/// The asymmetry ratio should be approximately 1.5 (1.2/0.8) when direction
/// modifiers are working correctly. This is computed from the average score
/// ratio between cause→effect and effect→cause directions.
fn compute_score_asymmetry_ratio(
    cause_to_effect: &[&AsymmetricRetrievalResult],
    effect_to_cause: &[&AsymmetricRetrievalResult],
) -> f64 {
    if cause_to_effect.is_empty() || effect_to_cause.is_empty() {
        return 0.0;
    }

    // Compute average asymmetric score for cause→effect queries
    let cause_avg: f64 = cause_to_effect
        .iter()
        .map(|r| r.target_asymmetric_score as f64)
        .sum::<f64>()
        / cause_to_effect.len() as f64;

    // Compute average asymmetric score for effect→cause queries
    let effect_avg: f64 = effect_to_cause
        .iter()
        .map(|r| r.target_asymmetric_score as f64)
        .sum::<f64>()
        / effect_to_cause.len() as f64;

    // Asymmetry ratio is cause→effect average / effect→cause average
    // Should be ~1.5 (1.2/0.8) when direction modifiers work correctly
    if effect_avg > 0.0 {
        cause_avg / effect_avg
    } else {
        0.0
    }
}

/// Compute average rank improvement over symmetric baseline.
fn compute_avg_rank_improvement(results: &[AsymmetricRetrievalResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    let improvements: Vec<f64> = results
        .iter()
        .filter_map(|r| {
            let asym_rank = r
                .asymmetric_ranks
                .iter()
                .enumerate()
                .find_map(|(pos, &idx)| {
                    if r.relevant_indices.contains(&idx) {
                        Some(pos + 1)
                    } else {
                        None
                    }
                });

            let sym_rank = r
                .symmetric_ranks
                .iter()
                .enumerate()
                .find_map(|(pos, &idx)| {
                    if r.relevant_indices.contains(&idx) {
                        Some(pos + 1)
                    } else {
                        None
                    }
                });

            match (asym_rank, sym_rank) {
                (Some(a), Some(s)) if s > 0 => {
                    // Improvement = (symmetric_rank - asymmetric_rank) / symmetric_rank
                    Some((s as f64 - a as f64) / s as f64)
                }
                _ => None,
            }
        })
        .collect();

    if improvements.is_empty() {
        0.0
    } else {
        improvements.iter().sum::<f64>() / improvements.len() as f64
    }
}

/// Compute Pearson correlation between intervention overlap and retrieval quality.
fn compute_overlap_correlation(results: &[AsymmetricRetrievalResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    // For each result, get (avg_overlap, reciprocal_rank)
    let pairs: Vec<(f64, f64)> = results
        .iter()
        .filter_map(|r| {
            if r.intervention_overlaps.is_empty() {
                return None;
            }

            let avg_overlap = r
                .intervention_overlaps
                .iter()
                .map(|&x| x as f64)
                .sum::<f64>()
                / r.intervention_overlaps.len() as f64;

            let rr = r
                .asymmetric_ranks
                .iter()
                .enumerate()
                .find_map(|(pos, &idx)| {
                    if r.relevant_indices.contains(&idx) {
                        Some(1.0 / (pos + 1) as f64)
                    } else {
                        None
                    }
                })?;

            Some((avg_overlap, rr))
        })
        .collect();

    if pairs.len() < 2 {
        return 0.0;
    }

    pearson_correlation(&pairs)
}

/// Compute Pearson correlation coefficient.
fn pearson_correlation(pairs: &[(f64, f64)]) -> f64 {
    let n = pairs.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.iter().cloned().unzip();

    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;

    let mut num = 0.0;
    let mut denom_x = 0.0;
    let mut denom_y = 0.0;

    for i in 0..pairs.len() {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        num += dx * dy;
        denom_x += dx * dx;
        denom_y += dy * dy;
    }

    let denom = (denom_x * denom_y).sqrt();
    if denom < f64::EPSILON {
        0.0
    } else {
        (num / denom).clamp(-1.0, 1.0)
    }
}

/// COPA question result.
#[derive(Debug, Clone)]
pub struct CopaResult {
    /// Question type: "cause" or "effect".
    pub question_type: String,

    /// Whether the answer was correct.
    pub correct: bool,

    /// Confidence score (optional).
    pub confidence: Option<f64>,
}

/// Chain traversal result.
#[derive(Debug, Clone)]
pub struct ChainTraversalResult {
    /// Chain length (number of hops).
    pub chain_length: usize,

    /// Number of correctly traversed hops.
    pub correct_hops: usize,

    /// Final target reached?
    pub target_reached: bool,
}

/// Causal ordering result (for Kendall's tau).
#[derive(Debug, Clone)]
pub struct CausalOrderingResult {
    /// Predicted order of causal chain.
    pub predicted_order: Vec<usize>,

    /// Actual order (ground truth).
    pub actual_order: Vec<usize>,
}

/// Compute causal reasoning metrics.
///
/// # Arguments
/// * `copa_results` - Results from COPA-style questions
/// * `chain_results` - Results from chain traversal
/// * `ordering_results` - Results from causal ordering
/// * `counterfactual_results` - Results from counterfactual questions
pub fn compute_causal_reasoning_metrics(
    copa_results: &[CopaResult],
    chain_results: &[ChainTraversalResult],
    ordering_results: &[CausalOrderingResult],
    counterfactual_correct: usize,
    counterfactual_total: usize,
) -> CausalReasoningMetrics {
    // COPA accuracy
    let copa_correct = copa_results.iter().filter(|r| r.correct).count();
    let copa_accuracy = if !copa_results.is_empty() {
        copa_correct as f64 / copa_results.len() as f64
    } else {
        0.0
    };

    // Accuracy by question type
    let mut accuracy_by_type = HashMap::new();
    let cause_correct = copa_results
        .iter()
        .filter(|r| r.question_type == "cause" && r.correct)
        .count();
    let cause_total = copa_results
        .iter()
        .filter(|r| r.question_type == "cause")
        .count();
    if cause_total > 0 {
        accuracy_by_type.insert(
            "cause".to_string(),
            cause_correct as f64 / cause_total as f64,
        );
    }

    let effect_correct = copa_results
        .iter()
        .filter(|r| r.question_type == "effect" && r.correct)
        .count();
    let effect_total = copa_results
        .iter()
        .filter(|r| r.question_type == "effect")
        .count();
    if effect_total > 0 {
        accuracy_by_type.insert(
            "effect".to_string(),
            effect_correct as f64 / effect_total as f64,
        );
    }

    // Chain traversal accuracy
    let chain_traversal_accuracy = if !chain_results.is_empty() {
        let total_hops: usize = chain_results.iter().map(|r| r.chain_length).sum();
        let correct_hops: usize = chain_results.iter().map(|r| r.correct_hops).sum();
        if total_hops > 0 {
            correct_hops as f64 / total_hops as f64
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Causal ordering Kendall's tau
    let causal_ordering_tau = compute_avg_kendalls_tau(ordering_results);

    // Counterfactual accuracy
    let counterfactual_accuracy = if counterfactual_total > 0 {
        counterfactual_correct as f64 / counterfactual_total as f64
    } else {
        0.0
    };

    CausalReasoningMetrics {
        copa_accuracy,
        chain_traversal_accuracy,
        causal_ordering_tau,
        counterfactual_accuracy,
        accuracy_by_type,
    }
}

/// Compute Kendall's tau between two orderings.
pub fn kendalls_tau(predicted: &[usize], actual: &[usize]) -> f64 {
    if predicted.len() != actual.len() || predicted.len() < 2 {
        return 0.0;
    }

    let n = predicted.len();
    let mut concordant = 0i64;
    let mut discordant = 0i64;

    for i in 0..n {
        for j in (i + 1)..n {
            let pred_diff = predicted[i] as i64 - predicted[j] as i64;
            let actual_diff = actual[i] as i64 - actual[j] as i64;

            if pred_diff * actual_diff > 0 {
                concordant += 1;
            } else if pred_diff * actual_diff < 0 {
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

/// Compute average Kendall's tau across multiple ordering results.
fn compute_avg_kendalls_tau(results: &[CausalOrderingResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    let sum: f64 = results
        .iter()
        .map(|r| kendalls_tau(&r.predicted_order, &r.actual_order))
        .sum();

    sum / results.len() as f64
}

// =============================================================================
// ALL METRICS COMPUTATION
// =============================================================================

/// Data for direction detection benchmark.
#[derive(Debug, Default)]
pub struct DirectionBenchmarkData {
    /// Predictions from detect_causal_query_intent().
    pub predictions: Vec<CausalDirection>,

    /// Ground truth directions.
    pub ground_truth: Vec<CausalDirection>,
}

/// Data for asymmetric retrieval benchmark.
#[derive(Debug, Default)]
pub struct AsymmetricBenchmarkData {
    /// Retrieval results.
    pub results: Vec<AsymmetricRetrievalResult>,

    /// K values for MRR@K.
    pub k_values: Vec<usize>,
}

/// Data for causal reasoning benchmark.
#[derive(Debug, Default)]
pub struct ReasoningBenchmarkData {
    /// COPA results.
    pub copa_results: Vec<CopaResult>,

    /// Chain traversal results.
    pub chain_results: Vec<ChainTraversalResult>,

    /// Ordering results.
    pub ordering_results: Vec<CausalOrderingResult>,

    /// Counterfactual correct count.
    pub counterfactual_correct: usize,

    /// Counterfactual total count.
    pub counterfactual_total: usize,
}

/// Compute all causal metrics from benchmark data.
pub fn compute_all_causal_metrics(
    direction_data: &DirectionBenchmarkData,
    asymmetric_data: &AsymmetricBenchmarkData,
    reasoning_data: &ReasoningBenchmarkData,
    symmetric_baseline_score: f64,
    without_e5_score: f64,
) -> CausalMetrics {
    let direction = compute_direction_detection_metrics(
        &direction_data.predictions,
        &direction_data.ground_truth,
    );

    let asymmetric =
        compute_asymmetric_retrieval_metrics(&asymmetric_data.results, &asymmetric_data.k_values);

    let reasoning = compute_causal_reasoning_metrics(
        &reasoning_data.copa_results,
        &reasoning_data.chain_results,
        &reasoning_data.ordering_results,
        reasoning_data.counterfactual_correct,
        reasoning_data.counterfactual_total,
    );

    let overall_score = 0.25 * direction.overall_score()
        + 0.35 * asymmetric.overall_score()
        + 0.40 * reasoning.overall_score();

    let improvement_over_symmetric = if symmetric_baseline_score > 0.0 {
        (overall_score - symmetric_baseline_score) / symmetric_baseline_score
    } else {
        0.0
    };

    let e5_contribution = if without_e5_score > 0.0 {
        (overall_score - without_e5_score) / without_e5_score
    } else {
        0.0
    };

    let composite = CompositeCausalMetrics {
        overall_causal_score: overall_score,
        improvement_over_symmetric,
        e5_contribution,
        feature_contributions: CausalFeatureContributions {
            direction_detection: direction.overall_score(),
            asymmetric_similarity: asymmetric.overall_score(),
            intervention_overlap: asymmetric.intervention_overlap_correlation.max(0.0),
            causal_reasoning: reasoning.overall_score(),
        },
    };

    let query_count = direction_data.predictions.len()
        + asymmetric_data.results.len()
        + reasoning_data.copa_results.len();

    CausalMetrics {
        direction,
        asymmetric,
        reasoning,
        composite,
        query_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_detection_perfect_accuracy() {
        let predictions = vec![
            CausalDirection::Cause,
            CausalDirection::Cause,
            CausalDirection::Effect,
            CausalDirection::Effect,
        ];
        let ground_truth = vec![
            CausalDirection::Cause,
            CausalDirection::Cause,
            CausalDirection::Effect,
            CausalDirection::Effect,
        ];

        let metrics = compute_direction_detection_metrics(&predictions, &ground_truth);

        assert!((metrics.accuracy - 1.0).abs() < 0.01);
        assert!((metrics.cause_precision - 1.0).abs() < 0.01);
        assert!((metrics.cause_recall - 1.0).abs() < 0.01);
        assert!((metrics.effect_precision - 1.0).abs() < 0.01);
        assert!((metrics.effect_recall - 1.0).abs() < 0.01);
        assert!((metrics.direction_f1 - 1.0).abs() < 0.01);

        println!("[VERIFIED] Perfect accuracy produces metrics = 1.0");
    }

    #[test]
    fn test_direction_detection_mixed() {
        let predictions = vec![
            CausalDirection::Cause,  // Correct
            CausalDirection::Effect, // Wrong
            CausalDirection::Effect, // Correct
            CausalDirection::Cause,  // Wrong
        ];
        let ground_truth = vec![
            CausalDirection::Cause,
            CausalDirection::Cause,
            CausalDirection::Effect,
            CausalDirection::Effect,
        ];

        let metrics = compute_direction_detection_metrics(&predictions, &ground_truth);

        assert!((metrics.accuracy - 0.5).abs() < 0.01);
        println!(
            "[VERIFIED] 50% accuracy produces metrics.accuracy = {}",
            metrics.accuracy
        );
    }

    #[test]
    fn test_kendalls_tau_perfect() {
        let order1 = vec![0, 1, 2, 3, 4];
        let order2 = vec![0, 1, 2, 3, 4];
        let tau = kendalls_tau(&order1, &order2);
        assert!((tau - 1.0).abs() < 0.01);
        println!("[VERIFIED] Perfect ordering → tau = 1.0");
    }

    #[test]
    fn test_kendalls_tau_reversed() {
        let order1 = vec![0, 1, 2, 3, 4];
        let order2 = vec![4, 3, 2, 1, 0];
        let tau = kendalls_tau(&order1, &order2);
        assert!((tau - (-1.0)).abs() < 0.01);
        println!("[VERIFIED] Reversed ordering → tau = -1.0");
    }

    #[test]
    fn test_pearson_correlation_perfect() {
        let pairs = vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0), (4.0, 4.0)];
        let r = pearson_correlation(&pairs);
        assert!((r - 1.0).abs() < 0.01);
        println!("[VERIFIED] Perfect correlation → r = 1.0");
    }

    #[test]
    fn test_pearson_correlation_negative() {
        let pairs = vec![(1.0, 4.0), (2.0, 3.0), (3.0, 2.0), (4.0, 1.0)];
        let r = pearson_correlation(&pairs);
        assert!((r - (-1.0)).abs() < 0.01);
        println!("[VERIFIED] Perfect negative correlation → r = -1.0");
    }

    #[test]
    fn test_copa_accuracy() {
        let copa_results = vec![
            CopaResult {
                question_type: "cause".to_string(),
                correct: true,
                confidence: Some(0.9),
            },
            CopaResult {
                question_type: "cause".to_string(),
                correct: false,
                confidence: Some(0.6),
            },
            CopaResult {
                question_type: "effect".to_string(),
                correct: true,
                confidence: Some(0.8),
            },
            CopaResult {
                question_type: "effect".to_string(),
                correct: true,
                confidence: Some(0.7),
            },
        ];

        let metrics = compute_causal_reasoning_metrics(&copa_results, &[], &[], 0, 0);

        assert!((metrics.copa_accuracy - 0.75).abs() < 0.01); // 3/4
        assert!((metrics.accuracy_by_type["cause"] - 0.5).abs() < 0.01); // 1/2
        assert!((metrics.accuracy_by_type["effect"] - 1.0).abs() < 0.01); // 2/2

        println!("[VERIFIED] COPA accuracy = 0.75 (3/4 correct)");
    }

    #[test]
    fn test_chain_traversal() {
        let chain_results = vec![
            ChainTraversalResult {
                chain_length: 3,
                correct_hops: 3,
                target_reached: true,
            },
            ChainTraversalResult {
                chain_length: 4,
                correct_hops: 2,
                target_reached: false,
            },
        ];

        let metrics = compute_causal_reasoning_metrics(&[], &chain_results, &[], 0, 0);

        // Total hops: 7, correct: 5 → 5/7 ≈ 0.714
        assert!(
            (metrics.chain_traversal_accuracy - 5.0 / 7.0).abs() < 0.01,
            "Got: {}",
            metrics.chain_traversal_accuracy
        );

        println!(
            "[VERIFIED] Chain traversal accuracy = {} (5/7)",
            metrics.chain_traversal_accuracy
        );
    }

    #[test]
    fn test_asymmetric_mrr() {
        let results = vec![
            AsymmetricRetrievalResult {
                query_direction: CausalDirection::Cause,
                asymmetric_ranks: vec![0, 1, 2, 3], // Relevant at position 0
                symmetric_ranks: vec![2, 0, 1, 3],  // Relevant at position 1
                relevant_indices: vec![0],
                intervention_overlaps: vec![0.8],
                target_asymmetric_score: 0.95 * 1.2 * 0.94, // base=0.95, mod=1.2, overlap=0.8
                target_symmetric_score: 0.95 * 0.85,        // base * 0.85 (neutral)
            },
            AsymmetricRetrievalResult {
                query_direction: CausalDirection::Cause,
                asymmetric_ranks: vec![1, 0, 2, 3], // Relevant at position 1
                symmetric_ranks: vec![2, 1, 0, 3],  // Relevant at position 2
                relevant_indices: vec![0],
                intervention_overlaps: vec![0.5],
                target_asymmetric_score: 0.90 * 1.2 * 0.85, // base=0.90, mod=1.2, overlap=0.5
                target_symmetric_score: 0.90 * 0.85,        // base * 0.85 (neutral)
            },
        ];

        let metrics = compute_asymmetric_retrieval_metrics(&results, &[1, 5, 10]);

        // MRR = (1/1 + 1/2) / 2 = 0.75
        assert!(
            (metrics.cause_to_effect_mrr - 0.75).abs() < 0.01,
            "Got: {}",
            metrics.cause_to_effect_mrr
        );

        println!(
            "[VERIFIED] Cause→Effect MRR = {} (expected 0.75)",
            metrics.cause_to_effect_mrr
        );
    }

    #[test]
    fn test_rank_improvement() {
        let results = vec![AsymmetricRetrievalResult {
            query_direction: CausalDirection::Cause,
            asymmetric_ranks: vec![0, 1, 2, 3], // Relevant at rank 1
            symmetric_ranks: vec![2, 1, 0, 3],  // Relevant at rank 3
            relevant_indices: vec![0],
            intervention_overlaps: vec![0.5],
            target_asymmetric_score: 0.90 * 1.2 * 0.85, // base=0.90, mod=1.2, overlap=0.5
            target_symmetric_score: 0.90 * 0.85,        // base * 0.85 (neutral)
        }];

        let improvement = compute_avg_rank_improvement(&results);

        // Improvement = (3 - 1) / 3 = 0.666...
        assert!(
            (improvement - 2.0 / 3.0).abs() < 0.01,
            "Got: {}",
            improvement
        );

        println!("[VERIFIED] Rank improvement = {} (rank 3 → 1)", improvement);
    }
}
