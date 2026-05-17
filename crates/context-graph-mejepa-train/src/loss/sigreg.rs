use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::{DType, Tensor};
use context_graph_mejepa_corpus::prng::SplitMix64;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SigregConfig {
    pub num_slices: usize,
    pub num_points: usize,
    pub t_min: f32,
    pub t_max: f32,
    pub projection_seed: u64,
}

impl Default for SigregConfig {
    fn default() -> Self {
        Self {
            num_slices: 16,
            num_points: 17,
            t_min: -5.0,
            t_max: 5.0,
            projection_seed: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SigregStats {
    pub loss: f32,
    pub batch: usize,
    pub dim: usize,
    pub num_slices: usize,
    pub num_points: usize,
    pub min_slice_stat: f32,
    pub max_slice_stat: f32,
    pub mean_slice_stat: f32,
}

pub fn sigreg_loss(predicted: &Tensor, config: SigregConfig) -> Result<Tensor, TrainerError> {
    let per_slice = sigreg_slice_stats_tensor(predicted, config)?;
    Ok(per_slice.mean_all()?)
}

pub fn sigreg_stats(predicted: &Tensor, config: SigregConfig) -> Result<SigregStats, TrainerError> {
    let per_slice = sigreg_slice_stats_tensor(predicted, config)?;
    let slice_stats = per_slice.to_vec1::<f32>()?;
    let (min_slice_stat, max_slice_stat, mean_slice_stat) = summarize(&slice_stats)?;

    let dims = predicted.dims();
    Ok(SigregStats {
        loss: mean_slice_stat,
        batch: dims[0],
        dim: dims[1],
        num_slices: config.num_slices,
        num_points: config.num_points,
        min_slice_stat,
        max_slice_stat,
        mean_slice_stat,
    })
}

fn sigreg_slice_stats_tensor(
    predicted: &Tensor,
    config: SigregConfig,
) -> Result<Tensor, TrainerError> {
    validate_config(config)?;
    validate_rank2(predicted)?;
    let x = predicted.to_dtype(DType::F32)?;
    let rows = x.to_vec2::<f32>()?;
    validate_values(&rows)?;

    let batch = rows.len();
    let dim = rows[0].len();
    let t_values = integration_points(config);
    let directions = direction_matrix(config, dim)?;
    let direction_tensor =
        Tensor::from_slice(&directions, (dim, config.num_slices), predicted.device())?;
    let projected = x.matmul(&direction_tensor)?;
    let mut weighted_errors = Vec::with_capacity(t_values.len());
    for &t in &t_values {
        let target_cf = (-0.5 * t * t).exp();
        let phase = projected.affine(t as f64, 0.0)?;
        let real = phase.cos()?.mean(0)?;
        let imag = phase.sin()?.mean(0)?;
        let real_error = real.affine(1.0, -(target_cf as f64))?.sqr()?;
        let imag_error = imag.sqr()?;
        let error = (&real_error + &imag_error)?;
        weighted_errors.push(error.affine(target_cf as f64, 0.0)?);
    }
    let mut integral = Tensor::zeros((config.num_slices,), DType::F32, predicted.device())?;
    for idx in 0..weighted_errors.len() - 1 {
        let dt = t_values[idx + 1] - t_values[idx];
        let pair_sum = (&weighted_errors[idx] + &weighted_errors[idx + 1])?;
        let area = pair_sum.affine((0.5 * dt) as f64, 0.0)?;
        integral = (&integral + &area)?;
    }
    Ok(integral.affine(batch as f64, 0.0)?)
}

fn validate_config(config: SigregConfig) -> Result<(), TrainerError> {
    if config.num_slices == 0 {
        return config_error("num_slices", "must be positive");
    }
    if config.num_points < 2 {
        return config_error("num_points", "must be at least 2");
    }
    if !config.t_min.is_finite() || !config.t_max.is_finite() || config.t_min >= config.t_max {
        return config_error("t_range", "must be finite with t_min < t_max");
    }
    Ok(())
}

fn validate_rank2(tensor: &Tensor) -> Result<(), TrainerError> {
    let dims = tensor.dims();
    if dims.len() != 2 || dims[0] < 2 || dims[1] == 0 {
        return config_error(
            "sigreg_input",
            format!("expects rank-2 tensor with batch>=2, got {dims:?}"),
        );
    }
    Ok(())
}

fn validate_values(rows: &[Vec<f32>]) -> Result<(), TrainerError> {
    if rows
        .iter()
        .flat_map(|row| row.iter())
        .any(|value| !value.is_finite())
    {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "SIGReg input contains NaN or Inf",
        ));
    }
    Ok(())
}

fn config_error(field: &'static str, why: impl Into<String>) -> Result<(), TrainerError> {
    Err(TrainerError::new(
        TrainerErrorCode::MejepaTrainConfigInvalid,
        format!("SIGReg {field}: {}", why.into()),
    ))
}

fn seed_for(config: SigregConfig, dim: usize) -> u64 {
    config.projection_seed ^ ((dim as u64) << 32) ^ config.num_slices as u64
}

fn direction_matrix(config: SigregConfig, dim: usize) -> Result<Vec<f32>, TrainerError> {
    let mut rng = SplitMix64::new(seed_for(config, dim));
    let mut matrix = vec![0.0f32; dim * config.num_slices];
    for slice in 0..config.num_slices {
        let direction = normalized_gaussian_direction(&mut rng, dim)?;
        for (row, value) in direction.into_iter().enumerate() {
            matrix[row * config.num_slices + slice] = value;
        }
    }
    Ok(matrix)
}

fn integration_points(config: SigregConfig) -> Vec<f32> {
    let step = (config.t_max - config.t_min) / (config.num_points - 1) as f32;
    (0..config.num_points)
        .map(|idx| config.t_min + step * idx as f32)
        .collect()
}

fn normalized_gaussian_direction(
    rng: &mut SplitMix64,
    dim: usize,
) -> Result<Vec<f32>, TrainerError> {
    let mut direction = Vec::with_capacity(dim);
    while direction.len() < dim {
        let (a, b) = standard_normal_pair(rng);
        direction.push(a);
        if direction.len() < dim {
            direction.push(b);
        }
    }
    let norm = direction
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if !norm.is_finite() || norm <= 0.0 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "SIGReg generated a degenerate projection direction",
        ));
    }
    for value in &mut direction {
        *value /= norm;
    }
    Ok(direction)
}

