//! Status query tool implementations (get_memetic_status).

use serde_json::json;
use tracing::error;

use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::helpers::ToolErrorKind;

impl Handlers {
    /// get_memetic_status tool implementation.
    ///
    /// Returns system status including:
    /// - Fingerprint count from TeleologicalMemoryStore
    /// - Number of embedders (14)
    /// - Storage backend and size
    /// - Layer status from LayerStatusProvider
    pub(crate) async fn call_get_memetic_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        let fingerprint_count = match self.teleological_store.count().await {
            Ok(count) => count,
            Err(e) => {
                error!(error = %e, "get_memetic_status: TeleologicalStore.count() FAILED");
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Storage,
                    &format!("Failed to get fingerprint count: {}", e),
                );
            }
        };

        // Get REAL layer statuses from LayerStatusProvider — fail fast on errors
        let perception_status = match self.layer_status_provider.perception_status().await {
            Ok(s) => s.as_str().to_string(),
            Err(e) => {
                error!(error = %e, layer = "perception", "get_memetic_status: layer status FAILED");
                return self.tool_error_typed(id, ToolErrorKind::Execution, &format!("Perception layer status failed: {}", e));
            }
        };
        let memory_status = match self.layer_status_provider.memory_status().await {
            Ok(s) => s.as_str().to_string(),
            Err(e) => {
                error!(error = %e, layer = "memory", "get_memetic_status: layer status FAILED");
                return self.tool_error_typed(id, ToolErrorKind::Execution, &format!("Memory layer status failed: {}", e));
            }
        };
        let action_status = match self.layer_status_provider.action_status().await {
            Ok(s) => s.as_str().to_string(),
            Err(e) => {
                error!(error = %e, layer = "action", "get_memetic_status: layer status FAILED");
                return self.tool_error_typed(id, ToolErrorKind::Execution, &format!("Action layer status failed: {}", e));
            }
        };
        let meta_status = match self.layer_status_provider.meta_status().await {
            Ok(s) => s.as_str().to_string(),
            Err(e) => {
                error!(error = %e, layer = "meta", "get_memetic_status: layer status FAILED");
                return self.tool_error_typed(id, ToolErrorKind::Execution, &format!("Meta layer status failed: {}", e));
            }
        };

        // E5 causal model health: report whether LoRA trained weights are loaded.
        // Without trained weights, the causal gate is non-functional.
        #[cfg(feature = "llm")]
        let e5_lora_loaded = self
            .causal_model
            .as_ref()
            .map(|m| m.has_trained_weights())
            .unwrap_or(false);
        #[cfg(not(feature = "llm"))]
        let e5_lora_loaded = false;

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
