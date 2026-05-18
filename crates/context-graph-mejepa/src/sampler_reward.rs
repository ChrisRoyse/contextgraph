//! TASK-PY-G-053 — online observe reward signals for training sampler pressure.
//!
//! Rows are keyed by `prediction_id` in `CF_MEJEPA_SAMPLER_REWARDS`. Valid
//! oracle-derived rows are applied by the training sampler; awaiting and
//! quarantine rows are durable audit states and must not influence weights.

use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::types::{OracleOutcome, PredictionId, RealityPrediction};

pub const SAMPLER_REWARD_SIGNAL_SCHEMA_VERSION: u32 = 1;
pub const SAMPLER_REWARD_MAX_SURPRISE_Z: f32 = 5.0;
pub const SAMPLER_REWARD_AWAITING_ORACLE_CODE: &str = "AWAITING_ORACLE";
pub const SAMPLER_REWARD_CLAMPED_EXTREME_SURPRISE_CODE: &str = "CLAMPED_EXTREME_SURPRISE";
pub const SAMPLER_REWARD_NON_FINITE_SURPRISE_CODE: &str =
    "MEJEPA_SAMPLER_REWARD_NON_FINITE_SURPRISE";

const MAX_CELL_ID_BYTES: usize = 512;
const MAX_SOURCE_ID_BYTES: usize = 512;
const MULTIPLIER_TOLERANCE: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SamplerRewardStatus {
    Ready,
    AwaitingOracle,
    ClampedExtremeSurprise,
    QuarantinedNonFinite,
}

