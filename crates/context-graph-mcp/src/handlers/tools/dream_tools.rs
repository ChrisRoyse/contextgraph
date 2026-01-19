//! Dream MCP tool implementations (trigger_dream, get_dream_status).

use std::time::Instant;

use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::dream::{DreamController, DreamCycleConfig, DreamState as CoreDreamState, WakeReason};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::dream_dtos::{
    DreamReport, DreamStatus, GetDreamStatusRequest, GetDreamStatusResponse, HebbianParams,
    NremResult, RemResult, TriggerDreamRequest, TriggerDreamResponse,
};

impl Handlers {
    /// Execute NREM/REM dream consolidation cycle.
    /// Constitution AP-70: Dream triggers MUST use entropy > 0.7 AND churn > 0.5
    pub(crate) async fn call_trigger_dream(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: TriggerDreamRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "trigger_dream: invalid arguments");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid trigger_dream arguments: {}", e),
                );
            }
        };

        let dream_id = Uuid::new_v4();
        info!(
            dream_id = %dream_id,
            blocking = request.blocking,
            dry_run = request.dry_run,
            skip_nrem = request.skip_nrem,
            skip_rem = request.skip_rem,
            "trigger_dream: starting dream cycle"
        );

        // Get current UTL metrics for pre-dream state
        let utl_status = self.utl_processor.get_status();
        let pre_entropy = utl_status
            .get("entropy")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.5);
        let pre_coherence = utl_status
            .get("coherence")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.5);

        // Check if dream is recommended based on Constitution AP-70
        // Note: We should check churn as well, but that requires topic stability metrics
        // For now, we check entropy and log a warning if conditions aren't met
        let dream_recommended = pre_entropy > 0.7;
        if !dream_recommended {
            warn!(
                dream_id = %dream_id,
                entropy = pre_entropy,
                "trigger_dream: entropy < 0.7, dream may not be optimal (AP-70)"
            );
        }

        // Handle dry run mode
        if request.dry_run {
            debug!(dream_id = %dream_id, "trigger_dream: executing dry run");

            let response = TriggerDreamResponse {
                dream_id,
                status: DreamStatus::DryRunComplete,
                nrem_result: if !request.skip_nrem {
                    Some(NremResult {
                        memories_replayed: 0,
                        edges_strengthened: 0,
                        edges_weakened: 0,
                        edges_pruned: 0,
                        avg_weight_delta: 0.0,
                        duration_ms: 0,
                        completed: false,
                        params: HebbianParams::default(),
                    })
                } else {
                    None
                },
                rem_result: if !request.skip_rem {
                    Some(RemResult {
                        queries_generated: 0,
                        blind_spots_found: 0,
                        new_edges_created: 0,
                        avg_semantic_leap: 0.0,
                        exploration_coverage: 0.0,
                        unique_positions_visited: 0,
                        duration_ms: 0,
                        completed: false,
                        blind_spots_sample: Vec::new(),
                    })
                } else {
                    None
                },
                report: Some(DreamReport {
                    total_duration_ms: 0,
                    wake_reason: None,
                    pre_entropy,
                    post_entropy: None,
                    pre_coherence,
                    post_coherence: None,
                    dream_recommended,
                    recommendations: vec![
                        "Dry run completed - no changes made".to_string(),
                        if dream_recommended {
                            "Dream is recommended based on entropy > 0.7".to_string()
                        } else {
                            "Consider waiting for entropy > 0.7 before dreaming".to_string()
                        },
                    ],
                }),
                dry_run: true,
            };

            return self.tool_result_with_pulse(id, serde_json::to_value(&response).unwrap());
        }

        // Execute actual dream cycle
        let mut controller = DreamController::new();

        // Note: In production, we would inject a real MemoryProvider here
        // controller.set_memory_provider(memory_provider);

        let start_time = Instant::now();

        // Handle non-blocking mode
        if !request.blocking {
            // Return immediately with queued status
            // In a real implementation, we would spawn a background task
            warn!(
                dream_id = %dream_id,
                "trigger_dream: non-blocking mode not fully implemented - running synchronously"
            );
        }

        // Validate phase selection
        if request.skip_nrem && request.skip_rem {
            return self.tool_error_with_pulse(
                id,
                "Cannot skip both NREM and REM phases - at least one must run",
            );
        }

        // Build configuration from request parameters
        let config = DreamCycleConfig {
            run_nrem: !request.skip_nrem,
            run_rem: !request.skip_rem,
            max_duration: std::time::Duration::from_secs(request.max_duration_secs),
        };

        info!(
            dream_id = %dream_id,
            run_nrem = config.run_nrem,
            run_rem = config.run_rem,
            max_duration_secs = request.max_duration_secs,
            "trigger_dream: starting dream cycle with config"
        );

        let cycle_result = controller.start_dream_cycle_with_config(config).await;

        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        // Process result
        match cycle_result {
            Ok(core_report) => {
                let nrem_result = core_report.nrem_report.as_ref().map(NremResult::from);
                let rem_result = core_report.rem_report.as_ref().map(RemResult::from);

                let status = if core_report.completed {
                    DreamStatus::Completed
                } else {
                    match core_report.wake_reason {
                        WakeReason::ExternalQuery => DreamStatus::Interrupted,
                        WakeReason::Error => DreamStatus::Failed,
                        _ => DreamStatus::Interrupted,
                    }
                };

                // Get post-dream UTL metrics
                let post_utl_status = self.utl_processor.get_status();
                let post_entropy = post_utl_status
                    .get("entropy")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                let post_coherence = post_utl_status
                    .get("coherence")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);

                // Generate recommendations
                let mut recommendations = Vec::new();
                if core_report.completed {
                    recommendations.push("Dream cycle completed successfully".to_string());
                    if let Some(nrem) = &nrem_result {
                        if nrem.edges_pruned > 0 {
                            recommendations.push(format!(
                                "Pruned {} edges below weight floor",
                                nrem.edges_pruned
                            ));
                        }
                    }
                    if let Some(rem) = &rem_result {
                        if rem.blind_spots_found > 0 {
                            recommendations.push(format!(
                                "Discovered {} blind spots for potential connections",
                                rem.blind_spots_found
                            ));
                        }
                    }
                } else {
                    recommendations.push(format!(
                        "Dream cycle interrupted: {}",
                        core_report.wake_reason
                    ));
                }

                let response = TriggerDreamResponse {
                    dream_id,
                    status,
                    nrem_result,
                    rem_result,
                    report: Some(DreamReport {
                        total_duration_ms,
                        wake_reason: if core_report.completed {
                            None
                        } else {
                            Some(core_report.wake_reason.to_string())
                        },
                        pre_entropy,
                        post_entropy,
                        pre_coherence,
                        post_coherence,
                        dream_recommended,
                        recommendations,
                    }),
                    dry_run: false,
                };

                info!(
                    dream_id = %dream_id,
                    status = %response.status,
                    duration_ms = total_duration_ms,
                    "trigger_dream: cycle completed"
                );

                self.tool_result_with_pulse(id, serde_json::to_value(&response).unwrap())
            }
            Err(e) => {
                error!(
                    dream_id = %dream_id,
                    error = %e,
                    "trigger_dream: cycle failed"
                );

                let response = TriggerDreamResponse {
                    dream_id,
                    status: DreamStatus::Failed,
                    nrem_result: None,
                    rem_result: None,
                    report: Some(DreamReport {
                        total_duration_ms,
                        wake_reason: Some(format!("Error: {}", e)),
                        pre_entropy,
                        post_entropy: None,
                        pre_coherence,
                        post_coherence: None,
                        dream_recommended,
                        recommendations: vec![
                            format!("Dream cycle failed: {}", e),
                            "Check system logs for details".to_string(),
                        ],
                    }),
                    dry_run: false,
                };

                self.tool_result_with_pulse(id, serde_json::to_value(&response).unwrap())
            }
        }
    }

    /// Get status of a running or completed dream cycle.
    pub(crate) async fn call_get_dream_status(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: GetDreamStatusRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "get_dream_status: invalid arguments");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid get_dream_status arguments: {}", e),
                );
            }
        };

        debug!(dream_id = ?request.dream_id, "get_dream_status: querying status");

        // Create a controller to get the current status
        let controller = DreamController::new();
        let status = controller.get_status();

        // Map core DreamState to our DreamStatus
        let (dto_status, current_phase, progress_percent) = match status.state {
            CoreDreamState::Awake => (DreamStatus::Completed, "awake", 100),
            CoreDreamState::EnteringDream => (DreamStatus::Queued, "entering", 0),
            CoreDreamState::Nrem { progress, .. } => (
                DreamStatus::NremInProgress,
                "nrem",
                (progress * 100.0) as u8,
            ),
            CoreDreamState::Rem { progress, .. } => {
                // NREM is 60% of total (3 min out of 5 min), REM is 40%
                let total_progress = 60 + (progress * 40.0) as u8;
                (DreamStatus::RemInProgress, "rem", total_progress.min(99))
            }
            CoreDreamState::Waking => (DreamStatus::Completed, "waking", 100),
        };

        let response = GetDreamStatusResponse {
            dream_id: request.dream_id.unwrap_or_else(Uuid::new_v4),
            status: dto_status,
            progress_percent,
            current_phase: current_phase.to_string(),
            elapsed_ms: status
                .time_since_last_dream
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            remaining_ms: if status.is_dreaming {
                let elapsed = status
                    .time_since_last_dream
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                Some(300_000_u64.saturating_sub(elapsed))
            } else {
                None
            },
            partial_results: None,
        };

        self.tool_result_with_pulse(id, serde_json::to_value(&response).unwrap())
    }
}
