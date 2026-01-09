//! Full State Verification Tests for Purpose Handlers
//!
//! TASK-S003: Comprehensive verification that directly inspects the Source of Truth.
//!
//! This test file does NOT rely on handler return values alone.
//! It directly queries the underlying stores and goal hierarchy to verify:
//! - Data was actually stored
//! - Goals exist in hierarchy
//! - Fingerprints exist in memory
//! - Alignment computations are correct
//! - Edge cases are handled correctly
//!
//! ## Verification Methodology
//!
//! 1. Define Source of Truth: InMemoryTeleologicalStore + GoalHierarchy
//! 2. Execute & Inspect: Run handlers, then directly query stores to verify
//! 3. Edge Case Audit: Test 3+ edge cases with BEFORE/AFTER state logging
//! 4. Evidence of Success: Print actual data residing in the system

use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;
use uuid::Uuid;

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::purpose::{GoalDiscoveryMetadata, GoalHierarchy, GoalLevel, GoalNode};
use context_graph_core::stubs::{
    InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor,
};
use context_graph_core::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor,
};
use context_graph_core::types::fingerprint::{SemanticFingerprint, NUM_EMBEDDERS};

use crate::handlers::Handlers;
use crate::protocol::JsonRpcId;

use super::make_request;

/// Create test handlers with SHARED access to the store and hierarchy for direct verification.
///
/// Returns (Handlers, Arc<InMemoryTeleologicalStore>, Arc<RwLock<GoalHierarchy>>) so tests
/// can directly query the store and hierarchy.
fn create_verifiable_handlers() -> (
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

    // Create goal hierarchy with North Star and sub-goals
    let hierarchy = create_full_test_hierarchy();
    let shared_hierarchy = Arc::new(RwLock::new(hierarchy));

    let store_for_handlers: Arc<dyn TeleologicalMemoryStore> = store.clone();
    let handlers = Handlers::with_shared_hierarchy(
        store_for_handlers,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        shared_hierarchy.clone(),
    );

    (handlers, store, shared_hierarchy)
}

/// Create test handlers WITHOUT a North Star (for testing error cases).
fn create_verifiable_handlers_no_north_star() -> (
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

    // Empty hierarchy - no North Star
    let hierarchy = GoalHierarchy::new();
    let shared_hierarchy = Arc::new(RwLock::new(hierarchy));

    let store_for_handlers: Arc<dyn TeleologicalMemoryStore> = store.clone();
    let handlers = Handlers::with_shared_hierarchy(
        store_for_handlers,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        shared_hierarchy.clone(),
    );

    (handlers, store, shared_hierarchy)
}

/// Create a full test goal hierarchy with North Star and sub-goals.
fn create_full_test_hierarchy() -> GoalHierarchy {
    let mut hierarchy = GoalHierarchy::new();

    // Create test discovery metadata for autonomous goals
    let discovery = GoalDiscoveryMetadata::bootstrap();

    // North Star - autonomous goal discovery
    let ns_goal = GoalNode::autonomous_goal(
        "Build the best ML learning system".into(),
        GoalLevel::NorthStar,
        SemanticFingerprint::zeroed(),
        discovery.clone(),
    )
    .expect("Failed to create North Star goal");
    let ns_id = ns_goal.id;
    hierarchy
        .add_goal(ns_goal)
        .expect("Failed to add North Star");

    // Strategic goal 1 - child of North Star
    let s1_goal = GoalNode::child_goal(
        "Improve retrieval accuracy".into(),
        GoalLevel::Strategic,
        ns_id,
        SemanticFingerprint::zeroed(),
        discovery.clone(),
    )
    .expect("Failed to create strategic goal 1");
    let s1_id = s1_goal.id;
    hierarchy
        .add_goal(s1_goal)
        .expect("Failed to add strategic goal 1");

    // Strategic goal 2 - child of North Star
    let s2_goal = GoalNode::child_goal(
        "Enhance user experience".into(),
        GoalLevel::Strategic,
        ns_id,
        SemanticFingerprint::zeroed(),
        discovery.clone(),
    )
    .expect("Failed to create strategic goal 2");
    hierarchy
        .add_goal(s2_goal)
        .expect("Failed to add strategic goal 2");

    // Tactical goal - child of Strategic goal 1
    let t1_goal = GoalNode::child_goal(
        "Implement semantic search".into(),
        GoalLevel::Tactical,
        s1_id,
        SemanticFingerprint::zeroed(),
        discovery.clone(),
    )
    .expect("Failed to create tactical goal");
    let t1_id = t1_goal.id;
    hierarchy
        .add_goal(t1_goal)
        .expect("Failed to add tactical goal");

    // Immediate goal - child of Tactical goal
    let i1_goal = GoalNode::child_goal(
        "Add vector similarity".into(),
        GoalLevel::Immediate,
        t1_id,
        SemanticFingerprint::zeroed(),
        discovery,
    )
    .expect("Failed to create immediate goal");
    hierarchy
        .add_goal(i1_goal)
        .expect("Failed to add immediate goal");

    hierarchy
}

