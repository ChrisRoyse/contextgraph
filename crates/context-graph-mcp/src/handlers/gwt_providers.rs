//! GWT Provider Wrapper Implementations
//!
//! TASK-GWT-001: Wrappers that connect GWT provider traits to real implementations.
//!
//! These wrappers implement the provider traits by delegating to actual GWT components
//! from context-graph-core and context-graph-utl. NO STUBS - uses REAL implementations.
//!
//! ## Architecture
//!
//! Each wrapper holds the real component and implements the trait by forwarding calls:
//! - KuramotoProviderImpl -> KuramotoNetwork (from context-graph-utl)
//! - GwtSystemProviderImpl -> ConsciousnessCalculator + StateMachineManager (from context-graph-core)
//! - WorkspaceProviderImpl -> GlobalWorkspace (from context-graph-core)
//! - MetaCognitiveProviderImpl -> MetaCognitiveLoop (from context-graph-core)
//! - SelfEgoProviderImpl -> SelfEgoNode + IdentityContinuity (from context-graph-core)

use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use async_trait::async_trait;
use context_graph_core::error::CoreResult;
use context_graph_core::gwt::{
    ego_node::{CrisisDetectionResult, IdentityContinuity, IdentityContinuityMonitor, IdentityStatus},
    ConsciousnessCalculator, ConsciousnessMetrics, ConsciousnessState, GlobalWorkspace,
    MetaCognitiveLoop, MetaCognitiveState, SelfEgoNode, StateMachineManager, StateTransition,
};
use context_graph_utl::phase::KuramotoNetwork;
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;

use super::gwt_traits::{
    GwtSystemProvider, KuramotoProvider, MetaCognitiveProvider, SelfEgoProvider, WorkspaceProvider,
    NUM_OSCILLATORS,
};

// ============================================================================
// KuramotoProviderImpl - Wraps real KuramotoNetwork
// ============================================================================

/// Wrapper implementing KuramotoProvider using real KuramotoNetwork
#[derive(Debug)]
pub struct KuramotoProviderImpl {
    network: KuramotoNetwork,
}

impl KuramotoProviderImpl {
    /// Create a new KuramotoProvider wrapping a fresh KuramotoNetwork
    pub fn new() -> Self {
        Self {
            network: KuramotoNetwork::new(),
        }
    }

    /// Create from an existing KuramotoNetwork instance
    #[allow(dead_code)]
    pub fn with_network(network: KuramotoNetwork) -> Self {
        Self { network }
    }

    /// Create a synchronized (r ≈ 1) network
    #[allow(dead_code)]
    pub fn synchronized() -> Self {
        Self {
            network: KuramotoNetwork::synchronized(),
        }
    }

    /// Create an incoherent (r ≈ 0) network
    #[allow(dead_code)]
    pub fn incoherent() -> Self {
        Self {
            network: KuramotoNetwork::incoherent(),
        }
    }
}

impl Default for KuramotoProviderImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl KuramotoProvider for KuramotoProviderImpl {
    fn order_parameter(&self) -> (f64, f64) {
        self.network.order_parameter()
    }

    fn synchronization(&self) -> f64 {
        self.network.synchronization()
    }

    fn is_conscious(&self) -> bool {
        self.network.is_conscious()
    }

    fn is_fragmented(&self) -> bool {
        self.network.is_fragmented()
    }

    fn is_hypersync(&self) -> bool {
        self.network.is_hypersync()
    }

    fn phases(&self) -> [f64; NUM_OSCILLATORS] {
        *self.network.phases()
    }

    fn natural_frequencies(&self) -> [f64; NUM_OSCILLATORS] {
        self.network.natural_frequencies()
    }

    fn coupling_strength(&self) -> f64 {
        self.network.coupling_strength()
    }

    fn step(&mut self, elapsed: Duration) {
        self.network.step(elapsed);
    }

    fn set_coupling_strength(&mut self, k: f64) {
        self.network.set_coupling_strength(k);
    }

    fn reset(&mut self) {
        self.network.reset();
    }

    fn reset_synchronized(&mut self) {
        self.network.reset_synchronized();
    }

    fn elapsed_total(&self) -> Duration {
        self.network.elapsed_total()
    }
}

// ============================================================================
// GwtSystemProviderImpl - Wraps ConsciousnessCalculator + StateMachineManager
// ============================================================================

