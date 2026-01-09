//! Manual Teleological Validation Tests (TELEO-VAL-001 through TELEO-VAL-012)
//!
//! Full State Verification (FSV) tests with predetermined inputs and expected outputs
//! for all 5 teleological MCP tools as specified in the validation guide.
//!
//! These tests verify the handler implementations work correctly with
//! exact inputs and validate outputs against expected values.
//! NO MOCKS. NO FALLBACKS. FAIL FAST if something doesn't work.

use crate::protocol::JsonRpcId;
use super::{create_test_handlers, make_request, extract_mcp_tool_data};
use serde_json::json;

// =============================================================================
// HELPER: Generate arrays of specific sizes
// =============================================================================

fn uniform_pv(val: f32) -> [f32; 13] {
    [val; 13]
}

fn uniform_cc(val: f32) -> Vec<f32> {
    vec![val; 78]
}

fn uniform_ga(val: f32) -> [f32; 6] {
    [val; 6]
}

fn one_hot_pv(idx: usize) -> [f32; 13] {
    let mut arr = [0.0f32; 13];
    if idx < 13 {
        arr[idx] = 1.0;
    }
    arr
}

fn one_hot_cc(idx: usize) -> Vec<f32> {
    let mut arr = vec![0.0f32; 78];
    if idx < 78 {
        arr[idx] = 1.0;
    }
    arr
}

fn one_hot_ga(idx: usize) -> [f32; 6] {
    let mut arr = [0.0f32; 6];
    if idx < 6 {
        arr[idx] = 1.0;
    }
    arr
}

// =============================================================================
// TELEO-VAL-001: search_teleological - Identical Vectors (similarity ≈ 1.0)
// =============================================================================

#[tokio::test]
async fn test_val_001_search_teleological_identical_vectors_similarity_1() {
    let handlers = create_test_handlers();

    // Create identical query and candidate vectors
    let pv = uniform_pv(0.5);
    let cc = uniform_cc(0.3);
    let ga = uniform_ga(0.4);

    let query = json!({
        "purpose_vector": pv,
        "cross_correlations": cc,
        "group_alignments": ga,
        "confidence": 1.0,
        "id": "query-vec"
    });

    let candidate = json!({
        "purpose_vector": pv,
        "cross_correlations": cc,
        "group_alignments": ga,
        "confidence": 1.0,
        "id": "identical-candidate"
    });

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query": query,
                "candidates": [candidate],
                "strategy": "cosine",
                "scope": "full",
                "top_k": 10,
                "threshold": 0.0,
                "include_breakdown": false
            }
        })),
    );

    let response = handlers.dispatch(request).await;

    // FAIL FAST: No error allowed
    assert!(response.error.is_none(), "search_teleological MUST NOT error: {:?}", response.error);

    let result = response.result.expect("MUST have result");
    let parsed = extract_mcp_tool_data(&result);

    // Validate success
    assert!(parsed["success"].as_bool().unwrap_or(false), "MUST succeed");

    // Validate structure
    assert_eq!(parsed["strategy"].as_str(), Some("cosine"));
    assert_eq!(parsed["num_candidates"].as_u64(), Some(1));
    assert_eq!(parsed["num_results"].as_u64(), Some(1));

    // Get results array
    let results = parsed["results"].as_array().expect("MUST have results array");
    assert!(!results.is_empty(), "MUST have at least one result");

    // CRITICAL: Similarity for identical vectors MUST be ≈ 1.0
    let first = &results[0];
    let similarity = first["similarity"].as_f64().expect("MUST have similarity");

    // Log physical evidence
    eprintln!("=== PHYSICAL EVIDENCE: search_teleological identical vectors ===");
    eprintln!("Query ID: query-vec");
    eprintln!("Candidate ID: identical-candidate");
    eprintln!("Computed Similarity: {}", similarity);
    eprintln!("Expected: >= 0.999 (identical vectors)");
    eprintln!("Vector ID returned: {:?}", first["vector_id"]);
    eprintln!("Rank: {:?}", first["rank"]);
    eprintln!("================================================================");

    assert!(
        similarity >= 0.999,
        "Identical vectors MUST have similarity >= 0.999, got {}",
        similarity
    );
    assert_eq!(first["rank"].as_u64(), Some(0), "First result MUST have rank 0");
    assert_eq!(first["vector_id"].as_str(), Some("identical-candidate"));
}

