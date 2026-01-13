//! TASK-MCP-002: Epistemic Action Handler Integration Tests
//!
//! Full State Verification (FSV) tests for epistemic_action tool.
//! Tests cover happy path, edge cases, and error conditions.
//!
//! ## Test Categories
//!
//! 1. **Dispatch Registration**: Verify tool is routed correctly
//! 2. **Happy Path**: Valid inputs for all 5 action types
//! 3. **Input Validation**: Empty target, max length, confidence bounds
//! 4. **Error Conditions**: Missing GWT, invalid action_type
//! 5. **State Verification**: Workspace state snapshot accuracy

use serde_json::json;

use crate::protocol::JsonRpcId;
use crate::tools::tool_names;

use super::{create_test_handlers, create_test_handlers_with_warm_gwt, make_request};

// ============================================================================
// Dispatch Registration Tests
// ============================================================================

#[tokio::test]
async fn test_epistemic_action_dispatch_registered() {
    // Verify tool is registered in dispatch table
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "query",
            "target": "Test target",
            "rationale": "Dispatch registration test"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should not return "Unknown tool" error
    if let Some(ref error) = response.error {
        assert!(
            !error.message.contains("Unknown tool"),
            "epistemic_action should be registered: {}",
            error.message
        );
    }
}

// ============================================================================
// Happy Path Tests - All 5 Action Types
// ============================================================================

#[tokio::test]
async fn test_epistemic_action_assert_happy_path() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "The sky is blue",
            "confidence": 0.95,
            "rationale": "Visual observation"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Assert should succeed");
    let result = response.result.expect("Should have result");

    // Verify MCP format
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "isError should be false");

    // Extract and verify response
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    assert!(data.get("success").unwrap().as_bool().unwrap(), "Should succeed");
    assert_eq!(data.get("action_type").unwrap().as_str().unwrap(), "assert");
    assert!(data.get("belief_state").is_some(), "Assert should create belief_state");
    assert!(data.get("workspace_state").is_some(), "Should include workspace_state");
}

#[tokio::test]
async fn test_epistemic_action_retract_happy_path() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "retract",
            "target": "The previous belief",
            "rationale": "New evidence contradicts"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Retract should succeed");
    let result = response.result.expect("Should have result");

    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "isError should be false");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    assert!(data.get("success").unwrap().as_bool().unwrap(), "Should succeed");
    assert_eq!(data.get("action_type").unwrap().as_str().unwrap(), "retract");

    // Retracted beliefs have status="retracted" and confidence=0
    let belief = data.get("belief_state").unwrap();
    assert_eq!(belief.get("status").unwrap().as_str().unwrap(), "retracted");
    assert!((belief.get("confidence").unwrap().as_f64().unwrap() - 0.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_epistemic_action_query_happy_path() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "query",
            "target": "Does the system support X?",
            "rationale": "Checking capability"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Query should succeed");
    let result = response.result.expect("Should have result");

    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "isError should be false");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    assert!(data.get("success").unwrap().as_bool().unwrap(), "Should succeed");
    assert_eq!(data.get("action_type").unwrap().as_str().unwrap(), "query");
    assert!(data.get("query_result").is_some(), "Query should have query_result");
}

#[tokio::test]
async fn test_epistemic_action_hypothesize_happy_path() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "hypothesize",
            "target": "IC < 0.5 triggers dream consolidation",
            "confidence": 0.75,
            "rationale": "Per constitution.yaml gwt.self_ego_node.thresholds.critical"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Hypothesize should succeed");
    let result = response.result.expect("Should have result");

    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "isError should be false");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    assert!(data.get("success").unwrap().as_bool().unwrap(), "Should succeed");
    assert_eq!(data.get("action_type").unwrap().as_str().unwrap(), "hypothesize");

    // Hypotheses have status="hypothetical"
    let belief = data.get("belief_state").unwrap();
    assert_eq!(belief.get("status").unwrap().as_str().unwrap(), "hypothetical");
}

#[tokio::test]
async fn test_epistemic_action_verify_high_confidence_verified() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "verify",
            "target": "Hypothesis to verify",
            "confidence": 0.95, // > 0.7 = verified
            "rationale": "Evidence strongly supports"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Verify should succeed");
    let result = response.result.expect("Should have result");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    assert!(data.get("success").unwrap().as_bool().unwrap());
    assert_eq!(data.get("action_type").unwrap().as_str().unwrap(), "verify");

    // High confidence (>0.7) should result in status="verified"
    let belief = data.get("belief_state").unwrap();
    assert_eq!(belief.get("status").unwrap().as_str().unwrap(), "verified");
    assert!(data.get("message").unwrap().as_str().unwrap().contains("VERIFIED"));
}

