//! Benchmark runners for executing comparative analysis.
//!
//! This module provides the main harness for running benchmarks and
//! comparing multi-space vs single-embedder approaches.

pub mod causal;
pub mod comparative;
pub mod failfast;
pub mod retrieval;
pub mod scaling;
pub mod temporal;
pub mod temporal_realdata;
pub mod topic;

pub use causal::{
    CausalBenchmarkConfig, CausalBenchmarkResults, CausalBenchmarkRunner,
    CausalAblationResults, CausalBenchmarkTimings, CausalDatasetStats,
    DirectionBenchmarkSettings, AsymmetricBenchmarkSettings, ReasoningBenchmarkSettings,
};
pub use comparative::{BenchmarkHarness, BenchmarkResults, ComparativeResults};
pub use retrieval::RetrievalRunner;
pub use scaling::ScalingRunner;
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
pub use topic::TopicRunner;
pub use failfast::{
    FailFastScenario, FailFastTrigger, ExpectedBehavior, FailFastResult,
    FailFastSummary, ErrorMessageQuality,
    build_sequence_tool_scenarios, analyze_error_message_quality, verify_failfast_behavior,
};
