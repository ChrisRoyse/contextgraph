//! AdamW optimizer for Candle Var tensors.
//!
//! Implements AdamW with:
//! - Per-parameter moment estimates (m, v)
//! - Linear warmup + cosine decay schedule
//! - Gradient clipping (max_norm)
//! - Decoupled weight decay

use candle_core::{DType, Tensor, Var};

use crate::error::{EmbeddingError, EmbeddingResult};

/// AdamW optimizer configuration.
#[derive(Debug, Clone)]
pub struct AdamWConfig {
    /// Base learning rate for projection matrices.
    pub lr_projection: f64,
    /// Learning rate for LoRA parameters (typically 10x smaller).
    pub lr_lora: f64,
    /// Learning rate for marker weights.
    pub lr_markers: f64,
    /// First moment exponential decay rate.
    pub beta1: f64,
    /// Second moment exponential decay rate.
    pub beta2: f64,
    /// Numerical stability constant.
    pub epsilon: f64,
    /// Decoupled weight decay coefficient.
    pub weight_decay: f64,
    /// Maximum gradient norm for clipping.
    pub max_grad_norm: f64,
    /// Total number of training steps (for schedule).
    pub total_steps: usize,
    /// Fraction of total steps for linear warmup.
    pub warmup_fraction: f64,
}

impl Default for AdamWConfig {
    fn default() -> Self {
        Self {
            lr_projection: 1e-4,
            lr_lora: 1e-5,
            lr_markers: 5e-4,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1e-8,
            weight_decay: 0.01,
            max_grad_norm: 1.0,
            total_steps: 1000,
            warmup_fraction: 0.1,
        }
    }
}

/// Parameter group for different learning rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamGroup {
    /// Projection matrices (W_cause, W_effect, biases).
    Projection,
    /// LoRA adapter weights.
    Lora,
    /// Marker weights and MLP parameters.
    Markers,
}

/// A tracked parameter with its moment estimates.
struct TrackedParam {
    /// The trainable variable.
    var: Var,
    /// First moment estimate (mean of gradients).
    m: Tensor,
    /// Second moment estimate (mean of squared gradients).
    v: Tensor,
    /// Parameter group (determines learning rate).
    group: ParamGroup,
}

/// AdamW optimizer for Candle Var tensors.
pub struct AdamW {
    config: AdamWConfig,
    params: Vec<TrackedParam>,
    /// Global step counter (for schedule + bias correction).
    step: usize,
}

impl AdamW {
    /// Create a new AdamW optimizer.
    pub fn new(config: AdamWConfig) -> Self {
        Self {
            config,
            params: Vec::new(),
            step: 0,
        }
    }

    /// Register a trainable parameter.
    pub fn add_param(&mut self, var: Var, group: ParamGroup) -> EmbeddingResult<()> {
        let shape = var.as_tensor().shape().clone();
        let device = var.as_tensor().device().clone();

        let m = Tensor::zeros(&shape, var.as_tensor().dtype(), &device).map_err(map_candle)?;
        let v = Tensor::zeros(&shape, var.as_tensor().dtype(), &device).map_err(map_candle)?;

        self.params.push(TrackedParam { var, m, v, group });
        Ok(())
    }

    /// Get the current learning rate (with warmup + cosine decay).
    pub fn current_lr(&self, group: ParamGroup) -> f64 {
        let base_lr = match group {
            ParamGroup::Projection => self.config.lr_projection,
            ParamGroup::Lora => self.config.lr_lora,
            ParamGroup::Markers => self.config.lr_markers,
        };

        let warmup_steps = (self.config.total_steps as f64 * self.config.warmup_fraction) as usize;

        if self.step < warmup_steps {
            // Linear warmup
            base_lr * (self.step as f64 / warmup_steps.max(1) as f64)
        } else {
            // Cosine decay
            let decay_steps = self.config.total_steps.saturating_sub(warmup_steps);
            let progress = (self.step - warmup_steps) as f64 / decay_steps.max(1) as f64;
            let cosine_factor = 0.5 * (1.0 + (std::f64::consts::PI * progress).cos());
            base_lr * cosine_factor
        }
    }

