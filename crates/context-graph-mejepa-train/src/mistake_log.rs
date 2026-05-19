//! TASK-PY-G-112 durable mistake log rows.
//!
//! A mistake row records disagreement with durable reality together with the
//! accepted label, failure-evidence, and Level-2 skill identities active at
//! prediction time. Replay can then learn from repeatable consequence patterns
//! instead of a flat panel projection.

use crate::error::{TrainerError, TrainerErrorCode};
use context_graph_mejepa::{PredictionId, Verdict};
use context_graph_mejepa_cf::CF_MEJEPA_MISTAKE_LOG;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MISTAKE_LOG_SCHEMA_VERSION: u32 = 1;
const MAX_ID_BYTES: usize = 512;
const MAX_LABEL_IDS: usize = 256;
const MAX_SKILL_IDS: usize = 128;
const MAX_FAILURE_EVIDENCE_SET_IDS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum MistakeTruthSource {
    SwebenchDockerOracle,
    PostToolUseCapture,
    ShiftLogReplay,
    OperatorOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MistakeLogRow {
    pub schema_version: u32,
    pub mistake_id: String,
    pub prediction_id: PredictionId,
    pub predicted_verdict: Verdict,
    pub ground_truth_verdict: Verdict,
    pub truth_source: MistakeTruthSource,
    pub code_state_key: String,
    pub named_failure_mode: Option<String>,
    pub accepted_label_ids: Vec<String>,
    pub active_skill_ids: Vec<String>,
    pub label_signature_hash: String,
    pub skill_signature_hash: Option<String>,
    pub failure_evidence_set_ids: Vec<String>,
    pub replay_row_key: String,
    pub accepted_registry_sha256: Option<String>,
    pub usefulness_metrics_sha256: Option<String>,
    pub learning_bridge_manifest_sha256: Option<String>,
    pub created_at_unix_ms: i64,
}

impl MistakeLogRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        if self.schema_version != MISTAKE_LOG_SCHEMA_VERSION {
            return Err(invalid(
                "schema_version",
                format!(
                    "expected schema_version {}, got {}",
                    MISTAKE_LOG_SCHEMA_VERSION, self.schema_version
                ),
            ));
        }
        validate_id("mistake_id", &self.mistake_id)?;
        if self.prediction_id.0 == [0_u8; 16] {
            return Err(invalid("prediction_id", "must be non-zero"));
        }
        if self.predicted_verdict == self.ground_truth_verdict {
            return Err(invalid(
                "verdicts",
                "mistake rows require predicted_verdict != ground_truth_verdict",
            ));
        }
        validate_id("code_state_key", &self.code_state_key)?;
        validate_id("label_signature_hash", &self.label_signature_hash)?;
        validate_id("replay_row_key", &self.replay_row_key)?;
        validate_id_list(
            "accepted_label_ids",
            &self.accepted_label_ids,
            MAX_LABEL_IDS,
        )?;
        validate_id_list("active_skill_ids", &self.active_skill_ids, MAX_SKILL_IDS)?;
        validate_id_list(
            "failure_evidence_set_ids",
            &self.failure_evidence_set_ids,
            MAX_FAILURE_EVIDENCE_SET_IDS,
        )?;
        if let Some(value) = &self.skill_signature_hash {
            validate_id("skill_signature_hash", value)?;
        }
        if let Some(value) = &self.named_failure_mode {
            validate_id("named_failure_mode", value)?;
        }
        for (field, value) in [
            ("accepted_registry_sha256", &self.accepted_registry_sha256),
            ("usefulness_metrics_sha256", &self.usefulness_metrics_sha256),
            (
                "learning_bridge_manifest_sha256",
                &self.learning_bridge_manifest_sha256,
            ),
        ] {
            if let Some(value) = value {
                validate_id(field, value)?;
            }
        }
        if self.created_at_unix_ms <= 0 {
            return Err(invalid("created_at_unix_ms", "must be positive"));
        }
        Ok(())
    }
}

pub fn mistake_id_from_parts(
    prediction_id: PredictionId,
    code_state_key: &str,
    label_signature_hash: &str,
    skill_signature_hash: Option<&str>,
    ground_truth_verdict: Verdict,
) -> Result<String, TrainerError> {
    if prediction_id.0 == [0_u8; 16] {
        return Err(invalid("prediction_id", "must be non-zero"));
    }
    validate_id("code_state_key", code_state_key)?;
    validate_id("label_signature_hash", label_signature_hash)?;
    if let Some(value) = skill_signature_hash {
        validate_id("skill_signature_hash", value)?;
    }
    let mut hasher = Sha256::new();
    hasher.update(prediction_id.0);
    hasher.update([0]);
    hasher.update(code_state_key.as_bytes());
    hasher.update([0]);
    hasher.update(label_signature_hash.as_bytes());
    hasher.update([0]);
    if let Some(value) = skill_signature_hash {
        hasher.update(value.as_bytes());
    }
    hasher.update([0]);
    hasher.update(format!("{ground_truth_verdict:?}").as_bytes());
    Ok(format!("mistake:{}", &hex::encode(hasher.finalize())[..24]))
}

