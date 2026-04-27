//! Unified benchmark results structures.
//!
//! Contains all result types for the unified real data benchmark, including
//! per-embedder metrics, fusion comparisons, and cross-embedder analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::{EmbedderName, FusionStrategy, UnifiedBenchmarkConfig};

/// Complete unified benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBenchmarkResults {
    /// Benchmark metadata.
    pub metadata: BenchmarkMetadata,
    /// Dataset information.
    pub dataset_info: DatasetInfo,
    /// Per-embedder results.
    pub per_embedder_results: HashMap<EmbedderName, EmbedderResults>,
    /// Fusion strategy comparison results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fusion_results: Option<FusionResults>,
    /// Cross-embedder correlation analysis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_embedder_analysis: Option<CrossEmbedderAnalysis>,
    /// Ablation study results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ablation_results: Option<AblationResults>,
    /// Recommendations based on results.
    pub recommendations: Vec<String>,
    /// Constitutional compliance checks.
    pub constitutional_compliance: ConstitutionalCompliance,
    /// Per-phase timing breakdown.
    pub timings: BenchmarkTimings,
}

/// Metadata about the benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetadata {
    /// Benchmark version.
    pub version: String,
    /// Start time.
    pub start_time: DateTime<Utc>,
    /// End time.
    pub end_time: DateTime<Utc>,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Configuration used.
    pub config: UnifiedBenchmarkConfig,
    /// Git commit hash (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    /// Hostname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
}

/// Information about the dataset used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    /// Total chunks in corpus.
    pub total_chunks: usize,
    /// Total documents.
    pub total_documents: usize,
    /// Number of unique topics.
    pub num_topics: usize,
    /// Top topics by chunk count.
    pub top_topics: Vec<TopicInfo>,
    /// Source datasets.
    pub source_datasets: Vec<String>,
    /// Chunks actually used (may be limited by max_chunks).
    pub chunks_used: usize,
    /// Queries generated.
    pub queries_generated: usize,
}

/// Information about a topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    /// Topic name.
    pub name: String,
    /// Number of chunks with this topic.
    pub chunk_count: usize,
    /// Percentage of total.
    pub percentage: f64,
}

/// Results for a single embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderResults {
    /// Embedder name.
    pub embedder_name: EmbedderName,
    /// Category (semantic, temporal, relational, structural).
    pub category: String,
    /// Topic weight for this embedder.
    pub topic_weight: f64,
    /// Mean Reciprocal Rank at k=10.
    pub mrr_at_10: f64,
    /// Precision at various k values.
    pub precision_at_k: HashMap<usize, f64>,
    /// Recall at various k values.
    pub recall_at_k: HashMap<usize, f64>,
    /// NDCG at various k values.
    pub ndcg_at_k: HashMap<usize, f64>,
    /// Mean Average Precision.
    pub map: f64,
    /// Hit rate (at least one relevant in top k).
    pub hit_rate_at_k: HashMap<usize, f64>,
    /// Contribution when added to E1 (improvement percentage).
    pub contribution_vs_e1: f64,
    /// Per-topic MRR breakdown.
    pub per_topic_mrr: HashMap<String, f64>,
    /// Latency metrics.
    pub latency: LatencyMetrics,
    /// Asymmetric ratio (for E5, E8, E10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asymmetric_ratio: Option<f64>,
    /// Additional embedder-specific metrics.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub extra_metrics: HashMap<String, f64>,
}

impl EmbedderResults {
    /// Create a new EmbedderResults with default values.
    pub fn new(embedder: EmbedderName) -> Self {
        Self {
            embedder_name: embedder,
            category: Self::category_for(embedder),
            topic_weight: embedder.topic_weight(),
            mrr_at_10: 0.0,
            precision_at_k: HashMap::new(),
            recall_at_k: HashMap::new(),
            ndcg_at_k: HashMap::new(),
            map: 0.0,
            hit_rate_at_k: HashMap::new(),
            contribution_vs_e1: 0.0,
            per_topic_mrr: HashMap::new(),
            latency: LatencyMetrics::default(),
            asymmetric_ratio: if EmbedderName::asymmetric().contains(&embedder) {
                Some(0.0)
            } else {
                None
            },
            extra_metrics: HashMap::new(),
        }
    }

    fn category_for(embedder: EmbedderName) -> String {
        if EmbedderName::semantic().contains(&embedder) {
            if embedder == EmbedderName::E1Semantic {
                "foundation"
            } else {
                "semantic_enhancer"
            }
        } else if EmbedderName::temporal().contains(&embedder) {
            "temporal_context"
        } else if EmbedderName::relational().contains(&embedder) {
            "relational_enhancer"
        } else {
            "structural_context"
        }
        .to_string()
    }
}

/// Latency metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyMetrics {
    /// Mean latency in milliseconds.
    pub mean_ms: f64,
    /// P50 latency.
    pub p50_ms: f64,
    /// P95 latency.
    pub p95_ms: f64,
    /// P99 latency.
    pub p99_ms: f64,
    /// Total queries.
    pub total_queries: usize,
}

