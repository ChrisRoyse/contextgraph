//! Tools Call Tests (MCP 2024-11-05 compliance)
//!
//! PERF: Tests that only validate dispatch errors or read static status use
//! `shared_readonly_handlers()` to amortize RocksDB setup without loading
//! unrelated production embedding models.
//! Tests that mutate storage (e.g. `store_memory`) keep their own isolated
//! real-embedding handler and run as explicit integration tests.

use serde_json::json;

use crate::protocol::JsonRpcId;

use super::{create_test_handlers, make_request, shared_readonly_handlers};

// =========================================================================
// get_memetic_status Tool Tests
// =========================================================================

#[tokio::test]
async fn test_tools_call_get_memetic_status() {
    // Read-only: just inspects embedder/layer counts.
    let handlers = shared_readonly_handlers().await;
    let params = json!({
        "name": "get_memetic_status",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "get_memetic_status should not return an error"
    );
    let result = response.result.expect("tools/call must return a result");

    // Verify MCP format
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed_text: serde_json::Value = serde_json::from_str(text).unwrap();

    // TASK-S001: Verify expected fields using TeleologicalMemoryStore terminology
    assert!(
        parsed_text.get("fingerprintCount").is_some(),
        "Response must contain fingerprintCount"
    );
    assert!(
        parsed_text.get("embedderCount").is_some(),
        "Response must contain embedderCount"
    );
    assert_eq!(
        parsed_text["embedderCount"],
        json!(context_graph_core::weights::active_embedder_count()),
        "embedderCount must report active embedders, not disabled storage slots"
    );
    assert_eq!(
        parsed_text["storageSlotCount"],
        json!(context_graph_core::types::fingerprint::NUM_EMBEDDERS),
        "storageSlotCount must preserve the 14-slot fingerprint contract"
    );
    assert_eq!(
        parsed_text["disabledEmbedders"],
        json!(context_graph_core::weights::disabled_embedder_names()),
        "status must expose retired/disabled slots"
    );
    assert!(
        parsed_text.get("layers").is_some(),
        "Response must contain layers"
    );
}

// =========================================================================
// store_memory Tool Tests
// =========================================================================

#[tokio::test]
#[ignore = "requires production E1-E14 embedding models under CONTEXT_GRAPH_MODELS_PATH"]
async fn test_tools_call_store_memory_valid() {
    // Writes to storage — keep isolated.
    let (handlers, _tempdir) = create_test_handlers().await;
    let params = json!({
        "name": "store_memory",
        "arguments": {
            "content": "Memory content to store",
            "importance": 0.8
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Valid store_memory should not return an error"
    );
    let result = response.result.expect("tools/call must return a result");

    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false for successful store");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed_text: serde_json::Value = serde_json::from_str(text).unwrap();

    // TASK-S001: Verify fingerprintId is present (replaces nodeId)
    assert!(
        parsed_text.get("fingerprintId").is_some(),
        "Response must contain fingerprintId"
    );
}

// =========================================================================
// search_graph Tool Tests
// =========================================================================

#[tokio::test]
#[ignore = "requires production E1-E14 embedding models under CONTEXT_GRAPH_MODELS_PATH"]
async fn test_tools_call_search_graph_valid() {
    // Read-only: searches (possibly empty) shared store.
    let handlers = shared_readonly_handlers().await;
    let params = json!({
        "name": "search_graph",
        "arguments": {
            "query": "test search query",
            "topK": 5
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Valid search_graph should not return an error"
    );
    let result = response.result.expect("tools/call must return a result");

    // Verify MCP format
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false for successful search");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed_text: serde_json::Value = serde_json::from_str(text).unwrap();

    // Verify search results structure
    assert!(
        parsed_text.get("results").is_some(),
        "Response must contain results"
    );
    assert!(
        parsed_text.get("count").is_some(),
        "Response must contain count"
    );
}

#[tokio::test]
async fn test_tools_call_search_graph_missing_query() {
    // Validation-only: never reaches storage.
    let handlers = shared_readonly_handlers().await;
    let params = json!({
        "name": "search_graph",
        "arguments": {
            "topK": 5
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Tool errors use isError flag");
    let result = response.result.expect("Tool error must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(is_error, "Missing query should set isError to true");
}

#[tokio::test]
async fn test_tool_error_sets_is_error_true() {
    // Validation-only: never reaches storage.
    let handlers = shared_readonly_handlers().await;
    // inject_context without required 'content' parameter
    let params = json!({
        "name": "inject_context",
        "arguments": {
            "rationale": "Missing content"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Tool errors return success with isError: true (MCP format)
    assert!(
        response.error.is_none(),
        "Tool errors use isError flag, not JSON-RPC error"
    );
    let result = response
        .result
        .expect("Tool error must return a result with isError");

    let is_error = result
        .get("isError")
        .expect("Response must contain isError")
        .as_bool()
        .expect("isError must be a boolean");
    assert!(is_error, "isError must be true for tool errors");

    // Verify error message is in content
    let content = result
        .get("content")
        .expect("Response must contain content")
        .as_array()
        .expect("content must be an array");
    assert!(!content.is_empty(), "Error content must not be empty");
}
