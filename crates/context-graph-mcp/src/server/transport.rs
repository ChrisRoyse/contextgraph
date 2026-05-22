//! Transport implementations for the MCP server.
//!
//! Contains TCP transport code extracted from the main server module.
//! TASK-INTEG-018: TCP transport with concurrent client handling.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Global connection counter for unique per-client IDs.
/// Monotonically incrementing, never resets. Used for log correlation
/// when multiple Claude Code terminals connect to the same daemon.
static CONNECTION_COUNTER: AtomicU64 = AtomicU64::new(0);

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

use super::McpServer;

// ============================================================================
// AGT-04 FIX: Bounded read_line to prevent OOM from unbounded input
// ============================================================================

/// Maximum line size in bytes (10 MB). Lines exceeding this are rejected.
/// Prevents OOM from clients sending multi-gigabyte data without newlines.
pub const MAX_LINE_BYTES: usize = 10 * 1024 * 1024;

/// Maximum number of requests in a JSON-RPC 2.0 batch.
/// L7 FIX: Prevents DoS via oversized batch arrays that could exhaust memory or CPU.
pub const MAX_BATCH_SIZE: usize = 100;

/// Read a line from an async buffered reader with a byte size limit.
///
/// AGT-04 FIX: `BufReader::read_line()` allocates unboundedly until it finds
/// a newline. A malicious client can send gigabytes without a newline, causing
/// OOM. This function reads in chunks via `fill_buf()` and enforces a limit.
///
/// Returns the number of bytes read, or an IO error if the limit is exceeded.
pub async fn read_line_bounded<R: tokio::io::AsyncBufRead + Unpin>(
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
            // Consume current chunk, then drain the rest of this line so the
            // next call doesn't pick up the tail as a phantom message.
            reader.consume(end);
            if !found_newline {
                loop {
                    let rest = reader.fill_buf().await?;
                    if rest.is_empty() {
                        break; // EOF
                    }
                    let drain_end = match rest.iter().position(|&b| b == b'\n') {
                        Some(pos) => {
                            reader.consume(pos + 1);
                            break;
                        }
                        None => rest.len(),
                    };
                    reader.consume(drain_end);
                }
            }
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

    // Convert to UTF-8. Fail closed: JSON-RPC 2.0 messages are JSON text per
    // RFC 8259 §8.1, which mandates UTF-8. Lossy conversion would silently
    // substitute U+FFFD inside string content; if the substitution happens to
    // produce structurally valid JSON the server would process a request with
    // silently corrupted strings (e.g. file paths or sha256 hashes). Better to
    // reject the frame and surface a parse error to the client so the malformed
    // input is observable.
    match String::from_utf8(raw) {
        Ok(s) => buf.push_str(&s),
        Err(e) => {
            warn!(
                "Non-UTF-8 input rejected: {} bytes; offending UTF-8 error at byte {}",
                e.as_bytes().len(),
                e.utf8_error().valid_up_to()
            );
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Non-UTF-8 input rejected: JSON-RPC requires UTF-8 per RFC 8259 (utf8_error_at_byte={})",
                    e.utf8_error().valid_up_to()
                ),
            ));
        }
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

            // Assign a unique, human-readable connection ID
            let conn_id = CONNECTION_COUNTER.fetch_add(1, Ordering::Relaxed);
            let conn_tag = format!("C{:03}", conn_id);

            // Spawn client handler task
            tokio::spawn(async move {
                // Acquire semaphore permit (blocks if at max_connections)
                let _permit = match semaphore.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        error!(
                            "[{}] Semaphore closed unexpectedly for client {}",
                            conn_tag, peer_addr
                        );
                        return;
                    }
                };

                // Track active connection count
                let conn_count = active_connections.fetch_add(1, Ordering::SeqCst) + 1;
                info!(
                    "[{}] Client connected: {} (active={})",
                    conn_tag, peer_addr, conn_count
                );

                // Handle client - permit is held until this returns
                if let Err(e) =
                    Self::handle_tcp_client(stream, peer_addr, handlers, request_timeout, &conn_tag)
                        .await
                {
                    // Log at different levels based on error type
                    if e.to_string().contains("connection reset")
                        || e.to_string().contains("broken pipe")
                    {
                        debug!("[{}] Client {} disconnected: {}", conn_tag, peer_addr, e);
                    } else {
                        warn!("[{}] Client {} error: {}", conn_tag, peer_addr, e);
                    }
                }

                // Decrement active connection count
                let conn_count = active_connections.fetch_sub(1, Ordering::SeqCst) - 1;
                info!(
                    "[{}] Client disconnected: {} (active={})",
                    conn_tag, peer_addr, conn_count
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
        conn_tag: &str,
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
                debug!(
                    "[{}] Client {} closed connection (EOF)",
                    conn_tag, peer_addr
                );
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // INFRA-H3 FIX: JSON-RPC 2.0 batch request support.
            // A JSON array `[{...},{...}]` is a batch request per spec section 6.
            // Must be handled before the single-request code path.
            if trimmed.starts_with('[') {
                let batch_values: Vec<serde_json::Value> = match serde_json::from_str(trimmed) {
                    Ok(reqs) => reqs,
                    Err(e) => {
                        warn!(
                            "[{}] {} sent invalid batch JSON: {}",
                            conn_tag, peer_addr, e
                        );
                        let error_response = JsonRpcResponse::error(
                            None,
                            crate::protocol::error_codes::PARSE_ERROR,
                            format!("Batch parse error: {}", e),
                        );
                        let response_json = serde_json::to_string(&error_response)?;
                        writer.write_all(response_json.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                        // Unlike single parse errors, batch parse errors don't disconnect.
                        // The framing is still valid (we got a complete NDJSON line).
                        continue;
                    }
                };

                // JSON-RPC 2.0 spec: empty batch array is an invalid request
                if batch_values.is_empty() {
                    let error_response = JsonRpcResponse::error(
                        None,
                        crate::protocol::error_codes::INVALID_REQUEST,
                        "Empty batch request",
                    );
                    let response_json = serde_json::to_string(&error_response)?;
                    writer.write_all(response_json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }

                // L7 FIX: Reject oversized batch requests to prevent DoS.
                if batch_values.len() > MAX_BATCH_SIZE {
                    warn!(
                        "[{}] {} sent batch with {} items (max {})",
                        conn_tag,
                        peer_addr,
                        batch_values.len(),
                        MAX_BATCH_SIZE
                    );
                    let error_response = JsonRpcResponse::error(
                        None,
                        crate::protocol::error_codes::INVALID_REQUEST,
                        format!(
                            "Batch too large: {} requests exceeds maximum of {}",
                            batch_values.len(),
                            MAX_BATCH_SIZE
                        ),
                    );
                    let response_json = serde_json::to_string(&error_response)?;
                    writer.write_all(response_json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }

                debug!(
                    "[{}] {} processing batch request with {} items",
                    conn_tag,
                    peer_addr,
                    batch_values.len()
                );

                let mut batch_responses: Vec<JsonRpcResponse> =
                    Vec::with_capacity(batch_values.len());

                for req_value in &batch_values {
                    // Parse individual request from the batch element
                    let request: JsonRpcRequest = match serde_json::from_value(req_value.clone()) {
                        Ok(r) => r,
                        Err(e) => {
                            warn!(
                                "[{}] {} batch element parse error: {}",
                                conn_tag, peer_addr, e
                            );
                            // Per JSON-RPC 2.0 spec, individual parse errors get individual
                            // error responses within the batch response array.
                            batch_responses.push(JsonRpcResponse::error(
                                None,
                                crate::protocol::error_codes::INVALID_REQUEST,
                                format!("Invalid request in batch: {}", e),
                            ));
                            continue;
                        }
                    };

                    // Validate JSON-RPC version
                    if request.jsonrpc != "2.0" {
                        batch_responses.push(JsonRpcResponse::error(
                            request.id.clone(),
                            crate::protocol::error_codes::INVALID_REQUEST,
                            "Invalid JSON-RPC version. Expected '2.0'.",
                        ));
                        continue;
                    }

                    // Apply per-request timeout
                    let request_id = request.id.clone();
                    let is_notification = request_id.is_none();
                    let response = match tokio::time::timeout(
                        Duration::from_secs(request_timeout),
                        handlers.dispatch(request),
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => {
                            error!(
                                "[{}] Batch sub-request timed out after {}s for {}",
                                conn_tag, request_timeout, peer_addr
                            );
                            if is_notification {
                                warn!(
                                    "[{}] {} batch notification timed out -- suppressing (JSON-RPC 2.0)",
                                    conn_tag, peer_addr
                                );
                                continue;
                            }
                            JsonRpcResponse::error(
                                request_id,
                                crate::protocol::error_codes::TCP_CLIENT_TIMEOUT,
                                format!("Batch sub-request timed out after {}s", request_timeout),
                            )
                        }
                    };

                    // Skip notification responses (no id, no result, no error)
                    if response.id.is_none()
                        && response.result.is_none()
                        && response.error.is_none()
                    {
                        continue;
                    }
                    batch_responses.push(response);
                }

                // JSON-RPC 2.0 spec: if all requests are notifications, send nothing
                if !batch_responses.is_empty() {
                    let response_json = serde_json::to_string(&batch_responses)?;
                    debug!(
                        "[{}] {} sending batch response with {} items",
                        conn_tag,
                        peer_addr,
                        batch_responses.len()
                    );
                    writer.write_all(response_json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                }
                continue;
            }

            debug!("[{}] {} received: {}", conn_tag, peer_addr, trimmed);

            // Parse request
            let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    // TCP disconnects on parse error (returns Err → connection closes) unlike
                    // stdio which continues reading. TCP parse errors are unrecoverable since
                    // the byte stream may be misaligned — no line-delimited framing guarantee.
                    warn!(
                        "[{}] {} sent invalid JSON, sending error and closing: {}",
                        conn_tag, peer_addr, e
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
                    return Err(anyhow::anyhow!(
                        "[{}] Client sent invalid JSON-RPC: {}",
                        conn_tag,
                        e
                    ));
                }
            };

            // Log tool calls with connection tag for multi-agent correlation
            if request.method == "tools/call" {
                if let Some(params) = &request.params {
                    if let Some(tool_name) = params.get("name").and_then(|v| v.as_str()) {
                        info!(
                            "[{}] {} → {} (id={:?})",
                            conn_tag, peer_addr, tool_name, request.id
                        );
                    }
                }
            }

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
            // MCP-H2 FIX: Clone request.id before dispatch consumes the request,
            // so timeout errors can include the correct id per JSON-RPC 2.0 spec.
            let request_id = request.id.clone();
            let is_notification = request_id.is_none();
            let response = match tokio::time::timeout(
                Duration::from_secs(request_timeout),
                handlers.dispatch(request),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    error!(
                        "[{}] Request timed out after {}s for {}",
                        conn_tag, request_timeout, peer_addr
                    );
                    // Audit-7 MCP7-M1 FIX: Notifications MUST NOT receive responses per
                    // JSON-RPC 2.0 spec. If a notification times out, log the error but
                    // do NOT create an error response -- the suppression check below would
                    // fail because error=Some, sending a spurious response to the client.
                    if is_notification {
                        warn!(
                            "[{}] {} notification timed out -- suppressing error response (JSON-RPC 2.0)",
                            conn_tag, peer_addr
                        );
                        continue;
                    }
                    JsonRpcResponse::error(
                        request_id,
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
                debug!(
                    "[{}] {} notification handled, no response",
                    conn_tag, peer_addr
                );
                continue;
            }

            // Send response
            let response_json = serde_json::to_string(&response)?;
            debug!("[{}] {} sending: {}", conn_tag, peer_addr, response_json);

            writer.write_all(response_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn read_line_bounded_fails_closed_on_non_utf8() {
        // RFC 8259 §8.1: JSON text exchanged between systems MUST be UTF-8.
        // The previous implementation silently substituted U+FFFD via
        // `from_utf8_lossy`, which would corrupt string contents (file paths,
        // sha256 hashes, etc.) without surfacing the malformed input to the
        // client. The new behavior rejects the frame with a structured
        // io::Error so the JSON-RPC layer can return a PARSE_ERROR response.
        let bytes: &[u8] = &[b'{', 0xff, b'}', b'\n'];
        let mut reader = BufReader::new(bytes);
        let mut buf = String::new();
        let result = read_line_bounded(&mut reader, &mut buf, MAX_LINE_BYTES).await;
        let err = result.expect_err("non-UTF-8 input must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        let msg = err.to_string();
        assert!(
            msg.contains("Non-UTF-8 input rejected"),
            "missing fail-closed marker: {msg}"
        );
    }

    #[tokio::test]
    async fn read_line_bounded_accepts_pure_utf8() {
        let bytes: &[u8] = "{\"hello\":\"\u{00e9}\"}\n".as_bytes();
        let mut reader = BufReader::new(bytes);
        let mut buf = String::new();
        let n = read_line_bounded(&mut reader, &mut buf, MAX_LINE_BYTES)
            .await
            .expect("valid utf-8 read");
        assert_eq!(n, bytes.len());
        assert_eq!(buf, "{\"hello\":\"\u{00e9}\"}\n");
    }
}
