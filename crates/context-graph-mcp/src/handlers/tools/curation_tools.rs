//! Curation tool handlers.
//!
//! Per PRD Section 10.3, implements:
//! - forget_concept: Soft-delete a memory (30-day recovery per SEC-06)
//! - boost_importance: Adjust memory importance score (deprecated - see note)
//!
//! Constitution Compliance:
//! - SEC-06: Soft delete 30-day recovery
//! - BR-MCP-001: forget_concept uses soft delete by default
//! - BR-MCP-002: boost_importance clamps final value to [0.0, 1.0]

use chrono::Utc;
use tracing::{debug, error, info, warn};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::curation_dtos::{
    BoostImportanceRequest, BoostImportanceResponse, ForgetConceptRequest, ForgetConceptResponse,
};

impl Handlers {
    /// Handle forget_concept tool call.
    ///
    /// Soft-deletes a memory with 30-day recovery window per SEC-06.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (node_id, soft_delete)
    ///
    /// # Returns
    /// JsonRpcResponse with ForgetConceptResponse
    ///
    /// # Constitution Compliance
    /// - SEC-06: 30-day recovery for soft delete
    /// - BR-MCP-001: soft_delete defaults to true
    pub(crate) async fn call_forget_concept(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling forget_concept");

        // Parse request
        let request: ForgetConceptRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "forget_concept: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Validate and parse UUID using DTO's validate method
        let node_id = match request.validate() {
            Ok(uuid) => uuid,
            Err(validation_error) => {
                error!(error = %validation_error, "forget_concept: Validation failed");
                return self
                    .tool_error(id, &format!("Invalid params: {}", validation_error));
            }
        };

        // Check if memory exists - FAIL FAST if not found
        match self.teleological_store.retrieve(node_id).await {
            Ok(Some(_)) => {
                debug!(node_id = %node_id, soft_delete = request.soft_delete, "forget_concept: Memory exists, proceeding with delete");
            }
            Ok(None) => {
                warn!(node_id = %node_id, "forget_concept: Memory not found");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Memory {} not found", node_id),
                );
            }
            Err(e) => {
                error!(error = %e, node_id = %node_id, "forget_concept: Failed to check memory existence");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to check memory: {}", e),
                );
            }
        };

        // Perform delete operation
        let delete_result = self
            .teleological_store
            .delete(node_id, request.soft_delete)
            .await;

        match delete_result {
            Ok(true) => {
                // PHASE-1.2: Append audit record for deletion
                {
                    use context_graph_core::types::audit::{AuditOperation, AuditRecord};
                    let audit_op = AuditOperation::MemoryDeleted {
                        soft: request.soft_delete,
                        reason: request.reason.clone(),
                    };
                    let mut audit_record = AuditRecord::new(audit_op, node_id);
                    if let Some(ref op_id) = request.operator_id {
                        audit_record = audit_record.with_operator(op_id.clone());
                    }

                    if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                        warn!(
                            node_id = %node_id,
                            error = %e,
                            "forget_concept: Failed to append audit record (deletion completed successfully)"
                        );
                    } else {
                        debug!(
                            node_id = %node_id,
                            audit_id = %audit_record.id,
                            "forget_concept: Audit record appended successfully"
                        );
                    }
                }

                // Build response using DTO factory methods
                let response = if request.soft_delete {
                    info!(node_id = %node_id, "forget_concept: Soft deleted memory (30-day recovery per SEC-06)");
                    ForgetConceptResponse::soft_deleted(node_id)
                } else {
                    warn!(node_id = %node_id, "forget_concept: HARD deleted memory (no recovery)");
                    ForgetConceptResponse::hard_deleted(node_id)
                };

                self.tool_result(
                    id,
                    serde_json::to_value(response).expect("ForgetConceptResponse should serialize"),
                )
            }
            Ok(false) => {
                // Store returned false - memory not found (race condition)
                warn!(node_id = %node_id, "forget_concept: Delete returned false - memory may have been deleted concurrently");
                JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!(
                        "Memory {} not found (may have been deleted concurrently)",
                        node_id
                    ),
                )
            }
            Err(e) => {
                error!(error = %e, node_id = %node_id, "forget_concept: Delete operation failed");
                self.tool_error(id, &format!("Storage error: Delete failed: {}", e))
            }
        }
    }

    /// Handle boost_importance tool call.
    ///
    /// Per PRD Section 7, importance is computed from:
    ///   Importance = Frequency_Score × Recency_Weight
    ///   Frequency_Score = BM25_saturated(log(1+access_count))
    ///
    /// This tool increases access_count to boost the computed importance score.
    /// A positive delta increments access_count, negative decrements (min 0).
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (node_id, delta)
    ///
    /// # Returns
    /// JsonRpcResponse with BoostImportanceResponse
    ///
    /// # Constitution Compliance
    /// - BR-MCP-002: Importance clamped to [0.0, 1.0]
    /// - AP-10: No NaN/Infinity in values
    pub(crate) async fn call_boost_importance(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling boost_importance");

        // Parse request
        let request: BoostImportanceRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "boost_importance: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Validate delta range and UUID using DTO's validate method
        // This checks: NaN/Infinity (AP-10), delta range [-1.0, 1.0], UUID format
        let node_id = match request.validate() {
            Ok(uuid) => uuid,
            Err(validation_error) => {
                error!(error = %validation_error, "boost_importance: Validation failed");
                return self
                    .tool_error(id, &format!("Invalid params: {}", validation_error));
            }
        };

        debug!(node_id = %node_id, delta = request.delta, "boost_importance: Processing request");

        // Get current memory
        let mut fingerprint = match self.teleological_store.retrieve(node_id).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                warn!(node_id = %node_id, "boost_importance: Memory not found");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Memory {} not found", node_id),
                );
            }
            Err(e) => {
                error!(error = %e, node_id = %node_id, "boost_importance: Failed to retrieve memory");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to retrieve memory: {}", e),
                );
            }
        };

        // Read actual importance field from fingerprint (not computed from access_count)
        let old_importance = fingerprint.importance;

        // Apply delta and clamp to [0.0, 1.0] per BR-MCP-002
        let (new_importance, clamped) = request.apply_delta(old_importance);

        // Update the importance field directly
        fingerprint.importance = new_importance;

        debug!(
            node_id = %node_id,
            old_importance = old_importance,
            delta = request.delta,
            new_importance = new_importance,
            clamped = clamped,
            "boost_importance: Updated importance field directly"
        );

        // Update last_updated timestamp
        fingerprint.last_updated = Utc::now();

        // Persist the updated fingerprint
        match self.teleological_store.update(fingerprint).await {
            Ok(true) => {
                info!(
                    node_id = %node_id,
                    old = old_importance,
                    new = new_importance,
                    clamped = clamped,
                    "boost_importance: Updated memory successfully"
                );

                // PHASE-1.2: Append audit record for importance boost
                {
                    use context_graph_core::types::audit::{AuditOperation, AuditRecord};
                    use serde_json::json;
                    let audit_op = AuditOperation::ImportanceBoosted {
                        old: old_importance,
                        new: new_importance,
                        delta: request.delta,
                    };
                    let mut audit_record = AuditRecord::new(audit_op, node_id);
                    if let Some(ref op_id) = request.operator_id {
                        audit_record = audit_record.with_operator(op_id.clone());
                    }
                    audit_record = audit_record.with_parameters(json!({
                        "old_importance": old_importance,
                        "new_importance": new_importance,
                        "delta": request.delta,
                        "clamped": clamped,
                    }));

                    if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                        warn!(
                            node_id = %node_id,
                            error = %e,
                            "boost_importance: Failed to append audit record (update completed successfully)"
                        );
                    } else {
                        debug!(
                            node_id = %node_id,
                            audit_id = %audit_record.id,
                            "boost_importance: Audit record appended successfully"
                        );
                    }
                }

                // PHASE-4: Write permanent ImportanceChangeRecord to CF_IMPORTANCE_HISTORY
                {
                    use context_graph_core::types::audit::ImportanceChangeRecord;
                    let change_record = ImportanceChangeRecord {
                        memory_id: node_id,
                        timestamp: Utc::now(),
                        old_value: old_importance,
                        new_value: new_importance,
                        delta: request.delta,
                        operator_id: request.operator_id.clone(),
                        reason: None,
                    };

                    if let Err(e) = self.teleological_store.append_importance_change(&change_record).await {
                        warn!(
                            node_id = %node_id,
                            error = %e,
                            "boost_importance: Failed to write importance history (update completed successfully)"
                        );
                    } else {
                        debug!(
                            node_id = %node_id,
                            old = old_importance,
                            new = new_importance,
                            "boost_importance: Importance history written to CF_IMPORTANCE_HISTORY"
                        );
                    }
                }

                // Build response using DTO factory method
                let response = BoostImportanceResponse::new(node_id, old_importance, request.delta);

                self.tool_result(
                    id,
                    serde_json::to_value(response)
                        .expect("BoostImportanceResponse should serialize"),
                )
            }
            Ok(false) => {
                // Update returned false - memory not found (race condition)
                warn!(node_id = %node_id, "boost_importance: Update returned false - memory may have been deleted concurrently");
                JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!(
                        "Memory {} not found (may have been deleted concurrently)",
                        node_id
                    ),
                )
            }
            Err(e) => {
                error!(error = %e, node_id = %node_id, "boost_importance: Update operation failed");
                self.tool_error(id, &format!("Storage error: Update failed: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::curation_dtos::{MAX_IMPORTANCE, MIN_IMPORTANCE, SOFT_DELETE_RECOVERY_DAYS};
    use super::*;

    #[test]
    fn test_constants_match_constitution() {
        // SEC-06: 30-day recovery
        assert_eq!(
            SOFT_DELETE_RECOVERY_DAYS, 30,
            "SOFT_DELETE_RECOVERY_DAYS must be 30 per SEC-06"
        );

        // BR-MCP-002: Importance clamped to [0.0, 1.0]
        assert!(
            (MIN_IMPORTANCE - 0.0).abs() < f32::EPSILON,
            "MIN_IMPORTANCE must be 0.0"
        );
        assert!(
            (MAX_IMPORTANCE - 1.0).abs() < f32::EPSILON,
            "MAX_IMPORTANCE must be 1.0"
        );
    }

    #[test]
    fn test_soft_delete_is_default() {
        // BR-MCP-001: forget_concept uses soft delete by default
        let json = r#"{"node_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: ForgetConceptRequest = serde_json::from_str(json).unwrap();
        assert!(
            req.soft_delete,
            "soft_delete must default to true per BR-MCP-001"
        );
    }
}
