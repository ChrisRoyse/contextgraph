//! ME-JEPA UTML MCP handlers.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use context_graph_mejepa::{
    open_mincut_rocksdb, pairwise_mi_corpus_hash, read_pairwise_mi_matrix,
    summarize_pairwise_mi_matrix, write_pairwise_mi_matrix_sync_readback, MejepaInferError,
    PAIRWISE_MI_ESTIMATOR_PARTITIONED_NMI,
};
use context_graph_mejepa_cf::CF_MEJEPA_PAIRWISE_MI;
use context_graph_mejepa_train::learning_signal::PairwiseMiAuditor;
use context_graph_mejepa_train::learning_signal::{UtmlError, UtmlErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PairwiseMiAuditRequest {
    output_dir: PathBuf,
    step: u64,
    #[serde(default = "default_period_steps")]
    period_steps: u64,
    series_by_slot: BTreeMap<String, Vec<f32>>,
    #[serde(default)]
    db_path: Option<PathBuf>,
    #[serde(default)]
    persist_to_cf: bool,
    #[serde(default)]
    corpus_shard_hash: Option<String>,
    #[serde(default)]
    created_at_unix_ms: Option<i64>,
}

fn default_period_steps() -> u64 {
    1
}

impl PairwiseMiAuditRequest {
    fn validate(&self) -> Result<(), String> {
        if self.period_steps == 0 {
            return Err("periodSteps must be greater than zero".to_string());
        }
        if self.series_by_slot.len() < 2 {
            return Err("seriesBySlot must contain at least two slots".to_string());
        }
        for (slot, values) in &self.series_by_slot {
            if slot.trim().is_empty() {
                return Err("seriesBySlot contains an empty slot name".to_string());
            }
            if values.len() < 2 {
                return Err(format!(
                    "seriesBySlot.{slot} must contain at least two samples"
                ));
            }
        }
        if let Some(db_path) = &self.db_path {
            if db_path.as_os_str().is_empty() {
                return Err("dbPath must be a non-empty path".to_string());
            }
        }
        if self.persist_to_cf && self.db_path.is_none() {
            return Err("dbPath is required when persistToCf is true".to_string());
        }
        if let Some(hash) = &self.corpus_shard_hash {
            if hash.len() != 64 || !hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return Err("corpusShardHash must be a 64-character hex hash".to_string());
            }
        }
        if self.created_at_unix_ms.is_some_and(|value| value <= 0) {
            return Err("createdAtUnixMs must be positive".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PairwiseMiCfEvidence {
    write: context_graph_mejepa::PairwiseMiCfWriteSummary,
    readback_rows: usize,
    persisted_matrix_equal: bool,
    corpus_shard_hash: String,
    created_at_unix_ms: i64,
}

impl Handlers {
    pub(crate) async fn call_mejepa_audit_pairwise_mi(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: PairwiseMiAuditRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(
                    error_code = "MEJEPA_AUDIT_PAIRWISE_MI_SCHEMA_INVALID",
                    error = %err,
                    tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                    "pairwise MI MCP request schema validation failed"
                );
                return self.tool_error_structured(
                    id,
                    ToolErrorKind::Validation,
                    "MEJEPA_AUDIT_PAIRWISE_MI_SCHEMA_INVALID",
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_AUDIT_PAIRWISE_MI
                    ),
                    json!({"tool": tool_names::MEJEPA_AUDIT_PAIRWISE_MI}),
                );
            }
        };
        if let Err(err) = request.validate() {
            tracing::error!(
                error_code = "MEJEPA_AUDIT_PAIRWISE_MI_INVALID_INPUT",
                error = %err,
                tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                "pairwise MI MCP request validation failed"
            );
            return self.tool_error_structured(
                id,
                ToolErrorKind::Validation,
                "MEJEPA_AUDIT_PAIRWISE_MI_INVALID_INPUT",
                &err,
                json!({"tool": tool_names::MEJEPA_AUDIT_PAIRWISE_MI}),
            );
        }

        let output_dir =
            match context_graph_paths::require_under_data_root(&request.output_dir, "outputDir") {
                Ok(path) => path,
                Err(err) => {
                    tracing::error!(
                        error_code = err.code,
                        error = %err,
                        output_dir = %request.output_dir.display(),
                        tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                        "pairwise MI output path rejected"
                    );
                    return self.tool_error_structured(
                        id,
                        ToolErrorKind::Validation,
                        err.code,
                        &err.to_string(),
                        json!({"outputDir": request.output_dir}),
                    );
                }
            };

        let result = run_pairwise_mi_audit(&request, output_dir.clone());

        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                tracing::error!(
                    error_code = err.code(),
                    error = %err,
                    output_dir = %output_dir.display(),
                    step = request.step,
                    tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                    "pairwise MI audit failed"
                );
                self.tool_error_structured(
                    id,
                    ToolErrorKind::Execution,
                    err.code(),
                    &err.to_string(),
                    json!({
                        "outputDir": output_dir,
                        "step": request.step,
                        "sourceOfTruth": "pairwise_mi_csv"
                    }),
                )
            }
        }
    }
}

