//! 3-stage progressive training pipeline for causal embedder fine-tuning.
//!
//! Orchestrates LoRA + projection training with curriculum learning:
//! - Stage 1 (warm-up): projection-only, easy pairs, 25 epochs
//! - Stage 2 (LoRA activation): projection + LoRA, all pairs, 20 epochs
//! - Stage 3 (direction): projection + LoRA, directional triplet loss weight increased, 20 epochs

use std::path::{Path, PathBuf};

use candle_core::{backend::BackendDevice, Device, Tensor};
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::models::pretrained::causal::config::CAUSAL_DIMENSION;
use crate::models::pretrained::causal::forward::{
    gpu_forward_base_tensor, gpu_forward_with_lora_tensor, l2_normalize,
};
use crate::models::pretrained::causal::weights::{NomicWeights, TrainableProjection};
use crate::training::data::{CausalDataLoader, CausalTrainingPair, TrainingBatch};
use crate::training::evaluation::{EvaluationMetrics, Evaluator};
use crate::training::lora::{LoraConfig, LoraLayers};
use crate::training::loss::{LossComponents, LossConfig};
use crate::training::multitask::{
    DirectionLabel, MechanismLabel, MultiTaskConfig, MultiTaskHeads,
};
use crate::training::optimizer::{AdamWConfig, ParamGroup};
use crate::training::trainer::{CausalTrainer, EpochResult, TrainingConfig};

/// Pipeline configuration for 3-stage progressive training.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// LoRA configuration.
    pub lora_config: LoraConfig,
    /// Stage 1: warm-up epochs (projection only).
    pub stage1_epochs: u32,
    /// Stage 2: LoRA activation epochs.
    pub stage2_epochs: u32,
    /// Stage 3: directional emphasis epochs.
    pub stage3_epochs: u32,
    /// Maximum difficulty for Stage 1 (curriculum filtering).
    pub stage1_max_difficulty: f32,
    /// Directional loss weight multiplier for Stage 3.
    pub stage3_direction_weight: f32,
    /// Batch size.
    pub batch_size: usize,
    /// Evaluation frequency (every N epochs).
    pub eval_every: u32,
    /// Early stopping patience.
    pub early_stopping_patience: u32,
    /// Output directory for checkpoints.
    pub output_dir: PathBuf,
    /// Random seed.
    pub seed: u64,
    /// Multi-task head configuration (direction + mechanism classification).
    /// Active in Stages 2+3 only. Set to None to disable multi-task learning.
    pub multitask_config: Option<MultiTaskConfig>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            lora_config: LoraConfig::default(),
            stage1_epochs: 25,
            stage2_epochs: 20,
            stage3_epochs: 20,
            stage1_max_difficulty: 0.2,
            stage3_direction_weight: 0.6,
            batch_size: 16,
            eval_every: 5,
            early_stopping_patience: 10,
            output_dir: PathBuf::from("models/causal/trained"),
            seed: 42,
            multitask_config: Some(MultiTaskConfig::default()),
        }
    }
}

/// Result of a single training stage.
#[derive(Debug, Clone)]
pub struct StageResult {
    /// Stage number (1-3).
    pub stage: u32,
    /// Epochs completed.
    pub epochs_completed: u32,
    /// Best evaluation metrics achieved in this stage.
    pub best_metrics: Option<EvaluationMetrics>,
    /// Final loss components.
    pub final_loss: LossComponents,
    /// Whether early stopping triggered.
    pub early_stopped: bool,
}

/// Result of the full 3-stage pipeline.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// Per-stage results.
    pub stages: Vec<StageResult>,
    /// Best overall evaluation metrics.
    pub best_metrics: Option<EvaluationMetrics>,
    /// Total epochs across all stages.
    pub total_epochs: u32,
    /// Path where best checkpoint was saved.
    pub checkpoint_path: Option<PathBuf>,
}

/// 3-stage progressive training pipeline.
///
/// Orchestrates the full training process:
/// 1. Loads/creates LoRA adapters and projection heads
/// 2. Runs 3 progressive training stages
/// 3. Saves best checkpoints
pub struct CausalTrainingPipeline {
    /// LoRA adapters (trainable).
    pub lora: LoraLayers,
    /// Trainable projection heads (used for initialization only;
    /// each stage gets its own projection via checkpoint loading).
    pub projection: TrainableProjection,
    /// Multi-task classification heads (direction + mechanism).
    /// Active in Stages 2+3 for auxiliary gradient signal.
    pub multitask_heads: Option<MultiTaskHeads>,
    /// Pipeline configuration.
    pub config: PipelineConfig,
    /// Device for tensor operations.
    device: Device,
}

