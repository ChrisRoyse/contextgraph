//! Handlers struct definition and constructors.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! TASK-S003: Added GoalAlignmentCalculator for purpose/goal operations.
//! TASK-S004: Added JohariTransitionManager for johari/* handlers.
//! TASK-S005: Added MetaUtlTracker for meta_utl/* handlers.
//! TASK-GWT-001: Added GWT/Kuramoto provider traits for consciousness operations.

use std::sync::Arc;

use parking_lot::RwLock;

use context_graph_core::alignment::GoalAlignmentCalculator;
use context_graph_core::atc::AdaptiveThresholdCalibration;
use context_graph_core::dream::{AmortizedLearner, DreamController, DreamScheduler};
// TASK-IDENTITY-P0-001: Import GWT identity components for shared monitor wiring
use context_graph_core::gwt::ego_node::SelfEgoNode;
use context_graph_core::gwt::listeners::IdentityContinuityListener;
use context_graph_core::gwt::workspace::WorkspaceEventBroadcaster;
use context_graph_core::johari::{DynDefaultJohariManager, JohariTransitionManager};
use context_graph_core::monitoring::{
    LayerStatusProvider, StubLayerStatusProvider, StubSystemMonitor, SystemMonitor,
};
use context_graph_core::neuromod::NeuromodulationManager;
use context_graph_core::purpose::GoalHierarchy;
use context_graph_core::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor,
};

use super::super::gwt_traits::{
    GwtSystemProvider, KuramotoProvider, MetaCognitiveProvider, SelfEgoProvider, WorkspaceProvider,
};
use super::meta_utl_tracker::MetaUtlTracker;

/// Request handlers for MCP protocol.
///
/// Uses TeleologicalMemoryStore for 13-embedding fingerprint storage
/// and MultiArrayEmbeddingProvider for generating all 13 embeddings.
/// TASK-S003: Added GoalAlignmentCalculator and GoalHierarchy for purpose operations.
/// TASK-S004: Added JohariTransitionManager for johari/* operations.
/// TASK-S005: Added MetaUtlTracker for meta_utl/* operations.
/// TASK-EMB-024: Added SystemMonitor and LayerStatusProvider for real health metrics.
pub struct Handlers {
    /// Teleological memory store - stores TeleologicalFingerprint with 13 embeddings.
    /// NO legacy MemoryStore support.
    pub(in crate::handlers) teleological_store: Arc<dyn TeleologicalMemoryStore>,

    /// UTL processor for computing learning metrics.
    pub(in crate::handlers) utl_processor: Arc<dyn UtlProcessor>,

    /// Multi-array embedding provider - generates all 13 embeddings per content.
    /// NO legacy single-embedding support.
    pub(in crate::handlers) multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,

    /// Goal alignment calculator - computes alignment between fingerprints and goal hierarchy.
    /// TASK-S003: Required for purpose/north_star_alignment and purpose/drift_check.
    /// TASK-INTEG-005: Will be used for cross-goal alignment calculations.
    #[allow(dead_code)]
    pub(in crate::handlers) alignment_calculator: Arc<dyn GoalAlignmentCalculator>,

    /// Goal hierarchy - defines North Star and sub-goals.
    /// TASK-S003: RwLock allows runtime updates via purpose/north_star_update.
    pub(in crate::handlers) goal_hierarchy: Arc<RwLock<GoalHierarchy>>,

    /// Johari transition manager - manages Johari quadrant transitions.
    /// TASK-S004: Required for johari/* handlers.
    pub(in crate::handlers) johari_manager: Arc<dyn JohariTransitionManager>,

    /// Meta-UTL tracker - tracks predictions and per-embedder accuracy.
    /// TASK-S005: Required for meta_utl/* handlers.
    pub(in crate::handlers) meta_utl_tracker: Arc<RwLock<MetaUtlTracker>>,

    /// System monitor for REAL health metrics.
    /// TASK-EMB-024: Required for meta_utl/health_metrics - NO hardcoded values.
    pub(in crate::handlers) system_monitor: Arc<dyn SystemMonitor>,

    /// Layer status provider for REAL layer statuses.
    /// TASK-EMB-024: Required for get_memetic_status and get_graph_manifest - NO hardcoded values.
    pub(in crate::handlers) layer_status_provider: Arc<dyn LayerStatusProvider>,

