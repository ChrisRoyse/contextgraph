//! Metrics for evaluating retrieval, clustering, and performance.
//!
//! This module provides comprehensive metrics for comparing multi-space and single-embedder approaches:
//!
//! - **Retrieval**: P@K, R@K, MRR, NDCG, MAP
//! - **Clustering**: Purity, NMI, ARI, Silhouette
//! - **Divergence**: TPR, FPR for topic drift detection
//! - **Performance**: Latency percentiles, throughput, memory
//! - **Temporal**: E2/E3/E4 embedder effectiveness metrics
//! - **Causal**: E5 embedder effectiveness metrics (direction detection, asymmetric retrieval)
//! - **Validation**: Input validation overhead and correctness metrics

pub mod causal;
pub mod clustering;
pub mod divergence;
pub mod e4_hybrid_session;
pub mod mcp_intent;
pub mod multimodal;
pub mod performance;
pub mod retrieval;
pub mod sparse;
pub mod temporal;
pub mod temporal_realdata;
pub mod validation;

pub use causal::CausalMetrics;
pub use multimodal::{
    AsymmetricRetrievalMetrics, AsymmetricRetrievalResult, BlendAnalysisPoint,
    ContextMatchingMetrics, ContextMatchingResult, E10AblationMetrics, E10MultimodalMetrics,
    IntentDetectionMetrics, IntentDetectionResult,
};
pub use clustering::ClusteringMetrics;
pub use divergence::DivergenceMetrics;
pub use performance::PerformanceMetrics;
pub use retrieval::RetrievalMetrics;
pub use sparse::{
    E6AblationMetrics, E6SparseMetrics, KeywordPrecisionMetrics, KeywordQueryResult,
    RetrievalQualityMetrics, SparsityAnalysisMetrics, SparseVectorStats,
};
pub use temporal::TemporalMetrics;
pub use temporal_realdata::TemporalRealdataMetrics;
pub use mcp_intent::{
    AsymmetricPairResult, AsymmetricValidationMetrics, BlendSweepPoint as MCPBlendSweepPoint,
    ConstitutionalComplianceMetrics, E10EnhancementMetrics, MCPIntentMetrics,
    MCPToolMetrics, RuleComplianceResult, ToolMetrics as MCPToolMetric,
    compute_mrr as mcp_compute_mrr, compute_ndcg_at_k as mcp_compute_ndcg,
    compute_percentile as mcp_compute_percentile, compute_precision_at_k as mcp_compute_precision,
    compute_recall_at_k as mcp_compute_recall,
};
pub use validation::{
    ValidationMetrics, ToolValidationMetrics, TestCaseResult,
    BoundaryTestConfig, BoundaryTestValue, sequence_tool_boundary_configs,
};

/// Combined metrics for a single benchmark run.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ScalingMetrics {
    /// Retrieval quality metrics.
    pub retrieval: RetrievalMetrics,
    /// Clustering quality metrics.
    pub clustering: ClusteringMetrics,
    /// Divergence detection metrics.
    pub divergence: DivergenceMetrics,
    /// Performance metrics.
    pub performance: PerformanceMetrics,
}

impl ScalingMetrics {
    /// Compute overall quality score (weighted combination).
    pub fn quality_score(&self) -> f64 {
        // 40% retrieval, 30% clustering, 20% divergence accuracy, 10% performance
        0.4 * self.retrieval.overall_score()
            + 0.3 * self.clustering.overall_score()
            + 0.2 * self.divergence.accuracy()
            + 0.1 * self.performance.normalized_score()
    }

    /// Check if all metrics meet minimum thresholds.
    pub fn meets_thresholds(&self, min_precision: f64, min_recall: f64, min_mrr: f64) -> bool {
        self.retrieval.precision_at.get(&10).copied().unwrap_or(0.0) >= min_precision
            && self.retrieval.recall_at.get(&10).copied().unwrap_or(0.0) >= min_recall
            && self.retrieval.mrr >= min_mrr
    }
}
