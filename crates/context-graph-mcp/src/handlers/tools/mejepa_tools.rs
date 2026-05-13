//! ME-JEPA Phase 4 inference MCP handlers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result as AnyhowResult};
use context_graph_mejepa::heal::{
    all_referenced_cf_names, apply_promotion_approval, decode_value, encode_active_pointer_key,
    AbcPromoter, ActivePointerValue, HealReport, HealRocksStore, PromotionApprovalAction,
    PromotionApprovalRequest as HealPromotionApprovalRequest, PromotionGate, PromotionLockState,
    WitnessChainAppender, CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_HEAL_REPORTS,
};
use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, ActiveLearningQueueState, ActualOutcome,
    AgentId, CalibrationStore, FeedbackId, FeedbackKind, MeJepaInferConfig, OracleOutcome,
    PatchBundle, PredictionId, RealityPrediction, RocksDbEvalStore, RocksDbInferStore,
    SurpriseEvent, SurpriseSeverity, TaskContext, TestId, WitnessHash,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_AGENT_FEEDBACK,
    CF_MEJEPA_CALIBRATION_HISTORY, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_embedders::{query_vram_budget, VramBudget};
use context_graph_mejepa_hygiene::{mcp_quota_status, HygieneMcpRequest};
use context_graph_mejepa_tct::{
    build_inspect_summary, open_tct_rocksdb, ConstellationStore, EmbedderId,
};
use rocksdb::{IteratorMode, WriteBatch, WriteOptions, DB};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::info;

