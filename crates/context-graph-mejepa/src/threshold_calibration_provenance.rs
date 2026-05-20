use context_graph_mejepa_cf::CF_MEJEPA_THRESHOLD_CALIBRATION_PROVENANCE;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const THRESHOLD_CALIBRATION_SCHEMA_VERSION: u32 = 1;
pub const THRESHOLD_CALIBRATION_NONDISJOINT: &str = "THRESHOLD_CALIBRATION_NONDISJOINT";
pub const THRESHOLD_CALIBRATION_FLOOR_PERCENTILE: f32 = 0.10;
pub const THRESHOLD_CALIBRATION_TAU_TOLERANCE: f32 = 0.0001;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdSurfaceKind {
    FailureFingerprint,
    OodCalibration,
    HeadCalibration,
    FailureModeSkill,
    Constellation,
    LabelRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThresholdPartitionMember {
    pub row_key: String,
    pub accepted_label_id: Option<String>,
    pub code_state_key: String,
}

impl ThresholdPartitionMember {
    fn pair_key(&self) -> String {
        format!(
            "{}::{}",
            self.row_key,
            self.accepted_label_id.as_deref().unwrap_or("label:none")
        )
    }

    fn validate(&self, field: &str) -> ThresholdCalibrationResult<()> {
        validate_id(&format!("{field}.row_key"), &self.row_key)?;
        validate_id(&format!("{field}.code_state_key"), &self.code_state_key)?;
        if let Some(label) = &self.accepted_label_id {
            validate_id(&format!("{field}.accepted_label_id"), label)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThresholdCalibrationProvenanceRecord {
    pub schema_version: u32,
    pub surface: ThresholdSurfaceKind,
    pub source_cf: String,
    pub source_row_key: String,
    pub threshold_name: String,
    pub tau: f32,
    pub calibration_partition_id: String,
    pub train_partition_id: String,
    pub eval_partition_id: String,
    pub calibration_members: Vec<ThresholdPartitionMember>,
    pub train_members: Vec<ThresholdPartitionMember>,
    pub eval_members: Vec<ThresholdPartitionMember>,
    pub authentic_sample_cosines: Vec<f32>,
    pub live_input_label_ids: Vec<String>,
    pub target_side_label_ids: Vec<String>,
    pub label_registry_hash: String,
    pub usefulness_metrics_hash: String,
    pub created_at_unix_ms: i64,
}

impl ThresholdCalibrationProvenanceRecord {
    pub fn key(&self) -> String {
        format!(
            "{}::{}::{}",
            self.source_cf, self.source_row_key, self.threshold_name
        )
    }

    pub fn validate_shape(&self) -> ThresholdCalibrationResult<()> {
        require(
            self.schema_version == THRESHOLD_CALIBRATION_SCHEMA_VERSION,
            "schema_version must match TASK-PY-G-113 provenance schema",
        )?;
        validate_id("source_cf", &self.source_cf)?;
        validate_id("source_row_key", &self.source_row_key)?;
        validate_id("threshold_name", &self.threshold_name)?;
        validate_probability("tau", self.tau)?;
        validate_id("calibration_partition_id", &self.calibration_partition_id)?;
        validate_id("train_partition_id", &self.train_partition_id)?;
        validate_id("eval_partition_id", &self.eval_partition_id)?;
        validate_sha("label_registry_hash", &self.label_registry_hash)?;
        validate_sha("usefulness_metrics_hash", &self.usefulness_metrics_hash)?;
        require(
            self.created_at_unix_ms > 0,
            "created_at_unix_ms must be positive",
        )?;
        require(
            !self.calibration_members.is_empty(),
            "calibration partition must have members",
        )?;
        require(
            !self.authentic_sample_cosines.is_empty(),
            "authentic_sample_cosines must not be empty",
        )?;
        for (idx, member) in self.calibration_members.iter().enumerate() {
            member.validate(&format!("calibration_members[{idx}]"))?;
        }
        for (idx, member) in self.train_members.iter().enumerate() {
            member.validate(&format!("train_members[{idx}]"))?;
        }
        for (idx, member) in self.eval_members.iter().enumerate() {
            member.validate(&format!("eval_members[{idx}]"))?;
        }
        for (idx, score) in self.authentic_sample_cosines.iter().enumerate() {
            validate_probability(&format!("authentic_sample_cosines[{idx}]"), *score)?;
        }
        for label in &self.live_input_label_ids {
            validate_id("live_input_label_ids", label)?;
        }
        for label in &self.target_side_label_ids {
            validate_id("target_side_label_ids", label)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThresholdCalibrationFailure {
    pub source_cf: String,
    pub source_row_key: String,
    pub threshold_name: String,
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThresholdCalibrationAuditConfig {
    pub expected_label_registry_hash: Option<String>,
    pub expected_usefulness_metrics_hash: Option<String>,
}

impl ThresholdCalibrationAuditConfig {
    pub fn from_expected_lineage(
        label_registry_hash: impl Into<String>,
        usefulness_metrics_hash: impl Into<String>,
    ) -> Self {
        Self {
            expected_label_registry_hash: Some(label_registry_hash.into()),
            expected_usefulness_metrics_hash: Some(usefulness_metrics_hash.into()),
        }
    }

    fn validate(&self) -> ThresholdCalibrationResult<()> {
        if let Some(hash) = &self.expected_label_registry_hash {
            validate_sha("expected_label_registry_hash", hash)?;
        }
        if let Some(hash) = &self.expected_usefulness_metrics_hash {
            validate_sha("expected_usefulness_metrics_hash", hash)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThresholdCalibrationAuditReport {
    pub schema_version: u32,
    pub all_passed: bool,
    pub threshold_rows_audited: usize,
    pub source_cfs_audited: Vec<String>,
    pub failures: Vec<ThresholdCalibrationFailure>,
    pub tenth_percentile_floor: f32,
    pub tolerance: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum ThresholdCalibrationError {
    #[error("{code}: {detail}")]
    Invalid { code: &'static str, detail: String },
    #[error("RocksDB error: {0}")]
    Rocks(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type ThresholdCalibrationResult<T> = Result<T, ThresholdCalibrationError>;

pub fn write_threshold_calibration_provenance(
    db: &DB,
    record: &ThresholdCalibrationProvenanceRecord,
) -> ThresholdCalibrationResult<()> {
    record.validate_shape()?;
    put_readback(
        db,
        CF_MEJEPA_THRESHOLD_CALIBRATION_PROVENANCE,
        record.key().as_bytes(),
        &bincode::serialize(record)
            .map_err(|err| ThresholdCalibrationError::Serialization(err.to_string()))?,
    )
}

pub fn read_threshold_calibration_provenance(
    db: &DB,
) -> ThresholdCalibrationResult<Vec<ThresholdCalibrationProvenanceRecord>> {
    let cf = cf(db, CF_MEJEPA_THRESHOLD_CALIBRATION_PROVENANCE)?;
    let mut records = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, bytes) = item.map_err(|err| ThresholdCalibrationError::Rocks(err.to_string()))?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| ThresholdCalibrationError::Serialization(err.to_string()))?;
        let record: ThresholdCalibrationProvenanceRecord = bincode::deserialize(&bytes)
            .map_err(|err| ThresholdCalibrationError::Serialization(err.to_string()))?;
        record.validate_shape()?;
        require(key == record.key(), "threshold provenance key mismatch")?;
        records.push(record);
    }
    Ok(records)
}

pub fn audit_threshold_calibration_provenance_db(
    db: &DB,
) -> ThresholdCalibrationResult<ThresholdCalibrationAuditReport> {
    audit_threshold_calibration_provenance_db_with_config(db, None)
}

pub fn audit_threshold_calibration_provenance_db_with_config(
    db: &DB,
    config: Option<&ThresholdCalibrationAuditConfig>,
) -> ThresholdCalibrationResult<ThresholdCalibrationAuditReport> {
    let records = read_threshold_calibration_provenance(db)?;
    let mut report = audit_threshold_calibration_provenance_with_config(&records, config);
    if records.is_empty() {
        report.failures.push(ThresholdCalibrationFailure {
            source_cf: CF_MEJEPA_THRESHOLD_CALIBRATION_PROVENANCE.to_string(),
            source_row_key: "all".to_string(),
            threshold_name: "all".to_string(),
            code: THRESHOLD_CALIBRATION_NONDISJOINT.to_string(),
            detail: "no threshold calibration provenance rows found".to_string(),
        });
    }
    for record in &records {
        match source_row_exists(db, &record.source_cf, &record.source_row_key) {
            Ok(true) => {}
            Ok(false) => report.failures.push(failure(
                record,
                format!(
                    "source threshold row {}:{} is missing",
                    record.source_cf, record.source_row_key
                ),
            )),
            Err(err) => report.failures.push(failure(record, err.to_string())),
        }
    }
    report.all_passed = report.failures.is_empty();
    Ok(report)
}

pub fn audit_threshold_calibration_provenance(
    records: &[ThresholdCalibrationProvenanceRecord],
) -> ThresholdCalibrationAuditReport {
    audit_threshold_calibration_provenance_with_config(records, None)
}

pub fn audit_threshold_calibration_provenance_with_config(
    records: &[ThresholdCalibrationProvenanceRecord],
    config: Option<&ThresholdCalibrationAuditConfig>,
) -> ThresholdCalibrationAuditReport {
    let mut failures = Vec::new();
    let mut source_cfs = BTreeSet::new();
    if let Some(config) = config {
        if let Err(err) = config.validate() {
            failures.push(ThresholdCalibrationFailure {
                source_cf: CF_MEJEPA_THRESHOLD_CALIBRATION_PROVENANCE.to_string(),
                source_row_key: "audit_config".to_string(),
                threshold_name: "audit_config".to_string(),
                code: THRESHOLD_CALIBRATION_NONDISJOINT.to_string(),
                detail: err.to_string(),
            });
        }
    }
    for record in records {
        source_cfs.insert(record.source_cf.clone());
        if let Err(err) = record.validate_shape() {
            failures.push(ThresholdCalibrationFailure {
                source_cf: record.source_cf.clone(),
                source_row_key: record.source_row_key.clone(),
                threshold_name: record.threshold_name.clone(),
                code: THRESHOLD_CALIBRATION_NONDISJOINT.to_string(),
                detail: err.to_string(),
            });
            continue;
        }
        if let Some(detail) =
            partition_overlap_detail("train", &record.calibration_members, &record.train_members)
        {
            failures.push(failure(record, detail));
        }
        if let Some(detail) =
            partition_overlap_detail("eval", &record.calibration_members, &record.eval_members)
        {
            failures.push(failure(record, detail));
        }
        let expected_tau = tenth_percentile_floor(&record.authentic_sample_cosines);
        if (record.tau - expected_tau).abs() > THRESHOLD_CALIBRATION_TAU_TOLERANCE {
            failures.push(failure(
                record,
                format!(
                    "tau {} does not match recomputed p10 floor {} within tolerance {}",
                    record.tau, expected_tau, THRESHOLD_CALIBRATION_TAU_TOLERANCE
                ),
            ));
        }
        if let Some(detail) = target_label_leakage_detail(record) {
            failures.push(failure(record, detail));
        }
        if let Some(detail) = label_lineage_detail(record, config) {
            failures.push(failure(record, detail));
        }
    }
    ThresholdCalibrationAuditReport {
        schema_version: THRESHOLD_CALIBRATION_SCHEMA_VERSION,
        all_passed: failures.is_empty(),
        threshold_rows_audited: records.len(),
        source_cfs_audited: source_cfs.into_iter().collect(),
        failures,
        tenth_percentile_floor: THRESHOLD_CALIBRATION_FLOOR_PERCENTILE,
        tolerance: THRESHOLD_CALIBRATION_TAU_TOLERANCE,
    }
}

fn label_lineage_detail(
    record: &ThresholdCalibrationProvenanceRecord,
    config: Option<&ThresholdCalibrationAuditConfig>,
) -> Option<String> {
    let config = config?;
    if let Some(expected) = &config.expected_label_registry_hash {
        if record.label_registry_hash != *expected {
            return Some(format!(
                "label_registry_hash {} does not match expected #413 hash {}",
                record.label_registry_hash, expected
            ));
        }
    }
    if let Some(expected) = &config.expected_usefulness_metrics_hash {
        if record.usefulness_metrics_hash != *expected {
            return Some(format!(
                "usefulness_metrics_hash {} does not match expected #413 hash {}",
                record.usefulness_metrics_hash, expected
            ));
        }
    }
    None
}

pub fn require_threshold_calibration_ship_eligible(
    report: &ThresholdCalibrationAuditReport,
) -> ThresholdCalibrationResult<()> {
    if report.all_passed {
        return Ok(());
    }
    Err(ThresholdCalibrationError::Invalid {
        code: THRESHOLD_CALIBRATION_NONDISJOINT,
        detail: format!(
            "{} threshold calibration provenance failures block ship eligibility",
            report.failures.len()
        ),
    })
}

pub fn tenth_percentile_floor(samples: &[f32]) -> f32 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = (THRESHOLD_CALIBRATION_FLOOR_PERCENTILE * sorted.len() as f32).ceil() as usize;
    sorted[rank.saturating_sub(1)]
}

fn partition_overlap_detail(
    other_name: &str,
    calibration: &[ThresholdPartitionMember],
    other: &[ThresholdPartitionMember],
) -> Option<String> {
    let cal_pairs = calibration
        .iter()
        .map(|m| m.pair_key())
        .collect::<BTreeSet<_>>();
    let other_pairs = other.iter().map(|m| m.pair_key()).collect::<BTreeSet<_>>();
    if let Some(pair) = cal_pairs.intersection(&other_pairs).next() {
        return Some(format!(
            "calibration partition overlaps {other_name} by row_key+accepted_label_id {pair}"
        ));
    }
    let cal_states = calibration
        .iter()
        .map(|m| m.code_state_key.clone())
        .collect::<BTreeSet<_>>();
    let other_states = other
        .iter()
        .map(|m| m.code_state_key.clone())
        .collect::<BTreeSet<_>>();
    cal_states.intersection(&other_states).next().map(|state| {
        format!("calibration partition overlaps {other_name} by code_state_key {state}")
    })
}

fn target_label_leakage_detail(record: &ThresholdCalibrationProvenanceRecord) -> Option<String> {
    let live = record
        .live_input_label_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for target in &record.target_side_label_ids {
        if live.contains(target) {
            return Some(format!(
                "target-side label {target} appears in live predictor input labels"
            ));
        }
    }
    for label in &record.live_input_label_ids {
        let lower = label.to_ascii_lowercase();
        if lower.contains("oracle")
            || lower.contains("docker")
            || lower.contains("test_phase")
            || lower.contains("code_state_outcome")
            || lower.contains("target_side_only")
        {
            return Some(format!(
                "live input label {label} contains target-side-only evidence"
            ));
        }
    }
    None
}

fn failure(
    record: &ThresholdCalibrationProvenanceRecord,
    detail: impl Into<String>,
) -> ThresholdCalibrationFailure {
    ThresholdCalibrationFailure {
        source_cf: record.source_cf.clone(),
        source_row_key: record.source_row_key.clone(),
        threshold_name: record.threshold_name.clone(),
        code: THRESHOLD_CALIBRATION_NONDISJOINT.to_string(),
        detail: detail.into(),
    }
}

fn source_row_exists(db: &DB, cf_name: &str, key: &str) -> ThresholdCalibrationResult<bool> {
    let Some(cf) = db.cf_handle(cf_name) else {
        return Ok(false);
    };
    db.get_pinned_cf(cf, key.as_bytes())
        .map(|value| value.is_some())
        .map_err(|err| ThresholdCalibrationError::Rocks(err.to_string()))
}

fn put_readback(
    db: &DB,
    cf_name: &str,
    key: &[u8],
    value: &[u8],
) -> ThresholdCalibrationResult<()> {
    let cf = cf(db, cf_name)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, value, &opts)
        .map_err(|err| ThresholdCalibrationError::Rocks(err.to_string()))?;
    db.flush_cf(cf)
        .map_err(|err| ThresholdCalibrationError::Rocks(err.to_string()))?;
    let readback = db
        .get_pinned_cf(cf, key)
        .map_err(|err| ThresholdCalibrationError::Rocks(err.to_string()))?
        .ok_or_else(|| ThresholdCalibrationError::Rocks("sync readback missing".to_string()))?;
    require(readback.as_ref() == value, "sync readback mismatch")
}

fn cf<'a>(db: &'a DB, name: &str) -> ThresholdCalibrationResult<&'a rocksdb::ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| ThresholdCalibrationError::Rocks(format!("missing column family {name}")))
}

fn require(ok: bool, detail: impl Into<String>) -> ThresholdCalibrationResult<()> {
    if ok {
        Ok(())
    } else {
        Err(ThresholdCalibrationError::Invalid {
            code: THRESHOLD_CALIBRATION_NONDISJOINT,
            detail: detail.into(),
        })
    }
}

fn validate_id(field: &str, value: &str) -> ThresholdCalibrationResult<()> {
    require(
        !value.trim().is_empty() && !value.chars().any(char::is_control) && value.len() <= 512,
        format!("{field} must be a non-empty single-line id"),
    )
}

fn validate_probability(field: &str, value: f32) -> ThresholdCalibrationResult<()> {
    require(
        value.is_finite() && (0.0..=1.0).contains(&value),
        format!("{field} must be finite in [0,1]"),
    )
}

fn validate_sha(field: &str, value: &str) -> ThresholdCalibrationResult<()> {
    require(
        value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()),
        format!("{field} must be a 64-character sha256 hex string"),
    )
}

impl fmt::Display for ThresholdSurfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: &str, state: &str) -> ThresholdPartitionMember {
        ThresholdPartitionMember {
            row_key: id.to_string(),
            accepted_label_id: Some("label:api".to_string()),
            code_state_key: state.to_string(),
        }
    }

    fn good_record() -> ThresholdCalibrationProvenanceRecord {
        ThresholdCalibrationProvenanceRecord {
            schema_version: THRESHOLD_CALIBRATION_SCHEMA_VERSION,
            surface: ThresholdSurfaceKind::FailureFingerprint,
            source_cf: "CF_MEJEPA_FAILURE_FINGERPRINTS".to_string(),
            source_row_key: "fingerprint:api".to_string(),
            threshold_name: "gtau:e1".to_string(),
            tau: 0.70,
            calibration_partition_id: "partition:calibration".to_string(),
            train_partition_id: "partition:train".to_string(),
            eval_partition_id: "partition:eval".to_string(),
            calibration_members: vec![member("row:cal", "state:cal")],
            train_members: vec![member("row:train", "state:train")],
            eval_members: vec![member("row:eval", "state:eval")],
            authentic_sample_cosines: vec![0.70, 0.72, 0.74, 0.76, 0.78],
            live_input_label_ids: vec!["label:api".to_string()],
            target_side_label_ids: vec!["oracle:pass".to_string()],
            label_registry_hash: "a".repeat(64),
            usefulness_metrics_hash: "b".repeat(64),
            created_at_unix_ms: 1,
        }
    }

    #[test]
    fn audit_accepts_disjoint_p10_record() {
        let report = audit_threshold_calibration_provenance(&[good_record()]);
        assert!(report.all_passed, "{:?}", report.failures);
        assert_eq!(report.threshold_rows_audited, 1);
    }

    #[test]
    fn audit_rejects_train_overlap() {
        let mut record = good_record();
        record.train_members = record.calibration_members.clone();
        let report = audit_threshold_calibration_provenance(&[record]);
        assert!(!report.all_passed);
        assert_eq!(report.failures[0].code, THRESHOLD_CALIBRATION_NONDISJOINT);
    }

    #[test]
    fn audit_rejects_tau_mismatch() {
        let mut record = good_record();
        record.tau = 0.90;
        let report = audit_threshold_calibration_provenance(&[record]);
        assert!(!report.all_passed);
        assert!(report.failures[0].detail.contains("recomputed p10"));
    }
}
