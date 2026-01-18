//! Integration tests for dream MCP tools.

use serde_json::json;
use uuid::Uuid;

use crate::handlers::tests::create_test_handlers;

#[tokio::test]
async fn test_trigger_dream_dry_run() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_trigger_dream(
            Some(crate::protocol::JsonRpcId::Number(1)),
            json!({ "dry_run": true, "blocking": true }),
        )
        .await;

    let result = response.result.unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text_content = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text_content).unwrap();

    assert_eq!(parsed.get("dry_run").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(parsed.get("status").and_then(|v| v.as_str()), Some("dry_run_complete"));

    let dream_id_str = parsed.get("dream_id").and_then(|v| v.as_str()).unwrap();
    Uuid::parse_str(dream_id_str).expect("dream_id should be valid UUID");

    let report = parsed.get("report").unwrap();
    assert!(!report.get("recommendations").unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_trigger_dream_skip_both_phases_fails() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_trigger_dream(
            Some(crate::protocol::JsonRpcId::Number(2)),
            json!({ "skip_nrem": true, "skip_rem": true }),
        )
        .await;

    let result = response.result.unwrap();
    assert_eq!(result.get("isError").and_then(|v| v.as_bool()), Some(true));

    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    assert!(text.contains("Cannot skip both"));
}

#[tokio::test]
async fn test_trigger_dream_skip_nrem() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_trigger_dream(
            Some(crate::protocol::JsonRpcId::Number(3)),
            json!({ "skip_nrem": true, "dry_run": true }),
        )
        .await;

    let result = response.result.unwrap();
    assert_eq!(result.get("isError").and_then(|v| v.as_bool()), Some(false));

    let content = result.get("content").unwrap().as_array().unwrap();
    let text_content = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text_content).unwrap();

    // nrem_result should be absent when skip_nrem=true
    assert!(parsed.get("nrem_result").map_or(true, |v| v.is_null()));
}

#[tokio::test]
async fn test_trigger_dream_skip_rem() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_trigger_dream(
            Some(crate::protocol::JsonRpcId::Number(4)),
            json!({ "skip_rem": true, "dry_run": true }),
        )
        .await;

    let result = response.result.unwrap();
    assert_eq!(result.get("isError").and_then(|v| v.as_bool()), Some(false));

    let content = result.get("content").unwrap().as_array().unwrap();
    let text_content = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text_content).unwrap();

    // rem_result should be absent when skip_rem=true
    assert!(parsed.get("rem_result").map_or(true, |v| v.is_null()));
}

#[tokio::test]
async fn test_get_dream_status_no_id() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_get_dream_status(
            Some(crate::protocol::JsonRpcId::Number(5)),
            json!({}),
        )
        .await;

    let result = response.result.unwrap();
    assert_eq!(result.get("isError").and_then(|v| v.as_bool()), Some(false));

    let content = result.get("content").unwrap().as_array().unwrap();
    let text_content = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text_content).unwrap();

    assert!(parsed.get("dream_id").is_some());
    assert!(parsed.get("status").is_some());
    assert!(parsed.get("progress_percent").is_some());
    assert!(parsed.get("current_phase").is_some());
    assert!(parsed.get("elapsed_ms").is_some());
}

#[tokio::test]
async fn test_get_dream_status_with_id() {
    let handlers = create_test_handlers();
    let test_id = Uuid::new_v4().to_string();

    let response = handlers
        .call_get_dream_status(
            Some(crate::protocol::JsonRpcId::Number(6)),
            json!({ "dream_id": test_id }),
        )
        .await;

    let result = response.result.unwrap();
    assert_eq!(result.get("isError").and_then(|v| v.as_bool()), Some(false));
}

#[tokio::test]
async fn test_trigger_dream_invalid_params() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_trigger_dream(
            Some(crate::protocol::JsonRpcId::Number(7)),
            json!({ "max_duration_secs": "not a number" }),
        )
        .await;

    assert!(response.error.is_some());
}

#[tokio::test]
async fn test_trigger_dream_ap70_entropy_check() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_trigger_dream(
            Some(crate::protocol::JsonRpcId::Number(8)),
            json!({ "dry_run": true }),
        )
        .await;

    let result = response.result.unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text_content = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text_content).unwrap();

    let report = parsed.get("report").unwrap();
    assert!(report.get("pre_entropy").is_some());
    assert!(report.get("dream_recommended").is_some());

    let pre_entropy = report.get("pre_entropy").unwrap().as_f64().unwrap();
    let dream_recommended = report.get("dream_recommended").unwrap().as_bool().unwrap();

    // Verify AP-70: dream_recommended should be true only if entropy > 0.7
    if pre_entropy > 0.7 {
        assert!(dream_recommended);
    } else {
        assert!(!dream_recommended);
    }
}

#[tokio::test]
async fn test_dream_tools_include_cognitive_pulse() {
    let handlers = create_test_handlers();

    let response = handlers
        .call_trigger_dream(
            Some(crate::protocol::JsonRpcId::Number(9)),
            json!({ "dry_run": true }),
        )
        .await;
    assert!(response.result.unwrap().get("_cognitive_pulse").is_some());

    let response = handlers
        .call_get_dream_status(
            Some(crate::protocol::JsonRpcId::Number(10)),
            json!({}),
        )
        .await;
    assert!(response.result.unwrap().get("_cognitive_pulse").is_some());
}