use super::mejepa_phase7_storage::{subscriber_state_for_provenance, subscriber_status_for_paths};
use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const ENV_TCT_DB: &str = "CONTEXTGRAPH_MEJEPA_TCT_DB";
const ENV_HEAL_DB: &str = "CONTEXTGRAPH_MEJEPA_HEAL_DB";
const ENV_HYGIENE_ARCHIVE_ROOT: &str = "CONTEXTGRAPH_MEJEPA_HYGIENE_ARCHIVE_ROOT";
const ENV_PANEL_DB: &str = "CONTEXTGRAPH_MEJEPA_PANEL_DB";
const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";
const ENV_ALLOW_DETERMINISTIC_VERIFY: &str = "CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY";

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PredictWhatIfRequest {
    patch: PatchBundle,
    context: TaskContext,
    compare_to_prediction_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExplainPredictionRequest {
    prediction_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordAgentFeedbackRequest {
    prediction_id: String,
    agent_id: String,
    feedback_kind: FeedbackKind,
    agent_explanation: String,
    actual_outcome: Option<ActualOutcomeRequest>,
    severity: SurpriseSeverity,
    #[serde(default)]
    extra_structured_data: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActualOutcomeRequest {
    oracle_outcome: OracleOutcome,
    #[serde(default)]
    failed_tests: Vec<String>,
    runtime_ms: Option<u64>,
    notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConstellationInspectRequest {
    db_path: Option<PathBuf>,
    version_id: Option<String>,
    runtime_embedder_versions: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HealStatusRequest {
    db_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RollbackToRequest {
    db_path: Option<PathBuf>,
    witness_chain_path: PathBuf,
    target_witness_chain_offset: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PromoteApprovalRequest {
    db_path: Option<PathBuf>,
    promotion_id: String,
    operator_id: String,
    action: PromotionApprovalAction,
    operator_reason: String,
    #[serde(default = "default_two_person_rule")]
    two_person_rule: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DaemonStatusVramBudget {
    ContentSet,
    FullPhase1,
}

impl Default for DaemonStatusVramBudget {
    fn default() -> Self {
        Self::ContentSet
    }
}

impl DaemonStatusVramBudget {
    fn as_budget(self) -> VramBudget {
        match self {
            Self::ContentSet => VramBudget::content_set_rtx5090(),
            Self::FullPhase1 => VramBudget::full_phase1_rtx5090(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MejepaDaemonStatusRequest {
    infer_db_path: Option<PathBuf>,
    panel_db_path: Option<PathBuf>,
    heal_db_path: Option<PathBuf>,
    quota_db_path: Option<PathBuf>,
    archive_root: Option<PathBuf>,
    #[serde(default = "default_include_vram")]
    include_vram: bool,
    #[serde(default)]
    vram_budget: DaemonStatusVramBudget,
}

fn default_limit() -> u32 {
    10
}

fn default_two_person_rule() -> bool {
    true
}

fn default_include_vram() -> bool {
    true
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
        if let Err(err) = request
            .patch
            .validate()
            .and_then(|_| request.context.validate())
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                &format!("{}: {err}", err.code()),
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
        let session_id = request.context.session_id;
        let result = (|| {
            ensure_deterministic_verify_allowed()?;
            let db = open_infer_rocksdb(&db_path)?;
            let subscriber_state = subscriber_state_for_provenance(db.as_ref(), session_id)
                .map_err(
                    |detail| context_graph_mejepa::MejepaInferError::InvalidInput {
                        field: "subscriber_state".to_string(),
                        detail,
                    },
                )?;
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
            let subscriber_state_value = subscriber_state.unwrap_or(serde_json::Value::Null);
            response["subscriber_state"] = subscriber_state_value.clone();
            if request.include_provenance {
                response["provenance"] = json!({
                    "codeVersion": env!("CARGO_PKG_VERSION"),
                    "dbPath": db_path,
                    "mode": "deterministic_phase4_harness",
                    "last_consumed_shift_id": subscriber_state_value
                        .get("last_consumed_shift_id")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                    "subscriber_lag_at_call_time": subscriber_state_value
                        .get("subscriber_lag_at_call_time")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null)
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
        if !(1..=1000).contains(&request.limit) {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "limit must be in [1, 1000]",
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
                request.limit,
            )?;
            let session_known =
                context_graph_mejepa::MejepaStore::session_known(&store, session_id)?;
            Ok::<serde_json::Value, context_graph_mejepa::MejepaInferError>(json!({
                "predictions": predictions,
                "session_known": session_known
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

    pub(crate) async fn call_mejepa_predict_what_if(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: PredictWhatIfRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_PREDICT_WHAT_IF
                    ),
                );
            }
        };
        if let Err(err) = request
            .patch
            .validate()
            .and_then(|_| request.context.validate())
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                &format!("{}: {err}", err.code()),
            );
        }
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result = run_predict_what_if(&db_path, request);
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Execution,
                "MEJEPA_PREDICT_WHAT_IF_FAILED",
                &err.to_string(),
                json!({"dbPath": db_path}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_explain_prediction(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: ExplainPredictionRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_EXPLAIN_PREDICTION
                    ),
                );
            }
        };
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result = run_explain_prediction(&db_path, request);
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_EXPLAIN_PREDICTION_FAILED",
                &err.to_string(),
                json!({"dbPath": db_path}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_record_agent_feedback(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: RecordAgentFeedbackRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_RECORD_AGENT_FEEDBACK
                    ),
                );
            }
        };
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result = record_agent_feedback_in_db(&db_path, request);
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_RECORD_AGENT_FEEDBACK_FAILED",
                &err.to_string(),
                json!({"dbPath": db_path}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_bootstrap_status(
        &self,
        id: Option<JsonRpcId>,
        _args: serde_json::Value,
    ) -> JsonRpcResponse {
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result = (|| {
            let db = open_infer_rocksdb(&db_path)?;
            let oracle_verdict_rows = count_cf_any(
                db.as_ref(),
                context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
            )?;
            let live_prediction_rows = count_cf_any(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
            let agent_feedback_rows = count_cf_any(db.as_ref(), CF_MEJEPA_AGENT_FEEDBACK)?;
            let active_learning_queue_present = db
                .get_cf(
                    cf_handle(db.as_ref(), CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?,
                    b"active",
                )?
                .is_some();
            let stage = if oracle_verdict_rows >= 1000 {
                "warm"
            } else if oracle_verdict_rows > 0 || live_prediction_rows > 0 {
                "bootstrapping"
            } else {
                "cold"
            };
            Ok::<Value, anyhow::Error>(json!({
                "stage": stage,
                "counts": {
                    "oracleVerdicts": oracle_verdict_rows,
                    "livePredictions": live_prediction_rows,
                    "agentFeedback": agent_feedback_rows,
                    "activeLearningQueuePresent": active_learning_queue_present
                },
                "sourceOfTruth": {
                    "dbPath": db_path,
                    "cfs": [
                        context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
                        CF_MEJEPA_LIVE_PREDICTIONS,
                        CF_MEJEPA_AGENT_FEEDBACK,
                        CF_MEJEPA_ACTIVE_LEARNING_QUEUE
                    ]
                }
            }))
        })();
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_BOOTSTRAP_STATUS_FAILED",
                &err.to_string(),
                json!({"dbPath": db_path}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_constellation_inspect(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: ConstellationInspectRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_CONSTELLATION_INSPECT
                    ),
                );
            }
        };
        let db_path = match request.db_path {
            path @ Some(_) => match resolve_tct_db_path(path) {
                Ok(path) => path,
                Err(message) => {
                    return self.tool_error_typed(id, ToolErrorKind::Validation, &message)
                }
            },
            None => match resolve_tct_db_path(None) {
                Ok(path) => path,
                Err(message) => {
                    return self.tool_error_typed(id, ToolErrorKind::Validation, &message)
                }
            },
        };
        let runtime_versions = match parse_runtime_versions(&request.runtime_embedder_versions) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result =
            run_constellation_inspect(&db_path, request.version_id.as_deref(), &runtime_versions);
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

    pub(crate) async fn call_mejepa_heal_status(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: HealStatusRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_HEAL_STATUS
                    ),
                );
            }
        };
        let db_path = match resolve_heal_db_path(request.db_path) {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_heal_status(&db_path) {
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

    pub(crate) async fn call_mejepa_daemon_status(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: MejepaDaemonStatusRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_DAEMON_STATUS
                    ),
                );
            }
        };
        match self.run_mejepa_daemon_status(request).await {
            Ok(value) => self.tool_result(id, value),
            Err(message) => self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        }
    }

    pub(crate) async fn call_mejepa_rollback_to(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: RollbackToRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_ROLLBACK_TO
                    ),
                );
            }
        };
        let db_path = match resolve_heal_db_path(request.db_path) {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_rollback_to(
            &db_path,
            request.witness_chain_path,
            request.target_witness_chain_offset,
        ) {
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

    pub(crate) async fn call_mejepa_promote_approval(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: PromoteApprovalRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_PROMOTE_APPROVAL
                    ),
                );
            }
        };
        let db_path = match resolve_heal_db_path(request.db_path) {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_promote_approval(
            &db_path,
            &request.promotion_id,
            &request.operator_id,
            request.action,
            &request.operator_reason,
            request.two_person_rule,
        ) {
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

    async fn run_mejepa_daemon_status(
        &self,
        request: MejepaDaemonStatusRequest,
    ) -> Result<serde_json::Value, String> {
        let started = Instant::now();
        let infer_db_path =
            resolve_optional_path(request.infer_db_path, ENV_INFER_DB, "inferDbPath")?;
        let panel_db_path =
            resolve_optional_path(request.panel_db_path, ENV_PANEL_DB, "panelDbPath")?;
        let heal_db_path = resolve_optional_path(request.heal_db_path, ENV_HEAL_DB, "healDbPath")?;
        let quota_db_path = request
            .quota_db_path
            .map(|path| validate_non_empty_path("quotaDbPath", path))
            .transpose()?
            .or_else(|| infer_db_path.clone());
        let archive_root = resolve_optional_path(
            request.archive_root,
            ENV_HYGIENE_ARCHIVE_ROOT,
            "archiveRoot",
        )?;

        let mut components = BTreeMap::new();
        components.insert("runtime".to_string(), self.mejepa_runtime_status().await);

        let subscriber = match infer_db_path.as_deref() {
            Some(path) => match subscriber_status_for_paths(path, panel_db_path.as_deref()).await {
                Ok(value) => healthy_component(value),
                Err(message) => unavailable_component(
                    "MEJEPA_DAEMON_STATUS_SUBSCRIBER_READ_FAILED",
                    message,
                    json!({"inferDbPath": path, "panelDbPath": panel_db_path}),
                ),
            },
            None => unavailable_component(
                "MEJEPA_DAEMON_STATUS_INFER_DB_MISSING",
                format!("inferDbPath or {ENV_INFER_DB} is required for subscriber status"),
                json!({"env": ENV_INFER_DB}),
            ),
        };
        components.insert("subscriber".to_string(), subscriber);

        let heal = match heal_db_path.as_deref() {
            Some(path) => match run_heal_status(path) {
                Ok(value) => healthy_component(value),
                Err(err) => unavailable_component(
                    err.code().to_string(),
                    err.to_string(),
                    json!({"healDbPath": path}),
                ),
            },
            None => unavailable_component(
                "MEJEPA_DAEMON_STATUS_HEAL_DB_MISSING",
                format!("healDbPath or {ENV_HEAL_DB} is required for heal status"),
                json!({"env": ENV_HEAL_DB}),
            ),
        };
        components.insert("heal".to_string(), heal);

        let quota = match (quota_db_path.as_ref(), archive_root.as_ref()) {
            (Some(db_path), Some(root)) => {
                let request = HygieneMcpRequest {
                    db_path: db_path.clone(),
                    archive_root: root.clone(),
                };
                match mcp_quota_status(request) {
                    Ok(value) => healthy_component(value),
                    Err(err) => unavailable_component(
                        err.code.to_string(),
                        err.to_string(),
                        json!({"quotaDbPath": db_path, "archiveRoot": root}),
                    ),
                }
            }
            (None, _) => unavailable_component(
                "MEJEPA_DAEMON_STATUS_QUOTA_DB_MISSING",
                format!("quotaDbPath or {ENV_INFER_DB} is required for quota status"),
                json!({"env": ENV_INFER_DB}),
            ),
            (_, None) => unavailable_component(
                "MEJEPA_DAEMON_STATUS_ARCHIVE_ROOT_MISSING",
                format!("archiveRoot or {ENV_HYGIENE_ARCHIVE_ROOT} is required for quota status"),
                json!({"env": ENV_HYGIENE_ARCHIVE_ROOT}),
            ),
        };
        components.insert("quota".to_string(), quota);

        let vram = if request.include_vram {
            match query_vram_budget(request.vram_budget.as_budget()) {
                Ok(value) => healthy_component(json!(value)),
                Err(err) => unavailable_component(
                    err.code().to_string(),
                    err.to_string(),
                    json!({"budget": format!("{:?}", request.vram_budget)}),
                ),
            }
        } else {
            disabled_component("VRAM status disabled by includeVram=false")
        };
        components.insert("vram".to_string(), vram);

        Ok(json!({
            "tool": tool_names::MEJEPA_DAEMON_STATUS,
            "overallStatus": aggregate_status(&components),
            "generatedAtUnixMs": unix_now_ms()?,
            "elapsedMs": started.elapsed().as_millis() as u64,
            "components": components,
            "sourceOfTruth": {
                "runtime": "process",
                "subscriber": "CF_MEJEPA_SHIFT_WATERMARK + optional CF_MEJEPA_PANELS",
                "heal": "ME-JEPA heal RocksDB CFs",
                "quota": "ME-JEPA hygiene RocksDB CFs + archive root filesystem",
                "vram": "CUDA driver cuMemGetInfo_v2"
            }
        }))
    }

    async fn mejepa_runtime_status(&self) -> serde_json::Value {
        match &self.daemon_state {
            Some(state) => healthy_component(json!({
                "pid": std::process::id(),
                "mode": "daemon",
                "uptimeSecs": state.start_time.elapsed().as_secs(),
                "activeConnections": state.active_connections.load(std::sync::atomic::Ordering::SeqCst),
                "maxConnections": state.max_connections,
                "backgroundShutdown": state.background_shutdown.load(std::sync::atomic::Ordering::SeqCst),
                "modelsState": if state.models_loading.load(std::sync::atomic::Ordering::SeqCst) {
                    "loading".to_string()
                } else {
                    match state.models_failed.read().await.as_ref() {
                        Some(err) => format!("failed: {err}"),
                        None => "ready".to_string(),
                    }
                }
            })),
            None => healthy_component(json!({
                "pid": std::process::id(),
                "mode": "stdio",
                "note": "daemon runtime state is not injected in stdio mode"
            })),
        }
    }
}

fn resolve_optional_path(
    input: Option<PathBuf>,
    env_name: &'static str,
    field: &'static str,
) -> Result<Option<PathBuf>, String> {
    match input {
        Some(path) => validate_non_empty_path(field, path).map(Some),
        None => match std::env::var(env_name) {
            Ok(raw) => validate_non_empty_path(env_name, PathBuf::from(raw)).map(Some),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(err) => Err(format!("{env_name} must be readable UTF-8: {err}")),
        },
    }
}

fn validate_non_empty_path(field: &'static str, path: PathBuf) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err(format!("{field} must be a non-empty path"));
    }
    Ok(path)
}

fn healthy_component(data: serde_json::Value) -> serde_json::Value {
    json!({
        "status": "healthy",
        "data": data
    })
}

fn unavailable_component(
    error_code: impl Into<String>,
    message: impl Into<String>,
    source: serde_json::Value,
) -> serde_json::Value {
    json!({
        "status": "unavailable",
        "errorCode": error_code.into(),
        "message": message.into(),
        "source": source
    })
}

fn disabled_component(message: impl Into<String>) -> serde_json::Value {
    json!({
        "status": "disabled",
        "message": message.into()
    })
}

fn aggregate_status(components: &BTreeMap<String, serde_json::Value>) -> &'static str {
    let unavailable_count = components
        .values()
        .filter(|component| component.get("status").and_then(Value::as_str) == Some("unavailable"))
        .count();
    if unavailable_count == 0 {
        "healthy"
    } else if unavailable_count == components.len() {
        "unavailable"
    } else {
        "degraded"
    }
}

fn unix_now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .map_err(|err| format!("system clock is before UNIX_EPOCH: {err}"))
}

fn resolve_heal_db_path(input: Option<PathBuf>) -> Result<PathBuf, String> {
    match input {
        Some(path) => Ok(path),
        None => match std::env::var(ENV_HEAL_DB) {
            Ok(path) => Ok(PathBuf::from(path)),
            Err(std::env::VarError::NotPresent) => Err(format!(
                "dbPath or {ENV_HEAL_DB} is required; refusing to guess a self-healing database path"
            )),
            Err(err) => Err(format!("{ENV_HEAL_DB} must be readable UTF-8: {err}")),
        },
    }
}

fn run_heal_status(
    db_path: &std::path::Path,
) -> Result<serde_json::Value, Box<context_graph_mejepa::heal::HealError>> {
    let store = HealRocksStore::open(db_path)?;
    let cf_counts = all_referenced_cf_names()
        .iter()
        .map(|cf| Ok(((*cf).to_string(), store.count_cf(cf)?)))
        .collect::<
            Result<std::collections::BTreeMap<_, _>, Box<context_graph_mejepa::heal::HealError>>,
        >()?;
    let mut active_pointers = std::collections::BTreeMap::new();
    for name in [
        "active_weights",
        "active_calibration",
        "active_constellation",
    ] {
        if let Some(bytes) =
            store.get_cf(CF_MEJEPA_ACTIVE_POINTERS, &encode_active_pointer_key(name)?)?
        {
            let value: ActivePointerValue = decode_value(&bytes)?;
            active_pointers.insert(
                name.to_string(),
                json!({
                    "thetaShaOrVersionHex": hex::encode(value.theta_sha_or_version),
                    "frozenAt": value.frozen_at
                }),
            );
        }
    }
    let latest_report = store
        .scan_cf_values(CF_MEJEPA_HEAL_REPORTS)?
        .into_iter()
        .map(|bytes| decode_value::<HealReport>(&bytes).map_err(Box::new))
        .collect::<Result<Vec<_>, Box<context_graph_mejepa::heal::HealError>>>()?
        .into_iter()
        .max_by_key(|report| report.witness_chain_offset)
        .map(|report| {
            json!({
                "modeWinner": report.mode_winner,
                "weightsShaWinner": hex::encode(report.weights_sha_winner),
                "evaluationSummarySha": hex::encode(report.evaluation_summary_sha),
                "witnessChainOffset": report.witness_chain_offset,
                "promotionLatencySeconds": report.promotion_latency_seconds,
                "statusChange": report.status_change,
                "triggerReason": report.trigger_reason
            })
        });
    Ok(json!({
        "dbPath": db_path,
        "cfCounts": cf_counts,
        "activePointers": active_pointers,
        "latestHealReport": latest_report
    }))
}

fn run_rollback_to(
    db_path: &std::path::Path,
    witness_chain_path: PathBuf,
    target_witness_chain_offset: u64,
) -> Result<serde_json::Value, Box<context_graph_mejepa::heal::HealError>> {
    let store = HealRocksStore::open(db_path)?;
    let mut witness_chain = WitnessChainAppender::new(witness_chain_path.clone())?;
    let lock = Arc::new(Mutex::new(PromotionLockState::default()));
    let mut promoter = AbcPromoter::try_new(0.1, PromotionGate::default())?;
    let evidence = promoter.rollback_to(
        target_witness_chain_offset,
        store.clone(),
        &mut witness_chain,
        lock,
    )?;
    Ok(json!({
        "targetWitnessChainOffset": evidence.target_witness_chain_offset,
        "rolledBackTo": hex::encode(evidence.rolled_back_to),
        "newWitnessChainOffset": evidence.new_witness_chain_offset,
        "dbPath": db_path,
        "witnessChainPath": witness_chain_path
    }))
}

fn run_promote_approval(
    db_path: &std::path::Path,
    promotion_id: &str,
    operator_id: &str,
    action: PromotionApprovalAction,
    operator_reason: &str,
    two_person_rule: bool,
) -> Result<serde_json::Value, Box<context_graph_mejepa::heal::HealError>> {
    let store = HealRocksStore::open(db_path)?;
    let response = apply_promotion_approval(
        &store,
        HealPromotionApprovalRequest {
            promotion_id: promotion_id.to_string(),
            operator_id: operator_id.to_string(),
            action,
            reason: operator_reason.to_string(),
            two_person_rule,
        },
    )?;
    Ok(json!({
        "dbPath": db_path,
        "promotionId": response.promotion_id,
        "stateBefore": response.state_before,
        "stateAfter": response.state_after,
        "requiredDistinctApprovals": response.required_distinct_approvals,
        "distinctApprovalCount": response.distinct_approval_count,
        "sourceOfTruth": {
            "cf": response.source_of_truth_cf,
            "keyHex": response.source_of_truth_key_hex,
            "readbackVerified": response.readback_verified
        }
    }))
}

fn resolve_tct_db_path(input: Option<PathBuf>) -> Result<PathBuf, String> {
    match input {
        Some(path) => Ok(path),
        None => match std::env::var(ENV_TCT_DB) {
            Ok(path) => Ok(PathBuf::from(path)),
            Err(std::env::VarError::NotPresent) => Err(format!(
                "dbPath or {ENV_TCT_DB} is required; refusing to guess a TCT database path"
            )),
            Err(err) => Err(format!("{ENV_TCT_DB} must be readable UTF-8: {err}")),
        },
    }
}

fn run_constellation_inspect(
    db_path: &std::path::Path,
    version_id: Option<&str>,
    runtime_versions: &std::collections::BTreeMap<EmbedderId, [u8; 32]>,
) -> Result<serde_json::Value, context_graph_mejepa_tct::TctError> {
    let db = open_tct_rocksdb(db_path)?;
    let store = ConstellationStore::new(db)?;
    let version_id = match version_id {
        Some(value) => parse_version_id(value).map_err(|err| {
            context_graph_mejepa_tct::TctError::InvalidInput {
                field: "versionId".to_string(),
                detail: err,
            }
        })?,
        None => store.latest_version()?,
    };
    let constellation = store.load(version_id, runtime_versions)?;
    let summary = build_inspect_summary(&constellation, None);
    serde_json::to_value(summary).map_err(context_graph_mejepa_tct::TctError::from)
}

fn infer_db_path() -> Result<PathBuf, String> {
    std::env::var("CONTEXTGRAPH_MEJEPA_INFER_DB")
        .map(PathBuf::from)
        .map_err(|_| {
            "CONTEXTGRAPH_MEJEPA_INFER_DB must point to the Phase 4 inference RocksDB; refusing to guess a database path".to_string()
        })
}

fn run_predict_what_if(db_path: &Path, request: PredictWhatIfRequest) -> AnyhowResult<Value> {
    ensure_deterministic_verify_allowed().map_err(|err| anyhow!("{}: {err}", err.code()))?;
    let repo_root = request.context.environment.repo_root.clone();
    let disk_sha_before = file_sha_snapshot(&repo_root, &request.patch)
        .context("read repository file SHA snapshot before what-if prediction")?;
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let store = Arc::new(RocksDbInferStore::new(db.clone()));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store,
        calibration,
        MeJepaInferConfig::default(),
    )
    .context("build deterministic ME-JEPA compiler")?;
    let prediction = compiler
        .compile(&request.patch, &request.context)
        .context("compile what-if prediction")?;
    let disk_sha_after = file_sha_snapshot(&repo_root, &request.patch)
        .context("read repository file SHA snapshot after what-if prediction")?;
    if disk_sha_before != disk_sha_after {
        bail!("what-if prediction mutated at least one referenced repository file");
    }

    let compare_to_current = match request.compare_to_prediction_id {
        Some(raw) => {
            let prediction_id = parse_prediction_id_hex(&raw)?;
            let prior = find_prediction_by_id(db.as_ref(), prediction_id)?;
            json!({
                "predictionId": raw,
                "priorVerdict": prior.verdict,
                "priorConfidence": prior.calibrated_confidence,
                "newVerdict": prediction.verdict,
                "newConfidence": prediction.calibrated_confidence,
                "confidenceDelta": prediction.calibrated_confidence - prior.calibrated_confidence,
                "failureModeDelta": prediction.predicted_failure_modes.len() as i64
                    - prior.predicted_failure_modes.len() as i64,
                "securityConcernDelta": prediction.predicted_security_concerns.len() as i64
                    - prior.predicted_security_concerns.len() as i64
            })
        }
        None => Value::Null,
    };

    Ok(json!({
        "prediction": prediction,
        "compareToCurrent": compare_to_current,
        "diskShaBefore": disk_sha_before,
        "diskShaAfter": disk_sha_after,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "calibrationCf": CF_MEJEPA_CALIBRATION_HISTORY,
            "trainCertCf": CF_MEJEPA_TRAIN_CERTS,
            "repositoryRoot": repo_root.display().to_string(),
            "persistedPrediction": false
        }
    }))
}

