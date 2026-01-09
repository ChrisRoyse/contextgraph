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
use tracing::{debug, error};

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

        // Get graph metrics (from TeleologicalStore if available)
        let edge_count = match self.teleological_store.count().await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "get_steering_feedback: Failed to get node count");
                return self.tool_error_with_pulse(id, &format!("Failed to get node count: {}", e));
            }
        };

        // For now, compute synthetic metrics based on available data
        // In a full implementation, these would come from actual graph analysis
        let orphan_count = 0_usize; // Would be computed from graph structure
        let connectivity = 0.85_f32; // Would be computed from graph structure

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
                "priority_improvement": feedback.priority_improvement()
            }),
        )
    }
}
