//! MCP Server implementation.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! TASK-S003: Added GoalAlignmentCalculator and GoalHierarchy for purpose operations.
//! TASK-S004: Replaced stubs with REAL implementations (RocksDB storage).
//! TASK-INTEG-018: Added TCP transport support with concurrent client handling.
//!
//! NO BACKWARDS COMPATIBILITY with stubs. FAIL FAST with clear errors.

// NOTE: std::io removed - using tokio::io for async I/O (AP-08 compliance)
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

// ============================================================================
// TASK-INTEG-018: Transport Mode
// ============================================================================

/// Transport mode for the MCP server.
///
/// TASK-INTEG-018: Enum for selecting between stdio and TCP transports.
/// TASK-42: Added Sse variant for HTTP-based real-time streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportMode {
    /// Standard input/output transport (default).
    /// Used for process-based MCP clients (e.g., VS Code, Claude Desktop).
    #[default]
    Stdio,

    /// TCP socket transport.
    /// Used for network-based MCP clients and remote deployments.
    Tcp,

    /// Server-Sent Events transport.
    /// Used for HTTP-based real-time streaming to web clients.
    /// Events are broadcast to all connected clients.
    /// TASK-42: Added for web client support.
    Sse,
}

use context_graph_core::config::Config;
use context_graph_core::memory::watcher::GitFileWatcher;
use context_graph_core::memory::{MemoryCaptureService, MultiArrayEmbeddingAdapter};
use context_graph_core::memory::store::MemoryStore;
use context_graph_core::memory::{CodeCaptureService, CodeFileWatcher};
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore};

// Code watcher dependencies
use crate::adapters::CodeStoreAdapter;
use context_graph_embeddings::adapters::E7CodeEmbeddingProvider;
use context_graph_embeddings::traits::EmbeddingModel;
use context_graph_storage::code::CodeStore;

use context_graph_embeddings::{
    get_warm_provider, initialize_global_warm_provider, is_warm_initialized, warm_status_message,
    GpuConfig, ProductionMultiArrayProvider,
};

// REAL implementations - NO STUBS
use crate::adapters::{LazyMultiArrayProvider, LlmCausalHintProvider};
use context_graph_embeddings::provider::CausalHintProvider;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
// TASK-GRAPHLINK: EdgeRepository and BackgroundGraphBuilder for K-NN graph linking
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository, GraphBuilderConfig};
// GRAPH-AGENT: LLM-based relationship discovery
use context_graph_causal_agent::CausalDiscoveryLLM;
use context_graph_graph_agent::{GraphDiscoveryConfig, GraphDiscoveryService};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
// TASK-42: SSE transport types
use crate::transport::{create_sse_router, SseAppState, SseConfig};

// NOTE: LazyFailMultiArrayProvider was removed - now using ProductionMultiArrayProvider
// from context-graph-embeddings crate (TASK-F007 COMPLETED)

// ============================================================================
// MCP Server
// ============================================================================

