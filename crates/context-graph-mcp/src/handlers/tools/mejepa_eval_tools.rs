//! ME-JEPA Phase 8 evaluation MCP handlers.

use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa::eval::{
    build_eval_compiler, build_patch_similarity_graph, corpus_sha_from_holdout, synthetic_holdout,
    synthetic_patch_embeddings, write_weekly_report, EvalConfig, EvalProvenance, EvalRunner,
    RocksDbEvalStore,
};
use context_graph_mejepa::{open_infer_rocksdb, FeedbackKind, HoldoutPanel, SurpriseEvent};
use rocksdb::{IteratorMode, DB};
use serde::Deserialize;
use serde_json::{json, Value};

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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WeeklyEvalDashboardRequest {
    db_path: PathBuf,
    d_root: Option<PathBuf>,
    exports_root: Option<PathBuf>,
    #[serde(default = "default_dashboard_max_cells")]
    max_cells: usize,
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

const DEFAULT_DASHBOARD_MAX_CELLS: usize = 64;
const DASHBOARD_MAX_CELLS_CAP: usize = 500;
const WEEKLY_CELL_CORRELATION_MIN: f32 = 0.25;
const WEEKLY_STATE_TRANSFER_MIN: f32 = 0.80;

fn default_dashboard_max_cells() -> usize {
    DEFAULT_DASHBOARD_MAX_CELLS
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
        let result: Result<serde_json::Value, context_graph_mejepa::EvalError> = (|| {
            let db = open_infer_rocksdb(&request.db_path)?;
            let eval_store = RocksDbEvalStore::new(db)?;
            let report = eval_store.load_latest_report()?.ok_or_else(|| {
                context_graph_mejepa::EvalError::new(
                    context_graph_mejepa::EvalErrorCode::ReportPersistFail,
                    "no eval report exists in CF_MEJEPA_EVAL_REPORTS",
                )
            })?;
            Ok(json!({
                "shipGatePassed": report.ship_gate_passed,
                "shipGateFailures": report.ship_gate_failures,
                "reportHash": report.determinism_hash()?,
                "holdoutCount": report.holdout_count,
                "perCellStateTransfer": report.per_cell_state_transfer,
                "lowStateTransferCells": low_state_transfer_cells(&report),
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

    pub(crate) async fn call_mejepa_weekly_eval_dashboard(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: WeeklyEvalDashboardRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_WEEKLY_EVAL_DASHBOARD
                    ),
                );
            }
        };
        if let Err(message) = validate_dashboard_request(&request) {
            return self.tool_error_typed(id, ToolErrorKind::Validation, &message);
        }
        let result = weekly_eval_dashboard(request);
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
        let request: EvalDbRequest = match serde_json::from_value(args) {
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
            Ok(json!({
                "capacity": queue.capacity,
                "queuedCount": queue.entries.len(),
                "evictedCount": queue.evicted.len(),
                "oodEscalationCount": queue.ood_escalations.len(),
                "entries": queue.entries,
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

fn validate_dashboard_request(request: &WeeklyEvalDashboardRequest) -> Result<(), String> {
    if request.db_path.as_os_str().is_empty() {
        return Err("dbPath must be a non-empty path".to_string());
    }
    if let Some(exports_root) = &request.exports_root {
        if exports_root.as_os_str().is_empty() {
            return Err("exportsRoot must be a non-empty path".to_string());
        }
    }
    if let Some(d_root) = &request.d_root {
        if d_root.as_os_str().is_empty() {
            return Err("dRoot must be a non-empty path".to_string());
        }
    }
    if request.max_cells == 0 || request.max_cells > DASHBOARD_MAX_CELLS_CAP {
        return Err(format!("maxCells must be in 1..={DASHBOARD_MAX_CELLS_CAP}"));
    }
    Ok(())
}

fn weekly_eval_dashboard(
    request: WeeklyEvalDashboardRequest,
) -> Result<Value, context_graph_mejepa::EvalError> {
    let db =
        context_graph_mejepa_hygiene::open_hygiene_rocksdb(&request.db_path).map_err(|err| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::Store,
                format!("open weekly dashboard RocksDB failed: {err}"),
            )
        })?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let report = eval_store.load_latest_report()?.ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::ReportPersistFail,
            "no weekly eval report exists in CF_MEJEPA_EVAL_REPORTS",
        )
    })?;
    report.validate()?;
    let report_hash = report.determinism_hash()?;
    let exports_root = resolve_exports_root(request.exports_root)?;
    let d_root = resolve_d_root(request.d_root, &exports_root)?;
    let weekly_files = weekly_file_status(&exports_root, &report, &report_hash);
    let weekly_files_status = weekly_files
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let holdout_feed = weekly_holdout_feed_status(&d_root);
    let holdout_feed_status = holdout_feed
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let feedback = agent_feedback_summary(db.as_ref(), 5)?;
    let overall_status = if weekly_files_status == "present"
        && holdout_feed_status == "ready"
        && report.ship_gate_passed
    {
        "healthy"
    } else if weekly_files_status == "present" || holdout_feed_status == "ready" {
        "attention_required"
    } else {
        "degraded"
    };
    Ok(json!({
        "tool": tool_names::MEJEPA_WEEKLY_EVAL_DASHBOARD,
        "overallStatus": overall_status,
        "report": {
            "reportDate": report.report_date,
            "generatedAtUnixMs": report.generated_at_unix_ms,
            "holdoutCount": report.holdout_count,
            "shipGatePassed": report.ship_gate_passed,
            "shipGateFailures": report.ship_gate_failures,
            "reportHash": report_hash,
            "sourceOfTruth": "CF_MEJEPA_EVAL_REPORTS"
        },
        "holdoutFeed": holdout_feed,
        "perCellShipGate": {
            "rows": per_cell_dashboard_rows(&report, request.max_cells),
            "totalCells": report.per_cell_correlation.len(),
            "maxCells": request.max_cells,
            "truncated": report.per_cell_correlation.len() > request.max_cells
        },
        "calibration": {
            "conformalCoverage": report.conformal_coverage_health,
            "oodCalibration": report.ood_calibration_health,
            "gtauPassRate": report.gtau_pass_rate
        },
        "activeLearning": report.active_learning,
        "agentFeedback": feedback,
        "operationalSignals": operational_signal_counts(db.as_ref())?,
        "weeklyFiles": weekly_files,
        "requiredWeeklySections": required_weekly_sections_status(&exports_root, &report.report_date),
        "runbook": runbook_actions(&report, weekly_files_status, holdout_feed_status),
        "sourceOfTruth": {
            "evalReport": "CF_MEJEPA_EVAL_REPORTS",
            "holdoutFeed": d_root.join("state/gold-labels/weekly_holdout_panels.json"),
            "exportsRoot": exports_root,
            "agentFeedback": context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
            "driftHistory": context_graph_mejepa_cf::CF_MEJEPA_DRIFT_HISTORY
        }
    }))
}

fn resolve_exports_root(
    input: Option<PathBuf>,
) -> Result<PathBuf, context_graph_mejepa::EvalError> {
    if let Some(path) = input {
        return Ok(path);
    }
    context_graph_paths::data_root()
        .map(|root| root.join("exports/eval"))
        .map_err(|err| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::InvalidInput,
                err.to_string(),
            )
        })
}

fn resolve_d_root(
    input: Option<PathBuf>,
    exports_root: &Path,
) -> Result<PathBuf, context_graph_mejepa::EvalError> {
    if let Some(path) = input {
        return Ok(path);
    }
    if exports_root.file_name().is_some_and(|name| name == "eval")
        && exports_root
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "exports")
    {
        if let Some(root) = exports_root.parent().and_then(Path::parent) {
            return Ok(root.to_path_buf());
        }
    }
    context_graph_paths::data_root().map_err(|err| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::InvalidInput,
            err.to_string(),
        )
    })
}

