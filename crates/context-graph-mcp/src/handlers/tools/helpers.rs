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
pub(crate) enum ToolErrorKind {
    /// Invalid request parameters (bad format, out of range, missing required fields)
    Validation,
    /// Storage layer failure (RocksDB read/write, serialization)
    Storage,
    /// Requested resource not found (memory, fingerprint, session)
    NotFound,
    /// General execution failure (internal error, unexpected state)
    Execution,
}

impl ToolErrorKind {
    /// Returns the JSON-RPC error code and human-readable label for this error kind.
    pub(crate) fn code_and_label(self) -> (i32, &'static str) {
        match self {
            Self::Validation => (error_codes::INVALID_PARAMS, "VALIDATION_ERROR"),
            Self::Storage => (error_codes::STORAGE_ERROR, "STORAGE_ERROR"),
            Self::NotFound => (error_codes::NODE_NOT_FOUND, "NOT_FOUND"),
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
                "structuredContent": data,
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
    #[allow(clippy::result_large_err)]
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

    /// Embed a query string using all 14 embedders and return the fingerprint.
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
    #[allow(clippy::result_large_err)]
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

// =============================================================================
// SHARED MATH UTILITIES
// =============================================================================

/// Compute cosine similarity between two dense vectors.
///
/// LOW-15: Consolidated from 4 identical private implementations in
/// robustness_tools.rs, keyword_tools.rs, code_tools.rs, consolidation.rs.
///
/// Returns cosine similarity normalized to [0.0, 1.0] per SRC-3 convention.
/// Uses `(raw + 1) / 2` to map [-1, 1] → [0, 1], matching core retrieval pipeline.
/// Returns 0.5 if either vector is empty, lengths differ, or either has zero norm
/// (0.5 = orthogonal in normalized space).
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.5;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.5;
    }

    let raw = (dot / (norm_a * norm_b)).clamp(-1.0, 1.0);
    // SRC-3: Normalize [-1,1] → [0,1]
    (raw + 1.0) / 2.0
}

/// Compute variance of vector components (measures how spread out activations are).
///
/// Used by both `infer_graph_direction` (E8) and `infer_causal_direction` (E5)
/// to determine directional strength from asymmetric embeddings.
pub(crate) fn component_variance_f32(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let n = v.len() as f32;
    let mean = v.iter().sum::<f32>() / n;
    v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / n
}

/// Compute human-readable position label for sequence numbers.
///
/// MT-L1: Deduplicated from memory_tools.rs and sequence_tools.rs.
///
/// Returns labels like:
/// - "current turn" (same sequence)
/// - "previous turn" (1 turn ago)
/// - "2 turns ago" (2 turns ago)
/// - "N turns ago" (N turns ago)
/// - "next turn" / "N turns ahead" (if result_seq > current_seq)
///
/// # Arguments
/// * `result_seq` - The session sequence of the result
/// * `current_seq` - The current session sequence
pub(crate) fn compute_position_label(result_seq: u64, current_seq: u64) -> String {
    if result_seq == current_seq {
        "current turn".to_string()
    } else if result_seq < current_seq {
        let turns_ago = current_seq - result_seq;
        if turns_ago == 1 {
            "previous turn".to_string()
        } else {
            format!("{} turns ago", turns_ago)
        }
    } else {
        // Future turn (shouldn't normally happen, but handle gracefully)
        let turns_ahead = result_seq - current_seq;
        if turns_ahead == 1 {
            "next turn".to_string()
        } else {
            format!("{} turns ahead", turns_ahead)
        }
    }
}

/// Compute importance decay factor based on time since last access.
///
/// Uses exponential half-life decay: `factor = 0.5^(days_since_access / half_life_days)`
///
/// - half_life_days = 30 (importance halves every 30 days without access)
/// - factor is clamped to [0.01, 1.0] (never fully zeroes out)
///
/// # Arguments
/// * `last_accessed_at` - When memory was last accessed
/// * `now` - Current time
#[allow(dead_code)] // Infrastructure for scoring-time importance decay
pub fn importance_decay_factor(
    last_accessed_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> f32 {
    const HALF_LIFE_DAYS: f64 = 30.0;
    const MIN_DECAY_FACTOR: f32 = 0.01;

    let days_since_access = (now - last_accessed_at).num_seconds().max(0) as f64 / 86400.0;
    let factor = (0.5f64).powf(days_since_access / HALF_LIFE_DAYS) as f32;
    factor.clamp(MIN_DECAY_FACTOR, 1.0)
}
