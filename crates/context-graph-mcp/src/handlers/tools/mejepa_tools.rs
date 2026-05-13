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
use context_graph_mejepa::operator_override::{
    count_operator_overrides, load_operator_override, operator_override_flags_for_predictions,
    persist_operator_override, OperatorOverride, OverrideVerdict,
};
use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, ActiveLearningQueueState, ActualOutcome,
    AgentId, CalibrationStore, ChunkId, DdaSignals, FailedGate, FeedbackId, FeedbackKind,
    LabelMethod, MeJepaInferConfig, OracleOutcome, PanelId, PatchBundle, PredictionId,
    RealityPrediction, RocksDbEvalStore, RocksDbInferStore, SurpriseEvent, SurpriseSeverity,
    TaskContext, TestId, VerifyVerdict, WitnessHash,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_AGENT_FEEDBACK, CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_OPERATOR_OVERRIDES, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_embedders::{query_vram_budget, VramBudget};
use context_graph_mejepa_hygiene::{mcp_quota_status, HygieneMcpRequest};
use context_graph_mejepa_tct::{
    build_inspect_summary, open_tct_rocksdb, ConstellationStore, EmbedderId,
};
use rocksdb::{IteratorMode, WriteBatch, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::info;

use super::mejepa_phase7_storage::{subscriber_state_for_provenance, subscriber_status_for_paths};
use super::mejepa_agent_identity::{
    resolve_feedback_identity, resolve_operator_identity, IdentityAttestationRequest,
    ResolvedAgentIdentity, MEJEPA_AGENT_IDENTITY_CONFIG_INVALID, MEJEPA_AGENT_IDENTITY_UNVERIFIED,
};
use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::telemetry::gpu_wsl::{default_query as gpu_wsl_default_query, nvidia_smi_query};
use crate::tools::names as tool_names;

const ENV_TCT_DB: &str = "CONTEXTGRAPH_MEJEPA_TCT_DB";
const ENV_HEAL_DB: &str = "CONTEXTGRAPH_MEJEPA_HEAL_DB";
const ENV_HYGIENE_ARCHIVE_ROOT: &str = "CONTEXTGRAPH_MEJEPA_HYGIENE_ARCHIVE_ROOT";
const ENV_PANEL_DB: &str = "CONTEXTGRAPH_MEJEPA_PANEL_DB";
const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";
const ENV_ALLOW_DETERMINISTIC_VERIFY: &str = "CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY";
const ENV_PAUSE_PATH: &str = "CONTEXTGRAPH_MEJEPA_PAUSE_PATH";
const DEFAULT_PAUSE_STATE_PATH: &str =
    "/mnt/d/contextgraph/state/cgreality/predictions_paused_until.json";

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
struct InspectPredictionRequest {
    prediction_id: String,
    db_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReplayPredictionRequest {
    prediction_id: String,
    db_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordAgentFeedbackRequest {
    prediction_id: String,
    #[serde(default)]
    agent_id: Option<String>,
    feedback_kind: FeedbackKind,
    agent_explanation: String,
    actual_outcome: Option<ActualOutcomeRequest>,
    severity: SurpriseSeverity,
    #[serde(default)]
    extra_structured_data: Value,
    identity_attestation: Option<IdentityAttestationRequest>,
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
struct PausePredictionsRequest {
    state_path: Option<PathBuf>,
    duration_mins: u64,
    #[serde(default = "default_pause_reason")]
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperatorOverridePredictionRequest {
    db_path: Option<PathBuf>,
    prediction_id: String,
    override_verdict: OverrideVerdict,
    reason: String,
    operator_id: String,
    identity_attestation: Option<IdentityAttestationRequest>,
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

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DaemonStatusVramBudget {
    #[default]
    ContentSet,
    FullPhase1,
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

fn default_pause_reason() -> String {
    "manual pause".to_string()
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
        let pause_path = match resolve_pause_state_path(None) {
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
        let now_ms = match now_unix_ms() {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_structured(
                    id,
                    ToolErrorKind::Execution,
                    "MEJEPA_PAUSE_CLOCK_FAILED",
                    &err.to_string(),
                    json!({"statePath": pause_path}),
                );
            }
        };
        match active_pause_state(&pause_path, now_ms) {
            Ok(Some(pause_state)) => {
                let verdict = VerifyVerdict::EscalateToHuman {
                    reality_prediction: None,
                    failed_gate: FailedGate::PredictionPaused {
                        paused_until_unix_ms: pause_state.paused_until_unix_ms,
                        reason: pause_state.reason.clone(),
                    },
                    gates_passed: 0,
                };
                let mut response = match serde_json::to_value(verdict) {
                    Ok(value) => value,
                    Err(err) => {
                        return self.tool_error_structured(
                            id,
                            ToolErrorKind::Execution,
                            "MEJEPA_PAUSE_RESPONSE_SERIALIZE_FAILED",
                            &err.to_string(),
                            json!({"statePath": pause_path}),
                        );
                    }
                };
                response["pause_state"] = pause_state_value(&pause_path, &pause_state, now_ms);
                if request.include_provenance {
                    response["provenance"] = json!({
                        "codeVersion": env!("CARGO_PKG_VERSION"),
                        "dbPath": db_path,
                        "mode": "deterministic_phase4_harness",
                        "pauseStatePath": pause_path,
                    });
                }
                return self.tool_result(id, response);
            }
            Ok(None) => {}
            Err(err) => {
                return self.tool_error_structured(
                    id,
                    ToolErrorKind::Storage,
                    "MEJEPA_PAUSE_STATE_INVALID",
                    &err.to_string(),
                    json!({"statePath": pause_path}),
                );
            }
        }
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

    pub(crate) async fn call_mejepa_inspect_prediction(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: InspectPredictionRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_INSPECT_PREDICTION
                    ),
                );
            }
        };
        let db_path = match resolve_optional_infer_db_path(request.db_path.as_ref()) {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result = run_inspect_prediction(&db_path, request);
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_INSPECT_PREDICTION_FAILED",
                &err.to_string(),
                json!({"dbPath": db_path}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_replay_prediction(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: ReplayPredictionRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_REPLAY_PREDICTION
                    ),
                );
            }
        };
        let db_path = match resolve_optional_infer_db_path(request.db_path.as_ref()) {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let prediction_id =
            match context_graph_mejepa::parse_prediction_id_hex(&request.prediction_id) {
                Ok(value) => value,
                Err(err) => {
                    return self.tool_error_structured(
                        id,
                        ToolErrorKind::Validation,
                        "MEJEPA_REPLAY_PREDICTION_ID_INVALID",
                        &err.to_string(),
                        json!({"predictionId": request.prediction_id}),
                    );
                }
            };
        let result = context_graph_mejepa::replay_prediction_from_db(&db_path, prediction_id);
        match result {
            Ok(value) => match serde_json::to_value(value) {
                Ok(value) => self.tool_result(id, value),
                Err(err) => self.tool_error_structured(
                    id,
                    ToolErrorKind::Execution,
                    "MEJEPA_REPLAY_PREDICTION_SERIALIZE_FAILED",
                    &err.to_string(),
                    json!({"dbPath": db_path}),
                ),
            },
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_REPLAY_PREDICTION_FAILED",
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
            Err(err) => {
                let message = err.to_string();
                let (kind, code) = mejepa_write_error_classification(
                    &message,
                    ToolErrorKind::Storage,
                    "MEJEPA_RECORD_AGENT_FEEDBACK_FAILED",
                );
                self.tool_error_structured(id, kind, code, &message, json!({"dbPath": db_path}))
            }
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

    pub(crate) async fn call_mejepa_pause_predictions(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: PausePredictionsRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_PAUSE_PREDICTIONS
                    ),
                );
            }
        };
        let state_path = match resolve_pause_state_path(request.state_path) {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match write_pause_state(&state_path, request.duration_mins, &request.reason) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_PAUSE_PREDICTIONS_FAILED",
                &err.to_string(),
                json!({"statePath": state_path}),
            ),
        }
    }

    pub(crate) async fn call_mejepa_operator_override_prediction(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: OperatorOverridePredictionRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_OPERATOR_OVERRIDE_PREDICTION
                    ),
                );
            }
        };
        let db_path = match resolve_optional_infer_db_path(request.db_path.as_ref()) {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match record_operator_override_in_db(&db_path, request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                let message = err.to_string();
                let (kind, code) = mejepa_write_error_classification(
                    &message,
                    ToolErrorKind::Storage,
                    "MEJEPA_OPERATOR_OVERRIDE_FAILED",
                );
                self.tool_error_structured(id, kind, code, &message, json!({"dbPath": db_path}))
            }
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
            let nvidia_smi = match nvidia_smi_query(gpu_wsl_default_query()).await {
                Ok(value) => json!({
                    "status": "available",
                    "unavailable": !value.unavailable_fields.is_empty(),
                    "telemetry": value
                }),
                Err(err) => json!({
                    "status": "unavailable",
                    "unavailable": true,
                    "fault": err
                }),
            };
            match query_vram_budget(request.vram_budget.as_budget()) {
                Ok(value) => healthy_component(json!({
                    "cudaDriverBudget": value,
                    "nvidiaSmi": nvidia_smi,
                    "sourceOfTruth": "cuda_driver_cuMemGetInfo_v2",
                    "nvidiaSmiAuthoritative": false
                })),
                Err(err) => unavailable_component(
                    err.code().to_string(),
                    err.to_string(),
                    json!({
                        "budget": format!("{:?}", request.vram_budget),
                        "nvidiaSmi": nvidia_smi
                    }),
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
                "vram": "CUDA driver cuMemGetInfo_v2; nvidia-smi diagnostic is non-authoritative under WSL"
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

fn resolve_optional_infer_db_path(input: Option<&PathBuf>) -> Result<PathBuf, String> {
    match input {
        Some(path) if path.as_os_str().is_empty() => {
            Err("dbPath must be a non-empty path".to_string())
        }
        Some(path) => Ok(path.clone()),
        None => infer_db_path(),
    }
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

fn run_inspect_prediction(
    db_path: &Path,
    request: InspectPredictionRequest,
) -> AnyhowResult<Value> {
    let prediction_id = parse_prediction_id_hex(&request.prediction_id)?;
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let row = find_prediction_row_by_id(db.as_ref(), prediction_id)?;
    let saliency = saliency_by_chunk(&row.prediction)?;
    let contributing_chunks = inspect_contributing_chunks(db.as_ref(), &row.prediction, &saliency)?;
    let granger_mean = mean_prediction_attestation(&row.prediction.granger_attestations);

    Ok(json!({
        "predictionId": request.prediction_id,
        "predictionIdentity": {
            "predictionId": hex::encode(row.prediction.prediction_id),
            "taskId": row.prediction.task_id.0.clone(),
            "sessionId": hex::encode(row.prediction.session_id),
            "language": row.prediction.language,
            "createdAtUnixMs": row.prediction.created_at_unix_ms
        },
        "witnessHash": hex::encode(row.prediction.witness_hash.0),
        "sourcePanelSha": hex::encode(row.prediction.source_panel_sha),
        "versions": {
            "predictorVersion": row.prediction.provenance.predictor_version.clone(),
            "constellationVersion": row.prediction.provenance.constellation_version.clone(),
            "calibrationVersion": row.prediction.provenance.calibration_version.clone(),
            "activePointer": row.prediction.provenance.active_pointer.clone(),
            "predictionCalibrationVersion": row.prediction.calibration_version.clone()
        },
        "conformalTrace": {
            "steps": [
                {
                    "step": "load_prediction_row",
                    "cf": CF_MEJEPA_LIVE_PREDICTIONS,
                    "keyHex": hex::encode(&row.key),
                    "valueSha256": row.value_sha256_hex,
                    "valueBytes": row.value_len
                },
                {
                    "step": "load_calibration_version",
                    "calibrationVersion": row.prediction.calibration_version.clone()
                },
                {
                    "step": "assemble_outcome_set",
                    "alpha": row.prediction.outcome_set.alpha,
                    "tau": row.prediction.outcome_set.tau,
                    "entropyBits": row.prediction.outcome_set.entropy_bits,
                    "outcomes": row.prediction.outcome_set.outcomes.clone()
                },
                {
                    "step": "compute_confidence_interval",
                    "method": row.prediction.confidence_interval.method,
                    "coverageTarget": row.prediction.confidence_interval.coverage_target,
                    "empiricalCoverage": row.prediction.confidence_interval.empirical_coverage,
                    "lower": row.prediction.confidence_interval.lower,
                    "upper": row.prediction.confidence_interval.upper,
                    "width": row.prediction.confidence_interval.width()
                },
                {
                    "step": "calibrate_confidence",
                    "predictedOraclePass": row.prediction.predicted_oracle_pass,
                    "oodScore": row.prediction.ood_score,
                    "grangerAttestationMean": granger_mean,
                    "calibratedConfidence": row.prediction.calibrated_confidence,
                    "degradedStatus": row.prediction.degraded_status
                }
            ],
            "predictedTestPass": row.prediction.predicted_test_pass.clone(),
            "grangerAttestations": row.prediction.granger_attestations.clone()
        },
        "tctTrace": {
            "constellationVersion": row.prediction.provenance.constellation_version.clone(),
            "guardViolations": row.prediction.guard_violations.clone(),
            "guardViolationCount": row.prediction.guard_violations.len()
        },
        "contributingChunks": contributing_chunks,
        "prediction": row.prediction,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "livePredictionKeyHex": hex::encode(&row.key),
            "livePredictionValueSha256": row.value_sha256_hex,
            "ddaSignalsCf": CF_MEJEPA_DDA_SIGNALS,
            "readbackVerified": true
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
    let identity = resolve_feedback_identity(
        request.agent_id.as_deref(),
        request.identity_attestation.as_ref(),
        tool_names::MEJEPA_RECORD_AGENT_FEEDBACK,
        chrono::Utc::now().timestamp_millis(),
    )?;
    if request.agent_explanation.len() > 4096 {
        bail!("agentExplanation exceeds 4096 bytes");
    }
    let agent_id = AgentId::try_new(identity.id.clone())
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
    let mut active_learning_queued = matches!(
        event.feedback_kind,
        FeedbackKind::Surprise | FeedbackKind::Omission
    );
    if active_learning_queued {
        active_learning_queued = queue
            .enqueue_agent_surprise_for_prediction(&prediction, event.severity)
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
        "identity": identity_status_json(&identity),
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

fn record_operator_override_in_db(
    db_path: &Path,
    request: OperatorOverridePredictionRequest,
) -> AnyhowResult<Value> {
    let prediction_id = parse_prediction_id_hex(&request.prediction_id)?;
    let identity = resolve_operator_identity(
        &request.operator_id,
        request.identity_attestation.as_ref(),
        tool_names::MEJEPA_OPERATOR_OVERRIDE_PREDICTION,
        chrono::Utc::now().timestamp_millis(),
    )?;
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let prediction = find_prediction_by_id(db.as_ref(), prediction_id)?;
    let created_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let override_record = OperatorOverride::new(
        prediction_id,
        request.override_verdict,
        request.reason,
        request.operator_id,
        created_at_unix_ms,
    )
    .map_err(|err| anyhow!("{err}"))?;
    let override_count_before =
        count_operator_overrides(db.as_ref()).map_err(|err| anyhow!("{err}"))?;
    persist_operator_override(db.as_ref(), &override_record).map_err(|err| anyhow!("{err}"))?;

    let eval_store = RocksDbEvalStore::new(db.clone()).context("construct RocksDbEvalStore")?;
    let label = override_record.active_learning_label(prediction.task_id.clone());
    eval_store
        .persist_label(&label)
        .context("persist operator override active-learning label")?;
    db.flush_cf(cf_handle(db.as_ref(), CF_MEJEPA_ACTIVE_LEARNING_LABELS)?)
        .context("flush operator override active-learning label CF")?;
    let override_readback = load_operator_override(db.as_ref(), prediction_id)
        .map_err(|err| anyhow!("{err}"))?
        .ok_or_else(|| anyhow!("MEJEPA_OPERATOR_OVERRIDE_READBACK_MISSING"))?;
    let label_readback = eval_store
        .load_label(&prediction.task_id)
        .context("load operator override active-learning label")?
        .ok_or_else(|| anyhow!("MEJEPA_OPERATOR_OVERRIDE_LABEL_READBACK_MISSING"))?;
    if override_readback != override_record {
        bail!("MEJEPA_OPERATOR_OVERRIDE_READBACK_MISMATCH");
    }
    if label_readback.oracle_outcome != override_record.override_verdict.oracle_outcome()
        || label_readback.method != LabelMethod::Human
    {
        bail!("MEJEPA_OPERATOR_OVERRIDE_LABEL_READBACK_MISMATCH");
    }
    let override_count_after =
        count_operator_overrides(db.as_ref()).map_err(|err| anyhow!("{err}"))?;
    let flags = operator_override_flags_for_predictions(db.as_ref(), &[prediction_id])
        .map_err(|err| anyhow!("{err}"))?;
    let sampling_weight_multiplier =
        context_graph_mejepa_train::learning_signal::sampling_weight(1.0, flags[0], 0.0, 0.995);
    let task_id_string = prediction.task_id.0.clone();
    let covered_chunks = prediction.covered_chunks.clone();
    Ok(json!({
        "status": "recorded",
        "predictionId": hex::encode(prediction_id.0),
        "taskId": task_id_string.clone(),
        "overrideVerdict": override_record.override_verdict,
        "operatorId": override_record.operator_id,
        "identity": identity_status_json(&identity),
        "samplingWeightMultiplier": sampling_weight_multiplier,
        "operatorOverrideCountBefore": override_count_before,
        "operatorOverrideCountAfter": override_count_after,
        "affectedChunks": covered_chunks,
        "report": {
            "operatorOverrideCount": override_count_after,
            "activeLearningLabelMethod": label_readback.method,
            "activeLearningOracleOutcome": label_readback.oracle_outcome
        },
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "overrideCf": CF_MEJEPA_OPERATOR_OVERRIDES,
            "overrideKeyHex": hex::encode(prediction_id.0),
            "labelCf": CF_MEJEPA_ACTIVE_LEARNING_LABELS,
            "labelKey": task_id_string
        },
        "readback": {
            "override": override_readback,
            "activeLearningLabel": label_readback,
            "overrideFlagForNextBatch": flags[0]
        }
    }))
}

fn identity_status_json(identity: &ResolvedAgentIdentity) -> Value {
    json!({
        "id": identity.id.as_str(),
        "authenticated": identity.authenticated,
        "configPath": identity.config_path.as_ref().map(|path| path.display().to_string()),
        "sessionId": identity.session_id.as_deref(),
        "nonce": identity.nonce.as_deref()
    })
}

fn mejepa_write_error_classification<'a>(
    message: &'a str,
    default_kind: ToolErrorKind,
    default_code: &'a str,
) -> (ToolErrorKind, &'a str) {
    if message.contains(MEJEPA_AGENT_IDENTITY_UNVERIFIED) {
        return (ToolErrorKind::Validation, MEJEPA_AGENT_IDENTITY_UNVERIFIED);
    }
    if message.contains(MEJEPA_AGENT_IDENTITY_CONFIG_INVALID) {
        return (ToolErrorKind::Validation, MEJEPA_AGENT_IDENTITY_CONFIG_INVALID);
    }
    (default_kind, default_code)
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

struct PredictionRow {
    prediction: RealityPrediction,
    key: Vec<u8>,
    value_sha256_hex: String,
    value_len: usize,
}

fn find_prediction_by_id(db: &DB, prediction_id: PredictionId) -> AnyhowResult<RealityPrediction> {
    Ok(find_prediction_row_by_id(db, prediction_id)?.prediction)
}

fn find_prediction_row_by_id(db: &DB, prediction_id: PredictionId) -> AnyhowResult<PredictionRow> {
    let cf = cf_handle(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        let prediction = decode_live_prediction_row(&key, &value)?;
        if prediction.prediction_id == prediction_id.0 {
            return Ok(PredictionRow {
                prediction,
                key: key.to_vec(),
                value_sha256_hex: hex::encode(Sha256::digest(&value)),
                value_len: value.len(),
            });
        }
    }
    bail!(
        "predictionId={} was not found in CF_MEJEPA_LIVE_PREDICTIONS",
        hex::encode(prediction_id.0)
    )
}

fn decode_live_prediction_row(key: &[u8], value: &[u8]) -> AnyhowResult<RealityPrediction> {
    if key.len() != 40 {
        bail!(
            "invalid CF_MEJEPA_LIVE_PREDICTIONS key length: expected 40, got {}",
            key.len()
        );
    }
    let prediction: RealityPrediction =
        bincode::deserialize(value).context("decode CF_MEJEPA_LIVE_PREDICTIONS row")?;
    prediction
        .validate()
        .map_err(|err| anyhow!("invalid persisted RealityPrediction: {err}"))?;
    if prediction.session_id != key[0..16] {
        bail!("CF_MEJEPA_LIVE_PREDICTIONS key session prefix does not match payload");
    }
    let mut created_at = [0u8; 8];
    created_at.copy_from_slice(&key[16..24]);
    if prediction.created_at_unix_ms != i64::from_be_bytes(created_at) {
        bail!("CF_MEJEPA_LIVE_PREDICTIONS key timestamp does not match payload");
    }
    if prediction.prediction_id != key[24..40] {
        bail!("CF_MEJEPA_LIVE_PREDICTIONS key prediction suffix does not match payload");
    }
    Ok(prediction)
}

fn inspect_contributing_chunks(
    db: &DB,
    prediction: &RealityPrediction,
    saliency: &BTreeMap<String, f64>,
) -> AnyhowResult<Vec<Value>> {
    let panel_id = PanelId(prediction.source_panel_sha);
    let mut chunks = Vec::with_capacity(prediction.covered_chunks.len());
    for chunk in &prediction.covered_chunks {
        let (key, signals) = read_required_dda_signals(db, &panel_id, chunk)?;
        let saliency_value = saliency
            .get(&chunk.0)
            .copied()
            .ok_or_else(|| anyhow!("saliency missing for covered chunk {}", chunk.0))?;
        chunks.push(json!({
            "chunkId": chunk.0.clone(),
            "saliency": saliency_value,
            "panelId": hex::encode(panel_id.0),
            "ddaSignals": {
                "perEmbedderCosineVector": signals.per_embedder_cosine.clone(),
                "pairwiseCosineVector": signals.pairwise_cosine_upper.clone(),
                "pairwiseMiVector": signals.pairwise_mi_upper.clone(),
                "blindSpotZScoreVector": signals.blind_spot_z_scores.clone()
            },
            "vectorDimensions": {
                "perEmbedderCosine": signals.per_embedder_cosine.len(),
                "pairwiseCosine": signals.pairwise_cosine_upper.len(),
                "pairwiseMi": signals.pairwise_mi_upper.len(),
                "blindSpotZScores": signals.blind_spot_z_scores.len()
            },
            "vectorNorms": {
                "perEmbedderCosine": vector_norm_f32(&signals.per_embedder_cosine),
                "pairwiseCosine": vector_norm_f32(&signals.pairwise_cosine_upper),
                "pairwiseMi": vector_norm_f32(&signals.pairwise_mi_upper),
                "blindSpotZScores": vector_norm_f32(&signals.blind_spot_z_scores)
            },
            "tctCellsConsulted": tct_cells_for_chunk(prediction, chunk),
            "sourceOfTruth": {
                "cf": CF_MEJEPA_DDA_SIGNALS,
                "keyHex": hex::encode(key),
                "readbackVerified": true
            }
        }));
    }
    Ok(chunks)
}

fn read_required_dda_signals(
    db: &DB,
    panel_id: &PanelId,
    chunk: &ChunkId,
) -> AnyhowResult<(Vec<u8>, DdaSignals)> {
    let cf = cf_handle(db, CF_MEJEPA_DDA_SIGNALS)?;
    let key = bincode::serialize(&(panel_id, chunk)).context("encode DDA signal key")?;
    let Some(bytes) = db.get_cf(cf, &key).context("read CF_MEJEPA_DDA_SIGNALS")? else {
        bail!(
            "MEJEPA_INSPECT_PREDICTION_DDA_MISSING: missing CF_MEJEPA_DDA_SIGNALS row for panel_id={} chunk_id={}",
            hex::encode(panel_id.0),
            chunk.0
        );
    };
    let signals: DdaSignals =
        serde_json::from_slice(&bytes).context("decode CF_MEJEPA_DDA_SIGNALS row")?;
    signals
        .validate()
        .map_err(|err| anyhow!("invalid CF_MEJEPA_DDA_SIGNALS row: {err}"))?;
    Ok((key, signals))
}

fn tct_cells_for_chunk(prediction: &RealityPrediction, chunk: &ChunkId) -> Vec<Value> {
    prediction
        .guard_violations
        .iter()
        .filter(|violation| violation.chunk == *chunk)
        .map(|violation| {
            json!({
                "language": prediction.language,
                "chunkId": chunk.0.clone(),
                "embedder": violation.embedder.0.clone(),
                "centroidId": violation.centroid_id.clone(),
                "cosine": violation.cosine,
                "thresholdTauM": violation.threshold_tau_m,
                "deficit": violation.deficit,
                "source": "RealityPrediction.guard_violations"
            })
        })
        .collect()
}

fn vector_norm_f32(values: &[f32]) -> f64 {
    values
        .iter()
        .map(|value| {
            let as_f64 = f64::from(*value);
            as_f64 * as_f64
        })
        .sum::<f64>()
        .sqrt()
}

fn mean_prediction_attestation(values: &BTreeMap<String, f32>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.values().map(|value| f64::from(*value)).sum::<f64>() / values.len() as f64)
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PausePredictionState {
    paused_until_unix_ms: i64,
    set_at_unix_ms: i64,
    reason: String,
    source: String,
}

fn resolve_pause_state_path(input: Option<PathBuf>) -> Result<PathBuf, String> {
    match input {
        Some(path) if path.as_os_str().is_empty() => {
            Err("statePath must be a non-empty path".to_string())
        }
        Some(path) => Ok(path),
        None => match std::env::var(ENV_PAUSE_PATH) {
            Ok(path) if path.trim().is_empty() => {
                Err(format!("{ENV_PAUSE_PATH} must be non-empty"))
            }
            Ok(path) => Ok(PathBuf::from(path)),
            Err(std::env::VarError::NotPresent) => Ok(PathBuf::from(DEFAULT_PAUSE_STATE_PATH)),
            Err(err) => Err(format!("{ENV_PAUSE_PATH} must be readable UTF-8: {err}")),
        },
    }
}

fn write_pause_state(state_path: &Path, duration_mins: u64, reason: &str) -> AnyhowResult<Value> {
    if state_path.as_os_str().is_empty() {
        bail!("MEJEPA_PAUSE_STATE_PATH_EMPTY: statePath must be a non-empty path");
    }
    if duration_mins == 0 {
        bail!("MEJEPA_PAUSE_DURATION_ZERO: durationMins must be > 0");
    }
    if duration_mins > 7 * 24 * 60 {
        bail!(
            "MEJEPA_PAUSE_DURATION_EXCEEDS_WEEK: durationMins must be <= 10080; got {duration_mins}"
        );
    }
    if reason.trim().is_empty() || reason.chars().any(char::is_control) {
        bail!("MEJEPA_PAUSE_REASON_INVALID: reason must be non-empty and contain no control characters");
    }
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("MEJEPA_PAUSE_PARENT_CREATE_FAILED: {}", parent.display()))?;
    }
    let set_at_unix_ms: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("MEJEPA_PAUSE_CLOCK_BEFORE_UNIX_EPOCH")?
        .as_millis()
        .try_into()
        .context("MEJEPA_PAUSE_CLOCK_OVERFLOW")?;
    let duration_ms = duration_mins
        .checked_mul(60_000)
        .and_then(|value| i64::try_from(value).ok())
        .context("MEJEPA_PAUSE_DURATION_OVERFLOW")?;
    let paused_until_unix_ms = set_at_unix_ms
        .checked_add(duration_ms)
        .context("MEJEPA_PAUSE_UNTIL_OVERFLOW")?;
    let state = PausePredictionState {
        paused_until_unix_ms,
        set_at_unix_ms,
        reason: reason.to_string(),
        source: tool_names::MEJEPA_PAUSE_PREDICTIONS.to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&state).context("MEJEPA_PAUSE_SERIALIZE_FAILED")?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(state_path)
        .with_context(|| format!("MEJEPA_PAUSE_WRITE_OPEN_FAILED: {}", state_path.display()))?;
    std::io::Write::write_all(&mut file, &bytes)
        .with_context(|| format!("MEJEPA_PAUSE_WRITE_FAILED: {}", state_path.display()))?;
    file.sync_all()
        .with_context(|| format!("MEJEPA_PAUSE_SYNC_FAILED: {}", state_path.display()))?;
    drop(file);
    let readback_bytes = std::fs::read(state_path)
        .with_context(|| format!("MEJEPA_PAUSE_READBACK_FAILED: {}", state_path.display()))?;
    if readback_bytes != bytes {
        bail!("MEJEPA_PAUSE_READBACK_BYTES_MISMATCH");
    }
    let readback: PausePredictionState = serde_json::from_slice(&readback_bytes)
        .context("MEJEPA_PAUSE_READBACK_DESERIALIZE_FAILED")?;
    if readback != state {
        bail!("MEJEPA_PAUSE_READBACK_STATE_MISMATCH");
    }
    validate_pause_state(&readback)?;
    Ok(json!({
        "statePath": state_path,
        "pausedUntilUnixMs": state.paused_until_unix_ms,
        "setAtUnixMs": state.set_at_unix_ms,
        "durationMins": duration_mins,
        "reason": state.reason,
        "readbackEqual": true,
        "sourceOfTruth": {
            "kind": "file",
            "path": state_path,
            "writer": "mcp__cgreality__mejepa_pause_predictions",
        }
    }))
}

fn now_unix_ms() -> AnyhowResult<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("MEJEPA_PAUSE_CLOCK_BEFORE_UNIX_EPOCH")?
        .as_millis()
        .try_into()
        .context("MEJEPA_PAUSE_CLOCK_OVERFLOW")
}

fn active_pause_state(
    state_path: &Path,
    now_unix_ms: i64,
) -> AnyhowResult<Option<PausePredictionState>> {
    if state_path.as_os_str().is_empty() {
        bail!("MEJEPA_PAUSE_STATE_PATH_EMPTY: statePath must be a non-empty path");
    }
    if !state_path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(state_path)
        .with_context(|| format!("MEJEPA_PAUSE_READBACK_FAILED: {}", state_path.display()))?;
    if bytes.is_empty() {
        bail!(
            "MEJEPA_PAUSE_STATE_EMPTY: {} is empty",
            state_path.display()
        );
    }
    let state: PausePredictionState =
        serde_json::from_slice(&bytes).context("MEJEPA_PAUSE_READBACK_DESERIALIZE_FAILED")?;
    validate_pause_state(&state)?;
    if state.paused_until_unix_ms > now_unix_ms {
        Ok(Some(state))
    } else {
        Ok(None)
    }
}

fn validate_pause_state(state: &PausePredictionState) -> AnyhowResult<()> {
    if state.paused_until_unix_ms <= state.set_at_unix_ms {
        bail!("MEJEPA_PAUSE_STATE_INVALID: pausedUntilUnixMs must be greater than setAtUnixMs");
    }
    if state.reason.trim().is_empty() || state.reason.chars().any(char::is_control) {
        bail!("MEJEPA_PAUSE_STATE_INVALID: reason must be non-empty and contain no control characters");
    }
    if state.source.trim().is_empty() || state.source.chars().any(char::is_control) {
        bail!("MEJEPA_PAUSE_STATE_INVALID: source must be non-empty and contain no control characters");
    }
    Ok(())
}

fn pause_state_value(state_path: &Path, state: &PausePredictionState, now_unix_ms: i64) -> Value {
    json!({
        "statePath": state_path,
        "pausedUntilUnixMs": state.paused_until_unix_ms,
        "setAtUnixMs": state.set_at_unix_ms,
        "remainingMs": state.paused_until_unix_ms.saturating_sub(now_unix_ms),
        "reason": state.reason.clone(),
        "source": state.source.clone(),
        "sourceOfTruth": {
            "kind": "file",
            "path": state_path,
            "reader": "mcp__cgreality__mejepa_verify",
        }
    })
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
    use std::fs::{self, OpenOptions};
    use std::io::Write;
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
        std::env::remove_var(ENV_INFER_DB);
        std::env::remove_var(ENV_PAUSE_PATH);
        std::env::remove_var(ENV_ALLOW_DETERMINISTIC_VERIFY);
        std::env::remove_var(context_graph_mejepa_tct::ENV_ALLOW_STALE);
        std::env::remove_var(context_graph_mejepa_tct::ENV_MAX_AGE_DAYS);
        guard
    }

    #[test]
    fn pause_predictions_writes_state_file_and_reads_back() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("pause-state.json");
        let value = write_pause_state(&path, 5, "unit-test").unwrap();
        assert_eq!(value["readbackEqual"], true);
        let readback: PausePredictionState =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert!(readback.paused_until_unix_ms > readback.set_at_unix_ms);
        assert_eq!(readback.reason, "unit-test");
        assert_eq!(readback.source, tool_names::MEJEPA_PAUSE_PREDICTIONS);
    }

    #[test]
    fn pause_predictions_rejects_invalid_duration() {
        let tmp = TempDir::new().unwrap();
        let err =
            write_pause_state(&tmp.path().join("pause-state.json"), 0, "unit-test").unwrap_err();
        assert!(err.to_string().contains("MEJEPA_PAUSE_DURATION_ZERO"));
    }

    #[test]
    fn active_pause_state_only_returns_future_pauses() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("pause-state.json");
        let future = PausePredictionState {
            paused_until_unix_ms: 2_000,
            set_at_unix_ms: 1_000,
            reason: "unit-test".to_string(),
            source: tool_names::MEJEPA_PAUSE_PREDICTIONS.to_string(),
        };
        std::fs::write(&path, serde_json::to_vec_pretty(&future).unwrap()).unwrap();
        assert!(active_pause_state(&path, 1_500).unwrap().is_some());
        assert!(active_pause_state(&path, 2_000).unwrap().is_none());
    }

    #[tokio::test]
    async fn mejepa_pause_predictions_handler_writes_state_file() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let tmp = TempDir::new().unwrap();
        let state_path = tmp.path().join("pause-state.json");
        let response = handlers
            .call_mejepa_pause_predictions(
                Some(JsonRpcId::Number(47)),
                json!({
                    "statePath": state_path.display().to_string(),
                    "durationMins": 5,
                    "reason": "incident response"
                }),
            )
            .await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["readbackEqual"], true);
        assert_eq!(structured["reason"], "incident response");
        let readback: PausePredictionState =
            serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
        assert!(readback.paused_until_unix_ms > readback.set_at_unix_ms);

        let zero_response = handlers
            .call_mejepa_pause_predictions(
                Some(JsonRpcId::Number(48)),
                json!({
                    "statePath": state_path.display().to_string(),
                    "durationMins": 0,
                    "reason": "x"
                }),
            )
            .await;
        let zero_result = zero_response.result.unwrap();
        assert_eq!(zero_result["isError"], true);
        assert!(zero_result["structuredContent"]["message"]
            .as_str()
            .unwrap()
            .contains("MEJEPA_PAUSE_DURATION_ZERO"));
    }

    #[test]
    fn mejepa_verify_honors_active_pause_state() {
        let _guard = env_guard();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (handlers, _handler_tempdir) =
            runtime.block_on(crate::handlers::tests::create_protocol_test_handlers());
        let tmp = TempDir::new().unwrap();
        let infer_db = tmp.path().join("infer-db");
        let pause_path = tmp.path().join("pause-state.json");
        std::env::set_var(ENV_INFER_DB, &infer_db);
        std::env::set_var(ENV_PAUSE_PATH, &pause_path);
        write_pause_state(&pause_path, 5, "operator requested pause").unwrap();
        let (patch, context) =
            context_graph_mejepa::fixture_patch_context(tmp.path(), "happy").unwrap();
        let response = runtime.block_on(handlers.call_mejepa_verify(
            Some(JsonRpcId::Number(49)),
            json!({
                "patch": patch,
                "context": context,
                "includeProvenance": true
            }),
        ));
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["verdict"], "escalate_to_human");
        assert_eq!(structured["failed_gate"]["kind"], "prediction_paused");
        assert_eq!(structured["gates_passed"], 0);
        assert_eq!(
            structured["pause_state"]["reason"],
            "operator requested pause"
        );
        assert!(structured["pause_state"]["remainingMs"].as_i64().unwrap() > 0);
        assert_eq!(
            structured["provenance"]["pauseStatePath"],
            json!(pause_path.display().to_string())
        );
    }

    #[tokio::test]
    async fn mejepa_operator_override_prediction_persists_override_and_label() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let tmp = TempDir::new().unwrap();
        let infer_db = tmp.path().join("infer-db");
        let prediction = inspect_prediction_fixture([0x60; 16], "src/lib.rs#fn#override");
        let prediction_id = PredictionId(prediction.prediction_id);
        let db = open_infer_rocksdb(&infer_db).unwrap();
        persist_prediction_and_dda(db.clone(), &prediction);
        drop(db);

        let response = handlers
            .call_mejepa_operator_override_prediction(
                Some(JsonRpcId::Number(60)),
                json!({
                    "dbPath": infer_db.display().to_string(),
                    "predictionId": hex::encode(prediction.prediction_id),
                    "overrideVerdict": "fail",
                    "reason": "operator readback test",
                    "operatorId": "operator-override-unit-test"
                }),
            )
            .await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["status"], "recorded");
        assert_eq!(structured["operatorOverrideCountBefore"], 0);
        assert_eq!(structured["operatorOverrideCountAfter"], 1);
        assert_eq!(structured["samplingWeightMultiplier"], 6.0);
        assert_eq!(
            structured["sourceOfTruth"]["overrideCf"],
            json!(CF_MEJEPA_OPERATOR_OVERRIDES)
        );
        assert_eq!(
            structured["sourceOfTruth"]["labelCf"],
            json!(CF_MEJEPA_ACTIVE_LEARNING_LABELS)
        );
        assert_eq!(structured["readback"]["overrideFlagForNextBatch"], true);

        let reopened = open_infer_rocksdb(&infer_db).unwrap();
        let override_readback = load_operator_override(reopened.as_ref(), prediction_id)
            .unwrap()
            .unwrap();
        assert_eq!(override_readback.override_verdict, OverrideVerdict::Fail);
        assert_eq!(override_readback.reason, "operator readback test");
        let eval_store = RocksDbEvalStore::new(reopened.clone()).unwrap();
        let label_readback = eval_store.load_label(&prediction.task_id).unwrap().unwrap();
        assert_eq!(
            label_readback.oracle_outcome,
            context_graph_mejepa::OracleOutcome::Fail
        );
        assert_eq!(label_readback.method, LabelMethod::Human);
        drop(eval_store);
        drop(reopened);

        let missing_response = handlers
            .call_mejepa_operator_override_prediction(
                Some(JsonRpcId::Number(61)),
                json!({
                    "dbPath": infer_db.display().to_string(),
                    "predictionId": hex::encode([0x61; 16]),
                    "overrideVerdict": "fail",
                    "reason": "operator readback test",
                    "operatorId": "operator-override-unit-test"
                }),
            )
            .await;
        let missing_result = missing_response.result.unwrap();
        assert_eq!(missing_result["isError"], true);
        let missing_text = missing_result["content"][0]["text"].as_str().unwrap();
        assert!(
            missing_text.contains("CF_MEJEPA_LIVE_PREDICTIONS"),
            "{missing_text}"
        );
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

    fn inspect_prediction_fixture(prediction_id: [u8; 16], chunk: &str) -> RealityPrediction {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id,
            witness_hash: WitnessHash([0x77; 32]),
            task_id: context_graph_mejepa::TaskId("inspect-test-task".to_string()),
            session_id: [0x19; 16],
            language: context_graph_mejepa::Language::Python,
            covered_chunks: vec![ChunkId(chunk.to_string())],
            verdict: context_graph_mejepa::Verdict::GuardRejected,
            confidence_interval: context_graph_mejepa::ConformalInterval {
                lower: 0.41,
                upper: 0.74,
                method: context_graph_mejepa::ConformalMethod::SplitConformal,
                coverage_target: 0.90,
                empirical_coverage: 0.89,
            },
            predicted_oracle_pass: 0.62,
            predicted_test_pass: vec![0.61, 0.83],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.22,
            outcome_set: context_graph_mejepa::ConformalSet::try_new(
                vec![context_graph_mejepa::OracleOutcome::Pass],
                0.1,
                0.14,
            )
            .unwrap(),
            calibrated_confidence: 0.58,
            degraded_status: false,
            granger_attestations: BTreeMap::from([("dda:mean_cosine".to_string(), 0.81)]),
            predicted_failure_modes: Vec::new(),
            predicted_failed_tests: Vec::new(),
            guard_violations: vec![context_graph_mejepa::GuardViolation {
                embedder: context_graph_mejepa::EmbedderId("E7".to_string()),
                chunk: ChunkId(chunk.to_string()),
                centroid_id: "known_good:function:python:e7".to_string(),
                cosine: 0.71,
                threshold_tau_m: 0.85,
                deficit: 0.14,
            }],
            closest_exemplars: Vec::new(),
            predicted_edge_cases: Vec::new(),
            predicted_latent_bugs: Vec::new(),
            predicted_tech_debt_added: Vec::new(),
            predicted_dead_code: Vec::new(),
            predicted_redundant_code: Vec::new(),
            predicted_perf_regressions: Vec::new(),
            predicted_security_concerns: Vec::new(),
            predicted_accuracy_degradations: Vec::new(),
            predicted_cost_regressions: Vec::new(),
            predicted_reasoning_class: context_graph_mejepa::ReasoningClass::MostlyCorrect,
            agent_claim_graph: context_graph_mejepa::AgentClaimGraph::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: context_graph_mejepa::PredictionProvenance {
                predictor_version: "inspect-test-predictor-v1".to_string(),
                constellation_version: "inspect-test-constellation-v1".to_string(),
                calibration_version: "inspect-test-calibration-v1".to_string(),
                active_pointer: hex::encode(prediction_id),
            },
            source_panel_sha: [0x42; 32],
            calibration_version: "inspect-test-calibration-v1".to_string(),
            created_at_unix_ms: 1_778_644_000_000,
        })
        .unwrap()
    }

    fn persist_prediction_and_dda(db: Arc<DB>, prediction: &RealityPrediction) {
        let store = RocksDbInferStore::new(db.clone());
        context_graph_mejepa::MejepaStore::write_live_prediction(&store, prediction).unwrap();
        persist_dda_for_prediction(db.as_ref(), prediction);
    }

    fn persist_dda_for_prediction(db: &DB, prediction: &RealityPrediction) {
        let cf = cf_handle(db, CF_MEJEPA_DDA_SIGNALS).unwrap();
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        for chunk in &prediction.covered_chunks {
            let key = bincode::serialize(&(PanelId(prediction.source_panel_sha), chunk)).unwrap();
            let signals = DdaSignals::try_new(DdaSignals {
                per_embedder_cosine: vec![0.91, 0.82, 0.73],
                pairwise_cosine_upper: vec![0.71, 0.62, 0.53],
                pairwise_mi_upper: vec![0.11, 0.07, 0.03],
                blind_spot_z_scores: vec![1.2, -0.4, 0.2],
            })
            .unwrap();
            let bytes = serde_json::to_vec(&signals).unwrap();
            db.put_cf_opt(cf, &key, bytes, &opts).unwrap();
            let readback = db.get_cf(cf, &key).unwrap().unwrap();
            let decoded: DdaSignals = serde_json::from_slice(&readback).unwrap();
            assert_eq!(decoded, signals);
        }
        db.flush_cf(cf).unwrap();
    }

    fn write_fsv_json(path: &Path, value: &Value) {
        let bytes = serde_json::to_vec_pretty(value).unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }

    fn physical_file_summary(root: &Path) -> Value {
        let mut file_count = 0u64;
        let mut total_bytes = 0u64;
        let mut sst_files = Vec::new();
        collect_physical_files(root, &mut file_count, &mut total_bytes, &mut sst_files);
        json!({
            "root": root.display().to_string(),
            "file_count": file_count,
            "total_file_bytes": total_bytes,
            "sst_files": sst_files
        })
    }

    fn collect_physical_files(
        root: &Path,
        file_count: &mut u64,
        total_bytes: &mut u64,
        sst_files: &mut Vec<Value>,
    ) {
        for entry in fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = entry.metadata().unwrap();
            if metadata.is_dir() {
                collect_physical_files(&path, file_count, total_bytes, sst_files);
                continue;
            }
            *file_count += 1;
            *total_bytes += metadata.len();
            if path.extension().and_then(|ext| ext.to_str()) == Some("sst") {
                sst_files.push(json!({
                    "path": path.display().to_string(),
                    "bytes": metadata.len()
                }));
            }
        }
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

    #[tokio::test]
    async fn mejepa_inspect_prediction_writes_phase_f_fsv_artifact() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
        let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-inspect-prediction-fsv");
        fs::create_dir_all(&fsv_root).unwrap();
        let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
        fs::create_dir_all(&run_root).unwrap();
        let infer_db = run_root.join("infer-db");

        let prediction = inspect_prediction_fixture([0x44; 16], "src/lib.rs#fn#0");
        let missing_dda_prediction =
            inspect_prediction_fixture([0x45; 16], "src/lib.rs#fn#missing-dda");
        let db = open_infer_rocksdb(&infer_db).unwrap();
        let cf_counts_before = json!({
            CF_MEJEPA_LIVE_PREDICTIONS: count_cf_any(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS).unwrap(),
            CF_MEJEPA_DDA_SIGNALS: count_cf_any(db.as_ref(), CF_MEJEPA_DDA_SIGNALS).unwrap()
        });
        persist_prediction_and_dda(db.clone(), &prediction);
        let store = RocksDbInferStore::new(db.clone());
        context_graph_mejepa::MejepaStore::write_live_prediction(&store, &missing_dda_prediction)
            .unwrap();
        let cf_counts_after = json!({
            CF_MEJEPA_LIVE_PREDICTIONS: count_cf_any(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS).unwrap(),
            CF_MEJEPA_DDA_SIGNALS: count_cf_any(db.as_ref(), CF_MEJEPA_DDA_SIGNALS).unwrap()
        });
        drop(store);
        drop(db);

        let reopened = open_infer_rocksdb(&infer_db).unwrap();
        let reopened_prediction =
            find_prediction_row_by_id(reopened.as_ref(), PredictionId(prediction.prediction_id))
                .unwrap();
        let (_dda_key, reopened_dda) = read_required_dda_signals(
            reopened.as_ref(),
            &PanelId(prediction.source_panel_sha),
            &prediction.covered_chunks[0],
        )
        .unwrap();
        let independent_reopen_equal = reopened_prediction.prediction == prediction
            && reopened_dda.per_embedder_cosine == vec![0.91, 0.82, 0.73];
        drop(reopened);

        let prediction_id = hex::encode(prediction.prediction_id);
        let happy_response = handlers
            .call_mejepa_inspect_prediction(
                Some(JsonRpcId::Number(440)),
                json!({"predictionId": prediction_id, "dbPath": infer_db.display().to_string()}),
            )
            .await;
        assert!(happy_response.error.is_none());
        let happy_result = happy_response.result.unwrap();
        let happy_structured = happy_result["structuredContent"].clone();
        let happy_pass = happy_result["isError"] == false
            && happy_structured["sourceOfTruth"]["readbackVerified"] == true
            && happy_structured["sourceOfTruth"]["livePredictionCf"]
                == json!(CF_MEJEPA_LIVE_PREDICTIONS)
            && happy_structured["sourceOfTruth"]["ddaSignalsCf"] == json!(CF_MEJEPA_DDA_SIGNALS)
            && happy_structured["versions"]["predictorVersion"]
                == json!("inspect-test-predictor-v1")
            && happy_structured["witnessHash"] == json!(hex::encode([0x77; 32]))
            && happy_structured["contributingChunks"]
                .as_array()
                .map(|chunks| chunks.len())
                == Some(1)
            && happy_structured["contributingChunks"][0]["ddaSignals"]["perEmbedderCosineVector"]
                .as_array()
                .map(|values| values.len())
                == Some(3)
            && happy_structured["contributingChunks"][0]["tctCellsConsulted"]
                .as_array()
                .map(|values| values.len())
                == Some(1);

        let malformed_response = handlers
            .call_mejepa_inspect_prediction(
                Some(JsonRpcId::Number(441)),
                json!({"predictionId": "not-hex", "dbPath": infer_db.display().to_string()}),
            )
            .await;
        let malformed_result = malformed_response.result.unwrap();

        let missing_prediction_response = handlers
            .call_mejepa_inspect_prediction(
                Some(JsonRpcId::Number(442)),
                json!({
                    "predictionId": hex::encode([0xaa; 16]),
                    "dbPath": infer_db.display().to_string()
                }),
            )
            .await;
        let missing_prediction_result = missing_prediction_response.result.unwrap();

        let missing_dda_response = handlers
            .call_mejepa_inspect_prediction(
                Some(JsonRpcId::Number(443)),
                json!({
                    "predictionId": hex::encode(missing_dda_prediction.prediction_id),
                    "dbPath": infer_db.display().to_string()
                }),
            )
            .await;
        let missing_dda_result = missing_dda_response.result.unwrap();

        let unknown_field_response = handlers
            .call_mejepa_inspect_prediction(
                Some(JsonRpcId::Number(444)),
                json!({
                    "predictionId": hex::encode(prediction.prediction_id),
                    "dbPath": infer_db.display().to_string(),
                    "extra": true
                }),
            )
            .await;
        let unknown_field_result = unknown_field_response.result.unwrap();

        let boundary_cases = vec![
            json!({
                "case": "malformed_prediction_id_fails_closed",
                "expected": "predictionId must be exactly 32 hexadecimal characters",
                "actual": malformed_result,
                "pass": malformed_result["isError"] == true
                    && malformed_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("predictionId must be exactly 32 hexadecimal characters")
            }),
            json!({
                "case": "missing_prediction_row_fails_closed",
                "expected": "not found in CF_MEJEPA_LIVE_PREDICTIONS",
                "actual": missing_prediction_result,
                "pass": missing_prediction_result["isError"] == true
                    && missing_prediction_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("CF_MEJEPA_LIVE_PREDICTIONS")
            }),
            json!({
                "case": "missing_dda_signal_row_fails_closed",
                "expected": "MEJEPA_INSPECT_PREDICTION_DDA_MISSING",
                "actual": missing_dda_result,
                "pass": missing_dda_result["isError"] == true
                    && missing_dda_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("MEJEPA_INSPECT_PREDICTION_DDA_MISSING")
            }),
            json!({
                "case": "unknown_request_field_fails_schema_validation",
                "expected": "schema validation failed",
                "actual": unknown_field_result,
                "pass": unknown_field_result["isError"] == true
                    && unknown_field_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("schema validation failed")
            }),
        ];
        let all_boundaries_pass = boundary_cases
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true));
        let report = json!({
            "fsv_root": fsv_root,
            "task_id": "TASK-OBS-009",
            "started_at_unix_ms": started_at_unix_ms,
            "build_release_sha": std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|text| text.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            "happy_path": [{
                "case": "inspect_prediction_reads_live_prediction_and_dda_vectors",
                "sot": format!("{CF_MEJEPA_LIVE_PREDICTIONS} + {CF_MEJEPA_DDA_SIGNALS}"),
                "before": {
                    "db_path": infer_db.display().to_string(),
                    "prediction_id": hex::encode(prediction.prediction_id),
                    "covered_chunks": prediction.covered_chunks.clone()
                },
                "trigger": "cargo test -p context-graph-mcp mejepa_inspect_prediction_writes_phase_f_fsv_artifact -- --nocapture",
                "after": happy_structured,
                "expected": {
                    "readbackVerified": true,
                    "chunk_count": 1,
                    "per_embedder_vector_len": 3,
                    "tct_cell_count": 1
                },
                "actual": happy_pass,
                "pass": happy_pass,
                "evidence_path": run_root.display().to_string()
            }],
            "boundary_cases": boundary_cases,
            "all_passed": happy_pass && all_boundaries_pass,
            "cf_counts_before": cf_counts_before,
            "cf_counts_after": cf_counts_after,
            "readback_equal": independent_reopen_equal,
            "physical_artifacts": {
                "infer_db_exists": infer_db.exists(),
                "run_root": run_root.display().to_string(),
                "infer_db": physical_file_summary(&infer_db)
            }
        });
        assert_eq!(
            report["all_passed"],
            true,
            "{}",
            serde_json::to_string_pretty(&report).unwrap()
        );
        let evidence_path = fsv_root.join("inspect_prediction_fsv.json");
        write_fsv_json(&evidence_path, &report);
        let readback: Value = serde_json::from_slice(&fs::read(&evidence_path).unwrap()).unwrap();
        assert_eq!(readback["all_passed"], true);
        assert_eq!(readback["readback_equal"], true);
    }

    #[tokio::test]
    async fn mejepa_replay_prediction_api_reads_byte_equal_source_of_truth() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let temp = TempDir::new().unwrap();
        let infer_db = temp.path().join("infer-db");
        let prediction = inspect_prediction_fixture([0x46; 16], "src/lib.rs#fn#replay");
        let db = open_infer_rocksdb(&infer_db).unwrap();
        persist_prediction_and_dda(db.clone(), &prediction);
        drop(db);

        let response = handlers
            .call_mejepa_replay_prediction(
                Some(JsonRpcId::Number(460)),
                json!({
                    "predictionId": hex::encode(prediction.prediction_id),
                    "dbPath": infer_db.display().to_string()
                }),
            )
            .await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["byteEqual"], true);
        assert_eq!(structured["semanticEqual"], true);
        assert_eq!(
            structured["columnFamily"],
            json!(CF_MEJEPA_LIVE_PREDICTIONS)
        );
        assert_eq!(
            structured["predictionId"],
            json!(hex::encode(prediction.prediction_id))
        );

        let missing_response = handlers
            .call_mejepa_replay_prediction(
                Some(JsonRpcId::Number(461)),
                json!({
                    "predictionId": hex::encode([0xee; 16]),
                    "dbPath": infer_db.display().to_string()
                }),
            )
            .await;
        let missing_result = missing_response.result.unwrap();
        assert_eq!(missing_result["isError"], true);
        assert!(missing_result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("MEJEPA_PREDICTION_REPLAY_NOT_FOUND"));
    }

    #[tokio::test]
    async fn mejepa_operator_override_prediction_writes_phase_f_fsv_artifact() {
        let (handlers, _handler_tempdir) =
            crate::handlers::tests::create_protocol_test_handlers().await;
        let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
        let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-operator-override-fsv");
        fs::create_dir_all(&fsv_root).unwrap();
        let run_root = fsv_root.join(format!(
            "mcp-run-{}-{}",
            started_at_unix_ms,
            std::process::id()
        ));
        fs::create_dir_all(&run_root).unwrap();
        let infer_db = run_root.join("infer-db");

        let prediction = inspect_prediction_fixture([0x60; 16], "src/lib.rs#fn#override");
        let db = open_infer_rocksdb(&infer_db).unwrap();
        let cf_counts_before = json!({
            CF_MEJEPA_LIVE_PREDICTIONS: count_cf_any(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS).unwrap(),
            CF_MEJEPA_OPERATOR_OVERRIDES: count_cf_any(db.as_ref(), CF_MEJEPA_OPERATOR_OVERRIDES).unwrap(),
            CF_MEJEPA_ACTIVE_LEARNING_LABELS: count_cf_any(db.as_ref(), CF_MEJEPA_ACTIVE_LEARNING_LABELS).unwrap()
        });
        persist_prediction_and_dda(db.clone(), &prediction);
        drop(db);

        let response = handlers
            .call_mejepa_operator_override_prediction(
                Some(JsonRpcId::Number(600)),
                json!({
                    "predictionId": hex::encode(prediction.prediction_id),
                    "dbPath": infer_db.display().to_string(),
                    "overrideVerdict": "fail",
                    "reason": "operator observed oracle failure",
                    "operatorId": "operator-1"
                }),
            )
            .await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        let structured = result["structuredContent"].clone();
        let happy_pass = result["isError"] == false
            && structured["samplingWeightMultiplier"].as_f64() == Some(6.0)
            && structured["operatorOverrideCountAfter"].as_u64() == Some(1)
            && structured["report"]["operatorOverrideCount"].as_u64() == Some(1)
            && structured["report"]["activeLearningLabelMethod"] == json!("human")
            && structured["report"]["activeLearningOracleOutcome"] == json!("fail")
            && structured["sourceOfTruth"]["overrideCf"] == json!(CF_MEJEPA_OPERATOR_OVERRIDES)
            && structured["sourceOfTruth"]["labelCf"] == json!(CF_MEJEPA_ACTIVE_LEARNING_LABELS)
            && structured["readback"]["overrideFlagForNextBatch"] == json!(true)
            && structured["readback"]["override"]["reason"]
                == json!("operator observed oracle failure");
        assert!(happy_pass, "{structured}");

        let reopened = open_infer_rocksdb(&infer_db).unwrap();
        let override_readback =
            load_operator_override(reopened.as_ref(), PredictionId(prediction.prediction_id))
                .unwrap()
                .unwrap();
        let eval_store = RocksDbEvalStore::new(reopened.clone()).unwrap();
        let label_readback = eval_store.load_label(&prediction.task_id).unwrap().unwrap();
        let cf_counts_after = json!({
            CF_MEJEPA_LIVE_PREDICTIONS: count_cf_any(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS).unwrap(),
            CF_MEJEPA_OPERATOR_OVERRIDES: count_cf_any(reopened.as_ref(), CF_MEJEPA_OPERATOR_OVERRIDES).unwrap(),
            CF_MEJEPA_ACTIVE_LEARNING_LABELS: count_cf_any(reopened.as_ref(), CF_MEJEPA_ACTIVE_LEARNING_LABELS).unwrap()
        });
        let independent_reopen_equal = override_readback.reason
            == "operator observed oracle failure"
            && override_readback.override_verdict == OverrideVerdict::Fail
            && label_readback.method == LabelMethod::Human
            && label_readback.oracle_outcome == context_graph_mejepa::OracleOutcome::Fail;
        drop(eval_store);
        drop(reopened);

        let malformed_response = handlers
            .call_mejepa_operator_override_prediction(
                Some(JsonRpcId::Number(601)),
                json!({
                    "predictionId": "not-hex",
                    "dbPath": infer_db.display().to_string(),
                    "overrideVerdict": "fail",
                    "reason": "x",
                    "operatorId": "operator-1"
                }),
            )
            .await;
        let malformed_result = malformed_response.result.unwrap();

        let missing_response = handlers
            .call_mejepa_operator_override_prediction(
                Some(JsonRpcId::Number(602)),
                json!({
                    "predictionId": hex::encode([0x61; 16]),
                    "dbPath": infer_db.display().to_string(),
                    "overrideVerdict": "fail",
                    "reason": "x",
                    "operatorId": "operator-1"
                }),
            )
            .await;
        let missing_result = missing_response.result.unwrap();

        let empty_reason_response = handlers
            .call_mejepa_operator_override_prediction(
                Some(JsonRpcId::Number(603)),
                json!({
                    "predictionId": hex::encode(prediction.prediction_id),
                    "dbPath": infer_db.display().to_string(),
                    "overrideVerdict": "fail",
                    "reason": "  ",
                    "operatorId": "operator-1"
                }),
            )
            .await;
        let empty_reason_result = empty_reason_response.result.unwrap();

        let unknown_field_response = handlers
            .call_mejepa_operator_override_prediction(
                Some(JsonRpcId::Number(604)),
                json!({
                    "predictionId": hex::encode(prediction.prediction_id),
                    "dbPath": infer_db.display().to_string(),
                    "overrideVerdict": "fail",
                    "reason": "x",
                    "operatorId": "operator-1",
                    "extra": true
                }),
            )
            .await;
        let unknown_field_result = unknown_field_response.result.unwrap();

        let boundary_cases = vec![
            json!({
                "case": "malformed_prediction_id_fails_closed",
                "expected": "predictionId must be exactly 32 hexadecimal characters",
                "actual": malformed_result,
                "pass": malformed_result["isError"] == true
                    && malformed_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("predictionId must be exactly 32 hexadecimal characters")
            }),
            json!({
                "case": "missing_prediction_row_fails_closed",
                "expected": "not found in CF_MEJEPA_LIVE_PREDICTIONS",
                "actual": missing_result,
                "pass": missing_result["isError"] == true
                    && missing_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("was not found in CF_MEJEPA_LIVE_PREDICTIONS")
            }),
            json!({
                "case": "empty_reason_fails_closed",
                "expected": "MEJEPA_OPERATOR_OVERRIDE_REASON_EMPTY",
                "actual": empty_reason_result,
                "pass": empty_reason_result["isError"] == true
                    && empty_reason_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("MEJEPA_OPERATOR_OVERRIDE_REASON_EMPTY")
            }),
            json!({
                "case": "unknown_request_field_fails_schema_validation",
                "expected": "schema validation failed",
                "actual": unknown_field_result,
                "pass": unknown_field_result["isError"] == true
                    && unknown_field_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("schema validation failed")
            }),
        ];
        let all_boundaries_pass = boundary_cases
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true));
        let report = json!({
            "fsv_root": fsv_root,
            "task_id": "TASK-PREDICT-012",
            "issue": 60,
            "started_at_unix_ms": started_at_unix_ms,
            "build_release_sha": std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|text| text.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            "happy_path": [{
                "case": "mcp_operator_override_persists_override_and_human_label",
                "sot": format!("{CF_MEJEPA_OPERATOR_OVERRIDES} + {CF_MEJEPA_ACTIVE_LEARNING_LABELS}"),
                "before": {
                    "db_path": infer_db.display().to_string(),
                    "prediction_id": hex::encode(prediction.prediction_id)
                },
                "trigger": "cargo test -p context-graph-mcp mejepa_operator_override_prediction_writes_phase_f_fsv_artifact -- --nocapture",
                "after": structured,
                "expected": {
                    "sampling_weight_multiplier": 6.0,
                    "override_count_after": 1,
                    "label_method": "human",
                    "label_oracle_outcome": "fail"
                },
                "actual": happy_pass,
                "pass": happy_pass,
                "evidence_path": run_root.display().to_string()
            }],
            "boundary_cases": boundary_cases,
            "all_passed": happy_pass && all_boundaries_pass && independent_reopen_equal,
            "cf_counts_before": cf_counts_before,
            "cf_counts_after": cf_counts_after,
            "readback_equal": independent_reopen_equal,
            "physical_artifacts": {
                "infer_db_exists": infer_db.exists(),
                "run_root": run_root.display().to_string(),
                "infer_db": physical_file_summary(&infer_db)
            }
        });
        let evidence_path = fsv_root.join("operator_override_mcp_fsv.json");
        write_fsv_json(&evidence_path, &report);
        let readback: Value = serde_json::from_slice(&fs::read(&evidence_path).unwrap()).unwrap();
        assert_eq!(
            readback["all_passed"],
            true,
            "{}",
            serde_json::to_string_pretty(&readback).unwrap()
        );
        assert_eq!(readback["readback_equal"], true);
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

        let empty_path_response = handlers
            .call_mejepa_daemon_status(
                Some(JsonRpcId::Number(44)),
                json!({"inferDbPath": "", "includeVram": false}),
            )
            .await;
        assert!(empty_path_response.error.is_none());
        let empty_path_result = empty_path_response.result.unwrap();

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
                "case": "empty_infer_path_fails_validation",
                "expected": "inferDbPath must be a non-empty path",
                "actual": empty_path_result,
                "pass": empty_path_result["isError"] == true
                    && empty_path_result["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("inferDbPath must be a non-empty path")
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
