// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

//! Durable readers for Phase 7 ME-JEPA MCP tools.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use context_graph_mejepa::{
    build_slot_preserving_cuda_compiler, decode_reality_prediction, materialize_inference_panels,
    open_infer_rocksdb, panel_sha, CalibrationStore, MeJepaInferConfig, MejepaStore, PatchBundle,
    RealityPrediction, RocksDbInferStore, TaskContext, TrainCertSummary,
};
use context_graph_mejepa_instruments::materialize::TimeStep;
use context_graph_mejepa_instruments::panel_store::{PanelKey, PanelStore, CF_MEJEPA_PANEL_META};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PanelEnvelope, PanelProvenance};
use context_graph_mejepa_shift_subscriber::{shift_to_inference, ShiftEntry, ShiftId};
use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

use crate::handlers::tools::reality_loop::helpers::{
    file_arg_to_path, file_sot, read_active_runtime_root,
};

const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";
const ENV_PANEL_DB: &str = "CONTEXTGRAPH_MEJEPA_PANEL_DB";
const ENV_SHIFT_LOG_ROOT: &str = "CONTEXTGRAPH_MEJEPA_SHIFT_LOG_ROOT";
const AUDIT_CHUNK_SIZE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct WatermarkRecord {
    session_id: String,
    last_consumed_shift_id: String,
    last_consumed_byte_offset: u64,
    last_advanced_at_unix_seconds: i64,
    producer_tool_name: Option<String>,
    source_log_path: Option<String>,
}

