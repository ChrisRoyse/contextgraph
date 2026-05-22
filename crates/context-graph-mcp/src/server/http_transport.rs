//! Streamable HTTP transport for MCP.
//!
//! This endpoint is intended for local Claude Code connections. It binds to
//! localhost and validates Host/Origin headers so a browser cannot reach the
//! local MCP server via DNS rebinding.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::header::{ACCEPT, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

use super::transport::MAX_BATCH_SIZE;
use super::McpServer;

static HTTP_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct HttpState {
    handlers: Arc<Handlers>,
    request_timeout: u64,
}

impl McpServer {
    /// Run the server using MCP Streamable HTTP on 127.0.0.1:`http_port`.
    pub async fn run_streamable_http(&self, http_port: u16) -> Result<()> {
        if http_port == 0 {
            anyhow::bail!("HTTP MCP port must be in range 1-65535, got 0");
        }

        let bind_addr: SocketAddr = format!("127.0.0.1:{http_port}").parse()?;
        let listener = TcpListener::bind(bind_addr).await.map_err(|e| {
            error!("FATAL: Failed to bind Streamable HTTP listener to {bind_addr}: {e}");
            anyhow::anyhow!(
                "Failed to bind Streamable HTTP listener to {bind_addr}: {e}. \
                 Check whether another process is using port {http_port}."
            )
        })?;

        match self.start_graph_builder().await {
            Ok(true) => info!("Background graph builder started for HTTP transport"),
            Ok(false) => debug!("Background graph builder not configured for HTTP transport"),
            Err(e) => warn!("Failed to start background graph builder for HTTP transport: {e}"),
        }

        let state = HttpState {
            handlers: Arc::clone(&self.handlers),
            request_timeout: self.config.mcp.request_timeout,
        };

        let app = Router::new()
            .route("/mcp", post(mcp_post))
            .route("/mcp", get(mcp_get_not_supported))
            .route("/healthz", get(healthz))
            .layer(DefaultBodyLimit::max(self.config.mcp.max_payload_size))
            .with_state(state);

        info!("MCP Streamable HTTP listening on http://{bind_addr}/mcp");
        axum::serve(listener, app)
            .await
            .map_err(|e| anyhow::anyhow!("Streamable HTTP server failed: {e}"))
    }
}

async fn healthz(headers: HeaderMap) -> Response {
    if let Err((status, message)) = validate_local_headers(&headers) {
        return error_response(status, message);
    }
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "transport": "streamable-http",
            "endpoint": "/mcp"
        })),
    )
        .into_response()
}

async fn mcp_get_not_supported(headers: HeaderMap) -> Response {
    if let Err((status, message)) = validate_local_headers(&headers) {
        return error_response(status, message);
    }
    error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        "GET /mcp is not enabled because this server does not emit server-initiated streams",
    )
}

async fn mcp_post(State(state): State<HttpState>, headers: HeaderMap, body: Bytes) -> Response {
    let request_id = HTTP_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let request_tag = format!("H{request_id:06}");

    if let Err((status, message)) = validate_local_headers(&headers) {
        warn!("[{request_tag}] rejected HTTP MCP request: {message}");
        return error_response(status, message);
    }
    if let Err((status, message)) = validate_mcp_post_headers(&headers) {
        warn!("[{request_tag}] rejected HTTP MCP request: {message}");
        return error_response(status, message);
    }

    let value: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => {
            warn!("[{request_tag}] invalid JSON body: {e}");
            return jsonrpc_response(
                StatusCode::BAD_REQUEST,
                JsonRpcResponse::error(
                    None,
                    crate::protocol::error_codes::PARSE_ERROR,
                    format!("Parse error: {e}"),
                ),
            );
        }
    };

    match handle_http_jsonrpc_value(&state, value, &request_tag).await {
        HttpMcpReply::Json(value) => {
            let mut response = (StatusCode::OK, Json(value)).into_response();
            response.headers_mut().insert(
                CONTENT_TYPE,
                "application/json".parse().expect("valid content type"),
            );
            response
        }
        HttpMcpReply::Accepted => StatusCode::ACCEPTED.into_response(),
    }
}

