use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{EventId, InstrumentId};
use crate::dynamicjepa::pair_kinds::PairKindName;
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::{ensure_finite, Validate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const INSTRUMENT_READING_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentSpec {
    pub instrument_id: InstrumentId,
    pub kind: InstrumentKind,
    pub input_fields: Vec<String>,
    pub output_shape: Vec<usize>,
    pub normalization: Normalization,
    pub required: bool,
    pub model_ref: Option<String>,
    #[serde(default)]
    pub pair_kinds: Vec<PairKindName>,
    pub version: u8,
    pub validation: InstrumentValidation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InstrumentKind {
    Scalar,
    Categorical { variants: Vec<String> },
    Onehot { dim: usize },
    Vector { dim: usize },
    Time { encoding: TimeEncoding },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeEncoding {
    FractionalDayOfWeek,
    UnixSeconds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Normalization {
    None,
    StandardScore { mean: f64, std: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentValidation {
    pub require_finite: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub reject_nan: bool,
    pub reject_inf: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentReading {
    pub header: DjRecordHeader,
    pub reading_id: Uuid,
    pub event_id: EventId,
    pub instrument_id: InstrumentId,
    pub instrument_hash: [u8; 16],
    pub input_hash: [u8; 32],
    pub output_dense: Vec<f32>,
    pub status: ReadingStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReadingStatus {
    Ok,
    Failed { error: String },
}

impl Validate for InstrumentSpec {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.instrument_id.validate()?;
        if self.version == 0 {
            return Err(DynamicJepaError::validation(
                "InstrumentSpec.version",
                "instrument version must be >= 1",
                "set version=1 for the demo instruments",
            ));
        }
        if self.input_fields.is_empty() {
            return Err(DynamicJepaError::validation(
                "InstrumentSpec.input_fields",
                "instrument must declare at least one input field",
                "add dotted input fields produced by the adapter",
            ));
        }
        if self.output_shape.is_empty() || self.output_shape.contains(&0) {
            return Err(DynamicJepaError::validation(
                "InstrumentSpec.output_shape",
                format!("invalid output shape {:?}", self.output_shape),
                "declare a strict positive output shape; wildcards are forbidden",
            ));
        }
        match &self.kind {
            InstrumentKind::Categorical { variants } => {
                if variants.is_empty() {
                    return Err(DynamicJepaError::validation(
                        "InstrumentKind::Categorical.variants",
                        "variants must not be empty",
                        "declare every allowed categorical variant",
                    ));
                }
                if self.output_shape != [variants.len()] {
                    return Err(DynamicJepaError::PanelShapeMismatch {
                        domain_pack_id: "instrument_spec".to_string(),
                        expected: vec![variants.len()],
                        actual: self.output_shape.clone(),
                    });
                }
            }
            InstrumentKind::Onehot { dim } if *dim == 0 || self.output_shape != [*dim] => {
                return Err(DynamicJepaError::PanelShapeMismatch {
                    domain_pack_id: "instrument_spec".to_string(),
                    expected: vec![*dim],
                    actual: self.output_shape.clone(),
                });
            }
            InstrumentKind::Vector { dim } if *dim == 0 || self.output_shape != [*dim] => {
                return Err(DynamicJepaError::PanelShapeMismatch {
                    domain_pack_id: "instrument_spec".to_string(),
                    expected: vec![*dim],
                    actual: self.output_shape.clone(),
                });
            }
            _ => {}
        }
        if let Normalization::StandardScore { std, .. } = self.normalization {
            if std <= 0.0 || !std.is_finite() {
                return Err(DynamicJepaError::validation(
                    "InstrumentSpec.normalization.std",
                    format!("std must be finite and > 0, got {std}"),
                    "fix normalization parameters in the domain pack",
                ));
            }
        }
        if let (Some(min), Some(max)) = (self.validation.min, self.validation.max) {
            if min > max {
                return Err(DynamicJepaError::validation(
                    "InstrumentSpec.validation",
                    format!("min {min} exceeds max {max}"),
                    "fix instrument validation bounds",
                ));
            }
        }
        if !self.pair_kinds.contains(&PairKindName::Cosine) {
            return Err(DynamicJepaError::PairwiseCosineKindMissing {
                instrument_id: self.instrument_id.to_string(),
                pair_kinds: self.pair_kinds.iter().map(ToString::to_string).collect(),
            });
        }
        let mut seen = BTreeSet::new();
        for kind in &self.pair_kinds {
            if !seen.insert(*kind) {
                return Err(DynamicJepaError::validation(
                    "InstrumentSpec.pair_kinds",
                    format!(
                        "duplicate pair kind {:?} for instrument {}",
                        kind, self.instrument_id
                    ),
                    "declare each pair kind at most once per instrument",
                ));
            }
        }
        Ok(())
    }
}

impl Validate for InstrumentReading {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.reading_id.is_nil() {
            return Err(DynamicJepaError::validation(
                "InstrumentReading.reading_id",
                "reading_id must not be nil",
                "generate a real UUID for the reading",
            ));
        }
        self.event_id.validate()?;
        self.instrument_id.validate()?;
        if matches!(self.status, ReadingStatus::Ok) && self.output_dense.is_empty() {
            return Err(DynamicJepaError::validation(
                "InstrumentReading.output_dense",
                "successful reading cannot have empty dense output",
                "write the actual instrument vector or mark the reading as Failed",
            ));
        }
        for (idx, value) in self.output_dense.iter().enumerate() {
            ensure_finite(*value, &format!("InstrumentReading.output_dense[{idx}]"))?;
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    InstrumentReading,
    INSTRUMENT_READING_RECORD_VERSION,
    "InstrumentReading"
);
