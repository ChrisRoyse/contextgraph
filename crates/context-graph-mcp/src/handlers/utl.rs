//! UTL computation handlers.
//!
//! TASK-S005: Extended with 6 Meta-UTL handlers for "learning about learning".

use std::time::Instant;

use serde_json::json;
use tracing::{debug, error, warn};
use uuid::Uuid;

use context_graph_core::johari::NUM_EMBEDDERS;
use context_graph_core::types::{CognitivePulse, SuggestedAction, UtlContext};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::core::{PredictionType, StoredPrediction};
use super::Handlers;

/// Embedder names for trajectory reporting.
/// 13 embedders: E1-E13.
const EMBEDDER_NAMES: [&str; NUM_EMBEDDERS] = [
    "semantic",       // E1
    "episodic",       // E2
    "procedural",     // E3
    "emotional",      // E4
    "temporal",       // E5
    "causal",         // E6
    "analogical",     // E7
    "contextual",     // E8
    "hierarchical",   // E9
    "associative",    // E10
    "metacognitive",  // E11
    "intentional",    // E12
    "sparse_lexical", // E13 (SPLADE)
];

/// Constitution.yaml targets (hardcoded per TASK-S005 spec).
const LEARNING_SCORE_TARGET: f32 = 0.6;
const COHERENCE_RECOVERY_TARGET_MS: u64 = 10000;
const ATTACK_DETECTION_TARGET: f32 = 0.95;
const FALSE_POSITIVE_TARGET: f32 = 0.02;

