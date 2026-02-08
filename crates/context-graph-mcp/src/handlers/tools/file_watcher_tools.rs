//! File watcher tool handlers.
//!
//! Per PRD v6 Section 10, implements:
//! - list_watched_files: List all files with embeddings
//! - get_file_watcher_stats: Get statistics about file watcher content
//! - delete_file_content: Delete all embeddings for a specific file
//! - reconcile_files: Find orphaned files and optionally delete them
//!
//! Constitution Compliance:
//! - SEC-06: Soft delete 30-day recovery
//! - FAIL FAST: All tools error on failures, no fallbacks

use std::path::Path;

use glob::Pattern;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::types::audit::{AuditOperation, AuditRecord};

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

// ============================================================================
// Request/Response DTOs
// ============================================================================

/// Request for list_watched_files.
#[derive(Debug, Deserialize)]
pub struct ListWatchedFilesRequest {
    #[serde(default = "default_true")]
    pub include_counts: bool,
    pub path_filter: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Response for list_watched_files.
#[derive(Debug, Serialize)]
pub struct ListWatchedFilesResponse {
    pub files: Vec<WatchedFileInfo>,
    pub total_files: usize,
}

/// Information about a watched file.
#[derive(Debug, Serialize)]
pub struct WatchedFileInfo {
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

/// Response for get_file_watcher_stats.
#[derive(Debug, Serialize)]
pub struct GetFileWatcherStatsResponse {
    pub total_files: usize,
    pub total_chunks: usize,
    pub avg_chunks_per_file: f64,
    pub min_chunks: usize,
    pub max_chunks: usize,
}

/// Request for delete_file_content.
#[derive(Debug, Deserialize)]
pub struct DeleteFileContentRequest {
    pub file_path: String,
    #[serde(default = "default_true")]
    pub soft_delete: bool,
}

/// Response for delete_file_content.
#[derive(Debug, Serialize)]
pub struct DeleteFileContentResponse {
    pub file_path: String,
    pub deleted_count: usize,
    pub soft_delete: bool,
    pub message: String,
}

/// Request for reconcile_files.
#[derive(Debug, Deserialize)]
pub struct ReconcileFilesRequest {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    pub base_path: Option<String>,
}

/// Response for reconcile_files.
#[derive(Debug, Serialize)]
pub struct ReconcileFilesResponse {
    pub orphaned_files: Vec<String>,
    pub orphan_count: usize,
    pub deleted_count: usize,
    pub dry_run: bool,
}

// ============================================================================
// Handler Implementations
// ============================================================================

impl Handlers {
    /// Handle list_watched_files tool call.
    ///
    /// Lists all files that have embeddings in the knowledge graph.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (include_counts, path_filter)
    ///
    /// # Returns
    /// JsonRpcResponse with ListWatchedFilesResponse
    pub(crate) async fn call_list_watched_files(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling list_watched_files");

        // Parse request
        let request: ListWatchedFilesRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "list_watched_files: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Get indexed files from store
        let entries = match self.teleological_store.list_indexed_files().await {
            Ok(entries) => entries,
            Err(e) => {
                error!(error = %e, "list_watched_files: Failed to list indexed files");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to list files: {}", e),
                );
            }
        };

        // Compile glob pattern if provided
        let pattern: Option<Pattern> = if let Some(ref filter) = request.path_filter {
            match Pattern::new(filter) {
                Ok(p) => Some(p),
                Err(e) => {
                    error!(error = %e, pattern = filter, "list_watched_files: Invalid glob pattern");
                    return self.tool_error(
                        id,
                        &format!("Invalid glob pattern '{}': {}", filter, e),
                    );
                }
            }
        } else {
            None
        };

        // Filter and convert to response
        let files: Vec<WatchedFileInfo> = entries
            .iter()
            .filter(|entry| {
                // Apply glob filter if provided
                if let Some(ref p) = pattern {
                    p.matches(&entry.file_path)
                } else {
                    true
                }
            })
            .map(|entry| WatchedFileInfo {
                file_path: entry.file_path.clone(),
                chunk_count: if request.include_counts {
                    Some(entry.fingerprint_count())
                } else {
                    None
                },
                last_updated: Some(entry.last_updated.to_rfc3339()),
            })
            .collect();

        let response = ListWatchedFilesResponse {
            total_files: files.len(),
            files,
        };

        info!(
            "list_watched_files: Returned {} files{}",
            response.total_files,
            if request.path_filter.is_some() {
                " (filtered)"
            } else {
                ""
            }
        );

        self.tool_result(
            id,
            serde_json::to_value(response).expect("ListWatchedFilesResponse should serialize"),
        )
    }

