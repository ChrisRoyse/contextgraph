//! Type definitions for the lifecycle manager.
//!
//! TASK-METAUTL-P0-006: Added lambda_override field for meta-learning integration.

use serde::{Deserialize, Serialize};

use super::super::lambda::LifecycleLambdaWeights;
use super::super::stage::LifecycleStage;

/// Manager for lifecycle stage tracking and transitions.
///
/// Tracks the current interaction count and lifecycle stage, providing
/// automatic stage transitions as interactions accumulate. Supports both
/// discrete stage weights and smooth interpolated weights at boundaries.
///
/// # TASK-METAUTL-P0-006: Lambda Override Support
///
/// The manager now supports lambda weight overrides from the meta-learning
/// system. When an override is set, `get_effective_weights()` returns the
/// override instead of the lifecycle-determined weights.
///
/// # Example
///
/// ```
/// use context_graph_utl::config::LifecycleConfig;
/// use context_graph_utl::lifecycle::{LifecycleManager, LifecycleStage};
///
/// let config = LifecycleConfig::default();
/// let mut manager = LifecycleManager::new(&config);
///
/// // Initial state
/// assert_eq!(manager.current_stage(), LifecycleStage::Infancy);
/// assert_eq!(manager.interaction_count(), 0);
///
/// // Simulate interactions
/// for _ in 0..60 {
///     manager.increment();
/// }
///
/// // Stage should have advanced
/// assert_eq!(manager.current_stage(), LifecycleStage::Growth);
/// assert_eq!(manager.interaction_count(), 60);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleManager {
    /// Current interaction count.
    pub(crate) interaction_count: u64,

    /// Current lifecycle stage.
    pub(crate) current_stage: LifecycleStage,

    /// Whether automatic transitions are enabled.
    pub(crate) auto_transition: bool,

    /// Hysteresis buffer to prevent rapid stage switching.
    pub(crate) transition_hysteresis: u64,

    /// Interaction count at last stage transition.
    pub(crate) last_transition_count: u64,

    /// Enable smooth interpolation between stages.
    pub(crate) smooth_transitions: bool,

    /// Smoothing window size for interpolation.
    pub(crate) smoothing_window: u64,

    /// TASK-METAUTL-P0-006: Lambda weight override from meta-learning correction.
    /// When set, `get_effective_weights()` returns this instead of lifecycle weights.
    ///
    /// TASK-METAUTL-P2-001: Now persisted across restarts.
    /// - Uses `skip_serializing_if` to omit None values from JSON (cleaner output)
    /// - Uses `default` for backwards compatibility with existing serialized data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) lambda_override: Option<LifecycleLambdaWeights>,
}
