// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

//! Phase 7 ME-JEPA shift-subscriber MCP handlers.

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::error;

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::tools::mejepa_phase7_storage::{
    capture_audit, locate_shift, replay_shift, subscriber_status, valid_shift_id,
    validate_attempt_id,
};
use crate::handlers::Handlers;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const RETIRED_TOOLS: &[&str] = &[
    "mcp__cgreality__reality_run_attempt",
    "mcp__cgreality__reality_alter_file",
    "mcp__cgreality__reality_alter_run",
    "mcp__cgreality__reality_alter_apply",
    "mcp__cgreality__reality_drip_attempt",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObserveShiftRequest {
    shift_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubscriberStatusRequest {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CaptureAuditRequest {
    attempt_id: String,
    #[serde(default)]
    page: u32,
}

pub(crate) fn is_retired_cgreality_tool(tool_name: &str) -> bool {
    RETIRED_TOOLS.contains(&tool_name)
}

pub(crate) fn retired_tool_response(id: Option<JsonRpcId>, tool_name: &str) -> JsonRpcResponse {
    JsonRpcResponse::error_with_data(
        id,
        error_codes::INVALID_PARAMS,
        "CCREALITY_ENGINE_RETIRED: tool retired in 2026-05-09 ME-JEPA pivot; use mcp__cgreality__mejepa_*",
        json!({
            "error_code": "CCREALITY_ENGINE_RETIRED",
            "attempted_tool": tool_name,
            "remediation": "use mcp__cgreality__mejepa_verify, mcp__cgreality__mejepa_predict_latest, or a Phase 7 mejepa subscriber tool"
        }),
    )
}

impl Handlers {
    pub(crate) async fn call_mejepa_observe_shift(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let request: ObserveShiftRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_structured(
                    id,
                    ToolErrorKind::Validation,
                    "MEJEPA_OBSERVE_SHIFT_SCHEMA_INVALID",
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_OBSERVE_SHIFT
                    ),
                    json!({}),
                );
            }
        };
        if !valid_shift_id(&request.shift_id) {
            return self.tool_error_structured(
                id,
                ToolErrorKind::Validation,
                "MEJEPA_OBSERVE_SHIFT_MALFORMED_ID",
                "shiftId must match ^01J[0-9A-F]{20}$",
                json!({"shift_id": request.shift_id}),
            );
        }
        match locate_shift(&request.shift_id).await {
            Ok(None) => {
                self.tool_result(id, json!({"shift_found": false, "replay_outcome": Value::Null}))
            }
            Ok(Some(shift)) => match replay_shift(&shift) {
                Ok(replay_outcome) => {
                    self.tool_result(id, json!({"shift_found": true, "replay_outcome": replay_outcome}))
                }
                Err(message) => {
                    error!(
                        code = "MEJEPA_OBSERVE_SHIFT_REPLAY_FAILED",
                        shift_id = %request.shift_id,
                        error = %message,
                        "ME-JEPA shift replay failed"
                    );
                    self.tool_error_structured(
                        id,
                        ToolErrorKind::Execution,
                        "MEJEPA_OBSERVE_SHIFT_REPLAY_FAILED",
                        &message,
                        json!({"shift": shift}),
                    )
                }
            },
            Err(message) => self.tool_error_structured(
                id,
                ToolErrorKind::Execution,
                "MEJEPA_OBSERVE_SHIFT_LOCATE_FAILED",
                &message,
                json!({"shift_id": request.shift_id}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_subscriber_status(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        if let Err(err) = serde_json::from_value::<SubscriberStatusRequest>(args) {
            return self.tool_error_structured(
                id,
                ToolErrorKind::Validation,
                "MEJEPA_SUBSCRIBER_STATUS_UNEXPECTED_ARG",
                &format!(
                    "{} expects an empty object: {err}",
                    tool_names::MEJEPA_SUBSCRIBER_STATUS
                ),
                json!({}),
            );
        }
        match subscriber_status().await {
            Ok(value) => self.tool_result(id, value),
            Err(message) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_SUBSCRIBER_STATUS_READ_FAILED",
                &message,
                json!({"env": "CONTEXTGRAPH_MEJEPA_INFER_DB"}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_capture_audit(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let request: CaptureAuditRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_structured(
                    id,
                    ToolErrorKind::Validation,
                    "MEJEPA_CAPTURE_AUDIT_SCHEMA_INVALID",
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_CAPTURE_AUDIT
                    ),
                    json!({}),
                );
            }
        };
        if let Err(message) = validate_attempt_id(&request.attempt_id) {
            return self.tool_error_structured(
                id,
                ToolErrorKind::Validation,
                "MEJEPA_CAPTURE_AUDIT_ATTEMPT_ID_INVALID",
                &message,
                json!({"attempt_id": request.attempt_id}),
            );
        }
        match capture_audit(&request.attempt_id, request.page) {
            Ok(value) => self.tool_result(id, value),
            Err(message) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_CAPTURE_AUDIT_READ_FAILED",
                &message,
                json!({"attempt_id": request.attempt_id}),
            ),
        }
    }
}