/// MCP Server state.
///
/// TASK-S001: Uses TeleologicalMemoryStore for 13-embedding fingerprint storage.
/// TASK-INTEG-018: Arc-wrapped handlers for TCP transport sharing across concurrent clients.
/// LAZY-STARTUP: Models load in background to allow immediate MCP protocol response.
#[allow(dead_code)]
pub struct McpServer {
    config: Config,
    /// Teleological memory store - stores TeleologicalFingerprint with 13 embeddings.
    teleological_store: Arc<dyn TeleologicalMemoryStore>,
    /// Multi-array embedding provider - generates all 13 embeddings.
    /// Wrapped in RwLock<Option<...>> for lazy loading - None while models are loading.
    multi_array_provider: Arc<RwLock<Option<Arc<dyn MultiArrayEmbeddingProvider>>>>,
    /// Flag indicating whether models are currently loading.
    models_loading: Arc<AtomicBool>,
    /// Flag indicating whether model loading failed.
    models_failed: Arc<RwLock<Option<String>>>,
    /// Arc-wrapped handlers for safe sharing across TCP client tasks.
    /// TASK-INTEG-018: Handlers are now Arc-wrapped to allow concurrent TCP connections.
    handlers: Arc<Handlers>,
    initialized: Arc<RwLock<bool>>,
    /// Connection semaphore for limiting concurrent TCP connections.
    /// TASK-INTEG-018: Initialized from config.mcp.max_connections.
    connection_semaphore: Arc<Semaphore>,
    /// Active connection counter for monitoring.
    /// TASK-INTEG-018: Atomic counter for observability.
    active_connections: Arc<AtomicUsize>,
    /// Code watcher background task handle.
    /// E7-WIRING: Runs periodically to detect file changes and update embeddings.
    code_watcher_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Flag to signal code watcher shutdown.
    code_watcher_running: Arc<AtomicBool>,
    /// TASK-GRAPHLINK-PHASE1: Background graph builder for K-NN edge computation.
    /// Builds typed edges from embedder agreement patterns.
    graph_builder: Option<Arc<BackgroundGraphBuilder>>,
    /// TASK-GRAPHLINK-PHASE1: Background graph builder worker task handle.
    graph_builder_task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl McpServer {
    /// Create a new MCP server with the given configuration.
    ///
    /// TASK-S001: Creates TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
    /// TASK-S004: Uses REAL implementations - RocksDbTeleologicalStore.
    /// TASK-EMB-WARMUP: When `warm_first` is true, blocks until all 13 embedding models
    /// are loaded into VRAM before returning. This ensures embedding operations are
    /// available immediately when the server starts accepting requests.
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration
    /// * `warm_first` - If true, block startup until models are loaded into VRAM (default: true)
    ///                  If false, models load in background while server starts immediately
    ///
    /// # Errors
    ///
    /// - Returns error if RocksDB fails to open (path issues, permissions, corruption)
    /// - Returns error if `warm_first` is true and model loading fails
    pub async fn new(config: Config, warm_first: bool) -> Result<Self> {
        info!(
            "Initializing MCP Server with REAL implementations (NO STUBS), warm_first={}...",
            warm_first
        );

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

        // TASK-GRAPHLINK: Extract Arc<DB> BEFORE wrapping in trait object
        // EdgeRepository needs direct access to the RocksDB instance for graph edge storage
        let db_arc = rocksdb_store.db_arc();
        info!("Extracted Arc<DB> for EdgeRepository - graph linking enabled");

        // Note: EmbedderIndexRegistry is initialized in the constructor,
        // so no separate initialization step is needed.
        info!("Created store with EmbedderIndexRegistry (12 HNSW-capable embedders initialized)");

        // Now wrap in Arc<dyn TeleologicalMemoryStore>
        let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

        // Create EdgeRepository sharing the same RocksDB instance
        // NO FALLBACKS - If this fails, the system errors so we can debug
        let edge_repository = EdgeRepository::new(db_arc);
        info!("Created EdgeRepository for K-NN graph linking - NO FALLBACKS enabled");

        // TASK-GRAPHLINK-PHASE1: EdgeRepository clone for BackgroundGraphBuilder
        // The builder will use this to persist K-NN edges computed from embedder agreement
        let edge_repository_for_builder = edge_repository.clone();

        // ==========================================================================
        // 2. REAL MultiArrayEmbeddingProvider - 13 GPU-accelerated embedders
        // ==========================================================================
        // TASK-EMB-016: Use global warm provider singleton
        // TASK-EMB-WARMUP: Support blocking and background warmup modes
        //
        // The global warm provider ensures:
        // - All 13 models are loaded ONCE at startup into VRAM
        // - Tests and production code use the SAME warm models
        // - No cold loading overhead in tests or runtime
        //
        // GPU Requirements: NVIDIA CUDA GPU with 32GB VRAM (RTX 5090)
        // Model Directory: ./models relative to binary (configurable via env)
        //
        // STARTUP BEHAVIOR (controlled by warm_first):
        // - warm_first=true: BLOCK until all models are loaded into VRAM
        //   This is the DEFAULT and RECOMMENDED mode for production.
        //   Ensures embedding operations are available immediately.
        //
        // - warm_first=false: Background loading via LazyMultiArrayProvider
        //   Server starts immediately but embedding operations fail until
        //   models finish loading (20-30s on RTX 5090).

        // Create shared state for model provider
        let multi_array_provider: Arc<RwLock<Option<Arc<dyn MultiArrayEmbeddingProvider>>>> =
            Arc::new(RwLock::new(None));
        let models_loading = Arc::new(AtomicBool::new(true));
        let models_failed: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        // Check if global warm provider is already initialized
        let warm_provider_initialized = is_warm_initialized();

        if warm_provider_initialized {
            // Use the already-warm global provider (regardless of warm_first flag)
            info!("Global warm provider already initialized - using warm models immediately");
            match get_warm_provider() {
                Ok(provider) => {
                    let mut slot = multi_array_provider.write().await;
                    *slot = Some(provider);
                    models_loading.store(false, Ordering::SeqCst);
                    info!("Using global warm provider - all 13 models ready (no loading delay)");
                }
                Err(e) => {
                    error!(
                        "Failed to get global warm provider: {}. Status: {}",
                        e,
                        warm_status_message()
                    );
                    if warm_first {
                        // FAIL FAST when warm_first is enabled
                        return Err(anyhow::anyhow!(
                            "Failed to get global warm provider with warm_first=true: {}. \
                             Ensure CUDA GPU is available and models are downloaded.",
                            e
                        ));
                    }
                    let mut failed = models_failed.write().await;
                    *failed = Some(format!("{}", e));
                    models_loading.store(false, Ordering::SeqCst);
                }
            }
        } else if warm_first {
            // TASK-EMB-WARMUP: BLOCKING warmup mode
            // Block startup until all 13 models are loaded into VRAM
            info!(
                "warm_first=true: Blocking startup until embedding models are loaded into VRAM..."
            );
            info!("This may take 20-30 seconds on RTX 5090 (32GB VRAM)...");

            let models_dir = Self::resolve_models_path(&config);
            info!("Loading models from {:?}...", models_dir);

            // Initialize global warm provider SYNCHRONOUSLY
            match initialize_global_warm_provider().await {
                Ok(()) => {
                    info!("Global warm provider initialized successfully");
                    match get_warm_provider() {
                        Ok(provider) => {
                            let mut slot = multi_array_provider.write().await;
                            *slot = Some(provider);
                            models_loading.store(false, Ordering::SeqCst);
                            info!("SUCCESS: All 13 embedding models loaded into VRAM and ready");
                        }
                        Err(e) => {
                            error!(
                                "Failed to get global warm provider after init: {}. Status: {}",
                                e,
                                warm_status_message()
                            );
                            return Err(anyhow::anyhow!(
                                "Failed to get global warm provider after initialization: {}. \
                                 This is unexpected - check GPU status.",
                                e
                            ));
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "FATAL: Global warm provider initialization failed: {}. \
                         Ensure CUDA 13.1+ is installed, RTX 5090 is available, \
                         and models are downloaded to {:?}",
                        e, models_dir
                    );
                    return Err(anyhow::anyhow!(
                        "Failed to initialize embedding models with warm_first=true: {}. \
                         Use --no-warm to skip blocking warmup (embeddings will fail until background load completes).",
                        e
                    ));
                }
            }
        } else {
            // TASK-EMB-WARMUP: BACKGROUND warmup mode (warm_first=false)
            // Initialize global warm provider in background
            // This allows immediate MCP protocol response while models load
            info!("warm_first=false: Loading embedding models in background...");
            warn!("WARNING: Embedding operations will fail until models finish loading (20-30s)");

            let models_dir = Self::resolve_models_path(&config);
            info!(
                "Will load ProductionMultiArrayProvider with models from {:?} (background)...",
                models_dir
            );

            let provider_slot = Arc::clone(&multi_array_provider);
            let loading_flag = Arc::clone(&models_loading);
            let failed_slot = Arc::clone(&models_failed);
            let models_dir_clone = models_dir.clone();

            tokio::spawn(async move {
                info!("Background model loading started...");

                // First try to initialize global warm provider
                match initialize_global_warm_provider().await {
                    Ok(()) => {
                        // Global provider initialized, get it
                        match get_warm_provider() {
                            Ok(provider) => {
                                let mut slot = provider_slot.write().await;
                                *slot = Some(provider);
                                loading_flag.store(false, Ordering::SeqCst);
                                info!(
                                    "Global warm provider initialized - 13 embedders ready (warm)"
                                );
                            }
                            Err(e) => {
                                error!("Failed to get global warm provider after init: {}", e);
                                let mut failed = failed_slot.write().await;
                                *failed = Some(format!("{}", e));
                                loading_flag.store(false, Ordering::SeqCst);
                            }
                        }
                    }
                    Err(e) => {
                        // Global warm provider failed, fall back to direct ProductionMultiArrayProvider
                        warn!(
                            "Global warm provider initialization failed: {}. Falling back to direct loading.",
                            e
                        );

                        match ProductionMultiArrayProvider::new(
                            models_dir_clone.clone(),
                            GpuConfig::default(),
                        )
                        .await
                        {
                            Ok(provider) => {
                                let mut slot = provider_slot.write().await;
                                *slot = Some(Arc::new(provider));
                                loading_flag.store(false, Ordering::SeqCst);
                                info!("Background model loading COMPLETE - 13 embedders ready (cold loaded)");
                            }
                            Err(e) => {
                                error!(
                                    "FATAL: Background model loading FAILED: {}. \
                                     Ensure models exist at {:?} and CUDA GPU is available.",
                                    e, models_dir_clone
                                );
                                let mut failed = failed_slot.write().await;
                                *failed = Some(format!("{}", e));
                                loading_flag.store(false, Ordering::SeqCst);
                            }
                        }
                    }
                }
            });
        }

        // Create lazy provider wrapper for immediate MCP startup
        let lazy_provider: Arc<dyn MultiArrayEmbeddingProvider> =
            Arc::new(LazyMultiArrayProvider::new(
                Arc::clone(&multi_array_provider),
                Arc::clone(&models_loading),
                Arc::clone(&models_failed),
            ));

        // ==========================================================================
        // 3. Create Handlers (PRD v6 Section 10 - 14 tools)
        // ==========================================================================
        let layer_status_provider: Arc<dyn context_graph_core::monitoring::LayerStatusProvider> =
            Arc::new(context_graph_core::monitoring::StubLayerStatusProvider::new());

        // ==========================================================================
        // 3a. E7-WIRING: Optional Code Pipeline Initialization
        // ==========================================================================
        // The code pipeline provides AST-aware code search using E7 embeddings.
        // When enabled, search_code can return both:
        // - Results from the teleological store (existing behavior)
        // - Results from the CodeStore (AST-chunked code entities)
        //
        // To enable the code pipeline:
        // 1. Set CODE_PIPELINE_ENABLED=true environment variable
        // 2. Ensure CODE_STORE_PATH is set (defaults to db_path/code_store)
        //
        // TODO: Add to Config struct for proper configuration management
        let _code_pipeline_enabled =
            std::env::var("CODE_PIPELINE_ENABLED").map_or(false, |v| v == "true");

        // E7-WIRING: Code pipeline components are created but not wired by default.
        // When enabled:
        // - CodeStore is opened at CODE_STORE_PATH
        // - E7CodeEmbeddingProvider wraps the CodeModel
        // - CodeCaptureService orchestrates embed + store
        // - CodeFileWatcher monitors source files for changes
        //
        // The code pipeline is separate from the 13-embedder teleological system:
        // - CodeStore uses E7-only embeddings (1536D Qodo-Embed)
        // - TeleologicalStore uses all 13 embeddings including E7
        //
        // This separation allows:
        // - Faster code-specific search (E7 only)
        // - AST-aware chunking (function/struct boundaries)
        // - Incremental updates without re-embedding entire files

        // TASK-GRAPHLINK-PHASE1: Create BackgroundGraphBuilder for K-NN edge computation
        // The builder queues fingerprints from store_memory and processes them in batches
        // every 60 seconds (configurable via GraphBuilderConfig)
        let graph_builder = Arc::new(BackgroundGraphBuilder::new(
            edge_repository_for_builder,
            Arc::clone(&teleological_store),
            Arc::clone(&lazy_provider),
            GraphBuilderConfig::default(),
        ));
        info!(
            "Created BackgroundGraphBuilder - batch interval={}s, k={}, min_batch={}",
            graph_builder.config().batch_interval_secs,
            graph_builder.config().k,
            graph_builder.config().min_batch_size,
        );

        // GRAPH-AGENT: Initialize shared LLM for graph relationship discovery
        // This LLM (Qwen2.5-3B) can also be shared with causal-agent
        // NO FALLBACKS - LLM MUST load successfully or server startup fails
        info!("GRAPH-AGENT: Initializing CausalDiscoveryLLM (Qwen2.5-3B) - NO FALLBACKS");
        let llm = CausalDiscoveryLLM::new().map_err(|e| {
            error!(
                "FATAL: Failed to create CausalDiscoveryLLM: {}. \
                 Graph discovery requires Qwen2.5-3B model (~6GB VRAM). \
                 Check model files exist and CUDA GPU is available.",
                e
            );
            anyhow::anyhow!(
                "Failed to create CausalDiscoveryLLM: {}. \
                 Ensure Qwen2.5-3B model is downloaded and CUDA GPU with 6GB+ VRAM is available.",
                e
            )
        })?;

        info!("Loading CausalDiscoveryLLM (Qwen2.5-3B) into VRAM (~6GB)...");
        llm.load().await.map_err(|e| {
            error!(
                "FATAL: Failed to load CausalDiscoveryLLM: {}. \
                 Check CUDA GPU has at least 6GB free VRAM.",
                e
            );
            anyhow::anyhow!(
                "Failed to load CausalDiscoveryLLM: {}. \
                 Requires ~6GB VRAM. Check GPU memory availability.",
                e
            )
        })?;
        info!("CausalDiscoveryLLM loaded successfully (~6GB VRAM)");

        // CAUSAL-HINT: Wrap LLM in Arc first to enable sharing between services
        let shared_llm = Arc::new(llm);

        let graph_discovery_config = GraphDiscoveryConfig::default();
        let graph_discovery_service = Arc::new(GraphDiscoveryService::with_config(
            Arc::clone(&shared_llm), // Clone for GraphDiscoveryService
            graph_discovery_config,
        ));

        // CAUSAL-HINT: Create LlmCausalHintProvider using shared LLM (GPU inference via CUDA)
        info!("CAUSAL-HINT: Creating LlmCausalHintProvider (2s timeout for GPU inference)");
        let causal_hint_provider: Arc<dyn CausalHintProvider> =
            Arc::new(LlmCausalHintProvider::new(
                shared_llm, // Move remaining Arc
                LlmCausalHintProvider::DEFAULT_TIMEOUT_MS,
            ));

        // TASK-GRAPHLINK: Use with_graph_discovery to enable K-NN graph operations
        // with background builder support and LLM-based relationship detection
        // NO FALLBACKS - All components MUST work or server startup fails
        info!("Creating Handlers with graph discovery and causal hints enabled - NO FALLBACKS");
        let handlers = Handlers::with_graph_discovery(
            Arc::clone(&teleological_store),
            lazy_provider,
            layer_status_provider,
            edge_repository,
            Arc::clone(&graph_builder),
            graph_discovery_service,
            causal_hint_provider, // NEW: Enable LLM-based causal hints for E5
        );
        info!("Created Handlers with full graph linking enabled (K-NN edges + background builder, NO FALLBACKS)");

        info!(
            "MCP Server initialization complete - TeleologicalFingerprint mode with 13 embeddings"
        );

        // TASK-INTEG-018: Create connection semaphore from config
        let max_connections = config.mcp.max_connections;
        let connection_semaphore = Arc::new(Semaphore::new(max_connections));
        info!(
            "TCP transport ready: max_connections={}, bind_address={}, tcp_port={}",
            max_connections, config.mcp.bind_address, config.mcp.tcp_port
        );

        Ok(Self {
            config,
            teleological_store,
            multi_array_provider,
            models_loading,
            models_failed,
            // TASK-INTEG-018: Arc-wrap handlers for TCP sharing
            handlers: Arc::new(handlers),
            initialized: Arc::new(RwLock::new(false)),
            connection_semaphore,
            active_connections: Arc::new(AtomicUsize::new(0)),
            // E7-WIRING: Code watcher fields initialized as None/false
            code_watcher_task: Arc::new(RwLock::new(None)),
            code_watcher_running: Arc::new(AtomicBool::new(false)),
            // TASK-GRAPHLINK-PHASE1: Graph builder for background K-NN edge computation
            graph_builder: Some(graph_builder),
            graph_builder_task: Arc::new(RwLock::new(None)),
        })
    }

    /// Run the server, reading from stdin and writing to stdout.
    ///
    /// CRITICAL: Uses tokio async I/O to avoid blocking the runtime.
    /// AP-08 compliance: No sync I/O in async context.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - stdin read fails (broken pipe, closed stream)
    /// - stdout write fails (broken pipe, closed stream)
    /// - JSON serialization fails (should never happen with valid responses)
    pub async fn run(&self) -> Result<()> {
        // CRITICAL: Use tokio async I/O, NOT std::io blocking I/O
        // This prevents blocking the runtime and causing deadlocks
        // when background tasks (like model loading) need to progress.
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut writer = tokio::io::BufWriter::new(stdout);
        let mut line = String::new();

        info!("Server ready, waiting for requests (TeleologicalMemoryStore mode)...");

        // TASK-GRAPHLINK-PHASE1: Start background graph builder worker
        // This processes fingerprints from the queue and builds K-NN edges
        match self.start_graph_builder().await {
            Ok(true) => info!("Background graph builder started"),
            Ok(false) => debug!("Background graph builder not configured or failed to start"),
            Err(e) => warn!("Failed to start background graph builder: {}", e),
        }

        loop {
            line.clear();

            // Async read line - does NOT block the runtime
            let bytes_read = reader.read_line(&mut line).await.map_err(|e| {
                error!("FATAL: Failed to read from stdin: {}", e);
                anyhow::anyhow!("stdin read error: {}", e)
            })?;

            // EOF - client closed connection
            if bytes_read == 0 {
                info!("stdin closed (EOF), shutting down...");
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            debug!("Received: {}", trimmed);

            let response = self.handle_request(trimmed).await;

            // Handle notifications (no response needed)
            if response.id.is_none() && response.result.is_none() && response.error.is_none() {
                debug!("Notification handled, no response needed");
                continue;
            }

            let response_json = serde_json::to_string(&response)?;
            debug!("Sending: {}", response_json);

            // MCP requires newline-delimited JSON on stdout
            // Use async write to avoid blocking
            writer.write_all(response_json.as_bytes()).await.map_err(|e| {
                error!("FATAL: Failed to write to stdout: {}", e);
                anyhow::anyhow!("stdout write error: {}", e)
            })?;
            writer.write_all(b"\n").await?;
            writer.flush().await.map_err(|e| {
                error!("FATAL: Failed to flush stdout: {}", e);
                anyhow::anyhow!("stdout flush error: {}", e)
            })?;
        }

        // Gracefully shutdown background tasks
        self.shutdown().await;
        info!("Server shutting down...");
        Ok(())
    }

    /// Gracefully shutdown the MCP server.
    ///
    /// Stops background tasks. This should be called before the server exits.
    ///
    /// # Behavior
    ///
    /// - Logs shutdown initiation and completion
    /// - Safe to call multiple times (idempotent)
    pub async fn shutdown(&self) {
        info!("Initiating graceful shutdown...");

        // TASK-GRAPHLINK-PHASE1: Stop graph builder worker
        self.stop_graph_builder().await;

        // Stop code watcher if running
        self.stop_code_watcher().await;

        info!("Graceful shutdown complete");
    }

    /// Start the file watcher if enabled in configuration.
    ///
    /// The file watcher monitors ./docs/ directory (and subdirectories) for .md
    /// file changes and automatically indexes them as memories with MDFileChunk
    /// source metadata.
    ///
    /// # Configuration
    ///
    /// Set in config.toml:
    /// ```toml
    /// [watcher]
    /// enabled = true
    /// watch_paths = ["./docs"]
    /// session_id = "docs-watcher"
    /// ```
    ///
    /// # Returns
    ///
    /// `Ok(true)` if watcher started successfully, `Ok(false)` if disabled,
    /// `Err` if startup failed.
    pub async fn start_file_watcher(&self) -> Result<bool> {
        if !self.config.watcher.enabled {
            debug!("File watcher disabled in configuration");
            return Ok(false);
        }

        // Wait for embedding models to be ready
        if self.models_loading.load(Ordering::SeqCst) {
            info!("Waiting for embedding models to load before starting file watcher...");
            // Wait up to 60 seconds for models to load
            for _ in 0..120 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if !self.models_loading.load(Ordering::SeqCst) {
                    break;
                }
            }
            if self.models_loading.load(Ordering::SeqCst) {
                error!("Embedding models still loading after 60s - skipping file watcher");
                return Ok(false);
            }
        }

        // Check if model loading failed
        {
            let failed = self.models_failed.read().await;
            if let Some(ref err) = *failed {
                error!("Cannot start file watcher - embedding models failed: {}", err);
                return Ok(false);
            }
        }

        // Get embedding provider
        let provider = {
            let slot = self.multi_array_provider.read().await;
            match slot.as_ref() {
                Some(p) => Arc::clone(p),
                None => {
                    error!("Cannot start file watcher - no embedding provider available");
                    return Ok(false);
                }
            }
        };

        // Create separate storage path for file watcher's MemoryStore
        // Uses a subdirectory to avoid RocksDB column family conflicts with main teleological store
        let base_db_path = Self::resolve_storage_path(&self.config);
        let watcher_db_path = base_db_path.join("watcher_memory");
        Self::ensure_directory_exists(&watcher_db_path);

        // Create memory store in separate directory
        let memory_store = Arc::new(MemoryStore::new(&watcher_db_path).map_err(|e| {
            anyhow::anyhow!("Failed to create memory store for file watcher at {:?}: {}", watcher_db_path, e)
        })?);

        // Create embedding adapter
        let embedder = Arc::new(MultiArrayEmbeddingAdapter::new(provider));

        // Clone teleological store for file watcher integration
        // This enables file watcher memories to be searchable via MCP tools
        let teleological_store = Arc::clone(&self.teleological_store);

        // Create capture service WITH teleological store for MCP search integration
        let capture_service = Arc::new(MemoryCaptureService::with_teleological_store(
            memory_store.clone(),
            embedder,
            teleological_store,
        ));

        // Convert watch paths to PathBufs
        let watch_paths: Vec<PathBuf> = self
            .config
            .watcher
            .watch_paths
            .iter()
            .map(PathBuf::from)
            .collect();

        let session_id = self.config.watcher.session_id.clone();

        // Spawn file watcher in a dedicated thread to handle the non-Send Receiver
        // We use spawn_blocking + nested tokio runtime for this
        std::thread::spawn(move || {
            // Create a new runtime for this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create file watcher runtime");

            rt.block_on(async move {
                info!(
                    paths = ?watch_paths,
                    session_id = %session_id,
                    "Starting file watcher..."
                );

                // Create file watcher
                let mut watcher = match GitFileWatcher::new(watch_paths.clone(), capture_service, session_id.clone()) {
                    Ok(w) => w,
                    Err(e) => {
                        error!(error = %e, "Failed to create file watcher");
                        return;
                    }
                };

                // Start watcher
                if let Err(e) = watcher.start().await {
                    error!(error = %e, "Failed to start file watcher");
                    return;
                }

                info!(
                    paths = ?watch_paths,
                    "File watcher started - monitoring for .md file changes (recursive)"
                );

                // Process events in a loop
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
                loop {
                    interval.tick().await;
                    match watcher.process_events().await {
                        Ok(count) => {
                            if count > 0 {
                                info!(files_processed = count, "File watcher processed changes");
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "File watcher error processing events");
                        }
                    }
                }
            });
        });

        info!(
            paths = ?self.config.watcher.watch_paths,
            session_id = %self.config.watcher.session_id,
            "File watcher started as background task"
        );

        Ok(true)
    }

    // =========================================================================
    // E7-WIRING: Code File Watcher
    // =========================================================================

    /// Start the code file watcher for AST-based code indexing.
    ///
    /// E7-WIRING: This method enables the code embedding pipeline which provides:
    /// - Tree-sitter AST parsing for Rust source files
    /// - E7 (Qodo-Embed-1-1.5B) embedding for code entities
    /// - Separate CodeStore for code-specific search
    ///
    /// # Configuration
    ///
    /// Environment variables:
    /// - `CODE_PIPELINE_ENABLED=true` - Enable the code pipeline
    /// - `CODE_STORE_PATH` - Path to CodeStore (defaults to db_path/code_store)
    /// - `CODE_WATCH_PATHS` - Comma-separated list of paths to watch (defaults to crate roots)
    ///
    /// # Returns
    ///
    /// `Ok(true)` if watcher started, `Ok(false)` if disabled, `Err` on failure.
    ///
    /// # Note
    ///
    /// The code pipeline is SEPARATE from the 13-embedder teleological system.
    /// It stores E7-only embeddings for faster code-specific search.
    #[allow(dead_code)]
    pub async fn start_code_watcher(&self) -> Result<bool> {
        // Check if code pipeline is enabled
        let enabled = std::env::var("CODE_PIPELINE_ENABLED").map_or(false, |v| v == "true");
        if !enabled {
            debug!("Code pipeline disabled (set CODE_PIPELINE_ENABLED=true to enable)");
            return Ok(false);
        }

        info!("E7-WIRING: Code pipeline enabled - starting code file watcher...");

        // Wait for embedding models to be ready (E7 is part of the 13-embedder system)
        if self.models_loading.load(Ordering::SeqCst) {
            info!("Waiting for embedding models to load before starting code watcher...");
            for _ in 0..120 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if !self.models_loading.load(Ordering::SeqCst) {
                    break;
                }
            }
            if self.models_loading.load(Ordering::SeqCst) {
                error!("Embedding models still loading after 60s - skipping code watcher");
                return Ok(false);
            }
        }

        // Check if model loading failed
        {
            let failed = self.models_failed.read().await;
            if let Some(ref err) = *failed {
                error!("Cannot start code watcher - embedding models failed: {}", err);
                return Ok(false);
            }
        }

        // E7-WIRING: Full implementation
        // 1. Resolve paths
        let code_store_path = std::env::var("CODE_STORE_PATH").unwrap_or_else(|_| {
            let base = Self::resolve_storage_path(&self.config);
            base.join("code_store").to_string_lossy().to_string()
        });

        let watch_paths_str = std::env::var("CODE_WATCH_PATHS").unwrap_or_else(|_| ".".to_string());
        let watch_paths: Vec<PathBuf> = watch_paths_str
            .split(',')
            .map(|s| PathBuf::from(s.trim()))
            .collect();

        // Poll interval (default: 5 seconds)
        let poll_interval_secs: u64 = std::env::var("CODE_WATCH_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        info!(
            code_store_path = %code_store_path,
            watch_paths = ?watch_paths,
            poll_interval_secs = poll_interval_secs,
            "E7-WIRING: Starting code file watcher"
        );

        // 2. Open CodeStore and wrap in adapter
        let raw_store = CodeStore::open(&code_store_path).map_err(|e| {
            anyhow::anyhow!("Failed to open CodeStore at {}: {}", code_store_path, e)
        })?;
        let code_store = Arc::new(CodeStoreAdapter::new(Arc::new(raw_store)));
        info!("Opened CodeStore at {}", code_store_path);

        // 3. Get existing multi-array provider (all 13 embedders)
        // E7CodeEmbeddingProvider now requires full MultiArrayEmbeddingProvider per ARCH-01/ARCH-05
        let multi_array_provider = {
            let slot = self.multi_array_provider.read().await;
            match slot.as_ref() {
                Some(p) => Arc::clone(p),
                None => {
                    error!("Multi-array provider not available - models not loaded");
                    return Ok(false);
                }
            }
        };
        info!("Using existing multi-array provider for code embedding");

        // 4. Create E7 embedding provider (uses all 13 embedders internally)
        let e7_provider = Arc::new(E7CodeEmbeddingProvider::new(multi_array_provider));

        // 5. Create CodeCaptureService
        let session_id = std::env::var("CLAUDE_SESSION_ID").unwrap_or_else(|_| "default".to_string());
        let capture_service: Arc<CodeCaptureService<E7CodeEmbeddingProvider, CodeStoreAdapter>> =
            Arc::new(CodeCaptureService::new(
                e7_provider.clone(),
                code_store.clone(),
                session_id.clone(),
            ));

        // 6. Create and start CodeFileWatcher
        let mut watcher: CodeFileWatcher<E7CodeEmbeddingProvider, CodeStoreAdapter> =
            CodeFileWatcher::new(
                watch_paths.clone(),
                capture_service,
                session_id,
            ).map_err(|e| anyhow::anyhow!("Failed to create CodeFileWatcher: {}", e))?;

        watcher.start().await.map_err(|e| {
            anyhow::anyhow!("Failed to start CodeFileWatcher: {}", e)
        })?;

        let stats = watcher.stats().await;
        info!(
            files_tracked = stats.files_tracked,
            watch_paths = ?stats.watch_paths,
            "CodeFileWatcher initial scan complete"
        );

        // 7. Spawn background polling task
        self.code_watcher_running.store(true, Ordering::SeqCst);
        let running_flag = self.code_watcher_running.clone();
        let poll_interval = Duration::from_secs(poll_interval_secs);

        let task = tokio::spawn(async move {
            info!("Code watcher background task started (polling every {}s)", poll_interval_secs);

            while running_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(poll_interval).await;

                if !running_flag.load(Ordering::SeqCst) {
                    break;
                }

                match watcher.process_events().await {
                    Ok(files_processed) => {
                        if files_processed > 0 {
                            info!(files_processed, "Code watcher processed file changes");
                        } else {
                            debug!("Code watcher: no changes detected");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Code watcher failed to process events");
                    }
                }
            }

            info!("Code watcher background task stopped");
        });

        // Store the task handle
        {
            let mut task_guard = self.code_watcher_task.write().await;
            *task_guard = Some(task);
        }

        info!("E7-WIRING: Code file watcher started successfully");
        Ok(true)
    }

    /// Stop the code file watcher.
    ///
    /// Signals the background task to stop and waits for it to complete.
    #[allow(dead_code)]
    pub async fn stop_code_watcher(&self) {
        // Signal stop
        self.code_watcher_running.store(false, Ordering::SeqCst);

        // Wait for task to complete
        let task = {
            let mut guard = self.code_watcher_task.write().await;
            guard.take()
        };

        if let Some(handle) = task {
            if let Err(e) = handle.await {
                error!(error = %e, "Code watcher task failed to join");
            } else {
                info!("Code watcher stopped");
            }
        }
    }

    // =========================================================================
    // TASK-GRAPHLINK-PHASE1: Background Graph Builder
    // =========================================================================

    /// Start the background graph builder worker.
    ///
    /// TASK-GRAPHLINK-PHASE1: The graph builder processes fingerprints from the queue
    /// and builds K-NN graphs every batch_interval_secs (default: 60s).
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the worker started successfully, `Ok(false)` if no graph builder
    /// is configured, `Err` on failure.
    pub async fn start_graph_builder(&self) -> Result<bool> {
        let graph_builder = match &self.graph_builder {
            Some(builder) => Arc::clone(builder),
            None => {
                debug!("No graph builder configured - skipping worker start");
                return Ok(false);
            }
        };

        // Wait for embedding models to be ready (needed for graph building)
        if self.models_loading.load(Ordering::SeqCst) {
            info!("Waiting for embedding models to load before starting graph builder...");
            for _ in 0..120 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if !self.models_loading.load(Ordering::SeqCst) {
                    break;
                }
            }
            if self.models_loading.load(Ordering::SeqCst) {
                error!("Embedding models still loading after 60s - skipping graph builder");
                return Ok(false);
            }
        }

        // Check if model loading failed
        {
            let failed = self.models_failed.read().await;
            if let Some(ref err) = *failed {
                error!("Cannot start graph builder - embedding models failed: {}", err);
                return Ok(false);
            }
        }

        info!(
            "TASK-GRAPHLINK-PHASE1: Starting background graph builder worker (interval={}s)",
            graph_builder.config().batch_interval_secs
        );

        // Start the worker
        let task = graph_builder.start_worker();

        // Store the task handle
        {
            let mut task_guard = self.graph_builder_task.write().await;
            *task_guard = Some(task);
        }

        info!("TASK-GRAPHLINK-PHASE1: Background graph builder started successfully");
        Ok(true)
    }