/// Fusion strategy comparison results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionResults {
    /// Results per fusion strategy.
    pub by_strategy: HashMap<FusionStrategy, FusionStrategyResults>,
    /// Best strategy overall.
    pub best_strategy: FusionStrategy,
    /// Improvement of best over E1-only baseline.
    pub improvement_over_baseline: f64,
    /// Recommendations.
    pub recommendations: Vec<String>,
}

/// Results for a single fusion strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionStrategyResults {
    /// Strategy name.
    pub strategy: FusionStrategy,
    /// Embedders included in this strategy.
    pub embedders_used: Vec<EmbedderName>,
    /// MRR at k=10.
    pub mrr_at_10: f64,
    /// P@10.
    pub precision_at_10: f64,
    /// Recall@20.
    pub recall_at_20: f64,
    /// Latency.
    pub latency_ms: f64,
    /// Quality/latency ratio.
    pub quality_latency_ratio: f64,
}

/// Cross-embedder analysis based on MRR proximity.
///
/// NOTE: The `mrr_proximity_matrix` is computed as `1.0 - |mrr_i - mrr_j| / max(mrr_i, mrr_j)`,
/// which measures how close two embedders' MRR scores are. This is NOT a statistical correlation
/// (Pearson/Spearman) -- it is a rank agreement / proximity metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossEmbedderAnalysis {
    /// MRR proximity matrix (embedder x embedder).
    /// Values near 1.0 mean similar MRR performance; near 0.0 means divergent.
    pub mrr_proximity_matrix: Vec<Vec<f64>>,
    /// Embedder names for matrix rows/columns.
    pub embedder_order: Vec<EmbedderName>,
    /// Complementarity scores (how much each pair adds to each other).
    pub complementarity_scores: HashMap<String, f64>,
    /// Redundancy pairs (high MRR proximity, limited complementarity).
    pub redundancy_pairs: Vec<(EmbedderName, EmbedderName, f64)>,
    /// Best complementary pairs.
    pub best_complementary_pairs: Vec<(EmbedderName, EmbedderName, f64)>,
}

impl CrossEmbedderAnalysis {
    /// Get MRR proximity between two embedders.
    ///
    /// Returns a value in [0, 1] where 1.0 means identical MRR scores.
    /// This is NOT a statistical correlation -- it measures MRR score proximity.
    pub fn get_mrr_proximity(&self, a: EmbedderName, b: EmbedderName) -> Option<f64> {
        let idx_a = self.embedder_order.iter().position(|&e| e == a)?;
        let idx_b = self.embedder_order.iter().position(|&e| e == b)?;
        Some(self.mrr_proximity_matrix[idx_a][idx_b])
    }
}

/// Ablation study results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationResults {
    /// Results when each embedder is removed.
    pub removal_impact: HashMap<EmbedderName, AblationImpact>,
    /// Results when each embedder is added to E1.
    pub addition_impact: HashMap<EmbedderName, AblationImpact>,
    /// Critical embedders (removing causes >10% degradation).
    pub critical_embedders: Vec<EmbedderName>,
    /// Redundant embedders (removing causes <2% change).
    pub redundant_embedders: Vec<EmbedderName>,
}

/// Impact of adding or removing an embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationImpact {
    /// Embedder affected.
    pub embedder: EmbedderName,
    /// MRR change (positive = improvement, negative = degradation).
    pub mrr_change: f64,
    /// P@10 change.
    pub precision_change: f64,
    /// Recall@20 change.
    pub recall_change: f64,
    /// Statistical significance (p-value).
    pub p_value: f64,
    /// Is change significant (p < 0.05)?
    pub is_significant: bool,
}

/// Constitutional compliance checks per CLAUDE.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionalCompliance {
    /// All rules passed?
    pub all_passed: bool,
    /// Individual rule checks.
    pub rules: Vec<RuleCheck>,
    /// Warnings (non-critical).
    pub warnings: Vec<String>,
    /// Errors (critical failures).
    pub errors: Vec<String>,
}

/// Single rule compliance check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCheck {
    /// Rule ID (e.g., "ARCH-09").
    pub rule_id: String,
    /// Rule description.
    pub description: String,
    /// Did it pass?
    pub passed: bool,
    /// Details.
    pub details: String,
}

