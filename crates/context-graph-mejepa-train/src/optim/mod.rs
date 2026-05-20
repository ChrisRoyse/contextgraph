use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::{Tensor, Var};
use context_graph_embeddings::training::optimizer::{AdamW, AdamWConfig, ParamGroup};
use serde_json::json;

pub const GRAD_EXPLODE_RATIO_THRESHOLD: f32 = 100.0;

pub struct OptimizerWiring {
    pub adamw: AdamW,
    pub total_steps: u64,
    pub warmup_steps: u32,
    pub base_lr: f64,
}

pub fn build_adamw(
    config: &TrainingConfig,
    total_steps: u64,
    predictor_params: Vec<Var>,
    lora_params: Vec<Var>,
    aux_params: Vec<Var>,
    layernorm_bias_params: Vec<Var>,
) -> Result<OptimizerWiring, TrainerError> {
    config.validate()?;
    if total_steps == 0 {
        return Err(config_error("total_steps", "must be positive"));
    }
    let warmup_steps = effective_warmup_steps(config.warmup_steps, total_steps);
    let adam_config = AdamWConfig {
        lr_projection: config.lr,
        lr_lora: config.lr * 0.1,
        lr_markers: config.lr,
        weight_decay: config.weight_decay,
        max_grad_norm: config.max_grad_norm as f64,
        total_steps: total_steps as usize,
        warmup_fraction: warmup_steps as f64 / total_steps.max(1) as f64,
        ..AdamWConfig::default()
    };
    let mut adamw = AdamW::new(adam_config);
    for param in predictor_params {
        adamw
            .add_param(param, ParamGroup::Projection)
            .map_err(|err| config_error("predictor_params", err.to_string()))?;
    }
    for param in lora_params {
        adamw
            .add_param(param, ParamGroup::Lora)
            .map_err(|err| config_error("lora_params", err.to_string()))?;
    }
    for param in aux_params {
        adamw
            .add_param(param, ParamGroup::Markers)
            .map_err(|err| config_error("aux_params", err.to_string()))?;
    }
    if !layernorm_bias_params.is_empty() {
        tracing::warn!(
            count = layernorm_bias_params.len(),
            "LayerNorm/bias tensors intentionally excluded from AdamW because the shipped optimizer has global decoupled weight decay"
        );
    }
    Ok(OptimizerWiring {
        adamw,
        total_steps,
        warmup_steps,
        base_lr: config.lr,
    })
}

pub fn current_lr(t: u64, total_steps: u64, warmup_steps: u32, base_lr: f64) -> f64 {
    if total_steps == 0 || base_lr <= 0.0 {
        return 0.0;
    }
    let warm = warmup_steps.max(1) as u64;
    if t < warm {
        return base_lr * (t as f64 / warm as f64);
    }
    if t >= total_steps {
        return 0.0;
    }
    let denom = total_steps.saturating_sub(warm).max(1);
    let progress = (t - warm) as f64 / denom as f64;
    (base_lr * 0.5 * (1.0 + (std::f64::consts::PI * progress).cos())).max(0.0)
}

pub fn effective_warmup_steps(config_warmup: u32, total_steps: u64) -> u32 {
    let cap = (total_steps / 4).max(1) as u32;
    config_warmup.min(cap).min(500)
}

pub fn nan_inf_finite_check(grads: &[Tensor]) -> Result<(), TrainerError> {
    for (i, grad) in grads.iter().enumerate() {
        let vals = grad
            .to_dtype(candle_core::DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        if let Some((offset, value)) = vals
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainLossNan,
                format!("non-finite gradient tensor={i} offset={offset} value={value}"),
            )
            .with_context(json!({"tensor_idx": i, "offset": offset, "value": value})));
        }
    }
    Ok(())
}

pub fn clip_grads(grads: &mut [Tensor], max_norm: f32) -> Result<f32, TrainerError> {
    if !max_norm.is_finite() || max_norm <= 0.0 {
        return Err(config_error("max_norm", "must be positive finite"));
    }
    let mut total_sq = 0.0f64;
    for grad in grads.iter() {
        let sq: f32 = grad.sqr()?.sum_all()?.to_scalar::<f32>()?;
        total_sq += sq as f64;
    }
    let norm = total_sq.sqrt() as f32;
    if norm > max_norm {
        let scale = max_norm / (norm + 1e-8);
        for grad in grads.iter_mut() {
            *grad = grad.affine(scale as f64, 0.0)?;
        }
    }
    Ok(norm)
}

pub fn detect_grad_explode(
    grad_norm: f32,
    running_mean: f32,
    ratio_threshold: f32,
) -> Option<TrainerError> {
    if running_mean > 0.0 && grad_norm > ratio_threshold * running_mean {
        Some(
            TrainerError::new(
                TrainerErrorCode::MejepaTrainGradExplode,
                format!("gradient norm {grad_norm} exceeds {ratio_threshold}x running mean {running_mean}"),
            )
            .with_context(json!({
                "grad_norm": grad_norm,
                "running_mean": running_mean,
                "ratio_threshold": ratio_threshold
            })),
        )
    } else {
        None
    }
}

pub fn update_grad_norm_running_mean(prev: f32, current: f32, ema_decay: f32) -> f32 {
    ema_decay * prev + (1.0 - ema_decay) * current
}

fn config_error(field: &'static str, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field,
        "file": "file:crates/context-graph-mejepa-train/src/optim/mod.rs",
        "remediation": "fix optimizer config or parameter wiring"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};

    #[test]
    fn lr_boundaries() {
        assert_eq!(current_lr(0, 100, 10, 5e-4), 0.0);
        assert!((current_lr(10, 100, 10, 5e-4) - 5e-4).abs() < 1e-12);
        assert_eq!(current_lr(100, 100, 10, 5e-4), 0.0);
    }

    #[test]
    fn warmup_clamps() {
        assert_eq!(effective_warmup_steps(500, 100), 25);
        assert_eq!(effective_warmup_steps(500, 3), 1);
    }

    #[test]
    fn finite_check_rejects_nan() {
        let t = Tensor::from_slice(&[1f32, f32::NAN], 2, &Device::Cpu).unwrap();
        assert_eq!(
            nan_inf_finite_check(&[t]).unwrap_err().code(),
            "MEJEPA_TRAIN_LOSS_NAN"
        );
    }

    #[test]
    fn grad_explode_detects_ratio() {
        assert!(detect_grad_explode(101.0, 1.0, 100.0).is_some());
        assert!(detect_grad_explode(99.0, 1.0, 100.0).is_none());
    }
}
