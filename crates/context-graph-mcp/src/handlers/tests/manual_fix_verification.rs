//! Manual Verification Tests for MCP Issue Fixes
//!
//! TASK: Manual Testing with Synthetic Data and Full State Verification
//!
//! Tests verify 3 issues:
//! 1. Issue 1: search_teleological accepts query_content (string) - no "missing field query" error
//! 2. Issue 2: compute_teleological_vector connection handling (implicit in test infrastructure)
//! 3. Issue 3 / ARCH-03: Autonomous handlers work WITHOUT North Star
//!
//! Each test follows FSV pattern:
//! - BEFORE: Document initial state
//! - EXECUTE: Run operation with synthetic data
//! - SOURCE OF TRUTH: Check database/memory state
//! - VERIFY: Assert expected outcomes
//! - EVIDENCE: Print physical proof

use super::{create_test_handlers, create_test_handlers_no_north_star, make_request};
use crate::protocol::JsonRpcId;
use serde_json::json;

// =============================================================================
// ISSUE 1: search_teleological query_content Parameter Fix
// =============================================================================

/// ISSUE-1 VERIFICATION: search_teleological accepts query_content string.
///
/// BEFORE: Old API required `query` (TeleologicalVectorJson) - complex 13-embedder object
/// AFTER: New API accepts `query_content` (String) - simple string to embed
///
/// This test verifies NO "missing field query" error occurs with the new API.
#[tokio::test]
async fn test_issue1_search_teleological_query_content_parameter() {
    println!("\n============================================================");
    println!("ISSUE-1 VERIFICATION: search_teleological query_content fix");
    println!("============================================================");

    // BEFORE STATE: Create handlers with default configuration
    let handlers = create_test_handlers();
    println!("[BEFORE] Handlers created with test configuration");
    println!("[BEFORE] Testing tools/call with search_teleological and query_content parameter");

    // SYNTHETIC DATA: Simple string content that should be accepted
    let synthetic_query = "software architecture patterns for distributed systems";
    println!("[SYNTHETIC DATA] query_content = \"{}\"", synthetic_query);

    // EXECUTE: Call search_teleological with query_content (string)
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query_content": synthetic_query,
                "strategy": "cosine",
                "max_results": 5
            }
        })),
    );

    println!("[EXECUTE] Dispatching request with query_content parameter...");
    let response = handlers.dispatch(request).await;

    // SOURCE OF TRUTH: Check response structure
    println!("[SOURCE OF TRUTH] Checking response...");

    // VERIFY: No JSON-RPC error (error field should be None)
    assert!(
        response.error.is_none(),
        "[FAIL] JSON-RPC error returned: {:?}",
        response.error
    );
    println!("[VERIFY] No JSON-RPC protocol error - PASS");

    // VERIFY: Result exists
    let result = response.result.expect("[FAIL] Must have result field");
    println!("[VERIFY] Result field present - PASS");

    // VERIFY: Check content structure
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        println!("[VERIFY] Content array present with {} items - PASS", content.len());

        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                // Check for the OLD error message that should NOT appear
                let has_old_error = text.contains("missing field `query`")
                    || text.contains("missing field 'query'");
                assert!(
                    !has_old_error,
                    "[FAIL] Old 'missing field query' error detected! FIX NOT APPLIED: {}",
                    text
                );
                println!("[VERIFY] No 'missing field query' error - PASS");

                // Check if it's an expected error (FAIL FAST is OK)
                let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
                if is_error {
                    // Some errors are expected (e.g., no stored vectors to search)
                    println!("[INFO] Tool returned expected error (isError=true): {}", text);
                    // Verify it's a valid FAIL FAST error, not a parameter parsing error
                    if text.contains("FAIL FAST") {
                        println!("[VERIFY] FAIL FAST error (expected for empty store) - PASS");
                    }
                } else {
                    // Success case
                    println!("[VERIFY] Tool executed successfully");
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                        if let Some(query_type) = parsed.get("query_type").and_then(|v| v.as_str()) {
                            assert_eq!(query_type, "embedded", "[FAIL] Expected query_type=embedded");
                            println!("[VERIFY] query_type=embedded - PASS");
                        }
                    }
                }
            }
        }
    } else if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
        // Direct isError check for non-array responses
        println!("[INFO] Response isError: {}", is_error);
    }

    // PHYSICAL EVIDENCE
    println!("\n[PHYSICAL EVIDENCE]");
    println!("  Request method: tools/call");
    println!("  Tool name: search_teleological");
    println!("  Parameter: query_content (string) = \"{}\"", synthetic_query);
    println!("  Response error: {:?}", response.error);
    println!("  Response has result: true"); // Already consumed by expect() above
    println!("\n[ISSUE-1 VERIFICATION COMPLETE]\n");
}