// =============================================================================
// TELEO-VAL-002: search_teleological - Orthogonal Vectors (similarity ≈ 0.0)
// =============================================================================

#[tokio::test]
async fn test_val_002_search_teleological_orthogonal_vectors_similarity_0() {
    let handlers = create_test_handlers();

    // Create orthogonal vectors (one-hot in different dimensions)
    // Set confidence to 0.0 to exclude confidence weight contribution
    // (default confidence weight is 0.1, which would add 0.1 to overall similarity)
    let query = json!({
        "purpose_vector": one_hot_pv(0),
        "cross_correlations": one_hot_cc(0),
        "group_alignments": one_hot_ga(0),
        "confidence": 0.0  // Zero confidence excludes confidence weight from similarity
    });

    let candidate = json!({
        "purpose_vector": one_hot_pv(1),
        "cross_correlations": one_hot_cc(1),
        "group_alignments": one_hot_ga(1),
        "confidence": 0.0,  // Zero confidence excludes confidence weight from similarity
        "id": "orthogonal-candidate"
    });

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query": query,
                "candidates": [candidate],
                "strategy": "cosine",
                "include_breakdown": true
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none(), "MUST NOT error: {:?}", response.error);

    let result = response.result.expect("MUST have result");
    let parsed = extract_mcp_tool_data(&result);

    assert!(parsed["success"].as_bool().unwrap_or(false));

    let results = parsed["results"].as_array().expect("MUST have results");
    let first = &results[0];
    let similarity = first["similarity"].as_f64().expect("MUST have similarity");

    // Verify breakdown is included
    assert!(first.get("breakdown").is_some(), "MUST include breakdown when requested");

    let breakdown = &first["breakdown"];
    let pv_sim = breakdown["purpose_vector"].as_f64().unwrap_or(-1.0);
    let cc_sim = breakdown["cross_correlations"].as_f64().unwrap_or(-1.0);
    let ga_sim = breakdown["group_alignments"].as_f64().unwrap_or(-1.0);

    // Log physical evidence
    eprintln!("=== PHYSICAL EVIDENCE: search_teleological orthogonal vectors ===");
    eprintln!("Computed Similarity: {}", similarity);
    eprintln!("Breakdown:");
    eprintln!("  - purpose_vector: {}", pv_sim);
    eprintln!("  - cross_correlations: {}", cc_sim);
    eprintln!("  - group_alignments: {}", ga_sim);
    eprintln!("NOTE: confidence=0.0 used to exclude confidence weight contribution");
    eprintln!("Expected: Overall similarity ≤ 0.05 (orthogonal, no confidence weight)");
    eprintln!("===================================================================");

    // CRITICAL: Orthogonal vectors with zero confidence MUST have near-zero similarity
    // The 0.05 threshold accounts for floating point precision
    assert!(
        similarity <= 0.05,
        "Orthogonal vectors MUST have similarity <= 0.05 (got {}). \
        Similarity formula: 0.4*pv + 0.35*cc + 0.15*ga + 0.1*conf. \
        With orthogonal vectors and conf=0, should be near 0.",
        similarity
    );

    // Individual components should be near-zero for orthogonal one-hot vectors
    assert!(pv_sim <= 0.05, "PV similarity for orthogonal: {} (expected <= 0.05)", pv_sim);
    assert!(cc_sim <= 0.05, "CC similarity for orthogonal: {} (expected <= 0.05)", cc_sim);
    assert!(ga_sim <= 0.05, "GA similarity for orthogonal: {} (expected <= 0.05)", ga_sim);
}

// =============================================================================
// TELEO-VAL-003: search_teleological - Ranking Order (descending similarity)
// =============================================================================

