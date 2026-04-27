//! Warmup command for pre-loading embedding models into VRAM.
//!
//! TASK-EMB-WARMUP: This command loads all 14 production embedding models into VRAM
//! before the MCP server starts, ensuring embedding operations are available
//! immediately.
//!
//! # Usage
//!
//! ```bash
//! # Pre-warm models before starting MCP server
//! context-graph-cli warmup
//!
//! # Then start MCP server (will use already-warm models)
//! context-graph-mcp
//! ```
//!
//! # Constitution Reference
//!
//! - ARCH-08: CUDA GPU required for production - no CPU fallbacks
//! - Constitution stack.gpu: RTX 5090, CUDA 13.2, 32GB VRAM

use clap::Args;
use tracing::{error, info};

use context_graph_embeddings::{
    get_warm_provider, initialize_global_warm_provider, is_warm_initialized, warm_status_message,
};

/// Arguments for the warmup command.
#[derive(Args, Debug)]
pub struct WarmupArgs {
    /// Skip warmup if models are already warm
    #[arg(long, default_value = "true")]
    pub skip_if_warm: bool,

    /// Timeout in seconds (0 = no timeout)
    #[arg(long, default_value = "300")]
    pub timeout_secs: u64,
}

/// Execute the warmup command.
///
/// Loads all 14 production embedding models into VRAM. This is a blocking operation
/// that takes approximately 20-30 seconds on RTX 5090.
///
/// # Returns
///
/// Exit code:
/// - 0: Success (models are warm, including "already warm")
/// - 1: Failed to initialize models
pub async fn handle_warmup(args: WarmupArgs) -> i32 {
    info!("Context Graph Warmup - Loading embedding models into VRAM...");

    // Check if already warm
    if is_warm_initialized() {
        info!("Models are already warm. Status: {}", warm_status_message());

        // Verify we can actually get the provider
        match get_warm_provider() {
            Ok(_provider) => {
                info!(
                    "Already warm, skipping. All 14 production embedding models are warm and ready"
                );
                return 0; // Success - already warm is still success
            }
            Err(e) => {
                error!(
                    "Models appear warm but provider failed: {}. Status: {}",
                    e,
                    warm_status_message()
                );
                // Fall through to re-initialize
            }
        }
    }

    info!("Initializing global warm provider...");
    info!("This may take 20-30 seconds on RTX 5090 (32GB VRAM)...");

    // Initialize with optional timeout
    let init_result = if args.timeout_secs > 0 {
        tokio::time::timeout(
            std::time::Duration::from_secs(args.timeout_secs),
            initialize_global_warm_provider(),
        )
        .await
    } else {
        Ok(initialize_global_warm_provider().await)
    };

    match init_result {
        Ok(Ok(())) => {
            info!("Global warm provider initialized successfully");

            // Verify we can get the provider
            match get_warm_provider() {
                Ok(_provider) => {
                    info!("SUCCESS: All 14 production embedding models loaded into VRAM and ready");
                    info!("Status: {}", warm_status_message());
                    0 // Success
                }
                Err(e) => {
                    error!("Provider initialized but get_warm_provider failed: {}", e);
                    error!("Status: {}", warm_status_message());
                    1 // Failure
                }
            }
        }
        Ok(Err(e)) => {
            error!("Failed to initialize embedding models: {}", e);
            error!("Status: {}", warm_status_message());
            error!(
                "\nEnsure:\n\
                 - CUDA 13.2 is installed and /usr/local/cuda points to it\n\
                 - RTX 5090 or compatible GPU is available\n\
                 - Models are downloaded (check CONTEXT_GRAPH_MODELS_PATH)"
            );
            1 // Failure
        }
        Err(_) => {
            error!(
                "Timeout: Model loading took longer than {} seconds",
                args.timeout_secs
            );
            error!("Status: {}", warm_status_message());
            1 // Failure
        }
    }
}
