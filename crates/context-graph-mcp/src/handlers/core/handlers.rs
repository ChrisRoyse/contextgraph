//! Handlers struct definition and constructors.
//!
//! PRD v6 Section 10 - Handlers for 56 MCP tools (52 without LLM feature).
//!
//! TASK-INTEG-TOPIC: Added clustering dependencies for topic tools integration.
//! E4-FIX: Added session sequence counter for proper E4 (V_ordering) embeddings.
//! E7-WIRING: Added code embedding pipeline fields for search_code enhancement.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;
use tokio::sync::RwLock as TokioRwLock;
use tracing::{info, warn};

use context_graph_core::clustering::{ClusterError, MultiSpaceClusterManager};
use context_graph_core::memory::{CodeEmbeddingProvider, CodeStorage};
use context_graph_core::monitoring::LayerStatusProvider;
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore};
use context_graph_mejepa::SystemCostCounters;
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository};

use crate::protocol::{JsonRpcId, JsonRpcResponse};

/// Request handlers for MCP protocol.
///
/// Supports 56 MCP tools with LLM feature (52 without) across 18 tool categories:
/// core, merge, curation, topic, file_watcher, sequence, causal, causal_discovery,
/// keyword, code, graph, robustness, entity, embedder, temporal, graph_link,
/// maintenance, provenance.
pub struct Handlers {
    /// Teleological memory store - stores TeleologicalFingerprint with 14 embeddings.
    pub(in crate::handlers) teleological_store: Arc<dyn TeleologicalMemoryStore>,

    /// Multi-array embedding provider - generates all 14 embeddings per content.
    pub(in crate::handlers) multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,

    /// Layer status provider for get_memetic_status.
    pub(in crate::handlers) layer_status_provider: Arc<dyn LayerStatusProvider>,

    /// Multi-space cluster manager for topic detection and clustering.
    /// TASK-INTEG-TOPIC: Added for topic tools integration.
    pub(in crate::handlers) cluster_manager: Arc<RwLock<MultiSpaceClusterManager>>,

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

    // =========================================================================
    // Graph Linking Pipeline (TASK-GRAPHLINK)
    // =========================================================================
    /// Edge repository for K-NN graph edges and typed edges.
    /// Optional - only present if graph linking is enabled.
    /// TASK-GRAPHLINK: Added for get_memory_neighbors, get_typed_edges, traverse_graph tools.
    pub(in crate::handlers) edge_repository: Option<EdgeRepository>,

    /// Background graph builder for K-NN graph construction.
    /// Optional - only present if graph linking is enabled.
    /// Queues fingerprints on store_memory and builds edges in batches.
    pub(in crate::handlers) graph_builder: Option<Arc<BackgroundGraphBuilder>>,

    // =========================================================================
    // Causal Hint Provider (CAUSAL-HINT Phase 5)
    // =========================================================================
    /// Causal hint provider for E5 embedding enhancement.
    /// Optional - currently uses the no-op provider after local discovery retirement.
    /// CAUSAL-HINT Phase 5: Provides direction hints to E5 embedder.
    pub(in crate::handlers) causal_hint_provider:
        Option<Arc<dyn context_graph_embeddings::provider::CausalHintProvider>>,

    // =========================================================================
    // Session-Scoped Custom Weight Profiles (NAV-GAP Phase 3.1)
    // =========================================================================
    /// Custom weight profiles created via create_weight_profile tool.
    /// Session-scoped: cleared when the server restarts.
    /// Accessible by name from search_graph's weightProfile, get_unified_neighbors, etc.
    pub(in crate::handlers) custom_profiles: Arc<RwLock<HashMap<String, [f32; 14]>>>,

    // =========================================================================
    // Daemon State (Multi-agent observability)
    // =========================================================================
    /// Daemon runtime state for the daemon_status tool.
    /// Injected by McpServer::new() via set_daemon_state() before Arc-wrapping.
    /// None in stdio mode or tests where daemon state is not applicable.
    pub(in crate::handlers) daemon_state:
        Option<Arc<crate::handlers::tools::daemon_tools::DaemonState>>,

    /// Resource counters used by ME-JEPA producer paths.
    pub(in crate::handlers) system_cost_counters: Arc<SystemCostCounters>,

    /// Auto-consolidation status for daemon_status reporting.
    pub(in crate::handlers) auto_consolidation_status:
        Arc<TokioRwLock<crate::handlers::tools::consolidation::AutoConsolidationStatus>>,
}

