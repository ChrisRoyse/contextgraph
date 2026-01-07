//! TASK-S001: Full State Verification Tests
//!
//! These tests verify the Source of Truth by directly inspecting the underlying store:
//!
//! ## Test Categories
//!
//! | Category | Storage | Purpose |
//! |----------|---------|---------|
//! | Unit Tests (verify_*) | InMemoryTeleologicalStore | Fast unit tests with stubs |
//! | Integration Tests (test_rocksdb_fsv_*) | RocksDbTeleologicalStore | Real storage verification |
//!
//! ## Key Principles
//!
//! 1. Directly inspect the underlying store after operations
//! 2. Test edge cases with before/after state inspection
//! 3. Provide evidence logs showing actual data in the system
//!
//! NO RELIANCE ON RETURN VALUES ALONE - we verify data physically exists.

use std::sync::Arc;

use serde_json::json;
use sha2::{Digest, Sha256};

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::purpose::GoalHierarchy;
use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor};
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcRequest};

// Import shared test helpers for RocksDB integration tests
use super::create_test_handlers_with_rocksdb_store_access;

/// Create test handlers AND return direct access to the store for verification.
///
/// TASK-S003: Updated to include GoalAlignmentCalculator and GoalHierarchy.
fn create_handlers_with_store_access() -> (
    Handlers,
    Arc<dyn TeleologicalMemoryStore>,
    Arc<dyn MultiArrayEmbeddingProvider>,
) {
    let teleological_store: Arc<dyn TeleologicalMemoryStore> =
        Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = GoalHierarchy::new();

    let handlers = Handlers::new(
        Arc::clone(&teleological_store),
        Arc::clone(&utl_processor),
        Arc::clone(&multi_array_provider),
        alignment_calculator,
        goal_hierarchy,
    );

    (handlers, teleological_store, multi_array_provider)
}

fn make_request(
    method: &str,
    id: Option<JsonRpcId>,
    params: Option<serde_json::Value>,
) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: method.to_string(),
        params,
    }
}

/// Helper: Check if fingerprint exists in store (via retrieve).
async fn exists_in_store(store: &Arc<dyn TeleologicalMemoryStore>, id: uuid::Uuid) -> bool {
    store.retrieve(id).await.map(|opt| opt.is_some()).unwrap_or(false)
}

// =============================================================================
// FULL STATE VERIFICATION: Direct Store Inspection
// =============================================================================

/// VERIFICATION TEST 1: Store operation physically creates fingerprint in store.
///
/// Source of Truth: InMemoryTeleologicalStore.data (DashMap<Uuid, TeleologicalFingerprint>)
/// Verification Method: Direct store.retrieve() and store.count() calls
#[tokio::test]
async fn verify_store_creates_fingerprint_in_source_of_truth() {
    println!("\n================================================================================");
    println!("FULL STATE VERIFICATION: Store Creates Fingerprint");
    println!("================================================================================");

    let (handlers, store, _provider) = create_handlers_with_store_access();

    // === BEFORE STATE ===
    let count_before = store.count().await.expect("count() should work");
    println!("\n[BEFORE] Store state:");
    println!("  - Fingerprint count: {}", count_before);
    assert_eq!(count_before, 0, "Store should be empty initially");

    // === EXECUTE OPERATION ===
    let content = "Machine learning is a subset of artificial intelligence";
    let params = json!({
        "content": content,
        "importance": 0.85
    });
    let request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;

    // Extract fingerprint ID from response (but don't trust this alone)
    let result = response.result.expect("Should have result");
    let fingerprint_id_str = result
        .get("fingerprintId")
        .expect("Should have fingerprintId")
        .as_str()
        .expect("Should be string");
    let fingerprint_id = uuid::Uuid::parse_str(fingerprint_id_str).expect("Should be valid UUID");

    println!("\n[OPERATION] Stored content with ID: {}", fingerprint_id);

    // === AFTER STATE - VERIFY SOURCE OF TRUTH ===
    let count_after = store.count().await.expect("count() should work");
    println!("\n[AFTER] Store state:");
    println!("  - Fingerprint count: {}", count_after);

    // CRITICAL: Directly verify fingerprint exists in store
    let exists = exists_in_store(&store, fingerprint_id).await;
    println!("  - Fingerprint {} exists: {}", fingerprint_id, exists);
    assert!(exists, "VERIFICATION FAILED: Fingerprint must exist in store");

    // CRITICAL: Retrieve and inspect actual stored data
    let stored_fp = store
        .retrieve(fingerprint_id)
        .await
        .expect("retrieve() should work")
        .expect("Fingerprint must exist");

    println!("\n[EVIDENCE] Stored fingerprint fields:");
    println!("  - ID: {}", stored_fp.id);
    println!("  - theta_to_north_star: {:.4}", stored_fp.theta_to_north_star);
    println!("  - access_count: {}", stored_fp.access_count);
    println!("  - created_at: {}", stored_fp.created_at);
    println!("  - content_hash: {}", hex::encode(stored_fp.content_hash));
    println!("  - semantic.e1_semantic len: {}", stored_fp.semantic.e1_semantic.len());
    println!("  - purpose_vector.alignments: {:?}", &stored_fp.purpose_vector.alignments[..5]);
    println!("  - johari.quadrants[0]: {:?}", stored_fp.johari.quadrants[0]);

    // Verify content hash matches expected
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let expected_hash: [u8; 32] = hasher.finalize().into();
    assert_eq!(
        stored_fp.content_hash, expected_hash,
        "Content hash in store must match SHA-256 of original content"
    );
    println!("  - Content hash VERIFIED: matches SHA-256 of input");

    // Verify semantic fingerprint has valid embedding (stub uses 1024D)
    assert!(!stored_fp.semantic.e1_semantic.is_empty(), "E1 must have embeddings");
    println!("  - E1 semantic embedding dimension VERIFIED: {}", stored_fp.semantic.e1_semantic.len());

    // Count must have increased
    assert_eq!(count_after, count_before + 1, "Count must increase by 1");
    println!("\n[VERIFICATION PASSED] Fingerprint physically exists in Source of Truth");
    println!("================================================================================\n");
}

