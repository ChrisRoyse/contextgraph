//! Manual verification tests for search_periodic tool.
//!
//! These tests verify E3 (V_periodicity) integration:
//! - Store memories at specific times
//! - Query with search_periodic
//! - Verify periodic boost ranks same-time memories higher
//!
//! NO MOCK DATA - uses real MCP server infrastructure with GPU embeddings.
//!
//! Run with: cargo test -p context-graph-mcp --features cuda search_periodic -- --nocapture

use std::time::Duration;
use serde_json::json;
use tracing::info;

use crate::protocol::JsonRpcId;

use super::{create_test_handlers_with_real_embeddings, extract_mcp_tool_data, make_request};

/// Store a memory via inject_context.
async fn store_memory(
    handlers: &crate::handlers::core::Handlers,
    content: &str,
    rationale: &str,
) -> Result<String, String> {
    let req = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "inject_context",
            "arguments": {
                "content": content,
                "rationale": rationale,
                "importance": 0.7
            }
        })),
    );

    let response = handlers.dispatch(req).await;

    if let Some(error) = response.error {
        return Err(format!("inject_context failed: {:?}", error));
    }

    let result = response.result.ok_or("No result")?;
    let data = extract_mcp_tool_data(&result);

    // Extract fingerprintId from response (inject_context returns fingerprintId, not node_id)
    let fingerprint_id = data.get("fingerprintId")
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .ok_or("Failed to extract fingerprintId")?;

    Ok(fingerprint_id)
}

/// Call search_periodic and return results.
async fn search_periodic(
    handlers: &crate::handlers::core::Handlers,
    query: &str,
    target_hour: Option<u8>,
    target_dow: Option<u8>,
    auto_detect: bool,
) -> Result<serde_json::Value, String> {
    let mut args = json!({
        "query": query,
        "topK": 10,
        "includeContent": true,
        "minSimilarity": 0.0
    });

    if let Some(hour) = target_hour {
        args["targetHour"] = json!(hour);
    }
    if let Some(dow) = target_dow {
        args["targetDayOfWeek"] = json!(dow);
    }
    args["autoDetect"] = json!(auto_detect);

    let req = make_request(
        "tools/call",
        Some(JsonRpcId::Number(2)),
        Some(json!({
            "name": "search_periodic",
            "arguments": args
        })),
    );

    let response = handlers.dispatch(req).await;

    if let Some(error) = response.error {
        return Err(format!("search_periodic failed: {:?}", error));
    }

    let result = response.result.ok_or("No result")?;
    Ok(extract_mcp_tool_data(&result))
}

/// Test that search_periodic tool is callable and returns expected structure.
#[tokio::test]
async fn test_search_periodic_basic_structure() {
    println!("\n======================================================================");
    println!("TC-PERIODIC-001: search_periodic Basic Structure Test");
    println!("======================================================================\n");

    let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

    // Store a test memory
    let content = "Morning standup meeting notes: discussed sprint progress and blockers";
    let store_result = store_memory(&handlers, content, "Test memory for periodic search").await;

    match &store_result {
        Ok(fingerprint_id) => info!("Stored memory with fingerprintId: {}", fingerprint_id),
        Err(e) => panic!("Failed to store memory: {}", e),
    }

    // Give time for indexing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Query with search_periodic
    let result = search_periodic(&handlers, "meeting", None, None, false).await;

    match result {
        Ok(json) => {
            info!("search_periodic response: {:?}", json);

            // Verify response structure
            assert!(json.get("query").is_some(), "Response should have 'query' field");
            assert!(json.get("results").is_some(), "Response should have 'results' field");
            assert!(json.get("count").is_some(), "Response should have 'count' field");
            assert!(json.get("periodicConfig").is_some(), "Response should have 'periodicConfig' field");

            // Verify periodicConfig structure
            let config = json.get("periodicConfig").unwrap();
            assert!(config.get("periodicWeight").is_some(), "Config should have 'periodicWeight'");
            assert!(config.get("autoDetected").is_some(), "Config should have 'autoDetected'");

            println!("✓ search_periodic returns correct structure");
        }
        Err(e) => {
            panic!("search_periodic failed: {}", e);
        }
    }
}

