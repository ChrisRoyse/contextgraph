// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

//! Phase 7 ME-JEPA shift-subscriber MCP handlers.

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::error;

use crate::deprecation::{
    apply_retired_cgreality_deprecation, is_retired_cgreality_tool as retired_tool_is_registered,
};
use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::tools::mejepa_phase7_storage::{
    capture_audit, locate_shift, replay_shift, subscriber_status, valid_shift_id,
    validate_attempt_id,
};
use crate::handlers::Handlers;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

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
    retired_tool_is_registered(tool_name)
}

pub(crate) fn retired_tool_response(id: Option<JsonRpcId>, tool_name: &str) -> JsonRpcResponse {
    error!(
        code = "CCREALITY_ENGINE_RETIRED",
        attempted_tool = tool_name,
        "retired ccreality tool call refused"
    );
    let structured = json!({
        "error_code": "CCREALITY_ENGINE_RETIRED",
        "attempted_tool": tool_name,
        "remediation": "use mcp__cgreality__mejepa_verify, mcp__cgreality__mejepa_predict_latest, or a Phase 7 mejepa subscriber tool"
    });
    let response = json!({
        "content": [{
            "type": "text",
            "text": "CCREALITY_ENGINE_RETIRED: tool retired in 2026-05-09 ME-JEPA pivot; use mcp__cgreality__mejepa_*"
        }],
        "structuredContent": structured,
        "isError": true,
        "errorCode": error_codes::INVALID_PARAMS
    });
    match apply_retired_cgreality_deprecation(tool_name, response) {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err(message) => JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, message),
    }
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
            Ok(None) => self.tool_result(
                id,
                json!({"shift_found": false, "replay_outcome": Value::Null}),
            ),
            Ok(Some(shift)) => match replay_shift(&shift) {
                Ok(replay_outcome) => self.tool_result(
                    id,
                    json!({"shift_found": true, "replay_outcome": replay_outcome}),
                ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deprecation::RETIRED_CGREALITY_TOOLS;

    #[test]
    fn retired_cgreality_tools_fail_closed_as_jsonrpc_responses() {
        let _guard = crate::handlers::tools::test_env_lock::lock();
        std::env::remove_var("CONTEXT_GRAPH_DEPRECATION_LEVEL");
        for (idx, tool_name) in RETIRED_CGREALITY_TOOLS.iter().enumerate() {
            let response =
                retired_tool_response(Some(JsonRpcId::Number(idx as i64 + 7)), tool_name);
            assert!(response.error.is_none(), "{tool_name}");
            let result = response.result.unwrap();
            assert_eq!(result["isError"], json!(true), "{tool_name}");
            assert_eq!(
                result["structuredContent"]["error_code"],
                json!("CCREALITY_ENGINE_RETIRED"),
                "{tool_name}"
            );
            assert_eq!(
                result["structuredContent"]["attempted_tool"],
                json!(tool_name),
                "{tool_name}"
            );
            assert_eq!(
                result["warnings"][0]["tool"],
                json!(tool_name),
                "{tool_name}"
            );
        }
    }

    #[test]
    fn retired_reality_run_attempt_jsonrpc_round_trip_has_structured_error() {
        let _guard = crate::handlers::tools::test_env_lock::lock();
        std::env::remove_var("CONTEXT_GRAPH_DEPRECATION_LEVEL");
        let response = retired_tool_response(
            Some(JsonRpcId::Number(9001)),
            "mcp__cgreality__reality_run_attempt",
        );

        let encoded = serde_json::to_value(&response).expect("JsonRpcResponse serializes");
        assert_eq!(encoded["jsonrpc"], json!("2.0"));
        assert_eq!(encoded["id"], json!(9001));
        assert_eq!(encoded["result"]["isError"], json!(true));
        assert_eq!(
            encoded["result"]["structuredContent"]["error_code"],
            json!("CCREALITY_ENGINE_RETIRED")
        );
        assert_eq!(
            encoded["result"]["structuredContent"]["attempted_tool"],
            json!("mcp__cgreality__reality_run_attempt")
        );

        let decoded: crate::protocol::JsonRpcResponse =
            serde_json::from_value(encoded).expect("JsonRpcResponse round-trip parses");
        assert!(decoded.error.is_none());
        let result = decoded.result.expect("retired tool returns result payload");
        assert_eq!(
            result["structuredContent"]["error_code"],
            json!("CCREALITY_ENGINE_RETIRED")
        );
    }

    #[test]
    fn retired_tool_deprecation_error_level_returns_jsonrpc_error() {
        let _guard = crate::handlers::tools::test_env_lock::lock();
        std::env::set_var("CONTEXT_GRAPH_DEPRECATION_LEVEL", "error");
        let response = retired_tool_response(
            Some(JsonRpcId::Number(8)),
            "mcp__cgreality__reality_run_attempt",
        );
        assert!(response.result.is_none());
        let error = response.error.unwrap();
        assert_eq!(error.code, error_codes::INVALID_PARAMS);
        assert!(error
            .message
            .contains("MEJEPA_TOOL_DEPRECATED_MCP__CGREALITY__REALITY_RUN_ATTEMPT"));
        std::env::remove_var("CONTEXT_GRAPH_DEPRECATION_LEVEL");
    }
}