impl WatermarkRecord {
    fn validate(&self, key: &str) -> Result<(), String> {
        validate_session_hex32(&self.session_id)?;
        let expected = watermark_key(&self.session_id);
        if key != expected {
            return Err(format!(
                "watermark key {key:?} does not match canonical key {expected:?}"
            ));
        }
        if !valid_shift_id(&self.last_consumed_shift_id) {
            return Err(format!(
                "watermark {key:?} has invalid last_consumed_shift_id {:?}",
                self.last_consumed_shift_id
            ));
        }
        if self.last_advanced_at_unix_seconds <= 0 {
            return Err(format!(
                "watermark {key:?} has non-positive last_advanced_at_unix_seconds {}",
                self.last_advanced_at_unix_seconds
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct LocatedShift {
    pub shift_id: String,
    pub session_id: String,
    pub byte_offset: u64,
    pub next_byte_offset: u64,
    pub path: String,
    pub record: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct PanelAuditSummary {
    time_step: String,
    panel_hash: String,
    filled_mask: u16,
    active_slots: Vec<&'static str>,
    missing_slots: Vec<&'static str>,
    provenance: Value,
}

pub(crate) fn subscriber_state_for_provenance(
    db: &DB,
    session_id: [u8; 16],
) -> Result<Option<Value>, String> {
    let session_hex = hex::encode(session_id);
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
        .ok_or_else(|| "missing CF_MEJEPA_SHIFT_WATERMARK handle".to_string())?;
    let key = watermark_key(&session_hex);
    let Some(bytes) = db
        .get_cf(cf, key.as_bytes())
        .map_err(|err| format!("watermark point read failed for {key}: {err}"))?
    else {
        return Ok(None);
    };
    let record: WatermarkRecord = serde_json::from_slice(&bytes)
        .map_err(|err| format!("watermark value for key {key:?} is invalid JSON: {err}"))?;
    record.validate(&key)?;
    Ok(Some(watermark_to_value(&record, &key)))
}

pub(crate) fn valid_shift_id(value: &str) -> bool {
    value.len() == 23
        && value.starts_with("01J")
        && value[3..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_lowercase())
}

pub(crate) fn validate_attempt_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128 {
        return Err("attemptId must be 1..=128 bytes".to_string());
    }
    if value.chars().any(|ch| ch.is_control() || ch == '\0') {
        return Err("attemptId must not contain control characters or NUL".to_string());
    }
    Ok(())
}

pub(crate) async fn locate_shift(shift_id: &str) -> Result<Option<LocatedShift>, String> {
    let log_dir = shift_log_dir().await?;
    if !log_dir.is_dir() {
        return Ok(None);
    }
    let entries = std::fs::read_dir(&log_dir)
        .map_err(|err| format!("failed to read shift-log dir {}: {err}", log_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read shift-log entry: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let bytes = std::fs::read(&path)
            .map_err(|err| format!("failed to read shift-log file {}: {err}", path.display()))?;
        let mut offset = 0u64;
        for line in bytes.split_inclusive(|byte| *byte == b'\n') {
            let next_offset = offset + line.len() as u64;
            let trimmed = trim_line(line);
            if trimmed.is_empty() {
                offset = next_offset;
                continue;
            }
            let record: Value = serde_json::from_slice(trimmed).map_err(|err| {
                format!(
                    "MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL: path={} byte_offset={} error={err}",
                    path.display(),
                    offset
                )
            })?;
            if record.get("shift_id").and_then(Value::as_str) == Some(shift_id) {
                let session_id = record
                    .get("session_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        format!(
                            "MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL: path={} byte_offset={} missing session_id",
                            path.display(),
                            offset
                        )
                    })?
                    .to_string();
                return Ok(Some(LocatedShift {
                    shift_id: shift_id.to_string(),
                    session_id,
                    byte_offset: offset,
                    next_byte_offset: next_offset,
                    path: path.display().to_string(),
                    record,
                }));
            }
            offset = next_offset;
        }
    }
    Ok(None)
}

pub(crate) fn replay_shift(shift: &LocatedShift) -> Result<Value, String> {
    validate_session_hex32(&shift.session_id)?;
    let infer_db_path = required_env_path(ENV_INFER_DB)?;
    let panel_db_path = required_env_path(ENV_PANEL_DB)?;
    let db = open_infer_rocksdb(&infer_db_path).map_err(|err| err.to_string())?;
    let session_id = decode_session_hex32(&shift.session_id)?;
    let watermark_before = subscriber_state_for_provenance(db.as_ref(), session_id)
        .map_err(|err| format!("watermark read before replay failed: {err}"))?;
    let (patch, context, source) = patch_context_from_shift(shift)?;
    let panels = materialize_inference_panels(&patch, &context).map_err(|err| err.to_string())?;
    let calibration = CalibrationStore::new(db.clone(), 30).map_err(|err| err.to_string())?;
    let store = RocksDbInferStore::new(db.clone());
    let compiler = build_slot_preserving_cuda_compiler(
        context.environment.repo_root.clone(),
        std::sync::Arc::new(store.clone()),
        calibration,
        MeJepaInferConfig::default(),
    )
    .map_err(|err| err.to_string())?;
    let mut prediction = compiler
        .compile(&patch, &context)
        .map_err(|err| err.to_string())?;
    prediction.created_at_unix_ms = timestamp_ns_to_ms(shift)?;
    prediction = RealityPrediction::try_new(prediction).map_err(|err| err.to_string())?;
    store
        .write_live_prediction(&prediction)
        .map_err(|err| err.to_string())?;
    let prediction_readback = MejepaStore::read_live_predictions(&store, context.session_id, 1000)
        .map_err(|err| err.to_string())?;
    let persisted_matches = prediction_readback
        .iter()
        .filter(|row| {
            row.prediction_id == prediction.prediction_id
                && row.created_at_unix_ms == prediction.created_at_unix_ms
                && row.task_id == prediction.task_id
        })
        .count();
    if persisted_matches != 1 {
        return Err(format!(
            "MEJEPA_OBSERVE_SHIFT_PREDICTION_READBACK_MISMATCH: expected 1 persisted prediction row for replay, found {persisted_matches}"
        ));
    }
    let panel_store = PanelStore::open(&panel_db_path).map_err(|err| err.to_string())?;
    persist_panels(&panel_store, &context, &patch, &source, panels)?;
    let watermark_after = subscriber_state_for_provenance(db.as_ref(), context.session_id)
        .map_err(|err| format!("watermark read after replay failed: {err}"))?;
    info!(
        code = "MEJEPA_OBSERVE_SHIFT_REPLAYED",
        shift_id = %shift.shift_id,
        session_id = %shift.session_id,
        prediction_id = %hex::encode(prediction.prediction_id),
        "replayed durable shift into ME-JEPA prediction and panel stores"
    );
    Ok(json!({
        "shift_id": shift.shift_id,
        "shift_log_path": shift.path,
        "shift_byte_offset": shift.byte_offset,
        "shift_next_byte_offset": shift.next_byte_offset,
        "attempt_id": context.task_id.0,
        "session_id": shift.session_id,
        "prediction_id": hex::encode(prediction.prediction_id),
        "prediction_created_at_unix_ms": prediction.created_at_unix_ms,
        "prediction_readback_count": persisted_matches,
        "panel_db": file_sot(&panel_db_path),
        "infer_db": file_sot(&infer_db_path),
        "source": source,
        "watermark_before": watermark_before,
        "watermark_after": watermark_after,
        "watermark_advanced": watermark_before != watermark_after
    }))
}

pub(crate) async fn subscriber_status() -> Result<Value, String> {
    let db_path = required_env_path(ENV_INFER_DB)?;
    let panel_db_path = match std::env::var(ENV_PANEL_DB) {
        Ok(path) => Some(PathBuf::from(path)),
        Err(std::env::VarError::NotPresent) => None,
        Err(err) => return Err(format!("{ENV_PANEL_DB} must be readable UTF-8: {err}")),
    };
    subscriber_status_for_paths(&db_path, panel_db_path.as_deref()).await
}

pub(crate) async fn subscriber_status_for_paths(
    db_path: &Path,
    panel_db_path: Option<&Path>,
) -> Result<Value, String> {
    let db = open_infer_rocksdb(db_path).map_err(|err| err.to_string())?;
    let watermarks = read_watermarks(db.as_ref())?;
    let last_watermark_per_session = watermarks
        .iter()
        .map(|(session, record)| {
            (
                session.clone(),
                record
                    .get("last_consumed_shift_id")
                    .cloned()
                    .unwrap_or(Value::Null),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let instrument_cache_entries = match panel_db_path {
        Some(path) => {
            let store = PanelStore::open(path).map_err(|err| err.to_string())?;
            json!({
                "panel_db": file_sot(path),
                "panels": store.count_cf(context_graph_mejepa_instruments::CF_MEJEPA_PANELS).map_err(|err| err.to_string())?,
                "panel_meta": store.count_cf(context_graph_mejepa_instruments::CF_MEJEPA_PANEL_META).map_err(|err| err.to_string())?
            })
        }
        None => Value::Null,
    };
    Ok(json!({
        "last_watermark_per_session": last_watermark_per_session,
        "watermark_records": watermarks,
        "lag": observed_shift_lag(&watermarks).await?,
        "lag_alert_active": false,
        "error_rate_per_1000": Value::Null,
        "instrument_cache_hit_rate": Value::Null,
        "instrument_cache_capacity": Value::Null,
        "instrument_cache_entries": instrument_cache_entries,
        "rss_bytes": rss_bytes()?,
        "task_alive_since": Value::Null,
        "dropped_l_step_below_threshold_count": 0,
        "last_panic": Value::Null,
        "subscriber_running": false,
        "source_of_truth": file_sot(db_path),
        "status_scope": "durable_rocksdb_watermarks_and_panel_counts_no_live_task"
    }))
}

pub(crate) fn capture_audit(attempt_id: &str, page: u32) -> Result<Value, String> {
    let panel_db = required_env_path(ENV_PANEL_DB)?;
    let infer_db = required_env_path(ENV_INFER_DB)?;
    let panel_store = PanelStore::open(&panel_db).map_err(|err| err.to_string())?;
    let panels = read_panel_summaries(&panel_store, attempt_id)?;
    let infer = open_infer_rocksdb(&infer_db).map_err(|err| err.to_string())?;
    let predictions =
        read_predictions_for_attempt(&RocksDbInferStore::new(infer.clone()), attempt_id)?;
    let train_certs = read_train_certs(infer.as_ref())?;
    let oracle_examples = read_oracle_examples(infer.as_ref(), attempt_id)?;

    if panels.is_empty() && predictions.is_empty() {
        if page != 0 {
            return Err(
                "MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE: not-found audit has one page".to_string(),
            );
        }
        return Ok(json!({
            "attempt_found": false,
            "nearest_neighbors": nearest_panel_attempts(&panel_store, attempt_id)?,
            "total_pages": 1,
            "current_page": page,
            "source_of_truth": {
                "panel_db": file_sot(&panel_db),
                "infer_db": file_sot(&infer_db)
            }
        }));
    }

    let missing_slots = panels
        .iter()
        .flat_map(|panel| panel.missing_slots.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let bundle = json!({
        "attempt_id": attempt_id,
        "attempt_found": true,
        "panels": panels,
        "predictions": predictions,
        "oracle_examples": oracle_examples,
        "train_certs": train_certs,
        "missing_slots": missing_slots,
        "counterfactuals": [],
        "aux_head_predictions": [],
        "source_of_truth": {
            "panel_db": file_sot(&panel_db),
            "infer_db": file_sot(&infer_db)
        }
    });
    paginate_bundle(bundle, page)
}

async fn shift_log_dir() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var(ENV_SHIFT_LOG_ROOT) {
        let path = PathBuf::from(path);
        if path.file_name().and_then(|name| name.to_str()) == Some("cgreality-shift-log") {
            return Ok(path);
        }
        return Ok(path.join("cgreality-shift-log"));
    }
    let runtime_root = read_active_runtime_root()
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| {
            format!("{ENV_SHIFT_LOG_ROOT} is not set and no active cgreality runtime root exists")
        })?;
    Ok(runtime_root.join("cgreality-shift-log"))
}

fn trim_line(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && (line[end - 1] == b'\n' || line[end - 1] == b'\r') {
        end -= 1;
    }
    &line[..end]
}

fn read_watermarks(db: &DB) -> Result<BTreeMap<String, Value>, String> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
        .ok_or_else(|| "missing CF_MEJEPA_SHIFT_WATERMARK handle".to_string())?;
    let mut out = BTreeMap::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item.map_err(|err| format!("watermark iterator failed: {err}"))?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| format!("watermark key is not UTF-8: {err}"))?;
        let record: WatermarkRecord = serde_json::from_slice(&value)
            .map_err(|err| format!("watermark value for key {key:?} is invalid JSON: {err}"))?;
        record.validate(&key)?;
        out.insert(record.session_id.clone(), watermark_to_value(&record, &key));
    }
    Ok(out)
}

fn watermark_to_value(record: &WatermarkRecord, key: &str) -> Value {
    json!({
        "session_id": record.session_id,
        "last_consumed_shift_id": record.last_consumed_shift_id,
        "last_consumed_byte_offset": record.last_consumed_byte_offset,
        "last_advanced_at_unix_seconds": record.last_advanced_at_unix_seconds,
        "producer_tool_name": record.producer_tool_name,
        "source_log_path": record.source_log_path,
        "source_key": key,
        "subscriber_lag_at_call_time": Value::Null
    })
}

fn read_panel_summaries(
    panel_store: &PanelStore,
    attempt_id: &str,
) -> Result<Vec<PanelAuditSummary>, String> {
    let mut out = Vec::new();
    for time_step in [TimeStep::T0, TimeStep::T1, TimeStep::T2] {
        let key = PanelKey::new(attempt_id, time_step).map_err(|err| err.to_string())?;
        let Some(envelope) = panel_store
            .get_envelope(&key)
            .map_err(|err| err.to_string())?
        else {
            continue;
        };
        let active_slots = InstrumentSlot::all()
            .into_iter()
            .filter(|slot| envelope.panel.is_filled(*slot))
            .map(|slot| slot.slug())
            .collect::<Vec<_>>();
        let missing_slots = InstrumentSlot::all()
            .into_iter()
            .filter(|slot| !envelope.panel.is_filled(*slot))
            .map(|slot| slot.slug())
            .collect::<Vec<_>>();
        out.push(PanelAuditSummary {
            time_step: format!("{time_step:?}").to_lowercase(),
            panel_hash: envelope.panel_hash,
            filled_mask: envelope.panel.filled_mask(),
            active_slots,
            missing_slots,
            provenance: serde_json::to_value(envelope.provenance).map_err(|err| err.to_string())?,
        });
    }
    Ok(out)
}

fn read_predictions_for_attempt(
    store: &RocksDbInferStore,
    attempt_id: &str,
) -> Result<Vec<RealityPrediction>, String> {
    let db = store.db();
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or_else(|| "missing CF_MEJEPA_LIVE_PREDICTIONS handle".to_string())?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) =
            item.map_err(|err| format!("live prediction iterator failed: {err}"))?;
        let prediction = decode_reality_prediction(&value)
            .map_err(|err| format!("live prediction payload is invalid: {err}"))?;
        if prediction.task_id.0 == attempt_id {
            out.push(prediction);
        }
    }
    out.sort_by_key(|row| std::cmp::Reverse(row.created_at_unix_ms));
    Ok(out)
}

fn read_train_certs(db: &DB) -> Result<Vec<TrainCertSummary>, String> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| "missing CF_MEJEPA_TRAIN_CERTS handle".to_string())?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::End) {
        let (_key, value) = item.map_err(|err| format!("train cert iterator failed: {err}"))?;
        let cert: TrainCertSummary = bincode::deserialize(&value)
            .map_err(|err| format!("train cert payload is invalid bincode: {err}"))?;
        cert.validate().map_err(|err| err.to_string())?;
        out.push(cert);
        if out.len() == 100 {
            break;
        }
    }
    Ok(out)
}