pub fn write_mistake_log_row_sync_readback(
    db: &DB,
    row: &MistakeLogRow,
) -> Result<(), TrainerError> {
    row.validate()?;
    let cf = mistake_log_cf(db)?;
    let bytes = bincode::serialize(row).map_err(map_bincode_error)?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.put_cf_opt(cf, row.mistake_id.as_bytes(), bytes, &write_opts)
        .map_err(map_rocksdb_error)?;
    db.flush_cf(cf).map_err(map_rocksdb_error)?;
    let readback = read_mistake_log_row(db, &row.mistake_id)?
        .ok_or_else(|| invalid("readback", "mistake row missing after write"))?;
    if readback != *row {
        return Err(invalid(
            "readback",
            "mistake row changed during write/readback",
        ));
    }
    Ok(())
}

pub fn read_mistake_log_row(
    db: &DB,
    mistake_id: &str,
) -> Result<Option<MistakeLogRow>, TrainerError> {
    validate_id("mistake_id", mistake_id)?;
    let cf = mistake_log_cf(db)?;
    let Some(bytes) = db
        .get_cf(cf, mistake_id.as_bytes())
        .map_err(map_rocksdb_error)?
    else {
        return Ok(None);
    };
    let row: MistakeLogRow = bincode::deserialize(&bytes).map_err(map_bincode_error)?;
    row.validate()?;
    Ok(Some(row))
}

pub fn read_all_mistake_log_rows(db: &DB) -> Result<Vec<MistakeLogRow>, TrainerError> {
    let cf = mistake_log_cf(db)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| invalid("mistake_log.key", err.to_string()))?;
        let row: MistakeLogRow = bincode::deserialize(&value).map_err(map_bincode_error)?;
        if key != row.mistake_id {
            return Err(invalid("mistake_log.key", "key does not match payload"));
        }
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn mistake_log_cf(db: &DB) -> Result<&rocksdb::ColumnFamily, TrainerError> {
    db.cf_handle(CF_MEJEPA_MISTAKE_LOG)
        .ok_or_else(|| invalid("rocksdb.column_family", "missing CF_MEJEPA_MISTAKE_LOG"))
}

fn validate_id_list(field: &str, values: &[String], max_items: usize) -> Result<(), TrainerError> {
    if values.len() > max_items {
        return Err(invalid(field, format!("too many ids: {}", values.len())));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        validate_id(field, value)?;
        if !seen.insert(value) {
            return Err(invalid(field, format!("duplicate id {value}")));
        }
    }
    Ok(())
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must be non-empty"));
    }
    if value.len() > MAX_ID_BYTES || value.chars().any(char::is_control) {
        return Err(invalid(field, "must be single-line text up to 512 bytes"));
    }
    Ok(())
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field.into(),
        "file": "file:crates/context-graph-mejepa-train/src/mistake_log.rs",
        "remediation": "repair the mistake row before online learning; repeated mistakes must be keyed by label and skill evidence"
    }))
}

fn map_rocksdb_error(err: rocksdb::Error) -> TrainerError {
    invalid("rocksdb", err.to_string())
}

fn map_bincode_error(err: Box<bincode::ErrorKind>) -> TrainerError {
    invalid("bincode", err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::{ColumnFamilyDescriptor, Options};

    fn open_db(path: &std::path::Path) -> DB {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        DB::open_cf_descriptors(
            &opts,
            path,
            vec![ColumnFamilyDescriptor::new(
                CF_MEJEPA_MISTAKE_LOG,
                Options::default(),
            )],
        )
        .unwrap()
    }

    fn row() -> MistakeLogRow {
        let prediction_id = PredictionId([0x8a; 16]);
        let label_signature_hash = "labels:abc123def456".to_string();
        let skill_signature_hash = Some("skills:def456abc123".to_string());
        let mistake_id = mistake_id_from_parts(
            prediction_id,
            "python:before:unit",
            &label_signature_hash,
            skill_signature_hash.as_deref(),
            Verdict::Fail,
        )
        .unwrap();
        MistakeLogRow {
            schema_version: MISTAKE_LOG_SCHEMA_VERSION,
            mistake_id,
            prediction_id,
            predicted_verdict: Verdict::Pass,
            ground_truth_verdict: Verdict::Fail,
            truth_source: MistakeTruthSource::SwebenchDockerOracle,
            code_state_key: "python:before:unit".to_string(),
            named_failure_mode: Some("failure:multi_point".to_string()),
            accepted_label_ids: vec!["ast_surface:function".to_string()],
            active_skill_ids: vec!["skill:unit_sequence".to_string()],
            label_signature_hash,
            skill_signature_hash,
            failure_evidence_set_ids: vec!["python:before:unit".to_string()],
            replay_row_key: "python:swap:state:abc:mode:failure:labels:abc".to_string(),
            accepted_registry_sha256: Some("sha256:registry".to_string()),
            usefulness_metrics_sha256: Some("sha256:usefulness".to_string()),
            learning_bridge_manifest_sha256: Some("sha256:bridge".to_string()),
            created_at_unix_ms: 1_778_000_000_000,
        }
    }

    #[test]
    fn mistake_log_writes_and_reads_back_label_and_skill_context() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_db(temp.path());
        let row = row();

        write_mistake_log_row_sync_readback(&db, &row).unwrap();
        let readback = read_mistake_log_row(&db, &row.mistake_id).unwrap().unwrap();

        assert_eq!(readback, row);
        assert_eq!(read_all_mistake_log_rows(&db).unwrap().len(), 1);
    }

    #[test]
    fn mistake_log_rejects_non_mistake_verdict_pair() {
        let mut row = row();
        row.ground_truth_verdict = row.predicted_verdict;

        let err = row.validate().unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }
}
