//! Live system monitoring for production MCP server.
//!
//! Replaces `HardcodedActiveLayerStatusProvider` (which always returns
//! "Active", formerly named `StubLayerStatusProvider` until the M-M1 rename
//! on 2026-05-19) with real component health checks. No fake data, no
//! fallbacks — honest reporting only.

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::error;

use context_graph_core::monitoring::{
    LayerInfo, LayerStatus, LayerStatusProvider, MonitorResult, SystemMonitorError,
};
use context_graph_core::traits::TeleologicalMemoryStore;

/// Live layer status provider that checks actual system component state.
///
/// - L1_Sensing: Checks if embedding models are loaded via `models_loading`/`models_failed` flags
/// - L3_Memory: Checks if `TeleologicalMemoryStore` is accessible via `count()`
/// - L4_Learning: Reports `NotImplemented` — no active learning loop exists
/// - L5_Coherence: Reports `NotImplemented` — no active topic synthesis loop exists
pub struct LiveLayerStatusProvider {
    models_loading: Arc<AtomicBool>,
    models_failed: Arc<RwLock<Option<String>>>,
    teleological_store: Arc<dyn TeleologicalMemoryStore>,
}

impl LiveLayerStatusProvider {
    pub fn new(
        models_loading: Arc<AtomicBool>,
        models_failed: Arc<RwLock<Option<String>>>,
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
    ) -> Self {
        Self {
            models_loading,
            models_failed,
            teleological_store,
        }
    }
}

#[async_trait]
impl LayerStatusProvider for LiveLayerStatusProvider {
    async fn perception_status(&self) -> MonitorResult<LayerStatus> {
        if self.models_loading.load(Ordering::SeqCst) {
            return Ok(LayerStatus::Error(
                "Embedding models still loading".to_string(),
            ));
        }
        let failed = self.models_failed.read().await;
        if let Some(ref error_msg) = *failed {
            return Ok(LayerStatus::Error(format!(
                "Model loading failed: {}",
                error_msg
            )));
        }
        Ok(LayerStatus::Active)
    }

    async fn memory_status(&self) -> MonitorResult<LayerStatus> {
        match self.teleological_store.count().await {
            Ok(_) => Ok(LayerStatus::Active),
            Err(e) => {
                error!(error = %e, "LiveLayerStatusProvider: memory store health check FAILED");
                Ok(LayerStatus::Error(format!("Store check failed: {}", e)))
            }
        }
    }

    async fn action_status(&self) -> MonitorResult<LayerStatus> {
        Ok(LayerStatus::NotImplemented)
    }

    async fn meta_status(&self) -> MonitorResult<LayerStatus> {
        Ok(LayerStatus::NotImplemented)
    }

    async fn all_layer_info(&self) -> MonitorResult<Vec<LayerInfo>> {
        let perception = self.perception_status().await?;
        let memory = self.memory_status().await?;
        let action = self.action_status().await?;
        let meta = self.meta_status().await?;

        Ok(vec![
            LayerInfo {
                name: "L1_Sensing".to_string(),
                status: perception.clone(),
                last_latency_us: None,
                error_count: None,
                health_check_passed: Some(perception.is_active()),
            },
            LayerInfo {
                name: "L3_Memory".to_string(),
                status: memory.clone(),
                last_latency_us: None,
                error_count: None,
                health_check_passed: Some(memory.is_active()),
            },
            LayerInfo {
                name: "L4_Learning".to_string(),
                status: action.clone(),
                last_latency_us: None,
                error_count: None,
                health_check_passed: Some(false),
            },
            LayerInfo {
                name: "L5_Coherence".to_string(),
                status: meta.clone(),
                last_latency_us: None,
                error_count: None,
                health_check_passed: Some(false),
            },
        ])
    }

    async fn layer_status_by_name(&self, layer_name: &str) -> MonitorResult<LayerStatus> {
        match layer_name.to_lowercase().as_str() {
            "perception" | "l1_sensing" | "l1" => self.perception_status().await,
            "memory" | "l3_memory" | "l3" => self.memory_status().await,
            "action" | "l4_learning" | "l4" => self.action_status().await,
            "meta" | "l5_coherence" | "l5" => self.meta_status().await,
            _ => Err(SystemMonitorError::LayerError {
                layer: layer_name.to_string(),
                message: format!(
                    "Unknown layer: '{}'. Valid: L1_Sensing, L3_Memory, L4_Learning, L5_Coherence",
                    layer_name
                ),
            }),
        }
    }
}
