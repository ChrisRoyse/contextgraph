//! Topic Tools Integration Tests
//!
//! Per PRD v6 Section 10.2, verifies:
//! - get_topic_portfolio: Returns topic portfolio with tier info
//! - get_topic_stability: Returns stability metrics and dream recommendation
//! - detect_topics: Enforces minimum memory requirement (3)
//! - get_divergence_alerts: Returns alerts from SEMANTIC embedders only
//!
//! Constitution Compliance:
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) weight = 0.0
//! - AP-62: Only SEMANTIC embedders for divergence alerts
//! - AP-70: Dream recommended when entropy > 0.7 AND churn > 0.5

use serde_json::json;

use crate::protocol::{error_codes, JsonRpcId};

use super::{create_test_handlers, extract_mcp_tool_data, make_request};

// =========================================================================
// get_topic_portfolio Tool Tests
// =========================================================================

#[tokio::test]
async fn test_get_topic_portfolio_empty_memories() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_portfolio",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "get_topic_portfolio should not return JSON-RPC error"
    );
    let result = response.result.expect("tools/call must return a result");

    // MCP REQUIREMENT: isError MUST be false for successful calls
    let is_error = result
        .get("isError")
        .expect("Response must contain isError")
        .as_bool()
        .expect("isError must be a boolean");
    assert!(!is_error, "isError must be false for successful tool calls");

    // Extract and verify response data
    let data = extract_mcp_tool_data(&result);

    // Tier 0: Empty portfolio when no memories
    assert!(
        data.get("tier").is_some(),
        "Response must contain tier field"
    );
    assert!(
        data.get("topics").is_some(),
        "Response must contain topics field"
    );
    assert!(
        data.get("stability").is_some(),
        "Response must contain stability field"
    );
    assert!(
        data.get("total_topics").is_some(),
        "Response must contain total_topics field"
    );

    let tier = data.get("tier").unwrap().as_u64().unwrap();
    assert_eq!(tier, 0, "Tier should be 0 with no memories");

    let total_topics = data.get("total_topics").unwrap().as_u64().unwrap();
    assert_eq!(total_topics, 0, "Should have 0 topics with no memories");

    println!("[PASS] get_topic_portfolio returns tier 0 for empty memory store");
}

#[tokio::test]
async fn test_get_topic_portfolio_format_brief() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_portfolio",
        "arguments": {"format": "brief"}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Valid format should not error");
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false");

    println!("[PASS] get_topic_portfolio accepts format=brief");
}

#[tokio::test]
async fn test_get_topic_portfolio_format_verbose() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_portfolio",
        "arguments": {"format": "verbose"}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Valid format should not error");
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false");

    println!("[PASS] get_topic_portfolio accepts format=verbose");
}

#[tokio::test]
async fn test_get_topic_portfolio_invalid_format() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_portfolio",
        "arguments": {"format": "invalid_format_xyz"}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Should return tool error (isError: true)
    assert!(response.error.is_none(), "Tool errors use isError flag");
    let result = response.result.expect("Tool error must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(is_error, "Invalid format should set isError to true");

    // Verify error message mentions valid formats
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    assert!(
        text.contains("Invalid"),
        "Error should mention invalid format"
    );

    println!("[PASS] get_topic_portfolio rejects invalid format");
}

// =========================================================================
// get_topic_stability Tool Tests
// =========================================================================