#[tokio::test]
async fn test_val_003_search_teleological_ranking_order_descending() {
    let handlers = create_test_handlers();

    // Query vector
    let query_pv = [0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0, 0.1, 0.2, 0.3];
    let query = json!({
        "purpose_vector": query_pv,
        "cross_correlations": uniform_cc(0.5),
        "group_alignments": [0.6, 0.5, 0.4, 0.3, 0.2, 0.1],
        "confidence": 1.0
    });

    // Best match: identical to query
    let best = json!({
        "purpose_vector": query_pv,
        "cross_correlations": uniform_cc(0.5),
        "group_alignments": [0.6, 0.5, 0.4, 0.3, 0.2, 0.1],
        "confidence": 1.0,
        "id": "best-match"
    });

    // Medium match: slightly different
    let medium = json!({
        "purpose_vector": uniform_pv(0.5),
        "cross_correlations": uniform_cc(0.5),
        "group_alignments": uniform_ga(0.5),
        "confidence": 0.8,
        "id": "medium-match"
    });

    // Worst match: very different
    let worst = json!({
        "purpose_vector": uniform_pv(0.1),
        "cross_correlations": uniform_cc(0.1),
        "group_alignments": uniform_ga(0.1),
        "confidence": 0.5,
        "id": "worst-match"
    });

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "search_teleological",
            "arguments": {
                "query": query,
                "candidates": [best, medium, worst],
                "strategy": "cosine",
                "top_k": 3
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.expect("result");
    let parsed = extract_mcp_tool_data(&result);

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["num_results"].as_u64(), Some(3));

    let results = parsed["results"].as_array().expect("results array");

    // Log physical evidence
    eprintln!("=== PHYSICAL EVIDENCE: search_teleological ranking order ===");
    for (i, r) in results.iter().enumerate() {
        eprintln!(
            "Rank {}: id={}, similarity={}",
            i,
            r["vector_id"].as_str().unwrap_or("?"),
            r["similarity"].as_f64().unwrap_or(-1.0)
        );
    }
    eprintln!("=============================================================");

    // Verify ranking
    assert_eq!(results[0]["rank"].as_u64(), Some(0));
    assert_eq!(results[0]["vector_id"].as_str(), Some("best-match"));

    let sim0 = results[0]["similarity"].as_f64().unwrap();
    let sim1 = results[1]["similarity"].as_f64().unwrap();
    let sim2 = results[2]["similarity"].as_f64().unwrap();

    // CRITICAL: Results MUST be in descending order by similarity
    assert!(sim0 >= sim1, "Result[0] sim {} MUST >= Result[1] sim {}", sim0, sim1);
    assert!(sim1 >= sim2, "Result[1] sim {} MUST >= Result[2] sim {}", sim1, sim2);

    // Best match should have ~1.0 similarity
    assert!(sim0 >= 0.99, "Best match should have similarity >= 0.99, got {}", sim0);
}

// =============================================================================
// TELEO-VAL-004: fuse_embeddings - Uniform Fusion (13 embeddings × 1024 dims)
// =============================================================================

#[tokio::test]
async fn test_val_004_fuse_embeddings_uniform_fusion() {
    let handlers = create_test_handlers();

    // Create 13 uniform embeddings of 1024 dimensions each
    let embeddings: Vec<Vec<f32>> = (0..13)
        .map(|i| vec![(i as f32 + 1.0) * 0.1; 1024])
        .collect();

    // Provide explicit alignments > 0.1 threshold for active_embedders count
    // FusionEngine counts alignments > 0.1 as "active"
    // Without explicit alignments, compute_alignments_from_embeddings normalizes to sum=1.0,
    // resulting in each alignment ≈ 1/13 ≈ 0.077 (below 0.1 threshold)
    let alignments: [f32; 13] = [0.8; 13]; // All above 0.1 threshold

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "fuse_embeddings",
            "arguments": {
                "embeddings": embeddings,
                "alignments": alignments,  // Explicit alignments > 0.1
                "fusion_method": "weighted_average"
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none(), "fuse_embeddings MUST NOT error: {:?}", response.error);

    let result = response.result.expect("result");
    let parsed = extract_mcp_tool_data(&result);

    assert!(parsed["success"].as_bool().unwrap_or(false), "MUST succeed");

    // Validate vector structure
    let vector = &parsed["vector"];

    let pv = vector["purpose_vector"].as_array().expect("purpose_vector array");
    let cc = vector["cross_correlations"].as_array().expect("cross_correlations array");
    let ga = vector["group_alignments"].as_array().expect("group_alignments array");

    // Log physical evidence
    eprintln!("=== PHYSICAL EVIDENCE: fuse_embeddings uniform fusion ===");
    eprintln!("Vector dimensions:");
    eprintln!("  - purpose_vector: {} elements (expected 13)", pv.len());
    eprintln!("  - cross_correlations: {} elements (expected 78)", cc.len());
    eprintln!("  - group_alignments: {} elements (expected 6)", ga.len());
    eprintln!("Confidence: {}", parsed["confidence"].as_f64().unwrap_or(-1.0));
    eprintln!("Metadata active_embedders: {}",
        parsed["metadata"]["active_embedders"].as_u64().unwrap_or(0));
    eprintln!("NOTE: Explicit alignments [0.8; 13] provided (all > 0.1 threshold)");
    eprintln!("==========================================================");

    // CRITICAL: Verify exact dimensions
    assert_eq!(pv.len(), 13, "purpose_vector MUST have 13 elements");
    assert_eq!(cc.len(), 78, "cross_correlations MUST have 78 elements");
    assert_eq!(ga.len(), 6, "group_alignments MUST have 6 elements");

    // Confidence must be in [0, 1]
    let confidence = parsed["confidence"].as_f64().expect("confidence");
    assert!(
        (0.0..=1.0).contains(&confidence),
        "Confidence {} MUST be in [0.0, 1.0]",
        confidence
    );

    // Metadata verification - with explicit alignments [0.8; 13], all 13 are > 0.1 threshold
    assert_eq!(
        parsed["metadata"]["active_embedders"].as_u64(),
        Some(13),
        "active_embedders MUST be 13 when all alignments > 0.1 threshold"
    );
}