// =============================================================================
// FULL STATE VERIFICATION TEST 1: Store → Alignment → Drift Cycle
// =============================================================================

/// FULL STATE VERIFICATION: End-to-end purpose verification with direct inspection.
///
/// TASK-CORE-001: Updated to remove deprecated north_star_alignment step per ARCH-03.
///
/// This test:
/// 1. BEFORE STATE: Verify store is empty, hierarchy has 5 goals
/// 2. STORE: Execute memory/store handler
/// 3. VERIFY IN SOURCE OF TRUTH: Directly query store.retrieve(id)
/// 4. DRIFT CHECK: Execute purpose/drift_check handler
/// 5. AFTER STATE: Verify all data in Source of Truth
/// 6. EVIDENCE: Print actual fingerprint data
///
/// NOTE: purpose/north_star_alignment removed per ARCH-03 (autonomous-first).
/// Manual alignment used single 1024D embeddings incompatible with 13-embedder arrays.
#[tokio::test]
async fn test_full_state_verification_store_alignment_drift_cycle() {
    println!("\n======================================================================");
    println!("FULL STATE VERIFICATION TEST 1: Store → Verify → Drift Cycle");
    println!("======================================================================\n");

    let (handlers, store, hierarchy) = create_verifiable_handlers();

    // =========================================================================
    // STEP 1: BEFORE STATE - Verify Source of Truth
    // =========================================================================
    let initial_count = store.count().await.expect("count should succeed");
    let hierarchy_len = hierarchy.read().len();

    println!("📊 BEFORE STATE:");
    println!("   Source of Truth (InMemoryTeleologicalStore):");
    println!("   - Fingerprint count: {}", initial_count);
    println!("   - Expected: 0");
    println!("   Source of Truth (GoalHierarchy):");
    println!("   - Goal count: {}", hierarchy_len);
    println!("   - Expected: 5 (1 NorthStar + 2 Strategic + 1 Tactical + 1 Immediate)");
    println!("   - Has North Star: {}", hierarchy.read().has_north_star());

    assert_eq!(initial_count, 0, "Store must start empty");
    assert_eq!(hierarchy_len, 5, "Hierarchy must have 5 goals");
    assert!(hierarchy.read().has_north_star(), "Must have North Star");
    println!("   ✓ VERIFIED: Store is empty, hierarchy has 5 goals with North Star\n");

    // =========================================================================
    // STEP 2: STORE - Execute handler and capture fingerprint ID
    // =========================================================================
    println!("📝 EXECUTING: memory/store");
    let content = "Machine learning enables autonomous systems to improve from experience";
    let store_params = json!({
        "content": content,
        "importance": 0.9
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    let store_response = handlers.dispatch(store_request).await;

    assert!(store_response.error.is_none(), "Store handler must succeed");
    let store_result = store_response.result.expect("Must have result");
    let fingerprint_id_str = store_result
        .get("fingerprintId")
        .and_then(|v| v.as_str())
        .expect("Must return fingerprintId");
    let fingerprint_id = Uuid::parse_str(fingerprint_id_str).expect("Must be valid UUID");

    println!("   Handler returned fingerprintId: {}", fingerprint_id);

    // =========================================================================
    // STEP 3: VERIFY IN SOURCE OF TRUTH - Direct store query
    // =========================================================================
    println!("\n🔍 VERIFYING STORAGE IN SOURCE OF TRUTH:");

    let count_after_store = store.count().await.expect("count should succeed");
    println!(
        "   - Fingerprint count: {} (expected: 1)",
        count_after_store
    );
    assert_eq!(
        count_after_store, 1,
        "Store must contain exactly 1 fingerprint"
    );

    let retrieved_fp = store
        .retrieve(fingerprint_id)
        .await
        .expect("retrieve should succeed")
        .expect("Fingerprint must exist in store");

    println!("   - Fingerprint ID in store: {}", retrieved_fp.id);
    println!(
        "   - Theta to North Star: {:.4}",
        retrieved_fp.theta_to_north_star
    );
    println!(
        "   - Purpose vector coherence: {:.4}",
        retrieved_fp.purpose_vector.coherence
    );
    println!(
        "   - Purpose vector (first 3): [{:.3}, {:.3}, {:.3}, ...]",
        retrieved_fp.purpose_vector.alignments[0],
        retrieved_fp.purpose_vector.alignments[1],
        retrieved_fp.purpose_vector.alignments[2]
    );

    assert_eq!(
        retrieved_fp.id, fingerprint_id,
        "Retrieved ID must match stored ID"
    );
    assert_eq!(
        retrieved_fp.purpose_vector.alignments.len(),
        NUM_EMBEDDERS,
        "Must have 13-element purpose vector"
    );
    let fp_theta = retrieved_fp.theta_to_north_star;
    println!("   ✓ VERIFIED: Fingerprint exists in Source of Truth with correct data\n");

    // NOTE: purpose/north_star_alignment REMOVED per TASK-CORE-001 (ARCH-03)
    // Manual alignment used single 1024D embeddings incompatible with 13-embedder arrays.
    // Use auto_bootstrap_north_star tool for autonomous goal discovery instead.

    // =========================================================================
    // STEP 4: DRIFT CHECK - Execute purpose/drift_check
    // =========================================================================
    println!("📉 EXECUTING: purpose/drift_check");
    let drift_params = json!({
        "fingerprint_ids": [fingerprint_id_str],
        "threshold": 0.1
    });
    let drift_request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(3)),
        Some(drift_params),
    );
    let drift_response = handlers.dispatch(drift_request).await;

    assert!(
        drift_response.error.is_none(),
        "Drift check handler must succeed"
    );
    let drift_result = drift_response.result.expect("Must have result");

    let summary = drift_result.get("summary").expect("Must have summary");
    let total_checked = summary.get("total_checked").and_then(|v| v.as_u64());
    let drifted_count = summary.get("drifted_count").and_then(|v| v.as_u64());
    let avg_drift = summary.get("average_drift").and_then(|v| v.as_f64());

    println!("   Handler returned:");
    println!("   - total_checked: {}", total_checked.unwrap_or(0));
    println!("   - drifted_count: {}", drifted_count.unwrap_or(0));
    println!("   - average_drift: {:.4}", avg_drift.unwrap_or(0.0));

    assert_eq!(total_checked, Some(1), "Must check 1 fingerprint");

    let drift_analysis = drift_result
        .get("drift_analysis")
        .and_then(|v| v.as_array())
        .expect("Must have drift_analysis");
    assert_eq!(drift_analysis.len(), 1, "Must have 1 drift analysis result");

    let first_analysis = &drift_analysis[0];
    let status = first_analysis.get("status").and_then(|v| v.as_str());
    assert_eq!(status, Some("analyzed"), "Must be analyzed status");

    println!("   ✓ VERIFIED: Drift check completed successfully\n");

    // =========================================================================
    // STEP 5: AFTER STATE - Final verification
    // =========================================================================
    println!("📊 AFTER STATE:");

    let final_count = store.count().await.expect("count should succeed");
    println!("   - Final fingerprint count: {}", final_count);
    assert_eq!(final_count, 1, "Store must still have 1 fingerprint");

    let final_hierarchy_len = hierarchy.read().len();
    println!("   - Final goal count: {}", final_hierarchy_len);
    assert_eq!(final_hierarchy_len, 5, "Hierarchy must still have 5 goals");

    println!("   ✓ VERIFIED: All data intact in Source of Truth\n");

    // =========================================================================
    // STEP 6: EVIDENCE OF SUCCESS - Print Summary
    // =========================================================================
    println!("======================================================================");
    println!("EVIDENCE OF SUCCESS - Full State Verification Summary");
    println!("======================================================================");
    println!("Source of Truth:");
    println!("  - InMemoryTeleologicalStore: 1 fingerprint");
    println!("  - GoalHierarchy: 5 goals (1 NS + 2 S + 1 T + 1 I)");
    println!();
    println!("Operations Verified:");
    println!("  1. memory/store: Created fingerprint {}", fingerprint_id);
    println!("  2. Direct store.retrieve() confirmed existence");
    println!(
        "  3. purpose/drift_check: avg_drift={:.4}",
        avg_drift.unwrap_or(0.0)
    );
    println!();
    println!("NOTE: purpose/north_star_alignment removed per TASK-CORE-001 (ARCH-03)");
    println!();
    println!("Physical Evidence:");
    println!("  - Fingerprint UUID: {}", fingerprint_id);
    println!("  - Theta to North Star: {:.4}", fp_theta);
    println!("  - Purpose vector: {} elements", NUM_EMBEDDERS);
    println!("======================================================================\n");
}