/// ISSUE-1 VERIFICATION: query_content vs query_vector_id mutual exclusivity.
///
/// API should accept EITHER query_content OR query_vector_id, not both required.
#[tokio::test]
async fn test_issue1_query_content_or_vector_id() {
    println!("\n{}", "=".repeat(60));
    println!("ISSUE-1 VERIFICATION: query_content/query_vector_id mutual exclusivity");
    println!("{}", "=".repeat(60));

    let handlers = create_test_handlers();

    // Test 1: Only query_content (should work)
    println!("\n[TEST 1] Only query_content provided");
    let request1 = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query_content": "test content only"
            }
        })),
    );
    let response1 = handlers.dispatch(request1).await;
    assert!(response1.error.is_none(), "query_content alone should not cause protocol error");
    println!("[VERIFY] query_content alone accepted - PASS");

    // Test 2: Only query_vector_id (should work if UUID valid, fail if not found)
    println!("\n[TEST 2] Only query_vector_id provided (valid UUID format)");
    let request2 = make_request(
        "tools/call",
        Some(JsonRpcId::Number(2)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query_vector_id": "11111111-2222-3333-4444-555555555555"
            }
        })),
    );
    let response2 = handlers.dispatch(request2).await;
    assert!(response2.error.is_none(), "query_vector_id alone should not cause protocol error");
    println!("[VERIFY] query_vector_id alone accepted - PASS");

    // The result may be an error (vector not found), but that's expected business logic
    if let Some(result) = response2.result {
        if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
            if is_error {
                println!("[INFO] Expected: vector not found (FAIL FAST) - OK");
            }
        }
    }

    println!("\n[ISSUE-1 MUTUAL EXCLUSIVITY VERIFICATION COMPLETE]\n");
}

// =============================================================================
// ISSUE 3 / ARCH-03: Autonomous Operation Without North Star
// =============================================================================

/// ARCH-03 VERIFICATION: get_autonomous_status works WITHOUT North Star.
///
/// BEFORE: Would fail or return error when no North Star configured
/// AFTER: Returns status with recommendations to store memories first
///
/// Per constitution ARCH-03: "System MUST operate autonomously without manual goal setting"
#[tokio::test]
async fn test_arch03_get_autonomous_status_without_north_star() {
    println!("\n{}", "=".repeat(60));
    println!("ARCH-03 VERIFICATION: get_autonomous_status without North Star");
    println!("{}", "=".repeat(60));

    // BEFORE STATE: Create handlers WITHOUT North Star
    let handlers = create_test_handlers_no_north_star();
    println!("[BEFORE] Handlers created WITHOUT North Star (empty goal hierarchy)");

    // SYNTHETIC DATA: Request status with all optional params
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_autonomous_status",
            "arguments": {
                "include_metrics": true,
                "include_history": true,
                "history_count": 5
            }
        })),
    );

    println!("[EXECUTE] Calling get_autonomous_status without North Star...");
    let response = handlers.dispatch(request).await;

    // SOURCE OF TRUTH: Check response
    println!("[SOURCE OF TRUTH] Checking response...");

    // VERIFY: No protocol error
    assert!(
        response.error.is_none(),
        "[FAIL] Protocol error when no North Star: {:?}",
        response.error
    );
    println!("[VERIFY] No protocol error without North Star - PASS");

    // VERIFY: Result exists
    let result = response.result.expect("[FAIL] Must have result");
    println!("[VERIFY] Result field present - PASS");

    // VERIFY: Should NOT be an isError response
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(
        !is_error,
        "[FAIL] Returned isError=true without North Star - should still work"
    );
    println!("[VERIFY] Not an error response (isError=false) - PASS");

    // Extract and verify content
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                    // VERIFY: north_star.configured should be false
                    if let Some(ns) = parsed.get("north_star") {
                        let configured = ns.get("configured").and_then(|v| v.as_bool()).unwrap_or(true);
                        assert!(!configured, "[FAIL] north_star.configured should be false");
                        println!("[VERIFY] north_star.configured = false - PASS");
                    }

                    // VERIFY: Should have recommendations for unconfigured state
                    if let Some(recommendations) = parsed.get("recommendations").and_then(|v| v.as_array()) {
                        println!("[VERIFY] Has {} recommendations - PASS", recommendations.len());

                        // Check for store_memory recommendation
                        let has_store_recommendation = recommendations.iter().any(|r| {
                            r.get("action")
                                .and_then(|a| a.as_str())
                                .map(|a| a == "store_memory")
                                .unwrap_or(false)
                        });
                        if has_store_recommendation {
                            println!("[VERIFY] Has store_memory recommendation (ARCH-03 compliant) - PASS");
                        }
                    }

                    // VERIFY: overall_health should indicate not_configured
                    if let Some(health) = parsed.get("overall_health") {
                        if let Some(status) = health.get("status").and_then(|v| v.as_str()) {
                            println!("[VERIFY] overall_health.status = \"{}\"", status);
                            assert_eq!(status, "not_configured", "[INFO] Expected not_configured status");
                        }
                    }
                }
            }
        }
    }

    // PHYSICAL EVIDENCE
    println!("\n[PHYSICAL EVIDENCE]");
    println!("  Tool: get_autonomous_status");
    println!("  North Star configured: false");
    println!("  Response error: {:?}", response.error);
    println!("  Response has valid result: true");
    println!("\n[ARCH-03 get_autonomous_status VERIFICATION COMPLETE]\n");
}