impl SamplerRewardStatus {
    pub fn code(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::AwaitingOracle => SAMPLER_REWARD_AWAITING_ORACLE_CODE,
            Self::ClampedExtremeSurprise => SAMPLER_REWARD_CLAMPED_EXTREME_SURPRISE_CODE,
            Self::QuarantinedNonFinite => SAMPLER_REWARD_NON_FINITE_SURPRISE_CODE,
        }
    }

    pub fn is_apply_ready(self) -> bool {
        matches!(self, Self::Ready | Self::ClampedExtremeSurprise)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SamplerRewardWriteDisposition {
    Inserted,
    Updated,
    Idempotent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SamplerRewardWriteReadback {
    pub disposition: SamplerRewardWriteDisposition,
    pub row: SamplerRewardSignal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SamplerRewardSignal {
    pub schema_version: u32,
    pub prediction_id: PredictionId,
    pub surprise_z: f32,
    pub sampling_weight_multiplier: f32,
    pub cell_id: String,
    pub oracle_observed_at_unix_ms: i64,
    pub prediction_created_at_unix_ms: i64,
    pub source_event_id: String,
    pub status: SamplerRewardStatus,
    pub error_code: Option<String>,
    pub source_prediction_cf: String,
    pub source_oracle_cf: String,
    pub source_reward_cf: String,
}

impl SamplerRewardSignal {
    pub fn is_apply_ready(&self) -> bool {
        self.status.is_apply_ready()
    }

    pub fn status_code(&self) -> &'static str {
        self.status.code()
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != SAMPLER_REWARD_SIGNAL_SCHEMA_VERSION {
            return invalid(
                "sampler_reward.schema_version",
                format!(
                    "expected {}; got {}",
                    SAMPLER_REWARD_SIGNAL_SCHEMA_VERSION, self.schema_version
                ),
            );
        }
        if self.prediction_id.0 == [0_u8; 16] {
            return invalid(
                "sampler_reward.prediction_id",
                "prediction_id must be non-zero",
            );
        }
        if !self.surprise_z.is_finite() || self.surprise_z < 0.0 {
            return invalid(
                "sampler_reward.surprise_z",
                format!(
                    "surprise_z must be finite and non-negative; got {}",
                    self.surprise_z
                ),
            );
        }
        if !self.sampling_weight_multiplier.is_finite() || self.sampling_weight_multiplier < 0.0 {
            return invalid(
                "sampler_reward.sampling_weight_multiplier",
                format!(
                    "sampling_weight_multiplier must be finite and non-negative; got {}",
                    self.sampling_weight_multiplier
                ),
            );
        }
        validate_bounded_id("sampler_reward.cell_id", &self.cell_id, MAX_CELL_ID_BYTES)?;
        validate_bounded_id(
            "sampler_reward.source_event_id",
            &self.source_event_id,
            MAX_SOURCE_ID_BYTES,
        )?;
        validate_bounded_id(
            "sampler_reward.source_prediction_cf",
            &self.source_prediction_cf,
            MAX_SOURCE_ID_BYTES,
        )?;
        validate_bounded_id(
            "sampler_reward.source_oracle_cf",
            &self.source_oracle_cf,
            MAX_SOURCE_ID_BYTES,
        )?;
        validate_bounded_id(
            "sampler_reward.source_reward_cf",
            &self.source_reward_cf,
            MAX_SOURCE_ID_BYTES,
        )?;
        if self.oracle_observed_at_unix_ms < 0 {
            return invalid(
                "sampler_reward.oracle_observed_at_unix_ms",
                "timestamp must be non-negative",
            );
        }
        if self.prediction_created_at_unix_ms < 0 {
            return invalid(
                "sampler_reward.prediction_created_at_unix_ms",
                "timestamp must be non-negative",
            );
        }
        match self.status {
            SamplerRewardStatus::Ready => {
                if self.error_code.is_some() {
                    return invalid(
                        "sampler_reward.error_code",
                        "ready rows must not carry an error_code",
                    );
                }
                validate_expected_multiplier(self)?;
            }
            SamplerRewardStatus::ClampedExtremeSurprise => {
                validate_expected_error_code(self, SAMPLER_REWARD_CLAMPED_EXTREME_SURPRISE_CODE)?;
                validate_expected_multiplier(self)?;
            }
            SamplerRewardStatus::AwaitingOracle => {
                validate_expected_error_code(self, SAMPLER_REWARD_AWAITING_ORACLE_CODE)?;
                validate_exact_multiplier(self, 1.0)?;
            }
            SamplerRewardStatus::QuarantinedNonFinite => {
                validate_expected_error_code(self, SAMPLER_REWARD_NON_FINITE_SURPRISE_CODE)?;
                validate_exact_multiplier(self, 1.0)?;
                if self.surprise_z != 0.0 {
                    return invalid(
                        "sampler_reward.surprise_z",
                        "quarantined non-finite rows must store sanitized surprise_z=0",
                    );
                }
            }
        }
        Ok(())
    }
}

pub fn sampler_reward_from_prediction_outcome(
    prediction: &RealityPrediction,
    observed_outcome: OracleOutcome,
    source_event_id: impl Into<String>,
    oracle_observed_at_unix_ms: i64,
) -> Result<SamplerRewardSignal, MejepaInferError> {
    prediction.validate()?;
    let Some(actual_pass) = hard_oracle_pass_value(observed_outcome) else {
        return sampler_reward_awaiting_oracle(
            prediction,
            source_event_id,
            oracle_observed_at_unix_ms,
        );
    };
    let surprise_z = (prediction.predicted_oracle_pass - actual_pass).abs() * 4.0;
    sampler_reward_from_parts(
        PredictionId(prediction.prediction_id),
        surprise_z,
        sampler_reward_cell_id(prediction),
        prediction.created_at_unix_ms,
        oracle_observed_at_unix_ms,
        source_event_id,
    )
}

pub fn sampler_reward_awaiting_oracle(
    prediction: &RealityPrediction,
    source_event_id: impl Into<String>,
    observed_at_unix_ms: i64,
) -> Result<SamplerRewardSignal, MejepaInferError> {
    prediction.validate()?;
    let row = SamplerRewardSignal {
        schema_version: SAMPLER_REWARD_SIGNAL_SCHEMA_VERSION,
        prediction_id: PredictionId(prediction.prediction_id),
        surprise_z: 0.0,
        sampling_weight_multiplier: 1.0,
        cell_id: sampler_reward_cell_id(prediction),
        oracle_observed_at_unix_ms: observed_at_unix_ms.max(0),
        prediction_created_at_unix_ms: prediction.created_at_unix_ms,
        source_event_id: source_event_id.into(),
        status: SamplerRewardStatus::AwaitingOracle,
        error_code: Some(SAMPLER_REWARD_AWAITING_ORACLE_CODE.to_string()),
        source_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        source_oracle_cf: context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS.to_string(),
        source_reward_cf: context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS.to_string(),
    };
    row.validate()?;
    Ok(row)
}

pub fn sampler_reward_from_parts(
    prediction_id: PredictionId,
    surprise_z: f32,
    cell_id: impl Into<String>,
    prediction_created_at_unix_ms: i64,
    oracle_observed_at_unix_ms: i64,
    source_event_id: impl Into<String>,
) -> Result<SamplerRewardSignal, MejepaInferError> {
    if prediction_id.0 == [0_u8; 16] {
        return invalid(
            "sampler_reward.prediction_id",
            "prediction_id must be non-zero",
        );
    }
    let (sanitized_surprise_z, multiplier, status, error_code) = if surprise_z.is_finite() {
        if surprise_z < 0.0 {
            return invalid(
                "sampler_reward.surprise_z",
                format!("surprise_z must be non-negative; got {surprise_z}"),
            );
        }
        let clamped = surprise_z.clamp(0.0, SAMPLER_REWARD_MAX_SURPRISE_Z);
        let status = if surprise_z > SAMPLER_REWARD_MAX_SURPRISE_Z {
            SamplerRewardStatus::ClampedExtremeSurprise
        } else {
            SamplerRewardStatus::Ready
        };
        let error_code = if status == SamplerRewardStatus::ClampedExtremeSurprise {
            Some(SAMPLER_REWARD_CLAMPED_EXTREME_SURPRISE_CODE.to_string())
        } else {
            None
        };
        (surprise_z, 1.0 + clamped, status, error_code)
    } else {
        (
            0.0,
            1.0,
            SamplerRewardStatus::QuarantinedNonFinite,
            Some(SAMPLER_REWARD_NON_FINITE_SURPRISE_CODE.to_string()),
        )
    };
    let row = SamplerRewardSignal {
        schema_version: SAMPLER_REWARD_SIGNAL_SCHEMA_VERSION,
        prediction_id,
        surprise_z: sanitized_surprise_z,
        sampling_weight_multiplier: multiplier,
        cell_id: cell_id.into(),
        oracle_observed_at_unix_ms: oracle_observed_at_unix_ms.max(0),
        prediction_created_at_unix_ms: prediction_created_at_unix_ms.max(0),
        source_event_id: source_event_id.into(),
        status,
        error_code,
        source_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        source_oracle_cf: context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS.to_string(),
        source_reward_cf: context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS.to_string(),
    };
    row.validate()?;
    Ok(row)
}

pub fn persist_sampler_reward_signal_readback(
    db: &DB,
    row: &SamplerRewardSignal,
) -> Result<SamplerRewardWriteReadback, MejepaInferError> {
    row.validate()?;
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS)?;
    let key = sampler_reward_key(row.prediction_id);
    let existing = db
        .get_cf(cf, key)?
        .map(|raw| decode_sampler_reward_signal(&raw))
        .transpose()?;
    let had_existing = existing.is_some();
    if let Some(existing) = existing {
        existing.validate()?;
        if should_keep_existing_sampler_reward(&existing, row) {
            return Ok(SamplerRewardWriteReadback {
                disposition: SamplerRewardWriteDisposition::Idempotent,
                row: existing,
            });
        }
    }
    let value = bincode::serialize(row)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &value, &opts)?;
    db.flush_wal(true)?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "sampler_reward.readback".to_string(),
            detail: "missing CF_MEJEPA_SAMPLER_REWARDS row after write".to_string(),
        })?;
    if readback != value {
        return invalid(
            "sampler_reward.readback",
            "CF_MEJEPA_SAMPLER_REWARDS readback bytes differ",
        );
    }
    let decoded = decode_sampler_reward_signal(&readback)?;
    if decoded != *row {
        return invalid(
            "sampler_reward.readback",
            "CF_MEJEPA_SAMPLER_REWARDS decoded readback differs",
        );
    }
    Ok(SamplerRewardWriteReadback {
        disposition: if had_existing {
            SamplerRewardWriteDisposition::Updated
        } else {
            SamplerRewardWriteDisposition::Inserted
        },
        row: decoded,
    })
}