fn run_explain_prediction(
    db_path: &Path,
    request: ExplainPredictionRequest,
) -> AnyhowResult<Value> {
    let prediction_id = parse_prediction_id_hex(&request.prediction_id)?;
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let prediction = find_prediction_by_id(db.as_ref(), prediction_id)?;
    let saliency = saliency_by_chunk(&prediction)?;
    Ok(json!({
        "predictionId": request.prediction_id,
        "verdict": prediction.verdict,
        "confidenceInterval": prediction.confidence_interval,
        "calibratedConfidence": prediction.calibrated_confidence,
        "oodScore": prediction.ood_score,
        "saliencyByChunk": saliency,
        "topFailureModes": prediction.predicted_failure_modes,
        "predictedFailedTests": prediction.predicted_failed_tests,
        "guardViolations": prediction.guard_violations,
        "edgeCases": prediction.predicted_edge_cases,
        "latentBugs": prediction.predicted_latent_bugs,
        "techDebt": prediction.predicted_tech_debt_added,
        "deadCode": prediction.predicted_dead_code,
        "redundantCode": prediction.predicted_redundant_code,
        "perfRegressions": prediction.predicted_perf_regressions,
        "securityConcerns": prediction.predicted_security_concerns,
        "accuracyDegradations": prediction.predicted_accuracy_degradations,
        "costRegressions": prediction.predicted_cost_regressions,
        "reasoningClass": prediction.predicted_reasoning_class,
        "agentClaimGraph": prediction.agent_claim_graph,
        "claimReconciliation": prediction.claim_reconciliation,
        "realityImpact": prediction.reality_impact,
        "provenance": prediction.provenance,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "cf": CF_MEJEPA_LIVE_PREDICTIONS,
            "predictionId": request.prediction_id
        }
    }))
}

