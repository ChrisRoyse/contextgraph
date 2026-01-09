//! TASK-EMB-024: Full State Verification Tests
//!
//! This module verifies the TASK-EMB-024 implementation:
//! 1. StubSystemMonitor fail-fast behavior (returns error code -32050)
//! 2. StubLayerStatusProvider returns honest layer statuses
//! 3. Pipeline breakdown fail-fast behavior (returns error code -32052)
//!
//! ## Test Methodology
//!
//! - Source of Truth: StubSystemMonitor, StubLayerStatusProvider, search handlers
//! - Execute & Inspect: Call handlers and verify error codes/responses
//! - Edge Cases: Empty inputs, malformed params
//!
//! NO mock data, NO fallbacks - verify that stubs FAIL with correct error codes.

use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::johari::{DynDefaultJohariManager, JohariTransitionManager};
use context_graph_core::purpose::GoalHierarchy;
use context_graph_core::stubs::{
    InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor,
};
use context_graph_core::traits::TeleologicalMemoryStore;

use crate::handlers::core::MetaUtlTracker;
use crate::handlers::Handlers;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcRequest};

/// Create test handlers using the DEFAULT constructor (which uses StubSystemMonitor).
///
/// This is the configuration that TASK-EMB-024 requires to fail-fast.
fn create_handlers_with_stub_monitors() -> Handlers {
    let store: Arc<dyn TeleologicalMemoryStore> = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor = Arc::new(StubUtlProcessor::new());
    let multi_array = Arc::new(StubMultiArrayProvider::new());
    let alignment_calc: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = GoalHierarchy::default();

    // NOTE: Handlers::new() uses StubSystemMonitor and StubLayerStatusProvider by default
    Handlers::new(store, utl_processor, multi_array, alignment_calc, goal_hierarchy)
}

/// Create test handlers with shared MetaUtlTracker for direct verification.
#[allow(dead_code)]
fn create_handlers_with_tracker() -> (Handlers, Arc<RwLock<MetaUtlTracker>>) {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor = Arc::new(StubUtlProcessor::new());
    let multi_array = Arc::new(StubMultiArrayProvider::new());
    let alignment_calc: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = Arc::new(RwLock::new(GoalHierarchy::default()));
    let johari_manager: Arc<dyn JohariTransitionManager> =
        Arc::new(DynDefaultJohariManager::new(store.clone()));
    let meta_utl_tracker = Arc::new(RwLock::new(MetaUtlTracker::new()));

    // Uses StubSystemMonitor and StubLayerStatusProvider
    let handlers = Handlers::with_meta_utl_tracker(
        store,
        utl_processor,
        multi_array,
        alignment_calc,
        goal_hierarchy,
        johari_manager,
        meta_utl_tracker.clone(),
    );

    (handlers, meta_utl_tracker)
}

fn make_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(1)),
        method: method.to_string(),
        params: Some(params),
    }
}

fn make_request_no_params(method: &str) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(1)),
        method: method.to_string(),
        params: None,
    }
}

// ============================================================================
// SECTION 1: StubSystemMonitor Fail-Fast Verification
// ============================================================================

/// Test that meta_utl/health_metrics returns SYSTEM_MONITOR_ERROR (-32050)
/// when using StubSystemMonitor.
#[tokio::test]
async fn test_task_emb_024_stub_system_monitor_returns_error_32050() {
    println!("\n======================================================================");
    println!("TASK-EMB-024 VERIFICATION: StubSystemMonitor Fail-Fast Behavior");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call meta_utl/health_metrics - should fail with -32050
    let request = make_request("meta_utl/health_metrics", json!({}));
    let response = handlers.dispatch(request).await;

    // VERIFY: Response should be an error
    println!("RESPONSE INSPECTION:");
    assert!(response.error.is_some(), "Expected error response, got success: {:?}", response.result);

    let error = response.error.unwrap();
    println!("  Error code: {} (expected: -32050 SYSTEM_MONITOR_ERROR)", error.code);
    println!("  Error message: {}", error.message);

    // VERIFY: Error code is -32050
    assert_eq!(
        error.code,
        error_codes::SYSTEM_MONITOR_ERROR,
        "Expected SYSTEM_MONITOR_ERROR (-32050), got {}", error.code
    );

    // VERIFY: Error message contains "Not implemented"
    assert!(
        error.message.to_lowercase().contains("not implemented") ||
        error.message.to_lowercase().contains("failed to get"),
        "Error message should indicate not implemented: {}", error.message
    );

    println!("\n======================================================================");
    println!("EVIDENCE: StubSystemMonitor correctly returns error code -32050");
    println!("  - meta_utl/health_metrics FAILS with SYSTEM_MONITOR_ERROR");
    println!("  - No hardcoded metrics returned");
    println!("======================================================================\n");
}

