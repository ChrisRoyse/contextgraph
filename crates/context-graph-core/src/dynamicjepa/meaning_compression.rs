use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{
    ConstellationId, DomainPackId, EventId, InstrumentId, PairwiseReadingId, ThresholdCalibrationId,
};
use crate::dynamicjepa::instrument::ReadingStatus;
use crate::dynamicjepa::pair_kinds::PairKindBitset;
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::{ensure_finite, Validate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PAIRWISE_READING_RECORD_VERSION: u8 = 1;
pub const CONSTELLATION_CENTROID_RECORD_VERSION: u8 = 1;
pub const THRESHOLD_CALIBRATION_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairwiseReading {
    pub header: DjRecordHeader,
    pub pairwise_id: PairwiseReadingId,
    pub event_id: EventId,
    pub instrument_j: InstrumentId,
    pub instrument_k: InstrumentId,
    pub instrument_j_artifact_hash: [u8; 32],
    pub instrument_k_artifact_hash: [u8; 32],
    pub kinds_emitted: PairKindBitset,
    pub cosine_agreement: f32,
    #[serde(default)]
    pub rank_disagreement: Option<f32>,
    #[serde(default)]
    pub modality_contradiction: Option<bool>,
    #[serde(default)]
    pub sparse_dense_mismatch: Option<f32>,
    #[serde(default)]
    pub temporal_surprise: Option<f32>,
    #[serde(default)]
    pub causal_direction_disagreement: Option<bool>,
    #[serde(default)]
    pub safety_proximity: Option<f32>,
    pub created_at_unix_ms: u64,
    pub validation_status: ReadingStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstellationCentroid {
    pub header: DjRecordHeader,
    pub constellation_id: ConstellationId,
    pub domain_pack_id: DomainPackId,
    pub subject_id: String,
    pub modality_id: InstrumentId,
    pub centroid: Vec<f32>,
    pub instrument_artifact_hash: [u8; 32],
    pub reference_set_count: u32,
    pub kept_count: u32,
    pub dropped_zero_norm: u32,
    pub loo_stability: f32,
    pub calibration_percentile: u8,
    pub calibration_set_size: u32,
    pub built_at_unix_ms: u64,
    pub built_by_run_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThresholdCalibration {
    pub header: DjRecordHeader,
    pub calibration_id: ThresholdCalibrationId,
    pub domain_pack_id: DomainPackId,
    pub subject_id: String,
    pub modality_id: InstrumentId,
    pub tau: f32,
    pub percentile: u8,
    pub calibration_set_count: u32,
    pub calibration_event_uuids_sample: Vec<EventId>,
    pub calibration_set_disjoint_proof: bool,
    pub calibration_min: f32,
    pub calibration_max: f32,
    pub calibration_p10: f32,
    pub supersede_seq: u32,
    pub supersedes_uuid: Option<ThresholdCalibrationId>,
    pub reason: Option<String>,
    pub calibrated_at_unix_ms: u64,
}

impl PairwiseReading {
    pub fn emitted_kinds_summary(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        out.push((
            "cosine_agreement".to_string(),
            format!("{:.4}", self.cosine_agreement),
        ));
        push_optional_summary(&mut out, "rank_disagreement", self.rank_disagreement);
        if let Some(value) = self.modality_contradiction {
            out.push(("modality_contradiction".to_string(), value.to_string()));
        }
        push_optional_summary(
            &mut out,
            "sparse_dense_mismatch",
            self.sparse_dense_mismatch,
        );
        push_optional_summary(&mut out, "temporal_surprise", self.temporal_surprise);
        if let Some(value) = self.causal_direction_disagreement {
            out.push((
                "causal_direction_disagreement".to_string(),
                value.to_string(),
            ));
        }
        push_optional_summary(&mut out, "safety_proximity", self.safety_proximity);
        out
    }
}

impl Validate for PairwiseReading {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.pairwise_id.validate()?;
        self.event_id.validate()?;
        self.instrument_j.validate()?;
        self.instrument_k.validate()?;
        if self.instrument_j == self.instrument_k {
            return Err(DynamicJepaError::PairwiseAsymmetricOrdering {
                instrument_j: self.instrument_j.to_string(),
                instrument_k: self.instrument_k.to_string(),
            });
        }
        if matches!(self.validation_status, ReadingStatus::Failed { .. }) {
            return Err(DynamicJepaError::validation(
                "PairwiseReading.instrument_order",
                "failed pairwise readings are not persisted in v2 compute-on-write",
                "fix the underlying pairwise computation and rerun materialization",
            ));
        }
        ensure_nonzero_hash(
            self.instrument_j_artifact_hash,
            "PairwiseReading.instrument_j_artifact_hash",
        )?;
        ensure_nonzero_hash(
            self.instrument_k_artifact_hash,
            "PairwiseReading.instrument_k_artifact_hash",
        )?;
        self.kinds_emitted.validate()?;
        ensure_finite(self.cosine_agreement, "PairwiseReading.cosine_agreement")?;
        if !(-1.0..=1.0).contains(&self.cosine_agreement) {
            return Err(DynamicJepaError::validation(
                "PairwiseReading.cosine_agreement",
                format!(
                    "cosine_agreement must be in [-1,1], got {}",
                    self.cosine_agreement
                ),
                "normalize vectors and compute cosine before writing the pairwise feature",
            ));
        }
        validate_optional_f32(
            self.rank_disagreement,
            PairKindBitset::RANK_DISAGREEMENT,
            self.kinds_emitted,
            "PairwiseReading.rank_disagreement",
        )?;
        validate_optional_bool(
            self.modality_contradiction,
            PairKindBitset::MODALITY_CONTRADICTION,
            self.kinds_emitted,
            "PairwiseReading.modality_contradiction",
        )?;
        validate_optional_f32(
            self.sparse_dense_mismatch,
            PairKindBitset::SPARSE_DENSE_MISMATCH,
            self.kinds_emitted,
            "PairwiseReading.sparse_dense_mismatch",
        )?;
        validate_optional_f32(
            self.temporal_surprise,
            PairKindBitset::TEMPORAL_SURPRISE,
            self.kinds_emitted,
            "PairwiseReading.temporal_surprise",
        )?;
        validate_optional_bool(
            self.causal_direction_disagreement,
            PairKindBitset::CAUSAL_DIRECTION,
            self.kinds_emitted,
            "PairwiseReading.causal_direction_disagreement",
        )?;
        validate_optional_f32(
            self.safety_proximity,
            PairKindBitset::SAFETY_PROXIMITY,
            self.kinds_emitted,
            "PairwiseReading.safety_proximity",
        )?;
        Ok(())
    }
}

impl Validate for ConstellationCentroid {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.constellation_id.validate()?;
        self.domain_pack_id.validate()?;
        if self.header.domain_pack_id != self.domain_pack_id {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.domain_pack_id",
                "payload domain_pack_id must match header.domain_pack_id",
                "copy the same domain pack id into both header and payload before persistence",
            ));
        }
        validate_subject_id(&self.subject_id, "ConstellationCentroid.subject_id")?;
        self.modality_id.validate()?;
        if self.centroid.is_empty() {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.centroid",
                "centroid must not be empty",
                "build constellations only after at least one finite reading survives filtering",
            ));
        }
        let mut norm_sq = 0.0f32;
        for (idx, value) in self.centroid.iter().enumerate() {
            ensure_finite(*value, &format!("ConstellationCentroid.centroid[{idx}]"))?;
            norm_sq += value * value;
        }
        let norm = norm_sq.sqrt();
        if (norm - 1.0).abs() > 1e-3 {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.centroid",
                format!("centroid L2 norm must be near 1.0, got {norm}"),
                "L2-normalize the centroid after averaging non-zero reference vectors",
            ));
        }
        ensure_nonzero_hash(
            self.instrument_artifact_hash,
            "ConstellationCentroid.instrument_artifact_hash",
        )?;
        if self.reference_set_count == 0 || self.kept_count == 0 {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.counts",
                "reference_set_count and kept_count must be positive",
                "do not persist empty constellations",
            ));
        }
        if self.kept_count + self.dropped_zero_norm > self.reference_set_count {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.counts",
                "kept_count + dropped_zero_norm exceeds reference_set_count",
                "derive constellation counts from one immutable reference set",
            ));
        }
        ensure_finite(self.loo_stability, "ConstellationCentroid.loo_stability")?;
        if !(0.0..=1.0).contains(&self.loo_stability) {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.loo_stability",
                format!("loo_stability must be in [0,1], got {}", self.loo_stability),
                "store cosine stability as a bounded score",
            ));
        }
        if self.calibration_percentile > 100 {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.calibration_percentile",
                format!(
                    "percentile must be <= 100, got {}",
                    self.calibration_percentile
                ),
                "use a percentile in [0,100]",
            ));
        }
        if self.calibration_set_size == 0 {
            return Err(DynamicJepaError::validation(
                "ConstellationCentroid.calibration_set_size",
                "calibration_set_size must be positive",
                "calibrate against a non-empty set before persisting the constellation",
            ));
        }
        if let Some(run_id) = self.built_by_run_id {
            if run_id.is_nil() {
                return Err(DynamicJepaError::validation(
                    "ConstellationCentroid.built_by_run_id",
                    "built_by_run_id must not be nil",
                    "write None or a concrete build run UUID",
                ));
            }
        }
        Ok(())
    }
}

