//! Status query tool implementations (get_memetic_status).

use serde_json::json;
use tracing::error;

use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_core::weights::{
    active_embedder_count, disabled_embedder_names, E11_ENTITY_ENABLED, E5_CAUSAL_ENABLED,
};

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::helpers::ToolErrorKind;

impl Handlers {
    /// get_memetic_status tool implementation.
    ///
    /// Returns system status including:
    /// - Fingerprint count from TeleologicalMemoryStore
    /// - Active embedder count and storage slot count
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
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Perception layer status failed: {}", e),
                );
            }
        };
        let memory_status = match self.layer_status_provider.memory_status().await {
            Ok(s) => s.as_str().to_string(),
            Err(e) => {
                error!(error = %e, layer = "memory", "get_memetic_status: layer status FAILED");
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Memory layer status failed: {}", e),
                );
            }
        };
        let action_status = match self.layer_status_provider.action_status().await {
            Ok(s) => s.as_str().to_string(),
            Err(e) => {
                error!(error = %e, layer = "action", "get_memetic_status: layer status FAILED");
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Action layer status failed: {}", e),
                );
            }
        };
        let meta_status = match self.layer_status_provider.meta_status().await {
            Ok(s) => s.as_str().to_string(),
            Err(e) => {
                error!(error = %e, layer = "meta", "get_memetic_status: layer status FAILED");
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Meta layer status failed: {}", e),
                );
            }
        };

        let e5_lora_loaded = false;

        self.tool_result(
            id,
            json!({
                "fingerprintCount": fingerprint_count,
                "embedderCount": active_embedder_count(),
                "activeEmbedderCount": active_embedder_count(),
                "storageSlotCount": NUM_EMBEDDERS,
                "disabledEmbedders": disabled_embedder_names(),
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
                    "causalGateFunctional": e5_lora_loaded,
                    "enabled": E5_CAUSAL_ENABLED,
                    "status": "retired_disabled"
                },
                "e11EntityModel": {
                    "enabled": E11_ENTITY_ENABLED,
                    "status": "disabled_until_verified_code_symbol_entity_embedder"
                }
            }),
        )
    }
}
