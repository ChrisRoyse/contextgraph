//! GPU Embedding Verification Tests
//!
//! CRITICAL: These tests verify REAL GPU embeddings per constitution.yaml:
//! - ARCH-08: "CUDA GPU required for production - no CPU fallbacks"
//! - AP-07: "No CPU fallback in production"
//! - AP-71: "Dream NREM/REM returning stubs forbidden"
//! - ARCH-05: "All 13 embedders required - missing embedder is fatal error"
//!
//! These tests MUST:
//! - Use `create_test_handlers_with_real_embeddings_store_access()` for GPU embeddings
//! - Verify all 13 embeddings have correct dimensions
//! - Verify embeddings contain non-zero values (not stub data)
//! - Verify search returns real similarity scores
//!
//! These tests MUST NOT:
//! - Use CPU fallbacks
//! - Use mock/stub data
//! - Be ignored (#[ignore])
//! - Pass silently with stub data

#![cfg(feature = "cuda")]

use serde_json::json;
use uuid::Uuid;

use context_graph_core::types::fingerprint::{
    E10_DIM, E11_DIM, E12_TOKEN_DIM, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E7_DIM, E8_DIM,
    E9_DIM, NUM_EMBEDDERS,
};

use crate::protocol::{JsonRpcId, JsonRpcRequest};

use super::create_test_handlers_with_real_embeddings_store_access;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a tools/call request for MCP API.
fn make_tools_call_request(
    tool_name: &str,
    id: i64,
    arguments: serde_json::Value,
) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(id)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": tool_name,
            "arguments": arguments
        })),
    }
}

/// Extract fingerprint ID from tools/call response.
fn extract_fingerprint_id(result: &serde_json::Value) -> Option<Uuid> {
    result["content"]
        .as_array()?
        .first()?
        .get("text")?
        .as_str()
        .and_then(|text| {
            serde_json::from_str::<serde_json::Value>(text)
                .ok()?
                .get("fingerprintId")?
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
        })
}

