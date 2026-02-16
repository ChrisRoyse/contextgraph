//! MCP Client for CLI commands.
//!
//! Connects to running MCP server via TCP to use warm-loaded embedding models.
//! ELIMINATES StubMultiArrayProvider - all embeddings go through MCP server.
//!
//! # Task 14: Connect CLI to MCP Server
//!
//! This module solves the architectural bug where CLI commands used
//! StubMultiArrayProvider (zeroed embeddings) instead of connecting to
//! the MCP server which has warm-loaded GPU models.
//!
//! # Constitution Compliance
//!
//! - ARCH-01: TeleologicalArray is atomic (MCP server handles all 13 embeddings)
//! - ARCH-06: All memory ops through MCP tools
//! - ARCH-08: CUDA GPU required (MCP server uses GPU, not CLI)
//! - AP-06: No direct DB access - MCP tools only
//! - AP-07: No CPU fallback in production

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};

// =============================================================================
// Constants (AP-12: No magic numbers)
// =============================================================================

/// Default MCP server hostname.
const DEFAULT_MCP_HOST: &str = "127.0.0.1";

/// Default MCP server TCP port.
const DEFAULT_MCP_PORT: u16 = 3100;

/// Connection timeout in milliseconds (5 seconds).
const CONNECTION_TIMEOUT_MS: u64 = 5000;

/// Request timeout in milliseconds (30 seconds).
const REQUEST_TIMEOUT_MS: u64 = 30000;

/// Fast path connection timeout (500ms) - for time-critical hooks.
const FAST_CONNECTION_TIMEOUT_MS: u64 = 500;

/// Fast path request timeout (800ms) - for time-critical hooks like user_prompt_submit.
const FAST_REQUEST_TIMEOUT_MS: u64 = 800;

// =============================================================================
// JSON-RPC Types
// =============================================================================

/// JSON-RPC 2.0 request structure.
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: serde_json::Value,
}

/// JSON-RPC 2.0 response structure.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error structure.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// =============================================================================
// MCP Client Error Types
// =============================================================================

/// MCP client errors with specific exit codes.
#[derive(Debug, thiserror::Error)]
pub enum McpClientError {
    /// MCP server not running or unreachable.
    #[error("MCP server not running on {host}:{port}: {source}")]
    ServerNotRunning {
        host: String,
        port: u16,
        source: std::io::Error,
    },

    /// Connection timeout exceeded.
    #[error("Connection timeout after {timeout_ms}ms to {host}:{port}")]
    ConnectionTimeout {
        host: String,
        port: u16,
        timeout_ms: u64,
    },

    /// Request timeout exceeded.
    #[error("Request timeout after {timeout_ms}ms")]
    RequestTimeout { timeout_ms: u64 },

    /// MCP server returned an error.
    #[error("MCP error {code}: {message}")]
    McpError { code: i32, message: String },

    /// IO error during communication.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Response missing expected result.
    #[error("No result in MCP response")]
    NoResult,
}

// =============================================================================
// MCP Client
// =============================================================================