fn per_cell_dashboard_rows(
    report: &context_graph_mejepa::EvalReport,
    max_cells: usize,
) -> Vec<Value> {
    report
        .per_cell_correlation
        .iter()
        .take(max_cells)
        .map(|(cell, correlation)| {
            let state_transfer = report
                .per_cell_state_transfer
                .get(cell)
                .and_then(Option::as_ref);
            let correlation_status = match correlation {
                Some(value) if *value >= WEEKLY_CELL_CORRELATION_MIN => "pass",
                Some(_) => "below_threshold",
                None => "insufficient_samples",
            };
            let state_transfer_candidate = match state_transfer {
                Some(value) if value.transfer_score < WEEKLY_STATE_TRANSFER_MIN => "fine_tune",
                Some(_) => "hold",
                None => "collect_more_labels",
            };
            json!({
                "cell": cell,
                "correlation": correlation,
                "bayesianShrinkage": report.bayesian_shrinkage.get(cell),
                "stateTransfer": state_transfer,
                "correlationStatus": correlation_status,
                "stateTransferCandidate": state_transfer_candidate
            })
        })
        .collect()
}

fn operational_signal_counts(db: &DB) -> Result<Value, context_graph_mejepa::EvalError> {
    Ok(json!({
        "agentFeedbackRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)?,
        "driftWindowRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW)?,
        "driftHistoryRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_DRIFT_HISTORY)?,
        "healReportRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)?,
        "modelPromotionRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS)?,
        "activeLearningQueueRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?,
        "oodEscalationRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_ESCALATIONS)?
    }))
}

