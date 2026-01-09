//! Tools Call Tests (MCP 2024-11-05 compliance)

use serde_json::json;

use crate::protocol::JsonRpcId;

use super::{create_test_handlers, make_request};

// =========================================================================
// inject_context Tool Tests
// =========================================================================

#[tokio::test]
async fn test_tools_call_inject_context_valid() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "inject_context",
        "arguments": {
            "content": "Test content for injection",
            "rationale": "Testing MCP protocol compliance"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Valid inject_context should not return an error"
    );
    let result = response.result.expect("tools/call must return a result");

    // MCP REQUIREMENT: content array MUST exist
    let content = result
        .get("content")
        .expect("Response must contain content array")
        .as_array()
        .expect("content must be an array");

    // MCP REQUIREMENT: content items must have type and text
    assert!(!content.is_empty(), "Content array must not be empty");
    let first_item = &content[0];
    let item_type = first_item
        .get("type")
        .expect("Content item must have type")
        .as_str()
        .expect("type must be a string");
    assert_eq!(item_type, "text", "Content item type must be 'text'");

    let text = first_item
        .get("text")
        .expect("Content item must have text")
        .as_str()
        .expect("text must be a string");
    assert!(!text.is_empty(), "Content text must not be empty");

    // MCP REQUIREMENT: isError MUST be false for successful calls
    let is_error = result
        .get("isError")
        .expect("Response must contain isError")
        .as_bool()
        .expect("isError must be a boolean");
    assert!(!is_error, "isError must be false for successful tool calls");

    // TASK-S001: Verify fingerprintId is present in the response text (replaces nodeId)
    let parsed_text: serde_json::Value =
        serde_json::from_str(text).expect("Content text should be valid JSON");
    assert!(
        parsed_text.get("fingerprintId").is_some(),
        "Response must contain fingerprintId"
    );
}

#[tokio::test]
async fn test_tools_call_inject_context_returns_utl_metrics() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "inject_context",
        "arguments": {
            "content": "Learning about MCP protocols",
            "rationale": "Understanding API standards"
        }
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("tools/call must return a result");
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed_text: serde_json::Value = serde_json::from_str(text).unwrap();

    // Verify UTL metrics are present
    let utl = parsed_text
        .get("utl")
        .expect("Response must contain utl object");
    assert!(
        utl.get("learningScore").is_some(),
        "utl must contain learningScore"
    );
    assert!(utl.get("entropy").is_some(), "utl must contain entropy");
    assert!(utl.get("coherence").is_some(), "utl must contain coherence");
    assert!(utl.get("surprise").is_some(), "utl must contain surprise");
}

// =========================================================================
// get_memetic_status Tool Tests
// =========================================================================

#[tokio::test]
async fn test_tools_call_get_memetic_status() {
    let handlers = create_test_handlers();
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
        parsed_text.get("phase").is_some(),
        "Response must contain phase"
    );
    assert!(
        parsed_text.get("fingerprintCount").is_some(),
        "Response must contain fingerprintCount"
    );
    assert!(
        parsed_text.get("utl").is_some(),
        "Response must contain utl"
    );
    assert!(
        parsed_text.get("layers").is_some(),
        "Response must contain layers"
    );
}

// =============================================================================
// get_memetic_status Tests (M05-T27)
// =============================================================================

#[tokio::test]
async fn test_get_memetic_status_utl_values_in_valid_range() {
    let handlers = create_test_handlers();
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_memetic_status",
            "arguments": {}
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(
        response.error.is_none(),
        "get_memetic_status should not error"
    );

    let result = response.result.expect("Must have result");
    let content = result.get("content").expect("Must have content array");
    let text = content[0]
        .get("text")
        .expect("First content must have text");
    let data: serde_json::Value =
        serde_json::from_str(text.as_str().unwrap()).expect("text must be valid JSON");

    let utl = data.get("utl").expect("Response must have utl field");

    // Verify ranges per constitution.yaml:154-157
    let entropy = utl["entropy"].as_f64().expect("entropy must be f64");
    let coherence = utl["coherence"].as_f64().expect("coherence must be f64");
    let learning_score = utl["learningScore"]
        .as_f64()
        .expect("learningScore must be f64");

    assert!(
        (0.0..=1.0).contains(&entropy),
        "entropy {} not in [0,1]",
        entropy
    );
    assert!(
        (0.0..=1.0).contains(&coherence),
        "coherence {} not in [0,1]",
        coherence
    );
    assert!(
        (0.0..=1.0).contains(&learning_score),
        "learningScore {} not in [0,1]",
        learning_score
    );
}