/// MCP TCP client for CLI commands.
///
/// Connects to the running MCP server to leverage warm-loaded GPU models
/// instead of using local stub embeddings.
///
/// # Environment Variables
///
/// - `CONTEXT_GRAPH_MCP_HOST`: MCP server hostname (default: 127.0.0.1)
/// - `CONTEXT_GRAPH_MCP_PORT`: MCP server TCP port (default: 3100)
///
/// # Example
///
/// ```rust,ignore
/// let client = McpClient::new();
/// let result = client.store_memory("Test content", 0.5, "text", None).await?;
/// ```
pub struct McpClient {
    host: String,
    port: u16,
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl McpClient {
    /// Create a new MCP client.
    ///
    /// Reads configuration from environment variables:
    /// - `CONTEXT_GRAPH_MCP_HOST` (default: 127.0.0.1)
    /// - `CONTEXT_GRAPH_MCP_PORT` (default: 3100)
    pub fn new() -> Self {
        let host = std::env::var("CONTEXT_GRAPH_MCP_HOST")
            .unwrap_or_else(|_| DEFAULT_MCP_HOST.to_string());
        let port = std::env::var("CONTEXT_GRAPH_MCP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_MCP_PORT);

        debug!("MCP client configured: {}:{}", host, port);

        Self { host, port }
    }

    /// Create a new MCP client with explicit host and port.
    pub fn with_address(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    /// Check if the MCP server is reachable.
    ///
    /// Attempts a quick TCP connection to verify the server is running.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if server is reachable
    /// - `Ok(false)` if server is not running
    /// - `Err(...)` if connection check fails unexpectedly
    pub async fn is_server_running(&self) -> Result<bool, McpClientError> {
        let addr = format!("{}:{}", self.host, self.port);

        match tokio::time::timeout(
            std::time::Duration::from_millis(1000),
            TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(_stream)) => {
                debug!("MCP server is reachable at {}", addr);
                Ok(true)
            }
            Ok(Err(_)) | Err(_) => {
                debug!("MCP server not reachable at {}", addr);
                Ok(false)
            }
        }
    }

    /// Call the `store_memory` MCP tool.
    ///
    /// Uses warm-loaded models on MCP server to generate real embeddings.
    ///
    /// # Arguments
    ///
    /// - `content`: Memory content to store
    /// - `importance`: Importance score [0.0, 1.0]
    /// - `modality`: Content type (text, code, etc.)
    /// - `tags`: Optional tags for categorization
    /// - `session_id`: Optional session ID for session-scoped storage
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value.
    pub async fn store_memory(
        &self,
        content: &str,
        importance: f64,
        modality: &str,
        tags: Option<Vec<String>>,
        session_id: Option<&str>,
    ) -> Result<serde_json::Value, McpClientError> {
        let mut arguments = json!({
            "content": content,
            "importance": importance,
            "modality": modality,
            "tags": tags.unwrap_or_default()
        });

        // SESSION-ID-FIX: Add sessionId if provided
        if let Some(sid) = session_id {
            arguments["sessionId"] = json!(sid);
        }

        let params = json!({
            "name": "store_memory",
            "arguments": arguments
        });

        info!(
            content_len = content.len(),
            importance,
            modality,
            session_id,
            "Calling MCP store_memory"
        );

        self.call_tool(params).await
    }

    /// Call the `inject_context` MCP tool.
    ///
    /// Uses warm-loaded models on MCP server for embedding and UTL processing.
    ///
    /// # Arguments
    ///
    /// - `content`: Context content to inject
    /// - `rationale`: Reason for storing this context
    /// - `importance`: Importance score [0.0, 1.0]
    /// - `session_id`: Optional session ID for session-scoped storage
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value.
    pub async fn inject_context(
        &self,
        content: &str,
        rationale: &str,
        importance: f64,
        session_id: Option<&str>,
    ) -> Result<serde_json::Value, McpClientError> {
        let mut arguments = json!({
            "content": content,
            "rationale": rationale,
            "importance": importance
        });

        // SESSION-ID-FIX: Add sessionId if provided
        if let Some(sid) = session_id {
            arguments["sessionId"] = json!(sid);
        }

        let params = json!({
            "name": "inject_context",
            "arguments": arguments
        });

        info!(
            content_len = content.len(),
            importance,
            session_id,
            "Calling MCP inject_context"
        );

        self.call_tool(params).await
    }

    /// Call the `search_graph` MCP tool.
    ///
    /// Searches the knowledge graph using semantic similarity.
    ///
    /// # Arguments
    ///
    /// - `query`: Search query text
    /// - `top_k`: Maximum number of results to return (default: 10)
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing matching memories.
    pub async fn search_graph(
        &self,
        query: &str,
        top_k: Option<u32>,
    ) -> Result<serde_json::Value, McpClientError> {
        self.search_graph_with_content(query, top_k, false).await
    }

    /// Call the `search_graph` MCP tool with content inclusion option.
    ///
    /// Searches the knowledge graph using semantic similarity with optional
    /// content hydration for context injection.
    ///
    /// # Arguments
    ///
    /// - `query`: Search query text
    /// - `top_k`: Maximum number of results to return (default: 10)
    /// - `include_content`: If true, includes full content text in results
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing matching memories.
    pub async fn search_graph_with_content(
        &self,
        query: &str,
        top_k: Option<u32>,
        include_content: bool,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "search_graph",
            "arguments": {
                "query": query,
                "topK": top_k.unwrap_or(10),
                "includeContent": include_content
            }
        });

        info!(
            query_len = query.len(),
            top_k,
            include_content,
            "Calling MCP search_graph"
        );

        self.call_tool(params).await
    }

