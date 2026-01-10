//! Purpose Handler Tests
//!
//! TASK-S003: Tests for purpose/query, goal/hierarchy_query,
//! goal/aligned_memories, and purpose/drift_check handlers.
//!
//! TASK-CORE-001: Removed tests for deprecated methods per ARCH-03:
//! - purpose/north_star_alignment - Returns METHOD_NOT_FOUND (-32601)
//! - purpose/north_star_update - Returns METHOD_NOT_FOUND (-32601)
//!
//! Uses STUBS (InMemoryTeleologicalStore, StubMultiArrayProvider) with real GoalHierarchy.
//!
//! Tests verify:
//! - purpose/query with 13D purpose vector similarity
//! - goal/hierarchy_query operations (get_all, get_goal, get_children, get_ancestors, get_subtree)
//! - goal/aligned_memories for finding memories aligned to specific goals
//! - purpose/drift_check for detecting alignment drift
//! - Deprecated methods return METHOD_NOT_FOUND
//! - Error handling for invalid parameters

use serde_json::json;

use crate::protocol::JsonRpcId;
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

use super::{create_test_handlers, create_test_handlers_no_north_star, make_request};
use crate::handlers::Handlers;

// =============================================================================
// Helper Functions for UUID-based Goal Tests (TASK-CORE-005)
// =============================================================================

/// Extract goal IDs from the hierarchy via get_all query.
/// Returns (north_star_id, strategic_ids, tactical_ids, immediate_ids).
async fn get_goal_ids_from_hierarchy(
    handlers: &Handlers,
) -> (String, Vec<String>, Vec<String>, Vec<String>) {
    let query_params = json!({ "operation": "get_all" });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(999)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;
    let result = response.result.expect("get_all should succeed");
    let goals = result
        .get("goals")
        .and_then(|v| v.as_array())
        .expect("Should have goals");

    let mut north_star_id = String::new();
    let mut strategic_ids = Vec::new();
    let mut tactical_ids = Vec::new();
    let mut immediate_ids = Vec::new();

    for goal in goals {
        let id = goal
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let level = goal.get("level").and_then(|v| v.as_str()).unwrap_or("");
        match level {
            "NorthStar" => north_star_id = id,
            "Strategic" => strategic_ids.push(id),
            "Tactical" => tactical_ids.push(id),
            "Immediate" => immediate_ids.push(id),
            _ => {}
        }
    }

    (north_star_id, strategic_ids, tactical_ids, immediate_ids)
}

// =============================================================================
// purpose/query Tests
// =============================================================================