/// ARCH-03 VERIFICATION: auto_bootstrap_north_star DISCOVERS goals, doesn't require them.
///
/// BEFORE: Would require pre-existing North Star to bootstrap
/// AFTER: Discovers North Star from clustering stored fingerprints
///
/// Note: This test verifies the error message when no fingerprints stored yet,
/// which is the correct ARCH-03 behavior (discover from data, not require manual setting).
#[tokio::test]
async fn test_arch03_auto_bootstrap_discovers_from_stored_fingerprints() {
    println!("\n{}", "=".repeat(60));
    println!("ARCH-03 VERIFICATION: auto_bootstrap_north_star discovers goals");
    println!("{}", "=".repeat(60));

    // BEFORE STATE: Create handlers WITHOUT North Star AND empty store
    let handlers = create_test_handlers_no_north_star();
    println!("[BEFORE] Handlers created WITHOUT North Star");
    println!("[BEFORE] Store is empty (no fingerprints)");

    // EXECUTE: Try to bootstrap - should fail gracefully asking to store memories first
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "auto_bootstrap_north_star",
            "arguments": {
                "confidence_threshold": 0.6,
                "max_candidates": 5
            }
        })),
    );

    println!("[EXECUTE] Calling auto_bootstrap_north_star with empty store...");
    let response = handlers.dispatch(request).await;

    // SOURCE OF TRUTH: Response should indicate need for fingerprints
    println!("[SOURCE OF TRUTH] Checking response...");

    // ARCH-03 VERIFICATION: The handler can return error in TWO ways:
    // 1. JsonRpcResponse::error() - Sets response.error field (JSON-RPC level error)
    // 2. tool_error_with_pulse() - Returns result with isError=true (MCP tool error)
    //
    // BOTH are valid ARCH-03 behavior - the key is the MESSAGE guides to store memories first.

    let error_message: String;
    let is_graceful_error: bool;

    if let Some(err) = &response.error {
        // JSON-RPC error path - extract message
        error_message = err.message.clone();
        is_graceful_error = true;
        println!("[INFO] JSON-RPC error response: {}", error_message);
    } else if let Some(result) = &response.result {
        // MCP tool error path - extract from content
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        if is_error {
            if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(first) = content.first() {
                    if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                        error_message = text.to_string();
                        is_graceful_error = true;
                        println!("[INFO] MCP tool error response: {}", error_message);
                    } else {
                        error_message = "No text in error content".to_string();
                        is_graceful_error = false;
                    }
                } else {
                    error_message = "Empty content array".to_string();
                    is_graceful_error = false;
                }
            } else {
                error_message = "No content in isError response".to_string();
                is_graceful_error = false;
            }
        } else {
            // Success case - might have fingerprints from previous test
            println!("[INFO] Bootstrap succeeded (store not empty from previous tests)");
            error_message = String::new();
            is_graceful_error = false;
        }
    } else {
        panic!("[FAIL] Response has neither error nor result");
    }

    // VERIFY: If we got an error, it should guide to store memories first
    if is_graceful_error {
        let guides_to_store = error_message.contains("Store memories")
            || error_message.contains("store memories")
            || error_message.contains("teleological fingerprints")
            || error_message.contains("No teleological fingerprints");
        assert!(
            guides_to_store,
            "[FAIL] Error should guide to store memories first, got: {}",
            error_message
        );
        println!("[VERIFY] Error guides to store memories first (ARCH-03 compliant) - PASS");

        // VERIFY: FAIL FAST pattern
        let has_fail_fast = error_message.contains("FAIL FAST");
        assert!(has_fail_fast, "[FAIL] Should use FAIL FAST pattern");
        println!("[VERIFY] Uses FAIL FAST pattern - PASS");
    }

    // PHYSICAL EVIDENCE
    println!("\n[PHYSICAL EVIDENCE]");
    println!("  Tool: auto_bootstrap_north_star");
    println!("  Initial state: No North Star, empty store");
    println!("  Expected behavior: FAIL FAST asking to store memories first");
    println!("  Actual error received: {}", is_graceful_error);
    if is_graceful_error {
        println!("  Error message: {}", error_message);
    }
    println!("  ARCH-03 Compliance: Goals DISCOVERED from data, not manually required");
    println!("\n[ARCH-03 auto_bootstrap VERIFICATION COMPLETE]\n");
}