/// Wrapper implementing GwtSystemProvider using real GWT components
///
/// TASK-IDENTITY-P0-007: Added identity_monitor for identity continuity exposure.
///
/// # Monitor Sharing (TASK-IDENTITY-P0-001)
///
/// The `identity_monitor` field should reference the SAME monitor instance used by
/// `IdentityContinuityListener` to ensure MCP tools read accurate IC values.
/// Use `with_shared_monitor()` for production, `new()` only for isolated tests.
#[derive(Debug)]
pub struct GwtSystemProviderImpl {
    calculator: ConsciousnessCalculator,
    state_machine: RwLock<StateMachineManager>,
    /// SHARED identity continuity monitor - references listener's monitor (TASK-IDENTITY-P0-001)
    identity_monitor: Arc<TokioRwLock<IdentityContinuityMonitor>>,
}

impl GwtSystemProviderImpl {
    /// Create with shared monitor reference (PREFERRED for production)
    ///
    /// Use this constructor when integrating with IdentityContinuityListener
    /// to ensure MCP tools read from the same monitor that receives events.
    ///
    /// # Arguments
    /// * `monitor` - Shared monitor from IdentityContinuityListener::monitor()
    ///
    /// # TASK-IDENTITY-P0-001
    #[allow(dead_code)]
    pub fn with_shared_monitor(
        monitor: Arc<TokioRwLock<IdentityContinuityMonitor>>
    ) -> Self {
        Self {
            calculator: ConsciousnessCalculator::new(),
            state_machine: RwLock::new(StateMachineManager::new()),
            identity_monitor: monitor,
        }
    }

    /// Create with isolated monitor (for testing only)
    ///
    /// WARNING: This creates a separate monitor instance that will NOT
    /// receive updates from IdentityContinuityListener. Use only for
    /// unit tests that don't require listener integration.
    ///
    /// # TASK-IDENTITY-P0-001: FORBIDDEN in production
    /// This method is only available in test builds. Production code MUST use
    /// `with_shared_monitor()` to share the IdentityContinuityListener's monitor.
    /// This ensures AP-40 compliance: MCP tools read from the correct monitor.
    #[cfg(test)]
    pub fn new() -> Self {
        tracing::warn!(
            "GwtSystemProviderImpl::new() creates isolated monitor - \
             use with_shared_monitor() for production"
        );
        Self {
            calculator: ConsciousnessCalculator::new(),
            state_machine: RwLock::new(StateMachineManager::new()),
            identity_monitor: Arc::new(TokioRwLock::new(IdentityContinuityMonitor::new())),
        }
    }
}