impl Validate for ThresholdCalibration {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.calibration_id.validate()?;
        self.domain_pack_id.validate()?;
        if self.header.domain_pack_id != self.domain_pack_id {
            return Err(DynamicJepaError::validation(
                "ThresholdCalibration.domain_pack_id",
                "payload domain_pack_id must match header.domain_pack_id",
                "copy the same domain pack id into both header and payload before persistence",
            ));
        }
        validate_subject_id(&self.subject_id, "ThresholdCalibration.subject_id")?;
        self.modality_id.validate()?;
        for (field, value) in [
            ("ThresholdCalibration.tau", self.tau),
            ("ThresholdCalibration.calibration_min", self.calibration_min),
            ("ThresholdCalibration.calibration_max", self.calibration_max),
            ("ThresholdCalibration.calibration_p10", self.calibration_p10),
        ] {
            ensure_finite(value, field)?;
        }
        if self.percentile > 100 {
            return Err(DynamicJepaError::validation(
                "ThresholdCalibration.percentile",
                format!("percentile must be <= 100, got {}", self.percentile),
                "use a percentile in [0,100]",
            ));
        }
        if self.calibration_set_count == 0 {
            return Err(DynamicJepaError::validation(
                "ThresholdCalibration.calibration_set_count",
                "calibration_set_count must be positive",
                "calibrate from a non-empty held-out set",
            ));
        }
        if !self.calibration_set_disjoint_proof {
            return Err(DynamicJepaError::validation(
                "ThresholdCalibration.calibration_set_disjoint_proof",
                "calibration set must be proven disjoint from training/evaluation inputs",
                "write the disjointness proof before persisting threshold calibration",
            ));
        }
        if self.calibration_min > self.calibration_p10
            || self.calibration_p10 > self.calibration_max
        {
            return Err(DynamicJepaError::validation(
                "ThresholdCalibration.calibration_bounds",
                "expected calibration_min <= calibration_p10 <= calibration_max",
                "sort and verify calibration cosine samples before deriving tau",
            ));
        }
        for id in &self.calibration_event_uuids_sample {
            id.validate()?;
        }
        if let Some(supersedes_uuid) = self.supersedes_uuid {
            supersedes_uuid.validate()?;
            if supersedes_uuid == self.calibration_id {
                return Err(DynamicJepaError::validation(
                    "ThresholdCalibration.supersedes_uuid",
                    "calibration row cannot supersede itself",
                    "write the previous calibration UUID or None",
                ));
            }
        }
        if self.supersede_seq > 0 && self.supersedes_uuid.is_none() {
            return Err(DynamicJepaError::validation(
                "ThresholdCalibration.supersedes_uuid",
                "supersede_seq > 0 requires supersedes_uuid",
                "link recalibrations to the prior threshold row",
            ));
        }
        if let Some(reason) = &self.reason {
            if reason.trim().is_empty() {
                return Err(DynamicJepaError::validation(
                    "ThresholdCalibration.reason",
                    "reason must not be blank when present",
                    "write None or a concrete supersede reason",
                ));
            }
        }
        Ok(())
    }
}

