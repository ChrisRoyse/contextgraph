//! Degradation analysis: tracking accuracy drop as corpus grows.
//!
//! The key hypothesis is that single-embedder RAG degrades faster than multi-space
//! as corpus size increases. This module provides tools to measure and visualize
//! this degradation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::{targets, Tier};
use crate::metrics::ScalingMetrics;

/// A single data point in the degradation curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationPoint {
    /// Corpus size (number of documents).
    pub corpus_size: usize,
    /// Tier this measurement belongs to.
    pub tier: Tier,
    /// Metrics at this point.
    pub metrics: ScalingMetrics,
    /// Relative performance vs Tier 0 (1.0 = same, <1.0 = degraded).
    pub relative_performance: f64,
}

/// Complete degradation analysis results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationAnalysis {
    /// System being analyzed (e.g., "single-embedder" or "multi-space").
    pub system_name: String,

    /// Data points for each tier.
    pub points: Vec<DegradationPoint>,

    /// Baseline metrics (Tier 0).
    pub baseline: ScalingMetrics,

    /// Computed scaling limits.
    pub limits: ScalingLimits,

    /// Degradation rate (percentage drop per 10x corpus increase).
    pub degradation_rate: DegradationRates,
}

/// Degradation rates for different metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DegradationRates {
    /// Precision@10 drop per 10x scale increase.
    pub precision_10_per_10x: f64,
    /// Recall@10 drop per 10x scale increase.
    pub recall_10_per_10x: f64,
    /// MRR drop per 10x scale increase.
    pub mrr_per_10x: f64,
    /// Clustering purity drop per 10x scale increase.
    pub purity_per_10x: f64,
}

/// Scaling limits: corpus sizes where metrics drop below thresholds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScalingLimits {
    /// Corpus size where P@10 drops below 80% of Tier 0.
    pub precision_10_limit: Option<usize>,
    /// Corpus size where R@10 drops below 80% of Tier 0.
    pub recall_10_limit: Option<usize>,
    /// Corpus size where MRR drops below 80% of Tier 0.
    pub mrr_limit: Option<usize>,
    /// Corpus size where clustering purity drops below 80% of Tier 0.
    pub purity_limit: Option<usize>,
    /// Overall limit (minimum of all limits).
    pub overall_limit: Option<usize>,
}

impl DegradationAnalysis {
    /// Create new analysis with baseline.
    pub fn new(system_name: &str, baseline: ScalingMetrics) -> Self {
        Self {
            system_name: system_name.to_string(),
            points: Vec::new(),
            baseline,
            limits: ScalingLimits::default(),
            degradation_rate: DegradationRates::default(),
        }
    }

    /// Add a data point.
    pub fn add_point(&mut self, tier: Tier, corpus_size: usize, metrics: ScalingMetrics) {
        let relative = self.compute_relative_performance(&metrics);

        self.points.push(DegradationPoint {
            corpus_size,
            tier,
            metrics,
            relative_performance: relative,
        });

        // Keep points sorted by corpus size
        self.points.sort_by_key(|p| p.corpus_size);
    }

    /// Compute relative performance vs baseline.
    fn compute_relative_performance(&self, metrics: &ScalingMetrics) -> f64 {
        let baseline_score = self.baseline.retrieval.overall_score();
        let current_score = metrics.retrieval.overall_score();

        if baseline_score < f64::EPSILON {
            1.0
        } else {
            current_score / baseline_score
        }
    }

    /// Finalize analysis: compute limits and degradation rates.
    pub fn finalize(&mut self) {
        self.compute_limits();
        self.compute_degradation_rates();
    }

    /// Compute scaling limits.
    fn compute_limits(&mut self) {
        let threshold = targets::DEGRADATION_THRESHOLD;

        let baseline_p10 = self
            .baseline
            .retrieval
            .precision_at
            .get(&10)
            .copied()
            .unwrap_or(1.0);
        let baseline_r10 = self
            .baseline
            .retrieval
            .recall_at
            .get(&10)
            .copied()
            .unwrap_or(1.0);
        let baseline_mrr = self.baseline.retrieval.mrr;
        let baseline_purity = self.baseline.clustering.purity;

        for point in &self.points {
            let p10 = point
                .metrics
                .retrieval
                .precision_at
                .get(&10)
                .copied()
                .unwrap_or(0.0);
            let r10 = point
                .metrics
                .retrieval
                .recall_at
                .get(&10)
                .copied()
                .unwrap_or(0.0);
            let mrr = point.metrics.retrieval.mrr;
            let purity = point.metrics.clustering.purity;

            // Check P@10
            if self.limits.precision_10_limit.is_none() && p10 < baseline_p10 * threshold {
                self.limits.precision_10_limit = Some(point.corpus_size);
            }

            // Check R@10
            if self.limits.recall_10_limit.is_none() && r10 < baseline_r10 * threshold {
                self.limits.recall_10_limit = Some(point.corpus_size);
            }

            // Check MRR
            if self.limits.mrr_limit.is_none() && mrr < baseline_mrr * threshold {
                self.limits.mrr_limit = Some(point.corpus_size);
            }

            // Check purity
            if self.limits.purity_limit.is_none() && purity < baseline_purity * threshold {
                self.limits.purity_limit = Some(point.corpus_size);
            }
        }

        // Overall limit is minimum of all limits
        let all_limits = [
            self.limits.precision_10_limit,
            self.limits.recall_10_limit,
            self.limits.mrr_limit,
            self.limits.purity_limit,
        ];

        self.limits.overall_limit = all_limits.iter().filter_map(|&x| x).min();
    }

