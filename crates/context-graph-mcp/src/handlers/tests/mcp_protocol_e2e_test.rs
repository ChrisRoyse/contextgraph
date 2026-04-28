//! End-to-End MCP Protocol Integration Test
//!
//! This test verifies the complete MCP protocol flow with REAL GPU embeddings.
//!
//! ## Constitution Compliance
//!
//! - ARCH-08: CUDA GPU required for production
//! - AP-35: No stub data when real data available
//! - MCP 2024-11-05 protocol specification compliance
//!
//! ## LOW-2 Note: `assert!(result.is_ok())` Pattern
//!
//! Many tests in this module (and across the crate) use `assert!(result.is_ok())`
//! to verify operation completion without inspecting returned values. This is
//! intentional for integration tests that verify the MCP protocol flow succeeds
//! end-to-end. Value inspection is handled by unit tests in the respective modules.
//!
//! ## Test Coverage
//!
//! 1. Initialize handshake (MCP lifecycle)
//! 2. List all 12 tools (MCP capability discovery)
//! 3. Call each tool with valid inputs
//! 4. Full State Verification of stored data
//!
//! ## Feature Gate
//!
//! MED-23 FIX: Changed from `cuda` to `llm` feature gate. The `llm` feature provides
//! the embedding provider needed for these tests. They run in default `cargo test`
//! (which enables default features including `llm`), and gracefully skip when the
//! embedding models are not available.
//!
//! Run explicitly with: `cargo test -p context-graph-mcp mcp_protocol_e2e --features llm`

use serde_json::json;
use std::collections::HashSet;

use crate::protocol::JsonRpcId;

use super::{extract_mcp_tool_data, make_request};

#[cfg(feature = "llm")]
use super::create_test_handlers_with_real_embeddings_store_access;

#[cfg(feature = "llm")]
use context_graph_core::constellation::{compile_constellation, ConstellationSelector};
#[cfg(feature = "llm")]
use context_graph_core::learning::LearningOutcomeLabel;
#[cfg(feature = "llm")]
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
#[cfg(feature = "llm")]
use context_graph_core::training::NUM_CROSS_CORRELATIONS;
#[cfg(feature = "llm")]
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
#[cfg(feature = "llm")]
use context_graph_storage::teleological::RocksDbTeleologicalStore;

// =============================================================================
// End-to-End Protocol Tests with Real GPU Embeddings
// =============================================================================

/// Complete MCP handshake verification.
///
/// Tests the full initialization sequence per MCP 2024-11-05:
/// 1. Client sends initialize request
/// 2. Server responds with capabilities
/// 3. Client sends initialized notification
#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_mcp_handshake_with_gpu() {
    println!("\n=== E2E TEST: MCP Handshake with GPU ===");

    let (handlers, _store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // STEP 1: Initialize request
    let init_request = make_request("initialize", Some(JsonRpcId::Number(1)), None);
    let init_response = handlers.dispatch(init_request).await;

    assert!(
        init_response.error.is_none(),
        "Initialize must not return error: {:?}",
        init_response.error
    );

    let init_result = init_response.result.expect("Initialize must return result");

    // Verify protocol version
    let protocol_version = init_result
        .get("protocolVersion")
        .expect("Must have protocolVersion")
        .as_str()
        .expect("protocolVersion must be string");
    assert_eq!(
        protocol_version, "2024-11-05",
        "Protocol version must be 2024-11-05"
    );
    println!("Protocol version: {}", protocol_version);

    // Verify capabilities
    let capabilities = init_result
        .get("capabilities")
        .expect("Must have capabilities");
    assert!(
        capabilities.get("tools").is_some(),
        "Must have tools capability"
    );
    println!("Capabilities: tools supported");

    // STEP 2: Initialized notification (no response expected)
    let initialized_request = make_request("notifications/initialized", None, None);
    let initialized_response = handlers.dispatch(initialized_request).await;
    assert!(
        initialized_response.id.is_none(),
        "Notification should have no ID in response"
    );
    println!("Initialized notification acknowledged");

    println!("=== HANDSHAKE COMPLETE ===\n");
}

/// Verify all MCP tools are listed (56 with LLM, 52 without).
/// Audit-11 TST-H3: Fixed test name and assertions to match actual 56 tools.
#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_tools_list_all_tools() {
    println!("\n=== E2E TEST: Verify All Tools Listed ===");

    let (handlers, _store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    let request = make_request("tools/list", Some(JsonRpcId::Number(1)), None);
    let response = handlers.dispatch(request).await;

    assert!(response.error.is_none(), "tools/list must not return error");
    let result = response.result.expect("tools/list must return result");
    let tools = result
        .get("tools")
        .expect("Must have tools array")
        .as_array()
        .expect("tools must be array");

    // Expected 11 tools per CLAUDE.md (inject_context merged into store_memory)
    let expected_tools: HashSet<&str> = [
        "store_memory",
        "get_memetic_status",
        "search_graph",
        "trigger_consolidation",
        "get_topic_portfolio",
        "get_topic_stability",
        "detect_topics",
        "get_divergence_alerts",
        "merge_concepts",
        "forget_concept",
        "boost_importance",
    ]
    .into_iter()
    .collect();

    let actual_tools: HashSet<String> = tools
        .iter()
        .map(|t| t.get("name").unwrap().as_str().unwrap().to_string())
        .collect();

    println!("Found {} tools:", tools.len());
    for tool in tools {
        let name = tool.get("name").unwrap().as_str().unwrap();
        let has_description = tool.get("description").is_some();
        let has_schema = tool.get("inputSchema").is_some();
        println!(
            "  - {} (desc: {}, schema: {})",
            name, has_description, has_schema
        );
    }

    // Verify all expected tools are present
    for expected in &expected_tools {
        assert!(
            actual_tools.contains(*expected),
            "Missing expected tool: {}",
            expected
        );
    }

    // Audit-12 TST-H3 FIX: Exact assertion (this test is #[cfg(feature = "llm")]).
    // 2026-04-14: Bumped 58 → 71 after the training-data-export phase added 13 tools:
    //   Phase 1 (4): export_training_corpus, list_training_records, get_training_record, count_training_records
    //   Phase 2 (5): compile_constellation, list_constellations, get_constellation, score_against_constellation, delete_constellation
    //   Phase 3 (4): mine_contrastive_pairs, list_contrastive_pairs, get_contrastive_pair, count_contrastive_pairs
    // 2026-04-15: Bumped 71 → 75 after Phase 4 typed-edge tools:
    //   export_typed_edges_corpus, derive_anomalies_from_edges,
    //   validate_low_confidence_edges, list_typed_edge_records
    // 2026-04-27: Bumped 75 → 79 after Learning-as-UTL tools:
    //   record_learning_event, list_learning_events, get_learning_event,
    //   count_learning_events
    // 2026-04-27: Bumped 79 → 87 after UTL learner-state tools:
    //   register_learner, record_session_observation, compute_delta_s,
    //   compute_delta_c, compute_delta_e, compute_L, get_learner_M,
    //   next_review_for_trace
    // 2026-04-28: Bumped 87 → 96 after MCP learner-ops completion:
    //   list_learner_embedders, preflight_learner_assets, count_learner_state,
    //   get_learner_state, record_learner_k_sleep, record_learner_retrieval,
    //   upsert_goal_centroid, get_goal_distance, compile_learner_constellation
    // 2026-04-28: Bumped 96 → 100 after learner matrix dataset export:
    //   export_learner_training_dataset, list/get/count_learner_training_datasets
    // 2026-04-28: Bumped 100 → 101 after state-conditioned learner retrieval policy:
    //   resolve_learner_retrieval_policy
    // 2026-04-28: Bumped 101 → 103 after conceptual capability tools:
    //   estimate_learning_outcome, derive_constellation
    assert_eq!(
        tools.len(),
        103,
        "Expected exactly 103 tools with LLM feature, found {}",
        tools.len()
    );

    println!("=== ALL {} TOOLS VERIFIED ===\n", tools.len());
}