fn read_oracle_examples(db: &DB, attempt_id: &str) -> Result<Vec<Value>, String> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
        .ok_or_else(|| "missing CF_MEJEPA_ORACLE_VERDICTS handle".to_string())?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item.map_err(|err| format!("oracle verdict iterator failed: {err}"))?;
        let key_text = String::from_utf8_lossy(&key);
        if !key_text.contains(attempt_id) {
            continue;
        }
        let example: context_graph_mejepa::CalibrationExample = bincode::deserialize(&value)
            .map_err(|err| format!("oracle verdict payload is invalid bincode: {err}"))?;
        out.push(json!({
            "key": key_text,
            "key_hex": hex::encode(key),
            "example": example
        }));
        if out.len() == 100 {
            break;
        }
    }
    Ok(out)
}

fn nearest_panel_attempts(
    panel_store: &PanelStore,
    attempt_id: &str,
) -> Result<Vec<String>, String> {
    let mut candidates = panel_store
        .scan_cf_json(CF_MEJEPA_PANEL_META)
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter_map(|(key, _)| key.split('/').next().map(ToOwned::to_owned))
        .filter(|candidate| candidate != attempt_id)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        nearest_score(left, attempt_id)
            .cmp(&nearest_score(right, attempt_id))
            .then_with(|| left.cmp(right))
    });
    candidates.dedup();
    candidates.truncate(5);
    Ok(candidates)
}

