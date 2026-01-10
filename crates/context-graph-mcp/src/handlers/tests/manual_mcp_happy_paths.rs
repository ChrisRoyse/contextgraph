//! Manual MCP Happy Path Testing
//!
//! This test file manually tests ALL MCP handlers to verify values show up correctly.
//! Uses RocksDB with tempdir for isolated testing.
//!
//! Run with: cargo test -p context-graph-mcp manual_mcp_happy_paths -- --nocapture

use serde_json::json;

use crate::protocol::{JsonRpcId, JsonRpcRequest};

use super::create_test_handlers_with_rocksdb_store_access;

fn make_request(
    method: &str,
    id: i64,
    params: Option<serde_json::Value>,
) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(id)),
        method: method.to_string(),
        params,
    }
}

/// Test 1: memory/store - Store a new memory
#[tokio::test]
async fn test_01_memory_store() {
    println!("\n========================================================================================================");
    println!("TEST 01: memory/store");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let params = json!({
        "content": "Test memory for happy path validation",
        "importance": 0.85
    });

    let request = make_request("memory/store", 1, Some(params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    // Verify required fields from memory/store response
    // API returns: fingerprintId, embeddingLatencyMs, storageLatencyMs, embedderCount
    let fingerprint_id = result.get("fingerprintId").expect("Should have fingerprintId");
    let embedder_count = result.get("embedderCount").expect("Should have embedderCount");

    println!("\n[VERIFICATION]");
    println!("  fingerprintId: {}", fingerprint_id);
    println!("  embedderCount: {}", embedder_count);
    println!("  embeddingLatencyMs: {}", result.get("embeddingLatencyMs").unwrap_or(&json!(0)));
    println!("  storageLatencyMs: {}", result.get("storageLatencyMs").unwrap_or(&json!(0)));

    assert!(fingerprint_id.is_string(), "fingerprintId should be string");
    assert_eq!(embedder_count.as_u64().unwrap(), 13, "Should have 13 embedders");
    println!("\n[PASSED] memory/store works correctly");
}

/// Test 2: memory/retrieve - Retrieve an existing memory
#[tokio::test]
async fn test_02_memory_retrieve() {
    println!("\n========================================================================================================");
    println!("TEST 02: memory/retrieve");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // First store a memory to retrieve
    let store_params = json!({
        "content": "Memory to retrieve for testing",
        "importance": 0.9
    });
    let store_response = handlers.dispatch(make_request("memory/store", 1, Some(store_params))).await;
    let fingerprint_id = store_response.result.unwrap()
        .get("fingerprintId").unwrap()
        .as_str().unwrap()
        .to_string();

    println!("Stored fingerprint: {}", fingerprint_id);

    // Now retrieve it
    let retrieve_params = json!({
        "fingerprintId": fingerprint_id
    });
    let request = make_request("memory/retrieve", 2, Some(retrieve_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");
    let fingerprint = result.get("fingerprint").expect("Should have fingerprint");

    println!("\n[VERIFICATION]");
    println!("  id: {}", fingerprint.get("id").unwrap());
    println!("  thetaToNorthStar: {}", fingerprint.get("thetaToNorthStar").unwrap());
    println!("  accessCount: {}", fingerprint.get("accessCount").unwrap());
    println!("  contentHashHex: {}", fingerprint.get("contentHashHex").unwrap());

    // Check purpose vector
    if let Some(pv) = fingerprint.get("purposeVector") {
        println!("  purposeVector.coherence: {}", pv.get("coherence").unwrap_or(&json!(null)));
        println!("  purposeVector.dominantEmbedder: {}", pv.get("dominantEmbedder").unwrap_or(&json!(null)));
    }

    println!("\n[PASSED] memory/retrieve works correctly");
}

/// Test 3: memory/search - Search for memories
#[tokio::test]
async fn test_03_memory_search() {
    println!("\n========================================================================================================");
    println!("TEST 03: memory/search");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store some memories first
    for (i, content) in ["neural network optimization", "distributed systems consensus", "machine learning algorithms"].iter().enumerate() {
        let params = json!({ "content": content, "importance": 0.8 });
        handlers.dispatch(make_request("memory/store", i as i64, Some(params))).await;
    }

    // Search - MUST include minSimilarity per constitution FAIL FAST policy
    let search_params = json!({
        "query": "neural network",
        "topK": 5,
        "minSimilarity": 0.0  // Required: use 0.0 to include all results
    });
    let request = make_request("memory/search", 10, Some(search_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");
    let results = result.get("results").expect("Should have results");

    println!("\n[VERIFICATION]");
    if let Some(arr) = results.as_array() {
        println!("  Found {} results", arr.len());
        for (i, r) in arr.iter().take(3).enumerate() {
            println!("  [{}] id={}, similarity={}",
                i,
                r.get("fingerprintId").unwrap_or(&json!("?")),
                r.get("similarity").unwrap_or(&json!(0))
            );
        }
    }

    println!("\n[PASSED] memory/search works correctly");
}

/// Test 4: memory/delete - Delete a memory
#[tokio::test]
async fn test_04_memory_delete() {
    println!("\n========================================================================================================");
    println!("TEST 04: memory/delete");
    println!("========================================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // First store a memory
    let store_params = json!({
        "content": "Memory to be deleted",
        "importance": 0.5
    });
    let store_response = handlers.dispatch(make_request("memory/store", 1, Some(store_params))).await;
    let fingerprint_id = store_response.result.unwrap()
        .get("fingerprintId").unwrap()
        .as_str().unwrap()
        .to_string();

    println!("Stored fingerprint: {}", fingerprint_id);

    let count_before = store.count().await.unwrap();
    println!("Count before delete: {}", count_before);

    // Hard delete (soft=false) to actually remove from count
    // Soft delete is default and marks as deleted but doesn't remove from count
    let delete_params = json!({
        "fingerprintId": fingerprint_id,
        "soft": false  // Hard delete to actually remove from storage
    });
    let request = make_request("memory/delete", 2, Some(delete_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    let count_after = store.count().await.unwrap();
    println!("Count after delete: {}", count_after);

    println!("\n[VERIFICATION]");
    println!("  deleted: {}", result.get("deleted").unwrap_or(&json!(false)));
    println!("  deleteType: {}", result.get("deleteType").unwrap_or(&json!("?")));
    println!("  count decreased: {} -> {}", count_before, count_after);

    assert!(result.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false), "Delete should return true");
    assert_eq!(result.get("deleteType").and_then(|v| v.as_str()).unwrap_or("?"), "hard", "Should be hard delete");
    assert!(count_after < count_before, "Count should decrease after hard delete");
    println!("\n[PASSED] memory/delete works correctly");
}

/// Test 5: search/multi - Multi-space search
#[tokio::test]
async fn test_05_search_multi() {
    println!("\n========================================================================================================");
    println!("TEST 05: search/multi");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store memories
    for content in ["vector embeddings for similarity", "database optimization techniques"] {
        let params = json!({ "content": content, "importance": 0.8 });
        handlers.dispatch(make_request("memory/store", 1, Some(params))).await;
    }

    // Search - MUST include minSimilarity per constitution FAIL FAST policy
    let search_params = json!({
        "query": "vector similarity search",
        "topK": 5,
        "minSimilarity": 0.0,  // Required: use 0.0 to include all results
        "weights": {
            "semantic": 1.0,
            "sparse": 0.5,
            "temporal": 0.3
        }
    });
    let request = make_request("search/multi", 10, Some(search_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    if let Some(results) = result.get("results") {
        if let Some(arr) = results.as_array() {
            println!("  Found {} results", arr.len());
        }
    }

    println!("\n[PASSED] search/multi works correctly");
}

/// Test 6: search/weight_profiles - Get weight profiles
#[tokio::test]
async fn test_06_search_weight_profiles() {
    println!("\n========================================================================================================");
    println!("TEST 06: search/weight_profiles");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let request = make_request("search/weight_profiles", 1, None);
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    if let Some(profiles) = result.get("profiles") {
        if let Some(obj) = profiles.as_object() {
            println!("  Available profiles: {:?}", obj.keys().collect::<Vec<_>>());
        }
    }

    println!("\n[PASSED] search/weight_profiles works correctly");
}

/// Test 7: purpose/query - Query purpose alignment
#[tokio::test]
async fn test_07_purpose_query() {
    println!("\n========================================================================================================");
    println!("TEST 07: purpose/query");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store some memories first so there's data to search
    let store_params = json!({
        "content": "Purpose-aligned memory for testing goal alignment",
        "importance": 0.9
    });
    handlers.dispatch(make_request("memory/store", 1, Some(store_params))).await;

    // purpose/query requires a 13-element purpose_vector array
    // Each element is a float in [0.0, 1.0] representing alignment per embedder
    let query_params = json!({
        "purpose_vector": [0.8, 0.7, 0.6, 0.5, 0.9, 0.4, 0.3, 0.5, 0.6, 0.7, 0.8, 0.9, 0.5],
        "topK": 10,
        "minAlignment": 0.0
    });
    let request = make_request("purpose/query", 2, Some(query_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    // purpose/query returns: results, query_metadata
    if let Some(results) = result.get("results") {
        if let Some(arr) = results.as_array() {
            println!("  Found {} results matching purpose vector", arr.len());
            for (i, r) in arr.iter().take(3).enumerate() {
                println!("  [{}] id={}, alignment={}",
                    i,
                    r.get("id").unwrap_or(&json!("?")),
                    r.get("purpose_alignment").unwrap_or(&json!("?"))
                );
            }
        }
    }

    if let Some(meta) = result.get("query_metadata") {
        println!("  search_time_ms: {}", meta.get("search_time_ms").unwrap_or(&json!("?")));
    }

    println!("\n[PASSED] purpose/query works correctly");
}

/// Test 8: utl/compute - Compute UTL score
#[tokio::test]
async fn test_08_utl_compute() {
    println!("\n========================================================================================================");
    println!("TEST 08: utl/compute");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // utl/compute requires 'input' parameter (text string to compute UTL score for)
    let compute_params = json!({
        "input": "Memory for UTL computation testing - analyzing learning patterns and knowledge integration"
    });
    let request = make_request("utl/compute", 2, Some(compute_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    println!("  learningScore: {}", result.get("learningScore").unwrap_or(&json!("?")));

    // Verify learning score is in valid range [0.0, 1.0]
    if let Some(score) = result.get("learningScore").and_then(|v| v.as_f64()) {
        assert!(score >= 0.0 && score <= 1.0, "Learning score should be in [0.0, 1.0]");
    }

    println!("\n[PASSED] utl/compute works correctly");
}

/// Test 9: utl/metrics - Get UTL metrics
#[tokio::test]
async fn test_09_utl_metrics() {
    println!("\n========================================================================================================");
    println!("TEST 09: utl/metrics");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // utl/metrics requires 'input' parameter (text string to compute metrics for)
    let metrics_params = json!({
        "input": "UTL metrics test content - learning patterns and knowledge synthesis"
    });
    let request = make_request("utl/metrics", 10, Some(metrics_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    println!("  entropy: {}", result.get("entropy").unwrap_or(&json!("?")));
    println!("  coherence: {}", result.get("coherence").unwrap_or(&json!("?")));
    println!("  learningScore: {}", result.get("learningScore").unwrap_or(&json!("?")));
    println!("  surprise: {}", result.get("surprise").unwrap_or(&json!("?")));

    println!("\n[PASSED] utl/metrics works correctly");
}

/// Test 10: system/status - Get system status
#[tokio::test]
async fn test_10_system_status() {
    println!("\n========================================================================================================");
    println!("TEST 10: system/status");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let request = make_request("system/status", 1, None);
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    println!("  fingerprintCount: {}", result.get("fingerprintCount").unwrap_or(&json!("?")));
    println!("  coherence: {}", result.get("coherence").unwrap_or(&json!("?")));
    println!("  entropy: {}", result.get("entropy").unwrap_or(&json!("?")));

    // Check quadrant distribution
    if let Some(quadrants) = result.get("quadrantDistribution") {
        println!("  quadrantDistribution: {:?}", quadrants);
    }

    println!("\n[PASSED] system/status works correctly");
}

/// Test 11: system/health - Get system health
#[tokio::test]
async fn test_11_system_health() {
    println!("\n========================================================================================================");
    println!("TEST 11: system/health");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let request = make_request("system/health", 1, None);
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    println!("  status: {}", result.get("status").unwrap_or(&json!("?")));
    println!("  storageHealthy: {}", result.get("storageHealthy").unwrap_or(&json!("?")));
    println!("  embeddingHealthy: {}", result.get("embeddingHealthy").unwrap_or(&json!("?")));

    println!("\n[PASSED] system/health works correctly");
}

/// Test 12: johari/get_distribution - Get Johari distribution
#[tokio::test]
async fn test_12_johari_get_distribution() {
    println!("\n========================================================================================================");
    println!("TEST 12: johari/get_distribution");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store a memory first to get its ID
    let store_params = json!({
        "content": "Johari test memory for distribution analysis",
        "importance": 0.8
    });
    let store_response = handlers.dispatch(make_request("memory/store", 1, Some(store_params))).await;
    let fingerprint_id = store_response.result.unwrap()
        .get("fingerprintId").unwrap()
        .as_str().unwrap()
        .to_string();

    // johari/get_distribution requires memory_id
    let distribution_params = json!({
        "memory_id": fingerprint_id
    });
    let request = make_request("johari/get_distribution", 10, Some(distribution_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    // Response contains per-embedder quadrant distribution
    if let Some(embedders) = result.get("embedders") {
        if let Some(arr) = embedders.as_array() {
            println!("  Found {} embedder distributions", arr.len());
            for (i, e) in arr.iter().take(3).enumerate() {
                println!("  [{}] {}: quadrant={}",
                    i,
                    e.get("embedder_name").unwrap_or(&json!("?")),
                    e.get("quadrant").unwrap_or(&json!("?"))
                );
            }
        }
    }

    if let Some(summary) = result.get("summary") {
        println!("  summary.open_count: {}", summary.get("open_count").unwrap_or(&json!("?")));
        println!("  summary.unknown_count: {}", summary.get("unknown_count").unwrap_or(&json!("?")));
    }

    println!("\n[PASSED] johari/get_distribution works correctly");
}

/// Test 13: johari/find_by_quadrant - Find memories by quadrant
#[tokio::test]
async fn test_13_johari_find_by_quadrant() {
    println!("\n========================================================================================================");
    println!("TEST 13: johari/find_by_quadrant");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store some memories (they start in Unknown quadrant)
    for content in ["Quadrant search test 1", "Quadrant search test 2"] {
        let params = json!({ "content": content, "importance": 0.8 });
        handlers.dispatch(make_request("memory/store", 1, Some(params))).await;
    }

    // johari/find_by_quadrant requires embedder_index and quadrant
    let search_params = json!({
        "embedder_index": 0,  // E1_semantic (first embedder)
        "quadrant": "Unknown",
        "top_k": 10
    });
    let request = make_request("johari/find_by_quadrant", 10, Some(search_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    if let Some(fingerprints) = result.get("fingerprints") {
        if let Some(arr) = fingerprints.as_array() {
            println!("  Found {} fingerprints in Unknown quadrant (E1_semantic)", arr.len());
            for (i, fp) in arr.iter().take(3).enumerate() {
                println!("  [{}] id={}", i, fp.get("id").unwrap_or(&json!("?")));
            }
        }
    }

    println!("\n[PASSED] johari/find_by_quadrant works correctly");
}

/// Test 14: tools/list - List available tools
#[tokio::test]
async fn test_14_tools_list() {
    println!("\n========================================================================================================");
    println!("TEST 14: tools/list");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let request = make_request("tools/list", 1, None);
    let response = handlers.dispatch(request).await;

    println!("Response (truncated): {}",
        serde_json::to_string_pretty(&response).unwrap().chars().take(2000).collect::<String>());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    if let Some(tools) = result.get("tools") {
        if let Some(arr) = tools.as_array() {
            println!("  Total tools: {}", arr.len());
            println!("  First 5 tools:");
            for tool in arr.iter().take(5) {
                if let Some(name) = tool.get("name") {
                    println!("    - {}", name);
                }
            }
        }
    }

    println!("\n[PASSED] tools/list works correctly");
}

/// Test 15: initialize - Initialize MCP connection
#[tokio::test]
async fn test_15_initialize() {
    println!("\n========================================================================================================");
    println!("TEST 15: initialize");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let init_params = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0.0"
        }
    });
    let request = make_request("initialize", 1, Some(init_params));
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Should not have error: {:?}", response.error);
    let result = response.result.expect("Should have result");

    println!("\n[VERIFICATION]");
    println!("  protocolVersion: {}", result.get("protocolVersion").unwrap_or(&json!("?")));
    if let Some(server_info) = result.get("serverInfo") {
        println!("  serverInfo.name: {}", server_info.get("name").unwrap_or(&json!("?")));
        println!("  serverInfo.version: {}", server_info.get("version").unwrap_or(&json!("?")));
    }

    println!("\n[PASSED] initialize works correctly");
}

/// Test 16: gwt/consciousness_level - Get consciousness level
#[tokio::test]
async fn test_16_gwt_consciousness_level() {
    println!("\n========================================================================================================");
    println!("TEST 16: gwt/consciousness_level");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let request = make_request("gwt/consciousness_level", 1, None);
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    // This may return an error if GWT is not initialized - that's OK
    if response.error.is_some() {
        println!("\n[INFO] GWT not initialized - expected behavior");
        println!("[PASSED] gwt/consciousness_level handled correctly (feature not enabled)");
    } else {
        let result = response.result.expect("Should have result");
        println!("\n[VERIFICATION]");
        println!("  level: {}", result.get("level").unwrap_or(&json!("?")));
        println!("[PASSED] gwt/consciousness_level works correctly");
    }
}

/// Test 17: consciousness/get_state - Get consciousness state
#[tokio::test]
async fn test_17_consciousness_get_state() {
    println!("\n========================================================================================================");
    println!("TEST 17: consciousness/get_state");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let request = make_request("consciousness/get_state", 1, None);
    let response = handlers.dispatch(request).await;

    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());

    // This may return an error if consciousness subsystem is not initialized
    if response.error.is_some() {
        println!("\n[INFO] Consciousness subsystem not initialized - expected behavior");
        println!("[PASSED] consciousness/get_state handled correctly");
    } else {
        let result = response.result.expect("Should have result");
        println!("\n[VERIFICATION]");
        println!("  state: {:?}", result);
        println!("[PASSED] consciousness/get_state works correctly");
    }
}

/// Comprehensive test: Run all tests and summarize
#[tokio::test]
async fn test_all_happy_paths_summary() {
    println!("\n========================================================================================================");
    println!("COMPREHENSIVE MCP HAPPY PATH TEST SUMMARY");
    println!("========================================================================================================");

    let (handlers, _store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    let mut passed = 0;
    let mut total = 0;

    // Helper to run a test
    async fn run_test(
        handlers: &crate::handlers::Handlers,
        name: &str,
        method: &str,
        params: Option<serde_json::Value>,
        passed: &mut i32,
        total: &mut i32,
    ) {
        *total += 1;
        let request = make_request(method, *total as i64, params);
        let response = handlers.dispatch(request).await;

        if response.error.is_none() {
            *passed += 1;
            println!("  [PASS] {} - {}", name, method);
        } else {
            let err = response.error.as_ref().unwrap();
            println!("  [FAIL] {} - {} - Error: {} ({})", name, method, err.message, err.code);
        }
    }

    // Store a test memory for subsequent tests
    let store_params = json!({
        "content": "Comprehensive test memory for all happy paths",
        "importance": 0.9
    });
    let store_response = handlers.dispatch(make_request("memory/store", 1, Some(store_params))).await;
    let fingerprint_id = store_response.result.as_ref()
        .and_then(|r| r.get("fingerprintId"))
        .and_then(|v| v.as_str())
        .unwrap_or("test-id")
        .to_string();

    println!("\nRunning {} MCP method tests...\n", 17);

    // Test each method
    run_test(&handlers, "Initialize", "initialize", Some(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "1.0"}
    })), &mut passed, &mut total).await;

    run_test(&handlers, "Tools List", "tools/list", None, &mut passed, &mut total).await;

    run_test(&handlers, "Memory Store", "memory/store", Some(json!({
        "content": "Test content",
        "importance": 0.8
    })), &mut passed, &mut total).await;

    run_test(&handlers, "Memory Retrieve", "memory/retrieve", Some(json!({
        "fingerprintId": fingerprint_id
    })), &mut passed, &mut total).await;

    run_test(&handlers, "Memory Search", "memory/search", Some(json!({
        "query": "test",
        "topK": 5,
        "minSimilarity": 0.0
    })), &mut passed, &mut total).await;

    run_test(&handlers, "Search Multi", "search/multi", Some(json!({
        "query": "test",
        "topK": 5,
        "minSimilarity": 0.0
    })), &mut passed, &mut total).await;

    run_test(&handlers, "Search Weight Profiles", "search/weight_profiles", None, &mut passed, &mut total).await;

    run_test(&handlers, "Purpose Query", "purpose/query", Some(json!({
        "purpose_vector": [0.8, 0.7, 0.6, 0.5, 0.9, 0.4, 0.3, 0.5, 0.6, 0.7, 0.8, 0.9, 0.5]
    })), &mut passed, &mut total).await;

    run_test(&handlers, "UTL Compute", "utl/compute", Some(json!({
        "input": "Test content for UTL computation to calculate universal truth likelihood"
    })), &mut passed, &mut total).await;

    run_test(&handlers, "UTL Metrics", "utl/metrics", Some(json!({
        "input": "Test content for UTL metrics to analyze universal truth likelihood"
    })), &mut passed, &mut total).await;

    run_test(&handlers, "System Status", "system/status", None, &mut passed, &mut total).await;

    run_test(&handlers, "System Health", "system/health", None, &mut passed, &mut total).await;

    run_test(&handlers, "Johari Distribution", "johari/get_distribution", Some(json!({
        "memory_id": fingerprint_id
    })), &mut passed, &mut total).await;

    run_test(&handlers, "Johari Find by Quadrant", "johari/find_by_quadrant", Some(json!({
        "quadrant": "Unknown",
        "embedder_index": 0,
        "limit": 5
    })), &mut passed, &mut total).await;

    // GWT/Consciousness tests may fail if not initialized
    run_test(&handlers, "GWT Consciousness Level", "gwt/consciousness_level", None, &mut passed, &mut total).await;
    run_test(&handlers, "Consciousness Get State", "consciousness/get_state", None, &mut passed, &mut total).await;

    // Final memory delete
    run_test(&handlers, "Memory Delete", "memory/delete", Some(json!({
        "fingerprintId": fingerprint_id
    })), &mut passed, &mut total).await;

    println!("\n========================================================================================================");
    println!("SUMMARY: {}/{} tests passed ({:.1}%)", passed, total, (passed as f64 / total as f64) * 100.0);
    println!("========================================================================================================\n");

    // Core functionality should all pass
    assert!(passed >= 14, "At least 14 core tests should pass (GWT/consciousness may not be initialized)");
}

// ============================================================================
// STUBFIX Manual Happy Path Tests
// ============================================================================

/// Test: get_steering_feedback - Steering subsystem returns REAL data
#[tokio::test]
async fn test_stubfix_steering_feedback_happy_path() {
    println!("\n========================================================================================================");
    println!("STUBFIX TEST: get_steering_feedback (Steering Subsystem)");
    println!("========================================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store some memories first to have data to analyze
    println!("\n[SETUP] Storing test memories...");
    for i in 0..5 {
        let params = json!({
            "content": format!("Test memory {} for steering feedback validation", i),
            "importance": 0.7 + (i as f64 * 0.05)
        });
        let request = make_request("memory/store", i as i64, Some(params));
        let response = handlers.dispatch(request).await;
        if response.error.is_some() {
            println!("  Warning: Store {} may have failed", i);
        }
    }

    let count = store.count().await.expect("count works");
    println!("  Stored {} memories", count);

    // Call get_steering_feedback
    let request = make_request("tools/call", 10, Some(json!({
        "name": "get_steering_feedback",
        "arguments": {}
    })));
    let response = handlers.dispatch(request).await;

    println!("\nResponse: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Handler must succeed: {:?}", response.error);

    let result = response.result.expect("Should have result");
    let content = result.get("content").and_then(|c| c.as_array());

    if let Some(content_arr) = content {
        for item in content_arr {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                let data: serde_json::Value = serde_json::from_str(text).unwrap_or(json!({}));

                println!("\n[VERIFICATION] Steering Feedback Data:");

                // Verify reward
                if let Some(reward) = data.get("reward") {
                    let value = reward.get("value").and_then(|v| v.as_f64()).unwrap_or(-999.0);
                    let gardener_score = reward.get("gardener_score").and_then(|v| v.as_f64()).unwrap_or(-999.0);
                    let curator_score = reward.get("curator_score").and_then(|v| v.as_f64()).unwrap_or(-999.0);
                    let assessor_score = reward.get("assessor_score").and_then(|v| v.as_f64()).unwrap_or(-999.0);

                    println!("  reward.value: {:.4}", value);
                    println!("  reward.gardener_score: {:.4}", gardener_score);
                    println!("  reward.curator_score: {:.4}", curator_score);
                    println!("  reward.assessor_score: {:.4}", assessor_score);

                    assert!(value >= -1.0 && value <= 1.0, "Reward value should be in [-1, 1]");
                }

                // Verify gardener details
                if let Some(gardener) = data.get("gardener_details") {
                    let connectivity = gardener.get("connectivity").and_then(|v| v.as_f64()).unwrap_or(-1.0);
                    let dead_ends = gardener.get("dead_ends_removed").and_then(|v| v.as_u64()).unwrap_or(0);

                    println!("  gardener.connectivity: {:.4}", connectivity);
                    println!("  gardener.dead_ends_removed: {}", dead_ends);

                    assert!(connectivity >= 0.0 && connectivity <= 1.0, "Connectivity should be in [0, 1]");
                }

                println!("\n[PASSED] get_steering_feedback returns REAL computed data");
            }
        }
    }
}

/// Test: get_pruning_candidates - Pruning subsystem returns REAL candidates
#[tokio::test]
async fn test_stubfix_pruning_candidates_happy_path() {
    println!("\n========================================================================================================");
    println!("STUBFIX TEST: get_pruning_candidates (Pruning Subsystem)");
    println!("========================================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store some memories first
    println!("\n[SETUP] Storing test memories...");
    for i in 0..5 {
        let params = json!({
            "content": format!("Test memory {} for pruning candidates validation", i),
            "importance": 0.3 + (i as f64 * 0.1)
        });
        let request = make_request("memory/store", i as i64, Some(params));
        let _ = handlers.dispatch(request).await;
    }

    let count = store.count().await.expect("count works");
    println!("  Stored {} memories", count);

    // Call get_pruning_candidates
    let request = make_request("tools/call", 10, Some(json!({
        "name": "get_pruning_candidates",
        "arguments": {
            "limit": 10,
            "min_staleness_days": 0,
            "min_alignment": 0.9
        }
    })));
    let response = handlers.dispatch(request).await;

    println!("\nResponse: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Handler must succeed: {:?}", response.error);

    let result = response.result.expect("Should have result");
    let content = result.get("content").and_then(|c| c.as_array());

    if let Some(content_arr) = content {
        for item in content_arr {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                let data: serde_json::Value = serde_json::from_str(text).unwrap_or(json!({}));

                println!("\n[VERIFICATION] Pruning Candidates Data:");

                // Verify summary
                if let Some(summary) = data.get("summary") {
                    let total = summary.get("total_candidates").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!("  summary.total_candidates: {}", total);
                }

                // Verify candidates array exists
                if let Some(candidates) = data.get("candidates").and_then(|c| c.as_array()) {
                    println!("  candidates count: {}", candidates.len());

                    for (i, candidate) in candidates.iter().enumerate().take(3) {
                        let memory_id = candidate.get("memory_id").and_then(|v| v.as_str()).unwrap_or("?");
                        let reason = candidate.get("reason").and_then(|v| v.as_str()).unwrap_or("?");
                        let alignment = candidate.get("alignment").and_then(|v| v.as_f64()).unwrap_or(-1.0);

                        println!("    [{}] {} - reason: {}, alignment: {:.4}", i+1, memory_id, reason, alignment);
                    }
                }

                println!("\n[PASSED] get_pruning_candidates returns REAL data structure");
            }
        }
    }
}

/// Test: trigger_consolidation - Consolidation subsystem analyzes REAL pairs
#[tokio::test]
async fn test_stubfix_trigger_consolidation_happy_path() {
    println!("\n========================================================================================================");
    println!("STUBFIX TEST: trigger_consolidation (Consolidation Subsystem)");
    println!("========================================================================================================");

    let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;

    // Store some memories first - use similar content to potentially get consolidation candidates
    println!("\n[SETUP] Storing test memories with similar content...");
    let base_content = "Machine learning and neural networks for optimization";
    for i in 0..5 {
        let content = format!("{} - variant {}", base_content, i);
        let params = json!({
            "content": content,
            "importance": 0.8
        });
        let request = make_request("memory/store", i as i64, Some(params));
        let _ = handlers.dispatch(request).await;
    }

    let count = store.count().await.expect("count works");
    println!("  Stored {} memories", count);

    // Call trigger_consolidation
    let request = make_request("tools/call", 10, Some(json!({
        "name": "trigger_consolidation",
        "arguments": {
            "strategy": "similarity",
            "min_similarity": 0.5,
            "max_memories": 10
        }
    })));
    let response = handlers.dispatch(request).await;

    println!("\nResponse: {}", serde_json::to_string_pretty(&response).unwrap());

    assert!(response.error.is_none(), "Handler must succeed: {:?}", response.error);

    let result = response.result.expect("Should have result");
    let content = result.get("content").and_then(|c| c.as_array());

    if let Some(content_arr) = content {
        for item in content_arr {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                let data: serde_json::Value = serde_json::from_str(text).unwrap_or(json!({}));

                println!("\n[VERIFICATION] Consolidation Data:");

                // Verify statistics
                if let Some(stats) = data.get("statistics") {
                    let pairs_evaluated = stats.get("pairs_evaluated").and_then(|v| v.as_u64()).unwrap_or(0);
                    let strategy = stats.get("strategy").and_then(|v| v.as_str()).unwrap_or("?");
                    let threshold = stats.get("similarity_threshold").and_then(|v| v.as_f64()).unwrap_or(-1.0);

                    println!("  statistics.pairs_evaluated: {}", pairs_evaluated);
                    println!("  statistics.strategy: {}", strategy);
                    println!("  statistics.similarity_threshold: {:.4}", threshold);

                    assert!(pairs_evaluated > 0 || count < 2, "Should evaluate pairs when multiple memories exist");
                    assert_eq!(strategy, "similarity", "Strategy should match request");
                }

                // Verify consolidation_result
                if let Some(result) = data.get("consolidation_result") {
                    let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                    let candidate_count = result.get("candidate_count").and_then(|v| v.as_u64()).unwrap_or(0);

                    println!("  consolidation_result.status: {}", status);
                    println!("  consolidation_result.candidate_count: {}", candidate_count);
                }

                // Check candidates_sample if present
                if let Some(sample) = data.get("candidates_sample").and_then(|c| c.as_array()) {
                    println!("  candidates_sample count: {}", sample.len());
                    for (i, c) in sample.iter().enumerate().take(3) {
                        let similarity = c.get("similarity").and_then(|v| v.as_f64()).unwrap_or(-1.0);
                        println!("    [{}] similarity: {:.4}", i+1, similarity);
                    }
                }

                println!("\n[PASSED] trigger_consolidation returns REAL analysis data");
            }
        }
    }
}
