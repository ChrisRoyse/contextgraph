//! Benchmark runners for executing comparative analysis.
//!
//! This module provides the main harness for running benchmarks and
//! comparing multi-space vs single-embedder approaches.

pub mod causal;
pub mod comparative;
pub mod e1_semantic;
pub mod e4_hybrid_session;
pub mod e11_entity;
pub mod failfast;
pub mod graph_realdata;
pub mod mcp_intent;
pub mod multimodal;
pub mod retrieval;
pub mod scaling;
pub mod sparse;
pub mod temporal;
pub mod temporal_realdata;
pub mod topic;
pub mod unified_realdata;

pub use causal::{
    CausalBenchmarkConfig, CausalBenchmarkResults, CausalBenchmarkRunner,
    CausalAblationResults, CausalBenchmarkTimings, CausalDatasetStats,
    DirectionBenchmarkSettings, AsymmetricBenchmarkSettings, ReasoningBenchmarkSettings,
};
pub use multimodal::{
    E10BenchmarkTimings, E10MultimodalBenchmarkConfig, E10MultimodalBenchmarkResults,
    E10MultimodalBenchmarkRunner,
};
pub use comparative::{BenchmarkHarness, BenchmarkResults, ComparativeResults};
pub use retrieval::RetrievalRunner;
pub use scaling::ScalingRunner;
pub use sparse::{
    E6BenchmarkTimings, E6DatasetStats, E6SparseBenchmarkConfig, E6SparseBenchmarkResults,
    E6SparseBenchmarkRunner, ThresholdMetrics, ThresholdSweepResults,
};
pub use temporal::{
    TemporalBenchmarkConfig, TemporalBenchmarkResults, TemporalBenchmarkRunner,
    // Mode-specific benchmark functions
    run_e2_recency_benchmark, run_e3_periodic_benchmark, run_e4_sequence_benchmark,
    run_ablation_benchmark, run_scaling_benchmark, run_regression_benchmark,
    // E2 types
    E2RecencyBenchmarkResults, AdaptiveHalfLifeResults, AdaptiveScalingPoint, TimeWindowResults,
    // E3 types
    E3PeriodicBenchmarkResults, SilhouetteValidation,
    // E4 types
    E4SequenceBenchmarkResults, ChainLengthPoint, BetweenQueryResults, FallbackComparison,
    // Ablation types
    AblationConfig, AblationBenchmarkResults, AblationConfigResult, InterferenceReport,
    // Scaling types
    ScalingConfig, ScalingBenchmarkResults, ScalingPoint, DegradationCurves,
    // Regression types
    RegressionConfig, RegressionBenchmarkResults, RegressionFailure, TemporalBaseline,
};
pub use temporal_realdata::{
    TemporalRealdataBenchmarkConfig, TemporalRealdataBenchmarkResults, TemporalRealdataBenchmarkRunner,
    TimestampBaselineResults, TemporalBenchmarkTimings, TemporalDatasetStats,
};
pub use mcp_intent::{
    MCPIntentBenchmarkConfig, MCPIntentBenchmarkResults, MCPIntentBenchmarkRunner,
    MCPIntentTimings,
};
pub use topic::TopicRunner;
pub use failfast::{
    FailFastScenario, FailFastTrigger, ExpectedBehavior, FailFastResult,
    FailFastSummary, ErrorMessageQuality,
    build_sequence_tool_scenarios, analyze_error_message_quality, verify_failfast_behavior,
};
pub use e4_hybrid_session::{
    E4HybridSessionBenchmarkConfig, E4HybridSessionBenchmarkResults, E4HybridSessionBenchmarkRunner,
    E4HybridBenchmarkTimings, E4HybridDatasetStats, LegacyComparisonResults,
    TimestampBaselineResults as E4TimestampBaselineResults, ValidationSummary as E4ValidationSummary,
};
pub use e11_entity::{
    E11EntityBenchmarkConfig, E11EntityBenchmarkResults, E11EntityBenchmarkRunner,
    E11EntityBenchmarkTimings, E11EntityDatasetStats as E11RunnerDatasetStats,
    ExtractionBenchmarkResults, RetrievalBenchmarkResults, TransEBenchmarkResults,
    ValidationBenchmarkResults, GraphBenchmarkResults, QueryResult, TripleScoreResult,
    RelationshipInferenceResult, SampleExtraction, ConfusionMatrix,
};
