//! Topic Tools Full State Verification Tests
//!
//! TASK-GAP-006: Tests that verify topic tools actually work with real storage
//! and verify state changes in the underlying database.
//!
//! These tests use RocksDB storage to ensure topic tools:
//! 1. Actually read from the database
//! 2. Return correct tier based on memory count
//! 3. Properly enforce minimum memory requirements
//!
//! Per task requirements:
//! - No mock data - uses real RocksDB storage
//! - Manual verification of database state
//! - Edge case testing (empty, boundary conditions)

use serde_json::json;

use crate::protocol::JsonRpcId;

use super::{create_test_handlers_with_rocksdb_store_access, extract_mcp_tool_data, make_request};

// ============================================================================
// Full State Verification Tests
// ============================================================================

/// FSV Test: Verify get_topic_portfolio returns tier 0 for empty database
///
/// Source of Truth: TeleologicalMemoryStore.count()
/// Edge Case: Empty database
#[tokio::test]
async fn test_fsv_topic_portfolio_empty_database() {
    println!("\n=== FSV Test: get_topic_portfolio with empty database ===");

    // Setup: Create handlers with real RocksDB storage
    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // PRE-CONDITION: Verify database is empty
    let count_before = store.count().await.expect("count() must work");
    println!("PRE-CONDITION: Memory count = {}", count_before);
    assert_eq!(count_before, 0, "Database must be empty at start of test");

    // EXECUTE: Call get_topic_portfolio
    let params = json!({
        "name": "get_topic_portfolio",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;

    // VERIFY RESPONSE
    assert!(
        response.error.is_none(),
        "get_topic_portfolio should not return JSON-RPC error"
    );
    let result = response.result.expect("Must have result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "Tool should succeed");

    let data = extract_mcp_tool_data(&result);
    let tier = data.get("tier").unwrap().as_u64().unwrap();
    let total_topics = data.get("total_topics").unwrap().as_u64().unwrap();

    println!("RESPONSE: tier={}, total_topics={}", tier, total_topics);

    // VERIFY STATE
    assert_eq!(tier, 0, "Tier must be 0 for empty database");
    assert_eq!(total_topics, 0, "Total topics must be 0 for empty database");

    // POST-CONDITION: Verify database unchanged
    let count_after = store.count().await.expect("count() must work");
    println!("POST-CONDITION: Memory count = {}", count_after);
    assert_eq!(count_after, 0, "Database must still be empty");

    println!("[FSV PASS] get_topic_portfolio returns tier 0 for empty database");
}

/// FSV Test: Verify detect_topics returns INSUFFICIENT_MEMORIES with 0 memories
///
/// Source of Truth: TeleologicalMemoryStore.count()
/// Edge Case: Minimum boundary (requires 3)
#[tokio::test]
async fn test_fsv_detect_topics_insufficient_memories() {
    use crate::protocol::error_codes;

    println!("\n=== FSV Test: detect_topics with insufficient memories ===");

    // Setup
    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // PRE-CONDITION: Verify database is empty
    let count = store.count().await.expect("count() must work");
    println!("PRE-CONDITION: Memory count = {} (min required = 3)", count);
    assert!(count < 3, "Must have fewer than 3 memories");

    // EXECUTE: Call detect_topics
    let params = json!({
        "name": "detect_topics",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;

    // VERIFY ERROR RESPONSE
    assert!(
        response.error.is_some(),
        "detect_topics must return JSON-RPC error for insufficient memories"
    );

    let error = response.error.as_ref().unwrap();
    println!("ERROR: code={}, message={}", error.code, error.message);

    assert_eq!(
        error.code,
        error_codes::INSUFFICIENT_MEMORIES,
        "Error code must be INSUFFICIENT_MEMORIES (-32021)"
    );
    assert!(
        error.message.contains("3"),
        "Error message must mention minimum of 3 memories"
    );
    assert!(
        error.message.contains("0") || error.message.contains(&count.to_string()),
        "Error message must mention current count"
    );

    println!("[FSV PASS] detect_topics returns INSUFFICIENT_MEMORIES with < 3 memories");
}

/// FSV Test: Verify get_topic_stability returns valid metrics
///
/// Source of Truth: Response structure contains all required fields
/// Edge Case: Default hours (should use 1)
#[tokio::test]
async fn test_fsv_topic_stability_default_parameters() {
    println!("\n=== FSV Test: get_topic_stability with default parameters ===");

    // Setup
    let (handlers, _, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // EXECUTE: Call get_topic_stability with no arguments
    let params = json!({
        "name": "get_topic_stability",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;

    // VERIFY RESPONSE STRUCTURE
    assert!(response.error.is_none(), "Should not return error");
    let result = response.result.expect("Must have result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "Tool should succeed");

    let data = extract_mcp_tool_data(&result);

    // VERIFY ALL REQUIRED FIELDS ARE PRESENT
    let required_fields = [
        "churn_rate",
        "entropy",
        "phases",
        "high_churn_warning",
        "average_churn",
    ];

    for field in required_fields {
        assert!(
            data.get(field).is_some(),
            "Response must contain '{}' field",
            field
        );
        println!("FIELD '{}': {:?}", field, data.get(field).unwrap());
    }

    // VERIFY VALUE RANGES
    let churn_rate = data.get("churn_rate").unwrap().as_f64().unwrap();
    let entropy = data.get("entropy").unwrap().as_f64().unwrap();

    assert!(
        (0.0..=1.0).contains(&churn_rate),
        "churn_rate {} must be in [0.0, 1.0]",
        churn_rate
    );
    assert!(
        (0.0..=1.0).contains(&entropy),
        "entropy {} must be in [0.0, 1.0]",
        entropy
    );

    println!("[FSV PASS] get_topic_stability returns valid metrics");
}

/// FSV Test: Verify get_divergence_alerts returns no alerts for empty database
///
/// Source of Truth: Response structure
/// Edge Case: Empty database has no divergence
#[tokio::test]
async fn test_fsv_divergence_alerts_empty_database() {
    println!("\n=== FSV Test: get_divergence_alerts with empty database ===");

    // Setup
    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // PRE-CONDITION: Verify database is empty
    let count = store.count().await.expect("count() must work");
    println!("PRE-CONDITION: Memory count = {}", count);
    assert_eq!(count, 0, "Database must be empty");

    // EXECUTE: Call get_divergence_alerts
    let params = json!({
        "name": "get_divergence_alerts",
        "arguments": {}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;

    // VERIFY RESPONSE
    assert!(response.error.is_none(), "Should not return error");
    let result = response.result.expect("Must have result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "Tool should succeed");

    let data = extract_mcp_tool_data(&result);

    // VERIFY STRUCTURE
    let alerts = data.get("alerts").expect("Must have alerts field");
    let severity = data.get("severity").expect("Must have severity field");

    println!("ALERTS: {:?}", alerts);
    println!("SEVERITY: {:?}", severity);

    // VERIFY NO ALERTS FOR EMPTY DATABASE
    let alerts_array = alerts.as_array().expect("alerts must be array");
    assert!(
        alerts_array.is_empty(),
        "Empty database should have no divergence alerts"
    );

    let severity_str = severity.as_str().expect("severity must be string");
    assert_eq!(
        severity_str, "none",
        "Severity must be 'none' with no alerts"
    );

    println!("[FSV PASS] get_divergence_alerts returns no alerts for empty database");
}

/// FSV Test: Verify validation errors use MCP isError format
///
/// Validates: Invalid parameters return isError:true, not JSON-RPC error
#[tokio::test]
async fn test_fsv_validation_errors_use_iserror() {
    println!("\n=== FSV Test: Validation errors use isError format ===");

    let (handlers, _, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Test cases: tool name, invalid args, expected error text
    let test_cases = [
        (
            "get_topic_portfolio",
            json!({"format": "invalid_xyz"}),
            "Invalid",
        ),
        ("get_topic_stability", json!({"hours": 0}), "hours"),
        ("get_topic_stability", json!({"hours": 200}), "hours"),
        (
            "get_divergence_alerts",
            json!({"lookback_hours": 0}),
            "lookback",
        ),
        (
            "get_divergence_alerts",
            json!({"lookback_hours": 100}),
            "lookback",
        ),
    ];

    for (tool_name, args, expected_text) in test_cases {
        println!("\nTesting {} with {:?}", tool_name, args);

        let params = json!({
            "name": tool_name,
            "arguments": args
        });
        let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
        let response = handlers.dispatch(request).await;

        // VERIFY: No JSON-RPC error (validation errors use isError)
        assert!(
            response.error.is_none(),
            "{}: Validation errors must use isError format, not JSON-RPC error",
            tool_name
        );

        let result = response.result.expect("Must have result");
        let is_error = result.get("isError").unwrap().as_bool().unwrap();

        // VERIFY: isError is true
        assert!(
            is_error,
            "{}: isError must be true for validation errors",
            tool_name
        );

        // VERIFY: Error message is present in content
        let content = result.get("content").unwrap().as_array().unwrap();
        let text = content[0].get("text").unwrap().as_str().unwrap();
        println!("  Error message: {}", text);

        assert!(
            text.to_lowercase().contains(&expected_text.to_lowercase()),
            "{}: Error message should mention '{}'",
            tool_name,
            expected_text
        );
    }

    println!("\n[FSV PASS] All validation errors use isError format");
}

/// FSV Test: Tier calculation based on memory count
///
/// Per constitution progressive_tiers:
/// - tier_0: 0 memories
/// - tier_1: 1-2 memories
/// - tier_2: 3-9 memories
/// - tier_3: 10-29 memories
#[tokio::test]
async fn test_fsv_tier_calculation_boundaries() {
    println!("\n=== FSV Test: Tier calculation boundaries ===");

    // We test the calculation logic directly via TopicPortfolioResponse
    use crate::handlers::tools::topic_dtos::TopicPortfolioResponse;

    let test_cases = [
        (0, 0),   // tier 0
        (1, 1),   // tier 1
        (2, 1),   // tier 1
        (3, 2),   // tier 2
        (9, 2),   // tier 2
        (10, 3),  // tier 3
        (29, 3),  // tier 3
        (30, 4),  // tier 4
        (99, 4),  // tier 4
        (100, 5), // tier 5
        (499, 5), // tier 5
        (500, 6), // tier 6
    ];

    for (memory_count, expected_tier) in test_cases {
        let actual_tier = TopicPortfolioResponse::tier_for_memory_count(memory_count);
        println!(
            "memory_count={} -> tier={} (expected={})",
            memory_count, actual_tier, expected_tier
        );
        assert_eq!(
            actual_tier, expected_tier,
            "tier_for_memory_count({}) should return {} but got {}",
            memory_count, expected_tier, actual_tier
        );
    }

    println!("\n[FSV PASS] Tier calculation matches constitution progressive_tiers");
}

/// FSV Test: Edge case - maximum values for hours/lookback
///
/// Verifies boundary conditions are handled correctly
#[tokio::test]
async fn test_fsv_max_boundary_values() {
    println!("\n=== FSV Test: Maximum boundary values ===");

    let (handlers, _, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Test max hours (168 = 1 week)
    let params = json!({
        "name": "get_topic_stability",
        "arguments": {"hours": 168}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none(), "hours=168 should be valid");
    let result = response.result.expect("Must have result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "hours=168 should succeed");
    println!("get_topic_stability(hours=168): PASS");

    // Test max lookback (48 hours)
    let params = json!({
        "name": "get_divergence_alerts",
        "arguments": {"lookback_hours": 48}
    });
    let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;
    assert!(
        response.error.is_none(),
        "lookback_hours=48 should be valid"
    );
    let result = response.result.expect("Must have result");
    let is_error = result.get("isError").unwrap().as_bool().unwrap();
    assert!(!is_error, "lookback_hours=48 should succeed");
    println!("get_divergence_alerts(lookback_hours=48): PASS");

    println!("\n[FSV PASS] Maximum boundary values accepted correctly");
}
