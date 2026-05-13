//! TASK-PREDICT-012 — Operator override prediction tool.
//!
//! Persists operator-supplied overrides for individual predictions
//! into `CF_MEJEPA_OPERATOR_OVERRIDES`, keyed by prediction-id. The
//! Training callers read this CF as an operator-override flag and apply a
//! `6.0x` sampling-weight multiplier to affected chunks for the next batch
//! (per `TECH-PREDICT §13`).

use context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES;
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::eval::{ActiveLearningLabel, LabelMethod};
use crate::types::{OracleOutcome, PredictionId, TaskId};

pub const OPERATOR_OVERRIDE_SAMPLING_WEIGHT: f32 = 6.0;
const MAX_REASON_BYTES: usize = 4096;
const MAX_OPERATOR_ID_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum OverrideVerdict {
    Pass,
    Fail,
    Abstain,
    OutOfDistribution,
}

impl OverrideVerdict {
    pub fn oracle_outcome(self) -> OracleOutcome {
        match self {
            Self::Pass => OracleOutcome::Pass,
            Self::Fail => OracleOutcome::Fail,
            Self::Abstain => OracleOutcome::Abstain,
            Self::OutOfDistribution => OracleOutcome::OutOfDistribution,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorOverride {
    pub prediction_id: [u8; 16],
    pub override_verdict: OverrideVerdict,
    pub reason: String,
    pub operator_id: String,
    pub created_at_unix_ms: i64,
    pub sampling_weight_multiplier: f32,
}

impl OperatorOverride {
    pub fn new(
        prediction_id: PredictionId,
        override_verdict: OverrideVerdict,
        reason: String,
        operator_id: String,
        created_at_unix_ms: i64,
    ) -> Result<Self, OperatorOverrideError> {
        let record = Self {
            prediction_id: prediction_id.0,
            override_verdict,
            reason,
            operator_id,
            created_at_unix_ms,
            sampling_weight_multiplier: OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
        };
        validate(&record)?;
        Ok(record)
    }

    pub fn prediction_id(&self) -> PredictionId {
        PredictionId(self.prediction_id)
    }

    pub fn active_learning_label(&self, task_id: TaskId) -> ActiveLearningLabel {
        ActiveLearningLabel {
            task_id,
            oracle_outcome: self.override_verdict.oracle_outcome(),
            method: LabelMethod::Human,
            labeled_at_unix_ms: self.created_at_unix_ms,
        }
    }
}

#[derive(Debug, Error)]
pub enum OperatorOverrideError {
    #[error("MEJEPA_OPERATOR_OVERRIDE_PREDICTION_ID_ZERO: prediction_id must be non-zero")]
    PredictionIdZero,
    #[error("MEJEPA_OPERATOR_OVERRIDE_REASON_EMPTY: reason must be non-empty")]
    ReasonEmpty,
    #[error("MEJEPA_OPERATOR_OVERRIDE_REASON_TOO_LONG: reason capped at 4096 bytes")]
    ReasonTooLong,
    #[error("MEJEPA_OPERATOR_OVERRIDE_REASON_INVALID: reason must contain no control characters")]
    ReasonInvalid,
    #[error("MEJEPA_OPERATOR_OVERRIDE_OPERATOR_EMPTY: operator_id must be non-empty")]
    OperatorEmpty,
    #[error("MEJEPA_OPERATOR_OVERRIDE_OPERATOR_INVALID: operator_id must be single-line text up to 256 bytes")]
    OperatorInvalid,
    #[error("MEJEPA_OPERATOR_OVERRIDE_CREATED_AT_INVALID: created_at_unix_ms must be positive")]
    CreatedAtInvalid,
    #[error(
        "MEJEPA_OPERATOR_OVERRIDE_WEIGHT_INVALID: sampling_weight_multiplier must be exactly 6.0"
    )]
    SamplingWeightInvalid,
    #[error("MEJEPA_OPERATOR_OVERRIDE_CF_MISSING: CF_MEJEPA_OPERATOR_OVERRIDES not present")]
    CfMissing,
    #[error("MEJEPA_OPERATOR_OVERRIDE_SERIALIZE: {0}")]
    Serialize(String),
    #[error("MEJEPA_OPERATOR_OVERRIDE_WRITE: {0}")]
    Write(String),
    #[error("MEJEPA_OPERATOR_OVERRIDE_READBACK_MISSING: row absent after write")]
    ReadbackMissing,
    #[error("MEJEPA_OPERATOR_OVERRIDE_READBACK_MISMATCH: row contents differ from input")]
    ReadbackMismatch,
}

/// Persist an operator override and verify it via read-after-write.
pub fn persist_operator_override(
    db: &DB,
    override_record: &OperatorOverride,
) -> Result<(), OperatorOverrideError> {
    validate(override_record)?;
    let cf = db
        .cf_handle(CF_MEJEPA_OPERATOR_OVERRIDES)
        .ok_or(OperatorOverrideError::CfMissing)?;
    let bytes = bincode::serialize(override_record)
        .map_err(|err| OperatorOverrideError::Serialize(err.to_string()))?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.put_cf_opt(cf, override_record.prediction_id, &bytes, &write_opts)
        .map_err(|err| OperatorOverrideError::Write(err.to_string()))?;
    db.flush_cf(cf)
        .map_err(|err| OperatorOverrideError::Write(err.to_string()))?;
    let readback = db
        .get_cf(cf, override_record.prediction_id)
        .map_err(|err| OperatorOverrideError::Write(err.to_string()))?
        .ok_or(OperatorOverrideError::ReadbackMissing)?;
    let decoded: OperatorOverride = bincode::deserialize(&readback)
        .map_err(|err| OperatorOverrideError::Serialize(err.to_string()))?;
    if decoded != *override_record {
        return Err(OperatorOverrideError::ReadbackMismatch);
    }
    Ok(())
}

/// Read a previously-persisted operator override by prediction-id.
pub fn load_operator_override(
    db: &DB,
    prediction_id: PredictionId,
) -> Result<Option<OperatorOverride>, OperatorOverrideError> {
    if prediction_id.0 == [0_u8; 16] {
        return Err(OperatorOverrideError::PredictionIdZero);
    }
    let cf = db
        .cf_handle(CF_MEJEPA_OPERATOR_OVERRIDES)
        .ok_or(OperatorOverrideError::CfMissing)?;
    let raw = db
        .get_cf(cf, prediction_id.0)
        .map_err(|err| OperatorOverrideError::Write(err.to_string()))?;
    raw.map(|bytes| bincode::deserialize(&bytes))
        .transpose()
        .map_err(|err| OperatorOverrideError::Serialize(err.to_string()))
}

pub fn operator_override_flags_for_predictions(
    db: &DB,
    prediction_ids: &[PredictionId],
) -> Result<Vec<bool>, OperatorOverrideError> {
    prediction_ids
        .iter()
        .map(|prediction_id| {
            load_operator_override(db, *prediction_id).map(|record| record.is_some())
        })
        .collect()
}

pub fn count_operator_overrides(db: &DB) -> Result<u64, OperatorOverrideError> {
    let cf = db
        .cf_handle(CF_MEJEPA_OPERATOR_OVERRIDES)
        .ok_or(OperatorOverrideError::CfMissing)?;
    let mut count = 0_u64;
    for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
        item.map_err(|err| OperatorOverrideError::Write(err.to_string()))?;
        count += 1;
    }
    Ok(count)
}

