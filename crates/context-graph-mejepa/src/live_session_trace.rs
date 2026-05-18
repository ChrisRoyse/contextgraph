use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::CF_MEJEPA_LIVE_SESSION_TRACES;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::types::{OracleOutcome, Verdict};

pub const LIVE_SESSION_TRACE_SCHEMA_VERSION: u32 = 1;
const MAX_TRACE_EVENTS: usize = 100_000;
const MAX_TRACE_ID_BYTES: usize = 512;
const MAX_TRACE_TEXT_BYTES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveSessionTraceStatus {
    Completed,
    ReadOnlySession,
    ContinuedAfterStructuredError,
    FailedClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveSessionPipelineStage {
    HookCapture,
    ShiftSubscriber,
    PanelMaterialization,
    Dda,
    Predictor,
    MarkdownInjection,
    AgentRead,
    Oracle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveSessionTraceErrorCode {
    OutOfRoot,
    ReadOnlySession,
    PredictionTimeout,
    PipelineStageError,
}

impl LiveSessionTraceErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OutOfRoot => "OUT_OF_ROOT",
            Self::ReadOnlySession => "READ_ONLY_SESSION",
            Self::PredictionTimeout => "PREDICTION_TIMEOUT",
            Self::PipelineStageError => "PIPELINE_STAGE_ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionStructuredError {
    pub error_id: String,
    pub code: LiveSessionTraceErrorCode,
    pub stage: LiveSessionPipelineStage,
    pub occurred_at_unix_ms: i64,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionStageEvent {
    pub stage_event_id: String,
    pub stage: LiveSessionPipelineStage,
    pub input_ref: String,
    pub output_ref: String,
    pub started_at_unix_ms: i64,
    pub finished_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionEditEvent {
    pub edit_id: String,
    pub edit_path: String,
    pub edit_time_unix_ms: i64,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub dropped_reason: Option<LiveSessionTraceErrorCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionShiftRow {
    pub shift_id: String,
    pub edit_id: String,
    pub edit_path: String,
    pub captured_at_unix_ms: i64,
    pub subscriber_row_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionPanelEvent {
    pub panel_id: String,
    pub shift_id: String,
    pub materialized_at_unix_ms: i64,
    pub source_panel_sha: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionPredictionEvent {
    pub prediction_id: [u8; 16],
    pub panel_id: String,
    pub predicted_at_unix_ms: i64,
    pub shift_to_prediction_ms: u64,
    pub verdict: Verdict,
    pub predicted_oracle_pass: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionInjectionEvent {
    pub injection_id: String,
    pub prediction_id: [u8; 16],
    pub injected_at_unix_ms: i64,
    pub markdown_sha256: String,
    pub history_jsonl_path: String,
    pub agent_read_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionContradictionEvent {
    pub contradiction_id: String,
    pub prediction_id: [u8; 16],
    pub predicted_verdict: Verdict,
    pub oracle_outcome: OracleOutcome,
    pub active_learning_task_id: String,
    pub active_learning_reason: String,
    pub queued_at_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionLatencySummary {
    pub sample_count: usize,
    pub p50_ms: u64,
    pub p99_ms: u64,
    pub max_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionTrace {
    pub schema_version: u32,
    pub session_id: String,
    pub session_key: [u8; 16],
    pub task_instance_id: String,
    pub project_root: String,
    pub pre_edit_sha: String,
    pub started_at_unix_ms: i64,
    pub finished_at_unix_ms: i64,
    pub status: LiveSessionTraceStatus,
    pub edit_events: Vec<LiveSessionEditEvent>,
    pub shift_rows: Vec<LiveSessionShiftRow>,
    pub panel_events: Vec<LiveSessionPanelEvent>,
    pub prediction_events: Vec<LiveSessionPredictionEvent>,
    pub injection_events: Vec<LiveSessionInjectionEvent>,
    pub stage_events: Vec<LiveSessionStageEvent>,
    pub structured_errors: Vec<LiveSessionStructuredError>,
    pub contradiction_events: Vec<LiveSessionContradictionEvent>,
    pub final_verdict: Verdict,
    pub oracle_outcome: OracleOutcome,
    pub latency_summary: LiveSessionLatencySummary,
    pub subscriber_backlog_max: u64,
}

impl LiveSessionTrace {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != LIVE_SESSION_TRACE_SCHEMA_VERSION {
            return invalid(
                "live_session_trace.schema_version",
                format!(
                    "expected {}, got {}",
                    LIVE_SESSION_TRACE_SCHEMA_VERSION, self.schema_version
                ),
            );
        }
        validate_id("live_session_trace.session_id", &self.session_id)?;
        validate_nonzero_16("live_session_trace.session_key", self.session_key)?;
        validate_id(
            "live_session_trace.task_instance_id",
            &self.task_instance_id,
        )?;
        validate_text("live_session_trace.project_root", &self.project_root)?;
        validate_text("live_session_trace.pre_edit_sha", &self.pre_edit_sha)?;
        validate_positive_time(
            "live_session_trace.started_at_unix_ms",
            self.started_at_unix_ms,
        )?;
        validate_positive_time(
            "live_session_trace.finished_at_unix_ms",
            self.finished_at_unix_ms,
        )?;
        if self.finished_at_unix_ms < self.started_at_unix_ms {
            return invalid(
                "live_session_trace.finished_at_unix_ms",
                "finished_at_unix_ms must be >= started_at_unix_ms",
            );
        }
        validate_event_count("edit_events", self.edit_events.len())?;
        validate_event_count("shift_rows", self.shift_rows.len())?;
        validate_event_count("panel_events", self.panel_events.len())?;
        validate_event_count("prediction_events", self.prediction_events.len())?;
        validate_event_count("injection_events", self.injection_events.len())?;
        validate_event_count("stage_events", self.stage_events.len())?;
        validate_event_count("structured_errors", self.structured_errors.len())?;
        validate_event_count("contradiction_events", self.contradiction_events.len())?;

        let edit_ids = validate_edit_events(&self.edit_events)?;
        let shift_ids = validate_shift_rows(&self.shift_rows, &edit_ids)?;
        let panel_ids = validate_panel_events(&self.panel_events, &shift_ids)?;
        let prediction_ids = validate_prediction_events(&self.prediction_events, &panel_ids)?;
        validate_injection_events(&self.injection_events, &prediction_ids)?;
        validate_stage_events(&self.stage_events)?;
        validate_structured_errors(&self.structured_errors)?;
        validate_contradictions(&self.contradiction_events, &prediction_ids)?;
        validate_latency_summary(&self.latency_summary, &self.prediction_events)?;

        match self.status {
            LiveSessionTraceStatus::Completed => {
                if self.prediction_events.is_empty() || self.injection_events.is_empty() {
                    return invalid(
                        "live_session_trace.status",
                        "completed trace requires at least one prediction and injection",
                    );
                }
            }
            LiveSessionTraceStatus::ReadOnlySession => {
                if !self.edit_events.is_empty()
                    || !self.shift_rows.is_empty()
                    || !self.panel_events.is_empty()
                    || !self.prediction_events.is_empty()
                    || !self.injection_events.is_empty()
                {
                    return invalid(
                        "live_session_trace.status",
                        "read-only trace must not contain edit, shift, panel, prediction, or injection events",
                    );
                }
                if !self
                    .structured_errors
                    .iter()
                    .any(|err| err.code == LiveSessionTraceErrorCode::ReadOnlySession)
                {
                    return invalid(
                        "live_session_trace.structured_errors",
                        "read-only trace must record READ_ONLY_SESSION",
                    );
                }
            }
            LiveSessionTraceStatus::ContinuedAfterStructuredError
            | LiveSessionTraceStatus::FailedClosed => {
                if self.structured_errors.is_empty() {
                    return invalid(
                        "live_session_trace.structured_errors",
                        "structured-error trace must include at least one error row",
                    );
                }
            }
        }

        if self
            .structured_errors
            .iter()
            .any(|err| err.code == LiveSessionTraceErrorCode::PredictionTimeout)
            && self.final_verdict != Verdict::Abstain
        {
            return invalid(
                "live_session_trace.final_verdict",
                "PREDICTION_TIMEOUT traces must keep final verdict at Abstain",
            );
        }
        Ok(())
    }
}

pub fn latency_summary_from_predictions(
    predictions: &[LiveSessionPredictionEvent],
) -> LiveSessionLatencySummary {
    let mut samples = predictions
        .iter()
        .map(|prediction| prediction.shift_to_prediction_ms)
        .collect::<Vec<_>>();
    if samples.is_empty() {
        return LiveSessionLatencySummary {
            sample_count: 0,
            p50_ms: 0,
            p99_ms: 0,
            max_ms: 0,
        };
    }
    samples.sort_unstable();
    let p50 = samples[samples.len() / 2];
    let p99_idx = (((samples.len() - 1) as f64) * 0.99).ceil() as usize;
    LiveSessionLatencySummary {
        sample_count: samples.len(),
        p50_ms: p50,
        p99_ms: samples[p99_idx],
        max_ms: *samples.last().unwrap_or(&0),
    }
}

pub fn persist_live_session_trace(
    db: &DB,
    trace: &LiveSessionTrace,
) -> Result<(), MejepaInferError> {
    trace.validate()?;
    let cf = cf(db, CF_MEJEPA_LIVE_SESSION_TRACES)?;
    let key = live_session_trace_key(&trace.session_id);
    let value = bincode::serialize(trace)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &value, &opts)?;
    let readback = db
        .get_cf(cf, &key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "live_session_trace.readback".to_string(),
            detail: "trace row missing after put_cf".to_string(),
        })?;
    if readback != value {
        return invalid(
            "live_session_trace.readback",
            "readback bytes differ from written trace payload",
        );
    }
    let decoded: LiveSessionTrace = bincode::deserialize(&readback)?;
    decoded.validate()?;
    if decoded != *trace {
        return invalid(
            "live_session_trace.readback",
            "decoded readback trace differs from input trace",
        );
    }
    Ok(())
}

pub fn load_live_session_trace(
    db: &DB,
    session_id: &str,
) -> Result<Option<LiveSessionTrace>, MejepaInferError> {
    validate_id("session_id", session_id)?;
    let cf = cf(db, CF_MEJEPA_LIVE_SESSION_TRACES)?;
    let Some(bytes) = db.get_cf(cf, live_session_trace_key(session_id))? else {
        return Ok(None);
    };
    let trace: LiveSessionTrace = bincode::deserialize(&bytes)?;
    trace.validate()?;
    Ok(Some(trace))
}

pub fn scan_live_session_traces(db: &DB) -> Result<Vec<LiveSessionTrace>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_LIVE_SESSION_TRACES)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let trace: LiveSessionTrace = bincode::deserialize(&value)?;
        trace.validate()?;
        rows.push(trace);
    }
    Ok(rows)
}

pub fn count_live_session_traces(db: &DB) -> Result<u64, MejepaInferError> {
    Ok(scan_live_session_traces(db)?.len() as u64)
}

pub fn write_live_session_trace_jsonl(
    runtime_root: impl AsRef<Path>,
    trace: &LiveSessionTrace,
) -> Result<PathBuf, MejepaInferError> {
    trace.validate()?;
    let runtime_root = runtime_root.as_ref();
    fs::create_dir_all(runtime_root)
        .map_err(|source| MejepaInferError::io("create_dir_all", runtime_root, source))?;
    let file_name = safe_session_file_name(&trace.session_id)?;
    let path = runtime_root.join(format!("{file_name}.jsonl"));
    let mut lines = Vec::new();
    lines.push(json!({
        "record_kind": "live_session_trace_header",
        "schema_version": trace.schema_version,
        "session_id": trace.session_id,
        "task_instance_id": trace.task_instance_id,
        "status": trace.status,
        "pre_edit_sha": trace.pre_edit_sha,
    }));
    for event in &trace.edit_events {
        lines.push(
            json!({"record_kind": "edit_event", "session_id": trace.session_id, "event": event}),
        );
    }
    for row in &trace.shift_rows {
        lines.push(json!({"record_kind": "shift_subscriber_row", "session_id": trace.session_id, "row": row}));
    }
    for event in &trace.panel_events {
        lines.push(json!({"record_kind": "panel_materialization", "session_id": trace.session_id, "event": event}));
    }
    for event in &trace.prediction_events {
        lines.push(json!({"record_kind": "prediction_event", "session_id": trace.session_id, "event": event}));
    }
    for event in &trace.injection_events {
        lines.push(json!({"record_kind": "injection_event", "session_id": trace.session_id, "event": event}));
    }
    for event in &trace.stage_events {
        lines.push(json!({"record_kind": "pipeline_stage_event", "session_id": trace.session_id, "event": event}));
    }
    for error in &trace.structured_errors {
        lines.push(json!({"record_kind": "structured_error", "session_id": trace.session_id, "error": error}));
    }
    for event in &trace.contradiction_events {
        lines.push(json!({"record_kind": "contradiction_event", "session_id": trace.session_id, "event": event}));
    }
    lines.push(json!({
        "record_kind": "live_session_trace_summary",
        "session_id": trace.session_id,
        "final_verdict": trace.final_verdict,
        "oracle_outcome": trace.oracle_outcome,
        "latency_summary": trace.latency_summary,
        "subscriber_backlog_max": trace.subscriber_backlog_max,
    }));

    let mut payload = String::new();
    for line in &lines {
        payload.push_str(&serde_json::to_string(line)?);
        payload.push('\n');
    }
    let tmp = path.with_extension("jsonl.tmp");
    fs::write(&tmp, payload.as_bytes())
        .map_err(|source| MejepaInferError::io("write", &tmp, source))?;
    fs::rename(&tmp, &path).map_err(|source| MejepaInferError::io("rename", &path, source))?;
    let readback = fs::read(&path).map_err(|source| MejepaInferError::io("read", &path, source))?;
    if readback != payload.as_bytes() {
        return invalid(
            "live_session_trace.jsonl_readback",
            format!(
                "{} readback bytes differ from written payload",
                path.display()
            ),
        );
    }
    Ok(path)
}

pub fn live_session_trace_key(session_id: &str) -> Vec<u8> {
    session_id.as_bytes().to_vec()
}

fn validate_edit_events(
    events: &[LiveSessionEditEvent],
) -> Result<BTreeSet<String>, MejepaInferError> {
    let mut ids = BTreeSet::new();
    for event in events {
        validate_id("live_session_trace.edit.edit_id", &event.edit_id)?;
        validate_text("live_session_trace.edit.edit_path", &event.edit_path)?;
        validate_positive_time(
            "live_session_trace.edit.edit_time_unix_ms",
            event.edit_time_unix_ms,
        )?;
        if !ids.insert(event.edit_id.clone()) {
            return invalid(
                "live_session_trace.edit.edit_id",
                format!("duplicate edit id {}", event.edit_id),
            );
        }
        if event.dropped_reason.is_none()
            && (event
                .before_sha256
                .as_deref()
                .unwrap_or_default()
                .is_empty()
                || event.after_sha256.as_deref().unwrap_or_default().is_empty())
        {
            return invalid(
                "live_session_trace.edit.sha256",
                "non-dropped edits require before_sha256 and after_sha256",
            );
        }
        if let Some(reason) = event.dropped_reason {
            if reason != LiveSessionTraceErrorCode::OutOfRoot {
                return invalid(
                    "live_session_trace.edit.dropped_reason",
                    "only OUT_OF_ROOT is valid as an edit dropped_reason",
                );
            }
        }
    }
    Ok(ids)
}

fn validate_shift_rows(
    rows: &[LiveSessionShiftRow],
    edit_ids: &BTreeSet<String>,
) -> Result<BTreeSet<String>, MejepaInferError> {
    let mut ids = BTreeSet::new();
    for row in rows {
        validate_id("live_session_trace.shift.shift_id", &row.shift_id)?;
        validate_id("live_session_trace.shift.edit_id", &row.edit_id)?;
        validate_text("live_session_trace.shift.edit_path", &row.edit_path)?;
        validate_positive_time(
            "live_session_trace.shift.captured_at_unix_ms",
            row.captured_at_unix_ms,
        )?;
        if !edit_ids.contains(&row.edit_id) {
            return invalid(
                "live_session_trace.shift.edit_id",
                format!("shift row references unknown edit_id {}", row.edit_id),
            );
        }
        if !ids.insert(row.shift_id.clone()) {
            return invalid(
                "live_session_trace.shift.shift_id",
                format!("duplicate shift id {}", row.shift_id),
            );
        }
    }
    Ok(ids)
}

fn validate_panel_events(
    events: &[LiveSessionPanelEvent],
    shift_ids: &BTreeSet<String>,
) -> Result<BTreeSet<String>, MejepaInferError> {
    let mut ids = BTreeSet::new();
    for event in events {
        validate_id("live_session_trace.panel.panel_id", &event.panel_id)?;
        validate_id("live_session_trace.panel.shift_id", &event.shift_id)?;
        validate_positive_time(
            "live_session_trace.panel.materialized_at_unix_ms",
            event.materialized_at_unix_ms,
        )?;
        if !shift_ids.contains(&event.shift_id) {
            return invalid(
                "live_session_trace.panel.shift_id",
                format!("panel references unknown shift_id {}", event.shift_id),
            );
        }
        if event.source_panel_sha.iter().all(|byte| *byte == 0) {
            return invalid(
                "live_session_trace.panel.source_panel_sha",
                "source_panel_sha must be non-zero",
            );
        }
        if !ids.insert(event.panel_id.clone()) {
            return invalid(
                "live_session_trace.panel.panel_id",
                format!("duplicate panel id {}", event.panel_id),
            );
        }
    }
    Ok(ids)
}

fn validate_prediction_events(
    events: &[LiveSessionPredictionEvent],
    panel_ids: &BTreeSet<String>,
) -> Result<BTreeSet<[u8; 16]>, MejepaInferError> {
    let mut ids = BTreeSet::new();
    for event in events {
        validate_nonzero_16(
            "live_session_trace.prediction.prediction_id",
            event.prediction_id,
        )?;
        validate_id("live_session_trace.prediction.panel_id", &event.panel_id)?;
        validate_positive_time(
            "live_session_trace.prediction.predicted_at_unix_ms",
            event.predicted_at_unix_ms,
        )?;
        validate_probability(
            "live_session_trace.prediction.predicted_oracle_pass",
            event.predicted_oracle_pass,
        )?;
        if !panel_ids.contains(&event.panel_id) {
            return invalid(
                "live_session_trace.prediction.panel_id",
                format!("prediction references unknown panel_id {}", event.panel_id),
            );
        }
        if !ids.insert(event.prediction_id) {
            return invalid(
                "live_session_trace.prediction.prediction_id",
                format!(
                    "duplicate prediction id {}",
                    hex::encode(event.prediction_id)
                ),
            );
        }
    }
    Ok(ids)
}

fn validate_injection_events(
    events: &[LiveSessionInjectionEvent],
    prediction_ids: &BTreeSet<[u8; 16]>,
) -> Result<(), MejepaInferError> {
    let mut ids = BTreeSet::new();
    for event in events {
        validate_id(
            "live_session_trace.injection.injection_id",
            &event.injection_id,
        )?;
        validate_nonzero_16(
            "live_session_trace.injection.prediction_id",
            event.prediction_id,
        )?;
        validate_positive_time(
            "live_session_trace.injection.injected_at_unix_ms",
            event.injected_at_unix_ms,
        )?;
        validate_text(
            "live_session_trace.injection.markdown_sha256",
            &event.markdown_sha256,
        )?;
        validate_text(
            "live_session_trace.injection.history_jsonl_path",
            &event.history_jsonl_path,
        )?;
        validate_positive_time(
            "live_session_trace.injection.agent_read_at_unix_ms",
            event.agent_read_at_unix_ms,
        )?;
        if event.agent_read_at_unix_ms < event.injected_at_unix_ms {
            return invalid(
                "live_session_trace.injection.agent_read_at_unix_ms",
                "agent_read_at_unix_ms must be >= injected_at_unix_ms",
            );
        }
        if !prediction_ids.contains(&event.prediction_id) {
            return invalid(
                "live_session_trace.injection.prediction_id",
                format!(
                    "injection references unknown prediction_id {}",
                    hex::encode(event.prediction_id)
                ),
            );
        }
        if !ids.insert(event.injection_id.clone()) {
            return invalid(
                "live_session_trace.injection.injection_id",
                format!("duplicate injection id {}", event.injection_id),
            );
        }
    }
    Ok(())
}

fn validate_stage_events(events: &[LiveSessionStageEvent]) -> Result<(), MejepaInferError> {
    let mut ids = BTreeSet::new();
    for event in events {
        validate_id(
            "live_session_trace.stage.stage_event_id",
            &event.stage_event_id,
        )?;
        validate_text("live_session_trace.stage.input_ref", &event.input_ref)?;
        validate_text("live_session_trace.stage.output_ref", &event.output_ref)?;
        validate_positive_time(
            "live_session_trace.stage.started_at_unix_ms",
            event.started_at_unix_ms,
        )?;
        validate_positive_time(
            "live_session_trace.stage.finished_at_unix_ms",
            event.finished_at_unix_ms,
        )?;
        if event.finished_at_unix_ms < event.started_at_unix_ms {
            return invalid(
                "live_session_trace.stage.finished_at_unix_ms",
                "stage finished_at_unix_ms must be >= started_at_unix_ms",
            );
        }
        if !ids.insert(event.stage_event_id.clone()) {
            return invalid(
                "live_session_trace.stage.stage_event_id",
                format!("duplicate stage event id {}", event.stage_event_id),
            );
        }
    }
    Ok(())
}

fn validate_structured_errors(
    errors: &[LiveSessionStructuredError],
) -> Result<(), MejepaInferError> {
    let mut ids = BTreeSet::new();
    for error in errors {
        validate_id("live_session_trace.error.error_id", &error.error_id)?;
        validate_positive_time(
            "live_session_trace.error.occurred_at_unix_ms",
            error.occurred_at_unix_ms,
        )?;
        validate_text("live_session_trace.error.detail", &error.detail)?;
        if !ids.insert(error.error_id.clone()) {
            return invalid(
                "live_session_trace.error.error_id",
                format!("duplicate structured error id {}", error.error_id),
            );
        }
    }
    Ok(())
}

fn validate_contradictions(
    events: &[LiveSessionContradictionEvent],
    prediction_ids: &BTreeSet<[u8; 16]>,
) -> Result<(), MejepaInferError> {
    let mut ids = BTreeSet::new();
    for event in events {
        validate_id(
            "live_session_trace.contradiction.contradiction_id",
            &event.contradiction_id,
        )?;
        validate_nonzero_16(
            "live_session_trace.contradiction.prediction_id",
            event.prediction_id,
        )?;
        validate_id(
            "live_session_trace.contradiction.active_learning_task_id",
            &event.active_learning_task_id,
        )?;
        validate_text(
            "live_session_trace.contradiction.active_learning_reason",
            &event.active_learning_reason,
        )?;
        validate_positive_time(
            "live_session_trace.contradiction.queued_at_unix_ms",
            event.queued_at_unix_ms,
        )?;
        if !prediction_ids.contains(&event.prediction_id) {
            return invalid(
                "live_session_trace.contradiction.prediction_id",
                format!(
                    "contradiction references unknown prediction_id {}",
                    hex::encode(event.prediction_id)
                ),
            );
        }
        if event.predicted_verdict != Verdict::Fail || event.oracle_outcome != OracleOutcome::Pass {
            return invalid(
                "live_session_trace.contradiction",
                "ContradictionEvent requires predictor Fail and oracle Pass",
            );
        }
        if !ids.insert(event.contradiction_id.clone()) {
            return invalid(
                "live_session_trace.contradiction.contradiction_id",
                format!("duplicate contradiction id {}", event.contradiction_id),
            );
        }
    }
    Ok(())
}

fn validate_latency_summary(
    summary: &LiveSessionLatencySummary,
    predictions: &[LiveSessionPredictionEvent],
) -> Result<(), MejepaInferError> {
    let expected = latency_summary_from_predictions(predictions);
    if *summary != expected {
        return invalid(
            "live_session_trace.latency_summary",
            "latency summary does not match prediction event latencies",
        );
    }
    Ok(())
}

fn validate_event_count(field: &str, count: usize) -> Result<(), MejepaInferError> {
    if count > MAX_TRACE_EVENTS {
        return invalid(
            format!("live_session_trace.{field}"),
            format!("event count {count} exceeds max {MAX_TRACE_EVENTS}"),
        );
    }
    Ok(())
}

fn validate_nonzero_16(field: &str, value: [u8; 16]) -> Result<(), MejepaInferError> {
    if value.iter().all(|byte| *byte == 0) {
        return invalid(field, "16-byte id must be non-zero");
    }
    Ok(())
}

fn validate_positive_time(field: &str, value: i64) -> Result<(), MejepaInferError> {
    if value <= 0 {
        return invalid(field, format!("timestamp must be positive, got {value}"));
    }
    Ok(())
}

fn validate_probability(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(field, format!("probability must be in [0,1], got {value}"));
    }
    Ok(())
}

fn validate_id(field: &str, value: &str) -> Result<(), MejepaInferError> {
    validate_text(field, value)?;
    if value.contains('/') || value.contains('\\') {
        return invalid(field, "id must not contain path separators");
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "value must be non-empty");
    }
    if value.len() > MAX_TRACE_TEXT_BYTES {
        return invalid(
            field,
            format!("value exceeds max {MAX_TRACE_TEXT_BYTES} bytes"),
        );
    }
    if value.chars().any(char::is_control) {
        return invalid(field, "value must not contain control characters");
    }
    if field.ends_with("id") && value.len() > MAX_TRACE_ID_BYTES {
        return invalid(field, format!("id exceeds max {MAX_TRACE_ID_BYTES} bytes"));
    }
    Ok(())
}

fn safe_session_file_name(session_id: &str) -> Result<String, MejepaInferError> {
    validate_id("session_id", session_id)?;
    if !session_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return invalid(
            "session_id",
            "session id must be ASCII alphanumeric plus '.', '_', or '-' for JSONL filenames",
        );
    }
    Ok(session_id.to_string())
}

fn invalid<T>(field: impl Into<String>, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    })
}