/// Full E2E workflow: store memories, search, get status.
#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_core_tools_workflow() {
    println!("\n=== E2E TEST: Core Tools Workflow with GPU ===");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // STEP 1: store_memory (with rationale) - Store first memory with GPU embeddings
    // Note: inject_context was merged into store_memory. When rationale is provided,
    // the same validation and response format is used.
    println!("\n--- STEP 1: store_memory (with rationale) ---");
    let inject_params = json!({
        "name": "store_memory",
        "arguments": {
            "content": "Rust programming language features async/await patterns",
            "rationale": "Testing end-to-end MCP protocol with real GPU embeddings",
            "importance": 0.9
        }
    });
    let inject_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(1)),
        Some(inject_params),
    );
    let inject_response = handlers.dispatch(inject_request).await;

    assert!(
        inject_response.error.is_none(),
        "store_memory must not return JSON-RPC error: {:?}",
        inject_response.error
    );

    let inject_result = inject_response
        .result
        .expect("store_memory must return result");
    let inject_data = extract_mcp_tool_data(&inject_result);

    let fingerprint_id1 = inject_data
        .get("fingerprintId")
        .expect("Must have fingerprintId")
        .as_str()
        .expect("fingerprintId must be string");
    println!("Stored memory 1: {}", fingerprint_id1);

    // STEP 2: store_memory - Store second memory
    println!("\n--- STEP 2: store_memory ---");
    let store_params = json!({
        "name": "store_memory",
        "arguments": {
            "content": "Tokio runtime provides green threads for async programming",
            "importance": 0.85,
            "tags": ["rust", "async", "tokio"]
        }
    });
    let store_request = make_request("tools/call", Some(JsonRpcId::Number(2)), Some(store_params));
    let store_response = handlers.dispatch(store_request).await;

    assert!(
        store_response.error.is_none(),
        "store_memory must not error"
    );
    let store_result = store_response
        .result
        .expect("store_memory must return result");
    let store_data = extract_mcp_tool_data(&store_result);

    let fingerprint_id2 = store_data
        .get("fingerprintId")
        .expect("Must have fingerprintId")
        .as_str()
        .expect("fingerprintId must be string");
    println!("Stored memory 2: {}", fingerprint_id2);

    // STEP 3: get_memetic_status
    println!("\n--- STEP 3: get_memetic_status ---");
    let status_params = json!({
        "name": "get_memetic_status",
        "arguments": {}
    });
    let status_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(3)),
        Some(status_params),
    );
    let status_response = handlers.dispatch(status_request).await;

    assert!(
        status_response.error.is_none(),
        "get_memetic_status must not error"
    );
    let status_result = status_response
        .result
        .expect("get_memetic_status must return result");
    let status_data = extract_mcp_tool_data(&status_result);

    let fingerprint_count = status_data
        .get("fingerprintCount")
        .expect("Must have fingerprintCount")
        .as_u64()
        .expect("fingerprintCount must be u64");
    println!("Fingerprint count: {}", fingerprint_count);
    assert!(
        fingerprint_count >= 2,
        "Should have at least 2 fingerprints stored, got {}",
        fingerprint_count
    );

    // STEP 4: search_graph - Search for related memories
    // Audit-11 TST-H2: Removed ghost "enrichMode" param — schema has additionalProperties:false,
    // handler never reads it. It was silently dropped.
    println!("\n--- STEP 4: search_graph ---");
    let search_params = json!({
        "name": "search_graph",
        "arguments": {
            "query": "async programming in Rust",
            "topK": 10,
            "includeContent": true
        }
    });
    let search_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(4)),
        Some(search_params),
    );
    let search_response = handlers.dispatch(search_request).await;

    assert!(
        search_response.error.is_none(),
        "search_graph must not error"
    );
    let search_result = search_response
        .result
        .expect("search_graph must return result");
    let search_data = extract_mcp_tool_data(&search_result);

    let results = search_data
        .get("results")
        .expect("Must have results")
        .as_array()
        .expect("results must be array");
    let count = search_data
        .get("count")
        .expect("Must have count")
        .as_u64()
        .expect("count must be u64");

    println!("Search returned {} results", count);
    for (i, result) in results.iter().enumerate() {
        let similarity = result
            .get("similarity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let dominant = result
            .get("dominantEmbedder")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!(
            "  Result {}: similarity={:.4}, dominant={}",
            i + 1,
            similarity,
            dominant
        );
    }

    // LOW-1 FIX: Remove conditional guard — assert results are non-empty first,
    // then verify similarity. Previously this silently passed with zero assertions.
    assert!(
        !results.is_empty(),
        "Expected non-empty search results from GPU embeddings"
    );
    let top_similarity = results[0]
        .get("similarity")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    assert!(
        top_similarity > 0.0,
        "Top result should have positive similarity with real GPU embeddings"
    );

    // STEP 5: FSV - Verify data in store
    println!("\n--- STEP 5: Full State Verification ---");
    let stored_count = store.count().await.expect("count must work");
    println!("FSV: Store contains {} fingerprints", stored_count);
    assert!(
        stored_count >= 2,
        "FSV: Store must contain at least 2 fingerprints, has {}",
        stored_count
    );

    println!("\n=== CORE TOOLS WORKFLOW COMPLETE ===\n");
}