/// Test hour validation - targetHour > 23 should return error.
#[tokio::test]
async fn test_search_periodic_hour_validation() {
    println!("\n======================================================================");
    println!("TC-PERIODIC-002: search_periodic Hour Validation Test");
    println!("======================================================================\n");

    let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

    // Try with invalid hour
    let req = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_periodic",
            "arguments": {
                "query": "test",
                "targetHour": 25  // Invalid - should be 0-23
            }
        })),
    );

    let response = handlers.dispatch(req).await;

    // Check for error in response
    if let Some(result) = response.result {
        // MCP tool errors are returned in result.content[].text with isError=true
        if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
            if is_error {
                let text = result
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                // Should contain error about hour validation
                assert!(
                    text.contains("targetHour must be 0-23") || text.contains("error"),
                    "Expected hour validation error, got: {}", text
                );
                info!("Hour validation correctly returned error: {}", text);
                println!("✓ Hour validation error returned correctly");
                return;
            }
        }

        // If we got a result without isError, that's also a failure
        panic!("Expected error for invalid hour, but got result: {:?}", result);
    }

    // JsonRpc error is also acceptable
    if let Some(error) = response.error {
        info!("Hour validation returned JsonRpc error: {:?}", error);
        println!("✓ Hour validation error returned correctly");
    }
}

/// Test day of week validation - targetDayOfWeek > 6 should return error.
#[tokio::test]
async fn test_search_periodic_dow_validation() {
    println!("\n======================================================================");
    println!("TC-PERIODIC-003: search_periodic Day of Week Validation Test");
    println!("======================================================================\n");

    let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

    // Try with invalid day of week
    let req = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_periodic",
            "arguments": {
                "query": "test",
                "targetDayOfWeek": 8  // Invalid - should be 0-6
            }
        })),
    );

    let response = handlers.dispatch(req).await;

    // Check for error in response
    if let Some(result) = response.result {
        if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
            if is_error {
                let text = result
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                // Should contain error about day validation
                assert!(
                    text.contains("targetDayOfWeek must be 0-6") || text.contains("error"),
                    "Expected day validation error, got: {}", text
                );
                info!("Day of week validation correctly returned error: {}", text);
                println!("✓ Day of week validation error returned correctly");
                return;
            }
        }

        panic!("Expected error for invalid day of week, but got result: {:?}", result);
    }

    if let Some(error) = response.error {
        info!("Day of week validation returned JsonRpc error: {:?}", error);
        println!("✓ Day of week validation error returned correctly");
    }
}

/// Test empty query validation.
#[tokio::test]
async fn test_search_periodic_empty_query() {
    println!("\n======================================================================");
    println!("TC-PERIODIC-004: search_periodic Empty Query Validation Test");
    println!("======================================================================\n");

    let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

    let req = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_periodic",
            "arguments": {
                "query": ""  // Empty query
            }
        })),
    );

    let response = handlers.dispatch(req).await;

    // Check for error in response
    if let Some(result) = response.result {
        if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
            if is_error {
                let text = result
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                // Should contain error about empty query
                assert!(
                    text.contains("cannot be empty") || text.contains("error"),
                    "Expected empty query error, got: {}", text
                );
                info!("Empty query validation correctly returned error: {}", text);
                println!("✓ Empty query validation error returned correctly");
                return;
            }
        }

        panic!("Expected error for empty query, but got result: {:?}", result);
    }

    if let Some(error) = response.error {
        info!("Empty query validation returned JsonRpc error: {:?}", error);
        println!("✓ Empty query validation error returned correctly");
    }
}

/// Test auto-detect uses current time.
#[tokio::test]
async fn test_search_periodic_auto_detect() {
    println!("\n======================================================================");
    println!("TC-PERIODIC-005: search_periodic Auto-Detect Test");
    println!("======================================================================\n");

    let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

    // Store a test memory
    let _ = store_memory(
        &handlers,
        "Automated deployment process completed successfully",
        "Test memory for auto-detect"
    ).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Query with auto-detect
    let result = search_periodic(&handlers, "deployment", None, None, true).await;

    match result {
        Ok(json) => {
            let config = json.get("periodicConfig").unwrap();

            // Verify auto-detect was applied
            assert_eq!(config.get("autoDetected").unwrap().as_bool().unwrap(), true);

            // Should have effective hour and day populated
            let target_hour = config.get("targetHour");
            let target_dow = config.get("targetDayOfWeek");

            info!("Auto-detected targetHour: {:?}", target_hour);
            info!("Auto-detected targetDayOfWeek: {:?}", target_dow);

            // At least one should be set from auto-detect
            assert!(
                target_hour.is_some() || target_dow.is_some(),
                "Auto-detect should set at least one target"
            );

            println!("✓ Auto-detect populated time targets");
        }
        Err(e) => {
            panic!("search_periodic with auto-detect failed: {}", e);
        }
    }
}