    // ========== GWT/Kuramoto Fields (TASK-GWT-001) ==========
    /// Kuramoto oscillator network for 13-embedding phase synchronization.
    /// TASK-GWT-001: Required for gwt/* handlers and consciousness computation.
    /// Uses RwLock because step() mutates internal state.
    pub(in crate::handlers) kuramoto_network: Option<Arc<RwLock<dyn KuramotoProvider>>>,

    /// GWT consciousness system provider.
    /// TASK-GWT-001: Required for consciousness computation C(t) = I(t) x R(t) x D(t).
    pub(in crate::handlers) gwt_system: Option<Arc<dyn GwtSystemProvider>>,

    /// Global workspace provider for winner-take-all memory selection.
    /// TASK-GWT-001: Required for workspace broadcast operations.
    pub(in crate::handlers) workspace_provider:
        Option<Arc<tokio::sync::RwLock<dyn WorkspaceProvider>>>,

    /// Meta-cognitive loop provider for self-correction.
    /// TASK-GWT-001: Required for meta_score computation and dream triggering.
    pub(in crate::handlers) meta_cognitive:
        Option<Arc<tokio::sync::RwLock<dyn MetaCognitiveProvider>>>,

    /// Self-ego node provider for system identity tracking.
    /// TASK-GWT-001: Required for identity continuity monitoring.
    pub(in crate::handlers) self_ego: Option<Arc<tokio::sync::RwLock<dyn SelfEgoProvider>>>,

    // ========== ADAPTIVE THRESHOLD CALIBRATION (TASK-ATC-001) ==========
    /// Adaptive Threshold Calibration system for self-learning thresholds.
    /// TASK-ATC-001: Required for get_threshold_status, get_calibration_metrics, trigger_recalibration.
    /// Uses RwLock because calibration operations mutate internal state.
    pub(in crate::handlers) atc:
        Option<Arc<RwLock<context_graph_core::atc::AdaptiveThresholdCalibration>>>,

    // ========== DREAM CONSOLIDATION (TASK-DREAM-MCP) ==========
    /// Dream controller for managing dream consolidation cycles.
    /// TASK-DREAM-MCP: Required for trigger_dream, get_dream_status, abort_dream.
    /// Uses RwLock because dream cycle operations mutate internal state.
    pub(in crate::handlers) dream_controller:
        Option<Arc<RwLock<context_graph_core::dream::DreamController>>>,

    /// Dream scheduler for determining when to trigger dream cycles.
    /// TASK-DREAM-MCP: Required for trigger_dream, get_dream_status.
    /// Uses RwLock because activity tracking mutates internal state.
    pub(in crate::handlers) dream_scheduler:
        Option<Arc<RwLock<context_graph_core::dream::DreamScheduler>>>,

    /// Amortized learner for shortcut creation during dreams.
    /// TASK-DREAM-MCP: Required for get_amortized_shortcuts.
    /// Uses RwLock because shortcut tracking mutates internal state.
    pub(in crate::handlers) amortized_learner:
        Option<Arc<RwLock<context_graph_core::dream::AmortizedLearner>>>,

    // ========== NEUROMODULATION (TASK-NEUROMOD-MCP) ==========
    /// Neuromodulation manager for controlling system behavior modulation.
    /// TASK-NEUROMOD-MCP: Required for get_neuromodulation_state, adjust_neuromodulator.
    /// Uses RwLock because modulator adjustments mutate internal state.
    pub(in crate::handlers) neuromod_manager:
        Option<Arc<RwLock<context_graph_core::neuromod::NeuromodulationManager>>>,

    // ========== IDENTITY CONTINUITY (TASK-IDENTITY-P0-001) ==========
    /// Identity continuity listener - processes workspace events and updates IC state.
    /// TASK-IDENTITY-P0-001: Required for shared monitor wiring between listener and provider.
    /// The GwtSystemProviderImpl MUST share this listener's monitor to satisfy AP-40.
    #[allow(dead_code)]
    pub(in crate::handlers) identity_listener: Option<Arc<IdentityContinuityListener>>,

    /// Workspace event broadcaster for identity event propagation.
    /// TASK-IDENTITY-P0-001: Required for IdentityContinuityListener subscription.
    #[allow(dead_code)]
    pub(in crate::handlers) workspace_broadcaster: Option<Arc<WorkspaceEventBroadcaster>>,
}

