//! ME-JEPA Phase 8 evaluation MCP handlers.

use std::path::{Path, PathBuf};

use context_graph_mejepa::eval::{
    build_eval_compiler, build_patch_similarity_graph, corpus_sha_from_holdout, synthetic_holdout,
    synthetic_patch_embeddings, validate_active_python_ship_gate_report, write_weekly_report,
    EvalConfig, EvalProvenance, EvalRunner, RocksDbEvalStore, ACTIVE_PYTHON_SHIP_GATE_CELL_COUNT,
    ACTIVE_PYTHON_SHIP_GATE_GRID, ACTIVE_PYTHON_SHIP_GATE_NAME,
    FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER, SHIP_GATE_STABILITY_BLOCKER,
};
use context_graph_mejepa::{open_infer_rocksdb, ActiveLearningRankBy};
use serde::Deserialize;
use serde_json::json;

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvalRunRequest {
    db_path: PathBuf,
    repo_root: PathBuf,
    output_fsv: PathBuf,
    report_date: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvalDbRequest {
    db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ActiveLearningRankByRequest {
    SchedulerPriority,
    Curiosity,
}

impl From<ActiveLearningRankByRequest> for ActiveLearningRankBy {
    fn from(value: ActiveLearningRankByRequest) -> Self {
        match value {
            ActiveLearningRankByRequest::SchedulerPriority => Self::SchedulerPriority,
            ActiveLearningRankByRequest::Curiosity => Self::Curiosity,
        }
    }
}

fn default_active_learning_rank_by() -> ActiveLearningRankByRequest {
    ActiveLearningRankByRequest::SchedulerPriority
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActiveLearningQueueRequest {
    db_path: PathBuf,
    #[serde(default = "default_active_learning_rank_by")]
    ranked_by: ActiveLearningRankByRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvalBuildGraphRequest {
    db_path: PathBuf,
    output_fsv: PathBuf,
    #[serde(default = "default_threshold")]
    threshold: f32,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

fn default_threshold() -> f32 {
    0.85
}

fn default_top_k() -> usize {
    3
}

impl Handlers {
    pub(crate) async fn call_mejepa_eval_run(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: EvalRunRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_EVAL_RUN
                    ),
                );
            }
        };
        let result: Result<serde_json::Value, context_graph_mejepa::EvalError> = (|| {
            let (_db, compiler, eval_store, calibration) =
                build_eval_compiler(&request.db_path, &request.repo_root)?;
            let holdout = synthetic_holdout(&request.repo_root)?;
            let provenance = EvalProvenance {
                corpus_sha: corpus_sha_from_holdout(&holdout),
                eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
                calibration_version: calibration.load_active()?.version.clone(),
                generated_by: tool_names::MEJEPA_EVAL_RUN.to_string(),
            };
            let runner = EvalRunner::new(compiler, eval_store.clone(), EvalConfig::default())?;
            let report = runner.run_holdout_eval(&holdout, &[], request.report_date, provenance)?;
            let report_path = request.output_fsv.join("mcp-weekly-eval-report.json");
            write_weekly_report(&report_path, &report)?;
            let latest = eval_store.load_latest_report()?.ok_or_else(|| {
                context_graph_mejepa::EvalError::new(
                    context_graph_mejepa::EvalErrorCode::ReportPersistFail,
                    "latest eval report missing after MCP eval-run",
                )
            })?;
            let persisted_hash = latest.determinism_hash()?;
            let report_hash = report.determinism_hash()?;
            if persisted_hash != report_hash {
                return Err(context_graph_mejepa::EvalError::new(
                    context_graph_mejepa::EvalErrorCode::ReadbackMismatch,
                    "MCP eval-run persisted report hash mismatch",
                ));
            }
            Ok(json!({
                "shipGatePassed": report.ship_gate_passed,
                "shipGateFailures": report.ship_gate_failures,
                "reportHash": report_hash,
                "reportPath": report_path,
                "holdoutCount": report.holdout_count,
                "sourceOfTruth": "CF_MEJEPA_EVAL_REPORTS"
            }))
        })();
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }

    pub(crate) async fn call_mejepa_ship_gate_status(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: EvalDbRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_SHIP_GATE_STATUS
                    ),
                );
            }
        };
        let result = mejepa_ship_gate_status_payload(&request.db_path);
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }

    pub(crate) async fn call_mejepa_active_learning_queue(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: ActiveLearningQueueRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_ACTIVE_LEARNING_QUEUE
                    ),
                );
            }
        };
        let result: Result<serde_json::Value, context_graph_mejepa::EvalError> = (|| {
            let db = open_infer_rocksdb(&request.db_path)?;
            let eval_store = RocksDbEvalStore::new(db)?;
            let queue = eval_store.load_queue()?.ok_or_else(|| {
                context_graph_mejepa::EvalError::new(
                    context_graph_mejepa::EvalErrorCode::ReadbackMismatch,
                    "active-learning queue missing in CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
                )
            })?;
            let ranked_by = ActiveLearningRankBy::from(request.ranked_by);
            let entries = queue
                .ranked_entries(ranked_by)
                .into_iter()
                .map(|entry| {
                    json!({
                        "taskId": entry.task_id.0.clone(),
                        "score": entry.score,
                        "outcomeSetLen": entry.outcome_set_len,
                        "oodScore": entry.ood_score,
                        "curiosityScore": entry.curiosity_score,
                        "reason": entry.reason.clone(),
                        "kind": entry.kind.clone(),
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "capacity": queue.capacity,
                "queuedCount": queue.entries.len(),
                "evictedCount": queue.evicted.len(),
                "oodEscalationCount": queue.ood_escalations.len(),
                "rankedBy": ranked_by,
                "entries": entries,
                "sourceOfTruth": "CF_MEJEPA_ACTIVE_LEARNING_QUEUE"
            }))
        })();
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }

    pub(crate) async fn call_mejepa_eval_build_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: EvalBuildGraphRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_EVAL_BUILD_GRAPH
                    ),
                );
            }
        };
        let result: Result<serde_json::Value, context_graph_mejepa::EvalError> = (|| {
            let db = open_infer_rocksdb(&request.db_path)?;
            let eval_store = RocksDbEvalStore::new(db)?;
            let graph = build_patch_similarity_graph(
                &synthetic_patch_embeddings(),
                request.threshold,
                request.top_k,
            )?;
            eval_store.persist_graph(&graph)?;
            let readback = eval_store.load_graph()?.ok_or_else(|| {
                context_graph_mejepa::EvalError::new(
                    context_graph_mejepa::EvalErrorCode::ReadbackMismatch,
                    "patch graph missing after persist",
                )
            })?;
            if readback.node_count != graph.node_count || readback.edge_count != graph.edge_count {
                return Err(context_graph_mejepa::EvalError::new(
                    context_graph_mejepa::EvalErrorCode::ReadbackMismatch,
                    "patch graph readback differs",
                ));
            }
            let path = request.output_fsv.join("mcp-patch-similarity-graph.json");
            context_graph_mejepa::eval::report::write_json_0600(&path, &graph)?;
            Ok(json!({
                "nodeCount": graph.node_count,
                "edgeCount": graph.edge_count,
                "graphPath": path,
                "sourceOfTruth": "CF_MEJEPA_TASK_GRAPH"
            }))
        })();
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }
}