    /// Perform one optimization step.
    ///
    /// Call after `loss.backward()` computes gradients via Candle autograd.
    pub fn step(&mut self, loss: &Tensor) -> EmbeddingResult<()> {
        self.step += 1;
        let t = self.step as f64;

        // Compute gradients
        let grads = loss.backward().map_err(map_candle)?;

        // Pre-compute gradient norm before mutable borrow of params
        let mut total_sq = 0.0f64;
        for param in &self.params {
            if let Some(grad) = grads.get(param.var.as_tensor()) {
                let sq_sum: f32 = grad
                    .sqr()
                    .map_err(map_candle)?
                    .sum_all()
                    .map_err(map_candle)?
                    .to_dtype(DType::F32)
                    .map_err(map_candle)?
                    .to_scalar()
                    .map_err(map_candle)?;
                total_sq += sq_sum as f64;
            }
        }
        let total_norm = total_sq.sqrt();

        let clip_scale = if total_norm > self.config.max_grad_norm {
            self.config.max_grad_norm / (total_norm + self.config.epsilon)
        } else {
            1.0
        };

        // Pre-compute learning rates per group (avoids borrowing self inside mutable loop)
        let lr_projection = self.current_lr(ParamGroup::Projection);
        let lr_lora = self.current_lr(ParamGroup::Lora);
        let lr_markers = self.current_lr(ParamGroup::Markers);

        for param in &mut self.params {
            let grad = match grads.get(param.var.as_tensor()) {
                Some(g) => g,
                None => continue, // No gradient for this parameter
            };

            // Apply gradient clipping
            let clipped_grad = if (clip_scale - 1.0).abs() > 1e-9 {
                grad.affine(clip_scale, 0.0).map_err(map_candle)?
            } else {
                grad.clone()
            };

            let lr = match param.group {
                ParamGroup::Projection => lr_projection,
                ParamGroup::Lora => lr_lora,
                ParamGroup::Markers => lr_markers,
            };

            // Bias-corrected moment estimates
            let bc1 = 1.0 - self.config.beta1.powi(t as i32);
            let bc2 = 1.0 - self.config.beta2.powi(t as i32);

            // Update first moment: m = β1 * m + (1 - β1) * grad
            // Detach to break computation graph chain (m/v are optimizer state, not model params)
            param.m = param
                .m
                .affine(self.config.beta1, 0.0)
                .map_err(map_candle)?
                .add(
                    &clipped_grad
                        .affine(1.0 - self.config.beta1, 0.0)
                        .map_err(map_candle)?,
                )
                .map_err(map_candle)?
                .detach();

            // Update second moment: v = β2 * v + (1 - β2) * grad^2
            let grad_sq = clipped_grad.sqr().map_err(map_candle)?;
            param.v = param
                .v
                .affine(self.config.beta2, 0.0)
                .map_err(map_candle)?
                .add(
                    &grad_sq
                        .affine(1.0 - self.config.beta2, 0.0)
                        .map_err(map_candle)?,
                )
                .map_err(map_candle)?
                .detach();

            // Bias-corrected estimates
            let m_hat = param.m.affine(1.0 / bc1, 0.0).map_err(map_candle)?;
            let v_hat = param.v.affine(1.0 / bc2, 0.0).map_err(map_candle)?;

            // Compute update: -lr * m_hat / (sqrt(v_hat) + eps)
            let v_sqrt = v_hat.sqrt().map_err(map_candle)?;
            let eps_tensor = Tensor::ones_like(&v_sqrt)
                .map_err(map_candle)?
                .affine(self.config.epsilon, 0.0)
                .map_err(map_candle)?;
            let denom = v_sqrt.add(&eps_tensor).map_err(map_candle)?;
            let step_update = m_hat
                .div(&denom)
                .map_err(map_candle)?
                .affine(-lr, 0.0)
                .map_err(map_candle)?;

            // Decoupled weight decay: θ = θ - lr * wd * θ
            let current = param.var.as_tensor().clone();
            let decay = current
                .affine(-lr * self.config.weight_decay, 0.0)
                .map_err(map_candle)?;

            // Combined update: θ = θ + step_update + decay
            // Detach to prevent computation graph from growing through Var across steps
            let new_val = current
                .add(&step_update)
                .map_err(map_candle)?
                .add(&decay)
                .map_err(map_candle)?
                .detach();

            param.var.set(&new_val).map_err(map_candle)?;
        }

        Ok(())
    }