// =============================================================================
// TELEO-VAL-005: fuse_embeddings - Wrong Count (error case)
// =============================================================================

#[tokio::test]
async fn test_val_005_fuse_embeddings_wrong_count_error() {
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
    let result = response.result.expect("Result");

    // Check for MCP error format
    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);

    // Log physical evidence
    eprintln!("=== PHYSICAL EVIDENCE: fuse_embeddings wrong count error ===");
    eprintln!("isError: {}", is_error);
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(text) = content.first().and_then(|c| c.get("text")).and_then(|t| t.as_str()) {
            eprintln!("Error message: {}", text);
        }
    }
    eprintln!("=============================================================");

    // CRITICAL: Must return error for wrong embedding count
    assert!(is_error, "MUST return isError=true for wrong embedding count");
}

// =============================================================================
// TELEO-VAL-006: update_synergy_matrix - Positive Feedback
// =============================================================================

#[tokio::test]
async fn test_val_006_update_synergy_positive_feedback() {
    let handlers = create_test_handlers();

    let vector_id = "550e8400-e29b-41d4-a716-446655440000";

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "update_synergy_matrix",
            "arguments": {
                "vector_id": vector_id,
                "feedback_type": "positive",
                "context": "User found this result highly relevant"
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.expect("result");
    let parsed = extract_mcp_tool_data(&result);

    // Log physical evidence
    eprintln!("=== PHYSICAL EVIDENCE: update_synergy_matrix positive ===");
    eprintln!("success: {}", parsed["success"].as_bool().unwrap_or(false));
    eprintln!("vector_id: {}", parsed["vector_id"].as_str().unwrap_or("?"));
    eprintln!("feedback_type: {}", parsed["feedback_type"].as_str().unwrap_or("?"));
    eprintln!("learning_triggered: {}", parsed["learning_triggered"].as_bool().unwrap_or(false));
    eprintln!("==========================================================");

    assert!(parsed["success"].as_bool().unwrap_or(false), "MUST succeed");
    assert_eq!(parsed["vector_id"].as_str(), Some(vector_id));
    assert_eq!(parsed["feedback_type"].as_str(), Some("positive"));
}

// =============================================================================
// TELEO-VAL-007: update_synergy_matrix - Negative Feedback
// =============================================================================

#[tokio::test]
async fn test_val_007_update_synergy_negative_feedback() {
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

    let result = response.result.expect("result");
    let parsed = extract_mcp_tool_data(&result);

    eprintln!("=== PHYSICAL EVIDENCE: update_synergy_matrix negative ===");
    eprintln!("success: {}", parsed["success"].as_bool().unwrap_or(false));
    eprintln!("feedback_type: {}", parsed["feedback_type"].as_str().unwrap_or("?"));
    eprintln!("==========================================================");

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["feedback_type"].as_str(), Some("negative"));
}

// =============================================================================
// TELEO-VAL-008: update_synergy_matrix - Invalid UUID (error case)
// =============================================================================

#[tokio::test]
async fn test_val_008_update_synergy_invalid_uuid_error() {
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

    eprintln!("=== PHYSICAL EVIDENCE: update_synergy_matrix invalid UUID ===");
    eprintln!("isError: {}", is_error);
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(text) = content.first().and_then(|c| c.get("text")).and_then(|t| t.as_str()) {
            eprintln!("Error message: {}", text);
        }
    }
    eprintln!("==============================================================");

    assert!(is_error, "MUST return isError=true for invalid UUID");
}

