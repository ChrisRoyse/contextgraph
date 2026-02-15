//! MCP Server implementation.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! TASK-S003: Added teleological scoring for multi-space retrieval operations.
//! TASK-S004: Replaced stubs with REAL implementations (RocksDB storage).
//! TASK-INTEG-018: Added TCP transport support with concurrent client handling.
//!
//! ## Module Structure (M4 Split)
//! - `mod.rs` — McpServer struct, constructor, stdio transport, handle_request, path resolution
//! - `transport.rs` — TCP transport (run_tcp, handle_tcp_client), SSE transport (run_sse), read_line_bounded
//! - `watchers.rs` — File watcher, code watcher, graph builder background tasks
//!
//! NO BACKWARDS COMPATIBILITY with stubs. FAIL FAST with clear errors.

mod transport;
mod watchers;

// NOTE: std::io removed - using tokio::io for async I/O (AP-08 compliance)
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncWriteExt, BufReader};
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
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore};

use context_graph_embeddings::{
    get_warm_provider, initialize_global_warm_provider, is_warm_initialized, warm_status_message,
    GpuConfig, ProductionMultiArrayProvider,
};
#[cfg(feature = "llm")]
use context_graph_embeddings::{get_warm_causal_model, get_warm_graph_model};

// REAL implementations - NO STUBS
use crate::adapters::LazyMultiArrayProvider;
#[cfg(feature = "llm")]
use crate::adapters::LlmCausalHintProvider;
#[cfg(feature = "llm")]
use context_graph_embeddings::provider::CausalHintProvider;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
// TASK-GRAPHLINK: EdgeRepository and BackgroundGraphBuilder for K-NN graph linking
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository, GraphBuilderConfig};
// GRAPH-AGENT: LLM-based relationship discovery (requires `llm` feature)
#[cfg(feature = "llm")]
use context_graph_causal_agent::CausalDiscoveryLLM;
#[cfg(feature = "llm")]
use context_graph_graph_agent::{GraphDiscoveryConfig, GraphDiscoveryService};

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
/// TASK-INTEG-018: Arc-wrapped handlers for TCP transport sharing across concurrent clients.
/// LAZY-STARTUP: Models load in background to allow immediate MCP protocol response.
#[allow(dead_code)]
pub struct McpServer {
    pub(in crate::server) config: Config,
    /// Teleological memory store - stores TeleologicalFingerprint with 13 embeddings.
    pub(in crate::server) teleological_store: Arc<dyn TeleologicalMemoryStore>,
    /// Multi-array embedding provider - generates all 13 embeddings.
    /// Wrapped in RwLock<Option<...>> for lazy loading - None while models are loading.
    pub(in crate::server) multi_array_provider: Arc<RwLock<Option<Arc<dyn MultiArrayEmbeddingProvider>>>>,
    /// Flag indicating whether models are currently loading.
    pub(in crate::server) models_loading: Arc<AtomicBool>,
    /// Flag indicating whether model loading failed.
    pub(in crate::server) models_failed: Arc<RwLock<Option<String>>>,
    /// Arc-wrapped handlers for safe sharing across TCP client tasks.
    pub(in crate::server) handlers: Arc<Handlers>,
    pub(in crate::server) initialized: Arc<RwLock<bool>>,
    /// Connection semaphore for limiting concurrent TCP connections.
    pub(in crate::server) connection_semaphore: Arc<Semaphore>,
    /// Active connection counter for monitoring.
    pub(in crate::server) active_connections: Arc<AtomicUsize>,
    /// Code watcher background task handle.
    pub(in crate::server) code_watcher_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Flag to signal code watcher shutdown.
    pub(in crate::server) code_watcher_running: Arc<AtomicBool>,
    /// CRIT-06 FIX: File watcher shutdown flag and thread handle.
    pub(in crate::server) file_watcher_running: Arc<AtomicBool>,
    pub(in crate::server) file_watcher_thread: Arc<std::sync::Mutex<Option<std::thread::JoinHandle<()>>>>,
    /// TASK-GRAPHLINK-PHASE1: Background graph builder for K-NN edge computation.
    pub(in crate::server) graph_builder: Option<Arc<BackgroundGraphBuilder>>,
    /// TASK-GRAPHLINK-PHASE1: Background graph builder worker task handle.
    pub(in crate::server) graph_builder_task: Arc<RwLock<Option<JoinHandle<()>>>>,
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

        // Wrap in Arc<RocksDbTeleologicalStore> first, then clone for background tasks
        let rocksdb_store_arc = Arc::new(rocksdb_store);
        let gc_store = Arc::clone(&rocksdb_store_arc);
        let persist_store = Arc::clone(&rocksdb_store_arc);

