//! Core Handlers struct and dispatch logic.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! TASK-S003: Added GoalAlignmentCalculator for purpose/goal operations.
//! TASK-S004: Added JohariTransitionManager for johari/* handlers.
//! TASK-S005: Added MetaUtlTracker for meta_utl/* handlers.
//! TASK-GWT-001: Added GWT/Kuramoto provider traits for consciousness operations.
//! NO BACKWARDS COMPATIBILITY with legacy MemoryStore trait.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tracing::debug;
use uuid::Uuid;

use context_graph_core::alignment::GoalAlignmentCalculator;
use context_graph_core::atc::AdaptiveThresholdCalibration;
use context_graph_core::dream::{AmortizedLearner, DreamController, DreamScheduler};
use context_graph_core::johari::{DynDefaultJohariManager, JohariTransitionManager, NUM_EMBEDDERS};
use context_graph_core::monitoring::{LayerStatusProvider, StubLayerStatusProvider, StubSystemMonitor, SystemMonitor};
use context_graph_core::neuromod::NeuromodulationManager;
use context_graph_core::purpose::GoalHierarchy;
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor};

// TASK-GWT-001: Import GWT provider traits
use super::gwt_traits::{
    GwtSystemProvider, KuramotoProvider, MetaCognitiveProvider,
    SelfEgoProvider, WorkspaceProvider,
};

/// Prediction type for tracking
/// TASK-S005: Used to distinguish storage vs retrieval predictions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionType {
    Storage,
    Retrieval,
}

/// Stored prediction for validation
/// TASK-S005: Stores predicted values for later validation against actual outcomes.
#[derive(Clone, Debug)]
pub struct StoredPrediction {
    pub _created_at: Instant,
    pub prediction_type: PredictionType,
    pub predicted_values: serde_json::Value,
    #[allow(dead_code)]
    pub fingerprint_id: Uuid,
}

/// Meta-UTL Tracker for learning about learning
///
/// TASK-S005: Tracks per-embedder accuracy, pending predictions, and optimized weights.
/// Uses rolling window for accuracy tracking to maintain recency bias.
#[derive(Debug)]
pub struct MetaUtlTracker {
    /// Pending predictions awaiting validation
    pub pending_predictions: HashMap<Uuid, StoredPrediction>,
    /// Per-embedder accuracy rolling window (100 samples per embedder)
    pub embedder_accuracy: [[f32; 100]; NUM_EMBEDDERS],
    /// Current index in each embedder's rolling window
    pub accuracy_indices: [usize; NUM_EMBEDDERS],
    /// Number of samples in each embedder's rolling window
    pub accuracy_counts: [usize; NUM_EMBEDDERS],
    /// Current optimized weights (sum to 1.0)
    pub current_weights: [f32; NUM_EMBEDDERS],
    /// Total predictions made
    pub prediction_count: usize,
    /// Total validations completed
    pub validation_count: usize,
    /// Last weight update timestamp
    pub last_weight_update: Option<Instant>,
}

impl Default for MetaUtlTracker {
    fn default() -> Self {
        // Initialize with uniform weights (1/13 each)
        let initial_weight = 1.0 / NUM_EMBEDDERS as f32;
        Self {
            pending_predictions: HashMap::new(),
            embedder_accuracy: [[0.0; 100]; NUM_EMBEDDERS],
            accuracy_indices: [0; NUM_EMBEDDERS],
            accuracy_counts: [0; NUM_EMBEDDERS],
            current_weights: [initial_weight; NUM_EMBEDDERS],
            prediction_count: 0,
            validation_count: 0,
            last_weight_update: None,
        }
    }
}

impl MetaUtlTracker {
    /// Create a new MetaUtlTracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a prediction for later validation
    pub fn store_prediction(&mut self, prediction_id: Uuid, prediction: StoredPrediction) {
        self.pending_predictions.insert(prediction_id, prediction);
        self.prediction_count += 1;
    }

    /// Get a pending prediction by ID
    #[allow(dead_code)]
    pub fn get_prediction(&self, prediction_id: &Uuid) -> Option<&StoredPrediction> {
        self.pending_predictions.get(prediction_id)
    }

    /// Remove and return a prediction (for validation)
    pub fn remove_prediction(&mut self, prediction_id: &Uuid) -> Option<StoredPrediction> {
        self.pending_predictions.remove(prediction_id)
    }