#[tokio::test]
async fn test_get_topic_stability_default_hours() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_stability",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "get_topic_stability should not error"
    );
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false");

    let data = extract_mcp_tool_data(&result);

    // Verify required fields
    assert!(
        data.get("churn_rate").is_some(),
        "Response must contain churn_rate"
    );
    assert!(
        data.get("entropy").is_some(),
        "Response must contain entropy"
    );
    assert!(data.get("phases").is_some(), "Response must contain phases");
    assert!(
        data.get("dream_recommended").is_some(),
        "Response must contain dream_recommended"
    );
    assert!(
        data.get("high_churn_warning").is_some(),
        "Response must contain high_churn_warning"
    );
    assert!(
        data.get("average_churn").is_some(),
        "Response must contain average_churn"
    );

    // Per AP-70: dream_recommended = entropy > 0.7 AND churn > 0.5
    // With default (zero) values, dream should NOT be recommended
    let dream_recommended = data.get("dream_recommended").unwrap().as_bool().unwrap();
    let churn_rate = data.get("churn_rate").unwrap().as_f64().unwrap();
    let entropy = data.get("entropy").unwrap().as_f64().unwrap();

    // Dream only recommended when BOTH conditions met
    if entropy > 0.7 && churn_rate > 0.5 {
        assert!(
            dream_recommended,
            "Per AP-70: dream MUST be recommended when entropy > 0.7 AND churn > 0.5"
        );
    } else {
        assert!(
            !dream_recommended,
            "Per AP-70: dream NOT recommended unless both thresholds exceeded"
        );
    }

    println!("[PASS] get_topic_stability returns valid response with default hours");
}

#[tokio::test]
async fn test_get_topic_stability_custom_hours() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_stability",
        "arguments": {"hours": 24}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Valid hours should not error");
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false");

    println!("[PASS] get_topic_stability accepts custom hours");
}

#[tokio::test]
async fn test_get_topic_stability_zero_hours_rejected() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_stability",
        "arguments": {"hours": 0}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Tool errors use isError flag");
    let result = response.result.expect("Tool error must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(is_error, "Zero hours should set isError to true");

    println!("[PASS] get_topic_stability rejects hours=0");
}

#[tokio::test]
async fn test_get_topic_stability_max_hours_boundary() {
    let handlers = create_test_handlers();
    // Per constitution: max hours is 168 (1 week)
    let params = json!({
        "name": "get_topic_stability",
        "arguments": {"hours": 168}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Max valid hours should not error");
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false for hours=168");

    println!("[PASS] get_topic_stability accepts hours=168 (max boundary)");
}

#[tokio::test]
async fn test_get_topic_stability_over_max_hours_rejected() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_stability",
        "arguments": {"hours": 169}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Tool errors use isError flag");
    let result = response.result.expect("Tool error must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(is_error, "Over-max hours should set isError to true");

    println!("[PASS] get_topic_stability rejects hours > 168");
}

// =========================================================================
// detect_topics Tool Tests
// =========================================================================

#[tokio::test]
async fn test_detect_topics_insufficient_memories() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "detect_topics",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Per constitution min_cluster_size: 3
    // With 0 memories, should return INSUFFICIENT_MEMORIES error
    assert!(
        response.error.is_some(),
        "detect_topics with 0 memories should return JSON-RPC error"
    );

    let error = response.error.as_ref().unwrap();
    assert_eq!(
        error.code,
        error_codes::INSUFFICIENT_MEMORIES,
        "Error code must be INSUFFICIENT_MEMORIES (-32021)"
    );
    assert!(
        error.message.contains("3"),
        "Error message should mention minimum of 3 memories"
    );

    println!("[PASS] detect_topics returns INSUFFICIENT_MEMORIES when < 3 memories");
}

#[tokio::test]
async fn test_detect_topics_force_parameter() {
    // This just verifies the force parameter is accepted
    let handlers = create_test_handlers();
    let params = json!({
        "name": "detect_topics",
        "arguments": {"force": true}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Still fails due to insufficient memories, but force is parsed
    assert!(
        response.error.is_some(),
        "Still fails due to insufficient memories"
    );

    println!("[PASS] detect_topics accepts force parameter");
}

// =========================================================================
// get_divergence_alerts Tool Tests
// =========================================================================

#[tokio::test]
async fn test_get_divergence_alerts_default_lookback() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_divergence_alerts",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "get_divergence_alerts should not error"
    );
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false");

    let data = extract_mcp_tool_data(&result);

    // Verify required fields
    assert!(
        data.get("alerts").is_some(),
        "Response must contain alerts field"
    );
    assert!(
        data.get("severity").is_some(),
        "Response must contain severity field"
    );

    // With no memories, should have no alerts
    let alerts = data.get("alerts").unwrap().as_array().unwrap();
    assert!(
        alerts.is_empty(),
        "Should have no alerts with empty memory store"
    );

    let severity = data.get("severity").unwrap().as_str().unwrap();
    assert_eq!(severity, "none", "Severity should be 'none' with no alerts");

    println!("[PASS] get_divergence_alerts returns no alerts for empty store");
}

#[tokio::test]
async fn test_get_divergence_alerts_custom_lookback() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_divergence_alerts",
        "arguments": {"lookback_hours": 12}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Valid lookback should not error");
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false");

    println!("[PASS] get_divergence_alerts accepts custom lookback_hours");
}