#[tokio::test]
async fn test_get_memetic_status_johari_quadrant_valid() {
    let handlers = create_test_handlers();
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_memetic_status",
            "arguments": {}
        })),
    );

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Must have result");
    let content = result.get("content").expect("Must have content");
    let text = content[0].get("text").expect("Must have text");
    let data: serde_json::Value = serde_json::from_str(text.as_str().unwrap()).unwrap();

    let utl = data.get("utl").expect("Must have utl");
    let johari = utl["johariQuadrant"]
        .as_str()
        .expect("johariQuadrant must be string");

    // Per constitution.yaml:159-163
    let valid_quadrants = ["Open", "Blind", "Hidden", "Unknown"];
    assert!(
        valid_quadrants.contains(&johari),
        "Invalid Johari quadrant: '{}'. Must be one of {:?}",
        johari,
        valid_quadrants
    );
}

#[tokio::test]
async fn test_get_memetic_status_suggested_action_matches_johari() {
    let handlers = create_test_handlers();
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_memetic_status",
            "arguments": {}
        })),
    );

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Must have result");
    let content = result.get("content").expect("Must have content");
    let text = content[0].get("text").expect("Must have text");
    let data: serde_json::Value = serde_json::from_str(text.as_str().unwrap()).unwrap();

    let utl = data.get("utl").expect("Must have utl");
    let johari = utl["johariQuadrant"].as_str().unwrap();
    let action = utl["suggestedAction"]
        .as_str()
        .expect("suggestedAction must be string");

    // Verify mapping per constitution.yaml:159-163
    let expected_action = match johari {
        "Open" => "direct_recall",
        "Blind" => "trigger_dream",
        "Hidden" => "get_neighborhood",
        "Unknown" => "epistemic_action",
        _ => "continue",
    };

    assert_eq!(
        action, expected_action,
        "Johari '{}' should map to '{}', got '{}'",
        johari, expected_action, action
    );
}

#[tokio::test]
async fn test_get_memetic_status_not_hardcoded() {
    // The old stub returned: entropy=0.5, coherence=0.8, learningScore=0.65
    // StubUtlProcessor returns: entropy=0.0, coherence=0.0, learning_score=0.0
    // This test verifies we're NOT returning the old hardcoded values

    let handlers = create_test_handlers();
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_memetic_status",
            "arguments": {}
        })),
    );

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Must have result");
    let content = result.get("content").expect("Must have content");
    let text = content[0].get("text").expect("Must have text");
    let data: serde_json::Value = serde_json::from_str(text.as_str().unwrap()).unwrap();

    let utl = data.get("utl").expect("Must have utl");

    // If these exact values appear, we're still using hardcoded stubs
    let entropy = utl["entropy"].as_f64().unwrap();
    let coherence = utl["coherence"].as_f64().unwrap();
    let learning_score = utl["learningScore"].as_f64().unwrap();

    // StubUtlProcessor returns all zeros initially
    // The OLD hardcoded stub returned 0.5, 0.8, 0.65
    let is_old_hardcoded = (entropy - 0.5).abs() < 0.001
        && (coherence - 0.8).abs() < 0.001
        && (learning_score - 0.65).abs() < 0.001;

    assert!(
        !is_old_hardcoded,
        "Values appear to be old hardcoded stub (0.5, 0.8, 0.65). \
         Implementation should use self.utl_processor.get_status()"
    );
}