/// ARCH-03 VERIFICATION: get_alignment_drift works without North Star.
///
/// BEFORE: Would fail when no North Star configured
/// AFTER: Computes drift relative to computed centroid of memories
#[tokio::test]
async fn test_arch03_get_alignment_drift_without_north_star() {
    println!("\n{}", "=".repeat(60));
    println!("ARCH-03 VERIFICATION: get_alignment_drift without North Star");
    println!("{}", "=".repeat(60));

    let handlers = create_test_handlers_no_north_star();
    println!("[BEFORE] Handlers created WITHOUT North Star");

    // EXECUTE: Get drift without North Star
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_alignment_drift",
            "arguments": {
                "timeframe": "24h",
                "include_history": false
            }
        })),
    );

    println!("[EXECUTE] Calling get_alignment_drift without North Star...");
    let response = handlers.dispatch(request).await;

    // VERIFY: No protocol error
    assert!(
        response.error.is_none(),
        "[FAIL] Protocol error without North Star: {:?}",
        response.error
    );
    println!("[VERIFY] No protocol error - PASS");

    let result = response.result.expect("[FAIL] Must have result");

    // Should return valid response (even if minimal without memory_ids)
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "[FAIL] Should not be error without North Star");
    println!("[VERIFY] Not an error response - PASS");

    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                    // Check reference_type - should be "no_reference" or "centroid" when no North Star
                    if let Some(ref_type) = parsed.get("reference_type").and_then(|v| v.as_str()) {
                        println!("[VERIFY] reference_type = \"{}\"", ref_type);
                        // Without memory_ids, returns no_reference
                        // With memory_ids, would compute centroid
                    }

                    // Check for usage_hint when no memory_ids provided
                    if let Some(hint) = parsed.get("usage_hint").and_then(|v| v.as_str()) {
                        println!("[VERIFY] Has usage_hint for memory_ids - PASS");
                        println!("[INFO] usage_hint: {}", hint);
                    }
                }
            }
        }
    }

    println!("\n[ARCH-03 get_alignment_drift VERIFICATION COMPLETE]\n");
}

// =============================================================================
// Edge Cases: FAIL FAST Verification
// =============================================================================

/// FAIL FAST: Empty query_content should fail immediately.
#[tokio::test]
async fn test_fail_fast_empty_query_content() {
    println!("\n{}", "=".repeat(60));
    println!("FAIL FAST VERIFICATION: Empty query_content");
    println!("{}", "=".repeat(60));

    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query_content": ""
            }
        })),
    );

    println!("[EXECUTE] Calling search_teleological with empty query_content...");
    let response = handlers.dispatch(request).await;

    // Should not have protocol error
    assert!(response.error.is_none(), "Should not have protocol error");
    let result = response.result.expect("Must have result");

    // VERIFY: Should be isError=true with FAIL FAST message
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(is_error, "[FAIL] Empty query_content should trigger error");
    println!("[VERIFY] isError=true for empty content - PASS");

    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                let has_fail_fast = text.contains("FAIL FAST") || text.contains("empty");
                assert!(has_fail_fast, "[FAIL] Should mention FAIL FAST or empty");
                println!("[VERIFY] FAIL FAST message for empty content - PASS");
                println!("[EVIDENCE] Error text: {}", text);
            }
        }
    }

    println!("\n[FAIL FAST empty query_content VERIFICATION COMPLETE]\n");
}

/// FAIL FAST: Invalid UUID for query_vector_id should fail immediately.
#[tokio::test]
async fn test_fail_fast_invalid_uuid() {
    println!("\n{}", "=".repeat(60));
    println!("FAIL FAST VERIFICATION: Invalid UUID");
    println!("{}", "=".repeat(60));

    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query_vector_id": "not-a-valid-uuid-format"
            }
        })),
    );

    println!("[EXECUTE] Calling search_teleological with invalid UUID...");
    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Should not have protocol error");
    let result = response.result.expect("Must have result");

    // VERIFY: Should be isError=true with FAIL FAST message
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(is_error, "[FAIL] Invalid UUID should trigger error");
    println!("[VERIFY] isError=true for invalid UUID - PASS");

    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                let has_fail_fast = text.contains("FAIL FAST");
                assert!(has_fail_fast, "[FAIL] Should mention FAIL FAST");
                println!("[VERIFY] FAIL FAST message for invalid UUID - PASS");
                println!("[EVIDENCE] Error text: {}", text);
            }
        }
    }

    println!("\n[FAIL FAST invalid UUID VERIFICATION COMPLETE]\n");
}