fn cf_count(db: &DB, cf_name: &'static str) -> Result<usize, context_graph_mejepa::EvalError> {
    let cf = db.cf_handle(cf_name).ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::Store,
            format!("missing column family {cf_name}"),
        )
    })?;
    Ok(db.iterator_cf(cf, IteratorMode::Start).count())
}

fn weekly_file_status(
    exports_root: &Path,
    report: &context_graph_mejepa::EvalReport,
    report_hash: &str,
) -> Value {
    let report_dir = exports_root.join(&report.report_date);
    let markdown_path = report_dir.join("weekly.md");
    let json_path = report_dir.join("weekly.json");
    let markdown = file_status(&markdown_path);
    let json_file = file_status(&json_path);
    let json_hash_status = match fs::read(&json_path) {
        Ok(bytes) => match serde_json::from_slice::<context_graph_mejepa::EvalReport>(&bytes) {
            Ok(exported) => match exported.determinism_hash() {
                Ok(hash) if hash == report_hash => json!({
                    "status": "matches",
                    "hash": hash
                }),
                Ok(hash) => json!({
                    "status": "mismatch",
                    "hash": hash,
                    "expected": report_hash
                }),
                Err(err) => json!({
                    "status": "invalid",
                    "error": err.to_string()
                }),
            },
            Err(err) => json!({
                "status": "invalid",
                "error": err.to_string()
            }),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            json!({"status": "missing"})
        }
        Err(err) => json!({
            "status": "unreadable",
            "error": err.to_string()
        }),
    };
    let present = markdown["exists"] == true && json_file["exists"] == true;
    let hash_matches = json_hash_status["status"] == "matches";
    json!({
        "status": if present && hash_matches { "present" } else { "degraded" },
        "reportDir": report_dir,
        "markdown": markdown,
        "json": json_file,
        "jsonHashReadback": json_hash_status
    })
}

fn weekly_holdout_feed_status(d_root: &Path) -> Value {
    let path = d_root.join("state/gold-labels/weekly_holdout_panels.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return json!({
                "status": "missing",
                "path": path,
                "errorCode": "MEJEPA_WEEKLY_HOLDOUT_MISSING"
            });
        }
        Err(err) => {
            return json!({
                "status": "unreadable",
                "path": path,
                "errorCode": "MEJEPA_WEEKLY_HOLDOUT_UNREADABLE",
                "error": err.to_string()
            });
        }
    };
    let panels: Vec<HoldoutPanel> = match serde_json::from_slice(&bytes) {
        Ok(panels) => panels,
        Err(err) => {
            return json!({
                "status": "invalid_json",
                "path": path,
                "bytes": bytes.len(),
                "errorCode": "MEJEPA_WEEKLY_HOLDOUT_INVALID_JSON",
                "error": err.to_string()
            });
        }
    };
    if panels.is_empty() {
        return json!({
            "status": "empty",
            "path": path,
            "bytes": bytes.len(),
            "errorCode": "MEJEPA_WEEKLY_HOLDOUT_EMPTY"
        });
    }
    let invalid = panels
        .iter()
        .enumerate()
        .filter_map(|(idx, panel)| {
            panel
                .validate()
                .err()
                .map(|err| json!({"index": idx, "error": err.to_string()}))
        })
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return json!({
            "status": "invalid_panel",
            "path": path,
            "bytes": bytes.len(),
            "panelCount": panels.len(),
            "invalidPanels": invalid,
            "errorCode": "MEJEPA_WEEKLY_HOLDOUT_INVALID_PANEL"
        });
    }
    json!({
        "status": "ready",
        "path": path,
        "bytes": bytes.len(),
        "panelCount": panels.len(),
        "corpusSha": corpus_sha_from_holdout(&panels),
        "sha256": sha256_hex(&bytes)
    })
}

