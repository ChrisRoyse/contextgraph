//! cgreality MCP tool family.

pub mod alter;
pub mod analyze;
pub mod autoresearch;
pub mod bandit;
pub mod diversity;
pub mod errors;
pub mod helpers;
pub mod influence;
pub mod interact;
pub mod lock;
pub mod optimizer;
pub mod path_policy;
pub mod recommendation_certificate;
pub mod recommendations;
pub mod reflexion;
pub mod schema_validator;
pub mod shift_log;
pub mod witness_chain;

#[cfg(test)]
mod recommendation_recall_active_fsv;
#[cfg(test)]
mod recommendation_recall_fsv;

pub use errors::CCRealityError;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::json;

pub(super) fn ok(
    handler: &Handlers,
    id: Option<JsonRpcId>,
    value: serde_json::Value,
) -> JsonRpcResponse {
    handler.tool_result(id, value)
}

pub(super) fn err(id: Option<JsonRpcId>, error: CCRealityError) -> JsonRpcResponse {
    tracing::error!(
        error_code = %error.error_code,
        field_path = %error.field_path,
        message = %error.message,
        source_of_truth = ?error.source_of_truth,
        details = %error.details,
        "cgreality tool call failed"
    );
    let value = error.into_value();
    eprintln!(
        "{}",
        serde_json::to_string(&json!({
            "event": "cgreality_tool_error",
            "error": value.clone()
        }))
        .unwrap_or_else(|_| {
            r#"{"event":"cgreality_tool_error","error":{"status":"error","error_code":"CCREALITY_ERROR_LOG_SERIALIZATION_FAILED"}}"#
                .to_string()
        })
    );
    JsonRpcResponse::success(
        id,
        json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())
            }],
            "structuredContent": value,
            "isError": true
        }),
    )
}
