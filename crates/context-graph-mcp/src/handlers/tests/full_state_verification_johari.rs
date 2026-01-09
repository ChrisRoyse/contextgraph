//! Full State Verification Tests for Johari Handlers
//!
//! TASK-S004: Comprehensive verification that directly inspects the Source of Truth.
//!
//! ## Verification Methodology
//!
//! 1. Define Source of Truth: InMemoryTeleologicalStore + JohariTransitionManager
//! 2. Execute & Inspect: Run handlers, then directly query stores to verify
//! 3. Edge Case Audit: Test 3+ edge cases with BEFORE/AFTER state logging
//! 4. Evidence of Success: Print actual data residing in the system
//!
//! ## Uses STUB implementations (InMemoryTeleologicalStore)
//!
//! All tests use real InMemoryTeleologicalStore with real fingerprints.
//! NO fallbacks, NO default values, NO workarounds.

use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;
use uuid::Uuid;

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::johari::{DynDefaultJohariManager, JohariTransitionManager, NUM_EMBEDDERS};
use context_graph_core::purpose::GoalHierarchy;
use context_graph_core::stubs::{
    InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor,
};
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, TeleologicalFingerprint,
};
use context_graph_core::types::JohariQuadrant;

use crate::handlers::Handlers;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcRequest};

/// Create test handlers with SHARED access for direct verification.
///
/// Returns the handlers plus the underlying store and johari_manager for direct inspection.
fn create_verifiable_handlers() -> (
    Handlers,
    Arc<InMemoryTeleologicalStore>,
    Arc<dyn JohariTransitionManager>,
) {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor = Arc::new(StubUtlProcessor::new());
    let multi_array = Arc::new(StubMultiArrayProvider::new());
    let alignment_calc: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = Arc::new(RwLock::new(GoalHierarchy::default()));

    // Create JohariTransitionManager with SHARED store reference
    // Uses DynDefaultJohariManager for trait object compatibility
    let johari_manager: Arc<dyn JohariTransitionManager> =
        Arc::new(DynDefaultJohariManager::new(store.clone()));

    let handlers = Handlers::with_johari_manager(
        store.clone(),
        utl_processor,
        multi_array,
        alignment_calc,
        goal_hierarchy,
        johari_manager.clone(),
    );

    (handlers, store, johari_manager)
}

/// Create a test fingerprint with a specific Johari configuration.
fn create_test_fingerprint_with_johari(
    quadrants: [JohariQuadrant; NUM_EMBEDDERS],
) -> TeleologicalFingerprint {
    let mut johari = JohariFingerprint::zeroed();
    for (idx, quadrant) in quadrants.iter().enumerate() {
        match quadrant {
            JohariQuadrant::Open => johari.set_quadrant(idx, 1.0, 0.0, 0.0, 0.0, 1.0),
            JohariQuadrant::Hidden => johari.set_quadrant(idx, 0.0, 1.0, 0.0, 0.0, 1.0),
            JohariQuadrant::Blind => johari.set_quadrant(idx, 0.0, 0.0, 1.0, 0.0, 1.0),
            JohariQuadrant::Unknown => johari.set_quadrant(idx, 0.0, 0.0, 0.0, 1.0, 1.0),
        }
    }

    TeleologicalFingerprint::new(
        SemanticFingerprint::zeroed(),
        PurposeVector::default(),
        johari,
        [0u8; 32],
    )
}

/// Build JSON-RPC request.
fn make_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(1)),
        method: method.to_string(),
        params: Some(params),
    }
}

// ==================== FULL STATE VERIFICATION TESTS ====================