enum HttpMcpReply {
    Json(Value),
    Accepted,
}

async fn handle_http_jsonrpc_value(
    state: &HttpState,
    value: Value,
    request_tag: &str,
) -> HttpMcpReply {
    if let Some(batch) = value.as_array() {
        if batch.is_empty() {
            return HttpMcpReply::Json(jsonrpc_error_value(
                None,
                crate::protocol::error_codes::INVALID_REQUEST,
                "Empty batch request",
            ));
        }
        if batch.len() > MAX_BATCH_SIZE {
            return HttpMcpReply::Json(jsonrpc_error_value(
                None,
                crate::protocol::error_codes::INVALID_REQUEST,
                format!(
                    "Batch too large: {} requests exceeds maximum of {}",
                    batch.len(),
                    MAX_BATCH_SIZE
                ),
            ));
        }

        let mut responses = Vec::with_capacity(batch.len());
        for item in batch {
            if let Some(response) = dispatch_http_request(state, item.clone(), request_tag).await {
                responses.push(response);
            }
        }
        if responses.is_empty() {
            HttpMcpReply::Accepted
        } else {
            HttpMcpReply::Json(Value::Array(responses))
        }
    } else if let Some(response) = dispatch_http_request(state, value, request_tag).await {
        HttpMcpReply::Json(response)
    } else {
        HttpMcpReply::Accepted
    }
}

async fn dispatch_http_request(
    state: &HttpState,
    value: Value,
    request_tag: &str,
) -> Option<Value> {
    let request: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(request) => request,
        Err(e) => {
            warn!("[{request_tag}] invalid JSON-RPC request: {e}");
            return Some(jsonrpc_error_value(
                None,
                crate::protocol::error_codes::INVALID_REQUEST,
                format!("Invalid request: {e}"),
            ));
        }
    };

    if request.jsonrpc != "2.0" {
        return Some(jsonrpc_error_value(
            request.id,
            crate::protocol::error_codes::INVALID_REQUEST,
            "Invalid JSON-RPC version. Expected '2.0'.",
        ));
    }

    if request.method == "tools/call" {
        if let Some(params) = &request.params {
            if let Some(tool_name) = params.get("name").and_then(|v| v.as_str()) {
                info!(
                    "[{request_tag}] HTTP MCP tools/call: {tool_name} id={:?}",
                    request.id
                );
            }
        }
    }

    let request_id = request.id.clone();
    let is_notification = request_id.is_none();
    let response = match tokio::time::timeout(
        Duration::from_secs(state.request_timeout),
        state.handlers.dispatch(request),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            error!(
                "[{request_tag}] HTTP MCP request timed out after {}s",
                state.request_timeout
            );
            if is_notification {
                return None;
            }
            JsonRpcResponse::error(
                request_id,
                crate::protocol::error_codes::TCP_CLIENT_TIMEOUT,
                format!("Request timed out after {}s", state.request_timeout),
            )
        }
    };

    if response.id.is_none() && response.result.is_none() && response.error.is_none() {
        debug!("[{request_tag}] HTTP MCP notification handled");
        None
    } else {
        match serde_json::to_value(response) {
            Ok(value) => Some(value),
            Err(e) => Some(jsonrpc_error_value(
                None,
                crate::protocol::error_codes::INTERNAL_ERROR,
                format!("Failed to serialize response: {e}"),
            )),
        }
    }
}

fn jsonrpc_response(status: StatusCode, response: JsonRpcResponse) -> Response {
    (status, Json(response)).into_response()
}

fn jsonrpc_error_value(
    id: Option<crate::protocol::JsonRpcId>,
    code: i32,
    message: impl Into<String>,
) -> Value {
    serde_json::to_value(JsonRpcResponse::error(id, code, message)).unwrap_or_else(
        |_| json!({"jsonrpc":"2.0","error":{"code":-32603,"message":"error serialization failed"}}),
    )
}

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "code": status.as_u16(),
                "message": message.into()
            }
        })),
    )
        .into_response()
}