fn file_sha_snapshot(
    repo_root: &Path,
    patch: &PatchBundle,
) -> AnyhowResult<BTreeMap<String, Option<String>>> {
    let mut out = BTreeMap::new();
    for hunk in &patch.ast_diff.hunks {
        let full_path = repo_root.join(&hunk.path);
        let value = match std::fs::read(&full_path) {
            Ok(bytes) => Some(hex::encode(Sha256::digest(&bytes))),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("read referenced file {}", full_path.display()))
            }
        };
        out.insert(hunk.path.display().to_string(), value);
    }
    Ok(out)
}

fn saliency_by_chunk(prediction: &RealityPrediction) -> AnyhowResult<BTreeMap<String, f64>> {
    if prediction.covered_chunks.is_empty() {
        bail!("prediction has no covered chunks; cannot build normalized saliency map");
    }
    let mut weights = prediction
        .covered_chunks
        .iter()
        .map(|chunk| (chunk.0.clone(), 1.0_f64))
        .collect::<BTreeMap<_, _>>();
    for mode in &prediction.predicted_failure_modes {
        *weights.entry(mode.chunk.0.clone()).or_insert(0.0) += 4.0;
    }
    for failed in &prediction.predicted_failed_tests {
        *weights.entry(failed.why.chunk.0.clone()).or_insert(0.0) += 3.0;
    }
    for item in &prediction.predicted_edge_cases {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 2.0;
    }
    for item in &prediction.predicted_latent_bugs {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 2.0;
    }
    for item in &prediction.predicted_tech_debt_added {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 1.5;
    }
    for item in &prediction.predicted_dead_code {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 1.0;
    }
    for item in &prediction.predicted_redundant_code {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 1.0;
    }
    for item in &prediction.predicted_perf_regressions {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 1.0;
    }
    for item in &prediction.predicted_security_concerns {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 4.0;
    }
    for item in &prediction.predicted_accuracy_degradations {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 2.0;
    }
    for item in &prediction.predicted_cost_regressions {
        *weights.entry(item.chunk.0.clone()).or_insert(0.0) += 1.0;
    }
    let total = weights.values().sum::<f64>();
    if !total.is_finite() || total <= 0.0 {
        bail!("prediction saliency weight total is invalid: {total}");
    }
    Ok(weights
        .into_iter()
        .map(|(chunk, weight)| (chunk, weight / total))
        .collect())
}

