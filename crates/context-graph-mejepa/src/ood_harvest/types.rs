use crate::eval::{EvalError, EvalErrorCode};
use crate::types::{ChunkId, OracleOutcome, PanelId, PredictionId, TaskId, Verdict};
use serde::{Deserialize, Serialize};

pub const OOD_HARVEST_SCHEMA_VERSION: u32 = 1;
pub const OOD_CALIBRATION_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_OOD_HARVEST_THRESHOLD: f32 = 0.85;
pub const OOD_HARVEST_ACTIVE_LEARNING_WEIGHT: f32 = 4.0;
pub const OOD_HARVEST_DOWNWEIGHTED_WEIGHT: f32 = 1.0;
pub const DEFAULT_OOD_HARVEST_RETENTION_ACTIVE_MS: i64 = 90 * 24 * 60 * 60 * 1000;
pub const OOD_HARVEST_ORPHAN: &str = "OOD_HARVEST_ORPHAN";
pub const OOD_HARVEST_DEGENERATE_PANEL: &str = "OOD_HARVEST_DEGENERATE_PANEL";
pub const OOD_GATE_OVER_FLAGGING: &str = "OOD_GATE_OVER_FLAGGING";
pub const OOD_HARVEST_EMPTY: &str = "OOD_HARVEST_EMPTY";
pub const OOD_CELL_AUC_REGRESSION: &str = "OOD_CELL_AUC_REGRESSION";
pub const OOD_CELL_INSUFFICIENT_SUPPORT: &str = "OOD_CELL_INSUFFICIENT_SUPPORT";
pub const OOD_RECALL_BELOW_TARGET: &str = "OOD_RECALL_BELOW_TARGET";
pub const OOD_AUC_BELOW_TARGET: &str = "OOD_AUC_BELOW_TARGET";
pub const OOD_CALIBRATOR_MISSING: &str = "OOD_CALIBRATOR_MISSING";
pub const OOD_SCORE_ABOVE_THRESHOLD: &str = "OOD_SCORE_ABOVE_THRESHOLD";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodHarvestConfig {
    pub threshold: f32,
    pub retention_active_ms: i64,
    pub queue_capacity: usize,
}

impl Default for OodHarvestConfig {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_OOD_HARVEST_THRESHOLD,
            retention_active_ms: DEFAULT_OOD_HARVEST_RETENTION_ACTIVE_MS,
            queue_capacity: 4096,
        }
    }
}

