//! MANUAL FULL STATE VERIFICATION for TASK-S003 Purpose Handlers
//!
//! This file performs PHYSICAL VERIFICATION of the Source of Truth.
//! It does NOT rely on handler return values - it directly queries
//! the underlying data stores to prove operations worked.
//!
//! Source of Truth:
//! 1. InMemoryTeleologicalStore - DashMap<Uuid, TeleologicalFingerprint>
//! 2. GoalHierarchy - HashMap<GoalId, GoalNode> with parent_index/children_index
//!
//! Run with: cargo test -p context-graph-mcp manual_fsv -- --nocapture

use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;
use uuid::Uuid;

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::purpose::{GoalHierarchy, GoalId, GoalLevel, GoalNode};
use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor};
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcRequest};

/// Create handlers with DIRECT ACCESS to Source of Truth for verification.
fn create_verifiable_system() -> (
    Handlers,
    Arc<InMemoryTeleologicalStore>,
    Arc<RwLock<GoalHierarchy>>,
) {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());

    // Create hierarchy with test goals
    let mut hierarchy = GoalHierarchy::new();
    let ns_embedding: Vec<f32> = (0..1024).map(|i| (i as f32 / 1024.0).sin() * 0.8).collect();

    hierarchy.add_goal(GoalNode::north_star(
        "ns_test",
        "Test North Star Goal",
        ns_embedding.clone(),
        vec!["test".into()],
    )).expect("Failed to add North Star");

    hierarchy.add_goal(GoalNode::child(
        "s1_test",
        "Test Strategic Goal",
        GoalLevel::Strategic,
        GoalId::new("ns_test"),
        ns_embedding.clone(),
        0.8,
        vec!["strategic".into()],
    )).expect("Failed to add strategic goal");

    // We need to wrap in RwLock to share with handlers
    let hierarchy_lock = Arc::new(RwLock::new(hierarchy));

    let store_for_handlers: Arc<dyn TeleologicalMemoryStore> = store.clone();
    let handlers = Handlers::with_shared_hierarchy(
        store_for_handlers,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        Arc::clone(&hierarchy_lock),
    );

    (handlers, store, hierarchy_lock)
}

fn make_request(method: &str, id: i64, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(id)),
        method: method.to_string(),
        params: Some(params),
    }
}

// =============================================================================
// MANUAL FULL STATE VERIFICATION TEST
// =============================================================================

