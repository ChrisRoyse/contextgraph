//! JSON report generation for CI integration.
//!
//! Produces structured JSON suitable for automated analysis and regression detection.

use serde::{Deserialize, Serialize};

use super::BenchmarkSummary;
use crate::config::Tier;
use crate::runners::BenchmarkResults;

/// Complete JSON report structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReport {
    /// Report metadata.
    pub metadata: ReportMetadata,
    /// Summary of results.
    pub summary: BenchmarkSummary,
    /// Detailed results per tier.
    pub tiers: Vec<TierReport>,
    /// Comparison analysis.
    pub comparison: ComparisonReport,
    /// Breaking points (scaling limits).
    pub breaking_points: BreakingPointsReport,
}

/// Report metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    /// Report version for schema compatibility.
    pub version: String,
    /// Timestamp of generation.
    pub generated_at: String,
    /// System information.
    pub system: SystemInfo,
}

/// System information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Platform (linux, macos, windows).
    pub platform: String,
    /// Number of CPU cores.
    pub cpu_cores: usize,
    /// Available memory (bytes).
    pub available_memory_bytes: usize,
}

/// Results for a single tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierReport {
    /// Tier name.
    pub tier: String,
    /// Corpus size.
    pub corpus_size: usize,
    /// Number of topics.
    pub topic_count: usize,
    /// Single-embedder results.
    pub single_embedder: SystemMetrics,
    /// Multi-space results.
    pub multi_space: SystemMetrics,
    /// Improvement percentages.
    pub improvement: ImprovementMetrics,
}

/// Metrics for a single system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Retrieval metrics.
    pub retrieval: RetrievalMetricsJson,
    /// Clustering metrics.
    pub clustering: ClusteringMetricsJson,
    /// Performance metrics.
    pub performance: PerformanceMetricsJson,
}

/// Retrieval metrics for JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMetricsJson {
    pub precision_at_1: f64,
    pub precision_at_5: f64,
    pub precision_at_10: f64,
    pub precision_at_20: f64,
    pub recall_at_5: f64,
    pub recall_at_10: f64,
    pub recall_at_20: f64,
    pub mrr: f64,
    pub ndcg_at_10: f64,
    pub map: f64,
}

/// Clustering metrics for JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringMetricsJson {
    pub purity: f64,
    pub nmi: f64,
    pub ari: f64,
    pub silhouette: f64,
}

/// Performance metrics for JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetricsJson {
    pub latency_p50_us: u64,
    pub latency_p95_us: u64,
    pub latency_p99_us: u64,
    pub throughput_ops_sec: f64,
}

/// Improvement metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementMetrics {
    pub precision_10_pct: f64,
    pub recall_10_pct: f64,
    pub mrr_pct: f64,
    pub purity_pct: f64,
    pub nmi_pct: f64,
}

/// Comparison report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    /// Winner of the comparison.
    pub winner: String,
    /// Scaling advantage factor.
    pub scaling_advantage_factor: Option<f64>,
    /// Overall improvement percentage.
    pub overall_improvement_pct: f64,
    /// Memory overhead factor.
    pub memory_overhead_factor: Option<f64>,
    /// Multi-space advantages summary.
    pub multi_space_advantages: Vec<String>,
    /// Single-embedder advantages summary.
    pub single_embedder_advantages: Vec<String>,
}

/// Breaking points report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingPointsReport {
    /// Single-embedder breaking points.
    pub single_embedder: BreakingPoints,
    /// Multi-space breaking points.
    pub multi_space: BreakingPoints,
}

/// Breaking points for a system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingPoints {
    /// Corpus size where P@10 drops below threshold.
    pub precision_10: Option<usize>,
    /// Corpus size where R@10 drops below threshold.
    pub recall_10: Option<usize>,
    /// Corpus size where MRR drops below threshold.
    pub mrr: Option<usize>,
    /// Overall breaking point (minimum of above).
    pub overall: Option<usize>,
}

/// Generate JSON report from benchmark results.
pub fn generate_json(results: &BenchmarkResults) -> String {
    let report = build_report(results);
    serde_json::to_string_pretty(&report)
        .unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize: {}\"}}", e))
}

/// Build the JSON report structure.
fn build_report(results: &BenchmarkResults) -> JsonReport {
    let metadata = ReportMetadata {
        version: "1.0.0".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        system: SystemInfo {
            platform: std::env::consts::OS.to_string(),
            cpu_cores: num_cpus(),
            available_memory_bytes: available_memory(),
        },
    };

    let summary = BenchmarkSummary::from(results);

    let tiers = build_tier_reports(results);
    let comparison = build_comparison_report(results);
    let breaking_points = build_breaking_points_report(results);

    JsonReport {
        metadata,
        summary,
        tiers,
        comparison,
        breaking_points,
    }
}