    /// Record accuracy for an embedder
    pub fn record_accuracy(&mut self, embedder_index: usize, accuracy: f32) {
        if embedder_index >= NUM_EMBEDDERS {
            return;
        }
        let idx = self.accuracy_indices[embedder_index];
        self.embedder_accuracy[embedder_index][idx] = accuracy;
        self.accuracy_indices[embedder_index] = (idx + 1) % 100;
        if self.accuracy_counts[embedder_index] < 100 {
            self.accuracy_counts[embedder_index] += 1;
        }
    }

    /// Get average accuracy for an embedder
    pub fn get_embedder_accuracy(&self, embedder_index: usize) -> Option<f32> {
        if embedder_index >= NUM_EMBEDDERS || self.accuracy_counts[embedder_index] == 0 {
            return None;
        }
        let count = self.accuracy_counts[embedder_index];
        let sum: f32 = self.embedder_accuracy[embedder_index][..count].iter().sum();
        Some(sum / count as f32)
    }

    /// Get accuracy trend for an embedder (recent vs older samples)
    pub fn get_accuracy_trend(&self, embedder_index: usize) -> Option<&'static str> {
        if embedder_index >= NUM_EMBEDDERS || self.accuracy_counts[embedder_index] < 10 {
            return None;
        }
        let count = self.accuracy_counts[embedder_index];
        let recent_start = count.saturating_sub(10);
        let recent_sum: f32 = self.embedder_accuracy[embedder_index][recent_start..count]
            .iter()
            .sum();
        let recent_avg = recent_sum / 10.0;

        let older_end = if count >= 20 { count - 10 } else { count - (count / 2) };
        let older_start = older_end.saturating_sub(10);
        let older_sum: f32 = self.embedder_accuracy[embedder_index][older_start..older_end]
            .iter()
            .sum();
        let older_count = older_end - older_start;
        if older_count == 0 {
            return Some("stable");
        }
        let older_avg = older_sum / older_count as f32;

        if recent_avg > older_avg + 0.02 {
            Some("improving")
        } else if recent_avg < older_avg - 0.02 {
            Some("declining")
        } else {
            Some("stable")
        }
    }

    /// Update weights based on accuracy (called every 100 validations)
    pub fn update_weights(&mut self) {
        // Calculate average accuracy per embedder
        let mut accuracies = [0.0f32; NUM_EMBEDDERS];
        let mut total_accuracy = 0.0f32;

        for i in 0..NUM_EMBEDDERS {
            accuracies[i] = self.get_embedder_accuracy(i).unwrap_or(1.0 / NUM_EMBEDDERS as f32);
            total_accuracy += accuracies[i];
        }

        // Normalize to get weights
        if total_accuracy > 0.0 {
            for i in 0..NUM_EMBEDDERS {
                self.current_weights[i] = accuracies[i] / total_accuracy;
            }
        }

        self.last_weight_update = Some(Instant::now());
    }

    /// Increment validation count and check if weights need update
    pub fn record_validation(&mut self) {
        self.validation_count += 1;
        if self.validation_count.is_multiple_of(100) {
            self.update_weights();
        }
    }
}

