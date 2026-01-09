//! NORTH Autonomous Logic Layer Services
//!
//! Module 3 of the NORTH system implementing active logic services
//! that operate on the foundation types defined in the parent module.
//!
//! # Architecture
//!
//! The Logic Layer provides operational services that:
//! - Bootstrap and configure the North Star goal
//! - Learn and adapt alignment thresholds
//! - Detect and correct goal drift
//! - Prune and consolidate memories
//! - Evolve the goal hierarchy
//! - Schedule and optimize autonomous workflows
//!
//! # Services
//!
//! - `BootstrapService` (NORTH-008): Initialize North Star from documents
//! - `ThresholdLearner` (NORTH-009): Adaptive threshold optimization
//! - `DriftDetector` (NORTH-010): Goal alignment drift detection
//! - `DriftCorrector` (NORTH-011): Drift correction strategies
//! - `PruningService` (NORTH-012): Memory pruning operations
//! - `ConsolidationService` (NORTH-013): Memory consolidation
//! - `GapDetectionService` (NORTH-014): Goal coverage gap analysis
//! - `SubGoalDiscovery` (NORTH-015): Sub-goal emergence detection
//! - `WeightAdjuster` (NORTH-016): Section weight optimization
//! - `ObsolescenceDetector` (NORTH-017): Goal obsolescence detection
//! - `DailyScheduler` (NORTH-018): Autonomous task scheduling
//! - `EventOptimizer` (NORTH-019): Event-driven optimization
//! - `SelfHealingManager` (NORTH-020): System health management

// NORTH Logic Layer Services (008-020)
pub mod bootstrap_service;
pub mod consolidation_service;
pub mod daily_scheduler;
pub mod drift_corrector;
pub mod drift_detector;
pub mod event_optimizer;
pub mod gap_detection;
pub mod obsolescence_detector;
pub mod pruning_service;
pub mod self_healing_manager;
pub mod subgoal_discovery;
pub mod threshold_learner;
pub mod weight_adjuster;

// Explicit re-exports - only service primary types to avoid ambiguous glob re-exports
// NORTH-008: BootstrapService
pub use bootstrap_service::{
    BootstrapResult, BootstrapService, BootstrapServiceConfig, GoalCandidate,
};

// NORTH-009: ThresholdLearner
pub use threshold_learner::{
    BayesianObservation, EmbedderLearningState, ThompsonState, ThresholdLearner,
    ThresholdLearnerConfig,
};

// NORTH-010: DriftDetector
pub use drift_detector::{DetectorDataPoint, DetectorState, DriftDetector, DriftRecommendation};

// NORTH-011: DriftCorrector
pub use drift_corrector::{
    CorrectionResult, CorrectionStrategy, DriftCorrector, DriftCorrectorConfig,
};

// NORTH-012: PruningService
pub use pruning_service::{
    ExtendedPruningConfig, MemoryMetadata, PruneReason, PruningReport, PruningService,
};

// NORTH-013: ConsolidationService
pub use consolidation_service::{
    ConsolidationService, MemoryContent, MemoryPair, ServiceConsolidationCandidate,
};

// NORTH-014: GapDetectionService
pub use gap_detection::{
    GapDetectionConfig, GapDetectionService, GapReport, GapType, GoalWithMetrics,
};

// NORTH-015: SubGoalDiscovery
pub use subgoal_discovery::{DiscoveryConfig, DiscoveryResult, MemoryCluster, SubGoalDiscovery};

// NORTH-016: WeightAdjuster
pub use weight_adjuster::{AdjustmentReport, WeightAdjuster, WeightAdjusterConfig};

// NORTH-017: ObsolescenceDetector
pub use obsolescence_detector::ObsolescenceDetector;

// NORTH-018: DailyScheduler
pub use daily_scheduler::{DailyScheduler, ScheduleResult, ScheduledTask, SchedulerCheckType};

// NORTH-019: EventOptimizer
pub use event_optimizer::{
    EventOptimizer, EventOptimizerConfig, OptimizationAction, OptimizationEventRecord,
    OptimizationPlan, OptimizationTrigger, SystemMetrics,
};

// NORTH-020: SelfHealingManager
pub use self_healing_manager::{
    HealingAction, HealingResult, HealthIssue, IssueSeverity, SelfHealingConfig, SelfHealingManager,
};