fn record_agent_feedback_in_db(
    db_path: &std::path::Path,
    request: RecordAgentFeedbackRequest,
) -> AnyhowResult<Value> {
    let prediction_id = parse_prediction_id_hex(&request.prediction_id)?;
    if request.agent_explanation.len() > 4096 {
        bail!("agentExplanation exceeds 4096 bytes");
    }
    let agent_id = AgentId::try_new(request.agent_id.clone())
        .map_err(|err| anyhow!("agentId validation failed: {err}"))?;
    let actual_outcome = request
        .actual_outcome
        .map(ActualOutcomeRequest::into_actual);
    let mut extra_structured_data = request.extra_structured_data;
    if extra_structured_data.is_null() {
        extra_structured_data = json!({});
    }
    if !extra_structured_data.is_object() {
        bail!("extraStructuredData must be a JSON object");
    }

    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let prediction = find_prediction_by_id(db.as_ref(), prediction_id)?;
    let mut ts_millis = chrono::Utc::now().timestamp_millis();
    let feedback_id;
    let persisted_feedback_key: Vec<u8>;
    loop {
        let candidate_id = feedback_id_for(
            prediction_id,
            &agent_id,
            ts_millis,
            request.feedback_kind,
            &request.agent_explanation,
        );
        let candidate_key = feedback_key(prediction_id, &agent_id, ts_millis)?;
        if db
            .get_cf(
                cf_handle(db.as_ref(), CF_MEJEPA_AGENT_FEEDBACK)?,
                &candidate_key,
            )?
            .is_none()
        {
            feedback_id = candidate_id;
            persisted_feedback_key = candidate_key;
            break;
        }
        ts_millis = ts_millis
            .checked_add(1)
            .ok_or_else(|| anyhow!("feedback timestamp overflow"))?;
    }

    let event = SurpriseEvent::try_new(SurpriseEvent {
        feedback_id,
        prediction_id,
        agent_id: agent_id.clone(),
        ts_millis,
        feedback_kind: request.feedback_kind,
        agent_explanation: request.agent_explanation,
        actual_outcome,
        severity: request.severity,
        extra_structured_data,
        witness_hash: WitnessHash(prediction.source_panel_sha),
    })
    .map_err(|err| anyhow!("SurpriseEvent validation failed: {err}"))?;

    let mut queue = RocksDbEvalStore::new(db.clone())
        .context("construct RocksDbEvalStore")?
        .load_queue()
        .context("load active-learning queue")?
        .unwrap_or(ActiveLearningQueueState::new(4096).context("create active-learning queue")?);
    let queue_count_before = queue.entries.len();
    let active_learning_queued = matches!(
        event.feedback_kind,
        FeedbackKind::Surprise | FeedbackKind::Omission
    );
    if active_learning_queued {
        queue
            .enqueue_agent_surprise(prediction.task_id.clone(), prediction_id, event.severity)
            .context("enqueue agent surprise")?;
    }

    persist_feedback_and_queue(db.as_ref(), &persisted_feedback_key, &event, &queue)?;
    let queue_entry = queue.entries.get(&prediction.task_id).cloned();
    let sampling_weight_multiplier = 1.0 + 2.0 * event.severity.severity_score();
    Ok(json!({
        "status": "recorded",
        "predictionId": hex::encode(prediction_id.0),
        "feedbackId": hex::encode(feedback_id.0),
        "agentId": event.agent_id.0,
        "feedbackKind": event.feedback_kind,
        "severity": event.severity,
        "severityScore": event.severity.severity_score(),
        "samplingWeightMultiplier": sampling_weight_multiplier,
        "activeLearningQueued": active_learning_queued,
        "catastrophicAlert": matches!(event.severity, SurpriseSeverity::Catastrophic),
        "queueCountBefore": queue_count_before,
        "queueCountAfter": queue.entries.len(),
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "feedbackCf": CF_MEJEPA_AGENT_FEEDBACK,
            "feedbackKeyHex": hex::encode(&persisted_feedback_key),
            "queueCf": CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            "queueKeyHex": hex::encode(b"active")
        },
        "readback": {
            "feedback": event,
            "queueEntry": queue_entry
        }
    }))
}

impl ActualOutcomeRequest {
    fn into_actual(self) -> ActualOutcome {
        ActualOutcome {
            oracle_outcome: self.oracle_outcome,
            failed_tests: self.failed_tests.into_iter().map(TestId).collect(),
            runtime_ms: self.runtime_ms,
            notes: self.notes,
        }
    }
}

fn parse_prediction_id_hex(raw: &str) -> AnyhowResult<PredictionId> {
    if raw.len() != 32 || !raw.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        bail!("predictionId must be exactly 32 hexadecimal characters");
    }
    let mut bytes = [0u8; 16];
    hex::decode_to_slice(raw, &mut bytes).context("decode predictionId")?;
    Ok(PredictionId(bytes))
}

fn find_prediction_by_id(db: &DB, prediction_id: PredictionId) -> AnyhowResult<RealityPrediction> {
    let cf = cf_handle(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let prediction: RealityPrediction =
            bincode::deserialize(&value).context("decode CF_MEJEPA_LIVE_PREDICTIONS row")?;
        prediction
            .validate()
            .map_err(|err| anyhow!("invalid persisted RealityPrediction: {err}"))?;
        if prediction.prediction_id == prediction_id.0 {
            return Ok(prediction);
        }
    }
    bail!(
        "predictionId={} was not found in CF_MEJEPA_LIVE_PREDICTIONS",
        hex::encode(prediction_id.0)
    )
}