use crate::protocol::{error_codes, methods, JsonRpcRequest, JsonRpcResponse};

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
    pub(super) teleological_store: Arc<dyn TeleologicalMemoryStore>,

    /// UTL processor for computing learning metrics.
    pub(super) utl_processor: Arc<dyn UtlProcessor>,

    /// Multi-array embedding provider - generates all 13 embeddings per content.
    /// NO legacy single-embedding support.
    pub(super) multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,

    /// Goal alignment calculator - computes alignment between fingerprints and goal hierarchy.
    /// TASK-S003: Required for purpose/north_star_alignment and purpose/drift_check.
    pub(super) alignment_calculator: Arc<dyn GoalAlignmentCalculator>,

    /// Goal hierarchy - defines North Star and sub-goals.
    /// TASK-S003: RwLock allows runtime updates via purpose/north_star_update.
    pub(super) goal_hierarchy: Arc<RwLock<GoalHierarchy>>,

    /// Johari transition manager - manages Johari quadrant transitions.
    /// TASK-S004: Required for johari/* handlers.
    pub(super) johari_manager: Arc<dyn JohariTransitionManager>,

    /// Meta-UTL tracker - tracks predictions and per-embedder accuracy.
    /// TASK-S005: Required for meta_utl/* handlers.
    pub(super) meta_utl_tracker: Arc<RwLock<MetaUtlTracker>>,

    /// System monitor for REAL health metrics.
    /// TASK-EMB-024: Required for meta_utl/health_metrics - NO hardcoded values.
    pub(super) system_monitor: Arc<dyn SystemMonitor>,

    /// Layer status provider for REAL layer statuses.
    /// TASK-EMB-024: Required for get_memetic_status and get_graph_manifest - NO hardcoded values.
    pub(super) layer_status_provider: Arc<dyn LayerStatusProvider>,

    // ========== GWT/Kuramoto Fields (TASK-GWT-001) ==========

    /// Kuramoto oscillator network for 13-embedding phase synchronization.
    /// TASK-GWT-001: Required for gwt/* handlers and consciousness computation.
    /// Uses RwLock because step() mutates internal state.
    pub(super) kuramoto_network: Option<Arc<RwLock<dyn KuramotoProvider>>>,

    /// GWT consciousness system provider.
    /// TASK-GWT-001: Required for consciousness computation C(t) = I(t) x R(t) x D(t).
    pub(super) gwt_system: Option<Arc<dyn GwtSystemProvider>>,

    /// Global workspace provider for winner-take-all memory selection.
    /// TASK-GWT-001: Required for workspace broadcast operations.
    pub(super) workspace_provider: Option<Arc<tokio::sync::RwLock<dyn WorkspaceProvider>>>,

    /// Meta-cognitive loop provider for self-correction.
    /// TASK-GWT-001: Required for meta_score computation and dream triggering.
    pub(super) meta_cognitive: Option<Arc<tokio::sync::RwLock<dyn MetaCognitiveProvider>>>,

    /// Self-ego node provider for system identity tracking.
    /// TASK-GWT-001: Required for identity continuity monitoring.
    pub(super) self_ego: Option<Arc<tokio::sync::RwLock<dyn SelfEgoProvider>>>,

    // ========== ADAPTIVE THRESHOLD CALIBRATION (TASK-ATC-001) ==========

    /// Adaptive Threshold Calibration system for self-learning thresholds.
    /// TASK-ATC-001: Required for get_threshold_status, get_calibration_metrics, trigger_recalibration.
    /// Uses RwLock because calibration operations mutate internal state.
    pub(super) atc: Option<Arc<RwLock<context_graph_core::atc::AdaptiveThresholdCalibration>>>,

    // ========== DREAM CONSOLIDATION (TASK-DREAM-MCP) ==========

    /// Dream controller for managing dream consolidation cycles.
    /// TASK-DREAM-MCP: Required for trigger_dream, get_dream_status, abort_dream.
    /// Uses RwLock because dream cycle operations mutate internal state.
    pub(super) dream_controller: Option<Arc<RwLock<context_graph_core::dream::DreamController>>>,

    /// Dream scheduler for determining when to trigger dream cycles.
    /// TASK-DREAM-MCP: Required for trigger_dream, get_dream_status.
    /// Uses RwLock because activity tracking mutates internal state.
    pub(super) dream_scheduler: Option<Arc<RwLock<context_graph_core::dream::DreamScheduler>>>,

    /// Amortized learner for shortcut creation during dreams.
    /// TASK-DREAM-MCP: Required for get_amortized_shortcuts.
    /// Uses RwLock because shortcut tracking mutates internal state.
    pub(super) amortized_learner: Option<Arc<RwLock<context_graph_core::dream::AmortizedLearner>>>,

    // ========== NEUROMODULATION (TASK-NEUROMOD-MCP) ==========

    /// Neuromodulation manager for controlling system behavior modulation.
    /// TASK-NEUROMOD-MCP: Required for get_neuromodulation_state, adjust_neuromodulator.
    /// Uses RwLock because modulator adjustments mutate internal state.
    pub(super) neuromod_manager: Option<Arc<RwLock<context_graph_core::neuromod::NeuromodulationManager>>>,
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
        let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider::new());

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
        let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider::new());

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
        let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider::new());

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
        let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider::new());

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
        use super::gwt_providers::{
            GwtSystemProviderImpl, KuramotoProviderImpl, MetaCognitiveProviderImpl,
            SelfEgoProviderImpl, WorkspaceProviderImpl,
        };

        // Create real GWT provider implementations
        let kuramoto_network: Arc<RwLock<dyn KuramotoProvider>> =
            Arc::new(RwLock::new(KuramotoProviderImpl::new()));
        let gwt_system: Arc<dyn GwtSystemProvider> =
            Arc::new(GwtSystemProviderImpl::new());
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
        }
    }

    /// Dispatch a request to the appropriate handler.
    pub async fn dispatch(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Dispatching method: {}", request.method);

        match request.method.as_str() {
            // MCP lifecycle methods
            methods::INITIALIZE => self.handle_initialize(request.id).await,
            "notifications/initialized" => self.handle_initialized_notification(),
            methods::SHUTDOWN => self.handle_shutdown(request.id).await,

            // MCP tools protocol
            methods::TOOLS_LIST => self.handle_tools_list(request.id).await,
            methods::TOOLS_CALL => self.handle_tools_call(request.id, request.params).await,

            // Legacy direct methods (kept for backward compatibility)
            methods::MEMORY_STORE => self.handle_memory_store(request.id, request.params).await,
            methods::MEMORY_RETRIEVE => {
                self.handle_memory_retrieve(request.id, request.params)
                    .await
            }
            methods::MEMORY_SEARCH => self.handle_memory_search(request.id, request.params).await,
            methods::MEMORY_DELETE => self.handle_memory_delete(request.id, request.params).await,

            // Search operations (TASK-S002)
            methods::SEARCH_MULTI => self.handle_search_multi(request.id, request.params).await,
            methods::SEARCH_SINGLE_SPACE => {
                self.handle_search_single_space(request.id, request.params)
                    .await
            }
            methods::SEARCH_BY_PURPOSE => {
                self.handle_search_by_purpose(request.id, request.params)
                    .await
            }
            methods::SEARCH_WEIGHT_PROFILES => self.handle_get_weight_profiles(request.id).await,

            // Purpose/goal operations (TASK-S003)
            // NOTE: PURPOSE_NORTH_STAR_ALIGNMENT and NORTH_STAR_UPDATE removed per TASK-CORE-001 (ARCH-03)
            // These methods now fall through to the default case returning METHOD_NOT_FOUND (-32601)
            // Use auto_bootstrap_north_star tool for autonomous goal discovery instead.
            methods::PURPOSE_QUERY => self.handle_purpose_query(request.id, request.params).await,
            methods::GOAL_HIERARCHY_QUERY => {
                self.handle_goal_hierarchy_query(request.id, request.params)
                    .await
            }
            methods::GOAL_ALIGNED_MEMORIES => {
                self.handle_goal_aligned_memories(request.id, request.params)
                    .await
            }
            methods::PURPOSE_DRIFT_CHECK => {
                self.handle_purpose_drift_check(request.id, request.params)
                    .await
            }

            // Johari operations (TASK-S004)
            methods::JOHARI_GET_DISTRIBUTION => {
                self.handle_johari_get_distribution(request.id, request.params)
                    .await
            }
            methods::JOHARI_FIND_BY_QUADRANT => {
                self.handle_johari_find_by_quadrant(request.id, request.params)
                    .await
            }
            methods::JOHARI_TRANSITION => {
                self.handle_johari_transition(request.id, request.params)
                    .await
            }
            methods::JOHARI_TRANSITION_BATCH => {
                self.handle_johari_transition_batch(request.id, request.params)
                    .await
            }
            methods::JOHARI_CROSS_SPACE_ANALYSIS => {
                self.handle_johari_cross_space_analysis(request.id, request.params)
                    .await
            }
            methods::JOHARI_TRANSITION_PROBABILITIES => {
                self.handle_johari_transition_probabilities(request.id, request.params)
                    .await
            }

            methods::UTL_COMPUTE => self.handle_utl_compute(request.id, request.params).await,
            methods::UTL_METRICS => self.handle_utl_metrics(request.id, request.params).await,

            // Meta-UTL operations (TASK-S005)
            methods::META_UTL_LEARNING_TRAJECTORY => {
                self.handle_meta_utl_learning_trajectory(request.id, request.params)
                    .await
            }
            methods::META_UTL_HEALTH_METRICS => {
                self.handle_meta_utl_health_metrics(request.id, request.params)
                    .await
            }
            methods::META_UTL_PREDICT_STORAGE => {
                self.handle_meta_utl_predict_storage(request.id, request.params)
                    .await
            }
            methods::META_UTL_PREDICT_RETRIEVAL => {
                self.handle_meta_utl_predict_retrieval(request.id, request.params)
                    .await
            }
            methods::META_UTL_VALIDATE_PREDICTION => {
                self.handle_meta_utl_validate_prediction(request.id, request.params)
                    .await
            }
            methods::META_UTL_OPTIMIZED_WEIGHTS => {
                self.handle_meta_utl_optimized_weights(request.id, request.params)
                    .await
            }

            methods::SYSTEM_STATUS => self.handle_system_status(request.id).await,
            methods::SYSTEM_HEALTH => self.handle_system_health(request.id).await,
            _ => JsonRpcResponse::error(
                request.id,
                error_codes::METHOD_NOT_FOUND,
                format!("Method not found: {}", request.method),
            ),
        }
    }
}
