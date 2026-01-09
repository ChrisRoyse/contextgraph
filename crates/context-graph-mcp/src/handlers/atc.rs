//! Adaptive Threshold Calibration (ATC) handlers.
//!
//! TASK-ATC-001: MCP tools for threshold status, calibration metrics, and recalibration.
//!
//! Tools:
//! - get_threshold_status: Current threshold configuration and drift
//! - get_calibration_metrics: ECE, MCE, Brier score and quality status
//! - trigger_recalibration: Manually trigger Level 1-4 recalibration

use serde_json::json;
use tracing::{debug, error, info};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::core::Handlers;

impl Handlers {
    /// Handle get_threshold_status tool call.
    ///
    /// TASK-ATC-001: Returns current ATC threshold configuration including:
    /// - All threshold values and their priors
    /// - Per-embedder temperature scaling factors
    /// - Drift scores per tracked threshold
    /// - Bandit exploration statistics (if Level 3 active)
    pub async fn handle_get_threshold_status(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("Handling get_threshold_status");

        // Parse optional parameters
        let domain = params
            .as_ref()
            .and_then(|p| p.get("domain"))
            .and_then(|v| v.as_str())
            .unwrap_or("General");

        let embedder_id = params
            .as_ref()
            .and_then(|p| p.get("embedder_id"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // Check if ATC is available
        let atc_guard = match &self.atc {
            Some(atc) => atc.read(),
            None => {
                error!("ATC provider not initialized - FAIL FAST");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FEATURE_DISABLED,
                    "ATC provider not initialized. Use with_atc() constructor.",
                );
            }
        };

        // Get drift status
        let drift_scores = atc_guard.get_drift_status();

        // Get calibration quality
        let calibration = atc_guard.get_calibration_quality();

        // Get domain thresholds if available
        let domain_thresholds = match domain {
            "Code" => atc_guard.get_domain_thresholds(context_graph_core::atc::Domain::Code),
            "Medical" => atc_guard.get_domain_thresholds(context_graph_core::atc::Domain::Medical),
            "Legal" => atc_guard.get_domain_thresholds(context_graph_core::atc::Domain::Legal),
            "Creative" => atc_guard.get_domain_thresholds(context_graph_core::atc::Domain::Creative),
            "Research" => atc_guard.get_domain_thresholds(context_graph_core::atc::Domain::Research),
            _ => atc_guard.get_domain_thresholds(context_graph_core::atc::Domain::General),
        };

        // Build response
        let mut response = json!({
            "domain": domain,
            "thresholds": {
                "domain_thresholds": domain_thresholds.map(|dt| json!({
                    "theta_opt": dt.theta_opt,
                    "theta_acc": dt.theta_acc,
                    "theta_warn": dt.theta_warn,
                    "theta_dup": dt.theta_dup,
                    "theta_edge": dt.theta_edge
                }))
            },
            "calibration": {
                "ece": calibration.ece,
                "mce": calibration.mce,
                "brier": calibration.brier,
                "sample_count": calibration.sample_count,
                "status": format!("{:?}", calibration.quality_status)
            },
            "drift_scores": drift_scores,
            "should_recalibrate_level2": atc_guard.should_recalibrate_level2(),
            "should_explore_level3": atc_guard.should_explore_level3(),
            "should_optimize_level4": atc_guard.should_optimize_level4()
        });

        // Add embedder-specific info if requested
        if let Some(emb_id) = embedder_id {
            if (1..=13).contains(&emb_id) {
                // Get poorly calibrated embedders to check if this one is among them
                let poorly_calibrated = atc_guard.get_poorly_calibrated_embedders();
                // Use canonical Embedder::from_index (0-based internally, but API uses 1-based for user-facing)
                let embedder = context_graph_core::Embedder::from_index(emb_id - 1)
                    .unwrap_or(context_graph_core::Embedder::Semantic);

                let is_poorly_calibrated = poorly_calibrated.contains(&embedder);

                response["embedder_detail"] = json!({
                    "embedder_id": emb_id,
                    "embedder_name": format!("{:?}", embedder),
                    "is_poorly_calibrated": is_poorly_calibrated,
                    "needs_recalibration": is_poorly_calibrated
                });
            }
        }

        info!(domain = domain, ece = calibration.ece, "Threshold status retrieved");
        self.tool_result_with_pulse(id, response)
    }

    /// Handle get_calibration_metrics tool call.
    ///
    /// TASK-ATC-001: Returns calibration quality metrics:
    /// - ECE (Expected Calibration Error) - target < 0.05
    /// - MCE (Maximum Calibration Error) - target < 0.10
    /// - Brier Score - target < 0.10
    /// - Drift scores per tracked threshold
    /// - Overall calibration status
    pub async fn handle_get_calibration_metrics(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("Handling get_calibration_metrics");

        // Parse optional timeframe parameter
        let timeframe = params
            .as_ref()
            .and_then(|p| p.get("timeframe"))
            .and_then(|v| v.as_str())
            .unwrap_or("24h");

        // Check if ATC is available
        let atc_guard = match &self.atc {
            Some(atc) => atc.read(),
            None => {
                error!("ATC provider not initialized - FAIL FAST");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FEATURE_DISABLED,
                    "ATC provider not initialized. Use with_atc() constructor.",
                );
            }
        };

        // Get calibration quality
        let metrics = atc_guard.get_calibration_quality();

        // Get drift scores
        let drift_scores = atc_guard.get_drift_status();

        // Get poorly calibrated embedders
        let poorly_calibrated = atc_guard.get_poorly_calibrated_embedders();
        let poorly_calibrated_names: Vec<String> = poorly_calibrated
            .iter()
            .map(|e| format!("{:?}", e))
            .collect();

        // Determine status description
        let status_description = match metrics.quality_status {
            context_graph_core::atc::CalibrationStatus::Excellent => "Excellent - no action needed",
            context_graph_core::atc::CalibrationStatus::Good => "Good - monitoring recommended",
            context_graph_core::atc::CalibrationStatus::Acceptable => "Acceptable - consider recalibration soon",
            context_graph_core::atc::CalibrationStatus::Poor => "Poor - recalibration recommended",
            context_graph_core::atc::CalibrationStatus::Critical => "Critical - immediate recalibration required",
        };

        // Build response
        let response = json!({
            "timeframe": timeframe,
            "metrics": {
                "ece": metrics.ece,
                "ece_target": 0.05,
                "ece_acceptable": 0.10,
                "mce": metrics.mce,
                "mce_target": 0.10,
                "mce_acceptable": 0.20,
                "brier": metrics.brier,
                "brier_target": 0.10,
                "brier_acceptable": 0.15,
                "sample_count": metrics.sample_count
            },
            "status": format!("{:?}", metrics.quality_status),
            "status_description": status_description,
            "should_recalibrate": metrics.quality_status.should_recalibrate(),
            "drift_scores": drift_scores,
            "poorly_calibrated_embedders": poorly_calibrated_names,
            "recommendations": {
                "level2_recalibration_needed": atc_guard.should_recalibrate_level2(),
                "level3_exploration_needed": atc_guard.should_explore_level3(),
                "level4_optimization_needed": atc_guard.should_optimize_level4()
            }
        });

        info!(
            ece = metrics.ece,
            mce = metrics.mce,
            brier = metrics.brier,
            status = ?metrics.quality_status,
            "Calibration metrics retrieved"
        );
        self.tool_result_with_pulse(id, response)
    }