fn nearest_score(candidate: &str, target: &str) -> (usize, usize) {
    let common = candidate
        .chars()
        .zip(target.chars())
        .take_while(|(left, right)| left == right)
        .count();
    (target.len().abs_diff(candidate.len()), usize::MAX - common)
}

fn paginate_bundle(bundle: Value, page: u32) -> Result<Value, String> {
    let encoded = serde_json::to_vec(&bundle).map_err(|err| err.to_string())?;
    if encoded.len() <= AUDIT_CHUNK_SIZE_BYTES {
        if page != 0 {
            return Err(format!(
                "MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE: page {page} is out of range for 1 page"
            ));
        }
        return Ok(json!({
            "attempt_found": true,
            "bundle": bundle,
            "total_pages": 1,
            "current_page": page
        }));
    }
    let panels = bundle
        .get("panels")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "capture audit bundle lost panels array during pagination".to_string())?;
    let total_pages = panels.len().max(1) as u32;
    if page >= total_pages {
        return Err(format!(
            "MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE: page {page} is out of range for {total_pages} pages"
        ));
    }
    let mut paged = bundle;
    paged["panels"] = json!([panels[page as usize].clone()]);
    Ok(json!({
        "attempt_found": true,
        "bundle": paged,
        "total_pages": total_pages,
        "current_page": page
    }))
}