fn validate(record: &OperatorOverride) -> Result<(), OperatorOverrideError> {
    if record.prediction_id == [0_u8; 16] {
        return Err(OperatorOverrideError::PredictionIdZero);
    }
    if record.reason.trim().is_empty() {
        return Err(OperatorOverrideError::ReasonEmpty);
    }
    if record.reason.len() > MAX_REASON_BYTES {
        return Err(OperatorOverrideError::ReasonTooLong);
    }
    if record.reason.chars().any(char::is_control) {
        return Err(OperatorOverrideError::ReasonInvalid);
    }
    if record.operator_id.trim().is_empty() {
        return Err(OperatorOverrideError::OperatorEmpty);
    }
    if record.operator_id.len() > MAX_OPERATOR_ID_BYTES
        || record.operator_id.chars().any(char::is_control)
    {
        return Err(OperatorOverrideError::OperatorInvalid);
    }
    if record.created_at_unix_ms <= 0 {
        return Err(OperatorOverrideError::CreatedAtInvalid);
    }
    if !record.sampling_weight_multiplier.is_finite()
        || (record.sampling_weight_multiplier - OPERATOR_OVERRIDE_SAMPLING_WEIGHT).abs()
            > f32::EPSILON
    {
        return Err(OperatorOverrideError::SamplingWeightInvalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(reason: &str, operator: &str) -> OperatorOverride {
        OperatorOverride {
            prediction_id: [0x42; 16],
            override_verdict: OverrideVerdict::Fail,
            reason: reason.to_string(),
            operator_id: operator.to_string(),
            created_at_unix_ms: 1_700_000_000_000,
            sampling_weight_multiplier: OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
        }
    }

    #[test]
    fn empty_reason_rejected() {
        let err = validate(&record("  ", "op")).unwrap_err();
        assert!(matches!(err, OperatorOverrideError::ReasonEmpty));
    }

    #[test]
    fn empty_operator_rejected() {
        let err = validate(&record("real reason", "")).unwrap_err();
        assert!(matches!(err, OperatorOverrideError::OperatorEmpty));
    }

    #[test]
    fn long_reason_rejected() {
        let huge = "x".repeat(4097);
        let err = validate(&record(&huge, "op")).unwrap_err();
        assert!(matches!(err, OperatorOverrideError::ReasonTooLong));
    }

    #[test]
    fn control_character_reason_rejected() {
        let err = validate(&record("bad\nreason", "op")).unwrap_err();
        assert!(matches!(err, OperatorOverrideError::ReasonInvalid));
    }

    #[test]
    fn valid_record_passes() {
        assert!(validate(&record("operator marked this as fail", "op-1")).is_ok());
    }

    #[test]
    fn sampling_weight_constant_is_six() {
        assert_eq!(OPERATOR_OVERRIDE_SAMPLING_WEIGHT, 6.0);
    }

    #[test]
    fn persist_load_count_and_flags_read_real_rocksdb_source_of_truth() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = crate::open_infer_rocksdb(temp.path()).unwrap();
        let record = OperatorOverride::new(
            PredictionId([0x51; 16]),
            OverrideVerdict::Fail,
            "operator marked prediction wrong".to_string(),
            "operator-1".to_string(),
            1_770_000_000_000,
        )
        .unwrap();
        persist_operator_override(db.as_ref(), &record).unwrap();
        assert_eq!(
            load_operator_override(db.as_ref(), record.prediction_id())
                .unwrap()
                .unwrap(),
            record
        );
        assert_eq!(count_operator_overrides(db.as_ref()).unwrap(), 1);
        assert_eq!(
            operator_override_flags_for_predictions(
                db.as_ref(),
                &[record.prediction_id(), PredictionId([0x52; 16])]
            )
            .unwrap(),
            vec![true, false]
        );
    }
}
