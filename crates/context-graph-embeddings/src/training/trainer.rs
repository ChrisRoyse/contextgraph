//! Training loop with momentum encoder for causal embedder fine-tuning.
//!
//! Implements the core training loop with:
//! - Forward: embed_as_cause/effect on batch texts
//! - Momentum encoder for stable negative representations (τ=0.999)
//! - Combined loss (InfoNCE + directional + separation + soft label)
//! - Candle autograd backward pass
//! - AdamW optimizer step on W_cause, W_effect, biases
//! - Checkpoint best model by directional accuracy

use std::path::{Path, PathBuf};

use candle_core::{Device, Tensor};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::models::pretrained::TrainableProjection;

use super::evaluation::EvaluationMetrics;
use super::loss::{DirectionalContrastiveLoss, LossComponents, LossConfig};
use super::optimizer::{AdamW, AdamWConfig, ParamGroup};

/// Training configuration.
#[derive(Debug, Clone)]
pub struct TrainingConfig {
    /// Batch size for training (default: 32).
    pub batch_size: u32,
    /// Number of training epochs (default: 50).
    pub epochs: u32,
    /// Evaluate every N epochs (default: 5).
    pub eval_every: u32,
    /// Checkpoint every N epochs (default: 10).
    pub checkpoint_every: u32,
    /// Directory for saving checkpoints.
    pub checkpoint_dir: PathBuf,
    /// Early stopping patience in epochs (default: 10).
    pub early_stopping_patience: u32,
    /// Momentum coefficient for momentum encoder (default: 0.999).
    pub momentum_tau: f64,
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Loss function configuration.
    pub loss_config: LossConfig,
    /// Optimizer configuration.
    pub optimizer_config: AdamWConfig,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            epochs: 50,
            eval_every: 5,
            checkpoint_every: 10,
            checkpoint_dir: PathBuf::from("models/causal/trained"),
            early_stopping_patience: 10,
            momentum_tau: 0.999,
            seed: 42,
            loss_config: LossConfig::default(),
            optimizer_config: AdamWConfig::default(),
        }
    }
}

/// Result of a single training epoch.
#[derive(Debug, Clone)]
pub struct EpochResult {
    /// Epoch number (1-indexed).
    pub epoch: u32,
    /// Average loss components across all batches.
    pub avg_loss: LossComponents,
    /// Number of batches processed.
    pub num_batches: usize,
    /// Evaluation metrics (if evaluation was run this epoch).
    pub eval_metrics: Option<EvaluationMetrics>,
    /// Whether this epoch's model was saved as best.
    pub is_best: bool,
}

/// Training metrics accumulated across all epochs.
#[derive(Debug, Clone, Default)]
pub struct TrainingHistory {
    /// Per-epoch results.
    pub epochs: Vec<EpochResult>,
    /// Best directional accuracy achieved.
    pub best_directional_accuracy: f32,
    /// Epoch that achieved best accuracy.
    pub best_epoch: u32,
    /// Whether training was stopped early.
    pub early_stopped: bool,
    /// Total training steps (across all epochs).
    pub total_steps: usize,
}

/// Momentum encoder weights for stable negative representations.
///
/// Maintains an exponential moving average of the trainable projections:
/// `W_mom ← τ * W_mom + (1 - τ) * W_online`
///
/// This provides stable targets for contrastive learning without
/// requiring huge batch sizes (per MoCo).
pub struct MomentumProjection {
    /// Momentum copy of cause projection [hidden_size, hidden_size].
    pub cause_projection: Tensor,
    /// Momentum copy of cause bias [hidden_size].
    pub cause_bias: Tensor,
    /// Momentum copy of effect projection [hidden_size, hidden_size].
    pub effect_projection: Tensor,
    /// Momentum copy of effect bias [hidden_size].
    pub effect_bias: Tensor,
    /// Momentum coefficient (τ).
    tau: f64,
}

impl MomentumProjection {
    /// Initialize momentum projection from trainable projection.
    pub fn from_trainable(proj: &TrainableProjection, tau: f64) -> EmbeddingResult<Self> {
        Ok(Self {
            cause_projection: proj.cause_projection_var.as_tensor().clone(),
            cause_bias: proj.cause_bias_var.as_tensor().clone(),
            effect_projection: proj.effect_projection_var.as_tensor().clone(),
            effect_bias: proj.effect_bias_var.as_tensor().clone(),
            tau,
        })
    }

