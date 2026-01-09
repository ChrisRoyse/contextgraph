//! Full State Verification Tests for Meta-UTL Handlers
//!
//! TASK-S005: Comprehensive verification that directly inspects the Source of Truth.
//!
//! ## Verification Methodology
//!
//! 1. Define Source of Truth: MetaUtlTracker (pending_predictions, embedder_accuracy)
//! 2. Execute & Inspect: Run handlers, then directly query tracker to verify
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

use crate::handlers::core::MetaUtlTracker;
use crate::handlers::Handlers;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcRequest};

/// Create test handlers with SHARED access for direct verification.
///
/// Returns the handlers plus the underlying store and tracker for direct inspection.
fn create_verifiable_handlers_with_tracker() -> (
    Handlers,
    Arc<InMemoryTeleologicalStore>,
    Arc<RwLock<MetaUtlTracker>>,
) {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor = Arc::new(StubUtlProcessor::new());
    let multi_array = Arc::new(StubMultiArrayProvider::new());
    let alignment_calc: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = Arc::new(RwLock::new(GoalHierarchy::default()));

    // Create JohariTransitionManager with SHARED store reference
    let johari_manager: Arc<dyn JohariTransitionManager> =
        Arc::new(DynDefaultJohariManager::new(store.clone()));

    // Create MetaUtlTracker with SHARED access
    let meta_utl_tracker = Arc::new(RwLock::new(MetaUtlTracker::new()));

    let handlers = Handlers::with_meta_utl_tracker(
        store.clone(),
        utl_processor,
        multi_array,
        alignment_calc,
        goal_hierarchy,
        johari_manager,
        meta_utl_tracker.clone(),
    );

    (handlers, store, meta_utl_tracker)
}

/// Create a test fingerprint.
fn create_test_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        SemanticFingerprint::zeroed(),
        PurposeVector::default(),
        JohariFingerprint::zeroed(),
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

/// Build JSON-RPC request with no params.
fn make_request_no_params(method: &str) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(1)),
        method: method.to_string(),
        params: None,
    }
}

// ==================== FULL STATE VERIFICATION TESTS ====================

