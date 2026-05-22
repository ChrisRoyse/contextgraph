//! MCP tool result and request-parsing helpers.

use std::path::{Component, Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use context_graph_core::types::audit::{AuditRecord, ImportanceChangeRecord};
use context_graph_paths::{
    AIWONDER_DURABLE_ROOT, AIWONDER_EXPLICIT_SCRATCH_ROOT, AIWONDER_HOT_ROOT,
};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::validate::{Validate, ValidateInto};

/// Typed error categories for consistent MCP tool error responses.
///
/// Maps tool-level error categories to JSON-RPC error codes from protocol.rs.
/// Used by `Handlers::tool_error_typed` to produce MCP-compliant responses
/// that include the error code for machine-parseable error handling.
///
/// ## MCP Protocol Note
/// Tool errors are returned as JSON-RPC *success* responses with `isError: true`
/// in the content (per MCP spec). Protocol errors (method_not_found, invalid_request)
/// use `JsonRpcResponse::error()` — do NOT use ToolErrorKind for those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolErrorKind {
    /// Invalid request parameters (bad format, out of range, missing required fields)
    Validation,
    /// Storage layer failure (RocksDB read/write, serialization)
    Storage,
    /// Requested resource not found (memory, fingerprint, session)
    NotFound,
    /// General execution failure (internal error, unexpected state)
    Execution,
}