/// Test purpose/query with valid 13-element purpose vector.
#[tokio::test]
async fn test_purpose_query_valid_vector() {
    let handlers = create_test_handlers();

    // First store some content to search
    let store_params = json!({
        "content": "Machine learning enables computers to learn from data",
        "importance": 0.9
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    handlers.dispatch(store_request).await;

    // Query with 13D purpose vector
    let purpose_vector: Vec<f64> = vec![
        0.8, 0.5, 0.3, 0.3, 0.6, 0.2, 0.7, 0.5, 0.4, 0.3, 0.5, 0.3, 0.2,
    ];

    let query_params = json!({
        "purpose_vector": purpose_vector,
        "topK": 10
    });
    let query_request = make_request(
        "purpose/query",
        Some(JsonRpcId::Number(2)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(response.error.is_none(), "purpose/query should succeed");
    let result = response.result.expect("Should have result");

    // Verify response structure
    assert!(result.get("results").is_some(), "Should have results array");
    assert!(result.get("count").is_some(), "Should have count");
    assert!(
        result.get("query_metadata").is_some(),
        "Should have query_metadata"
    );

    // Verify query_metadata structure
    let metadata = result.get("query_metadata").unwrap();
    assert!(
        metadata.get("purpose_vector_used").is_some(),
        "Should include purpose_vector_used"
    );
    assert!(
        metadata.get("dominant_embedder").is_some(),
        "Should include dominant_embedder"
    );
    assert!(
        metadata.get("query_coherence").is_some(),
        "Should include query_coherence"
    );
    assert!(
        metadata.get("search_time_ms").is_some(),
        "Should report search_time_ms"
    );

    // Verify purpose_vector_used has 13 elements
    let pv_used = metadata
        .get("purpose_vector_used")
        .and_then(|v| v.as_array());
    assert!(pv_used.is_some(), "purpose_vector_used must be array");
    assert_eq!(
        pv_used.unwrap().len(),
        NUM_EMBEDDERS,
        "purpose_vector_used must have 13 elements"
    );
}

/// Test purpose/query fails with missing purpose_vector.
#[tokio::test]
async fn test_purpose_query_missing_vector_fails() {
    let handlers = create_test_handlers();

    let query_params = json!({
        "topK": 10
    });
    let query_request = make_request(
        "purpose/query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_some(),
        "purpose/query must fail without purpose_vector"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
    assert!(
        error.message.contains("purpose_vector"),
        "Error should mention missing purpose_vector"
    );
}

/// Test purpose/query fails with 12-element purpose vector (must be 13).
#[tokio::test]
async fn test_purpose_query_wrong_vector_size_fails() {
    let handlers = create_test_handlers();

    // Only 12 elements (WRONG - must be 13!)
    let invalid_vector: Vec<f64> = vec![0.5; 12];

    let query_params = json!({
        "purpose_vector": invalid_vector
    });
    let query_request = make_request(
        "purpose/query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_some(),
        "purpose/query must fail with 12-element vector"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
    assert!(
        error.message.contains("13") || error.message.contains("elements"),
        "Error should mention 13 elements required"
    );
}

/// Test purpose/query fails with out-of-range values.
#[tokio::test]
async fn test_purpose_query_out_of_range_values_fails() {
    let handlers = create_test_handlers();

    // Value > 1.0 is invalid
    let mut invalid_vector: Vec<f64> = vec![0.5; 13];
    invalid_vector[5] = 1.5; // Out of range!

    let query_params = json!({
        "purpose_vector": invalid_vector
    });
    let query_request = make_request(
        "purpose/query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_some(),
        "purpose/query must fail with out-of-range value"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
    assert!(
        error.message.contains("range")
            || error.message.contains("0.0")
            || error.message.contains("1.0"),
        "Error should mention valid range [0.0, 1.0]"
    );
}

/// Test purpose/query with min_alignment filter.
#[tokio::test]
async fn test_purpose_query_min_alignment_filter() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Purpose alignment testing content",
        "importance": 0.8
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    handlers.dispatch(store_request).await;

    let purpose_vector: Vec<f64> = vec![0.7; 13];

    let query_params = json!({
        "purpose_vector": purpose_vector,
        "minAlignment": 0.5,
        "topK": 5
    });
    let query_request = make_request(
        "purpose/query",
        Some(JsonRpcId::Number(2)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(response.error.is_none(), "purpose/query should succeed");
    let result = response.result.expect("Should have result");

    // Verify min_alignment_filter is reported
    let metadata = result.get("query_metadata").unwrap();
    let min_filter = metadata
        .get("min_alignment_filter")
        .and_then(|v| v.as_f64());
    assert_eq!(min_filter, Some(0.5), "Should report min_alignment_filter");
}

// =============================================================================
// purpose/north_star_alignment Tests - TASK-CORE-001: DEPRECATED
// =============================================================================
// NOTE: purpose/north_star_alignment was removed per ARCH-03 (autonomous-first).
// All calls now return METHOD_NOT_FOUND (-32601).
// Use auto_bootstrap_north_star tool for autonomous goal discovery instead.

/// TASK-CORE-001: Test purpose/north_star_alignment returns METHOD_NOT_FOUND.
#[tokio::test]
async fn test_north_star_alignment_valid_fingerprint() {
    let handlers = create_test_handlers();

    // Store content first
    let store_params = json!({
        "content": "Building the best ML learning system for education",
        "importance": 0.9
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    let store_response = handlers.dispatch(store_request).await;
    let fingerprint_id = store_response
        .result
        .unwrap()
        .get("fingerprintId")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Check alignment - should return METHOD_NOT_FOUND (deprecated)
    let align_params = json!({
        "fingerprint_id": fingerprint_id,
        "include_breakdown": true,
        "include_patterns": true
    });
    let align_request = make_request(
        "purpose/north_star_alignment",
        Some(JsonRpcId::Number(2)),
        Some(align_params),
    );
    let response = handlers.dispatch(align_request).await;

    // TASK-CORE-001: Deprecated method returns METHOD_NOT_FOUND
    assert!(
        response.error.is_some(),
        "purpose/north_star_alignment must return error (deprecated per TASK-CORE-001)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601), got {}",
        error.code
    );
}

/// TASK-CORE-001: Test purpose/north_star_alignment returns METHOD_NOT_FOUND for missing params.
#[tokio::test]
async fn test_north_star_alignment_missing_id_fails() {
    let handlers = create_test_handlers();

    let align_params = json!({
        "include_breakdown": true
    });
    let align_request = make_request(
        "purpose/north_star_alignment",
        Some(JsonRpcId::Number(1)),
        Some(align_params),
    );
    let response = handlers.dispatch(align_request).await;

    // TASK-CORE-001: Deprecated method returns METHOD_NOT_FOUND (not INVALID_PARAMS)
    assert!(
        response.error.is_some(),
        "purpose/north_star_alignment must return error (deprecated per TASK-CORE-001)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601), got {}",
        error.code
    );
}

/// TASK-CORE-001: Test purpose/north_star_alignment returns METHOD_NOT_FOUND for invalid UUID.
#[tokio::test]
async fn test_north_star_alignment_invalid_uuid_fails() {
    let handlers = create_test_handlers();

    let align_params = json!({
        "fingerprint_id": "not-a-valid-uuid"
    });
    let align_request = make_request(
        "purpose/north_star_alignment",
        Some(JsonRpcId::Number(1)),
        Some(align_params),
    );
    let response = handlers.dispatch(align_request).await;

    // TASK-CORE-001: Deprecated method returns METHOD_NOT_FOUND (not INVALID_PARAMS)
    assert!(
        response.error.is_some(),
        "purpose/north_star_alignment must return error (deprecated per TASK-CORE-001)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601), got {}",
        error.code
    );
}

/// TASK-CORE-001: Test purpose/north_star_alignment returns METHOD_NOT_FOUND for non-existent ID.
#[tokio::test]
async fn test_north_star_alignment_not_found_fails() {
    let handlers = create_test_handlers();

    let align_params = json!({
        "fingerprint_id": "00000000-0000-0000-0000-000000000000"
    });
    let align_request = make_request(
        "purpose/north_star_alignment",
        Some(JsonRpcId::Number(1)),
        Some(align_params),
    );
    let response = handlers.dispatch(align_request).await;

    // TASK-CORE-001: Deprecated method returns METHOD_NOT_FOUND (not FINGERPRINT_NOT_FOUND)
    assert!(
        response.error.is_some(),
        "purpose/north_star_alignment must return error (deprecated per TASK-CORE-001)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601), got {}",
        error.code
    );
}

/// Test autonomous operation: store succeeds without North Star.
///
/// AUTONOMOUS OPERATION: Per contextprd.md, memory storage works without North Star
/// by using a default purpose vector [0.0; 13]. The 13-embedding array IS the
/// teleological vector - purpose alignment is secondary metadata.
///
/// TASK-CORE-001: purpose/north_star_alignment is deprecated, returns METHOD_NOT_FOUND.
#[tokio::test]
async fn test_north_star_alignment_autonomous_operation() {
    let handlers = create_test_handlers_no_north_star();

    // Store content - should SUCCEED without North Star (AUTONOMOUS OPERATION)
    let store_params = json!({
        "content": "Test content for autonomous operation",
        "importance": 0.8
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    let store_response = handlers.dispatch(store_request).await;

    // Store should succeed with default purpose vector
    assert!(
        store_response.error.is_none(),
        "memory/store must succeed without North Star (AUTONOMOUS OPERATION). Error: {:?}",
        store_response.error
    );
    let result = store_response.result.expect("Should have result");
    assert!(
        result.get("fingerprintId").is_some(),
        "Must return fingerprintId"
    );

    // TASK-CORE-001: purpose/north_star_alignment is deprecated
    let align_params = json!({
        "fingerprint_id": "00000000-0000-0000-0000-000000000001"
    });
    let align_request = make_request(
        "purpose/north_star_alignment",
        Some(JsonRpcId::Number(2)),
        Some(align_params),
    );
    let response = handlers.dispatch(align_request).await;

    // Should fail with METHOD_NOT_FOUND (deprecated per TASK-CORE-001)
    assert!(
        response.error.is_some(),
        "purpose/north_star_alignment must return error (deprecated per TASK-CORE-001)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601), got {}",
        error.code
    );
}

// =============================================================================
// goal/hierarchy_query Tests
// =============================================================================

/// Test goal/hierarchy_query get_all operation.
#[tokio::test]
async fn test_goal_hierarchy_get_all() {
    let handlers = create_test_handlers();

    let query_params = json!({
        "operation": "get_all"
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_none(),
        "goal/hierarchy_query get_all should succeed"
    );
    let result = response.result.expect("Should have result");

    let goals = result.get("goals").and_then(|v| v.as_array());
    assert!(goals.is_some(), "Should have goals array");
    assert!(
        !goals.unwrap().is_empty(),
        "Should have at least one goal (North Star)"
    );

    let stats = result
        .get("hierarchy_stats")
        .expect("Should have hierarchy_stats");
    assert!(
        stats.get("total_goals").is_some(),
        "Should have total_goals"
    );
    assert!(
        stats.get("has_north_star").is_some(),
        "Should have has_north_star"
    );
    assert!(
        stats.get("level_counts").is_some(),
        "Should have level_counts"
    );
}

/// Test goal/hierarchy_query get_goal operation.
/// TASK-CORE-005: Updated to use UUID-based goal IDs instead of hardcoded strings.
#[tokio::test]
async fn test_goal_hierarchy_get_goal() {
    let handlers = create_test_handlers();

    // First, get the actual North Star ID from the hierarchy
    let (north_star_id, _, _, _) = get_goal_ids_from_hierarchy(&handlers).await;
    assert!(!north_star_id.is_empty(), "Should have North Star goal");

    let query_params = json!({
        "operation": "get_goal",
        "goal_id": north_star_id
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_none(),
        "goal/hierarchy_query get_goal should succeed"
    );
    let result = response.result.expect("Should have result");

    let goal = result.get("goal").expect("Should have goal");
    assert_eq!(
        goal.get("id").and_then(|v| v.as_str()),
        Some(north_star_id.as_str()),
        "Should return correct goal"
    );
    assert_eq!(
        goal.get("level").and_then(|v| v.as_str()),
        Some("NorthStar"),
        "Should be NorthStar level"
    );
    assert_eq!(
        goal.get("is_north_star").and_then(|v| v.as_bool()),
        Some(true),
        "Should be marked as North Star"
    );
}

/// Test goal/hierarchy_query get_children operation.
/// TASK-CORE-005: Updated to use UUID-based goal IDs instead of hardcoded strings.
#[tokio::test]
async fn test_goal_hierarchy_get_children() {
    let handlers = create_test_handlers();

    // First, get the actual North Star ID from the hierarchy
    let (north_star_id, _, _, _) = get_goal_ids_from_hierarchy(&handlers).await;
    assert!(!north_star_id.is_empty(), "Should have North Star goal");

    let query_params = json!({
        "operation": "get_children",
        "goal_id": north_star_id
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_none(),
        "goal/hierarchy_query get_children should succeed"
    );
    let result = response.result.expect("Should have result");

    assert_eq!(
        result.get("parent_goal_id").and_then(|v| v.as_str()),
        Some(north_star_id.as_str()),
        "Should return parent_goal_id"
    );

    let children = result.get("children").and_then(|v| v.as_array());
    assert!(children.is_some(), "Should have children array");

    // From test hierarchy, North Star has 2 strategic children
    let children = children.unwrap();
    assert_eq!(
        children.len(),
        2,
        "North Star should have 2 strategic children"
    );
}

/// Test goal/hierarchy_query get_ancestors operation.
/// TASK-CORE-005: Updated to use UUID-based goal IDs instead of hardcoded strings.
#[tokio::test]
async fn test_goal_hierarchy_get_ancestors() {
    let handlers = create_test_handlers();

    // First, get the actual immediate goal ID from the hierarchy
    let (_, _, _, immediate_ids) = get_goal_ids_from_hierarchy(&handlers).await;
    assert!(
        !immediate_ids.is_empty(),
        "Should have at least one Immediate goal"
    );
    let immediate_id = &immediate_ids[0];

    // Get ancestors of immediate goal
    let query_params = json!({
        "operation": "get_ancestors",
        "goal_id": immediate_id
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_none(),
        "goal/hierarchy_query get_ancestors should succeed"
    );
    let result = response.result.expect("Should have result");

    let ancestors = result.get("ancestors").and_then(|v| v.as_array());
    assert!(ancestors.is_some(), "Should have ancestors array");

    // Path should be: Immediate -> Tactical -> Strategic -> NorthStar
    let ancestors = ancestors.unwrap();
    assert!(
        ancestors.len() >= 3,
        "Should have at least 3 ancestors (including self)"
    );
}

/// Test goal/hierarchy_query get_subtree operation.
/// TASK-CORE-005: Updated to use UUID-based goal IDs instead of hardcoded strings.
#[tokio::test]
async fn test_goal_hierarchy_get_subtree() {
    let handlers = create_test_handlers();

    // First, get the actual strategic goal ID from the hierarchy
    let (_, strategic_ids, _, _) = get_goal_ids_from_hierarchy(&handlers).await;
    assert!(
        !strategic_ids.is_empty(),
        "Should have at least one Strategic goal"
    );
    let strategic_id = &strategic_ids[0];

    // Get subtree rooted at strategic goal
    let query_params = json!({
        "operation": "get_subtree",
        "goal_id": strategic_id
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_none(),
        "goal/hierarchy_query get_subtree should succeed"
    );
    let result = response.result.expect("Should have result");

    assert_eq!(
        result.get("root_goal_id").and_then(|v| v.as_str()),
        Some(strategic_id.as_str()),
        "Should return root_goal_id"
    );

    let subtree = result.get("subtree").and_then(|v| v.as_array());
    assert!(subtree.is_some(), "Should have subtree array");

    // Subtree of s1_retrieval includes: s1_retrieval, t1_semantic, i1_vector
    let subtree = subtree.unwrap();
    assert!(subtree.len() >= 3, "Subtree should have at least 3 nodes");
}

/// Test goal/hierarchy_query fails with missing operation.
#[tokio::test]
async fn test_goal_hierarchy_missing_operation_fails() {
    let handlers = create_test_handlers();

    let query_params = json!({
        "goal_id": "ns_ml_system"
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_some(),
        "goal/hierarchy_query must fail without operation"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
    assert!(
        error.message.contains("operation"),
        "Error should mention missing operation"
    );
}

/// Test goal/hierarchy_query fails with unknown operation.
#[tokio::test]
async fn test_goal_hierarchy_unknown_operation_fails() {
    let handlers = create_test_handlers();

    let query_params = json!({
        "operation": "invalid_op"
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_some(),
        "goal/hierarchy_query must fail with unknown operation"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
}

/// Test goal/hierarchy_query fails with non-existent goal.
/// TASK-CORE-005: Updated to use valid UUID format for non-existent goal.
#[tokio::test]
async fn test_goal_hierarchy_goal_not_found_fails() {
    let handlers = create_test_handlers();

    // Use a valid UUID format that doesn't exist in the hierarchy
    let query_params = json!({
        "operation": "get_goal",
        "goal_id": "00000000-0000-0000-0000-000000000000"
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    assert!(
        response.error.is_some(),
        "goal/hierarchy_query must fail with non-existent goal"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32020,
        "Should return GOAL_NOT_FOUND error code"
    );
}

// =============================================================================
// goal/aligned_memories Tests
// =============================================================================

/// Test goal/aligned_memories with valid goal.
/// TASK-CORE-005: Updated to use UUID-based goal IDs instead of hardcoded strings.
#[tokio::test]
async fn test_goal_aligned_memories_valid() {
    let handlers = create_test_handlers();

    // First, get the actual strategic goal ID from the hierarchy
    let (_, strategic_ids, _, _) = get_goal_ids_from_hierarchy(&handlers).await;
    assert!(
        !strategic_ids.is_empty(),
        "Should have at least one Strategic goal"
    );
    let strategic_id = &strategic_ids[0];

    // Store content
    let store_params = json!({
        "content": "Improving retrieval accuracy through semantic search",
        "importance": 0.9
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    handlers.dispatch(store_request).await;

    // Find memories aligned to strategic goal
    let aligned_params = json!({
        "goal_id": strategic_id,
        "topK": 10,
        "minAlignment": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let aligned_request = make_request(
        "goal/aligned_memories",
        Some(JsonRpcId::Number(2)),
        Some(aligned_params),
    );
    let response = handlers.dispatch(aligned_request).await;

    assert!(
        response.error.is_none(),
        "goal/aligned_memories should succeed"
    );
    let result = response.result.expect("Should have result");

    let goal = result.get("goal").expect("Should have goal");
    assert_eq!(
        goal.get("id").and_then(|v| v.as_str()),
        Some(strategic_id.as_str()),
        "Should return correct goal"
    );

    assert!(result.get("results").is_some(), "Should have results array");
    assert!(result.get("count").is_some(), "Should have count");
    assert!(
        result.get("search_time_ms").is_some(),
        "Should report search_time_ms"
    );
}

/// Test goal/aligned_memories fails with missing goal_id.
#[tokio::test]
async fn test_goal_aligned_memories_missing_id_fails() {
    let handlers = create_test_handlers();

    let aligned_params = json!({
        "topK": 10
    });
    let aligned_request = make_request(
        "goal/aligned_memories",
        Some(JsonRpcId::Number(1)),
        Some(aligned_params),
    );
    let response = handlers.dispatch(aligned_request).await;

    assert!(
        response.error.is_some(),
        "goal/aligned_memories must fail without goal_id"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
    assert!(
        error.message.contains("goal_id"),
        "Error should mention missing goal_id"
    );
}

/// Test goal/aligned_memories fails with non-existent goal.
/// TASK-CORE-005: Updated to use valid UUID format for non-existent goal.
#[tokio::test]
async fn test_goal_aligned_memories_goal_not_found_fails() {
    let handlers = create_test_handlers();

    // Use a valid UUID format that doesn't exist in the hierarchy
    let aligned_params = json!({
        "goal_id": "00000000-0000-0000-0000-000000000000",
        "minAlignment": 0.0  // P1-FIX-1: Required parameter (test expects goal not found error)
    });
    let aligned_request = make_request(
        "goal/aligned_memories",
        Some(JsonRpcId::Number(1)),
        Some(aligned_params),
    );
    let response = handlers.dispatch(aligned_request).await;

    assert!(
        response.error.is_some(),
        "goal/aligned_memories must fail with non-existent goal"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32020,
        "Should return GOAL_NOT_FOUND error code"
    );
}

// =============================================================================
// purpose/drift_check Tests
// =============================================================================

/// Test purpose/drift_check with valid fingerprints.
#[tokio::test]
async fn test_drift_check_valid_fingerprints() {
    let handlers = create_test_handlers();

    // Store multiple fingerprints
    let mut fingerprint_ids = Vec::new();
    for i in 0..3 {
        let store_params = json!({
            "content": format!("Drift check test content number {}", i),
            "importance": 0.7
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(i as i64 + 1)),
            Some(store_params),
        );
        let store_response = handlers.dispatch(store_request).await;
        let fp_id = store_response
            .result
            .unwrap()
            .get("fingerprintId")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        fingerprint_ids.push(fp_id);
    }

    // Check drift
    let drift_params = json!({
        "fingerprint_ids": fingerprint_ids,
        "threshold": 0.1
    });
    let drift_request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(10)),
        Some(drift_params),
    );
    let response = handlers.dispatch(drift_request).await;

    assert!(
        response.error.is_none(),
        "purpose/drift_check should succeed"
    );
    let result = response.result.expect("Should have result");

    // TASK-INTEG-002: Verify NEW response structure with TeleologicalDriftDetector
    // Response now includes: overall_drift, per_embedder_drift, most_drifted_embedders, recommendations, trend

    // Verify overall_drift (5-level classification: None, Low, Medium, High, Critical)
    let overall_drift = result.get("overall_drift").expect("Must have overall_drift");
    assert!(overall_drift.get("level").is_some(), "Must have drift level");
    assert!(overall_drift.get("similarity").is_some(), "Must have similarity");
    assert!(overall_drift.get("drift_score").is_some(), "Must have drift_score");
    assert!(overall_drift.get("has_drifted").is_some(), "Must have has_drifted");

    // Verify per_embedder_drift (exactly 13 entries, one per embedder E1-E13)
    let per_embedder = result.get("per_embedder_drift").and_then(|v| v.as_array());
    assert!(per_embedder.is_some(), "Must have per_embedder_drift array");
    assert_eq!(per_embedder.unwrap().len(), 13, "Must have exactly 13 embedder entries");

    // Verify most_drifted_embedders (top 5, sorted worst-first)
    let most_drifted = result.get("most_drifted_embedders").and_then(|v| v.as_array());
    assert!(most_drifted.is_some(), "Must have most_drifted_embedders");
    assert!(most_drifted.unwrap().len() <= 5, "Must have at most 5 most drifted");

    // Verify recommendations array
    assert!(result.get("recommendations").and_then(|v| v.as_array()).is_some(), "Must have recommendations");

    // Verify analyzed_count and check_time_ms
    assert!(result.get("analyzed_count").is_some(), "Must have analyzed_count");
    assert!(result.get("check_time_ms").is_some(), "Must have check_time_ms");
    assert!(result.get("timestamp").is_some(), "Must have timestamp");
}

/// Test purpose/drift_check fails with missing fingerprint_ids.
#[tokio::test]
async fn test_drift_check_missing_ids_fails() {
    let handlers = create_test_handlers();

    let drift_params = json!({
        "threshold": 0.1
    });
    let drift_request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(1)),
        Some(drift_params),
    );
    let response = handlers.dispatch(drift_request).await;

    assert!(
        response.error.is_some(),
        "purpose/drift_check must fail without fingerprint_ids"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
    assert!(
        error.message.contains("fingerprint_ids"),
        "Error should mention missing fingerprint_ids"
    );
}

/// Test purpose/drift_check fails with empty fingerprint_ids array.
#[tokio::test]
async fn test_drift_check_empty_ids_fails() {
    let handlers = create_test_handlers();

    let drift_params = json!({
        "fingerprint_ids": []
    });
    let drift_request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(1)),
        Some(drift_params),
    );
    let response = handlers.dispatch(drift_request).await;

    assert!(
        response.error.is_some(),
        "purpose/drift_check must fail with empty fingerprint_ids"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32602,
        "Should return INVALID_PARAMS error code"
    );
}

/// Test autonomous operation: store succeeds without North Star.
///
/// AUTONOMOUS OPERATION: Per contextprd.md, memory storage works without North Star
/// by using a default purpose vector [0.0; 13]. The 13-embedding array IS the
/// teleological vector - purpose alignment is secondary metadata.
///
/// Note: drift_check inherently requires a North Star (to compare alignment drift),
/// so this test focuses on verifying that store works autonomously. Drift detection
/// becomes meaningful only after a North Star is established.
#[tokio::test]
async fn test_store_autonomous_operation_for_drift() {
    let handlers = create_test_handlers_no_north_star();

    // Store content - should SUCCEED without North Star (AUTONOMOUS OPERATION)
    let store_params = json!({
        "content": "Test content for autonomous drift check",
        "importance": 0.8
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    let store_response = handlers.dispatch(store_request).await;

    // Store should succeed with default purpose vector
    assert!(
        store_response.error.is_none(),
        "memory/store must succeed without North Star (AUTONOMOUS OPERATION). Error: {:?}",
        store_response.error
    );
    let result = store_response.result.expect("Should have result");
    let fingerprint_id = result
        .get("fingerprintId")
        .expect("Must return fingerprintId");

    // Verify the response contains expected fields demonstrating autonomous storage
    assert!(
        result.get("embeddingLatencyMs").is_some(),
        "Must include embedding latency"
    );

    println!(
        "Successfully stored fingerprint {} autonomously (no North Star)",
        fingerprint_id
    );
}

/// TASK-INTEG-002: Test purpose/drift_check FAILS FAST on not-found fingerprints.
/// Per FAIL FAST design, missing fingerprints should return an error, not succeed.
#[tokio::test]
async fn test_drift_check_not_found_fingerprints() {
    let handlers = create_test_handlers();

    let drift_params = json!({
        "fingerprint_ids": [
            "00000000-0000-0000-0000-000000000001",
            "00000000-0000-0000-0000-000000000002"
        ]
    });
    let drift_request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(1)),
        Some(drift_params),
    );
    let response = handlers.dispatch(drift_request).await;

    // FAIL FAST: Not-found fingerprints should return error
    assert!(
        response.error.is_some(),
        "purpose/drift_check must FAIL FAST when fingerprints not found"
    );
    let error = response.error.unwrap();

    // Verify error code is FINGERPRINT_NOT_FOUND (-32010)
    assert_eq!(
        error.code, -32010,
        "Must return FINGERPRINT_NOT_FOUND error code"
    );
    assert!(
        error.message.contains("not found") || error.message.contains("No fingerprint"),
        "Error message must indicate fingerprint not found"
    );
}

// =============================================================================
// purpose/north_star_update Tests (DEPRECATED - TASK-CORE-001)
// =============================================================================
// NOTE: purpose/north_star_update is REMOVED per ARCH-03 (autonomous-first).
// All tests now verify METHOD_NOT_FOUND (-32601) is returned.
// Use auto_bootstrap_north_star for autonomous goal discovery instead.

/// TASK-CORE-001: Test purpose/north_star_update returns METHOD_NOT_FOUND.
/// Previously tested creating new North Star, now verifies deprecation.
#[tokio::test]
async fn test_north_star_update_create_returns_method_not_found() {
    let handlers = create_test_handlers_no_north_star();

    let update_params = json!({
        "description": "Build the best AI assistant for developers",
        "keywords": ["ai", "assistant", "developer"]
    });
    let update_request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(update_params),
    );
    let response = handlers.dispatch(update_request).await;

    // TASK-CORE-001: Must return METHOD_NOT_FOUND for deprecated method
    assert!(
        response.error.is_some(),
        "purpose/north_star_update must return error (deprecated per ARCH-03)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );
}

/// TASK-CORE-001: Test purpose/north_star_update with replace=false returns METHOD_NOT_FOUND.
#[tokio::test]
async fn test_north_star_update_exists_no_replace_returns_method_not_found() {
    let handlers = create_test_handlers();

    let update_params = json!({
        "description": "New North Star goal",
        "replace": false
    });
    let update_request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(update_params),
    );
    let response = handlers.dispatch(update_request).await;

    // TASK-CORE-001: Must return METHOD_NOT_FOUND for deprecated method
    assert!(
        response.error.is_some(),
        "purpose/north_star_update must return error (deprecated per ARCH-03)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );
}

/// TASK-CORE-001: Test purpose/north_star_update with replace=true returns METHOD_NOT_FOUND.
#[tokio::test]
async fn test_north_star_update_replace_returns_method_not_found() {
    let handlers = create_test_handlers();

    let update_params = json!({
        "description": "New improved North Star goal",
        "keywords": ["improved", "goal"],
        "replace": true
    });
    let update_request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(update_params),
    );
    let response = handlers.dispatch(update_request).await;

    // TASK-CORE-001: Must return METHOD_NOT_FOUND for deprecated method
    assert!(
        response.error.is_some(),
        "purpose/north_star_update must return error (deprecated per ARCH-03)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );
}

/// TASK-CORE-001: Test purpose/north_star_update without description returns METHOD_NOT_FOUND.
/// Note: METHOD_NOT_FOUND takes precedence over INVALID_PARAMS since method is removed.
#[tokio::test]
async fn test_north_star_update_missing_description_returns_method_not_found() {
    let handlers = create_test_handlers_no_north_star();

    let update_params = json!({
        "keywords": ["test"]
    });
    let update_request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(update_params),
    );
    let response = handlers.dispatch(update_request).await;

    // TASK-CORE-001: Must return METHOD_NOT_FOUND for deprecated method
    assert!(
        response.error.is_some(),
        "purpose/north_star_update must return error (deprecated per ARCH-03)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );
}

/// TASK-CORE-001: Test purpose/north_star_update with empty description returns METHOD_NOT_FOUND.
#[tokio::test]
async fn test_north_star_update_empty_description_returns_method_not_found() {
    let handlers = create_test_handlers_no_north_star();

    let update_params = json!({
        "description": ""
    });
    let update_request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(update_params),
    );
    let response = handlers.dispatch(update_request).await;

    // TASK-CORE-001: Must return METHOD_NOT_FOUND for deprecated method
    assert!(
        response.error.is_some(),
        "purpose/north_star_update must return error (deprecated per ARCH-03)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );
}

/// TASK-CORE-001: Test purpose/north_star_update with embedding returns METHOD_NOT_FOUND.
/// Note: METHOD_NOT_FOUND takes precedence over INVALID_PARAMS since method is removed.
#[tokio::test]
async fn test_north_star_update_with_embedding_returns_method_not_found() {
    let handlers = create_test_handlers_no_north_star();

    // 768 dimensions instead of 1024 - doesn't matter since method is removed
    let wrong_embedding: Vec<f64> = vec![0.5; 768];

    let update_params = json!({
        "description": "Test North Star",
        "embedding": wrong_embedding
    });
    let update_request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(update_params),
    );
    let response = handlers.dispatch(update_request).await;

    // TASK-CORE-001: Must return METHOD_NOT_FOUND for deprecated method
    assert!(
        response.error.is_some(),
        "purpose/north_star_update must return error (deprecated per ARCH-03)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );
}

// =============================================================================
// Full State Verification Test
// =============================================================================

/// FULL STATE VERIFICATION: Complete purpose workflow test.
///
/// TASK-CORE-001: Updated to remove deprecated north_star_alignment step.
///
/// Tests the full purpose lifecycle with real data:
/// 1. Create handlers with test hierarchy
/// 2. Verify hierarchy exists via goal/hierarchy_query get_all
/// 3. Store content and verify storage
/// 4. Query via purpose/query with 13D vector
/// 5. Find aligned memories via goal/aligned_memories
/// 6. Check drift via purpose/drift_check
///
/// NOTE: purpose/north_star_alignment removed per ARCH-03.
/// Uses real GoalHierarchy with STUB storage (InMemoryTeleologicalStore).
#[tokio::test]
async fn test_full_state_verification_purpose_workflow() {
    let handlers = create_test_handlers();

    // =========================================================================
    // STEP 1: Verify hierarchy exists via goal/hierarchy_query get_all
    // =========================================================================
    let hierarchy_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(json!({ "operation": "get_all" })),
    );
    let hierarchy_response = handlers.dispatch(hierarchy_request).await;

    assert!(
        hierarchy_response.error.is_none(),
        "goal/hierarchy_query get_all must succeed"
    );
    let hierarchy_result = hierarchy_response.result.expect("Must have result");

    let stats = hierarchy_result
        .get("hierarchy_stats")
        .expect("Must have stats");
    assert_eq!(
        stats.get("has_north_star").and_then(|v| v.as_bool()),
        Some(true),
        "Must have North Star configured"
    );

    let goals = hierarchy_result
        .get("goals")
        .and_then(|v| v.as_array())
        .expect("Must have goals");
    assert!(!goals.is_empty(), "Must have at least one goal");

    println!(
        "[FSV] STEP 1 VERIFIED: Hierarchy has {} goals with North Star",
        goals.len()
    );

    // =========================================================================
    // STEP 2: Store multiple fingerprints with different content
    // =========================================================================
    let contents = [
        "Machine learning algorithms for predictive analytics",
        "Improving retrieval accuracy through semantic embeddings",
        "Knowledge graph construction for entity linking",
    ];

    let mut stored_ids: Vec<String> = Vec::new();
    for (i, content) in contents.iter().enumerate() {
        let store_params = json!({
            "content": content,
            "importance": 0.7 + (i as f64 * 0.1)
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(10 + i as i64)),
            Some(store_params),
        );
        let store_response = handlers.dispatch(store_request).await;
        assert!(store_response.error.is_none(), "Store {} must succeed", i);

        let fp_id = store_response
            .result
            .unwrap()
            .get("fingerprintId")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        stored_ids.push(fp_id);
    }

    assert_eq!(stored_ids.len(), 3, "Must have stored 3 fingerprints");
    println!(
        "[FSV] STEP 2 VERIFIED: Stored {} fingerprints",
        stored_ids.len()
    );

    // =========================================================================
    // STEP 3: Query via purpose/query with 13D vector
    // =========================================================================
    let purpose_vector: Vec<f64> = vec![
        0.8, 0.5, 0.3, 0.3, 0.6, 0.2, 0.7, 0.5, 0.4, 0.3, 0.5, 0.3, 0.2,
    ];

    let purpose_query_params = json!({
        "purpose_vector": purpose_vector,
        "topK": 10,
        "include_scores": true
    });
    let purpose_query_request = make_request(
        "purpose/query",
        Some(JsonRpcId::Number(20)),
        Some(purpose_query_params),
    );
    let purpose_query_response = handlers.dispatch(purpose_query_request).await;

    assert!(
        purpose_query_response.error.is_none(),
        "purpose/query must succeed"
    );
    let purpose_result = purpose_query_response.result.expect("Must have result");
    let purpose_results = purpose_result
        .get("results")
        .and_then(|v| v.as_array())
        .expect("Must have results");

    println!(
        "[FSV] STEP 3 VERIFIED: purpose/query returned {} results",
        purpose_results.len()
    );

    // NOTE: STEP 4 (purpose/north_star_alignment) REMOVED per TASK-CORE-001 (ARCH-03)
    // Manual North Star alignment creates single 1024D embeddings incompatible with 13-embedder arrays.
    // Use auto_bootstrap_north_star tool for autonomous goal discovery instead.

    // =========================================================================
    // STEP 4: Find aligned memories via goal/aligned_memories
    // =========================================================================
    let aligned_params = json!({
        "goal_id": "s1_retrieval",
        "topK": 10,
        "minAlignment": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let aligned_request = make_request(
        "goal/aligned_memories",
        Some(JsonRpcId::Number(40)),
        Some(aligned_params),
    );
    let aligned_response = handlers.dispatch(aligned_request).await;

    assert!(
        aligned_response.error.is_none(),
        "goal/aligned_memories must succeed"
    );
    let aligned_result = aligned_response.result.expect("Must have result");
    let aligned_count = aligned_result
        .get("count")
        .and_then(|v| v.as_u64())
        .expect("Must have count");

    println!(
        "[FSV] STEP 4 VERIFIED: goal/aligned_memories found {} memories",
        aligned_count
    );

    // =========================================================================
    // STEP 5: Check drift via purpose/drift_check
    // =========================================================================
    let drift_params = json!({
        "fingerprint_ids": &stored_ids,
        "threshold": 0.1
    });
    let drift_request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(50)),
        Some(drift_params),
    );
    let drift_response = handlers.dispatch(drift_request).await;

    assert!(
        drift_response.error.is_none(),
        "purpose/drift_check must succeed"
    );
    let drift_result = drift_response.result.expect("Must have result");

    let summary = drift_result.get("summary").expect("Must have summary");
    let total_checked = summary
        .get("total_checked")
        .and_then(|v| v.as_u64())
        .expect("Must have total_checked");
    let drifted_count = summary
        .get("drifted_count")
        .and_then(|v| v.as_u64())
        .expect("Must have drifted_count");

    assert_eq!(total_checked, 3, "Must check all 3 fingerprints");

    println!(
        "[FSV] STEP 5 VERIFIED: drift_check checked {} fingerprints, {} drifted",
        total_checked, drifted_count
    );

    // =========================================================================
    // VERIFICATION COMPLETE
    // =========================================================================
    println!("\n======================================================================");
    println!("[FSV] FULL STATE VERIFICATION COMPLETE - All purpose handlers working");
    println!("======================================================================\n");
}
