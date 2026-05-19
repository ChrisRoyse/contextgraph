//! TASK-PY-G-112 durable mistake log rows.
//!
//! A mistake row records disagreement with durable reality together with the
//! accepted label, failure-evidence, and Level-2 skill identities active at
//! prediction time. Replay can then learn from repeatable consequence patterns
//! instead of a flat panel projection.

use crate::error::{TrainerError, TrainerErrorCode};
use crate::label_bridge::{
    ability_signature_hash, accepted_label_signature_hash, membership_signature_hash,
    skill_signature_hash,
};
use crate::skill_validation;
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
const MAX_HIGHER_ABILITY_IDS: usize = 128;
const MAX_SOURCE_MEMBERSHIP_KEYS: usize = 512;
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
    #[serde(default)]
    pub active_higher_ability_ids: Vec<String>,
    #[serde(default)]
    pub source_membership_keys: Vec<String>,
    #[serde(default)]
    pub ability_signature_hash: Option<String>,
    #[serde(default)]
    pub membership_signature_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyMistakeLogRow {
    schema_version: u32,
    mistake_id: String,
    prediction_id: PredictionId,
    predicted_verdict: Verdict,
    ground_truth_verdict: Verdict,
    truth_source: MistakeTruthSource,
    code_state_key: String,
    named_failure_mode: Option<String>,
    accepted_label_ids: Vec<String>,
    active_skill_ids: Vec<String>,
    label_signature_hash: String,
    skill_signature_hash: Option<String>,
    failure_evidence_set_ids: Vec<String>,
    replay_row_key: String,
    accepted_registry_sha256: Option<String>,
    usefulness_metrics_sha256: Option<String>,
    learning_bridge_manifest_sha256: Option<String>,
    created_at_unix_ms: i64,
}