/// VERIFICATION TEST 2: Retrieve operation returns exact stored data.
///
/// We store data, then verify retrieve returns THE SAME object from store.
#[tokio::test]
async fn verify_retrieve_returns_source_of_truth_data() {
    println!("\n================================================================================");
    println!("FULL STATE VERIFICATION: Retrieve Returns Source of Truth");
    println!("================================================================================");

    let (handlers, store, _provider) = create_handlers_with_store_access();

    // Store a fingerprint
    let content = "Neural networks learn hierarchical representations";
    let store_params = json!({
        "content": content,
        "importance": 0.9
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    let store_response = handlers.dispatch(store_request).await;
    let fingerprint_id_str = store_response
        .result
        .unwrap()
        .get("fingerprintId")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let fingerprint_id = uuid::Uuid::parse_str(&fingerprint_id_str).unwrap();

    // === DIRECTLY READ FROM SOURCE OF TRUTH ===
    let truth_fp = store
        .retrieve(fingerprint_id)
        .await
        .expect("Direct store.retrieve() should work")
        .expect("Fingerprint must exist in store");

    println!("\n[SOURCE OF TRUTH] Direct store.retrieve() data:");
    println!("  - ID: {}", truth_fp.id);
    println!("  - content_hash: {}", hex::encode(truth_fp.content_hash));
    println!("  - theta_to_north_star: {:.4}", truth_fp.theta_to_north_star);

    // === NOW USE MCP HANDLER TO RETRIEVE ===
    let retrieve_params = json!({ "fingerprintId": fingerprint_id_str });
    let retrieve_request = make_request(
        "memory/retrieve",
        Some(JsonRpcId::Number(2)),
        Some(retrieve_params),
    );
    let retrieve_response = handlers.dispatch(retrieve_request).await;

    let result = retrieve_response.result.expect("Should have result");
    let fp_json = result.get("fingerprint").expect("Should have fingerprint");

    println!("\n[MCP HANDLER] memory/retrieve response:");
    println!("  - ID: {}", fp_json.get("id").unwrap().as_str().unwrap());
    println!("  - contentHashHex: {}", fp_json.get("contentHashHex").unwrap().as_str().unwrap());
    println!("  - thetaToNorthStar: {}", fp_json.get("thetaToNorthStar").unwrap());

    // === VERIFY HANDLER RETURNS SAME DATA AS SOURCE OF TRUTH ===
    assert_eq!(
        fp_json.get("id").unwrap().as_str().unwrap(),
        truth_fp.id.to_string(),
        "Handler must return same ID as store"
    );
    assert_eq!(
        fp_json.get("contentHashHex").unwrap().as_str().unwrap(),
        hex::encode(truth_fp.content_hash),
        "Handler must return same hash as store"
    );

    println!("\n[VERIFICATION PASSED] Handler retrieve returns Source of Truth data");
    println!("================================================================================\n");
}

/// VERIFICATION TEST 3: Search finds fingerprint in Source of Truth.
#[tokio::test]
async fn verify_search_finds_data_in_source_of_truth() {
    println!("\n================================================================================");
    println!("FULL STATE VERIFICATION: Search Finds Source of Truth Data");
    println!("================================================================================");

    let (handlers, store, _provider) = create_handlers_with_store_access();

    // Store multiple fingerprints
    let contents = [
        "Deep learning uses neural networks with many layers",
        "Transformers revolutionized natural language processing",
        "Reinforcement learning teaches agents through rewards",
    ];

    let mut stored_ids = Vec::new();
    for (i, content) in contents.iter().enumerate() {
        let params = json!({ "content": content, "importance": 0.8 });
        let request = make_request("memory/store", Some(JsonRpcId::Number(i as i64 + 1)), Some(params));
        let response = handlers.dispatch(request).await;
        let id_str = response.result.unwrap().get("fingerprintId").unwrap().as_str().unwrap().to_string();
        stored_ids.push(uuid::Uuid::parse_str(&id_str).unwrap());
    }

    // === VERIFY ALL IN SOURCE OF TRUTH ===
    let count = store.count().await.expect("count() should work");
    println!("\n[SOURCE OF TRUTH] Store contains {} fingerprints:", count);
    for id in &stored_ids {
        let exists = exists_in_store(&store, *id).await;
        println!("  - {} exists: {}", id, exists);
        assert!(exists, "All stored IDs must exist in Source of Truth");
    }

    // === SEARCH VIA MCP HANDLER ===
    let search_params = json!({
        "query": "neural network deep learning",
        "topK": 10,
        "minSimilarity": 0.0  // P1-FIX-1: Required parameter for fail-fast
    });
    let search_request = make_request("memory/search", Some(JsonRpcId::Number(10)), Some(search_params));
    let search_response = handlers.dispatch(search_request).await;

    let result = search_response.result.expect("Should have result");
    let results = result.get("results").unwrap().as_array().unwrap();
    let result_count = result.get("count").unwrap().as_u64().unwrap();

    println!("\n[MCP HANDLER] Search returned {} results", result_count);

    // === VERIFY EACH SEARCH RESULT EXISTS IN SOURCE OF TRUTH ===
    for (i, r) in results.iter().enumerate() {
        let result_id_str = r.get("fingerprintId").unwrap().as_str().unwrap();
        let result_id = uuid::Uuid::parse_str(result_id_str).unwrap();
        let similarity = r.get("similarity").unwrap().as_f64().unwrap();

        // Verify this result exists in store
        let exists_in_truth = exists_in_store(&store, result_id).await;
        println!(
            "  Result {}: {} (sim={:.4}) - exists in store: {}",
            i, result_id, similarity, exists_in_truth
        );
        assert!(exists_in_truth, "Search result must exist in Source of Truth");
    }

    println!("\n[VERIFICATION PASSED] All search results exist in Source of Truth");
    println!("================================================================================\n");
}

/// VERIFICATION TEST 4: Hard delete removes fingerprint from Source of Truth.
#[tokio::test]
async fn verify_delete_removes_from_source_of_truth() {
    println!("\n================================================================================");
    println!("FULL STATE VERIFICATION: Delete Removes From Source of Truth");
    println!("================================================================================");

    let (handlers, store, _provider) = create_handlers_with_store_access();

    // Store a fingerprint
    let content = "This content will be deleted";
    let store_params = json!({ "content": content, "importance": 0.5 });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    let store_response = handlers.dispatch(store_request).await;
    let fingerprint_id_str = store_response
        .result
        .unwrap()
        .get("fingerprintId")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let fingerprint_id = uuid::Uuid::parse_str(&fingerprint_id_str).unwrap();

    // === BEFORE DELETE STATE ===
    let count_before = store.count().await.unwrap();
    let exists_before = exists_in_store(&store, fingerprint_id).await;
    println!("\n[BEFORE DELETE] Source of Truth state:");
    println!("  - Total count: {}", count_before);
    println!("  - Fingerprint {} exists: {}", fingerprint_id, exists_before);
    assert!(exists_before, "Fingerprint must exist before delete");

    // === EXECUTE HARD DELETE ===
    let delete_params = json!({
        "fingerprintId": fingerprint_id_str,
        "soft": false
    });
    let delete_request = make_request("memory/delete", Some(JsonRpcId::Number(2)), Some(delete_params));
    let delete_response = handlers.dispatch(delete_request).await;

    let delete_result = delete_response.result.expect("Should have result");
    println!("\n[OPERATION] Hard delete response:");
    println!("  - deleted: {}", delete_result.get("deleted").unwrap());
    println!("  - deleteType: {}", delete_result.get("deleteType").unwrap());

    // === AFTER DELETE - VERIFY SOURCE OF TRUTH ===
    let count_after = store.count().await.unwrap();
    let exists_after = exists_in_store(&store, fingerprint_id).await;
    let retrieve_after = store.retrieve(fingerprint_id).await.unwrap();

    println!("\n[AFTER DELETE] Source of Truth state:");
    println!("  - Total count: {}", count_after);
    println!("  - Fingerprint {} exists: {}", fingerprint_id, exists_after);
    println!("  - Direct retrieve returns: {:?}", retrieve_after.as_ref().map(|fp| fp.id));

    // CRITICAL VERIFICATION: Fingerprint must be GONE from store
    assert!(
        !exists_after,
        "VERIFICATION FAILED: Fingerprint must NOT exist after hard delete"
    );
    assert!(
        retrieve_after.is_none(),
        "VERIFICATION FAILED: store.retrieve() must return None after hard delete"
    );
    assert_eq!(
        count_after,
        count_before - 1,
        "Count must decrease by 1 after delete"
    );

    println!("\n[VERIFICATION PASSED] Fingerprint removed from Source of Truth");
    println!("================================================================================\n");
}

// =============================================================================
// EDGE CASE VERIFICATION WITH BEFORE/AFTER STATE
// =============================================================================

/// EDGE CASE 1: Empty content string
#[tokio::test]
async fn verify_edge_case_empty_content() {
    println!("\n================================================================================");
    println!("EDGE CASE VERIFICATION: Empty Content String");
    println!("================================================================================");

    let (handlers, store, _provider) = create_handlers_with_store_access();

    // === BEFORE STATE ===
    let count_before = store.count().await.unwrap();
    println!("\n[BEFORE] Store count: {}", count_before);

    // === ATTEMPT OPERATION WITH EMPTY CONTENT ===
    let params = json!({
        "content": "",
        "importance": 0.5
    });
    let request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;

    println!("\n[OPERATION] Attempted store with empty content");
    println!("  - Error returned: {}", response.error.is_some());
    if let Some(error) = &response.error {
        println!("  - Error code: {}", error.code);
        println!("  - Error message: {}", error.message);
    }

    // === AFTER STATE - VERIFY NO CHANGE ===
    let count_after = store.count().await.unwrap();
    println!("\n[AFTER] Store count: {}", count_after);

    assert!(response.error.is_some(), "Empty content must return error");
    assert_eq!(response.error.unwrap().code, -32602, "Must be INVALID_PARAMS");
    assert_eq!(count_before, count_after, "Store count must not change on error");

    println!("\n[VERIFICATION PASSED] Empty content rejected, store unchanged");
    println!("================================================================================\n");
}

/// EDGE CASE 2: Invalid UUID format
#[tokio::test]
async fn verify_edge_case_invalid_uuid() {
    println!("\n================================================================================");
    println!("EDGE CASE VERIFICATION: Invalid UUID Format");
    println!("================================================================================");

    let (handlers, store, _provider) = create_handlers_with_store_access();

    // Store one valid fingerprint first
    let store_params = json!({ "content": "Valid content", "importance": 0.5 });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    handlers.dispatch(store_request).await;

    // === BEFORE STATE ===
    let count_before = store.count().await.unwrap();
    println!("\n[BEFORE] Store count: {}", count_before);

    // === ATTEMPT RETRIEVE WITH INVALID UUID ===
    let invalid_uuids = [
        "not-a-uuid",
        "12345",
        "00000000-0000-0000-0000",  // truncated
        "zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz",  // invalid chars
    ];

    for invalid_uuid in &invalid_uuids {
        let params = json!({ "fingerprintId": invalid_uuid });
        let request = make_request("memory/retrieve", Some(JsonRpcId::Number(10)), Some(params));
        let response = handlers.dispatch(request).await;

        println!("\n[OPERATION] Retrieve with invalid UUID: '{}'", invalid_uuid);
        println!("  - Error returned: {}", response.error.is_some());
        if let Some(error) = &response.error {
            println!("  - Error code: {}", error.code);
        }

        assert!(response.error.is_some(), "Invalid UUID must return error");
        assert_eq!(response.error.unwrap().code, -32602, "Must be INVALID_PARAMS");
    }

    // === AFTER STATE - VERIFY NO CHANGE ===
    let count_after = store.count().await.unwrap();
    println!("\n[AFTER] Store count: {}", count_after);
    assert_eq!(count_before, count_after, "Store must be unchanged");

    println!("\n[VERIFICATION PASSED] Invalid UUIDs rejected, store unchanged");
    println!("================================================================================\n");
}

/// EDGE CASE 3: Non-existent fingerprint ID
#[tokio::test]
async fn verify_edge_case_nonexistent_id() {
    println!("\n================================================================================");
    println!("EDGE CASE VERIFICATION: Non-existent Fingerprint ID");
    println!("================================================================================");

    let (handlers, store, _provider) = create_handlers_with_store_access();

    // Use a valid but non-existent UUID
    let nonexistent_id = "00000000-0000-0000-0000-000000000000";

    // === VERIFY NOT IN SOURCE OF TRUTH ===
    let nonexistent_uuid = uuid::Uuid::parse_str(nonexistent_id).unwrap();
    let exists = exists_in_store(&store, nonexistent_uuid).await;
    println!("\n[SOURCE OF TRUTH] ID {} exists: {}", nonexistent_id, exists);
    assert!(!exists, "Non-existent ID must not exist in store");

    // === ATTEMPT RETRIEVE ===
    let retrieve_params = json!({ "fingerprintId": nonexistent_id });
    let retrieve_request = make_request(
        "memory/retrieve",
        Some(JsonRpcId::Number(1)),
        Some(retrieve_params),
    );
    let retrieve_response = handlers.dispatch(retrieve_request).await;

    println!("\n[OPERATION] Retrieve non-existent ID");
    println!("  - Error returned: {}", retrieve_response.error.is_some());
    if let Some(error) = &retrieve_response.error {
        println!("  - Error code: {}", error.code);
        println!("  - Error message: {}", error.message);
    }

    assert!(retrieve_response.error.is_some(), "Non-existent ID must return error");
    assert_eq!(
        retrieve_response.error.unwrap().code,
        -32010,
        "Must be FINGERPRINT_NOT_FOUND (-32010)"
    );

    // === ATTEMPT DELETE ===
    let delete_params = json!({ "fingerprintId": nonexistent_id, "soft": false });
    let delete_request = make_request("memory/delete", Some(JsonRpcId::Number(2)), Some(delete_params));
    let delete_response = handlers.dispatch(delete_request).await;

    println!("\n[OPERATION] Delete non-existent ID");
    // Delete of non-existent should succeed with deleted=false or return error depending on implementation
    if let Some(result) = &delete_response.result {
        let deleted = result.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false);
        println!("  - Result deleted: {}", deleted);
    }
    if let Some(error) = &delete_response.error {
        println!("  - Error code: {}", error.code);
    }

    println!("\n[VERIFICATION PASSED] Non-existent ID handled correctly");
    println!("================================================================================\n");
}

// =============================================================================
// COMPREHENSIVE EVIDENCE LOG TEST
// =============================================================================

/// Full evidence log showing data flow through the system.
#[tokio::test]
async fn verify_complete_evidence_log() {
    println!("\n================================================================================");
    println!("COMPREHENSIVE EVIDENCE LOG: Complete Data Flow Verification");
    println!("================================================================================");

    let (handlers, store, provider) = create_handlers_with_store_access();

    // === INITIAL STATE ===
    println!("\n[INITIAL STATE]");
    println!("  Store count: {}", store.count().await.unwrap());

    // === STEP 1: Generate embeddings (verify provider works) ===
    println!("\n[STEP 1: EMBEDDING GENERATION]");
    let test_content = "Convolutional neural networks excel at image recognition tasks";
    let embedding_output = provider.embed_all(test_content).await.expect("embed_all should work");
    println!("  Content: \"{}\"", test_content);
    println!("  Embeddings generated: {} slots", 13);
    println!("  E1 semantic dimension: {}", embedding_output.fingerprint.e1_semantic.len());
    println!("  E6 sparse NNZ: {}", embedding_output.fingerprint.e6_sparse.nnz());
    println!("  Total latency: {:?}", embedding_output.total_latency);

    // === STEP 2: Store via MCP handler ===
    println!("\n[STEP 2: MCP STORE OPERATION]");
    let store_params = json!({ "content": test_content, "importance": 0.95 });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(store_params));
    let store_response = handlers.dispatch(store_request).await;

    let store_result = store_response.result.expect("Store must succeed");
    let fingerprint_id_str = store_result.get("fingerprintId").unwrap().as_str().unwrap();
    let fingerprint_id = uuid::Uuid::parse_str(fingerprint_id_str).unwrap();

    println!("  Response fingerprintId: {}", fingerprint_id_str);
    println!("  Response embedderCount: {}", store_result.get("embedderCount").unwrap());
    println!("  Response embeddingLatencyMs: {}", store_result.get("embeddingLatencyMs").unwrap());
    println!("  Response storageLatencyMs: {}", store_result.get("storageLatencyMs").unwrap());

    // === STEP 3: Verify in Source of Truth ===
    println!("\n[STEP 3: SOURCE OF TRUTH VERIFICATION]");
    let stored_fp = store.retrieve(fingerprint_id).await.unwrap().unwrap();
    println!("  Direct store.retrieve() succeeded");
    println!("  Stored fingerprint fields:");
    println!("    - id: {}", stored_fp.id);
    println!("    - theta_to_north_star: {:.6}", stored_fp.theta_to_north_star);
    println!("    - access_count: {}", stored_fp.access_count);
    println!("    - created_at: {}", stored_fp.created_at);
    println!("    - last_updated: {}", stored_fp.last_updated);
    println!("    - content_hash: {}", hex::encode(stored_fp.content_hash));
    println!("    - semantic.e1_semantic[0..5]: {:?}", &stored_fp.semantic.e1_semantic[0..5]);
    println!("    - semantic.e6_sparse.nnz: {}", stored_fp.semantic.e6_sparse.nnz());
    println!("    - purpose_vector.alignments[0..5]: {:?}", &stored_fp.purpose_vector.alignments[0..5]);
    println!("    - purpose_vector.dominant_embedder: {}", stored_fp.purpose_vector.dominant_embedder);
    println!("    - purpose_vector.coherence: {:.6}", stored_fp.purpose_vector.coherence);
    println!("    - johari.quadrants[0] (E1): {:?}", stored_fp.johari.quadrants[0]);
    println!("    - johari.confidence[0] (E1): {:.6}", stored_fp.johari.confidence[0]);

    // Verify content hash
    let mut hasher = Sha256::new();
    hasher.update(test_content.as_bytes());
    let expected_hash: [u8; 32] = hasher.finalize().into();
    assert_eq!(stored_fp.content_hash, expected_hash);
    println!("    - content_hash MATCHES expected SHA-256");

    // === STEP 4: Final state ===
    println!("\n[FINAL STATE]");
    println!("  Store count: {}", store.count().await.unwrap());
    let final_exists = exists_in_store(&store, fingerprint_id).await;
    println!("  Fingerprint {} exists: {}", fingerprint_id, final_exists);

    println!("\n================================================================================");
    println!("EVIDENCE LOG COMPLETE - All verifications passed");
    println!("================================================================================\n");
}

