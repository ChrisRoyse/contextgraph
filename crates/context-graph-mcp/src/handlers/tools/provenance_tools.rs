//! Provenance query tool handlers (Phase P3).

use chrono::DateTime;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use tracing::{debug, error};
use uuid::Uuid;

use context_graph_core::types::audit::{AuditOperation, AuditRecord};

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

/// Serialize an audit record to JSON for API responses.
fn audit_record_to_json(r: &AuditRecord) -> serde_json::Value {
    json!({
        "id": r.id.to_string(),
        "timestamp": r.timestamp.to_rfc3339(),
        "operation": serde_json::to_value(&r.operation).unwrap_or_else(|_| json!(format!("{}", r.operation))),
        "target_id": r.target_id.to_string(),
        "operator_id": r.operator_id,
        "session_id": r.session_id,
        "rationale": r.rationale,
        "parameters": r.parameters.clone(),
        "result": serde_json::to_value(&r.result).unwrap_or_else(|_| json!(format!("{}", r.result))),
    })
}

// ========== get_audit_trail ==========

#[derive(Debug, Deserialize)]
struct GetAuditTrailParams {
    target_id: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
    #[serde(default = "default_audit_limit", deserialize_with = "deserialize_usize_lenient")]
    limit: usize,
}

fn default_audit_limit() -> usize {
    50
}

/// MCP clients may send integers as JSON strings — accept both.
fn deserialize_usize_lenient<'de, D: Deserializer<'de>>(d: D) -> Result<usize, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    match &v {
        serde_json::Value::Number(n) => n
            .as_u64()
            .map(|n| n as usize)
            .ok_or_else(|| serde::de::Error::custom("limit must be a non-negative integer")),
        serde_json::Value::String(s) => s
            .parse::<usize>()
            .map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("limit must be an integer or string")),
    }
}

// ========== get_merge_history ==========

#[derive(Debug, Deserialize)]
struct GetMergeHistoryParams {
    memory_id: String,
    #[serde(default)]
    include_source_metadata: bool,
}

// ========== get_provenance_chain ==========

#[derive(Debug, Deserialize)]
struct GetProvenanceChainParams {
    memory_id: String,
    #[serde(default)]
    include_audit: bool,
    #[serde(default)]
    include_embedding_version: bool,
    #[serde(default)]
    include_importance_history: bool,
    #[serde(default)]
    include_merge_history: bool,
    /// When true, includes ALL provenance data (audit, embedding, importance, merge).
    #[serde(default)]
    depth_full: bool,
}