        // Spawn soft-delete GC background task (runs every 5 minutes)
        tokio::spawn(async move {
            let gc_interval = std::time::Duration::from_secs(5 * 60);
            let gc_retention = 7 * 24 * 3600u64; // 7 days
            info!("Soft-delete GC background task started (interval=5min, retention=7d)");
            loop {
                tokio::time::sleep(gc_interval).await;
                match gc_store.gc_soft_deleted(gc_retention).await {
                    Ok(deleted) => {
                        if deleted > 0 {
                            info!("GC cycle: hard-deleted {deleted} expired soft-deleted entries");
                        }
                    }
                    Err(e) => {
                        tracing::error!("GC cycle failed: {e}");
                    }
                }
            }
        });

        // Spawn HNSW index persistence background task (runs every 10 minutes)
        tokio::spawn(async move {
            let persist_interval = std::time::Duration::from_secs(10 * 60);
            info!("HNSW persistence background task started (interval=10min)");
            loop {
                tokio::time::sleep(persist_interval).await;
                if let Err(e) = persist_store.persist_hnsw_indexes() {
                    tracing::error!("HNSW persistence failed: {e}");
                }
            }
        });

        // Now wrap in Arc<dyn TeleologicalMemoryStore>
        let teleological_store: Arc<dyn TeleologicalMemoryStore> = rocksdb_store_arc;

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
            Arc::new(crate::monitoring::LiveLayerStatusProvider::new(
                Arc::clone(&models_loading),
                Arc::clone(&models_failed),
                Arc::clone(&teleological_store),
            ));

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

        // ==========================================================================
        // 3b. LLM Initialization (requires `llm` feature)
        // ==========================================================================
        // When `llm` feature is enabled (default): Load CausalDiscoveryLLM + GraphDiscoveryService
        // When `llm` feature is disabled: Create Handlers without LLM (graph/causal tools unavailable)

        #[cfg(feature = "llm")]
        let handlers = {
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

            // MODEL-INJECTION: Get GraphModel from warm provider for E8 embeddings
            // This shares the already-loaded model (~1.3GB VRAM) with the embedding system
            //
            // In --no-warm mode, the background task may still hold the write lock on
            // GLOBAL_WARM_PROVIDER. We must wait for it to finish before calling
            // get_warm_graph_model() which uses try_read().
            let graph_model = {
                let max_wait = std::time::Duration::from_secs(180);
                let poll_interval = std::time::Duration::from_millis(500);
                let start = std::time::Instant::now();

                loop {
                    match get_warm_graph_model() {
                        Ok(model) => break model,
                        Err(e) => {
                            let elapsed = start.elapsed();
                            if elapsed >= max_wait {
                                error!(
                                    "FATAL: Timed out waiting for GraphModel after {:.1}s: {}. \
                                     Ensure embedding models are initialized before graph discovery.",
                                    elapsed.as_secs_f64(), e
                                );
                                return Err(anyhow::anyhow!(
                                    "Failed to get GraphModel after {:.1}s: {}. \
                                     GraphDiscoveryService requires E8 model for relationship embeddings.",
                                    elapsed.as_secs_f64(), e
                                ));
                            }
                            // Log first attempt and then every 10s
                            if elapsed.as_millis() < 600 || elapsed.as_secs() % 10 == 0 {
                                info!(
                                    "Waiting for warm provider to be ready ({:.1}s elapsed): {}",
                                    elapsed.as_secs_f64(), e
                                );
                            }
                            tokio::time::sleep(poll_interval).await;
                        }
                    }
                }
            };
            info!("GraphModel obtained from warm provider - E8 embeddings ready");

            let graph_discovery_config = GraphDiscoveryConfig::default();
            let graph_discovery_service = Arc::new(GraphDiscoveryService::with_models(
                Arc::clone(&shared_llm), // Shared LLM for relationship classification
                graph_model,             // GraphModel for E8 asymmetric embeddings
                graph_discovery_config,
            ));

            // INLINE-CAUSAL: Get CausalModel for E5 asymmetric embeddings (shares warm provider)
            // Used by inline causal extraction during store_memory (no background loop).
            let causal_model = get_warm_causal_model().map_err(|e| {
                error!(
                    "FATAL: Failed to get CausalModel from warm provider: {}. \
                     Inline causal extraction requires E5 model for relationship embeddings.",
                    e
                );
                anyhow::anyhow!("Failed to get CausalModel: {}", e)
            })?;
            info!("CausalModel obtained from warm provider - E5 embeddings ready for inline causal extraction");

            // INLINE-CAUSAL: Clone shared_llm BEFORE it's moved into LlmCausalHintProvider
            // This clone is passed to Handlers for inline extract_causal_relationships()
            let causal_llm_for_inline = Arc::clone(&shared_llm);

            // CAUSAL-HINT: Create LlmCausalHintProvider using shared LLM (GPU inference via CUDA)
            info!("CAUSAL-HINT: Creating LlmCausalHintProvider (2s timeout for GPU inference)");
            let causal_hint_provider: Arc<dyn CausalHintProvider> =
                Arc::new(LlmCausalHintProvider::new(
                    shared_llm, // Move remaining Arc
                    LlmCausalHintProvider::DEFAULT_TIMEOUT_MS,
                ));

            // TASK-GRAPHLINK + INLINE-CAUSAL: Use with_graph_discovery to enable K-NN graph operations
            // with background builder support, LLM-based relationship detection, and inline causal extraction.
            // NO FALLBACKS - All components MUST work or server startup fails.
            info!("Creating Handlers with graph discovery, causal hints, and inline causal extraction - NO FALLBACKS");
            let h = Handlers::with_graph_discovery(
                Arc::clone(&teleological_store),
                lazy_provider,
                layer_status_provider,
                edge_repository,
                Arc::clone(&graph_builder),
                graph_discovery_service,
                causal_hint_provider,
                causal_llm_for_inline, // INLINE-CAUSAL: Shared LLM for extract_causal_relationships()
                causal_model,          // INLINE-CAUSAL: E5 CausalModel for asymmetric embeddings
            );
            info!("Created Handlers with inline causal extraction enabled (K-NN edges + background builder, NO FALLBACKS)");
            h
        };

