//! Comparative benchmark harness.
//!
//! Main entry point for running side-by-side comparisons of multi-space
//! and single-embedder systems.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::{BenchmarkConfig, Tier};
use crate::metrics::ScalingMetrics;
use crate::scaling::DegradationComparison;

use super::scaling::{ScalingAnalysisResults, ScalingRunner};

/// Main benchmark harness for comparative analysis.
pub struct BenchmarkHarness {
    config: BenchmarkConfig,
}

impl BenchmarkHarness {
    /// Create a new harness with default CI config.
    pub fn new() -> Self {
        Self {
            config: BenchmarkConfig::ci(),
        }
    }

    /// Create with specific config.
    pub fn with_config(config: BenchmarkConfig) -> Self {
        Self { config }
    }

    /// Create for full benchmark (all tiers).
    pub fn full() -> Self {
        Self {
            config: BenchmarkConfig::full(),
        }
    }

    /// Run the full benchmark suite.
    pub fn run_full_suite(&self) -> BenchmarkResults {
        let scaling_runner = ScalingRunner::new(self.config.clone());

        // Run both systems
        let single_results = scaling_runner.run_single_embedder();
        let multi_results = scaling_runner.run_multi_space();

        // Build comparative results
        let comparison = self.build_comparison(&single_results, &multi_results);

        BenchmarkResults {
            config: self.config.clone(),
            single_embedder: single_results,
            multi_space: multi_results,
            comparison,
        }
    }

    /// Run scaling analysis only.
    pub fn run_scaling_analysis(&self) -> ScalingAnalysisResults {
        let scaling_runner = ScalingRunner::new(self.config.clone());
        scaling_runner.run_multi_space()
    }

    /// Build comparison between two systems.
    fn build_comparison(
        &self,
        single: &ScalingAnalysisResults,
        multi: &ScalingAnalysisResults,
    ) -> ComparativeResults {
        // Compute improvement metrics per tier
        let mut tier_improvements = HashMap::new();

        for tier_config in &self.config.tiers {
            let tier = tier_config.tier;

            if let (Some(single_metrics), Some(multi_metrics)) =
                (single.get_tier_metrics(tier), multi.get_tier_metrics(tier))
            {
                tier_improvements.insert(
                    tier,
                    TierImprovement {
                        precision_10: compute_improvement(
                            single_metrics
                                .retrieval
                                .precision_at
                                .get(&10)
                                .copied()
                                .unwrap_or(0.0),
                            multi_metrics
                                .retrieval
                                .precision_at
                                .get(&10)
                                .copied()
                                .unwrap_or(0.0),
                        ),
                        recall_10: compute_improvement(
                            single_metrics
                                .retrieval
                                .recall_at
                                .get(&10)
                                .copied()
                                .unwrap_or(0.0),
                            multi_metrics
                                .retrieval
                                .recall_at
                                .get(&10)
                                .copied()
                                .unwrap_or(0.0),
                        ),
                        mrr: compute_improvement(
                            single_metrics.retrieval.mrr,
                            multi_metrics.retrieval.mrr,
                        ),
                        purity: compute_improvement(
                            single_metrics.clustering.purity,
                            multi_metrics.clustering.purity,
                        ),
                        nmi: compute_improvement(
                            single_metrics.clustering.nmi,
                            multi_metrics.clustering.nmi,
                        ),
                    },
                );
            }
        }

        // Compute degradation comparison
        let degradation_comparison = if let (Some(ref single_deg), Some(ref multi_deg)) =
            (&single.degradation, &multi.degradation)
        {
            Some(DegradationComparison::compare(multi_deg, single_deg))
        } else {
            None
        };

        // Compute scaling advantage factor
        let scaling_advantage = if let (Some(ref single_deg), Some(ref multi_deg)) =
            (&single.degradation, &multi.degradation)
        {
            multi_deg.scaling_advantage_factor(single_deg)
        } else {
            None
        };

        // Determine winner based on scaling (if meaningful) or metric improvements
        // Need at least 2 tiers for meaningful scaling comparison
        let has_meaningful_scaling =
            single.tier_results.len() >= 2 && multi.tier_results.len() >= 2;

        let winner = if has_meaningful_scaling {
            if let (Some(ref single_deg), Some(ref multi_deg)) =
                (&single.degradation, &multi.degradation)
            {
                // Use scaling analysis for multi-tier benchmarks
                if multi_deg.scales_better_than(single_deg) {
                    "multi-space".to_string()
                } else {
                    "single-embedder".to_string()
                }
            } else {
                "unknown".to_string()
            }
        } else {
            // For single-tier tests, use metric improvements to determine winner
            // Multi-space wins if it has better retrieval or clustering metrics overall
            let overall_improvement = compute_overall_improvement(&tier_improvements);
            if overall_improvement > 0.0 {
                "multi-space".to_string()
            } else if overall_improvement < 0.0 {
                "single-embedder".to_string()
            } else {
                "tie".to_string()
            }
        };

        // Compute overall improvement
        let overall_improvement = compute_overall_improvement(&tier_improvements);

        // Memory overhead
        let memory_overhead = if let (Some(ref single_mem), Some(ref multi_mem)) =
            (&single.memory_report, &multi.memory_report)
        {
            if let (Some(single_first), Some(multi_first)) =
                (single_mem.profiles.first(), multi_mem.profiles.first())
            {
                if single_first.total_bytes > 0 {
                    Some(multi_first.total_bytes as f64 / single_first.total_bytes as f64)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        ComparativeResults {
            tier_improvements,
            degradation_comparison,
            scaling_advantage_factor: scaling_advantage,
            winner,
            overall_improvement,
            memory_overhead_factor: memory_overhead,
        }
    }
}

impl Default for BenchmarkHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// Configuration used.
    pub config: BenchmarkConfig,
    /// Single-embedder results.
    #[serde(skip)]
    pub single_embedder: ScalingAnalysisResults,
    /// Multi-space results.
    #[serde(skip)]
    pub multi_space: ScalingAnalysisResults,
    /// Comparative analysis.
    pub comparison: ComparativeResults,
}

impl BenchmarkResults {
    /// Get summary string.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "Benchmark Results: {} vs {}\n",
            self.multi_space.system_name, self.single_embedder.system_name
        ));
        s.push_str(&format!("Winner: {}\n", self.comparison.winner));

