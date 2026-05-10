use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::{Device, Tensor};
use context_graph_mejepa_corpus::prng::SplitMix64;

pub const K_PERTURBATIONS: usize = 4;
pub const EPSILON_VALUES: [f32; 4] = [0.01, 0.05, 0.1, 0.2];
pub const DEFAULT_SMOOTHNESS_ANOMALY_CEILING: f32 = 10.0;

pub trait PredictorFn {
    fn forward(&self, panel: &Tensor) -> Result<Tensor, TrainerError>;
}

pub trait FrozenTargetFn {
    fn readback(&self, panel: &Tensor) -> Result<Tensor, TrainerError>;
}

#[derive(Debug, Clone)]
pub struct CounterfactualOutput {
    pub loss: Tensor,
    pub local_smoothness: f32,
    pub smoothness_anomaly: bool,
}

pub fn sample_unit_vector(dim: usize, seed: u64, device: &Device) -> Result<Tensor, TrainerError> {
    if dim == 0 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "sample_unit_vector dim must be positive",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let mut values = Vec::with_capacity(dim);
    for _ in 0..dim {
        values.push(rng.next_f32_signed());
    }
    let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-8);
    for value in &mut values {
        *value /= norm;
    }
    Tensor::from_vec(values, dim, device).map_err(TrainerError::from)
}

pub fn perturb_panel(panel_t1: &Tensor, eps: f32, v: &Tensor) -> Result<Tensor, TrainerError> {
    if panel_t1.dims() != v.dims() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!(
                "counterfactual perturb dim mismatch: panel={:?} v={:?}",
                panel_t1.dims(),
                v.dims()
            ),
        ));
    }
    Ok((panel_t1 + &v.affine(eps as f64, 0.0)?)?)
}

pub fn compute_counterfactual_loss(
    panel_t1: &Tensor,
    predictor: &dyn PredictorFn,
    frozen_target: &dyn FrozenTargetFn,
    step: u64,
    smoothness_anomaly_ceiling: f32,
) -> Result<CounterfactualOutput, TrainerError> {
    if panel_t1.dims().len() != 1 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!(
                "counterfactual panel must be rank-1, got {:?}",
                panel_t1.dims()
            ),
        ));
    }
    let dim = panel_t1.dims()[0];
    let mut sum = Tensor::new(0f32, panel_t1.device())?;
    let p_orig = predictor.forward(panel_t1)?;
    let mut smooth_acc = 0.0f32;
    for (eps_idx, eps) in EPSILON_VALUES.iter().copied().enumerate() {
        let seed = step
            .checked_mul(K_PERTURBATIONS as u64)
            .and_then(|s| s.checked_add(eps_idx as u64))
            .ok_or_else(|| {
                TrainerError::new(
                    TrainerErrorCode::MejepaTrainConfigInvalid,
                    "counterfactual seed overflow",
                )
            })?;
        let v = sample_unit_vector(dim, seed, panel_t1.device())?;
        let perturbed = perturb_panel(panel_t1, eps, &v)?;
        let pred_pert = predictor.forward(&perturbed)?;
        let target_pert = frozen_target.readback(&perturbed)?;
        let mse = (&pred_pert - &target_pert)?.sqr()?.mean_all()?;
        sum = (&sum + &mse)?;
        smooth_acc += (1.0 - cosine_scalar(&pred_pert, &p_orig)?) / eps;
    }
    let loss = (sum / K_PERTURBATIONS as f64)?;
    let local_smoothness = smooth_acc / K_PERTURBATIONS as f32;
    Ok(CounterfactualOutput {
        loss,
        local_smoothness,
        smoothness_anomaly: local_smoothness > smoothness_anomaly_ceiling,
    })
}

pub fn should_run_counterfactual_cycle(step: u64, interval: u64, warmup: u64) -> bool {
    interval > 0 && step >= warmup && step.is_multiple_of(interval)
}

fn cosine_scalar(a: &Tensor, b: &Tensor) -> Result<f32, TrainerError> {
    if a.dims() != b.dims() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "counterfactual cosine tensor shape mismatch",
        ));
    }
    let av = a
        .to_dtype(candle_core::DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let bv = b
        .to_dtype(candle_core::DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let dot = av.iter().zip(&bv).map(|(x, y)| x * y).sum::<f32>();
    let na = av.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = bv.iter().map(|x| x * x).sum::<f32>().sqrt();
    Ok(if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    struct Id;
    impl PredictorFn for Id {
        fn forward(&self, panel: &Tensor) -> Result<Tensor, TrainerError> {
            Ok(panel.clone())
        }
    }
    impl FrozenTargetFn for Id {
        fn readback(&self, panel: &Tensor) -> Result<Tensor, TrainerError> {
            Ok(panel.clone())
        }
    }

    #[test]
    fn deterministic_unit_vector() {
        let a = sample_unit_vector(8, 11, &Device::Cpu)
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        let b = sample_unit_vector(8, 11, &Device::Cpu)
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn cycle_gate() {
        assert!(should_run_counterfactual_cycle(1000, 5, 1000));
        assert!(!should_run_counterfactual_cycle(500, 5, 1000));
    }

    #[test]
    fn zero_loss_for_identity_target() {
        let panel = Tensor::from_slice(&[1f32, 2., 3., 4.], 4, &Device::Cpu).unwrap();
        let out = compute_counterfactual_loss(&panel, &Id, &Id, 10, 10.0).unwrap();
        assert!(out.loss.to_scalar::<f32>().unwrap().abs() < 1e-6);
    }
}
