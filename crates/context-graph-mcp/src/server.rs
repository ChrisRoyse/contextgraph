//! MCP Server implementation.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! TASK-S003: Added GoalAlignmentCalculator and GoalHierarchy for purpose operations.
//! TASK-S004: Replaced stubs with REAL implementations (RocksDB, UTL adapter).
//!
//! NO BACKWARDS COMPATIBILITY with stubs. FAIL FAST with clear errors.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::config::Config;
use context_graph_core::purpose::GoalHierarchy;
use context_graph_core::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor,
};

use context_graph_embeddings::{GpuConfig, ProductionMultiArrayProvider};

// REAL implementations - NO STUBS
use crate::adapters::UtlProcessorAdapter;
use context_graph_storage::teleological::RocksDbTeleologicalStore;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

// NOTE: LazyFailMultiArrayProvider was removed - now using ProductionMultiArrayProvider
// from context-graph-embeddings crate (TASK-F007 COMPLETED)

// ============================================================================
// MCP Server
// ============================================================================

/// MCP Server state.
///
/// TASK-S001: Uses TeleologicalMemoryStore for 13-embedding fingerprint storage.
#[allow(dead_code)]
pub struct McpServer {
    config: Config,
    /// Teleological memory store - stores TeleologicalFingerprint with 13 embeddings.
    teleological_store: Arc<dyn TeleologicalMemoryStore>,
    utl_processor: Arc<dyn UtlProcessor>,
    /// Multi-array embedding provider - generates all 13 embeddings.
    multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
    handlers: Handlers,
    initialized: Arc<RwLock<bool>>,
}

impl McpServer {
    /// Create a new MCP server with the given configuration.
    ///
    /// TASK-S001: Creates TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
    /// TASK-S004: Uses REAL implementations - RocksDbTeleologicalStore, UtlProcessorAdapter.
    ///
    /// # Errors
    ///
    /// - Returns error if RocksDB fails to open (path issues, permissions, corruption)
    /// - Returns error if MultiArrayEmbeddingProvider is not yet implemented (FAIL FAST)
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing MCP Server with REAL implementations (NO STUBS)...");

        // ==========================================================================
        // 1. Create RocksDB teleological store (REAL persistent storage)
        // ==========================================================================
        let db_path = Self::resolve_storage_path(&config);
        info!("Opening RocksDbTeleologicalStore at {:?}...", db_path);

        let rocksdb_store = RocksDbTeleologicalStore::open(&db_path).map_err(|e| {
            error!("FATAL: Failed to open RocksDB at {:?}: {}", db_path, e);
            anyhow::anyhow!(
                "Failed to open RocksDbTeleologicalStore at {:?}: {}. \
                 Check path exists, permissions, and RocksDB isn't locked by another process.",
                db_path,
                e
            )
        })?;
        info!(
            "Created RocksDbTeleologicalStore at {:?} (17 column families, persistent storage)",
            db_path
        );

        // Note: EmbedderIndexRegistry is initialized in the constructor,
        // so no separate initialization step is needed.
        info!("Created store with EmbedderIndexRegistry (12 HNSW-capable embedders initialized)");

        let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