pub(crate) fn mejepa_db_source_of_truth(db_path: &Path, extra: Value) -> Value {
    let classification = classify_mejepa_db_path(db_path);
    let mut object = match extra {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    object.insert("dbPath".to_string(), json!(db_path.display().to_string()));
    object.insert(
        "resolvedDbPath".to_string(),
        json!(classification.resolved.display().to_string()),
    );
    object.insert("sourceOfTruthKind".to_string(), json!(classification.kind));
    object.insert(
        "productionRootVerified".to_string(),
        json!(classification.production_root_verified),
    );
    object.insert(
        "fixtureOrLocal".to_string(),
        json!(!classification.production_root_verified),
    );
    object.insert(
        "shipGateCountable".to_string(),
        json!(classification.ship_gate_countable),
    );
    object.insert(
        "aiwonderRootPolicy".to_string(),
        json!(
            "durable gate evidence must live under /zfs/archive/contextgraph; hot production work may live under /zfs/hot/contextgraph; all other DB paths are fixture/local and not ship-gate-countable"
        ),
    );
    if let Some(root) = classification.matched_root {
        object.insert("matchedAiwonderRoot".to_string(), json!(root));
    }
    Value::Object(object)
}

struct MejepaDbPathClassification {
    resolved: PathBuf,
    kind: &'static str,
    matched_root: Option<&'static str>,
    production_root_verified: bool,
    ship_gate_countable: bool,
}

fn classify_mejepa_db_path(db_path: &Path) -> MejepaDbPathClassification {
    let resolved = normalize_absolute_lexical(db_path);
    if resolved.starts_with(AIWONDER_DURABLE_ROOT) {
        MejepaDbPathClassification {
            resolved,
            kind: "aiwonder_durable",
            matched_root: Some(AIWONDER_DURABLE_ROOT),
            production_root_verified: true,
            ship_gate_countable: true,
        }
    } else if resolved.starts_with(AIWONDER_HOT_ROOT) {
        MejepaDbPathClassification {
            resolved,
            kind: "aiwonder_hot",
            matched_root: Some(AIWONDER_HOT_ROOT),
            production_root_verified: true,
            ship_gate_countable: false,
        }
    } else if resolved.starts_with(AIWONDER_EXPLICIT_SCRATCH_ROOT) {
        MejepaDbPathClassification {
            resolved,
            kind: "aiwonder_explicit_scratch",
            matched_root: Some(AIWONDER_EXPLICIT_SCRATCH_ROOT),
            production_root_verified: false,
            ship_gate_countable: false,
        }
    } else {
        MejepaDbPathClassification {
            resolved,
            kind: "fixture_or_local",
            matched_root: None,
            production_root_verified: false,
            ship_gate_countable: false,
        }
    }
}

fn normalize_absolute_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

impl ToolErrorKind {
    /// Returns the JSON-RPC error code and human-readable label for this error kind.
    pub(crate) fn code_and_label(self) -> (i32, &'static str) {
        match self {
            Self::Validation => (error_codes::INVALID_PARAMS, "VALIDATION_ERROR"),
            Self::Storage => (error_codes::STORAGE_ERROR, "STORAGE_ERROR"),
            Self::NotFound => (error_codes::NODE_NOT_FOUND, "NOT_FOUND"),
            Self::Execution => (error_codes::INTERNAL_ERROR, "EXECUTION_ERROR"),
        }
    }
}

impl Handlers {
    /// MCP-compliant tool result helper.
    ///
    /// Wraps tool output in the required MCP format:
    /// ```json
    /// {
    ///   "content": [{"type": "text", "text": "..."}],
    ///   "isError": false
    /// }
    /// ```
    pub(crate) fn tool_result(
        &self,
        id: Option<JsonRpcId>,
        data: serde_json::Value,
    ) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
                }],
                "structuredContent": data,
                "isError": false
            }),
        )
    }

    /// MCP-compliant tool error with typed error category.
    ///
    /// Returns an MCP-compliant error response that includes the error code
    /// for machine-parseable error handling. Format:
    /// ```json
    /// {
    ///   "content": [{"type": "text", "text": "[LABEL -CODE] message"}],
    ///   "isError": true,
    ///   "errorCode": -32xxx
    /// }
    /// ```
    pub(crate) fn tool_error_typed(
        &self,
        id: Option<JsonRpcId>,
        kind: ToolErrorKind,
        message: &str,
    ) -> JsonRpcResponse {
        let (code, label) = kind.code_and_label();
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("[{} {}] {}", label, code, message)
                }],
                "isError": true,
                "errorCode": code
            }),
        )
    }

    pub(crate) fn tool_error_structured(
        &self,
        id: Option<JsonRpcId>,
        kind: ToolErrorKind,
        error_code: &str,
        message: &str,
        payload: serde_json::Value,
    ) -> JsonRpcResponse {
        let (code, label) = kind.code_and_label();
        let structured = json!({
            "error_code": error_code,
            "message": message,
            "payload": payload
        });
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("[{} {}] {}: {}", label, code, error_code, message)
                }],
                "structuredContent": structured,
                "isError": true,
                "errorCode": code
            }),
        )
    }

    /// MCP-compliant tool error helper (untyped convenience).
    ///
    /// For cases where the error category is obvious from context.
    /// Prefer `tool_error_typed` for new code to include the error code.
    pub(crate) fn tool_error(&self, id: Option<JsonRpcId>, message: &str) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": message
                }],
                "isError": true
            }),
        )
    }

    pub(crate) async fn append_audit_record_with_readback(
        &self,
        record: &AuditRecord,
        context: &str,
    ) -> Result<(), String> {
        self.teleological_store
            .append_audit_record(record)
            .await
            .map_err(|e| format!("{context}: append_audit_record failed: {e}"))?;

        let records = self
            .teleological_store
            .get_audit_by_target(record.target_id, 0)
            .await
            .map_err(|e| format!("{context}: audit readback failed: {e}"))?;
        if records.iter().any(|entry| entry.id == record.id) {
            Ok(())
        } else {
            Err(format!(
                "{context}: audit readback missing record {} for target {}",
                record.id, record.target_id
            ))
        }
    }

    pub(crate) async fn verify_deleted_fingerprint_readback(
        &self,
        fingerprint_id: Uuid,
        context: &str,
    ) -> Result<(), String> {
        match self.teleological_store.retrieve(fingerprint_id).await {
            Ok(None) => Ok(()),
            Ok(Some(_)) => Err(format!(
                "{context}: fingerprint {fingerprint_id} is still visible after delete"
            )),
            Err(e) => Err(format!(
                "{context}: fingerprint {fingerprint_id} delete readback failed: {e}"
            )),
        }
    }

    pub(crate) async fn verify_file_index_cleared_readback(
        &self,
        file_path: &str,
        context: &str,
    ) -> Result<(), String> {
        let ids = self
            .teleological_store
            .get_fingerprints_for_file(file_path)
            .await
            .map_err(|e| format!("{context}: file-index readback failed for {file_path}: {e}"))?;
        if ids.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{context}: file index for {file_path} still contains {} fingerprints",
                ids.len()
            ))
        }
    }

    pub(crate) async fn verify_importance_readbacks(
        &self,
        record: &ImportanceChangeRecord,
        context: &str,
    ) -> Result<(), String> {
        let Some(fingerprint) = self
            .teleological_store
            .retrieve(record.memory_id)
            .await
            .map_err(|e| {
                format!(
                    "{context}: fingerprint readback failed for {}: {e}",
                    record.memory_id
                )
            })?
        else {
            return Err(format!(
                "{context}: fingerprint {} missing after importance update",
                record.memory_id
            ));
        };
        if (fingerprint.importance - record.new_value).abs() > f32::EPSILON {
            return Err(format!(
                "{context}: fingerprint {} importance readback mismatch: expected {}, found {}",
                record.memory_id, record.new_value, fingerprint.importance
            ));
        }

        let history = self
            .teleological_store
            .get_importance_history(record.memory_id, 0)
            .await
            .map_err(|e| {
                format!(
                    "{context}: importance-history readback failed for {}: {e}",
                    record.memory_id
                )
            })?;
        if history.iter().any(|entry| {
            entry.timestamp == record.timestamp
                && (entry.old_value - record.old_value).abs() <= f32::EPSILON
                && (entry.new_value - record.new_value).abs() <= f32::EPSILON
                && (entry.delta - record.delta).abs() <= f32::EPSILON
                && entry.operator_id == record.operator_id
        }) {
            Ok(())
        } else {
            Err(format!(
                "{context}: importance-history readback missing row for {} at {}",
                record.memory_id, record.timestamp
            ))
        }
    }

    /// Parse JSON args into a typed DTO and run `validate() -> Result<(), String>`.
    ///
    /// Eliminates the repeated parse+validate boilerplate across all handler
    /// methods whose DTOs implement [`Validate`].
    ///
    /// Returns `Ok(request)` on success, or an MCP error `JsonRpcResponse`
    /// on parse/validation failure.
    #[allow(clippy::result_large_err)]
    pub(crate) fn parse_request<T: DeserializeOwned + Validate>(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
        tool_name: &str,
    ) -> Result<T, JsonRpcResponse> {
        let request: T = serde_json::from_value(args).map_err(|e| {
            tracing::error!("[{}] Invalid request: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        request.validate().map_err(|e| {
            tracing::error!("[{}] Validation failed: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        Ok(request)
    }

    /// Embed a query string using all 14 embedders and return the fingerprint.
    ///
    /// Eliminates the repeated embed+error-handling boilerplate across ~25 search
    /// handlers. Returns the `SemanticFingerprint` on success, or an MCP error
    /// `JsonRpcResponse` on embedding failure.
    pub(crate) async fn embed_query(
        &self,
        id: Option<JsonRpcId>,
        query: &str,
        tool_name: &str,
    ) -> Result<context_graph_core::types::fingerprint::SemanticFingerprint, JsonRpcResponse> {
        self.multi_array_provider
            .embed_all(query)
            .await
            .map(|output| output.fingerprint)
            .map_err(|e| {
                tracing::error!("[{}] Embedding failed: {}", tool_name, e);
                self.tool_error(id, &format!("Embedding failed: {}", e))
            })
    }

    /// Parse JSON args into a typed DTO and run `validate() -> Result<Output, String>`.
    ///
    /// Eliminates the repeated parse+validate boilerplate across all handler
    /// methods whose DTOs implement [`ValidateInto`] (i.e. validation produces
    /// a parsed value such as `Uuid`, `Vec<Uuid>`, or `(Uuid, Uuid)`).
    ///
    /// Returns `Ok((request, validated_output))` on success, or an MCP error
    /// `JsonRpcResponse` on parse/validation failure.
    #[allow(clippy::result_large_err)]
    pub(crate) fn parse_request_validated<T: DeserializeOwned + ValidateInto>(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
        tool_name: &str,
    ) -> Result<(T, T::Output), JsonRpcResponse> {
        let request: T = serde_json::from_value(args).map_err(|e| {
            tracing::error!("[{}] Invalid request: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        let output = request.validate().map_err(|e| {
            tracing::error!("[{}] Validation failed: {}", tool_name, e);
            self.tool_error(id.clone(), &format!("Invalid request: {}", e))
        })?;

        Ok((request, output))
    }
}

// =============================================================================
// SHARED MATH UTILITIES
// =============================================================================

/// Compute cosine similarity between two dense vectors.
///
/// LOW-15: Consolidated from 4 identical private implementations in
/// robustness_tools.rs, keyword_tools.rs, code_tools.rs, consolidation.rs.
///
/// Returns cosine similarity normalized to [0.0, 1.0] per SRC-3 convention.
/// Uses `(raw + 1) / 2` to map [-1, 1] → [0, 1], matching core retrieval pipeline.
/// Returns 0.5 if either vector is empty, lengths differ, or either has zero norm
/// (0.5 = orthogonal in normalized space).
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.5;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.5;
    }

    let raw = (dot / (norm_a * norm_b)).clamp(-1.0, 1.0);
    // SRC-3: Normalize [-1,1] → [0,1]
    (raw + 1.0) / 2.0
}

/// Compute variance of vector components (measures how spread out activations are).
///
/// Used by both `infer_graph_direction` (E8) and `infer_causal_direction` (E5)
/// to determine directional strength from asymmetric embeddings.
pub(crate) fn component_variance_f32(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let n = v.len() as f32;
    let mean = v.iter().sum::<f32>() / n;
    v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / n
}

/// Compute human-readable position label for sequence numbers.
///
/// MT-L1: Deduplicated from memory_tools.rs and sequence_tools.rs.
///
/// Returns labels like:
/// - "current turn" (same sequence)
/// - "previous turn" (1 turn ago)
/// - "2 turns ago" (2 turns ago)
/// - "N turns ago" (N turns ago)
/// - "next turn" / "N turns ahead" (if result_seq > current_seq)
///
/// # Arguments
/// * `result_seq` - The session sequence of the result
/// * `current_seq` - The current session sequence
pub(crate) fn compute_position_label(result_seq: u64, current_seq: u64) -> String {
    if result_seq == current_seq {
        "current turn".to_string()
    } else if result_seq < current_seq {
        let turns_ago = current_seq - result_seq;
        if turns_ago == 1 {
            "previous turn".to_string()
        } else {
            format!("{} turns ago", turns_ago)
        }
    } else {
        // Future turn (shouldn't normally happen, but handle gracefully)
        let turns_ahead = result_seq - current_seq;
        if turns_ahead == 1 {
            "next turn".to_string()
        } else {
            format!("{} turns ahead", turns_ahead)
        }
    }
}

/// Compute importance decay factor based on time since last access.
///
/// Uses exponential half-life decay: `factor = 0.5^(days_since_access / half_life_days)`
///
/// - half_life_days = 30 (importance halves every 30 days without access)
/// - factor is clamped to [0.01, 1.0] (never fully zeroes out)
///
/// # Arguments
/// * `last_accessed_at` - When memory was last accessed
/// * `now` - Current time
#[allow(dead_code)] // Infrastructure for scoring-time importance decay
pub fn importance_decay_factor(
    last_accessed_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> f32 {
    const HALF_LIFE_DAYS: f64 = 30.0;
    const MIN_DECAY_FACTOR: f32 = 0.01;

    let days_since_access = (now - last_accessed_at).num_seconds().max(0) as f64 / 86400.0;
    let factor = (0.5f64).powf(days_since_access / HALF_LIFE_DAYS) as f32;
    factor.clamp(MIN_DECAY_FACTOR, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mejepa_db_source_of_truth_marks_fixture_paths_not_gate_countable() {
        let source = mejepa_db_source_of_truth(
            Path::new("/tmp/contextgraph-fixture/mistakes.rocksdb"),
            json!({"cf": "CF_MEJEPA_MISTAKE_LOG"}),
        );

        assert_eq!(source["sourceOfTruthKind"], json!("fixture_or_local"));
        assert_eq!(source["productionRootVerified"], json!(false));
        assert_eq!(source["fixtureOrLocal"], json!(true));
        assert_eq!(source["shipGateCountable"], json!(false));
        assert_eq!(source["cf"], json!("CF_MEJEPA_MISTAKE_LOG"));
    }

    #[test]
    fn mejepa_db_source_of_truth_marks_aiwonder_durable_gate_countable() {
        let source = mejepa_db_source_of_truth(
            Path::new("/zfs/archive/contextgraph/state/mejepa/infer.rocksdb"),
            json!({}),
        );

        assert_eq!(source["sourceOfTruthKind"], json!("aiwonder_durable"));
        assert_eq!(source["matchedAiwonderRoot"], json!(AIWONDER_DURABLE_ROOT));
        assert_eq!(source["productionRootVerified"], json!(true));
        assert_eq!(source["fixtureOrLocal"], json!(false));
        assert_eq!(source["shipGateCountable"], json!(true));
    }

    #[test]
    fn mejepa_db_source_of_truth_marks_aiwonder_hot_not_gate_countable() {
        let source = mejepa_db_source_of_truth(
            Path::new("/zfs/hot/contextgraph/runtime/infer.rocksdb"),
            json!({}),
        );

        assert_eq!(source["sourceOfTruthKind"], json!("aiwonder_hot"));
        assert_eq!(source["matchedAiwonderRoot"], json!(AIWONDER_HOT_ROOT));
        assert_eq!(source["productionRootVerified"], json!(true));
        assert_eq!(source["fixtureOrLocal"], json!(false));
        assert_eq!(source["shipGateCountable"], json!(false));
    }
}
