use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::{DType, Tensor};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const UTML_LATENT_ENTROPY_DEGENERATE: &str = "UTML_LATENT_ENTROPY_DEGENERATE";
const DEFAULT_K: usize = 3;
const DEFAULT_MIN_PAIRWISE_DISTANCE: f32 = 1.0e-6;
const DEFAULT_BRIER_REGRESSION_TOLERANCE: f32 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LatentEntropyConfig {
    pub lambda: f32,
    pub k: usize,
    pub min_pairwise_distance: f32,
}

impl Default for LatentEntropyConfig {
    fn default() -> Self {
        Self {
            lambda: 0.0,
            k: DEFAULT_K,
            min_pairwise_distance: DEFAULT_MIN_PAIRWISE_DISTANCE,
        }
    }
}

impl LatentEntropyConfig {
    pub fn validate(&self) -> Result<(), TrainerError> {
        if !self.lambda.is_finite() || self.lambda < 0.0 {
            return Err(config_error(
                "latent_entropy.lambda",
                "must be finite and non-negative",
            ));
        }
        if self.k == 0 {
            return Err(config_error("latent_entropy.k", "must be positive"));
        }
        if !self.min_pairwise_distance.is_finite() || self.min_pairwise_distance <= 0.0 {
            return Err(config_error(
                "latent_entropy.min_pairwise_distance",
                "must be positive finite",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LatentEntropyEstimate {
    pub entropy_nats: f32,
    pub batch_size: usize,
    pub latent_dim: usize,
    pub k_effective: usize,
    pub mean_knn_radius: f32,
    pub active_dimensions: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LatentEntropyLossReport {
    pub enabled: bool,
    pub lambda: f32,
    pub entropy_nats: f32,
    pub weighted_loss: f32,
    pub estimate: Option<LatentEntropyEstimate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EntropyLambdaScheduler {
    pub base_lambda: f32,
    pub brier_regression_tolerance: f32,
}

impl Default for EntropyLambdaScheduler {
    fn default() -> Self {
        Self {
            base_lambda: 1.0e-4,
            brier_regression_tolerance: DEFAULT_BRIER_REGRESSION_TOLERANCE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EntropyLambdaDecision {
    pub effective_lambda: f32,
    pub baseline_brier: f32,
    pub enabled_brier: f32,
    pub tolerance: f32,
    pub regressed: bool,
}

impl EntropyLambdaScheduler {
    pub fn decide(
        &self,
        baseline_brier: f32,
        enabled_brier: f32,
    ) -> Result<EntropyLambdaDecision, TrainerError> {
        if !self.base_lambda.is_finite() || self.base_lambda < 0.0 {
            return Err(config_error(
                "entropy_lambda_scheduler.base_lambda",
                "must be finite and non-negative",
            ));
        }
        if !self.brier_regression_tolerance.is_finite() || self.brier_regression_tolerance < 0.0 {
            return Err(config_error(
                "entropy_lambda_scheduler.brier_regression_tolerance",
                "must be finite and non-negative",
            ));
        }
        if !baseline_brier.is_finite() || !(0.0..=1.0).contains(&baseline_brier) {
            return Err(config_error(
                "entropy_lambda_scheduler.baseline_brier",
                "must be finite in [0,1]",
            ));
        }
        if !enabled_brier.is_finite() || !(0.0..=1.0).contains(&enabled_brier) {
            return Err(config_error(
                "entropy_lambda_scheduler.enabled_brier",
                "must be finite in [0,1]",
            ));
        }
        let regressed = enabled_brier > baseline_brier + self.brier_regression_tolerance;
        Ok(EntropyLambdaDecision {
            effective_lambda: if regressed { 0.0 } else { self.base_lambda },
            baseline_brier,
            enabled_brier,
            tolerance: self.brier_regression_tolerance,
            regressed,
        })
    }
}

pub fn latent_entropy_loss(
    latent: &Tensor,
    config: LatentEntropyConfig,
) -> Result<(Tensor, LatentEntropyLossReport), TrainerError> {
    config.validate()?;
    if config.lambda == 0.0 {
        let zero = Tensor::new(0f32, latent.device())?;
        return Ok((
            zero,
            LatentEntropyLossReport {
                enabled: false,
                lambda: 0.0,
                entropy_nats: 0.0,
                weighted_loss: 0.0,
                estimate: None,
            },
        ));
    }
    let estimate = estimate_latent_entropy_nats(latent, config.k, config.min_pairwise_distance)?;
    let loss = estimate.entropy_nats;
    let tensor = Tensor::new(loss, latent.device())?;
    Ok((
        tensor,
        LatentEntropyLossReport {
            enabled: true,
            lambda: config.lambda,
            entropy_nats: loss,
            weighted_loss: loss * config.lambda,
            estimate: Some(estimate),
        },
    ))
}

pub fn estimate_latent_entropy_nats(
    latent: &Tensor,
    k: usize,
    min_pairwise_distance: f32,
) -> Result<LatentEntropyEstimate, TrainerError> {
    if k == 0 {
        return Err(config_error("latent_entropy.k", "must be positive"));
    }
    if !min_pairwise_distance.is_finite() || min_pairwise_distance <= 0.0 {
        return Err(config_error(
            "latent_entropy.min_pairwise_distance",
            "must be positive finite",
        ));
    }
    let dims = latent.dims();
    if dims.len() != 2 {
        return Err(config_error(
            "latent_entropy.latent",
            "must be a rank-2 (batch, dim) tensor",
        ));
    }
    let batch = dims[0];
    let dim = dims[1];
    if batch < 2 || dim == 0 {
        return Err(degenerate_error(
            "latent entropy requires at least two rows and one latent dimension",
            batch,
            dim,
            0,
            0.0,
        ));
    }
    let values = latent
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "latent_entropy latent tensor contains NaN or Inf",
        ));
    }
    let active_dimensions = active_dimension_count(&values, batch, dim, min_pairwise_distance);
    if active_dimensions < dim.min(2) {
        return Err(degenerate_error(
            "latent batch is constant or effectively one-dimensional",
            batch,
            dim,
            active_dimensions,
            0.0,
        ));
    }

    let k_effective = k.min(batch - 1);
    let mut radii = Vec::with_capacity(batch);
    for row in 0..batch {
        let mut distances = Vec::with_capacity(batch - 1);
        for other in 0..batch {
            if row == other {
                continue;
            }
            distances.push(euclidean_distance(&values, dim, row, other));
        }
        distances.sort_by(|a, b| a.total_cmp(b));
        radii.push(distances[k_effective - 1]);
    }
    let max_radius = radii.iter().copied().fold(0.0_f32, f32::max);
    if max_radius <= min_pairwise_distance {
        return Err(degenerate_error(
            "latent pairwise radius collapsed below entropy floor",
            batch,
            dim,
            active_dimensions,
            max_radius,
        ));
    }
    let mean_radius = radii.iter().sum::<f32>() / radii.len() as f32;
    let mean_log_radius = radii
        .iter()
        .map(|radius| (radius + min_pairwise_distance).ln())
        .sum::<f32>()
        / radii.len() as f32;
    let entropy_nats =
        (mean_log_radius + (batch as f32).ln() + (active_dimensions as f32).ln()).max(0.0);
    if !entropy_nats.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "latent entropy estimate is non-finite",
        ));
    }
    Ok(LatentEntropyEstimate {
        entropy_nats,
        batch_size: batch,
        latent_dim: dim,
        k_effective,
        mean_knn_radius: mean_radius,
        active_dimensions,
    })
}

fn active_dimension_count(values: &[f32], batch: usize, dim: usize, min_distance: f32) -> usize {
    let variance_floor = min_distance * min_distance;
    (0..dim)
        .filter(|col| {
            let mean = (0..batch).map(|row| values[row * dim + *col]).sum::<f32>() / batch as f32;
            let variance = (0..batch)
                .map(|row| {
                    let delta = values[row * dim + *col] - mean;
                    delta * delta
                })
                .sum::<f32>()
                / batch as f32;
            variance > variance_floor
        })
        .count()
}

fn euclidean_distance(values: &[f32], dim: usize, a: usize, b: usize) -> f32 {
    let mut sum = 0.0_f32;
    for col in 0..dim {
        let delta = values[a * dim + col] - values[b * dim + col];
        sum += delta * delta;
    }
    sum.sqrt()
}

fn config_error(field: &'static str, detail: &'static str) -> TrainerError {
    TrainerError::new(
        TrainerErrorCode::MejepaTrainConfigInvalid,
        format!("{field}: {detail}"),
    )
}

fn degenerate_error(
    detail: &'static str,
    batch: usize,
    dim: usize,
    active_dimensions: usize,
    max_radius: f32,
) -> TrainerError {
    TrainerError::new(TrainerErrorCode::UtmlLatentEntropyDegenerate, detail).with_context(json!({
        "code": UTML_LATENT_ENTROPY_DEGENERATE,
        "batch": batch,
        "latent_dim": dim,
        "active_dimensions": active_dimensions,
        "max_radius": max_radius,
        "remediation": "disable lambda_entropy for this batch or provide a non-collapsed slot-preserving latent z"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};

    #[test]
    fn entropy_decreases_with_compression() {
        let device = Device::Cpu;
        let wide = Tensor::from_slice(
            &[0.0f32, 0.0, 4.0, 1.0, 8.0, 4.0, 12.0, 9.0],
            (4, 2),
            &device,
        )
        .unwrap();
        let tight = Tensor::from_slice(
            &[0.0f32, 0.0, 1.0, 0.25, 2.0, 1.0, 3.0, 2.25],
            (4, 2),
            &device,
        )
        .unwrap();
        let wide_entropy = estimate_latent_entropy_nats(&wide, 3, 1e-6)
            .unwrap()
            .entropy_nats;
        let tight_entropy = estimate_latent_entropy_nats(&tight, 3, 1e-6)
            .unwrap()
            .entropy_nats;
        assert!(wide_entropy > tight_entropy);
    }

    #[test]
    fn degenerate_latent_fails_closed() {
        let device = Device::Cpu;
        let latent = Tensor::zeros((4, 2), DType::F32, &device).unwrap();
        let err = estimate_latent_entropy_nats(&latent, 3, 1e-6).unwrap_err();
        assert_eq!(err.code(), UTML_LATENT_ENTROPY_DEGENERATE);
    }

    #[test]
    fn scheduler_disables_regressing_lambda() {
        let scheduler = EntropyLambdaScheduler::default();
        let decision = scheduler.decide(0.10, 0.11).unwrap();
        assert!(decision.regressed);
        assert_eq!(decision.effective_lambda, 0.0);
        let promoted = scheduler.decide(0.10, 0.09).unwrap();
        assert!(!promoted.regressed);
        assert_eq!(promoted.effective_lambda, scheduler.base_lambda);
    }
}
