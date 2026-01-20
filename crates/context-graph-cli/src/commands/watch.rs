//! File watcher command - Watch directories for markdown file changes.
//!
//! # Usage
//!
//! ```bash
//! # Watch ./docs/ directory for .md file changes
//! context-graph-cli watch --path ./docs --session-id my-session
//!
//! # Watch with verbose logging
//! context-graph-cli -vv watch --path ./docs --session-id my-session
//! ```
//!
//! This command starts the GitFileWatcher service that:
//! 1. Monitors the specified directory for .md file changes
//! 2. Chunks new/modified files using TextChunker (200 words, 50 overlap)
//! 3. Stores chunks with source metadata (MDFileChunk, file_path, chunk_index)
//! 4. Clears old embeddings before storing new ones on file modification
//! 5. **Phase 8 Fix**: Stores fingerprints in TeleologicalMemoryStore for MCP search
//!
//! # Prerequisites
//!
//! Run `context-graph-cli warmup` first to load embedding models into VRAM.

use clap::Args;
use context_graph_core::memory::{
    EmbeddingProvider, MemoryCaptureService, MultiArrayEmbeddingAdapter,
};
use context_graph_core::memory::store::MemoryStore;
use context_graph_core::memory::watcher::GitFileWatcher;
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider, is_warm_initialized};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};

/// Arguments for the watch command
#[derive(Args)]
pub struct WatchArgs {
    /// Directory path to watch for .md files
    #[arg(short, long)]
    pub path: PathBuf,

    /// Session ID for captured memories
    #[arg(short, long, default_value = "file-watcher-session")]
    pub session_id: String,

    /// Database path for storing memories
    #[arg(long, env = "CONTEXT_GRAPH_DATA_DIR")]
    pub db_path: Option<PathBuf>,
}

/// Handle the watch command
pub async fn handle_watch(args: WatchArgs) -> i32 {
    info!(
        path = ?args.path,
        session_id = %args.session_id,
        "Starting file watcher"
    );

    // Validate path exists
    if !args.path.exists() {
        error!(path = ?args.path, "Path does not exist");
        return 1;
    }

    if !args.path.is_dir() {
        error!(path = ?args.path, "Path is not a directory");
        return 1;
    }

    // Initialize warm provider if not already done
    // The MCP server initializes this at startup, but CLI runs as a separate process
    if !is_warm_initialized() {
        info!("Embedding models not warm - initializing GPU embedding pipeline...");
        info!("This may take 20-30 seconds on RTX 5090...");

        match initialize_global_warm_provider().await {
            Ok(()) => {
                info!("GPU embedding models initialized successfully");
            }
            Err(e) => {
                error!(error = %e, "Failed to initialize GPU embedding models");
                error!("Ensure GPU is available and CUDA drivers are installed");
                return 1;
            }
        }
    } else {
        info!("Using already-warm embedding models");
    }

    // Get GPU embedding provider
    let warm_provider = match get_warm_provider() {
        Ok(provider) => provider,
        Err(e) => {
            error!(error = %e, "Failed to get warm embedding provider");
            return 1;
        }
    };

    // Setup database path - use env var or default to ./contextgraph_data
    let db_path = args.db_path.clone().unwrap_or_else(|| {
        std::env::var("CONTEXT_GRAPH_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./contextgraph_data"))
    });

    info!(db_path = ?db_path, "Using database path");

    // Create watcher-specific memory store in a subdirectory (for local storage)
    let watcher_db_path = db_path.join("watcher_memory");
    let memory_store = match MemoryStore::new(&watcher_db_path) {
        Ok(store) => Arc::new(store),
        Err(e) => {
            error!(error = %e, "Failed to create memory store");
            return 1;
        }
    };

    // Create embedding provider adapter wrapping the GPU provider
    let embedder: Arc<dyn EmbeddingProvider> =
        Arc::new(MultiArrayEmbeddingAdapter::new(warm_provider));

    // Phase 8 Fix: Try to create TeleologicalMemoryStore for MCP search integration
    // This ensures file watcher memories are searchable via MCP tools
    let capture_service: Arc<MemoryCaptureService> = match RocksDbTeleologicalStore::open(&db_path) {
        Ok(store) => {
            info!("Opened TeleologicalStore at {:?} - memories will be searchable via MCP tools", db_path);
            let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(store);
            // Create capture service WITH teleological store for MCP integration
            Arc::new(MemoryCaptureService::with_teleological_store(
                memory_store.clone(),
                embedder,
                teleological_store,
            ))
        }
        Err(e) => {
            warn!(
                error = %e,
                "Failed to open TeleologicalStore - memories will NOT be searchable via MCP tools"
            );
            // Fall back to local-only storage
            Arc::new(MemoryCaptureService::new(memory_store.clone(), embedder))
        }
    };

    // Create file watcher
    let mut watcher = match GitFileWatcher::new(
        vec![args.path.clone()],
        capture_service,
        args.session_id.clone(),
    ) {
        Ok(w) => w,
        Err(e) => {
            error!(error = %e, "Failed to create file watcher");
            return 1;
        }
    };

    // Start watcher
    if let Err(e) = watcher.start().await {
        error!(error = %e, "Failed to start file watcher");
        return 1;
    }

    info!(
        path = ?args.path,
        "File watcher started. Press Ctrl+C to stop."
    );

    // Process events in a loop until Ctrl+C
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                match watcher.process_events().await {
                    Ok(count) => {
                        if count > 0 {
                            info!(files_processed = count, "Processed file changes");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Error processing events");
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, stopping file watcher...");
                break;
            }
        }
    }

    watcher.stop();

    let final_count = memory_store.count().unwrap_or(0);
    info!(
        total_memories = final_count,
        cached_files = watcher.cached_file_count().await,
        "File watcher stopped"
    );

    0
}
