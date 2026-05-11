// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

//! Durable readers for Phase 7 ME-JEPA MCP tools.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use context_graph_mejepa::{RealityPrediction, RocksDbInferStore, TrainCertSummary};
use context_graph_mejepa_instruments::materialize::TimeStep;
use context_graph_mejepa_instruments::panel_store::{
    PanelKey, PanelStore, CF_MEJEPA_PANEL_META,
};
use context_graph_mejepa_instruments::InstrumentSlot;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::handlers::tools::reality_loop::helpers::{file_sot, read_active_runtime_root};

const ENV_SUBSCRIBER_DB: &str = "CONTEXTGRAPH_MEJEPA_SUBSCRIBER_DB";
const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";
const ENV_PANEL_DB: &str = "CONTEXTGRAPH_MEJEPA_PANEL_DB";
const ENV_SHIFT_LOG_ROOT: &str = "CONTEXTGRAPH_MEJEPA_SHIFT_LOG_ROOT";
const AUDIT_CHUNK_SIZE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WatermarkRecord {
    session_id: String,
    last_consumed_shift_id: String,
    last_consumed_byte_offset: u64,
    updated_at_unix_ms: i64,
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

pub(crate) async fn subscriber_state_for_provenance() -> Option<Value> {
    let db_path = std::env::var(ENV_SUBSCRIBER_DB).ok()?;
    match read_watermarks(Path::new(&db_path)) {
        Ok(watermarks) => {
            let latest = watermarks
                .values()
                .max_by_key(|record| record.updated_at_unix_ms)
                .cloned();
            Some(json!({
                "last_consumed_shift_id": latest.as_ref().map(|record| record.last_consumed_shift_id.clone()),
                "subscriber_lag_at_call_time": Value::Null,
                "subscriber_db_path": db_path
            }))
        }
        Err(err) => Some(json!({
            "error_code": "MEJEPA_SUBSCRIBER_STATUS_READ_FAILED",
            "message": err,
            "subscriber_db_path": db_path
        })),
    }
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

pub(crate) async fn subscriber_status() -> Result<Value, String> {
    let db_path = required_env_path(ENV_SUBSCRIBER_DB)?;
    let watermarks = read_watermarks(&db_path)?;
    let last_watermark_per_session = watermarks
        .iter()
        .map(|(session, record)| (session.clone(), record.last_consumed_shift_id.clone()))
        .collect::<BTreeMap<_, _>>();
    Ok(json!({
        "last_watermark_per_session": last_watermark_per_session,
        "watermark_records": watermarks,
        "lag": Value::Null,
        "lag_alert_active": false,
        "error_rate_per_1000": Value::Null,
        "instrument_cache_hit_rate": Value::Null,
        "instrument_cache_capacity": Value::Null,
        "instrument_cache_entries": Value::Null,
        "rss_bytes": rss_bytes()?,
        "task_alive_since": Value::Null,
        "dropped_l_step_below_threshold_count": Value::Null,
        "last_panic": Value::Null,
        "subscriber_running": false,
        "source_of_truth": file_sot(&db_path),
        "status_scope": "persisted_watermarks_only_no_injected_subscriber_metrics"
    }))
}

pub(crate) fn capture_audit(attempt_id: &str, page: u32) -> Result<Value, String> {
    let panel_db = required_env_path(ENV_PANEL_DB)?;
    let infer_db = required_env_path(ENV_INFER_DB)?;
    let panel_store = PanelStore::open(&panel_db).map_err(|err| err.to_string())?;
    let panels = read_panel_summaries(&panel_store, attempt_id)?;
    let infer =
        context_graph_mejepa::open_infer_rocksdb(&infer_db).map_err(|err| err.to_string())?;
    let predictions =
        read_predictions_for_attempt(&RocksDbInferStore::new(infer.clone()), attempt_id)?;
    let train_certs = read_train_certs(infer.as_ref())?;
    let oracle_examples = read_oracle_examples(infer.as_ref())?;

    if panels.is_empty() && predictions.is_empty() {
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
        .collect::<Vec<_>>();
    let bundle = json!({
        "attempt_id": attempt_id,
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

fn read_watermarks(db_path: &Path) -> Result<BTreeMap<String, WatermarkRecord>, String> {
    let db = open_single_cf_db(db_path, context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)?;
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
        if record.session_id.trim().is_empty() {
            return Err(format!("watermark value for key {key:?} has empty session_id"));
        }
        out.insert(record.session_id.clone(), record);
    }
    Ok(out)
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
        let (_key, value) = item.map_err(|err| format!("live prediction iterator failed: {err}"))?;
        let prediction: RealityPrediction = bincode::deserialize(&value)
            .map_err(|err| format!("live prediction payload is invalid bincode: {err}"))?;
        prediction.validate().map_err(|err| err.to_string())?;
        if prediction.task_id.0 == attempt_id {
            out.push(prediction);
        }
    }
    out.sort_by(|a, b| b.created_at_unix_ms.cmp(&a.created_at_unix_ms));
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

fn read_oracle_examples(db: &DB) -> Result<Vec<Value>, String> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
        .ok_or_else(|| "missing CF_MEJEPA_ORACLE_VERDICTS handle".to_string())?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item.map_err(|err| format!("oracle verdict iterator failed: {err}"))?;
        let example: context_graph_mejepa::CalibrationExample =
            bincode::deserialize(&value)
                .map_err(|err| format!("oracle verdict payload is invalid bincode: {err}"))?;
        out.push(json!({
            "key_hex": hex::encode(key),
            "example": example
        }));
        if out.len() == 100 {
            break;
        }
    }
    Ok(out)
}

fn nearest_panel_attempts(panel_store: &PanelStore, attempt_id: &str) -> Result<Vec<String>, String> {
    let mut candidates = panel_store
        .scan_cf_json(CF_MEJEPA_PANEL_META)
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter_map(|(key, _)| key.split('/').next().map(ToOwned::to_owned))
        .filter(|candidate| candidate != attempt_id)
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates.truncate(5);
    Ok(candidates)
}

fn paginate_bundle(bundle: Value, page: u32) -> Result<Value, String> {
    let encoded = serde_json::to_vec(&bundle).map_err(|err| err.to_string())?;
    if encoded.len() <= AUDIT_CHUNK_SIZE_BYTES {
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
        return Err(format!("page {page} is out of range for {total_pages} pages"));
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
    std::env::var(name)
        .map(PathBuf::from)
        .map_err(|_| format!("{name} must be set; refusing to guess a Phase 7 source-of-truth path"))
}

fn open_single_cf_db(path: &Path, cf_name: &'static str) -> Result<DB, String> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let mut cf_names = context_graph_mejepa_cf::all_hygiene_referenced_cfs();
    if !cf_names.contains(&cf_name) {
        cf_names.push(cf_name);
    }
    DB::open_cf_descriptors(
        &opts,
        path,
        cf_names
            .into_iter()
            .map(|name| ColumnFamilyDescriptor::new(name, Options::default()))
            .collect::<Vec<_>>(),
    )
    .map_err(|err| format!("failed to open RocksDB {} with {cf_name}: {err}", path.display()))
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
            .map_err(|err| format!("failed to parse resident pages from /proc/self/statm: {err}"))?;
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