fn agent_feedback_summary(db: &DB, top_n: usize) -> Result<Value, context_graph_mejepa::EvalError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
        .ok_or_else(|| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::Store,
                format!(
                    "missing column family {}",
                    context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK
                ),
            )
        })?;
    let mut counts = std::collections::BTreeMap::<&'static str, usize>::from([
        ("confirmed", 0),
        ("surprise", 0),
        ("omission", 0),
        ("calibration", 0),
    ]);
    let mut corrupt_rows = 0usize;
    let mut surprises = Vec::<SurpriseEvent>::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item.map_err(|err| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::Store,
                err.to_string(),
            )
        })?;
        match serde_json::from_slice::<SurpriseEvent>(&value) {
            Ok(event) => {
                let key = match event.feedback_kind {
                    FeedbackKind::Confirmed => "confirmed",
                    FeedbackKind::Surprise => "surprise",
                    FeedbackKind::Omission => "omission",
                    FeedbackKind::Calibration => "calibration",
                };
                *counts.entry(key).or_default() += 1;
                if event.feedback_kind == FeedbackKind::Surprise {
                    surprises.push(event);
                }
            }
            Err(_) => corrupt_rows += 1,
        }
    }
    surprises.sort_by_key(|event| std::cmp::Reverse(event.ts_millis));
    let top_surprises = surprises
        .into_iter()
        .take(top_n)
        .map(|event| {
            json!({
                "predictionId": hex::encode(event.prediction_id.0),
                "agentId": event.agent_id.0,
                "tsMillis": event.ts_millis,
                "severity": event.severity,
                "explanation": event.agent_explanation
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "counts": counts,
        "topSurprises": top_surprises,
        "corruptRows": corrupt_rows,
        "sourceOfTruth": context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK
    }))
}

fn required_weekly_sections_status(exports_root: &Path, report_date: &str) -> Value {
    let path = exports_root.join(report_date).join("weekly.md");
    let required = [
        "## Catastrophic Events",
        "## Per-Cell Ship Gate Status",
        "## Active-Learning Queue Summary",
        "## Agent-Feedback Summary",
        "## Drift Events Of The Week",
        "## Promotions / Rollbacks Of The Week",
        "## Storage Utilization",
        "## Per-Cell State Transfer T",
        "## Ship Gate Failures",
    ];
    let markdown = match fs::read_to_string(&path) {
        Ok(markdown) => markdown,
        Err(err) => {
            let sections = required
                .into_iter()
                .map(|heading| json!({"heading": heading, "present": false}))
                .collect::<Vec<_>>();
            return json!({
                "path": path,
                "readable": false,
                "error": err.to_string(),
                "allPresent": false,
                "sections": sections
            });
        }
    };
    let sections = required
        .into_iter()
        .map(|heading| {
            json!({
                "heading": heading,
                "present": markdown.contains(heading)
            })
        })
        .collect::<Vec<_>>();
    let all_present = sections.iter().all(|section| section["present"] == true);
    json!({
        "path": path,
        "readable": true,
        "allPresent": all_present,
        "sections": sections
    })
}