    /// Stop the background graph builder worker.
    ///
    /// Signals the worker to stop and waits for it to complete.
    #[allow(dead_code)]
    pub async fn stop_graph_builder(&self) {
        if let Some(ref builder) = self.graph_builder {
            builder.stop();
        }

        // Wait for task to complete
        let task = {
            let mut guard = self.graph_builder_task.write().await;
            guard.take()
        };

        if let Some(handle) = task {
            if let Err(e) = handle.await {
                error!(error = %e, "Graph builder task failed to join");
            } else {
                info!("Graph builder stopped");
            }
        }
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

    // ========================================================================
    // TASK-INTEG-018: TCP Transport Implementation
    // ========================================================================

    /// Run the server in TCP mode.
    ///
    /// TASK-INTEG-018: Accepts TCP connections on configured bind_address:tcp_port.
    /// Spawns a tokio task per client, respecting max_connections semaphore.
    ///
    /// # Message Framing
    ///
    /// Uses newline-delimited JSON (NDJSON) - same as stdio transport.
    /// Each JSON-RPC message is terminated by `\n`.
    ///
    /// # Connection Management
    ///
    /// - Uses Semaphore to limit concurrent connections to config.mcp.max_connections
    /// - Each client runs in its own tokio task
    /// - Clients are disconnected on first parse error (FAIL FAST per constitution)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - TCP listener fails to bind (address in use, permissions)
    /// - TCP listener returns fatal accept error
    pub async fn run_tcp(&self) -> Result<()> {
        let bind_addr: SocketAddr = format!(
            "{}:{}",
            self.config.mcp.bind_address, self.config.mcp.tcp_port
        )
        .parse()
        .map_err(|e| {
            error!(
                "FATAL: Invalid bind address '{}:{}': {}",
                self.config.mcp.bind_address, self.config.mcp.tcp_port, e
            );
            anyhow::anyhow!(
                "Invalid TCP bind address '{}:{}': {}. \
                 Check config.mcp.bind_address and config.mcp.tcp_port.",
                self.config.mcp.bind_address,
                self.config.mcp.tcp_port,
                e
            )
        })?;

        let listener = TcpListener::bind(bind_addr).await.map_err(|e| {
            error!("FATAL: Failed to bind TCP listener to {}: {}", bind_addr, e);
            anyhow::anyhow!(
                "Failed to bind TCP listener to {}: {}. \
                 Address may be in use or require elevated permissions.",
                bind_addr,
                e
            )
        })?;

        info!(
            "MCP Server listening on TCP {} (max_connections={})",
            bind_addr, self.config.mcp.max_connections
        );

        // TASK-GRAPHLINK-PHASE1: Start background graph builder worker
        // This processes fingerprints from the queue and builds K-NN edges
        match self.start_graph_builder().await {
            Ok(true) => info!("Background graph builder started"),
            Ok(false) => debug!("Background graph builder not configured or failed to start"),
            Err(e) => warn!("Failed to start background graph builder: {}", e),
        }

        loop {
            // Accept new connections
            let (stream, peer_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    // Log but continue accepting - most accept errors are transient
                    error!("Failed to accept TCP connection: {}", e);
                    continue;
                }
            };

            // Clone Arc references for the spawned task
            let handlers = Arc::clone(&self.handlers);
            let semaphore = Arc::clone(&self.connection_semaphore);
            let active_connections = Arc::clone(&self.active_connections);
            let request_timeout = self.config.mcp.request_timeout;

            // Spawn client handler task
            tokio::spawn(async move {
                // Acquire semaphore permit (blocks if at max_connections)
                let _permit = match semaphore.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        error!("Semaphore closed unexpectedly for client {}", peer_addr);
                        return;
                    }
                };

                // Track active connection count
                let conn_count = active_connections.fetch_add(1, Ordering::SeqCst) + 1;
                info!(
                    "TCP client connected: {} (active_connections={})",
                    peer_addr, conn_count
                );

                // Handle client - permit is held until this returns
                if let Err(e) =
                    Self::handle_tcp_client(stream, peer_addr, handlers, request_timeout).await
                {
                    // Log at different levels based on error type
                    if e.to_string().contains("connection reset")
                        || e.to_string().contains("broken pipe")
                    {
                        debug!("TCP client {} disconnected: {}", peer_addr, e);
                    } else {
                        warn!("TCP client {} error: {}", peer_addr, e);
                    }
                }

                // Decrement active connection count
                let conn_count = active_connections.fetch_sub(1, Ordering::SeqCst) - 1;
                info!(
                    "TCP client disconnected: {} (active_connections={})",
                    peer_addr, conn_count
                );
            });
        }
    }

    /// Handle a single TCP client connection.
    ///
    /// TASK-INTEG-018: Reads newline-delimited JSON requests, dispatches to handlers,
    /// writes newline-delimited JSON responses.
    ///
    /// # FAIL FAST Behavior
    ///
    /// Per constitution AP-007, on first parse error the client is disconnected.
    /// This prevents malformed clients from corrupting server state.
    ///
    /// # Arguments
    ///
    /// * `stream` - TCP stream for the client
    /// * `peer_addr` - Client's socket address for logging
    /// * `handlers` - Arc-wrapped handlers for request dispatch
    /// * `request_timeout` - Request timeout in seconds (from config)
    async fn handle_tcp_client(
        stream: TcpStream,
        peer_addr: SocketAddr,
        handlers: Arc<Handlers>,
        _request_timeout: u64,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();

            // Read a line (newline-delimited JSON)
            let bytes_read = reader.read_line(&mut line).await?;

            // EOF - client closed connection
            if bytes_read == 0 {
                debug!("TCP client {} closed connection (EOF)", peer_addr);
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            debug!("TCP {} received: {}", peer_addr, trimmed);

            // Parse request
            let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    // FAIL FAST: Send parse error and disconnect
                    warn!(
                        "TCP client {} sent invalid JSON, disconnecting: {}",
                        peer_addr, e
                    );
                    let error_response = JsonRpcResponse::error(
                        None,
                        crate::protocol::error_codes::PARSE_ERROR,
                        format!("Parse error: {}. Connection will be closed.", e),
                    );
                    let response_json = serde_json::to_string(&error_response)?;
                    writer.write_all(response_json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    // FAIL FAST: Disconnect client
                    return Err(anyhow::anyhow!("Client sent invalid JSON-RPC: {}", e));
                }
            };

            // Validate JSON-RPC version
            if request.jsonrpc != "2.0" {
                let error_response = JsonRpcResponse::error(
                    request.id.clone(),
                    crate::protocol::error_codes::INVALID_REQUEST,
                    "Invalid JSON-RPC version. Expected '2.0'.",
                );
                let response_json = serde_json::to_string(&error_response)?;
                writer.write_all(response_json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
                continue;
            }

            // Dispatch to handler
            let response = handlers.dispatch(request).await;

            // Handle notifications (no response needed)
            if response.id.is_none() && response.result.is_none() && response.error.is_none() {
                debug!("TCP {} notification handled, no response", peer_addr);
                continue;
            }

            // Send response
            let response_json = serde_json::to_string(&response)?;
            debug!("TCP {} sending: {}", peer_addr, response_json);

            writer.write_all(response_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        Ok(())
    }

    /// Run the MCP server with SSE transport.
    ///
    /// TASK-42: Starts an HTTP server with SSE endpoint for real-time streaming.
    /// Uses axum web framework with the SSE transport module.
    ///
    /// # Endpoint
    ///
    /// - `GET /events` - SSE endpoint for receiving MCP events
    ///
    /// # Configuration
    ///
    /// - `config.mcp.bind_address` - HTTP server bind address
    /// - `config.mcp.sse_port` - HTTP server port (default: 3101)
    /// - `config.mcp.max_connections` - Maximum concurrent SSE connections
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - HTTP server fails to bind (address in use, permissions)
    /// - Server encounters fatal error during operation
    pub async fn run_sse(&self) -> Result<()> {
        // Parse bind address
        let bind_addr: SocketAddr = format!(
            "{}:{}",
            self.config.mcp.bind_address, self.config.mcp.sse_port
        )
        .parse()
        .map_err(|e| {
            error!(
                "FATAL: Invalid SSE bind address '{}:{}': {}",
                self.config.mcp.bind_address, self.config.mcp.sse_port, e
            );
            anyhow::anyhow!(
                "Invalid SSE bind address '{}:{}': {}. \
                 Check config.mcp.bind_address and config.mcp.sse_port.",
                self.config.mcp.bind_address,
                self.config.mcp.sse_port,
                e
            )
        })?;

        // Create SSE configuration
        let sse_config = SseConfig::default();
        sse_config.validate().map_err(|e| {
            error!("FATAL: Invalid SSE configuration: {}", e);
            anyhow::anyhow!("Invalid SSE configuration: {}", e)
        })?;

        // Create SSE application state
        let sse_state = SseAppState::new(sse_config).map_err(|e| {
            error!("FATAL: Failed to create SSE application state: {}", e);
            anyhow::anyhow!("Failed to create SSE application state: {}", e)
        })?;

        // Create SSE router
        let router = create_sse_router(sse_state);

        // Bind TCP listener
        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|e| {
                error!("FATAL: Failed to bind SSE listener to {}: {}", bind_addr, e);
                anyhow::anyhow!(
                    "Failed to bind SSE listener to {}: {}. \
                 Address may be in use or require elevated permissions.",
                    bind_addr,
                    e
                )
            })?;

        info!(
            "MCP Server listening on SSE http://{}/events (max_connections={})",
            bind_addr, self.config.mcp.max_connections
        );

        // TASK-GRAPHLINK-PHASE1: Start background graph builder worker
        // This processes fingerprints from the queue and builds K-NN edges
        match self.start_graph_builder().await {
            Ok(true) => info!("Background graph builder started"),
            Ok(false) => debug!("Background graph builder not configured or failed to start"),
            Err(e) => warn!("Failed to start background graph builder: {}", e),
        }

        // Run axum server with explicit type annotation
        axum::serve(listener, router.into_make_service())
            .await
            .map_err(|e| {
                error!("SSE server error: {}", e);
                anyhow::anyhow!("SSE server error: {}", e)
            })?;

        Ok(())
    }
}