fn standard_normal_pair(rng: &mut SplitMix64) -> (f32, f32) {
    let u1 = rng.next_unit_f32().clamp(1e-7, 1.0);
    let u2 = rng.next_unit_f32();
    let radius = (-2.0 * u1.ln()).sqrt();
    let theta = std::f32::consts::TAU * u2;
    (radius * theta.cos(), radius * theta.sin())
}

fn summarize(values: &[f32]) -> Result<(f32, f32, f32), TrainerError> {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sum = 0.0f64;
    for value in values {
        if !value.is_finite() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainLossNan,
                "SIGReg produced a non-finite slice statistic",
            ));
        }
        min = min.min(*value);
        max = max.max(*value);
        sum += f64::from(*value);
    }
    Ok((min, max, (sum / values.len() as f64) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor, Var};

    #[test]
    fn collapsed_batch_scores_worse_than_gaussian_like_batch() {
        let device = Device::Cpu;
        let config = SigregConfig {
            num_slices: 32,
            projection_seed: 42,
            ..SigregConfig::default()
        };
        let collapsed = Tensor::zeros((128, 8), DType::F32, &device).unwrap();
        let gaussian = Tensor::from_slice(&gaussian_rows(128, 8, 7), (128, 8), &device).unwrap();
        let collapsed_stats = sigreg_stats(&collapsed, config).unwrap();
        let gaussian_stats = sigreg_stats(&gaussian, config).unwrap();
        assert!(
            collapsed_stats.loss > gaussian_stats.loss * 2.0,
            "collapsed={} gaussian={}",
            collapsed_stats.loss,
            gaussian_stats.loss
        );
    }

    #[test]
    fn anisotropic_batch_scores_worse_than_isotropic_batch() {
        let device = Device::Cpu;
        let config = SigregConfig {
            num_slices: 32,
            projection_seed: 17,
            ..SigregConfig::default()
        };
        let isotropic = gaussian_rows(128, 8, 11);
        let mut anisotropic = isotropic.clone();
        for row in anisotropic.chunks_mut(8) {
            row[0] *= 5.0;
            row[1] *= 0.1;
        }
        let iso_tensor = Tensor::from_slice(&isotropic, (128, 8), &device).unwrap();
        let aniso_tensor = Tensor::from_slice(&anisotropic, (128, 8), &device).unwrap();
        let iso = sigreg_stats(&iso_tensor, config).unwrap();
        let aniso = sigreg_stats(&aniso_tensor, config).unwrap();
        assert!(
            aniso.loss > iso.loss,
            "anisotropic={} isotropic={}",
            aniso.loss,
            iso.loss
        );
    }

    #[test]
    fn invalid_inputs_fail_closed() {
        let device = Device::Cpu;
        let too_small = Tensor::zeros((1, 4), DType::F32, &device).unwrap();
        let err = sigreg_stats(&too_small, SigregConfig::default()).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");

        let non_finite =
            Tensor::from_slice(&[0.0f32, f32::NAN, 1.0, 2.0], (2, 2), &device).unwrap();
        let err = sigreg_stats(&non_finite, SigregConfig::default()).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_LOSS_NAN");
    }

    #[test]
    fn sigreg_loss_backpropagates_to_predicted_latent() {
        let device = Device::Cpu;
        let values = gaussian_rows(16, 4, 19);
        let predicted = Var::from_slice(&values, (16, 4), &device).unwrap();
        let config = SigregConfig {
            num_slices: 8,
            projection_seed: 23,
            ..SigregConfig::default()
        };
        let loss = sigreg_loss(predicted.as_tensor(), config).unwrap();
        let grads = loss.backward().unwrap();
        let grad = grads.get(&predicted).expect("missing predicted gradient");
        let grad_values = grad.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        assert!(grad_values.iter().all(|value| value.is_finite()));
        assert!(grad_values.iter().map(|value| value.abs()).sum::<f32>() > 0.0);
    }

    fn gaussian_rows(batch: usize, dim: usize, seed: u64) -> Vec<f32> {
        let mut rng = SplitMix64::new(seed);
        let mut values = Vec::with_capacity(batch * dim);
        while values.len() < batch * dim {
            let (a, b) = standard_normal_pair(&mut rng);
            values.push(a);
            if values.len() < batch * dim {
                values.push(b);
            }
        }
        values
    }
}