fn push_optional_summary(out: &mut Vec<(String, String)>, name: &str, value: Option<f32>) {
    if let Some(value) = value {
        out.push((name.to_string(), format!("{value:.4}")));
    }
}

fn ensure_nonzero_hash(hash: [u8; 32], field: &str) -> DynamicJepaResult<()> {
    if hash == [0; 32] {
        return Err(DynamicJepaError::validation(
            field,
            "artifact hash must not be all zero",
            "compute and persist the SHA-256 of the frozen artifact",
        ));
    }
    Ok(())
}

fn validate_optional_f32(
    value: Option<f32>,
    bit: u8,
    bitset: PairKindBitset,
    field: &str,
) -> DynamicJepaResult<()> {
    if let Some(value) = value {
        ensure_finite(value, field)?;
        if !bitset.has(bit) {
            return Err(DynamicJepaError::validation(
                field,
                "optional value is present but its PairKindBitset bit is not set",
                "keep kinds_emitted and optional pairwise feature fields in sync",
            ));
        }
    } else if bitset.has(bit) {
        return Err(DynamicJepaError::validation(
            field,
            "PairKindBitset bit is set but the optional value is missing",
            "write the feature value or clear the bit before persistence",
        ));
    }
    Ok(())
}

fn validate_optional_bool(
    value: Option<bool>,
    bit: u8,
    bitset: PairKindBitset,
    field: &str,
) -> DynamicJepaResult<()> {
    if value.is_some() && !bitset.has(bit) {
        return Err(DynamicJepaError::validation(
            field,
            "optional value is present but its PairKindBitset bit is not set",
            "keep kinds_emitted and optional pairwise feature fields in sync",
        ));
    }
    if value.is_none() && bitset.has(bit) {
        return Err(DynamicJepaError::validation(
            field,
            "PairKindBitset bit is set but the optional value is missing",
            "write the feature value or clear the bit before persistence",
        ));
    }
    Ok(())
}