/// E2E Topic tools workflow.
#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_topic_tools_workflow() {
    println!("\n=== E2E TEST: Topic Tools Workflow with GPU ===");

    let (handlers, _store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // First store some memories for topic detection
    for i in 0..5 {
        let params = json!({
            "name": "store_memory",
            "arguments": {
                "content": format!("Memory {} about machine learning and neural networks", i),
                "importance": 0.7
            }
        });
        let request = make_request("tools/call", Some(JsonRpcId::Number(i)), Some(params));
        let response = handlers.dispatch(request).await;
        assert!(response.error.is_none(), "store_memory {} failed", i);
    }
    println!("Stored 5 memories for topic detection");

    // get_topic_portfolio
    println!("\n--- get_topic_portfolio ---");
    let portfolio_params = json!({
        "name": "get_topic_portfolio",
        "arguments": {
            "format": "standard"
        }
    });
    let portfolio_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(10)),
        Some(portfolio_params),
    );
    let portfolio_response = handlers.dispatch(portfolio_request).await;

    assert!(
        portfolio_response.error.is_none(),
        "get_topic_portfolio must not error: {:?}",
        portfolio_response.error
    );
    let portfolio_result = portfolio_response
        .result
        .expect("get_topic_portfolio must return result");
    let is_error = portfolio_result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        !is_error,
        "get_topic_portfolio should not return tool error"
    );
    println!("get_topic_portfolio: success");

    // get_topic_stability
    println!("\n--- get_topic_stability ---");
    let stability_params = json!({
        "name": "get_topic_stability",
        "arguments": {
            "hours": 6
        }
    });
    let stability_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(11)),
        Some(stability_params),
    );
    let stability_response = handlers.dispatch(stability_request).await;

    assert!(
        stability_response.error.is_none(),
        "get_topic_stability must not error"
    );
    let stability_result = stability_response
        .result
        .expect("get_topic_stability must return result");
    let is_error = stability_result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        !is_error,
        "get_topic_stability should not return tool error"
    );
    println!("get_topic_stability: success");

    // detect_topics
    println!("\n--- detect_topics ---");
    let detect_params = json!({
        "name": "detect_topics",
        "arguments": {
            "force": true
        }
    });
    let detect_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(12)),
        Some(detect_params),
    );
    let detect_response = handlers.dispatch(detect_request).await;

    assert!(
        detect_response.error.is_none(),
        "detect_topics must not error"
    );
    println!("detect_topics: success");

    // get_divergence_alerts
    println!("\n--- get_divergence_alerts ---");
    let divergence_params = json!({
        "name": "get_divergence_alerts",
        "arguments": {
            "lookback_hours": 2
        }
    });
    let divergence_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(13)),
        Some(divergence_params),
    );
    let divergence_response = handlers.dispatch(divergence_request).await;

    assert!(
        divergence_response.error.is_none(),
        "get_divergence_alerts must not error"
    );
    println!("get_divergence_alerts: success");

    println!("\n=== TOPIC TOOLS WORKFLOW COMPLETE ===\n");
}

/// E2E Curation tools workflow.
#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_curation_tools_workflow() {
    println!("\n=== E2E TEST: Curation Tools Workflow with GPU ===");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // Store memories to curate
    let mut fingerprint_ids = Vec::new();
    for i in 0..3 {
        let params = json!({
            "name": "store_memory",
            "arguments": {
                "content": format!("Curation test memory {} about databases", i),
                "importance": 0.5
            }
        });
        let request = make_request("tools/call", Some(JsonRpcId::Number(i)), Some(params));
        let response = handlers.dispatch(request).await;
        assert!(response.error.is_none(), "store_memory {} failed", i);

        let result = response.result.expect("must have result");
        let data = extract_mcp_tool_data(&result);
        let id = data
            .get("fingerprintId")
            .expect("must have fingerprintId")
            .as_str()
            .expect("must be string");
        fingerprint_ids.push(id.to_string());
        println!("Stored memory {}: {}", i, id);
    }

    // boost_importance
    println!("\n--- boost_importance ---");
    let boost_params = json!({
        "name": "boost_importance",
        "arguments": {
            "node_id": fingerprint_ids[0],
            "delta": 0.3
        }
    });
    let boost_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(10)),
        Some(boost_params),
    );
    let boost_response = handlers.dispatch(boost_request).await;

    assert!(
        boost_response.error.is_none(),
        "boost_importance must not error"
    );
    let boost_result = boost_response
        .result
        .expect("boost_importance must return result");
    let is_error = boost_result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(!is_error, "boost_importance should not return tool error");
    println!("boost_importance: success (boosted {})", fingerprint_ids[0]);

    // forget_concept (soft delete)
    println!("\n--- forget_concept ---");
    let forget_params = json!({
        "name": "forget_concept",
        "arguments": {
            "node_id": fingerprint_ids[2],
            "soft_delete": true
        }
    });
    let forget_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(11)),
        Some(forget_params),
    );
    let forget_response = handlers.dispatch(forget_request).await;

    assert!(
        forget_response.error.is_none(),
        "forget_concept must not error"
    );
    let forget_result = forget_response
        .result
        .expect("forget_concept must return result");
    let is_error = forget_result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(!is_error, "forget_concept should not return tool error");
    println!(
        "forget_concept: success (soft-deleted {})",
        fingerprint_ids[2]
    );

    // merge_concepts
    println!("\n--- merge_concepts ---");
    let merge_params = json!({
        "name": "merge_concepts",
        "arguments": {
            "source_ids": [fingerprint_ids[0], fingerprint_ids[1]],
            "target_name": "Merged Database Concept",
            "rationale": "Consolidating similar database memories",
            "merge_strategy": "union"
        }
    });
    let merge_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(12)),
        Some(merge_params),
    );
    let merge_response = handlers.dispatch(merge_request).await;

    assert!(
        merge_response.error.is_none(),
        "merge_concepts must not error"
    );
    let merge_result = merge_response
        .result
        .expect("merge_concepts must return result");
    let is_error = merge_result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(!is_error, "merge_concepts should not return tool error");
    println!("merge_concepts: success");

    // trigger_consolidation
    println!("\n--- trigger_consolidation ---");
    let consolidation_params = json!({
        "name": "trigger_consolidation",
        "arguments": {
            "strategy": "similarity",
            "min_similarity": 0.9,
            "max_memories": 50
        }
    });
    let consolidation_request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(13)),
        Some(consolidation_params),
    );
    let consolidation_response = handlers.dispatch(consolidation_request).await;

    assert!(
        consolidation_response.error.is_none(),
        "trigger_consolidation must not error"
    );
    println!("trigger_consolidation: success");

    // FSV
    println!("\n--- Full State Verification ---");
    let final_count = store.count().await.expect("count must work");
    println!("FSV: Final store count: {}", final_count);

    println!("\n=== CURATION TOOLS WORKFLOW COMPLETE ===\n");
}

