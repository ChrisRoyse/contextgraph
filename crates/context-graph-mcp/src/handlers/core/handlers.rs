//! Handlers struct definition and constructors.
//!
//! PRD v6 Section 10 - Handlers for all 14 MCP tools.
//!
//! TASK-INTEG-TOPIC: Added clustering dependencies for topic tools integration.
//! E4-FIX: Added session sequence counter for proper E4 (V_ordering) embeddings.
//! E7-WIRING: Added code embedding pipeline fields for search_code enhancement.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;
use tracing::{info, warn};

use context_graph_core::clustering::{MultiSpaceClusterManager, TopicStabilityTracker};
use context_graph_core::memory::{CodeEmbeddingProvider, CodeStorage};
use context_graph_core::monitoring::LayerStatusProvider;
use context_graph_core::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor,
};

use crate::protocol::{JsonRpcId, JsonRpcResponse};

/// Request handlers for MCP protocol.
///
/// PRD v6 Section 10 - Supports all 14 MCP tools:
/// - Core: inject_context, store_memory, get_memetic_status, search_graph, trigger_consolidation
/// - Topic: get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts
/// - Curation: merge_concepts, forget_concept, boost_importance
/// - Dream: trigger_dream, get_dream_status
pub struct Handlers {
    /// Teleological memory store - stores TeleologicalFingerprint with 13 embeddings.
    pub(in crate::handlers) teleological_store: Arc<dyn TeleologicalMemoryStore>,

    /// UTL processor for computing learning metrics.
    pub(in crate::handlers) utl_processor: Arc<dyn UtlProcessor>,

    /// Multi-array embedding provider - generates all 13 embeddings per content.
    pub(in crate::handlers) multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,

    /// Layer status provider for get_memetic_status.
    pub(in crate::handlers) layer_status_provider: Arc<dyn LayerStatusProvider>,

    /// Multi-space cluster manager for topic detection and clustering.
    /// TASK-INTEG-TOPIC: Added for topic tools integration.
    pub(in crate::handlers) cluster_manager: Arc<RwLock<MultiSpaceClusterManager>>,

    /// Topic stability tracker for portfolio-level stability metrics.
    /// TASK-INTEG-TOPIC: Added for topic tools integration.
    pub(in crate::handlers) stability_tracker: Arc<RwLock<TopicStabilityTracker>>,

    /// Session sequence counter for E4 (V_ordering) embeddings.
    /// Monotonically increasing within a session, used to track memory ordering.
    /// E4-FIX: Added to fix E4 sequence embedding.
    session_sequence_counter: Arc<AtomicU64>,

    /// Current session ID for session-scoped operations.
    /// E4-FIX: Added to track session context for E4 embeddings.
    current_session_id: Arc<RwLock<Option<String>>>,

    // =========================================================================
    // Code Embedding Pipeline (E7-WIRING)
    // =========================================================================

    /// Code storage backend for storing and retrieving code entities.
    /// Optional - only present if code embedding is enabled.
    /// E7-WIRING: Added for search_code to query CodeStore directly.
    pub(in crate::handlers) code_store: Option<Arc<dyn CodeStorage>>,

    /// Code embedding provider (E7 Qodo-Embed-1-1.5B).
    /// Optional - only present if code embedding is enabled.
    /// E7-WIRING: Added for generating E7 embeddings for code queries.
    pub(in crate::handlers) code_embedding_provider: Option<Arc<dyn CodeEmbeddingProvider>>,
}