impl Handlers {
    /// Create new handlers with teleological dependencies.
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint (TASK-F008)
    /// * `utl_processor` - UTL metrics computation
    /// * `multi_array_provider` - 13-embedding generator (TASK-F007)
    /// * `alignment_calculator` - Goal alignment calculator (TASK-S003)
    /// * `goal_hierarchy` - Goal hierarchy with North Star (TASK-S003)
    ///
    /// # TASK-EMB-024 Note
    ///
    /// This constructor uses StubSystemMonitor and StubLayerStatusProvider as defaults.
    /// For production use with real metrics, use `with_full_monitoring()`.
    #[allow(dead_code)]
    pub fn new(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: GoalHierarchy,
    ) -> Self {
        // TASK-S004: Create Johari manager from teleological store
        let johari_manager: Arc<dyn JohariTransitionManager> =
            Arc::new(DynDefaultJohariManager::new(teleological_store.clone()));

        // TASK-S005: Create Meta-UTL tracker
        let meta_utl_tracker = Arc::new(RwLock::new(MetaUtlTracker::new()));

        // TASK-EMB-024: Default to stub monitors (will fail with explicit errors)
        let system_monitor: Arc<dyn SystemMonitor> = Arc::new(StubSystemMonitor::new());
        let layer_status_provider: Arc<dyn LayerStatusProvider> =
            Arc::new(StubLayerStatusProvider::new());

        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy: Arc::new(RwLock::new(goal_hierarchy)),
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            // TASK-GWT-001: GWT fields default to None - use with_gwt() for full GWT support
            kuramoto_network: None,
            gwt_system: None,
            workspace_provider: None,
            meta_cognitive: None,
            self_ego: None,
            // TASK-ATC-001: ATC defaults to None - use with_atc() for full ATC support
            atc: None,
            // TASK-DREAM-MCP: Dream fields default to None - use with_dream() for full dream support
            dream_controller: None,
            dream_scheduler: None,
            amortized_learner: None,
            // TASK-NEUROMOD-MCP: Neuromod defaults to None - use with_neuromod() for full support
            neuromod_manager: None,
            // TASK-IDENTITY-P0-001: Identity fields default to None - use with_default_gwt() for full support
            identity_listener: None,
            workspace_broadcaster: None,
        }
    }

    /// Create new handlers with shared goal hierarchy reference.
    ///
    /// Use this variant when you need to share the goal hierarchy across
    /// multiple handler instances or access it from outside.
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint (TASK-F008)
    /// * `utl_processor` - UTL metrics computation
    /// * `multi_array_provider` - 13-embedding generator (TASK-F007)
    /// * `alignment_calculator` - Goal alignment calculator (TASK-S003)
    /// * `goal_hierarchy` - Shared goal hierarchy reference (TASK-S003)
    ///
    /// # TASK-EMB-024 Note
    ///
    /// This constructor uses StubSystemMonitor and StubLayerStatusProvider as defaults.
    #[allow(dead_code)]
    pub fn with_shared_hierarchy(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
    ) -> Self {
        // TASK-S004: Create Johari manager from teleological store
        let johari_manager: Arc<dyn JohariTransitionManager> =
            Arc::new(DynDefaultJohariManager::new(teleological_store.clone()));

        // TASK-S005: Create Meta-UTL tracker
        let meta_utl_tracker = Arc::new(RwLock::new(MetaUtlTracker::new()));

        // TASK-EMB-024: Default to stub monitors (will fail with explicit errors)
        let system_monitor: Arc<dyn SystemMonitor> = Arc::new(StubSystemMonitor::new());
        let layer_status_provider: Arc<dyn LayerStatusProvider> =
            Arc::new(StubLayerStatusProvider::new());

        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            // TASK-GWT-001: GWT fields default to None - use with_gwt() for full GWT support
            kuramoto_network: None,
            gwt_system: None,
            workspace_provider: None,
            meta_cognitive: None,
            self_ego: None,
            atc: None,
            // TASK-DREAM-MCP: Dream fields default to None
            dream_controller: None,
            dream_scheduler: None,
            amortized_learner: None,
            // TASK-NEUROMOD-MCP: Neuromod defaults to None
            neuromod_manager: None,
            // TASK-IDENTITY-P0-001: Identity fields default to None
            identity_listener: None,
            workspace_broadcaster: None,
        }
    }

    /// Create new handlers with explicit Johari manager.
    ///
    /// Use this variant when you need to provide a custom JohariTransitionManager
    /// implementation or share it across multiple handler instances.
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint (TASK-F008)
    /// * `utl_processor` - UTL metrics computation
    /// * `multi_array_provider` - 13-embedding generator (TASK-F007)
    /// * `alignment_calculator` - Goal alignment calculator (TASK-S003)
    /// * `goal_hierarchy` - Shared goal hierarchy reference (TASK-S003)
    /// * `johari_manager` - Shared Johari manager reference (TASK-S004)
    ///
    /// # TASK-EMB-024 Note
    ///
    /// This constructor uses StubSystemMonitor and StubLayerStatusProvider as defaults.
    #[allow(dead_code)]
    pub fn with_johari_manager(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
        johari_manager: Arc<dyn JohariTransitionManager>,
    ) -> Self {
        // TASK-S005: Create Meta-UTL tracker
        let meta_utl_tracker = Arc::new(RwLock::new(MetaUtlTracker::new()));

        // TASK-EMB-024: Default to stub monitors (will fail with explicit errors)
        let system_monitor: Arc<dyn SystemMonitor> = Arc::new(StubSystemMonitor::new());
        let layer_status_provider: Arc<dyn LayerStatusProvider> =
            Arc::new(StubLayerStatusProvider::new());

        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            // TASK-GWT-001: GWT fields default to None - use with_gwt() for full GWT support
            kuramoto_network: None,
            gwt_system: None,
            workspace_provider: None,
            meta_cognitive: None,
            self_ego: None,
            atc: None,
            // TASK-DREAM-MCP: Dream fields default to None
            dream_controller: None,
            dream_scheduler: None,
            amortized_learner: None,
            // TASK-NEUROMOD-MCP: Neuromod defaults to None
            neuromod_manager: None,
            // TASK-IDENTITY-P0-001: Identity fields default to None
            identity_listener: None,
            workspace_broadcaster: None,
        }
    }

    /// Create new handlers with explicit Meta-UTL tracker.
    ///
    /// Use this variant when you need to provide a custom MetaUtlTracker
    /// implementation or share it across multiple handler instances (for testing).
    ///
    /// TASK-S005: Added for full state verification tests.
    ///
    /// # TASK-EMB-024 Note
    ///
    /// This constructor uses StubSystemMonitor and StubLayerStatusProvider as defaults.
    #[allow(dead_code)]
    pub fn with_meta_utl_tracker(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
        johari_manager: Arc<dyn JohariTransitionManager>,
        meta_utl_tracker: Arc<RwLock<MetaUtlTracker>>,
    ) -> Self {
        // TASK-EMB-024: Default to stub monitors (will fail with explicit errors)
        let system_monitor: Arc<dyn SystemMonitor> = Arc::new(StubSystemMonitor::new());
        let layer_status_provider: Arc<dyn LayerStatusProvider> =
            Arc::new(StubLayerStatusProvider::new());

        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            // TASK-GWT-001: GWT fields default to None - use with_gwt() for full GWT support
            kuramoto_network: None,
            gwt_system: None,
            workspace_provider: None,
            meta_cognitive: None,
            self_ego: None,
            atc: None,
            // TASK-DREAM-MCP: Dream fields default to None
            dream_controller: None,
            dream_scheduler: None,
            amortized_learner: None,
            // TASK-NEUROMOD-MCP: Neuromod defaults to None
            neuromod_manager: None,
            // TASK-IDENTITY-P0-001: Identity fields default to None
            identity_listener: None,
            workspace_broadcaster: None,
        }
    }

    /// Create new handlers with full monitoring support.
    ///
    /// TASK-EMB-024: This is the recommended constructor for production use
    /// when you need REAL health metrics (no hardcoded values).
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint (TASK-F008)
    /// * `utl_processor` - UTL metrics computation
    /// * `multi_array_provider` - 13-embedding generator (TASK-F007)
    /// * `alignment_calculator` - Goal alignment calculator (TASK-S003)
    /// * `goal_hierarchy` - Shared goal hierarchy reference (TASK-S003)
    /// * `johari_manager` - Shared Johari manager reference (TASK-S004)
    /// * `meta_utl_tracker` - Shared Meta-UTL tracker (TASK-S005)
    /// * `system_monitor` - Real system monitor for health metrics
    /// * `layer_status_provider` - Real layer status provider
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn with_full_monitoring(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
        johari_manager: Arc<dyn JohariTransitionManager>,
        meta_utl_tracker: Arc<RwLock<MetaUtlTracker>>,
        system_monitor: Arc<dyn SystemMonitor>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
    ) -> Self {
        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            // TASK-GWT-001: GWT fields default to None - use with_gwt() for full GWT support
            kuramoto_network: None,
            gwt_system: None,
            workspace_provider: None,
            meta_cognitive: None,
            self_ego: None,
            // TASK-ATC-001: ATC provider default to None - use with_atc() for ATC support
            atc: None,
            // TASK-DREAM-MCP: Dream fields default to None
            dream_controller: None,
            dream_scheduler: None,
            amortized_learner: None,
            // TASK-NEUROMOD-MCP: Neuromod defaults to None
            neuromod_manager: None,
            // TASK-IDENTITY-P0-001: Identity fields default to None
            identity_listener: None,
            workspace_broadcaster: None,
        }
    }

    /// Create new handlers with full GWT/consciousness support.
    ///
    /// TASK-GWT-001: This is the recommended constructor for production use
    /// with REAL GWT consciousness features. All GWT providers are REQUIRED.
    /// No stub implementations allowed - FAIL FAST on missing components.
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint (TASK-F008)
    /// * `utl_processor` - UTL metrics computation
    /// * `multi_array_provider` - 13-embedding generator (TASK-F007)
    /// * `alignment_calculator` - Goal alignment calculator (TASK-S003)
    /// * `goal_hierarchy` - Shared goal hierarchy reference (TASK-S003)
    /// * `johari_manager` - Shared Johari manager reference (TASK-S004)
    /// * `meta_utl_tracker` - Shared Meta-UTL tracker (TASK-S005)
    /// * `system_monitor` - Real system monitor for health metrics
    /// * `layer_status_provider` - Real layer status provider
    /// * `kuramoto_network` - Kuramoto oscillator network (TASK-GWT-001)
    /// * `gwt_system` - GWT consciousness system (TASK-GWT-001)
    /// * `workspace_provider` - Global workspace provider (TASK-GWT-001)
    /// * `meta_cognitive` - Meta-cognitive loop provider (TASK-GWT-001)
    /// * `self_ego` - Self-ego node provider (TASK-GWT-001)
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn with_gwt(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
        johari_manager: Arc<dyn JohariTransitionManager>,
        meta_utl_tracker: Arc<RwLock<MetaUtlTracker>>,
        system_monitor: Arc<dyn SystemMonitor>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
        kuramoto_network: Arc<RwLock<dyn KuramotoProvider>>,
        gwt_system: Arc<dyn GwtSystemProvider>,
        workspace_provider: Arc<tokio::sync::RwLock<dyn WorkspaceProvider>>,
        meta_cognitive: Arc<tokio::sync::RwLock<dyn MetaCognitiveProvider>>,
        self_ego: Arc<tokio::sync::RwLock<dyn SelfEgoProvider>>,
    ) -> Self {
        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            kuramoto_network: Some(kuramoto_network),
            gwt_system: Some(gwt_system),
            workspace_provider: Some(workspace_provider),
            meta_cognitive: Some(meta_cognitive),
            self_ego: Some(self_ego),
            // TASK-ATC-001: ATC provider default to None - use with_atc() for ATC support
            atc: None,
            // TASK-DREAM-MCP: Dream fields default to None
            dream_controller: None,
            dream_scheduler: None,
            amortized_learner: None,
            // TASK-NEUROMOD-MCP: Neuromod defaults to None
            neuromod_manager: None,
            // TASK-IDENTITY-P0-001: Identity fields default to None
            identity_listener: None,
            workspace_broadcaster: None,
        }
    }

    /// Create new handlers with ALL subsystems wired (GWT + ATC + Dream + Neuromod).
    ///
    /// TASK-EXHAUSTIVE-MCP: This is the ONLY constructor that enables ALL 35 MCP tools.
    /// Use for:
    /// - Exhaustive MCP tool testing
    /// - Full system integration tests
    /// - Production deployment where all features are required
    ///
    /// # Arguments
    /// All GWT providers (same as with_gwt) plus:
    /// * `atc` - Adaptive Threshold Calibration for get_threshold_status, get_calibration_metrics, trigger_recalibration
    /// * `dream_controller` - Dream consolidation controller for trigger_dream, get_dream_status, abort_dream
    /// * `dream_scheduler` - Dream scheduling logic
    /// * `amortized_learner` - Amortized shortcut learner for get_amortized_shortcuts
    /// * `neuromod_manager` - Neuromodulation manager for get_neuromodulation_state, adjust_neuromodulator
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)] // Reserved for future production integration
    pub fn with_gwt_and_subsystems(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
        johari_manager: Arc<dyn JohariTransitionManager>,
        meta_utl_tracker: Arc<RwLock<MetaUtlTracker>>,
        system_monitor: Arc<dyn SystemMonitor>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
        kuramoto_network: Arc<RwLock<dyn KuramotoProvider>>,
        gwt_system: Arc<dyn GwtSystemProvider>,
        workspace_provider: Arc<tokio::sync::RwLock<dyn WorkspaceProvider>>,
        meta_cognitive: Arc<tokio::sync::RwLock<dyn MetaCognitiveProvider>>,
        self_ego: Arc<tokio::sync::RwLock<dyn SelfEgoProvider>>,
        atc: Arc<RwLock<context_graph_core::atc::AdaptiveThresholdCalibration>>,
        dream_controller: Arc<RwLock<context_graph_core::dream::DreamController>>,
        dream_scheduler: Arc<RwLock<context_graph_core::dream::DreamScheduler>>,
        amortized_learner: Arc<RwLock<context_graph_core::dream::AmortizedLearner>>,
        neuromod_manager: Arc<RwLock<context_graph_core::neuromod::NeuromodulationManager>>,
    ) -> Self {
        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            kuramoto_network: Some(kuramoto_network),
            gwt_system: Some(gwt_system),
            workspace_provider: Some(workspace_provider),
            meta_cognitive: Some(meta_cognitive),
            self_ego: Some(self_ego),
            atc: Some(atc),
            dream_controller: Some(dream_controller),
            dream_scheduler: Some(dream_scheduler),
            amortized_learner: Some(amortized_learner),
            neuromod_manager: Some(neuromod_manager),
            // TASK-IDENTITY-P0-001: Identity fields default to None for this constructor
            identity_listener: None,
            workspace_broadcaster: None,
        }
    }

    /// Create new handlers with default GWT provider implementations.
    ///
    /// TASK-GWT-001: Convenience constructor that uses the real GWT provider
    /// implementations from `gwt_providers` module. This creates fresh instances
    /// of KuramotoNetwork, ConsciousnessCalculator, GlobalWorkspace, etc.
    ///
    /// All GWT tools will return REAL data - no stubs, no mocks.
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint (TASK-F008)
    /// * `utl_processor` - UTL metrics computation
    /// * `multi_array_provider` - 13-embedding generator (TASK-F007)
    /// * `alignment_calculator` - Goal alignment calculator (TASK-S003)
    /// * `goal_hierarchy` - Shared goal hierarchy reference (TASK-S003)
    /// * `johari_manager` - Shared Johari manager reference (TASK-S004)
    /// * `meta_utl_tracker` - Shared Meta-UTL tracker (TASK-S005)
    /// * `system_monitor` - Real system monitor for health metrics
    /// * `layer_status_provider` - Real layer status provider
    #[allow(clippy::too_many_arguments)]
    pub fn with_default_gwt(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
        goal_hierarchy: Arc<RwLock<GoalHierarchy>>,
        johari_manager: Arc<dyn JohariTransitionManager>,
        meta_utl_tracker: Arc<RwLock<MetaUtlTracker>>,
        system_monitor: Arc<dyn SystemMonitor>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
    ) -> Self {
        use super::super::gwt_providers::{
            GwtSystemProviderImpl, KuramotoProviderImpl, MetaCognitiveProviderImpl,
            SelfEgoProviderImpl, WorkspaceProviderImpl,
        };

        // =====================================================================
        // TASK-IDENTITY-P0-001: Create shared identity continuity infrastructure
        // =====================================================================
        // Constitution rule AP-40: MCP must read from correct monitor instance
        // The GwtSystemProviderImpl MUST share the IdentityContinuityListener's monitor
        // to ensure MCP tools return real IC values (not isolated defaults of 0.0).

        // Step 1: Create SelfEgoNode for identity snapshot recording
        let ego_node = Arc::new(tokio::sync::RwLock::new(SelfEgoNode::new()));

        // Step 2: Create WorkspaceEventBroadcaster for event propagation
        let workspace_broadcaster = Arc::new(WorkspaceEventBroadcaster::new());

        // Step 3: Create IdentityContinuityListener with dependencies
        // This listener owns the AUTHORITATIVE IdentityContinuityMonitor
        let identity_listener = Arc::new(IdentityContinuityListener::new(
            ego_node.clone(),
            workspace_broadcaster.clone(),
        ));

        // Step 4: Create GwtSystemProviderImpl with SHARED monitor (AP-40 fix)
        // This ensures MCP tools read from the same monitor that processes workspace events
        let gwt_system: Arc<dyn GwtSystemProvider> = Arc::new(
            GwtSystemProviderImpl::with_shared_monitor(identity_listener.monitor())
        );

        // Create real GWT provider implementations
        let kuramoto_network: Arc<RwLock<dyn KuramotoProvider>> =
            Arc::new(RwLock::new(KuramotoProviderImpl::new()));
        let workspace_provider: Arc<tokio::sync::RwLock<dyn WorkspaceProvider>> =
            Arc::new(tokio::sync::RwLock::new(WorkspaceProviderImpl::new()));
        let meta_cognitive: Arc<tokio::sync::RwLock<dyn MetaCognitiveProvider>> =
            Arc::new(tokio::sync::RwLock::new(MetaCognitiveProviderImpl::new()));
        let self_ego: Arc<tokio::sync::RwLock<dyn SelfEgoProvider>> =
            Arc::new(tokio::sync::RwLock::new(SelfEgoProviderImpl::new()));

        // TASK-NEUROMOD-MCP: Create REAL NeuromodulationManager with default baselines
        // Constitution neuromod section: Dopamine [1,5], Serotonin [0,1], Noradrenaline [0.5,2]
        // Acetylcholine is READ-ONLY, managed by GWT MetaCognitiveLoop
        let neuromod_manager: Arc<RwLock<NeuromodulationManager>> =
            Arc::new(RwLock::new(NeuromodulationManager::new()));

        // TASK-DREAM-MCP: Create REAL Dream components with constitution-mandated defaults
        // Constitution dream section:
        // - Trigger: activity < 0.15, idle 10min
        // - NREM: 3min, replay recent, tight coupling, recency_bias 0.8
        // - REM: 2min, explore attractors, temp 2.0
        // - Constraints: 100 queries, semantic_leap 0.7, abort_on_query, wake <100ms, gpu <30%
        // - Amortized: 3+ hop ≥5×, weight product(path), confidence ≥0.7
        let dream_controller: Arc<RwLock<DreamController>> =
            Arc::new(RwLock::new(DreamController::new()));
        let dream_scheduler: Arc<RwLock<DreamScheduler>> =
            Arc::new(RwLock::new(DreamScheduler::new()));
        let amortized_learner: Arc<RwLock<AmortizedLearner>> =
            Arc::new(RwLock::new(AmortizedLearner::new()));

        // TASK-ATC-001: Create REAL AdaptiveThresholdCalibration with constitution-mandated defaults
        // Constitution adaptive_thresholds section:
        // - Level 1 EWMA Drift Tracker (per-query)
        // - Level 2 Temperature Scaling (hourly, per-embedder T values)
        // - Level 3 Bandit Threshold Selector (session, UCB/Thompson Sampling)
        // - Level 4 Bayesian Meta-Optimizer (weekly, GP surrogate + EI acquisition)
        // Threshold priors: θ_opt=0.75, θ_acc=0.70, θ_warn=0.55, θ_dup=0.90, θ_edge=0.70, etc.
        let atc: Arc<RwLock<AdaptiveThresholdCalibration>> =
            Arc::new(RwLock::new(AdaptiveThresholdCalibration::new()));

        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
            kuramoto_network: Some(kuramoto_network),
            gwt_system: Some(gwt_system),
            workspace_provider: Some(workspace_provider),
            meta_cognitive: Some(meta_cognitive),
            self_ego: Some(self_ego),
            // TASK-ATC-001: REAL AdaptiveThresholdCalibration wired
            atc: Some(atc),
            // TASK-DREAM-MCP: REAL Dream components wired
            dream_controller: Some(dream_controller),
            dream_scheduler: Some(dream_scheduler),
            amortized_learner: Some(amortized_learner),
            // TASK-NEUROMOD-MCP: REAL NeuromodulationManager wired
            neuromod_manager: Some(neuromod_manager),
            // TASK-IDENTITY-P0-001: REAL identity continuity wiring (AP-40 fix)
            identity_listener: Some(identity_listener),
            workspace_broadcaster: Some(workspace_broadcaster),
        }
    }
}
