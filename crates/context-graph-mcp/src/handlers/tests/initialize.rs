//! Initialize Handshake Tests (MCP 2024-11-05 compliance)
//!
//! PERF: These tests only exercise JSON-RPC dispatch; they never mutate
//! storage state, so they all share a single handler fixture via
//! `shared_readonly_handlers()`. This cuts per-test setup from ~800ms
//! (fresh RocksDB + warm-load) to ~0ms after the first test.

use crate::protocol::JsonRpcId;

use super::{make_request, shared_readonly_handlers};

#[tokio::test]
async fn test_initialize_returns_protocol_version_2024_11_05() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request("initialize", Some(JsonRpcId::Number(1)), None);

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Initialize should not return an error"
    );
    let result = response.result.expect("Initialize must return a result");

    // MCP REQUIREMENT: protocolVersion MUST be "2024-11-05"
    let protocol_version = result
        .get("protocolVersion")
        .expect("Response must contain protocolVersion")
        .as_str()
        .expect("protocolVersion must be a string");
    assert_eq!(
        protocol_version, "2024-11-05",
        "Protocol version must be 2024-11-05"
    );
}

#[tokio::test]
async fn test_initialized_notification_no_response_needed() {
    let handlers = shared_readonly_handlers().await;
    // Notifications have no ID
    let request = make_request("notifications/initialized", None, None);

    let response = handlers.dispatch(request).await;

    // JSON-RPC 2.0: Notifications should return "no response" indicator
    // In this implementation, we return a response with no id, result, or error
    assert!(
        response.id.is_none(),
        "Notification response should have no ID"
    );
}

#[tokio::test]
async fn test_id_echoed_correctly_number() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request("initialize", Some(JsonRpcId::Number(42)), None);

    let response = handlers.dispatch(request).await;

    // JSON-RPC 2.0 REQUIREMENT: ID must be echoed back exactly
    let response_id = response.id.expect("Response must include ID");
    assert_eq!(
        response_id,
        JsonRpcId::Number(42),
        "Response ID must match request ID"
    );
}

#[tokio::test]
async fn test_id_echoed_correctly_string() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request(
        "initialize",
        Some(JsonRpcId::String("request-abc-123".to_string())),
        None,
    );

    let response = handlers.dispatch(request).await;

    let response_id = response.id.expect("Response must include ID");
    assert_eq!(
        response_id,
        JsonRpcId::String("request-abc-123".to_string()),
        "Response ID must match request ID"
    );
}

#[tokio::test]
async fn test_id_echoed_on_error() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request("unknown/method", Some(JsonRpcId::Number(999)), None);

    let response = handlers.dispatch(request).await;

    // ID must be echoed even on error responses
    let response_id = response.id.expect("Error response must include ID");
    assert_eq!(
        response_id,
        JsonRpcId::Number(999),
        "Error response ID must match request ID"
    );
}

#[tokio::test]
async fn test_response_jsonrpc_version_is_2_0() {
    let handlers = shared_readonly_handlers().await;
    let request = make_request("initialize", Some(JsonRpcId::Number(1)), None);

    let response = handlers.dispatch(request).await;

    // JSON-RPC 2.0 REQUIREMENT: jsonrpc field must be "2.0"
    assert_eq!(response.jsonrpc, "2.0", "JSON-RPC version must be 2.0");
}