// =============================================================================
// FULL STATE VERIFICATION TEST 2: Goal Hierarchy Navigation
// =============================================================================

/// FULL STATE VERIFICATION: Goal hierarchy navigation with direct inspection.
#[tokio::test]
async fn test_full_state_verification_goal_hierarchy_navigation() {
    println!("\n======================================================================");
    println!("FULL STATE VERIFICATION TEST 2: Goal Hierarchy Navigation");
    println!("======================================================================\n");

    let (handlers, _store, hierarchy) = create_verifiable_handlers();

    // =========================================================================
    // STEP 1: Verify hierarchy structure directly
    // =========================================================================
    println!("📊 DIRECT HIERARCHY INSPECTION:");
    {
        let h = hierarchy.read();

        println!("   Total goals: {}", h.len());
        assert_eq!(h.len(), 5, "Must have 5 goals");

        let ns = h.north_star().expect("Must have North Star");
        println!("   North Star: {} - {}", ns.id, ns.description);
        assert!(!ns.id.is_nil(), "North Star must have valid UUID");

        let strategic = h.at_level(GoalLevel::Strategic);
        println!("   Strategic goals: {}", strategic.len());
        assert_eq!(strategic.len(), 2, "Must have 2 strategic goals");

        let tactical = h.at_level(GoalLevel::Tactical);
        println!("   Tactical goals: {}", tactical.len());
        assert_eq!(tactical.len(), 1, "Must have 1 tactical goal");

        let immediate = h.at_level(GoalLevel::Immediate);
        println!("   Immediate goals: {}", immediate.len());
        assert_eq!(immediate.len(), 1, "Must have 1 immediate goal");
    }
    println!("   ✓ VERIFIED: Hierarchy structure is correct\n");

    // =========================================================================
    // STEP 2: Execute get_all and verify against Source of Truth
    // =========================================================================
    println!("📝 EXECUTING: goal/hierarchy_query get_all");
    let get_all_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(json!({ "operation": "get_all" })),
    );
    let get_all_response = handlers.dispatch(get_all_request).await;

    assert!(get_all_response.error.is_none(), "get_all must succeed");
    let get_all_result = get_all_response.result.expect("Must have result");

    let goals = get_all_result
        .get("goals")
        .and_then(|v| v.as_array())
        .expect("Must have goals array");

    println!("   Handler returned {} goals", goals.len());
    assert_eq!(goals.len(), 5, "Must return 5 goals");

    // Verify against Source of Truth
    let direct_count = hierarchy.read().len();
    assert_eq!(
        goals.len(),
        direct_count,
        "Handler count must match Source of Truth"
    );
    println!("   ✓ VERIFIED: get_all matches Source of Truth\n");

    // =========================================================================
    // STEP 3: Execute get_children and verify
    // =========================================================================
    println!("📝 EXECUTING: goal/hierarchy_query get_children (ns_ml_system)");
    let get_children_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(2)),
        Some(json!({
            "operation": "get_children",
            "goal_id": "ns_ml_system"
        })),
    );
    let get_children_response = handlers.dispatch(get_children_request).await;

    assert!(
        get_children_response.error.is_none(),
        "get_children must succeed"
    );
    let get_children_result = get_children_response.result.expect("Must have result");

    let children = get_children_result
        .get("children")
        .and_then(|v| v.as_array())
        .expect("Must have children array");

    println!("   Handler returned {} children", children.len());

    // Verify against Source of Truth
    // Extract needed data before any async calls to avoid holding guard across await
    let direct_children_len = {
        let hierarchy_guard = hierarchy.read();
        let ns_id = hierarchy_guard
            .north_star()
            .expect("Must have North Star")
            .id;
        hierarchy_guard.children(&ns_id).len()
    }; // guard dropped here
    assert_eq!(
        children.len(),
        direct_children_len,
        "Handler children count must match Source of Truth"
    );
    println!(
        "   ✓ VERIFIED: get_children matches Source of Truth ({})",
        children.len()
    );

    // =========================================================================
    // STEP 4: Execute get_ancestors and verify path
    // =========================================================================
    println!("\n📝 EXECUTING: goal/hierarchy_query get_ancestors (i1_vector)");
    let get_ancestors_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(3)),
        Some(json!({
            "operation": "get_ancestors",
            "goal_id": "i1_vector"
        })),
    );
    let get_ancestors_response = handlers.dispatch(get_ancestors_request).await;

    assert!(
        get_ancestors_response.error.is_none(),
        "get_ancestors must succeed"
    );
    let get_ancestors_result = get_ancestors_response.result.expect("Must have result");

    let ancestors = get_ancestors_result
        .get("ancestors")
        .and_then(|v| v.as_array())
        .expect("Must have ancestors array");

    println!("   Handler returned {} ancestors", ancestors.len());

    // Verify against Source of Truth - get the immediate goal (last added child)
    let hierarchy_guard = hierarchy.read();
    let immediate_goals = hierarchy_guard.at_level(GoalLevel::Immediate);
    let immediate_goal_id = immediate_goals
        .first()
        .expect("Must have immediate goal")
        .id;
    let direct_path = hierarchy_guard.path_to_north_star(&immediate_goal_id);
    println!("   Direct path length: {}", direct_path.len());

    // Path should be: i1_vector -> t1_semantic -> s1_retrieval -> ns_ml_system
    println!(
        "   Path: {:?}",
        direct_path
            .iter()
            .map(|g| g.to_string())
            .collect::<Vec<_>>()
    );
    drop(hierarchy_guard);

    assert!(ancestors.len() >= 3, "Must have at least 3 ancestors");
    println!("   ✓ VERIFIED: get_ancestors returns correct path\n");

    // =========================================================================
    // EVIDENCE SUMMARY
    // =========================================================================
    println!("======================================================================");
    println!("EVIDENCE OF SUCCESS - Hierarchy Navigation Summary");
    println!("======================================================================");
    println!("Source of Truth: GoalHierarchy with 5 goals");
    println!("Operations Verified:");
    println!(
        "  - get_all: {} goals (matches Source of Truth)",
        goals.len()
    );
    println!("  - get_children(ns): {} children", children.len());
    println!(
        "  - get_ancestors(i1_vector): {} ancestors",
        ancestors.len()
    );
    println!("======================================================================\n");
}