    /// Update momentum weights: W_mom ← τ * W_mom + (1 - τ) * W_online
    pub fn update(&mut self, online: &TrainableProjection) -> EmbeddingResult<()> {
        let tau = self.tau;
        let one_minus_tau = 1.0 - tau;

        self.cause_projection = self
            .cause_projection
            .affine(tau, 0.0)
            .map_err(map_candle)?
            .add(
                &online
                    .cause_projection_var
                    .as_tensor()
                    .clone()
                    .affine(one_minus_tau, 0.0)
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?;

        self.cause_bias = self
            .cause_bias
            .affine(tau, 0.0)
            .map_err(map_candle)?
            .add(
                &online
                    .cause_bias_var
                    .as_tensor()
                    .clone()
                    .affine(one_minus_tau, 0.0)
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?;

        self.effect_projection = self
            .effect_projection
            .affine(tau, 0.0)
            .map_err(map_candle)?
            .add(
                &online
                    .effect_projection_var
                    .as_tensor()
                    .clone()
                    .affine(one_minus_tau, 0.0)
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?;

        self.effect_bias = self
            .effect_bias
            .affine(tau, 0.0)
            .map_err(map_candle)?
            .add(
                &online
                    .effect_bias_var
                    .as_tensor()
                    .clone()
                    .affine(one_minus_tau, 0.0)
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?;

        Ok(())
    }

    /// Apply cause projection using momentum weights (no gradients).
    pub fn project_cause(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .cause_projection
                    .t()
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?;
        projected
            .broadcast_add(&self.cause_bias)
            .map_err(map_candle)
    }

    /// Apply effect projection using momentum weights (no gradients).
    pub fn project_effect(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .effect_projection
                    .t()
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?;
        projected
            .broadcast_add(&self.effect_bias)
            .map_err(map_candle)
    }
}

/// Causal embedder trainer.
///
/// Orchestrates the training loop:
/// 1. Forward pass through trainable projections
/// 2. Momentum encoder update
/// 3. Combined loss computation
/// 4. Backward pass (Candle autograd)
/// 5. AdamW optimizer step
/// 6. Periodic evaluation and checkpointing
pub struct CausalTrainer {
    /// Trainable projection weights.
    projection: TrainableProjection,
    /// Momentum encoder for stable negatives.
    momentum: MomentumProjection,
    /// AdamW optimizer.
    optimizer: AdamW,
    /// Loss function.
    loss_fn: DirectionalContrastiveLoss,
    /// Training configuration.
    config: TrainingConfig,
    /// Training history.
    history: TrainingHistory,
    /// Device for tensor operations.
    device: Device,
}

impl CausalTrainer {
    /// Create a new trainer from existing projection weights.
    pub fn new(
        projection: TrainableProjection,
        config: TrainingConfig,
        device: Device,
    ) -> EmbeddingResult<Self> {
        let momentum =
            MomentumProjection::from_trainable(&projection, config.momentum_tau)?;
        let loss_fn = DirectionalContrastiveLoss::new(config.loss_config.clone());

        let optimizer = AdamW::new(config.optimizer_config.clone());

        Ok(Self {
            projection,
            momentum,
            optimizer,
            loss_fn,
            config,
            history: TrainingHistory::default(),
            device,
        })
    }

    /// Register trainable parameters with the optimizer.
    pub fn register_params(&mut self) -> EmbeddingResult<()> {
        for var in self.projection.trainable_vars() {
            self.optimizer.add_param(var.clone(), ParamGroup::Projection)?;
        }
        Ok(())
    }

    /// Run a single training step on a batch of pre-computed embeddings.
    ///
    /// # Arguments
    /// * `cause_vecs` - Cause embeddings [N, D] (L2-normalized, from trainable projection)
    /// * `effect_vecs` - Effect embeddings [N, D] (L2-normalized, from trainable projection)
    /// * `confidences` - LLM confidence scores [N]
    ///
    /// # Returns
    /// Loss components for logging.
    pub fn train_step(
        &mut self,
        cause_vecs: &Tensor,
        effect_vecs: &Tensor,
        confidences: &Tensor,
    ) -> EmbeddingResult<LossComponents> {
        // Compute combined loss
        let (total_loss, components) = self.loss_fn.compute(cause_vecs, effect_vecs, confidences)?;

        // Backward pass + optimizer step
        self.optimizer.step(&total_loss)?;

        // Update momentum encoder
        self.momentum.update(&self.projection)?;

        self.history.total_steps += 1;

        Ok(components)
    }

    /// Get the trainable projection (for applying to embeddings in forward pass).
    pub fn projection(&self) -> &TrainableProjection {
        &self.projection
    }

    /// Get the momentum projection (for encoding negatives).
    pub fn momentum(&self) -> &MomentumProjection {
        &self.momentum
    }

    /// Get the training configuration.
    pub fn config(&self) -> &TrainingConfig {
        &self.config
    }

    /// Get the training history.
    pub fn history(&self) -> &TrainingHistory {
        &self.history
    }

    /// Record an epoch result and check for early stopping.
    ///
    /// Returns true if training should continue, false if early stopping triggered.
    pub fn record_epoch(&mut self, result: EpochResult) -> bool {
        let is_best = result
            .eval_metrics
            .as_ref()
            .map(|m| m.directional_accuracy > self.history.best_directional_accuracy)
            .unwrap_or(false);

        if is_best {
            if let Some(ref metrics) = result.eval_metrics {
                self.history.best_directional_accuracy = metrics.directional_accuracy;
                self.history.best_epoch = result.epoch;
            }
        }

        self.history.epochs.push(result);

        // Check early stopping
        let epochs_since_best = self.history.epochs.len() as u32 - self.history.best_epoch;
        if epochs_since_best > self.config.early_stopping_patience && self.history.best_epoch > 0 {
            self.history.early_stopped = true;
            return false;
        }

        true
    }

    /// Save a checkpoint of the current trainable projection.
    pub fn save_checkpoint(&self, path: &Path) -> EmbeddingResult<()> {
        self.projection.save_trained(path)
    }

    /// Save the best checkpoint to the configured checkpoint directory.
    pub fn save_best(&self) -> EmbeddingResult<()> {
        let path = self.config.checkpoint_dir.join("projection_best.safetensors");
        self.save_checkpoint(&path)
    }

    /// Get the current learning rate for projection parameters.
    pub fn current_lr(&self) -> f64 {
        self.optimizer.current_lr(ParamGroup::Projection)
    }

    /// Get the device.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get mutable access to the optimizer for registering additional parameters.
    pub fn optimizer_mut(&mut self) -> &mut AdamW {
        &mut self.optimizer
    }
}

/// Map candle errors to EmbeddingError.
fn map_candle(e: candle_core::Error) -> EmbeddingError {
    EmbeddingError::GpuError {
        message: format!("Trainer error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_config_default() {
        let config = TrainingConfig::default();
        assert_eq!(config.batch_size, 32);
        assert_eq!(config.epochs, 50);
        assert_eq!(config.eval_every, 5);
        assert_eq!(config.early_stopping_patience, 10);
        assert!((config.momentum_tau - 0.999).abs() < 1e-9);
    }

    #[test]
    fn test_epoch_result_tracking() {
        let config = TrainingConfig::default();
        let projection = TrainableProjection::new(768, &Device::Cpu).unwrap();
        let mut trainer = CausalTrainer::new(projection, config, Device::Cpu).unwrap();

        // Record epochs without eval metrics
        let result = EpochResult {
            epoch: 1,
            avg_loss: LossComponents::default(),
            num_batches: 10,
            eval_metrics: None,
            is_best: false,
        };
        assert!(trainer.record_epoch(result));
        assert_eq!(trainer.history().epochs.len(), 1);
    }

    #[test]
    fn test_momentum_projection_init() {
        let proj = TrainableProjection::new(16, &Device::Cpu).unwrap();
        let momentum = MomentumProjection::from_trainable(&proj, 0.999).unwrap();

        // Momentum weights should match initial trainable weights
        let cause_data: Vec<f32> = momentum
            .cause_projection
            .flatten_all()
            .unwrap()
            .to_vec1()
            .unwrap();
        let online_data: Vec<f32> = proj
            .cause_projection_var
            .as_tensor()
            .flatten_all()
            .unwrap()
            .to_vec1()
            .unwrap();

        for (a, b) in cause_data.iter().zip(online_data.iter()) {
            assert!((a - b).abs() < 1e-6, "Momentum should match online at init");
        }
    }
}
