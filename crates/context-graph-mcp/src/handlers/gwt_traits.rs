//! GWT Provider Traits for MCP Handler Integration
//!
//! TASK-GWT-001: Provider traits that wrap GWT components for handler injection.
//! All traits require Send + Sync for thread-safe Arc wrapping.
//!
//! These traits define the interface for GWT/Kuramoto components to be wired
//! into the MCP Handlers struct. All implementations must use REAL components -
//! NO STUBS, NO FALLBACKS, FAIL FAST on errors.

use std::time::Duration;

use async_trait::async_trait;
use context_graph_core::error::CoreResult;
use context_graph_core::gwt::{
    ConsciousnessMetrics, ConsciousnessState, MetaCognitiveState, StateTransition,
};
use uuid::Uuid;

/// Number of oscillators (13 embedding spaces)
pub const NUM_OSCILLATORS: usize = 13;

/// Provider trait for Kuramoto oscillator network operations.
///
/// Wraps KuramotoNetwork from context-graph-utl for handler access.
/// All methods are synchronous as KuramotoNetwork operates in-memory.
///
/// TASK-GWT-001: Required for gwt/* handlers and consciousness computation.
pub trait KuramotoProvider: Send + Sync {
    /// Get the order parameter (r, psi) measuring synchronization.
    /// r in [0,1]: synchronization level
    /// psi in [0, 2pi]: mean phase
    fn order_parameter(&self) -> (f64, f64);

    /// Get synchronization level (r only) for quick checks.
    fn synchronization(&self) -> f64;

    /// Check if network is in CONSCIOUS state (r >= 0.8).
    #[allow(dead_code)]
    fn is_conscious(&self) -> bool;

    /// Check if network is FRAGMENTED (r < 0.5).
    #[allow(dead_code)]
    fn is_fragmented(&self) -> bool;

    /// Check if network is HYPERSYNC (r > 0.95) - warning state.
    #[allow(dead_code)]
    fn is_hypersync(&self) -> bool;

    /// Get all 13 oscillator phases.
    fn phases(&self) -> [f64; NUM_OSCILLATORS];

    /// Get all 13 natural frequencies (Hz).
    fn natural_frequencies(&self) -> [f64; NUM_OSCILLATORS];

    /// Get coupling strength K.
    fn coupling_strength(&self) -> f64;

    /// Step the network forward by elapsed duration.
    #[allow(dead_code)]
    fn step(&mut self, elapsed: Duration);

    /// Set coupling strength K (clamped to [0, 10]).
    fn set_coupling_strength(&mut self, k: f64);

    /// Reset network to initial incoherent state.
    #[allow(dead_code)]
    fn reset(&mut self);

    /// Reset network to synchronized state (all phases = 0).
    #[allow(dead_code)]
    fn reset_synchronized(&mut self);

    /// Get elapsed time since creation/reset.
    fn elapsed_total(&self) -> Duration;
}

/// Provider trait for GWT consciousness computation.
///
/// Wraps ConsciousnessCalculator from context-graph-core for handler access.
/// TASK-GWT-001: Required for consciousness computation C(t) = I(t) x R(t) x D(t).
pub trait GwtSystemProvider: Send + Sync {
    /// Compute consciousness level: C(t) = I(t) x R(t) x D(t)
    ///
    /// # Arguments
    /// - kuramoto_r: Integration factor from Kuramoto order parameter
    /// - meta_accuracy: Reflection factor from Meta-UTL accuracy
    /// - purpose_vector: 13D purpose vector for differentiation
    ///
    /// # Returns
    /// Consciousness level in [0, 1]
    #[allow(dead_code)]
    fn compute_consciousness(
        &self,
        kuramoto_r: f32,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> CoreResult<f32>;

    /// Compute full consciousness metrics with component analysis.
    fn compute_metrics(
        &self,
        kuramoto_r: f32,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> CoreResult<ConsciousnessMetrics>;

    /// Get current consciousness state (DORMANT, FRAGMENTED, EMERGING, CONSCIOUS, HYPERSYNC).
    fn current_state(&self) -> ConsciousnessState;

    /// Check if system is in a conscious state (CONSCIOUS or HYPERSYNC).
    #[allow(dead_code)]
    fn is_conscious(&self) -> bool;

    /// Get the last state transition if any.
    #[allow(dead_code)]
    fn last_transition(&self) -> Option<StateTransition>;

    /// Get time spent in current state.
    fn time_in_state(&self) -> Duration;
}

/// Provider trait for workspace selection operations.
///
/// Handles winner-take-all memory selection for global workspace.
/// TASK-GWT-001: Required for workspace broadcast operations.
#[async_trait]
pub trait WorkspaceProvider: Send + Sync {
    /// Select winning memory via winner-take-all algorithm.
    ///
    /// # Arguments
    /// - candidates: Vec of (memory_id, order_parameter_r, importance, alignment)
    ///
    /// # Returns
    /// UUID of winning memory, or None if no candidates pass coherence threshold (0.8)
    async fn select_winning_memory(
        &self,
        candidates: Vec<(Uuid, f32, f32, f32)>,
    ) -> CoreResult<Option<Uuid>>;

    /// Get currently active (conscious) memory if broadcasting.
    fn get_active_memory(&self) -> Option<Uuid>;

    /// Check if broadcast window is still active.
    fn is_broadcasting(&self) -> bool;

    /// Check for workspace conflict (multiple memories with r > 0.8).
    fn has_conflict(&self) -> bool;

    /// Get conflicting memory IDs if present.
    fn get_conflict_details(&self) -> Option<Vec<Uuid>>;

    /// Get coherence threshold for workspace entry.
    fn coherence_threshold(&self) -> f32;
}

/// Provider trait for meta-cognitive loop operations.
/// TASK-GWT-001: Required for meta_score computation and dream triggering.
#[async_trait]
pub trait MetaCognitiveProvider: Send + Sync {
    /// Evaluate meta-cognitive score.
    ///
    /// MetaScore = sigmoid(2 x (L_predicted - L_actual))
    ///
    /// # Arguments
    /// - predicted_learning: L_predicted in [0, 1]
    /// - actual_learning: L_actual in [0, 1]
    #[allow(dead_code)]
    async fn evaluate(
        &self,
        predicted_learning: f32,
        actual_learning: f32,
    ) -> CoreResult<MetaCognitiveState>;

    /// Get current Acetylcholine level (learning rate modulator).
    fn acetylcholine(&self) -> f32;

    /// Get current monitoring frequency (Hz).
    #[allow(dead_code)]
    fn monitoring_frequency(&self) -> f32;

    /// Get recent meta-scores for trend analysis.
    #[allow(dead_code)]
    fn get_recent_scores(&self) -> Vec<f32>;
}

/// Provider trait for self-ego node operations.
/// TASK-GWT-001: Required for identity continuity monitoring.
pub trait SelfEgoProvider: Send + Sync {
    /// Get current purpose vector (13D).
    fn purpose_vector(&self) -> [f32; 13];

    /// Get coherence between current actions and purpose vector.
    fn coherence_with_actions(&self) -> f32;

    /// Get identity trajectory length.
    fn trajectory_length(&self) -> usize;

    /// Get identity status.
    fn identity_status(&self) -> context_graph_core::gwt::ego_node::IdentityStatus;

    /// Get identity coherence value.
    fn identity_coherence(&self) -> f32;
}