fn validate_mcp_post_headers(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let content_type = header_value(headers, CONTENT_TYPE).ok_or_else(|| {
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "missing Content-Type header".to_string(),
        )
    })?;
    if !header_contains(content_type, "application/json") {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("Content-Type must include application/json, got {content_type}"),
        ));
    }

    let accept = header_value(headers, ACCEPT).ok_or_else(|| {
        (
            StatusCode::NOT_ACCEPTABLE,
            "missing Accept header".to_string(),
        )
    })?;
    if !header_contains(accept, "application/json") || !header_contains(accept, "text/event-stream")
    {
        return Err((
            StatusCode::NOT_ACCEPTABLE,
            format!(
                "Accept must include application/json and text/event-stream for MCP Streamable HTTP, got {accept}"
            ),
        ));
    }

    Ok(())
}

fn validate_local_headers(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let host = header_value(headers, HOST)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing Host header".to_string()))?;
    if !is_loopback_authority(host) {
        return Err((
            StatusCode::FORBIDDEN,
            format!("Host must be localhost or loopback, got {host}"),
        ));
    }

    if let Some(origin) = header_value(headers, ORIGIN) {
        if !is_loopback_origin(origin) {
            return Err((
                StatusCode::FORBIDDEN,
                format!("Origin must be localhost or loopback, got {origin}"),
            ));
        }
    }

    Ok(())
}

fn header_value(headers: &HeaderMap, name: axum::http::header::HeaderName) -> Option<&str> {
    headers.get(name)?.to_str().ok()
}

fn header_contains(value: &str, needle: &str) -> bool {
    // Media types are case-insensitive per RFC 9110 §5.6.2. Both arms must
    // compare case-insensitively, otherwise a legal `Content-Type: Application/JSON`
    // would be rejected because `starts_with` is byte-exact and the trimmed
    // value `"Application/JSON; charset=utf-8"` does NOT start with the lower-
    // case `"application/json"`.
    value.split(',').any(|part| {
        let trimmed = part.trim();
        trimmed.eq_ignore_ascii_case(needle)
            || trimmed
                .get(..needle.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(needle))
    })
}

fn is_loopback_origin(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
    else {
        return false;
    };
    is_loopback_authority(rest)
}

fn is_loopback_authority(value: &str) -> bool {
    let authority = value.split('/').next().unwrap_or(value);
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = if authority.starts_with('[') {
        match authority.find(']') {
            Some(end) => &authority[..=end],
            None => authority,
        }
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_authority_accepts_local_hosts() {
        assert!(is_loopback_authority("127.0.0.1:3101"));
        assert!(is_loopback_authority("localhost:3101"));
        assert!(is_loopback_authority("[::1]:3101"));
    }

    #[test]
    fn loopback_authority_rejects_non_local_hosts() {
        assert!(!is_loopback_authority("example.com:3101"));
        assert!(!is_loopback_authority("192.168.1.20:3101"));
        assert!(!is_loopback_origin("https://example.com"));
    }

    #[test]
    fn header_contains_handles_parameters_and_lists() {
        assert!(header_contains(
            "application/json, text/event-stream",
            "text/event-stream"
        ));
        assert!(header_contains(
            "application/json; charset=utf-8",
            "application/json"
        ));
    }

    #[test]
    fn header_contains_is_case_insensitive_on_prefix_match_per_rfc_9110() {
        // Media types are case-insensitive per RFC 9110 §5.6.2; the previous
        // implementation used a byte-exact `starts_with` and silently rejected
        // legal headers like `Content-Type: Application/JSON; charset=utf-8`.
        assert!(header_contains(
            "Application/JSON; charset=utf-8",
            "application/json"
        ));
        assert!(header_contains("APPLICATION/JSON", "application/json"));
        assert!(header_contains(
            "TEXT/Event-Stream",
            "text/event-stream"
        ));
        // Negative case: distinct subtype must still not match.
        assert!(!header_contains(
            "application/xml; charset=utf-8",
            "application/json"
        ));
    }
}