fn run_pairwise_mi_audit(
    request: &PairwiseMiAuditRequest,
    output_dir: PathBuf,
) -> Result<serde_json::Value, context_graph_mejepa_train::learning_signal::UtmlError> {
    let auditor = PairwiseMiAuditor::new(request.period_steps, &output_dir)?;
    let should_run = auditor.should_run(request.step);
    let summary = auditor.run_from_slot_series(&request.series_by_slot, request.step)?;
    let health = summarize_pairwise_mi_matrix(&summary.matrix.slots, &summary.matrix.values)
        .map_err(infer_to_utml)?;
    let persist_to_cf = request.persist_to_cf || request.db_path.is_some();
    let cf_evidence = if persist_to_cf {
        let db_path = request.db_path.as_ref().ok_or_else(|| {
            UtmlError::new(
                UtmlErrorCode::MissingSourceOfTruth,
                "dbPath is required when persisting pairwise MI to CF",
            )
        })?;
        let corpus_shard_hash = match &request.corpus_shard_hash {
            Some(hash) => hash.clone(),
            None => pairwise_mi_corpus_hash(&summary.matrix.slots, &summary.matrix.values)
                .map_err(infer_to_utml)?,
        };
        let created_at_unix_ms = request
            .created_at_unix_ms
            .map(Ok)
            .unwrap_or_else(now_unix_ms)?;
        let db = open_mincut_rocksdb(db_path).map_err(infer_to_utml)?;
        let write = write_pairwise_mi_matrix_sync_readback(
            db.as_ref(),
            &corpus_shard_hash,
            &summary.matrix.slots,
            &summary.matrix.values,
            request.step,
            summary.sample_count,
            created_at_unix_ms,
            PAIRWISE_MI_ESTIMATOR_PARTITIONED_NMI,
        )
        .map_err(infer_to_utml)?;
        let readback = read_pairwise_mi_matrix(
            db.as_ref(),
            Some(&corpus_shard_hash),
            Some(created_at_unix_ms),
            context_graph_mejepa::DEFAULT_PAIRWISE_MI_READ_MAX_ROWS,
        )
        .map_err(infer_to_utml)?;
        let persisted_matrix_equal =
            readback.slots == summary.matrix.slots && readback.values == summary.matrix.values;
        if !persisted_matrix_equal {
            return Err(UtmlError::new(
                UtmlErrorCode::ReadbackMismatch,
                "CF_MEJEPA_PAIRWISE_MI matrix readback differs from audit matrix",
            ));
        }
        Some(PairwiseMiCfEvidence {
            write,
            readback_rows: readback.source_row_count,
            persisted_matrix_equal,
            corpus_shard_hash,
            created_at_unix_ms,
        })
    } else {
        None
    };
    Ok(json!({
        "step": summary.step,
        "periodSteps": request.period_steps,
        "shouldRunAtStep": should_run,
        "sampleCount": summary.sample_count,
        "slotCount": summary.matrix.slots.len(),
        "slots": summary.matrix.slots,
        "maxOffDiagonal": summary.max_off_diagonal,
        "meanOffDiagonal": summary.mean_off_diagonal,
        "effectiveSignalCount": health.effective_signal_count,
        "redundancyHistogram": health.redundancy_histogram,
        "adaptiveWeights": health.adaptive_weights,
        "csvPath": summary.path,
        "readbackPath": summary.readback_path,
        "matrix": summary.matrix.values,
        "cfEvidence": cf_evidence,
        "sourceOfTruth": if persist_to_cf { CF_MEJEPA_PAIRWISE_MI } else { "pairwise_mi_csv" },
    }))
}