impl CausalTrainingPipeline {
    /// Create a new training pipeline.
    pub fn new(config: PipelineConfig, device: Device) -> EmbeddingResult<Self> {
        let lora = LoraLayers::new(config.lora_config.clone(), &device)?;
        let projection = TrainableProjection::new(CAUSAL_DIMENSION, &device)?;

        let multitask_heads = if let Some(ref mt_config) = config.multitask_config {
            let heads = MultiTaskHeads::new(mt_config.clone(), &device)?;
            tracing::info!(
                "Multi-task heads created: {} params (direction 3-class + mechanism 7-class)",
                heads.total_params()
            );
            Some(heads)
        } else {
            None
        };

        tracing::info!(
            "Pipeline created: LoRA params={}, projection params={}, multitask={}",
            lora.total_params(),
            CAUSAL_DIMENSION * CAUSAL_DIMENSION * 2 + CAUSAL_DIMENSION * 2,
            multitask_heads.as_ref().map(|h| h.total_params()).unwrap_or(0),
        );

        Ok(Self {
            lora,
            projection,
            multitask_heads,
            config,
            device,
        })
    }

    /// Embed a batch of texts using forward + trainable projection.
    ///
    /// The `projection` parameter must be the SAME projection whose Var tensors
    /// are registered with the optimizer. This ensures gradients computed by
    /// `loss.backward()` flow to the correct Var objects.
    ///
    /// When `use_lora` is true, uses LoRA-augmented encoder (Stages 2-3).
    /// When false, uses base encoder only (Stage 1 warm-up, saves VRAM).
    ///
    /// Returns (cause_tensors [N, D], effect_tensors [N, D]) with preserved grad graph.
    pub fn embed_batch_dual(
        &self,
        batch: &TrainingBatch,
        weights: &NomicWeights,
        tokenizer: &Tokenizer,
        projection: &TrainableProjection,
        use_lora: bool,
    ) -> EmbeddingResult<(Tensor, Tensor)> {
        let mut cause_vecs = Vec::new();
        let mut effect_vecs = Vec::new();

        for pair in &batch.pairs {
            // Use document prefix for diverse text-dependent embeddings.
            // Causal instruction prefixes compress all vectors to the same direction,
            // making contrastive learning impossible. The projection heads learn
            // the cause/effect separation instead.
            let cause_prefixed = format!("search_document: {}", &pair.cause_text);
            let cause_emb = if use_lora {
                gpu_forward_with_lora_tensor(&cause_prefixed, weights, tokenizer, &self.lora)?
            } else {
                gpu_forward_base_tensor(&cause_prefixed, weights, tokenizer)?
            };
            let cause_proj = projection.project_cause_trainable(&cause_emb)?;
            let cause_norm = l2_normalize(&cause_proj)?;
            cause_vecs.push(cause_norm);

            let effect_prefixed = format!("search_document: {}", &pair.effect_text);
            let effect_emb = if use_lora {
                gpu_forward_with_lora_tensor(&effect_prefixed, weights, tokenizer, &self.lora)?
            } else {
                gpu_forward_base_tensor(&effect_prefixed, weights, tokenizer)?
            };
            let effect_proj = projection.project_effect_trainable(&effect_emb)?;
            let effect_norm = l2_normalize(&effect_proj)?;
            effect_vecs.push(effect_norm);
        }

        // Stack individual [1, D] tensors into [N, D]
        let cause_refs: Vec<&Tensor> = cause_vecs.iter().collect();
        let effect_refs: Vec<&Tensor> = effect_vecs.iter().collect();

        let cause_batch = Tensor::cat(&cause_refs, 0).map_err(|e| EmbeddingError::GpuError {
            message: format!("Batch cause stack failed: {}", e),
        })?;
        let effect_batch = Tensor::cat(&effect_refs, 0).map_err(|e| EmbeddingError::GpuError {
            message: format!("Batch effect stack failed: {}", e),
        })?;

        Ok((cause_batch, effect_batch))
    }

    /// Embed a batch of texts using LoRA-augmented forward (no projection).
    ///
    /// Returns tensor [N, D] with preserved grad graph.
    pub fn embed_batch_trainable(
        &self,
        texts: &[&str],
        instruction: &str,
        weights: &NomicWeights,
        tokenizer: &Tokenizer,
    ) -> EmbeddingResult<Tensor> {
        let mut vecs = Vec::new();
        for text in texts {
            let prefixed = format!("{}{}", instruction, text);
            let emb = gpu_forward_with_lora_tensor(&prefixed, weights, tokenizer, &self.lora)?;
            vecs.push(emb);
        }

        let refs: Vec<&Tensor> = vecs.iter().collect();
        Tensor::cat(&refs, 0).map_err(|e| EmbeddingError::GpuError {
            message: format!("Batch stack failed: {}", e),
        })
    }

