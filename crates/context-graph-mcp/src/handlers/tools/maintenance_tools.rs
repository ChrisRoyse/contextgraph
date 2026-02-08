//! Maintenance tool handlers for data repair and cleanup.

use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::types::audit::{AuditOperation, AuditRecord};

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
                let retained_count = total_scanned.saturating_sub(deleted_count);
                info!(deleted = deleted_count, scanned = total_scanned, retained = retained_count, "Repair complete");

                // Emit CausalRelationshipRepaired audit record
                let audit_record = AuditRecord::new(
                    AuditOperation::CausalRelationshipRepaired {
                        deleted_count,
                        retained_count,
                        reason: "deserialization failure during repair scan".to_string(),
                    },
                    // Use a sentinel UUID since repair targets the whole CF, not a single entity
                    Uuid::nil(),
                )
                .with_operator("repair_causal_relationships")
                .with_rationale(format!(
                    "Scanned {} causal relationships, deleted {} corrupted, retained {}",
                    total_scanned, deleted_count, retained_count
                ))
                .with_parameters(json!({
                    "total_scanned": total_scanned,
                    "deleted_count": deleted_count,
                    "retained_count": retained_count,
                }));

                if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                    warn!(error = %e, "repair_causal_relationships: Failed to write audit record (non-fatal)");
                }

                self.tool_result(
                    id,
                    json!({
                        "status": "success",
                        "deleted_count": deleted_count,
                        "total_scanned": total_scanned,
                        "retained_count": retained_count,
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