    /// Get the current global step.
    pub fn global_step(&self) -> usize {
        self.step
    }

    /// Get the number of tracked parameters.
    pub fn num_params(&self) -> usize {
        self.params.len()
    }

    /// Get the optimizer configuration.
    pub fn config(&self) -> &AdamWConfig {
        &self.config
    }

    /// Zero all moment estimates (for curriculum stage transitions).
    pub fn reset_moments(&mut self) -> EmbeddingResult<()> {
        for param in &mut self.params {
            let shape = param.m.shape().clone();
            let dtype = param.m.dtype();
            let device = param.m.device().clone();
            param.m = Tensor::zeros(&shape, dtype, &device).map_err(map_candle)?;
            param.v = Tensor::zeros(&shape, dtype, &device).map_err(map_candle)?;
        }
        Ok(())
    }
}

/// Map candle errors to EmbeddingError.
fn map_candle(e: candle_core::Error) -> EmbeddingError {
    EmbeddingError::GpuError {
        message: format!("Optimizer error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_warmup_schedule() {
        let config = AdamWConfig {
            lr_projection: 1e-4,
            total_steps: 100,
            warmup_fraction: 0.1,
            ..Default::default()
        };
        let mut opt = AdamW::new(config);

        // Step 0: lr = 0 (start of warmup)
        assert_eq!(opt.current_lr(ParamGroup::Projection), 0.0);

        // Step 5: lr = 0.5 * base (mid warmup, 5/10 = 0.5)
        opt.step = 5;
        let lr5 = opt.current_lr(ParamGroup::Projection);
        assert!((lr5 - 0.5e-4).abs() < 1e-10);

        // Step 10: lr = base (end of warmup)
        opt.step = 10;
        let lr10 = opt.current_lr(ParamGroup::Projection);
        assert!((lr10 - 1e-4).abs() < 1e-10);

        // Step 100: lr ≈ 0 (end of cosine decay)
        opt.step = 100;
        let lr100 = opt.current_lr(ParamGroup::Projection);
        assert!(lr100 < 1e-8, "LR at end should be ~0, got {}", lr100);
    }

    #[test]
    fn test_cosine_decay() {
        let config = AdamWConfig {
            lr_projection: 1e-4,
            total_steps: 100,
            warmup_fraction: 0.0, // No warmup
            ..Default::default()
        };
        let mut opt = AdamW::new(config);

        opt.step = 0;
        let lr0 = opt.current_lr(ParamGroup::Projection);
        assert!((lr0 - 1e-4).abs() < 1e-10, "Start should be full LR");

        opt.step = 50;
        let lr50 = opt.current_lr(ParamGroup::Projection);
        assert!(
            (lr50 - 0.5e-4).abs() < 1e-6,
            "Midpoint should be ~half, got {}",
            lr50
        );
    }

    #[test]
    fn test_param_groups_different_lr() {
        let config = AdamWConfig::default();
        let opt = AdamW::new(config);

        // Different groups should have different learning rates
        let lr_proj = opt.config.lr_projection;
        let lr_lora = opt.config.lr_lora;
        let lr_markers = opt.config.lr_markers;

        assert!(lr_proj > lr_lora, "Projection LR should be > LoRA LR");
        assert!(lr_markers > lr_lora, "Marker LR should be > LoRA LR");
    }

    #[test]
    fn test_add_param() {
        let config = AdamWConfig::default();
        let mut opt = AdamW::new(config);

        let var = Var::from_tensor(
            &Tensor::zeros(&[4, 4], candle_core::DType::F32, &Device::Cpu).unwrap(),
        )
        .unwrap();

        opt.add_param(var, ParamGroup::Projection).unwrap();
        assert_eq!(opt.num_params(), 1);
    }
}