pub fn read_sampler_reward_signal(
    db: &DB,
    prediction_id: PredictionId,
) -> Result<Option<SamplerRewardSignal>, MejepaInferError> {
    if prediction_id.0 == [0_u8; 16] {
        return invalid(
            "sampler_reward.prediction_id",
            "prediction_id must be non-zero",
        );
    }
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS)?;
    db.get_cf(cf, sampler_reward_key(prediction_id))?
        .map(|raw| decode_sampler_reward_signal(&raw))
        .transpose()
}

pub fn read_all_sampler_reward_signals(
    db: &DB,
) -> Result<Vec<SamplerRewardSignal>, MejepaInferError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        let row = decode_sampler_reward_signal(&value)?;
        if key.as_ref() != sampler_reward_key(row.prediction_id) {
            return invalid(
                "sampler_reward.key",
                "CF_MEJEPA_SAMPLER_REWARDS key does not match payload prediction_id",
            );
        }
        rows.push(row);
    }
    Ok(rows)
}

pub fn count_sampler_reward_signals(db: &DB) -> Result<u64, MejepaInferError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS)?;
    let mut count = 0;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

pub fn sampler_reward_key(prediction_id: PredictionId) -> [u8; 16] {
    prediction_id.0
}

fn decode_sampler_reward_signal(raw: &[u8]) -> Result<SamplerRewardSignal, MejepaInferError> {
    let row: SamplerRewardSignal = bincode::deserialize(raw)?;
    row.validate()?;
    Ok(row)
}