    /// Compute degradation rates using linear regression on log-scale.
    fn compute_degradation_rates(&mut self) {
        if self.points.len() < 2 {
            return;
        }

        let baseline_p10 = self
            .baseline
            .retrieval
            .precision_at
            .get(&10)
            .copied()
            .unwrap_or(1.0);
        let baseline_r10 = self
            .baseline
            .retrieval
            .recall_at
            .get(&10)
            .copied()
            .unwrap_or(1.0);
        let baseline_mrr = self.baseline.retrieval.mrr;
        let baseline_purity = self.baseline.clustering.purity;

        // Collect data points (log10(corpus_size), metric_value)
        let mut p10_points: Vec<(f64, f64)> = Vec::new();
        let mut r10_points: Vec<(f64, f64)> = Vec::new();
        let mut mrr_points: Vec<(f64, f64)> = Vec::new();
        let mut purity_points: Vec<(f64, f64)> = Vec::new();

        for point in &self.points {
            let log_size = (point.corpus_size as f64).log10();

            let p10 = point
                .metrics
                .retrieval
                .precision_at
                .get(&10)
                .copied()
                .unwrap_or(0.0);
            let r10 = point
                .metrics
                .retrieval
                .recall_at
                .get(&10)
                .copied()
                .unwrap_or(0.0);
            let mrr = point.metrics.retrieval.mrr;
            let purity = point.metrics.clustering.purity;

            p10_points.push((log_size, p10 / baseline_p10.max(0.01)));
            r10_points.push((log_size, r10 / baseline_r10.max(0.01)));
            mrr_points.push((log_size, mrr / baseline_mrr.max(0.01)));
            purity_points.push((log_size, purity / baseline_purity.max(0.01)));
        }

        // Compute slopes (degradation per log10 increase = per 10x)
        self.degradation_rate.precision_10_per_10x = 1.0 - compute_slope(&p10_points);
        self.degradation_rate.recall_10_per_10x = 1.0 - compute_slope(&r10_points);
        self.degradation_rate.mrr_per_10x = 1.0 - compute_slope(&mrr_points);
        self.degradation_rate.purity_per_10x = 1.0 - compute_slope(&purity_points);
    }

    /// Get degradation curve as (corpus_size, relative_performance) pairs.
    pub fn degradation_curve(&self) -> Vec<(usize, f64)> {
        self.points
            .iter()
            .map(|p| (p.corpus_size, p.relative_performance))
            .collect()
    }

    /// Check if this system has better scaling than another.
    pub fn scales_better_than(&self, other: &DegradationAnalysis) -> bool {
        // Compare overall limits (higher is better)
        match (self.limits.overall_limit, other.limits.overall_limit) {
            (None, Some(_)) => true, // No limit beats having a limit
            (Some(_), None) => false,
            (Some(a), Some(b)) => a > b,
            (None, None) => {
                // Compare degradation rates (lower is better)
                let self_rate = self.degradation_rate.precision_10_per_10x
                    + self.degradation_rate.recall_10_per_10x
                    + self.degradation_rate.mrr_per_10x;
                let other_rate = other.degradation_rate.precision_10_per_10x
                    + other.degradation_rate.recall_10_per_10x
                    + other.degradation_rate.mrr_per_10x;
                self_rate < other_rate
            }
        }
    }

    /// Compute Scaling Advantage Factor (SAF).
    ///
    /// SAF = breaking_point_multispace / breaking_point_single
    /// SAF > 1 means multi-space scales better.
    pub fn scaling_advantage_factor(&self, other: &DegradationAnalysis) -> Option<f64> {
        match (self.limits.overall_limit, other.limits.overall_limit) {
            (Some(self_limit), Some(other_limit)) => Some(self_limit as f64 / other_limit as f64),
            (None, Some(_)) => Some(f64::INFINITY), // No limit is infinite advantage
            _ => None,
        }
    }
}

/// Compute slope using simple linear regression.
fn compute_slope(points: &[(f64, f64)]) -> f64 {
    if points.len() < 2 {
        return 1.0;
    }

    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f64 = points.iter().map(|(x, _)| x * x).sum();

    let denominator = n * sum_xx - sum_x * sum_x;
    if denominator.abs() < f64::EPSILON {
        return 1.0;
    }

    (n * sum_xy - sum_x * sum_y) / denominator
}

/// Compare two systems' degradation analyses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationComparison {
    /// Name of system A.
    pub system_a: String,
    /// Name of system B.
    pub system_b: String,
    /// Scaling advantage factor (SAF) of A over B.
    pub scaling_advantage_factor: Option<f64>,
    /// Which system scales better.
    pub winner: String,
    /// Improvement percentages at each tier.
    pub tier_improvements: BTreeMap<String, TierImprovement>,
}

