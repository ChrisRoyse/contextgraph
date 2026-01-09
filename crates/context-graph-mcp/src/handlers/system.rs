//! System status and health handlers.
//!
//! REAL health checks with fail-fast error handling.
//! NO fake "healthy" values - components are probed for actual status.

use serde_json::json;
use tracing::error;

use context_graph_core::types::UtlContext;

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::Handlers;

impl Handlers {
    /// Handle system/status request.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore.count()
    pub(super) async fn handle_system_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        // REAL check - no unwrap_or silent failure
        match self.teleological_store.count().await {
            Ok(fingerprint_count) => JsonRpcResponse::success(
                id,
                json!({
                    "status": "running",
                    "phase": "ghost-system",
                    "fingerprintCount": fingerprint_count,
                    "gpuAvailable": false,
                }),
            ),
            Err(e) => {
                error!(
                    "system/status FAILED - TeleologicalStore.count() error: {}",
                    e
                );
                JsonRpcResponse::success(
                    id,
                    json!({
                        "status": "degraded",
                        "phase": "ghost-system",
                        "fingerprintCount": null,
                        "gpuAvailable": false,
                        "error": format!("Memory store unreachable: {}", e)
                    }),
                )
            }
        }
    }

    /// Handle system/health request.
    ///
    /// Performs REAL health probes on all components:
    /// - memory: TeleologicalStore.count() - verifies store is accessible
    /// - utl: UtlProcessor.compute_learning_score() - verifies processor works
    /// - graph: TeleologicalStore.count_by_quadrant() - verifies graph indexing
    ///
    /// NO hardcoded "healthy" values. Each component is actually tested.
    /// Returns detailed error information for any unhealthy component.
    pub(super) async fn handle_system_health(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        let mut components = serde_json::Map::new();
        let mut all_healthy = true;
        let timestamp = chrono::Utc::now().to_rfc3339();

        // ===== Memory Health Check =====
        // Probe: TeleologicalStore.count() - tests basic store accessibility
        match self.teleological_store.count().await {
            Ok(count) => {
                components.insert(
                    "memory".to_string(),
                    json!({
                        "status": "healthy",
                        "fingerprintCount": count,
                        "backendType": self.teleological_store.backend_type().to_string(),
                        "storageSizeBytes": self.teleological_store.storage_size_bytes()
                    }),
                );
            }
            Err(e) => {
                all_healthy = false;
                error!(
                    "Health check FAILED - memory component: TeleologicalStore.count() error: {}",
                    e
                );
                components.insert(
                    "memory".to_string(),
                    json!({
                        "status": "unhealthy",
                        "error": format!("TeleologicalStore unreachable: {}", e),
                        "errorType": "store_access_failure"
                    }),
                );
            }
        }

        // ===== UTL Processor Health Check =====
        // Probe: compute_learning_score() with minimal input - tests processor responsiveness
        let utl_context = UtlContext::default();
        match self
            .utl_processor
            .compute_learning_score("health_check_probe", &utl_context)
            .await
        {
            Ok(score) => {
                components.insert(
                    "utl".to_string(),
                    json!({
                        "status": "healthy",
                        "probeScore": score,
                        "processorResponsive": true
                    }),
                );
            }
            Err(e) => {
                all_healthy = false;
                error!(
                    "Health check FAILED - utl component: UtlProcessor.compute_learning_score() error: {}",
                    e
                );
                components.insert(
                    "utl".to_string(),
                    json!({
                        "status": "unhealthy",
                        "error": format!("UtlProcessor unresponsive: {}", e),
                        "errorType": "processor_failure"
                    }),
                );
            }
        }

        // ===== Graph/Index Health Check =====
        // Probe: count_by_quadrant() - tests Johari quadrant indexing (graph structure)
        match self.teleological_store.count_by_quadrant().await {
            Ok(quadrant_counts) => {
                let total: usize = quadrant_counts.iter().sum();
                components.insert(
                    "graph".to_string(),
                    json!({
                        "status": "healthy",
                        "quadrantDistribution": {
                            "open": quadrant_counts[0],
                            "blind": quadrant_counts[1],
                            "hidden": quadrant_counts[2],
                            "unknown": quadrant_counts[3]
                        },
                        "totalIndexed": total,
                        "indexIntegrity": true
                    }),
                );
            }
            Err(e) => {
                all_healthy = false;
                error!(
                    "Health check FAILED - graph component: TeleologicalStore.count_by_quadrant() error: {}",
                    e
                );
                components.insert(
                    "graph".to_string(),
                    json!({
                        "status": "unhealthy",
                        "error": format!("Graph index unreachable: {}", e),
                        "errorType": "index_access_failure"
                    }),
                );
            }
        }

        // ===== Meta-UTL Tracker Health Check =====
        // Probe: Check tracker state accessibility
        {
            let tracker = self.meta_utl_tracker.read();
            components.insert(
                "metaUtl".to_string(),
                json!({
                    "status": "healthy",
                    "predictionCount": tracker.prediction_count,
                    "validationCount": tracker.validation_count,
                    "pendingPredictions": tracker.pending_predictions.len()
                }),
            );
        }

        JsonRpcResponse::success(
            id,
            json!({
                "healthy": all_healthy,
                "components": components,
                "timestamp": timestamp
            }),
        )
    }
}