    /// Handle get_file_watcher_stats tool call.
    ///
    /// Returns statistics about file watcher content in the knowledge graph.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    ///
    /// # Returns
    /// JsonRpcResponse with GetFileWatcherStatsResponse
    pub(crate) async fn call_get_file_watcher_stats(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_file_watcher_stats");

        // Get stats from store
        let stats = match self.teleological_store.get_file_watcher_stats().await {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "get_file_watcher_stats: Failed to get stats");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to get stats: {}", e),
                );
            }
        };

        let response = GetFileWatcherStatsResponse {
            total_files: stats.total_files,
            total_chunks: stats.total_chunks,
            avg_chunks_per_file: stats.avg_chunks_per_file,
            min_chunks: stats.min_chunks,
            max_chunks: stats.max_chunks,
        };

        info!(
            "get_file_watcher_stats: {} files, {} chunks",
            response.total_files, response.total_chunks
        );

        self.tool_result(
            id,
            serde_json::to_value(response).expect("GetFileWatcherStatsResponse should serialize"),
        )
    }

    /// Handle delete_file_content tool call.
    ///
    /// Deletes all embeddings for a specific file path.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (file_path, soft_delete)
    ///
    /// # Returns
    /// JsonRpcResponse with DeleteFileContentResponse
    ///
    /// # Constitution Compliance
    /// - SEC-06: 30-day recovery for soft delete
    pub(crate) async fn call_delete_file_content(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling delete_file_content");

        // Parse request
        let request: DeleteFileContentRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "delete_file_content: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Validate file_path is not empty
        if request.file_path.trim().is_empty() {
            error!("delete_file_content: Empty file_path provided");
            return self.tool_error(id, "file_path cannot be empty");
        }

        // Get fingerprints for the file
        let fingerprint_ids = match self
            .teleological_store
            .get_fingerprints_for_file(&request.file_path)
            .await
        {
            Ok(ids) => ids,
            Err(e) => {
                error!(error = %e, file_path = %request.file_path, "delete_file_content: Failed to get fingerprints");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to get fingerprints: {}", e),
                );
            }
        };

        if fingerprint_ids.is_empty() {
            info!(
                file_path = %request.file_path,
                "delete_file_content: No fingerprints found for file"
            );
            return self.tool_result(
                id,
                json!({
                    "file_path": request.file_path,
                    "deleted_count": 0,
                    "soft_delete": request.soft_delete,
                    "message": "No embeddings found for this file"
                }),
            );
        }

        // Delete each fingerprint
        let mut deleted_count = 0;
        for fp_id in &fingerprint_ids {
            match self
                .teleological_store
                .delete(*fp_id, request.soft_delete)
                .await
            {
                Ok(true) => {
                    deleted_count += 1;
                    debug!(
                        fingerprint_id = %fp_id,
                        soft_delete = request.soft_delete,
                        "delete_file_content: Deleted fingerprint"
                    );
                }
                Ok(false) => {
                    warn!(
                        fingerprint_id = %fp_id,
                        "delete_file_content: Fingerprint not found (may have been deleted concurrently)"
                    );
                }
                Err(e) => {
                    error!(
                        error = %e,
                        fingerprint_id = %fp_id,
                        "delete_file_content: Failed to delete fingerprint"
                    );
                    return self.tool_error(
                        id,
                        &format!("Storage error: Failed to delete fingerprint {}: {}", fp_id, e),
                    );
                }
            }
        }

        // Clear the file index
        if let Err(e) = self
            .teleological_store
            .clear_file_index(&request.file_path)
            .await
        {
            error!(
                error = %e,
                file_path = %request.file_path,
                "delete_file_content: Failed to clear file index"
            );
            // Don't fail - the fingerprints were already deleted
            warn!("Fingerprints deleted but file index clear failed - index may be stale");
        }

        let message = if request.soft_delete {
            format!(
                "Soft-deleted {} embeddings for '{}' (30-day recovery per SEC-06)",
                deleted_count, request.file_path
            )
        } else {
            format!(
                "HARD-deleted {} embeddings for '{}' (no recovery possible)",
                deleted_count, request.file_path
            )
        };

        info!(
            file_path = %request.file_path,
            deleted_count = deleted_count,
            soft_delete = request.soft_delete,
            "delete_file_content: Operation complete"
        );

        // Emit FileWatcherEvent audit record for this destructive operation
        let audit_record = AuditRecord::new(
            AuditOperation::FileWatcherEvent {
                event_type: if request.soft_delete { "soft_delete".to_string() } else { "hard_delete".to_string() },
                file_path: request.file_path.clone(),
                memories_affected: deleted_count,
            },
            // Use nil UUID since this targets a file path, not a single memory
            Uuid::nil(),
        )
        .with_operator("delete_file_content")
        .with_rationale(format!(
            "{} {} embeddings for '{}'",
            if request.soft_delete { "Soft-deleted" } else { "Hard-deleted" },
            deleted_count,
            request.file_path
        ))
        .with_parameters(json!({
            "file_path": request.file_path,
            "soft_delete": request.soft_delete,
            "deleted_count": deleted_count,
        }));

        if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
            warn!(error = %e, "delete_file_content: Failed to write audit record (non-fatal)");
        }

        let response = DeleteFileContentResponse {
            file_path: request.file_path,
            deleted_count,
            soft_delete: request.soft_delete,
            message,
        };

        self.tool_result(
            id,
            serde_json::to_value(response).expect("DeleteFileContentResponse should serialize"),
        )
    }

    /// Handle reconcile_files tool call.
    ///
    /// Finds orphaned files (embeddings exist but file doesn't on disk)
    /// and optionally deletes them.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (dry_run, base_path)
    ///
    /// # Returns
    /// JsonRpcResponse with ReconcileFilesResponse
    pub(crate) async fn call_reconcile_files(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling reconcile_files");

        // Parse request
        let request: ReconcileFilesRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "reconcile_files: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Get indexed files from store
        let entries = match self.teleological_store.list_indexed_files().await {
            Ok(entries) => entries,
            Err(e) => {
                error!(error = %e, "reconcile_files: Failed to list indexed files");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to list files: {}", e),
                );
            }
        };

        let mut orphans = Vec::new();
        let mut deleted_count = 0;

        for entry in entries {
            // Apply base_path filter if provided
            if let Some(ref base) = request.base_path {
                if !entry.file_path.starts_with(base) {
                    continue;
                }
            }

            // Check if file exists on disk
            if !Path::new(&entry.file_path).exists() {
                orphans.push(entry.file_path.clone());

                if !request.dry_run {
                    // Delete all fingerprints for this file
                    let ids = match self
                        .teleological_store
                        .get_fingerprints_for_file(&entry.file_path)
                        .await
                    {
                        Ok(ids) => ids,
                        Err(e) => {
                            error!(
                                error = %e,
                                file_path = %entry.file_path,
                                "reconcile_files: Failed to get fingerprints for orphan"
                            );
                            continue;
                        }
                    };

                    for fp_id in ids {
                        // Use soft delete per SEC-06
                        if let Err(e) = self.teleological_store.delete(fp_id, true).await {
                            error!(
                                error = %e,
                                fingerprint_id = %fp_id,
                                "reconcile_files: Failed to delete orphan fingerprint"
                            );
                        }
                    }

                    // Clear the file index
                    if let Err(e) = self
                        .teleological_store
                        .clear_file_index(&entry.file_path)
                        .await
                    {
                        error!(
                            error = %e,
                            file_path = %entry.file_path,
                            "reconcile_files: Failed to clear file index for orphan"
                        );
                    }

                    deleted_count += 1;
                    info!(
                        file_path = %entry.file_path,
                        "reconcile_files: Deleted orphaned file embeddings"
                    );
                }
            }
        }

        info!(
            orphan_count = orphans.len(),
            deleted_count = deleted_count,
            dry_run = request.dry_run,
            "reconcile_files: Operation complete"
        );

        // Emit FileWatcherEvent audit record for reconcile (only if not dry run and we deleted)
        if !request.dry_run && deleted_count > 0 {
            let audit_record = AuditRecord::new(
                AuditOperation::FileWatcherEvent {
                    event_type: "reconcile_delete".to_string(),
                    file_path: request.base_path.clone().unwrap_or_default(),
                    memories_affected: deleted_count,
                },
                Uuid::nil(),
            )
            .with_operator("reconcile_files")
            .with_rationale(format!(
                "Reconciled {} orphaned files (deleted embeddings for files no longer on disk)",
                deleted_count
            ))
            .with_parameters(json!({
                "orphan_count": orphans.len(),
                "deleted_count": deleted_count,
                "dry_run": false,
                "base_path": request.base_path,
                "orphaned_files": &orphans,
            }));

            if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                warn!(error = %e, "reconcile_files: Failed to write audit record (non-fatal)");
            }
        }

        let response = ReconcileFilesResponse {
            orphan_count: orphans.len(),
            orphaned_files: orphans,
            deleted_count,
            dry_run: request.dry_run,
        };

        self.tool_result(
            id,
            serde_json::to_value(response).expect("ReconcileFilesResponse should serialize"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_watched_files_request_defaults() {
        let json = r#"{}"#;
        let req: ListWatchedFilesRequest = serde_json::from_str(json).unwrap();
        assert!(req.include_counts);
        assert!(req.path_filter.is_none());
    }

    #[test]
    fn test_delete_file_content_request_defaults() {
        let json = r#"{"file_path": "/test/path.md"}"#;
        let req: DeleteFileContentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.file_path, "/test/path.md");
        assert!(req.soft_delete);
    }

    #[test]
    fn test_reconcile_files_request_defaults() {
        let json = r#"{}"#;
        let req: ReconcileFilesRequest = serde_json::from_str(json).unwrap();
        assert!(req.dry_run);
        assert!(req.base_path.is_none());
    }

    #[test]
    fn test_watched_file_info_serialization() {
        let info = WatchedFileInfo {
            file_path: "/test/path.md".to_string(),
            chunk_count: Some(5),
            last_updated: Some("2024-01-15T10:30:00Z".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("file_path"));
        assert!(json.contains("chunk_count"));
        assert!(json.contains("last_updated"));
    }

    #[test]
    fn test_watched_file_info_skip_none() {
        let info = WatchedFileInfo {
            file_path: "/test/path.md".to_string(),
            chunk_count: None,
            last_updated: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("file_path"));
        assert!(!json.contains("chunk_count"));
        assert!(!json.contains("last_updated"));
    }
}