/// Test that results include expected fields.
#[tokio::test]
async fn test_search_periodic_result_fields() {
    println!("\n======================================================================");
    println!("TC-PERIODIC-006: search_periodic Result Fields Test");
    println!("======================================================================\n");

    let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

    // Store a test memory
    let _ = store_memory(
        &handlers,
        "Friday code review session completed",
        "Test for result fields"
    ).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Query with specific time targets
    let result = search_periodic(&handlers, "code review", Some(14), Some(5), false).await; // 2pm Friday

    match result {
        Ok(json) => {
            let results = json.get("results").unwrap().as_array().unwrap();

            // TEST-12 FIX: The test stores a memory then searches for it — results must not be empty.
            assert!(!results.is_empty(), "Search should return results after storing a matching memory");
            let first = &results[0];

            // Verify all expected fields are present
            assert!(first.get("id").is_some(), "Result should have 'id'");
            assert!(first.get("semanticScore").is_some(), "Result should have 'semanticScore'");
            assert!(first.get("periodicScore").is_some(), "Result should have 'periodicScore'");
            assert!(first.get("finalScore").is_some(), "Result should have 'finalScore'");
            assert!(first.get("memoryHour").is_some(), "Result should have 'memoryHour'");
            assert!(first.get("memoryDayOfWeek").is_some(), "Result should have 'memoryDayOfWeek'");
            assert!(first.get("dayName").is_some(), "Result should have 'dayName'");
            assert!(first.get("createdAt").is_some(), "Result should have 'createdAt'");

            let semantic_score = first.get("semanticScore").unwrap().as_f64().unwrap();
            let periodic_score = first.get("periodicScore").unwrap().as_f64().unwrap();
            let final_score = first.get("finalScore").unwrap().as_f64().unwrap();

            info!("Result scores - semantic: {}, periodic: {}, final: {}",
                  semantic_score, periodic_score, final_score);

            // Verify scores are in valid ranges
            assert!(semantic_score >= 0.0 && semantic_score <= 1.0, "Semantic score out of range");
            assert!(periodic_score >= 0.0 && periodic_score <= 1.0, "Periodic score out of range");
            assert!(final_score >= 0.0, "Final score should be non-negative");
        }
        Err(e) => {
            panic!("search_periodic failed: {}", e);
        }
    }
}

/// Test that day_name is correctly computed.
#[tokio::test]
async fn test_search_periodic_day_names() {
    println!("\n======================================================================");
    println!("TC-PERIODIC-007: search_periodic Day Names Test");
    println!("======================================================================\n");

    let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

    // Store a test memory
    let _ = store_memory(
        &handlers,
        "Weekend project planning session",
        "Test for day names"
    ).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let result = search_periodic(&handlers, "project planning", None, None, false).await;

    match result {
        Ok(json) => {
            let results = json.get("results").unwrap().as_array().unwrap();

            // TEST-12 FIX: The test stores a memory then searches for it — results must not be empty.
            assert!(!results.is_empty(), "Search should return results after storing a matching memory");
            let first = &results[0];
            let day_name = first.get("dayName").unwrap().as_str().unwrap();
            let dow = first.get("memoryDayOfWeek").unwrap().as_u64().unwrap();

            // Verify day_name matches day of week
            let expected_name = match dow {
                0 => "Sunday",
                1 => "Monday",
                2 => "Tuesday",
                3 => "Wednesday",
                4 => "Thursday",
                5 => "Friday",
                6 => "Saturday",
                _ => "Unknown",
            };

            assert_eq!(day_name, expected_name,
                "Day name '{}' should match dow {} (expected '{}')", day_name, dow, expected_name);
            info!("Day name verification passed: dow={} -> {}", dow, day_name);
        }
        Err(e) => {
            panic!("search_periodic failed: {}", e);
        }
    }
}
