use std::collections::BTreeMap;

use context_graph_mejepa_instruments::InstrumentSlot;
use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;
use crate::types::{EmbedderId, Language};

pub fn complete_per_slot_sigma_squared(sigma_squared: f32) -> BTreeMap<InstrumentSlot, f32> {
    InstrumentSlot::all()
        .into_iter()
        .map(|slot| (slot, sigma_squared))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationRecord {
    pub version: String,
    pub alpha: f32,
    pub target_coverage: f32,
    pub tau: f32,
    pub sigma_squared: f32,
    pub empirical_coverage: f32,
    pub min_samples_per_stratum: usize,
    pub sample_count: usize,
    pub per_language_counts: BTreeMap<Language, usize>,
    pub per_slot_sigma_squared: Option<BTreeMap<InstrumentSlot, f32>>,
    pub corpus_sha: [u8; 32],
    pub embedder_versions: BTreeMap<EmbedderId, String>,
    pub frozen_at: i64,
}

impl CalibrationRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.version.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "version".to_string(),
                detail: "calibration version must be non-empty".to_string(),
            });
        }
        for (field, value) in [
            ("alpha", self.alpha),
            ("target_coverage", self.target_coverage),
            ("tau", self.tau),
            ("sigma_squared", self.sigma_squared),
            ("empirical_coverage", self.empirical_coverage),
        ] {
            if !value.is_finite() {
                return Err(MejepaInferError::NanDetected {
                    nan_source: field.to_string(),
                    detail: format!("{field} is non-finite: {value}"),
                });
            }
        }
        if self.alpha <= 0.0 || self.alpha >= 1.0 {
            return Err(MejepaInferError::InvalidInput {
                field: "alpha".to_string(),
                detail: format!("alpha must be in (0, 1), got {}", self.alpha),
            });
        }
        if !(0.0..=1.0).contains(&self.target_coverage) {
            return Err(MejepaInferError::InvalidInput {
                field: "target_coverage".to_string(),
                detail: format!(
                    "target_coverage must be in [0, 1], got {}",
                    self.target_coverage
                ),
            });
        }
        if (self.target_coverage - (1.0 - self.alpha)).abs() > 1e-4 {
            return Err(MejepaInferError::InvalidInput {
                field: "target_coverage".to_string(),
                detail: format!(
                    "target_coverage {} must equal 1 - alpha {}",
                    self.target_coverage, self.alpha
                ),
            });
        }
        if !(0.0..=1.0).contains(&self.tau) {
            return Err(MejepaInferError::InvalidInput {
                field: "tau".to_string(),
                detail: format!("tau must be in [0, 1], got {}", self.tau),
            });
        }
        if self.sigma_squared <= 0.0 {
            return Err(MejepaInferError::InvalidInput {
                field: "sigma_squared".to_string(),
                detail: format!("sigma_squared must be > 0, got {}", self.sigma_squared),
            });
        }
        if !(0.0..=1.0).contains(&self.empirical_coverage) {
            return Err(MejepaInferError::InvalidInput {
                field: "empirical_coverage".to_string(),
                detail: format!(
                    "empirical_coverage must be in [0, 1], got {}",
                    self.empirical_coverage
                ),
            });
        }
        if self.sample_count == 0 {
            return Err(MejepaInferError::ConformalInsufficientSamples {
                language: None,
                expected: 1,
                actual: 0,
            });
        }
        let counted = self.per_language_counts.values().sum::<usize>();
        if counted != self.sample_count {
            return Err(MejepaInferError::InvalidInput {
                field: "per_language_counts".to_string(),
                detail: format!(
                    "per-language count sum {counted} does not match sample_count {}",
                    self.sample_count
                ),
            });
        }
        for (language, count) in &self.per_language_counts {
            if *count == 0 {
                return Err(MejepaInferError::InvalidInput {
                    field: "per_language_counts".to_string(),
                    detail: format!("language {language:?} has zero samples"),
                });
            }
            if *count < self.min_samples_per_stratum {
                return Err(MejepaInferError::ConformalInsufficientSamples {
                    language: Some(format!("{language:?}")),
                    expected: self.min_samples_per_stratum,
                    actual: *count,
                });
            }
        }
        if let Some(per_slot_sigma_squared) = &self.per_slot_sigma_squared {
            if per_slot_sigma_squared.len() != InstrumentSlot::all().len() {
                return Err(MejepaInferError::OodPerSlotCalibratorMissing {
                    detail: format!(
                        "per-slot sigma calibration must cover all {} instrument slots; got {}",
                        InstrumentSlot::all().len(),
                        per_slot_sigma_squared.len()
                    ),
                });
            }
            for (slot, sigma_squared) in per_slot_sigma_squared {
                if !sigma_squared.is_finite() || *sigma_squared <= 0.0 {
                    return Err(MejepaInferError::InvalidInput {
                        field: "per_slot_sigma_squared".to_string(),
                        detail: format!(
                            "slot {slot:?} sigma_squared must be finite and > 0; got {sigma_squared}"
                        ),
                    });
                }
            }
        }
        if self.frozen_at <= 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "frozen_at".to_string(),
                detail: format!(
                    "frozen_at must be a positive UNIX timestamp, got {}",
                    self.frozen_at
                ),
            });
        }
        for (embedder, version) in &self.embedder_versions {
            embedder.validate("embedder_versions.key")?;
            if version.trim().is_empty() {
                return Err(MejepaInferError::InvalidInput {
                    field: "embedder_versions".to_string(),
                    detail: "embedder version values must be non-empty".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceBundle {
    pub panel_sha: [u8; 32],
    pub embedder_versions: BTreeMap<EmbedderId, String>,
    pub calibration_version: Option<String>,
    pub training_cert_chain_offset: Option<u64>,
    pub constellation_version: Option<String>,
    pub code_version: String,
    pub provenance_warnings: Vec<String>,
}
