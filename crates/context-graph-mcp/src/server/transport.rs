//! Transport implementations for the MCP server.
//!
//! Contains TCP and SSE transport code extracted from the main server module.
//! TASK-INTEG-018: TCP transport with concurrent client handling.
//! TASK-42: SSE transport for web client support.

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::transport::{create_sse_router, SseAppState, SseConfig};

use super::McpServer;

// ============================================================================
// AGT-04 FIX: Bounded read_line to prevent OOM from unbounded input
// ============================================================================

/// Maximum line size in bytes (10 MB). Lines exceeding this are rejected.
/// Prevents OOM from clients sending multi-gigabyte data without newlines.
pub(in crate::server) const MAX_LINE_BYTES: usize = 10 * 1024 * 1024;

/// Read a line from an async buffered reader with a byte size limit.
///
/// AGT-04 FIX: `BufReader::read_line()` allocates unboundedly until it finds
/// a newline. A malicious client can send gigabytes without a newline, causing
/// OOM. This function reads in chunks via `fill_buf()` and enforces a limit.
///
/// Returns the number of bytes read, or an IO error if the limit is exceeded.
pub(in crate::server) async fn read_line_bounded<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut String,
    max_bytes: usize,
) -> std::io::Result<usize> {
    let mut total = 0usize;
    let mut raw = Vec::new();

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            // EOF
            break;
        }

        // Find newline in available data
        let (end, found_newline) = match available.iter().position(|&b| b == b'\n') {
            Some(pos) => (pos + 1, true),
            None => (available.len(), false),
        };

        if total + end > max_bytes {
            // Consume what we've seen so far to avoid leaving stale data in the buffer
            reader.consume(end);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Line exceeds {} byte limit ({} bytes read so far)",
                    max_bytes,
                    total + end
                ),
            ));
        }

        raw.extend_from_slice(&available[..end]);
        total += end;
        reader.consume(end);

        if found_newline {
            break;
        }
    }

    // Convert to UTF-8
    match String::from_utf8(raw) {
        Ok(s) => buf.push_str(&s),
        Err(e) => buf.push_str(&String::from_utf8_lossy(e.as_bytes())),
    }

    Ok(total)
}

// ============================================================================
// TASK-INTEG-018: TCP Transport Implementation
// ============================================================================

impl McpServer {
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
        request_timeout: u64,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();

            // AGT-04 FIX: Use bounded read_line to prevent OOM from unbounded input.
            // TCP is externally exploitable - a malicious client can send multi-GB data without newlines.
            let bytes_read = read_line_bounded(&mut reader, &mut line, MAX_LINE_BYTES).await?;

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

            // HIGH-15 FIX: Apply request timeout to prevent unbounded request processing.
            let response = match tokio::time::timeout(
                Duration::from_secs(request_timeout),
                handlers.dispatch(request),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    error!(
                        "Request timed out after {}s for peer {}",
                        request_timeout, peer_addr
                    );
                    JsonRpcResponse::error(
                        None,
                        crate::protocol::error_codes::TCP_CLIENT_TIMEOUT,
                        format!(
                            "Request timed out after {}s. Consider increasing request_timeout.",
                            request_timeout
                        ),
                    )
                }
            };

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
