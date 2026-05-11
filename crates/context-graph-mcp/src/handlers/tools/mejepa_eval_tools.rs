//! ME-JEPA Phase 8 evaluation MCP handlers.

use std::path::PathBuf;

use context_graph_mejepa::eval::{
    build_eval_compiler, build_patch_similarity_graph, corpus_sha_from_holdout, synthetic_holdout,
    synthetic_patch_embeddings, write_weekly_report, EvalConfig, EvalProvenance, EvalRunner,
    RocksDbEvalStore,
};
use context_graph_mejepa::open_infer_rocksdb;
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