impl Handlers {
    pub(crate) async fn call_get_audit_trail(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_audit_trail tool call");

        let params: GetAuditTrailParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "get_audit_trail: Failed to parse parameters");
                return self.tool_error(id, &format!("Invalid parameters: {}", e));
            }
        };

        let limit = params.limit.min(500);

        if let Some(target_id_str) = &params.target_id {
            // Query by target
            let target_uuid = match Uuid::parse_str(target_id_str) {
                Ok(u) => u,
                Err(e) => {
                    error!(error = %e, "get_audit_trail: Invalid target_id UUID");
                    return self.tool_error(id, &format!("Invalid target_id UUID: {}", e));
                }
            };

            match self.teleological_store.get_audit_by_target(target_uuid, limit).await {
                Ok(records) => {
                    let records_json: Vec<serde_json::Value> =
                        records.iter().map(audit_record_to_json).collect();

                    self.tool_result(id, json!({
                        "audit_trail": records_json,
                        "count": records_json.len(),
                        "target_id": target_id_str,
                    }))
                }
                Err(e) => {
                    error!(error = %e, "get_audit_trail: Store query failed");
                    self.tool_error(id, &format!("Audit query failed: {}", e))
                }
            }
        } else if params.start_time.is_some() || params.end_time.is_some() {
            // Query by time range — default end_time to "now" if only start_time provided
            let start_str = match &params.start_time {
                Some(s) => s.clone(),
                None => {
                    return self.tool_error(id, "end_time requires start_time");
                }
            };
            let end_str = params.end_time.clone().unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let start = match DateTime::parse_from_rfc3339(&start_str) {
                Ok(dt) => dt.with_timezone(&chrono::Utc),
                Err(e) => {
                    error!(error = %e, "get_audit_trail: Invalid start_time");
                    return self.tool_error(id, &format!("Invalid start_time: {}", e));
                }
            };
            let end = match DateTime::parse_from_rfc3339(&end_str) {
                Ok(dt) => dt.with_timezone(&chrono::Utc),
                Err(e) => {
                    error!(error = %e, "get_audit_trail: Invalid end_time");
                    return self.tool_error(id, &format!("Invalid end_time: {}", e));
                }
            };

            match self.teleological_store.get_audit_by_time_range(start, end, limit).await {
                Ok(records) => {
                    let records_json: Vec<serde_json::Value> =
                        records.iter().map(audit_record_to_json).collect();

                    self.tool_result(id, json!({
                        "audit_trail": records_json,
                        "count": records_json.len(),
                        "time_range": { "start": start_str, "end": end_str },
                    }))
                }
                Err(e) => {
                    error!(error = %e, "get_audit_trail: Time range query failed");
                    self.tool_error(id, &format!("Audit time range query failed: {}", e))
                }
            }
        } else {
            self.tool_error(id, "Provide target_id or time range (start_time + end_time)")
        }
    }

    pub(crate) async fn call_get_merge_history(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_merge_history tool call");

        let params: GetMergeHistoryParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "get_merge_history: Failed to parse parameters");
                return self.tool_error(id, &format!("Invalid parameters: {}", e));
            }
        };

        let memory_uuid = match Uuid::parse_str(&params.memory_id) {
            Ok(u) => u,
            Err(e) => {
                error!(error = %e, "get_merge_history: Invalid memory_id UUID");
                return self.tool_error(id, &format!("Invalid memory_id UUID: {}", e));
            }
        };

        // Get audit records and filter to merges
        let merge_records = match self.teleological_store.get_audit_by_target(memory_uuid, 100).await {
            Ok(records) => records
                .into_iter()
                .filter(|r| matches!(r.operation, AuditOperation::MemoryMerged { .. }))
                .collect::<Vec<_>>(),
            Err(e) => {
                error!(error = %e, "get_merge_history: Audit query failed");
                return self.tool_error(id, &format!("Merge history query failed: {}", e));
            }
        };

        // Get source metadata for derived_from info
        let source_metadata = match self.teleological_store.get_source_metadata(memory_uuid).await {
            Ok(meta) => meta,
            Err(e) => {
                error!(error = %e, "get_merge_history: Source metadata query failed");
                None
            }
        };

        let derived_from = source_metadata.as_ref().and_then(|m| m.derived_from.as_ref());
        let derivation_method = source_metadata.as_ref().and_then(|m| m.derivation_method.as_deref());

        let merge_records_json: Vec<serde_json::Value> = merge_records
            .iter()
            .map(|r| {
                let (source_ids, strategy) = match &r.operation {
                    AuditOperation::MemoryMerged { source_ids, strategy } => {
                        (source_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(), strategy.clone())
                    }
                    _ => (vec![], String::new()),
                };
                json!({
                    "timestamp": r.timestamp.to_rfc3339(),
                    "source_ids": source_ids,
                    "strategy": strategy,
                    "operator_id": r.operator_id,
                    "rationale": r.rationale,
                })
            })
            .collect();

        // Optionally include source metadata for each merged source
        let mut source_details = Vec::new();
        if params.include_source_metadata {
            if let Some(derived) = derived_from {
                for source_id in derived {
                    match self.teleological_store.get_source_metadata(*source_id).await {
                        Ok(Some(meta)) => {
                            source_details.push(json!({
                                "source_id": source_id.to_string(),
                                "source_type": format!("{}", meta.source_type),
                                "file_path": meta.file_path,
                                "created_by": meta.created_by,
                                "created_at": meta.created_at.map(|t| t.to_rfc3339()),
                            }));
                        }
                        _ => {
                            source_details.push(json!({
                                "source_id": source_id.to_string(),
                                "metadata": "not_found",
                            }));
                        }
                    }
                }
            }
        }

        self.tool_result(id, json!({
            "memory_id": params.memory_id,
            "merge_events": merge_records_json,
            "merge_event_count": merge_records_json.len(),
            "derived_from": derived_from.map(|ids| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()),
            "derivation_method": derivation_method,
            "source_details": if params.include_source_metadata { Some(source_details) } else { None },
        }))
    }

    pub(crate) async fn call_get_provenance_chain(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_provenance_chain tool call");

        let params: GetProvenanceChainParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "get_provenance_chain: Failed to parse parameters");
                return self.tool_error(id, &format!("Invalid parameters: {}", e));
            }
        };

        let memory_uuid = match Uuid::parse_str(&params.memory_id) {
            Ok(u) => u,
            Err(e) => {
                error!(error = %e, "get_provenance_chain: Invalid memory_id UUID");
                return self.tool_error(id, &format!("Invalid memory_id UUID: {}", e));
            }
        };

        // Get source metadata
        let source_metadata = match self.teleological_store.get_source_metadata(memory_uuid).await {
            Ok(meta) => meta,
            Err(e) => {
                error!(error = %e, "get_provenance_chain: Source metadata query failed");
                return self.tool_error(id, &format!("Source metadata query failed: {}", e));
            }
        };

        let chain = if let Some(meta) = &source_metadata {
            json!({
                "memory_id": params.memory_id,
                "source_type": format!("{}", meta.source_type),
                "file_path": meta.file_path,
                "chunk_info": {
                    "chunk_index": meta.chunk_index,
                    "total_chunks": meta.total_chunks,
                    "start_line": meta.start_line,
                    "end_line": meta.end_line,
                },
                "operator": {
                    "created_by": meta.created_by,
                    "created_at": meta.created_at.map(|t| t.to_rfc3339()),
                },
                "session": {
                    "session_id": meta.session_id,
                    "session_sequence": meta.session_sequence,
                },
                "causal_direction": meta.causal_direction,
                "file_provenance": {
                    "file_content_hash": meta.file_content_hash,
                    "file_modified_at": meta.file_modified_at.map(|t| t.to_rfc3339()),
                },
                "derivation": {
                    "derived_from": meta.derived_from.as_ref().map(|ids| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()),
                    "derivation_method": meta.derivation_method,
                },
                "tool_provenance": {
                    "tool_use_id": meta.tool_use_id,
                    "mcp_request_id": meta.mcp_request_id,
                    "hook_execution_timestamp_ms": meta.hook_execution_timestamp_ms,
                },
                "hook_info": {
                    "hook_type": meta.hook_type,
                    "tool_name": meta.tool_name,
                },
                "causal_explanation": {
                    "source_fingerprint_id": meta.source_fingerprint_id.map(|id| id.to_string()),
                    "causal_relationship_id": meta.causal_relationship_id.map(|id| id.to_string()),
                    "mechanism_type": meta.mechanism_type,
                    "confidence": meta.confidence,
                },
            })
        } else {
            json!({
                "memory_id": params.memory_id,
                "source_metadata": null,
                "note": "No source metadata found for this memory"
            })
        };

        // Effective flags - depth_full overrides individual flags
        let eff_audit = params.include_audit || params.depth_full;
        let eff_embedding = params.include_embedding_version || params.depth_full;
        let eff_importance = params.include_importance_history || params.depth_full;
        let eff_merge = params.include_merge_history || params.depth_full;

        // Optionally include audit trail (CF_AUDIT_LOG)
        let audit_trail = if eff_audit {
            match self.teleological_store.get_audit_by_target(memory_uuid, 50).await {
                Ok(records) => Some(
                    records
                        .iter()
                        .map(|r| {
                            json!({
                                "timestamp": r.timestamp.to_rfc3339(),
                                "operation": format!("{}", r.operation),
                                "operator_id": r.operator_id,
                                "result": format!("{}", r.result),
                            })
                        })
                        .collect::<Vec<_>>(),
                ),
                Err(e) => {
                    error!(
                        error = %e,
                        memory_id = %memory_uuid,
                        "get_provenance_chain: Audit trail query FAILED — storage error"
                    );
                    Some(vec![json!({
                        "error": format!("Audit trail query failed: {}", e),
                        "note": "Audit trail data may exist but could not be retrieved"
                    })])
                }
            }
        } else {
            None
        };

        // Query CF_EMBEDDING_REGISTRY for actual embedding version info
        let embedding_version = if eff_embedding {
            match self.teleological_store.get_embedding_version(memory_uuid).await {
                Ok(Some(record)) => {
                    Some(json!({
                        "fingerprint_id": record.fingerprint_id.to_string(),
                        "computed_at": record.computed_at.to_rfc3339(),
                        "embedder_versions": record.embedder_versions,
                        "e7_model_version": record.e7_model_version,
                        "computation_time_ms": record.computation_time_ms,
                    }))
                }
                Ok(None) => {
                    Some(json!({
                        "status": "not_tracked",
                        "note": "No embedding version record for this memory"
                    }))
                }
                Err(e) => {
                    error!(error = %e, "get_provenance_chain: Embedding version query failed");
                    Some(json!({
                        "status": "error",
                        "error": format!("{}", e),
                    }))
                }
            }
        } else {
            None
        };

        // Query CF_IMPORTANCE_HISTORY for importance change records
        let importance_history = if eff_importance {
            match self.teleological_store.get_importance_history(memory_uuid, 50).await {
                Ok(records) => {
                    if records.is_empty() {
                        None
                    } else {
                        Some(records.iter().map(|r| {
                            json!({
                                "timestamp": r.timestamp.to_rfc3339(),
                                "old_value": r.old_value,
                                "new_value": r.new_value,
                                "delta": r.delta,
                                "operator_id": r.operator_id,
                                "reason": r.reason,
                            })
                        }).collect::<Vec<_>>())
                    }
                }
                Err(e) => {
                    error!(error = %e, "get_provenance_chain: Importance history query failed");
                    None
                }
            }
        } else {
            None
        };

        // Query CF_MERGE_HISTORY for merge records
        let merge_history = if eff_merge {
            match self.teleological_store.get_merge_history(memory_uuid, 50).await {
                Ok(records) => {
                    if records.is_empty() {
                        None
                    } else {
                        Some(records.iter().map(|r| {
                            json!({
                                "id": r.id.to_string(),
                                "merged_id": r.merged_id.to_string(),
                                "source_ids": r.source_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                                "strategy": r.strategy,
                                "rationale": r.rationale,
                                "operator_id": r.operator_id,
                                "timestamp": r.timestamp.to_rfc3339(),
                            })
                        }).collect::<Vec<_>>())
                    }
                }
                Err(e) => {
                    error!(error = %e, "get_provenance_chain: Merge history query failed");
                    None
                }
            }
        } else {
            None
        };

        self.tool_result(id, json!({
            "provenance_chain": chain,
            "audit_trail": audit_trail,
            "embedding_version": embedding_version,
            "importance_history": importance_history,
            "merge_history": merge_history,
        }))
    }
}