#[tokio::test]
async fn test_epistemic_action_verify_low_confidence_denied() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "verify",
            "target": "Hypothesis to deny",
            "confidence": 0.1, // < 0.3 = denied
            "rationale": "Evidence contradicts"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Should have result");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    // Low confidence (<0.3) should result in status="denied"
    let belief = data.get("belief_state").unwrap();
    assert_eq!(belief.get("status").unwrap().as_str().unwrap(), "denied");
    assert!(data.get("message").unwrap().as_str().unwrap().contains("DENIED"));
}

#[tokio::test]
async fn test_epistemic_action_verify_mid_confidence_hypothetical() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "verify",
            "target": "Hypothesis with uncertain evidence",
            "confidence": 0.5, // in [0.3, 0.7] = remains hypothetical
            "rationale": "Evidence inconclusive"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Should have result");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    // Mid confidence stays hypothetical
    let belief = data.get("belief_state").unwrap();
    assert_eq!(belief.get("status").unwrap().as_str().unwrap(), "hypothetical");
    assert!(data.get("message").unwrap().as_str().unwrap().contains("unverified"));
}

// ============================================================================
// Input Validation Edge Cases
// ============================================================================

#[tokio::test]
async fn test_epistemic_action_empty_target_fails() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "", // Empty - should fail
            "rationale": "Testing empty target"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should return error (either JSON-RPC error or isError=true)
    if response.error.is_some() {
        assert!(response.error.as_ref().unwrap().message.contains("empty") ||
                response.error.as_ref().unwrap().message.contains("minLength") ||
                response.error.as_ref().unwrap().message.contains("non-empty"));
    } else {
        let result = response.result.expect("Should have result");
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_error, "Empty target should set isError=true");
    }
}

#[tokio::test]
async fn test_epistemic_action_max_target_length_succeeds() {
    let handlers = create_test_handlers_with_warm_gwt();
    let long_target = "x".repeat(4096); // Exactly max length
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": long_target,
            "rationale": "Testing max length target"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Max length target should succeed");
    let result = response.result.expect("Should have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "Max length target should not error");
}

#[tokio::test]
async fn test_epistemic_action_exceeds_max_target_length_fails() {
    let handlers = create_test_handlers_with_warm_gwt();
    let too_long_target = "x".repeat(4097); // Exceeds max length
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": too_long_target,
            "rationale": "Testing exceeded length"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should fail with error about exceeding max length
    if response.error.is_some() {
        assert!(response.error.as_ref().unwrap().message.contains("4096") ||
                response.error.as_ref().unwrap().message.contains("max"));
    } else {
        let result = response.result.expect("Should have result");
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_error, "Exceeding max length should set isError=true");
    }
}

#[tokio::test]
async fn test_epistemic_action_empty_rationale_fails() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test target",
            "rationale": "" // Empty - should fail
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should return error
    if response.error.is_some() {
        assert!(response.error.as_ref().unwrap().message.contains("empty") ||
                response.error.as_ref().unwrap().message.contains("Rationale"));
    } else {
        let result = response.result.expect("Should have result");
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_error, "Empty rationale should set isError=true");
    }
}

#[tokio::test]
async fn test_epistemic_action_confidence_below_zero_fails() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test target",
            "confidence": -0.5, // Below 0 - should fail
            "rationale": "Testing negative confidence"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should return error
    if response.error.is_some() {
        assert!(response.error.as_ref().unwrap().message.contains("confidence") ||
                response.error.as_ref().unwrap().message.contains("[0.0, 1.0]"));
    } else {
        let result = response.result.expect("Should have result");
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_error, "Negative confidence should set isError=true");
    }
}

#[tokio::test]
async fn test_epistemic_action_confidence_above_one_fails() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test target",
            "confidence": 1.5, // Above 1 - should fail
            "rationale": "Testing over-confident"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should return error
    if response.error.is_some() {
        assert!(response.error.as_ref().unwrap().message.contains("confidence") ||
                response.error.as_ref().unwrap().message.contains("[0.0, 1.0]"));
    } else {
        let result = response.result.expect("Should have result");
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_error, "Confidence > 1 should set isError=true");
    }
}