/// Improvement of one system over another at a specific tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierImprovement {
    pub precision_10: f64,
    pub recall_10: f64,
    pub mrr: f64,
    pub purity: f64,
}

impl DegradationComparison {
    /// Create comparison between two systems.
    pub fn compare(a: &DegradationAnalysis, b: &DegradationAnalysis) -> Self {
        let scaling_advantage_factor = a.scaling_advantage_factor(b);
        let winner = if a.scales_better_than(b) {
            a.system_name.clone()
        } else {
            b.system_name.clone()
        };

        let mut tier_improvements = BTreeMap::new();

        // Find matching tiers
        for point_a in &a.points {
            if let Some(point_b) = b.points.iter().find(|p| p.tier == point_a.tier) {
                let tier_name = point_a.tier.to_string();

                let p10_a = point_a
                    .metrics
                    .retrieval
                    .precision_at
                    .get(&10)
                    .copied()
                    .unwrap_or(0.0);
                let p10_b = point_b
                    .metrics
                    .retrieval
                    .precision_at
                    .get(&10)
                    .copied()
                    .unwrap_or(0.0);

                let r10_a = point_a
                    .metrics
                    .retrieval
                    .recall_at
                    .get(&10)
                    .copied()
                    .unwrap_or(0.0);
                let r10_b = point_b
                    .metrics
                    .retrieval
                    .recall_at
                    .get(&10)
                    .copied()
                    .unwrap_or(0.0);

                let mrr_a = point_a.metrics.retrieval.mrr;
                let mrr_b = point_b.metrics.retrieval.mrr;

                let purity_a = point_a.metrics.clustering.purity;
                let purity_b = point_b.metrics.clustering.purity;

                tier_improvements.insert(
                    tier_name,
                    TierImprovement {
                        precision_10: improvement_pct(p10_a, p10_b),
                        recall_10: improvement_pct(r10_a, r10_b),
                        mrr: improvement_pct(mrr_a, mrr_b),
                        purity: improvement_pct(purity_a, purity_b),
                    },
                );
            }
        }

        Self {
            system_a: a.system_name.clone(),
            system_b: b.system_name.clone(),
            scaling_advantage_factor,
            winner,
            tier_improvements,
        }
    }
}

/// Compute improvement percentage of a over b.
fn improvement_pct(a: f64, b: f64) -> f64 {
    if b.abs() < f64::EPSILON {
        0.0
    } else {
        (a - b) / b * 100.0
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use crate::metrics::{ClusteringMetrics, RetrievalMetrics};
    use std::collections::HashMap;

    fn make_metrics(p10: f64, r10: f64, mrr: f64, purity: f64) -> ScalingMetrics {
        let mut precision_at = HashMap::new();
        precision_at.insert(10, p10);

        let mut recall_at = HashMap::new();
        recall_at.insert(10, r10);

        ScalingMetrics {
            retrieval: RetrievalMetrics {
                precision_at,
                recall_at,
                mrr,
                ..Default::default()
            },
            clustering: ClusteringMetrics {
                purity,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_degradation_analysis() {
        let baseline = make_metrics(0.9, 0.8, 0.85, 0.9);
        let mut analysis = DegradationAnalysis::new("test", baseline);

        analysis.add_point(Tier::Tier0, 100, make_metrics(0.9, 0.8, 0.85, 0.9));
        analysis.add_point(Tier::Tier1, 1000, make_metrics(0.85, 0.75, 0.8, 0.85));
        analysis.add_point(Tier::Tier2, 10000, make_metrics(0.7, 0.6, 0.65, 0.7));

        analysis.finalize();

        // Check that degradation was detected
        assert!(analysis.limits.precision_10_limit.is_some());
        assert!(analysis.degradation_rate.precision_10_per_10x > 0.0);
    }

    #[test]
    fn test_scaling_comparison() {
        let baseline_good = make_metrics(0.9, 0.8, 0.85, 0.9);
        let baseline_bad = make_metrics(0.9, 0.8, 0.85, 0.9);

        let mut good_system = DegradationAnalysis::new("multi-space", baseline_good);
        let mut bad_system = DegradationAnalysis::new("single-embedder", baseline_bad);

        // Good system maintains performance
        good_system.add_point(Tier::Tier0, 100, make_metrics(0.9, 0.8, 0.85, 0.9));
        good_system.add_point(Tier::Tier1, 1000, make_metrics(0.88, 0.78, 0.83, 0.88));
        good_system.add_point(Tier::Tier2, 10000, make_metrics(0.85, 0.75, 0.8, 0.85));

        // Bad system degrades quickly
        bad_system.add_point(Tier::Tier0, 100, make_metrics(0.9, 0.8, 0.85, 0.9));
        bad_system.add_point(Tier::Tier1, 1000, make_metrics(0.7, 0.6, 0.65, 0.7));
        bad_system.add_point(Tier::Tier2, 10000, make_metrics(0.5, 0.4, 0.45, 0.5));

        good_system.finalize();
        bad_system.finalize();

        assert!(good_system.scales_better_than(&bad_system));
    }
}