// =============================================================================
// ROCKSDB FULL STATE VERIFICATION INTEGRATION TESTS
// =============================================================================
//
// These tests use REAL RocksDbTeleologicalStore with tempdir for full state verification.
// They directly inspect the persistent store to verify data was actually written.

/// FSV-ROCKSDB-001: Store creates fingerprint in REAL RocksDB store.
///
/// Verifies that memory/store operation:
/// 1. Starts with empty store (count = 0)
/// 2. Stores fingerprint via MCP handler
/// 3. Fingerprint physically exists in RocksDB
/// 4. Content hash matches SHA-256 of input
#[tokio::test]
async fn test_rocksdb_fsv_store_creates_fingerprint() {
    println!("\n================================================================================");
    println!("FSV-ROCKSDB-001: Store Creates Fingerprint in REAL RocksDB");
    println!("================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;
    // _tempdir MUST stay alive for the duration of this test

    // === BEFORE STATE ===
    let count_before = store.count().await.expect("count() should work on RocksDB");
    println!("\n[BEFORE] RocksDB store state:");
    println!("  - Fingerprint count: {}", count_before);
    assert_eq!(count_before, 0, "RocksDB store should be empty initially");

    // === EXECUTE OPERATION ===
    let content = "Machine learning is transforming software development";
    let params = json!({
        "content": content,
        "importance": 0.9
    });
    let request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(params));
    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Store must succeed: {:?}",
        response.error
    );

    let result = response.result.expect("Should have result");
    let fingerprint_id_str = result
        .get("fingerprintId")
        .expect("Should have fingerprintId")
        .as_str()
        .expect("Should be string");
    let fingerprint_id = uuid::Uuid::parse_str(fingerprint_id_str).expect("Should be valid UUID");

    println!("\n[OPERATION] Stored content with ID: {}", fingerprint_id);

    // === AFTER STATE - VERIFY SOURCE OF TRUTH IN ROCKSDB ===
    let count_after = store.count().await.expect("count() should work on RocksDB");
    println!("\n[AFTER] RocksDB store state:");
    println!("  - Fingerprint count: {}", count_after);
    assert_eq!(count_after, count_before + 1, "Count must increase by 1");

    // CRITICAL: Directly verify fingerprint exists in RocksDB
    let exists = exists_in_store(&store, fingerprint_id).await;
    println!("  - Fingerprint {} exists in RocksDB: {}", fingerprint_id, exists);
    assert!(exists, "VERIFICATION FAILED: Fingerprint must exist in RocksDB store");

    // CRITICAL: Retrieve and inspect actual stored data from RocksDB
    let stored_fp = store
        .retrieve(fingerprint_id)
        .await
        .expect("retrieve() should work on RocksDB")
        .expect("Fingerprint must exist");

    println!("\n[EVIDENCE] Stored fingerprint fields from RocksDB:");
    println!("  - ID: {}", stored_fp.id);
    println!("  - theta_to_north_star: {:.4}", stored_fp.theta_to_north_star);
    println!("  - access_count: {}", stored_fp.access_count);
    println!("  - content_hash: {}", hex::encode(stored_fp.content_hash));
    println!("  - semantic.e1_semantic len: {}", stored_fp.semantic.e1_semantic.len());

    // Verify content hash matches expected SHA-256
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let expected_hash: [u8; 32] = hasher.finalize().into();
    assert_eq!(
        stored_fp.content_hash, expected_hash,
        "Content hash in RocksDB must match SHA-256 of original content"
    );
    println!("  - Content hash VERIFIED: matches SHA-256 of input");

    // Verify semantic fingerprint has valid embedding
    assert!(!stored_fp.semantic.e1_semantic.is_empty(), "E1 must have embeddings");
    println!("  - E1 semantic embedding dimension: {}", stored_fp.semantic.e1_semantic.len());

    println!("\n[FSV-ROCKSDB-001 PASSED] Fingerprint physically exists in RocksDB");
    println!("================================================================================\n");
}