/// Test that health_metrics fails for coherence_recovery_time_ms specifically.
#[tokio::test]
async fn test_task_emb_024_coherence_recovery_fails_with_stub() {
    println!("\n======================================================================");
    println!("TASK-EMB-024: coherence_recovery_time_ms Fail-Fast");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // The first metric to fail is coherence_recovery_time_ms
    let request = make_request("meta_utl/health_metrics", json!({ "include_targets": true }));
    let response = handlers.dispatch(request).await;

    assert!(response.error.is_some(), "Should fail with error");
    let error = response.error.unwrap();

    // Should fail with SYSTEM_MONITOR_ERROR
    assert_eq!(error.code, error_codes::SYSTEM_MONITOR_ERROR);

    // Error message should mention the metric
    assert!(
        error.message.contains("coherence_recovery") ||
        error.message.contains("Not implemented"),
        "Error should mention the failing metric: {}", error.message
    );

    println!("EVIDENCE: coherence_recovery_time_ms FAILS (no hardcoded 8500 value)");
    println!("  Error: {}", error.message);
}

// ============================================================================
// SECTION 2: StubLayerStatusProvider Verification
// ============================================================================

/// Test that get_memetic_status returns proper layer statuses from StubLayerStatusProvider.
#[tokio::test]
async fn test_task_emb_024_layer_status_provider_honest_statuses() {
    println!("\n======================================================================");
    println!("TASK-EMB-024 VERIFICATION: StubLayerStatusProvider Layer Statuses");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call get_memetic_status via tools/call
    let request = make_request(
        "tools/call",
        json!({
            "name": "get_memetic_status",
            "arguments": {}
        }),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Response should succeed (layer statuses are available)
    assert!(response.error.is_none(), "get_memetic_status should succeed: {:?}", response.error);

    let result = response.result.unwrap();
    // The result is wrapped in MCP tool response format
    let content = result["content"].as_array()
        .and_then(|arr| arr.first())
        .and_then(|obj| obj["text"].as_str())
        .expect("Should have content text");

    let data: serde_json::Value = serde_json::from_str(content)
        .expect("Content should be valid JSON");

    println!("LAYER STATUSES FROM StubLayerStatusProvider:");
    let layers = &data["layers"];
    println!("  perception: {}", layers["perception"]);
    println!("  memory: {}", layers["memory"]);
    println!("  reasoning: {}", layers["reasoning"]);
    println!("  action: {}", layers["action"]);
    println!("  meta: {}", layers["meta"]);

    // VERIFY: Expected statuses per StubLayerStatusProvider
    // Perception and Memory are "active" (have working implementations)
    // Reasoning, Action, Meta are "stub" (not yet implemented)
    assert_eq!(layers["perception"].as_str().unwrap(), "active", "perception should be active");
    assert_eq!(layers["memory"].as_str().unwrap(), "active", "memory should be active");
    assert_eq!(layers["reasoning"].as_str().unwrap(), "stub", "reasoning should be stub");
    assert_eq!(layers["action"].as_str().unwrap(), "stub", "action should be stub");
    assert_eq!(layers["meta"].as_str().unwrap(), "stub", "meta should be stub");

    println!("\n======================================================================");
    println!("EVIDENCE: StubLayerStatusProvider returns honest layer statuses");
    println!("  - perception: active (working implementation)");
    println!("  - memory: active (InMemoryTeleologicalStore works)");
    println!("  - reasoning: stub (not yet implemented - HONEST)");
    println!("  - action: stub (not yet implemented - HONEST)");
    println!("  - meta: stub (not yet implemented - HONEST)");
    println!("======================================================================\n");
}

// ============================================================================
// SECTION 3: Pipeline Breakdown Fail-Fast Verification
// ============================================================================

/// Test that search/multi with include_pipeline_breakdown=true returns
/// PIPELINE_METRICS_UNAVAILABLE (-32052).
#[tokio::test]
async fn test_task_emb_024_pipeline_breakdown_returns_error_32052() {
    println!("\n======================================================================");
    println!("TASK-EMB-024 VERIFICATION: Pipeline Breakdown Fail-Fast");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call search/multi with include_pipeline_breakdown=true
    let request = make_request(
        "search/multi",
        json!({
            "query": "test query for pipeline breakdown verification",
            "include_pipeline_breakdown": true,
            "top_k": 5,
            "minSimilarity": 0.0  // P1-FIX-1: Required parameter (test expects pipeline breakdown error)
        }),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Response should be an error
    println!("RESPONSE INSPECTION:");
    assert!(response.error.is_some(), "Expected error response when include_pipeline_breakdown=true");

    let error = response.error.unwrap();
    println!("  Error code: {} (expected: -32052 PIPELINE_METRICS_UNAVAILABLE)", error.code);
    println!("  Error message: {}", error.message);

    // VERIFY: Error code is -32052
    assert_eq!(
        error.code,
        error_codes::PIPELINE_METRICS_UNAVAILABLE,
        "Expected PIPELINE_METRICS_UNAVAILABLE (-32052), got {}", error.code
    );

    // VERIFY: Error message is descriptive
    assert!(
        error.message.contains("not yet implemented") ||
        error.message.contains("Pipeline breakdown"),
        "Error message should explain the issue: {}", error.message
    );

    println!("\n======================================================================");
    println!("EVIDENCE: Pipeline breakdown correctly returns error code -32052");
    println!("  - search/multi with include_pipeline_breakdown=true FAILS");
    println!("  - No simulated timing data returned");
    println!("======================================================================\n");
}

/// Test that search/multi WITHOUT include_pipeline_breakdown succeeds.
#[tokio::test]
async fn test_task_emb_024_search_multi_without_breakdown_succeeds() {
    println!("\n======================================================================");
    println!("TASK-EMB-024: search/multi Without Pipeline Breakdown Succeeds");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call search/multi without include_pipeline_breakdown (default false)
    let request = make_request(
        "search/multi",
        json!({
            "query": "test query without pipeline breakdown",
            "top_k": 5,
            "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
        }),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Should succeed (no pipeline breakdown requested)
    assert!(response.error.is_none(), "search/multi should succeed without pipeline breakdown: {:?}", response.error);

    let result = response.result.unwrap();
    println!("SUCCESS: search/multi returned {} results", result["count"]);

    // Verify no pipeline_breakdown in response
    assert!(
        result.get("pipeline_breakdown").is_none(),
        "Should not have pipeline_breakdown field"
    );

    println!("EVIDENCE: search/multi works normally when include_pipeline_breakdown=false");
}

// ============================================================================
// SECTION 4: Edge Case Tests
// ============================================================================

/// Edge Case 1: Empty params for health_metrics - should still fail with SYSTEM_MONITOR_ERROR.
#[tokio::test]
async fn test_task_emb_024_edge_case_empty_params() {
    println!("\n======================================================================");
    println!("TASK-EMB-024 EDGE CASE: Empty Params for health_metrics");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call with empty params
    let request = make_request_no_params("meta_utl/health_metrics");
    let response = handlers.dispatch(request).await;

    // Should still fail with SYSTEM_MONITOR_ERROR
    assert!(response.error.is_some(), "Should fail even with empty params");
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::SYSTEM_MONITOR_ERROR);

    println!("EVIDENCE: Empty params still triggers SYSTEM_MONITOR_ERROR (-32050)");
    println!("  - Error: {}", error.message);
}

/// Edge Case 2: Invalid format params - should handle gracefully.
#[tokio::test]
async fn test_task_emb_024_edge_case_invalid_params_format() {
    println!("\n======================================================================");
    println!("TASK-EMB-024 EDGE CASE: Invalid Params Format");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call with malformed params (wrong types)
    let request = make_request(
        "meta_utl/health_metrics",
        json!({
            "include_targets": "not_a_boolean",  // Should be bool
            "include_recommendations": 12345     // Should be bool
        }),
    );
    let response = handlers.dispatch(request).await;

    // The handler should still fail with SYSTEM_MONITOR_ERROR (params are optional)
    // because it reaches the actual monitoring call
    assert!(response.error.is_some(), "Should fail");
    let error = response.error.unwrap();

    // Should be SYSTEM_MONITOR_ERROR because invalid params just default to false
    // and then it hits the actual monitoring code which fails
    println!("Response error code: {}", error.code);
    println!("Response error message: {}", error.message);

    // The error should be either INVALID_PARAMS or SYSTEM_MONITOR_ERROR
    assert!(
        error.code == error_codes::SYSTEM_MONITOR_ERROR ||
        error.code == error_codes::INVALID_PARAMS,
        "Should be SYSTEM_MONITOR_ERROR or INVALID_PARAMS, got {}", error.code
    );

    println!("EVIDENCE: Invalid params handled gracefully (no panic)");
}

/// Edge Case 3: health_metrics with all optional params set.
#[tokio::test]
async fn test_task_emb_024_edge_case_all_optional_params() {
    println!("\n======================================================================");
    println!("TASK-EMB-024 EDGE CASE: All Optional Params");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call with all optional params
    let request = make_request(
        "meta_utl/health_metrics",
        json!({
            "include_targets": true,
            "include_recommendations": true
        }),
    );
    let response = handlers.dispatch(request).await;

    // Should still fail with SYSTEM_MONITOR_ERROR
    assert!(response.error.is_some(), "Should fail with all params set");
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::SYSTEM_MONITOR_ERROR);

    println!("EVIDENCE: All optional params still results in SYSTEM_MONITOR_ERROR");
    println!("  - No hardcoded values returned regardless of params");
}

// ============================================================================
// SECTION 5: No Hardcoded Values Verification
// ============================================================================

/// Verify that no hardcoded metric values appear in responses.
#[tokio::test]
async fn test_task_emb_024_no_hardcoded_values_8500_097_0015() {
    println!("\n======================================================================");
    println!("TASK-EMB-024: No Hardcoded Values (8500, 0.97, 0.015)");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    // Call health_metrics
    let request = make_request("meta_utl/health_metrics", json!({}));
    let response = handlers.dispatch(request).await;

    // Convert entire response to string for inspection
    let response_str = serde_json::to_string(&response).unwrap();

    // VERIFY: No hardcoded values in response
    assert!(!response_str.contains("8500"), "Should not contain hardcoded 8500");
    assert!(!response_str.contains("0.97"), "Should not contain hardcoded 0.97");
    assert!(!response_str.contains("0.015"), "Should not contain hardcoded 0.015");

    // Should be an error response
    assert!(response.error.is_some(), "Should be error, not success with fake values");

    println!("EVIDENCE: No hardcoded values found in response");
    println!("  - '8500' NOT found (old coherence_recovery_time_ms)");
    println!("  - '0.97' NOT found (old attack_detection_rate)");
    println!("  - '0.015' NOT found (old false_positive_rate)");
    println!("  - Response is ERROR (as expected)");
}

/// Verify that layer statuses don't contain hardcoded "stub" strings as
/// the RESULT - only as honest reporting when appropriate.
#[tokio::test]
async fn test_task_emb_024_layer_statuses_are_honest_not_placeholder() {
    println!("\n======================================================================");
    println!("TASK-EMB-024: Layer Statuses Are Honest (Not Placeholder)");
    println!("======================================================================\n");

    let handlers = create_handlers_with_stub_monitors();

    let request = make_request(
        "tools/call",
        json!({
            "name": "get_memetic_status",
            "arguments": {}
        }),
    );
    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    let content = result["content"].as_array()
        .and_then(|arr| arr.first())
        .and_then(|obj| obj["text"].as_str())
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(content).unwrap();

    let layers = &data["layers"];

    // VERIFY: Some layers are "active" (not all "stub")
    // This proves we're getting real status from LayerStatusProvider
    let perception = layers["perception"].as_str().unwrap();
    let memory = layers["memory"].as_str().unwrap();

    assert_eq!(perception, "active", "perception should be active, not placeholder stub");
    assert_eq!(memory, "active", "memory should be active, not placeholder stub");

    // And some ARE stub - which is HONEST reporting
    let reasoning = layers["reasoning"].as_str().unwrap();
    assert_eq!(reasoning, "stub", "reasoning IS stub (honest reporting, not placeholder)");

    println!("EVIDENCE: Layer statuses are honest, not placeholders");
    println!("  - perception='active' (has real implementation)");
    println!("  - memory='active' (InMemoryTeleologicalStore works)");
    println!("  - reasoning='stub' (honest - not implemented yet)");
}
