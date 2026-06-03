//! TASK-PY-G-112B MCP surfaces for the online mistake loop.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result as AnyhowResult};
use rocksdb::IteratorMode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use context_graph_mejepa::{PredictionId, Verdict};
use context_graph_mejepa_cf::{
    CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_ONLINE_HEAD_STATE, CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
};
use context_graph_mejepa_train::{
    apply_online_mistake_update_sync_readback, count_cf_rows, online_head_key,
    open_ability_resolver_rocksdb, predict_with_online_head_neighbors, read_all_mistake_log_rows,
    read_all_skill_lifecycle_audit_rows, read_mistake_log_row, read_online_head_state_row,
    read_replay_row, write_mistake_log_row_sync_readback, write_replay_row_sync_readback,
    MistakeLogRow, OnlineHeadNeighbor, OnlineHeadNeighborConfig, OnlineHeadNeighborContext,
    OnlineHeadRepeatMetricRow, OnlineHeadStateRow, OnlineHeadUpdateConfig, OnlineHeadUpdateInput,
    ReplayBufferRow,
};

use crate::handlers::tools::helpers::{mejepa_db_source_of_truth, ToolErrorKind};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const ENV_TRAIN_DB: &str = "CONTEXTGRAPH_MEJEPA_TRAIN_DB";
const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordMistakeRequest {
    db_path: Option<PathBuf>,
    panel_signature_hash: String,
    mistake_row: MistakeLogRow,
    replay_row: ReplayBufferRow,
    base_verdict_before_update: Verdict,
    now_unix_ms: i64,
    online_head_config: Option<OnlineHeadUpdateConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MistakeHistoryRequest {
    db_path: Option<PathBuf>,
    mistake_id: Option<String>,
    prediction_id_hex: Option<String>,
    code_state_key: Option<String>,
    label_signature_hash: Option<String>,
    skill_signature_hash: Option<String>,
    ability_signature_hash: Option<String>,
    membership_signature_hash: Option<String>,
    #[serde(default = "default_history_limit")]
    limit: usize,
    #[serde(default = "default_true")]
    include_replay_rows: bool,
    #[serde(default = "default_true")]
    include_lifecycle_audits: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MistakeLoopStatusRequest {
    db_path: Option<PathBuf>,
    panel_signature_hash: String,
    #[serde(default = "default_repeat_metric_limit")]
    repeat_metric_limit: usize,
    base_verdict: Option<Verdict>,
    neighbor_context: Option<OnlineHeadNeighborContext>,
    #[serde(default)]
    neighbors: Vec<OnlineHeadNeighbor>,
    neighbor_config: Option<OnlineHeadNeighborConfig>,
}

impl Handlers {
    pub(crate) async fn call_mejepa_record_mistake(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_RECORD_MISTAKE) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_record_mistake(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => mistake_loop_error(self, id, "MEJEPA_RECORD_MISTAKE_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_mistake_history(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_MISTAKE_HISTORY) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_mistake_history(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => mistake_loop_error(self, id, "MEJEPA_MISTAKE_HISTORY_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_mistake_loop_status(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_MISTAKE_LOOP_STATUS) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_mistake_loop_status(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => mistake_loop_error(self, id, "MEJEPA_MISTAKE_LOOP_STATUS_FAILED", err),
        }
    }
}

fn run_record_mistake(request: RecordMistakeRequest) -> AnyhowResult<Value> {
    let config = request.online_head_config.unwrap_or_default();
    validate_record_request_prewrite(&request, config)?;
    let db_path = resolve_mistake_loop_db_path(request.db_path)?;
    let db = open_ability_resolver_rocksdb(&db_path, false)
        .with_context(|| format!("open mistake-loop DB {}", db_path.display()))?;
    write_mistake_log_row_sync_readback(&db, &request.mistake_row)?;
    write_replay_row_sync_readback(&db, &request.replay_row)?;
    let report = apply_online_mistake_update_sync_readback(
        &db,
        OnlineHeadUpdateInput {
            panel_signature_hash: request.panel_signature_hash.clone(),
            mistake_row: request.mistake_row.clone(),
            replay_row: request.replay_row.clone(),
            base_verdict_before_update: request.base_verdict_before_update,
            now_unix_ms: request.now_unix_ms,
        },
        config,
    )?;
    let mistake_readback = read_mistake_log_row(&db, &request.mistake_row.mistake_id)?
        .ok_or_else(|| anyhow!("mistake readback missing after write"))?;
    let replay_readback = read_replay_row(&db, request.replay_row.prediction_id)?
        .ok_or_else(|| anyhow!("replay readback missing after write"))?;
    Ok(json!({
        "tool": tool_names::MEJEPA_RECORD_MISTAKE,
        "sourceOfTruth": source_of_truth(&db_path),
        "mistakeRow": mistake_readback,
        "replayRow": replay_readback,
        "onlineHeadUpdate": report,
        "counts": cf_counts(&db)?
    }))
}

fn validate_record_request_prewrite(
    request: &RecordMistakeRequest,
    config: OnlineHeadUpdateConfig,
) -> AnyhowResult<()> {
    if request.now_unix_ms <= 0 {
        bail!("nowUnixMs must be positive");
    }
    online_head_key(&request.panel_signature_hash)?;
    request.mistake_row.validate()?;
    request.replay_row.validate()?;
    if request.mistake_row.prediction_id != request.replay_row.prediction_id {
        bail!("predictionId mismatch between mistakeRow and replayRow");
    }
    if request.mistake_row.replay_row_key != hex::encode(request.replay_row.prediction_id.0) {
        bail!("mistakeRow.replayRowKey does not reference replayRow.predictionId");
    }
    if !record_contexts_agree(&request.mistake_row, &request.replay_row) {
        bail!("mistakeRow and replayRow disagree on label/skill/ability/membership context");
    }
    ensure_binary_verdict(
        "baseVerdictBeforeUpdate",
        request.base_verdict_before_update,
    )?;
    ensure_binary_verdict(
        "mistakeRow.groundTruthVerdict",
        request.mistake_row.ground_truth_verdict,
    )?;
    if !config.learning_rate.is_finite() || !(0.0..=1.0).contains(&config.learning_rate) {
        bail!("onlineHeadConfig.learningRate must be finite in [0,1]");
    }
    if config.learning_rate == 0.0 || config.repeat_window_size == 0 {
        bail!("onlineHeadConfig.learningRate and repeatWindowSize must be positive");
    }
    Ok(())
}

fn run_mistake_history(request: MistakeHistoryRequest) -> AnyhowResult<Value> {
    if request.limit == 0 || request.limit > 10_000 {
        bail!("limit must be in 1..=10000");
    }
    let db_path = resolve_mistake_loop_db_path(request.db_path)?;
    let db = open_ability_resolver_rocksdb(&db_path, false)
        .with_context(|| format!("open mistake-loop DB {}", db_path.display()))?;
    let prediction_id = request
        .prediction_id_hex
        .as_deref()
        .map(prediction_id_from_hex)
        .transpose()?;
    let lifecycle_rows = if request.include_lifecycle_audits {
        read_all_skill_lifecycle_audit_rows(&db)?
    } else {
        Vec::new()
    };
    let mut rows = if let Some(mistake_id) = request.mistake_id.as_deref() {
        read_mistake_log_row(&db, mistake_id)?.into_iter().collect()
    } else {
        read_all_mistake_log_rows(&db)?
    };
    rows.retain(|row| {
        prediction_id.is_none_or(|id| row.prediction_id == id)
            && request
                .code_state_key
                .as_ref()
                .is_none_or(|value| row.code_state_key == *value)
            && request
                .label_signature_hash
                .as_ref()
                .is_none_or(|value| row.label_signature_hash == *value)
            && request
                .skill_signature_hash
                .as_ref()
                .is_none_or(|value| row.skill_signature_hash.as_ref() == Some(value))
            && request
                .ability_signature_hash
                .as_ref()
                .is_none_or(|value| row.ability_signature_hash.as_ref() == Some(value))
            && request
                .membership_signature_hash
                .as_ref()
                .is_none_or(|value| row.membership_signature_hash.as_ref() == Some(value))
    });
    rows.sort_by(|a, b| {
        b.created_at_unix_ms
            .cmp(&a.created_at_unix_ms)
            .then_with(|| a.mistake_id.cmp(&b.mistake_id))
    });
    rows.truncate(request.limit);
    let entries = rows
        .into_iter()
        .map(|row| {
            let replay = if request.include_replay_rows {
                Some(read_replay_row(&db, row.prediction_id)?)
            } else {
                None
            };
            let lifecycle = lifecycle_rows
                .iter()
                .filter(|audit| audit.mistake_id.as_deref() == Some(row.mistake_id.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            Ok(json!({
                "mistakeRow": row,
                "replayRow": replay.flatten(),
                "skillLifecycleAudits": lifecycle
            }))
        })
        .collect::<AnyhowResult<Vec<_>>>()?;
    Ok(json!({
        "tool": tool_names::MEJEPA_MISTAKE_HISTORY,
        "sourceOfTruth": source_of_truth(&db_path),
        "returned": entries.len(),
        "entries": entries,
        "counts": cf_counts(&db)?
    }))
}

fn run_mistake_loop_status(request: MistakeLoopStatusRequest) -> AnyhowResult<Value> {
    if request.repeat_metric_limit > 10_000 {
        bail!("repeatMetricLimit must be <= 10000");
    }
    let db_path = resolve_mistake_loop_db_path(request.db_path.clone())?;
    let db = open_ability_resolver_rocksdb(&db_path, false)
        .with_context(|| format!("open mistake-loop DB {}", db_path.display()))?;
    let head_key = online_head_key(&request.panel_signature_hash)?;
    let online_head_state = read_online_head_state_row(&db, &head_key)?;
    let neighbor_prediction = neighbor_prediction_report(&db, &request)?;
    let (online_head_state_count, repeat_metrics) =
        scan_online_head_cf(&db, request.repeat_metric_limit)?;
    let latest_mistake = read_all_mistake_log_rows(&db)?.into_iter().max_by(|a, b| {
        a.created_at_unix_ms
            .cmp(&b.created_at_unix_ms)
            .then_with(|| b.mistake_id.cmp(&a.mistake_id))
    });
    Ok(json!({
        "tool": tool_names::MEJEPA_MISTAKE_LOOP_STATUS,
        "sourceOfTruth": source_of_truth(&db_path),
        "panelSignatureHash": request.panel_signature_hash,
        "onlineHeadKey": head_key,
        "matchingOnlineHeadStateExists": online_head_state.is_some(),
        "onlineHeadState": online_head_state,
        "neighborPrediction": neighbor_prediction,
        "onlineHeadStateCount": online_head_state_count,
        "repeatMetrics": repeat_metrics,
        "repeatMetricCountReturned": repeat_metrics.len(),
        "latestMistake": latest_mistake,
        "counts": cf_counts(&db)?
    }))
}

fn neighbor_prediction_report(
    db: &rocksdb::DB,
    request: &MistakeLoopStatusRequest,
) -> AnyhowResult<Option<Value>> {
    let requested = request.neighbor_context.is_some()
        || !request.neighbors.is_empty()
        || request.neighbor_config.is_some()
        || request.base_verdict.is_some();
    if !requested {
        return Ok(None);
    }
    let context = request
        .neighbor_context
        .as_ref()
        .ok_or_else(|| anyhow!("neighborContext is required for neighbor prediction status"))?;
    let base_verdict = request.base_verdict.unwrap_or(Verdict::Pass);
    ensure_binary_verdict("baseVerdict", base_verdict)?;
    let report = predict_with_online_head_neighbors(
        db,
        &request.panel_signature_hash,
        base_verdict,
        context,
        &request.neighbors,
        request.neighbor_config.unwrap_or_default(),
    )?;
    Ok(Some(serde_json::to_value(report)?))
}

fn scan_online_head_cf(
    db: &rocksdb::DB,
    limit: usize,
) -> AnyhowResult<(usize, Vec<OnlineHeadRepeatMetricRow>)> {
    let cf = db
        .cf_handle(CF_MEJEPA_ONLINE_HEAD_STATE)
        .ok_or_else(|| anyhow!("missing {CF_MEJEPA_ONLINE_HEAD_STATE}"))?;
    let mut online_head_state_count = 0_usize;
    let mut repeat_metrics = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        let key = String::from_utf8(key.to_vec()).context("online-head key is not utf8")?;
        if key.starts_with("online_head:") {
            let row: OnlineHeadStateRow = bincode::deserialize(&value)?;
            row.validate()?;
            online_head_state_count += 1;
        } else if key.starts_with("mistake_repeat:") {
            let row: OnlineHeadRepeatMetricRow = bincode::deserialize(&value)?;
            row.validate()?;
            repeat_metrics.push(row);
        }
    }
    repeat_metrics.sort_by(|a, b| {
        b.updated_at_unix_ms
            .cmp(&a.updated_at_unix_ms)
            .then_with(|| a.metric_key.cmp(&b.metric_key))
    });
    repeat_metrics.truncate(limit);
    Ok((online_head_state_count, repeat_metrics))
}

fn cf_counts(db: &rocksdb::DB) -> AnyhowResult<Value> {
    Ok(json!({
        CF_MEJEPA_MISTAKE_LOG: count_cf_rows(db, CF_MEJEPA_MISTAKE_LOG)?,
        CF_MEJEPA_REPLAY_BUFFER: count_cf_rows(db, CF_MEJEPA_REPLAY_BUFFER)?,
        CF_MEJEPA_ONLINE_HEAD_STATE: count_cf_rows(db, CF_MEJEPA_ONLINE_HEAD_STATE)?,
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT: count_cf_rows(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?
    }))
}

fn default_history_limit() -> usize {
    100
}

fn default_repeat_metric_limit() -> usize {
    20
}

fn default_true() -> bool {
    true
}

fn parse_tool_request<T: DeserializeOwned>(
    args: serde_json::Value,
    tool_name: &str,
) -> Result<T, String> {
    serde_json::from_value(args)
        .map_err(|err| format!("{tool_name} schema validation failed: {err}"))
}

fn resolve_mistake_loop_db_path(input: Option<PathBuf>) -> AnyhowResult<PathBuf> {
    match input {
        Some(path) if !path.as_os_str().is_empty() => Ok(path),
        Some(_) => bail!("dbPath must be a non-empty path"),
        None => {
            let raw = std::env::var(ENV_TRAIN_DB)
                .or_else(|_| std::env::var(ENV_INFER_DB))
                .with_context(|| {
                    format!("dbPath, {ENV_TRAIN_DB}, or {ENV_INFER_DB} is required")
                })?;
            if raw.trim().is_empty() {
                bail!("{ENV_TRAIN_DB}/{ENV_INFER_DB} must not be empty");
            }
            Ok(PathBuf::from(raw))
        }
    }
}

fn source_of_truth(db_path: &Path) -> Value {
    mejepa_db_source_of_truth(
        db_path,
        json!({
            "cfs": context_graph_mejepa_train::ability_resolver_cfs(),
            "writesMistakeLog": CF_MEJEPA_MISTAKE_LOG,
            "writesReplayBuffer": CF_MEJEPA_REPLAY_BUFFER,
            "readsOnlineHeadState": CF_MEJEPA_ONLINE_HEAD_STATE,
            "readsSkillLifecycleAudit": CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
            "readsNeighborPredictionReport": true,
            "noCachedJsonSummary": true,
            "noNewPredictionHeadIntroduced": true
        }),
    )
}

fn prediction_id_from_hex(value: &str) -> AnyhowResult<PredictionId> {
    let bytes = hex::decode(value).context("predictionIdHex must be hex")?;
    if bytes.len() != 16 {
        bail!("predictionIdHex must decode to 16 bytes");
    }
    let mut id = [0_u8; 16];
    id.copy_from_slice(&bytes);
    Ok(PredictionId(id))
}

fn ensure_binary_verdict(field: &str, verdict: Verdict) -> AnyhowResult<()> {
    match verdict {
        Verdict::Pass | Verdict::Fail => Ok(()),
        Verdict::OutOfDistribution | Verdict::Abstain | Verdict::GuardRejected => {
            bail!("{field} must be pass/fail for mistake-loop learning")
        }
    }
}

fn record_contexts_agree(mistake: &MistakeLogRow, replay: &ReplayBufferRow) -> bool {
    mistake.accepted_label_ids == replay.accepted_label_ids
        && mistake.active_skill_ids == replay.active_skill_ids
        && mistake.active_higher_ability_ids == replay.active_higher_ability_ids
        && mistake.source_membership_keys == replay.source_membership_keys
        && Some(mistake.label_signature_hash.clone()) == replay.label_signature_hash
        && mistake.skill_signature_hash == replay.skill_signature_hash
        && mistake.ability_signature_hash == replay.ability_signature_hash
        && mistake.membership_signature_hash == replay.membership_signature_hash
}

fn mistake_loop_error(
    handlers: &Handlers,
    id: Option<JsonRpcId>,
    code: &str,
    err: anyhow::Error,
) -> JsonRpcResponse {
    handlers.tool_error_structured(
        id,
        ToolErrorKind::Storage,
        code,
        &err.to_string(),
        json!({"toolFamily": "mejepa_mistake_loop"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa_train::label_bridge::{
        ability_signature_hash, accepted_label_signature_hash, membership_signature_hash,
        skill_signature_hash,
    };
    use context_graph_mejepa_train::{
        ability_aware_replay_cell_id, mistake_id_from_evidence_parts, MistakeTruthSource,
        ReplayBufferSource, ReplayRetentionTier,
    };
    use tempfile::TempDir;

    const TEST_NOW_MS: i64 = 1_780_470_400_000;

    #[test]
    fn rejects_bad_prediction_hex() {
        assert!(prediction_id_from_hex("abcd").is_err());
        assert!(prediction_id_from_hex("not-hex").is_err());
    }

    #[test]
    fn status_returns_bounded_neighbor_prediction_report() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("mistake-loop.db");
        {
            let _db = open_ability_resolver_rocksdb(&db_path, true).unwrap();
        }
        let rows = build_test_rows([0x42; 16], "mcp-neighbor").unwrap();
        run_record_mistake(RecordMistakeRequest {
            db_path: Some(db_path.clone()),
            panel_signature_hash: "panel:mcp:test:source".to_string(),
            mistake_row: rows.mistake.clone(),
            replay_row: rows.replay.clone(),
            base_verdict_before_update: Verdict::Pass,
            now_unix_ms: TEST_NOW_MS + 1,
            online_head_config: Some(OnlineHeadUpdateConfig {
                learning_rate: 1.0,
                repeat_window_size: 100,
            }),
        })
        .unwrap();

        let status = run_mistake_loop_status(MistakeLoopStatusRequest {
            db_path: Some(db_path),
            panel_signature_hash: "panel:mcp:test:heldout".to_string(),
            repeat_metric_limit: 10,
            base_verdict: Some(Verdict::Pass),
            neighbor_context: Some(
                OnlineHeadNeighborContext::from_replay_row(&rows.replay).unwrap(),
            ),
            neighbors: vec![OnlineHeadNeighbor {
                panel_signature_hash: "panel:mcp:test:source".to_string(),
                distance: 0.01,
            }],
            neighbor_config: Some(OnlineHeadNeighborConfig::default()),
        })
        .unwrap();

        assert_eq!(status["matchingOnlineHeadStateExists"], json!(false));
        assert_eq!(
            status["neighborPrediction"]["source"],
            json!("neighborPanel")
        );
        assert_eq!(
            status["neighborPrediction"]["correctedVerdict"],
            json!("fail")
        );
        assert_eq!(
            status["neighborPrediction"]["matchedPanelSignatureHash"],
            json!("panel:mcp:test:source")
        );
        assert_eq!(
            status["neighborPrediction"]["flatVectorConcatUsed"],
            json!(false)
        );
        assert_eq!(
            status["neighborPrediction"]["claimsFisherEwcProtection"],
            json!(false)
        );
    }

    struct TestRows {
        mistake: MistakeLogRow,
        replay: ReplayBufferRow,
    }

    fn build_test_rows(
        prediction_bytes: [u8; 16],
        suffix: &str,
    ) -> Result<TestRows, Box<dyn std::error::Error>> {
        let prediction_id = PredictionId(prediction_bytes);
        let accepted_label_ids = vec!["ast_surface:function".to_string()];
        let active_skill_ids = vec!["skill:unit_sequence".to_string()];
        let active_higher_ability_ids = vec!["ability:boundary_sequence".to_string()];
        let source_membership_keys =
            vec!["chunk_skill::chunk:mcp-status::python:before::skill:unit_sequence".to_string()];
        let label_signature_hash = accepted_label_signature_hash(&accepted_label_ids)?;
        let skill_signature_hash = Some(skill_signature_hash(&active_skill_ids)?);
        let ability_signature_hash = Some(ability_signature_hash(&active_higher_ability_ids)?);
        let membership_signature_hash = Some(membership_signature_hash(&source_membership_keys)?);
        let code_state_key = format!("python:before:mcp-status:{suffix}");
        let named_failure_mode = "failure:neighbor_status".to_string();
        let replay_row_key = hex::encode(prediction_id.0);
        let mistake_id = mistake_id_from_evidence_parts(
            prediction_id,
            &code_state_key,
            &label_signature_hash,
            skill_signature_hash.as_deref(),
            ability_signature_hash.as_deref(),
            membership_signature_hash.as_deref(),
            Verdict::Fail,
        )?;
        let cell_id = ability_aware_replay_cell_id(
            "python",
            "mcp_status_mistake_loop",
            &code_state_key,
            &named_failure_mode,
            &label_signature_hash,
            skill_signature_hash.as_deref(),
            ability_signature_hash.as_deref(),
            membership_signature_hash.as_deref(),
        )?;
        Ok(TestRows {
            mistake: MistakeLogRow {
                schema_version: 1,
                mistake_id,
                prediction_id,
                predicted_verdict: Verdict::Pass,
                ground_truth_verdict: Verdict::Fail,
                truth_source: MistakeTruthSource::SwebenchDockerOracle,
                code_state_key: code_state_key.clone(),
                named_failure_mode: Some(named_failure_mode),
                accepted_label_ids: accepted_label_ids.clone(),
                active_skill_ids: active_skill_ids.clone(),
                label_signature_hash: label_signature_hash.clone(),
                skill_signature_hash: skill_signature_hash.clone(),
                failure_evidence_set_ids: vec![code_state_key],
                replay_row_key,
                accepted_registry_sha256: Some("sha256:mcp-status-accepted-registry".to_string()),
                usefulness_metrics_sha256: Some("sha256:mcp-status-usefulness".to_string()),
                learning_bridge_manifest_sha256: Some("sha256:mcp-status-bridge".to_string()),
                created_at_unix_ms: TEST_NOW_MS,
                active_higher_ability_ids: active_higher_ability_ids.clone(),
                source_membership_keys: source_membership_keys.clone(),
                ability_signature_hash: ability_signature_hash.clone(),
                membership_signature_hash: membership_signature_hash.clone(),
            },
            replay: ReplayBufferRow {
                prediction_id,
                surprise_z: 2.0,
                cell_id,
                coverage_gap_score: 0.75,
                last_replayed_ts: None,
                replay_count: 0,
                retention_weight: 1.0,
                protected: true,
                retention_tier: ReplayRetentionTier::Hot,
                source: ReplayBufferSource::AcceptedLabelMistake,
                created_at_unix_ms: TEST_NOW_MS,
                updated_at_unix_ms: TEST_NOW_MS,
                accepted_label_ids,
                active_skill_ids,
                active_higher_ability_ids,
                source_membership_keys,
                label_signature_hash: Some(label_signature_hash),
                skill_signature_hash,
                ability_signature_hash,
                membership_signature_hash,
            },
        })
    }
}
