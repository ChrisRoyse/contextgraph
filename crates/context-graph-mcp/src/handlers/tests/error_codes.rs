//! Error Code Tests (MCP 2024-11-05 compliance)
//!
//! PERF: These tests drive only JSON-RPC error paths; they don't mutate
//! storage. Uses the shared read-only handler fixture.

use serde_json::json;

use crate::protocol::{error_codes, JsonRpcId};

use super::{make_request, shared_readonly_handlers};

#[tokio::test]
async fn test_method_not_found_error() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request("unknown/method", Some(JsonRpcId::Number(1)), None);

    let response = handlers.dispatch(request).await;

    assert!(
        response.result.is_none(),
        "Should not have result for error"
    );
    let error = response
        .error
        .expect("Should have error for unknown method");
    assert_eq!(error.code, error_codes::METHOD_NOT_FOUND);
    assert!(error.message.contains("Method not found"));
}

#[tokio::test]
async fn test_invalid_params_missing_tools_call_params() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), None);

    let response = handlers.dispatch(request).await;

    let error = response
        .error
        .expect("Should have error for missing params");
    assert_eq!(error.code, error_codes::INVALID_PARAMS);
    assert!(error.message.contains("Missing params"));
}

#[tokio::test]
async fn test_invalid_params_missing_tool_name() {
    let handlers = shared_readonly_handlers().await;
    let params = json!({
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    let error = response.error.expect("Should have error for missing name");
    assert_eq!(error.code, error_codes::INVALID_PARAMS);
    assert!(error.message.contains("name"));
}

#[tokio::test]
async fn test_tool_not_found_error() {
    let handlers = shared_readonly_handlers().await;
    let params = json!({
        "name": "nonexistent_tool",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    let error = response.error.expect("Should have error for unknown tool");
    assert_eq!(error.code, error_codes::TOOL_NOT_FOUND);
    assert!(error.message.contains("Unknown tool"));
}

#[tokio::test]
async fn test_error_response_has_correct_id() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request("unknown/method", Some(JsonRpcId::Number(42)), None);

    let response = handlers.dispatch(request).await;

    assert_eq!(response.id, Some(JsonRpcId::Number(42)));
}

#[tokio::test]
async fn test_error_response_has_string_id() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request(
        "unknown/method",
        Some(JsonRpcId::String("test-id".to_string())),
        None,
    );

    let response = handlers.dispatch(request).await;

    assert_eq!(response.id, Some(JsonRpcId::String("test-id".to_string())));
}
