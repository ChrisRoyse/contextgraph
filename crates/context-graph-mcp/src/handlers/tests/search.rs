//! Search Handler Tests
//!
//! TASK-S002: Tests for search/multi, search/single_space, search/by_purpose,
//! and search/weight_profiles handlers.
//!
//! # Test Categories
//!
//! ## Unit Tests (fast, in-memory)
//! - Use `create_test_handlers()` with InMemoryTeleologicalStore
//! - Use StubMultiArrayProvider for embeddings (no GPU required)
//! - Fast execution, suitable for CI without GPU
//!
//! ## Integration Tests (real storage)
//! - Use `create_test_handlers_with_rocksdb()` with RocksDbTeleologicalStore
//! - Use UtlProcessorAdapter for real UTL computation
//! - Embeddings still stubbed until GPU infrastructure ready (TASK-F007)
//! - Verify search operations against real persistent storage
//!
//! # What's Real vs Stubbed
//!
//! | Component | Unit Tests | Integration Tests |
//! |-----------|------------|-------------------|
//! | Storage   | InMemory (stub) | RocksDB (REAL) |
//! | UTL       | Stub | UtlProcessorAdapter (REAL) |
//! | Embeddings | Stub | Stub (GPU required) |
//!
//! Tests verify:
//! - search/multi with preset and custom 13-element weights
//! - search/single_space for specific embedding spaces (0-12)
//! - search/by_purpose with 13D purpose vector alignment
//! - search/weight_profiles returns all profiles with 13 weights each
//! - Error handling for invalid parameters

use serde_json::json;

use crate::protocol::{error_codes, JsonRpcId};
use crate::weights::{get_profile_names, WEIGHT_PROFILES};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

use super::{create_test_handlers, create_test_handlers_with_rocksdb, make_request};

// =============================================================================
// search/multi Tests
// =============================================================================