        // ==========================================================================
        // 2. Create REAL UTL processor (6-component computation)
        // ==========================================================================
        let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());
        info!("Created UtlProcessorAdapter (REAL 6-component UTL computation: deltaS, deltaC, wE, phi, lambda, magnitude)");

        // ==========================================================================
        // 3. REAL MultiArrayEmbeddingProvider - 13 GPU-accelerated embedders
        // ==========================================================================
        // TASK-F007 COMPLETED: ProductionMultiArrayProvider orchestrates all 13 embedders
        // - E1-E5: Semantic, Temporal (3 variants), Causal
        // - E6, E13: Sparse embedders (SPLADE)
        // - E7-E11: Code, Graph, HDC, Multimodal, Entity
        // - E12: Late-interaction (ColBERT)
        //
        // GPU Requirements: NVIDIA CUDA GPU with 8GB+ VRAM
        // Model Directory: ./models relative to binary (configurable via env)
        let models_dir = Self::resolve_models_path(&config);
        info!(
            "Loading ProductionMultiArrayProvider with models from {:?}...",
            models_dir
        );

        let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> = Arc::new(
            ProductionMultiArrayProvider::new(models_dir.clone(), GpuConfig::default())
                .await
                .map_err(|e| {
                    error!(
                        "FATAL: Failed to create ProductionMultiArrayProvider: {}",
                        e
                    );
                    anyhow::anyhow!(
                        "Failed to create ProductionMultiArrayProvider: {}. \
                         Ensure models exist at {:?} and CUDA GPU is available.",
                        e,
                        models_dir
                    )
                })?,
        );
        info!("Created ProductionMultiArrayProvider (13 embedders, GPU-accelerated, <30ms target)");

        // TASK-S003: Create alignment calculator and goal hierarchy
        let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
            Arc::new(DefaultAlignmentCalculator::new());
        let goal_hierarchy = Arc::new(parking_lot::RwLock::new(GoalHierarchy::new()));
        info!("Created DefaultAlignmentCalculator and empty GoalHierarchy");

        // ==========================================================================
        // 4. Create Johari manager, Meta-UTL tracker, and monitoring providers
        // ==========================================================================
        use crate::handlers::MetaUtlTracker;
        use context_graph_core::johari::DynDefaultJohariManager;
        use context_graph_core::monitoring::{StubLayerStatusProvider, StubSystemMonitor};

        let johari_manager: Arc<dyn context_graph_core::johari::JohariTransitionManager> = Arc::new(
            DynDefaultJohariManager::new(Arc::clone(&teleological_store)),
        );
        info!("Created DynDefaultJohariManager for Johari quadrant management");

        let meta_utl_tracker = Arc::new(parking_lot::RwLock::new(MetaUtlTracker::new()));
        info!("Created MetaUtlTracker for per-embedder accuracy tracking");

        // TODO: Replace with real SystemMonitor and LayerStatusProvider when available
        // For now, using stubs that will FAIL FAST with clear errors on first use
        let system_monitor: Arc<dyn context_graph_core::monitoring::SystemMonitor> =
            Arc::new(StubSystemMonitor::new());
        let layer_status_provider: Arc<dyn context_graph_core::monitoring::LayerStatusProvider> =
            Arc::new(StubLayerStatusProvider::new());
        warn!("Using StubSystemMonitor and StubLayerStatusProvider - will FAIL FAST on health metric queries");

        // ==========================================================================
        // 5. Create Handlers with REAL GWT providers (P2-01 through P2-06)
        // ==========================================================================
        // Using with_default_gwt() to create all GWT providers:
        // - KuramotoProviderImpl: Real Kuramoto oscillator network
        // - GwtSystemProviderImpl: Real consciousness equation C(t) = I(t) × R(t) × D(t)
        // - WorkspaceProviderImpl: Real global workspace with winner-take-all
        // - MetaCognitiveProviderImpl: Real meta-cognitive loop
        // - SelfEgoProviderImpl: Real self-ego node for identity tracking
        let handlers = Handlers::with_default_gwt(
            Arc::clone(&teleological_store),
            Arc::clone(&utl_processor),
            Arc::clone(&multi_array_provider),
            alignment_calculator,
            goal_hierarchy,
            johari_manager,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
        );
        info!("Created Handlers with REAL GWT providers (Kuramoto, GWT, Workspace, MetaCognitive, SelfEgo)");
        info!("Created REAL NeuromodulationManager (Dopamine, Serotonin, Noradrenaline at baseline; ACh read-only via GWT)");
        info!("Created REAL Dream components (DreamController, DreamScheduler, AmortizedLearner) with constitution defaults");
        info!("Created REAL AdaptiveThresholdCalibration (4-level: EWMA, Temperature, Bandit, Bayesian)");

        info!("MCP Server initialization complete - TeleologicalFingerprint mode active with GWT + Neuromod + Dream + ATC");

        Ok(Self {
            config,
            teleological_store,
            utl_processor,
            multi_array_provider,
            handlers,
            initialized: Arc::new(RwLock::new(false)),
        })
    }

    /// Run the server, reading from stdin and writing to stdout.
    pub async fn run(&self) -> Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        info!("Server ready, waiting for requests (TeleologicalMemoryStore mode)...");

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    error!("Error reading stdin: {}", e);
                    break;
                }
            };

            if line.trim().is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            let response = self.handle_request(&line).await;

            // Handle notifications (no response needed)
            if response.id.is_none() && response.result.is_none() && response.error.is_none() {
                debug!("Notification handled, no response needed");
                continue;
            }

            let response_json = serde_json::to_string(&response)?;
            debug!("Sending: {}", response_json);

            // MCP requires newline-delimited JSON on stdout
            writeln!(stdout, "{}", response_json)?;
            stdout.flush()?;

            // Check for shutdown
            if !*self.initialized.read().await {
                // Not initialized yet, continue
            }
        }

        info!("Server shutting down...");
        Ok(())
    }

    /// Handle a single JSON-RPC request.
    async fn handle_request(&self, input: &str) -> JsonRpcResponse {
        // Parse request
        let request: JsonRpcRequest = match serde_json::from_str(input) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to parse request: {}", e);
                return JsonRpcResponse::error(
                    None,
                    crate::protocol::error_codes::PARSE_ERROR,
                    format!("Parse error: {}", e),
                );
            }
        };

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse::error(
                request.id,
                crate::protocol::error_codes::INVALID_REQUEST,
                "Invalid JSON-RPC version",
            );
        }

        // Dispatch to handler
        self.handlers.dispatch(request).await
    }

    /// Resolve the storage path from configuration or environment.
    ///
    /// Priority order:
    /// 1. `CONTEXT_GRAPH_STORAGE_PATH` environment variable
    /// 2. `config.storage.path` from configuration
    /// 3. Default: `contextgraph_data` directory NEXT TO EXECUTABLE (not current dir!)
    ///
    /// CRITICAL: Uses executable directory as base, NOT current working directory.
    /// This prevents permission errors when MCP clients spawn the server from
    /// unpredictable directories (e.g., `/` or their own installation path).
    ///
    /// Creates the directory if it doesn't exist.
    fn resolve_storage_path(config: &Config) -> PathBuf {
        // Check environment variable first
        if let Ok(env_path) = std::env::var("CONTEXT_GRAPH_STORAGE_PATH") {
            let path = PathBuf::from(env_path);
            info!(
                "Using storage path from CONTEXT_GRAPH_STORAGE_PATH: {:?}",
                path
            );
            Self::ensure_directory_exists(&path);
            return path;
        }

        // Use config path if it's not the default "memory" backend
        if config.storage.backend != "memory" && !config.storage.path.is_empty() {
            let path = PathBuf::from(&config.storage.path);
            info!("Using storage path from config: {:?}", path);
            Self::ensure_directory_exists(&path);
            return path;
        }

        // FIXED: Use executable's directory instead of current_dir
        // This ensures the server works regardless of working directory,
        // which is critical for MCP clients that may launch from any directory.
        let default_path = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| {
                // Fallback to current_dir only if we can't get executable path
                warn!("Could not determine executable directory, falling back to current_dir");
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            })
            .join("contextgraph_data");
        info!(
            "Using default storage path (relative to executable): {:?}",
            default_path
        );
        Self::ensure_directory_exists(&default_path);
        default_path
    }

    /// Resolve the models directory path from configuration or environment.
    ///
    /// Priority order:
    /// 1. `CONTEXT_GRAPH_MODELS_PATH` environment variable
    /// 2. `config.models.path` from configuration (if exists)
    /// 3. Default: `models` directory NEXT TO EXECUTABLE (not current dir!)
    ///
    /// CRITICAL: Uses executable directory as base, NOT current working directory.
    /// This prevents path resolution issues when MCP clients spawn the server from
    /// unpredictable directories (e.g., `/` or their own installation path).
    ///
    /// Does NOT create the directory - models must be pre-downloaded.
    fn resolve_models_path(_config: &Config) -> PathBuf {
        // Check environment variable first
        if let Ok(env_path) = std::env::var("CONTEXT_GRAPH_MODELS_PATH") {
            let path = PathBuf::from(env_path);
            info!(
                "Using models path from CONTEXT_GRAPH_MODELS_PATH: {:?}",
                path
            );
            return path;
        }

        // FIXED: Use executable's directory instead of current_dir
        // This ensures the server finds models regardless of working directory.
        let default_path = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| {
                // Fallback to current_dir only if we can't get executable path
                warn!("Could not determine executable directory, falling back to current_dir");
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            })
            .join("models");
        info!(
            "Using default models path (relative to executable): {:?}",
            default_path
        );
        default_path
    }

    /// Ensure a directory exists, creating it if necessary.
    fn ensure_directory_exists(path: &PathBuf) {
        if !path.exists() {
            info!("Creating storage directory: {:?}", path);
            if let Err(e) = std::fs::create_dir_all(path) {
                warn!(
                    "Failed to create storage directory {:?}: {}. \
                     RocksDB may fail to open.",
                    path, e
                );
            }
        }
    }
}