fn required_env_path(name: &str) -> Result<PathBuf, String> {
    std::env::var(name).map(PathBuf::from).map_err(|_| {
        format!("{name} must be set; refusing to guess a Phase 7 source-of-truth path")
    })
}

fn patch_context_from_shift(
    shift: &LocatedShift,
) -> Result<(PatchBundle, TaskContext, Value), String> {
    let entry = located_shift_to_entry(shift)?;
    let repo_root = repo_root_from_shift(shift);
    let (patch, context, source_sha, _attempt_id) =
        shift_to_inference(&entry, repo_root).map_err(|err| err.to_string())?;
    let source = json!({
        "relative_path": patch.ast_diff.hunks[0].path,
        "after_sha256": format!("sha256:{}", hex::encode(source_sha)),
        "patch_sha256": hex::encode(patch.patch_sha)
    });
    Ok((patch, context, source))
}

fn located_shift_to_entry(shift: &LocatedShift) -> Result<ShiftEntry, String> {
    Ok(ShiftEntry {
        shift_id: ShiftId::parse(shift.shift_id.clone()).map_err(|err| err.to_string())?,
        timestamp_unix_ns: shift
            .record
            .get("timestamp_unix_ns")
            .and_then(Value::as_u64)
            .ok_or_else(|| "shift timestamp_unix_ns must fit in u64".to_string())?
            as u128,
        tool_name: shift
            .record
            .get("tool_name")
            .and_then(Value::as_str)
            .ok_or_else(|| "shift tool_name is required".to_string())?
            .to_string(),
        tool_use_id: shift
            .record
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        session_id: context_graph_mejepa_shift_subscriber::decode_session_hex32(&shift.session_id)
            .map_err(|err| err.to_string())?,
        subject: shift
            .record
            .get("subject")
            .cloned()
            .ok_or_else(|| "shift subject is required".to_string())?,
        before: shift
            .record
            .get("before")
            .cloned()
            .ok_or_else(|| "shift before is required".to_string())?,
        after: shift
            .record
            .get("after")
            .cloned()
            .ok_or_else(|| "shift after is required".to_string())?,
        delta_summary: shift
            .record
            .get("delta_summary")
            .cloned()
            .ok_or_else(|| "shift delta_summary is required".to_string())?,
        verification: shift
            .record
            .get("verification")
            .cloned()
            .ok_or_else(|| "shift verification is required".to_string())?,
        harness_transition_path: shift
            .record
            .get("harness_transition_path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        byte_offset: shift.byte_offset,
        next_byte_offset: shift.next_byte_offset,
        source_log_path: PathBuf::from(&shift.path),
    })
}

fn repo_root_from_shift(shift: &LocatedShift) -> PathBuf {
    for pointer in [
        "/after/source_of_truth",
        "/after/sourceOfTruth",
        "/after_source_path",
    ] {
        if let Some(raw) = shift.record.pointer(pointer).and_then(Value::as_str) {
            let path = file_arg_to_path(raw);
            if let Some(parent) = path.parent() {
                return parent.to_path_buf();
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn persist_panels(
    panel_store: &PanelStore,
    context: &TaskContext,
    patch: &PatchBundle,
    source: &Value,
    panels: (Panel, Panel, Panel),
) -> Result<(), String> {
    for (time_step, panel) in [
        (TimeStep::T0, panels.0),
        (TimeStep::T1, panels.1),
        (TimeStep::T2, panels.2),
    ] {
        let key = PanelKey::new(&context.task_id.0, time_step).map_err(|err| err.to_string())?;
        let provenance = PanelProvenance {
            code_version: env!("CARGO_PKG_VERSION").to_string(),
            embedder_versions: BTreeMap::from([(
                "mejepa_deterministic_phase7_replay".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            )]),
            corpus_sha: hex::encode(patch.patch_sha),
            frozen_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            source_sha256: source
                .get("after_sha256")
                .and_then(Value::as_str)
                .and_then(|value| value.strip_prefix("sha256:"))
                .ok_or_else(|| "source after_sha256 missing sha256: prefix".to_string())?
                .to_string(),
        };
        let envelope =
            PanelEnvelope::try_new(time_step, panel, provenance).map_err(|err| err.to_string())?;
        panel_store
            .put_envelope(&key, &envelope)
            .map_err(|err| err.to_string())?;
        let loaded = panel_store
            .get_envelope(&key)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| format!("panel read-after-write missing {}", key.storage_key()))?;
        if loaded.panel_hash != envelope.panel_hash {
            return Err(format!(
                "panel read-after-write hash mismatch for {}",
                key.storage_key()
            ));
        }
        let actual_sha = hex::encode(panel_sha(&loaded.panel));
        if actual_sha != loaded.panel_hash {
            return Err(format!(
                "panel read-after-write payload hash mismatch for {}",
                key.storage_key()
            ));
        }
    }
    Ok(())
}

fn timestamp_ns_to_ms(shift: &LocatedShift) -> Result<i64, String> {
    let ns = shift
        .record
        .get("timestamp_unix_ns")
        .and_then(Value::as_u64)
        .ok_or_else(|| "shift timestamp_unix_ns must fit in u64".to_string())?;
    i64::try_from(ns / 1_000_000)
        .map_err(|err| format!("timestamp_unix_ns millis overflowed i64: {err}"))
}

fn validate_session_hex32(value: &str) -> Result<(), String> {
    if value.len() != 32 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!(
            "session_id must be exactly 32 hexadecimal characters, got {value:?}"
        ));
    }
    if value.bytes().all(|byte| byte == b'0') {
        return Err("session_id must be non-zero".to_string());
    }
    Ok(())
}

fn decode_session_hex32(value: &str) -> Result<[u8; 16], String> {
    validate_session_hex32(value)?;
    let mut out = [0u8; 16];
    hex::decode_to_slice(value, &mut out)
        .map_err(|err| format!("session_id hex decode failed: {err}"))?;
    Ok(out)
}

fn watermark_key(session_id_hex: &str) -> String {
    format!("wm:{session_id_hex}")
}

async fn observed_shift_lag(watermarks: &BTreeMap<String, Value>) -> Result<Value, String> {
    if watermarks.is_empty() {
        return Ok(json!({}));
    }
    let log_dir = shift_log_dir().await?;
    if !log_dir.is_dir() {
        return Ok(Value::Null);
    }
    let mut out = BTreeMap::new();
    for (session, watermark) in watermarks {
        let Some(offset) = watermark
            .get("last_consumed_byte_offset")
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let path = log_dir.join(format!("{session}.jsonl"));
        if !path.is_file() {
            continue;
        }
        let len = std::fs::metadata(&path)
            .map_err(|err| format!("failed to stat shift log {}: {err}", path.display()))?
            .len();
        out.insert(session.clone(), len.saturating_sub(offset));
    }
    Ok(json!(out))
}

fn rss_bytes() -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        let raw = std::fs::read_to_string("/proc/self/statm")
            .map_err(|err| format!("failed to read /proc/self/statm: {err}"))?;
        let resident_pages = raw
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| "/proc/self/statm missing resident pages field".to_string())?
            .parse::<u64>()
            .map_err(|err| {
                format!("failed to parse resident pages from /proc/self/statm: {err}")
            })?;
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page_size <= 0 {
            return Err("sysconf(_SC_PAGESIZE) returned non-positive page size".to_string());
        }
        Ok(resident_pages.saturating_mul(page_size as u64))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("rss_bytes requires Linux /proc/self/statm".to_string())
    }
}

