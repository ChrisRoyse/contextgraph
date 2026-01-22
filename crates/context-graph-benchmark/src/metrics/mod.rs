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
pub mod performance;
pub mod retrieval;
pub mod temporal;
pub mod temporal_realdata;
pub mod validation;

pub use causal::CausalMetrics;
pub use clustering::ClusteringMetrics;
pub use divergence::DivergenceMetrics;
pub use performance::PerformanceMetrics;
pub use retrieval::RetrievalMetrics;
pub use temporal::TemporalMetrics;
pub use temporal_realdata::TemporalRealdataMetrics;
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
