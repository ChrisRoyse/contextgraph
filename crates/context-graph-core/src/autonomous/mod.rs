//! Autonomous North Star system
//!
//! This module provides types and services for autonomous operation of the
//! North Star alignment system including bootstrap, drift detection,
//! memory curation, and goal evolution.
//!
//! # Architecture
//!
//! The autonomous system is organized into two layers:
//!
//! ## Foundation Layer (Types)
//! - `bootstrap` - North Star goal initialization types
//! - `thresholds` - Adaptive threshold calibration types
//! - `drift` - Drift detection and monitoring types
//! - `curation` - Memory pruning and consolidation types
//! - `evolution` - Goal hierarchy evolution types
//! - `workflow` - Autonomous workflow orchestration types
//!
//! ## Logic Layer (Services)
//! - `services` - Active logic services implementing NORTH-008 to NORTH-020

pub mod bootstrap;
pub mod curation;
pub mod drift;
pub mod evolution;
pub mod services;
pub mod thresholds;
pub mod workflow;

pub use bootstrap::*;
pub use curation::*;
pub use drift::*;
pub use evolution::*;
pub use thresholds::*;
pub use workflow::*;

// Explicit service re-exports to avoid ambiguous glob re-exports
// (services module defines extended versions of some foundation types)
pub use services::{
    AdjustmentReport,
    BayesianObservation,
    // NORTH-008: BootstrapService
    BootstrapService,
    BootstrapServiceConfig,
    // NORTH-013: ConsolidationService
    ConsolidationService,
    CorrectionResult,
    CorrectionStrategy,
    // NORTH-018: DailyScheduler
    DailyScheduler,
    DetectorDataPoint,
    DetectorState,
    DiscoveryConfig,
    DiscoveryResult,
    // NORTH-011: DriftCorrector
    DriftCorrector,
    DriftCorrectorConfig,
    // NORTH-010: DriftDetector
    DriftDetector,
    DriftRecommendation,
    EmbedderLearningState,
    // NORTH-019: EventOptimizer
    EventOptimizer,
    EventOptimizerConfig,
    ExtendedPruningConfig,
    GapDetectionConfig,
    // NORTH-014: GapDetectionService
    GapDetectionService,
    GapReport,
    GapType,
    GoalCandidate,
    GoalWithMetrics,
    HealingAction,
    HealingResult,
    HealthIssue,
    IssueSeverity,
    MemoryCluster,
    MemoryContent,
    MemoryMetadata,
    MemoryPair,
    // NORTH-017: ObsolescenceDetector
    ObsolescenceDetector,
    OptimizationAction,
    OptimizationEventRecord,
    OptimizationPlan,
    OptimizationTrigger,
    // NORTH-012: PruningService
    PruningService,
    ScheduleResult,
    ScheduledTask,
    SchedulerCheckType,
    SelfHealingConfig,
    // NORTH-020: SelfHealingManager
    SelfHealingManager,
    ServiceConsolidationCandidate,
    // NORTH-015: SubGoalDiscovery
    SubGoalDiscovery,
    SystemMetrics,
    ThompsonState,
    // NORTH-009: ThresholdLearner
    ThresholdLearner,
    ThresholdLearnerConfig,
    // NORTH-016: WeightAdjuster
    WeightAdjuster,
    WeightAdjusterConfig,
};