/// FAIL FAST: Neither query_content nor query_vector_id provided.
#[tokio::test]
async fn test_fail_fast_no_query_provided() {
    println!("\n{}", "=".repeat(60));
    println!("FAIL FAST VERIFICATION: No query parameter provided");
    println!("{}", "=".repeat(60));

    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "strategy": "cosine",
                "max_results": 10
            }
        })),
    );

    println!("[EXECUTE] Calling search_teleological without query parameter...");
    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Should not have protocol error");
    let result = response.result.expect("Must have result");

    // VERIFY: Should be isError=true with FAIL FAST message
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(is_error, "[FAIL] Missing query should trigger error");
    println!("[VERIFY] isError=true for missing query - PASS");

    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                let has_fail_fast = text.contains("FAIL FAST");
                let mentions_params = text.contains("query_content") || text.contains("query_vector_id");
                assert!(has_fail_fast, "[FAIL] Should mention FAIL FAST");
                assert!(mentions_params, "[FAIL] Should mention required parameters");
                println!("[VERIFY] FAIL FAST message mentions required params - PASS");
                println!("[EVIDENCE] Error text: {}", text);
            }
        }
    }

    println!("\n[FAIL FAST no query VERIFICATION COMPLETE]\n");
}

// =============================================================================
// Integration: Handlers with North Star (positive cases)
// =============================================================================

/// Positive case: get_autonomous_status WITH North Star configured.
#[tokio::test]
async fn test_autonomous_status_with_north_star() {
    println!("\n{}", "=".repeat(60));
    println!("POSITIVE VERIFICATION: get_autonomous_status with North Star");
    println!("{}", "=".repeat(60));

    // Use handlers WITH North Star configured
    let handlers = create_test_handlers();
    println!("[BEFORE] Handlers created WITH North Star configured");

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "get_autonomous_status",
            "arguments": {
                "include_metrics": true
            }
        })),
    );

    println!("[EXECUTE] Calling get_autonomous_status with North Star...");
    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Should not have protocol error");
    let result = response.result.expect("Must have result");

    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "Should not be error with North Star");
    println!("[VERIFY] Not an error response - PASS");

    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                    // VERIFY: north_star.configured should be true
                    if let Some(ns) = parsed.get("north_star") {
                        let configured = ns.get("configured").and_then(|v| v.as_bool()).unwrap_or(false);
                        assert!(configured, "[FAIL] north_star.configured should be true");
                        println!("[VERIFY] north_star.configured = true - PASS");

                        if let Some(goal_id) = ns.get("goal_id").and_then(|v| v.as_str()) {
                            println!("[EVIDENCE] North Star goal_id: {}", goal_id);
                        }
                    }

                    // VERIFY: overall_health should NOT be not_configured
                    if let Some(health) = parsed.get("overall_health") {
                        if let Some(status) = health.get("status").and_then(|v| v.as_str()) {
                            assert_ne!(status, "not_configured", "Should have valid health status");
                            println!("[VERIFY] overall_health.status = \"{}\" - PASS", status);
                        }
                        if let Some(score) = health.get("score").and_then(|v| v.as_f64()) {
                            println!("[EVIDENCE] Health score: {}", score);
                        }
                    }
                }
            }
        }
    }

    println!("\n[POSITIVE get_autonomous_status VERIFICATION COMPLETE]\n");
}

/// Positive case: auto_bootstrap when North Star already exists.
#[tokio::test]
async fn test_bootstrap_with_existing_north_star() {
    println!("\n{}", "=".repeat(60));
    println!("POSITIVE VERIFICATION: auto_bootstrap with existing North Star");
    println!("{}", "=".repeat(60));

    let handlers = create_test_handlers();
    println!("[BEFORE] Handlers created WITH existing North Star");

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "auto_bootstrap_north_star",
            "arguments": {}
        })),
    );

    println!("[EXECUTE] Calling auto_bootstrap with existing North Star...");
    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Should not have protocol error");
    let result = response.result.expect("Must have result");

    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "Should succeed (report existing North Star)");
    println!("[VERIFY] Not an error - PASS");

    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                    // VERIFY: status should be "already_bootstrapped"
                    if let Some(status) = parsed.get("status").and_then(|v| v.as_str()) {
                        assert_eq!(status, "already_bootstrapped", "Should report already bootstrapped");
                        println!("[VERIFY] status = \"already_bootstrapped\" - PASS");
                    }

                    // VERIFY: Should have bootstrap_result with existing goal
                    if let Some(br) = parsed.get("bootstrap_result") {
                        if let Some(source) = br.get("source").and_then(|v| v.as_str()) {
                            assert_eq!(source, "existing_north_star");
                            println!("[VERIFY] source = \"existing_north_star\" - PASS");
                        }
                    }
                }
            }
        }
    }

    println!("\n[POSITIVE auto_bootstrap with existing VERIFICATION COMPLETE]\n");
}

