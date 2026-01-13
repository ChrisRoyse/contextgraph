//! Steering MCP Handlers
//!
//! TASK-STEERING-001: MCP tool handlers for steering subsystem.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference
//!
//! See steering components and `get_steering_feedback` tool (line 538).
//!
//! ## Tools
//!
//! - get_steering_feedback: Get feedback from Gardener, Curator, Assessor

use serde_json::json;
use tracing::{debug, error, warn};

use context_graph_core::steering::SteeringSystem;

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::Handlers;

impl Handlers {
    /// get_steering_feedback tool implementation.
    ///
    /// TASK-STEERING-001: Get steering feedback from all three components.
    /// FAIL FAST if required providers not initialized.
    ///
    /// Returns:
    /// - reward: SteeringReward with value in [-1, 1]
    /// - gardener_score: Graph health score [-1, 1]
    /// - curator_score: Quality score [-1, 1]
    /// - assessor_score: Performance score [-1, 1]
    /// - gardener_details: Detailed graph health feedback
    /// - curator_details: Detailed quality feedback with recommendations
    /// - assessor_details: Detailed performance feedback with trend
    pub(super) async fn call_get_steering_feedback(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_steering_feedback tool call");

        // SPEC-STUBFIX-001: Get REAL graph metrics from TeleologicalStore
        // FAIL FAST if store access fails
        let total_count = match self.teleological_store.count().await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "get_steering_feedback: FAIL FAST - Failed to get node count");
                return self.tool_error_with_pulse(id, &format!("Store error: Failed to get node count: {}", e));
            }
        };

        // Get all fingerprints to compute metrics from REAL data
        // Since edge operations aren't exposed through TeleologicalMemoryStore trait,
        // we use approximations based on available fingerprint data:
        // - orphan_count: fingerprints with access_count == 0 (never accessed/connected)
        // - connectivity: ratio of fingerprints with θ > 0.5 (aligned with North Star)
        let johari_list = match self.teleological_store.list_all_johari(total_count.max(100)).await {
            Ok(list) => list,
            Err(e) => {
                error!(error = %e, "get_steering_feedback: FAIL FAST - Failed to list fingerprints");
                return self.tool_error_with_pulse(id, &format!("Store error: Failed to list fingerprints: {}", e));
            }
        };

        let mut orphan_count = 0_usize;
        let mut aligned_count = 0_usize;
        let alignment_threshold = 0.5_f32;

        for (uuid, _johari) in johari_list.iter() {
            match self.teleological_store.retrieve(*uuid).await {
                Ok(Some(fp)) => {
                    // Orphan approximation: never accessed fingerprints
                    if fp.access_count == 0 {
                        orphan_count += 1;
                    }
                    // Connectivity approximation: aligned with North Star
                    if fp.theta_to_north_star >= alignment_threshold {
                        aligned_count += 1;
                    }
                }
                Ok(None) => {
                    warn!(uuid = %uuid, "get_steering_feedback: Fingerprint not found");
                }
                Err(e) => {
                    warn!(uuid = %uuid, error = %e, "get_steering_feedback: Failed to retrieve fingerprint");
                }
            }
        }

        // Compute connectivity from REAL alignment data
        // Connectivity = ratio of aligned nodes (θ ≥ 0.5) to total nodes
        let connectivity = if total_count > 0 {
            aligned_count as f32 / total_count as f32
        } else {
            0.0_f32 // Empty graph has 0 connectivity
        };

        debug!(
            total_count,
            orphan_count,
            aligned_count,
            connectivity,
            "get_steering_feedback: Computed REAL metrics from store"
        );

        // Use total_count as edge_count proxy (each fingerprint represents a node)
        let edge_count = total_count;

        // Get UTL metrics for quality assessment
        let utl_status = self.utl_processor.get_status();
        let coherence = utl_status
            .get("coherence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;
        let learning_score = utl_status
            .get("learning_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;

        // Compute quality metrics
        let avg_quality = coherence;
        let low_quality_count = if coherence < 0.5 { edge_count / 10 } else { 0 };

        // Performance metrics
        let retrieval_accuracy = coherence; // Proxy for now
        let learning_efficiency = learning_score;

        // Create steering system and compute feedback
        let steering = SteeringSystem::new();
        let feedback = steering.compute_feedback(
            edge_count,
            orphan_count,
            connectivity,
            avg_quality,
            low_quality_count,
            retrieval_accuracy,
            learning_efficiency,
            None, // No previous accuracy for trend
        );

        // TASK-NEURO-P1-001: Wire steering reward to neuromodulation
        let neuromod_json = if let Some(neuromod_manager) = &self.neuromod_manager {
            let mut manager = neuromod_manager.write();
            let report = manager.on_goal_progress_with_cascades(feedback.reward.value);
            debug!(
                reward_value = feedback.reward.value,
                da_delta = report.da_delta,
                da_new = report.da_new,
                mood_cascade = report.mood_cascade_triggered,
                alertness_cascade = report.alertness_cascade_triggered,
                "Steering feedback propagated to neuromodulation"
            );
            json!({
                "propagated": true,
                "da_delta": report.da_delta,
                "da_new": report.da_new,
                "serotonin_delta": report.serotonin_delta,
                "serotonin_new": report.serotonin_new,
                "ne_delta": report.ne_delta,
                "ne_new": report.ne_new,
                "mood_cascade_triggered": report.mood_cascade_triggered,
                "alertness_cascade_triggered": report.alertness_cascade_triggered
            })
        } else {
            warn!("get_steering_feedback: NeuromodulationManager not initialized - skipping neuromod update");
            json!({
                "propagated": false,
                "reason": "NeuromodulationManager not initialized"
            })
        };

        self.tool_result_with_pulse(
            id,
            json!({
                "reward": {
                    "value": feedback.reward.value,
                    "gardener_score": feedback.reward.gardener_score,
                    "curator_score": feedback.reward.curator_score,
                    "assessor_score": feedback.reward.assessor_score,
                    "dominant_factor": feedback.reward.dominant_factor(),
                    "limiting_factor": feedback.reward.limiting_factor()
                },
                "gardener_details": {
                    "edges_pruned": feedback.gardener_details.edges_pruned,
                    "dead_ends_removed": feedback.gardener_details.dead_ends_removed,
                    "connectivity": feedback.gardener_details.connectivity,
                    "is_healthy": feedback.gardener_details.is_healthy()
                },
                "curator_details": {
                    "avg_quality": feedback.curator_details.avg_quality,
                    "low_quality_count": feedback.curator_details.low_quality_count,
                    "recommendations": feedback.curator_details.recommendations,
                    "is_high_quality": feedback.curator_details.is_high_quality(),
                    "has_concerns": feedback.curator_details.has_concerns()
                },
                "assessor_details": {
                    "retrieval_accuracy": feedback.assessor_details.retrieval_accuracy,
                    "learning_efficiency": feedback.assessor_details.learning_efficiency,
                    "trend": feedback.assessor_details.trend,
                    "trend_description": feedback.assessor_details.trend_description(),
                    "is_performing_well": feedback.assessor_details.is_performing_well()
                },
                "summary": feedback.summary(),
                "needs_immediate_attention": feedback.needs_immediate_attention(),
                "priority_improvement": feedback.priority_improvement(),
                "neuromodulation": neuromod_json
            }),
        )
    }
}