impl Handlers {
    /// Handle utl/compute request.
    pub(super) async fn handle_utl_compute(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters",
                );
            }
        };

        let input = match params.get("input").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'input' parameter",
                );
            }
        };

        let context = UtlContext::default();

        match self
            .utl_processor
            .compute_learning_score(input, &context)
            .await
        {
            Ok(score) => {
                let action = if score > 0.7 {
                    SuggestedAction::Consolidate
                } else if score > 0.4 {
                    SuggestedAction::Continue
                } else {
                    SuggestedAction::Explore
                };

                let pulse = CognitivePulse::new(
                    context.prior_entropy,
                    context.current_coherence,
                    0.0,
                    1.0,
                    action,
                    None,
                );
                JsonRpcResponse::success(id, json!({ "learningScore": score })).with_pulse(pulse)
            }
            Err(e) => JsonRpcResponse::error(id, error_codes::INTERNAL_ERROR, e.to_string()),
        }
    }

    /// Handle utl/metrics request.
    pub(super) async fn handle_utl_metrics(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters",
                );
            }
        };

        let input = match params.get("input").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'input' parameter",
                );
            }
        };

        let context = UtlContext::default();

        match self.utl_processor.compute_metrics(input, &context).await {
            Ok(metrics) => JsonRpcResponse::success(
                id,
                json!({
                    "entropy": metrics.entropy,
                    "coherence": metrics.coherence,
                    "learningScore": metrics.learning_score,
                    "surprise": metrics.surprise,
                    "coherenceChange": metrics.coherence_change,
                    "emotionalWeight": metrics.emotional_weight,
                    "alignment": metrics.alignment,
                }),
            ),
            Err(e) => JsonRpcResponse::error(id, error_codes::INTERNAL_ERROR, e.to_string()),
        }
    }

    // =========================================================================
    // Meta-UTL Handlers (TASK-S005)
    // =========================================================================

    /// Handle meta_utl/learning_trajectory request.
    ///
    /// Returns per-embedder learning trajectories with accuracy trends.
    /// TASK-S005: Exposes MetaUtlTracker accuracy data for monitoring.
    pub(super) async fn handle_meta_utl_learning_trajectory(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("meta_utl/learning_trajectory: starting");

        // Parse optional parameters
        let params = params.unwrap_or(json!({}));

        // Parse embedder_indices - validate all are < 13
        let embedder_indices: Vec<usize> = match params.get("embedder_indices") {
            Some(indices) => {
                let indices = match indices.as_array() {
                    Some(arr) => arr,
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            "embedder_indices must be an array",
                        );
                    }
                };
                let mut result = Vec::with_capacity(indices.len());
                for idx in indices {
                    let idx = match idx.as_u64() {
                        Some(n) => n as usize,
                        None => {
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                "embedder_indices must contain integers",
                            );
                        }
                    };
                    if idx >= NUM_EMBEDDERS {
                        warn!(
                            "meta_utl/learning_trajectory: invalid embedder index {}",
                            idx
                        );
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Invalid embedder index {}: must be 0-12", idx),
                        );
                    }
                    result.push(idx);
                }
                result
            }
            None => (0..NUM_EMBEDDERS).collect(), // All 13 embedders
        };

        let _history_window = params
            .get("history_window")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let include_accuracy_trend = params
            .get("include_accuracy_trend")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Get tracker data
        let tracker = self.meta_utl_tracker.read();

        // Build trajectories
        let initial_weight = 1.0 / NUM_EMBEDDERS as f32;
        let mut trajectories = Vec::with_capacity(embedder_indices.len());
        let mut total_accuracy = 0.0f32;
        let mut accuracy_count = 0usize;
        let mut best_space = 0usize;
        let mut best_accuracy = 0.0f32;
        let mut worst_space = 0usize;
        let mut worst_accuracy = 1.0f32;
        let mut spaces_above_target = 0usize;
        

        for &idx in &embedder_indices {
            let current_weight = tracker.current_weights[idx];
            let recent_accuracy = tracker.get_embedder_accuracy(idx);

            // Build accuracy history (last few samples from rolling window)
            let count = tracker.accuracy_counts[idx];
            let history_len = count.min(4);
            let mut accuracy_history = Vec::with_capacity(history_len);
            if count > 0 {
                let start_idx = count.saturating_sub(history_len);
                for i in start_idx..count {
                    accuracy_history.push(tracker.embedder_accuracy[idx][i]);
                }
            }

            let accuracy_trend = if include_accuracy_trend {
                tracker.get_accuracy_trend(idx)
            } else {
                None
            };

            let acc = recent_accuracy.unwrap_or(0.0);
            if acc > best_accuracy {
                best_accuracy = acc;
                best_space = idx;
            }
            if acc < worst_accuracy {
                worst_accuracy = acc;
                worst_space = idx;
            }
            if acc >= LEARNING_SCORE_TARGET {
                spaces_above_target += 1;
            }
            if recent_accuracy.is_some() {
                total_accuracy += acc;
                accuracy_count += 1;
            }

            trajectories.push(json!({
                "embedder_index": idx,
                "embedder_name": EMBEDDER_NAMES[idx],
                "current_weight": current_weight,
                "initial_weight": initial_weight,
                "weight_delta": current_weight - initial_weight,
                "recent_accuracy": recent_accuracy,
                "prediction_count": tracker.prediction_count,
                "accuracy_trend": accuracy_trend,
                "accuracy_history": accuracy_history,
            }));
        }

        let spaces_below_target = embedder_indices.len() - spaces_above_target;

        let overall_accuracy = if accuracy_count > 0 {
            total_accuracy / accuracy_count as f32
        } else {
            0.0
        };

        debug!(
            "meta_utl/learning_trajectory: returning {} trajectories",
            trajectories.len()
        );

        JsonRpcResponse::success(
            id,
            json!({
                "trajectories": trajectories,
                "system_summary": {
                    "overall_accuracy": overall_accuracy,
                    "best_performing_space": best_space,
                    "worst_performing_space": worst_space,
                    "spaces_above_target": spaces_above_target,
                    "spaces_below_target": spaces_below_target,
                }
            }),
        )
    }

    /// Handle meta_utl/health_metrics request.
    ///
    /// Returns system health metrics with constitution.yaml targets.
    /// TASK-S005: Hardcoded targets from constitution.yaml.
    pub(super) async fn handle_meta_utl_health_metrics(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("meta_utl/health_metrics: starting");

        let params = params.unwrap_or(json!({}));

        let include_targets = params
            .get("include_targets")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_recommendations = params
            .get("include_recommendations")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get tracker for per-space accuracy
        let tracker = self.meta_utl_tracker.read();

        // Calculate per-space accuracy
        let mut per_space_accuracy = Vec::with_capacity(NUM_EMBEDDERS);
        let mut total_accuracy = 0.0f32;
        let mut accuracy_count = 0usize;

        for i in 0..NUM_EMBEDDERS {
            let acc = tracker.get_embedder_accuracy(i).unwrap_or(0.0);
            per_space_accuracy.push(acc);
            if tracker.accuracy_counts[i] > 0 {
                total_accuracy += acc;
                accuracy_count += 1;
            }
        }

        // Compute metrics
        let learning_score = if accuracy_count > 0 {
            total_accuracy / accuracy_count as f32
        } else {
            0.5 // Default when no data
        };

        // TASK-EMB-024: Get REAL metrics from SystemMonitor - NO HARDCODED VALUES
        // FAIL FAST if SystemMonitor is not configured
        let coherence_recovery_time_ms = match self.system_monitor.coherence_recovery_time_ms().await {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "meta_utl/health_metrics: coherence_recovery_time_ms FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::SYSTEM_MONITOR_ERROR,
                    format!("Failed to get coherence_recovery_time_ms: {}", e),
                );
            }
        };

        let attack_detection_rate = match self.system_monitor.attack_detection_rate().await {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "meta_utl/health_metrics: attack_detection_rate FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::SYSTEM_MONITOR_ERROR,
                    format!("Failed to get attack_detection_rate: {}", e),
                );
            }
        };

        let false_positive_rate = match self.system_monitor.false_positive_rate().await {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "meta_utl/health_metrics: false_positive_rate FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::SYSTEM_MONITOR_ERROR,
                    format!("Failed to get false_positive_rate: {}", e),
                );
            }
        };

        // Check against targets
        let learning_score_status = if learning_score >= LEARNING_SCORE_TARGET {
            "passing"
        } else {
            "failing"
        };
        let coherence_recovery_status = if coherence_recovery_time_ms < COHERENCE_RECOVERY_TARGET_MS
        {
            "passing"
        } else {
            "failing"
        };
        let attack_detection_status = if attack_detection_rate >= ATTACK_DETECTION_TARGET {
            "passing"
        } else {
            "failing"
        };
        let false_positive_status = if false_positive_rate < FALSE_POSITIVE_TARGET {
            "passing"
        } else {
            "failing"
        };

        // Determine overall status and failed targets
        let mut failed_targets: Vec<&str> = Vec::new();
        if learning_score < LEARNING_SCORE_TARGET {
            failed_targets.push("learning_score");
        }
        if coherence_recovery_time_ms >= COHERENCE_RECOVERY_TARGET_MS {
            failed_targets.push("coherence_recovery_time_ms");
        }
        if attack_detection_rate < ATTACK_DETECTION_TARGET {
            failed_targets.push("attack_detection_rate");
        }
        if false_positive_rate >= FALSE_POSITIVE_TARGET {
            failed_targets.push("false_positive_rate");
        }

        let overall_status = if failed_targets.is_empty() {
            "healthy"
        } else if failed_targets.len() <= 1 {
            "degraded"
        } else {
            "unhealthy"
        };

        // Build recommendations if requested
        let recommendations: Vec<&str> = if include_recommendations && !failed_targets.is_empty() {
            failed_targets
                .iter()
                .map(|t| match *t {
                    "learning_score" => "Increase training data quality or quantity",
                    "coherence_recovery_time_ms" => "Optimize cache invalidation strategy",
                    "attack_detection_rate" => "Enhance anomaly detection thresholds",
                    "false_positive_rate" => "Adjust classification sensitivity",
                    _ => "Review system configuration",
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut metrics = json!({
            "learning_score": learning_score,
            "coherence_recovery_time_ms": coherence_recovery_time_ms,
            "attack_detection_rate": attack_detection_rate,
            "false_positive_rate": false_positive_rate,
            "per_space_accuracy": per_space_accuracy,
        });

        // Add target fields if requested
        if include_targets {
            metrics["learning_score_target"] = json!(LEARNING_SCORE_TARGET);
            metrics["learning_score_status"] = json!(learning_score_status);
            metrics["coherence_recovery_target_ms"] = json!(COHERENCE_RECOVERY_TARGET_MS);
            metrics["coherence_recovery_status"] = json!(coherence_recovery_status);
            metrics["attack_detection_target"] = json!(ATTACK_DETECTION_TARGET);
            metrics["attack_detection_status"] = json!(attack_detection_status);
            metrics["false_positive_target"] = json!(FALSE_POSITIVE_TARGET);
            metrics["false_positive_status"] = json!(false_positive_status);
        }

        debug!(
            "meta_utl/health_metrics: overall_status={}, failed={}",
            overall_status,
            failed_targets.len()
        );

        JsonRpcResponse::success(
            id,
            json!({
                "metrics": metrics,
                "overall_status": overall_status,
                "failed_targets": failed_targets,
                "recommendations": recommendations,
            }),
        )
    }

    /// Handle meta_utl/predict_storage request.
    ///
    /// Predicts storage impact before committing a fingerprint.
    /// TASK-S005: Stores prediction in MetaUtlTracker for later validation.
    pub(super) async fn handle_meta_utl_predict_storage(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("meta_utl/predict_storage: starting");

        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters",
                );
            }
        };

        // Parse fingerprint_id
        let fingerprint_id_str = match params.get("fingerprint_id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'fingerprint_id' parameter",
                );
            }
        };

        let fingerprint_id = match Uuid::parse_str(fingerprint_id_str) {
            Ok(uuid) => uuid,
            Err(_) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid UUID format: {}", fingerprint_id_str),
                );
            }
        };

        let include_confidence = params
            .get("include_confidence")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Verify fingerprint exists
        match self.teleological_store.retrieve(fingerprint_id).await {
            Ok(Some(_fingerprint)) => {
                // Fingerprint exists, proceed with prediction
            }
            Ok(None) => {
                warn!(
                    "meta_utl/predict_storage: fingerprint not found: {}",
                    fingerprint_id
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Fingerprint not found: {}", fingerprint_id),
                );
            }
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to retrieve fingerprint: {}", e),
                );
            }
        }

        // Check for sufficient validation data
        let tracker = self.meta_utl_tracker.read();
        if tracker.validation_count < 10 {
            drop(tracker); // Release lock before returning error
            warn!(
                "meta_utl/predict_storage: insufficient data, need 10 validations, have {}",
                self.meta_utl_tracker.read().validation_count
            );
            return JsonRpcResponse::error(
                id,
                error_codes::META_UTL_INSUFFICIENT_DATA,
                format!(
                    "Insufficient data: need 10 validations, have {}",
                    self.meta_utl_tracker.read().validation_count
                ),
            );
        }
        drop(tracker);

        // Generate prediction
        let prediction_id = Uuid::new_v4();
        let coherence_delta: f32 = 0.02;
        let alignment_delta: f32 = 0.05;
        let storage_impact_bytes: u64 = 4096;
        let index_rebuild_required = false;

        // Calculate confidence based on validation history
        let tracker = self.meta_utl_tracker.read();
        let confidence = if tracker.validation_count >= 50 {
            // Higher confidence with more validations
            let accuracy_sum: f32 = (0..NUM_EMBEDDERS)
                .filter_map(|i| tracker.get_embedder_accuracy(i))
                .sum();
            let accuracy_count = (0..NUM_EMBEDDERS)
                .filter(|&i| tracker.accuracy_counts[i] > 0)
                .count();
            if accuracy_count > 0 {
                (accuracy_sum / accuracy_count as f32).min(0.99)
            } else {
                0.5
            }
        } else {
            // Lower confidence with fewer validations
            0.5 + (tracker.validation_count as f32 / 100.0)
        };
        drop(tracker);

        // Store prediction for later validation
        let predicted_values = json!({
            "coherence_delta": coherence_delta,
            "alignment_delta": alignment_delta,
            "storage_impact_bytes": storage_impact_bytes,
            "index_rebuild_required": index_rebuild_required,
        });

        let stored_prediction = StoredPrediction {
            _created_at: Instant::now(),
            prediction_type: PredictionType::Storage,
            predicted_values: predicted_values.clone(),
            fingerprint_id,
        };

        {
            let mut tracker = self.meta_utl_tracker.write();
            tracker.store_prediction(prediction_id, stored_prediction);
        }

        debug!(
            "meta_utl/predict_storage: stored prediction {} for fingerprint {}",
            prediction_id, fingerprint_id
        );

        let mut response = json!({
            "predictions": {
                "coherence_delta": coherence_delta,
                "alignment_delta": alignment_delta,
                "storage_impact_bytes": storage_impact_bytes,
                "index_rebuild_required": index_rebuild_required,
            },
            "prediction_id": prediction_id.to_string(),
        });

        if include_confidence {
            response["confidence"] = json!(confidence);
        }

        JsonRpcResponse::success(id, response)
    }

    /// Handle meta_utl/predict_retrieval request.
    ///
    /// Predicts retrieval quality before querying.
    /// TASK-S005: Stores prediction in MetaUtlTracker for later validation.
    pub(super) async fn handle_meta_utl_predict_retrieval(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("meta_utl/predict_retrieval: starting");

        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters",
                );
            }
        };

        // Parse query_fingerprint_id
        let query_fingerprint_id_str =
            match params.get("query_fingerprint_id").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        "Missing 'query_fingerprint_id' parameter",
                    );
                }
            };

        let query_fingerprint_id = match Uuid::parse_str(query_fingerprint_id_str) {
            Ok(uuid) => uuid,
            Err(_) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid UUID format: {}", query_fingerprint_id_str),
                );
            }
        };

        let target_top_k = params
            .get("target_top_k")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        // Verify fingerprint exists
        match self.teleological_store.retrieve(query_fingerprint_id).await {
            Ok(Some(_fingerprint)) => {
                // Fingerprint exists, proceed with prediction
            }
            Ok(None) => {
                warn!(
                    "meta_utl/predict_retrieval: fingerprint not found: {}",
                    query_fingerprint_id
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Fingerprint not found: {}", query_fingerprint_id),
                );
            }
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to retrieve fingerprint: {}", e),
                );
            }
        }

        // Generate prediction
        let prediction_id = Uuid::new_v4();

        // Calculate per-space contributions from current weights
        let tracker = self.meta_utl_tracker.read();
        let per_space_contribution: Vec<f32> = tracker.current_weights.to_vec();

        // Expected metrics based on historical accuracy
        let expected_relevance: f32 = {
            let mut total = 0.0f32;
            let mut count = 0usize;
            for i in 0..NUM_EMBEDDERS {
                if let Some(acc) = tracker.get_embedder_accuracy(i) {
                    total += acc * tracker.current_weights[i];
                    count += 1;
                }
            }
            if count > 0 {
                (total / count as f32 * NUM_EMBEDDERS as f32).min(0.95)
            } else {
                0.7
            }
        };

        let expected_alignment: f32 = expected_relevance * 1.1; // Slightly higher for alignment
        let expected_result_count = (target_top_k as f32 * 0.8).ceil() as usize;

        // Calculate confidence
        let confidence = if tracker.validation_count >= 50 {
            let accuracy_sum: f32 = (0..NUM_EMBEDDERS)
                .filter_map(|i| tracker.get_embedder_accuracy(i))
                .sum();
            let accuracy_count = (0..NUM_EMBEDDERS)
                .filter(|&i| tracker.accuracy_counts[i] > 0)
                .count();
            if accuracy_count > 0 {
                (accuracy_sum / accuracy_count as f32).min(0.99)
            } else {
                0.5
            }
        } else {
            0.4 + (tracker.validation_count as f32 / 125.0)
        };
        drop(tracker);

        // Store prediction for later validation
        let predicted_values = json!({
            "expected_relevance": expected_relevance,
            "expected_alignment": expected_alignment,
            "expected_result_count": expected_result_count,
            "per_space_contribution": per_space_contribution,
            "target_top_k": target_top_k,
        });

        let stored_prediction = StoredPrediction {
            _created_at: Instant::now(),
            prediction_type: PredictionType::Retrieval,
            predicted_values: predicted_values.clone(),
            fingerprint_id: query_fingerprint_id,
        };

        {
            let mut tracker = self.meta_utl_tracker.write();
            tracker.store_prediction(prediction_id, stored_prediction);
        }

        debug!(
            "meta_utl/predict_retrieval: stored prediction {} for query {}",
            prediction_id, query_fingerprint_id
        );

        JsonRpcResponse::success(
            id,
            json!({
                "predictions": {
                    "expected_relevance": expected_relevance,
                    "expected_alignment": expected_alignment,
                    "expected_result_count": expected_result_count,
                    "per_space_contribution": per_space_contribution,
                },
                "confidence": confidence,
                "prediction_id": prediction_id.to_string(),
            }),
        )
    }

    /// Handle meta_utl/validate_prediction request.
    ///
    /// Validates a prediction against actual outcome.
    /// TASK-S005: Updates embedder_accuracy and triggers weight optimization.
    pub(super) async fn handle_meta_utl_validate_prediction(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("meta_utl/validate_prediction: starting");

        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters",
                );
            }
        };

        // Parse prediction_id
        let prediction_id_str = match params.get("prediction_id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'prediction_id' parameter",
                );
            }
        };

        let prediction_id = match Uuid::parse_str(prediction_id_str) {
            Ok(uuid) => uuid,
            Err(_) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid UUID format: {}", prediction_id_str),
                );
            }
        };

        // Parse actual_outcome
        let actual_outcome = match params.get("actual_outcome") {
            Some(outcome) => outcome.clone(),
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'actual_outcome' parameter",
                );
            }
        };

        // Get stored prediction
        let stored_prediction = {
            let mut tracker = self.meta_utl_tracker.write();
            match tracker.remove_prediction(&prediction_id) {
                Some(p) => p,
                None => {
                    warn!(
                        "meta_utl/validate_prediction: prediction not found: {}",
                        prediction_id
                    );
                    return JsonRpcResponse::error(
                        id,
                        error_codes::META_UTL_PREDICTION_NOT_FOUND,
                        format!("Prediction not found: {}", prediction_id),
                    );
                }
            }
        };

        let prediction_type = match stored_prediction.prediction_type {
            PredictionType::Storage => "storage",
            PredictionType::Retrieval => "retrieval",
        };

        // Calculate prediction error based on type
        let prediction_error: f32;
        let accuracy_score: f32;

        match stored_prediction.prediction_type {
            PredictionType::Storage => {
                // Validate storage prediction
                let predicted_coherence = stored_prediction
                    .predicted_values
                    .get("coherence_delta")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let predicted_alignment = stored_prediction
                    .predicted_values
                    .get("alignment_delta")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;

                let actual_coherence = match actual_outcome
                    .get("coherence_delta")
                    .and_then(|v| v.as_f64())
                {
                    Some(c) => c as f32,
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::META_UTL_INVALID_OUTCOME,
                            "Invalid outcome: missing field 'coherence_delta'",
                        );
                    }
                };
                let actual_alignment =
                    match actual_outcome.get("alignment_delta").and_then(|v| v.as_f64()) {
                        Some(a) => a as f32,
                        None => {
                            return JsonRpcResponse::error(
                                id,
                                error_codes::META_UTL_INVALID_OUTCOME,
                                "Invalid outcome: missing field 'alignment_delta'",
                            );
                        }
                    };

                let coherence_error = (predicted_coherence - actual_coherence).abs();
                let alignment_error = (predicted_alignment - actual_alignment).abs();
                prediction_error = (coherence_error + alignment_error) / 2.0;
                accuracy_score = 1.0 - prediction_error.min(1.0);
            }
            PredictionType::Retrieval => {
                // Validate retrieval prediction
                let predicted_relevance = stored_prediction
                    .predicted_values
                    .get("expected_relevance")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;

                let actual_relevance = match actual_outcome
                    .get("actual_relevance")
                    .and_then(|v| v.as_f64())
                {
                    Some(r) => r as f32,
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::META_UTL_INVALID_OUTCOME,
                            "Invalid outcome: missing field 'actual_relevance'",
                        );
                    }
                };

                prediction_error = (predicted_relevance - actual_relevance).abs();
                accuracy_score = 1.0 - prediction_error.min(1.0);
            }
        }

        // Update embedder accuracy for all embedders (weighted by contribution)
        let mut tracker = self.meta_utl_tracker.write();
        // Copy weights first to avoid borrow conflict
        let weights = tracker.current_weights;
        for i in 0..NUM_EMBEDDERS {
            // Weight accuracy by the embedder's contribution
            let weighted_accuracy = accuracy_score * weights[i];
            tracker.record_accuracy(i, weighted_accuracy + (1.0 - weights[i]));
        }

        // Record validation (triggers weight update every 100 validations)
        tracker.record_validation();

        // Get new accuracies
        let new_embedder_accuracy: Vec<Option<f32>> = (0..NUM_EMBEDDERS)
            .map(|i| tracker.get_embedder_accuracy(i))
            .collect();

        let accuracy_updated = tracker.validation_count.is_multiple_of(100);

        debug!(
            "meta_utl/validate_prediction: validated {} prediction {} with accuracy {}",
            prediction_type, prediction_id, accuracy_score
        );

        JsonRpcResponse::success(
            id,
            json!({
                "validation": {
                    "prediction_type": prediction_type,
                    "prediction_error": prediction_error,
                    "accuracy_score": accuracy_score,
                    "accuracy_updated": accuracy_updated,
                    "new_embedder_accuracy": new_embedder_accuracy,
                }
            }),
        )
    }

    /// Handle meta_utl/optimized_weights request.
    ///
    /// Returns current meta-learned optimized weights.
    /// TASK-S005: Requires sufficient validation data before returning.
    pub(super) async fn handle_meta_utl_optimized_weights(
        &self,
        id: Option<JsonRpcId>,
        _params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("meta_utl/optimized_weights: starting");

        let tracker = self.meta_utl_tracker.read();

        // Check for sufficient data
        if tracker.validation_count < 50 {
            warn!(
                "meta_utl/optimized_weights: insufficient data, need 50 validations, have {}",
                tracker.validation_count
            );
            return JsonRpcResponse::error(
                id,
                error_codes::META_UTL_INSUFFICIENT_DATA,
                format!(
                    "Insufficient data: need 50 validations, have {}",
                    tracker.validation_count
                ),
            );
        }

        // Check if weights have been computed
        if tracker.last_weight_update.is_none() {
            warn!("meta_utl/optimized_weights: weights not yet computed");
            return JsonRpcResponse::error(
                id,
                error_codes::META_UTL_NOT_INITIALIZED,
                "Weights not computed yet: no weight optimization has occurred",
            );
        }

        // Calculate confidence based on training samples
        let confidence = if tracker.validation_count >= 500 {
            0.95
        } else if tracker.validation_count >= 200 {
            0.85
        } else if tracker.validation_count >= 100 {
            0.75
        } else {
            0.6 + (tracker.validation_count as f32 / 500.0)
        };

        // Format last_updated timestamp
        let last_updated = tracker
            .last_weight_update
            .map(|instant| {
                // Convert Instant to approximate ISO timestamp
                let elapsed = instant.elapsed();
                let now = chrono::Utc::now();
                let updated_time = now - chrono::Duration::from_std(elapsed).unwrap_or_default();
                updated_time.format("%Y-%m-%dT%H:%M:%SZ").to_string()
            })
            .unwrap_or_else(|| "unknown".to_string());

        debug!(
            "meta_utl/optimized_weights: returning weights with {} training samples",
            tracker.validation_count
        );

        JsonRpcResponse::success(
            id,
            json!({
                "weights": tracker.current_weights.to_vec(),
                "confidence": confidence,
                "training_samples": tracker.validation_count,
                "last_updated": last_updated,
            }),
        )
    }
}