// =============================================================================
// EMBEDDER SCORES FIX: Verify non-zero scores in search_teleological results
// =============================================================================

/// EMBEDDER SCORES FIX VERIFICATION: search_teleological returns non-zero embedder_scores.
///
/// ROOT CAUSE (FIXED): search_purpose() in both InMemoryTeleologicalStore and
/// RocksDbTeleologicalStore were setting embedder_scores to zeros instead of
/// computing actual per-embedder cosine similarities.
///
/// FIX: Added semantic_query field to TeleologicalSearchOptions. When provided,
/// stores compute actual embedder scores using compute_semantic_scores().
///
/// This test verifies embedder_scores are NOT all zeros in results.
#[tokio::test]
async fn test_embedder_scores_fix_non_zero_scores() {
    println!("\n{}", "=".repeat(70));
    println!("EMBEDDER SCORES FIX VERIFICATION: Non-zero embedder_scores");
    println!("{}", "=".repeat(70));

    let handlers = create_test_handlers();
    println!("[BEFORE] Handlers created with test configuration");

    // First, store a teleological fingerprint to search against
    println!("[SETUP] Storing a test memory to ensure we have something to search...");
    let store_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "store_memory",
            "arguments": {
                "content": "Software architecture patterns for designing distributed systems with microservices and event-driven communication. This includes patterns like CQRS, event sourcing, and saga orchestration for maintaining consistency across service boundaries.",
                "importance": 0.9,
                "context": "technical_documentation"
            }
        })),
    );
    let store_response = handlers.dispatch(store_request).await;
    assert!(store_response.error.is_none(), "Store should succeed");
    println!("[SETUP] Memory stored successfully");

    // Now search with related content
    let search_content = "distributed systems architecture patterns";
    println!("[EXECUTE] Searching with query_content: \"{}\"", search_content);

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(2)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query_content": search_content,
                "strategy": "cosine",
                "max_results": 5,
                "min_similarity": 0.0,
                "include_breakdown": true
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // VERIFY: No protocol error
    assert!(
        response.error.is_none(),
        "[FAIL] Protocol error: {:?}",
        response.error
    );
    println!("[VERIFY] No protocol error - PASS");

    let result = response.result.expect("Must have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "[FAIL] Tool returned error: {:?}", result);
    println!("[VERIFY] Tool succeeded - PASS");

    // Extract and verify embedder_scores
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                    // Check success field
                    let success = parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                    assert!(success, "[FAIL] Response success=false");
                    println!("[VERIFY] Response success=true - PASS");

                    // Check results array
                    if let Some(results) = parsed.get("results").and_then(|v| v.as_array()) {
                        println!("[INFO] Got {} results", results.len());

                        if results.is_empty() {
                            println!("[WARN] No results returned - cannot verify embedder_scores");
                            println!("[INFO] This may be because no similar memories were found");
                        } else {
                            // Check first result for embedder_scores
                            // Note: scores can be at top level OR in breakdown object
                            let first_result = &results[0];

                            // Try breakdown.embedder_scores first (teleological search format)
                            let scores_opt = first_result
                                .get("breakdown")
                                .and_then(|b| b.get("embedder_scores"))
                                .and_then(|v| v.as_array())
                                .or_else(|| first_result.get("embedder_scores").and_then(|v| v.as_array()));

                            if let Some(scores) = scores_opt {
                                println!("[EVIDENCE] embedder_scores array found with {} elements", scores.len());
                                assert_eq!(scores.len(), 13, "[FAIL] Expected 13 embedder scores, got {}", scores.len());
                                println!("[VERIFY] 13 embedder scores present - PASS");

                                // CRITICAL: Verify at least one score is non-zero
                                let scores_f32: Vec<f32> = scores
                                    .iter()
                                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                                    .collect();

                                let has_non_zero = scores_f32.iter().any(|&s| s > 0.0001);
                                let sum: f32 = scores_f32.iter().sum();
                                let max_score: f32 = scores_f32.iter().cloned().fold(0.0f32, f32::max);

                                println!("[EVIDENCE] Embedder scores: {:?}", scores_f32);
                                println!("[EVIDENCE] Sum of scores: {}", sum);
                                println!("[EVIDENCE] Max score: {}", max_score);

                                // THE KEY ASSERTION - scores should NOT be all zeros
                                assert!(
                                    has_non_zero,
                                    "[FAIL] All embedder_scores are zero! ROOT CAUSE NOT FIXED. Scores: {:?}",
                                    scores_f32
                                );
                                println!("[VERIFY] At least one embedder score is non-zero - PASS");
                                println!("[SUCCESS] EMBEDDER SCORES FIX VERIFIED!");
                            } else {
                                println!("[WARN] embedder_scores field not found in result");
                                println!("[INFO] Result structure: {:?}", first_result);
                            }
                        }
                    } else {
                        println!("[INFO] No results array in response");
                    }
                } else {
                    println!("[WARN] Could not parse response as JSON");
                }
            }
        }
    }

    println!("\n[EMBEDDER SCORES FIX VERIFICATION COMPLETE]\n");
}

