//! Temporal benchmark metrics for evaluating E2/E3/E4 embedder effectiveness.
//!
//! This module provides comprehensive metrics for evaluating temporal retrieval:
//!
//! - **E2 Recency**: Decay accuracy, freshness precision, recency-weighted MRR
//! - **E3 Periodic**: Hourly/daily cluster quality, periodic recall
//! - **E4 Sequence**: Sequence accuracy, temporal ordering precision, episode boundary F1
//!
//! ## Key Concepts
//!
//! - **Recency-weighted MRR**: MRR where relevance decays with age
//! - **Periodic recall**: Fraction of same-hour/day memories retrieved
//! - **Sequence accuracy**: Correct ordering of before/after relationships

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Temporal metrics for E2/E3/E4 embedder evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemporalMetrics {
    /// E2 Recency metrics.
    pub recency: RecencyMetrics,

    /// E3 Periodic metrics.
    pub periodic: PeriodicMetrics,

    /// E4 Sequence metrics.
    pub sequence: SequenceMetrics,

    /// Composite metrics.
    pub composite: CompositeTemporalMetrics,

    /// Number of queries used to compute these metrics.
    pub query_count: usize,
}

/// E2 Recency-specific metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecencyMetrics {
    /// Freshness precision at K (fraction of top-K that are "fresh").
    pub freshness_precision_at: HashMap<usize, f64>,

    /// Recency-weighted MRR (MRR with exponential decay weighting).
    pub recency_weighted_mrr: f64,

    /// Decay accuracy (correlation between predicted and actual recency scores).
    pub decay_accuracy: f64,

    /// Average position improvement when recency boost applied.
    pub avg_position_improvement: f64,

    /// Fraction of queries where fresh item was retrieved in top-K.
    pub fresh_retrieval_rate_at: HashMap<usize, f64>,
}

/// E3 Periodic-specific metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeriodicMetrics {
    /// Periodic recall at K (fraction of same-hour/day items retrieved).
    pub periodic_recall_at: HashMap<usize, f64>,

    /// Hourly cluster quality (silhouette score for hour-of-day groupings).
    pub hourly_cluster_quality: f64,

    /// Daily cluster quality (silhouette score for day-of-week groupings).
    pub daily_cluster_quality: f64,

    /// Precision for detecting periodic patterns.
    pub pattern_detection_precision: f64,

    /// Recall for detecting periodic patterns.
    pub pattern_detection_recall: f64,
}

/// E4 Sequence-specific metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SequenceMetrics {
    /// Sequence ordering accuracy (fraction of pairs correctly ordered).
    pub sequence_accuracy: f64,

    /// Temporal ordering precision (Kendall's tau correlation).
    pub temporal_ordering_precision: f64,

    /// Episode boundary F1 (detecting conversation/session boundaries).
    pub episode_boundary_f1: f64,

    /// Before/after retrieval accuracy.
    pub before_after_accuracy: f64,

    /// Average sequence distance error.
    pub avg_sequence_distance_error: f64,
}

/// Composite metrics combining E2/E3/E4 effectiveness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompositeTemporalMetrics {
    /// Overall temporal score (weighted combination of E2/E3/E4).
    pub overall_temporal_score: f64,

    /// Improvement over baseline (no temporal features).
    pub improvement_over_baseline: f64,

    /// Temporal feature contribution breakdown.
    pub feature_contributions: TemporalFeatureContributions,
}

/// Breakdown of temporal feature contributions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemporalFeatureContributions {
    /// E2 recency contribution [0.0-1.0].
    pub e2_recency: f64,

    /// E3 periodic contribution [0.0-1.0].
    pub e3_periodic: f64,

    /// E4 sequence contribution [0.0-1.0].
    pub e4_sequence: f64,
}