impl From<LegacyMistakeLogRow> for MistakeLogRow {
    fn from(value: LegacyMistakeLogRow) -> Self {
        Self {
            schema_version: value.schema_version,
            mistake_id: value.mistake_id,
            prediction_id: value.prediction_id,
            predicted_verdict: value.predicted_verdict,
            ground_truth_verdict: value.ground_truth_verdict,
            truth_source: value.truth_source,
            code_state_key: value.code_state_key,
            named_failure_mode: value.named_failure_mode,
            accepted_label_ids: value.accepted_label_ids,
            active_skill_ids: value.active_skill_ids,
            label_signature_hash: value.label_signature_hash,
            skill_signature_hash: value.skill_signature_hash,
            failure_evidence_set_ids: value.failure_evidence_set_ids,
            replay_row_key: value.replay_row_key,
            accepted_registry_sha256: value.accepted_registry_sha256,
            usefulness_metrics_sha256: value.usefulness_metrics_sha256,
            learning_bridge_manifest_sha256: value.learning_bridge_manifest_sha256,
            created_at_unix_ms: value.created_at_unix_ms,
            active_higher_ability_ids: Vec::new(),
            source_membership_keys: Vec::new(),
            ability_signature_hash: None,
            membership_signature_hash: None,
        }
    }
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
        validate_live_id_list(
            "accepted_label_ids",
            &self.accepted_label_ids,
            MAX_LABEL_IDS,
        )?;
        validate_id_list("active_skill_ids", &self.active_skill_ids, MAX_SKILL_IDS)?;
        validate_id_list(
            "active_higher_ability_ids",
            &self.active_higher_ability_ids,
            MAX_HIGHER_ABILITY_IDS,
        )?;
        validate_id_list(
            "source_membership_keys",
            &self.source_membership_keys,
            MAX_SOURCE_MEMBERSHIP_KEYS,
        )?;
        let expected_label_signature = accepted_label_signature_hash(&self.accepted_label_ids)?;
        if self.label_signature_hash != expected_label_signature {
            return Err(invalid(
                "label_signature_hash",
                "does not match accepted_label_ids",
            ));
        }
        let expected_skill_signature = if self.active_skill_ids.is_empty() {
            None
        } else {
            Some(skill_signature_hash(&self.active_skill_ids)?)
        };
        if self.skill_signature_hash != expected_skill_signature {
            return Err(invalid(
                "skill_signature_hash",
                "does not match active_skill_ids",
            ));
        }
        let expected_ability_signature = if self.active_higher_ability_ids.is_empty() {
            None
        } else {
            Some(ability_signature_hash(&self.active_higher_ability_ids)?)
        };
        if self.ability_signature_hash != expected_ability_signature {
            return Err(invalid(
                "ability_signature_hash",
                "does not match active_higher_ability_ids",
            ));
        }
        let expected_membership_signature = if self.source_membership_keys.is_empty() {
            None
        } else {
            Some(membership_signature_hash(&self.source_membership_keys)?)
        };
        if self.membership_signature_hash != expected_membership_signature {
            return Err(invalid(
                "membership_signature_hash",
                "does not match source_membership_keys",
            ));
        }
        validate_id_list(
            "failure_evidence_set_ids",
            &self.failure_evidence_set_ids,
            MAX_FAILURE_EVIDENCE_SET_IDS,
        )?;
        if let Some(value) = &self.skill_signature_hash {
            validate_id("skill_signature_hash", value)?;
        }
        if let Some(value) = &self.ability_signature_hash {
            validate_id("ability_signature_hash", value)?;
        }
        if let Some(value) = &self.membership_signature_hash {
            validate_id("membership_signature_hash", value)?;
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

pub fn mistake_id_from_evidence_parts(
    prediction_id: PredictionId,
    code_state_key: &str,
    label_signature_hash: &str,
    skill_signature_hash: Option<&str>,
    ability_signature_hash: Option<&str>,
    membership_signature_hash: Option<&str>,
    ground_truth_verdict: Verdict,
) -> Result<String, TrainerError> {
    if prediction_id.0 == [0_u8; 16] {
        return Err(invalid("prediction_id", "must be non-zero"));
    }
    validate_id("code_state_key", code_state_key)?;
    validate_id("label_signature_hash", label_signature_hash)?;
    for (field, value) in [
        ("skill_signature_hash", skill_signature_hash),
        ("ability_signature_hash", ability_signature_hash),
        ("membership_signature_hash", membership_signature_hash),
    ] {
        if let Some(value) = value {
            validate_id(field, value)?;
        }
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
    if let Some(value) = ability_signature_hash {
        hasher.update(value.as_bytes());
    }
    hasher.update([0]);
    if let Some(value) = membership_signature_hash {
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
    let row = decode_mistake_log_row(&bytes)?;
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
        let row = decode_mistake_log_row(&value)?;
        if key != row.mistake_id {
            return Err(invalid("mistake_log.key", "key does not match payload"));
        }
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn decode_mistake_log_row(bytes: &[u8]) -> Result<MistakeLogRow, TrainerError> {
    bincode::deserialize::<MistakeLogRow>(bytes)
        .or_else(|_| bincode::deserialize::<LegacyMistakeLogRow>(bytes).map(Into::into))
        .map_err(map_bincode_error)
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

fn validate_live_id_list(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    skill_validation::validate_live_id_list(
        "file:crates/context-graph-mejepa-train/src/mistake_log.rs",
        "mistake rows must preserve live-safe labels; target outcomes belong in truth_source",
        field,
        values,
        max_items,
    )
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
        let accepted_label_ids = vec!["ast_surface:function".to_string()];
        let active_skill_ids = vec!["skill:unit_sequence".to_string()];
        let active_higher_ability_ids = vec!["ability:boundary_sequence".to_string()];
        let source_membership_keys =
            vec!["chunk_skill::chunk:unit::python:before:unit::skill:unit_sequence".to_string()];
        let label_signature_hash =
            crate::label_bridge::accepted_label_signature_hash(&accepted_label_ids).unwrap();
        let skill_signature_hash =
            Some(crate::label_bridge::skill_signature_hash(&active_skill_ids).unwrap());
        let ability_signature_hash =
            Some(crate::label_bridge::ability_signature_hash(&active_higher_ability_ids).unwrap());
        let membership_signature_hash =
            Some(crate::label_bridge::membership_signature_hash(&source_membership_keys).unwrap());
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
            accepted_label_ids,
            active_skill_ids,
            active_higher_ability_ids,
            source_membership_keys,
            label_signature_hash,
            skill_signature_hash,
            ability_signature_hash,
            membership_signature_hash,
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

    #[test]
    fn mistake_log_rejects_signature_that_does_not_match_ids() {
        let mut row = row();
        row.label_signature_hash = "labels:wrong".to_string();

        let err = row.validate().unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn mistake_log_rejects_target_side_accepted_label() {
        let mut row = row();
        row.accepted_label_ids = vec!["oracle:fail".to_string()];
        row.label_signature_hash =
            crate::label_bridge::accepted_label_signature_hash(&row.accepted_label_ids).unwrap();

        let err = row.validate().unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
        assert!(err.to_string().contains("target-side supervision"));
    }

    #[test]
    fn evidence_mistake_id_includes_ability_and_membership_signatures() {
        let prediction_id = PredictionId([0x7b; 16]);
        let base = mistake_id_from_evidence_parts(
            prediction_id,
            "state:unit",
            "labels:abc",
            Some("skills:def"),
            Some("abilities:first"),
            Some("memberships:one"),
            Verdict::Fail,
        )
        .unwrap();
        let changed = mistake_id_from_evidence_parts(
            prediction_id,
            "state:unit",
            "labels:abc",
            Some("skills:def"),
            Some("abilities:second"),
            Some("memberships:one"),
            Verdict::Fail,
        )
        .unwrap();

        assert_ne!(base, changed);
    }

    #[test]
    fn mistake_row_deserializes_legacy_rows_with_empty_ability_context() {
        let row = row();
        let legacy = LegacyMistakeLogRow {
            schema_version: row.schema_version,
            mistake_id: row.mistake_id,
            prediction_id: row.prediction_id,
            predicted_verdict: row.predicted_verdict,
            ground_truth_verdict: row.ground_truth_verdict,
            truth_source: row.truth_source,
            code_state_key: row.code_state_key,
            named_failure_mode: row.named_failure_mode,
            accepted_label_ids: row.accepted_label_ids,
            active_skill_ids: row.active_skill_ids,
            label_signature_hash: row.label_signature_hash,
            skill_signature_hash: row.skill_signature_hash,
            failure_evidence_set_ids: row.failure_evidence_set_ids,
            replay_row_key: row.replay_row_key,
            accepted_registry_sha256: row.accepted_registry_sha256,
            usefulness_metrics_sha256: row.usefulness_metrics_sha256,
            learning_bridge_manifest_sha256: row.learning_bridge_manifest_sha256,
            created_at_unix_ms: row.created_at_unix_ms,
        };
        let bytes = bincode::serialize(&legacy).unwrap();
        let decoded = decode_mistake_log_row(&bytes).unwrap();

        decoded.validate().unwrap();
        assert!(decoded.active_higher_ability_ids.is_empty());
        assert!(decoded.source_membership_keys.is_empty());
        assert!(decoded.ability_signature_hash.is_none());
        assert!(decoded.membership_signature_hash.is_none());
    }
}