fn hard_oracle_pass_value(outcome: OracleOutcome) -> Option<f32> {
    match outcome {
        OracleOutcome::Pass => Some(1.0),
        OracleOutcome::Fail => Some(0.0),
        OracleOutcome::OutOfDistribution | OracleOutcome::Abstain => None,
    }
}

fn sampler_reward_cell_id(prediction: &RealityPrediction) -> String {
    if let Some(matched) = &prediction.matched_fingerprint {
        format!("fingerprint:{}", hex::encode(matched.fingerprint_id))
    } else {
        format!("{}::{:?}", prediction.task_id.0.trim(), prediction.language).to_ascii_lowercase()
    }
}

fn should_keep_existing_sampler_reward(
    existing: &SamplerRewardSignal,
    incoming: &SamplerRewardSignal,
) -> bool {
    if existing.prediction_id != incoming.prediction_id {
        return false;
    }
    if existing == incoming {
        return true;
    }
    if existing.is_apply_ready() && incoming.is_apply_ready() {
        return existing.oracle_observed_at_unix_ms >= incoming.oracle_observed_at_unix_ms;
    }
    if existing.is_apply_ready() && !incoming.is_apply_ready() {
        return true;
    }
    !existing.is_apply_ready()
        && !incoming.is_apply_ready()
        && existing.oracle_observed_at_unix_ms >= incoming.oracle_observed_at_unix_ms
}