impl OodHarvestConfig {
    pub fn validate(&self) -> Result<(), EvalError> {
        validate_probability("ood_harvest.threshold", self.threshold)?;
        if self.retention_active_ms <= 0 {
            return Err(invalid("ood_harvest.retention_active_ms must be positive"));
        }
        if self.queue_capacity == 0 {
            return Err(invalid("ood_harvest.queue_capacity must be positive"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OodHarvestStatus {
    Active,
    TieredDown,
    DownweightedInDistribution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodHarvestRow {
    pub schema_version: u32,
    pub prediction_id: PredictionId,
    pub task_id: TaskId,
    pub session_id: [u8; 16],
    pub panel_id: PanelId,
    pub calibration_cell: String,
    pub affected_chunk_ids: Vec<ChunkId>,
    pub agent_prose: String,
    pub verdict: Verdict,
    pub ood_score: f32,
    pub created_at_unix_ms: i64,
    pub harvested_at_unix_ms: i64,
    pub oracle_outcome: Option<OracleOutcome>,
    pub status: OodHarvestStatus,
    pub priority_weight: f32,
    pub source_live_prediction_cf: String,
}

impl OodHarvestRow {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.schema_version != OOD_HARVEST_SCHEMA_VERSION {
            return Err(invalid(format!(
                "OOD harvest schema_version must be {OOD_HARVEST_SCHEMA_VERSION}"
            )));
        }
        validate_prediction_id(self.prediction_id, "ood_harvest.prediction_id")?;
        self.task_id
            .validate("ood_harvest.task_id")
            .map_err(EvalError::from)?;
        if self.session_id.iter().all(|byte| *byte == 0) {
            return Err(invalid("ood_harvest.session_id must be non-zero"));
        }
        validate_panel_id(self.panel_id)?;
        validate_calibration_cell(&self.calibration_cell)?;
        if self.affected_chunk_ids.is_empty() {
            return Err(invalid("ood_harvest.affected_chunk_ids must be non-empty"));
        }
        for chunk in &self.affected_chunk_ids {
            chunk
                .validate("ood_harvest.affected_chunk_ids")
                .map_err(EvalError::from)?;
        }
        if self.agent_prose.trim().is_empty() {
            return Err(invalid("ood_harvest.agent_prose must be non-empty"));
        }
        if self.agent_prose.len() > 65_536 {
            return Err(invalid("ood_harvest.agent_prose exceeds 65536 bytes"));
        }
        validate_probability("ood_harvest.ood_score", self.ood_score)?;
        if self.created_at_unix_ms <= 0 || self.harvested_at_unix_ms <= 0 {
            return Err(invalid(
                "ood_harvest created_at_unix_ms and harvested_at_unix_ms must be positive",
            ));
        }
        if !self.priority_weight.is_finite() || self.priority_weight <= 0.0 {
            return Err(invalid(
                "ood_harvest.priority_weight must be finite and positive",
            ));
        }
        if self.source_live_prediction_cf != context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS {
            return Err(invalid(
                "ood_harvest.source_live_prediction_cf must point at CF_MEJEPA_LIVE_PREDICTIONS",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodHarvestQuarantineRow {
    pub schema_version: u32,
    pub prediction_id: PredictionId,
    pub task_id: TaskId,
    pub panel_id: PanelId,
    pub code: String,
    pub detail: String,
    pub observed_at_unix_ms: i64,
    pub source_live_prediction_cf: String,
}

impl OodHarvestQuarantineRow {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.schema_version != OOD_HARVEST_SCHEMA_VERSION {
            return Err(invalid(format!(
                "OOD quarantine schema_version must be {OOD_HARVEST_SCHEMA_VERSION}"
            )));
        }
        validate_prediction_id(self.prediction_id, "ood_harvest_quarantine.prediction_id")?;
        self.task_id
            .validate("ood_harvest_quarantine.task_id")
            .map_err(EvalError::from)?;
        if self.panel_id.0.iter().all(|byte| *byte == 0) && self.code != OOD_HARVEST_ORPHAN {
            return Err(invalid(
                "OOD quarantine zero panel_id is only valid for OOD_HARVEST_ORPHAN",
            ));
        }
        if self.code != OOD_HARVEST_ORPHAN && self.code != OOD_HARVEST_DEGENERATE_PANEL {
            return Err(invalid(format!(
                "unknown OOD quarantine code {}",
                self.code
            )));
        }
        if self.detail.trim().is_empty() {
            return Err(invalid("OOD quarantine detail must be non-empty"));
        }
        if self.observed_at_unix_ms <= 0 {
            return Err(invalid(
                "OOD quarantine observed_at_unix_ms must be positive",
            ));
        }
        if self.source_live_prediction_cf != context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS {
            return Err(invalid(
                "OOD quarantine source_live_prediction_cf must point at CF_MEJEPA_LIVE_PREDICTIONS",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SyntheticOodCalibrationRow {
    pub row_id: String,
    pub calibration_cell: String,
    pub ood_score: f32,
    pub predicted_ood: bool,
    pub actual_ood: bool,
}

impl SyntheticOodCalibrationRow {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.row_id.trim().is_empty() {
            return Err(invalid(
                "synthetic OOD calibration row_id must be non-empty",
            ));
        }
        validate_calibration_cell(&self.calibration_cell)?;
        validate_probability("synthetic_ood_calibration.ood_score", self.ood_score)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodCalibrationCellReport {
    pub cell_id: String,
    pub threshold: f32,
    pub id_rows: usize,
    pub ood_rows: usize,
    pub auc: Option<f32>,
    pub false_positive_rate: f32,
    pub ood_recall: f32,
    pub flags: Vec<String>,
}

impl OodCalibrationCellReport {
    pub fn validate(&self) -> Result<(), EvalError> {
        validate_calibration_cell(&self.cell_id)?;
        validate_probability("ood_calibration_cell.threshold", self.threshold)?;
        validate_probability(
            "ood_calibration_cell.false_positive_rate",
            self.false_positive_rate,
        )?;
        validate_probability("ood_calibration_cell.ood_recall", self.ood_recall)?;
        if let Some(auc) = self.auc {
            validate_probability("ood_calibration_cell.auc", auc)?;
        }
        for flag in &self.flags {
            validate_ood_calibration_flag(flag)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodCalibrationReport {
    pub schema_version: u32,
    pub report_id: String,
    pub generated_at_unix_ms: i64,
    pub window_start_unix_ms: i64,
    pub window_end_unix_ms: i64,
    pub threshold: f32,
    pub harvested_rows: usize,
    pub synthetic_ood_rows: usize,
    pub id_rows: usize,
    pub ood_rows: usize,
    pub true_positive: usize,
    pub false_positive: usize,
    pub true_negative: usize,
    pub false_negative: usize,
    pub global_auc: Option<f32>,
    pub ood_recall: f32,
    pub false_positive_rate: f32,
    pub min_required_auc: f32,
    pub selected_for_serving: bool,
    pub flags: Vec<String>,
    pub cell_reports: Vec<OodCalibrationCellReport>,
    pub source_harvest_cf: String,
    pub source_synthetic_cf: String,
}

impl OodCalibrationReport {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.schema_version != OOD_CALIBRATION_SCHEMA_VERSION {
            return Err(invalid(format!(
                "OOD calibration schema_version must be {OOD_CALIBRATION_SCHEMA_VERSION}"
            )));
        }
        if self.report_id.trim().is_empty() {
            return Err(invalid("OOD calibration report_id must be non-empty"));
        }
        if self.generated_at_unix_ms <= 0 {
            return Err(invalid(
                "OOD calibration generated_at_unix_ms must be positive",
            ));
        }
        if self.window_start_unix_ms > self.window_end_unix_ms {
            return Err(invalid(
                "OOD calibration window_start_unix_ms must be <= window_end_unix_ms",
            ));
        }
        validate_probability("ood_calibration.threshold", self.threshold)?;
        if let Some(auc) = self.global_auc {
            validate_probability("ood_calibration.global_auc", auc)?;
        }
        validate_probability("ood_calibration.ood_recall", self.ood_recall)?;
        validate_probability(
            "ood_calibration.false_positive_rate",
            self.false_positive_rate,
        )?;
        validate_probability("ood_calibration.min_required_auc", self.min_required_auc)?;
        for flag in &self.flags {
            validate_ood_calibration_flag(flag)?;
        }
        for cell in &self.cell_reports {
            cell.validate()?;
        }
        if self.source_harvest_cf != context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST {
            return Err(invalid(
                "OOD calibration source_harvest_cf must point at CF_MEJEPA_OOD_HARVEST",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodGateDecision {
    pub verdict: Verdict,
    pub reason: String,
    pub ood_score: f32,
    pub threshold: Option<f32>,
}

impl OodGateDecision {
    pub fn validate(&self) -> Result<(), EvalError> {
        validate_probability("ood_gate_decision.ood_score", self.ood_score)?;
        if let Some(threshold) = self.threshold {
            validate_probability("ood_gate_decision.threshold", threshold)?;
        }
        if self.reason.trim().is_empty() {
            return Err(invalid("ood_gate_decision.reason must be non-empty"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodHarvestReport {
    pub scanned_live_predictions: usize,
    pub above_threshold_predictions: usize,
    pub harvested_count: usize,
    pub queued_count: usize,
    pub quarantined_count: usize,
    pub tiered_down_count: usize,
    pub skipped_existing_count: usize,
    pub harvested_prediction_ids: Vec<String>,
    pub quarantine_codes: Vec<String>,
    pub source_live_prediction_cf: String,
    pub harvest_cf: String,
    pub calibration_cf: String,
    pub active_learning_queue_cf: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodHarvestBytesAnchor {
    pub db_path: Option<String>,
    pub cf: String,
    pub key_hex: String,
    pub value_len: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OodHarvestReviewRow {
    pub prediction_id_hex: String,
    pub task_id: String,
    pub ood_score: f32,
    pub status: OodHarvestStatus,
    pub priority_weight: f32,
    pub harvested_at_unix_ms: i64,
    pub panel_id_hex: String,
    pub affected_chunk_count: usize,
    pub bytes_anchor: OodHarvestBytesAnchor,
}

pub(super) fn validate_prediction_id(
    prediction_id: PredictionId,
    field: &str,
) -> Result<(), EvalError> {
    if prediction_id.0.iter().all(|byte| *byte == 0) {
        return Err(invalid(format!("{field} must be non-zero")));
    }
    Ok(())
}

pub(super) fn validate_probability(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(invalid(format!("{field} must be finite and in [0,1]")));
    }
    Ok(())
}

pub(super) fn validate_calibration_cell(value: &str) -> Result<(), EvalError> {
    if value.trim().is_empty() {
        return Err(invalid("OOD calibration cell must be non-empty"));
    }
    if value.len() > 256 {
        return Err(invalid("OOD calibration cell exceeds 256 bytes"));
    }
    if value.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(invalid("OOD calibration cell contains a control character"));
    }
    Ok(())
}

fn validate_ood_calibration_flag(flag: &str) -> Result<(), EvalError> {
    match flag {
        OOD_GATE_OVER_FLAGGING
        | OOD_HARVEST_EMPTY
        | OOD_CELL_AUC_REGRESSION
        | OOD_CELL_INSUFFICIENT_SUPPORT
        | OOD_RECALL_BELOW_TARGET
        | OOD_AUC_BELOW_TARGET
        | OOD_CALIBRATOR_MISSING
        | OOD_SCORE_ABOVE_THRESHOLD => Ok(()),
        _ => Err(invalid(format!("unknown OOD calibration flag {flag}"))),
    }
}

pub(super) fn invalid(message: impl Into<String>) -> EvalError {
    EvalError::new(EvalErrorCode::InvalidInput, message)
}

fn validate_panel_id(panel_id: PanelId) -> Result<(), EvalError> {
    if panel_id.0.iter().all(|byte| *byte == 0) {
        return Err(invalid("ood_harvest.panel_id must be non-zero"));
    }
    Ok(())
}