fn feedback_id_for(
    prediction_id: PredictionId,
    agent_id: &AgentId,
    ts_millis: i64,
    feedback_kind: FeedbackKind,
    explanation: &str,
) -> FeedbackId {
    let mut hasher = Sha256::new();
    hasher.update(prediction_id.0);
    hasher.update(agent_id.0.as_bytes());
    hasher.update(ts_millis.to_be_bytes());
    hasher.update(serde_json::to_vec(&feedback_kind).unwrap_or_default());
    hasher.update(explanation.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    FeedbackId(out)
}

fn feedback_key(
    prediction_id: PredictionId,
    agent_id: &AgentId,
    ts_millis: i64,
) -> AnyhowResult<Vec<u8>> {
    bincode::serialize(&(prediction_id, agent_id, ts_millis)).context("encode feedback key")
}

fn persist_feedback_and_queue(
    db: &DB,
    feedback_key: &[u8],
    event: &SurpriseEvent,
    queue: &ActiveLearningQueueState,
) -> AnyhowResult<()> {
    let feedback_cf = cf_handle(db, CF_MEJEPA_AGENT_FEEDBACK)?;
    let queue_cf = cf_handle(db, CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?;
    let evictions_cf = cf_handle(db, CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS)?;
    let ood_cf = cf_handle(db, CF_MEJEPA_OOD_ESCALATIONS)?;
    let feedback_bytes = serde_json::to_vec(event).context("encode SurpriseEvent")?;
    let queue_bytes = bincode::serialize(queue).context("encode ActiveLearningQueueState")?;
    let mut batch = WriteBatch::default();
    batch.put_cf(feedback_cf, feedback_key, &feedback_bytes);
    batch.put_cf(queue_cf, b"active", &queue_bytes);
    for entry in &queue.evicted {
        batch.put_cf(
            evictions_cf,
            entry.task_id.0.as_bytes(),
            bincode::serialize(entry).context("encode active-learning eviction")?,
        );
    }
    for entry in &queue.ood_escalations {
        batch.put_cf(
            ood_cf,
            entry.task_id.0.as_bytes(),
            bincode::serialize(entry).context("encode OOD escalation")?,
        );
    }
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.write_opt(batch, &opts).context("atomic RocksDB write")?;

    let feedback_readback = db
        .get_cf(feedback_cf, feedback_key)?
        .ok_or_else(|| anyhow!("CF_MEJEPA_AGENT_FEEDBACK readback missing"))?;
    if feedback_readback != feedback_bytes {
        bail!("CF_MEJEPA_AGENT_FEEDBACK readback bytes differ");
    }
    let decoded_event: SurpriseEvent =
        serde_json::from_slice(&feedback_readback).context("decode SurpriseEvent readback")?;
    decoded_event
        .validate()
        .map_err(|err| anyhow!("SurpriseEvent readback validation failed: {err}"))?;
    if decoded_event != *event {
        bail!("CF_MEJEPA_AGENT_FEEDBACK decoded readback differs");
    }

    let queue_readback = db
        .get_cf(queue_cf, b"active")?
        .ok_or_else(|| anyhow!("CF_MEJEPA_ACTIVE_LEARNING_QUEUE readback missing"))?;
    if queue_readback != queue_bytes {
        bail!("CF_MEJEPA_ACTIVE_LEARNING_QUEUE readback bytes differ");
    }
    let _decoded_queue: ActiveLearningQueueState =
        bincode::deserialize(&queue_readback).context("decode queue readback")?;
    Ok(())
}

fn cf_handle<'a>(db: &'a DB, name: &str) -> AnyhowResult<&'a rocksdb::ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| anyhow!("missing RocksDB column family {name}"))
}

fn count_cf_any(db: &DB, name: &str) -> AnyhowResult<u64> {
    let cf = cf_handle(db, name)?;
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

fn ensure_deterministic_verify_allowed() -> Result<(), context_graph_mejepa::MejepaInferError> {
    match std::env::var(ENV_ALLOW_DETERMINISTIC_VERIFY) {
        Ok(value) if value.eq_ignore_ascii_case("true") || value == "1" => Ok(()),
        Ok(value) => Err(context_graph_mejepa::MejepaInferError::FeatureDisabled {
            feature: format!(
                "deterministic ME-JEPA verify harness requires {ENV_ALLOW_DETERMINISTIC_VERIFY}=true or 1; observed {value:?}"
            ),
        }),
        Err(std::env::VarError::NotPresent) => {
            Err(context_graph_mejepa::MejepaInferError::FeatureDisabled {
                feature: format!(
                    "deterministic ME-JEPA verify harness requires {ENV_ALLOW_DETERMINISTIC_VERIFY}=true or 1"
                ),
            })
        }
        Err(err) => Err(context_graph_mejepa::MejepaInferError::FeatureDisabled {
            feature: format!("failed to read {ENV_ALLOW_DETERMINISTIC_VERIFY}: {err}"),
        }),
    }
}

fn parse_runtime_versions(
    input: &std::collections::BTreeMap<String, String>,
) -> Result<std::collections::BTreeMap<EmbedderId, [u8; 32]>, String> {
    if input.len() != EmbedderId::all().len() {
        return Err(format!(
            "runtimeEmbedderVersions must contain exactly {} entries, got {}",
            EmbedderId::all().len(),
            input.len()
        ));
    }
    let mut out = std::collections::BTreeMap::new();
    for (key, value) in input {
        let embedder = EmbedderId::from_str(key)
            .map_err(|err| format!("invalid embedder id {key:?}: {err}"))?;
        let digest = parse_version_id(value)
            .map_err(|err| format!("invalid digest for {embedder}: {err}"))?;
        out.insert(embedder, digest);
    }
    for embedder in EmbedderId::all() {
        if !out.contains_key(&embedder) {
            return Err(format!("runtimeEmbedderVersions missing {embedder}"));
        }
    }
    Ok(out)
}

fn parse_version_id(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!(
            "expected exactly 64 hexadecimal characters, got {value:?}"
        ));
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(value, &mut out)
        .map_err(|err| format!("hex decode failed for {value:?}: {err}"))?;
    Ok(out)
}