        #[cfg(not(feature = "llm"))]
        let handlers = {
            info!("LLM feature disabled - creating Handlers without LLM (graph/causal discovery tools unavailable)");
            // Without LLM: no CausalDiscoveryLLM, no GraphDiscoveryService, no CausalHintProvider
            // LLM-dependent tools will return errors at call time.
            Handlers::without_llm(
                Arc::clone(&teleological_store),
                lazy_provider,
                layer_status_provider,
                edge_repository,
                Arc::clone(&graph_builder),
                // No causal hint provider without LLM - pass a no-op provider
                Arc::new(context_graph_embeddings::provider::NoOpCausalHintProvider),
            )
        };

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
            // CRIT-06: File watcher shutdown flag and thread handle
            file_watcher_running: Arc::new(AtomicBool::new(false)),
            file_watcher_thread: Arc::new(std::sync::Mutex::new(None)),
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

        // BACKFILL: If edge repository has no data, run full rebuild SYNCHRONOUSLY.
        // Constitution: "JoinHandle must be awaited or aborted — never silently dropped"
        // This blocks the server for 1-5s (acceptable for 260 fingerprints) but guarantees
        // graph link tools have edges ready when the first request arrives.
        if let Some(ref builder) = self.graph_builder {
            let needs_rebuild = self.handlers.edge_repository()
                .map(|repo| repo.is_empty().unwrap_or(true))
                .unwrap_or(false);
            if needs_rebuild {
                info!("Edge repository empty — running SYNCHRONOUS K-NN graph rebuild (FAIL FAST)");
                match builder.rebuild_all().await {
                    Ok(result) => {
                        if result.total_knn_edges == 0 && result.total_processed > 0 {
                            error!(
                                "K-NN graph rebuild produced ZERO edges from {} fingerprints in {}ms. \
                                 This indicates an embedding or NN-Descent issue. \
                                 Check that fingerprints have non-empty E1 embeddings.",
                                result.total_processed, result.elapsed_ms
                            );
                        } else {
                            info!(
                                "K-NN graph rebuild complete: {} fingerprints → {} K-NN edges + {} typed edges in {}ms",
                                result.total_processed, result.total_knn_edges, result.total_typed_edges, result.elapsed_ms
                            );
                        }
                    }
                    Err(e) => {
                        error!("K-NN graph rebuild FAILED (FAIL FAST): {}", e);
                    }
                }
            } else {
                info!("Edge repository has data — skipping K-NN graph rebuild");
            }
        }

        loop {
            line.clear();

            // AGT-04 FIX: Use bounded read_line to prevent OOM from unbounded input.
            // read_line() allocates until newline; a malicious/broken client can OOM the process.
            let bytes_read = transport::read_line_bounded(&mut reader, &mut line, transport::MAX_LINE_BYTES)
                .await
                .map_err(|e| {
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

        // CRIT-06: Stop file watcher if running
        self.stop_file_watcher();

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
    pub(in crate::server) fn resolve_storage_path(config: &Config) -> PathBuf {
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
    pub(in crate::server) fn ensure_directory_exists(path: &PathBuf) {
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