impl Handlers {
    /// Create handlers with all dependencies explicitly provided.
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint
    /// * `utl_processor` - UTL processor for learning metrics
    /// * `multi_array_provider` - 13-embedding generator
    /// * `layer_status_provider` - Provider for layer status information
    /// * `cluster_manager` - Multi-space cluster manager for topic detection
    /// * `stability_tracker` - Topic stability tracker for portfolio metrics
    #[allow(dead_code)]
    pub fn with_all(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
        cluster_manager: Arc<RwLock<MultiSpaceClusterManager>>,
        stability_tracker: Arc<RwLock<TopicStabilityTracker>>,
    ) -> Self {
        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            layer_status_provider,
            cluster_manager,
            stability_tracker,
            // E4-FIX: Initialize session sequence counter and session ID
            session_sequence_counter: Arc::new(AtomicU64::new(0)),
            current_session_id: Arc::new(RwLock::new(None)),
            // E7-WIRING: Code pipeline disabled by default in with_all
            code_store: None,
            code_embedding_provider: None,
        }
    }

    /// Create handlers with all dependencies including code embedding pipeline.
    ///
    /// E7-WIRING: Extended constructor for full code embedding support.
    ///
    /// # Arguments
    /// * `teleological_store` - Store for TeleologicalFingerprint
    /// * `utl_processor` - UTL processor for learning metrics
    /// * `multi_array_provider` - 13-embedding generator
    /// * `layer_status_provider` - Provider for layer status information
    /// * `cluster_manager` - Multi-space cluster manager for topic detection
    /// * `stability_tracker` - Topic stability tracker for portfolio metrics
    /// * `code_store` - Code storage backend for code entities
    /// * `code_embedding_provider` - E7 code embedding provider
    #[allow(dead_code)]
    pub fn with_code_pipeline(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
        cluster_manager: Arc<RwLock<MultiSpaceClusterManager>>,
        stability_tracker: Arc<RwLock<TopicStabilityTracker>>,
        code_store: Arc<dyn CodeStorage>,
        code_embedding_provider: Arc<dyn CodeEmbeddingProvider>,
    ) -> Self {
        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            layer_status_provider,
            cluster_manager,
            stability_tracker,
            session_sequence_counter: Arc::new(AtomicU64::new(0)),
            current_session_id: Arc::new(RwLock::new(None)),
            code_store: Some(code_store),
            code_embedding_provider: Some(code_embedding_provider),
        }
    }

    /// Create handlers with default clustering components.
    ///
    /// This is a convenience constructor that creates default cluster manager
    /// and stability tracker. Use `with_all` for full control over dependencies.
    ///
    /// TASK-INTEG-TOPIC: Added for backwards compatibility during integration.
    pub fn with_defaults(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        utl_processor: Arc<dyn UtlProcessor>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
    ) -> Self {
        // Create default cluster manager
        let cluster_manager = MultiSpaceClusterManager::with_defaults()
            .expect("Default cluster manager should always succeed");

        // Create default stability tracker
        let stability_tracker = TopicStabilityTracker::new();

        Self {
            teleological_store,
            utl_processor,
            multi_array_provider,
            layer_status_provider,
            cluster_manager: Arc::new(RwLock::new(cluster_manager)),
            stability_tracker: Arc::new(RwLock::new(stability_tracker)),
            // E4-FIX: Initialize session sequence counter and session ID
            session_sequence_counter: Arc::new(AtomicU64::new(0)),
            current_session_id: Arc::new(RwLock::new(None)),
            // E7-WIRING: Code pipeline disabled by default
            code_store: None,
            code_embedding_provider: None,
        }
    }

    // =========================================================================
    // Code Pipeline Accessors (E7-WIRING)
    // =========================================================================

    /// Check if the code embedding pipeline is available.
    ///
    /// Returns true if both code_store and code_embedding_provider are configured.
    pub fn has_code_pipeline(&self) -> bool {
        self.code_store.is_some() && self.code_embedding_provider.is_some()
    }

    /// Get the code store if available.
    pub fn code_store(&self) -> Option<&Arc<dyn CodeStorage>> {
        self.code_store.as_ref()
    }

    /// Get the code embedding provider if available.
    pub fn code_embedding_provider(&self) -> Option<&Arc<dyn CodeEmbeddingProvider>> {
        self.code_embedding_provider.as_ref()
    }

    // =========================================================================
    // Session Sequence Management (E4-FIX)
    // =========================================================================

    /// Get the next session sequence number and atomically increment the counter.
    ///
    /// Returns a monotonically increasing sequence number within the current session.
    /// Used by memory tools to generate E4 (V_ordering) embeddings.
    ///
    /// # Returns
    /// The current sequence number before incrementing.
    pub fn get_next_sequence(&self) -> u64 {
        self.session_sequence_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Reset the session sequence counter to 0.
    ///
    /// Should be called at the start of a new session.
    pub fn reset_sequence(&self) {
        self.session_sequence_counter.store(0, Ordering::SeqCst);
    }

    /// Get the current session ID.
    ///
    /// Priority order:
    /// 1. CLAUDE_SESSION_ID environment variable
    /// 2. Previously stored session ID
    /// 3. None if no session ID is available
    pub fn get_session_id(&self) -> Option<String> {
        std::env::var("CLAUDE_SESSION_ID")
            .ok()
            .or_else(|| self.current_session_id.read().clone())
    }

    /// Set the current session ID.
    ///
    /// Also resets the sequence counter for the new session.
    pub fn set_session_id(&self, session_id: Option<String>) {
        *self.current_session_id.write() = session_id;
        self.reset_sequence();
    }

    /// Get the current sequence number without incrementing.
    ///
    /// Useful for debugging and status reporting.
    pub fn current_sequence(&self) -> u64 {
        self.session_sequence_counter.load(Ordering::SeqCst)
    }

    /// Handle MCP initialize request.
    ///
    /// Returns server capabilities per MCP protocol.
    /// Also restores topic portfolio from storage on initialization.
    pub async fn handle_initialize(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        info!("MCP initialize request received");

        // Restore topic portfolio from storage on server init
        match self.restore_topic_portfolio().await {
            Ok(topic_count) => {
                info!(topic_count, "Topic portfolio restored during MCP initialize");
            }
            Err(e) => {
                // Log error but don't fail initialization - new sessions can start fresh
                warn!(error = %e, "Failed to restore topic portfolio during init (continuing with empty portfolio)");
            }
        }

        let capabilities = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "context-graph",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        JsonRpcResponse::success(id, capabilities)
    }

    /// Handle MCP initialized notification.
    ///
    /// This is a notification (no response expected), but we return
    /// an empty success for consistency in dispatch.
    pub fn handle_initialized_notification(&self) -> JsonRpcResponse {
        info!("MCP initialized notification received");
        JsonRpcResponse::success(None, json!({}))
    }

    /// Handle MCP shutdown request.
    ///
    /// Performs graceful shutdown of handlers.
    /// PHASE-7: Persists topic portfolio before shutdown.
    pub async fn handle_shutdown(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        info!("MCP shutdown request received");

        // Persist topic portfolio before shutdown
        if let Err(e) = self.persist_topic_portfolio().await {
            tracing::error!(error = %e, "Failed to persist topic portfolio on shutdown");
        } else {
            info!("Topic portfolio persisted on shutdown");
        }

        JsonRpcResponse::success(id, json!({}))
    }

    // =========================================================================
    // Topic Portfolio Persistence (Phase 7)
    // =========================================================================

    /// Restore topic portfolio from storage on startup.
    ///
    /// Loads the latest persisted topic portfolio from RocksDB and imports
    /// it into the cluster manager. This ensures topics survive across sessions.
    ///
    /// # Returns
    ///
    /// Number of topics restored, or 0 if no portfolio was found.
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail.
    pub async fn restore_topic_portfolio(&self) -> Result<usize, context_graph_core::error::CoreError> {
        info!("Restoring topic portfolio from storage...");

        // Load latest portfolio from storage
        let portfolio = self.teleological_store.load_latest_topic_portfolio().await?;

        match portfolio {
            Some(portfolio) => {
                let _topic_count = portfolio.topic_count();
                let session_id = portfolio.session_id.clone();

                // Import into cluster manager
                let mut cluster_manager = self.cluster_manager.write();
                let imported = cluster_manager.import_portfolio(&portfolio);

                info!(
                    topic_count = imported,
                    original_session_id = %session_id,
                    churn_rate = portfolio.churn_rate,
                    entropy = portfolio.entropy,
                    "Topic portfolio restored from storage"
                );

                Ok(imported)
            }
            None => {
                info!("No existing topic portfolio found in storage");
                Ok(0)
            }
        }
    }

    /// Persist current topic portfolio to storage.
    ///
    /// Exports the current topic portfolio from the cluster manager and
    /// persists it to RocksDB. Called automatically on shutdown and can
    /// be called manually for checkpointing.
    ///
    /// # Returns
    ///
    /// Number of topics persisted.
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail.
    pub async fn persist_topic_portfolio(&self) -> Result<usize, context_graph_core::error::CoreError> {
        // Extract all data from locks BEFORE any async operations
        let (session_id, portfolio, churn_rate, entropy) = {
            // Get stability metrics from tracker
            let stability_tracker = self.stability_tracker.read();
            let churn_rate = stability_tracker.current_churn();
            // Lock is released at end of block

            // Get entropy from UTL processor (synchronous)
            let utl_status = self.utl_processor.get_status();
            let entropy = utl_status
                .get("entropy")
                .and_then(|v| v.as_f64())
                .map(|f| f as f32)
                .unwrap_or(0.0);

            // Export portfolio from cluster manager
            let cluster_manager = self.cluster_manager.read();
            let session_id = format!("session-{}", chrono::Utc::now().timestamp_millis());
            let portfolio = cluster_manager.export_portfolio(&session_id, churn_rate, entropy);
            // Lock is released at end of block

            (session_id, portfolio, churn_rate, entropy)
        };

        let topic_count = portfolio.topic_count();

        // Now all locks are released - safe to await
        self.teleological_store
            .persist_topic_portfolio(&session_id, &portfolio)
            .await?;

        info!(
            session_id = %session_id,
            topic_count = topic_count,
            churn_rate = churn_rate,
            entropy = entropy,
            "Topic portfolio persisted to storage"
        );

        Ok(topic_count)
    }
}