#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_resolve_learner_retrieval_policy_fsv() {
    println!("\n=== E2E TEST: Learner Retrieval Policy FSV ===");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;
    let rocksdb_store = store
        .as_any()
        .downcast_ref::<context_graph_storage::teleological::RocksDbTeleologicalStore>()
        .expect("FSV requires RocksDbTeleologicalStore");

    let learner_id = "12345678-1234-4234-9234-123456789abc";
    let session_ts = 1_900_100_001u64;

    println!(
        "SOURCE OF TRUTH: RocksDB CF_LEARNER_STATE_HISTORY before={}",
        rocksdb_store.count_learner_state_history().await.unwrap()
    );

    let record = make_request(
        "tools/call",
        Some(JsonRpcId::Number(501)),
        Some(json!({
            "name": "record_session_observation",
            "arguments": {
                "learnerId": learner_id,
                "sessionTs": session_ts,
                "modality": "affect_text",
                "rawText": "policy fsv high stress learner state",
                "components": {
                    "plasticityWindow": 0.55,
                    "hrvCoherence": 0.40,
                    "valence": 0.10,
                    "arousal": 0.20,
                    "stressFloor": 0.70,
                    "kSleep": 1.0
                },
                "stateVector": [0.55, 0.40, 0.10, 0.20, 0.70, 1.0],
                "embeddings": [{
                    "modality": "affect_text",
                    "vector": [0.2, 0.4, 0.8]
                }]
            }
        })),
    );
    let record_response = handlers.dispatch(record).await;
    assert!(
        record_response.error.is_none(),
        "record_session_observation JSON-RPC error: {:?}",
        record_response.error
    );
    let record_data =
        extract_mcp_tool_data(&record_response.result.expect("record must return result"));
    assert_eq!(record_data["status"], "stored");

    let readback = rocksdb_store
        .get_learner_state_vector(uuid::Uuid::parse_str(learner_id).unwrap(), session_ts)
        .await
        .unwrap()
        .expect("state vector must be physically present after MCP write");
    assert_eq!(readback.components.stress_floor, 0.70);
    println!(
        "AFTER RECORD state_history={} stress_floor={}",
        rocksdb_store.count_learner_state_history().await.unwrap(),
        readback.components.stress_floor
    );

    let resolve = make_request(
        "tools/call",
        Some(JsonRpcId::Number(502)),
        Some(json!({
            "name": "resolve_learner_retrieval_policy",
            "arguments": {
                "learnerId": learner_id,
                "sessionTs": session_ts,
                "baseWeightProfile": "semantic_search",
                "includeWeights": true
            }
        })),
    );
    let resolve_response = handlers.dispatch(resolve).await;
    assert!(
        resolve_response.error.is_none(),
        "resolve_learner_retrieval_policy JSON-RPC error: {:?}",
        resolve_response.error
    );
    let resolve_data =
        extract_mcp_tool_data(&resolve_response.result.expect("resolve must return result"));
    assert_eq!(resolve_data["policy"]["selected_profile"], "affect_repair");
    assert_eq!(resolve_data["state_vector_len"], 6);
    let e14_weight = resolve_data["weights"]
        .as_array()
        .expect("weights must be an array")
        .iter()
        .find(|item| item["embedder"] == "E14_BgeM3Dense")
        .and_then(|item| item["weight"].as_f64())
        .expect("E14_BgeM3Dense weight must be present");
    assert!((e14_weight - 0.2).abs() < 0.0001);
    println!(
        "AFTER RESOLVE selected={} source_of_truth={}",
        resolve_data["policy"]["selected_profile"], resolve_data["source_of_truth"]
    );

    let invalid = make_request(
        "tools/call",
        Some(JsonRpcId::Number(503)),
        Some(json!({
            "name": "resolve_learner_retrieval_policy",
            "arguments": {
                "learnerId": learner_id,
                "sessionTs": session_ts,
                "baseWeightProfile": "missing_profile"
            }
        })),
    );
    let invalid_response = handlers.dispatch(invalid).await;
    assert!(
        invalid_response.error.is_none(),
        "tool errors must be returned as MCP tool results"
    );
    let invalid_result = invalid_response
        .result
        .expect("invalid call returns tool result");
    assert_eq!(invalid_result["isError"], true);
    println!(
        "EDGE INVALID_PROFILE state_history={} isError={}",
        rocksdb_store.count_learner_state_history().await.unwrap(),
        invalid_result["isError"]
    );
}

