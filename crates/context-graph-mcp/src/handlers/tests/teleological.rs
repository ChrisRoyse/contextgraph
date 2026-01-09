//! Teleological Handler Tests (TELEO-H1 through TELEO-H5)
//!
//! Manual validation tests with synthetic data for teleological MCP tools.
//! These tests verify the handler implementations work correctly with
//! predetermined inputs and expected outputs.

use crate::protocol::JsonRpcId;
use super::{create_test_handlers, make_request};
use serde_json::json;

// =============================================================================
// TELEO-H1: search_teleological
// =============================================================================

#[tokio::test]
async fn test_search_teleological_basic() {
    let handlers = create_test_handlers();

    // Create synthetic teleological vectors
    let query = json!({
        "purpose_vector": [0.5, 0.3, 0.2, 0.4, 0.6, 0.1, 0.7, 0.5, 0.3, 0.8, 0.2, 0.4, 0.9],
        "cross_correlations": vec![0.1f32; 78],
        "group_alignments": [0.4, 0.3, 0.5, 0.6, 0.2, 0.7],
        "confidence": 0.95,
        "id": "query-vector"
    });

    let candidate1 = json!({
        "purpose_vector": [0.5, 0.3, 0.2, 0.4, 0.6, 0.1, 0.7, 0.5, 0.3, 0.8, 0.2, 0.4, 0.9],
        "cross_correlations": vec![0.1f32; 78],
        "group_alignments": [0.4, 0.3, 0.5, 0.6, 0.2, 0.7],
        "confidence": 0.9,
        "id": "identical-vector"
    });

    let candidate2 = json!({
        "purpose_vector": [0.1, 0.9, 0.8, 0.2, 0.1, 0.9, 0.3, 0.1, 0.7, 0.2, 0.8, 0.1, 0.1],
        "cross_correlations": vec![0.9f32; 78],
        "group_alignments": [0.9, 0.1, 0.2, 0.1, 0.8, 0.1],
        "confidence": 0.7,
        "id": "different-vector"
    });

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query": query,
                "candidates": [candidate1, candidate2],
                "strategy": "cosine",
                "scope": "full",
                "top_k": 10,
                "threshold": 0.0,
                "include_breakdown": false
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "search_teleological should succeed");
    let result = response.result.expect("Must have result");

    // Verify structure
    assert!(result.get("content").is_some(), "Must have content");
    let content = result["content"].as_array().expect("content is array");
    assert!(!content.is_empty(), "Content should not be empty");

    // Parse the text content
    let text = content[0]["text"].as_str().expect("Must have text");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Must be valid JSON");

    assert!(parsed["success"].as_bool().unwrap_or(false), "Should succeed");
    assert_eq!(parsed["num_candidates"].as_u64(), Some(2));

    // Results should exist
    let results = parsed["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "Should have results");

    // First result should be the identical vector (highest similarity)
    let first = &results[0];
    assert_eq!(first["rank"].as_u64(), Some(0));
    assert_eq!(first["vector_id"].as_str(), Some("identical-vector"));

    // Similarity should be close to 1.0 for identical vectors
    let similarity = first["similarity"].as_f64().expect("similarity");
    assert!(similarity > 0.9, "Identical vectors should have high similarity: {}", similarity);
}

