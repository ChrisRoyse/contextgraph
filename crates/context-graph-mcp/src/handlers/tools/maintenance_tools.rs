//! Maintenance tool handlers for data repair and cleanup.

use serde_json::json;
use tracing::{debug, error, info};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

impl Handlers {
    /// Handle repair_causal_relationships tool call.
    ///
    /// Scans CF_CAUSAL_RELATIONSHIPS and removes entries that fail deserialization.
    pub(crate) async fn call_repair_causal_relationships(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling repair_causal_relationships tool call");

        let store_any = self.teleological_store.as_any();
        let Some(rocksdb_store) = store_any.downcast_ref::<context_graph_storage::teleological::RocksDbTeleologicalStore>() else {
            error!("Store does not support repair operation");
            return self.tool_error(id, "Store does not support repair. Only RocksDbTeleologicalStore supports this operation.");
        };

        match rocksdb_store.repair_corrupted_causal_relationships().await {
            Ok((deleted_count, total_scanned)) => {
                info!(deleted = deleted_count, scanned = total_scanned, "Repair complete");
                self.tool_result(
                    id,
                    json!({
                        "status": "success",
                        "deleted_count": deleted_count,
                        "total_scanned": total_scanned,
                        "message": format!(
                            "Repaired {} corrupted entries out of {} total scanned",
                            deleted_count, total_scanned
                        )
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "Repair failed");
                self.tool_error(id, &format!("Repair failed: {}", e))
            }
        }
    }
}