        if let Some(saf) = self.comparison.scaling_advantage_factor {
            s.push_str(&format!("Scaling Advantage Factor: {:.2}x\n", saf));
        }

        s.push_str(&format!(
            "Overall Improvement: {:.1}%\n",
            self.comparison.overall_improvement * 100.0
        ));

        if let Some(mem_overhead) = self.comparison.memory_overhead_factor {
            s.push_str(&format!("Memory Overhead: {:.2}x\n", mem_overhead));
        }

        s
    }

    /// Check if multi-space is the winner.
    pub fn multi_space_wins(&self) -> bool {
        self.comparison.winner == "multi-space"
    }

    /// Get tier results as JSON-serializable map.
    pub fn tier_results_map(&self) -> HashMap<String, TierResultsSummary> {
        let mut map = HashMap::new();

        for (tier, metrics) in &self.multi_space.tier_results {
            let single_metrics = self
                .single_embedder
                .tier_results
                .iter()
                .find(|(t, _)| t == tier)
                .map(|(_, m)| m);

            map.insert(
                tier.to_string(),
                TierResultsSummary {
                    multi_space: MetricsSummary::from_metrics(metrics),
                    single_embedder: single_metrics
                        .map(MetricsSummary::from_metrics)
                        .unwrap_or_default(),
                    improvement: self
                        .comparison
                        .tier_improvements
                        .get(tier)
                        .cloned()
                        .unwrap_or_default(),
                },
            );
        }

        map
    }
}

/// Comparative analysis results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparativeResults {
    /// Improvement metrics per tier.
    pub tier_improvements: HashMap<Tier, TierImprovement>,
    /// Degradation comparison.
    #[serde(skip)]
    pub degradation_comparison: Option<DegradationComparison>,
    /// Scaling Advantage Factor (multi-space limit / single limit).
    pub scaling_advantage_factor: Option<f64>,
    /// Which system performed better overall.
    pub winner: String,
    /// Overall improvement (average across metrics and tiers).
    pub overall_improvement: f64,
    /// Memory overhead factor (multi-space / single).
    pub memory_overhead_factor: Option<f64>,
}

/// Improvement metrics for a single tier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierImprovement {
    /// P@10 improvement (percentage).
    pub precision_10: f64,
    /// R@10 improvement (percentage).
    pub recall_10: f64,
    /// MRR improvement (percentage).
    pub mrr: f64,
    /// Clustering purity improvement (percentage).
    pub purity: f64,
    /// NMI improvement (percentage).
    pub nmi: f64,
}

/// Summary of key metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub precision_10: f64,
    pub recall_10: f64,
    pub mrr: f64,
    pub ndcg_10: f64,
    pub purity: f64,
    pub nmi: f64,
    pub latency_p95_us: u64,
}

impl MetricsSummary {
    fn from_metrics(metrics: &ScalingMetrics) -> Self {
        Self {
            precision_10: metrics
                .retrieval
                .precision_at
                .get(&10)
                .copied()
                .unwrap_or(0.0),
            recall_10: metrics.retrieval.recall_at.get(&10).copied().unwrap_or(0.0),
            mrr: metrics.retrieval.mrr,
            ndcg_10: metrics.retrieval.ndcg_at.get(&10).copied().unwrap_or(0.0),
            purity: metrics.clustering.purity,
            nmi: metrics.clustering.nmi,
            latency_p95_us: metrics.performance.latency_us.p95(),
        }
    }
}

/// Summary for a single tier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierResultsSummary {
    pub multi_space: MetricsSummary,
    pub single_embedder: MetricsSummary,
    pub improvement: TierImprovement,
}

/// Compute percentage improvement from baseline to improved.
fn compute_improvement(baseline: f64, improved: f64) -> f64 {
    if baseline.abs() < f64::EPSILON {
        0.0
    } else {
        (improved - baseline) / baseline * 100.0
    }
}

/// Compute overall improvement across all tiers and metrics.
fn compute_overall_improvement(tier_improvements: &HashMap<Tier, TierImprovement>) -> f64 {
    if tier_improvements.is_empty() {
        return 0.0;
    }

    let total: f64 = tier_improvements
        .values()
        .map(|imp| (imp.precision_10 + imp.recall_10 + imp.mrr + imp.purity + imp.nmi) / 5.0)
        .sum();

    total / tier_improvements.len() as f64 / 100.0 // Convert to fraction
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_harness() {
        let harness = BenchmarkHarness::with_config(BenchmarkConfig::single_tier(Tier::Tier0));
        let results = harness.run_full_suite();

        // Should have results for both systems
        assert!(!results.single_embedder.tier_results.is_empty());
        assert!(!results.multi_space.tier_results.is_empty());
        assert!(!results.comparison.winner.is_empty());
    }

    #[test]
    fn test_compute_improvement() {
        assert!((compute_improvement(0.5, 0.6) - 20.0).abs() < 0.1);
        assert!((compute_improvement(1.0, 1.0) - 0.0).abs() < 0.1);
        assert!((compute_improvement(0.0, 0.5) - 0.0).abs() < 0.1); // Division by zero protection
    }
}