#[cfg(test)]
mod mejepa_constellation_inspect_tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Mutex, MutexGuard};
    use std::time::{Duration, SystemTime};

    use context_graph_mejepa_tct::{
        Centroid, CorpusProvenance, EntityType, Language, MutationCategory, OracleOutcome,
        ShrinkageOrigin, TctConstellation, Thresholds,
    };
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_guard() -> MutexGuard<'static, ()> {
        let guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var(ENV_TCT_DB);
        std::env::remove_var(ENV_ALLOW_DETERMINISTIC_VERIFY);
        std::env::remove_var(context_graph_mejepa_tct::ENV_ALLOW_STALE);
        std::env::remove_var(context_graph_mejepa_tct::ENV_MAX_AGE_DAYS);
        guard
    }

    fn versions(seed: u8) -> BTreeMap<EmbedderId, [u8; 32]> {
        EmbedderId::all()
            .into_iter()
            .enumerate()
            .map(|(idx, embedder)| (embedder, [seed.wrapping_add(idx as u8); 32]))
            .collect()
    }

    fn centroid(sample_count: usize) -> Centroid {
        Centroid::try_new(
            vec![1.0],
            sample_count,
            ShrinkageOrigin::OwnCell,
            "mcp-test",
        )
        .unwrap()
    }

    fn by_embedder(sample_count: usize) -> BTreeMap<EmbedderId, Centroid> {
        EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, centroid(sample_count)))
            .collect()
    }

    fn thresholds() -> Thresholds {
        let panel_level = EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, 0.8))
            .collect();
        let per_chunk_type = EmbedderId::all()
            .into_iter()
            .map(|embedder| ((EntityType::Function, embedder), 0.9))
            .collect();
        Thresholds::try_new(panel_level, per_chunk_type).unwrap()
    }

    fn constellation(frozen_at: SystemTime, seed: u8) -> TctConstellation {
        let embedder_versions = versions(seed);
        let provenance =
            CorpusProvenance::try_new([seed; 32], embedder_versions, frozen_at, "a".repeat(40))
                .unwrap();
        let mut per_category = BTreeMap::new();
        per_category.insert(MutationCategory::KnownGood, by_embedder(50));
        let mut per_language = BTreeMap::new();
        per_language.insert(
            (Language::Python, MutationCategory::KnownGood),
            by_embedder(50),
        );
        let mut outcomes = BTreeMap::new();
        outcomes.insert(OracleOutcome::Pass, by_embedder(50));
        let per_chunk = EmbedderId::all()
            .into_iter()
            .map(|embedder| {
                (
                    (
                        MutationCategory::KnownGood,
                        EntityType::Function,
                        Language::Python,
                        embedder,
                    ),
                    centroid(50),
                )
            })
            .collect();
        TctConstellation::try_new(
            per_category,
            per_language,
            outcomes,
            per_chunk,
            thresholds(),
            provenance,
            frozen_at,
        )
        .unwrap()
    }

    fn persist(temp: &TempDir, constellation: &TctConstellation) {
        let db = open_tct_rocksdb(temp.path()).unwrap();
        let store = ConstellationStore::new(db).unwrap();
        store.persist(constellation).unwrap();
    }

    fn contains_key(value: &serde_json::Value, needle: &str) -> bool {
        match value {
            serde_json::Value::Object(map) => map
                .iter()
                .any(|(key, child)| key == needle || contains_key(child, needle)),
            serde_json::Value::Array(items) => {
                items.iter().any(|child| contains_key(child, needle))
            }
            _ => false,
        }
    }

    #[test]
    fn mejepa_constellation_inspect_returns_summary_for_loaded_version() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 1);
        persist(&temp, &constellation);
        let output = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(constellation.version_id())),
            &constellation.corpus_provenance.embedder_versions,
        )
        .unwrap();
        assert_eq!(
            output["version_id"],
            hex::encode(constellation.version_id())
        );
        assert_eq!(output["cell_counts"]["panel_level_cells"], 63);
        assert_eq!(output["cell_counts"]["per_chunk_type_cells"], 21);
        assert_eq!(output["sample_support_histogram"]["n_50_plus"], 21);
        assert_eq!(
            output["per_chunk_type_threshold_summary"]["function"]["n_cells"],
            21
        );
    }

    #[test]
    fn mejepa_constellation_inspect_does_not_leak_raw_centroid_vectors() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 2);
        persist(&temp, &constellation);
        let output = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(constellation.version_id())),
            &constellation.corpus_provenance.embedder_versions,
        )
        .unwrap();
        assert!(!contains_key(&output, "values"));
        assert!(!contains_key(&output, "per_category_centroids"));
        assert!(!contains_key(&output, "per_chunk_type_centroids"));
    }

    #[test]
    fn mejepa_constellation_inspect_loads_newest_frozen_at_when_version_omitted() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let older = constellation(SystemTime::now() - Duration::from_secs(60), 3);
        let newer = constellation(SystemTime::now(), 4);
        persist(&temp, &older);
        persist(&temp, &newer);
        let output = run_constellation_inspect(
            temp.path(),
            None,
            &newer.corpus_provenance.embedder_versions,
        )
        .unwrap();
        assert_eq!(output["version_id"], hex::encode(newer.version_id()));
    }

    #[test]
    fn mejepa_constellation_inspect_surfaces_provenance_mismatch() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 5);
        persist(&temp, &constellation);
        let mut runtime = constellation.corpus_provenance.embedder_versions.clone();
        runtime.insert(EmbedderId::E3, [99; 32]);
        let err = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(constellation.version_id())),
            &runtime,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TCT_PROVENANCE_MISMATCH");
    }

    #[test]
    fn mejepa_constellation_inspect_rejects_stale_constellation() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let stale = constellation(SystemTime::now() - Duration::from_secs(91 * 86_400), 6);
        persist(&temp, &stale);
        let err = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(stale.version_id())),
            &stale.corpus_provenance.embedder_versions,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_GTAU_STALE_CONSTELLATION");
    }

    #[test]
    fn mejepa_constellation_inspect_rejects_malformed_version_id() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 7);
        persist(&temp, &constellation);
        let err = run_constellation_inspect(
            temp.path(),
            Some("not-a-64-byte-hex-digest"),
            &constellation.corpus_provenance.embedder_versions,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TCT_INVALID_INPUT");
    }

    #[test]
    fn mejepa_constellation_inspect_requires_db_path_or_env() {
        let _guard = env_guard();
        let err = resolve_tct_db_path(None).unwrap_err();
        assert!(err.contains(ENV_TCT_DB));
        assert!(err.contains("refusing to guess"));
    }

    #[test]
    fn mejepa_constellation_inspect_rejects_empty_store() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let runtime = versions(8);
        let err = run_constellation_inspect(temp.path(), None, &runtime).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TCT_MISSING_CENTROID");
    }

    #[test]
    fn mejepa_verify_requires_explicit_deterministic_harness_opt_in() {
        let _guard = env_guard();
        let err = ensure_deterministic_verify_allowed().unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_FEATURE_DISABLED");
        std::env::set_var(ENV_ALLOW_DETERMINISTIC_VERIFY, "true");
        ensure_deterministic_verify_allowed().unwrap();
    }

    #[test]
    fn mejepa_heal_status_and_rollback_read_persisted_source_of_truth() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("heal-db");
        let chain_path = temp.path().join("chain.bin");
        let storage = HealRocksStore::open(&db_path).unwrap();
        let mut witness_chain = WitnessChainAppender::new(chain_path.clone()).unwrap();
        let holdout = context_graph_mejepa::heal::HoldoutDataset::try_new(
            (0..50)
                .map(|idx| context_graph_mejepa::heal::HoldoutExample {
                    predicted: vec![context_graph_mejepa::OracleOutcome::Pass],
                    actual: if idx % 5 == 0 {
                        context_graph_mejepa::OracleOutcome::Fail
                    } else {
                        context_graph_mejepa::OracleOutcome::Pass
                    },
                    ood_score: 0.05,
                })
                .collect(),
            [7; 32],
        )
        .unwrap();
        let lock = std::sync::Arc::new(std::sync::Mutex::new(PromotionLockState::default()));
        let mut promoter = AbcPromoter::try_new(0.1, PromotionGate::default()).unwrap();
        let report = promoter
            .retrain_and_promote(context_graph_mejepa::heal::RetrainPromoteRequest {
                trigger_reason: context_graph_mejepa::heal::TriggerReason::DriftHard,
                current_weights: &[0.2; 16],
                storage,
                witness_chain: &mut witness_chain,
                holdout,
                lock,
                calibration_version: "mcp-test-calibration",
            })
            .unwrap();

        let status = run_heal_status(&db_path).unwrap();
        assert_eq!(status["cfCounts"][CF_MEJEPA_HEAL_REPORTS], 1);
        assert_eq!(
            status["latestHealReport"]["witnessChainOffset"],
            serde_json::json!(report.witness_chain_offset)
        );
        let rollback = run_rollback_to(&db_path, chain_path, report.witness_chain_offset).unwrap();
        assert_eq!(rollback["targetWitnessChainOffset"], 0);
        let status_after = run_heal_status(&db_path).unwrap();
        assert_eq!(status_after["cfCounts"][CF_MEJEPA_ACTIVE_POINTERS], 1);
    }

    #[tokio::test]
    async fn mejepa_daemon_status_aggregates_real_sources_of_truth() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let temp = TempDir::new().unwrap();
        let infer_db = temp.path().join("infer-db");
        let panel_db = temp.path().join("panel-db");
        let heal_db = temp.path().join("heal-db");
        let quota_db = temp.path().join("quota-db");
        let archive_root = temp.path().join("archive");

        let infer = open_infer_rocksdb(&infer_db).unwrap();
        drop(infer);
        let panel =
            context_graph_mejepa_instruments::panel_store::PanelStore::open(&panel_db).unwrap();
        drop(panel);
        let heal = HealRocksStore::open(&heal_db).unwrap();
        drop(heal);

        let response = handlers
            .call_mejepa_daemon_status(
                Some(JsonRpcId::Number(40)),
                json!({
                    "inferDbPath": infer_db,
                    "panelDbPath": panel_db,
                    "healDbPath": heal_db,
                    "quotaDbPath": quota_db,
                    "archiveRoot": archive_root,
                    "includeVram": false
                }),
            )
            .await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["overallStatus"], "healthy");
        assert_eq!(structured["components"]["runtime"]["status"], "healthy");
        assert_eq!(structured["components"]["subscriber"]["status"], "healthy");
        assert_eq!(structured["components"]["heal"]["status"], "healthy");
        assert_eq!(structured["components"]["quota"]["status"], "healthy");
        assert_eq!(structured["components"]["vram"]["status"], "disabled");
        assert_eq!(
            structured["components"]["subscriber"]["data"]["source_of_truth"],
            json!(format!("file:{}", temp.path().join("infer-db").display()))
        );
    }

    #[tokio::test]
    async fn mejepa_daemon_status_reports_missing_component_sots() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let temp = TempDir::new().unwrap();
        let bad_infer_db = temp.path().join("not-a-rocksdb-infer-file");
        let bad_heal_db = temp.path().join("not-a-rocksdb-heal-file");
        let quota_db = temp.path().join("quota-db");
        let nested_archive_root = quota_db.join("archive");
        std::fs::write(&bad_infer_db, b"not rocksdb").unwrap();
        std::fs::write(&bad_heal_db, b"not rocksdb").unwrap();
        let response = handlers
            .call_mejepa_daemon_status(
                Some(JsonRpcId::Number(41)),
                json!({
                    "inferDbPath": bad_infer_db,
                    "healDbPath": bad_heal_db,
                    "quotaDbPath": quota_db,
                    "archiveRoot": nested_archive_root,
                    "includeVram": false
                }),
            )
            .await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["overallStatus"], "degraded");
        assert_eq!(
            structured["components"]["subscriber"]["errorCode"],
            "MEJEPA_DAEMON_STATUS_SUBSCRIBER_READ_FAILED"
        );
        assert_eq!(
            structured["components"]["quota"]["errorCode"],
            "MEJEPA_HYGIENE_INVALID_CONFIG"
        );
    }

    #[tokio::test]
    async fn mejepa_daemon_status_rejects_unknown_args() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let response = handlers
            .call_mejepa_daemon_status(
                Some(JsonRpcId::Number(42)),
                json!({"includeVram": false, "unexpected": true}),
            )
            .await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], true);
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("schema validation failed"));
    }

    #[tokio::test]
    async fn mejepa_daemon_status_writes_phase_f_fsv_artifact() {
        let _guard = env_guard();
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
        let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-daemon-status-fsv");
        std::fs::create_dir_all(&fsv_root).unwrap();
        let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
        std::fs::create_dir_all(&run_root).unwrap();

        let infer_db = run_root.join("infer-db");
        let panel_db = run_root.join("panel-db");
        let heal_db = run_root.join("heal-db");
        let quota_db = run_root.join("quota-db");
        let archive_root = run_root.join("archive");

        let infer = open_infer_rocksdb(&infer_db).unwrap();
        drop(infer);
        let panel =
            context_graph_mejepa_instruments::panel_store::PanelStore::open(&panel_db).unwrap();
        drop(panel);
        let heal = HealRocksStore::open(&heal_db).unwrap();
        drop(heal);

        let happy_response = handlers
            .call_mejepa_daemon_status(
                Some(JsonRpcId::Number(43)),
                json!({
                    "inferDbPath": infer_db,
                    "panelDbPath": panel_db,
                    "healDbPath": heal_db,
                    "quotaDbPath": quota_db,
                    "archiveRoot": archive_root,
                    "includeVram": false
                }),
            )
            .await;
        assert!(happy_response.error.is_none());
        let happy_result = happy_response.result.unwrap();
        let happy_structured = happy_result["structuredContent"].clone();
        let happy_pass = happy_result["isError"] == false
            && happy_structured["overallStatus"] == "healthy"
            && happy_structured["components"]["runtime"]["status"] == "healthy"
            && happy_structured["components"]["subscriber"]["status"] == "healthy"
            && happy_structured["components"]["heal"]["status"] == "healthy"
            && happy_structured["components"]["quota"]["status"] == "healthy"
            && happy_structured["components"]["vram"]["status"] == "disabled";

        let missing_response = handlers
            .call_mejepa_daemon_status(Some(JsonRpcId::Number(44)), json!({"includeVram": false}))
            .await;
        assert!(missing_response.error.is_none());
        let missing_structured = missing_response.result.unwrap()["structuredContent"].clone();

        let unknown_response = handlers
            .call_mejepa_daemon_status(
                Some(JsonRpcId::Number(45)),
                json!({"includeVram": false, "unexpected": true}),
            )
            .await;
        assert!(unknown_response.error.is_none());
        let unknown_result = unknown_response.result.unwrap();

        let bad_infer_db = run_root.join("not-a-rocksdb-infer-file");
        let bad_heal_db = run_root.join("not-a-rocksdb-heal-file");
        std::fs::write(&bad_infer_db, b"not rocksdb").unwrap();
        std::fs::write(&bad_heal_db, b"not rocksdb").unwrap();
        let bad_response = handlers
            .call_mejepa_daemon_status(
                Some(JsonRpcId::Number(46)),
                json!({
                    "inferDbPath": bad_infer_db,
                    "healDbPath": bad_heal_db,
                    "quotaDbPath": run_root.join("bad-quota-db"),
                    "archiveRoot": run_root.join("bad-quota-db/archive"),
                    "includeVram": false
                }),
            )
            .await;
        assert!(bad_response.error.is_none());
        let bad_structured = bad_response.result.unwrap()["structuredContent"].clone();

        let boundary_cases = vec![
            json!({
                "case": "missing_component_sources_degrades",
                "expected": "degraded",
                "actual": missing_structured["overallStatus"],
                "pass": missing_structured["overallStatus"] == "degraded"
                    && missing_structured["components"]["subscriber"]["errorCode"]
                        == "MEJEPA_DAEMON_STATUS_INFER_DB_MISSING"
                    && missing_structured["components"]["heal"]["errorCode"]
                        == "MEJEPA_DAEMON_STATUS_HEAL_DB_MISSING"
            }),
            json!({
                "case": "unknown_argument_fails_schema",
                "expected": "schema validation failed",
                "actual": unknown_result,
                "pass": unknown_result["isError"] == true
                    && unknown_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("schema validation failed")
            }),
            json!({
                "case": "malformed_component_sots_are_component_errors",
                "expected": {
                    "overallStatus": "degraded",
                    "subscriberErrorCode": "MEJEPA_DAEMON_STATUS_SUBSCRIBER_READ_FAILED"
                },
                "actual": bad_structured,
                "pass": bad_structured["overallStatus"] == "degraded"
                    && bad_structured["components"]["subscriber"]["errorCode"]
                        == "MEJEPA_DAEMON_STATUS_SUBSCRIBER_READ_FAILED"
            }),
        ];
        let all_passed = happy_pass && boundary_cases.iter().all(|case| case["pass"] == true);
        let evidence = json!({
            "fsv_root": fsv_root,
            "task_id": "TASK-OBS-003",
            "started_at_unix_ms": started_at_unix_ms,
            "build_release_sha": git_head_short_for_test(),
            "happy_path": [{
                "case": "daemon_status_aggregates_sources",
                "sot": "mcp__cgreality__mejepa_daemon_status structuredContent",
                "before": null,
                "trigger": "cargo test -p context-graph-mcp mejepa_daemon_status_writes_phase_f_fsv_artifact",
                "after": happy_structured,
                "expected": "runtime/subscriber/heal/quota healthy and VRAM disabled",
                "actual": happy_pass,
                "pass": happy_pass,
                "evidence_path": run_root
            }],
            "boundary_cases": boundary_cases,
            "all_passed": all_passed,
            "readback_equal": true,
            "physical_artifacts": {
                "run_root": run_root
            }
        });
        assert!(all_passed);
        let evidence_path = fsv_root.join("daemon_status_fsv.json");
        context_graph_mejepa::eval::report::write_json_0600(&evidence_path, &evidence).unwrap();
        let readback: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&evidence_path).unwrap()).unwrap();
        assert_eq!(readback, evidence);
        let metadata = std::fs::metadata(&evidence_path).unwrap();
        assert!(metadata.len() > 0);
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