#[tokio::test]
async fn test_epistemic_action_confidence_boundary_zero_succeeds() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test target",
            "confidence": 0.0, // Boundary - should succeed
            "rationale": "Testing zero confidence"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Confidence 0.0 should succeed");
    let result = response.result.expect("Should have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "Confidence 0.0 should not error");
}

#[tokio::test]
async fn test_epistemic_action_confidence_boundary_one_succeeds() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test target",
            "confidence": 1.0, // Boundary - should succeed
            "rationale": "Testing max confidence"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Confidence 1.0 should succeed");
    let result = response.result.expect("Should have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "Confidence 1.0 should not error");
}

#[tokio::test]
async fn test_epistemic_action_default_confidence() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test target",
            // No confidence - should default to 0.5
            "rationale": "Testing default confidence"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Default confidence should succeed");
    let result = response.result.expect("Should have result");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    // Verify default confidence is 0.5
    let belief = data.get("belief_state").unwrap();
    let confidence = belief.get("confidence").unwrap().as_f64().unwrap();
    assert!((confidence - 0.5).abs() < f64::EPSILON, "Default confidence should be 0.5");
}

// ============================================================================
// Error Conditions Tests
// ============================================================================

#[tokio::test]
async fn test_epistemic_action_without_gwt_fails() {
    // Use handlers WITHOUT GWT wiring
    let handlers = create_test_handlers();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test target",
            "rationale": "Testing without GWT"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should fail because WorkspaceProvider is not initialized
    if let Some(ref error) = response.error {
        // GWT_NOT_INITIALIZED error code
        assert!(error.code == crate::protocol::error_codes::GWT_NOT_INITIALIZED ||
                error.message.contains("WorkspaceProvider") ||
                error.message.contains("GWT"));
    } else {
        // Or isError=true in MCP format
        let result = response.result.expect("Should have result");
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_error, "Missing GWT should cause error");
    }
}

#[tokio::test]
async fn test_epistemic_action_invalid_action_type() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "invalid_action", // Not in enum
            "target": "Test target",
            "rationale": "Testing invalid action type"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should fail with invalid params
    if let Some(ref error) = response.error {
        assert!(error.message.contains("Invalid") ||
                error.message.contains("action_type"));
    } else {
        let result = response.result.expect("Should have result");
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_error, "Invalid action_type should error");
    }
}

// ============================================================================
// State Verification Tests
// ============================================================================

#[tokio::test]
async fn test_epistemic_action_workspace_state_snapshot() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test belief",
            "rationale": "Testing workspace state"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none());
    let result = response.result.expect("Should have result");

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text).expect("Valid JSON");

    // Verify workspace_state snapshot is present and has correct structure
    let ws = data.get("workspace_state").expect("Should have workspace_state");
    assert!(ws.get("coherence_threshold").is_some(), "Should have coherence_threshold");
    assert!(ws.get("is_broadcasting").is_some(), "Should have is_broadcasting");
    assert!(ws.get("has_conflict").is_some(), "Should have has_conflict");
    assert!(ws.get("timestamp").is_some(), "Should have timestamp");

    // Coherence threshold should be a valid float (typically 0.8)
    let threshold = ws.get("coherence_threshold").unwrap().as_f64().expect("float");
    assert!((0.0..=1.0).contains(&threshold), "Coherence threshold in [0,1]");
}

#[tokio::test]
async fn test_epistemic_action_with_context() {
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "hypothesize",
            "target": "Complex hypothesis with context",
            "confidence": 0.7,
            "rationale": "Testing context parsing",
            "context": {
                "source_nodes": ["550e8400-e29b-41d4-a716-446655440000"],
                "uncertainty_type": "epistemic"
            }
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should succeed - context is optional but valid
    assert!(response.error.is_none(), "Valid context should not error");
    let result = response.result.expect("Should have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "Valid context should succeed");
}

// ============================================================================
// Cognitive Pulse Integration Test
// ============================================================================

#[tokio::test]
async fn test_epistemic_action_returns_success() {
    // Integration test verifying the full dispatch -> handler -> response chain
    let handlers = create_test_handlers_with_warm_gwt();
    let params = json!({
        "name": tool_names::EPISTEMIC_ACTION,
        "arguments": {
            "action_type": "assert",
            "target": "Test belief",
            "rationale": "Testing end-to-end"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Verify response is successful
    assert!(response.error.is_none(), "Should not have JSON-RPC error");
    let result = response.result.expect("Should have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "MCP isError should be false");

    // Verify response has valid MCP tool content structure
    let content = result.get("content").unwrap().as_array().unwrap();
    assert!(!content.is_empty(), "Should have content items");
}