#[tokio::test]
async fn test_get_memetic_status_consolidation_phase_valid() {
    let handlers = create_test_handlers();
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_memetic_status",
            "arguments": {}
        })),
    );

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("Must have result");
    let content = result.get("content").expect("Must have content");
    let text = content[0].get("text").expect("Must have text");
    let data: serde_json::Value = serde_json::from_str(text.as_str().unwrap()).unwrap();

    let utl = data.get("utl").expect("Must have utl");
    let phase = utl["consolidationPhase"]
        .as_str()
        .expect("consolidationPhase must be string");

    // Per constitution.yaml:211-213 (dream phases)
    let valid_phases = ["NREM", "REM", "Wake"];
    assert!(
        valid_phases.contains(&phase),
        "Invalid consolidation phase: '{}'. Must be one of {:?}",
        phase,
        valid_phases
    );
}

// =========================================================================
// store_memory Tool Tests
// =========================================================================

#[tokio::test]
async fn test_tools_call_store_memory_valid() {
    let handlers = create_test_handlers();
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
async fn test_tools_call_search_graph_valid() {
    let handlers = create_test_handlers();
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
    let handlers = create_test_handlers();
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
    let handlers = create_test_handlers();
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

// =========================================================================
// utl_status Tool Tests
// =========================================================================

#[tokio::test]
async fn test_tools_call_utl_status() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "utl_status",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "utl_status should not return an error"
    );
    let result = response.result.expect("tools/call must return a result");

    // MCP REQUIREMENT: isError MUST be false for successful calls
    let is_error = result
        .get("isError")
        .expect("Response must contain isError")
        .as_bool()
        .expect("isError must be a boolean");
    assert!(!is_error, "isError must be false for utl_status");

    // Verify MCP format
    let content = result
        .get("content")
        .expect("Response must contain content array")
        .as_array()
        .expect("content must be an array");
    assert!(!content.is_empty(), "Content array must not be empty");

    let text = content[0]
        .get("text")
        .expect("Content item must have text")
        .as_str()
        .expect("text must be a string");
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("Content text should be valid JSON");

    // Verify UtlStatusResponse schema fields
    assert!(
        parsed.get("lifecycle_phase").is_some(),
        "Response must contain lifecycle_phase"
    );
    assert!(
        parsed.get("interaction_count").is_some(),
        "Response must contain interaction_count"
    );
    assert!(
        parsed.get("entropy").is_some(),
        "Response must contain entropy"
    );
    assert!(
        parsed.get("coherence").is_some(),
        "Response must contain coherence"
    );
    assert!(
        parsed.get("learning_score").is_some(),
        "Response must contain learning_score"
    );
    assert!(
        parsed.get("johari_quadrant").is_some(),
        "Response must contain johari_quadrant"
    );
    assert!(
        parsed.get("consolidation_phase").is_some(),
        "Response must contain consolidation_phase"
    );
    assert!(
        parsed.get("phase_angle").is_some(),
        "Response must contain phase_angle"
    );
    assert!(
        parsed.get("thresholds").is_some(),
        "Response must contain thresholds"
    );

    // Verify thresholds sub-structure
    let thresholds = parsed.get("thresholds").unwrap();
    assert!(
        thresholds.get("entropy_trigger").is_some(),
        "thresholds must contain entropy_trigger"
    );
    assert!(
        thresholds.get("coherence_trigger").is_some(),
        "thresholds must contain coherence_trigger"
    );
    assert!(
        thresholds.get("min_importance_store").is_some(),
        "thresholds must contain min_importance_store"
    );
    assert!(
        thresholds.get("consolidation_threshold").is_some(),
        "thresholds must contain consolidation_threshold"
    );
}

#[tokio::test]
async fn test_tools_call_utl_status_returns_stub_values() {
    let handlers = create_test_handlers();
    let params = json!({
        "name": "utl_status",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("tools/call must return a result");
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    // Verify stub returns expected initial values
    assert_eq!(
        parsed.get("lifecycle_phase").unwrap().as_str().unwrap(),
        "Infancy",
        "Stub should return Infancy lifecycle phase"
    );
    assert_eq!(
        parsed.get("interaction_count").unwrap().as_u64().unwrap(),
        0,
        "Stub should return 0 interaction count"
    );
    assert_eq!(
        parsed.get("consolidation_phase").unwrap().as_str().unwrap(),
        "Wake",
        "Stub should return Wake consolidation phase"
    );
}