    /// Fast-path search for time-critical hooks (user_prompt_submit).
    ///
    /// Uses shorter timeouts (500ms connection, 800ms request) to ensure
    /// the hook completes within its 2-second budget.
    pub async fn search_graph_fast(
        &self,
        query: &str,
        top_k: Option<u32>,
        include_content: bool,
        min_similarity: Option<f32>,
    ) -> Result<serde_json::Value, McpClientError> {
        let mut arguments = json!({
            "query": query,
            "topK": top_k.unwrap_or(10),
            "includeContent": include_content
        });

        if let Some(min_sim) = min_similarity {
            arguments["minSimilarity"] = json!(min_sim);
        }

        let params = json!({
            "name": "search_graph",
            "arguments": arguments
        });

        debug!(
            query_len = query.len(),
            top_k,
            include_content,
            min_similarity,
            "Calling MCP search_graph (fast path)"
        );

        self.call_tool_fast(params).await
    }

    /// Fast-path divergence alerts for time-critical hooks.
    ///
    /// Uses shorter timeouts to ensure the hook completes within budget.
    pub async fn get_divergence_alerts_fast(
        &self,
        lookback_hours: Option<u32>,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "get_divergence_alerts",
            "arguments": {
                "lookback_hours": lookback_hours.unwrap_or(2)
            }
        });

        debug!(
            lookback_hours = lookback_hours.unwrap_or(2),
            "Calling MCP get_divergence_alerts (fast path)"
        );

        self.call_tool_fast(params).await
    }

    /// Fast-path get_conversation_context for recent session turns.
    ///
    /// Uses shorter timeouts to ensure the hook completes within budget.
    /// Returns memories around the current conversation turn with position labels.
    ///
    /// # Arguments
    ///
    /// - `direction`: "before", "after", or "both" (default: "before")
    /// - `window_size`: Number of turns to retrieve (1-50, default: 5)
    /// - `include_content`: Include full text in results (default: true)
    ///
    /// # Returns
    ///
    /// The MCP tool result with memories and sequence info including position labels.
    pub async fn get_conversation_context_fast(
        &self,
        direction: Option<&str>,
        window_size: Option<u32>,
        include_content: bool,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "get_conversation_context",
            "arguments": {
                "direction": direction.unwrap_or("before"),
                "windowSize": window_size.unwrap_or(5),
                "sessionOnly": true,
                "includeContent": include_content
            }
        });

        debug!(
            direction = direction.unwrap_or("before"),
            window_size = window_size.unwrap_or(5),
            include_content,
            "Calling MCP get_conversation_context (fast path)"
        );

        self.call_tool_fast(params).await
    }

    /// Call the `get_memetic_status` MCP tool.
    ///
    /// Gets current system status with UTL metrics.
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing system status.
    pub async fn get_memetic_status(&self) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "get_memetic_status",
            "arguments": {}
        });

        info!("Calling MCP get_memetic_status");

        self.call_tool(params).await
    }

    /// Call the `get_divergence_alerts` MCP tool.
    ///
    /// Checks for divergence from recent activity using SEMANTIC embedders only
    /// (E1, E5, E6, E7, E10, E12, E13). Temporal embedders (E2-E4) are excluded
    /// per AP-62, AP-63.
    ///
    /// # Arguments
    ///
    /// - `lookback_hours`: Hours to look back for recent activity comparison (1-48, default: 2)
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing divergence alerts.
    pub async fn get_divergence_alerts(
        &self,
        lookback_hours: Option<u32>,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "get_divergence_alerts",
            "arguments": {
                "lookback_hours": lookback_hours.unwrap_or(2)
            }
        });

        info!(
            lookback_hours = lookback_hours.unwrap_or(2),
            "Calling MCP get_divergence_alerts"
        );

        self.call_tool(params).await
    }

    /// Call the `get_topic_portfolio` MCP tool.
    ///
    /// Gets all discovered topics with profiles and stability metrics.
    /// Topics emerge from weighted multi-space clustering (threshold >= 2.5).
    ///
    /// # Arguments
    ///
    /// - `format`: Output format - "brief", "standard", or "verbose"
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing topic portfolio.
    pub async fn get_topic_portfolio(
        &self,
        format: Option<&str>,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "get_topic_portfolio",
            "arguments": {
                "format": format.unwrap_or("standard")
            }
        });

        info!(
            format = format.unwrap_or("standard"),
            "Calling MCP get_topic_portfolio"
        );

        self.call_tool(params).await
    }

    /// Call the `get_topic_stability` MCP tool.
    ///
    /// Gets portfolio-level stability metrics including churn rate and entropy.
    /// Dream consolidation is recommended when entropy > 0.7 AND churn > 0.5.
    ///
    /// # Arguments
    ///
    /// - `hours`: Lookback period in hours for computing averages (1-168, default: 6)
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing stability metrics.
    pub async fn get_topic_stability(
        &self,
        hours: Option<u32>,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "get_topic_stability",
            "arguments": {
                "hours": hours.unwrap_or(6)
            }
        });

        info!(
            hours = hours.unwrap_or(6),
            "Calling MCP get_topic_stability"
        );

        self.call_tool(params).await
    }

    /// Call the `detect_topics` MCP tool.
    ///
    /// Force topic detection recalculation using HDBSCAN clustering.
    /// Requires minimum 3 memories (per clustering.parameters.min_cluster_size).
    /// Topics require weighted_agreement >= 2.5 to be recognized.
    ///
    /// # Arguments
    ///
    /// - `force`: Force detection even if recently computed
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing detected topics.
    pub async fn detect_topics(
        &self,
        force: bool,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "detect_topics",
            "arguments": {
                "force": force
            }
        });

        info!(force, "Calling MCP detect_topics");

        self.call_tool(params).await
    }

    /// Call the `trigger_dream` MCP tool.
    ///
    /// Execute NREM/REM dream consolidation cycle.
    ///
    /// # Arguments
    ///
    /// - `blocking`: Wait for completion (default: true)
    /// - `dry_run`: Simulate without modifying graph (default: false)
    /// - `skip_nrem`: Skip NREM phase (default: false)
    /// - `skip_rem`: Skip REM phase (default: false)
    /// - `max_duration_secs`: Max duration in seconds (10-600, default: 300)
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing dream status.
    pub async fn trigger_dream(
        &self,
        blocking: bool,
        dry_run: bool,
        skip_nrem: bool,
        skip_rem: bool,
        max_duration_secs: Option<u32>,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "trigger_dream",
            "arguments": {
                "blocking": blocking,
                "dry_run": dry_run,
                "skip_nrem": skip_nrem,
                "skip_rem": skip_rem,
                "max_duration_secs": max_duration_secs.unwrap_or(300)
            }
        });

        info!(
            blocking,
            dry_run,
            skip_nrem,
            skip_rem,
            max_duration_secs = max_duration_secs.unwrap_or(300),
            "Calling MCP trigger_dream"
        );

        self.call_tool(params).await
    }

    /// Call the `get_dream_status` MCP tool.
    ///
    /// Get status of a running or completed dream cycle.
    ///
    /// # Arguments
    ///
    /// - `dream_id`: Dream cycle UUID (default: most recent)
    ///
    /// # Returns
    ///
    /// The MCP tool result as JSON value containing dream status.
    pub async fn get_dream_status(
        &self,
        dream_id: Option<&str>,
    ) -> Result<serde_json::Value, McpClientError> {
        let mut args = serde_json::Map::new();
        if let Some(id) = dream_id {
            args.insert("dream_id".to_string(), json!(id));
        }

        let params = json!({
            "name": "get_dream_status",
            "arguments": args
        });

        info!(dream_id, "Calling MCP get_dream_status");

        self.call_tool(params).await
    }

    /// Internal method to call an MCP tool.
    ///
    /// Establishes TCP connection, sends JSON-RPC request, and reads response.
    async fn call_tool(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpClientError> {
        self.call_tool_with_timeout(params, CONNECTION_TIMEOUT_MS, REQUEST_TIMEOUT_MS)
            .await
    }

    /// Fast-path call for time-critical hooks (user_prompt_submit).
    ///
    /// Uses shorter timeouts (500ms connection, 800ms request) to ensure
    /// the hook completes within its 2-second budget.
    pub async fn call_tool_fast(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpClientError> {
        self.call_tool_with_timeout(params, FAST_CONNECTION_TIMEOUT_MS, FAST_REQUEST_TIMEOUT_MS)
            .await
    }

    /// Internal method to call an MCP tool with configurable timeouts.
    async fn call_tool_with_timeout(
        &self,
        params: serde_json::Value,
        connection_timeout_ms: u64,
        request_timeout_ms: u64,
    ) -> Result<serde_json::Value, McpClientError> {
        let addr = format!("{}:{}", self.host, self.port);
        debug!("Connecting to MCP server at {}", addr);

        // Connect with timeout
        let stream = tokio::time::timeout(
            std::time::Duration::from_millis(connection_timeout_ms),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| McpClientError::ConnectionTimeout {
            host: self.host.clone(),
            port: self.port,
            timeout_ms: connection_timeout_ms,
        })?
        .map_err(|e| McpClientError::ServerNotRunning {
            host: self.host.clone(),
            port: self.port,
            source: e,
        })?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Send tools/call request
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "tools/call",
            params,
        };

        let request_json = serde_json::to_string(&request)?;
        debug!("Sending: {}", request_json);

        writer.write_all(request_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read response with timeout
        let mut response_line = String::new();
        let bytes_read = tokio::time::timeout(
            std::time::Duration::from_millis(request_timeout_ms),
            reader.read_line(&mut response_line),
        )
        .await
        .map_err(|_| McpClientError::RequestTimeout {
            timeout_ms: request_timeout_ms,
        })??;

        if bytes_read == 0 {
            warn!("MCP server closed connection before responding");
            return Err(McpClientError::IoError(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Server closed connection",
            )));
        }

        debug!("Received: {}", response_line.trim());

        let response: JsonRpcResponse = serde_json::from_str(&response_line)?;

        if let Some(error) = response.error {
            error!("MCP error {}: {}", error.code, error.message);
            return Err(McpClientError::McpError {
                code: error.code,
                message: error.message,
            });
        }

        let result = response.result.ok_or(McpClientError::NoResult)?;

        // Unwrap MCP content wrapper: result.content[0].text contains the actual JSON
        Self::unwrap_mcp_response(result)
    }

    /// Unwrap MCP response content wrapper.
    ///
    /// MCP tools return responses in the format:
    /// ```json
    /// { "content": [{"text": "{...actual result...}", "type": "text"}], "isError": false }
    /// ```
    ///
    /// This function extracts the actual tool result from the wrapper.
    fn unwrap_mcp_response(response: serde_json::Value) -> Result<serde_json::Value, McpClientError> {
        // Check for error response
        if response.get("isError").and_then(|v| v.as_bool()).unwrap_or(false) {
            let error_msg = Self::extract_content_text(&response)
                .map(String::from)
                .unwrap_or_else(|| "Unknown MCP error".to_string());
            return Err(McpClientError::McpError {
                code: -32000,
                message: error_msg,
            });
        }

        // Extract content[0].text and parse as JSON
        match Self::extract_content_text(&response) {
            Some(json_str) => serde_json::from_str(json_str).map_err(|e| {
                error!("Failed to parse MCP content text: {}", e);
                McpClientError::JsonError(e)
            }),
            None => {
                // No content wrapper - return as-is (might be legacy format)
                debug!("No content wrapper found, returning raw response");
                Ok(response)
            }
        }
    }

    /// Extract text from MCP response content[0].text field.
    fn extract_content_text(resp: &serde_json::Value) -> Option<&str> {
        resp.get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
    }

    /// Get the server address string.
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    // =========================================================================
    // E11 ENTITY TOOLS - KEPLER Integration
    // Per E11_KEPLER_INTEGRATION_PLAN.md Phase 2: UserPromptSubmit Hook
    // =========================================================================

    /// Fast-path extract_entities for time-critical hooks.
    ///
    /// Extracts and canonicalizes entities from text using E11 (KEPLER) knowledge.
    /// Detects programming languages, frameworks, databases, cloud services, etc.
    ///
    /// # Arguments
    ///
    /// - `text`: Text to extract entities from
    /// - `include_unknown`: Include entities not in KB (default: true)
    ///
    /// # Returns
    ///
    /// Extracted entities with canonical IDs and types.
    pub async fn extract_entities_fast(
        &self,
        text: &str,
        include_unknown: bool,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "extract_entities",
            "arguments": {
                "text": text,
                "includeUnknown": include_unknown,
                "groupByType": false
            }
        });

        debug!(
            text_len = text.len(),
            include_unknown,
            "Calling MCP extract_entities (fast path)"
        );

        self.call_tool_fast(params).await
    }

    /// Fast-path search_by_entities for time-critical hooks.
    ///
    /// Multi-embedder discovery: searches E1 AND E11 in parallel, returns UNION.
    /// E11 finds what E1 misses (e.g., "Diesel ORM" when searching for "database").
    ///
    /// # Arguments
    ///
    /// - `entities`: Entity names to search for (will be canonicalized)
    /// - `match_mode`: "any" (match any entity) or "all" (match all entities)
    /// - `top_k`: Maximum results to return (1-50, default: 5)
    /// - `include_content`: Include full memory content in results
    ///
    /// # Returns
    ///
    /// Memories found by E1 OR E11 (E11 surfaces things E1 missed!).
    ///
    /// # Constitution Compliance
    ///
    /// - ARCH-12: E1 is semantic foundation
    /// - E11 DISCOVERS candidates E1 misses (not just boosts E1's scores)
    pub async fn search_by_entities_fast(
        &self,
        entities: &[String],
        match_mode: &str,
        top_k: u32,
        include_content: bool,
    ) -> Result<serde_json::Value, McpClientError> {
        let params = json!({
            "name": "search_by_entities",
            "arguments": {
                "entities": entities,
                "matchMode": match_mode,
                "topK": top_k,
                "minScore": 0.2,
                "includeContent": include_content,
                "boostExactMatch": 1.15
            }
        });

        debug!(
            entities = ?entities,
            match_mode,
            top_k,
            include_content,
            "Calling MCP search_by_entities (fast path) - E11 multi-embedder discovery"
        );

        self.call_tool_fast(params).await
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_utils::GLOBAL_IDENTITY_LOCK;

    #[test]
    fn test_mcp_client_new_defaults() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();

        // Clear environment to test defaults
        std::env::remove_var("CONTEXT_GRAPH_MCP_HOST");
        std::env::remove_var("CONTEXT_GRAPH_MCP_PORT");

        let client = McpClient::new();
        assert_eq!(client.host, DEFAULT_MCP_HOST);
        assert_eq!(client.port, DEFAULT_MCP_PORT);
        assert_eq!(client.server_address(), "127.0.0.1:3100");
    }

    #[test]
    fn test_mcp_client_from_env() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();

        // Clear first to ensure known state
        std::env::remove_var("CONTEXT_GRAPH_MCP_HOST");
        std::env::remove_var("CONTEXT_GRAPH_MCP_PORT");

        // Set test values
        std::env::set_var("CONTEXT_GRAPH_MCP_HOST", "192.168.1.100");
        std::env::set_var("CONTEXT_GRAPH_MCP_PORT", "9000");

        let client = McpClient::new();
        assert_eq!(client.host, "192.168.1.100");
        assert_eq!(client.port, 9000);

        // Cleanup
        std::env::remove_var("CONTEXT_GRAPH_MCP_HOST");
        std::env::remove_var("CONTEXT_GRAPH_MCP_PORT");
    }

    #[test]
    fn test_mcp_client_with_address() {
        let client = McpClient::with_address("localhost", 8080);
        assert_eq!(client.host, "localhost");
        assert_eq!(client.port, 8080);
    }

    #[test]
    fn test_mcp_client_invalid_port_env() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();

        // Clear first
        std::env::remove_var("CONTEXT_GRAPH_MCP_HOST");
        std::env::remove_var("CONTEXT_GRAPH_MCP_PORT");

        std::env::set_var("CONTEXT_GRAPH_MCP_PORT", "not-a-number");

        let client = McpClient::new();
        // Should fall back to default
        assert_eq!(client.port, DEFAULT_MCP_PORT);

        std::env::remove_var("CONTEXT_GRAPH_MCP_PORT");
    }

    #[test]
    fn test_mcp_client_error_display() {
        let server_not_running = McpClientError::ServerNotRunning {
            host: "127.0.0.1".to_string(),
            port: 3100,
            source: std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"),
        };
        let msg = format!("{}", server_not_running);
        assert!(msg.contains("MCP server not running"));
        assert!(msg.contains("127.0.0.1:3100"));

        let timeout = McpClientError::ConnectionTimeout {
            host: "localhost".to_string(),
            port: 9000,
            timeout_ms: 5000,
        };
        let msg = format!("{}", timeout);
        assert!(msg.contains("Connection timeout"));
        assert!(msg.contains("5000ms"));

        let mcp_error = McpClientError::McpError {
            code: -32600,
            message: "Invalid request".to_string(),
        };
        let msg = format!("{}", mcp_error);
        assert!(msg.contains("MCP error -32600"));
        assert!(msg.contains("Invalid request"));
    }

    #[tokio::test]
    async fn test_is_server_running_not_running() {
        // Test with a port that should not have anything running
        let client = McpClient::with_address("127.0.0.1", 59999);
        let result = client.is_server_running().await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