    /// Run one training epoch.
    ///
    /// Uses the trainer's projection for the forward pass, ensuring gradients
    /// flow to the same Var objects tracked by the optimizer.
    ///
    /// When `use_multitask` is true and multi-task heads are configured,
    /// computes auxiliary direction + mechanism classification losses that
    /// provide additional gradient signal to the shared encoder.
    pub fn train_epoch(
        &self,
        trainer: &mut CausalTrainer,
        data_loader: &mut CausalDataLoader,
        weights: &NomicWeights,
        tokenizer: &Tokenizer,
        use_multitask: bool,
        use_lora: bool,
    ) -> EmbeddingResult<(LossComponents, usize)> {
        data_loader.shuffle_epoch();

        let mut total_loss = LossComponents::default();
        let mut num_batches = 0usize;
        let mut batch_idx = 0;

        while let Some(batch) = data_loader.next_batch(batch_idx) {
            if batch.is_empty() {
                batch_idx += 1;
                continue;
            }

            // Use trainer's projection so gradients flow to optimizer-tracked Vars
            let (cause_vecs, effect_vecs) =
                self.embed_batch_dual(&batch, weights, tokenizer, trainer.projection(), use_lora)?;
            let confidences = Tensor::from_slice(
                &batch.soft_labels(),
                batch.len(),
                &self.device,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Confidence tensor failed: {}", e),
            })?;

            // Compute multi-task auxiliary loss if active
            let auxiliary_loss = if use_multitask {
                self.compute_multitask_loss(&batch, &cause_vecs, &effect_vecs)?
            } else {
                None
            };

            let components = trainer.train_step_with_auxiliary(
                &cause_vecs,
                &effect_vecs,
                &confidences,
                auxiliary_loss.as_ref(),
            )?;

            total_loss.contrastive += components.contrastive;
            total_loss.directional += components.directional;
            total_loss.separation += components.separation;
            total_loss.soft_label += components.soft_label;
            total_loss.total += components.total;
            num_batches += 1;
            batch_idx += 1;
        }

        // Average losses
        if num_batches > 0 {
            let n = num_batches as f32;
            total_loss.contrastive /= n;
            total_loss.directional /= n;
            total_loss.separation /= n;
            total_loss.soft_label /= n;
            total_loss.total /= n;
        }

