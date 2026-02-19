//! Daemon status tool handler for multi-agent observability.
//!
//! Returns health metrics: PID, uptime, active connections, model state,
//! background task status. Used by Claude Code terminals to verify
//! multi-agent setup is working.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use tracing::debug;

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

/// Daemon runtime state shared between McpServer and Handlers.
///
/// Created by McpServer::new() and injected into Handlers via set_daemon_state()
/// before Arc-wrapping. All fields are Arc-cloned from McpServer fields for
/// zero-copy sharing.
pub(crate) struct DaemonState {
    /// Active TCP connection count (shared with McpServer::active_connections).
    pub(crate) active_connections: Arc<AtomicUsize>,
    /// Maximum allowed connections (from config.mcp.max_connections).
    pub(crate) max_connections: usize,
    /// Whether embedding models are currently loading.
    pub(crate) models_loading: Arc<AtomicBool>,
    /// Model loading failure message, if any.
    pub(crate) models_failed: Arc<tokio::sync::RwLock<Option<String>>>,
    /// Background task shutdown flag.
    pub(crate) background_shutdown: Arc<AtomicBool>,
    /// Server start time for uptime calculation.
    pub(crate) start_time: Instant,
}

impl Handlers {
    /// Set daemon state for the daemon_status tool.
    ///
    /// Must be called before wrapping Handlers in Arc.
    pub(crate) fn set_daemon_state(&mut self, state: DaemonState) {
        self.daemon_state = Some(Arc::new(state));
    }

    /// Handle daemon_status tool call.
    ///
    /// Returns health metrics for multi-agent observability:
    /// - PID of the daemon process
    /// - Uptime in seconds
    /// - Active and max connection counts
    /// - Model loading state (loading/ready/failed)
    /// - Background task status (GC, HNSW persist, graph builder)
    pub(crate) async fn call_daemon_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("daemon_status: collecting health metrics");

        let pid = std::process::id();

        let (uptime_secs, active_connections, max_connections, models_state, background_shutdown) =
            match &self.daemon_state {
                Some(state) => {
                    let uptime = state.start_time.elapsed().as_secs();
                    let active = state.active_connections.load(Ordering::SeqCst);
                    let max = state.max_connections;
                    let shutdown = state.background_shutdown.load(Ordering::SeqCst);

                    let models = if state.models_loading.load(Ordering::SeqCst) {
                        "loading".to_string()
                    } else {
                        let failed = state.models_failed.read().await;
                        match failed.as_ref() {
                            Some(err) => format!("failed: {}", err),
                            None => "ready".to_string(),
                        }
                    };

                    (uptime, active, max, models, shutdown)
                }
                None => {
                    // Daemon state not injected (stdio mode or tests)
                    (0u64, 0usize, 0usize, "unknown".to_string(), false)
                }
            };

        let graph_builder_running = self
            .graph_builder
            .as_ref()
            .map(|b| b.is_running())
            .unwrap_or(false);

        let result = json!({
            "pid": pid,
            "uptime_secs": uptime_secs,
            "active_connections": active_connections,
            "max_connections": max_connections,
            "models_state": models_state,
            "background_tasks": {
                "running": !background_shutdown,
                "graph_builder": graph_builder_running
            },
        });

        self.tool_result(id, result)
    }
}
