//! MCP Server implementation.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! TASK-S003: Added GoalAlignmentCalculator and GoalHierarchy for purpose operations.
//! TASK-S004: Replaced stubs with REAL implementations (RocksDB, UTL adapter).
//! TASK-INTEG-018: Added TCP transport support with concurrent client handling.
//!
//! NO BACKWARDS COMPATIBILITY with stubs. FAIL FAST with clear errors.

use std::io::{self, BufRead, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, Semaphore};
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

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::config::Config;
use context_graph_core::purpose::GoalHierarchy;
use context_graph_core::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor,
};

use context_graph_embeddings::{GpuConfig, ProductionMultiArrayProvider};

// REAL implementations - NO STUBS
use crate::adapters::{LazyMultiArrayProvider, UtlProcessorAdapter};
use context_graph_storage::teleological::RocksDbTeleologicalStore;

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
    utl_processor: Arc<dyn UtlProcessor>,
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
        //
        // LAZY-STARTUP: Models are loaded in a background task to allow immediate
        // MCP protocol response. The server will be ready for protocol messages
        // immediately, but embedding tools will return "models loading" until ready.
        let models_dir = Self::resolve_models_path(&config);
        info!(
            "Will load ProductionMultiArrayProvider with models from {:?} (background)...",
            models_dir
        );

        // Create shared state for lazy model loading
        let multi_array_provider: Arc<RwLock<Option<Arc<dyn MultiArrayEmbeddingProvider>>>> =
            Arc::new(RwLock::new(None));
        let models_loading = Arc::new(AtomicBool::new(true));
        let models_failed: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        // Spawn background task to load models
        {
            let provider_slot = Arc::clone(&multi_array_provider);
            let loading_flag = Arc::clone(&models_loading);
            let failed_slot = Arc::clone(&models_failed);
            let models_dir_clone = models_dir.clone();

            tokio::spawn(async move {
                info!("Background model loading started...");
                match ProductionMultiArrayProvider::new(models_dir_clone.clone(), GpuConfig::default()).await {
                    Ok(provider) => {
                        let mut slot = provider_slot.write().await;
                        *slot = Some(Arc::new(provider));
                        loading_flag.store(false, Ordering::SeqCst);
                        info!("Background model loading COMPLETE - 13 embedders ready");
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
            });
        }
        info!("Spawned background task to load 13 embedding models (GPU-accelerated)");

        // Create lazy provider wrapper for immediate MCP startup
        let lazy_provider: Arc<dyn MultiArrayEmbeddingProvider> = Arc::new(LazyMultiArrayProvider::new(
            Arc::clone(&multi_array_provider),
            Arc::clone(&models_loading),
            Arc::clone(&models_failed),
        ));

        // TASK-S003: Create alignment calculator and goal hierarchy
        let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
            Arc::new(DefaultAlignmentCalculator::new());
        let goal_hierarchy = Arc::new(parking_lot::RwLock::new(GoalHierarchy::new()));
        info!("Created DefaultAlignmentCalculator and empty GoalHierarchy");

        // ==========================================================================
        // 4. Create Meta-UTL tracker and monitoring providers
        // ==========================================================================
        use crate::handlers::MetaUtlTracker;
        use context_graph_core::monitoring::{StubLayerStatusProvider, StubSystemMonitor};

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
        // - GwtSystemProviderImpl: Real GWT state management
        // - WorkspaceProviderImpl: Real global workspace with winner-take-all
        // - MetaCognitiveProviderImpl: Real meta-cognitive loop
        // NOTE: Using lazy_provider to allow immediate MCP startup
        let handlers = Handlers::with_default_gwt(
            Arc::clone(&teleological_store),
            Arc::clone(&utl_processor),
            lazy_provider,
            alignment_calculator,
            goal_hierarchy,
            meta_utl_tracker,
            system_monitor,
            layer_status_provider,
        );
        info!("Created Handlers with REAL GWT providers (GWT, Workspace, MetaCognitive)");
        info!("Created REAL NeuromodulationManager (Dopamine, Serotonin, Noradrenaline at baseline; ACh read-only via GWT)");
        info!("Created REAL Dream components (DreamController, DreamScheduler, AmortizedLearner) with constitution defaults");
        info!("Created REAL AdaptiveThresholdCalibration (4-level: EWMA, Temperature, Bandit, Bayesian)");

        info!("MCP Server initialization complete - TeleologicalFingerprint mode active with GWT + Neuromod + Dream + ATC");

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
            utl_processor,
            multi_array_provider,
            models_loading,
            models_failed,
            // TASK-INTEG-018: Arc-wrap handlers for TCP sharing
            handlers: Arc::new(handlers),
            initialized: Arc::new(RwLock::new(false)),
            connection_semaphore,
            active_connections: Arc::new(AtomicUsize::new(0)),
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
        info!("Graceful shutdown complete");
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