#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_derive_constellation_fsv() {
    println!("\n=== E2E TEST: Derived Constellation FSV ===");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;
    let rocksdb_store = store
        .as_any()
        .downcast_ref::<RocksDbTeleologicalStore>()
        .expect("FSV requires RocksDbTeleologicalStore");

    println!(
        "SOURCE OF TRUTH: RocksDB CF_CONSTELLATIONS before={}",
        rocksdb_store.count_constellations().await.unwrap()
    );

    let source_fps = (0..3)
        .map(|seed| fsv_fingerprint_with_e1_axis(seed, 0, 1.0))
        .collect::<Vec<_>>();
    let other_fps = (10..13)
        .map(|seed| fsv_fingerprint_with_e1_axis(seed, 1, 1.0))
        .collect::<Vec<_>>();
    let antipole_fps = (20..23)
        .map(|seed| fsv_fingerprint_with_e1_axis(seed, 0, -1.0))
        .collect::<Vec<_>>();

    for fp in source_fps
        .iter()
        .chain(other_fps.iter())
        .chain(antipole_fps.iter())
    {
        let stored_id = store.store(fp.clone()).await.expect("store fingerprint");
        assert_eq!(stored_id, fp.id);
        assert!(
            store.retrieve(fp.id).await.unwrap().is_some(),
            "fingerprint must read back from CF_FINGERPRINTS"
        );
    }

    let source = compile_constellation(
        ConstellationSelector::ExplicitIds {
            rationale: "fsv source".into(),
            ids: source_fps.iter().map(|fp| fp.id).collect(),
        },
        "fsv-source".into(),
        source_fps
            .iter()
            .map(|fp| (fp.id, fp.semantic.clone(), None))
            .collect::<Vec<_>>(),
        &SynergyMatrix::with_base_synergies(),
    )
    .expect("compile source");
    let other = compile_constellation(
        ConstellationSelector::ExplicitIds {
            rationale: "fsv other".into(),
            ids: other_fps.iter().map(|fp| fp.id).collect(),
        },
        "fsv-other".into(),
        other_fps
            .iter()
            .map(|fp| (fp.id, fp.semantic.clone(), None))
            .collect::<Vec<_>>(),
        &SynergyMatrix::with_base_synergies(),
    )
    .expect("compile other");
    rocksdb_store
        .store_constellation(&source)
        .await
        .expect("store source constellation");
    rocksdb_store
        .store_constellation(&other)
        .await
        .expect("store other constellation");
    assert_eq!(rocksdb_store.count_constellations().await.unwrap(), 2);
    println!(
        "AFTER SEED constellations={} source={} other={}",
        rocksdb_store.count_constellations().await.unwrap(),
        source.id,
        other.id
    );

    let interpolate = make_request(
        "tools/call",
        Some(JsonRpcId::Number(601)),
        Some(json!({
            "name": "derive_constellation",
            "arguments": {
                "operation": "interpolate",
                "sourceConstellationId": source.id.to_string(),
                "otherConstellationId": other.id.to_string(),
                "alpha": 0.25,
                "label": "fsv-interpolate"
            }
        })),
    );
    let interpolate_response = handlers.dispatch(interpolate).await;
    assert!(
        interpolate_response.error.is_none(),
        "derive_constellation JSON-RPC error: {:?}",
        interpolate_response.error
    );
    let interpolate_data =
        extract_mcp_tool_data(&interpolate_response.result.expect("derive returns result"));
    assert_eq!(interpolate_data["status"], "stored");
    let derived_id = interpolate_data["derived_constellation_id"]
        .as_str()
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
        .expect("derived UUID");
    let derived = rocksdb_store
        .get_constellation(derived_id)
        .await
        .unwrap()
        .expect("derived constellation must physically exist");
    assert!((derived.per_embedder[0].centroid[0] - 0.75).abs() < 0.0001);
    assert!((derived.per_embedder[0].centroid[1] - 0.25).abs() < 0.0001);
    println!(
        "HAPPY INTERPOLATE after={} derived={} e1[0]={} e1[1]={}",
        rocksdb_store.count_constellations().await.unwrap(),
        derived.id,
        derived.per_embedder[0].centroid[0],
        derived.per_embedder[0].centroid[1]
    );

    let before_invalid = rocksdb_store.count_constellations().await.unwrap();
    let invalid_alpha = make_request(
        "tools/call",
        Some(JsonRpcId::Number(602)),
        Some(json!({
            "name": "derive_constellation",
            "arguments": {
                "operation": "interpolate",
                "sourceConstellationId": source.id.to_string(),
                "otherConstellationId": other.id.to_string(),
                "alpha": 1.5,
                "label": "invalid-alpha"
            }
        })),
    );
    let invalid_alpha_response = handlers.dispatch(invalid_alpha).await;
    let invalid_alpha_result = invalid_alpha_response.result.expect("tool result");
    assert_eq!(invalid_alpha_result["isError"], true);
    assert_eq!(
        rocksdb_store.count_constellations().await.unwrap(),
        before_invalid
    );
    println!(
        "EDGE INVALID_ALPHA before={} after={} isError={}",
        before_invalid,
        rocksdb_store.count_constellations().await.unwrap(),
        invalid_alpha_result["isError"]
    );

    let before_missing = rocksdb_store.count_constellations().await.unwrap();
    let missing_other = make_request(
        "tools/call",
        Some(JsonRpcId::Number(603)),
        Some(json!({
            "name": "derive_constellation",
            "arguments": {
                "operation": "add",
                "sourceConstellationId": source.id.to_string(),
                "otherConstellationId": "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
                "label": "missing-other"
            }
        })),
    );
    let missing_other_response = handlers.dispatch(missing_other).await;
    let missing_other_result = missing_other_response.result.expect("tool result");
    assert_eq!(missing_other_result["isError"], true);
    assert_eq!(
        rocksdb_store.count_constellations().await.unwrap(),
        before_missing
    );
    println!(
        "EDGE MISSING_OTHER before={} after={} isError={}",
        before_missing,
        rocksdb_store.count_constellations().await.unwrap(),
        missing_other_result["isError"]
    );

    let antipole = make_request(
        "tools/call",
        Some(JsonRpcId::Number(604)),
        Some(json!({
            "name": "derive_constellation",
            "arguments": {
                "operation": "anti_pole",
                "sourceConstellationId": source.id.to_string(),
                "label": "fsv-anti-pole",
                "selector": "explicit_ids",
                "memoryIds": antipole_fps.iter().map(|fp| fp.id.to_string()).collect::<Vec<_>>(),
                "selectedMembers": 3
            }
        })),
    );
    let antipole_response = handlers.dispatch(antipole).await;
    assert!(
        antipole_response.error.is_none(),
        "anti_pole JSON-RPC error: {:?}",
        antipole_response.error
    );
    let antipole_data = extract_mcp_tool_data(&antipole_response.result.expect("anti_pole result"));
    let antipole_id = antipole_data["derived_constellation_id"]
        .as_str()
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
        .expect("anti-pole UUID");
    let antipole_readback = rocksdb_store
        .get_constellation(antipole_id)
        .await
        .unwrap()
        .expect("anti-pole constellation must physically exist");
    assert!(antipole_readback.per_embedder[0].centroid[0] < -0.99);
    println!(
        "HAPPY ANTI_POLE after={} derived={} e1[0]={}",
        rocksdb_store.count_constellations().await.unwrap(),
        antipole_readback.id,
        antipole_readback.per_embedder[0].centroid[0]
    );

    let before_short_pool = rocksdb_store.count_constellations().await.unwrap();
    let short_pool = make_request(
        "tools/call",
        Some(JsonRpcId::Number(605)),
        Some(json!({
            "name": "derive_constellation",
            "arguments": {
                "operation": "anti_pole",
                "sourceConstellationId": source.id.to_string(),
                "label": "short-anti-pole",
                "selector": "explicit_ids",
                "memoryIds": antipole_fps.iter().take(2).map(|fp| fp.id.to_string()).collect::<Vec<_>>(),
                "selectedMembers": 3
            }
        })),
    );
    let short_pool_response = handlers.dispatch(short_pool).await;
    let short_pool_result = short_pool_response.result.expect("tool result");
    assert_eq!(short_pool_result["isError"], true);
    assert_eq!(
        rocksdb_store.count_constellations().await.unwrap(),
        before_short_pool
    );
    println!(
        "EDGE SHORT_ANTI_POOL before={} after={} isError={}",
        before_short_pool,
        rocksdb_store.count_constellations().await.unwrap(),
        short_pool_result["isError"]
    );
}

