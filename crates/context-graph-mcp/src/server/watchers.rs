//! Background watcher and builder implementations for the MCP server.
//!
//! Contains file watcher, code watcher, and graph builder code extracted
//! from the main server module.
//! - File watcher: monitors ./docs/ for .md changes (CRIT-06 shutdown fix)
//! - Code watcher: E7-based AST code indexing
//! - Graph builder: K-NN edge computation (TASK-GRAPHLINK-PHASE1)

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::{debug, error, info};

use context_graph_core::memory::watcher::GitFileWatcher;
use context_graph_core::memory::{CodeCaptureService, CodeFileWatcher, MemoryCaptureService, MultiArrayEmbeddingAdapter};
use context_graph_core::memory::store::MemoryStore;

use crate::adapters::CodeStoreAdapter;
use context_graph_embeddings::adapters::E7CodeEmbeddingProvider;
use context_graph_storage::code::CodeStore;

use super::McpServer;

impl McpServer {
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

        // CRIT-06 FIX: Set the running flag and pass a clone into the thread
        // so the loop can be stopped from outside.
        self.file_watcher_running.store(true, Ordering::SeqCst);
        let running_flag = Arc::clone(&self.file_watcher_running);

        // Spawn file watcher in a dedicated thread to handle the non-Send Receiver
        // We use spawn_blocking + nested tokio runtime for this
        let thread_handle = std::thread::spawn(move || {
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
                        running_flag.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                // Start watcher
                if let Err(e) = watcher.start().await {
                    error!(error = %e, "Failed to start file watcher");
                    running_flag.store(false, Ordering::SeqCst);
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

                    // CRIT-06 FIX: Check shutdown flag each iteration.
                    if !running_flag.load(Ordering::SeqCst) {
                        info!("File watcher received shutdown signal");
                        break;
                    }

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

        // Store the thread handle for joining during shutdown
        if let Ok(mut guard) = self.file_watcher_thread.lock() {
            *guard = Some(thread_handle);
        }

        info!(
            paths = ?self.config.watcher.watch_paths,
            session_id = %self.config.watcher.session_id,
            "File watcher started as background task"
        );

        Ok(true)
    }

    /// Stop the file watcher thread.
    ///
    /// CRIT-06 FIX: Signals the file watcher thread to stop via the atomic flag,
    /// then joins the thread with a timeout to ensure clean shutdown.
    pub(in crate::server) fn stop_file_watcher(&self) {
        if !self.file_watcher_running.load(Ordering::SeqCst) {
            return;
        }

        info!("Stopping file watcher...");
        self.file_watcher_running.store(false, Ordering::SeqCst);

        // Join the thread (with a timeout via try_lock to avoid blocking forever)
        if let Ok(mut guard) = self.file_watcher_thread.lock() {
            if let Some(handle) = guard.take() {
                // The thread checks the flag every 500ms, so it should exit within ~1s.
                // Use join() which blocks the current thread until the spawned thread finishes.
                // This is acceptable in shutdown since we are tearing down.
                match handle.join() {
                    Ok(()) => info!("File watcher thread stopped"),
                    Err(_) => error!("File watcher thread panicked"),
                }
            }
        }
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
}