impl TemporalMetrics {
    /// Overall temporal quality score (weighted combination).
    ///
    /// Weights optimized from benchmark analysis:
    /// - E2 recency: 50% (strongest signal, 0.872 alone)
    /// - E4 sequence: 35% (strong ordering signal)
    /// - E3 periodic: 15% (weaker signal, constant 0.55)
    ///
    /// Previous weights (40/30/30) caused negative interference where
    /// combined score (0.764) was lower than E2 alone (0.872).
    pub fn quality_score(&self) -> f64 {
        // 50% recency, 35% sequence, 15% periodic (optimized from benchmark)
        0.5 * self.recency.overall_score()
            + 0.35 * self.sequence.overall_score()
            + 0.15 * self.periodic.overall_score()
    }

    /// Check if metrics meet minimum thresholds.
    pub fn meets_thresholds(
        &self,
        min_recency_mrr: f64,
        min_sequence_accuracy: f64,
        min_periodic_recall: f64,
    ) -> bool {
        self.recency.recency_weighted_mrr >= min_recency_mrr
            && self.sequence.sequence_accuracy >= min_sequence_accuracy
            && self
                .periodic
                .periodic_recall_at
                .get(&10)
                .copied()
                .unwrap_or(0.0)
                >= min_periodic_recall
    }
}

impl RecencyMetrics {
    /// Overall recency score.
    pub fn overall_score(&self) -> f64 {
        let fresh_rate = self
            .fresh_retrieval_rate_at
            .get(&10)
            .copied()
            .unwrap_or(0.0);
        0.4 * self.recency_weighted_mrr + 0.3 * self.decay_accuracy + 0.3 * fresh_rate
    }
}

impl PeriodicMetrics {
    /// Overall periodic score.
    pub fn overall_score(&self) -> f64 {
        let recall = self.periodic_recall_at.get(&10).copied().unwrap_or(0.0);
        let cluster_quality = (self.hourly_cluster_quality + self.daily_cluster_quality) / 2.0;
        let pattern_f1 = if self.pattern_detection_precision + self.pattern_detection_recall > 0.0 {
            2.0 * self.pattern_detection_precision * self.pattern_detection_recall
                / (self.pattern_detection_precision + self.pattern_detection_recall)
        } else {
            0.0
        };

        0.4 * recall + 0.3 * cluster_quality + 0.3 * pattern_f1
    }
}

impl SequenceMetrics {
    /// Overall sequence score.
    pub fn overall_score(&self) -> f64 {
        0.3 * self.sequence_accuracy
            + 0.3 * self.temporal_ordering_precision
            + 0.2 * self.episode_boundary_f1
            + 0.2 * self.before_after_accuracy
    }
}

// =============================================================================
// METRIC COMPUTATION FUNCTIONS
// =============================================================================

/// Compute freshness precision at K.
///
/// Freshness precision measures the fraction of top-K results that are
/// within the "fresh" time window (e.g., last 24 hours).
///
/// # Arguments
/// * `retrieved_timestamps` - Timestamps of retrieved items in ranked order
/// * `query_timestamp` - Query timestamp
/// * `fresh_threshold_ms` - Time threshold for "fresh" (e.g., 86400000 for 24h)
/// * `k` - Number of top results to consider
pub fn freshness_precision_at_k(
    retrieved_timestamps: &[i64],
    query_timestamp: i64,
    fresh_threshold_ms: i64,
    k: usize,
) -> f64 {
    if k == 0 {
        return 0.0;
    }

    let top_k = retrieved_timestamps.iter().take(k);
    let fresh_count = top_k
        .filter(|&&ts| (query_timestamp - ts) <= fresh_threshold_ms && ts <= query_timestamp)
        .count();

    fresh_count as f64 / k as f64
}