#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_estimate_learning_outcome_fsv() {
    println!("\n=== E2E TEST: Learning Outcome Estimator FSV ===");

    let (handlers, store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;
    let rocksdb_store = store
        .as_any()
        .downcast_ref::<RocksDbTeleologicalStore>()
        .expect("FSV requires RocksDbTeleologicalStore");

    println!(
        "SOURCE OF TRUTH: RocksDB CF_LEARNING_EVENTS before={}",
        rocksdb_store.count_learning_events().await.unwrap()
    );

    let useful_id = "11111111-1111-4111-8111-111111111111";
    let harmful_id = "22222222-2222-4222-8222-222222222222";
    let neutral_id = "33333333-3333-4333-8333-333333333333";
    for (idx, (event_id, before_value, after_value, label, utility, domain)) in [
        (useful_id, 0.20, 0.80, "useful", 0.80, "math"),
        (harmful_id, 0.80, 0.20, "harmful", -0.75, "math"),
        (neutral_id, 0.40, 0.42, "neutral", 0.02, "language"),
    ]
    .into_iter()
    .enumerate()
    {
        let record = make_request(
            "tools/call",
            Some(JsonRpcId::Number(650 + idx as i64)),
            Some(json!({
                "name": "record_learning_event",
                "arguments": {
                    "eventId": event_id,
                    "taskId": "outcome-fsv-task",
                    "query": "synthetic but persisted outcome FSV",
                    "retrievedContext": "controlled context",
                    "assistantResponse": "controlled response",
                    "before": learning_state_json(before_value, 5, domain),
                    "after": learning_state_json(after_value, 1, domain),
                    "outcome": {
                        "label": label,
                        "utilityDelta": utility,
                        "correctionRequired": label == "harmful",
                        "reuseObserved": label == "useful"
                    }
                }
            })),
        );
        let response = handlers.dispatch(record).await;
        assert!(
            response.error.is_none(),
            "record_learning_event JSON-RPC error: {:?}",
            response.error
        );
        let data = extract_mcp_tool_data(&response.result.expect("record result"));
        assert_eq!(data["status"], "stored");
    }

    assert_eq!(rocksdb_store.count_learning_events().await.unwrap(), 3);
    let useful_readback = rocksdb_store
        .get_learning_event(uuid::Uuid::parse_str(useful_id).unwrap())
        .await
        .unwrap()
        .expect("useful event must physically exist");
    assert_eq!(outcome_label_name(useful_readback.outcome.label), "useful");
    assert!((useful_readback.outcome.utility_delta - 0.80).abs() < 0.0001);
    println!(
        "AFTER RECORD learning_events={} useful_delta={} feature_delta_e={}",
        rocksdb_store.count_learning_events().await.unwrap(),
        useful_readback.outcome.utility_delta,
        useful_readback.features.delta_e_scalar
    );

    let estimate = make_request(
        "tools/call",
        Some(JsonRpcId::Number(670)),
        Some(json!({
            "name": "estimate_learning_outcome",
            "arguments": {
                "before": learning_state_json(0.20, 5, "math"),
                "candidateAfter": learning_state_json(0.82, 1, "math"),
                "maxNeighbors": 2,
                "domain": "math",
                "includeNeighbors": true
            }
        })),
    );
    let estimate_response = handlers.dispatch(estimate).await;
    assert!(
        estimate_response.error.is_none(),
        "estimate_learning_outcome JSON-RPC error: {:?}",
        estimate_response.error
    );
    let estimate_data = extract_mcp_tool_data(&estimate_response.result.expect("estimate result"));
    assert_eq!(estimate_data["state_mutated"], false);
    assert_eq!(estimate_data["before_count"], 3);
    assert_eq!(estimate_data["after_count"], 3);
    let predicted = estimate_data["predicted_utility_delta"]
        .as_f64()
        .expect("prediction numeric");
    assert!(
        predicted > 0.60,
        "expected useful prediction from nearest useful case, got {predicted}"
    );
    assert_eq!(estimate_data["predicted_label"], "useful");
    println!(
        "HAPPY ESTIMATE before={} after={} predicted={} label={} neighbors={}",
        estimate_data["before_count"],
        estimate_data["after_count"],
        estimate_data["predicted_utility_delta"],
        estimate_data["predicted_label"],
        estimate_data["matched_neighbors"]
    );

    let before_invalid_dims = rocksdb_store.count_learning_events().await.unwrap();
    let invalid_dims = make_request(
        "tools/call",
        Some(JsonRpcId::Number(671)),
        Some(json!({
            "name": "estimate_learning_outcome",
            "arguments": {
                "before": learning_state_json(0.20, 5, "math"),
                "candidateAfter": {
                    "topicProfile": vec![0.8; 14],
                    "crossCorrelations": vec![0.1; 90]
                }
            }
        })),
    );
    let invalid_dims_response = handlers.dispatch(invalid_dims).await;
    let invalid_dims_result = invalid_dims_response.result.expect("tool result");
    assert_eq!(invalid_dims_result["isError"], true);
    assert_eq!(
        rocksdb_store.count_learning_events().await.unwrap(),
        before_invalid_dims
    );
    println!(
        "EDGE INVALID_DIMS before={} after={} isError={}",
        before_invalid_dims,
        rocksdb_store.count_learning_events().await.unwrap(),
        invalid_dims_result["isError"]
    );

    let before_no_match = rocksdb_store.count_learning_events().await.unwrap();
    let no_match = make_request(
        "tools/call",
        Some(JsonRpcId::Number(672)),
        Some(json!({
            "name": "estimate_learning_outcome",
            "arguments": {
                "before": learning_state_json(0.20, 5, "math"),
                "candidateAfter": learning_state_json(0.82, 1, "math"),
                "domain": "physics"
            }
        })),
    );
    let no_match_response = handlers.dispatch(no_match).await;
    let no_match_result = no_match_response.result.expect("tool result");
    assert_eq!(no_match_result["isError"], true);
    assert_eq!(
        rocksdb_store.count_learning_events().await.unwrap(),
        before_no_match
    );
    println!(
        "EDGE NO_MATCH before={} after={} isError={}",
        before_no_match,
        rocksdb_store.count_learning_events().await.unwrap(),
        no_match_result["isError"]
    );

    let before_bad_limit = rocksdb_store.count_learning_events().await.unwrap();
    let bad_limit = make_request(
        "tools/call",
        Some(JsonRpcId::Number(673)),
        Some(json!({
            "name": "estimate_learning_outcome",
            "arguments": {
                "before": learning_state_json(0.20, 5, "math"),
                "candidateAfter": learning_state_json(0.82, 1, "math"),
                "maxNeighbors": 0
            }
        })),
    );
    let bad_limit_response = handlers.dispatch(bad_limit).await;
    let bad_limit_result = bad_limit_response.result.expect("tool result");
    assert_eq!(bad_limit_result["isError"], true);
    assert_eq!(
        rocksdb_store.count_learning_events().await.unwrap(),
        before_bad_limit
    );
    println!(
        "EDGE BAD_LIMIT before={} after={} isError={}",
        before_bad_limit,
        rocksdb_store.count_learning_events().await.unwrap(),
        bad_limit_result["isError"]
    );
}

