//! Dream Layer - Memory Consolidation System
//!
//! Implements NREM/REM sleep cycles for memory consolidation as specified in
//! Constitution v4.0.0 Section dream (lines 446-453).
//!
//! ## Architecture
//!
//! 1. **NREM Phase** (3 min): Hebbian replay with tight coupling (0.9)
//! 2. **REM Phase** (2 min): Attractor exploration with temp=2.0
//! 3. **Amortized Learning**: Shortcut creation for 3+ hop paths traversed 5+ times
//! 4. **Dream Scheduler**: Triggers when activity < 0.15 for 10 minutes
//!
//! ## Constraints
//!
//! - queries: 100 max during REM
//! - semantic_leap: 0.7 minimum distance for exploration
//! - abort_on_query: true (immediate wake on external query)
//! - wake: <100ms latency
//! - gpu: <30% usage during dream
//!
//! ## Usage
//!
//! ```ignore
//! use context_graph_core::dream::{DreamController, DreamScheduler};
//!
//! let mut controller = DreamController::new();
//! let scheduler = DreamScheduler::new();
//!
//! // Check if we should enter dream state
//! if scheduler.should_trigger_dream() {
//!     let report = controller.start_dream_cycle().await?;
//!     println!("Dream cycle completed: {:?}", report);
//! }
//! ```
//!
//! ## Constitution Compliance
//!
//! | Constraint | Value | Enforcement |
//! |------------|-------|-------------|
//! | NREM duration | 3 min | Timed phase in controller |
//! | REM duration | 2 min | Timed phase in controller |
//! | Query limit | 100 | Counter in REM phase |
//! | Semantic leap | 0.7 | Filter in REM search |
//! | Wake latency | <100ms | Abort controller timeout |
//! | GPU budget | <30% | Resource monitor check |

pub mod amortized;
pub mod controller;
pub mod hebbian;
pub mod hyperbolic_walk;
pub mod mcp_events;
pub mod nrem;
pub mod poincare_walk;
pub mod rem;
pub mod scheduler;
pub mod thresholds;
pub mod triggers;
pub mod types;
pub mod wake_controller;

// Re-exports for convenience
pub use amortized::{
    AmortizedLearner, EdgeCreator, NullEdgeCreator, PathSignature, ShortcutCandidate, ShortcutEdge,
};
pub use controller::{DreamController, DreamCycleConfig, DreamReport, DreamState, DreamStatus};
pub use hebbian::{
    find_coactivated_pairs, select_replay_memories, EdgeUpdate, HebbianEngine, HebbianUpdateResult,
};
pub use hyperbolic_walk::{
    position_to_query, sample_starting_positions, DiscoveredBlindSpot, ExplorationResult,
    HyperbolicExplorer, WalkResult,
};
pub use nrem::{MemoryProvider, NremPhase, NremReport, NullMemoryProvider};
pub use poincare_walk::{
    direction_toward, geodesic_distance, inner_product_64, is_far_from_all, mobius_add, norm_64,
    norm_squared_64, project_to_ball, random_direction, sample_direction_with_temperature,
    scale_direction, softmax_temperature, validate_in_ball, PoincareBallConfig,
};
pub use rem::{RemPhase, RemReport};
pub use scheduler::DreamScheduler;
pub use thresholds::DreamThresholds;
pub use triggers::{
    gpu_thresholds, EntropyCalculator, GpuMonitor, GpuMonitorError, StubGpuMonitor, TriggerConfig,
    TriggerManager,
};

pub use mcp_events::{
    BlindSpotDiscovered, BroadcastError, DreamCycleCompleted, DreamCycleStarted, DreamEvent,
    DreamEventBroadcaster, GpuBudgetWarning, LoggingBroadcaster, NoOpBroadcaster,
    NremPhaseCompleted, RemPhaseCompleted, ShortcutCreated, WakeTriggered,
};
#[cfg(feature = "nvml")]
pub use triggers::NvmlGpuMonitor;
pub use types::{
    ConsolidationCallback, ConsolidationMetrics, DreamPhase, EntropyWindow, ExtendedTriggerReason,
    GpuTriggerState, HebbianConfig, HyperbolicWalkConfig, InvalidMetricsError, NodeActivation,
    WalkStep,
};
pub use wake_controller::{
    ResourceSnapshot, WakeController, WakeError, WakeHandle, WakeState, WakeStats,
};

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Dream layer configuration constants from Constitution v4.0.0
pub mod constants {
    use std::time::Duration;

    /// NREM phase duration (Constitution: 3 minutes)
    pub const NREM_DURATION: Duration = Duration::from_secs(180);

    /// REM phase duration (Constitution: 2 minutes)
    pub const REM_DURATION: Duration = Duration::from_secs(120);