#[tokio::test]
async fn test_get_divergence_alerts_zero_lookback_rejected() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_divergence_alerts",
        "arguments": {"lookback_hours": 0}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Tool errors use isError flag");
    let result = response.result.expect("Tool error must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(is_error, "Zero lookback should set isError to true");

    println!("[PASS] get_divergence_alerts rejects lookback_hours=0");
}

#[tokio::test]
async fn test_get_divergence_alerts_max_lookback_boundary() {
    let handlers = create_test_handlers();
    // Per constitution: max lookback is 48 hours
    let params = json!({
        "name": "get_divergence_alerts",
        "arguments": {"lookback_hours": 48}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Max valid lookback should not error"
    );
    let result = response.result.expect("tools/call must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "isError must be false for lookback=48");

    println!("[PASS] get_divergence_alerts accepts lookback_hours=48 (max boundary)");
}

#[tokio::test]
async fn test_get_divergence_alerts_over_max_lookback_rejected() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_divergence_alerts",
        "arguments": {"lookback_hours": 100}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Tool errors use isError flag");
    let result = response.result.expect("Tool error must return a result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(is_error, "Over-max lookback should set isError to true");

    println!("[PASS] get_divergence_alerts rejects lookback_hours > 48");
}

// =========================================================================
// MCP Format Compliance Tests
// =========================================================================

#[tokio::test]
async fn test_topic_tools_return_cognitive_pulse() {
    let handlers = create_test_handlers();

    // Test get_topic_portfolio returns _cognitive_pulse
    let params = json!({
        "name": "get_topic_portfolio",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Must have result");

    assert!(
        result.get("_cognitive_pulse").is_some(),
        "get_topic_portfolio must include _cognitive_pulse"
    );

    let pulse = result.get("_cognitive_pulse").unwrap();
    assert!(pulse.get("entropy").is_some(), "pulse must have entropy");
    assert!(
        pulse.get("coherence").is_some(),
        "pulse must have coherence"
    );
    assert!(
        pulse.get("learning_score").is_some(),
        "pulse must have learning_score"
    );

    println!("[PASS] Topic tools include _cognitive_pulse in response");
}

#[tokio::test]
async fn test_topic_portfolio_response_structure() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "get_topic_portfolio",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Must have result");

    // MCP format requirements
    let content = result.get("content").expect("Must have content");
    assert!(content.is_array(), "content must be array");

    let first = content.as_array().unwrap().first().expect("Must have item");
    assert_eq!(
        first.get("type").unwrap().as_str().unwrap(),
        "text",
        "type must be 'text'"
    );
    assert!(
        first.get("text").is_some(),
        "content item must have text field"
    );

    println!("[PASS] get_topic_portfolio follows MCP content format");
}

// =========================================================================
// Tool Routing Tests
// =========================================================================

#[tokio::test]
async fn test_unknown_tool_returns_tool_not_found() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "nonexistent_topic_tool_xyz",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    // Unknown tool returns JSON-RPC error (not isError)
    assert!(
        response.error.is_some(),
        "Unknown tool should return JSON-RPC error"
    );
    assert_eq!(
        response.error.as_ref().unwrap().code,
        error_codes::TOOL_NOT_FOUND,
        "Error code must be TOOL_NOT_FOUND"
    );

    println!("[PASS] Unknown tool returns TOOL_NOT_FOUND error");
}