#[tokio::test]
async fn test_search_teleological_with_breakdown() {
    let handlers = create_test_handlers();

    let pv_half: Vec<f32> = vec![0.5; 13];
    let cc_half: Vec<f32> = vec![0.5; 78];
    let ga_half: Vec<f32> = vec![0.5; 6];

    let query = json!({
        "purpose_vector": pv_half.clone(),
        "cross_correlations": cc_half.clone(),
        "group_alignments": ga_half.clone(),
        "confidence": 1.0
    });

    let candidate = json!({
        "purpose_vector": pv_half,
        "cross_correlations": cc_half,
        "group_alignments": ga_half,
        "confidence": 1.0,
        "id": "test-candidate"
    });

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query": query,
                "candidates": [candidate],
                "include_breakdown": true
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    let content = result["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    let results = parsed["results"].as_array().unwrap();
    assert!(!results.is_empty());

    // Verify breakdown is included
    let first = &results[0];
    assert!(first.get("breakdown").is_some(), "Should include breakdown");

    let breakdown = &first["breakdown"];
    assert!(breakdown.get("overall").is_some());
    assert!(breakdown.get("purpose_vector").is_some());
    assert!(breakdown.get("cross_correlations").is_some());
    assert!(breakdown.get("group_alignments").is_some());
}

#[tokio::test]
async fn test_search_teleological_different_strategies() {
    let handlers = create_test_handlers();

    let pv_query: Vec<f32> = vec![0.5; 13];
    let cc_query: Vec<f32> = vec![0.5; 78];
    let ga_query: Vec<f32> = vec![0.5; 6];

    let pv_cand: Vec<f32> = vec![0.6; 13];
    let cc_cand: Vec<f32> = vec![0.4; 78];
    let ga_cand: Vec<f32> = vec![0.6; 6];

    let query = json!({
        "purpose_vector": pv_query,
        "cross_correlations": cc_query,
        "group_alignments": ga_query,
        "confidence": 1.0
    });

    let candidate = json!({
        "purpose_vector": pv_cand,
        "cross_correlations": cc_cand,
        "group_alignments": ga_cand,
        "confidence": 0.9
    });

    let strategies = vec!["cosine", "euclidean", "synergy_weighted", "group_hierarchical"];

    for strategy in strategies {
        let request = make_request(
            "tools/call",
            Some(JsonRpcId::Number(1)),
            Some(json!({
                "name": "search_teleological",
                "arguments": {
                    "query": query,
                    "candidates": [candidate.clone()],
                    "strategy": strategy
                }
            })),
        );

        let response = handlers.dispatch(request).await;
        assert!(
            response.error.is_none(),
            "Strategy '{}' should succeed",
            strategy
        );

        let result = response.result.unwrap();
        let content = result["content"].as_array().unwrap();
        let text = content[0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

        assert_eq!(
            parsed["strategy"].as_str(),
            Some(strategy),
            "Strategy should match"
        );
    }
}

// =============================================================================
// TELEO-H3: fuse_embeddings
// =============================================================================

#[tokio::test]
async fn test_fuse_embeddings_basic() {
    let handlers = create_test_handlers();

    // Create 13 synthetic embeddings (1024 dimensions each as required by FusionEngine)
    let embeddings: Vec<Vec<f32>> = (0..13)
        .map(|i| vec![(i as f32) * 0.1 + 0.1; 1024])
        .collect();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "fuse_embeddings",
            "arguments": {
                "embeddings": embeddings,
                "fusion_method": "weighted_average"
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "fuse_embeddings should succeed");
    let result = response.result.expect("Must have result");

    let content = result["content"].as_array().expect("content is array");
    let text = content[0]["text"].as_str().expect("Must have text");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Must be valid JSON");

    assert!(parsed["success"].as_bool().unwrap_or(false), "Should succeed");

    // Verify vector is returned
    assert!(parsed.get("vector").is_some(), "Must have vector");
    let vector = &parsed["vector"];

    // Check vector components exist
    assert!(vector.get("purpose_vector").is_some());
    assert!(vector.get("cross_correlations").is_some());
    assert!(vector.get("group_alignments").is_some());
    assert!(vector.get("confidence").is_some());

    // Verify confidence in valid range
    let confidence = parsed["confidence"].as_f64().expect("confidence");
    assert!((0.0..=1.0).contains(&confidence), "Confidence should be [0,1]");
}

#[tokio::test]
async fn test_fuse_embeddings_with_alignments() {
    let handlers = create_test_handlers();

    let embeddings: Vec<Vec<f32>> = (0..13)
        .map(|_| vec![0.5; 1024])
        .collect();

    // Custom alignments that sum to 1.0
    let alignments: [f32; 13] = [
        0.15, 0.10, 0.08, 0.10, 0.05,
        0.03, 0.08, 0.10, 0.05, 0.10,
        0.05, 0.06, 0.05
    ];

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "fuse_embeddings",
            "arguments": {
                "embeddings": embeddings,
                "alignments": alignments
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none(), "Should succeed with custom alignments");

    let result = response.result.unwrap();
    let content = result["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(parsed["success"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn test_fuse_embeddings_wrong_count_fails() {
    let handlers = create_test_handlers();

    // Only 5 embeddings instead of 13
    let embeddings: Vec<Vec<f32>> = (0..5)
        .map(|_| vec![0.5; 1024])
        .collect();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "fuse_embeddings",
            "arguments": {
                "embeddings": embeddings
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should fail with error
    let result = response.result.expect("Result");
    let _content = result["content"].as_array().expect("content");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);

    assert!(is_error, "Should return error for wrong embedding count");
}

// =============================================================================
// TELEO-H4: update_synergy_matrix
// =============================================================================

#[tokio::test]
async fn test_update_synergy_matrix_positive_feedback() {
    let handlers = create_test_handlers();

    let vector_id = "11111111-2222-3333-4444-555555555555";

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "update_synergy_matrix",
            "arguments": {
                "vector_id": vector_id,
                "feedback_type": "positive",
                "context": "User selected this result as relevant"
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "Should succeed");
    let result = response.result.unwrap();

    let content = result["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["vector_id"].as_str(), Some(vector_id));
    assert_eq!(parsed["feedback_type"].as_str(), Some("positive"));
}

#[tokio::test]
async fn test_update_synergy_matrix_negative_feedback() {
    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "update_synergy_matrix",
            "arguments": {
                "vector_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
                "feedback_type": "negative"
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    let content = result["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(parsed["success"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn test_update_synergy_matrix_with_contributions() {
    let handlers = create_test_handlers();

    // Contributions that highlight specific embedders
    let contributions: [f32; 13] = [
        0.3, 0.1, 0.05, 0.1, 0.05,
        0.02, 0.08, 0.1, 0.05, 0.05,
        0.05, 0.03, 0.02
    ];

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "update_synergy_matrix",
            "arguments": {
                "vector_id": "12345678-1234-1234-1234-123456789012",
                "feedback_type": "positive",
                "contributions": contributions
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());
}

#[tokio::test]
async fn test_update_synergy_matrix_invalid_uuid_fails() {
    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "update_synergy_matrix",
            "arguments": {
                "vector_id": "not-a-valid-uuid",
                "feedback_type": "positive"
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    let result = response.result.expect("Result");
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);

    assert!(is_error, "Should fail for invalid UUID");
}

// =============================================================================
// TELEO-H5: manage_teleological_profile
// =============================================================================

#[tokio::test]
async fn test_manage_profile_create_and_get() {
    let handlers = create_test_handlers();

    // Create profile
    let create_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "manage_teleological_profile",
            "arguments": {
                "action": "create",
                "profile_id": "test-profile-1",
                "weights": [0.15, 0.10, 0.08, 0.10, 0.05, 0.03, 0.08, 0.10, 0.05, 0.10, 0.05, 0.06, 0.05]
            }
        })),
    );

    let response = handlers.dispatch(create_request).await;
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    let content = result["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["action"].as_str(), Some("create"));
    assert_eq!(parsed["profile_id"].as_str(), Some("test-profile-1"));
}

#[tokio::test]
async fn test_manage_profile_list() {
    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "manage_teleological_profile",
            "arguments": {
                "action": "list"
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    let content = result["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["action"].as_str(), Some("list"));
    assert!(parsed.get("profiles").is_some());
    assert!(parsed.get("count").is_some());
}

#[tokio::test]
async fn test_manage_profile_update_nonexistent() {
    let handlers = create_test_handlers();

    let weights_uniform: Vec<f32> = vec![0.1; 13];

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "manage_teleological_profile",
            "arguments": {
                "action": "update",
                "profile_id": "nonexistent-profile",
                "weights": weights_uniform
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // Should succeed but report profile not found
    let result = response.result.unwrap();
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(is_error, "Should error for nonexistent profile");
}

#[tokio::test]
async fn test_manage_profile_invalid_action() {
    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "manage_teleological_profile",
            "arguments": {
                "action": "invalid_action"
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    let result = response.result.unwrap();
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(is_error, "Should error for invalid action");
}

#[tokio::test]
async fn test_manage_profile_find_best() {
    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "manage_teleological_profile",
            "arguments": {
                "action": "find_best",
                "context": "code analysis with semantic search"
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    let content = result["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["action"].as_str(), Some("find_best"));
}