    /// Activity threshold for dream trigger (Constitution: 0.15)
    #[deprecated(since = "0.5.0", note = "Use DreamThresholds.activity instead")]
    pub const ACTIVITY_THRESHOLD: f32 = 0.15;

    /// Idle duration before dream trigger (Constitution: 10 minutes)
    pub const IDLE_DURATION_TRIGGER: Duration = Duration::from_secs(600);

    /// Maximum synthetic queries during REM (Constitution: 100)
    pub const MAX_REM_QUERIES: usize = 100;

    /// Minimum semantic leap for REM exploration (Constitution: 0.7)
    #[deprecated(since = "0.5.0", note = "Use DreamThresholds.semantic_leap instead")]
    pub const MIN_SEMANTIC_LEAP: f32 = 0.7;

    /// Maximum wake latency (Constitution: <100ms)
    /// Set to 99ms to satisfy strict less-than requirement
    pub const MAX_WAKE_LATENCY: Duration = Duration::from_millis(99);

    /// Maximum GPU usage during dream (Constitution: <30%)
    pub const MAX_GPU_USAGE: f32 = 0.30;

    /// NREM coupling strength (Constitution: 0.9)
    pub const NREM_COUPLING: f32 = 0.9;

    /// REM temperature for exploration (Constitution: 2.0)
    pub const REM_TEMPERATURE: f32 = 2.0;

    /// NREM recency bias (Constitution: 0.8)
    pub const NREM_RECENCY_BIAS: f32 = 0.8;

    /// Minimum hops for shortcut creation (Constitution: 3)
    pub const MIN_SHORTCUT_HOPS: usize = 3;

    /// Minimum traversals for shortcut creation (Constitution: 5)
    pub const MIN_SHORTCUT_TRAVERSALS: usize = 5;

    /// Confidence threshold for shortcuts (Constitution: 0.7)
    #[deprecated(
        since = "0.5.0",
        note = "Use DreamThresholds.shortcut_confidence instead"
    )]
    pub const SHORTCUT_CONFIDENCE_THRESHOLD: f32 = 0.7;
}

/// Dream cycle result containing metrics from both phases
#[derive(Debug, Clone)]
pub struct DreamCycleResult {
    /// Whether the cycle completed successfully
    pub completed: bool,

    /// NREM phase report
    pub nrem_report: Option<NremReport>,

    /// REM phase report
    pub rem_report: Option<RemReport>,

    /// Total cycle duration
    pub total_duration: Duration,

    /// Wake reason if aborted
    pub wake_reason: Option<WakeReason>,
}

/// Reasons for waking from dream state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeReason {
    /// External query received (abort_on_query=true)
    ExternalQuery,

    /// GPU usage exceeded budget
    GpuOverBudget,

    /// Cycle completed normally
    CycleComplete,

    /// Manual abort requested
    ManualAbort,

    /// Resource pressure (memory, etc.)
    ResourcePressure,

    /// Error during dream processing
    Error,
}

impl std::fmt::Display for WakeReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WakeReason::ExternalQuery => write!(f, "external_query"),
            WakeReason::GpuOverBudget => write!(f, "gpu_over_budget"),
            WakeReason::CycleComplete => write!(f, "cycle_complete"),
            WakeReason::ManualAbort => write!(f, "manual_abort"),
            WakeReason::ResourcePressure => write!(f, "resource_pressure"),
            WakeReason::Error => write!(f, "error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn test_constants_compliance() {
        // Verify constitution-mandated values
        assert_eq!(constants::NREM_DURATION.as_secs(), 180);
        assert_eq!(constants::REM_DURATION.as_secs(), 120);
        // Legacy constants - deprecated but must retain values for compatibility
        assert_eq!(constants::ACTIVITY_THRESHOLD, 0.15);
        assert_eq!(constants::MAX_REM_QUERIES, 100);
        assert_eq!(constants::MIN_SEMANTIC_LEAP, 0.7);
        assert!(constants::MAX_WAKE_LATENCY.as_millis() < 100);
        assert_eq!(constants::MAX_GPU_USAGE, 0.30);
        assert_eq!(constants::NREM_COUPLING, 0.9);
        assert_eq!(constants::REM_TEMPERATURE, 2.0);
        assert_eq!(constants::MIN_SHORTCUT_HOPS, 3);
        assert_eq!(constants::MIN_SHORTCUT_TRAVERSALS, 5);
        assert_eq!(constants::SHORTCUT_CONFIDENCE_THRESHOLD, 0.7);
    }

    #[test]
    fn test_wake_reason_display() {
        assert_eq!(WakeReason::ExternalQuery.to_string(), "external_query");
        assert_eq!(WakeReason::GpuOverBudget.to_string(), "gpu_over_budget");
        assert_eq!(WakeReason::CycleComplete.to_string(), "cycle_complete");
    }
}
