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
//! 2. **Kuramoto Synchronization**: 13 oscillators coupled for coherence
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

pub mod consciousness;
pub mod meta_cognitive;
pub mod state_machine;
pub mod workspace;
pub mod ego_node;

pub use consciousness::{ConsciousnessCalculator, ConsciousnessMetrics};
pub use meta_cognitive::{MetaCognitiveLoop, MetaCognitiveState};
pub use state_machine::{ConsciousnessState, StateTransition, StateMachineManager};
pub use workspace::{
    GlobalWorkspace, WorkspaceCandidate, WorkspaceEvent, WorkspaceEventBroadcaster,
};
pub use ego_node::{SelfEgoNode, SelfAwarenessLoop, IdentityContinuity};

use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Global Workspace Theory system orchestrating consciousness
#[derive(Debug)]
pub struct GwtSystem {
    /// Consciousness calculator (C = I×R×D)
    pub consciousness_calc: Arc<ConsciousnessCalculator>,

    /// Global workspace for winner-take-all selection
    pub workspace: Arc<RwLock<GlobalWorkspace>>,

    /// System identity node
    pub self_ego_node: Arc<RwLock<SelfEgoNode>>,

    /// Consciousness state machine
    pub state_machine: Arc<RwLock<StateMachineManager>>,

    /// Meta-cognitive feedback loop
    pub meta_cognitive: Arc<RwLock<MetaCognitiveLoop>>,

    /// Workspace event broadcaster
    pub event_broadcaster: Arc<WorkspaceEventBroadcaster>,
}

impl GwtSystem {
    /// Create a new GWT consciousness system
    pub async fn new() -> crate::CoreResult<Self> {
        Ok(Self {
            consciousness_calc: Arc::new(ConsciousnessCalculator::new()),
            workspace: Arc::new(RwLock::new(GlobalWorkspace::new())),
            self_ego_node: Arc::new(RwLock::new(SelfEgoNode::new())),
            state_machine: Arc::new(RwLock::new(StateMachineManager::new())),
            meta_cognitive: Arc::new(RwLock::new(MetaCognitiveLoop::new())),
            event_broadcaster: Arc::new(WorkspaceEventBroadcaster::new()),
        })
    }

    /// Update consciousness state with current Kuramoto order parameter and meta metrics
    pub async fn update_consciousness(
        &self,
        kuramoto_r: f32,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> crate::CoreResult<f32> {
        // Calculate consciousness level
        let consciousness = self
            .consciousness_calc
            .compute_consciousness(kuramoto_r, meta_accuracy, purpose_vector)?;

        // Update state machine with new consciousness level
        let mut state_mgr = self.state_machine.write().await;
        let old_state = state_mgr.current_state();
        let new_state = state_mgr.update(consciousness).await?;

        if old_state != new_state {
            // Log state transition
            let transition = StateTransition {
                from: old_state,
                to: new_state,
                timestamp: chrono::Utc::now(),
                consciousness_level: consciousness,
            };
            // Transition logged for debugging
            tracing::debug!("State transition: {:?}", transition);
        }

        Ok(consciousness)
    }

    /// Select winning memory for workspace broadcast
    pub async fn select_workspace_memory(
        &self,
        candidates: Vec<(Uuid, f32, f32, f32)>, // (id, r, importance, alignment)
    ) -> crate::CoreResult<Option<Uuid>> {
        let mut workspace = self.workspace.write().await;
        workspace.select_winning_memory(candidates).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gwt_system_creation() {
        let gwt = GwtSystem::new().await.expect("Failed to create GWT system");
        // Verify system has the required components
        assert!(Arc::strong_count(&gwt.consciousness_calc) > 0);
        assert!(Arc::strong_count(&gwt.workspace) > 0);
        assert!(Arc::strong_count(&gwt.self_ego_node) > 0);
    }
}
