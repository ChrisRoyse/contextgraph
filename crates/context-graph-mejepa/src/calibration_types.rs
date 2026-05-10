use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;
use crate::types::{EmbedderId, Language};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationRecord {
    pub version: String,
    pub alpha: f32,
    pub tau: f32,
    pub sigma_squared: f32,
    pub empirical_coverage: f32,
    pub min_samples_per_stratum: usize,
    pub sample_count: usize,
    pub per_language_counts: BTreeMap<Language, usize>,
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