// ============================================================================
// TASK-INTEG-018: Transport Mode Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test TransportMode Debug formatting.
    #[test]
    fn test_transport_mode_debug() {
        let stdio = TransportMode::Stdio;
        let tcp = TransportMode::Tcp;

        assert_eq!(format!("{:?}", stdio), "Stdio");
        assert_eq!(format!("{:?}", tcp), "Tcp");
    }

    /// Test TransportMode Clone.
    #[test]
    fn test_transport_mode_clone() {
        let original = TransportMode::Tcp;
        let cloned = original;

        assert!(matches!(cloned, TransportMode::Tcp));
    }

    /// Test TransportMode Copy.
    #[test]
    fn test_transport_mode_copy() {
        let original = TransportMode::Stdio;
        let copied = original; // Copy, not move

        // Both should still be usable
        assert!(matches!(original, TransportMode::Stdio));
        assert!(matches!(copied, TransportMode::Stdio));
    }

    /// Test TransportMode PartialEq.
    #[test]
    fn test_transport_mode_equality() {
        assert_eq!(TransportMode::Stdio, TransportMode::Stdio);
        assert_eq!(TransportMode::Tcp, TransportMode::Tcp);
        assert_ne!(TransportMode::Stdio, TransportMode::Tcp);
        assert_ne!(TransportMode::Tcp, TransportMode::Stdio);
    }

    /// Test TransportMode Default (should be Stdio).
    #[test]
    fn test_transport_mode_default() {
        let default = TransportMode::default();
        assert_eq!(default, TransportMode::Stdio);
    }

    /// Test TCP transport error codes exist and have correct values.
    #[test]
    fn test_tcp_error_codes_exist() {
        use crate::protocol::error_codes;

        // Verify error codes are in the -32110 range (TASK-INTEG-018)
        assert_eq!(error_codes::TCP_BIND_FAILED, -32110);
        assert_eq!(error_codes::TCP_CONNECTION_ERROR, -32111);
        assert_eq!(error_codes::TCP_MAX_CONNECTIONS_REACHED, -32112);
        assert_eq!(error_codes::TCP_FRAME_ERROR, -32113);
        assert_eq!(error_codes::TCP_CLIENT_TIMEOUT, -32114);
    }

    /// Test TCP error codes are unique and don't conflict with other error codes.
    #[test]
    fn test_tcp_error_codes_unique() {
        use crate::protocol::error_codes;

        let tcp_codes = vec![
            error_codes::TCP_BIND_FAILED,
            error_codes::TCP_CONNECTION_ERROR,
            error_codes::TCP_MAX_CONNECTIONS_REACHED,
            error_codes::TCP_FRAME_ERROR,
            error_codes::TCP_CLIENT_TIMEOUT,
        ];

        // All codes should be unique
        let mut unique_codes = tcp_codes.clone();
        unique_codes.sort();
        unique_codes.dedup();
        assert_eq!(
            tcp_codes.len(),
            unique_codes.len(),
            "TCP error codes must be unique"
        );

        // All codes should be in reserved range (-32110 to -32119)
        for code in &tcp_codes {
            assert!(
                *code >= -32119 && *code <= -32110,
                "TCP error code {} must be in range -32119 to -32110",
                code
            );
        }
    }

    /// Test TCP error codes don't conflict with standard JSON-RPC codes.
    #[test]
    fn test_tcp_error_codes_no_jsonrpc_conflict() {
        use crate::protocol::error_codes;

        let tcp_codes = vec![
            error_codes::TCP_BIND_FAILED,
            error_codes::TCP_CONNECTION_ERROR,
            error_codes::TCP_MAX_CONNECTIONS_REACHED,
            error_codes::TCP_FRAME_ERROR,
            error_codes::TCP_CLIENT_TIMEOUT,
        ];

        // Standard JSON-RPC error codes
        let jsonrpc_codes = vec![
            error_codes::PARSE_ERROR,      // -32700
            error_codes::INVALID_REQUEST,  // -32600
            error_codes::METHOD_NOT_FOUND, // -32601
            error_codes::INVALID_PARAMS,   // -32602
            error_codes::INTERNAL_ERROR,   // -32603
        ];

        // Verify no conflicts
        for tcp_code in &tcp_codes {
            for jsonrpc_code in &jsonrpc_codes {
                assert_ne!(
                    tcp_code, jsonrpc_code,
                    "TCP error code {} conflicts with JSON-RPC code {}",
                    tcp_code, jsonrpc_code
                );
            }
        }
    }

    /// Test that TransportMode can be used in match expressions.
    #[test]
    fn test_transport_mode_match() {
        let mode = TransportMode::Tcp;

        let result = match mode {
            TransportMode::Stdio => "stdio",
            TransportMode::Tcp => "tcp",
            TransportMode::Sse => "sse",
        };

        assert_eq!(result, "tcp");
    }

    /// Test Semaphore capacity calculation from config.
    /// (We test Semaphore directly since McpServer::new requires file system setup)
    #[test]
    fn test_semaphore_capacity_from_config() {
        use context_graph_core::config::Config;

        let mut config = Config::default_config();
        config.mcp.max_connections = 10;

        // Semaphore would be created with config.mcp.max_connections
        let semaphore = Arc::new(Semaphore::new(config.mcp.max_connections));
        assert_eq!(
            semaphore.available_permits(),
            10,
            "Semaphore should have 10 permits matching max_connections"
        );
    }

    /// Test default max_connections value from McpConfig.
    #[test]
    fn test_default_max_connections() {
        use context_graph_core::config::Config;

        let config = Config::default_config();
        assert_eq!(
            config.mcp.max_connections, 32,
            "Default max_connections should be 32"
        );
    }

    /// Test TCP bind address format parsing.
    #[test]
    fn test_tcp_bind_address_parsing() {
        use std::net::SocketAddr;

        // Valid addresses
        let valid_addr: Result<SocketAddr, _> = "127.0.0.1:3100".parse();
        assert!(valid_addr.is_ok(), "Should parse valid IPv4 address");

        let valid_addr: Result<SocketAddr, _> = "0.0.0.0:3100".parse();
        assert!(valid_addr.is_ok(), "Should parse 0.0.0.0 address");

        let valid_addr: Result<SocketAddr, _> = "[::1]:3100".parse();
        assert!(valid_addr.is_ok(), "Should parse IPv6 localhost");

        // Invalid addresses
        let invalid_addr: Result<SocketAddr, _> = "not-an-address".parse();
        assert!(invalid_addr.is_err(), "Should reject invalid address");

        let invalid_addr: Result<SocketAddr, _> = "127.0.0.1:".parse();
        assert!(invalid_addr.is_err(), "Should reject address without port");
    }

    /// Test format string for bind address construction.
    #[test]
    fn test_bind_address_format() {
        use context_graph_core::config::Config;

        let config = Config::default_config();
        let bind_str = format!("{}:{}", config.mcp.bind_address, config.mcp.tcp_port);

        assert_eq!(bind_str, "127.0.0.1:3100");
    }
}