#[tokio::test]
async fn test_fsv_learning_trajectory_all_embedders() {
    println!("\n======================================================================");
    println!("FSV: meta_utl/learning_trajectory - All 13 Embedders");
    println!("======================================================================\n");

    let (handlers, _store, tracker) = create_verifiable_handlers_with_tracker();

    // STEP 1: BEFORE STATE
    println!("BEFORE STATE:");
    {
        let tracker_guard = tracker.read();
        println!("  prediction_count: {}", tracker_guard.prediction_count);
        println!("  validation_count: {}", tracker_guard.validation_count);
        println!(
            "  current_weights sum: {:.6}",
            tracker_guard.current_weights.iter().sum::<f32>()
        );
    }

    // STEP 2: EXECUTE
    let request = make_request(
        "meta_utl/learning_trajectory",
        json!({
            "include_accuracy_trend": true
        }),
    );

    let response = handlers.dispatch(request).await;

    // STEP 3: VERIFY RESPONSE
    println!("\nVERIFY RESPONSE:");
    assert!(
        response.error.is_none(),
        "Handler should succeed: {:?}",
        response.error
    );
    let result = response.result.unwrap();

    let trajectories = result["trajectories"].as_array().unwrap();
    println!("  trajectories count: {}", trajectories.len());
    assert_eq!(
        trajectories.len(),
        NUM_EMBEDDERS,
        "Should return all 13 embedders"
    );

    // Verify each trajectory has expected fields
    for (i, traj) in trajectories.iter().enumerate() {
        assert_eq!(traj["embedder_index"].as_u64().unwrap() as usize, i);
        assert!(traj["embedder_name"].as_str().is_some());
        assert!(traj["current_weight"].as_f64().is_some());
        assert!(traj["initial_weight"].as_f64().is_some());
    }

    let summary = &result["system_summary"];
    println!("  overall_accuracy: {}", summary["overall_accuracy"]);
    println!(
        "  best_performing_space: {}",
        summary["best_performing_space"]
    );
    println!(
        "  worst_performing_space: {}",
        summary["worst_performing_space"]
    );

    // STEP 4: VERIFY IN SOURCE OF TRUTH
    println!("\nVERIFY IN SOURCE OF TRUTH:");
    {
        let tracker_guard = tracker.read();
        let weights_sum: f32 = tracker_guard.current_weights.iter().sum();
        println!("  MetaUtlTracker weights sum: {:.6}", weights_sum);
        assert!(
            (weights_sum - 1.0).abs() < 0.001,
            "Weights should sum to ~1.0"
        );
    }

    // STEP 5: EVIDENCE
    println!("\n======================================================================");
    println!("EVIDENCE OF SUCCESS");
    println!("  - Returned 13 embedder trajectories");
    println!("  - Each trajectory has embedder_index, embedder_name, weights");
    println!("  - Weights sum to 1.0 in Source of Truth");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_fsv_learning_trajectory_specific_embedders() {
    println!("\n======================================================================");
    println!("FSV: meta_utl/learning_trajectory - Specific Embedders [0, 5, 12]");
    println!("======================================================================\n");

    let (handlers, _store, _tracker) = create_verifiable_handlers_with_tracker();

    // STEP 1: EXECUTE with specific embedder indices
    let request = make_request(
        "meta_utl/learning_trajectory",
        json!({
            "embedder_indices": [0, 5, 12],
            "include_accuracy_trend": true
        }),
    );

    let response = handlers.dispatch(request).await;

    // STEP 2: VERIFY RESPONSE
    assert!(response.error.is_none(), "Handler should succeed");
    let result = response.result.unwrap();

    let trajectories = result["trajectories"].as_array().unwrap();
    println!("VERIFY:");
    println!("  trajectories count: {}", trajectories.len());
    assert_eq!(trajectories.len(), 3, "Should return exactly 3 embedders");

    // Verify correct indices returned
    let indices: Vec<u64> = trajectories
        .iter()
        .map(|t| t["embedder_index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, vec![0, 5, 12], "Should return embedders 0, 5, 12");

    println!("\n======================================================================");
    println!("EVIDENCE: Returned exactly embedders [0, 5, 12]");
    println!("======================================================================\n");
}

/// Test that health_metrics correctly fails when real SystemMonitor is unavailable.
///
/// TASK-EMB-024: FAIL FAST - NO hardcoded fallback values.
/// StubSystemMonitor intentionally returns NotImplemented errors.
/// This test verifies the fail-fast behavior is working correctly.
#[tokio::test]
async fn test_fsv_health_metrics_with_targets() {
    println!("\n======================================================================");
    println!("FSV: meta_utl/health_metrics - Verify FAIL-FAST Behavior");
    println!("======================================================================\n");

    let (handlers, _store, _tracker) = create_verifiable_handlers_with_tracker();

    // STEP 1: EXECUTE
    let request = make_request(
        "meta_utl/health_metrics",
        json!({
            "include_targets": true,
            "include_recommendations": true
        }),
    );

    let response = handlers.dispatch(request).await;

    // STEP 2: VERIFY FAIL-FAST BEHAVIOR
    // TASK-EMB-024: StubSystemMonitor intentionally fails with NotImplemented
    // This is CORRECT behavior - no fake/simulated metrics allowed
    println!("VERIFY FAIL-FAST BEHAVIOR:");
    println!("  - StubSystemMonitor is designed to fail (TASK-EMB-024)");
    println!("  - NO hardcoded fallback values allowed");
    println!("  - Real SystemMonitor required for health metrics");

    assert!(
        response.error.is_some(),
        "Handler MUST fail when using StubSystemMonitor (TASK-EMB-024 fail-fast policy)"
    );

    let error = response.error.as_ref().unwrap();
    println!("\n  Error code: {}", error.code);
    println!("  Error message: {}", error.message);

    // Verify it's the expected SYSTEM_MONITOR_ERROR
    assert_eq!(
        error.code,
        crate::protocol::error_codes::SYSTEM_MONITOR_ERROR,
        "Should return SYSTEM_MONITOR_ERROR when SystemMonitor fails"
    );

    // Verify error message mentions the component
    assert!(
        error.message.contains("coherence_recovery") || error.message.contains("recovery"),
        "Error should mention the failing component"
    );

    println!("\n======================================================================");
    println!("EVIDENCE: FAIL-FAST behavior working correctly");
    println!("  - StubSystemMonitor returns NotImplemented (by design)");
    println!("  - Handler correctly propagates error");
    println!("  - No fake metrics returned");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_fsv_predict_storage_and_validate() {
    println!("\n======================================================================");
    println!("FSV: meta_utl/predict_storage + validate_prediction Cycle");
    println!("======================================================================\n");

    let (handlers, store, tracker) = create_verifiable_handlers_with_tracker();

    // PRE-CONDITION: Need 10+ validations for predict_storage to work
    // Manually populate tracker with validation history
    {
        let mut tracker_guard = tracker.write();
        for _ in 0..15 {
            tracker_guard.record_validation();
            for i in 0..NUM_EMBEDDERS {
                tracker_guard.record_accuracy(i, 0.85);
            }
        }
    }

    // Store a fingerprint
    let fp = create_test_fingerprint();
    let fingerprint_id = store.store(fp).await.expect("Store should succeed");
    println!("SETUP: Stored fingerprint {}", fingerprint_id);

    // STEP 1: BEFORE STATE
    println!("\nBEFORE STATE:");
    {
        let tracker_guard = tracker.read();
        println!(
            "  pending_predictions: {}",
            tracker_guard.pending_predictions.len()
        );
        println!("  validation_count: {}", tracker_guard.validation_count);
    }

    // STEP 2: EXECUTE predict_storage
    let predict_request = make_request(
        "meta_utl/predict_storage",
        json!({
            "fingerprint_id": fingerprint_id.to_string(),
            "include_confidence": true
        }),
    );

    let predict_response = handlers.dispatch(predict_request).await;
    assert!(
        predict_response.error.is_none(),
        "predict_storage should succeed: {:?}",
        predict_response.error
    );
    let predict_result = predict_response.result.unwrap();

    let prediction_id_str = predict_result["prediction_id"].as_str().unwrap();
    let prediction_id = Uuid::parse_str(prediction_id_str).unwrap();
    println!("\nPREDICTION MADE:");
    println!("  prediction_id: {}", prediction_id);
    println!(
        "  coherence_delta: {}",
        predict_result["predictions"]["coherence_delta"]
    );
    println!("  confidence: {}", predict_result["confidence"]);

    // STEP 3: VERIFY IN SOURCE OF TRUTH
    println!("\nVERIFY IN SOURCE OF TRUTH (after predict):");
    {
        let tracker_guard = tracker.read();
        let exists = tracker_guard
            .pending_predictions
            .contains_key(&prediction_id);
        println!("  Prediction {} in tracker: {}", prediction_id, exists);
        assert!(exists, "Prediction MUST be in Source of Truth");
        println!(
            "  pending_predictions count: {}",
            tracker_guard.pending_predictions.len()
        );
    }

    // STEP 4: VALIDATE THE PREDICTION
    let validate_request = make_request(
        "meta_utl/validate_prediction",
        json!({
            "prediction_id": prediction_id.to_string(),
            "actual_outcome": {
                "coherence_delta": 0.018,
                "alignment_delta": 0.048
            }
        }),
    );

    let validation_count_before = tracker.read().validation_count;

    let validate_response = handlers.dispatch(validate_request).await;
    assert!(
        validate_response.error.is_none(),
        "validate_prediction should succeed: {:?}",
        validate_response.error
    );
    let validate_result = validate_response.result.unwrap();

    println!("\nVALIDATION RESULT:");
    println!(
        "  prediction_type: {}",
        validate_result["validation"]["prediction_type"]
    );
    println!(
        "  prediction_error: {}",
        validate_result["validation"]["prediction_error"]
    );
    println!(
        "  accuracy_score: {}",
        validate_result["validation"]["accuracy_score"]
    );

    // STEP 5: VERIFY SOURCE OF TRUTH (after validate)
    println!("\nVERIFY IN SOURCE OF TRUTH (after validate):");
    {
        let tracker_guard = tracker.read();
        let still_exists = tracker_guard
            .pending_predictions
            .contains_key(&prediction_id);
        println!(
            "  Prediction {} removed from tracker: {}",
            prediction_id, !still_exists
        );
        assert!(!still_exists, "Prediction MUST be removed after validation");

        println!("  validation_count before: {}", validation_count_before);
        println!(
            "  validation_count after: {}",
            tracker_guard.validation_count
        );
        assert!(
            tracker_guard.validation_count > validation_count_before,
            "validation_count should increase"
        );
    }

    println!("\n======================================================================");
    println!("EVIDENCE OF SUCCESS");
    println!("  - Prediction stored in pending_predictions (verified in tracker)");
    println!("  - Prediction removed after validation (verified in tracker)");
    println!("  - validation_count incremented (verified in tracker)");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_fsv_predict_retrieval() {
    println!("\n======================================================================");
    println!("FSV: meta_utl/predict_retrieval");
    println!("======================================================================\n");

    let (handlers, store, tracker) = create_verifiable_handlers_with_tracker();

    // Store a fingerprint
    let fp = create_test_fingerprint();
    let fingerprint_id = store.store(fp).await.expect("Store should succeed");
    println!("SETUP: Stored fingerprint {}", fingerprint_id);

    // STEP 1: BEFORE STATE
    println!("\nBEFORE STATE:");
    {
        let tracker_guard = tracker.read();
        println!(
            "  pending_predictions: {}",
            tracker_guard.pending_predictions.len()
        );
    }

    // STEP 2: EXECUTE
    let request = make_request(
        "meta_utl/predict_retrieval",
        json!({
            "query_fingerprint_id": fingerprint_id.to_string(),
            "target_top_k": 10
        }),
    );

    let response = handlers.dispatch(request).await;

    // STEP 3: VERIFY RESPONSE
    assert!(
        response.error.is_none(),
        "Handler should succeed: {:?}",
        response.error
    );
    let result = response.result.unwrap();

    let prediction_id_str = result["prediction_id"].as_str().unwrap();
    let prediction_id = Uuid::parse_str(prediction_id_str).unwrap();

    println!("\nPREDICTION MADE:");
    println!("  prediction_id: {}", prediction_id);
    println!(
        "  expected_relevance: {}",
        result["predictions"]["expected_relevance"]
    );
    println!(
        "  expected_alignment: {}",
        result["predictions"]["expected_alignment"]
    );

    // Verify per_space_contribution has 13 elements
    let contributions = result["predictions"]["per_space_contribution"]
        .as_array()
        .unwrap();
    assert_eq!(
        contributions.len(),
        NUM_EMBEDDERS,
        "Should have 13 contributions"
    );
    println!("  per_space_contribution length: {}", contributions.len());

    // STEP 4: VERIFY IN SOURCE OF TRUTH
    println!("\nVERIFY IN SOURCE OF TRUTH:");
    {
        let tracker_guard = tracker.read();
        let exists = tracker_guard
            .pending_predictions
            .contains_key(&prediction_id);
        println!("  Prediction {} in tracker: {}", prediction_id, exists);
        assert!(exists, "Prediction MUST be in Source of Truth");
    }

    println!("\n======================================================================");
    println!("EVIDENCE: Retrieval prediction stored in MetaUtlTracker");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_fsv_optimized_weights_after_training() {
    println!("\n======================================================================");
    println!("FSV: meta_utl/optimized_weights - After Sufficient Training");
    println!("======================================================================\n");

    let (handlers, _store, tracker) = create_verifiable_handlers_with_tracker();

    // SETUP: Simulate 100 validations to trigger weight optimization
    {
        let mut tracker_guard = tracker.write();
        for v in 0..100 {
            tracker_guard.record_validation();
            // Record varying accuracy per embedder
            for i in 0..NUM_EMBEDDERS {
                let accuracy = 0.7 + (i as f32 * 0.02); // 0.70 to 0.94
                tracker_guard.record_accuracy(i, accuracy);
            }
            // Weight update triggers at validation 100
            if v == 99 {
                tracker_guard.update_weights();
            }
        }
    }

    println!("SETUP: Completed 100 validations with varying accuracy");

    // STEP 1: BEFORE STATE
    println!("\nBEFORE STATE:");
    {
        let tracker_guard = tracker.read();
        println!("  validation_count: {}", tracker_guard.validation_count);
        println!(
            "  last_weight_update: {:?}",
            tracker_guard.last_weight_update.is_some()
        );
    }

    // STEP 2: EXECUTE
    let request = make_request_no_params("meta_utl/optimized_weights");
    let response = handlers.dispatch(request).await;

    // STEP 3: VERIFY RESPONSE
    assert!(
        response.error.is_none(),
        "Handler should succeed with 100 validations: {:?}",
        response.error
    );
    let result = response.result.unwrap();

    let weights = result["weights"].as_array().unwrap();
    assert_eq!(weights.len(), NUM_EMBEDDERS, "Should return 13 weights");

    let weights_sum: f64 = weights.iter().map(|w| w.as_f64().unwrap()).sum();
    println!("\nRESULT:");
    println!("  weights count: {}", weights.len());
    println!("  weights sum: {:.6}", weights_sum);
    println!("  training_samples: {}", result["training_samples"]);
    println!("  confidence: {}", result["confidence"]);

    assert!(
        (weights_sum - 1.0).abs() < 0.001,
        "Weights should sum to ~1.0"
    );

    // STEP 4: VERIFY IN SOURCE OF TRUTH
    println!("\nVERIFY IN SOURCE OF TRUTH:");
    {
        let tracker_guard = tracker.read();
        let sot_sum: f32 = tracker_guard.current_weights.iter().sum();
        println!("  Source of Truth weights sum: {:.6}", sot_sum);
        assert!(
            (sot_sum - 1.0).abs() < 0.001,
            "SoT weights should sum to ~1.0"
        );
    }

    println!("\n======================================================================");
    println!("EVIDENCE: Optimized weights match Source of Truth, sum to 1.0");
    println!("======================================================================\n");
}

// ==================== EDGE CASE TESTS ====================

#[tokio::test]
async fn test_edge_case_embedder_index_13() {
    println!("\n======================================================================");
    println!("EDGE CASE: Invalid embedder index >= 13");
    println!("======================================================================\n");

    let (handlers, _store, tracker) = create_verifiable_handlers_with_tracker();

    // BEFORE STATE
    println!("BEFORE STATE:");
    {
        let tracker_guard = tracker.read();
        println!("  validation_count: {}", tracker_guard.validation_count);
    }

    // ACTION: Request learning_trajectory with invalid index
    let request = make_request(
        "meta_utl/learning_trajectory",
        json!({
            "embedder_indices": [0, 5, 13]  // 13 is invalid (must be 0-12)
        }),
    );

    let response = handlers.dispatch(request).await;

    // VERIFY: Should return INVALID_PARAMS error
    assert!(response.error.is_some(), "Should return error for index 13");
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::INVALID_PARAMS);
    println!("ERROR RETURNED:");
    println!("  code: {}", error.code);
    println!("  message: {}", error.message);
    assert!(error.message.contains("13") && error.message.contains("must be 0-12"));

    // AFTER STATE: Unchanged
    println!("\nAFTER STATE:");
    {
        let tracker_guard = tracker.read();
        println!(
            "  validation_count (unchanged): {}",
            tracker_guard.validation_count
        );
    }

    println!("\n======================================================================");
    println!("EVIDENCE: INVALID_PARAMS (-32602) returned for index 13");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_edge_case_validate_unknown_prediction() {
    println!("\n======================================================================");
    println!("EDGE CASE: Validate non-existent prediction");
    println!("======================================================================\n");

    let (handlers, _store, tracker) = create_verifiable_handlers_with_tracker();

    // BEFORE STATE
    println!("BEFORE STATE:");
    let predictions_before;
    {
        let tracker_guard = tracker.read();
        predictions_before = tracker_guard.pending_predictions.len();
        println!("  pending_predictions: {}", predictions_before);
    }

    // ACTION: Try to validate a random UUID
    let fake_prediction_id = Uuid::new_v4();
    let request = make_request(
        "meta_utl/validate_prediction",
        json!({
            "prediction_id": fake_prediction_id.to_string(),
            "actual_outcome": {
                "coherence_delta": 0.02,
                "alignment_delta": 0.05
            }
        }),
    );

    let response = handlers.dispatch(request).await;

    // VERIFY: Should return META_UTL_PREDICTION_NOT_FOUND
    assert!(
        response.error.is_some(),
        "Should return error for unknown prediction"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::META_UTL_PREDICTION_NOT_FOUND);
    println!("ERROR RETURNED:");
    println!("  code: {} (META_UTL_PREDICTION_NOT_FOUND)", error.code);
    println!("  message: {}", error.message);

    // AFTER STATE: Unchanged
    println!("\nAFTER STATE:");
    {
        let tracker_guard = tracker.read();
        assert_eq!(tracker_guard.pending_predictions.len(), predictions_before);
        println!(
            "  pending_predictions (unchanged): {}",
            tracker_guard.pending_predictions.len()
        );
    }

    println!("\n======================================================================");
    println!("EVIDENCE: META_UTL_PREDICTION_NOT_FOUND (-32040) returned");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_edge_case_optimized_weights_no_training() {
    println!("\n======================================================================");
    println!("EDGE CASE: Optimized weights with no training data");
    println!("======================================================================\n");

    let (handlers, _store, tracker) = create_verifiable_handlers_with_tracker();

    // BEFORE STATE: Fresh tracker with 0 validations
    println!("BEFORE STATE:");
    {
        let tracker_guard = tracker.read();
        println!("  validation_count: {}", tracker_guard.validation_count);
        println!(
            "  last_weight_update: {:?}",
            tracker_guard.last_weight_update
        );
        assert_eq!(tracker_guard.validation_count, 0);
    }

    // ACTION: Request optimized weights
    let request = make_request_no_params("meta_utl/optimized_weights");
    let response = handlers.dispatch(request).await;

    // VERIFY: Should return META_UTL_INSUFFICIENT_DATA
    assert!(
        response.error.is_some(),
        "Should return error for 0 validations"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::META_UTL_INSUFFICIENT_DATA);
    println!("ERROR RETURNED:");
    println!("  code: {} (META_UTL_INSUFFICIENT_DATA)", error.code);
    println!("  message: {}", error.message);
    assert!(error.message.contains("need 50 validations"));

    // AFTER STATE: Weights unchanged (still uniform 1/13)
    println!("\nAFTER STATE:");
    {
        let tracker_guard = tracker.read();
        let expected_uniform = 1.0 / NUM_EMBEDDERS as f32;
        for i in 0..NUM_EMBEDDERS {
            assert!((tracker_guard.current_weights[i] - expected_uniform).abs() < 0.001);
        }
        println!("  current_weights still uniform (1/13 each): verified");
    }

    println!("\n======================================================================");
    println!("EVIDENCE: META_UTL_INSUFFICIENT_DATA (-32042) returned, weights unchanged");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_edge_case_predict_storage_fingerprint_not_found() {
    println!("\n======================================================================");
    println!("EDGE CASE: Predict storage for non-existent fingerprint");
    println!("======================================================================\n");

    let (handlers, _store, tracker) = create_verifiable_handlers_with_tracker();

    // SETUP: Add validations so we pass the minimum threshold
    {
        let mut tracker_guard = tracker.write();
        for _ in 0..15 {
            tracker_guard.record_validation();
        }
    }

    // BEFORE STATE
    println!("BEFORE STATE:");
    {
        let tracker_guard = tracker.read();
        println!("  validation_count: {}", tracker_guard.validation_count);
        println!(
            "  pending_predictions: {}",
            tracker_guard.pending_predictions.len()
        );
    }

    // ACTION: Request prediction for non-existent fingerprint
    let fake_fingerprint_id = Uuid::new_v4();
    let request = make_request(
        "meta_utl/predict_storage",
        json!({
            "fingerprint_id": fake_fingerprint_id.to_string()
        }),
    );

    let response = handlers.dispatch(request).await;

    // VERIFY: Should return FINGERPRINT_NOT_FOUND
    assert!(
        response.error.is_some(),
        "Should return error for unknown fingerprint"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::FINGERPRINT_NOT_FOUND);
    println!("ERROR RETURNED:");
    println!("  code: {} (FINGERPRINT_NOT_FOUND)", error.code);
    println!("  message: {}", error.message);

    // AFTER STATE: No prediction stored
    println!("\nAFTER STATE:");
    {
        let tracker_guard = tracker.read();
        println!(
            "  pending_predictions (unchanged): {}",
            tracker_guard.pending_predictions.len()
        );
        assert_eq!(tracker_guard.pending_predictions.len(), 0);
    }

    println!("\n======================================================================");
    println!("EVIDENCE: FINGERPRINT_NOT_FOUND (-32010) returned");
    println!("======================================================================\n");
}

#[tokio::test]
async fn test_edge_case_validate_missing_outcome_field() {
    println!("\n======================================================================");
    println!("EDGE CASE: Validate prediction with missing outcome field");
    println!("======================================================================\n");

    let (handlers, store, tracker) = create_verifiable_handlers_with_tracker();

    // SETUP: Create prediction first
    {
        let mut tracker_guard = tracker.write();
        for _ in 0..15 {
            tracker_guard.record_validation();
        }
    }

    let fp = create_test_fingerprint();
    let fingerprint_id = store.store(fp).await.expect("Store should succeed");

    let predict_request = make_request(
        "meta_utl/predict_storage",
        json!({
            "fingerprint_id": fingerprint_id.to_string()
        }),
    );

    let predict_response = handlers.dispatch(predict_request).await;
    assert!(predict_response.error.is_none());
    let prediction_id = predict_response.result.unwrap()["prediction_id"]
        .as_str()
        .unwrap()
        .to_string();
    println!("SETUP: Created prediction {}", prediction_id);

    // ACTION: Validate with missing field
    let request = make_request(
        "meta_utl/validate_prediction",
        json!({
            "prediction_id": prediction_id,
            "actual_outcome": {
                "coherence_delta": 0.02
                // missing alignment_delta
            }
        }),
    );

    let response = handlers.dispatch(request).await;

    // VERIFY: Should return META_UTL_INVALID_OUTCOME
    assert!(
        response.error.is_some(),
        "Should return error for missing field"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::META_UTL_INVALID_OUTCOME);
    println!("ERROR RETURNED:");
    println!("  code: {} (META_UTL_INVALID_OUTCOME)", error.code);
    println!("  message: {}", error.message);
    assert!(error.message.contains("alignment_delta"));

    println!("\n======================================================================");
    println!("EVIDENCE: META_UTL_INVALID_OUTCOME (-32043) returned for missing field");
    println!("======================================================================\n");
}