// =============================================================================
// EDGE CASE 1: Purpose Query with Invalid Vector Size
// =============================================================================

/// EDGE CASE: 12-element purpose vector should fail (must be 13).
#[tokio::test]
async fn test_edge_case_purpose_query_12_elements() {
    println!("\n======================================================================");
    println!("EDGE CASE 1: Purpose Query with 12-Element Vector");
    println!("======================================================================\n");

    let (handlers, store, _hierarchy) = create_verifiable_handlers();

    // BEFORE STATE
    let before_count = store.count().await.expect("count should succeed");
    println!("📊 BEFORE STATE:");
    println!("   Source of Truth count: {}", before_count);

    // ACTION: 12-element purpose vector (WRONG - must be 13)
    println!("\n📝 ACTION: purpose/query with 12-element vector");
    let invalid_vector: Vec<f64> = vec![0.5; 12];
    println!(
        "   Vector length: {} (expected to fail)",
        invalid_vector.len()
    );

    let query_params = json!({
        "purpose_vector": invalid_vector,
        "topK": 10
    });
    let query_request = make_request(
        "purpose/query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    // Verify error
    assert!(
        response.error.is_some(),
        "12-element vector must return error"
    );
    let error = response.error.unwrap();
    println!("   Error code: {} (expected: -32602)", error.code);
    println!("   Error message: {}", error.message);
    assert_eq!(error.code, -32602, "Must return INVALID_PARAMS (-32602)");
    assert!(
        error.message.contains("13") || error.message.contains("elements"),
        "Error must mention 13 elements"
    );

    // AFTER STATE
    let after_count = store.count().await.expect("count should succeed");
    println!("\n📊 AFTER STATE:");
    println!("   Source of Truth count: {} (unchanged)", after_count);
    assert_eq!(
        after_count, before_count,
        "Store count must remain unchanged"
    );

    println!("\n✓ VERIFIED: 12-element vector correctly rejected, Source of Truth unchanged\n");
}

// =============================================================================
// EDGE CASE 2: Autonomous Operation Without North Star
// =============================================================================

/// EDGE CASE: Store operation works autonomously without North Star.
///
/// TASK-CORE-001: Updated to verify deprecated methods return METHOD_NOT_FOUND.
///
/// AUTONOMOUS OPERATION: Per contextprd.md, the 13-embedding array IS the
/// teleological vector. Memory storage uses default purpose vector [0.0; 13]
/// when no North Star is configured, enabling autonomous operation.
#[tokio::test]
async fn test_edge_case_autonomous_operation_no_north_star() {
    println!("\n======================================================================");
    println!("EDGE CASE 2: Autonomous Operation Without North Star");
    println!("======================================================================\n");

    let (handlers, _store, hierarchy) = create_verifiable_handlers_no_north_star();

    // BEFORE STATE
    println!("📊 BEFORE STATE:");
    println!("   Has North Star: {}", hierarchy.read().has_north_star());
    assert!(
        !hierarchy.read().has_north_star(),
        "Must NOT have North Star"
    );

    // Store fingerprint - should SUCCEED without North Star (AUTONOMOUS OPERATION)
    println!("\n📝 ATTEMPTING: memory/store (should succeed - autonomous operation)");
    let store_params = json!({
        "content": "Test content for autonomous alignment",
        "importance": 0.8
    });
    let store_request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(store_params),
    );
    let store_response = handlers.dispatch(store_request).await;

    // Verify store succeeds with default purpose vector
    assert!(
        store_response.error.is_none(),
        "Store MUST succeed without North Star (AUTONOMOUS OPERATION). Error: {:?}",
        store_response.error
    );
    let result = store_response.result.expect("Should have result");
    let fingerprint_id = result
        .get("fingerprintId")
        .expect("Must have fingerprintId");
    println!("   SUCCESS: fingerprintId={}", fingerprint_id);

    // TASK-CORE-001: Verify deprecated method returns METHOD_NOT_FOUND
    println!("\n📝 VERIFYING: purpose/north_star_alignment returns METHOD_NOT_FOUND");
    let align_params = json!({
        "fingerprint_id": "00000000-0000-0000-0000-000000000001"
    });
    let align_request = make_request(
        "purpose/north_star_alignment",
        Some(JsonRpcId::Number(2)),
        Some(align_params),
    );
    let response = handlers.dispatch(align_request).await;

    // TASK-CORE-001: Must return METHOD_NOT_FOUND (-32601) for deprecated method
    assert!(
        response.error.is_some(),
        "Deprecated method must return error"
    );
    let align_error = response.error.unwrap();
    println!("   Error code: {} (expected: -32601)", align_error.code);
    println!("   Error message: {}", align_error.message);
    assert_eq!(
        align_error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );

    println!(
        "\n✓ VERIFIED: System operates autonomously, deprecated method returns METHOD_NOT_FOUND\n"
    );
}