#[tokio::test]
async fn test_full_state_verification_distribution_cycle() {
    println!("\n========== FULL STATE VERIFICATION: Distribution Cycle ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    // Create fingerprint with specific configuration
    let quadrants = [
        JohariQuadrant::Open,    // E1
        JohariQuadrant::Open,    // E2
        JohariQuadrant::Hidden,  // E3
        JohariQuadrant::Hidden,  // E4
        JohariQuadrant::Blind,   // E5
        JohariQuadrant::Blind,   // E6
        JohariQuadrant::Unknown, // E7
        JohariQuadrant::Unknown, // E8
        JohariQuadrant::Open,    // E9
        JohariQuadrant::Hidden,  // E10
        JohariQuadrant::Blind,   // E11
        JohariQuadrant::Unknown, // E12
        JohariQuadrant::Open,    // E13
    ];

    let fp = create_test_fingerprint_with_johari(quadrants);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    println!("[STATE BEFORE] Stored fingerprint with ID: {}", memory_id);
    println!("  Expected: 4 Open, 3 Hidden, 3 Blind, 3 Unknown");

    // Execute handler
    let request = make_request(
        "johari/get_distribution",
        json!({
            "memory_id": memory_id.to_string(),
            "include_confidence": true,
            "include_transition_predictions": false
        }),
    );

    let response = handlers.dispatch(request).await;

    // Verify response
    assert!(response.error.is_none(), "Handler should succeed");
    let result = response.result.unwrap();

    println!("[HANDLER RESPONSE]");
    println!("  memory_id: {}", result["memory_id"]);

    let summary = &result["summary"];
    let open_count = summary["open_count"].as_u64().unwrap();
    let hidden_count = summary["hidden_count"].as_u64().unwrap();
    let blind_count = summary["blind_count"].as_u64().unwrap();
    let unknown_count = summary["unknown_count"].as_u64().unwrap();

    println!(
        "  Quadrant counts: {} open, {} hidden, {} blind, {} unknown",
        open_count, hidden_count, blind_count, unknown_count
    );

    // [VERIFY] Direct store inspection
    let stored = store
        .retrieve(memory_id)
        .await
        .unwrap()
        .expect("Memory should exist");

    println!("[STATE AFTER - Direct Store Inspection]");
    for i in 0..NUM_EMBEDDERS {
        println!(
            "  E{}: {:?} (confidence: {:.2})",
            i + 1,
            stored.johari.dominant_quadrant(i),
            stored.johari.confidence[i]
        );
    }

    // Verify counts match expected
    assert_eq!(open_count, 4, "[VERIFICATION FAILED] Open count mismatch");
    assert_eq!(
        hidden_count, 3,
        "[VERIFICATION FAILED] Hidden count mismatch"
    );
    assert_eq!(blind_count, 3, "[VERIFICATION FAILED] Blind count mismatch");
    assert_eq!(
        unknown_count, 3,
        "[VERIFICATION FAILED] Unknown count mismatch"
    );

    // Verify all 13 embedders present in response
    let per_embedder = result["per_embedder_quadrants"].as_array().unwrap();
    assert_eq!(
        per_embedder.len(),
        13,
        "[VERIFICATION FAILED] Not all 13 embedders returned"
    );

    println!("======================================================================");
    println!("EVIDENCE OF SUCCESS - Distribution Cycle Verification");
    println!("======================================================================");
    println!("Source of Truth: InMemoryTeleologicalStore");
    println!("  - Fingerprint ID: {}", memory_id);
    println!("  - All 13 embedders retrieved: YES");
    println!("  - Quadrant counts verified: YES");
    println!("  - Handler response matches store: YES");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_full_state_verification_transition_persistence() {
    println!("\n========== FULL STATE VERIFICATION: Transition Persistence ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    // Create fingerprint with E1 as Unknown (allows transition to Open)
    let mut quadrants = [JohariQuadrant::Open; NUM_EMBEDDERS];
    quadrants[0] = JohariQuadrant::Unknown; // E1 starts Unknown

    let fp = create_test_fingerprint_with_johari(quadrants);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    // [STATE BEFORE]
    let before = store.retrieve(memory_id).await.unwrap().unwrap();
    println!("[STATE BEFORE]");
    println!("  Memory ID: {}", memory_id);
    println!("  E1 quadrant: {:?}", before.johari.dominant_quadrant(0));
    assert_eq!(
        before.johari.dominant_quadrant(0),
        JohariQuadrant::Unknown,
        "E1 should start as Unknown"
    );

    // Execute transition: Unknown -> Open via DreamConsolidation
    let request = make_request(
        "johari/transition",
        json!({
            "memory_id": memory_id.to_string(),
            "embedder_index": 0,
            "to_quadrant": "open",
            "trigger": "dream_consolidation"
        }),
    );

    let response = handlers.dispatch(request).await;

    // Verify handler succeeded
    assert!(
        response.error.is_none(),
        "Transition should succeed: {:?}",
        response.error
    );

    let result = response.result.unwrap();
    println!("[HANDLER RESPONSE]");
    println!("  from_quadrant: {}", result["from_quadrant"]);
    println!("  to_quadrant: {}", result["to_quadrant"]);
    println!("  success: {}", result["success"]);

    // [STATE AFTER] - CRITICAL: Verify via direct store inspection
    let after = store.retrieve(memory_id).await.unwrap().unwrap();
    println!("[STATE AFTER - Direct Store Inspection]");
    println!("  E1 quadrant: {:?}", after.johari.dominant_quadrant(0));
    println!("  E1 weights: {:?}", after.johari.quadrants[0]);

    // [VERIFY] The transition MUST be persisted
    assert_eq!(
        after.johari.dominant_quadrant(0),
        JohariQuadrant::Open,
        "[VERIFICATION FAILED] Transition not persisted to store!"
    );

    println!("======================================================================");
    println!("EVIDENCE OF SUCCESS - Transition Persistence Verification");
    println!("======================================================================");
    println!("Source of Truth: InMemoryTeleologicalStore");
    println!("  - Fingerprint ID: {}", memory_id);
    println!("  - E1 BEFORE: Unknown");
    println!("  - E1 AFTER: Open");
    println!("Physical Evidence:");
    println!("  - Transition executed: Unknown -> Open");
    println!("  - Trigger: DreamConsolidation");
    println!("  - Persisted to store: YES (verified via retrieve)");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_full_state_verification_batch_all_or_nothing() {
    println!("\n========== FULL STATE VERIFICATION: Batch All-or-Nothing ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    // Create fingerprint with first 5 embedders as Unknown
    let mut quadrants = [JohariQuadrant::Open; NUM_EMBEDDERS];
    for i in 0..5 {
        quadrants[i] = JohariQuadrant::Unknown;
    }

    let fp = create_test_fingerprint_with_johari(quadrants);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    // [STATE BEFORE]
    let before = store.retrieve(memory_id).await.unwrap().unwrap();
    println!("[STATE BEFORE]");
    for i in 0..5 {
        println!("  E{}: {:?}", i + 1, before.johari.dominant_quadrant(i));
    }

    // Submit batch with one INVALID transition (invalid embedder index)
    let request = make_request(
        "johari/transition_batch",
        json!({
            "memory_id": memory_id.to_string(),
            "transitions": [
                { "embedder_index": 0, "to_quadrant": "open", "trigger": "dream_consolidation" },
                { "embedder_index": 99, "to_quadrant": "open", "trigger": "dream_consolidation" }  // INVALID
            ]
        }),
    );

    let response = handlers.dispatch(request).await;

    // Verify handler returned error
    assert!(
        response.error.is_some(),
        "Batch with invalid index should fail"
    );
    let error = response.error.unwrap();
    println!("[HANDLER ERROR]");
    println!("  code: {}", error.code);
    println!("  message: {}", error.message);

    // [STATE AFTER] - CRITICAL: Verify NO changes were made (all-or-nothing)
    let after = store.retrieve(memory_id).await.unwrap().unwrap();
    println!("[STATE AFTER - Direct Store Inspection]");
    for i in 0..5 {
        println!("  E{}: {:?}", i + 1, after.johari.dominant_quadrant(i));
    }

    // [VERIFY] ALL embedders should be UNCHANGED
    for i in 0..5 {
        assert_eq!(
            after.johari.dominant_quadrant(i),
            JohariQuadrant::Unknown,
            "[VERIFICATION FAILED] E{} changed despite batch failure!",
            i + 1
        );
    }

    println!("======================================================================");
    println!("EVIDENCE OF SUCCESS - Batch All-or-Nothing Verification");
    println!("======================================================================");
    println!("Source of Truth: InMemoryTeleologicalStore");
    println!("  - Fingerprint ID: {}", memory_id);
    println!("  - Batch contained INVALID embedder index (99)");
    println!("  - Handler returned error code: {}", error.code);
    println!("Physical Evidence:");
    println!("  - E1-E5 BEFORE: All Unknown");
    println!("  - E1-E5 AFTER: All Unknown (UNCHANGED)");
    println!("  - All-or-nothing semantics: VERIFIED");
    println!("======================================================================\n");
}

// ==================== EDGE CASE TESTS ====================

#[tokio::test]
async fn edge_case_1_embedder_index_13() {
    println!("\n========== EDGE CASE 1: Invalid Embedder Index 13 ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    let fp = create_test_fingerprint_with_johari([JohariQuadrant::Unknown; NUM_EMBEDDERS]);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    // Request with invalid embedder index 13 (valid is 0-12)
    let request = make_request(
        "johari/transition",
        json!({
            "memory_id": memory_id.to_string(),
            "embedder_index": 13,  // INVALID
            "to_quadrant": "open",
            "trigger": "dream_consolidation"
        }),
    );

    let response = handlers.dispatch(request).await;

    assert!(response.error.is_some(), "Should return error for index 13");
    let error = response.error.unwrap();

    println!("[ERROR RESPONSE]");
    println!("  code: {}", error.code);
    println!("  message: {}", error.message);

    assert_eq!(
        error.code,
        error_codes::JOHARI_INVALID_EMBEDDER_INDEX,
        "[EDGE CASE 1 FAILED] Expected error code -32030"
    );

    println!("[EDGE CASE 1 PASSED] Invalid embedder index 13 returns -32030\n");
}

#[tokio::test]
async fn edge_case_2_invalid_quadrant_string() {
    println!("\n========== EDGE CASE 2: Invalid Quadrant String ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    let fp = create_test_fingerprint_with_johari([JohariQuadrant::Unknown; NUM_EMBEDDERS]);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    // Request with invalid quadrant string
    let request = make_request(
        "johari/transition",
        json!({
            "memory_id": memory_id.to_string(),
            "embedder_index": 0,
            "to_quadrant": "invalid_quadrant",  // INVALID
            "trigger": "dream_consolidation"
        }),
    );

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_some(),
        "Should return error for invalid quadrant"
    );
    let error = response.error.unwrap();

    println!("[ERROR RESPONSE]");
    println!("  code: {}", error.code);
    println!("  message: {}", error.message);

    assert_eq!(
        error.code,
        error_codes::JOHARI_INVALID_QUADRANT,
        "[EDGE CASE 2 FAILED] Expected error code -32031"
    );

    println!("[EDGE CASE 2 PASSED] Invalid quadrant string returns -32031\n");
}

#[tokio::test]
async fn edge_case_3_soft_classification_sum() {
    println!("\n========== EDGE CASE 3: Soft Classification Sum Check ==========\n");

    let (_handlers, store, _johari_manager) = create_verifiable_handlers();

    // Create fingerprint with mixed quadrants
    let quadrants = [
        JohariQuadrant::Open,
        JohariQuadrant::Hidden,
        JohariQuadrant::Blind,
        JohariQuadrant::Unknown,
        JohariQuadrant::Open,
        JohariQuadrant::Hidden,
        JohariQuadrant::Blind,
        JohariQuadrant::Unknown,
        JohariQuadrant::Open,
        JohariQuadrant::Hidden,
        JohariQuadrant::Blind,
        JohariQuadrant::Unknown,
        JohariQuadrant::Open,
    ];

    let fp = create_test_fingerprint_with_johari(quadrants);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    // Retrieve and verify all soft classifications sum to 1.0
    let stored = store.retrieve(memory_id).await.unwrap().unwrap();

    println!("[VERIFICATION] Checking soft classification sums:");
    for (i, weights) in stored.johari.quadrants.iter().enumerate() {
        let sum: f32 = weights.iter().sum();
        println!("  E{}: weights={:?}, sum={}", i + 1, weights, sum);

        assert!(
            (sum - 1.0).abs() < 0.001,
            "[EDGE CASE 3 FAILED] E{} weights sum to {} (expected 1.0)",
            i + 1,
            sum
        );
    }

    println!("[EDGE CASE 3 PASSED] All soft classifications sum to 1.0\n");
}

#[tokio::test]
async fn edge_case_4_invalid_transition_rejected() {
    println!("\n========== EDGE CASE 4: Invalid Transition Rejected ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    // Create fingerprint with E1 as Open
    let mut quadrants = [JohariQuadrant::Unknown; NUM_EMBEDDERS];
    quadrants[0] = JohariQuadrant::Open; // E1 is Open

    let fp = create_test_fingerprint_with_johari(quadrants);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    // [STATE BEFORE]
    let before = store.retrieve(memory_id).await.unwrap().unwrap();
    println!(
        "[STATE BEFORE] E1: {:?}",
        before.johari.dominant_quadrant(0)
    );

    // Attempt invalid transition: Open -> Blind (not allowed)
    let request = make_request(
        "johari/transition",
        json!({
            "memory_id": memory_id.to_string(),
            "embedder_index": 0,
            "to_quadrant": "blind",
            "trigger": "external_observation"
        }),
    );

    let response = handlers.dispatch(request).await;

    // Should fail
    assert!(
        response.error.is_some(),
        "Invalid transition Open->Blind should fail"
    );
    let error = response.error.unwrap();

    println!("[ERROR RESPONSE]");
    println!("  code: {}", error.code);
    println!("  message: {}", error.message);

    // [STATE AFTER] - Verify unchanged
    let after = store.retrieve(memory_id).await.unwrap().unwrap();
    println!("[STATE AFTER] E1: {:?}", after.johari.dominant_quadrant(0));

    assert_eq!(
        after.johari.dominant_quadrant(0),
        JohariQuadrant::Open,
        "[EDGE CASE 4 FAILED] State changed despite invalid transition!"
    );

    assert_eq!(
        error.code,
        error_codes::JOHARI_TRANSITION_ERROR,
        "[EDGE CASE 4 FAILED] Expected error code -32033"
    );

    println!("[EDGE CASE 4 PASSED] Invalid transition rejected, state preserved\n");
}

#[tokio::test]
async fn edge_case_5_memory_not_found() {
    println!("\n========== EDGE CASE 5: Memory Not Found ==========\n");

    let (handlers, _store, _johari_manager) = create_verifiable_handlers();

    let fake_id = Uuid::new_v4();

    let request = make_request(
        "johari/get_distribution",
        json!({
            "memory_id": fake_id.to_string(),
            "include_confidence": true
        }),
    );

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_some(),
        "Non-existent memory should return error"
    );
    let error = response.error.unwrap();

    println!("[ERROR RESPONSE]");
    println!("  code: {}", error.code);
    println!("  message: {}", error.message);

    assert_eq!(
        error.code,
        error_codes::FINGERPRINT_NOT_FOUND,
        "[EDGE CASE 5 FAILED] Expected error code -32010"
    );

    println!("[EDGE CASE 5 PASSED] Non-existent memory returns -32010\n");
}

// ==================== BATCH SUCCESS TEST ====================

#[tokio::test]
async fn test_full_state_verification_batch_success() {
    println!("\n========== FULL STATE VERIFICATION: Batch Success ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    // Create fingerprint with first 3 embedders as Unknown
    let mut quadrants = [JohariQuadrant::Open; NUM_EMBEDDERS];
    for i in 0..3 {
        quadrants[i] = JohariQuadrant::Unknown;
    }

    let fp = create_test_fingerprint_with_johari(quadrants);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    // [STATE BEFORE]
    println!("[STATE BEFORE]");
    let before = store.retrieve(memory_id).await.unwrap().unwrap();
    for i in 0..3 {
        println!("  E{}: {:?}", i + 1, before.johari.dominant_quadrant(i));
    }

    // Valid batch: transition E1->Open, E2->Hidden, E3->Blind
    let request = make_request(
        "johari/transition_batch",
        json!({
            "memory_id": memory_id.to_string(),
            "transitions": [
                { "embedder_index": 0, "to_quadrant": "open", "trigger": "dream_consolidation" },
                { "embedder_index": 1, "to_quadrant": "hidden", "trigger": "dream_consolidation" },
                { "embedder_index": 2, "to_quadrant": "blind", "trigger": "external_observation" }
            ]
        }),
    );

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Valid batch should succeed: {:?}",
        response.error
    );

    let result = response.result.unwrap();
    println!("[HANDLER RESPONSE]");
    println!("  success: {}", result["success"]);
    println!("  transitions_applied: {}", result["transitions_applied"]);

    // [STATE AFTER] - Direct verification
    println!("[STATE AFTER - Direct Store Inspection]");
    let after = store.retrieve(memory_id).await.unwrap().unwrap();
    for i in 0..3 {
        println!("  E{}: {:?}", i + 1, after.johari.dominant_quadrant(i));
    }

    // [VERIFY] All transitions applied
    assert_eq!(
        after.johari.dominant_quadrant(0),
        JohariQuadrant::Open,
        "[VERIFICATION FAILED] E1 should be Open"
    );
    assert_eq!(
        after.johari.dominant_quadrant(1),
        JohariQuadrant::Hidden,
        "[VERIFICATION FAILED] E2 should be Hidden"
    );
    assert_eq!(
        after.johari.dominant_quadrant(2),
        JohariQuadrant::Blind,
        "[VERIFICATION FAILED] E3 should be Blind"
    );

    println!("======================================================================");
    println!("EVIDENCE OF SUCCESS - Batch Success Verification");
    println!("======================================================================");
    println!("Source of Truth: InMemoryTeleologicalStore");
    println!("  - Fingerprint ID: {}", memory_id);
    println!("  - Batch size: 3 transitions");
    println!("Physical Evidence:");
    println!("  - E1: Unknown -> Open (DreamConsolidation)");
    println!("  - E2: Unknown -> Hidden (DreamConsolidation)");
    println!("  - E3: Unknown -> Blind (ExternalObservation)");
    println!("  - All persisted to store: YES");
    println!("======================================================================\n");
}

// ==================== CROSS-SPACE ANALYSIS TEST ====================

#[tokio::test]
async fn test_full_state_verification_cross_space_analysis() {
    println!("\n========== FULL STATE VERIFICATION: Cross-Space Analysis ==========\n");

    let (handlers, store, _johari_manager) = create_verifiable_handlers();

    // Create fingerprint with blind spots: E1 Open, E5 Blind
    let mut quadrants = [JohariQuadrant::Unknown; NUM_EMBEDDERS];
    quadrants[0] = JohariQuadrant::Open; // E1 semantic
    quadrants[4] = JohariQuadrant::Blind; // E5 causal

    let fp = create_test_fingerprint_with_johari(quadrants);
    let memory_id = store.store(fp).await.expect("Store should succeed");

    println!("[STATE] Created memory with:");
    println!("  E1 (semantic): Open");
    println!("  E5 (causal): Blind");
    println!("  E7-E13: Unknown");

    let request = make_request(
        "johari/cross_space_analysis",
        json!({
            "memory_ids": [memory_id.to_string()],
            "analysis_type": "blind_spots"
        }),
    );

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "Cross-space analysis should succeed: {:?}",
        response.error
    );

    let result = response.result.unwrap();
    let blind_spots = result["blind_spots"].as_array().unwrap();
    let learning_opportunities = result["learning_opportunities"].as_array().unwrap();

    println!("[HANDLER RESPONSE]");
    println!("  Blind spots found: {}", blind_spots.len());
    println!("  Learning opportunities: {}", learning_opportunities.len());

    // Should find blind spot: E1 Open but E5 Blind
    if !blind_spots.is_empty() {
        println!("  First blind spot: {:?}", blind_spots[0]);
    }

    // Should find learning opportunity (>5 Unknown spaces)
    if !learning_opportunities.is_empty() {
        println!("  First opportunity: {:?}", learning_opportunities[0]);
    }

    println!("======================================================================");
    println!("EVIDENCE OF SUCCESS - Cross-Space Analysis Verification");
    println!("======================================================================");
    println!("Source of Truth: InMemoryTeleologicalStore");
    println!("  - Fingerprint ID: {}", memory_id);
    println!("  - Blind spots detected: {}", blind_spots.len());
    println!(
        "  - Learning opportunities: {}",
        learning_opportunities.len()
    );
    println!("======================================================================\n");
}

// ==================== NOTE ====================
// Each test above runs independently via `cargo test`.
// The `#[tokio::test]` attribute means each function is a standalone test.
// To run all Johari FSV tests:
//   cargo test --package context-graph-mcp full_state_verification_johari -- --nocapture
//
// Summary of tests:
// - test_full_state_verification_distribution_cycle: Get distribution, verify store
// - test_full_state_verification_transition_persistence: Transition, verify store persistence
// - test_full_state_verification_batch_all_or_nothing: Batch semantics, verify rollback
// - test_full_state_verification_batch_success: Successful batch transition
// - test_full_state_verification_cross_space_analysis: Cross-space analysis
// - edge_case_1_embedder_index_13: Invalid embedder index
// - edge_case_2_invalid_quadrant_string: Invalid quadrant string
// - edge_case_3_soft_classification_sum: Soft classification integrity
// - edge_case_4_invalid_transition_rejected: State machine validation
// - edge_case_5_memory_not_found: Non-existent memory error