#[cfg(feature = "llm")]
fn fsv_fingerprint_with_e1_axis(seed: u8, axis: usize, sign: f32) -> TeleologicalFingerprint {
    let mut e1 = vec![0.0f32; 1024];
    e1[axis] = sign;

    let semantic = SemanticFingerprint {
        e1_semantic: e1,
        e2_temporal_recent: fsv_dense(512, seed.wrapping_add(2)),
        e3_temporal_periodic: fsv_dense(512, seed.wrapping_add(3)),
        e4_temporal_positional: fsv_dense(512, seed.wrapping_add(4)),
        e5_causal_as_cause: fsv_dense(768, seed.wrapping_add(5)),
        e5_causal_as_effect: fsv_dense(768, seed.wrapping_add(6)),
        e5_causal: Vec::new(),
        e6_sparse: SparseVector {
            indices: vec![1, 2, seed as u16 + 10],
            values: vec![0.5, 0.3, 0.2],
        },
        e7_code: fsv_dense(1536, seed.wrapping_add(7)),
        e8_graph_as_source: fsv_dense(1024, seed.wrapping_add(8)),
        e8_graph_as_target: fsv_dense(1024, seed.wrapping_add(9)),
        e8_graph: Vec::new(),
        e9_hdc: fsv_dense(1024, seed.wrapping_add(10)),
        e10_multimodal_paraphrase: fsv_dense(768, seed.wrapping_add(11)),
        e10_multimodal_as_context: fsv_dense(768, seed.wrapping_add(12)),
        e11_entity: fsv_dense(768, seed.wrapping_add(13)),
        e12_late_interaction: vec![fsv_dense(128, seed.wrapping_add(14))],
        e13_splade: SparseVector {
            indices: vec![100, 200, seed as u16 + 300],
            values: vec![0.4, 0.4, 0.2],
        },
        e14_bge_m3_dense: fsv_dense(1024, seed.wrapping_add(15)),
    };

    TeleologicalFingerprint::new(semantic, fsv_hash(seed))
}

#[cfg(feature = "llm")]
fn fsv_dense(dim: usize, seed: u8) -> Vec<f32> {
    let mut values = (0..dim)
        .map(|idx| (((idx as u32 + seed as u32) % 31) as f32 + 1.0) / 100.0)
        .collect::<Vec<_>>();
    let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt();
    for value in &mut values {
        *value /= norm;
    }
    values
}

#[cfg(feature = "llm")]
fn fsv_hash(seed: u8) -> [u8; 32] {
    let mut hash = [0u8; 32];
    hash[0] = 0xF5;
    hash[1] = seed;
    hash[30] = seed.wrapping_mul(17);
    hash[31] = 0x5F;
    hash
}

#[cfg(feature = "llm")]
fn learning_state_json(value: f32, rank: u32, domain: &str) -> serde_json::Value {
    json!({
        "topicProfile": vec![value; 14],
        "crossCorrelations": vec![value * 0.5; NUM_CROSS_CORRELATIONS],
        "retrievalRank": rank,
        "embedderScores": vec![value; 14],
        "contradictionPressure": (1.0 - value).clamp(0.0, 1.0),
        "integrationConfidence": value.clamp(0.0, 1.0),
        "recurrenceCount": 2,
        "stabilityScore": value.clamp(0.0, 1.0),
        "domain": domain,
        "successfulTransferCount": if value > 0.5 { 1 } else { 0 }
    })
}

#[cfg(feature = "llm")]
fn outcome_label_name(label: LearningOutcomeLabel) -> &'static str {
    match label {
        LearningOutcomeLabel::Useful => "useful",
        LearningOutcomeLabel::Neutral => "neutral",
        LearningOutcomeLabel::Harmful => "harmful",
        LearningOutcomeLabel::NoLearning => "no_learning",
    }
}