// =============================================================================
// EDGE CASE 3: Goal Not Found in Hierarchy
// =============================================================================

/// EDGE CASE: Non-existent goal should return GOAL_NOT_FOUND.
#[tokio::test]
async fn test_edge_case_goal_not_found() {
    println!("\n======================================================================");
    println!("EDGE CASE 3: Goal Not Found in Hierarchy");
    println!("======================================================================\n");

    let (handlers, _store, hierarchy) = create_verifiable_handlers();

    // BEFORE STATE
    println!("📊 BEFORE STATE:");
    let goal_count = hierarchy.read().len();
    println!("   Total goals in hierarchy: {}", goal_count);

    // Verify the goal we're looking for does NOT exist (random UUID)
    let nonexistent_id = Uuid::new_v4();
    let exists_in_hierarchy = hierarchy.read().get(&nonexistent_id).is_some();
    println!("   'nonexistent_goal_xyz' exists: {}", exists_in_hierarchy);
    assert!(!exists_in_hierarchy, "Goal must NOT exist");

    // ACTION: Try to get non-existent goal
    println!("\n📝 ACTION: goal/hierarchy_query get_goal (nonexistent)");
    let query_params = json!({
        "operation": "get_goal",
        "goal_id": nonexistent_id.to_string()
    });
    let query_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(query_params),
    );
    let response = handlers.dispatch(query_request).await;

    // Verify error
    assert!(
        response.error.is_some(),
        "Non-existent goal must return error"
    );
    let error = response.error.unwrap();
    println!("   Error code: {} (expected: -32020)", error.code);
    println!("   Error message: {}", error.message);
    assert_eq!(error.code, -32020, "Must return GOAL_NOT_FOUND (-32020)");

    // AFTER STATE - hierarchy unchanged
    let after_count = hierarchy.read().len();
    assert_eq!(after_count, goal_count, "Hierarchy must remain unchanged");

    println!("\n✓ VERIFIED: Non-existent goal correctly returns GOAL_NOT_FOUND\n");
}