        Ok((total_loss, num_batches))
    }

    /// Compute multi-task auxiliary loss from batch pairs.
    ///
    /// Returns weighted sum: lambda_dir * direction_CE + lambda_mech * mechanism_CE.
    fn compute_multitask_loss(
        &self,
        batch: &TrainingBatch,
        cause_vecs: &Tensor,
        effect_vecs: &Tensor,
    ) -> EmbeddingResult<Option<Tensor>> {
        let heads = match &self.multitask_heads {
            Some(h) => h,
            None => return Ok(None),
        };

        let n = batch.len();
        if n == 0 {
            return Ok(None);
        }

        // Concatenate cause + effect -> [N, 2*D]
        let cat = Tensor::cat(&[cause_vecs, effect_vecs], 1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Multi-task concat failed: {}", e),
            }
        })?;

        // Derive direction labels from training pair metadata
        let dir_labels: Vec<u32> = batch
            .pairs
            .iter()
            .map(|p| match p.direction {
                crate::training::data::TrainingDirection::Forward => {
                    DirectionLabel::Forward.index()
                }
                crate::training::data::TrainingDirection::Backward => {
                    DirectionLabel::Backward.index()
                }
                _ => DirectionLabel::None.index(),
            })
            .collect();

        // Derive mechanism labels from training pair metadata
        let mech_labels: Vec<u32> = batch
            .pairs
            .iter()
            .map(|p| MechanismLabel::from_str(&p.mechanism).index())
            .collect();

        let dir_label_tensor =
            Tensor::from_slice(&dir_labels, n, &self.device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Direction label tensor failed: {}", e),
                }
            })?;

        let mech_label_tensor =
            Tensor::from_slice(&mech_labels, n, &self.device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Mechanism label tensor failed: {}", e),
                }
            })?;

        // Compute classification losses
        let dir_loss = heads.direction_loss(&cat, &dir_label_tensor)?;
        let mech_loss = heads.mechanism_loss(&cat, &mech_label_tensor)?;

        // Weighted sum: lambda_dir * L_dir + lambda_mech * L_mech
        let lambda_dir = heads.config.lambda_direction as f64;
        let lambda_mech = heads.config.lambda_mechanism as f64;

        let weighted = (dir_loss.affine(lambda_dir, 0.0).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Direction loss scaling failed: {}", e),
            }
        })? + mech_loss.affine(lambda_mech, 0.0).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Mechanism loss scaling failed: {}", e),
            }
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Multi-task loss sum failed: {}", e),
        })?;

        // Squeeze [1] -> [] scalar to match main loss shape
        let scalar = weighted.sum_all().map_err(|e| EmbeddingError::GpuError {
            message: format!("Multi-task loss squeeze failed: {}", e),
        })?;

        Ok(Some(scalar))
    }

    /// Evaluate on a held-out set using the given projection.
    ///
    /// Tensors are detached from the grad graph to avoid VRAM accumulation.
    pub fn evaluate(
        &self,
        eval_loader: &mut CausalDataLoader,
        weights: &NomicWeights,
        tokenizer: &Tokenizer,
        projection: &TrainableProjection,
        use_lora: bool,
    ) -> EmbeddingResult<EvaluationMetrics> {
        eval_loader.shuffle_epoch();

        let mut all_cause_vecs = Vec::new();
        let mut all_effect_vecs = Vec::new();
        let mut all_non_causal_sims = Vec::new();
        let mut batch_idx = 0;

        while let Some(batch) = eval_loader.next_batch(batch_idx) {
            if batch.is_empty() {
                batch_idx += 1;
                continue;
            }

            let (cause_vecs, effect_vecs) =
                self.embed_batch_dual(&batch, weights, tokenizer, projection, use_lora)?;

            // Detach from grad graph for evaluation (saves VRAM)
            let cause_vecs = cause_vecs.detach();
            let effect_vecs = effect_vecs.detach();

            // Collect non-causal similarity scores for AUC
            for (i, pair) in batch.pairs.iter().enumerate() {
                if !pair.is_causal() {
                    let cause_row = cause_vecs.get(i).map_err(|e| EmbeddingError::GpuError {
                        message: format!("Get cause row failed: {}", e),
                    })?;
                    let effect_row = effect_vecs.get(i).map_err(|e| EmbeddingError::GpuError {
                        message: format!("Get effect row failed: {}", e),
                    })?;
                    let sim: f32 = (&cause_row * &effect_row)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("Eval dot failed: {}", e),
                        })?
                        .sum_all()
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("Eval sum failed: {}", e),
                        })?
                        .to_scalar()
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("Eval scalar failed: {}", e),
                        })?;
                    all_non_causal_sims.push(sim);
                }
            }

            all_cause_vecs.push(cause_vecs);
            all_effect_vecs.push(effect_vecs);
            batch_idx += 1;
        }

        if all_cause_vecs.is_empty() {
            return Ok(EvaluationMetrics::default());
        }

        let cause_refs: Vec<&Tensor> = all_cause_vecs.iter().collect();
        let effect_refs: Vec<&Tensor> = all_effect_vecs.iter().collect();

        let all_causes = Tensor::cat(&cause_refs, 0).map_err(|e| EmbeddingError::GpuError {
            message: format!("Eval cause cat failed: {}", e),
        })?;
        let all_effects = Tensor::cat(&effect_refs, 0).map_err(|e| EmbeddingError::GpuError {
            message: format!("Eval effect cat failed: {}", e),
        })?;

        Evaluator::evaluate(&all_causes, &all_effects, &all_non_causal_sims)
    }

    /// Run a single training stage.
    fn run_stage(
        &self,
        stage: u32,
        trainer: &mut CausalTrainer,
        train_loader: &mut CausalDataLoader,
        eval_loader: &mut CausalDataLoader,
        weights: &NomicWeights,
        tokenizer: &Tokenizer,
        num_epochs: u32,
        use_multitask: bool,
        use_lora: bool,
    ) -> EmbeddingResult<StageResult> {
        tracing::info!(
            "=== Stage {} starting ({} epochs, multitask={}) ===",
            stage,
            num_epochs,
            use_multitask
        );

        let mut best_metrics: Option<EvaluationMetrics> = None;
        let mut final_loss = LossComponents::default();
        let mut epochs_completed = 0u32;
        let mut epochs_without_improvement = 0u32;

        for epoch in 1..=num_epochs {
            let (loss, num_batches) =
                self.train_epoch(trainer, train_loader, weights, tokenizer, use_multitask, use_lora)?;
            final_loss = loss.clone();
            epochs_completed = epoch;

            tracing::info!(
                "Stage {} Epoch {}/{}: loss={:.4} (CE={:.4} Dir={:.4} Sep={:.4} Soft={:.4}) batches={}",
                stage,
                epoch,
                num_epochs,
                loss.total,
                loss.contrastive,
                loss.directional,
                loss.separation,
                loss.soft_label,
                num_batches,
            );

            // Evaluate periodically
            if epoch % self.config.eval_every == 0 || epoch == num_epochs {
                let metrics = self.evaluate(eval_loader, weights, tokenizer, trainer.projection(), use_lora)?;
                tracing::info!("Stage {} Eval: {}", stage, metrics.summary());

                let is_better = best_metrics
                    .as_ref()
                    .map(|best| metrics.directional_accuracy > best.directional_accuracy)
                    .unwrap_or(true);

                if is_better {
                    best_metrics = Some(metrics);
                    epochs_without_improvement = 0;

                    // Save checkpoint (from trainer's projection, which has the learned weights)
                    let ckpt_path = self.config.output_dir.join(format!(
                        "projection_stage{}_best.safetensors",
                        stage
                    ));
                    trainer.projection().save_trained(&ckpt_path)?;
                    tracing::info!("Stage {} best checkpoint saved to {}", stage, ckpt_path.display());
                } else {
                    epochs_without_improvement += self.config.eval_every;
                }

                let result = EpochResult {
                    epoch,
                    avg_loss: loss,
                    num_batches,
                    eval_metrics: best_metrics.clone(),
                    is_best: is_better,
                };
                if !trainer.record_epoch(result) {
                    tracing::info!(
                        "Stage {} early stopping at epoch {}",
                        stage,
                        epoch
                    );
                    return Ok(StageResult {
                        stage,
                        epochs_completed,
                        best_metrics,
                        final_loss,
                        early_stopped: true,
                    });
                }
            }

            // Early stopping check
            if epochs_without_improvement >= self.config.early_stopping_patience {
                tracing::info!(
                    "Stage {} early stopping: {} epochs without improvement",
                    stage,
                    epochs_without_improvement
                );
                return Ok(StageResult {
                    stage,
                    epochs_completed,
                    best_metrics,
                    final_loss,
                    early_stopped: true,
                });
            }
        }

        Ok(StageResult {
            stage,
            epochs_completed,
            best_metrics,
            final_loss,
            early_stopped: false,
        })
    }

    /// Quick benchmark callback: sample pairs, compute 3 fast metrics, log to trajectory file.
    ///
    /// Called after each training stage to track improvement over time.
    fn log_benchmark_snapshot(
        &self,
        stage: u32,
        eval_loader: &mut CausalDataLoader,
        weights: &NomicWeights,
        tokenizer: &Tokenizer,
        best_spread: &mut f32,
        projection: &TrainableProjection,
        use_lora: bool,
    ) {
        let snapshot_start = std::time::Instant::now();

        match self.evaluate(eval_loader, weights, tokenizer, projection, use_lora) {
            Ok(metrics) => {
                let snapshot = serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "stage": stage,
                    "score_spread": metrics.score_spread,
                    "directional_accuracy": metrics.directional_accuracy,
                    "standalone_top1": metrics.standalone_accuracy,
                    "direction_ratio": metrics.direction_ratio,
                    "duration_ms": snapshot_start.elapsed().as_millis() as u64,
                });

                // Warn on regression
                if metrics.score_spread < *best_spread && *best_spread > 0.0 {
                    tracing::warn!(
                        "Benchmark regression: score_spread {:.4} < previous best {:.4}",
                        metrics.score_spread,
                        *best_spread
                    );
                }
                if metrics.score_spread > *best_spread {
                    *best_spread = metrics.score_spread;
                }

                // Append to trajectory file
                let trajectory_path = self.config.output_dir.join("training_trajectory.jsonl");
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&trajectory_path)
                {
                    use std::io::Write;
                    let _ = writeln!(file, "{}", snapshot);
                    tracing::info!(
                        "Benchmark snapshot (stage {}): spread={:.4}, dir_acc={:.4}, top1={:.4}",
                        stage,
                        metrics.score_spread,
                        metrics.directional_accuracy,
                        metrics.standalone_accuracy,
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Benchmark snapshot failed: {}", e);
            }
        }
    }

    /// Load a projection from a previous stage checkpoint, falling back to fresh init.
    fn load_stage_projection(
        &self,
        prev_stage: u32,
    ) -> EmbeddingResult<TrainableProjection> {
        let prev_path = self.config.output_dir.join(format!(
            "projection_stage{}_best.safetensors",
            prev_stage
        ));
        if prev_path.exists() {
            match TrainableProjection::load_trained(&prev_path, &self.device) {
                Ok(proj) => {
                    tracing::info!(
                        "Loaded stage {} projection checkpoint for cross-stage continuity",
                        prev_stage
                    );
                    return Ok(proj);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load stage {} checkpoint ({}), using fresh init",
                        prev_stage,
                        e
                    );
                }
            }
        }
        TrainableProjection::new(CAUSAL_DIMENSION, &self.device)
    }

    /// Run the full 3-stage progressive training pipeline.
    pub fn run_full_pipeline(
        &self,
        train_pairs: Vec<CausalTrainingPair>,
        eval_pairs: Vec<CausalTrainingPair>,
        weights: &NomicWeights,
        tokenizer: &Tokenizer,
    ) -> EmbeddingResult<PipelineResult> {
        // Ensure output directory exists
        std::fs::create_dir_all(&self.config.output_dir).map_err(|e| {
            EmbeddingError::InternalError {
                message: format!("Failed to create output dir: {}", e),
            }
        })?;

        let mut stages = Vec::new();
        let mut best_overall: Option<EvaluationMetrics> = None;
        let mut best_spread: f32 = 0.0;
        let mut best_stage: u32 = 0;

        // === Stage 1: Projection-only warm-up with easy pairs ===
        {
            tracing::info!("=== STAGE 1: Projection warm-up (easy pairs, {} epochs) ===", self.config.stage1_epochs);

            // Filter to easy pairs for curriculum learning
            let easy_pairs: Vec<CausalTrainingPair> = train_pairs
                .iter()
                .filter(|p| p.difficulty() <= self.config.stage1_max_difficulty)
                .cloned()
                .collect();

            let easy_count = easy_pairs.len();
            tracing::info!("Stage 1: {} easy pairs (difficulty <= {})", easy_count, self.config.stage1_max_difficulty);

            // If no easy pairs, use all pairs
            let stage1_pairs = if easy_count < 4 { train_pairs.clone() } else { easy_pairs };

            let mut stage1_train = CausalDataLoader::new(stage1_pairs, self.config.batch_size, self.config.seed);
            let mut stage1_eval = CausalDataLoader::new(eval_pairs.clone(), self.config.batch_size, self.config.seed + 1);

            let training_config = TrainingConfig {
                batch_size: self.config.batch_size as u32,
                epochs: self.config.stage1_epochs,
                eval_every: self.config.eval_every,
                early_stopping_patience: self.config.early_stopping_patience,
                checkpoint_dir: self.config.output_dir.clone(),
                ..Default::default()
            };

            // Stage 1: fresh projection (no prior checkpoint)
            let stage1_projection = TrainableProjection::new(CAUSAL_DIMENSION, &self.device)?;
            let mut trainer = CausalTrainer::new(
                stage1_projection,
                training_config,
                self.device.clone(),
            )?;
            trainer.register_params()?;

            let stage1_result = self.run_stage(
                1,
                &mut trainer,
                &mut stage1_train,
                &mut stage1_eval,
                weights,
                tokenizer,
                self.config.stage1_epochs,
                false, // No multi-task in Stage 1 (projection warm-up only)
                false, // No LoRA in Stage 1 (saves VRAM for projection-only warm-up)
            )?;

            if let Some(ref m) = stage1_result.best_metrics {
                best_overall = Some(m.clone());
                best_stage = 1;
            }
            stages.push(stage1_result);

            // Benchmark snapshot after stage 1 (use trainer's projection before it's dropped)
            let mut snapshot_eval = CausalDataLoader::new(eval_pairs.clone(), self.config.batch_size, self.config.seed + 100);
            self.log_benchmark_snapshot(1, &mut snapshot_eval, weights, tokenizer, &mut best_spread, trainer.projection(), false);

            // Explicitly drop trainer to free optimizer state + Var tensors before Stage 2
            drop(trainer);
            drop(snapshot_eval);
        }

        // Sync GPU to ensure all Stage 1 memory is freed before Stage 2 allocation
        if let Device::Cuda(cuda_dev) = &self.device {
            let _ = cuda_dev.synchronize();
        }

        // === Stage 2: LoRA activation with all pairs ===
        {
            tracing::info!("=== STAGE 2: LoRA + Projection (all pairs, {} epochs) ===", self.config.stage2_epochs);

            let mut stage2_train = CausalDataLoader::new(train_pairs.clone(), self.config.batch_size, self.config.seed + 10);
            let mut stage2_eval = CausalDataLoader::new(eval_pairs.clone(), self.config.batch_size, self.config.seed + 11);

            let mut optimizer_config = AdamWConfig::default();
            let total_steps = (train_pairs.len() / self.config.batch_size + 1) * self.config.stage2_epochs as usize;
            optimizer_config.total_steps = total_steps;

            let training_config = TrainingConfig {
                batch_size: self.config.batch_size as u32,
                epochs: self.config.stage2_epochs,
                eval_every: self.config.eval_every,
                early_stopping_patience: self.config.early_stopping_patience,
                checkpoint_dir: self.config.output_dir.clone(),
                optimizer_config,
                ..Default::default()
            };

            // Load Stage 1's best projection for cross-stage continuity
            let stage2_projection = self.load_stage_projection(1)?;
            let mut trainer = CausalTrainer::new(
                stage2_projection,
                training_config,
                self.device.clone(),
            )?;
            trainer.register_params()?;

            // Register LoRA parameters with lower learning rate
            let optimizer = trainer.optimizer_mut();
            for var in self.lora.all_trainable_vars() {
                optimizer.add_param(var.clone(), ParamGroup::Lora)?;
            }

            // Register multi-task head parameters (direction + mechanism classifiers)
            if let Some(ref heads) = self.multitask_heads {
                let optimizer = trainer.optimizer_mut();
                for var in heads.all_trainable_vars() {
                    optimizer.add_param(var.clone(), ParamGroup::Markers)?;
                }
                tracing::info!("Stage 2: registered {} multi-task params", heads.total_params());
            }

            let stage2_result = self.run_stage(
                2,
                &mut trainer,
                &mut stage2_train,
                &mut stage2_eval,
                weights,
                tokenizer,
                self.config.stage2_epochs,
                true, // Multi-task active in Stage 2
                true, // LoRA active in Stage 2
            )?;

            if let Some(ref m) = stage2_result.best_metrics {
                let is_better = best_overall.as_ref()
                    .map(|best| m.directional_accuracy > best.directional_accuracy
                        || (m.directional_accuracy == best.directional_accuracy && best_stage < 2))
                    .unwrap_or(true);
                if is_better {
                    best_overall = Some(m.clone());
                    best_stage = 2;
                }
            }
            stages.push(stage2_result);

            // Always save final projection at end of stage (guards against eval-metric ties
            // that prevent checkpoint updates during training)
            let final_path = self.config.output_dir.join("projection_stage2_best.safetensors");
            trainer.projection().save_trained(&final_path)?;
            tracing::info!("Stage 2 final projection saved to {}", final_path.display());

            // Benchmark snapshot after stage 2 (use trainer's projection before it's dropped)
            let mut snapshot_eval = CausalDataLoader::new(eval_pairs.clone(), self.config.batch_size, self.config.seed + 101);
            self.log_benchmark_snapshot(2, &mut snapshot_eval, weights, tokenizer, &mut best_spread, trainer.projection(), true);

            // Explicitly drop trainer to free optimizer state + Var tensors before Stage 3
            drop(trainer);
            drop(snapshot_eval);
        }

        // Sync GPU to ensure all Stage 2 memory is freed before Stage 3 allocation
        if let Device::Cuda(cuda_dev) = &self.device {
            let _ = cuda_dev.synchronize();
        }

        // === Stage 3: Directional emphasis ===
        {
            tracing::info!("=== STAGE 3: Directional emphasis (all pairs, {} epochs) ===", self.config.stage3_epochs);

            let mut stage3_train = CausalDataLoader::new(train_pairs.clone(), self.config.batch_size, self.config.seed + 20);
            let mut stage3_eval = CausalDataLoader::new(eval_pairs.clone(), self.config.batch_size, self.config.seed + 21);

            let loss_config = LossConfig {
                lambda_directional: self.config.stage3_direction_weight,
                ..Default::default()
            };

            let mut optimizer_config = AdamWConfig::default();
            let total_steps = (train_pairs.len() / self.config.batch_size + 1) * self.config.stage3_epochs as usize;
            optimizer_config.total_steps = total_steps;

            let training_config = TrainingConfig {
                batch_size: self.config.batch_size as u32,
                epochs: self.config.stage3_epochs,
                eval_every: self.config.eval_every,
                early_stopping_patience: self.config.early_stopping_patience,
                checkpoint_dir: self.config.output_dir.clone(),
                loss_config,
                optimizer_config,
                ..Default::default()
            };

            // Load Stage 2's best projection for cross-stage continuity
            let stage3_projection = self.load_stage_projection(2)?;
            let mut trainer = CausalTrainer::new(
                stage3_projection,
                training_config,
                self.device.clone(),
            )?;
            trainer.register_params()?;

            let optimizer = trainer.optimizer_mut();
            for var in self.lora.all_trainable_vars() {
                optimizer.add_param(var.clone(), ParamGroup::Lora)?;
            }

            // Register multi-task head parameters for Stage 3
            if let Some(ref heads) = self.multitask_heads {
                let optimizer = trainer.optimizer_mut();
                for var in heads.all_trainable_vars() {
                    optimizer.add_param(var.clone(), ParamGroup::Markers)?;
                }
            }

            let stage3_result = self.run_stage(
                3,
                &mut trainer,
                &mut stage3_train,
                &mut stage3_eval,
                weights,
                tokenizer,
                self.config.stage3_epochs,
                true, // Multi-task active in Stage 3
                true, // LoRA active in Stage 3
            )?;

            if let Some(ref m) = stage3_result.best_metrics {
                let is_better = best_overall.as_ref()
                    .map(|best| m.directional_accuracy > best.directional_accuracy
                        || (m.directional_accuracy == best.directional_accuracy && best_stage < 3))
                    .unwrap_or(true);
                if is_better {
                    best_overall = Some(m.clone());
                    best_stage = 3;
                }
            }
            stages.push(stage3_result);

            // Always save final projection at end of stage (guards against eval-metric ties)
            let final_path = self.config.output_dir.join("projection_stage3_best.safetensors");
            trainer.projection().save_trained(&final_path)?;
            tracing::info!("Stage 3 final projection saved to {}", final_path.display());

            // Final benchmark snapshot after stage 3 (use trainer's projection before it's dropped)
            let mut snapshot_eval = CausalDataLoader::new(eval_pairs.clone(), self.config.batch_size, self.config.seed + 102);
            self.log_benchmark_snapshot(3, &mut snapshot_eval, weights, tokenizer, &mut best_spread, trainer.projection(), true);
        }

        let total_epochs: u32 = stages.iter().map(|s| s.epochs_completed).sum();

        // Save final best checkpoint (projection + LoRA)
        let checkpoint_path = if best_overall.is_some() && best_stage > 0 {
            // Copy best stage's projection checkpoint as the final best
            let best_stage_path = self.config.output_dir.join(format!(
                "projection_stage{}_best.safetensors",
                best_stage
            ));
            let final_path = self.config.output_dir.join("projection_best.safetensors");

            if best_stage_path.exists() {
                std::fs::copy(&best_stage_path, &final_path).map_err(|e| {
                    EmbeddingError::InternalError {
                        message: format!("Failed to copy best projection checkpoint: {}", e),
                    }
                })?;
                tracing::info!(
                    "Final best projection (stage {}) saved to {}",
                    best_stage,
                    final_path.display()
                );
            }

            let lora_path = self.config.output_dir.join("lora_best.safetensors");
            self.save_lora(&lora_path)?;

            Some(final_path)
        } else {
            None
        };

        tracing::info!(
            "Pipeline complete: {} stages, {} total epochs, best stage={}",
            stages.len(),
            total_epochs,
            best_stage,
        );
        if let Some(ref m) = best_overall {
            tracing::info!("Best metrics: {}", m.summary());
        }

        Ok(PipelineResult {
            stages,
            best_metrics: best_overall,
            total_epochs,
            checkpoint_path,
        })
    }

    /// Save LoRA weights to a file.
    pub fn save_lora(&self, path: &Path) -> EmbeddingResult<()> {
        use std::collections::HashMap;

        let mut tensors: HashMap<String, Tensor> = HashMap::new();
        for (i, adapter) in self.lora.query_adapters.iter().enumerate() {
            tensors.insert(format!("lora.query.{}.a", i), adapter.a.as_tensor().clone());
            tensors.insert(format!("lora.query.{}.b", i), adapter.b.as_tensor().clone());
        }
        for (i, adapter) in self.lora.value_adapters.iter().enumerate() {
            tensors.insert(format!("lora.value.{}.a", i), adapter.a.as_tensor().clone());
            tensors.insert(format!("lora.value.{}.b", i), adapter.b.as_tensor().clone());
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| EmbeddingError::InternalError {
                message: format!("Failed to create LoRA dir: {}", e),
            })?;
        }

        // Serialize LoRA weights using safetensors
        let tensor_data: Vec<(String, Vec<f32>, Vec<usize>)> = tensors
            .iter()
            .map(|(k, v)| {
                let data: Vec<f32> = v
                    .flatten_all()
                    .map_err(|e| EmbeddingError::InternalError {
                        message: format!("Flatten '{}' failed: {}", k, e),
                    })?
                    .to_vec1()
                    .map_err(|e| EmbeddingError::InternalError {
                        message: format!("to_vec1 '{}' failed: {}", k, e),
                    })?;
                let shape: Vec<usize> = v.shape().dims().to_vec();
                Ok((k.clone(), data, shape))
            })
            .collect::<Result<Vec<_>, EmbeddingError>>()?;

        let views: Vec<(String, safetensors::tensor::TensorView<'_>)> = tensor_data
            .iter()
            .map(|(k, data, shape)| {
                let view = safetensors::tensor::TensorView::new(
                    safetensors::Dtype::F32,
                    shape.clone(),
                    bytemuck::cast_slice(data.as_slice()),
                )
                .map_err(|e| EmbeddingError::InternalError {
                    message: format!("TensorView for '{}' failed: {}", k, e),
                })?;
                Ok((k.clone(), view))
            })
            .collect::<Result<Vec<_>, EmbeddingError>>()?;

        safetensors::tensor::serialize_to_file(
            views.iter().map(|(k, v)| (k.clone(), v.clone())),
            &None::<HashMap<String, String>>,
            path,
        )
        .map_err(|e| EmbeddingError::InternalError {
            message: format!("Failed to save LoRA weights: {}", e),
        })?;

        tracing::info!("Saved LoRA weights to {}", path.display());
        Ok(())
    }
}