fn infer_to_utml(err: MejepaInferError) -> UtmlError {
    UtmlError::new(UtmlErrorCode::InvalidSignal, err.to_string())
}

fn now_unix_ms() -> Result<i64, UtmlError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| UtmlError::new(UtmlErrorCode::Io, format!("system clock error: {err}")))?;
    Ok(elapsed.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pairwise_mi_audit_writes_and_reads_csv_source_of_truth() {
        let root = context_graph_paths::ensure_subdir("fsv/mcp-pairwise-mi-unit").unwrap();
        let output_dir = root.join("audit");
        let _ = fs::remove_dir_all(&output_dir);
        let request = PairwiseMiAuditRequest {
            output_dir: output_dir.clone(),
            step: 42,
            period_steps: 7,
            series_by_slot: BTreeMap::from([
                ("e_ast".to_string(), vec![0.1, 0.2, 0.3, 0.4]),
                ("e_diff".to_string(), vec![0.4, 0.3, 0.2, 0.1]),
                ("e_test".to_string(), vec![0.1, 0.1, 0.9, 0.9]),
            ]),
            db_path: None,
            persist_to_cf: false,
            corpus_shard_hash: None,
            created_at_unix_ms: None,
        };
        request.validate().unwrap();
        let output = run_pairwise_mi_audit(&request, output_dir).unwrap();
        assert_eq!(output["sourceOfTruth"], json!("pairwise_mi_csv"));
        assert_eq!(output["step"], json!(42));
        assert_eq!(output["shouldRunAtStep"], json!(true));
        let csv_path = output["csvPath"].as_str().unwrap();
        let text = fs::read_to_string(csv_path).unwrap();
        assert!(text.starts_with("slot,e_ast,e_diff,e_test"));
        let readback =
            context_graph_mejepa_train::learning_signal::load_pairwise_mi_matrix_csv(csv_path)
                .unwrap();
        assert_eq!(readback.slots.len(), 3);
        assert_eq!(readback.values.len(), 3);
    }

    #[test]
    fn pairwise_mi_audit_rejects_empty_slot_series() {
        let request = PairwiseMiAuditRequest {
            output_dir: PathBuf::from("/zfs/archive/contextgraph/fsv/mcp-pairwise-mi-unit/edge"),
            step: 1,
            period_steps: 1,
            series_by_slot: BTreeMap::from([("e_ast".to_string(), vec![0.1])]),
            db_path: None,
            persist_to_cf: false,
            corpus_shard_hash: None,
            created_at_unix_ms: None,
        };
        assert!(request.validate().unwrap_err().contains("at least two"));
    }

    #[test]
    fn pairwise_mi_audit_persists_cf_readback_when_db_path_supplied() {
        let root = context_graph_paths::ensure_subdir("fsv/mcp-pairwise-mi-cf-unit").unwrap();
        let output_dir = root.join("audit");
        let db_path = root.join("rocksdb");
        let _ = fs::remove_dir_all(&output_dir);
        let _ = fs::remove_dir_all(&db_path);
        let request = PairwiseMiAuditRequest {
            output_dir: output_dir.clone(),
            step: 43,
            period_steps: 1,
            series_by_slot: BTreeMap::from([
                ("e_ast".to_string(), vec![0.0, 0.0, 1.0, 1.0]),
                ("e_diff".to_string(), vec![0.0, 0.0, 1.0, 1.0]),
                ("e_test".to_string(), vec![1.0, 0.0, 1.0, 0.0]),
            ]),
            db_path: Some(db_path),
            persist_to_cf: true,
            corpus_shard_hash: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            created_at_unix_ms: Some(1_779_000_000_000),
        };
        request.validate().unwrap();
        let output = run_pairwise_mi_audit(&request, output_dir).unwrap();
        assert_eq!(output["sourceOfTruth"], json!(CF_MEJEPA_PAIRWISE_MI));
        assert_eq!(output["cfEvidence"]["write"]["rowsWritten"], json!(3));
        assert_eq!(output["cfEvidence"]["persistedMatrixEqual"], json!(true));
        assert!(output["effectiveSignalCount"].as_f64().unwrap() > 0.0);
    }
}
