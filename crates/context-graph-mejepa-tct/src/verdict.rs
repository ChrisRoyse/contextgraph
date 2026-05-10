use serde::{Deserialize, Serialize};

use crate::error::TctError;
use crate::types::{ChunkId, GtauViolation, MutationCategory};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictorOutput {
    pub predicted_class: MutationCategory,
    pub per_test_probabilities: Vec<f32>,
    pub confidence_interval: (f32, f32),
    pub latent_density: f32,
}

impl PredictorOutput {
    pub fn try_new(
        predicted_class: MutationCategory,
        per_test_probabilities: Vec<f32>,
        confidence_interval: (f32, f32),
        latent_density: f32,
    ) -> Result<Self, TctError> {
        if per_test_probabilities.is_empty() {
            return Err(TctError::InsufficientSamples {
                cell: "PredictorOutput.per_test_probabilities".to_string(),
                observed: 0,
                required: 1,
            });
        }
        for (idx, value) in per_test_probabilities.iter().enumerate() {
            if !value.is_finite() || !(0.0..=1.0).contains(value) {
                return Err(TctError::invalid(
                    "PredictorOutput.per_test_probabilities",
                    format!("probability[{idx}] must be finite in [0,1], got {value}"),
                ));
            }
        }
        let (lo, hi) = confidence_interval;
        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return Err(TctError::invalid(
                "PredictorOutput.confidence_interval",
                format!("invalid confidence interval ({lo}, {hi})"),
            ));
        }
        if !latent_density.is_finite() || latent_density < 0.0 {
            return Err(TctError::invalid(
                "PredictorOutput.latent_density",
                format!("latent_density must be finite and non-negative, got {latent_density}"),
            ));
        }
        Ok(Self {
            predicted_class,
            per_test_probabilities,
            confidence_interval,
            latent_density,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictGuardRejected {
    pub violating_embedders: Vec<GtauViolation>,
    pub predictor_predicted: PredictorOutput,
    pub violating_chunks: Vec<ChunkId>,
    pub constellation_version_id: [u8; 32],
}

impl VerdictGuardRejected {
    pub fn try_new(
        violating_embedders: Vec<GtauViolation>,
        predictor_predicted: PredictorOutput,
        violating_chunks: Vec<ChunkId>,
        constellation_version_id: [u8; 32],
    ) -> Result<Self, TctError> {
        if violating_embedders.is_empty() {
            return Err(TctError::ConstellationViolation {
                detail: "GuardRejected requires at least one violating embedder".to_string(),
            });
        }
        Ok(Self {
            violating_embedders,
            predictor_predicted,
            violating_chunks,
            constellation_version_id,
        })
    }
}