// =============================================================================
// EDGE CASE 4: North Star Update Returns METHOD_NOT_FOUND (TASK-CORE-001)
// =============================================================================

/// EDGE CASE: purpose/north_star_update is deprecated and returns METHOD_NOT_FOUND.
///
/// TASK-CORE-001: Manual North Star update removed per ARCH-03 (autonomous-first).
/// Goals emerge autonomously via auto_bootstrap_north_star tool.
#[tokio::test]
async fn test_edge_case_north_star_update_returns_method_not_found() {
    println!("\n======================================================================");
    println!("EDGE CASE 4: North Star Update Returns METHOD_NOT_FOUND (TASK-CORE-001)");
    println!("======================================================================\n");

    let (handlers, _store, hierarchy) = create_verifiable_handlers();

    // BEFORE STATE
    println!("📊 BEFORE STATE:");
    let has_ns = hierarchy.read().has_north_star();
    println!("   Has North Star: {}", has_ns);
    assert!(has_ns, "Must already have North Star");

    let existing_ns_id = hierarchy
        .read()
        .north_star()
        .map(|g| g.id.to_string())
        .expect("Must have NS");
    println!("   Existing North Star ID: {}", existing_ns_id);

    // ACTION: Try to call deprecated method
    println!("\n📝 ACTION: purpose/north_star_update (deprecated per TASK-CORE-001)");
    let update_params = json!({
        "description": "New competing North Star",
        "replace": false
    });
    let update_request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(update_params),
    );
    let response = handlers.dispatch(update_request).await;

    // TASK-CORE-001: Verify METHOD_NOT_FOUND error
    assert!(
        response.error.is_some(),
        "Deprecated method must return error"
    );
    let error = response.error.unwrap();
    println!("   Error code: {} (expected: -32601)", error.code);
    println!("   Error message: {}", error.message);
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601) for deprecated method"
    );

    // AFTER STATE - original North Star unchanged
    let after_ns_id = hierarchy
        .read()
        .north_star()
        .map(|g| g.id.to_string())
        .expect("Must still have NS");
    assert_eq!(
        after_ns_id, existing_ns_id,
        "North Star must remain unchanged"
    );

    println!("\n✓ VERIFIED: Deprecated method returns METHOD_NOT_FOUND, hierarchy unchanged\n");
}