/// Complete E2E test covering all tools.
/// Note: 11 tools now (inject_context merged into store_memory)
#[tokio::test]
#[cfg(feature = "llm")]
async fn test_e2e_all_11_tools_callable() {
    println!("\n=== E2E TEST: All 11 Tools Callable with GPU ===");

    let (handlers, _store, _tempdir) =
        create_test_handlers_with_real_embeddings_store_access().await;

    // Track which tools we've successfully called
    let mut successful_tools: HashSet<&str> = HashSet::new();

    // Helper to call a tool and verify it works
    async fn call_tool(
        handlers: &crate::handlers::Handlers,
        name: &str,
        arguments: serde_json::Value,
    ) -> bool {
        let params = json!({
            "name": name,
            "arguments": arguments
        });
        let request = make_request("tools/call", Some(JsonRpcId::Number(1)), Some(params));
        let response = handlers.dispatch(request).await;

        if response.error.is_some() {
            println!("  {} - JSON-RPC ERROR: {:?}", name, response.error);
            return false;
        }

        let result = match response.result {
            Some(r) => r,
            None => {
                println!("  {} - NO RESULT", name);
                return false;
            }
        };

        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_error {
            let content = result.get("content");
            println!("  {} - TOOL ERROR: {:?}", name, content);
            return false;
        }

        println!("  {} - OK", name);
        true
    }

    // First store some test data
    let store_result = call_tool(
        &handlers,
        "store_memory",
        json!({
            "content": "Test content for all tools",
            "importance": 0.7
        }),
    )
    .await;
    if store_result {
        successful_tools.insert("store_memory");
    }

    // Audit-11 TST-M1: Store TWO memories and extract both IDs for merge_concepts test.
    // Helper to extract fingerprintId from a store_memory response
    fn extract_fingerprint_id(response: &crate::protocol::JsonRpcResponse) -> String {
        response
            .result
            .as_ref()
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|data| data.get("fingerprintId").cloned())
            .and_then(|id| id.as_str().map(String::from))
            .unwrap_or_else(|| "00000000-0000-0000-0000-000000000000".to_string())
    }

    // Store second memory and extract its ID
    let params2 = json!({
        "name": "store_memory",
        "arguments": {
            "content": "Second test memory for merge target",
            "importance": 0.6
        }
    });
    let request2 = make_request("tools/call", Some(JsonRpcId::Number(99)), Some(params2));
    let response2 = handlers.dispatch(request2).await;
    let fingerprint_id = extract_fingerprint_id(&response2);

    // Store third memory for merge source (different from target)
    let params3 = json!({
        "name": "store_memory",
        "arguments": {
            "content": "Third test memory for merge source",
            "importance": 0.5
        }
    });
    let request3 = make_request("tools/call", Some(JsonRpcId::Number(98)), Some(params3));
    let response3 = handlers.dispatch(request3).await;
    let fingerprint_id_for_merge = extract_fingerprint_id(&response3);

    // Core tools (4 - inject_context merged into store_memory)
    println!("\n--- Core Tools ---");
    if call_tool(
        &handlers,
        "store_memory",
        json!({
            "content": "E2E test content",
            "rationale": "Testing all tools"
        }),
    )
    .await
    {
        successful_tools.insert("store_memory");
    }
    if call_tool(&handlers, "get_memetic_status", json!({})).await {
        successful_tools.insert("get_memetic_status");
    }
    if call_tool(
        &handlers,
        "search_graph",
        json!({
            "query": "test",
            "topK": 5
        }),
    )
    .await
    {
        successful_tools.insert("search_graph");
    }
    if call_tool(
        &handlers,
        "trigger_consolidation",
        json!({
            "strategy": "similarity"
        }),
    )
    .await
    {
        successful_tools.insert("trigger_consolidation");
    }

    // Topic tools (4)
    println!("\n--- Topic Tools ---");
    if call_tool(
        &handlers,
        "get_topic_portfolio",
        json!({
            "format": "brief"
        }),
    )
    .await
    {
        successful_tools.insert("get_topic_portfolio");
    }
    if call_tool(
        &handlers,
        "get_topic_stability",
        json!({
            "hours": 1
        }),
    )
    .await
    {
        successful_tools.insert("get_topic_stability");
    }
    if call_tool(
        &handlers,
        "detect_topics",
        json!({
            "force": false
        }),
    )
    .await
    {
        successful_tools.insert("detect_topics");
    }
    if call_tool(
        &handlers,
        "get_divergence_alerts",
        json!({
            "lookback_hours": 1
        }),
    )
    .await
    {
        successful_tools.insert("get_divergence_alerts");
    }

    // Store a 4th memory for forget_concept (separate from merge sources)
    let params4 = json!({
        "name": "store_memory",
        "arguments": {
            "content": "Fourth test memory for forget_concept",
            "importance": 0.4
        }
    });
    let request4 = make_request("tools/call", Some(JsonRpcId::Number(97)), Some(params4));
    let response4 = handlers.dispatch(request4).await;
    let fingerprint_id_for_forget = extract_fingerprint_id(&response4);

    // Curation tools (3)
    println!("\n--- Curation Tools ---");
    if call_tool(
        &handlers,
        "boost_importance",
        json!({
            "node_id": &fingerprint_id,
            "delta": 0.1
        }),
    )
    .await
    {
        successful_tools.insert("boost_importance");
    }
    // merge_concepts: use fingerprint_id + fingerprint_id_for_merge as sources
    if call_tool(
        &handlers,
        "merge_concepts",
        json!({
            "source_ids": [&fingerprint_id, &fingerprint_id_for_merge],
            "target_name": "Merged Concept",
            "rationale": "Consolidating similar memories",
            "merge_strategy": "union"
        }),
    )
    .await
    {
        successful_tools.insert("merge_concepts");
    }
    // forget_concept: use dedicated memory not consumed by merge
    if call_tool(
        &handlers,
        "forget_concept",
        json!({
            "node_id": &fingerprint_id_for_forget,
            "soft_delete": true
        }),
    )
    .await
    {
        successful_tools.insert("forget_concept");
    }

    // Summary
    println!("\n=== SUMMARY ===");
    println!("Successful tools: {}/11", successful_tools.len());
    for tool in &successful_tools {
        println!("  ✓ {}", tool);
    }

    // TST-M1 FIX: Exact assertion instead of >= to catch regressions.
    // If a tool silently breaks, >= would hide it. == ensures all 11 succeed.
    let expected = 11;
    let actual = successful_tools.len();
    let all_expected: HashSet<&str> = [
        "store_memory",
        "get_memetic_status",
        "search_graph",
        "trigger_consolidation",
        "get_topic_portfolio",
        "get_topic_stability",
        "detect_topics",
        "get_divergence_alerts",
        "boost_importance",
        "merge_concepts",
        "forget_concept",
    ]
    .into_iter()
    .collect();
    let missing: Vec<&&str> = all_expected.difference(&successful_tools).collect();
    assert_eq!(
        actual, expected,
        "Expected exactly {} tools to succeed, got {}. Missing: {:?}",
        expected, actual, missing
    );

    println!("\n=== ALL 11 TOOLS CALLABLE ===\n");
}

// =============================================================================
// Evidence Log
// =============================================================================

/// TST-01 FIX: This test previously contained ONLY println!() statements with
/// zero assertions. It always passed regardless of system state, creating false
/// confidence. Replaced with a concrete assertion that verifies the tool count
/// constant matches the expected number of registered tools.
#[test]
fn evidence_of_e2e_test_coverage() {
    // TEST-2 FIX: Actually verify tools are registered instead of counting a hardcoded array.
    let all_defs = crate::tools::definitions::get_tool_definitions();
    let registered_names: Vec<&str> = all_defs.iter().map(|d| d.name.as_str()).collect();

    let expected_core_tools = [
        "store_memory",
        "get_memetic_status",
        "search_graph",
        "trigger_consolidation",
        "get_topic_portfolio",
        "get_topic_stability",
        "detect_topics",
        "get_divergence_alerts",
        "merge_concepts",
        "forget_concept",
        "boost_importance",
    ];

    for tool_name in &expected_core_tools {
        assert!(
            registered_names.contains(tool_name),
            "Core tool '{}' not found in registered definitions. Registered: {:?}",
            tool_name,
            registered_names
        );
    }
}
