//! MCP tool result and request-parsing helpers.

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::validate::{Validate, ValidateInto};

/// Typed error categories for consistent MCP tool error responses.
///
/// Maps tool-level error categories to JSON-RPC error codes from protocol.rs.
/// Used by `Handlers::tool_error_typed` to produce MCP-compliant responses
/// that include the error code for machine-parseable error handling.
///
/// ## MCP Protocol Note
/// Tool errors are returned as JSON-RPC *success* responses with `isError: true`
/// in the content (per MCP spec). Protocol errors (method_not_found, invalid_request)
/// use `JsonRpcResponse::error()` — do NOT use ToolErrorKind for those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ToolErrorKind {
    /// Invalid request parameters (bad format, out of range, missing required fields)
    Validation,
    /// Storage layer failure (RocksDB read/write, serialization)
    Storage,
    /// Embedding operation failed (model not ready, OOM, dimension mismatch)
    Embedding,
    /// Requested resource not found (memory, fingerprint, session)
    NotFound,
    /// Feature or subsystem not available (disabled, not initialized)
    Unavailable,
    /// General execution failure (internal error, unexpected state)
    Execution,
}

impl ToolErrorKind {
    /// Returns the JSON-RPC error code and human-readable label for this error kind.
    pub(crate) fn code_and_label(self) -> (i32, &'static str) {
        match self {
            Self::Validation => (error_codes::INVALID_PARAMS, "VALIDATION_ERROR"),
            Self::Storage => (error_codes::STORAGE_ERROR, "STORAGE_ERROR"),
            Self::Embedding => (error_codes::EMBEDDING_ERROR, "EMBEDDING_ERROR"),
            Self::NotFound => (error_codes::NODE_NOT_FOUND, "NOT_FOUND"),
            Self::Unavailable => (error_codes::FEATURE_DISABLED, "UNAVAILABLE"),
            Self::Execution => (error_codes::INTERNAL_ERROR, "EXECUTION_ERROR"),
        }
    }
}

impl Handlers {
    /// MCP-compliant tool result helper.
    ///
    /// Wraps tool output in the required MCP format:
    /// ```json
    /// {
    ///   "content": [{"type": "text", "text": "..."}],
    ///   "isError": false
    /// }
    /// ```
    pub(crate) fn tool_result(
        &self,
        id: Option<JsonRpcId>,
        data: serde_json::Value,
    ) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
                }],
                "isError": false
            }),
        )
    }

    /// MCP-compliant tool error with typed error category.
    ///
    /// Returns an MCP-compliant error response that includes the error code
    /// for machine-parseable error handling. Format:
    /// ```json
    /// {
    ///   "content": [{"type": "text", "text": "[LABEL -CODE] message"}],
    ///   "isError": true,
    ///   "errorCode": -32xxx
    /// }
    /// ```
    pub(crate) fn tool_error_typed(
        &self,
        id: Option<JsonRpcId>,
        kind: ToolErrorKind,
        message: &str,
    ) -> JsonRpcResponse {
        let (code, label) = kind.code_and_label();
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("[{} {}] {}", label, code, message)
                }],
                "isError": true,
                "errorCode": code
            }),
        )
    }

    /// MCP-compliant tool error helper (untyped convenience).
    ///
    /// For cases where the error category is obvious from context.
    /// Prefer `tool_error_typed` for new code to include the error code.
    pub(crate) fn tool_error(&self, id: Option<JsonRpcId>, message: &str) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": message
                }],
                "isError": true
            }),
        )
    }

    /// Parse JSON args into a typed DTO and run `validate() -> Result<(), String>`.
    ///
    /// Eliminates the repeated parse+validate boilerplate across all handler
    /// methods whose DTOs implement [`Validate`].
    ///
    /// Returns `Ok(request)` on success, or an MCP error `JsonRpcResponse`
    /// on parse/validation failure.
    pub(crate) fn parse_request<T: DeserializeOwned + Validate>(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
        tool_name: &str,
    ) -> Result<T, JsonRpcResponse> {
        let request: T = serde_json::from_value(args).map_err(|e| {
            tracing::error!("[{}] Invalid request: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        request.validate().map_err(|e| {
            tracing::error!("[{}] Validation failed: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        Ok(request)
    }

    /// Embed a query string using all 13 embedders and return the fingerprint.
    ///
    /// Eliminates the repeated embed+error-handling boilerplate across ~25 search
    /// handlers. Returns the `SemanticFingerprint` on success, or an MCP error
    /// `JsonRpcResponse` on embedding failure.
    pub(crate) async fn embed_query(
        &self,
        id: Option<JsonRpcId>,
        query: &str,
        tool_name: &str,
    ) -> Result<context_graph_core::types::fingerprint::SemanticFingerprint, JsonRpcResponse> {
        self.multi_array_provider
            .embed_all(query)
            .await
            .map(|output| output.fingerprint)
            .map_err(|e| {
                tracing::error!("[{}] Embedding failed: {}", tool_name, e);
                self.tool_error(id, &format!("Embedding failed: {}", e))
            })
    }

    /// Parse JSON args into a typed DTO and run `validate() -> Result<Output, String>`.
    ///
    /// Eliminates the repeated parse+validate boilerplate across all handler
    /// methods whose DTOs implement [`ValidateInto`] (i.e. validation produces
    /// a parsed value such as `Uuid`, `Vec<Uuid>`, or `(Uuid, Uuid)`).
    ///
    /// Returns `Ok((request, validated_output))` on success, or an MCP error
    /// `JsonRpcResponse` on parse/validation failure.
    pub(crate) fn parse_request_validated<T: DeserializeOwned + ValidateInto>(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
        tool_name: &str,
    ) -> Result<(T, T::Output), JsonRpcResponse> {
        let request: T = serde_json::from_value(args).map_err(|e| {
            tracing::error!("[{}] Invalid request: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        let output = request.validate().map_err(|e| {
            tracing::error!("[{}] Validation failed: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        Ok((request, output))
    }
}