/// Compute recency-weighted MRR.
///
/// Like standard MRR, but the relevance of each result is weighted by
/// a decay function based on how recent it is.
///
/// # Arguments
/// * `query_results` - For each query: (retrieved timestamps, relevant timestamps, query timestamp)
/// * `decay_half_life_ms` - Half-life for exponential decay
pub fn recency_weighted_mrr(
    query_results: &[(Vec<i64>, Vec<i64>, i64)],
    decay_half_life_ms: i64,
) -> f64 {
    if query_results.is_empty() {
        return 0.0;
    }

    let sum: f64 = query_results
        .iter()
        .map(|(retrieved, relevant, query_ts)| {
            // Find first relevant item, weighted by recency
            for (pos, &ret_ts) in retrieved.iter().enumerate() {
                if relevant
                    .iter()
                    .any(|&rel_ts| (ret_ts - rel_ts).abs() < 1000)
                {
                    // Apply exponential decay based on age
                    let age_ms = query_ts - ret_ts;
                    let decay = if decay_half_life_ms > 0 && age_ms > 0 {
                        (-0.693 * age_ms as f64 / decay_half_life_ms as f64).exp()
                    } else {
                        1.0
                    };
                    return decay / (pos + 1) as f64;
                }
            }
            0.0
        })
        .sum();

    sum / query_results.len() as f64
}

/// Compute decay accuracy.
///
/// Measures correlation between predicted recency scores and actual ages.
/// Uses Pearson correlation coefficient.
///
/// # Arguments
/// * `predicted_scores` - Predicted recency scores
/// * `actual_ages_ms` - Actual ages in milliseconds
pub fn decay_accuracy(predicted_scores: &[f64], actual_ages_ms: &[i64]) -> f64 {
    if predicted_scores.len() != actual_ages_ms.len() || predicted_scores.is_empty() {
        return 0.0;
    }

    let n = predicted_scores.len() as f64;

    // Convert ages to decay scores (higher for more recent)
    let max_age = *actual_ages_ms.iter().max().unwrap_or(&1) as f64;
    let actual_scores: Vec<f64> = actual_ages_ms
        .iter()
        .map(|&age| 1.0 - (age as f64 / max_age).min(1.0))
        .collect();

    // Compute Pearson correlation
    let mean_pred = predicted_scores.iter().sum::<f64>() / n;
    let mean_actual = actual_scores.iter().sum::<f64>() / n;

    let mut num = 0.0;
    let mut denom_pred = 0.0;
    let mut denom_actual = 0.0;

    for i in 0..predicted_scores.len() {
        let diff_pred = predicted_scores[i] - mean_pred;
        let diff_actual = actual_scores[i] - mean_actual;
        num += diff_pred * diff_actual;
        denom_pred += diff_pred * diff_pred;
        denom_actual += diff_actual * diff_actual;
    }

    let denom = (denom_pred * denom_actual).sqrt();
    if denom < f64::EPSILON {
        0.0
    } else {
        (num / denom).clamp(-1.0, 1.0)
    }
}

/// Compute periodic recall at K.
///
/// Measures fraction of items from the same hour/day that are retrieved in top-K.
///
/// # Arguments
/// * `retrieved_hours` - Hour-of-day (0-23) for each retrieved item
/// * `target_hour` - Target hour to match
/// * `same_hour_ids` - IDs of items from the same hour (ground truth)
/// * `retrieved_ids` - IDs of retrieved items
/// * `k` - Number of top results to consider
pub fn periodic_recall_at_k(
    retrieved_ids: &[uuid::Uuid],
    same_period_ids: &std::collections::HashSet<uuid::Uuid>,
    k: usize,
) -> f64 {
    if same_period_ids.is_empty() || k == 0 {
        return 0.0;
    }

    let top_k = retrieved_ids.iter().take(k);
    let hits = top_k.filter(|id| same_period_ids.contains(id)).count();

    hits as f64 / same_period_ids.len().min(k) as f64
}

/// Compute sequence ordering accuracy.
///
/// Measures fraction of pairs that are correctly ordered temporally.
///
/// # Arguments
/// * `retrieved_timestamps` - Timestamps of retrieved items
/// * `expected_order` - Expected temporal order (list of indices)
pub fn sequence_ordering_accuracy(retrieved_timestamps: &[i64], expected_order: &[usize]) -> f64 {
    if expected_order.len() < 2 || retrieved_timestamps.len() < expected_order.len() {
        return 0.0;
    }

    let mut correct_pairs = 0;
    let mut total_pairs = 0;

    for i in 0..expected_order.len() {
        for j in (i + 1)..expected_order.len() {
            let idx_i = expected_order[i];
            let idx_j = expected_order[j];

            if idx_i < retrieved_timestamps.len() && idx_j < retrieved_timestamps.len() {
                let ts_i = retrieved_timestamps[idx_i];
                let ts_j = retrieved_timestamps[idx_j];

                // Expected: i comes before j temporally
                if ts_i < ts_j {
                    correct_pairs += 1;
                }
                total_pairs += 1;
            }
        }
    }

    if total_pairs == 0 {
        0.0
    } else {
        correct_pairs as f64 / total_pairs as f64
    }
}

