use crate::types::Verdict;
use context_graph_mejepa_cf::{CF_MEJEPA_CROSS_PANEL_AGREEMENT, CF_MEJEPA_PANEL_B_OBSERVATIONS};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

pub const CROSS_PANEL_SCHEMA_VERSION: u32 = 1;
pub const CROSS_PANEL_GOODHART_DETECTED: &str = "CROSS_PANEL_GOODHART_DETECTED";
pub const PANEL_A_GATE_THRESHOLD: f32 = 0.95;
pub const PANEL_B_GATE_THRESHOLD: f32 = 0.95;
pub const PANEL_CORRELATION_DELTA_MAX: f32 = 0.05;
pub const PANEL_B_GOODHART_FLOOR: f32 = 0.85;
pub const PANEL_B_GOODHART_DROP: f32 = 0.10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossPanelFlag {
    Healthy,
    CrossPanelGoodhartDetected,
    Underpowered,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelBObservationRecord {
    pub schema_version: u32,
    pub prediction_id_hex: String,
    pub panel_b_run_id: String,
    pub panel_id: String,
    pub model_backed_panel_b: bool,
    pub ship_gate_eligible: bool,
    pub score_source: PanelBScoreSource,
    pub score_b: f32,
    pub verdict_b: Verdict,
    pub oracle_verdict: Verdict,
    pub cell_key: String,
    pub accepted_label_ids: Vec<String>,
    pub failure_evidence_set_ids: Vec<String>,
    pub panel_b_artifact_shas: BTreeMap<String, String>,
    pub panel_b_resident_during_normal_inference: bool,
    pub created_at_unix_ms: i64,
}

impl PanelBObservationRecord {
    pub fn key(&self) -> String {
        format!("{}::{}", self.prediction_id_hex, self.panel_b_run_id)
    }

    pub fn validate(&self) -> CrossPanelResult<()> {
        require(
            self.schema_version == CROSS_PANEL_SCHEMA_VERSION,
            "panel_b_observation.schema_version",
        )?;
        validate_id("prediction_id_hex", &self.prediction_id_hex)?;
        validate_id("panel_b_run_id", &self.panel_b_run_id)?;
        validate_id("panel_id", &self.panel_id)?;
        validate_id("cell_key", &self.cell_key)?;
        self.score_source.validate()?;
        require(
            self.model_backed_panel_b,
            "panel_b_observation must come from model-backed Panel B scoring",
        )?;
        require(
            self.ship_gate_eligible,
            "panel_b_observation must be ship-gate eligible",
        )?;
        require(
            self.score_source.row_count > 0,
            "panel_b score source row_count must be positive",
        )?;
        validate_score("score_b", self.score_b)?;
        require(
            !self.panel_b_resident_during_normal_inference,
            "panel_b must not be resident during normal inference",
        )?;
        require(
            !self.panel_b_artifact_shas.is_empty(),
            "panel_b_artifact_shas must not be empty",
        )?;
        for (slot, sha) in &self.panel_b_artifact_shas {
            validate_id("panel_b_artifact_slot", slot)?;
            validate_sha("panel_b_artifact_sha", sha)?;
        }
        for label in &self.accepted_label_ids {
            validate_id("accepted_label_id", label)?;
        }
        for evidence in &self.failure_evidence_set_ids {
            validate_id("failure_evidence_set_id", evidence)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossPanelAgreementRecord {
    pub schema_version: u32,
    pub prediction_id_hex: String,
    pub panel_pair_id: String,
    pub model_backed_panel_b: bool,
    pub ship_gate_eligible: bool,
    pub score_source: PanelBScoreSource,
    pub verdict_a: Verdict,
    pub verdict_b: Verdict,
    pub score_a: f32,
    pub score_b: f32,
    pub oracle_verdict: Verdict,
    pub agree_flag: bool,
    pub cell_key: String,
    pub accepted_label_ids: Vec<String>,
    pub failure_evidence_set_ids: Vec<String>,
    pub goodhart_flag: Option<String>,
    pub created_at_unix_ms: i64,
}

impl CrossPanelAgreementRecord {
    pub fn key(&self) -> String {
        format!("{}::{}", self.prediction_id_hex, self.panel_pair_id)
    }

    pub fn to_score_row(&self) -> CrossPanelScoreRow {
        CrossPanelScoreRow {
            prediction_id_hex: self.prediction_id_hex.clone(),
            score_a: self.score_a,
            score_b: self.score_b,
            oracle_pass: self.oracle_verdict == Verdict::Pass,
            accepted_label_ids: self.accepted_label_ids.clone(),
            failure_evidence_set_ids: self.failure_evidence_set_ids.clone(),
        }
    }

    pub fn validate(&self) -> CrossPanelResult<()> {
        require(
            self.schema_version == CROSS_PANEL_SCHEMA_VERSION,
            "cross_panel_agreement.schema_version",
        )?;
        validate_id("prediction_id_hex", &self.prediction_id_hex)?;
        validate_id("panel_pair_id", &self.panel_pair_id)?;
        validate_id("cell_key", &self.cell_key)?;
        self.score_source.validate()?;
        require(
            self.model_backed_panel_b,
            "cross_panel_agreement must come from model-backed Panel B scoring",
        )?;
        require(
            self.ship_gate_eligible,
            "cross_panel_agreement must be ship-gate eligible",
        )?;
        validate_score("score_a", self.score_a)?;
        validate_score("score_b", self.score_b)?;
        if let Some(flag) = &self.goodhart_flag {
            validate_id("goodhart_flag", flag)?;
        }
        for label in &self.accepted_label_ids {
            validate_id("accepted_label_id", label)?;
        }
        for evidence in &self.failure_evidence_set_ids {
            validate_id("failure_evidence_set_id", evidence)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelBScoreSource {
    pub source_id: String,
    pub source_uri: String,
    pub source_sha256: String,
    pub artifact_manifest_sha256: String,
    pub row_count: usize,
}

impl PanelBScoreSource {
    pub fn validate(&self) -> CrossPanelResult<()> {
        validate_non_synthetic_id("panel_b_score_source.source_id", &self.source_id)?;
        validate_non_synthetic_id("panel_b_score_source.source_uri", &self.source_uri)?;
        validate_sha("panel_b_score_source.source_sha256", &self.source_sha256)?;
        validate_sha(
            "panel_b_score_source.artifact_manifest_sha256",
            &self.artifact_manifest_sha256,
        )?;
        require(
            self.row_count > 0,
            "panel_b_score_source.row_count must be positive",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossPanelScoreRow {
    pub prediction_id_hex: String,
    pub score_a: f32,
    pub score_b: f32,
    pub oracle_pass: bool,
    pub accepted_label_ids: Vec<String>,
    pub failure_evidence_set_ids: Vec<String>,
}

impl CrossPanelScoreRow {
    pub fn validate(&self) -> CrossPanelResult<()> {
        validate_prediction_id_hex("prediction_id_hex", &self.prediction_id_hex)?;
        validate_score("score_a", self.score_a)?;
        validate_score("score_b", self.score_b)?;
        validate_required_id_set("accepted_label_ids", &self.accepted_label_ids)?;
        validate_required_id_set("failure_evidence_set_ids", &self.failure_evidence_set_ids)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossPanelMetric {
    pub n: usize,
    pub panel_a_oracle_correlation: f32,
    pub panel_b_oracle_correlation: f32,
    pub abs_correlation_delta: f32,
    pub flag: CrossPanelFlag,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossPanelWindowReport {
    pub schema_version: u32,
    pub window_id: String,
    pub metric: CrossPanelMetric,
    pub label_family_metrics: BTreeMap<String, CrossPanelMetric>,
    pub failure_evidence_metrics: BTreeMap<String, CrossPanelMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderNonOverlapAudit {
    pub panel_a_artifact_shas: BTreeMap<String, String>,
    pub panel_b_artifact_shas: BTreeMap<String, String>,
    pub overlap_count: usize,
    pub overlapping_shas: Vec<String>,
}

impl EncoderNonOverlapAudit {
    pub fn passes(&self) -> bool {
        self.overlap_count == 0
    }
}

pub fn write_panel_b_observation(
    db: &DB,
    record: &PanelBObservationRecord,
) -> CrossPanelResult<()> {
    record.validate()?;
    put_readback(
        db,
        CF_MEJEPA_PANEL_B_OBSERVATIONS,
        record.key().as_bytes(),
        &bincode::serialize(record).map_err(CrossPanelError::from_err)?,
    )
}

pub fn write_cross_panel_agreement(
    db: &DB,
    record: &CrossPanelAgreementRecord,
) -> CrossPanelResult<()> {
    record.validate()?;
    put_readback(
        db,
        CF_MEJEPA_CROSS_PANEL_AGREEMENT,
        record.key().as_bytes(),
        &bincode::serialize(record).map_err(CrossPanelError::from_err)?,
    )
}

pub fn read_panel_b_observations(db: &DB) -> CrossPanelResult<Vec<PanelBObservationRecord>> {
    let cf = cf(db, CF_MEJEPA_PANEL_B_OBSERVATIONS)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, bytes) = item.map_err(CrossPanelError::from_err)?;
        let key = String::from_utf8(key.to_vec()).map_err(CrossPanelError::from_err)?;
        let record: PanelBObservationRecord =
            bincode::deserialize(&bytes).map_err(CrossPanelError::from_err)?;
        record.validate()?;
        require(key == record.key(), "panel_b_observation key mismatch")?;
        out.push(record);
    }
    Ok(out)
}

pub fn read_cross_panel_agreements(db: &DB) -> CrossPanelResult<Vec<CrossPanelAgreementRecord>> {
    let cf = cf(db, CF_MEJEPA_CROSS_PANEL_AGREEMENT)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, bytes) = item.map_err(CrossPanelError::from_err)?;
        let key = String::from_utf8(key.to_vec()).map_err(CrossPanelError::from_err)?;
        let record: CrossPanelAgreementRecord =
            bincode::deserialize(&bytes).map_err(CrossPanelError::from_err)?;
        record.validate()?;
        require(key == record.key(), "cross_panel_agreement key mismatch")?;
        out.push(record);
    }
    Ok(out)
}

pub fn build_cross_panel_window_report(
    window_id: impl Into<String>,
    rows: &[CrossPanelScoreRow],
) -> CrossPanelResult<CrossPanelWindowReport> {
    let window_id = window_id.into();
    validate_id("window_id", &window_id)?;
    validate_cross_panel_score_rows(rows)?;
    let metric = cross_panel_metric(rows)?;
    let label_family_metrics = grouped_metrics(rows, |row| {
        let mut groups = vec!["label:all_rows".to_string()];
        if row.accepted_label_ids.is_empty() {
            groups.push("label:unlabeled".to_string());
        } else {
            groups.extend(row.accepted_label_ids.clone());
        }
        groups
    })?;
    let failure_evidence_metrics = grouped_metrics(rows, |row| {
        let mut groups = vec!["failure_evidence:all_rows".to_string()];
        if row.failure_evidence_set_ids.is_empty() {
            groups.push("failure_evidence:none".to_string());
        } else {
            groups.extend(row.failure_evidence_set_ids.clone());
        }
        groups
    })?;
    Ok(CrossPanelWindowReport {
        schema_version: CROSS_PANEL_SCHEMA_VERSION,
        window_id,
        metric,
        label_family_metrics,
        failure_evidence_metrics,
    })
}

pub fn cross_panel_metric(rows: &[CrossPanelScoreRow]) -> CrossPanelResult<CrossPanelMetric> {
    validate_cross_panel_score_rows(rows)?;
    require(
        rows.len() >= 2,
        "cross panel window requires at least two rows",
    )?;
    let panel_a = pearson(
        rows.iter().map(|row| row.score_a),
        rows.iter().map(|row| row.oracle_pass),
    )?;
    let panel_b = pearson(
        rows.iter().map(|row| row.score_b),
        rows.iter().map(|row| row.oracle_pass),
    )?;
    let abs_correlation_delta = (panel_a - panel_b).abs();
    let flag = if panel_a >= PANEL_A_GATE_THRESHOLD
        && (panel_b < PANEL_B_GOODHART_FLOOR || panel_a - panel_b >= PANEL_B_GOODHART_DROP)
    {
        CrossPanelFlag::CrossPanelGoodhartDetected
    } else if panel_a >= PANEL_A_GATE_THRESHOLD
        && panel_b >= PANEL_B_GATE_THRESHOLD
        && abs_correlation_delta <= PANEL_CORRELATION_DELTA_MAX
    {
        CrossPanelFlag::Healthy
    } else {
        CrossPanelFlag::Underpowered
    };
    Ok(CrossPanelMetric {
        n: rows.len(),
        panel_a_oracle_correlation: panel_a,
        panel_b_oracle_correlation: panel_b,
        abs_correlation_delta,
        flag,
    })
}

pub fn validate_cross_panel_score_rows(rows: &[CrossPanelScoreRow]) -> CrossPanelResult<()> {
    require(
        rows.len() >= 2,
        "cross panel window requires at least two rows",
    )?;
    let mut prediction_ids = BTreeSet::new();
    for row in rows {
        row.validate()?;
        require(
            prediction_ids.insert(row.prediction_id_hex.clone()),
            format!(
                "duplicate prediction_id_hex {} in cross panel score rows",
                row.prediction_id_hex
            ),
        )?;
    }
    Ok(())
}

pub fn encoder_non_overlap_audit(
    panel_a_artifact_shas: BTreeMap<String, String>,
    panel_b_artifact_shas: BTreeMap<String, String>,
) -> CrossPanelResult<EncoderNonOverlapAudit> {
    require(
        !panel_a_artifact_shas.is_empty() && !panel_b_artifact_shas.is_empty(),
        "encoder artifact maps must not be empty",
    )?;
    for (slot, sha) in panel_a_artifact_shas
        .iter()
        .chain(panel_b_artifact_shas.iter())
    {
        validate_id("encoder_slot", slot)?;
        validate_sha("encoder_sha", sha)?;
    }
    let panel_a: BTreeSet<_> = panel_a_artifact_shas.values().cloned().collect();
    let panel_b: BTreeSet<_> = panel_b_artifact_shas.values().cloned().collect();
    let overlapping_shas: Vec<_> = panel_a.intersection(&panel_b).cloned().collect();
    Ok(EncoderNonOverlapAudit {
        panel_a_artifact_shas,
        panel_b_artifact_shas,
        overlap_count: overlapping_shas.len(),
        overlapping_shas,
    })
}

pub fn agreement_goodhart_flag(metric: &CrossPanelMetric) -> Option<String> {
    if metric.flag == CrossPanelFlag::CrossPanelGoodhartDetected {
        Some(CROSS_PANEL_GOODHART_DETECTED.to_string())
    } else {
        None
    }
}

fn grouped_metrics<F>(
    rows: &[CrossPanelScoreRow],
    mut group_ids: F,
) -> CrossPanelResult<BTreeMap<String, CrossPanelMetric>>
where
    F: FnMut(&CrossPanelScoreRow) -> Vec<String>,
{
    let mut grouped: BTreeMap<String, Vec<CrossPanelScoreRow>> = BTreeMap::new();
    for row in rows {
        for group in group_ids(row) {
            validate_id("cross_panel_group", &group)?;
            grouped.entry(group).or_default().push(row.clone());
        }
    }
    let mut out = BTreeMap::new();
    for (group, group_rows) in grouped {
        if group_rows.len() >= 2 && has_correlation_variance(&group_rows) {
            out.insert(group, cross_panel_metric(&group_rows)?);
        }
    }
    Ok(out)
}

fn has_correlation_variance(rows: &[CrossPanelScoreRow]) -> bool {
    let Some(first) = rows.first() else {
        return false;
    };
    let has_panel_a_variance = rows
        .iter()
        .any(|row| (row.score_a - first.score_a).abs() > f32::EPSILON);
    let has_panel_b_variance = rows
        .iter()
        .any(|row| (row.score_b - first.score_b).abs() > f32::EPSILON);
    let has_oracle_variance = rows.iter().any(|row| row.oracle_pass != first.oracle_pass);
    has_panel_a_variance && has_panel_b_variance && has_oracle_variance
}

fn pearson<S, O>(scores: S, oracle_pass: O) -> CrossPanelResult<f32>
where
    S: IntoIterator<Item = f32>,
    O: IntoIterator<Item = bool>,
{
    let scores: Vec<f32> = scores.into_iter().collect();
    let oracle: Vec<f32> = oracle_pass
        .into_iter()
        .map(|pass| if pass { 1.0 } else { 0.0 })
        .collect();
    require(scores.len() == oracle.len(), "score/oracle length mismatch")?;
    require(scores.len() >= 2, "correlation requires at least two rows")?;
    for score in &scores {
        validate_score("score", *score)?;
    }
    let n = scores.len() as f32;
    let mean_x = scores.iter().sum::<f32>() / n;
    let mean_y = oracle.iter().sum::<f32>() / n;
    let mut cov = 0.0_f32;
    let mut var_x = 0.0_f32;
    let mut var_y = 0.0_f32;
    for (x, y) in scores.iter().zip(oracle.iter()) {
        let dx = *x - mean_x;
        let dy = *y - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    require(var_x > f32::EPSILON, "score variance is zero")?;
    require(var_y > f32::EPSILON, "oracle variance is zero")?;
    Ok((cov / (var_x.sqrt() * var_y.sqrt())).clamp(-1.0, 1.0))
}

fn put_readback(db: &DB, cf_name: &str, key: &[u8], value: &[u8]) -> CrossPanelResult<()> {
    let cf = cf(db, cf_name)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, value, &opts)
        .map_err(CrossPanelError::from_err)?;
    let readback = db
        .get_cf(cf, key)
        .map_err(CrossPanelError::from_err)?
        .ok_or_else(|| CrossPanelError::new(format!("missing readback from {cf_name}")))?;
    require(readback.as_slice() == value, "CF readback changed payload")
}

fn cf<'a>(db: &'a DB, cf_name: &str) -> CrossPanelResult<&'a rocksdb::ColumnFamily> {
    db.cf_handle(cf_name)
        .ok_or_else(|| CrossPanelError::new(format!("missing column family {cf_name}")))
}

fn validate_id(field: &str, value: &str) -> CrossPanelResult<()> {
    require(
        !value.trim().is_empty(),
        format!("{field} must not be empty"),
    )?;
    require(
        !value.contains('\n') && !value.contains('\r'),
        format!("{field} must be single-line"),
    )
}

fn validate_non_synthetic_id(field: &str, value: &str) -> CrossPanelResult<()> {
    validate_id(field, value)?;
    let lower = value.to_ascii_lowercase();
    for forbidden in [
        "fake",
        "synthetic",
        "placeholder",
        "dummy",
        "stub",
        "hardcoded",
    ] {
        require(
            !lower.contains(forbidden),
            format!("{field} must not contain {forbidden}"),
        )?;
    }
    Ok(())
}

fn validate_prediction_id_hex(field: &str, value: &str) -> CrossPanelResult<()> {
    validate_id(field, value)?;
    require(
        value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        format!("{field} must be exactly 32 hexadecimal characters"),
    )
}

fn validate_required_id_set(field: &str, values: &[String]) -> CrossPanelResult<()> {
    require(!values.is_empty(), format!("{field} must not be empty"))?;
    let mut seen = BTreeSet::new();
    for value in values {
        validate_id(field, value)?;
        require(
            seen.insert(value.clone()),
            format!("{field} contains duplicate id {value}"),
        )?;
    }
    Ok(())
}

fn validate_score(field: &str, value: f32) -> CrossPanelResult<()> {
    require(
        value.is_finite() && (0.0..=1.0).contains(&value),
        format!("{field} must be finite in [0,1]"),
    )
}

fn validate_sha(field: &str, value: &str) -> CrossPanelResult<()> {
    validate_id(field, value)?;
    require(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        format!("{field} must be a 64-char hex sha256"),
    )
}

fn require(condition: bool, detail: impl Into<String>) -> CrossPanelResult<()> {
    if condition {
        Ok(())
    } else {
        Err(CrossPanelError::new(detail))
    }
}

pub type CrossPanelResult<T> = Result<T, CrossPanelError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossPanelError {
    detail: String,
}

impl CrossPanelError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    fn from_err(err: impl fmt::Display) -> Self {
        Self::new(err.to_string())
    }
}

impl fmt::Display for CrossPanelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cross panel error: {}", self.detail)
    }
}

impl Error for CrossPanelError {}