/// Build tier reports.
fn build_tier_reports(results: &BenchmarkResults) -> Vec<TierReport> {
    let tier_map = results.tier_results_map();

    tier_map
        .into_iter()
        .map(|(tier_name, tier_summary)| {
            let tier = match tier_name.as_str() {
                "Tier0" => Tier::Tier0,
                "Tier1" => Tier::Tier1,
                "Tier2" => Tier::Tier2,
                "Tier3" => Tier::Tier3,
                "Tier4" => Tier::Tier4,
                "Tier5" => Tier::Tier5,
                _ => Tier::Tier0,
            };
            let config = crate::config::TierConfig::for_tier(tier);

            TierReport {
                tier: tier_name,
                corpus_size: config.memory_count,
                topic_count: config.topic_count,
                single_embedder: SystemMetrics {
                    retrieval: RetrievalMetricsJson {
                        precision_at_1: 0.0, // Not tracked in summary
                        precision_at_5: 0.0,
                        precision_at_10: tier_summary.single_embedder.precision_10,
                        precision_at_20: 0.0,
                        recall_at_5: 0.0,
                        recall_at_10: tier_summary.single_embedder.recall_10,
                        recall_at_20: 0.0,
                        mrr: tier_summary.single_embedder.mrr,
                        ndcg_at_10: tier_summary.single_embedder.ndcg_10,
                        map: 0.0,
                    },
                    clustering: ClusteringMetricsJson {
                        purity: tier_summary.single_embedder.purity,
                        nmi: tier_summary.single_embedder.nmi,
                        ari: 0.0,
                        silhouette: 0.0,
                    },
                    performance: PerformanceMetricsJson {
                        latency_p50_us: 0,
                        latency_p95_us: tier_summary.single_embedder.latency_p95_us,
                        latency_p99_us: 0,
                        throughput_ops_sec: 0.0,
                    },
                },
                multi_space: SystemMetrics {
                    retrieval: RetrievalMetricsJson {
                        precision_at_1: 0.0,
                        precision_at_5: 0.0,
                        precision_at_10: tier_summary.multi_space.precision_10,
                        precision_at_20: 0.0,
                        recall_at_5: 0.0,
                        recall_at_10: tier_summary.multi_space.recall_10,
                        recall_at_20: 0.0,
                        mrr: tier_summary.multi_space.mrr,
                        ndcg_at_10: tier_summary.multi_space.ndcg_10,
                        map: 0.0,
                    },
                    clustering: ClusteringMetricsJson {
                        purity: tier_summary.multi_space.purity,
                        nmi: tier_summary.multi_space.nmi,
                        ari: 0.0,
                        silhouette: 0.0,
                    },
                    performance: PerformanceMetricsJson {
                        latency_p50_us: 0,
                        latency_p95_us: tier_summary.multi_space.latency_p95_us,
                        latency_p99_us: 0,
                        throughput_ops_sec: 0.0,
                    },
                },
                improvement: ImprovementMetrics {
                    precision_10_pct: tier_summary.improvement.precision_10,
                    recall_10_pct: tier_summary.improvement.recall_10,
                    mrr_pct: tier_summary.improvement.mrr,
                    purity_pct: tier_summary.improvement.purity,
                    nmi_pct: tier_summary.improvement.nmi,
                },
            }
        })
        .collect()
}

/// Build comparison report.
fn build_comparison_report(results: &BenchmarkResults) -> ComparisonReport {
    let mut multi_advantages = Vec::new();
    let mut single_advantages = Vec::new();

    if results.comparison.overall_improvement > 0.0 {
        multi_advantages.push("Better overall retrieval quality".to_string());
    }

    if let Some(saf) = results.comparison.scaling_advantage_factor {
        if saf > 1.0 {
            multi_advantages.push(format!("{:.1}x better scaling", saf));
        }
    }

    if let Some(overhead) = results.comparison.memory_overhead_factor {
        if overhead < 15.0 {
            multi_advantages.push("Reasonable memory overhead for accuracy gains".to_string());
        } else {
            single_advantages.push("Lower memory footprint".to_string());
        }
    }

    single_advantages.push("Simpler implementation".to_string());
    single_advantages.push("Faster embedding time".to_string());

    ComparisonReport {
        winner: results.comparison.winner.clone(),
        scaling_advantage_factor: results.comparison.scaling_advantage_factor,
        overall_improvement_pct: results.comparison.overall_improvement * 100.0,
        memory_overhead_factor: results.comparison.memory_overhead_factor,
        multi_space_advantages: multi_advantages,
        single_embedder_advantages: single_advantages,
    }
}

/// Build breaking points report.
fn build_breaking_points_report(results: &BenchmarkResults) -> BreakingPointsReport {
    let single_bp = if let Some(ref deg) = results.single_embedder.degradation {
        BreakingPoints {
            precision_10: deg.limits.precision_10_limit,
            recall_10: deg.limits.recall_10_limit,
            mrr: deg.limits.mrr_limit,
            overall: deg.limits.overall_limit,
        }
    } else {
        BreakingPoints {
            precision_10: None,
            recall_10: None,
            mrr: None,
            overall: None,
        }
    };

    let multi_bp = if let Some(ref deg) = results.multi_space.degradation {
        BreakingPoints {
            precision_10: deg.limits.precision_10_limit,
            recall_10: deg.limits.recall_10_limit,
            mrr: deg.limits.mrr_limit,
            overall: deg.limits.overall_limit,
        }
    } else {
        BreakingPoints {
            precision_10: None,
            recall_10: None,
            mrr: None,
            overall: None,
        }
    };

    BreakingPointsReport {
        single_embedder: single_bp,
        multi_space: multi_bp,
    }
}

/// Get number of CPUs.
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1)
}

/// Get available memory (placeholder).
fn available_memory() -> usize {
    // In production, use sys-info crate or similar
    16 * 1024 * 1024 * 1024 // 16GB placeholder
}
