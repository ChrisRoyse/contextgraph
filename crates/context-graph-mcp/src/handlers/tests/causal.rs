//! Causal Inference MCP Handler Tests
//!
//! TASK-CAUSAL-001: Tests for omni_infer tool validation.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST.

use serde_json::json;
use uuid::Uuid;

use crate::protocol::JsonRpcId;

use super::{create_test_handlers, make_request};

// ============================================================================
// omni_infer Target Parameter Validation Tests (P3-06)
// ============================================================================

/// Test that omni_infer REQUIRES target for forward direction.
#[tokio::test]
async fn test_omni_infer_forward_requires_target() {
    let handlers = create_test_handlers();

    let source = Uuid::new_v4();

    // Forward direction WITHOUT target - should fail
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                "source": source.to_string(),
                "direction": "forward"
                // NOTE: No target provided - this is the error case
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should be an error response
    assert!(
        response.error.is_some()
            || response
                .result
                .as_ref()
                .map(|r| r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false))
                .unwrap_or(false),
        "Forward direction should require target parameter"
    );

    // Error message should mention target
    if let Some(err) = response.error {
        assert!(
            err.message.to_lowercase().contains("target"),
            "Error should mention missing target: {}",
            err.message
        );
    } else if let Some(result) = response.result {
        if let Some(content) = result.get("content") {
            let text = content.to_string().to_lowercase();
            assert!(
                text.contains("target"),
                "Error should mention missing target"
            );
        }
    }
}

/// Test that omni_infer REQUIRES target for backward direction.
#[tokio::test]
async fn test_omni_infer_backward_requires_target() {
    let handlers = create_test_handlers();

    let source = Uuid::new_v4();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(2)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                "source": source.to_string(),
                "direction": "backward"
                // NOTE: No target provided
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should be an error response
    assert!(
        response.error.is_some()
            || response
                .result
                .as_ref()
                .map(|r| r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false))
                .unwrap_or(false),
        "Backward direction should require target parameter"
    );
}

/// Test that omni_infer REQUIRES target for bidirectional direction.
#[tokio::test]
async fn test_omni_infer_bidirectional_requires_target() {
    let handlers = create_test_handlers();

    let source = Uuid::new_v4();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(3)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                "source": source.to_string(),
                "direction": "bidirectional"
                // NOTE: No target provided
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should be an error response
    assert!(
        response.error.is_some()
            || response
                .result
                .as_ref()
                .map(|r| r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false))
                .unwrap_or(false),
        "Bidirectional direction should require target parameter"
    );
}

/// Test that omni_infer allows OPTIONAL target for bridge direction.
#[tokio::test]
async fn test_omni_infer_bridge_allows_no_target() {
    let handlers = create_test_handlers();

    let source = Uuid::new_v4();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(4)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                "source": source.to_string(),
                "direction": "bridge"
                // NOTE: No target - this is ALLOWED for bridge
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should NOT error on missing target - may fail for other reasons (no data)
    // but should not complain about missing target parameter
    if let Some(result) = &response.result {
        if let Some(content) = result.get("content") {
            let text = content.to_string().to_lowercase();
            assert!(
                !text.contains("missing required 'target'"),
                "Bridge direction should NOT require target parameter"
            );
        }
    }
    if let Some(err) = &response.error {
        assert!(
            !err.message
                .to_lowercase()
                .contains("missing required 'target'"),
            "Bridge direction should NOT require target parameter"
        );
    }
}

/// Test that omni_infer allows OPTIONAL target for abduction direction.
#[tokio::test]
async fn test_omni_infer_abduction_allows_no_target() {
    let handlers = create_test_handlers();

    let source = Uuid::new_v4();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(5)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                "source": source.to_string(),
                "direction": "abduction"
                // NOTE: No target - this is ALLOWED for abduction
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should NOT error on missing target
    if let Some(result) = &response.result {
        if let Some(content) = result.get("content") {
            let text = content.to_string().to_lowercase();
            assert!(
                !text.contains("missing required 'target'"),
                "Abduction direction should NOT require target parameter"
            );
        }
    }
    if let Some(err) = &response.error {
        assert!(
            !err.message
                .to_lowercase()
                .contains("missing required 'target'"),
            "Abduction direction should NOT require target parameter"
        );
    }
}

/// Test that omni_infer works with both source AND target for forward.
#[tokio::test]
async fn test_omni_infer_forward_with_target() {
    let handlers = create_test_handlers();

    let source = Uuid::new_v4();
    let target = Uuid::new_v4();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(6)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                "source": source.to_string(),
                "target": target.to_string(),
                "direction": "forward"
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should NOT error on target validation - may fail for other reasons
    // (no actual nodes in graph) but target validation should pass
    if let Some(result) = &response.result {
        if let Some(content) = result.get("content") {
            let text = content.to_string().to_lowercase();
            assert!(
                !text.contains("missing required 'target'"),
                "Forward with target should not report missing target"
            );
        }
    }
}

/// Test that omni_infer rejects invalid direction.
#[tokio::test]
async fn test_omni_infer_invalid_direction() {
    let handlers = create_test_handlers();

    let source = Uuid::new_v4();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(7)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                "source": source.to_string(),
                "direction": "invalid_direction"
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should error on invalid direction
    assert!(
        response.error.is_some()
            || response
                .result
                .as_ref()
                .map(|r| r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false))
                .unwrap_or(false),
        "Invalid direction should be rejected"
    );
}

/// Test that omni_infer requires source parameter.
#[tokio::test]
async fn test_omni_infer_requires_source() {
    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(8)),
        Some(json!({
            "name": "omni_infer",
            "arguments": {
                // NOTE: No source provided
                "direction": "bridge"
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should error on missing source
    assert!(
        response.error.is_some()
            || response
                .result
                .as_ref()
                .map(|r| r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false))
                .unwrap_or(false),
        "Missing source should be rejected"
    );
}