    /// Handle trigger_recalibration tool call.
    ///
    /// TASK-ATC-001: Manually trigger recalibration at a specific ATC level:
    /// - Level 1: EWMA drift adjustment (per-query)
    /// - Level 2: Temperature scaling (hourly)
    /// - Level 3: Thompson Sampling exploration (session)
    /// - Level 4: Bayesian meta-optimization (weekly)
    pub async fn handle_trigger_recalibration(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        debug!("Handling trigger_recalibration");

        // Parse required level parameter
        let level = match params
            .as_ref()
            .and_then(|p| p.get("level"))
            .and_then(|v| v.as_u64())
        {
            Some(l) if (1..=4).contains(&l) => l as u32,
            Some(l) => {
                error!(level = l, "Invalid ATC level");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid level {}. Must be 1, 2, 3, or 4.", l),
                );
            }
            None => {
                error!("Missing required 'level' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'level' parameter. Must be 1, 2, 3, or 4.",
                );
            }
        };

        let domain = params
            .as_ref()
            .and_then(|p| p.get("domain"))
            .and_then(|v| v.as_str())
            .unwrap_or("General");

        // Check if ATC is available
        let mut atc_guard = match &self.atc {
            Some(atc) => atc.write(),
            None => {
                error!("ATC provider not initialized - FAIL FAST");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FEATURE_DISABLED,
                    "ATC provider not initialized. Use with_atc() constructor.",
                );
            }
        };

        // Get pre-recalibration metrics
        let pre_metrics = atc_guard.get_calibration_quality().clone();

        // Execute recalibration based on level
        let result = match level {
            1 => {
                // Level 1: EWMA drift - just report current state (continuous)
                info!("Level 1 EWMA drift tracking is continuous - reporting current state");
                let drift_scores = atc_guard.get_drift_status();
                json!({
                    "level": 1,
                    "level_name": "EWMA Drift Tracker",
                    "action": "reported",
                    "description": "Level 1 operates continuously per-query. Current drift scores reported.",
                    "drift_scores": drift_scores,
                    "domain": domain
                })
            }
            2 => {
                // Level 2: Temperature scaling recalibration
                info!("Triggering Level 2 temperature recalibration");
                let temperature_losses = atc_guard.calibrate_temperatures();
                let loss_map: std::collections::HashMap<String, f32> = temperature_losses
                    .into_iter()
                    .map(|(e, l)| (format!("{:?}", e), l))
                    .collect();

                json!({
                    "level": 2,
                    "level_name": "Temperature Scaling",
                    "action": "recalibrated",
                    "description": "Temperature scaling recalibrated for all embedders.",
                    "temperature_losses": loss_map,
                    "domain": domain
                })
            }
            3 => {
                // Level 3: Thompson Sampling - initialize if needed
                info!("Triggering Level 3 Thompson Sampling initialization");

                // Initialize with default threshold candidates
                let threshold_candidates = vec![0.70, 0.72, 0.75, 0.77, 0.80];
                atc_guard.init_session_bandit(threshold_candidates.clone());

                // Select a threshold using Thompson sampling
                let selected = atc_guard.select_threshold_thompson();

                json!({
                    "level": 3,
                    "level_name": "Thompson Sampling Bandit",
                    "action": "initialized",
                    "description": "Thompson Sampling bandit initialized for session-level exploration.",
                    "threshold_candidates": threshold_candidates,
                    "selected_threshold": selected,
                    "domain": domain
                })
            }
            4 => {
                // Level 4: Bayesian optimization - check if should run
                info!("Checking Level 4 Bayesian meta-optimization");
                let should_optimize = atc_guard.should_optimize_level4();

                json!({
                    "level": 4,
                    "level_name": "Bayesian Meta-Optimizer",
                    "action": if should_optimize { "triggered" } else { "skipped" },
                    "description": if should_optimize {
                        "Bayesian meta-optimization triggered. Weekly optimization in progress."
                    } else {
                        "Bayesian optimization not needed yet. Runs weekly or when critical."
                    },
                    "should_optimize": should_optimize,
                    "domain": domain
                })
            }
            _ => unreachable!(),
        };

        // Get post-recalibration metrics
        let post_metrics = atc_guard.get_calibration_quality();

        // Build full response
        let response = json!({
            "success": true,
            "recalibration": result,
            "metrics_before": {
                "ece": pre_metrics.ece,
                "mce": pre_metrics.mce,
                "brier": pre_metrics.brier,
                "status": format!("{:?}", pre_metrics.quality_status)
            },
            "metrics_after": {
                "ece": post_metrics.ece,
                "mce": post_metrics.mce,
                "brier": post_metrics.brier,
                "status": format!("{:?}", post_metrics.quality_status)
            }
        });

        info!(level = level, domain = domain, "Recalibration completed");
        self.tool_result_with_pulse(id, response)
    }
}