fn validate_expected_multiplier(row: &SamplerRewardSignal) -> Result<(), MejepaInferError> {
    let expected = 1.0 + row.surprise_z.clamp(0.0, SAMPLER_REWARD_MAX_SURPRISE_Z);
    validate_exact_multiplier(row, expected)
}

fn validate_exact_multiplier(
    row: &SamplerRewardSignal,
    expected: f32,
) -> Result<(), MejepaInferError> {
    if (row.sampling_weight_multiplier - expected).abs() > MULTIPLIER_TOLERANCE {
        return invalid(
            "sampler_reward.sampling_weight_multiplier",
            format!(
                "expected {expected}; got {}",
                row.sampling_weight_multiplier
            ),
        );
    }
    Ok(())
}

fn validate_expected_error_code(
    row: &SamplerRewardSignal,
    expected: &str,
) -> Result<(), MejepaInferError> {
    match row.error_code.as_deref() {
        Some(actual) if actual == expected => Ok(()),
        actual => invalid(
            "sampler_reward.error_code",
            format!("expected {expected}; got {actual:?}"),
        ),
    }
}

fn validate_bounded_id(field: &str, value: &str, max_bytes: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > max_bytes {
        return invalid(field, format!("exceeds {max_bytes} bytes"));
    }
    if value.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return invalid(field, "contains a control character");
    }
    Ok(())
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConformalInterval, ConformalSet, Language, PredictionProvenance, RealityPredictionBuilder,
        TaskId, Verdict,
    };

    fn prediction() -> RealityPrediction {
        RealityPredictionBuilder::from_parts(
            TaskId("sampler-reward-test".to_string()),
            [0x53; 16],
            Language::Python,
            ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
        )
        .prediction_id([0x54; 16])
        .covered_chunks(vec![crate::ChunkId("src/lib.py#fn#demo".to_string())])
        .verdict(Verdict::Pass)
        .confidence_interval(ConformalInterval::default())
        .predicted_oracle_pass(0.8)
        .predicted_test_pass(vec![0.8])
        .predicted_runtime_trace([0.0; 32])
        .ood_score(0.1)
        .calibrated_confidence(0.8)
        .provenance(PredictionProvenance::default())
        .calibration_version("sampler-reward-test")
        .created_at_unix_ms(1_779_000_000_000)
        .build()
        .unwrap()
    }

    #[test]
    fn contradicted_pass_prediction_yields_expected_multiplier() {
        let row = sampler_reward_from_prediction_outcome(
            &prediction(),
            OracleOutcome::Fail,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1_779_000_000_100,
        )
        .unwrap();
        assert_eq!(row.status, SamplerRewardStatus::Ready);
        assert!((row.surprise_z - 3.2).abs() < 1e-6);
        assert!((row.sampling_weight_multiplier - 4.2).abs() < 1e-6);
        assert_eq!(row.error_code, None);
    }

    #[test]
    fn non_finite_surprise_is_quarantined() {
        let row = sampler_reward_from_parts(
            PredictionId([0x55; 16]),
            f32::NAN,
            "python::quarantine",
            1,
            2,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap();
        assert_eq!(row.status, SamplerRewardStatus::QuarantinedNonFinite);
        assert_eq!(
            row.error_code.as_deref(),
            Some(SAMPLER_REWARD_NON_FINITE_SURPRISE_CODE)
        );
        assert_eq!(row.sampling_weight_multiplier, 1.0);
    }

    #[test]
    fn extreme_surprise_clamps_multiplier() {
        let row = sampler_reward_from_parts(
            PredictionId([0x56; 16]),
            12.0,
            "python::extreme",
            1,
            2,
            "cccccccccccccccccccccccccccccccc",
        )
        .unwrap();
        assert_eq!(row.status, SamplerRewardStatus::ClampedExtremeSurprise);
        assert_eq!(row.sampling_weight_multiplier, 6.0);
    }
}