fn validate_subject_id(value: &str, field: &str) -> DynamicJepaResult<()> {
    if value.trim().is_empty() {
        return Err(DynamicJepaError::validation(
            field,
            "subject_id must not be blank",
            "write the stable constellation subject key",
        ));
    }
    if value.len() > 256 {
        return Err(DynamicJepaError::validation(
            field,
            format!("subject_id must be <= 256 bytes, got {}", value.len()),
            "hash long external identifiers before writing constellation records",
        ));
    }
    Ok(())
}

crate::impl_dynamic_jepa_record!(
    PairwiseReading,
    PAIRWISE_READING_RECORD_VERSION,
    "PairwiseReading"
);
crate::impl_dynamic_jepa_record!(
    ConstellationCentroid,
    CONSTELLATION_CENTROID_RECORD_VERSION,
    "ConstellationCentroid"
);
crate::impl_dynamic_jepa_record!(
    ThresholdCalibration,
    THRESHOLD_CALIBRATION_RECORD_VERSION,
    "ThresholdCalibration"
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamicjepa::{DomainPackId, DynamicJepaRecord};

    const TS: i64 = 1_700_000_000_000;

    fn domain_id() -> DomainPackId {
        DomainPackId::new("counter_world").unwrap()
    }

    fn header(record_id: Uuid, version: u8) -> DjRecordHeader {
        DjRecordHeader::new(record_id, version, domain_id(), "1.0.0", TS, None)
    }

    fn seal<R: DynamicJepaRecord>(mut record: R) -> R {
        record.refresh_content_hash().unwrap();
        record
    }

    fn uuid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn pairwise_reading_roundtrips_and_validates_minimal_cosine_row() {
        let record = seal(PairwiseReading {
            header: header(uuid(1), PAIRWISE_READING_RECORD_VERSION),
            pairwise_id: PairwiseReadingId(uuid(2)),
            event_id: EventId(uuid(3)),
            instrument_j: InstrumentId::new("alpha_sensor").unwrap(),
            instrument_k: InstrumentId::new("beta_sensor").unwrap(),
            instrument_j_artifact_hash: [1; 32],
            instrument_k_artifact_hash: [2; 32],
            kinds_emitted: PairKindBitset(PairKindBitset::COSINE_AGREEMENT),
            cosine_agreement: 0.42,
            rank_disagreement: None,
            modality_contradiction: None,
            sparse_dense_mismatch: None,
            temporal_surprise: None,
            causal_direction_disagreement: None,
            safety_proximity: None,
            created_at_unix_ms: TS as u64,
            validation_status: ReadingStatus::Ok,
        });
        record.validate_record().unwrap();
        let json = serde_json::to_vec(&record).unwrap();
        let decoded: PairwiseReading = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.cosine_agreement, 0.42);
    }

    #[test]
    fn pairwise_reading_rejects_duplicate_instruments() {
        let record = PairwiseReading {
            header: header(uuid(1), PAIRWISE_READING_RECORD_VERSION),
            pairwise_id: PairwiseReadingId(uuid(2)),
            event_id: EventId(uuid(3)),
            instrument_j: InstrumentId::new("alpha_sensor").unwrap(),
            instrument_k: InstrumentId::new("alpha_sensor").unwrap(),
            instrument_j_artifact_hash: [1; 32],
            instrument_k_artifact_hash: [2; 32],
            kinds_emitted: PairKindBitset(PairKindBitset::COSINE_AGREEMENT),
            cosine_agreement: 0.42,
            rank_disagreement: None,
            modality_contradiction: None,
            sparse_dense_mismatch: None,
            temporal_surprise: None,
            causal_direction_disagreement: None,
            safety_proximity: None,
            created_at_unix_ms: TS as u64,
            validation_status: ReadingStatus::Ok,
        };
        assert!(record.validate().is_err());
    }

    #[test]
    fn constellation_rejects_non_unit_centroid() {
        let record = ConstellationCentroid {
            header: header(uuid(4), CONSTELLATION_CENTROID_RECORD_VERSION),
            constellation_id: ConstellationId(uuid(5)),
            domain_pack_id: domain_id(),
            subject_id: "subject.alpha".to_string(),
            modality_id: InstrumentId::new("alpha_sensor").unwrap(),
            centroid: vec![2.0, 0.0],
            instrument_artifact_hash: [3; 32],
            reference_set_count: 4,
            kept_count: 4,
            dropped_zero_norm: 0,
            loo_stability: 0.9,
            calibration_percentile: 10,
            calibration_set_size: 4,
            built_at_unix_ms: TS as u64,
            built_by_run_id: None,
        };
        assert!(record.validate().is_err());
    }

    #[test]
    fn threshold_calibration_rejects_missing_disjoint_proof() {
        let record = ThresholdCalibration {
            header: header(uuid(6), THRESHOLD_CALIBRATION_RECORD_VERSION),
            calibration_id: ThresholdCalibrationId(uuid(7)),
            domain_pack_id: domain_id(),
            subject_id: "subject.alpha".to_string(),
            modality_id: InstrumentId::new("alpha_sensor").unwrap(),
            tau: 0.7,
            percentile: 10,
            calibration_set_count: 5,
            calibration_event_uuids_sample: vec![EventId(uuid(8))],
            calibration_set_disjoint_proof: false,
            calibration_min: 0.1,
            calibration_max: 0.9,
            calibration_p10: 0.2,
            supersede_seq: 0,
            supersedes_uuid: None,
            reason: None,
            calibrated_at_unix_ms: TS as u64,
        };
        assert!(record.validate().is_err());
    }
}
