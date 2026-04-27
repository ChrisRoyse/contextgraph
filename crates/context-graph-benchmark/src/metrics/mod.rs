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
//! - **E1 Semantic**: Foundation embedder retrieval and topic separation metrics

pub mod causal;
pub mod clustering;
pub mod divergence;
pub mod e11_entity;
pub mod e1_semantic;
pub mod e4_hybrid_session;
pub mod e7_code;
pub mod e7_iou;
pub mod embedder_contribution;
pub mod graph_linking;
pub mod graph_structure;
pub mod multimodal;
pub mod performance;
pub mod resource_usage;
pub mod retrieval;
pub mod sparse;
pub mod temporal;
pub mod temporal_realdata;
pub mod validation;

pub use causal::{
    CausalMetrics, DirectionDistributionMetrics, DirectionMRRBreakdown, E5ConstitutionalCompliance,
    E5ImpactAnalysis, E5VectorVerificationMetrics, SymmetricVsAsymmetricComparison,
};
pub use clustering::ClusteringMetrics;
pub use divergence::DivergenceMetrics;
pub use e11_entity::{
    compute_mrr as e11_compute_mrr, compute_ndcg_at_k as e11_compute_ndcg,
    thresholds as e11_thresholds, E11EntityMetrics, E11PerformanceMetrics, EntityGraphMetrics,
    EntityRetrievalMetrics, EntityTypeMetrics, ExtractionMetrics, LatencyStats,
    RetrievalApproachComparison, ScoreDistribution, TransEMetrics,
};
pub use e7_code::{
    e1_unique_finds, e7_unique_finds, mrr as e7_mrr, ndcg_at_k as e7_ndcg,
    precision_at_k as e7_precision, recall_at_k as e7_recall, E7BenchmarkMetrics, E7GroundTruth,
    E7QueryResult, E7QueryType, E7QueryTypeMetrics,
};
pub use e7_iou::{
    compute_iou_at_k, compute_iou_result, compute_token_iou, extract_token_strings, tokenize_code,
    CodeToken, IoUMetrics, IoUResult, TokenType,
};
pub use embedder_contribution::{
    AgreementPatternAnalysis, BlindSpotAnalysis, BlindSpotExample, ContributionAttribution,
    RRFRankContribution, ResultContribution, UniqueFinds,
};
pub use graph_linking::{
    BackgroundBuilderMetrics, EdgeBuilderMetrics, GraphExpansionMetrics, GraphLinkingReport,
    LatencyStats as GraphLatencyStats, LatencyTracker, MemoryStats, NNDescentMetrics, RGCNMetrics,
    WeightComparisonMetrics, WeightProjectionMetrics,
};
pub use graph_structure::{
    AgreementDistribution, CategoryWeights, ConnectivitySnapshot, EdgeTypeDistribution,
    EmbedderConnectivity, GraphConnectivityMetrics, GraphStructureImpact, TopicFormationImpact,
    WeightedAgreementAnalysis,
};
pub use multimodal::{
    AsymmetricRetrievalMetrics, AsymmetricRetrievalResult, BlendAnalysisPoint,
    ContextMatchingMetrics, ContextMatchingResult, E10AblationMetrics, E10MultimodalMetrics,
    ParaphraseDetectionMetrics, ParaphraseDetectionResult,
};
pub use performance::PerformanceMetrics;
pub use resource_usage::{
    EmbedderEfficiency, IndexStats, IndexType, LatencyStats as ResourceLatencyStats,
    MemoryFootprint, ResourceImpact, StorageEfficiency,
};
pub use retrieval::RetrievalMetrics;
pub use sparse::{
    E6AblationMetrics, E6SparseMetrics, KeywordPrecisionMetrics, KeywordQueryResult,
    RetrievalQualityMetrics, SparseVectorStats, SparsityAnalysisMetrics,
};
pub use temporal::TemporalMetrics;
pub use temporal_realdata::TemporalRealdataMetrics;
pub use validation::{
    sequence_tool_boundary_configs, BoundaryTestConfig, BoundaryTestValue, TestCaseResult,
    ToolValidationMetrics, ValidationMetrics,
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