impl ConstitutionalCompliance {
    /// Create a new compliance checker.
    pub fn new() -> Self {
        Self {
            all_passed: true,
            rules: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Add a rule check.
    pub fn check_rule(&mut self, rule_id: &str, description: &str, passed: bool, details: &str) {
        if !passed {
            self.all_passed = false;
        }
        self.rules.push(RuleCheck {
            rule_id: rule_id.to_string(),
            description: description.to_string(),
            passed,
            details: details.to_string(),
        });
    }

    /// Add a warning.
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Add an error.
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.all_passed = false;
    }

    /// Check ARCH-09: Topic threshold is weighted_agreement >= 2.5.
    pub fn check_arch_09(&mut self, weighted_agreement: f64, is_topic: bool) {
        let should_be_topic = weighted_agreement >= 2.5;
        let passed = is_topic == should_be_topic;
        self.check_rule(
            "ARCH-09",
            "Topic threshold is weighted_agreement >= 2.5",
            passed,
            &format!(
                "weighted_agreement={:.2}, is_topic={}, should_be_topic={}",
                weighted_agreement, is_topic, should_be_topic
            ),
        );
    }

    /// Check ARCH-10: Divergence detection uses SEMANTIC embedders only.
    pub fn check_arch_10(&mut self, embedders_used: &[EmbedderName]) {
        let non_semantic: Vec<_> = embedders_used
            .iter()
            .filter(|e| !EmbedderName::semantic().contains(e))
            .collect();
        let passed = non_semantic.is_empty();
        self.check_rule(
            "ARCH-10",
            "Divergence detection uses SEMANTIC embedders only",
            passed,
            &format!("Non-semantic embedders in divergence: {:?}", non_semantic),
        );
    }

    /// Check AP-73: Temporal embedders not in similarity fusion.
    pub fn check_ap_73(&mut self, fusion_embedders: &[EmbedderName]) {
        let temporal_in_fusion: Vec<_> = fusion_embedders
            .iter()
            .filter(|e| EmbedderName::temporal().contains(e))
            .collect();
        let passed = temporal_in_fusion.is_empty();
        self.check_rule(
            "AP-73",
            "Temporal embedders (E2-E4) not in similarity fusion",
            passed,
            &format!("Temporal embedders in fusion: {:?}", temporal_in_fusion),
        );
    }

    /// Check asymmetric embedders achieve expected ratio (1.5 +/- 0.15).
    pub fn check_asymmetric_ratio(&mut self, embedder: EmbedderName, ratio: f64) {
        let expected = 1.5;
        let tolerance = 0.15;
        let passed = (ratio - expected).abs() <= tolerance;
        self.check_rule(
            &format!("ASYMMETRIC-{}", embedder.as_str()),
            &format!("{} asymmetric ratio within 1.5 +/- 0.15", embedder.as_str()),
            passed,
            &format!(
                "ratio={:.3}, expected={:.3}+/-{:.3}",
                ratio, expected, tolerance
            ),
        );
    }
}

impl Default for ConstitutionalCompliance {
    fn default() -> Self {
        Self::new()
    }
}

/// Timing breakdown for benchmark phases.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchmarkTimings {
    /// Dataset loading time.
    pub load_dataset_ms: u64,
    /// Temporal injection time.
    pub temporal_injection_ms: u64,
    /// Ground truth generation time.
    pub ground_truth_ms: u64,
    /// Embedding time (all 13 embedders).
    pub embedding_ms: u64,
    /// Per-embedder evaluation time.
    pub evaluation_ms: u64,
    /// Fusion comparison time.
    pub fusion_comparison_ms: u64,
    /// Cross-embedder analysis time.
    pub cross_embedder_ms: u64,
    /// Ablation study time.
    pub ablation_ms: u64,
    /// Report generation time.
    pub report_ms: u64,
    /// Total time.
    pub total_ms: u64,
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_results_new() {
        let results = EmbedderResults::new(EmbedderName::E1Semantic);
        assert_eq!(results.embedder_name, EmbedderName::E1Semantic);
        assert_eq!(results.category, "foundation");
        assert_eq!(results.topic_weight, 1.0);
    }

    #[test]
    fn test_constitutional_compliance() {
        let mut compliance = ConstitutionalCompliance::new();

        // Check ARCH-09
        compliance.check_arch_09(3.0, true); // Pass: 3.0 >= 2.5 and is_topic
        assert!(compliance.all_passed);

        compliance.check_arch_09(2.0, true); // Fail: 2.0 < 2.5 but is_topic=true
        assert!(!compliance.all_passed);
    }

    #[test]
    fn test_ap_73_check() {
        let mut compliance = ConstitutionalCompliance::new();

        // Should pass: no temporal embedders
        compliance.check_ap_73(&[EmbedderName::E1Semantic, EmbedderName::E5Causal]);
        assert!(compliance.all_passed);

        // Should fail: temporal embedder in fusion
        let mut compliance2 = ConstitutionalCompliance::new();
        compliance2.check_ap_73(&[EmbedderName::E1Semantic, EmbedderName::E2Recency]);
        assert!(!compliance2.all_passed);
    }

    #[test]
    fn test_asymmetric_check() {
        let mut compliance = ConstitutionalCompliance::new();

        // Should pass: ratio within tolerance
        compliance.check_asymmetric_ratio(EmbedderName::E5Causal, 1.52);
        assert!(compliance.all_passed);

        // Should fail: ratio outside tolerance
        let mut compliance2 = ConstitutionalCompliance::new();
        compliance2.check_asymmetric_ratio(EmbedderName::E5Causal, 1.8);
        assert!(!compliance2.all_passed);
    }
}