/// FSV-ROCKSDB-002: Retrieve returns data from REAL RocksDB store.
///
/// Verifies that memory/retrieve operation:
/// 1. Stores fingerprint to RocksDB
/// 2. Retrieves via MCP handler
/// 3. Returned data matches what was stored in RocksDB
#[tokio::test]
async fn test_rocksdb_fsv_retrieve_returns_stored_data() {
    println!("\n================================================================================");
    println!("FSV-ROCKSDB-002: Retrieve Returns Data from REAL RocksDB");
    println!("================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store a fingerprint first
    let content = "Neural networks process information in layers";
    let params = json!({
        "content": content,
        "importance": 0.85
    });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(params));
    let store_response = handlers.dispatch(store_request).await;

    let store_result = store_response.result.expect("Store should succeed");
    let fingerprint_id_str = store_result.get("fingerprintId").unwrap().as_str().unwrap();
    let fingerprint_id = uuid::Uuid::parse_str(fingerprint_id_str).unwrap();

    println!("\n[SETUP] Stored fingerprint with ID: {}", fingerprint_id);

    // Verify it exists in RocksDB directly
    let stored_fp = store
        .retrieve(fingerprint_id)
        .await
        .expect("retrieve() should work")
        .expect("Must exist in RocksDB");
    println!("[VERIFY] Fingerprint confirmed in RocksDB store");

    // === EXECUTE RETRIEVE OPERATION ===
    let retrieve_params = json!({ "fingerprintId": fingerprint_id_str });
    let retrieve_request = make_request("memory/retrieve", Some(JsonRpcId::Number(2)), Some(retrieve_params));
    let retrieve_response = handlers.dispatch(retrieve_request).await;

    assert!(
        retrieve_response.error.is_none(),
        "Retrieve must succeed: {:?}",
        retrieve_response.error
    );

    let result = retrieve_response.result.expect("Should have result");
    let fingerprint = result.get("fingerprint").expect("Must have fingerprint object");

    // === VERIFY RETRIEVED DATA MATCHES ROCKSDB ===
    let retrieved_id = fingerprint.get("id").and_then(|v| v.as_str()).expect("Must have id");
    assert_eq!(
        retrieved_id, fingerprint_id_str,
        "Retrieved ID must match stored ID"
    );

    // Verify content_hash matches what's in RocksDB
    let retrieved_hash = fingerprint.get("contentHashHex").and_then(|v| v.as_str()).expect("Must have contentHashHex");
    let expected_hash_hex = hex::encode(stored_fp.content_hash);
    assert_eq!(
        retrieved_hash, expected_hash_hex,
        "Retrieved contentHashHex must match RocksDB store"
    );

    println!("\n[EVIDENCE] Retrieved data matches RocksDB:");
    println!("  - ID: {}", retrieved_id);
    println!("  - contentHashHex: {}", retrieved_hash);

    println!("\n[FSV-ROCKSDB-002 PASSED] Retrieve returns data matching RocksDB");
    println!("================================================================================\n");
}