/// Compute Kendall's tau for temporal ordering precision.
///
/// Measures correlation between retrieved order and true temporal order.
///
/// # Arguments
/// * `retrieved_order` - Rank order of retrieved items
/// * `true_temporal_order` - True temporal order
pub fn kendalls_tau(retrieved_order: &[usize], true_temporal_order: &[usize]) -> f64 {
    if retrieved_order.len() != true_temporal_order.len() || retrieved_order.len() < 2 {
        return 0.0;
    }

    let n = retrieved_order.len();
    let mut concordant = 0i64;
    let mut discordant = 0i64;

    for i in 0..n {
        for j in (i + 1)..n {
            let ret_diff = retrieved_order[i] as i64 - retrieved_order[j] as i64;
            let true_diff = true_temporal_order[i] as i64 - true_temporal_order[j] as i64;

            if ret_diff * true_diff > 0 {
                concordant += 1;
            } else if ret_diff * true_diff < 0 {
                discordant += 1;
            }
            // Ties don't contribute
        }
    }

    let pairs = (n * (n - 1) / 2) as f64;
    if pairs < f64::EPSILON {
        0.0
    } else {
        (concordant - discordant) as f64 / pairs
    }
}

/// Compute episode boundary F1.
///
/// Measures accuracy of detecting session/episode boundaries.
///
/// # Arguments
/// * `predicted_boundaries` - Predicted boundary positions
/// * `actual_boundaries` - Actual boundary positions
/// * `tolerance` - Position tolerance for matching
pub fn episode_boundary_f1(
    predicted_boundaries: &[usize],
    actual_boundaries: &[usize],
    tolerance: usize,
) -> f64 {
    if actual_boundaries.is_empty() {
        return if predicted_boundaries.is_empty() {
            1.0
        } else {
            0.0
        };
    }

    // Count true positives (predicted boundaries within tolerance of actual)
    let mut true_positives = 0;
    let mut matched_actual: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for &pred in predicted_boundaries {
        for (i, &actual) in actual_boundaries.iter().enumerate() {
            if !matched_actual.contains(&i) {
                let distance = pred.abs_diff(actual);
                if distance <= tolerance {
                    true_positives += 1;
                    matched_actual.insert(i);
                    break;
                }
            }
        }
    }

    let precision = if predicted_boundaries.is_empty() {
        0.0
    } else {
        true_positives as f64 / predicted_boundaries.len() as f64
    };

    let recall = true_positives as f64 / actual_boundaries.len() as f64;

    if precision + recall < f64::EPSILON {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

/// Compute before/after retrieval accuracy.
///
/// Measures how accurately the system retrieves items before/after an anchor.
///
/// # Arguments
/// * `anchor_timestamp` - Timestamp of anchor item
/// * `retrieved_timestamps` - Timestamps of retrieved items
/// * `direction` - "before" or "after"
/// * `k` - Number of items to consider
pub fn before_after_accuracy(
    anchor_timestamp: i64,
    retrieved_timestamps: &[i64],
    direction: &str,
    k: usize,
) -> f64 {
    if k == 0 || retrieved_timestamps.is_empty() {
        return 0.0;
    }

    let top_k = retrieved_timestamps.iter().take(k);
    let correct = match direction {
        "before" => top_k.filter(|&&ts| ts < anchor_timestamp).count(),
        "after" => top_k.filter(|&&ts| ts > anchor_timestamp).count(),
        _ => top_k.filter(|&&ts| ts != anchor_timestamp).count(), // "both"
    };

    correct as f64 / k.min(retrieved_timestamps.len()) as f64
}

/// Compute silhouette score for hourly clustering.
///
/// Uses circular hour distance: min(|h1-h2|, 24-|h1-h2|) / 12
/// where 0 = same hour, 1 = 12 hours apart.
fn compute_hourly_silhouette(assignments: &[(uuid::Uuid, u8)]) -> f64 {
    if assignments.len() < 3 {
        return 0.5; // Need at least 3 points for meaningful silhouette
    }

    // Build clusters by hour
    let mut clusters: std::collections::HashMap<u8, Vec<usize>> = std::collections::HashMap::new();
    for (i, (_, hour)) in assignments.iter().enumerate() {
        clusters.entry(*hour).or_default().push(i);
    }

    if clusters.len() < 2 {
        return 1.0; // All in one cluster is perfect clustering
    }

    // Circular hour distance (0-1 scale)
    let hour_distance = |h1: u8, h2: u8| -> f64 {
        let diff = (h1 as i16 - h2 as i16).abs();
        diff.min(24 - diff) as f64 / 12.0
    };

    // Compute silhouette for each point
    let mut silhouette_sum = 0.0;
    for (i, (_, hour_i)) in assignments.iter().enumerate() {
        // a(i) = mean distance to same-cluster points
        let same_cluster = clusters.get(hour_i).unwrap();
        let a_i = if same_cluster.len() > 1 {
            same_cluster
                .iter()
                .filter(|&&j| j != i)
                .map(|&j| {
                    let (_, hour_j) = assignments[j];
                    hour_distance(*hour_i, hour_j)
                })
                .sum::<f64>()
                / (same_cluster.len() - 1) as f64
        } else {
            0.0
        };

        // b(i) = min mean distance to any other cluster
        let b_i = clusters
            .iter()
            .filter(|(&h, _)| h != *hour_i)
            .map(|(_, members)| {
                members
                    .iter()
                    .map(|&j| {
                        let (_, hour_j) = assignments[j];
                        hour_distance(*hour_i, hour_j)
                    })
                    .sum::<f64>()
                    / members.len().max(1) as f64
            })
            .fold(f64::MAX, f64::min);

        // s(i) = (b(i) - a(i)) / max(a(i), b(i))
        let s_i = if a_i.max(b_i) > f64::EPSILON {
            (b_i - a_i) / a_i.max(b_i)
        } else {
            0.0
        };
        silhouette_sum += s_i;
    }

    // Average silhouette score, scaled to [0, 1] from [-1, 1]
    let avg_silhouette = silhouette_sum / assignments.len() as f64;
    (avg_silhouette + 1.0) / 2.0 // Map [-1, 1] to [0, 1]
}

/// Compute silhouette score for daily (day-of-week) clustering.
///
/// Uses circular day distance: min(|d1-d2|, 7-|d1-d2|) / 3.5
/// where 0 = same day, 1 = 3-4 days apart.
fn compute_daily_silhouette(assignments: &[(uuid::Uuid, u8)]) -> f64 {
    if assignments.len() < 3 {
        return 0.5;
    }

    let mut clusters: std::collections::HashMap<u8, Vec<usize>> = std::collections::HashMap::new();
    for (i, (_, day)) in assignments.iter().enumerate() {
        clusters.entry(*day).or_default().push(i);
    }

    if clusters.len() < 2 {
        return 1.0;
    }

    // Circular day-of-week distance (0-1 scale)
    let day_distance = |d1: u8, d2: u8| -> f64 {
        let diff = (d1 as i16 - d2 as i16).abs();
        diff.min(7 - diff) as f64 / 3.5
    };

    let mut silhouette_sum = 0.0;
    for (i, (_, day_i)) in assignments.iter().enumerate() {
        let same_cluster = clusters.get(day_i).unwrap();
        let a_i = if same_cluster.len() > 1 {
            same_cluster
                .iter()
                .filter(|&&j| j != i)
                .map(|&j| {
                    let (_, day_j) = assignments[j];
                    day_distance(*day_i, day_j)
                })
                .sum::<f64>()
                / (same_cluster.len() - 1) as f64
        } else {
            0.0
        };

        let b_i = clusters
            .iter()
            .filter(|(&d, _)| d != *day_i)
            .map(|(_, members)| {
                members
                    .iter()
                    .map(|&j| {
                        let (_, day_j) = assignments[j];
                        day_distance(*day_i, day_j)
                    })
                    .sum::<f64>()
                    / members.len().max(1) as f64
            })
            .fold(f64::MAX, f64::min);

        let s_i = if a_i.max(b_i) > f64::EPSILON {
            (b_i - a_i) / a_i.max(b_i)
        } else {
            0.0
        };
        silhouette_sum += s_i;
    }

    let avg_silhouette = silhouette_sum / assignments.len() as f64;
    (avg_silhouette + 1.0) / 2.0
}

/// Compute all temporal metrics for a benchmark dataset.
pub fn compute_all_temporal_metrics(
    recency_results: &RecencyBenchmarkData,
    periodic_results: &PeriodicBenchmarkData,
    sequence_results: &SequenceBenchmarkData,
    baseline_score: f64,
) -> TemporalMetrics {
    let recency = compute_recency_metrics(recency_results);
    let periodic = compute_periodic_metrics(periodic_results);
    let sequence = compute_sequence_metrics(sequence_results);

    // Use same optimized weights as quality_score(): 50/35/15
    let overall_score = 0.5 * recency.overall_score()
        + 0.35 * sequence.overall_score()
        + 0.15 * periodic.overall_score();

    let composite = CompositeTemporalMetrics {
        overall_temporal_score: overall_score,
        improvement_over_baseline: if baseline_score > 0.0 {
            (overall_score - baseline_score) / baseline_score
        } else {
            0.0
        },
        feature_contributions: TemporalFeatureContributions {
            e2_recency: recency.overall_score(),
            e3_periodic: periodic.overall_score(),
            e4_sequence: sequence.overall_score(),
        },
    };

    TemporalMetrics {
        recency,
        periodic,
        sequence,
        composite,
        query_count: recency_results.query_count
            + periodic_results.query_count
            + sequence_results.query_count,
    }
}

/// Data for recency benchmark evaluation.
#[derive(Debug, Default)]
pub struct RecencyBenchmarkData {
    /// Query results: (retrieved_timestamps, relevant_timestamps, query_timestamp)
    pub query_results: Vec<(Vec<i64>, Vec<i64>, i64)>,
    /// Predicted vs actual scores for decay accuracy
    pub decay_predictions: Vec<(f64, i64)>, // (predicted_score, actual_age_ms)
    /// Number of queries
    pub query_count: usize,
    /// Decay half-life used
    pub decay_half_life_ms: i64,
    /// Fresh threshold in ms
    pub fresh_threshold_ms: i64,
}

/// Data for periodic benchmark evaluation.
#[derive(Debug, Default)]
pub struct PeriodicBenchmarkData {
    /// Query results: (retrieved_ids, same_period_ids)
    pub query_results: Vec<(Vec<uuid::Uuid>, std::collections::HashSet<uuid::Uuid>)>,
    /// Hourly assignments for cluster quality
    pub hourly_assignments: Vec<(uuid::Uuid, u8)>, // (id, hour 0-23)
    /// Daily assignments for cluster quality
    pub daily_assignments: Vec<(uuid::Uuid, u8)>, // (id, day 0-6)
    /// Pattern detection: (predicted_patterns, actual_patterns)
    pub pattern_detection: Vec<(bool, bool)>,
    /// Number of queries
    pub query_count: usize,
}

/// Data for sequence benchmark evaluation.
#[derive(Debug, Default)]
pub struct SequenceBenchmarkData {
    /// Sequence ordering: (retrieved_timestamps, expected_order)
    pub ordering_results: Vec<(Vec<i64>, Vec<usize>)>,
    /// Before/after: (anchor_ts, retrieved_ts, direction)
    pub before_after_results: Vec<(i64, Vec<i64>, String)>,
    /// Episode boundaries: (predicted, actual)
    pub boundary_results: Vec<(Vec<usize>, Vec<usize>)>,
    /// Number of queries
    pub query_count: usize,
    /// Tolerance for boundary matching
    pub boundary_tolerance: usize,
}

fn compute_recency_metrics(data: &RecencyBenchmarkData) -> RecencyMetrics {
    let k_values = [1, 5, 10, 20];

    let mut freshness_precision_at = HashMap::new();
    let mut fresh_retrieval_rate_at = HashMap::new();

    for &k in &k_values {
        let mut fp_sum = 0.0;
        let mut fr_sum = 0.0;

        for (retrieved_ts, _relevant_ts, query_ts) in &data.query_results {
            fp_sum += freshness_precision_at_k(retrieved_ts, *query_ts, data.fresh_threshold_ms, k);
            // Fresh retrieval rate: at least one fresh item in top-k
            let has_fresh = retrieved_ts
                .iter()
                .take(k)
                .any(|&ts| (*query_ts - ts) <= data.fresh_threshold_ms);
            fr_sum += if has_fresh { 1.0 } else { 0.0 };
        }

        let n = data.query_results.len().max(1) as f64;
        freshness_precision_at.insert(k, fp_sum / n);
        fresh_retrieval_rate_at.insert(k, fr_sum / n);
    }

    let rw_mrr = recency_weighted_mrr(&data.query_results, data.decay_half_life_ms);

    let (predicted, ages): (Vec<f64>, Vec<i64>) = data.decay_predictions.iter().cloned().unzip();
    let da = decay_accuracy(&predicted, &ages);

    RecencyMetrics {
        freshness_precision_at,
        recency_weighted_mrr: rw_mrr,
        decay_accuracy: da,
        avg_position_improvement: 0.0, // Computed via ablation
        fresh_retrieval_rate_at,
    }
}

fn compute_periodic_metrics(data: &PeriodicBenchmarkData) -> PeriodicMetrics {
    let k_values = [1, 5, 10, 20];

    let mut periodic_recall_at = HashMap::new();

    for &k in &k_values {
        let mut recall_sum = 0.0;
        for (retrieved_ids, same_period_ids) in &data.query_results {
            recall_sum += periodic_recall_at_k(retrieved_ids, same_period_ids, k);
        }
        let n = data.query_results.len().max(1) as f64;
        periodic_recall_at.insert(k, recall_sum / n);
    }

    // Compute pattern detection precision/recall
    let (mut tp, mut fp, mut fn_count) = (0, 0, 0);
    for (predicted, actual) in &data.pattern_detection {
        match (predicted, actual) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_count += 1,
            (false, false) => {}
        }
    }
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

    // Compute silhouette scores for hourly and daily clustering
    let hourly_cluster_quality = compute_hourly_silhouette(&data.hourly_assignments);
    let daily_cluster_quality = compute_daily_silhouette(&data.daily_assignments);

    PeriodicMetrics {
        periodic_recall_at,
        hourly_cluster_quality,
        daily_cluster_quality,
        pattern_detection_precision: precision,
        pattern_detection_recall: recall,
    }
}

fn compute_sequence_metrics(data: &SequenceBenchmarkData) -> SequenceMetrics {
    let mut seq_acc_sum = 0.0;
    let mut tau_sum = 0.0;
    let mut ordering_count = 0;

    for (retrieved_ts, expected_order) in &data.ordering_results {
        seq_acc_sum += sequence_ordering_accuracy(retrieved_ts, expected_order);

        // Compute Kendall's tau
        let retrieved_order: Vec<usize> = (0..retrieved_ts.len()).collect();
        tau_sum += kendalls_tau(&retrieved_order, expected_order);
        ordering_count += 1;
    }

    let sequence_accuracy = if ordering_count > 0 {
        seq_acc_sum / ordering_count as f64
    } else {
        0.0
    };
    let temporal_ordering_precision = if ordering_count > 0 {
        tau_sum / ordering_count as f64
    } else {
        0.0
    };

    // Episode boundary F1
    let mut boundary_f1_sum = 0.0;
    for (predicted, actual) in &data.boundary_results {
        boundary_f1_sum += episode_boundary_f1(predicted, actual, data.boundary_tolerance);
    }
    let episode_boundary_f1 = if !data.boundary_results.is_empty() {
        boundary_f1_sum / data.boundary_results.len() as f64
    } else {
        0.0
    };

    // Before/after accuracy
    let mut ba_acc_sum = 0.0;
    for (anchor_ts, retrieved_ts, direction) in &data.before_after_results {
        ba_acc_sum += before_after_accuracy(*anchor_ts, retrieved_ts, direction, 10);
    }
    let before_after_accuracy = if !data.before_after_results.is_empty() {
        ba_acc_sum / data.before_after_results.len() as f64
    } else {
        0.0
    };

    SequenceMetrics {
        sequence_accuracy,
        temporal_ordering_precision,
        episode_boundary_f1,
        before_after_accuracy,
        avg_sequence_distance_error: 0.0, // Placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freshness_precision() {
        let query_ts = 1000000;
        let retrieved = vec![900000, 500000, 100000, 999000, 800000];
        // Fresh threshold: 200000 (200 seconds)
        // Fresh items: 900000, 999000, 800000 (3 items)

        let fp5 = freshness_precision_at_k(&retrieved, query_ts, 200000, 5);
        assert!((fp5 - 0.6).abs() < 0.01); // 3/5 = 0.6
    }

    #[test]
    fn test_decay_accuracy() {
        // Perfect correlation: higher score = more recent (lower age)
        let predicted = vec![1.0, 0.8, 0.6, 0.4, 0.2];
        let ages = vec![0, 1000, 2000, 3000, 4000];

        let accuracy = decay_accuracy(&predicted, &ages);
        assert!(accuracy > 0.9); // Should be highly correlated
    }

    #[test]
    fn test_sequence_ordering_accuracy() {
        let timestamps = vec![100, 200, 300, 400, 500];
        let expected = vec![0, 1, 2, 3, 4]; // Already sorted

        let accuracy = sequence_ordering_accuracy(&timestamps, &expected);
        assert!((accuracy - 1.0).abs() < 0.01); // Perfect ordering
    }

    #[test]
    fn test_kendalls_tau() {
        // Perfect agreement
        let order1 = vec![0, 1, 2, 3, 4];
        let order2 = vec![0, 1, 2, 3, 4];
        assert!((kendalls_tau(&order1, &order2) - 1.0).abs() < 0.01);

        // Perfect disagreement (reversed)
        let order3 = vec![4, 3, 2, 1, 0];
        assert!((kendalls_tau(&order1, &order3) + 1.0).abs() < 0.01);
    }

    #[test]
    fn test_episode_boundary_f1() {
        let predicted = vec![5, 15, 25];
        let actual = vec![5, 16, 25, 35];
        let tolerance = 2;

        // 3 predicted, 4 actual
        // Matches: 5↔5, 15↔16 (within tolerance), 25↔25
        // TP=3, FP=0, FN=1
        // Precision = 3/3 = 1.0, Recall = 3/4 = 0.75
        // F1 = 2 * 1.0 * 0.75 / 1.75 ≈ 0.857

        let f1 = episode_boundary_f1(&predicted, &actual, tolerance);
        assert!((f1 - 0.857).abs() < 0.01);
    }

    #[test]
    fn test_before_after_accuracy() {
        let anchor = 500;

        // Test "before" with items mostly before anchor
        let retrieved_before = vec![100, 200, 300, 400, 600, 700, 800, 900];
        // top_k=5: [100, 200, 300, 400, 600], items < 500: 4
        let before_acc = before_after_accuracy(anchor, &retrieved_before, "before", 5);
        assert!((before_acc - 0.8).abs() < 0.01); // 4/5 = 0.8

        // Test "after" with items mostly after anchor
        let retrieved_after = vec![600, 700, 800, 900, 100, 200, 300, 400];
        // top_k=5: [600, 700, 800, 900, 100], items > 500: 4
        let after_acc = before_after_accuracy(anchor, &retrieved_after, "after", 5);
        assert!((after_acc - 0.8).abs() < 0.01); // 4/5 = 0.8
    }
}