/// Extract parsed JSON from MCP tool response content.
fn extract_response_json(result: &serde_json::Value) -> serde_json::Value {
    let text = result["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .expect("Response must have content[0].text");
    serde_json::from_str(text).expect("Content text must be valid JSON")
}

/// Check if an embedding is non-zero (has at least one non-zero value).
fn has_nonzero_values(embedding: &[f32]) -> bool {
    embedding.iter().any(|&v| v.abs() > f32::EPSILON)
}

/// Compute L2 norm (magnitude) of an embedding.
fn l2_norm(embedding: &[f32]) -> f32 {
    embedding.iter().map(|v| v * v).sum::<f32>().sqrt()
}

// ============================================================================
// Test 1: inject_context produces all 13 embeddings with correct dimensions
// ============================================================================

/// Verify inject_context with REAL GPU embeddings produces correct dimensions.
///
/// Per ARCH-05: All 13 embedders MUST be present.
/// Per ARCH-08: CUDA GPU required - no CPU fallbacks.
#[tokio::test]
async fn test_inject_context_produces_all_13_embeddings_with_gpu() {
    println!("\n================================================================================");
    println!("GPU EMBEDDING VERIFICATION: inject_context produces all 13 embeddings");
    println!("================================================================================\n");

    // Create handlers with REAL GPU embeddings
    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // Verify store starts empty
    let before_count = store.count().await.expect("count should succeed");
    assert_eq!(before_count, 0, "Store must start empty");
    println!("BEFORE: store.count() = {}", before_count);

    // Execute inject_context with meaningful content
    let content =
        "Rust's ownership system prevents data races at compile time through borrowing rules.";
    println!("\nEXECUTE: inject_context with content:\n  \"{}\"", content);

    let request = make_tools_call_request(
        "inject_context",
        1,
        json!({
            "content": content,
            "rationale": "Testing GPU embedding generation",
            "importance": 0.9
        }),
    );

    let response = handlers.dispatch(request).await;

    // Verify no error in response
    assert!(
        response.error.is_none(),
        "inject_context should not return JSON-RPC error. Got: {:?}",
        response.error
    );

    let result = response.result.expect("Response must have result");

    // Check for tool-level error
    if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
        if is_error {
            let error_text = result["content"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("Unknown error");
            panic!(
                "inject_context returned tool error (GPU embedding likely failed): {}",
                error_text
            );
        }
    }

    // Extract fingerprint ID
    let fingerprint_id =
        extract_fingerprint_id(&result).expect("Response must contain fingerprintId");
    println!("\nRESULT: fingerprintId = {}", fingerprint_id);

    // Verify response metadata
    let response_json = extract_response_json(&result);
    let embedder_count = response_json["embedderCount"]
        .as_u64()
        .expect("embedderCount must be present");
    assert_eq!(
        embedder_count, NUM_EMBEDDERS as u64,
        "embedderCount must be {} (all 13 embedders)",
        NUM_EMBEDDERS
    );
    println!("VERIFIED: embedderCount = {}", embedder_count);

    let latency_ms = response_json["embeddingLatencyMs"]
        .as_u64()
        .expect("embeddingLatencyMs must be present");
    println!("VERIFIED: embeddingLatencyMs = {}ms", latency_ms);

    // GPU embeddings should be fast - warn if suspiciously slow
    if latency_ms > 5000 {
        println!(
            "WARNING: Embedding latency {}ms is high - possible CPU fallback?",
            latency_ms
        );
    }

    // =========================================================================
    // CRITICAL: Direct store verification of embedding dimensions
    // =========================================================================
    println!("\n--- DIRECT STORE VERIFICATION ---");

    let stored_fp = store
        .retrieve(fingerprint_id)
        .await
        .expect("retrieve should succeed")
        .expect("Fingerprint MUST exist in store");

    // Verify all 13 embedding dimensions
    println!("\nEmbedding Dimensions (ARCH-05 compliance):");

    // E1: Semantic (1024D)
    let e1_dim = stored_fp.semantic.e1_semantic.len();
    println!(
        "  E1  (Semantic):         {} dims (expected {})",
        e1_dim, E1_DIM
    );
    assert_eq!(e1_dim, E1_DIM, "E1 must be {}D", E1_DIM);

    // E2: Temporal-Recent (512D)
    let e2_dim = stored_fp.semantic.e2_temporal_recent.len();
    println!(
        "  E2  (Temporal-Recent):  {} dims (expected {})",
        e2_dim, E2_DIM
    );
    assert_eq!(e2_dim, E2_DIM, "E2 must be {}D", E2_DIM);

    // E3: Temporal-Periodic (512D)
    let e3_dim = stored_fp.semantic.e3_temporal_periodic.len();
    println!(
        "  E3  (Temporal-Periodic):{} dims (expected {})",
        e3_dim, E3_DIM
    );
    assert_eq!(e3_dim, E3_DIM, "E3 must be {}D", E3_DIM);

    // E4: Temporal-Positional (512D)
    let e4_dim = stored_fp.semantic.e4_temporal_positional.len();
    println!(
        "  E4  (Temporal-Position):{} dims (expected {})",
        e4_dim, E4_DIM
    );
    assert_eq!(e4_dim, E4_DIM, "E4 must be {}D", E4_DIM);

    // E5: Causal (768D)
    let e5_dim = stored_fp.semantic.e5_causal.len();
    println!(
        "  E5  (Causal):           {} dims (expected {})",
        e5_dim, E5_DIM
    );
    assert_eq!(e5_dim, E5_DIM, "E5 must be {}D", E5_DIM);

    // E6: Sparse (variable active, 30522 vocab)
    let e6_active = stored_fp.semantic.e6_sparse.indices.len();
    println!("  E6  (Sparse):           {} active entries", e6_active);
    // Sparse can have 0 active entries for some content

    // E7: Code (1536D)
    let e7_dim = stored_fp.semantic.e7_code.len();
    println!(
        "  E7  (Code):             {} dims (expected {})",
        e7_dim, E7_DIM
    );
    assert_eq!(e7_dim, E7_DIM, "E7 must be {}D", E7_DIM);

    // E8: Graph (384D)
    let e8_dim = stored_fp.semantic.e8_graph.len();
    println!(
        "  E8  (Graph):            {} dims (expected {})",
        e8_dim, E8_DIM
    );
    assert_eq!(e8_dim, E8_DIM, "E8 must be {}D", E8_DIM);

    // E9: HDC (1024D projected)
    let e9_dim = stored_fp.semantic.e9_hdc.len();
    println!(
        "  E9  (HDC):              {} dims (expected {})",
        e9_dim, E9_DIM
    );
    assert_eq!(e9_dim, E9_DIM, "E9 must be {}D", E9_DIM);

    // E10: Multimodal (768D)
    let e10_dim = stored_fp.semantic.e10_multimodal.len();
    println!(
        "  E10 (Multimodal):       {} dims (expected {})",
        e10_dim, E10_DIM
    );
    assert_eq!(e10_dim, E10_DIM, "E10 must be {}D", E10_DIM);

    // E11: Entity (384D)
    let e11_dim = stored_fp.semantic.e11_entity.len();
    println!(
        "  E11 (Entity):           {} dims (expected {})",
        e11_dim, E11_DIM
    );
    assert_eq!(e11_dim, E11_DIM, "E11 must be {}D", E11_DIM);

    // E12: Late-Interaction (128D per token, variable tokens)
    let e12_tokens = stored_fp.semantic.e12_late_interaction.len();
    println!("  E12 (Late-Interaction): {} tokens", e12_tokens);
    if e12_tokens > 0 {
        let first_token_dim = stored_fp.semantic.e12_late_interaction[0].len();
        println!(
            "      First token dim:    {} (expected {})",
            first_token_dim, E12_TOKEN_DIM
        );
        assert_eq!(
            first_token_dim, E12_TOKEN_DIM,
            "E12 tokens must be {}D",
            E12_TOKEN_DIM
        );
    }

    // E13: SPLADE (variable active, 30522 vocab)
    let e13_active = stored_fp.semantic.e13_splade.indices.len();
    println!("  E13 (SPLADE):           {} active entries", e13_active);

    println!("\nALL 13 EMBEDDING DIMENSIONS VERIFIED");
    println!("================================================================================\n");
}

// ============================================================================
// Test 2: Embeddings contain non-zero values (not stub data)
// ============================================================================

/// Verify GPU embeddings contain actual computed values, not zeros.
///
/// This catches stub providers that return zero vectors.
/// Real embeddings from GPU have non-trivial magnitudes.
#[tokio::test]
async fn test_gpu_embeddings_are_nonzero() {
    println!("\n================================================================================");
    println!("GPU EMBEDDING VERIFICATION: Embeddings contain non-zero values");
    println!("================================================================================\n");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // Store content
    let content = "Machine learning models learn patterns from training data.";
    let request = make_tools_call_request(
        "inject_context",
        1,
        json!({
            "content": content,
            "rationale": "Testing non-zero embedding values"
        }),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none(), "inject_context should succeed");

    let result = response.result.expect("Must have result");
    let fingerprint_id = extract_fingerprint_id(&result).expect("Must have fingerprintId");

    // Retrieve and verify non-zero embeddings
    let stored_fp = store
        .retrieve(fingerprint_id)
        .await
        .expect("retrieve succeeds")
        .expect("Fingerprint exists");

    println!("Checking for non-zero embedding values:\n");

    // E1: Semantic - MUST have non-zero values for meaningful content
    let e1_nonzero = has_nonzero_values(&stored_fp.semantic.e1_semantic);
    let e1_norm = l2_norm(&stored_fp.semantic.e1_semantic);
    println!(
        "  E1  (Semantic):    non-zero={}, L2 norm={:.6}",
        e1_nonzero, e1_norm
    );
    assert!(
        e1_nonzero,
        "E1 (Semantic) MUST have non-zero values for real content"
    );
    assert!(
        e1_norm > 0.01,
        "E1 L2 norm {} too small - likely stub data",
        e1_norm
    );

    // E5: Causal - should have values for content with causal language
    let e5_nonzero = has_nonzero_values(&stored_fp.semantic.e5_causal);
    let e5_norm = l2_norm(&stored_fp.semantic.e5_causal);
    println!(
        "  E5  (Causal):      non-zero={}, L2 norm={:.6}",
        e5_nonzero, e5_norm
    );
    assert!(e5_nonzero, "E5 (Causal) MUST have non-zero values");

    // E7: Code - should work on technical content
    let e7_nonzero = has_nonzero_values(&stored_fp.semantic.e7_code);
    let e7_norm = l2_norm(&stored_fp.semantic.e7_code);
    println!(
        "  E7  (Code):        non-zero={}, L2 norm={:.6}",
        e7_nonzero, e7_norm
    );
    assert!(e7_nonzero, "E7 (Code) MUST have non-zero values");

    // E8: Graph - structural embedding
    let e8_nonzero = has_nonzero_values(&stored_fp.semantic.e8_graph);
    let e8_norm = l2_norm(&stored_fp.semantic.e8_graph);
    println!(
        "  E8  (Graph):       non-zero={}, L2 norm={:.6}",
        e8_nonzero, e8_norm
    );
    assert!(e8_nonzero, "E8 (Graph) MUST have non-zero values");

    // E10: Multimodal - intent embedding
    let e10_nonzero = has_nonzero_values(&stored_fp.semantic.e10_multimodal);
    let e10_norm = l2_norm(&stored_fp.semantic.e10_multimodal);
    println!(
        "  E10 (Multimodal):  non-zero={}, L2 norm={:.6}",
        e10_nonzero, e10_norm
    );
    assert!(e10_nonzero, "E10 (Multimodal) MUST have non-zero values");

    // E11: Entity - entity embedding
    let e11_nonzero = has_nonzero_values(&stored_fp.semantic.e11_entity);
    let e11_norm = l2_norm(&stored_fp.semantic.e11_entity);
    println!(
        "  E11 (Entity):      non-zero={}, L2 norm={:.6}",
        e11_nonzero, e11_norm
    );
    assert!(e11_nonzero, "E11 (Entity) MUST have non-zero values");

    // Count non-zero dense embeddings
    let dense_embeddings = [
        ("E1", &stored_fp.semantic.e1_semantic),
        ("E2", &stored_fp.semantic.e2_temporal_recent),
        ("E3", &stored_fp.semantic.e3_temporal_periodic),
        ("E4", &stored_fp.semantic.e4_temporal_positional),
        ("E5", &stored_fp.semantic.e5_causal),
        ("E7", &stored_fp.semantic.e7_code),
        ("E8", &stored_fp.semantic.e8_graph),
        ("E9", &stored_fp.semantic.e9_hdc),
        ("E10", &stored_fp.semantic.e10_multimodal),
        ("E11", &stored_fp.semantic.e11_entity),
    ];

    let nonzero_count = dense_embeddings
        .iter()
        .filter(|(_, emb)| has_nonzero_values(emb))
        .count();

    println!(
        "\nSummary: {}/{} dense embeddings have non-zero values",
        nonzero_count,
        dense_embeddings.len()
    );

    // At minimum, semantic embeddings (E1, E5, E7, E8, E10, E11) must be non-zero
    assert!(
        nonzero_count >= 6,
        "At least 6 semantic embeddings must be non-zero. Got {}. Likely stub data!",
        nonzero_count
    );

    println!("\nNON-ZERO EMBEDDING VALUES VERIFIED");
    println!("================================================================================\n");
}

// ============================================================================
// Test 3: search_graph returns real similarity scores
// ============================================================================

/// Verify search_graph returns meaningful similarity scores, not zeros.
///
/// With real embeddings, similar content should have high similarity.
/// Dissimilar content should have lower similarity.
#[tokio::test]
async fn test_search_graph_returns_real_similarity_scores() {
    println!("\n================================================================================");
    println!("GPU EMBEDDING VERIFICATION: search_graph returns real similarity scores");
    println!("================================================================================\n");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // Store multiple memories with varying semantic similarity
    let contents = [
        "Rust's ownership system prevents data races through borrowing rules.",
        "Rust memory safety is enforced at compile time without garbage collection.",
        "Python uses dynamic typing and garbage collection for memory management.",
        "The capital of France is Paris, a beautiful city known for the Eiffel Tower.",
    ];

    println!("Storing {} memories...", contents.len());
    for (i, content) in contents.iter().enumerate() {
        let request =
            make_tools_call_request("store_memory", i as i64 + 1, json!({ "content": content }));
        let response = handlers.dispatch(request).await;
        assert!(
            response.error.is_none(),
            "store_memory {} should succeed",
            i
        );
        println!(
            "  [{}] Stored: \"{}...\"",
            i,
            &content[..50.min(content.len())]
        );
    }

    // Verify count
    let count = store.count().await.expect("count succeeds");
    assert_eq!(
        count,
        contents.len(),
        "All {} memories must be stored",
        contents.len()
    );

    // Search for "Rust memory safety" - should match first two highly
    println!("\nSearching for: \"Rust memory safety and ownership\"");
    let search_request = make_tools_call_request(
        "search_graph",
        100,
        json!({
            "query": "Rust memory safety and ownership",
            "topK": 10
        }),
    );

    let search_response = handlers.dispatch(search_request).await;
    assert!(
        search_response.error.is_none(),
        "search_graph should succeed"
    );

    let search_result = search_response.result.expect("Must have result");

    // Check for tool error
    if let Some(is_error) = search_result.get("isError").and_then(|v| v.as_bool()) {
        if is_error {
            let error_text = search_result["content"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("Unknown error");
            panic!("search_graph returned tool error: {}", error_text);
        }
    }

    let search_json = extract_response_json(&search_result);
    let results = search_json["results"]
        .as_array()
        .expect("results must be array");
    let result_count = search_json["count"]
        .as_u64()
        .expect("count must be present");

    println!("\nSearch results (count={}):", result_count);

    let mut similarities: Vec<f64> = Vec::new();
    for (i, r) in results.iter().enumerate() {
        let similarity = r["similarity"].as_f64().expect("similarity must be f64");
        let dominant = r["dominantEmbedder"].as_str().unwrap_or("?");
        let fp_id = r["fingerprintId"].as_str().unwrap_or("?");

        println!(
            "  [{}] similarity={:.6}, dominant={}, id={}",
            i, similarity, dominant, fp_id
        );
        similarities.push(similarity);
    }

    // Verify we got results
    assert!(!results.is_empty(), "Search must return results");

    // Verify similarities are real (not all zeros)
    let nonzero_similarities = similarities.iter().filter(|&&s| s > 0.001).count();
    println!(
        "\nNon-zero similarities: {}/{}",
        nonzero_similarities,
        similarities.len()
    );

    assert!(
        nonzero_similarities > 0,
        "At least one similarity score must be non-zero. Got all zeros - likely stub embeddings!"
    );

    // Verify similarity ordering makes sense (if we have multiple results)
    if similarities.len() >= 2 {
        // Results should be sorted by similarity descending
        for i in 1..similarities.len() {
            assert!(
                similarities[i - 1] >= similarities[i] - 0.0001, // small epsilon for float comparison
                "Results must be sorted by similarity descending. Got {} before {}",
                similarities[i - 1],
                similarities[i]
            );
        }
        println!("VERIFIED: Results sorted by similarity (descending)");
    }

    // Top result should have reasonable similarity for semantically similar content
    let top_similarity = similarities[0];
    println!("\nTop similarity score: {:.6}", top_similarity);

    // With real embeddings, similar Rust content should have decent similarity
    // Note: exact threshold depends on the model, but should be > 0.1 for related content
    assert!(
        top_similarity > 0.1,
        "Top similarity {} too low for semantically similar content. Likely stub data!",
        top_similarity
    );

    println!("\nREAL SIMILARITY SCORES VERIFIED");
    println!("================================================================================\n");
}

// ============================================================================
// Test 4: Verify embedding consistency (same content = same embeddings)
// ============================================================================

/// Verify that embedding the same content twice produces consistent results.
///
/// Real GPU embeddings should be deterministic for the same input.
#[tokio::test]
async fn test_embedding_consistency_for_same_content() {
    println!("\n================================================================================");
    println!("GPU EMBEDDING VERIFICATION: Embedding consistency for same content");
    println!("================================================================================\n");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    let content = "Deterministic embeddings should produce the same vectors for the same input.";

    // Store the same content twice
    println!("Storing identical content twice...");

    let request1 = make_tools_call_request("store_memory", 1, json!({ "content": content }));
    let response1 = handlers.dispatch(request1).await;
    assert!(response1.error.is_none(), "First store should succeed");
    let result1 = response1.result.expect("Must have result");
    let id1 = extract_fingerprint_id(&result1).expect("Must have ID");

    let request2 = make_tools_call_request("store_memory", 2, json!({ "content": content }));
    let response2 = handlers.dispatch(request2).await;
    assert!(response2.error.is_none(), "Second store should succeed");
    let result2 = response2.result.expect("Must have result");
    let id2 = extract_fingerprint_id(&result2).expect("Must have ID");

    println!("  ID 1: {}", id1);
    println!("  ID 2: {}", id2);
    assert_ne!(
        id1, id2,
        "Different store operations should produce different IDs"
    );

    // Retrieve both fingerprints
    let fp1 = store
        .retrieve(id1)
        .await
        .expect("retrieve 1")
        .expect("fp1 exists");
    let fp2 = store
        .retrieve(id2)
        .await
        .expect("retrieve 2")
        .expect("fp2 exists");

    // Compare E1 (semantic) embeddings - should be very similar or identical
    println!("\nComparing E1 (Semantic) embeddings...");
    let e1_1 = &fp1.semantic.e1_semantic;
    let e1_2 = &fp2.semantic.e1_semantic;

    // Compute cosine similarity between the two E1 embeddings
    let dot_product: f32 = e1_1.iter().zip(e1_2.iter()).map(|(a, b)| a * b).sum();
    let norm1 = l2_norm(e1_1);
    let norm2 = l2_norm(e1_2);
    let cosine_sim = dot_product / (norm1 * norm2);

    println!("  E1 cosine similarity: {:.6}", cosine_sim);
    println!("  E1 norm 1: {:.6}, norm 2: {:.6}", norm1, norm2);

    // Same content should produce very high similarity (>0.999 for deterministic models)
    // Allow some tolerance for potential floating point differences
    assert!(
        cosine_sim > 0.99,
        "Same content should produce nearly identical embeddings. Got cosine similarity: {}",
        cosine_sim
    );

    // Compare E7 (code) embeddings
    let e7_1 = &fp1.semantic.e7_code;
    let e7_2 = &fp2.semantic.e7_code;
    let e7_dot: f32 = e7_1.iter().zip(e7_2.iter()).map(|(a, b)| a * b).sum();
    let e7_cosine = e7_dot / (l2_norm(e7_1) * l2_norm(e7_2));
    println!("  E7 cosine similarity: {:.6}", e7_cosine);

    assert!(
        e7_cosine > 0.99,
        "E7 embeddings should be consistent. Got: {}",
        e7_cosine
    );

    println!("\nEMBEDDING CONSISTENCY VERIFIED");
    println!("================================================================================\n");
}

// ============================================================================
// Test 5: Verify different content produces different embeddings
// ============================================================================

/// Verify that semantically different content produces different embeddings.
///
/// This ensures the embedding model is actually computing meaningful vectors.
#[tokio::test]
async fn test_different_content_produces_different_embeddings() {
    println!("\n================================================================================");
    println!("GPU EMBEDDING VERIFICATION: Different content produces different embeddings");
    println!("================================================================================\n");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    let content1 = "Rust programming language memory safety without garbage collection.";
    let content2 = "French cuisine and traditional recipes from Provence region.";

    println!("Content 1: \"{}\"", content1);
    println!("Content 2: \"{}\"", content2);

    // Store both
    let request1 = make_tools_call_request("store_memory", 1, json!({ "content": content1 }));
    let response1 = handlers.dispatch(request1).await;
    assert!(response1.error.is_none());
    let id1 = extract_fingerprint_id(&response1.result.as_ref().unwrap()).unwrap();

    let request2 = make_tools_call_request("store_memory", 2, json!({ "content": content2 }));
    let response2 = handlers.dispatch(request2).await;
    assert!(response2.error.is_none());
    let id2 = extract_fingerprint_id(&response2.result.as_ref().unwrap()).unwrap();

    // Retrieve and compare
    let fp1 = store.retrieve(id1).await.unwrap().unwrap();
    let fp2 = store.retrieve(id2).await.unwrap().unwrap();

    // Compute cosine similarity
    let e1_1 = &fp1.semantic.e1_semantic;
    let e1_2 = &fp2.semantic.e1_semantic;
    let dot: f32 = e1_1.iter().zip(e1_2.iter()).map(|(a, b)| a * b).sum();
    let cosine = dot / (l2_norm(e1_1) * l2_norm(e1_2));

    println!(
        "\nE1 (Semantic) cosine similarity between different content: {:.6}",
        cosine
    );

    // Very different content should have lower similarity
    // "Rust programming" vs "French cuisine" should be quite different
    assert!(
        cosine < 0.9,
        "Semantically different content should have lower similarity. Got: {}. \
         This suggests embeddings might be stubs returning similar values for all inputs.",
        cosine
    );

    // But similarity should still be positive (not negative for unrelated content)
    assert!(
        cosine > -0.5,
        "Cosine similarity {} seems unreasonably low",
        cosine
    );

    println!("\nDIFFERENT CONTENT PRODUCES DIFFERENT EMBEDDINGS - VERIFIED");
    println!("================================================================================\n");
}

// ============================================================================
// Test 6: Verify store_memory also produces correct embeddings
// ============================================================================

/// Verify store_memory (alternative to inject_context) also uses real GPU embeddings.
#[tokio::test]
async fn test_store_memory_uses_gpu_embeddings() {
    println!("\n================================================================================");
    println!("GPU EMBEDDING VERIFICATION: store_memory uses GPU embeddings");
    println!("================================================================================\n");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    let content = "Neural networks with backpropagation for deep learning.";

    let request = make_tools_call_request(
        "store_memory",
        1,
        json!({
            "content": content,
            "importance": 0.8
        }),
    );

    let response = handlers.dispatch(request).await;
    assert!(response.error.is_none(), "store_memory should succeed");

    let result = response.result.expect("Must have result");

    // Check for tool error
    if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
        if is_error {
            let error_text = result["content"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("Unknown");
            panic!("store_memory tool error: {}", error_text);
        }
    }

    let response_json = extract_response_json(&result);
    let embedder_count = response_json["embedderCount"]
        .as_u64()
        .expect("embedderCount must be present");

    println!("store_memory embedderCount: {}", embedder_count);
    assert_eq!(
        embedder_count, NUM_EMBEDDERS as u64,
        "Must use all 13 embedders"
    );

    let fingerprint_id = extract_fingerprint_id(&result).expect("Must have ID");
    let stored_fp = store.retrieve(fingerprint_id).await.unwrap().unwrap();

    // Verify E1 has non-zero values
    let e1_nonzero = has_nonzero_values(&stored_fp.semantic.e1_semantic);
    let e1_norm = l2_norm(&stored_fp.semantic.e1_semantic);

    println!("E1 non-zero: {}, L2 norm: {:.6}", e1_nonzero, e1_norm);
    assert!(e1_nonzero, "E1 must have non-zero values");
    assert!(e1_norm > 0.01, "E1 norm {} too small", e1_norm);

    println!("\nstore_memory GPU EMBEDDINGS VERIFIED");
    println!("================================================================================\n");
}