#[cfg(test)]
// Tests use the shared cross-module env lock (see test_env_lock in the
// handlers/tools mod.rs). The sync MutexGuard is intentionally held across
// `.await` points to keep CONTEXTGRAPH_MEJEPA_INFER_DB / PANEL_DB /
// SHIFT_LOG_ROOT stable for the duration of each `current_thread` tokio test.
// No deadlock risk: tests run single-threaded and the lock serializes them.
#[allow(clippy::await_holding_lock)]
mod phase7_fsv_tests {
    use super::*;
    use crate::handlers::tools::reality_loop::helpers::sha256_file;
    use context_graph_mejepa::Language;
    use context_graph_mejepa::{CalibrationExample, EmbedderId};
    use context_graph_mejepa_instruments::panel_store::PanelStore;
    use serde::Serialize;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::MutexGuard;
    use tempfile::TempDir;

    fn env_guard() -> MutexGuard<'static, ()> {
        // Shared cross-module lock — see crate::handlers::tools::test_env_lock.
        // We hold a sync MutexGuard for the duration of the test; tokio tests
        // here run on a current_thread flavor and never hand off the runtime
        // while the env vars are live, so the blocking lock is safe.
        let guard = crate::handlers::tools::test_env_lock::lock();
        std::env::remove_var(ENV_INFER_DB);
        std::env::remove_var(ENV_PANEL_DB);
        std::env::remove_var(ENV_SHIFT_LOG_ROOT);
        guard
    }

    #[tokio::test(flavor = "current_thread")]
    async fn phase7_mcp_fsv_happy_path_and_edges() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        let infer_db = root.join("infer-db");
        let panel_db = root.join("panel-db");
        let runtime_root = root.join("runtime");
        let repo = root.join("repo");
        let evidence_dir = root.join("evidence");
        std::fs::create_dir_all(&runtime_root).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&evidence_dir).unwrap();
        std::env::set_var(ENV_INFER_DB, &infer_db);
        std::env::set_var(ENV_PANEL_DB, &panel_db);
        std::env::set_var(ENV_SHIFT_LOG_ROOT, &runtime_root);

        let session = "01010101010101010101010101010101";
        let session_bytes = decode_session_hex32(session).unwrap();
        let shift_id = "01J0123456789ABCDEF1234";
        let attempt_id = "phase7_fsv_attempt";
        let source = repo.join("phase7_replay.py");
        std::fs::write(&source, "def answer():\n    return 4\n").unwrap();
        let after_sha = sha256_file(&source).unwrap();
        let before_text = "def answer():\n    return 3\n";
        let before_sha = format!("sha256:{}", sha256_bytes_plain(before_text.as_bytes()));

        seed_infer_db(&infer_db, session, shift_id, attempt_id).unwrap();
        let shift = crate::handlers::tools::reality_loop::shift_log::ShiftRecord {
            shift_id: shift_id.to_string(),
            timestamp_unix_ns: 1_772_000_000_000_000_000,
            tool_name: "harness_apply_line_window_edit".to_string(),
            tool_use_id: Some("toolu_phase7_fsv".to_string()),
            session_id: session.to_string(),
            subject: json!({
                "type": "file",
                "path": "phase7_replay.py",
                "attempt_id": attempt_id
            }),
            before: json!({"sha256": before_sha, "text": before_text}),
            after: json!({"sha256": after_sha, "source_of_truth": file_sot(&source)}),
            delta_summary: json!({"lines_added": 2, "lines_removed": 0}),
            verification: json!({
                "synthetic_expected_return": 4,
                "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment())
            }),
            harness_transition_path: None,
        };
        crate::handlers::tools::reality_loop::shift_log::append_shift(
            &runtime_root,
            session,
            &shift,
        )
        .unwrap();

        let before_state = readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        println!(
            "FSV before happy path: {}",
            serde_json::to_string_pretty(&before_state).unwrap()
        );
        let located = locate_shift(shift_id).await.unwrap().unwrap();
        let replay = replay_shift(&located).unwrap();
        let after_state = readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        println!(
            "FSV after happy path: {}",
            serde_json::to_string_pretty(&after_state).unwrap()
        );
        assert_eq!(after_state["prediction_count"], json!(1));
        assert_eq!(after_state["panel_t0_exists"], json!(true));
        assert_eq!(after_state["panel_t1_exists"], json!(true));
        assert_eq!(after_state["panel_t2_exists"], json!(true));
        assert_eq!(
            before_state["watermark_record"], after_state["watermark_record"],
            "operator replay must not advance subscriber watermark"
        );

        let audit = capture_audit(attempt_id, 0).unwrap();
        assert_eq!(audit["attempt_found"], json!(true));
        let status = subscriber_status().await.unwrap();
        assert_eq!(
            status["last_watermark_per_session"][session],
            json!(shift_id)
        );

        let invalid_before =
            readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        let invalid_shift_id_result = valid_shift_id("01j0123456789ABCDEF1234");
        let invalid_after =
            readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        println!(
            "FSV edge invalid shift id before={} after={}",
            invalid_before, invalid_after
        );
        assert!(!invalid_shift_id_result);
        assert_eq!(invalid_before, invalid_after);

        let not_found_before =
            readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        let not_found = locate_shift("01JFFFFFFFFFFFFFFFFFFFF").await.unwrap();
        let not_found_after =
            readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        println!(
            "FSV edge missing shift before={} after={}",
            not_found_before, not_found_after
        );
        assert!(not_found.is_none());
        assert_eq!(not_found_before, not_found_after);

        let page_before = readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        let page_err = capture_audit(attempt_id, 1).unwrap_err();
        let page_after = readback_state(&infer_db, &panel_db, session_bytes, attempt_id).unwrap();
        println!(
            "FSV edge page oob error={} before={} after={}",
            page_err, page_before, page_after
        );
        assert!(page_err.contains("MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE"));
        assert_eq!(page_before, page_after);

        let happy_path = json!({
            "source_of_truth": {
                "shift_log": file_sot(&runtime_root.join("cgreality-shift-log").join(format!("{session}.jsonl"))),
                "infer_db": file_sot(&infer_db),
                "panel_db": file_sot(&panel_db)
            },
            "before_state": before_state,
            "replay_response": replay,
            "after_state": after_state,
            "capture_audit": audit,
            "subscriber_status": status
        });
        let edge_invalid = json!({
            "scenario": "invalid_shift_id",
            "input": "01j0123456789ABCDEF1234",
            "valid": invalid_shift_id_result,
            "before_state": invalid_before,
            "after_state": invalid_after
        });
        let edge_not_found = json!({
            "scenario": "missing_shift",
            "input": "01JFFFFFFFFFFFFFFFFFFFF",
            "found": not_found.is_some(),
            "before_state": not_found_before,
            "after_state": not_found_after
        });
        let edge_page = json!({
            "scenario": "capture_audit_page_out_of_range",
            "error": page_err,
            "before_state": page_before,
            "after_state": page_after
        });
        let mut files = BTreeMap::new();
        for (name, value) in [
            ("happy_path.json", happy_path),
            ("edge_invalid_shift_id.json", edge_invalid),
            ("edge_missing_shift.json", edge_not_found),
            ("edge_page_out_of_range.json", edge_page),
        ] {
            let path = evidence_dir.join(name);
            write_json_0600(&path, &value).unwrap();
            files.insert(name.to_string(), sha256_file_plain(&path).unwrap());
        }
        let manifest = json!({
            "phase": "phase7-mcp-tools",
            "source_of_truth": {
                "infer_db": file_sot(&infer_db),
                "panel_db": file_sot(&panel_db),
                "shift_log_root": file_sot(&runtime_root.join("cgreality-shift-log"))
            },
            "evidence_files_sha256": files
        });
        write_json_0600(&evidence_dir.join("manifest.json"), &manifest).unwrap();
        println!(
            "FSV evidence manifest: {}",
            serde_json::to_string_pretty(&manifest).unwrap()
        );
    }

    fn seed_infer_db(
        infer_db: &Path,
        session: &str,
        shift_id: &str,
        attempt_id: &str,
    ) -> Result<(), String> {
        let db = open_infer_rocksdb(infer_db).map_err(|err| err.to_string())?;
        let calibration = CalibrationStore::new(db.clone(), 30).map_err(|err| err.to_string())?;
        let examples = (0..40)
            .map(|idx| CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
                actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
            })
            .collect::<Vec<_>>();
        let norms = vec![0.01; examples.len()];
        calibration
            .calibrate(
                &examples,
                &norms,
                0.10,
                30,
                0.30,
                [7; 32],
                BTreeMap::<EmbedderId, String>::new(),
            )
            .map_err(|err| err.to_string())?;
        let train_cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
            .ok_or_else(|| "missing train cert CF".to_string())?;
        let cert = TrainCertSummary {
            step: 1,
            delta_omega: 0.8,
            delta_xi: 0.8,
            witness_offset: 44,
            // #699: phase7-storage FSV cert simulates a trained cert path.
            predictor_parameter_update_count: 1,
        };
        db.put_cf(
            train_cf,
            b"cert:phase7-fsv:0001",
            bincode::serialize(&cert).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        let oracle_cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
            .ok_or_else(|| "missing oracle CF".to_string())?;
        let example = CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.95],
            actual_test_pass: vec![1.0],
        };
        db.put_cf(
            oracle_cf,
            format!("oracle:{attempt_id}:0001").as_bytes(),
            bincode::serialize(&example).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        let watermark_cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
            .ok_or_else(|| "missing watermark CF".to_string())?;
        let watermark = WatermarkRecord {
            session_id: session.to_string(),
            last_consumed_shift_id: shift_id.to_string(),
            last_consumed_byte_offset: 0,
            last_advanced_at_unix_seconds: 1_772_000_000,
            producer_tool_name: Some("phase7_fsv_seed".to_string()),
            source_log_path: None,
        };
        db.put_cf(
            watermark_cf,
            watermark_key(session).as_bytes(),
            serde_json::to_vec(&watermark).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn readback_state(
        infer_db: &Path,
        panel_db: &Path,
        session_id: [u8; 16],
        attempt_id: &str,
    ) -> Result<Value, String> {
        let db = open_infer_rocksdb(infer_db).map_err(|err| err.to_string())?;
        let store = RocksDbInferStore::new(db.clone());
        let predictions = MejepaStore::read_live_predictions(&store, session_id, 1000)
            .map_err(|err| err.to_string())?;
        let watermark = subscriber_state_for_provenance(db.as_ref(), session_id)
            .map_err(|err| err.to_string())?;
        drop(store);
        drop(db);
        let panel_store = PanelStore::open(panel_db).map_err(|err| err.to_string())?;
        let panel_t0_exists = panel_store
            .get_envelope(&PanelKey::new(attempt_id, TimeStep::T0).map_err(|err| err.to_string())?)
            .map_err(|err| err.to_string())?
            .is_some();
        let panel_t1_exists = panel_store
            .get_envelope(&PanelKey::new(attempt_id, TimeStep::T1).map_err(|err| err.to_string())?)
            .map_err(|err| err.to_string())?
            .is_some();
        let panel_t2_exists = panel_store
            .get_envelope(&PanelKey::new(attempt_id, TimeStep::T2).map_err(|err| err.to_string())?)
            .map_err(|err| err.to_string())?
            .is_some();
        Ok(json!({
            "prediction_count": predictions.len(),
            "prediction_ids": predictions.iter().map(|row| hex::encode(row.prediction_id)).collect::<Vec<_>>(),
            "watermark_record": watermark,
            "panel_t0_exists": panel_t0_exists,
            "panel_t1_exists": panel_t1_exists,
            "panel_t2_exists": panel_t2_exists
        }))
    }

    fn write_json_0600<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(value)?;
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(path)?
        };
        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(())
    }

    fn sha256_file_plain(path: &Path) -> std::io::Result<String> {
        let bytes = std::fs::read(path)?;
        Ok(sha256_bytes_plain(&bytes))
    }

    fn sha256_bytes_plain(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }
}