fn file_status(path: &Path) -> Value {
    match fs::metadata(path) {
        Ok(metadata) => json!({
            "path": path,
            "exists": true,
            "bytes": metadata.len(),
            "sha256": fs::read(path).map(|bytes| sha256_hex(&bytes)).ok()
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json!({
            "path": path,
            "exists": false,
            "error": "missing"
        }),
        Err(err) => json!({
            "path": path,
            "exists": false,
            "error": err.to_string()
        }),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn runbook_actions(
    report: &context_graph_mejepa::EvalReport,
    weekly_files_status: &str,
    holdout_feed_status: &str,
) -> Value {
    let mut actions = Vec::new();
    if weekly_files_status != "present" {
        actions.push("rerun weekly eval job and inspect export filesystem permissions");
    }
    if holdout_feed_status != "ready" {
        actions
            .push("restore weekly holdout feed under state/gold-labels before Phase G evaluation");
    }
    if !report.ship_gate_passed {
        actions.push("inspect ship_gate_failures and block Phase G promotion");
    }
    if !low_state_transfer_cells(report).is_empty() {
        actions.push("queue fine-tune review for low state-transfer cells");
    }
    json!({
        "status": if actions.is_empty() { "no_action" } else { "action_required" },
        "actions": actions
    })
}

#[cfg(test)]
mod weekly_dashboard_tests {
    use super::*;
    use context_graph_mejepa::{
        ActiveLearningSummary, AgentId, ConformalHealthEntry, EvalReport, FeedbackId, Language,
        MutationCategory, PredictionId, RegressionCheck, StateTransferDiagnostic, SurpriseSeverity,
        WitnessHash,
    };
    use rocksdb::WriteOptions;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::io::Write;

    #[tokio::test]
    async fn weekly_eval_dashboard_writes_phase_f_fsv_artifact() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
        let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-weekly-dashboard-fsv");
        std::fs::create_dir_all(&fsv_root).unwrap();
        let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
        std::fs::create_dir_all(&run_root).unwrap();

        let happy = seed_dashboard_fixture(&run_root.join("happy"), true, true);
        let happy_response = handlers
            .call_mejepa_weekly_eval_dashboard(
                Some(JsonRpcId::Number(38001)),
                json!({
                    "dbPath": happy.db_path,
                    "dRoot": happy.d_root,
                    "exportsRoot": happy.exports_root,
                    "maxCells": 10
                }),
            )
            .await;
        assert!(happy_response.error.is_none());
        let happy_result = happy_response.result.unwrap();
        let happy_structured = happy_result["structuredContent"].clone();
        let happy_pass = happy_result["isError"] == false
            && happy_structured["holdoutFeed"]["status"] == "ready"
            && happy_structured["weeklyFiles"]["status"] == "present"
            && happy_structured["requiredWeeklySections"]["allPresent"] == true
            && happy_structured["agentFeedback"]["counts"]["surprise"] == 1;

        let missing_holdout =
            seed_dashboard_fixture(&run_root.join("missing-holdout"), false, true);
        let missing_holdout_response = handlers
            .call_mejepa_weekly_eval_dashboard(
                Some(JsonRpcId::Number(38002)),
                json!({
                    "dbPath": missing_holdout.db_path,
                    "dRoot": missing_holdout.d_root,
                    "exportsRoot": missing_holdout.exports_root,
                    "maxCells": 10
                }),
            )
            .await;
        let missing_holdout_result = missing_holdout_response.result.unwrap();

        let corrupt_export = seed_dashboard_fixture(&run_root.join("corrupt-export"), true, true);
        std::fs::write(
            corrupt_export
                .exports_root
                .join(&corrupt_export.report_date)
                .join("weekly.json"),
            b"{not-json",
        )
        .unwrap();
        let corrupt_export_response = handlers
            .call_mejepa_weekly_eval_dashboard(
                Some(JsonRpcId::Number(38003)),
                json!({
                    "dbPath": corrupt_export.db_path,
                    "dRoot": corrupt_export.d_root,
                    "exportsRoot": corrupt_export.exports_root,
                    "maxCells": 10
                }),
            )
            .await;
        let corrupt_export_result = corrupt_export_response.result.unwrap();

        let unknown_arg_response = handlers
            .call_mejepa_weekly_eval_dashboard(
                Some(JsonRpcId::Number(38004)),
                json!({"dbPath": happy.db_path, "unexpected": true}),
            )
            .await;
        let unknown_arg_result = unknown_arg_response.result.unwrap();

        let boundary_cases = vec![
            json!({
                "case": "missing_holdout_feed_is_visible",
                "expected": "MEJEPA_WEEKLY_HOLDOUT_MISSING",
                "actual": missing_holdout_result,
                "pass": missing_holdout_result["structuredContent"]["holdoutFeed"]["errorCode"]
                    == "MEJEPA_WEEKLY_HOLDOUT_MISSING"
            }),
            json!({
                "case": "corrupt_weekly_json_is_degraded",
                "expected": "weeklyFiles.jsonHashReadback.status=invalid",
                "actual": corrupt_export_result,
                "pass": corrupt_export_result["structuredContent"]["weeklyFiles"]["jsonHashReadback"]["status"]
                    == "invalid"
            }),
            json!({
                "case": "unknown_argument_fails_schema",
                "expected": "schema validation failed",
                "actual": unknown_arg_result,
                "pass": unknown_arg_result["isError"] == true
                    && unknown_arg_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("schema validation failed")
            }),
        ];
        let all_passed = happy_pass && boundary_cases.iter().all(|case| case["pass"] == true);
        let evidence = json!({
            "fsv_root": fsv_root,
            "task_id": "TASK-OBS-001",
            "started_at_unix_ms": started_at_unix_ms,
            "build_release_sha": git_head_short_for_test(),
            "happy_path": [{
                "case": "weekly_dashboard_reads_report_exports_holdout_and_feedback",
                "sot": [
                    "CF_MEJEPA_EVAL_REPORTS",
                    "CF_MEJEPA_AGENT_FEEDBACK",
                    "state/gold-labels/weekly_holdout_panels.json",
                    "exports/eval/<date>/weekly.md",
                    "exports/eval/<date>/weekly.json"
                ],
                "before": null,
                "trigger": "cargo test -p context-graph-mcp weekly_eval_dashboard_writes_phase_f_fsv_artifact",
                "after": happy_structured,
                "expected": "holdout ready, weekly files present, required sections present, surprise feedback counted",
                "actual": happy_pass,
                "pass": happy_pass,
                "evidence_path": happy.run_root
            }],
            "boundary_cases": boundary_cases,
            "all_passed": all_passed,
            "readback_equal": true,
            "physical_artifacts": {
                "run_root": run_root
            }
        });
        assert!(all_passed);
        let evidence_path = fsv_root.join("weekly_dashboard_fsv.json");
        context_graph_mejepa::eval::report::write_json_0600(&evidence_path, &evidence).unwrap();
        assert!(std::fs::metadata(&evidence_path).unwrap().len() > 0);
    }

    struct DashboardFixture {
        run_root: PathBuf,
        db_path: PathBuf,
        d_root: PathBuf,
        exports_root: PathBuf,
        report_date: String,
    }

    fn seed_dashboard_fixture(
        run_root: &Path,
        write_holdout: bool,
        write_exports: bool,
    ) -> DashboardFixture {
        std::fs::create_dir_all(run_root).unwrap();
        let db_path = run_root.join("storage-db");
        let d_root = run_root.join("d-root");
        let exports_root = d_root.join("exports/eval");
        let report_date = "2026-05-13-phase-f-dashboard".to_string();
        std::fs::create_dir_all(&d_root).unwrap();
        let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&db_path).unwrap();
        let eval_store = RocksDbEvalStore::new(db.clone()).unwrap();
        let report = synthetic_dashboard_report(&report_date);
        eval_store.persist_report(&report).unwrap();
        seed_surprise_feedback(db.as_ref());
        if write_holdout {
            let repo = run_root.join("repo");
            std::fs::create_dir_all(&repo).unwrap();
            let holdout = synthetic_holdout(&repo).unwrap();
            let holdout_path = d_root.join("state/gold-labels/weekly_holdout_panels.json");
            context_graph_mejepa::eval::report::write_json_0600(&holdout_path, &holdout).unwrap();
        }
        if write_exports {
            let report_dir = exports_root.join(&report_date);
            std::fs::create_dir_all(&report_dir).unwrap();
            context_graph_mejepa::eval::report::write_json_0600(
                &report_dir.join("weekly.json"),
                &report,
            )
            .unwrap();
            write_text_0600(
                &report_dir.join("weekly.md"),
                weekly_markdown_with_required_sections(),
            );
        }
        for cf in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
            if let Some(handle) = db.cf_handle(cf) {
                db.flush_cf(handle).unwrap();
            }
        }
        drop(db);
        DashboardFixture {
            run_root: run_root.to_path_buf(),
            db_path,
            d_root,
            exports_root,
            report_date,
        }
    }

    fn synthetic_dashboard_report(report_date: &str) -> EvalReport {
        let cell = context_graph_mejepa::cell_key(MutationCategory::KnownGood, Language::Python);
        let mut per_cell_correlation = BTreeMap::new();
        per_cell_correlation.insert(cell.clone(), Some(0.96));
        let mut per_cell_state_transfer = BTreeMap::new();
        per_cell_state_transfer.insert(
            cell.clone(),
            Some(StateTransferDiagnostic {
                wasserstein_1: 0.05,
                transfer_score: 0.92,
                performance_deploy: 0.91,
            }),
        );
        EvalReport {
            report_date: report_date.to_string(),
            generated_at_unix_ms: 1_778_640_000_000,
            rolling_window_size: 100,
            holdout_count: 4,
            overall_correlation: Some(0.96),
            per_category_correlation: BTreeMap::from([(MutationCategory::KnownGood, Some(0.96))]),
            per_language_correlation: BTreeMap::from([(Language::Python, Some(0.96))]),
            per_cell_correlation,
            bayesian_shrinkage: BTreeMap::from([(cell.clone(), 0.95)]),
            conformal_coverage_health: BTreeMap::from([(
                Language::Python,
                ConformalHealthEntry {
                    expected_coverage: 0.90,
                    empirical_coverage: 0.90,
                    sample_count: 4,
                    within_band: true,
                },
            )]),
            ood_calibration_health: BTreeMap::from([(Language::Python, Some(0.88))]),
            gtau_pass_rate: BTreeMap::from([(Language::Python, 0.98)]),
            active_learning: ActiveLearningSummary {
                queued_count: 1,
                evicted_count: 0,
                ood_escalation_count: 0,
            },
            state_transfer_diagnostic: Some(StateTransferDiagnostic {
                wasserstein_1: 0.05,
                transfer_score: 0.92,
                performance_deploy: 0.91,
            }),
            per_cell_state_transfer,
            aux_head_distillation: None,
            regression_checks: vec![RegressionCheck {
                name: "overall_correlation".to_string(),
                previous: 0.95,
                current: 0.96,
                drop: 0.0,
                passed: true,
            }],
            open_research_questions: Vec::new(),
            q1_pass_rate: 1.0,
            q2_report_correlation: Some(0.96),
            q3_side_effect_agreement: Some(1.0),
            ship_gate_passed: true,
            ship_gate_failures: Vec::new(),
            provenance: EvalProvenance {
                corpus_sha: "phase-f-dashboard-corpus".to_string(),
                eval_code_version: "phase-f-dashboard-test".to_string(),
                calibration_version: "phase-f-dashboard-calibration".to_string(),
                generated_by: "weekly-dashboard-test".to_string(),
            },
            wall_clock_seconds: 0.1,
        }
    }

    fn seed_surprise_feedback(db: &DB) {
        let event = SurpriseEvent::try_new(SurpriseEvent {
            feedback_id: FeedbackId([1; 16]),
            prediction_id: PredictionId([2; 16]),
            agent_id: AgentId("agent:weekly-dashboard-test".to_string()),
            ts_millis: 1_778_640_000_001,
            feedback_kind: FeedbackKind::Surprise,
            agent_explanation: "synthetic surprise for weekly dashboard FSV".to_string(),
            actual_outcome: Some(context_graph_mejepa::ActualOutcome {
                oracle_outcome: context_graph_mejepa::OracleOutcome::Fail,
                failed_tests: Vec::new(),
                runtime_ms: Some(10),
                notes: "synthetic".to_string(),
            }),
            severity: SurpriseSeverity::High,
            extra_structured_data: json!({"source": "weekly-dashboard-fsv"}),
            witness_hash: WitnessHash([3; 32]),
        })
        .unwrap();
        let cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
            .unwrap();
        let bytes = serde_json::to_vec(&event).unwrap();
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        db.put_cf_opt(cf, b"feedback:weekly-dashboard-fsv", &bytes, &opts)
            .unwrap();
        let readback = db
            .get_cf(cf, b"feedback:weekly-dashboard-fsv")
            .unwrap()
            .unwrap();
        assert_eq!(readback, bytes);
    }

    fn weekly_markdown_with_required_sections() -> &'static str {
        "# ME-JEPA Weekly Evaluation\n\n\
         ## Catastrophic Events\n\n- none\n\n\
         ## Per-Cell Ship Gate Status\n\n- known_good::python: pass\n\n\
         ## Active-Learning Queue Summary\n\n- queued_count: 1\n\n\
         ## Agent-Feedback Summary\n\n- surprise: 1\n\n\
         ## Drift Events Of The Week\n\n- none\n\n\
         ## Promotions / Rollbacks Of The Week\n\n- none\n\n\
         ## Storage Utilization\n\n- ok\n\n\
         ## Per-Cell State Transfer T\n\n- known_good::python: hold\n\n\
         ## Ship Gate Failures\n\n- none\n"
    }

    fn write_text_0600(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(path)
                .unwrap()
        };
        #[cfg(not(unix))]
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.sync_all().unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }

    fn git_head_short_for_test() -> String {
        std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string())
    }
}