pub fn mejepa_ship_gate_status_payload(
    db_path: &Path,
) -> Result<serde_json::Value, context_graph_mejepa::EvalError> {
    let exemptions_path = context_graph_mejepa::eval::default_cell_exemptions_path();
    mejepa_ship_gate_status_payload_with_exemptions(db_path, &exemptions_path)
}

pub fn mejepa_ship_gate_status_payload_with_exemptions(
    db_path: &Path,
    exemptions_path: &Path,
) -> Result<serde_json::Value, context_graph_mejepa::EvalError> {
    let db = open_infer_rocksdb(db_path)?;
    let eval_store = RocksDbEvalStore::new(db)?;
    let report = eval_store.load_latest_report()?.ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::ReportPersistFail,
            "no eval report exists in CF_MEJEPA_EVAL_REPORTS",
        )
    })?;
    validate_active_python_ship_gate_report(&report)?;
    let cell_exemptions = context_graph_mejepa::eval::load_cell_exemptions(
        exemptions_path,
        chrono::Utc::now().timestamp_millis(),
    )?;
    let stability = context_graph_mejepa::eval::ship_gate_stability_status_with_exemptions(
        &eval_store,
        &cell_exemptions,
    )?;
    let fingerprint_stability =
        context_graph_mejepa::eval::fingerprint_ship_gate_stability_status(&eval_store)?;
    let mut ship_gate_failures =
        context_graph_mejepa::eval::non_exempt_ship_gate_failures(&report, &cell_exemptions);
    if !stability.ready
        && !ship_gate_failures
            .iter()
            .any(|failure| failure.starts_with(SHIP_GATE_STABILITY_BLOCKER))
    {
        ship_gate_failures.push(format!(
            "{SHIP_GATE_STABILITY_BLOCKER}: consecutive_passing_windows={}/{}",
            stability.consecutive_passing_windows, stability.required_consecutive_windows
        ));
    }
    let mut fingerprint_failures = fingerprint_stability.latest_failures.clone();
    if !fingerprint_stability.ready
        && !fingerprint_failures
            .iter()
            .any(|failure| failure.starts_with(FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER))
    {
        fingerprint_failures.push(format!(
            "{FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER}: consecutive_passing_windows={}/{}",
            fingerprint_stability.consecutive_passing_windows,
            fingerprint_stability.required_consecutive_windows
        ));
    }
    ship_gate_failures.extend(fingerprint_failures.clone());
    let cell_ship_gate_passed = stability.ready;
    let fingerprint_ship_gate_passed = fingerprint_stability.ready;
    Ok(json!({
        "shipGatePassed": cell_ship_gate_passed && fingerprint_ship_gate_passed,
        "cellShipGatePassed": cell_ship_gate_passed,
        "fingerprintShipGatePassed": fingerprint_ship_gate_passed,
        "latestWindowShipGatePassed": stability.latest_report_passed_window,
        "latestWindowRawShipGatePassed": report.ship_gate_passed,
        "latestWindowPassedStability": stability.latest_report_passed_window,
        "latestWindowCorrelation": stability.latest_report_correlation,
        "effectiveShipGateCorrelation": stability.latest_effective_correlation,
        "consecutivePassingWindows": stability.consecutive_passing_windows,
        "requiredConsecutivePassingWindows": stability.required_consecutive_windows,
        "shipGateCorrelationThreshold": stability.correlation_threshold,
        "shipGateFailures": ship_gate_failures,
        "fingerprintShipGate": fingerprint_stability,
        "fingerprintShipGateFailures": fingerprint_failures,
        "stabilityResetReason": stability.latest_reset_reason,
        "stabilityResetUnixMs": stability.latest_reset_unix_ms,
        "modelPromotionResetCount": stability.model_promotion_reset_count,
        "cellExemptionsPath": exemptions_path,
        "cellExemptions": cell_exemptions,
        "activeGate": ACTIVE_PYTHON_SHIP_GATE_NAME,
        "requiredGrid": ACTIVE_PYTHON_SHIP_GATE_GRID,
        "requiredCellCount": ACTIVE_PYTHON_SHIP_GATE_CELL_COUNT,
        "perCellCorrelation": report.per_cell_correlation.clone(),
        "reportHash": report.determinism_hash()?,
        "holdoutCount": report.holdout_count,
        "perCellStateTransfer": report.per_cell_state_transfer.clone(),
        "perCellConvergenceEta": report.per_cell_convergence_eta.clone(),
        "lowStateTransferCells": low_state_transfer_cells(&report),
        "perPredictionClassCalibration": report.per_prediction_class_calibration.clone(),
        "perFailureModeClass": report.per_failure_mode_class.clone(),
        "sourceOfTruth": "CF_MEJEPA_EVAL_REPORTS",
        "fingerprintSourceOfTruth": "CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS"
    }))
}

fn low_state_transfer_cells(report: &context_graph_mejepa::EvalReport) -> Vec<String> {
    report
        .per_cell_state_transfer
        .iter()
        .filter_map(|(cell, diagnostic)| {
            diagnostic
                .as_ref()
                .filter(|value| value.transfer_score < 0.80)
                .map(|_| cell.clone())
        })
        .collect()
}
