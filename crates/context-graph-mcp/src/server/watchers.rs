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

use anyhow::Result;
use tracing::{debug, error, info, warn};

use context_graph_core::memory::store::MemoryStore;
use context_graph_core::memory::watcher::GitFileWatcher;
use context_graph_core::memory::{MemoryCaptureService, MultiArrayEmbeddingAdapter};

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
                error!("Embedding models still loading after 60s — file watcher cannot start");
                return Err(anyhow::anyhow!(
                    "Embedding models timed out after 60s — file watcher cannot start"
                ));
            }
        }

        // Check if model loading failed
        {
            let failed = self.models_failed.read().await;
            if let Some(ref err) = *failed {
                error!(
                    "Cannot start file watcher — embedding models failed: {}",
                    err
                );
                return Err(anyhow::anyhow!(
                    "Embedding models failed: {} — file watcher cannot start",
                    err
                ));
            }
        }

        // Get embedding provider
        let provider = {
            let slot = self.multi_array_provider.read().await;
            match slot.as_ref() {
                Some(p) => Arc::clone(p),
                None => {
                    error!("Cannot start file watcher — no embedding provider available");
                    return Err(anyhow::anyhow!(
                        "No embedding provider available — file watcher cannot start"
                    ));
                }
            }
        };

        // Create separate storage path for file watcher's MemoryStore
        // Uses a subdirectory to avoid RocksDB column family conflicts with main teleological store
        let base_db_path = Self::resolve_storage_path(&self.config)?;
        let watcher_db_path = base_db_path.join("watcher_memory");
        Self::ensure_directory_exists(&watcher_db_path)?;

        // Create memory store in separate directory
        let memory_store = Arc::new(MemoryStore::new(&watcher_db_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create memory store for file watcher at {:?}: {}",
                watcher_db_path,
                e
            )
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
                let mut watcher = match GitFileWatcher::new(
                    watch_paths.clone(),
                    capture_service,
                    session_id.clone(),
                ) {
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
        // ERR-12 FIX: Panic on poisoned lock — if another thread panicked, state is corrupt
        {
            let mut guard = self
                .file_watcher_thread
                .lock()
                .expect("file_watcher_thread mutex poisoned: another thread panicked");
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

        // Join the thread — log error on poisoned lock during shutdown
        match self.file_watcher_thread.lock() {
            Err(poisoned) => {
                error!("file_watcher_thread mutex poisoned during shutdown — thread likely already panicked");
                // Still try to recover the guard so we can attempt join
                let mut guard = poisoned.into_inner();
                if let Some(handle) = guard.take() {
                    let _ = handle.join();
                }
            }
            Ok(mut guard) => {
                if let Some(handle) = guard.take() {
                    // MCP-L3 FIX: The thread checks the flag every 500ms, so it should exit
                    // within ~1s. Use a bounded spin-wait to avoid blocking the async runtime
                    // indefinitely if process_events() hangs.
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                    loop {
                        if handle.is_finished() {
                            match handle.join() {
                                Ok(()) => info!("File watcher thread stopped"),
                                Err(_) => error!("File watcher thread panicked"),
                            }
                            break;
                        }
                        if std::time::Instant::now() >= deadline {
                            warn!("File watcher thread did not stop within 2s — abandoning join");
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
        }
    }

    // =========================================================================
    // E7-WIRING: Code File Watcher
    // =========================================================================

    /// Stop the code file watcher.
    ///
    /// Signals the background task to stop and waits for it to complete.
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
        // L9 FIX: Idempotency guard — skip if already running.
        // Use the write guard for the whole scheduling path so TCP and HTTP transports
        // cannot race and start duplicate graph-builder supervisors.
        let mut task_guard = self.graph_builder_task.write().await;
        if task_guard.is_some() {
            debug!("Graph builder already running — skipping duplicate start");
            return Ok(true);
        }

        let graph_builder = match &self.graph_builder {
            Some(builder) => Arc::clone(builder),
            None => {
                debug!("No graph builder configured - skipping worker start");
                return Ok(false);
            }
        };

        // Graph building needs embeddings, but transport readiness must not wait
        // for model warmup when --no-warm/background warmup is explicitly used.
        if self.models_loading.load(Ordering::SeqCst) {
            info!(
                "Embedding models still loading; graph builder will start after warmup completes"
            );

            let loading = Arc::clone(&self.models_loading);
            let failed_slot = Arc::clone(&self.models_failed);
            let mut shutdown_rx = self.shutdown_tx.subscribe();
            let delayed_builder = Arc::clone(&graph_builder);

            let task = tokio::spawn(async move {
                loop {
                    if !loading.load(Ordering::SeqCst) {
                        break;
                    }

                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {}
                        changed = shutdown_rx.changed() => {
                            match changed {
                                Ok(()) if *shutdown_rx.borrow() => {
                                    info!(
                                        "Graph builder startup cancelled during shutdown before embeddings were ready"
                                    );
                                    return;
                                }
                                Ok(()) => {}
                                Err(_) => {
                                    info!(
                                        "Graph builder startup cancelled because shutdown channel closed"
                                    );
                                    return;
                                }
                            }
                        }
                    }
                }

                if let Some(err) = (*failed_slot.read().await).clone() {
                    error!(
                        "Cannot start graph builder - embedding models failed: {}",
                        err
                    );
                    return;
                }

                info!(
                    "TASK-GRAPHLINK-PHASE1: Starting delayed background graph builder worker (interval={}s)",
                    delayed_builder.config().batch_interval_secs
                );
                let worker = delayed_builder.start_worker();
                match worker.await {
                    Ok(()) => info!("Delayed graph builder worker stopped"),
                    Err(e) => error!(error = %e, "Delayed graph builder worker failed to join"),
                }
            });

            *task_guard = Some(task);
            return Ok(true);
        }

        // Check if model loading failed
        {
            let failed = self.models_failed.read().await;
            if let Some(ref err) = *failed {
                error!(
                    "Cannot start graph builder - embedding models failed: {}",
                    err
                );
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
        *task_guard = Some(task);

        info!("TASK-GRAPHLINK-PHASE1: Background graph builder started successfully");
        Ok(true)
    }

    /// Stop the background graph builder worker.
    ///
    /// Signals the worker to stop and waits for it to complete.
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