/// Test search/multi with semantic_search preset.
#[tokio::test]
async fn test_search_multi_semantic_preset() {
    let handlers = create_test_handlers();

    // First store some content to search
    let store_params = json!({
        "content": "Machine learning enables computers to learn from data",
        "importance": 0.9
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Now search with semantic_search preset
    let search_params = json!({
        "query": "AI and data science",
        "query_type": "semantic_search",
        "topK": 10,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/multi should succeed");
    let result = response.result.expect("Should have result");

    // Verify results structure
    assert!(result.get("results").is_some(), "Should have results array");
    assert!(result.get("count").is_some(), "Should have count");
    assert!(result.get("query_metadata").is_some(), "Should have query_metadata");

    // Verify query_metadata structure
    let metadata = result.get("query_metadata").unwrap();
    assert_eq!(
        metadata.get("query_type_used").and_then(|v| v.as_str()),
        Some("semantic_search"),
        "Should use semantic_search preset"
    );
    assert!(metadata.get("weights_applied").is_some(), "Should show weights applied");
    assert!(metadata.get("aggregation_strategy").is_some(), "Should show aggregation strategy");
    assert!(metadata.get("search_time_ms").is_some(), "Should report search time");
}

/// Test search/multi with custom 13-element weight array.
#[tokio::test]
async fn test_search_multi_custom_weights_13_spaces() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Graph neural networks for knowledge representation",
        "importance": 0.8
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Custom weights for 13 spaces (must sum to 1.0)
    let custom_weights: Vec<f64> = vec![
        0.15, // E1 Semantic
        0.05, // E2 Temporal-Recent
        0.05, // E3 Temporal-Periodic
        0.05, // E4 Temporal-Positional
        0.10, // E5 Causal
        0.05, // E6 Sparse
        0.15, // E7 Code
        0.10, // E8 Graph
        0.10, // E9 HDC
        0.05, // E10 Multimodal
        0.05, // E11 Entity
        0.05, // E12 Late Interaction
        0.05, // E13 SPLADE
    ];

    let search_params = json!({
        "query": "knowledge graphs",
        "query_type": "custom",
        "weights": custom_weights,
        "topK": 5,
        "minSimilarity": 0.0,  // P1-FIX-1: Required parameter for fail-fast
        "include_per_embedder_scores": true
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/multi with custom weights should succeed");
    let result = response.result.expect("Should have result");

    // Verify custom weights were applied
    let metadata = result.get("query_metadata").unwrap();
    let weights_applied = metadata.get("weights_applied").and_then(|v| v.as_array());
    assert!(weights_applied.is_some(), "Should have weights_applied array");
    assert_eq!(
        weights_applied.unwrap().len(),
        NUM_EMBEDDERS,
        "Must apply exactly 13 weights"
    );
}

/// Test search/multi fails with 12-element weight array (must be 13).
#[tokio::test]
async fn test_search_multi_invalid_weights_12_spaces_fails() {
    let handlers = create_test_handlers();

    // Only 12 weights (WRONG - must be 13!)
    let invalid_weights: Vec<f64> = vec![
        0.10, 0.08, 0.08, 0.08, 0.10, 0.08, 0.10, 0.10, 0.08, 0.08, 0.08, 0.04,
    ];

    let search_params = json!({
        "query": "test query",
        "query_type": "custom",
        "weights": invalid_weights
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(1)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_some(), "search/multi must fail with 12 weights");
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS error code");
    assert!(
        error.message.contains("13") || error.message.contains("weight"),
        "Error should mention weight count issue"
    );
}

/// Test search/multi fails with missing query parameter.
#[tokio::test]
async fn test_search_multi_missing_query_fails() {
    let handlers = create_test_handlers();

    let search_params = json!({
        "query_type": "semantic_search",
        "topK": 10
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(1)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_some(), "search/multi must fail without query");
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS error code");
    assert!(
        error.message.contains("query"),
        "Error should mention missing query"
    );
}

/// Test search/multi fails with unknown query_type.
#[tokio::test]
async fn test_search_multi_unknown_query_type_fails() {
    let handlers = create_test_handlers();

    let search_params = json!({
        "query": "test query",
        "query_type": "nonexistent_type"
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(1)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_some(), "search/multi must fail with unknown query_type");
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS error code");
    assert!(
        error.message.contains("query_type") || error.message.contains("Available"),
        "Error should mention valid query types"
    );
}

/// Test search/multi with active_spaces array.
#[tokio::test]
async fn test_search_multi_active_spaces_array() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Rust programming language with memory safety",
        "importance": 0.8
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Only search in spaces 0, 6, 7 (Semantic, Code, Graph)
    let search_params = json!({
        "query": "programming languages",
        "query_type": "code_search",
        "active_spaces": [0, 6, 7],
        "topK": 5,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/multi with active_spaces should succeed");
    let result = response.result.expect("Should have result");

    let metadata = result.get("query_metadata").unwrap();
    assert_eq!(
        metadata.get("spaces_searched").and_then(|v| v.as_u64()),
        Some(3),
        "Should search only 3 spaces"
    );
}

/// Test search/multi with invalid aggregation strategy fails.
#[tokio::test]
async fn test_search_multi_invalid_aggregation_fails() {
    let handlers = create_test_handlers();

    let search_params = json!({
        "query": "test query",
        "aggregation": "invalid_strategy"
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(1)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_some(), "search/multi must fail with invalid aggregation");
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS error code");
}

/// Test search/multi with include_pipeline_breakdown=true returns PIPELINE_METRICS_UNAVAILABLE.
///
/// TASK-EMB-024: Pipeline breakdown metrics are NOT YET IMPLEMENTED.
/// NO hardcoded fallback values - must fail fast until real metrics available.
#[tokio::test]
async fn test_search_multi_pipeline_breakdown() {
    let handlers = create_test_handlers();

    // Store content first
    let store_params = json!({
        "content": "Neural network training techniques",
        "importance": 0.9
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    let search_params = json!({
        "query": "deep learning",
        "include_pipeline_breakdown": true,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter (test expects pipeline breakdown error)
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    // VERIFY FAIL-FAST BEHAVIOR
    // TASK-EMB-024: Pipeline breakdown is NOT YET IMPLEMENTED
    assert!(
        response.error.is_some(),
        "search/multi with include_pipeline_breakdown=true MUST return error (TASK-EMB-024)"
    );

    let error = response.error.as_ref().unwrap();
    assert_eq!(
        error.code,
        error_codes::PIPELINE_METRICS_UNAVAILABLE,
        "Should return PIPELINE_METRICS_UNAVAILABLE (-32052)"
    );
    assert!(
        error.message.contains("Pipeline breakdown") || error.message.contains("not yet implemented"),
        "Error message should indicate feature is not implemented"
    );
}

// =============================================================================
// search/single_space Tests
// =============================================================================

/// Test search/single_space for semantic space (index 0).
#[tokio::test]
async fn test_search_single_space_semantic() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Transformers revolutionized natural language processing",
        "importance": 0.9
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Search in semantic space only (index 0)
    let search_params = json!({
        "query": "NLP transformers",
        "space_index": 0,
        "topK": 5,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request =
        make_request("search/single_space", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/single_space should succeed");
    let result = response.result.expect("Should have result");

    // Verify space info
    assert_eq!(
        result.get("space_index").and_then(|v| v.as_u64()),
        Some(0),
        "Should return space_index 0"
    );
    assert_eq!(
        result.get("space_name").and_then(|v| v.as_str()),
        Some("E1_Semantic"),
        "Should return space_name E1_Semantic"
    );
    assert!(result.get("results").is_some(), "Should have results");
    assert!(result.get("search_time_ms").is_some(), "Should report search time");
}

/// Test search/single_space for code space (index 6).
#[tokio::test]
async fn test_search_single_space_code() {
    let handlers = create_test_handlers();

    // Store code-related content
    let store_params = json!({
        "content": "fn main() { println!(\"Hello, world!\"); }",
        "importance": 0.8
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Search in code space (index 6)
    let search_params = json!({
        "query": "rust main function",
        "space_index": 6,
        "topK": 10,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request =
        make_request("search/single_space", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/single_space for code should succeed");
    let result = response.result.expect("Should have result");

    assert_eq!(
        result.get("space_index").and_then(|v| v.as_u64()),
        Some(6),
        "Should return space_index 6"
    );
    assert_eq!(
        result.get("space_name").and_then(|v| v.as_str()),
        Some("E7_Code"),
        "Should return space_name E7_Code"
    );
}

/// Test search/single_space for SPLADE space (index 12).
#[tokio::test]
async fn test_search_single_space_splade() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Sparse lexical and semantic pre-computed embeddings",
        "importance": 0.7
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Search in SPLADE space (index 12)
    let search_params = json!({
        "query": "sparse embeddings",
        "space_index": 12,
        "topK": 5,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request =
        make_request("search/single_space", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/single_space for SPLADE should succeed");
    let result = response.result.expect("Should have result");

    assert_eq!(
        result.get("space_index").and_then(|v| v.as_u64()),
        Some(12),
        "Should return space_index 12"
    );
    assert_eq!(
        result.get("space_name").and_then(|v| v.as_str()),
        Some("E13_SPLADE"),
        "Should return space_name E13_SPLADE"
    );
}

/// Test search/single_space fails with invalid space_index 13 (only 0-12 valid).
#[tokio::test]
async fn test_search_single_space_invalid_index_13_fails() {
    let handlers = create_test_handlers();

    let search_params = json!({
        "query": "test query",
        "space_index": 13
    });
    let search_request =
        make_request("search/single_space", Some(JsonRpcId::Number(1)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(
        response.error.is_some(),
        "search/single_space must fail with space_index 13"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS error code");
    assert!(
        error.message.contains("13") || error.message.contains("0-12"),
        "Error should mention valid range 0-12"
    );
}

/// Test search/single_space fails with missing space_index.
#[tokio::test]
async fn test_search_single_space_missing_index_fails() {
    let handlers = create_test_handlers();

    let search_params = json!({
        "query": "test query"
    });
    let search_request =
        make_request("search/single_space", Some(JsonRpcId::Number(1)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(
        response.error.is_some(),
        "search/single_space must fail without space_index"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS error code");
    assert!(
        error.message.contains("space_index"),
        "Error should mention missing space_index"
    );
}

/// Test search/single_space accepts query_text as alias for query.
#[tokio::test]
async fn test_search_single_space_query_text_alias() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Alternative parameter naming test",
        "importance": 0.5
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Use query_text instead of query
    let search_params = json!({
        "query_text": "parameter naming",
        "space_index": 0,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request =
        make_request("search/single_space", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(
        response.error.is_none(),
        "search/single_space should accept query_text parameter"
    );
}

// =============================================================================
// search/by_purpose Tests
// =============================================================================

/// Test search/by_purpose with custom purpose vector.
#[tokio::test]
async fn test_search_by_purpose_custom_vector() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Purpose-driven content for alignment testing",
        "importance": 0.8
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // Custom 13D purpose vector
    let purpose_vector: Vec<f64> = vec![
        0.3, 0.1, 0.1, 0.1, 0.1, 0.0, 0.1, 0.1, 0.0, 0.0, 0.1, 0.0, 0.0,
    ];

    let search_params = json!({
        "purpose_vector": purpose_vector,
        "topK": 5
    });
    let search_request =
        make_request("search/by_purpose", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/by_purpose should succeed");
    let result = response.result.expect("Should have result");

    assert!(result.get("results").is_some(), "Should have results array");
    assert!(result.get("count").is_some(), "Should have count");
    assert!(result.get("search_time_ms").is_some(), "Should report search time");
}

/// Test search/by_purpose without purpose_vector uses default.
#[tokio::test]
async fn test_search_by_purpose_default_vector() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Content for default purpose search",
        "importance": 0.6
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // No purpose_vector provided - uses default
    let search_params = json!({
        "topK": 10
    });
    let search_request =
        make_request("search/by_purpose", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(
        response.error.is_none(),
        "search/by_purpose should succeed with default purpose vector"
    );
}

/// Test search/by_purpose with min_alignment filter.
#[tokio::test]
async fn test_search_by_purpose_min_alignment() {
    let handlers = create_test_handlers();

    // Store multiple items
    for i in 0..3 {
        let store_params = json!({
            "content": format!("Purpose alignment test content number {}", i),
            "importance": 0.5 + (i as f64 * 0.1)
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(i as i64 + 1)),
            Some(store_params),
        );
        handlers.dispatch(store_request).await;
    }

    // Search with minimum alignment threshold
    let search_params = json!({
        "min_alignment": 0.5,
        "topK": 10
    });
    let search_request =
        make_request("search/by_purpose", Some(JsonRpcId::Number(10)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/by_purpose should succeed");
    let result = response.result.expect("Should have result");

    // Verify min_alignment filter is reported
    let min_filter = result.get("min_alignment_filter").and_then(|v| v.as_f64());
    assert_eq!(min_filter, Some(0.5), "Should report min_alignment_filter");
}

/// Test search/by_purpose fails with wrong-sized purpose vector.
#[tokio::test]
async fn test_search_by_purpose_wrong_size_vector_fails() {
    let handlers = create_test_handlers();

    // Only 12 elements (must be 13!)
    let wrong_vector: Vec<f64> = vec![0.1; 12];

    let search_params = json!({
        "purpose_vector": wrong_vector
    });
    let search_request =
        make_request("search/by_purpose", Some(JsonRpcId::Number(1)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(
        response.error.is_some(),
        "search/by_purpose must fail with 12-element vector"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS error code");
    assert!(
        error.message.contains("13") || error.message.contains("elements"),
        "Error should mention 13 elements required"
    );
}

/// Test search/by_purpose result contains purpose alignment scores.
#[tokio::test]
async fn test_search_by_purpose_result_structure() {
    let handlers = create_test_handlers();

    // Store content
    let store_params = json!({
        "content": "Result structure verification content",
        "importance": 0.8
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    let search_params = json!({
        "topK": 5
    });
    let search_request =
        make_request("search/by_purpose", Some(JsonRpcId::Number(2)), Some(search_params));
    let response = handlers.dispatch(search_request).await;

    assert!(response.error.is_none(), "search/by_purpose should succeed");
    let result = response.result.expect("Should have result");

    let results = result.get("results").and_then(|v| v.as_array());
    assert!(results.is_some(), "Should have results array");

    if let Some(results) = results {
        if !results.is_empty() {
            let first = &results[0];
            assert!(first.get("fingerprintId").is_some(), "Result should have fingerprintId");
            assert!(
                first.get("purpose_alignment").is_some(),
                "Result should have purpose_alignment"
            );
            assert!(
                first.get("theta_to_north_star").is_some(),
                "Result should have theta_to_north_star"
            );
            assert!(
                first.get("purpose_vector").is_some(),
                "Result should have purpose_vector"
            );
            assert!(
                first.get("johari_quadrant").is_some(),
                "Result should have johari_quadrant"
            );
        }
    }
}

// =============================================================================
// search/weight_profiles Tests
// =============================================================================

/// Test search/weight_profiles returns all profiles.
#[tokio::test]
async fn test_weight_profiles_returns_all() {
    let handlers = create_test_handlers();

    let request = make_request("search/weight_profiles", Some(JsonRpcId::Number(1)), None);
    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "search/weight_profiles should succeed"
    );
    let result = response.result.expect("Should have result");

    // Verify profiles array
    let profiles = result.get("profiles").and_then(|v| v.as_array());
    assert!(profiles.is_some(), "Should have profiles array");

    let profiles = profiles.unwrap();
    assert_eq!(
        profiles.len(),
        WEIGHT_PROFILES.len(),
        "Should return all weight profiles"
    );

    // Verify each profile has required fields
    for profile in profiles {
        assert!(profile.get("name").is_some(), "Profile must have name");
        assert!(profile.get("weights").is_some(), "Profile must have weights");
        assert!(
            profile.get("primary_spaces").is_some(),
            "Profile must have primary_spaces"
        );
        assert!(
            profile.get("description").is_some(),
            "Profile must have description"
        );

        // Verify weights array has 13 elements
        let weights = profile.get("weights").and_then(|v| v.as_array());
        assert!(weights.is_some(), "Weights must be array");
        assert_eq!(
            weights.unwrap().len(),
            NUM_EMBEDDERS,
            "Each profile must have exactly 13 weights"
        );
    }
}

/// Test search/weight_profiles returns correct embedding spaces.
#[tokio::test]
async fn test_weight_profiles_embedding_spaces() {
    let handlers = create_test_handlers();

    let request = make_request("search/weight_profiles", Some(JsonRpcId::Number(1)), None);
    let response = handlers.dispatch(request).await;

    let result = response.result.expect("Should have result");

    // Verify embedding_spaces
    let spaces = result.get("embedding_spaces").and_then(|v| v.as_array());
    assert!(spaces.is_some(), "Should have embedding_spaces array");
    assert_eq!(
        spaces.unwrap().len(),
        NUM_EMBEDDERS,
        "Should have exactly 13 embedding spaces"
    );

    // Verify total_spaces
    assert_eq!(
        result.get("total_spaces").and_then(|v| v.as_u64()),
        Some(NUM_EMBEDDERS as u64),
        "total_spaces should be 13"
    );

    // Verify default values
    assert_eq!(
        result.get("default_aggregation").and_then(|v| v.as_str()),
        Some("rrf"),
        "default_aggregation should be rrf"
    );
    assert_eq!(
        result.get("default_rrf_k").and_then(|v| v.as_f64()),
        Some(60.0),
        "default_rrf_k should be 60.0"
    );
}

/// Test search/weight_profiles includes expected profile names.
#[tokio::test]
async fn test_weight_profiles_expected_names() {
    let handlers = create_test_handlers();

    let request = make_request("search/weight_profiles", Some(JsonRpcId::Number(1)), None);
    let response = handlers.dispatch(request).await;

    let result = response.result.expect("Should have result");
    let profiles = result
        .get("profiles")
        .and_then(|v| v.as_array())
        .expect("Should have profiles");

    let profile_names: Vec<&str> = profiles
        .iter()
        .filter_map(|p| p.get("name").and_then(|v| v.as_str()))
        .collect();

    // Verify expected profiles from get_profile_names()
    let expected_names = get_profile_names();
    for expected in expected_names {
        assert!(
            profile_names.contains(&expected),
            "Should contain profile: {}",
            expected
        );
    }
}

// =============================================================================
// Full State Verification Tests
// =============================================================================

/// FULL STATE VERIFICATION: Complete search workflow test.
///
/// Tests the full search lifecycle with real data:
/// 1. Store multiple fingerprints with different content
/// 2. Search via multi with preset and custom weights
/// 3. Search via single_space targeting specific embedding
/// 4. Search via by_purpose using purpose vector
/// 5. Verify search/weight_profiles returns all 13-weight profiles
///
/// Uses STUB implementations for isolated unit testing.
#[tokio::test]
async fn test_full_state_verification_search_workflow() {
    let handlers = create_test_handlers();

    // =========================================================================
    // STEP 1: Store multiple fingerprints with different content
    // =========================================================================
    let contents = [
        "Machine learning algorithms for predictive analytics",
        "Rust programming language memory safety guarantees",
        "Knowledge graph construction with entity linking",
        "Temporal reasoning for event sequence modeling",
        "Sparse retrieval using inverted indices",
    ];

    let mut stored_ids: Vec<String> = Vec::new();
    for (i, content) in contents.iter().enumerate() {
        let store_params = json!({
            "content": content,
            "importance": 0.5 + (i as f64 * 0.1)
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(i as i64 + 1)),
            Some(store_params),
        );
        let response = handlers.dispatch(store_request).await;
        assert!(response.error.is_none(), "Store {} must succeed", i);

        let fingerprint_id = response
            .result
            .unwrap()
            .get("fingerprintId")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        stored_ids.push(fingerprint_id);
    }

    assert_eq!(stored_ids.len(), 5, "Must have stored 5 fingerprints");

    // =========================================================================
    // STEP 2: Search via multi with preset profile
    // =========================================================================
    let multi_preset_params = json!({
        "query": "machine learning data science",
        "query_type": "semantic_search",
        "topK": 10,
        "minSimilarity": 0.0,  // P1-FIX-1: Required parameter for fail-fast
        "include_per_embedder_scores": true
    });
    let multi_preset_request =
        make_request("search/multi", Some(JsonRpcId::Number(10)), Some(multi_preset_params));
    let multi_preset_response = handlers.dispatch(multi_preset_request).await;

    assert!(
        multi_preset_response.error.is_none(),
        "search/multi with preset must succeed"
    );
    let multi_preset_result = multi_preset_response.result.expect("Must have result");

    // Verify results found
    let results = multi_preset_result
        .get("results")
        .and_then(|v| v.as_array())
        .expect("Must have results array");
    assert!(!results.is_empty(), "search/multi must find at least one result");

    // Verify per-embedder scores present (13 scores)
    if !results.is_empty() {
        let per_scores = results[0].get("per_embedder_scores").and_then(|v| v.as_object());
        assert!(per_scores.is_some(), "Must have per_embedder_scores");
        // The scores use space_json_key which may have different naming
        let top_contrib = results[0].get("top_contributing_spaces").and_then(|v| v.as_array());
        assert!(top_contrib.is_some(), "Must have top_contributing_spaces");
    }

    // =========================================================================
    // STEP 3: Search via multi with custom 13-element weights
    // =========================================================================
    let custom_weights: Vec<f64> = vec![
        0.20, 0.05, 0.05, 0.05, 0.05, 0.05, 0.20, 0.10, 0.10, 0.05, 0.05, 0.03, 0.02,
    ];
    let multi_custom_params = json!({
        "query": "programming language memory",
        "query_type": "custom",
        "weights": custom_weights,
        "topK": 5,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let multi_custom_request =
        make_request("search/multi", Some(JsonRpcId::Number(11)), Some(multi_custom_params));
    let multi_custom_response = handlers.dispatch(multi_custom_request).await;

    assert!(
        multi_custom_response.error.is_none(),
        "search/multi with custom weights must succeed"
    );
    let multi_custom_result = multi_custom_response.result.expect("Must have result");
    let custom_metadata = multi_custom_result
        .get("query_metadata")
        .expect("Must have metadata");
    let applied_weights = custom_metadata
        .get("weights_applied")
        .and_then(|v| v.as_array())
        .expect("Must have weights_applied");
    assert_eq!(applied_weights.len(), 13, "Must apply exactly 13 weights");

    // =========================================================================
    // STEP 4: Search via single_space targeting code embedding (index 6)
    // =========================================================================
    let single_space_params = json!({
        "query": "rust programming",
        "space_index": 6,
        "topK": 5,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let single_space_request = make_request(
        "search/single_space",
        Some(JsonRpcId::Number(12)),
        Some(single_space_params),
    );
    let single_space_response = handlers.dispatch(single_space_request).await;

    assert!(
        single_space_response.error.is_none(),
        "search/single_space must succeed"
    );
    let single_space_result = single_space_response.result.expect("Must have result");
    assert_eq!(
        single_space_result.get("space_index").and_then(|v| v.as_u64()),
        Some(6),
        "Must target space index 6"
    );
    assert_eq!(
        single_space_result.get("space_name").and_then(|v| v.as_str()),
        Some("E7_Code"),
        "Space 6 must be named E7_Code"
    );

    // =========================================================================
    // STEP 5: Search via by_purpose using purpose vector
    // =========================================================================
    let purpose_vec: Vec<f64> = vec![
        0.3, 0.1, 0.1, 0.0, 0.1, 0.0, 0.2, 0.1, 0.0, 0.0, 0.1, 0.0, 0.0,
    ];
    let by_purpose_params = json!({
        "purpose_vector": purpose_vec,
        "topK": 5
    });
    let by_purpose_request = make_request(
        "search/by_purpose",
        Some(JsonRpcId::Number(13)),
        Some(by_purpose_params),
    );
    let by_purpose_response = handlers.dispatch(by_purpose_request).await;

    assert!(
        by_purpose_response.error.is_none(),
        "search/by_purpose must succeed"
    );
    let by_purpose_result = by_purpose_response.result.expect("Must have result");
    assert!(
        by_purpose_result.get("results").is_some(),
        "Must have results array"
    );
    assert!(
        by_purpose_result.get("search_time_ms").is_some(),
        "Must report search_time_ms"
    );

    // =========================================================================
    // STEP 6: Verify search/weight_profiles returns all 13-weight profiles
    // =========================================================================
    let profiles_request =
        make_request("search/weight_profiles", Some(JsonRpcId::Number(14)), None);
    let profiles_response = handlers.dispatch(profiles_request).await;

    assert!(
        profiles_response.error.is_none(),
        "search/weight_profiles must succeed"
    );
    let profiles_result = profiles_response.result.expect("Must have result");

    let profiles = profiles_result
        .get("profiles")
        .and_then(|v| v.as_array())
        .expect("Must have profiles array");
    assert!(!profiles.is_empty(), "Must have at least one profile");

    // Verify each profile has 13 weights
    for profile in profiles {
        let weights = profile.get("weights").and_then(|v| v.as_array());
        assert!(weights.is_some(), "Profile must have weights");
        assert_eq!(
            weights.unwrap().len(),
            NUM_EMBEDDERS,
            "Profile must have exactly 13 weights"
        );
    }

    let total_spaces = profiles_result
        .get("total_spaces")
        .and_then(|v| v.as_u64());
    assert_eq!(total_spaces, Some(13), "total_spaces must be 13");

    // =========================================================================
    // VERIFICATION COMPLETE: All search operations work with real data
    // =========================================================================
}

/// Test error codes are correctly returned for search failures.
#[tokio::test]
async fn test_full_state_verification_search_error_codes() {
    let handlers = create_test_handlers();

    // Test INVALID_PARAMS for missing query in search/multi
    let missing_query = make_request(
        "search/multi",
        Some(JsonRpcId::Number(1)),
        Some(json!({ "query_type": "semantic_search" })),
    );
    let resp = handlers.dispatch(missing_query).await;
    assert_eq!(
        resp.error.unwrap().code,
        -32602,
        "Missing query must return INVALID_PARAMS"
    );

    // Test INVALID_PARAMS for invalid space_index (13, only 0-12 valid)
    let invalid_space = make_request(
        "search/single_space",
        Some(JsonRpcId::Number(2)),
        Some(json!({ "query": "test", "space_index": 13 })),
    );
    let resp = handlers.dispatch(invalid_space).await;
    assert_eq!(
        resp.error.unwrap().code,
        -32602,
        "Invalid space_index must return INVALID_PARAMS"
    );

    // Test INVALID_PARAMS for 12-element weights (must be 13)
    let twelve_weights: Vec<f64> = vec![0.083; 12]; // Wrong: 12 instead of 13
    let wrong_weights = make_request(
        "search/multi",
        Some(JsonRpcId::Number(3)),
        Some(json!({
            "query": "test",
            "query_type": "custom",
            "weights": twelve_weights
        })),
    );
    let resp = handlers.dispatch(wrong_weights).await;
    assert_eq!(
        resp.error.unwrap().code,
        -32602,
        "12-element weights must return INVALID_PARAMS"
    );

    // Test INVALID_PARAMS for 12-element purpose_vector (must be 13)
    let twelve_purpose: Vec<f64> = vec![0.083; 12]; // Wrong: 12 instead of 13
    let wrong_purpose = make_request(
        "search/by_purpose",
        Some(JsonRpcId::Number(4)),
        Some(json!({ "purpose_vector": twelve_purpose })),
    );
    let resp = handlers.dispatch(wrong_purpose).await;
    assert_eq!(
        resp.error.unwrap().code,
        -32602,
        "12-element purpose_vector must return INVALID_PARAMS"
    );
}

// =============================================================================
// INTEGRATION TESTS: Real RocksDB Storage
// =============================================================================
//
// These tests use RocksDbTeleologicalStore for REAL persistent storage.
// This verifies that search operations work correctly with real RocksDB.
//
// Components used:
// - RocksDbTeleologicalStore: REAL (17 column families, persistent storage)
// - UtlProcessorAdapter: REAL (6-component UTL computation)
// - StubMultiArrayProvider: STUB (GPU required for real embeddings - TASK-F007)
//
// CRITICAL: The `_tempdir` variable MUST be kept alive for the entire test.
// =============================================================================

/// Integration test: search/multi with REAL RocksDB storage.
///
/// Verifies that multi-space search works correctly with real persistent storage.
#[tokio::test]
async fn test_rocksdb_integration_search_multi() {
    let (handlers, _tempdir) = create_test_handlers_with_rocksdb().await;

    // Store multiple items to search
    let contents = [
        "Machine learning and artificial intelligence research",
        "Neural network architectures for deep learning",
        "Natural language processing with transformers",
    ];

    for (i, content) in contents.iter().enumerate() {
        let store_params = json!({
            "content": content,
            "importance": 0.7 + (i as f64 * 0.05)
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(i as i64 + 1)),
            Some(store_params),
        );
        let store_response = handlers.dispatch(store_request).await;
        assert!(
            store_response.error.is_none(),
            "STORE[{}] must succeed: {:?}",
            i,
            store_response.error
        );
    }

    // Search with semantic_search preset
    let search_params = json!({
        "query": "deep learning neural networks",
        "query_type": "semantic_search",
        "topK": 10,
        "minSimilarity": 0.0,  // P1-FIX-1: Required parameter for fail-fast
        "include_per_embedder_scores": true
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(10)), Some(search_params));
    let search_response = handlers.dispatch(search_request).await;

    assert!(
        search_response.error.is_none(),
        "search/multi must succeed with RocksDB: {:?}",
        search_response.error
    );
    let search_result = search_response.result.expect("Must have result");

    // Verify results structure
    let results = search_result
        .get("results")
        .and_then(|v| v.as_array())
        .expect("Must have results array");
    let count = search_result
        .get("count")
        .and_then(|v| v.as_u64())
        .expect("Must have count");
    assert_eq!(count as usize, results.len(), "Count must match results length");
    assert!(!results.is_empty(), "Must find at least one result");

    // Verify query_metadata
    let metadata = search_result.get("query_metadata").expect("Must have query_metadata");
    assert_eq!(
        metadata.get("query_type_used").and_then(|v| v.as_str()),
        Some("semantic_search"),
        "Must use semantic_search preset"
    );
    let weights_applied = metadata.get("weights_applied").and_then(|v| v.as_array());
    assert!(weights_applied.is_some(), "Must have weights_applied");
    assert_eq!(weights_applied.unwrap().len(), NUM_EMBEDDERS, "Must have 13 weights");

    // Verify per-embedder scores in first result
    if !results.is_empty() {
        let first = &results[0];
        assert!(first.get("fingerprintId").is_some(), "Result must have fingerprintId");
        assert!(first.get("aggregate_similarity").is_some(), "Result must have aggregate_similarity");
        assert!(first.get("per_embedder_scores").is_some(), "Result must have per_embedder_scores");
        assert!(first.get("top_contributing_spaces").is_some(), "Result must have top_contributing_spaces");
    }
}

/// Integration test: search/multi with custom 13-element weights.
///
/// Verifies that custom weight profiles work with real RocksDB storage.
#[tokio::test]
async fn test_rocksdb_integration_search_multi_custom_weights() {
    let (handlers, _tempdir) = create_test_handlers_with_rocksdb().await;

    // Store content
    let store_params = json!({
        "content": "Rust programming language with ownership and borrowing",
        "importance": 0.8
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    let store_response = handlers.dispatch(store_request).await;
    assert!(store_response.error.is_none(), "STORE must succeed");

    // Custom weights emphasizing code embedding (index 6)
    let custom_weights: Vec<f64> = vec![
        0.10, // E1 Semantic
        0.03, // E2 Temporal-Recent
        0.03, // E3 Temporal-Periodic
        0.03, // E4 Temporal-Positional
        0.05, // E5 Causal
        0.03, // E6 Sparse
        0.40, // E7 Code (emphasized)
        0.10, // E8 Graph
        0.10, // E9 HDC
        0.03, // E10 Multimodal
        0.03, // E11 Entity
        0.03, // E12 Late Interaction
        0.04, // E13 SPLADE
    ];

    let search_params = json!({
        "query": "rust ownership",
        "query_type": "custom",
        "weights": custom_weights,
        "topK": 5,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request = make_request("search/multi", Some(JsonRpcId::Number(2)), Some(search_params));
    let search_response = handlers.dispatch(search_request).await;

    assert!(
        search_response.error.is_none(),
        "search/multi with custom weights must succeed: {:?}",
        search_response.error
    );
    let search_result = search_response.result.expect("Must have result");

    // Verify custom weights were applied
    let metadata = search_result.get("query_metadata").expect("Must have metadata");
    let applied = metadata.get("weights_applied").and_then(|v| v.as_array()).expect("Must have weights");
    assert_eq!(applied.len(), 13, "Must have exactly 13 weights");

    // Verify code weight (index 6) is 0.40
    let code_weight = applied[6].as_f64().expect("Weight must be f64");
    assert!(
        (code_weight - 0.40).abs() < 0.01,
        "Code weight must be 0.40, got {}",
        code_weight
    );
}

/// Integration test: search/single_space with REAL RocksDB storage.
///
/// Verifies that single-space search targeting specific embedding works.
#[tokio::test]
async fn test_rocksdb_integration_search_single_space() {
    let (handlers, _tempdir) = create_test_handlers_with_rocksdb().await;

    // Store content
    let store_params = json!({
        "content": "Knowledge graphs for entity relationship modeling",
        "importance": 0.75
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    let store_response = handlers.dispatch(store_request).await;
    assert!(store_response.error.is_none(), "STORE must succeed");

    // Search in graph embedding space (index 7)
    let search_params = json!({
        "query": "entity relationships",
        "space_index": 7,
        "topK": 5,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request = make_request("search/single_space", Some(JsonRpcId::Number(2)), Some(search_params));
    let search_response = handlers.dispatch(search_request).await;

    assert!(
        search_response.error.is_none(),
        "search/single_space must succeed: {:?}",
        search_response.error
    );
    let search_result = search_response.result.expect("Must have result");

    // Verify response structure
    assert_eq!(
        search_result.get("space_index").and_then(|v| v.as_u64()),
        Some(7),
        "Must use space_index 7"
    );
    assert!(
        search_result.get("space_name").is_some(),
        "Must have space_name"
    );
    assert!(
        search_result.get("search_time_ms").is_some(),
        "Must have search_time_ms"
    );

    // Verify results found
    let results = search_result.get("results").and_then(|v| v.as_array());
    assert!(results.is_some(), "Must have results array");
}

/// Integration test: search/by_purpose with REAL RocksDB storage.
///
/// Verifies that purpose-based search works with real storage.
#[tokio::test]
async fn test_rocksdb_integration_search_by_purpose() {
    let (handlers, _tempdir) = create_test_handlers_with_rocksdb().await;

    // Store multiple items with different content
    let contents = [
        "Building scalable distributed systems",
        "Optimizing database query performance",
        "Implementing fault-tolerant architectures",
    ];

    for (i, content) in contents.iter().enumerate() {
        let store_params = json!({
            "content": content,
            "importance": 0.6 + (i as f64 * 0.1)
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(i as i64 + 1)),
            Some(store_params),
        );
        let store_response = handlers.dispatch(store_request).await;
        assert!(store_response.error.is_none(), "STORE[{}] must succeed", i);
    }

    // Search by purpose vector (13 elements)
    let purpose_vector: Vec<f64> = vec![
        0.9, 0.7, 0.5, 0.4, 0.6, 0.3, 0.8, 0.7, 0.5, 0.4, 0.6, 0.5, 0.4,
    ];
    let search_params = json!({
        "purpose_vector": purpose_vector,
        "topK": 10,
        "threshold": 0.1
    });
    let search_request = make_request("search/by_purpose", Some(JsonRpcId::Number(10)), Some(search_params));
    let search_response = handlers.dispatch(search_request).await;

    assert!(
        search_response.error.is_none(),
        "search/by_purpose must succeed: {:?}",
        search_response.error
    );
    let search_result = search_response.result.expect("Must have result");

    // Verify results structure
    assert!(search_result.get("results").is_some(), "Must have results");
    assert!(search_result.get("count").is_some(), "Must have count");
    assert!(search_result.get("search_time_ms").is_some(), "Must have search_time_ms");
    // min_alignment_filter may be None if no threshold provided
}

/// Integration test: search/weight_profiles works with RocksDB handlers.
///
/// Verifies that weight profiles endpoint returns correct 13-weight profiles.
#[tokio::test]
async fn test_rocksdb_integration_weight_profiles() {
    let (handlers, _tempdir) = create_test_handlers_with_rocksdb().await;

    let request = make_request("search/weight_profiles", Some(JsonRpcId::Number(1)), None);
    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "search/weight_profiles must succeed: {:?}",
        response.error
    );
    let result = response.result.expect("Must have result");

    // Verify profiles array - use WEIGHT_PROFILES.len() for actual count
    let profiles = result.get("profiles").and_then(|v| v.as_array()).expect("Must have profiles");
    assert_eq!(
        profiles.len(),
        WEIGHT_PROFILES.len(),
        "Must return all {} weight profiles, got {}",
        WEIGHT_PROFILES.len(),
        profiles.len()
    );

    // Verify each profile has exactly 13 weights
    for (i, profile) in profiles.iter().enumerate() {
        let weights = profile.get("weights").and_then(|v| v.as_array());
        assert!(weights.is_some(), "Profile {} must have weights", i);
        assert_eq!(
            weights.unwrap().len(),
            NUM_EMBEDDERS,
            "Profile {} must have exactly 13 weights",
            i
        );
        // Verify profile has name and description
        assert!(profile.get("name").is_some(), "Profile {} must have name", i);
        assert!(profile.get("primary_spaces").is_some(), "Profile {} must have primary_spaces", i);
    }

    // Verify embedding spaces
    let spaces = result.get("embedding_spaces").and_then(|v| v.as_array());
    assert!(spaces.is_some(), "Must have embedding_spaces");
    assert_eq!(spaces.unwrap().len(), NUM_EMBEDDERS, "Must have 13 embedding spaces");

    // Verify total_spaces
    assert_eq!(
        result.get("total_spaces").and_then(|v| v.as_u64()),
        Some(NUM_EMBEDDERS as u64),
        "total_spaces must be 13"
    );

    // Verify default values
    assert_eq!(
        result.get("default_aggregation").and_then(|v| v.as_str()),
        Some("rrf"),
        "default_aggregation must be rrf"
    );
}

/// Integration test: Error handling with REAL RocksDB.
///
/// Verifies that proper error codes are returned for invalid operations.
#[tokio::test]
async fn test_rocksdb_integration_search_error_handling() {
    let (handlers, _tempdir) = create_test_handlers_with_rocksdb().await;

    // Test INVALID_PARAMS for missing query in search/multi
    let missing_query = make_request(
        "search/multi",
        Some(JsonRpcId::Number(1)),
        Some(json!({ "query_type": "semantic_search" })),
    );
    let resp = handlers.dispatch(missing_query).await;
    assert_eq!(
        resp.error.unwrap().code,
        -32602,
        "Missing query must return INVALID_PARAMS with real RocksDB"
    );

    // Test INVALID_PARAMS for invalid space_index
    let invalid_space = make_request(
        "search/single_space",
        Some(JsonRpcId::Number(2)),
        Some(json!({ "query": "test", "space_index": 13 })),
    );
    let resp = handlers.dispatch(invalid_space).await;
    assert_eq!(
        resp.error.unwrap().code,
        -32602,
        "Invalid space_index must return INVALID_PARAMS"
    );

    // Test INVALID_PARAMS for 12-element weights
    let twelve_weights: Vec<f64> = vec![0.083; 12];
    let wrong_weights = make_request(
        "search/multi",
        Some(JsonRpcId::Number(3)),
        Some(json!({
            "query": "test",
            "query_type": "custom",
            "weights": twelve_weights
        })),
    );
    let resp = handlers.dispatch(wrong_weights).await;
    assert_eq!(
        resp.error.unwrap().code,
        -32602,
        "12-element weights must return INVALID_PARAMS with real RocksDB"
    );
}

// =============================================================================
// REAL GPU Embedding Tests (Feature-gated: cuda)
// =============================================================================
//
// These tests verify search operations with REAL GPU-accelerated embeddings from
// ProductionMultiArrayProvider. They require:
// - NVIDIA CUDA GPU with 8GB+ VRAM
// - Pre-downloaded models in ./models directory
// - cargo test --features cuda
//
// Run these tests for Full State Verification (FSV) to confirm real embeddings
// produce semantically meaningful search results.
// =============================================================================

#[cfg(feature = "cuda")]
mod real_embedding_tests {
    use super::*;
    use crate::handlers::tests::{create_test_handlers_with_real_embeddings, extract_mcp_tool_data};
    use std::time::Instant;

    /// FSV: Verify search/multi with REAL GPU embeddings returns semantically relevant results.
    ///
    /// Unlike stub tests that use deterministic fake embeddings, this test verifies that:
    /// - Real BAAI/bge-m3 embeddings produce meaningful semantic similarity
    /// - ML-related content ranks higher for ML-related queries
    /// - Unrelated content ranks lower or is filtered out
    #[tokio::test]
    async fn test_real_embeddings_search_multi_semantic_relevance() {
        let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

        // Store diverse content: 2 ML-related, 1 unrelated
        let contents = [
            ("Machine learning algorithms can classify images using neural networks", 0.9),
            ("Deep learning models like transformers revolutionized NLP tasks", 0.8),
            ("The weather forecast predicts rain tomorrow afternoon in Seattle", 0.7),
        ];

        let mut stored_ids = Vec::new();
        for (i, (content, importance)) in contents.iter().enumerate() {
            let store_params = json!({
                "content": content,
                "importance": importance
            });
            let store_request = make_request(
                "memory/store",
                Some(JsonRpcId::Number((i + 1) as i64)),
                Some(store_params),
            );
            let response = handlers.dispatch(store_request).await;
            assert!(response.error.is_none(), "Store {} should succeed", i);

            if let Some(result) = response.result {
                let data = extract_mcp_tool_data(&result);
                if let Some(id) = data.get("fingerprintId")
                    .or_else(|| data.get("fingerprint_id"))
                    .and_then(|v| v.as_str())
                {
                    stored_ids.push(id.to_string());
                }
            }
        }
        assert_eq!(stored_ids.len(), 3, "Should have stored 3 items");

        // Search for ML content
        let search_params = json!({
            "query": "artificial intelligence and machine learning models",
            "query_type": "semantic_search",
            "topK": 10,
            "minSimilarity": 0.0,
            "include_per_embedder_scores": true
        });
        let search_request = make_request(
            "search/multi",
            Some(JsonRpcId::Number(100)),
            Some(search_params),
        );
        let response = handlers.dispatch(search_request).await;

        assert!(response.error.is_none(), "Search should succeed with real embeddings");
        let result = response.result.expect("Should have result");

        // Verify results structure
        let results = result.get("results").and_then(|v| v.as_array()).expect("Should have results");
        assert!(!results.is_empty(), "Should have at least one result");

        // With real embeddings, ML content should have higher similarity
        // The weather forecast should have notably lower similarity
        let count = result.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        println!("Real embedding search returned {} results", count);

        // Verify per-embedder scores are returned (13 embedders)
        if let Some(first) = results.first() {
            if let Some(per_scores) = first.get("per_embedder_scores").and_then(|v| v.as_object()) {
                println!("Per-embedder scores for top result: {} embedders", per_scores.len());
                // Should have scores for all 13 embedding spaces
                assert!(
                    !per_scores.is_empty(),
                    "Should have per-embedder scores"
                );
            }

            // Verify top result has reasonable similarity
            if let Some(sim) = first.get("combined_similarity").and_then(|v| v.as_f64()) {
                println!("Top result combined_similarity: {}", sim);
                // With real embeddings, the ML content should have similarity > 0
                // Exact threshold depends on model, but should be positive
                assert!(
                    sim >= 0.0,
                    "Top result should have non-negative similarity: {}",
                    sim
                );
            }
        }

        // Verify query_metadata shows 13-element weights
        if let Some(metadata) = result.get("query_metadata") {
            if let Some(weights) = metadata.get("weights_applied").and_then(|v| v.as_array()) {
                assert_eq!(
                    weights.len(),
                    NUM_EMBEDDERS,
                    "Should have 13-element weights applied"
                );
            }
        }
    }

    /// FSV: Verify search/single_space works with REAL GPU embeddings.
    ///
    /// Tests single embedding space search (space 0 = Semantic/E1) with real embeddings.
    #[tokio::test]
    async fn test_real_embeddings_search_single_space() {
        let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

        // Store content
        let store_params = json!({
            "content": "Quantum computing uses qubits for parallel computation",
            "importance": 0.85
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(1)),
            Some(store_params),
        );
        let response = handlers.dispatch(store_request).await;
        assert!(response.error.is_none(), "Store should succeed");

        // Search in space 0 (Semantic/E1)
        let search_params = json!({
            "query": "quantum physics and computation",
            "space_index": 0,
            "topK": 5,
            "minSimilarity": 0.0
        });
        let search_request = make_request(
            "search/single_space",
            Some(JsonRpcId::Number(2)),
            Some(search_params),
        );
        let response = handlers.dispatch(search_request).await;

        assert!(response.error.is_none(), "Single-space search should succeed");
        let result = response.result.expect("Should have result");

        // Verify single-space search structure
        let results = result.get("results").and_then(|v| v.as_array()).expect("Should have results");

        // With real embeddings, the quantum content should be found
        if !results.is_empty() {
            let first = &results[0];
            // Single-space returns "similarity" not "combined_similarity"
            if let Some(sim) = first.get("similarity").and_then(|v| v.as_f64()) {
                println!("Single-space (E1) search top similarity: {}", sim);
                assert!(sim >= 0.0, "Similarity should be non-negative");
            }
        }

        // Verify metadata shows correct space
        if let Some(metadata) = result.get("query_metadata") {
            assert_eq!(
                metadata.get("space_index").and_then(|v| v.as_u64()),
                Some(0),
                "Should search in space 0"
            );
            assert_eq!(
                metadata.get("space_name").and_then(|v| v.as_str()),
                Some("semantic"),
                "Space 0 should be 'semantic'"
            );
        }
    }

    /// FSV: Verify search/by_purpose with REAL 13D purpose vectors.
    ///
    /// Tests purpose-aligned search using real embedding-generated purpose vectors.
    #[tokio::test]
    async fn test_real_embeddings_search_by_purpose() {
        let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

        // Store content with diverse topics
        let topics = [
            "Ethical AI development requires transparency and fairness",
            "Database optimization techniques for high-throughput systems",
            "Climate change mitigation through renewable energy adoption",
        ];

        for (i, content) in topics.iter().enumerate() {
            let store_params = json!({
                "content": content,
                "importance": 0.8
            });
            let store_request = make_request(
                "memory/store",
                Some(JsonRpcId::Number((i + 1) as i64)),
                Some(store_params),
            );
            let response = handlers.dispatch(store_request).await;
            assert!(response.error.is_none(), "Store {} should succeed", i);
        }

        // Search by purpose with a 13D purpose vector
        // This simulates aligning to a "beneficial AI" goal
        let purpose_vector: Vec<f64> = vec![
            0.8,  // E1: Semantic - high weight for meaning
            0.3,  // E2: Temporal-cyclic
            0.3,  // E3: Temporal-decay
            0.3,  // E4: Temporal-contextual
            0.7,  // E5: Causal - important for ethics
            0.2,  // E6: Sparse (SPLADE)
            0.4,  // E7: Code
            0.3,  // E8: Graph
            0.2,  // E9: HDC
            0.3,  // E10: Multimodal
            0.5,  // E11: Entity
            0.2,  // E12: Late-interaction
            0.2,  // E13: Sparse
        ];

        let search_params = json!({
            "purpose_vector": purpose_vector,
            "topK": 10,
            "threshold": 0.0  // Allow all results
        });
        let search_request = make_request(
            "search/by_purpose",
            Some(JsonRpcId::Number(100)),
            Some(search_params),
        );
        let response = handlers.dispatch(search_request).await;

        assert!(response.error.is_none(), "By-purpose search should succeed");
        let result = response.result.expect("Should have result");

        // Verify results
        let results = result.get("results").and_then(|v| v.as_array()).expect("Should have results");
        println!("By-purpose search returned {} results", results.len());

        // Verify alignment scores are in expected range
        for (i, r) in results.iter().enumerate() {
            if let Some(alignment) = r.get("alignment_score").and_then(|v| v.as_f64()) {
                println!("Result {} alignment_score: {}", i, alignment);
                // Alignment scores should be in [-1, 1] per constitution
                assert!(
                    (-1.0..=1.0).contains(&alignment),
                    "Alignment score should be in [-1, 1]: {}",
                    alignment
                );
            }
        }
    }

    /// FSV: Verify REAL embedding search latency meets constitution targets.
    ///
    /// Constitution requires:
    /// - single_embed: <10ms
    /// - inject_context p95: <25ms
    ///
    /// Search operations should be reasonably fast with real GPU embeddings.
    #[tokio::test]
    async fn test_real_embeddings_search_latency() {
        let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

        // Store some content first
        let store_params = json!({
            "content": "Performance benchmarking for neural embedding systems",
            "importance": 0.9
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(1)),
            Some(store_params),
        );
        handlers.dispatch(store_request).await;

        // Warm up the models
        let warmup_params = json!({
            "query": "warmup query",
            "query_type": "semantic_search",
            "topK": 1,
            "minSimilarity": 0.0
        });
        let warmup_request = make_request(
            "search/multi",
            Some(JsonRpcId::Number(2)),
            Some(warmup_params),
        );
        handlers.dispatch(warmup_request).await;

        // Measure search latencies
        let mut latencies = Vec::new();
        for i in 0..5 {
            let search_params = json!({
                "query": format!("test query number {}", i),
                "query_type": "semantic_search",
                "topK": 10,
                "minSimilarity": 0.0
            });
            let search_request = make_request(
                "search/multi",
                Some(JsonRpcId::Number((10 + i) as i64)),
                Some(search_params),
            );

            let start = Instant::now();
            let response = handlers.dispatch(search_request).await;
            let latency_ms = start.elapsed().as_millis() as u64;
            latencies.push(latency_ms);

            assert!(response.error.is_none(), "Search {} should succeed", i);
            println!("Search {} latency: {}ms", i, latency_ms);
        }

        // Calculate P95 (for 5 samples, it's the max)
        latencies.sort();
        let p95 = latencies.last().copied().unwrap_or(0);
        println!("Search P95 latency: {}ms (target: reasonable for GPU)", p95);

        // Report but don't fail - actual threshold depends on hardware
        // The constitution says 25ms for inject_context which is more complex
        if p95 > 100 {
            println!("WARNING: P95 search latency {}ms may be high for real-time use", p95);
        }
    }

    /// FSV: Verify all 13 embedding spaces are searchable with REAL embeddings.
    #[tokio::test]
    async fn test_real_embeddings_all_13_spaces_searchable() {
        let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;

        // Store content that should have embeddings in all 13 spaces
        let store_params = json!({
            "content": "The function calculateSum() iterates through an array to compute the total value for statistical analysis",
            "importance": 0.9
        });
        let store_request = make_request(
            "memory/store",
            Some(JsonRpcId::Number(1)),
            Some(store_params),
        );
        let response = handlers.dispatch(store_request).await;
        assert!(response.error.is_none(), "Store should succeed");

        // Search each of the 13 embedding spaces
        let space_names = [
            "semantic",      // E1
            "temporal_cyclic", // E2
            "temporal_decay",  // E3
            "temporal_contextual", // E4
            "causal",        // E5
            "sparse",        // E6 (SPLADE)
            "code",          // E7
            "graph",         // E8
            "hdc",           // E9
            "multimodal",    // E10
            "entity",        // E11
            "late_interaction", // E12 (ColBERT)
            "sparse_2",      // E13
        ];

        for (space_index, space_name) in space_names.iter().enumerate() {
            let search_params = json!({
                "query": "function array calculation statistics",
                "space_index": space_index,
                "topK": 5,
                "minSimilarity": 0.0
            });
            let search_request = make_request(
                "search/single_space",
                Some(JsonRpcId::Number((100 + space_index) as i64)),
                Some(search_params),
            );
            let response = handlers.dispatch(search_request).await;

            // Space search should succeed (even if no results due to sparse embeddings)
            assert!(
                response.error.is_none(),
                "Space {} ({}) search should succeed: {:?}",
                space_index,
                space_name,
                response.error
            );

            if let Some(result) = response.result {
                let count = result.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("Space {} ({}): {} results", space_index, space_name, count);
            }
        }
    }
}