impl Handlers {
    /// Create handlers with default clustering components.
    ///
    /// # Errors
    ///
    /// Returns `ClusterError` if default cluster manager initialization fails.
    /// Create handlers after local discovery actor retirement.
    ///
    /// Graph linking remains deterministic. Causal hints use the injected provider,
    /// which is currently the no-op provider from `context-graph-embeddings`.
    pub fn without_llm(
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        layer_status_provider: Arc<dyn LayerStatusProvider>,
        edge_repository: EdgeRepository,
        graph_builder: Arc<BackgroundGraphBuilder>,
        causal_hint_provider: Arc<dyn context_graph_embeddings::provider::CausalHintProvider>,
    ) -> Result<Self, ClusterError> {
        info!("Creating Handlers without LLM (graph/causal discovery tools unavailable)");

        let cluster_manager = MultiSpaceClusterManager::with_defaults()?;

        Ok(Self {
            teleological_store,
            multi_array_provider,
            layer_status_provider,
            cluster_manager: Arc::new(RwLock::new(cluster_manager)),
            session_sequence_counter: Arc::new(AtomicU64::new(0)),
            current_session_id: Arc::new(RwLock::new(None)),
            code_store: None,
            code_embedding_provider: None,
            edge_repository: Some(edge_repository),
            graph_builder: Some(graph_builder),
            causal_hint_provider: Some(causal_hint_provider),
            custom_profiles: Arc::new(RwLock::new(HashMap::new())),
            daemon_state: None,
            system_cost_counters: Arc::new(SystemCostCounters::new()),
            auto_consolidation_status: Arc::new(TokioRwLock::new(
                crate::handlers::tools::consolidation::AutoConsolidationStatus::default(),
            )),
        })
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
    // Graph Linking Pipeline Accessors (TASK-GRAPHLINK)
    // =========================================================================

    /// Get the edge repository if available.
    pub fn edge_repository(&self) -> Option<&EdgeRepository> {
        self.edge_repository.as_ref()
    }

    /// Get the background graph builder if available.
    ///
    /// The graph builder queues fingerprints on store_memory and builds K-NN edges in batches.
    pub fn graph_builder(&self) -> Option<&Arc<BackgroundGraphBuilder>> {
        self.graph_builder.as_ref()
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

    /// Atomically get or initialize the session ID.
    ///
    /// If no session ID exists (neither env var nor stored), generates a new UUID
    /// and stores it. Uses a single write lock to prevent TOCTOU races where
    /// concurrent calls could generate different session IDs.
    /// Does NOT reset the sequence counter (caller should call get_next_sequence after).
    pub fn get_or_init_session_id(&self) -> String {
        // Fast path: check env var first (no lock needed)
        if let Ok(env_id) = std::env::var("CLAUDE_SESSION_ID") {
            return env_id;
        }

        // Single write lock for atomic check-and-set
        let mut guard = self.current_session_id.write();
        if let Some(existing) = guard.as_ref() {
            return existing.clone();
        }

        let new_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(session_id = %new_id, "Auto-generated session ID for this server session");
        *guard = Some(new_id.clone());
        new_id
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
                info!(
                    topic_count,
                    "Topic portfolio restored during MCP initialize"
                );
            }
            Err(e) => {
                // Log error but don't fail initialization - new sessions can start fresh
                warn!(error = %e, "Failed to restore topic portfolio during init (continuing with empty portfolio)");
            }
        }

        // Load custom weight profiles from RocksDB into in-memory cache
        match self.teleological_store.list_custom_weight_profiles().await {
            Ok(profiles) => {
                let count = profiles.len();
                if count > 0 {
                    let mut cache = self.custom_profiles.write();
                    for (name, weights) in profiles {
                        cache.insert(name, weights);
                    }
                    info!(
                        profile_count = count,
                        "Custom weight profiles loaded from RocksDB"
                    );
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to load custom weight profiles from RocksDB (continuing with empty cache)");
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
    /// Per JSON-RPC 2.0 / MCP spec: notifications MUST NOT receive a response.
    /// MCP-M1 FIX: Return all-None sentinel so the transport suppression check
    /// (`id.is_none() && result.is_none() && error.is_none()`) correctly
    /// prevents sending any response to the client.
    pub fn handle_initialized_notification(&self) -> JsonRpcResponse {
        info!("MCP initialized notification received");
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result: None,
            error: None,
        }
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
    pub async fn restore_topic_portfolio(
        &self,
    ) -> Result<usize, context_graph_core::error::CoreError> {
        info!("Restoring topic portfolio from storage...");

        // Load latest portfolio from storage
        let portfolio = self
            .teleological_store
            .load_latest_topic_portfolio()
            .await?;

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
    pub async fn persist_topic_portfolio(
        &self,
    ) -> Result<usize, context_graph_core::error::CoreError> {
        // Extract all data from locks BEFORE any async operations
        let (session_id, portfolio, churn_rate) = {
            // Get stability metrics from cluster_manager's internal tracker
            let cluster_manager = self.cluster_manager.read();
            let churn_rate = cluster_manager.current_churn();
            // Entropy is no longer tracked via UTL processor
            let entropy = 0.0_f32;

            let session_id = format!("session-{}", chrono::Utc::now().timestamp_millis());
            let portfolio = cluster_manager.export_portfolio(&session_id, churn_rate, entropy);

            (session_id, portfolio, churn_rate)
        };

        let topic_count = portfolio.topics.len();

        // Now all locks are released - safe to await
        self.teleological_store
            .persist_topic_portfolio(&session_id, &portfolio)
            .await?;

        info!(
            session_id = %session_id,
            topic_count = topic_count,
            churn_rate = churn_rate,
            "Topic portfolio persisted to storage"
        );

        Ok(topic_count)
    }
}
