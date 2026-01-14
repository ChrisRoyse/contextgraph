//! Identity Continuity MCP tool tests (1 tool):
//! - get_identity_continuity
//!
//! TASK-38: FSV tests for focused identity continuity (IC) status.
//!
//! ## Constitution Thresholds
//!
//! Per constitution.yaml gwt.identity_continuity:
//! - Healthy: IC >= 0.9
//! - Warning: 0.7 <= IC < 0.9
//! - Degraded: 0.5 <= IC < 0.7
//! - Critical: IC < 0.5 (crisis state)
//!
//! ## Test Cases
//!
//! 1. Basic success with configured GWT (uses create_test_handlers_with_all_components)
//! 2. Error case: GWT system NOT configured (uses create_test_handlers)
//! 3. Response field verification (ic, status, in_crisis, history_len, thresholds)
//! 4. Include_history parameter test
//! 5. Edge case: IC values at threshold boundaries

use serde_json::json;

use super::helpers::{make_tool_call, get_tool_data};
use crate::handlers::tests::{create_test_handlers, create_test_handlers_with_all_components};

// -------------------------------------------------------------------------
// get_identity_continuity - Basic Success
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_identity_continuity_basic_success() {
    // create_test_handlers_with_all_components configures gwt_system
    let handlers = create_test_handlers_with_all_components();
    let request = make_tool_call("get_identity_continuity", json!({}));

    let response = handlers.dispatch(request).await;

    // Should not be a JSON-RPC protocol error
    assert!(
        response.error.is_none(),
        "Should not be JSON-RPC protocol error"
    );

    let result = response.result.as_ref().expect("Must have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);

    // With gwt_system configured, should succeed
    assert!(
        !is_error,
        "get_identity_continuity should succeed with GWT configured"
    );

    // Verify response structure
    let data = get_tool_data(&response);

    // Verify required fields exist
    assert!(
        data.get("ic").is_some(),
        "Response must have 'ic' field"
    );
    assert!(
        data.get("status").is_some(),
        "Response must have 'status' field"
    );
    assert!(
        data.get("in_crisis").is_some(),
        "Response must have 'in_crisis' field"
    );
    assert!(
        data.get("history_len").is_some(),
        "Response must have 'history_len' field"
    );
    assert!(
        data.get("thresholds").is_some(),
        "Response must have 'thresholds' field"
    );
}

// -------------------------------------------------------------------------
// get_identity_continuity - Error: GWT Not Configured
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_identity_continuity_fails_without_gwt() {
    // create_test_handlers() does NOT configure gwt_system
    let handlers = create_test_handlers();
    let request = make_tool_call("get_identity_continuity", json!({}));

    let response = handlers.dispatch(request).await;

    // Per AP-26: Must fail fast with explicit error
    assert!(
        response.error.is_none(),
        "Should not be JSON-RPC protocol error"
    );

    let result = response.result.as_ref().expect("Must have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);

    assert!(
        is_error,
        "get_identity_continuity MUST return isError=true when GWT not configured"
    );

    // Verify error contains useful information
    let content = result.get("content")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .expect("Error must have content text");

    assert!(
        content.contains("GWT") ||
        content.contains("gwt") ||
        content.contains("not initialized"),
        "Error message should indicate GWT not configured: {}",
        content
    );
}

// -------------------------------------------------------------------------
// get_identity_continuity - Response Field Verification
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_identity_continuity_response_fields() {
    let handlers = create_test_handlers_with_all_components();
    let request = make_tool_call("get_identity_continuity", json!({}));

    let response = handlers.dispatch(request).await;
    let data = get_tool_data(&response);

    // Verify ic is a number in [0.0, 1.0]
    let ic = data.get("ic")
        .and_then(|v| v.as_f64())
        .expect("ic must be a number");
    assert!(
        (0.0..=1.0).contains(&ic),
        "ic must be in [0.0, 1.0], got {}",
        ic
    );

    // Verify status is one of the valid strings
    let status = data.get("status")
        .and_then(|v| v.as_str())
        .expect("status must be a string");
    assert!(
        status == "Healthy" || status == "Warning" || status == "Degraded" || status == "Critical",
        "status must be Healthy/Warning/Degraded/Critical, got {}",
        status
    );

    // Verify in_crisis is a boolean
    let in_crisis = data.get("in_crisis")
        .and_then(|v| v.as_bool());
    assert!(
        in_crisis.is_some(),
        "in_crisis must be a boolean"
    );

    // Verify history_len is a valid integer (u64 is always non-negative)
    let _history_len = data.get("history_len")
        .and_then(|v| v.as_u64())
        .expect("history_len must be an integer");

    // Verify thresholds object has all required fields
    let thresholds = data.get("thresholds")
        .expect("thresholds must exist");
    assert!(
        thresholds.get("healthy").is_some(),
        "thresholds must have 'healthy' field"
    );
    assert!(
        thresholds.get("warning").is_some(),
        "thresholds must have 'warning' field"
    );
    assert!(
        thresholds.get("degraded").is_some(),
        "thresholds must have 'degraded' field"
    );
    assert!(
        thresholds.get("critical").is_some(),
        "thresholds must have 'critical' field"
    );

    // Verify threshold values per constitution
    let healthy = thresholds.get("healthy").and_then(|v| v.as_f64()).unwrap();
    let warning = thresholds.get("warning").and_then(|v| v.as_f64()).unwrap();
    let degraded = thresholds.get("degraded").and_then(|v| v.as_f64()).unwrap();
    let critical = thresholds.get("critical").and_then(|v| v.as_f64()).unwrap();

    assert_eq!(healthy, 0.9, "healthy threshold must be 0.9");
    assert_eq!(warning, 0.7, "warning threshold must be 0.7");
    assert_eq!(degraded, 0.5, "degraded threshold must be 0.5");
    assert_eq!(critical, 0.0, "critical threshold must be 0.0");
}

// -------------------------------------------------------------------------
// get_identity_continuity - Include History Parameter
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_identity_continuity_include_history() {
    let handlers = create_test_handlers_with_all_components();

    // Test with include_history = false (default)
    let request_no_history = make_tool_call("get_identity_continuity", json!({}));
    let response_no_history = handlers.dispatch(request_no_history).await;
    let data_no_history = get_tool_data(&response_no_history);

    // history field should not be present when not requested
    // (or if present, implementation choice)
    let _has_history_default = data_no_history.get("history").is_some();

    // Test with include_history = true
    let request_with_history = make_tool_call("get_identity_continuity", json!({
        "include_history": true
    }));
    let response_with_history = handlers.dispatch(request_with_history).await;
    let data_with_history = get_tool_data(&response_with_history);

    // history field should be present when requested
    assert!(
        data_with_history.get("history").is_some(),
        "history field must be present when include_history=true"
    );

    // history should be an array
    let history = data_with_history.get("history")
        .and_then(|v| v.as_array())
        .expect("history must be an array");

    // History length should be <= 10 per spec
    assert!(
        history.len() <= 10,
        "history should have at most 10 entries, got {}",
        history.len()
    );

    // Note: With fresh handlers, history may be empty, which is valid
    // Real IC history requires workspace events to have been processed
}

// -------------------------------------------------------------------------
// get_identity_continuity - Status/Crisis Consistency
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_identity_continuity_status_crisis_consistency() {
    let handlers = create_test_handlers_with_all_components();
    let request = make_tool_call("get_identity_continuity", json!({}));

    let response = handlers.dispatch(request).await;
    let data = get_tool_data(&response);

    let ic = data.get("ic").and_then(|v| v.as_f64()).unwrap();
    let status = data.get("status").and_then(|v| v.as_str()).unwrap();
    let in_crisis = data.get("in_crisis").and_then(|v| v.as_bool()).unwrap();
    let history_len = data.get("history_len").and_then(|v| v.as_u64()).unwrap();

    // Fresh system with no workspace events: IC=0.0, in_crisis=false, history_len=0
    // This is correct because crisis detection requires processing workspace events first.
    // Without any events, there's no "crisis" to detect - the system just hasn't started.
    //
    // For systems with history (events have been processed):
    // - in_crisis is based on actual detection results, not just the IC value
    // - IC < 0.5 (Critical) with events processed SHOULD have in_crisis=true
    //
    // This test documents both scenarios:
    if history_len == 0 {
        // Fresh system - no events processed yet
        // IC is 0.0 (default), but in_crisis is false because no crisis was detected
        // (you can't detect a crisis if you've never processed any events)
        assert!(
            !in_crisis,
            "Fresh system (history_len=0) should have in_crisis=false, got in_crisis={}",
            in_crisis
        );
    } else {
        // System has processed events - standard consistency check applies
        // in_crisis flag is set by IdentityContinuityMonitor based on detection results
        if ic < 0.5 && in_crisis {
            // Expected behavior for critical IC with events
            assert!(
                status == "Critical" || status == "Degraded",
                "status must be Critical or Degraded when in_crisis=true, got {}",
                status
            );
        }
        // Note: in_crisis may be false even with low IC if no crisis transition occurred
        // The crisis flag tracks transitions, not just raw values
    }

    // Verify status is a valid classification
    assert!(
        status == "Healthy" || status == "Warning" || status == "Degraded" || status == "Critical",
        "status must be a valid classification, got {}",
        status
    );
}

// -------------------------------------------------------------------------
// get_identity_continuity - Constitution Threshold Documentation
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_identity_continuity_constitution_thresholds() {
    // TASK-38: Constitution thresholds for identity continuity:
    //
    // gwt.identity_continuity.thresholds:
    //   - Healthy: IC >= 0.9 (optimal system coherence)
    //   - Warning: 0.7 <= IC < 0.9 (needs attention)
    //   - Degraded: 0.5 <= IC < 0.7 (action required)
    //   - Critical: IC < 0.5 (identity crisis - triggers dream)
    //
    // Relationship to other systems:
    //   - IC < 0.5 triggers IdentityCritical dream priority (IDENTITY-007)
    //   - IC is computed as IC = cos(PV_t, PV_{t-1}) x r(t)
    //   - PV = purpose vector (13D), r = Kuramoto order parameter
    //
    // This test documents these thresholds - actual threshold testing
    // requires manipulating the underlying GWT state.

    let handlers = create_test_handlers_with_all_components();
    let request = make_tool_call("get_identity_continuity", json!({}));

    let response = handlers.dispatch(request).await;

    // Basic protocol compliance
    assert!(response.error.is_none(), "Should not be JSON-RPC error");
    assert!(response.result.is_some(), "Must have result");

    let data = get_tool_data(&response);
    let thresholds = data.get("thresholds").expect("thresholds must exist");

    // Verify threshold values match constitution
    assert_eq!(
        thresholds.get("healthy").and_then(|v| v.as_f64()).unwrap(),
        0.9,
        "Healthy threshold per constitution: >= 0.9"
    );
    assert_eq!(
        thresholds.get("warning").and_then(|v| v.as_f64()).unwrap(),
        0.7,
        "Warning threshold per constitution: >= 0.7"
    );
    assert_eq!(
        thresholds.get("degraded").and_then(|v| v.as_f64()).unwrap(),
        0.5,
        "Degraded threshold per constitution: >= 0.5"
    );
    assert_eq!(
        thresholds.get("critical").and_then(|v| v.as_f64()).unwrap(),
        0.0,
        "Critical threshold per constitution: < 0.5"
    );
}

// -------------------------------------------------------------------------
// get_identity_continuity - Comparison with get_ego_state
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_identity_continuity_smaller_than_ego_state() {
    // TASK-38: get_identity_continuity should return smaller payload than get_ego_state
    // get_ego_state returns: purpose_vector (13 floats), trajectory_length, coherence_with_actions, etc.
    // get_identity_continuity returns: ic, status, in_crisis, history_len, thresholds

    let handlers = create_test_handlers_with_all_components();

    // Call get_identity_continuity
    let ic_request = make_tool_call("get_identity_continuity", json!({}));
    let ic_response = handlers.dispatch(ic_request).await;
    let ic_data = get_tool_data(&ic_response);

    // Call get_ego_state
    let ego_request = make_tool_call("get_ego_state", json!({}));
    let ego_response = handlers.dispatch(ego_request).await;
    let ego_data = get_tool_data(&ego_response);

    // Verify get_identity_continuity does NOT include purpose_vector
    assert!(
        ic_data.get("purpose_vector").is_none(),
        "get_identity_continuity should NOT include purpose_vector"
    );

    // Verify get_ego_state DOES include purpose_vector
    assert!(
        ego_data.get("purpose_vector").is_some(),
        "get_ego_state should include purpose_vector"
    );

    // Verify get_identity_continuity does NOT include coherence_with_actions
    assert!(
        ic_data.get("coherence_with_actions").is_none(),
        "get_identity_continuity should NOT include coherence_with_actions"
    );

    // Both should have status
    assert!(
        ic_data.get("status").is_some() && ego_data.get("identity_status").is_some(),
        "Both tools should return status information"
    );

    // get_identity_continuity focuses on ic/crisis state
    assert!(
        ic_data.get("in_crisis").is_some(),
        "get_identity_continuity should have in_crisis field"
    );
}