// =============================================================================
// TELEO-VAL-009: manage_teleological_profile - Create Profile
// =============================================================================

#[tokio::test]
async fn test_val_009_manage_profile_create() {
    let handlers = create_test_handlers();

    let weights: [f32; 13] = [
        0.20, 0.10, 0.05, 0.15, 0.05, 0.05, 0.10, 0.08, 0.05, 0.05, 0.04, 0.05, 0.03
    ];

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "manage_teleological_profile",
            "arguments": {
                "action": "create",
                "profile_id": "code-analysis-profile",
                "weights": weights
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.expect("result");
    let parsed = extract_mcp_tool_data(&result);

    eprintln!("=== PHYSICAL EVIDENCE: manage_teleological_profile create ===");
    eprintln!("success: {}", parsed["success"].as_bool().unwrap_or(false));
    eprintln!("action: {}", parsed["action"].as_str().unwrap_or("?"));
    eprintln!("profile_id: {}", parsed["profile_id"].as_str().unwrap_or("?"));
    if let Some(w) = parsed["weights"].as_array() {
        eprintln!("weights count: {} (expected 13)", w.len());
    }
    eprintln!("===============================================================");

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["action"].as_str(), Some("create"));
    assert_eq!(parsed["profile_id"].as_str(), Some("code-analysis-profile"));

    let returned_weights = parsed["weights"].as_array().expect("weights array");
    assert_eq!(returned_weights.len(), 13, "MUST have 13 weights");
}

// =============================================================================
// TELEO-VAL-010: manage_teleological_profile - List Profiles
// =============================================================================

#[tokio::test]
async fn test_val_010_manage_profile_list() {
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

    let result = response.result.expect("result");
    let parsed = extract_mcp_tool_data(&result);

    eprintln!("=== PHYSICAL EVIDENCE: manage_teleological_profile list ===");
    eprintln!("success: {}", parsed["success"].as_bool().unwrap_or(false));
    eprintln!("action: {}", parsed["action"].as_str().unwrap_or("?"));
    if let Some(profiles) = parsed["profiles"].as_array() {
        eprintln!("profiles: {:?}", profiles);
    }
    eprintln!("count: {}", parsed["count"].as_u64().unwrap_or(0));
    eprintln!("=============================================================");

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["action"].as_str(), Some("list"));
    assert!(parsed.get("profiles").is_some(), "MUST have profiles array");
    assert!(parsed.get("count").is_some(), "MUST have count");
}

// =============================================================================
// TELEO-VAL-011: manage_teleological_profile - Find Best
// =============================================================================

#[tokio::test]
async fn test_val_011_manage_profile_find_best() {
    let handlers = create_test_handlers();

    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(json!({
            "name": "manage_teleological_profile",
            "arguments": {
                "action": "find_best",
                "context": "semantic code search with entity extraction"
            }
        })),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none());

    let result = response.result.expect("result");
    let parsed = extract_mcp_tool_data(&result);

    eprintln!("=== PHYSICAL EVIDENCE: manage_teleological_profile find_best ===");
    eprintln!("success: {}", parsed["success"].as_bool().unwrap_or(false));
    eprintln!("action: {}", parsed["action"].as_str().unwrap_or("?"));
    eprintln!("profile_id: {:?}", parsed.get("profile_id"));
    if let Some(sim) = parsed.get("similarity") {
        eprintln!("similarity: {}", sim);
    }
    eprintln!("=================================================================");

    assert!(parsed["success"].as_bool().unwrap_or(false));
    assert_eq!(parsed["action"].as_str(), Some("find_best"));
    // profile_id can be null if no profiles match
}

// =============================================================================
// TELEO-VAL-012: manage_teleological_profile - Invalid Action (error case)
// =============================================================================

#[tokio::test]
async fn test_val_012_manage_profile_invalid_action_error() {
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
    let result = response.result.expect("Result");

    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);

    eprintln!("=== PHYSICAL EVIDENCE: manage_teleological_profile invalid action ===");
    eprintln!("isError: {}", is_error);
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        if let Some(text) = content.first().and_then(|c| c.get("text")).and_then(|t| t.as_str()) {
            eprintln!("Error message: {}", text);
            assert!(
                text.contains("Unknown action") || text.contains("invalid_action"),
                "Error message should mention the invalid action"
            );
        }
    }
    eprintln!("=====================================================================");

    assert!(is_error, "MUST return isError=true for invalid action");
}