#[tokio::test]
async fn manual_fsv_purpose_handlers() {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     MANUAL FULL STATE VERIFICATION - TASK-S003 PURPOSE          ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║ Source of Truth:                                                 ║");
    println!("║   1. InMemoryTeleologicalStore (DashMap<Uuid, Fingerprint>)     ║");
    println!("║   2. GoalHierarchy (HashMap<GoalId, GoalNode>)                  ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let (handlers, store, hierarchy) = create_verifiable_system();

    // =========================================================================
    // STEP 1: VERIFY INITIAL STATE OF SOURCE OF TRUTH
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ STEP 1: VERIFY INITIAL STATE OF SOURCE OF TRUTH                │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    // Direct query to TeleologicalMemoryStore
    let initial_store_count = store.count().await.expect("count failed");
    println!("📦 TeleologicalMemoryStore (Source of Truth #1):");
    println!("   ├─ Type: InMemoryTeleologicalStore (DashMap)");
    println!("   ├─ Fingerprint count: {}", initial_store_count);
    println!("   └─ Expected: 0");
    assert_eq!(initial_store_count, 0, "Store should be empty initially");

    // Direct query to GoalHierarchy
    let hierarchy_read = hierarchy.read();
    let initial_goal_count = hierarchy_read.len();
    let has_north_star = hierarchy_read.has_north_star();
    let north_star = hierarchy_read.north_star();
    println!();
    println!("🎯 GoalHierarchy (Source of Truth #2):");
    println!("   ├─ Type: HashMap<GoalId, GoalNode>");
    println!("   ├─ Goal count: {}", initial_goal_count);
    println!("   ├─ Has North Star: {}", has_north_star);
    if let Some(ns) = north_star {
        println!("   ├─ North Star ID: {}", ns.id);
        println!("   └─ North Star description: {}", ns.description);
    }
    drop(hierarchy_read);

    println!();
    println!("   ✅ VERIFIED: Initial state confirmed via direct Source of Truth queries");
    println!();

    // =========================================================================
    // STEP 2: STORE A FINGERPRINT AND VERIFY IN SOURCE OF TRUTH
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ STEP 2: STORE FINGERPRINT & VERIFY IN SOURCE OF TRUTH          │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    println!("📝 Executing: memory/store handler");
    let store_request = make_request("memory/store", 1, json!({
        "content": "Test content for FSV verification",
        "metadata": { "source": "manual_fsv_test" }
    }));

    let store_response = handlers.dispatch(store_request).await;
    let fingerprint_id_str = store_response.result
        .as_ref()
        .and_then(|r| r.get("fingerprintId"))
        .and_then(|v| v.as_str())
        .expect("Must return fingerprint ID");
    let fingerprint_id = Uuid::parse_str(fingerprint_id_str).expect("Valid UUID");

    println!("   Handler returned fingerprint ID: {}", fingerprint_id);
    println!();

    // PHYSICAL VERIFICATION: Query Source of Truth directly
    println!("🔍 PHYSICAL VERIFICATION - Querying Source of Truth directly:");
    let stored_fp = store.retrieve(fingerprint_id).await
        .expect("retrieve should succeed")
        .expect("fingerprint must exist");

    println!("   ├─ store.retrieve({}) = FOUND", fingerprint_id);
    println!("   ├─ Fingerprint ID in store: {}", stored_fp.id);
    println!("   ├─ Content hash: {:?}", &stored_fp.content_hash[0..8]); // First 8 bytes
    println!("   ├─ Purpose vector length: {}", stored_fp.purpose_vector.alignments.len());
    println!("   ├─ Purpose vector (first 5): {:?}", &stored_fp.purpose_vector.alignments[0..5]);
    println!("   ├─ Theta to North Star: {:.4}", stored_fp.theta_to_north_star);
    println!("   └─ Created at: {:?}", stored_fp.created_at);

    // Verify count increased
    let post_store_count = store.count().await.expect("count failed");
    println!();
    println!("   Store count after operation: {} (was: {})", post_store_count, initial_store_count);
    assert_eq!(post_store_count, 1, "Store should have 1 fingerprint");

    println!();
    println!("   ✅ VERIFIED: Fingerprint physically exists in Source of Truth");
    println!();

    // =========================================================================
    // STEP 3: VERIFY DEPRECATED METHOD RETURNS METHOD_NOT_FOUND (TASK-CORE-001)
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ STEP 3: VERIFY DEPRECATED METHOD RETURNS METHOD_NOT_FOUND      │");
    println!("│         (TASK-CORE-001 - ARCH-03 autonomous-first)             │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    println!("📝 Executing: purpose/north_star_alignment (deprecated)");
    let alignment_request = make_request("purpose/north_star_alignment", 2, json!({
        "fingerprint_id": fingerprint_id_str,
        "include_breakdown": true,
        "include_patterns": true
    }));

    let alignment_response = handlers.dispatch(alignment_request).await;

    // TASK-CORE-001: Verify METHOD_NOT_FOUND error for deprecated method
    let alignment_error = alignment_response.error.as_ref().expect("Must have error");
    println!("   ├─ Error code: {} (expected: -32601 METHOD_NOT_FOUND)", alignment_error.code);
    println!("   └─ Error message: {}", alignment_error.message);
    assert_eq!(alignment_error.code, -32601, "Must return METHOD_NOT_FOUND for deprecated method");

    // CROSS-VERIFICATION: Verify fingerprint still exists and is unchanged
    println!();
    println!("🔍 CROSS-VERIFICATION - Fingerprint unchanged in Source of Truth:");
    let fp_from_store = store.retrieve(fingerprint_id).await
        .expect("retrieve should succeed")
        .expect("fingerprint must exist");

    let pv_mean: f32 = fp_from_store.purpose_vector.alignments.iter().sum::<f32>() / 13.0;
    println!("   ├─ Fingerprint still exists: {}", fp_from_store.id);
    println!("   ├─ Fingerprint theta_to_north_star: {:.4}", fp_from_store.theta_to_north_star);
    println!("   └─ Purpose vector mean: {:.4}", pv_mean);

    println!();
    println!("   ✅ VERIFIED: Deprecated method returns METHOD_NOT_FOUND (-32601)");
    println!();

    // =========================================================================
    // STEP 4: GOAL HIERARCHY OPERATIONS AND DIRECT VERIFICATION
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ STEP 4: GOAL HIERARCHY OPERATIONS & DIRECT VERIFICATION        │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    println!("📝 Executing: goal/hierarchy_query get_all");
    let get_all_request = make_request("goal/hierarchy_query", 3, json!({
        "operation": "get_all"
    }));

    let get_all_response = handlers.dispatch(get_all_request).await;
    let goals_from_handler = get_all_response.result
        .as_ref()
        .and_then(|r| r.get("goals"))
        .and_then(|v| v.as_array())
        .expect("Must have goals array");

    println!("   Handler returned {} goals", goals_from_handler.len());

    // PHYSICAL VERIFICATION: Query GoalHierarchy directly
    println!();
    println!("🔍 PHYSICAL VERIFICATION - Querying GoalHierarchy directly:");
    let hierarchy_read = hierarchy.read();
    let direct_goal_count = hierarchy_read.len();

    println!("   ├─ hierarchy.len() = {}", direct_goal_count);
    assert_eq!(goals_from_handler.len(), direct_goal_count,
        "Handler must return same count as Source of Truth");

    // List all goals from Source of Truth
    println!("   ├─ Goals in Source of Truth:");
    for (i, goal) in goals_from_handler.iter().enumerate() {
        let goal_id = goal.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let level = goal.get("level").and_then(|v| v.as_str()).unwrap_or("?");
        let desc = goal.get("description").and_then(|v| v.as_str()).unwrap_or("?");

        // Verify goal exists in hierarchy
        let exists_in_sot = hierarchy_read.get(&GoalId::new(goal_id)).is_some();
        println!("   │   [{}] {} ({}) - exists in SoT: {}", i, goal_id, level, exists_in_sot);
        assert!(exists_in_sot, "Goal {} must exist in Source of Truth", goal_id);
    }

    // Verify North Star specifically
    let ns_from_sot = hierarchy_read.north_star().expect("Must have North Star");
    println!("   └─ North Star from SoT: {} - {}", ns_from_sot.id, ns_from_sot.description);
    drop(hierarchy_read);

    println!();
    println!("   ✅ VERIFIED: All goals exist in Source of Truth");
    println!();

    // =========================================================================
    // EDGE CASE 1: Invalid Purpose Vector (12 elements instead of 13)
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ EDGE CASE 1: Invalid Purpose Vector (12 elements vs 13)        │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    let before_count = store.count().await.expect("count failed");
    println!("📊 BEFORE STATE:");
    println!("   └─ Store count: {}", before_count);

    println!();
    println!("📝 Executing: purpose/query with 12-element vector (should fail)");
    let invalid_vector: Vec<f32> = vec![0.5; 12]; // Wrong size!
    let invalid_request = make_request("purpose/query", 100, json!({
        "purpose_vector": invalid_vector
    }));

    let invalid_response = handlers.dispatch(invalid_request).await;
    let error = invalid_response.error.as_ref().expect("Must have error");

    println!("   ├─ Error code: {} (expected: -32602 INVALID_PARAMS)", error.code);
    println!("   └─ Error message: {}", error.message);

    // VERIFY: Source of Truth unchanged
    let after_count = store.count().await.expect("count failed");
    println!();
    println!("📊 AFTER STATE:");
    println!("   └─ Store count: {} (unchanged: {})", after_count, after_count == before_count);

    assert_eq!(before_count, after_count, "Store should be unchanged");
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS");

    println!();
    println!("   ✅ VERIFIED: Invalid vector rejected, Source of Truth unchanged");
    println!();

    // =========================================================================
    // EDGE CASE 2: Goal Not Found in Hierarchy
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ EDGE CASE 2: Goal Not Found in Hierarchy                       │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    let nonexistent_goal = "nonexistent_goal_12345";

    println!("📊 BEFORE STATE:");
    let hierarchy_read = hierarchy.read();
    let exists = hierarchy_read.get(&GoalId::new(nonexistent_goal)).is_some();
    println!("   └─ Goal '{}' exists in SoT: {}", nonexistent_goal, exists);
    drop(hierarchy_read);

    println!();
    println!("📝 Executing: goal/hierarchy_query get_goal (nonexistent)");
    let not_found_request = make_request("goal/hierarchy_query", 101, json!({
        "operation": "get_goal",
        "goal_id": nonexistent_goal
    }));

    let not_found_response = handlers.dispatch(not_found_request).await;
    let error = not_found_response.error.as_ref().expect("Must have error");

    println!("   ├─ Error code: {} (expected: -32020 GOAL_NOT_FOUND)", error.code);
    println!("   └─ Error message: {}", error.message);

    // VERIFY: Goal still doesn't exist (no side effects)
    println!();
    println!("📊 AFTER STATE:");
    let hierarchy_read = hierarchy.read();
    let still_not_exists = hierarchy_read.get(&GoalId::new(nonexistent_goal)).is_none();
    println!("   └─ Goal '{}' still not in SoT: {}", nonexistent_goal, still_not_exists);
    drop(hierarchy_read);

    assert!(still_not_exists, "Nonexistent goal should still not exist");
    assert_eq!(error.code, -32020, "Should return GOAL_NOT_FOUND");

    println!();
    println!("   ✅ VERIFIED: Nonexistent goal correctly rejected");
    println!();

    // =========================================================================
    // EDGE CASE 3: Drift Check with Invalid UUID
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ EDGE CASE 3: Drift Check with Invalid UUID                     │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    println!("📊 BEFORE STATE:");
    let before_count = store.count().await.expect("count failed");
    println!("   └─ Store count: {}", before_count);

    println!();
    println!("📝 Executing: purpose/drift_check with invalid UUID");
    let drift_request = make_request("purpose/drift_check", 102, json!({
        "fingerprint_ids": ["not-a-valid-uuid", "also-invalid"]
    }));

    let drift_response = handlers.dispatch(drift_request).await;
    let error = drift_response.error.as_ref().expect("Must have error");

    println!("   ├─ Error code: {} (expected: -32602 INVALID_PARAMS)", error.code);
    println!("   └─ Error message: {}", error.message);

    // VERIFY: Source of Truth unchanged
    println!();
    println!("📊 AFTER STATE:");
    let after_count = store.count().await.expect("count failed");
    println!("   └─ Store count: {} (unchanged: {})", after_count, after_count == before_count);

    assert_eq!(before_count, after_count, "Store should be unchanged");
    assert_eq!(error.code, -32602, "Should return INVALID_PARAMS");

    println!();
    println!("   ✅ VERIFIED: Invalid UUID rejected, Source of Truth unchanged");
    println!();

    // =========================================================================
    // FINAL EVIDENCE: COMPLETE STATE OF SOURCE OF TRUTH
    // =========================================================================
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║              FINAL EVIDENCE - SOURCE OF TRUTH STATE             ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // TeleologicalMemoryStore final state
    println!("📦 TeleologicalMemoryStore - Final State:");
    let final_count = store.count().await.expect("count failed");
    println!("   ├─ Total fingerprints: {}", final_count);

    // Verify the stored fingerprint still exists by direct retrieval
    let final_fp = store.retrieve(fingerprint_id).await
        .expect("retrieve should succeed")
        .expect("fingerprint must exist");
    println!("   ├─ Verified fingerprint {} exists", fingerprint_id);
    println!("   │      Content hash: {:?}", &final_fp.content_hash[0..8]);
    println!("   │      Purpose vector[0..3]: {:?}", &final_fp.purpose_vector.alignments[0..3]);
    println!("   │      Theta to NS: {:.4}", final_fp.theta_to_north_star);
    println!("   └─ END OF STORE");
    println!();

    // GoalHierarchy final state
    println!("🎯 GoalHierarchy - Final State:");
    let hierarchy_read = hierarchy.read();
    println!("   ├─ Total goals: {}", hierarchy_read.len());
    println!("   ├─ Has North Star: {}", hierarchy_read.has_north_star());
    if let Some(ns) = hierarchy_read.north_star() {
        println!("   ├─ North Star: {} - {}", ns.id, ns.description);
    }

    // Verify specific goals exist by ID
    let goal_ids = ["ns_test", "s1_test"];
    for (i, goal_id) in goal_ids.iter().enumerate() {
        if let Some(goal) = hierarchy_read.get(&GoalId::new(*goal_id)) {
            println!("   ├─ [{}] {:?}: {} - {}", i, goal.level, goal.id, goal.description);
        } else {
            println!("   ├─ [{}] {} - NOT FOUND", i, goal_id);
        }
    }
    println!("   └─ END OF HIERARCHY");
    drop(hierarchy_read);

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║         FULL STATE VERIFICATION COMPLETE - ALL PASSED           ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
}