#[cfg(test)]
impl Default for GwtSystemProviderImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GwtSystemProvider for GwtSystemProviderImpl {
    fn compute_consciousness(
        &self,
        kuramoto_r: f32,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> CoreResult<f32> {
        self.calculator
            .compute_consciousness(kuramoto_r, meta_accuracy, purpose_vector)
    }

    fn compute_metrics(
        &self,
        kuramoto_r: f32,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> CoreResult<ConsciousnessMetrics> {
        self.calculator
            .compute_metrics(kuramoto_r, meta_accuracy, purpose_vector)
    }

    fn current_state(&self) -> ConsciousnessState {
        self.state_machine
            .read()
            .expect("GwtSystemProviderImpl: state_machine lock poisoned - FATAL ERROR")
            .current_state()
    }

    fn is_conscious(&self) -> bool {
        self.state_machine
            .read()
            .expect("GwtSystemProviderImpl: state_machine lock poisoned - FATAL ERROR")
            .is_conscious()
    }

    fn last_transition(&self) -> Option<StateTransition> {
        self.state_machine
            .read()
            .expect("GwtSystemProviderImpl: state_machine lock poisoned - FATAL ERROR")
            .last_transition()
            .cloned()
    }

    fn time_in_state(&self) -> Duration {
        let chrono_duration = self
            .state_machine
            .read()
            .expect("GwtSystemProviderImpl: state_machine lock poisoned - FATAL ERROR")
            .time_in_state();

        // Convert chrono::Duration to std::time::Duration
        Duration::from_millis(chrono_duration.num_milliseconds().max(0) as u64)
    }

    // === TASK-IDENTITY-P0-007: Identity Continuity Methods ===

    async fn identity_coherence(&self) -> f32 {
        self.identity_monitor
            .read()
            .await
            .identity_coherence()
            .unwrap_or(0.0)
    }

    async fn identity_status(&self) -> IdentityStatus {
        self.identity_monitor
            .read()
            .await
            .current_status()
            .unwrap_or(IdentityStatus::Critical)
    }

    async fn is_identity_crisis(&self) -> bool {
        self.identity_monitor.read().await.is_in_crisis()
    }

    async fn identity_history_len(&self) -> usize {
        self.identity_monitor.read().await.history_len()
    }

    async fn last_detection(&self) -> Option<CrisisDetectionResult> {
        self.identity_monitor.read().await.last_detection()
    }
}

// ============================================================================
// WorkspaceProviderImpl - Wraps GlobalWorkspace
// ============================================================================

/// Wrapper implementing WorkspaceProvider using real GlobalWorkspace
#[derive(Debug)]
pub struct WorkspaceProviderImpl {
    workspace: TokioRwLock<GlobalWorkspace>,
}

impl WorkspaceProviderImpl {
    /// Create a new WorkspaceProvider with fresh GlobalWorkspace
    pub fn new() -> Self {
        Self {
            workspace: TokioRwLock::new(GlobalWorkspace::new()),
        }
    }

    /// Create from an existing GlobalWorkspace
    #[allow(dead_code)]
    pub fn with_workspace(workspace: GlobalWorkspace) -> Self {
        Self {
            workspace: TokioRwLock::new(workspace),
        }
    }
}

impl Default for WorkspaceProviderImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkspaceProvider for WorkspaceProviderImpl {
    async fn select_winning_memory(
        &self,
        candidates: Vec<(Uuid, f32, f32, f32)>,
    ) -> CoreResult<Option<Uuid>> {
        let mut workspace = self.workspace.write().await;
        workspace.select_winning_memory(candidates).await
    }

    fn get_active_memory(&self) -> Option<Uuid> {
        // Use blocking read for sync accessor
        // This is acceptable because GlobalWorkspace doesn't have long-running operations
        let workspace = futures::executor::block_on(self.workspace.read());
        workspace.get_active_memory()
    }

    fn is_broadcasting(&self) -> bool {
        let workspace = futures::executor::block_on(self.workspace.read());
        workspace.is_broadcasting()
    }

    fn has_conflict(&self) -> bool {
        let workspace = futures::executor::block_on(self.workspace.read());
        workspace.has_conflict()
    }

    fn get_conflict_details(&self) -> Option<Vec<Uuid>> {
        let workspace = futures::executor::block_on(self.workspace.read());
        workspace.get_conflict_details()
    }

    fn coherence_threshold(&self) -> f32 {
        let workspace = futures::executor::block_on(self.workspace.read());
        workspace.coherence_threshold
    }
}

// ============================================================================
// MetaCognitiveProviderImpl - Wraps MetaCognitiveLoop
// ============================================================================

/// Wrapper implementing MetaCognitiveProvider using real MetaCognitiveLoop
#[derive(Debug)]
pub struct MetaCognitiveProviderImpl {
    meta_cognitive: TokioRwLock<MetaCognitiveLoop>,
}

impl MetaCognitiveProviderImpl {
    /// Create a new MetaCognitiveProvider with fresh loop
    pub fn new() -> Self {
        Self {
            meta_cognitive: TokioRwLock::new(MetaCognitiveLoop::new()),
        }
    }

    /// Create from existing MetaCognitiveLoop
    #[allow(dead_code)]
    pub fn with_loop(meta_cognitive: MetaCognitiveLoop) -> Self {
        Self {
            meta_cognitive: TokioRwLock::new(meta_cognitive),
        }
    }
}

impl Default for MetaCognitiveProviderImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MetaCognitiveProvider for MetaCognitiveProviderImpl {
    async fn evaluate(
        &self,
        predicted_learning: f32,
        actual_learning: f32,
    ) -> CoreResult<MetaCognitiveState> {
        let mut meta_cognitive = self.meta_cognitive.write().await;
        meta_cognitive
            .evaluate(predicted_learning, actual_learning)
            .await
    }

    fn acetylcholine(&self) -> f32 {
        let meta_cognitive = futures::executor::block_on(self.meta_cognitive.read());
        meta_cognitive.acetylcholine()
    }

    fn monitoring_frequency(&self) -> f32 {
        let meta_cognitive = futures::executor::block_on(self.meta_cognitive.read());
        meta_cognitive.monitoring_frequency()
    }

    fn get_recent_scores(&self) -> Vec<f32> {
        let meta_cognitive = futures::executor::block_on(self.meta_cognitive.read());
        meta_cognitive.get_recent_scores()
    }
}

// ============================================================================
// SelfEgoProviderImpl - Wraps SelfEgoNode + IdentityContinuity
// ============================================================================

/// Wrapper implementing SelfEgoProvider using real SelfEgoNode
#[derive(Debug)]
pub struct SelfEgoProviderImpl {
    ego_node: SelfEgoNode,
    identity_continuity: IdentityContinuity,
}

impl SelfEgoProviderImpl {
    /// Create a new SelfEgoProvider with fresh node
    pub fn new() -> Self {
        Self {
            ego_node: SelfEgoNode::new(),
            identity_continuity: IdentityContinuity::default_initial(),
        }
    }

    /// Create with specific purpose vector
    #[allow(dead_code)]
    pub fn with_purpose_vector(purpose_vector: [f32; 13]) -> Self {
        Self {
            ego_node: SelfEgoNode::with_purpose_vector(purpose_vector),
            identity_continuity: IdentityContinuity::default_initial(),
        }
    }

    /// Update identity continuity metrics
    ///
    /// Call this after purpose vector changes to update identity coherence
    #[allow(dead_code)]
    pub fn update_continuity(
        &mut self,
        pv_cosine: f32,
        kuramoto_r: f32,
    ) -> CoreResult<IdentityStatus> {
        self.identity_continuity.update(pv_cosine, kuramoto_r)
    }

    /// Get mutable access to ego node for updates
    #[allow(dead_code)]
    pub fn ego_node_mut(&mut self) -> &mut SelfEgoNode {
        &mut self.ego_node
    }

    /// Get access to identity continuity for updates
    #[allow(dead_code)]
    pub fn identity_continuity_mut(&mut self) -> &mut IdentityContinuity {
        &mut self.identity_continuity
    }
}

impl Default for SelfEgoProviderImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfEgoProvider for SelfEgoProviderImpl {
    fn purpose_vector(&self) -> [f32; 13] {
        self.ego_node.purpose_vector
    }

    fn coherence_with_actions(&self) -> f32 {
        self.ego_node.coherence_with_actions
    }

    fn trajectory_length(&self) -> usize {
        self.ego_node.identity_trajectory.len()
    }

    fn identity_status(&self) -> IdentityStatus {
        self.identity_continuity.status
    }

    fn identity_coherence(&self) -> f32 {
        self.identity_continuity.identity_coherence
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kuramoto_provider_real_data() {
        let provider = KuramotoProviderImpl::new();

        // Verify real data is returned
        let (r, psi) = provider.order_parameter();
        assert!(
            (0.0..=1.0).contains(&r),
            "Order parameter r out of range: {}",
            r
        );
        assert!(psi >= 0.0, "Mean phase psi should be non-negative: {}", psi);

        let phases = provider.phases();
        assert_eq!(phases.len(), 13, "Should have 13 phases");

        let freqs = provider.natural_frequencies();
        assert_eq!(freqs.len(), 13, "Should have 13 frequencies");
        for freq in freqs {
            assert!(freq > 0.0, "Frequency should be positive: {}", freq);
        }
    }

    #[test]
    fn test_kuramoto_provider_synchronized() {
        let provider = KuramotoProviderImpl::synchronized();

        let r = provider.synchronization();
        assert!(
            r > 0.99,
            "Synchronized network should have r ≈ 1, got {}",
            r
        );
        assert!(
            provider.is_conscious(),
            "Synchronized network should be conscious"
        );
    }

    #[test]
    fn test_kuramoto_provider_incoherent() {
        let provider = KuramotoProviderImpl::incoherent();

        let r = provider.synchronization();
        assert!(r < 0.1, "Incoherent network should have r ≈ 0, got {}", r);
        assert!(
            provider.is_fragmented(),
            "Incoherent network should be fragmented"
        );
    }

    #[test]
    fn test_kuramoto_provider_step_updates_network() {
        let mut provider = KuramotoProviderImpl::new();
        provider.set_coupling_strength(5.0);

        let initial_r = provider.synchronization();

        // Step the network forward
        for _ in 0..100 {
            provider.step(Duration::from_millis(10));
        }

        let final_r = provider.synchronization();
        // With high coupling, synchronization should increase
        assert!(
            final_r > initial_r,
            "High coupling should increase sync: {} vs {}",
            final_r,
            initial_r
        );
    }

    #[test]
    fn test_gwt_system_provider_real_computation() {
        let provider = GwtSystemProviderImpl::new();

        let purpose_vector = [1.0; 13];
        let consciousness = provider
            .compute_consciousness(0.85, 0.9, &purpose_vector)
            .expect("Consciousness computation failed");

        assert!(
            consciousness > 0.0 && consciousness <= 1.0,
            "Consciousness should be in (0,1]: {}",
            consciousness
        );

        // High inputs should yield reasonable consciousness
        assert!(
            consciousness > 0.4,
            "High inputs should yield consciousness > 0.4: {}",
            consciousness
        );
    }

    #[test]
    fn test_gwt_system_provider_metrics() {
        let provider = GwtSystemProviderImpl::new();

        let purpose_vector = [1.0; 13];
        let metrics = provider
            .compute_metrics(0.85, 0.9, &purpose_vector)
            .expect("Metrics computation failed");

        assert!(metrics.integration > 0.0, "Integration should be positive");
        assert!(metrics.reflection > 0.0, "Reflection should be positive");
        assert!(
            metrics.differentiation > 0.0,
            "Differentiation should be positive"
        );
        assert!(
            metrics.consciousness > 0.0,
            "Consciousness should be positive"
        );
    }

    #[test]
    fn test_gwt_system_provider_initial_state() {
        let provider = GwtSystemProviderImpl::new();

        // Initial state should be Dormant
        assert_eq!(provider.current_state(), ConsciousnessState::Dormant);
        assert!(!provider.is_conscious());
        assert!(provider.last_transition().is_none());
    }

    #[tokio::test]
    async fn test_workspace_provider_selection() {
        let provider = WorkspaceProviderImpl::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let candidates = vec![
            (id1, 0.85, 0.9, 0.88),  // score ≈ 0.67
            (id2, 0.88, 0.95, 0.92), // score ≈ 0.77 (winner)
        ];

        let winner = provider
            .select_winning_memory(candidates)
            .await
            .expect("Selection failed");

        assert_eq!(winner, Some(id2), "Should select highest score candidate");
    }

    #[tokio::test]
    async fn test_workspace_provider_threshold_filtering() {
        let provider = WorkspaceProviderImpl::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Both below coherence threshold (0.8)
        let candidates = vec![(id1, 0.5, 0.9, 0.88), (id2, 0.6, 0.95, 0.92)];

        let winner = provider
            .select_winning_memory(candidates)
            .await
            .expect("Selection failed");

        assert_eq!(winner, None, "Should return None when all below threshold");
    }

    #[test]
    fn test_workspace_provider_coherence_threshold() {
        let provider = WorkspaceProviderImpl::new();

        let threshold = provider.coherence_threshold();
        assert!(
            (threshold - 0.8).abs() < 0.01,
            "Threshold should be 0.8: {}",
            threshold
        );
    }

    #[tokio::test]
    async fn test_meta_cognitive_provider_evaluation() {
        let provider = MetaCognitiveProviderImpl::new();

        let state = provider
            .evaluate(0.8, 0.8)
            .await
            .expect("Evaluation failed");

        // Perfect prediction should have meta_score around 0.5 (σ(0))
        assert!(
            state.meta_score >= 0.4 && state.meta_score <= 0.6,
            "Perfect prediction should give meta_score ≈ 0.5: {}",
            state.meta_score
        );
        assert!(!state.dream_triggered);
    }

    #[test]
    fn test_meta_cognitive_provider_initial_state() {
        let provider = MetaCognitiveProviderImpl::new();

        // Default acetylcholine is 0.001
        let ach = provider.acetylcholine();
        assert!(
            (ach - 0.001).abs() < 0.0001,
            "Initial ACh should be 0.001: {}",
            ach
        );

        // Default monitoring frequency is 1.0 Hz
        let freq = provider.monitoring_frequency();
        assert!(
            (freq - 1.0).abs() < 0.01,
            "Initial freq should be 1.0: {}",
            freq
        );

        // No recent scores initially
        let scores = provider.get_recent_scores();
        assert!(scores.is_empty(), "Should have no scores initially");
    }

    #[test]
    fn test_self_ego_provider_purpose_vector() {
        let purpose = [0.5; 13];
        let provider = SelfEgoProviderImpl::with_purpose_vector(purpose);

        assert_eq!(provider.purpose_vector(), purpose);
    }

    #[test]
    fn test_self_ego_provider_initial_state() {
        let provider = SelfEgoProviderImpl::new();

        assert_eq!(provider.purpose_vector(), [0.0; 13]);
        assert_eq!(provider.coherence_with_actions(), 0.0);
        assert_eq!(provider.trajectory_length(), 0);
        // Issue 2 fix: coherence=0.0 should result in Critical, not Healthy
        assert_eq!(provider.identity_status(), IdentityStatus::Critical);
        assert_eq!(provider.identity_coherence(), 0.0);
    }

    #[test]
    fn test_self_ego_provider_continuity_update() {
        let mut provider = SelfEgoProviderImpl::new();

        // Update with high values - should be Healthy
        let status = provider
            .update_continuity(0.95, 0.95)
            .expect("Update failed");
        assert_eq!(status, IdentityStatus::Healthy);
        assert!(provider.identity_coherence() > 0.9);

        // Update with low values - should be Critical
        let status = provider.update_continuity(0.3, 0.3).expect("Update failed");
        assert_eq!(status, IdentityStatus::Critical);
        assert!(provider.identity_coherence() < 0.5);
    }
}