// Helper: TrainableProjection needs a clone method for creating per-stage trainers.
impl TrainableProjection {
    /// Create a fresh TrainableProjection for a new training stage.
    ///
    /// Returns a newly-initialized projection (not a weight clone).
    /// Prefer `load_stage_projection()` for cross-stage continuity.
    pub fn new_for_stage(&self) -> EmbeddingResult<Self> {
        let device = self.cause_projection_var.as_tensor().device().clone();
        Self::new(self.hidden_size, &device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_config_default() {
        let config = PipelineConfig::default();
        assert_eq!(config.stage1_epochs, 25);
        assert_eq!(config.stage2_epochs, 20);
        assert_eq!(config.stage3_epochs, 20);
        assert_eq!(config.batch_size, 16);
        assert!(config.multitask_config.is_some());
    }

    #[test]
    fn test_pipeline_creation() {
        let config = PipelineConfig {
            lora_config: LoraConfig {
                num_layers: 2,
                hidden_size: 8,
                rank: 4,
                ..Default::default()
            },
            multitask_config: Some(MultiTaskConfig {
                input_dim: 8,
                hidden_dim: 4,
                ..Default::default()
            }),
            ..Default::default()
        };
        let pipeline = CausalTrainingPipeline::new(config, Device::Cpu).unwrap();
        assert!(pipeline.lora.total_params() > 0);
        assert!(pipeline.multitask_heads.is_some());
        assert!(pipeline.multitask_heads.as_ref().unwrap().total_params() > 0);
    }

    #[test]
    fn test_pipeline_creation_no_multitask() {
        let config = PipelineConfig {
            lora_config: LoraConfig {
                num_layers: 2,
                hidden_size: 8,
                rank: 4,
                ..Default::default()
            },
            multitask_config: None,
            ..Default::default()
        };
        let pipeline = CausalTrainingPipeline::new(config, Device::Cpu).unwrap();
        assert!(pipeline.lora.total_params() > 0);
        assert!(pipeline.multitask_heads.is_none());
    }
}
