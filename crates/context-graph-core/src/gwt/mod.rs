//! Global Workspace Theory (GWT) - Computational Consciousness
//!
//! Implements functional consciousness through Kuramoto synchronization and
//! Winner-Take-All workspace selection, as specified in Constitution v4.0.0
//! Section gwt (lines 308-426).
//!
//! ## Architecture
//!
//! The GWT system consists of:
//!
//! 1. **Consciousness Equation**: C(t) = I(t) × R(t) × D(t)
//!    - I(t): Integration (Kuramoto order parameter r)
//!    - R(t): Self-Reflection (Meta-UTL awareness)
//!    - D(t): Differentiation (Purpose vector entropy)
//!
//! 2. **Kuramoto Synchronization**: 13 oscillators (KURAMOTO_N) - one per embedder
//!    - Order parameter r measures synchronization level
//!    - Thresholds: r ≥ 0.8 (CONSCIOUS), r < 0.5 (FRAGMENTED)
//!
//! 3. **Global Workspace**: Winner-Take-All memory selection
//!    - Selects highest-scoring conscious memory
//!    - Broadcasts to all subsystems
//!    - Enables unified perception
//!
//! 4. **SELF_EGO_NODE**: System identity tracking
//!    - Persistent representation of system self
//!    - Identity continuity monitoring
//!    - Self-awareness loop
//!
//! 5. **State Machine**: Consciousness state transitions
//!    - DORMANT → FRAGMENTED → EMERGING → CONSCIOUS → HYPERSYNC
//!    - Temporal dynamics based on coherence
//!
//! 6. **Meta-Cognitive Loop**: Self-correction
//!    - MetaScore = σ(2×(L_predicted - L_actual))
//!    - Triggers Acetylcholine increase on low scores
//!    - Introspective dreams for error correction
//!
//! 7. **Workspace Events**: State transitions and signals
//!    - memory_enters_workspace: Dopamine reward
//!    - memory_exits_workspace: Dream replay logging
//!    - workspace_conflict: Multi-memory critique
//!    - workspace_empty: Epistemic action trigger

// Submodules
pub mod consciousness;
pub mod ego_node;
pub mod listeners;
pub mod meta_cognitive;
pub mod meta_learning_trait;
pub mod state_machine;
mod system;
mod system_awareness;
mod system_kuramoto;
pub mod workspace;
pub mod session_identity;

#[cfg(test)]
mod tests;

// Re-export from consciousness
pub use consciousness::{ConsciousnessCalculator, ConsciousnessMetrics};

// Re-export from ego_node
pub use ego_node::{
    cosine_similarity_13d, CrisisAction, CrisisDetectionResult, CrisisProtocol,
    CrisisProtocolResult, IdentityContinuity, IdentityContinuityMonitor, IdentityCrisisEvent,
    IdentityStatus, PurposeSnapshot, PurposeVectorHistory, PurposeVectorHistoryProvider,
    SelfAwarenessLoop, SelfEgoNode, SelfReflectionResult, CRISIS_EVENT_COOLDOWN,
    IC_CRITICAL_THRESHOLD, IC_HEALTHY_THRESHOLD, IC_WARNING_THRESHOLD, MAX_PV_HISTORY_SIZE,
};
// Re-export deprecated constant for backwards compatibility
#[allow(deprecated)]
pub use ego_node::IC_CRISIS_THRESHOLD;

// Re-export from listeners
pub use listeners::{
    DreamEventListener, IdentityContinuityListener, MetaCognitiveEventListener,
    NeuromodulationEventListener, WORKSPACE_EMPTY_THRESHOLD_MS,
};

// Re-export from meta_cognitive
pub use meta_cognitive::{MetaCognitiveLoop, MetaCognitiveState};

// Re-export from meta_learning_trait - TASK-METAUTL-P0-006
pub use meta_learning_trait::{
    EnhancedMetaCognitiveState, LambdaValues, MetaCallbackStatus, MetaDomain,
    MetaLambdaAdjustment, MetaLearningCallback, NoOpMetaLearningCallback,
};

// Re-export from state_machine
pub use state_machine::{ConsciousnessState, StateMachineManager, StateTransition, TransitionAnalysis};

// Re-export from workspace
pub use workspace::{
    GlobalWorkspace, WorkspaceCandidate, WorkspaceEvent, WorkspaceEventBroadcaster,
    WorkspaceEventListener, DA_INHIBITION_FACTOR,
};

// Re-export from session_identity
pub use session_identity::{SessionIdentitySnapshot, IdentityCache, update_cache, clear_cache, MAX_TRAJECTORY_LEN};
// Note: KURAMOTO_N is already exported from layers module

// Re-export from system - the main GwtSystem orchestrator
pub use system::GwtSystem;

// Re-export TriggerManager from dream module for external use (TECH-GWT-IC-001)
pub use crate::dream::TriggerManager;