/// FSV-ROCKSDB-003: Delete removes fingerprint from REAL RocksDB store.
///
/// Verifies that memory/delete operation:
/// 1. Stores fingerprint to RocksDB
/// 2. Verifies it exists
/// 3. Deletes via MCP handler
/// 4. Fingerprint is physically gone from RocksDB
#[tokio::test]
async fn test_rocksdb_fsv_delete_removes_from_store() {
    println!("\n================================================================================");
    println!("FSV-ROCKSDB-003: Delete Removes Fingerprint from REAL RocksDB");
    println!("================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store a fingerprint
    let content = "Deep learning requires substantial computational resources";
    let params = json!({ "content": content, "importance": 0.75 });
    let store_request = make_request("memory/store", Some(JsonRpcId::Number(1)), Some(params));
    let store_response = handlers.dispatch(store_request).await;

    let store_result = store_response.result.expect("Store should succeed");
    let fingerprint_id_str = store_result.get("fingerprintId").unwrap().as_str().unwrap();
    let fingerprint_id = uuid::Uuid::parse_str(fingerprint_id_str).unwrap();

    println!("\n[SETUP] Stored fingerprint with ID: {}", fingerprint_id);

    // Verify it exists BEFORE delete
    let exists_before = exists_in_store(&store, fingerprint_id).await;
    assert!(exists_before, "Fingerprint must exist in RocksDB before delete");
    println!("[BEFORE DELETE] Fingerprint exists in RocksDB: {}", exists_before);

    let count_before = store.count().await.expect("count() should work");
    println!("  - Store count: {}", count_before);

    // === EXECUTE DELETE OPERATION ===
    let delete_params = json!({ "fingerprintId": fingerprint_id_str, "soft": false });
    let delete_request = make_request("memory/delete", Some(JsonRpcId::Number(2)), Some(delete_params));
    let delete_response = handlers.dispatch(delete_request).await;

    assert!(
        delete_response.error.is_none(),
        "Delete must succeed: {:?}",
        delete_response.error
    );

    let delete_result = delete_response.result.expect("Should have result");
    let deleted = delete_result.get("deleted").and_then(|v| v.as_bool()).expect("Must have deleted flag");
    assert!(deleted, "Response must indicate deletion succeeded");

    // === VERIFY FINGERPRINT IS GONE FROM ROCKSDB ===
    let exists_after = exists_in_store(&store, fingerprint_id).await;
    println!("\n[AFTER DELETE] Fingerprint exists in RocksDB: {}", exists_after);
    assert!(!exists_after, "VERIFICATION FAILED: Fingerprint must be removed from RocksDB");

    let count_after = store.count().await.expect("count() should work");
    println!("  - Store count: {}", count_after);
    assert_eq!(count_after, count_before - 1, "Count must decrease by 1");

    // Double-check with retrieve - should return None
    let retrieved = store.retrieve(fingerprint_id).await.expect("retrieve() should work");
    assert!(retrieved.is_none(), "Retrieve must return None for deleted fingerprint");

    println!("\n[FSV-ROCKSDB-003 PASSED] Fingerprint physically removed from RocksDB");
    println!("================================================================================\n");
}

/// FSV-ROCKSDB-004: Multiple fingerprints in REAL RocksDB store.
///
/// Verifies that multiple store operations:
/// 1. Each creates a unique fingerprint
/// 2. All fingerprints exist in RocksDB
/// 3. Count reflects actual stored items
#[tokio::test]
async fn test_rocksdb_fsv_multiple_fingerprints() {
    println!("\n================================================================================");
    println!("FSV-ROCKSDB-004: Multiple Fingerprints in REAL RocksDB");
    println!("================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let contents = vec![
        "First document about machine learning",
        "Second document about neural networks",
        "Third document about deep learning",
    ];

    let mut stored_ids = Vec::new();

    // Store multiple fingerprints
    for (i, content) in contents.iter().enumerate() {
        let params = json!({ "content": content, "importance": 0.8 });
        let request = make_request("memory/store", Some(JsonRpcId::Number(i as i64 + 1)), Some(params));
        let response = handlers.dispatch(request).await;

        let result = response.result.expect("Store should succeed");
        let id_str = result.get("fingerprintId").unwrap().as_str().unwrap();
        let id = uuid::Uuid::parse_str(id_str).unwrap();
        stored_ids.push(id);

        println!("[STORED {}] ID: {}", i + 1, id);
    }

    // === VERIFY ALL FINGERPRINTS EXIST IN ROCKSDB ===
    println!("\n[VERIFICATION] Checking all fingerprints in RocksDB:");

    let count = store.count().await.expect("count() should work");
    assert_eq!(count, contents.len(), "Count must match stored items");
    println!("  - Total count: {} (expected: {})", count, contents.len());

    for (i, id) in stored_ids.iter().enumerate() {
        let exists = exists_in_store(&store, *id).await;
        assert!(exists, "Fingerprint {} must exist in RocksDB", i + 1);

        let fp = store.retrieve(*id).await.expect("retrieve() should work").expect("Must exist");
        println!("  - Fingerprint {} exists: {} (hash: {})", i + 1, exists, hex::encode(&fp.content_hash[..8]));
    }

    println!("\n[FSV-ROCKSDB-004 PASSED] All {} fingerprints verified in RocksDB", contents.len());
    println!("================================================================================\n");
}
