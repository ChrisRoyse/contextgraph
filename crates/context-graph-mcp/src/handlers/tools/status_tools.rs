//! Status query tool implementations (get_memetic_status).

use serde_json::json;
use tracing::error;

use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

impl Handlers {
    /// get_memetic_status tool implementation.
    ///
    /// Returns system status including:
    /// - Fingerprint count from TeleologicalMemoryStore
    /// - Number of embedders (13)
    /// - Storage backend and size
    /// - Layer status from LayerStatusProvider
    pub(crate) async fn call_get_memetic_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        let fingerprint_count = match self.teleological_store.count().await {
            Ok(count) => count,
            Err(e) => {
                error!(error = %e, "get_memetic_status: TeleologicalStore.count() FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to get fingerprint count: {}", e),
                );
            }
        };

        // Get REAL layer statuses from LayerStatusProvider
        let perception_status = self
            .layer_status_provider
            .perception_status()
            .await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: perception_status FAILED");
                "error".to_string()
            });
        let memory_status = self
            .layer_status_provider
            .memory_status()
            .await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: memory_status FAILED");
                "error".to_string()
            });
        let action_status = self
            .layer_status_provider
            .action_status()
            .await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: action_status FAILED");
                "error".to_string()
            });
        let meta_status = self
            .layer_status_provider
            .meta_status()
            .await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: meta_status FAILED");
                "error".to_string()
            });

        // E5 causal model health: report whether LoRA trained weights are loaded.
        // Without trained weights, the causal gate is non-functional.
        let e5_lora_loaded = self
            .causal_model
            .as_ref()
            .map(|m| m.has_trained_weights())
            .unwrap_or(false);

        self.tool_result(
            id,
            json!({
                "fingerprintCount": fingerprint_count,
                "embedderCount": NUM_EMBEDDERS,
                "storageBackend": self.teleological_store.backend_type().to_string(),
                "storageSizeBytes": self.teleological_store.storage_size_bytes(),
                "layers": {
                    "perception": perception_status,
                    "memory": memory_status,
                    "action": action_status,
                    "meta": meta_status
                },
                "e5CausalModel": {
                    "loraLoaded": e5_lora_loaded,
                    "causalGateFunctional": e5_lora_loaded
                }
            }),
        )
    }
}
