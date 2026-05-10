//! ME-JEPA Phase 4 inference MCP handlers.

use std::path::PathBuf;
use std::sync::Arc;

use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, CalibrationStore, MeJepaInferConfig,
    PatchBundle, RocksDbInferStore, TaskContext,
};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerifyRequest {
    patch: PatchBundle,
    context: TaskContext,
    #[serde(default)]
    include_provenance: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PredictLatestRequest {
    session_id: String,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    10
}

impl Handlers {
    pub(crate) async fn call_mejepa_verify(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: VerifyRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_VERIFY
                    ),
                );
            }
        };
        if request.context.problem_statement.len() > 65_536 {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "problem_statement exceeds 65536 bytes",
            );
        }
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let repo_root = request.context.environment.repo_root.clone();
        info!(
            tool = tool_names::MEJEPA_VERIFY,
            task_id = %request.context.task_id.0,
            db_path = %db_path.display(),
            "ME-JEPA verify"
        );
        let result = (|| {
            let db = open_infer_rocksdb(&db_path)?;
            let calibration = CalibrationStore::new(db.clone(), 30)?;
            let store = Arc::new(RocksDbInferStore::new(db));
            let compiler = build_deterministic_compiler(
                repo_root,
                store,
                calibration,
                MeJepaInferConfig::default(),
            )?;
            let verdict = compiler.verify(&request.patch, &request.context)?;
            let mut response = serde_json::to_value(verdict)?;
            if request.include_provenance {
                response["provenance"] = json!({
                    "codeVersion": env!("CARGO_PKG_VERSION"),
                    "dbPath": db_path,
                    "mode": "phase4_inference"
                });
            }
            Ok::<serde_json::Value, context_graph_mejepa::MejepaInferError>(response)
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

    pub(crate) async fn call_mejepa_predict_latest(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: PredictLatestRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_PREDICT_LATEST
                    ),
                );
            }
        };
        if request.session_id.len() != 32
            || !request.session_id.chars().all(|ch| ch.is_ascii_hexdigit())
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "sessionId must be exactly 32 hexadecimal characters",
            );
        }
        let mut session_id = [0u8; 16];
        if let Err(err) = hex::decode_to_slice(&request.session_id, &mut session_id) {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                &format!("sessionId decode failed: {err}"),
            );
        }
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result = (|| {
            let db = open_infer_rocksdb(&db_path)?;
            let store = RocksDbInferStore::new(db);
            let predictions = context_graph_mejepa::MejepaStore::read_live_predictions(
                &store,
                session_id,
                request.limit.clamp(1, 1000),
            )?;
            Ok::<serde_json::Value, context_graph_mejepa::MejepaInferError>(json!({
                "predictions": predictions
            }))
        })();
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Storage,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }
}

fn infer_db_path() -> Result<PathBuf, String> {
    std::env::var("CONTEXTGRAPH_MEJEPA_INFER_DB")
        .map(PathBuf::from)
        .map_err(|_| {
            "CONTEXTGRAPH_MEJEPA_INFER_DB must point to the Phase 4 inference RocksDB; refusing to guess a database path".to_string()
        })
}
