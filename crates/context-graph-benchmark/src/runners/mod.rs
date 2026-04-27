//! Benchmark runners for executing comparative analysis.
//!
//! This module provides the main harness for running benchmarks and
//! comparing multi-space vs single-embedder approaches.

pub mod causal;
pub mod comparative;
pub mod e11_entity;
pub mod e1_semantic;
pub mod e4_hybrid_session;
pub mod embedder_impact;
pub mod failfast;
pub mod graph_linking;
pub mod graph_realdata;
pub mod multimodal;
pub mod retrieval;
pub mod scaling;
pub mod sparse;
pub mod temporal;
pub mod temporal_realdata;
pub mod topic;
pub mod unified_realdata;

pub use causal::{
    AsymmetricBenchmarkSettings, CausalAblationResults, CausalBenchmarkConfig,
    CausalBenchmarkResults, CausalBenchmarkRunner, CausalBenchmarkTimings, CausalDatasetStats,
    DirectionBenchmarkSettings, ReasoningBenchmarkSettings,
};
pub use comparative::{BenchmarkHarness, BenchmarkResults, ComparativeResults};
pub use e11_entity::{
    ConfusionMatrix, E11EntityBenchmarkConfig, E11EntityBenchmarkResults, E11EntityBenchmarkRunner,
    E11EntityBenchmarkTimings, E11EntityDatasetStats as E11RunnerDatasetStats,
    ExtractionBenchmarkResults, GraphBenchmarkResults, QueryResult, RelationshipInferenceResult,
    RetrievalBenchmarkResults, SampleExtraction, TransEBenchmarkResults, TripleScoreResult,
    ValidationBenchmarkResults,
};
pub use e4_hybrid_session::{
    E4HybridBenchmarkTimings, E4HybridDatasetStats, E4HybridSessionBenchmarkConfig,
    E4HybridSessionBenchmarkResults, E4HybridSessionBenchmarkRunner, LegacyComparisonResults,
    TimestampBaselineResults as E4TimestampBaselineResults,
    ValidationSummary as E4ValidationSummary,
};
pub use embedder_impact::{
    AblationDelta, BenchmarkMetadata, ConstitutionalCompliance, EmbedderImpactConfig,
    EmbedderImpactResults, EmbedderImpactRunner, EnhancementDelta, ImpactRecommendation,
    RecommendationType, RetrievalImpactAnalysis, TierImpactResults,
};
pub use failfast::{
    analyze_error_message_quality, build_sequence_tool_scenarios, verify_failfast_behavior,
    ErrorMessageQuality, ExpectedBehavior, FailFastResult, FailFastScenario, FailFastSummary,
    FailFastTrigger,
};
pub use graph_linking::{
    run_quick_benchmark, run_tier_benchmark, GraphLinkingBenchmarkConfig,
    GraphLinkingBenchmarkResults, GraphLinkingBenchmarkRunner, GraphLinkingDatasetStats,
    ValidationCheck as GraphLinkingValidationCheck,
    ValidationSummary as GraphLinkingValidationSummary,
};
pub use multimodal::{
    E10BenchmarkTimings, E10MultimodalBenchmarkConfig, E10MultimodalBenchmarkResults,
    E10MultimodalBenchmarkRunner,
};
pub use retrieval::RetrievalRunner;
pub use scaling::ScalingRunner;
pub use sparse::{
    E6BenchmarkTimings, E6DatasetStats, E6SparseBenchmarkConfig, E6SparseBenchmarkResults,
    E6SparseBenchmarkRunner, ThresholdMetrics, ThresholdSweepResults,
};
pub use temporal::{
    run_ablation_benchmark,
    // Mode-specific benchmark functions
    run_e2_recency_benchmark,
    run_e3_periodic_benchmark,
    run_e4_sequence_benchmark,
    run_regression_benchmark,
    run_scaling_benchmark,
    AblationBenchmarkResults,
    // Ablation types
    AblationConfig,
    AblationConfigResult,
    AdaptiveHalfLifeResults,
    AdaptiveScalingPoint,
    BetweenQueryResults,
    ChainLengthPoint,
    DegradationCurves,
    // E2 types
    E2RecencyBenchmarkResults,
    // E3 types
    E3PeriodicBenchmarkResults,
    // E4 types
    E4SequenceBenchmarkResults,
    FallbackComparison,
    InterferenceReport,
    RegressionBenchmarkResults,
    // Regression types
    RegressionConfig,
    RegressionFailure,
    ScalingBenchmarkResults,
    // Scaling types
    ScalingConfig,
    ScalingPoint,
    SilhouetteValidation,
    TemporalBaseline,
    TemporalBenchmarkConfig,
    TemporalBenchmarkResults,
    TemporalBenchmarkRunner,
    TimeWindowResults,
};
pub use temporal_realdata::{
    TemporalBenchmarkTimings, TemporalDatasetStats, TemporalRealdataBenchmarkConfig,
    TemporalRealdataBenchmarkResults, TemporalRealdataBenchmarkRunner, TimestampBaselineResults,
};
pub use topic::TopicRunner;