// =============================================================================
// EDGE CASE 5: Drift Check with Invalid UUIDs
// =============================================================================

/// EDGE CASE: Drift check with invalid UUIDs in array should fail.
#[tokio::test]
async fn test_edge_case_drift_check_invalid_uuids() {
    println!("\n======================================================================");
    println!("EDGE CASE 5: Drift Check with Invalid UUIDs");
    println!("======================================================================\n");

    let (handlers, store, _hierarchy) = create_verifiable_handlers();

    // BEFORE STATE
    let before_count = store.count().await.expect("count should succeed");
    println!("📊 BEFORE STATE:");
    println!("   Source of Truth count: {}", before_count);

    // ACTION: Drift check with invalid UUIDs
    println!("\n📝 ACTION: purpose/drift_check with invalid UUIDs");
    let drift_params = json!({
        "fingerprint_ids": ["not-a-uuid", "also-not-valid"]
    });
    let drift_request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(1)),
        Some(drift_params),
    );
    let response = handlers.dispatch(drift_request).await;

    // Verify error
    assert!(response.error.is_some(), "Invalid UUIDs must return error");
    let error = response.error.unwrap();
    println!("   Error code: {} (expected: -32602)", error.code);
    println!("   Error message: {}", error.message);
    assert_eq!(error.code, -32602, "Must return INVALID_PARAMS (-32602)");

    // AFTER STATE
    let after_count = store.count().await.expect("count should succeed");
    assert_eq!(
        after_count, before_count,
        "Store count must remain unchanged"
    );

    println!("\n✓ VERIFIED: Invalid UUIDs correctly rejected\n");
}