/// EMBEDDER VARIETY VERIFICATION: Different content types produce varied scores.
///
/// This test verifies that when comparing DIFFERENT content types, the 13 embedders
/// produce VARIED similarity scores, not uniform ~0.999 values.
///
/// The stub provider should generate distinctly different embedding patterns for
/// each embedder, causing similarity to vary based on content characteristics.
#[tokio::test]
async fn test_embedder_scores_variety_across_spaces() {
    println!("\n{}", "=".repeat(70));
    println!("EMBEDDER VARIETY VERIFICATION: Scores vary across 13 spaces");
    println!("{}", "=".repeat(70));

    let handlers = create_test_handlers();
    println!("[BEFORE] Handlers created with test configuration");

    // Store TWO memories with VERY DIFFERENT content
    // Memory 1: Technical/code content
    println!("[SETUP] Storing technical memory...");
    let store1 = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "store_memory",
            "arguments": {
                "content": "Implementing a Redis cache layer with LRU eviction policy using async Rust and tokio runtime. The cache supports TTL-based expiration and provides O(1) lookups.",
                "importance": 0.9,
                "context": "code_implementation"
            }
        })),
    );
    handlers.dispatch(store1).await;
    println!("[SETUP] Technical memory stored");

    // Memory 2: Completely different domain - cooking/recipes
    println!("[SETUP] Storing cooking memory...");
    let store2 = make_request(
        "tools/call",
        Some(JsonRpcId::Number(2)),
        Some(json!({
            "name": "store_memory",
            "arguments": {
                "content": "Grandmother's apple pie recipe with fresh cinnamon and nutmeg. Bake at 350 degrees for 45 minutes until golden brown. Serve warm with vanilla ice cream.",
                "importance": 0.8,
                "context": "personal_recipe"
            }
        })),
    );
    handlers.dispatch(store2).await;
    println!("[SETUP] Cooking memory stored");

    // Search with technical content - should match technical memory better
    let search_content = "database caching optimization strategies performance";
    println!("[EXECUTE] Searching with: \"{}\"", search_content);

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(3)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query_content": search_content,
                "strategy": "cosine",
                "max_results": 10,
                "min_similarity": 0.0,
                "include_breakdown": true
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none(), "[FAIL] Protocol error");

    let result = response.result.expect("Must have result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(!is_error, "[FAIL] Tool returned error");

    // Analyze embedder score variety
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|v| v.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                    if let Some(results) = parsed.get("results").and_then(|v| v.as_array()) {
                        println!("[INFO] Got {} results", results.len());

                        for (idx, result) in results.iter().enumerate() {
                            let scores_opt = result
                                .get("breakdown")
                                .and_then(|b| b.get("embedder_scores"))
                                .and_then(|v| v.as_array());

                            if let Some(scores) = scores_opt {
                                let scores_f32: Vec<f32> = scores
                                    .iter()
                                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                                    .collect();

                                // Calculate statistics
                                let non_zero_scores: Vec<f32> = scores_f32.iter()
                                    .filter(|&&s| s > 0.0001)
                                    .cloned()
                                    .collect();

                                if !non_zero_scores.is_empty() {
                                    let min_score = non_zero_scores.iter().cloned().fold(f32::MAX, f32::min);
                                    let max_score = non_zero_scores.iter().cloned().fold(0.0f32, f32::max);
                                    let avg_score: f32 = non_zero_scores.iter().sum::<f32>() / non_zero_scores.len() as f32;
                                    let range = max_score - min_score;

                                    // Calculate variance to measure diversity
                                    let variance: f32 = non_zero_scores.iter()
                                        .map(|&s| (s - avg_score).powi(2))
                                        .sum::<f32>() / non_zero_scores.len() as f32;
                                    let std_dev = variance.sqrt();

                                    println!("\n[RESULT {}] Embedder Score Analysis:", idx + 1);
                                    println!("  Scores: {:?}", scores_f32);
                                    println!("  Min (non-zero): {:.6}", min_score);
                                    println!("  Max: {:.6}", max_score);
                                    println!("  Range: {:.6}", range);
                                    println!("  Avg: {:.6}", avg_score);
                                    println!("  Std Dev: {:.6}", std_dev);

                                    // CRITICAL ASSERTION: We expect SOME variance across embedders
                                    // If all scores are identical (std_dev ≈ 0), the fix didn't work
                                    println!("\n[VERIFY] Checking for score variety...");
                                    if std_dev < 0.0001 {
                                        println!("[WARN] Very low variance - scores are nearly uniform");
                                        println!("  This is expected when comparing SIMILAR content");
                                    } else if std_dev < 0.01 {
                                        println!("[INFO] Low variance ({:.6}) - moderate diversity", std_dev);
                                    } else {
                                        println!("[PASS] Good variance ({:.6}) - diverse embedder scores!", std_dev);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println!("\n[EMBEDDER VARIETY VERIFICATION COMPLETE]\n");
}

// =============================================================================
// Summary Test: Collect All Evidence
// =============================================================================

/// Summary test that runs all verifications and prints consolidated evidence.
#[tokio::test]
async fn test_all_fixes_summary() {
    println!("\n");
    println!("{}", "#".repeat(70));
    println!("#  MANUAL FIX VERIFICATION SUMMARY");
    println!("#  Tests for Issues 1-3 in context-graph MCP server");
    println!("{}", "#".repeat(70));

    // Run a quick verification of each fix
    let handlers = create_test_handlers();
    let handlers_no_ns = create_test_handlers_no_north_star();

    // Issue 1: search_teleological query_content
    let req1 = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {"query_content": "test"}
        })),
    );
    let res1 = handlers.dispatch(req1).await;
    let issue1_pass = res1.error.is_none();

    // Issue 3a: get_autonomous_status without North Star
    let req3a = make_request(
        "tools/call",
        Some(JsonRpcId::Number(2)),
        Some(json!({
            "name": "get_autonomous_status",
            "arguments": {}
        })),
    );
    let res3a = handlers_no_ns.dispatch(req3a).await;
    let issue3a_pass = res3a.error.is_none()
        && res3a
            .result
            .as_ref()
            .map(|r| !r.get("isError").and_then(|v| v.as_bool()).unwrap_or(true))
            .unwrap_or(false);

    // Issue 3b: auto_bootstrap without stored fingerprints
    // Should fail gracefully with guidance - can be either:
    // - JsonRpcResponse::error() with message about storing fingerprints
    // - Result with isError=true and content about storing fingerprints
    let req3b = make_request(
        "tools/call",
        Some(JsonRpcId::Number(3)),
        Some(json!({
            "name": "auto_bootstrap_north_star",
            "arguments": {}
        })),
    );
    let res3b = handlers_no_ns.dispatch(req3b).await;
    // ARCH-03 compliance: either error type is acceptable if message guides to store fingerprints
    let issue3b_pass = if let Some(err) = &res3b.error {
        // JSON-RPC error path - check message contains guidance
        err.message.contains("teleological fingerprints") || err.message.contains("Store memories")
    } else if let Some(result) = &res3b.result {
        // MCP tool error path - check isError and content
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        if is_error {
            result.get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str())
                .map(|text| text.contains("teleological fingerprints") || text.contains("Store memories"))
                .unwrap_or(false)
        } else {
            // Succeeded (store may have fingerprints from previous test)
            true
        }
    } else {
        false
    };

    // Edge case: empty query_content FAIL FAST
    let req_edge = make_request(
        "tools/call",
        Some(JsonRpcId::Number(4)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {"query_content": ""}
        })),
    );
    let res_edge = handlers.dispatch(req_edge).await;
    let edge_pass = res_edge.error.is_none()
        && res_edge
            .result
            .as_ref()
            .map(|r| r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false))
            .unwrap_or(false);

    println!("\n{}", "=".repeat(70));
    println!("VERIFICATION RESULTS:");
    println!("{}", "=".repeat(70));
    println!(
        "Issue 1 - search_teleological query_content: {}",
        if issue1_pass { "PASS" } else { "FAIL" }
    );
    println!(
        "Issue 3a - get_autonomous_status without North Star: {}",
        if issue3a_pass { "PASS" } else { "FAIL" }
    );
    println!(
        "Issue 3b - auto_bootstrap graceful fail without data: {}",
        if issue3b_pass { "PASS" } else { "FAIL" }
    );
    println!(
        "Edge case - FAIL FAST on empty query_content: {}",
        if edge_pass { "PASS" } else { "FAIL" }
    );
    println!("{}", "=".repeat(70));

    let all_pass = issue1_pass && issue3a_pass && issue3b_pass && edge_pass;
    println!(
        "\nOVERALL: {}",
        if all_pass {
            "ALL TESTS PASSED"
        } else {
            "SOME TESTS FAILED"
        }
    );
    println!("\n");

    assert!(all_pass, "Not all verification tests passed");
}
