//! Loss functions for causal embedder fine-tuning.
//!
//! Four loss components informed by Causal2Vec, A-InfoNCE, and contrastive learning literature:
//! 1. InfoNCE contrastive (τ=0.05)
//! 2. Directional margin (margin=0.2)
//! 3. Causal separation
//! 4. Soft label distillation

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

/// Configuration for the combined loss function.
#[derive(Debug, Clone)]
pub struct LossConfig {
    /// Temperature for InfoNCE (default: 0.05 per Causal2Vec).
    pub temperature: f32,
    /// Margin for directional loss (default: 0.2).
    pub margin: f32,
    /// Weight for contrastive loss (default: 1.0).
    pub lambda_contrastive: f32,
    /// Weight for directional loss (default: 0.3).
    pub lambda_directional: f32,
    /// Weight for separation loss (default: 0.1).
    pub lambda_separation: f32,
    /// Weight for soft label loss (default: 0.2).
    pub lambda_soft: f32,
    /// Minimum E1 similarity threshold to exclude potential false negatives.
    pub false_negative_threshold: f32,
    /// Scale factor for hard negative logits in InfoNCE (default: 1.0 = disabled).
    /// Hard negatives (top-k most similar incorrect pairs) get their logits
    /// multiplied by this factor before softmax, amplifying gradient signal.
    /// WARNING: Values >= 2.0 with temperature=0.05 cause catastrophic score
    /// collapse — cause/effect vectors become orthogonal. Use <= 1.3 if enabling.
    pub hard_negative_scale: f32,
    /// Number of hard negatives per sample to amplify (default: 3).
    pub hard_negative_count: usize,
}

impl Default for LossConfig {
    fn default() -> Self {
        Self {
            temperature: 0.05,
            margin: 0.2,
            lambda_contrastive: 1.0,
            lambda_directional: 0.3,
            lambda_separation: 0.1,
            lambda_soft: 0.2,
            false_negative_threshold: 0.8,
            hard_negative_scale: 1.0,
            hard_negative_count: 3,
        }
    }
}

/// Combined loss function for causal embedder training.
pub struct DirectionalContrastiveLoss {
    config: LossConfig,
}

/// Per-component loss values for logging.
#[derive(Debug, Clone, Default)]
pub struct LossComponents {
    /// InfoNCE contrastive loss value.
    pub contrastive: f32,
    /// Directional margin loss value.
    pub directional: f32,
    /// Causal separation loss value.
    pub separation: f32,
    /// Soft label distillation loss value.
    pub soft_label: f32,
    /// Total combined loss.
    pub total: f32,
}

impl DirectionalContrastiveLoss {
    /// Create a new loss function with the given configuration.
    pub fn new(config: LossConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn default_config() -> Self {
        Self::new(LossConfig::default())
    }

    /// Compute InfoNCE contrastive loss.
    ///
    /// Pulls cause→effect pairs together, pushes non-causal pairs apart.
    /// Uses in-batch negatives: each batch of N pairs provides N*(N-1) free negatives.
    ///
    /// L = -log(exp(sim(cause_i, effect_i)/τ) / Σ_j exp(sim(cause_i, effect_j)/τ))
    ///
    /// Uses candle_nn::loss::cross_entropy for differentiable gather-based indexing.
    ///
    /// # Arguments
    /// * `cause_vecs` - Cause embeddings [N, D] (L2-normalized)
    /// * `effect_vecs` - Effect embeddings [N, D] (L2-normalized)
    pub fn info_nce_loss(
        &self,
        cause_vecs: &Tensor,
        effect_vecs: &Tensor,
    ) -> EmbeddingResult<Tensor> {
        let tau = self.config.temperature as f64;
        let n = cause_vecs.dim(0).map_err(map_candle)?;

        // Cosine similarity matrix: [N, N] where sim[i,j] = cos(cause_i, effect_j)
        let sim_matrix = cause_vecs
            .matmul(&effect_vecs.t().map_err(map_candle)?)
            .map_err(map_candle)?;

        // Scale by temperature
        let logits = sim_matrix.affine(1.0 / tau, 0.0).map_err(map_candle)?;

        // Online hard negative mining: amplify top-k hardest negative logits per row
        // This focuses gradient signal on the most confusing negatives, improving separation.
        // Semi-hard strategy: only amplify negatives with sim < positive sim (avoid mislabeled).
        let logits = if n > 2 && self.config.hard_negative_scale > 1.0 {
            self.apply_hard_negative_mining(&logits, n)?
        } else {
            logits
        };

        // Labels: diagonal (each cause_i pairs with effect_i)
        let labels = Tensor::arange(0u32, n as u32, cause_vecs.device()).map_err(map_candle)?;

        // Differentiable cross-entropy via gather (maintains computation graph)
        candle_nn::loss::cross_entropy(&logits, &labels).map_err(map_candle)
    }

    /// Apply online hard negative mining by scaling up the hardest negative logits.
    ///
    /// For each row i, identifies the top-k off-diagonal logits that are below
    /// the diagonal (positive) logit (semi-hard strategy), and multiplies them
    /// by `hard_negative_scale`. This amplifies gradient on confusing negatives.
    fn apply_hard_negative_mining(&self, logits: &Tensor, n: usize) -> EmbeddingResult<Tensor> {
        let scale = self.config.hard_negative_scale;
        let k = self.config.hard_negative_count.min(n - 1);

        // Extract logits to CPU for mask computation
        let logits_data: Vec<Vec<f32>> = (0..n)
            .map(|i| {
                logits
                    .get(i)
                    .and_then(|row| row.to_vec1::<f32>())
                    .unwrap_or_else(|e| {
                        tracing::warn!(
                            "Hard negative mining: failed to extract logit row {}: {}",
                            i,
                            e
                        );
                        vec![0.0; n]
                    })
            })
            .collect();

        // Build scaling mask: 1.0 for normal, `scale` for hard negatives
        let mut mask_data = vec![1.0f32; n * n];
        for i in 0..n {
            let positive_logit = logits_data[i][i]; // diagonal = positive
                                                    // Collect off-diagonal (negative) logits with their indices
            let mut negatives: Vec<(usize, f32)> = logits_data[i]
                .iter()
                .enumerate()
                .filter(|&(j, &val)| j != i && val < positive_logit) // semi-hard: only below positive
                .map(|(j, &val)| (j, val))
                .collect();
            // Sort descending by logit value — highest negatives are "hardest"
            negatives.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            // Scale top-k hardest
            for &(j, _) in negatives.iter().take(k) {
                mask_data[i * n + j] = scale;
            }
        }

        let mask = Tensor::from_vec(mask_data, (n, n), logits.device()).map_err(map_candle)?;
        logits.mul(&mask).map_err(map_candle)
    }

    /// Compute directional margin loss.
    ///
    /// Enforces that correct cause→effect pairing scores higher than mismatched
    /// pairings by at least `margin`. Compares diagonal (paired) similarity
    /// against mean off-diagonal (wrong pairing) similarity.
    ///
    /// L = max(0, margin - (sim_paired - sim_wrong_mean))
    ///
    /// For N=1, falls back to pushing paired similarity above margin.
    ///
    /// # Arguments
    /// * `cause_vecs` - Cause embeddings [N, D] (L2-normalized)
    /// * `effect_vecs` - Effect embeddings [N, D] (L2-normalized)
    pub fn directional_margin_loss(
        &self,
        cause_vecs: &Tensor,
        effect_vecs: &Tensor,
    ) -> EmbeddingResult<Tensor> {
        let n = cause_vecs.dim(0).map_err(map_candle)?;

        // Paired (diagonal) similarity: dot(cause_i, effect_i)
        let diag_sim = (cause_vecs * effect_vecs)
            .map_err(map_candle)?
            .sum(1)
            .map_err(map_candle)?; // [N]

        if n < 2 {
            // Single sample: push paired similarity above margin
            let margin_t = Tensor::ones_like(&diag_sim)
                .map_err(map_candle)?
                .affine(self.config.margin as f64, 0.0)
                .map_err(map_candle)?;
            let loss = margin_t.sub(&diag_sim).map_err(map_candle)?;
            let zeros = Tensor::zeros_like(&loss).map_err(map_candle)?;
            return loss
                .maximum(&zeros)
                .map_err(map_candle)?
                .mean_all()
                .map_err(map_candle);
        }

        // Full cosine similarity matrix [N, N]
        let cos_matrix = cause_vecs
            .matmul(&effect_vecs.t().map_err(map_candle)?)
            .map_err(map_candle)?;

        // Off-diagonal mean per row: (row_sum - diagonal) / (N-1)
        let row_sum = cos_matrix.sum(1).map_err(map_candle)?; // [N]
        let off_diag_mean = row_sum
            .sub(&diag_sim)
            .map_err(map_candle)?
            .affine(1.0 / (n as f64 - 1.0), 0.0)
            .map_err(map_candle)?; // [N]

        // Gap between correct pairing and wrong pairings
        let gap = diag_sim.sub(&off_diag_mean).map_err(map_candle)?;

        // Loss: max(0, margin - gap)
        let margin_t = Tensor::ones_like(&gap)
            .map_err(map_candle)?
            .affine(self.config.margin as f64, 0.0)
            .map_err(map_candle)?;
        let loss = margin_t.sub(&gap).map_err(map_candle)?;
        let zeros = Tensor::zeros_like(&loss).map_err(map_candle)?;
        loss.maximum(&zeros)
            .map_err(map_candle)?
            .mean_all()
            .map_err(map_candle)
    }

    /// Compute causal separation loss.
    ///
    /// Same text's cause-vector and effect-vector should differ.
    /// L = -distance(cause_vec, effect_vec) for same input text.
    ///
    /// # Arguments
    /// * `cause_vecs` - Cause embeddings [N, D] (from same texts)
    /// * `effect_vecs` - Effect embeddings [N, D] (from same texts)
    pub fn separation_loss(
        &self,
        cause_vecs: &Tensor,
        effect_vecs: &Tensor,
    ) -> EmbeddingResult<Tensor> {
        // Cosine similarity between paired cause/effect vectors
        let sim = batch_cosine_similarity(cause_vecs, effect_vecs)?;

        // We want to minimize similarity → loss = mean(sim)
        // (Equivalent to maximizing distance)
        sim.mean_all().map_err(map_candle)
    }

    /// Compute soft label distillation loss.
    ///
    /// Uses LLM confidence as soft target instead of hard binary labels.
    /// L_soft = MSE(sim(cause, effect), confidence)
    ///
    /// # Arguments
    /// * `cause_vecs` - Cause embeddings [N, D]
    /// * `effect_vecs` - Effect embeddings [N, D]
    /// * `confidences` - LLM confidence scores [N] as soft labels
    pub fn soft_label_loss(
        &self,
        cause_vecs: &Tensor,
        effect_vecs: &Tensor,
        confidences: &Tensor,
    ) -> EmbeddingResult<Tensor> {
        // Predicted similarity
        let sim = batch_cosine_similarity(cause_vecs, effect_vecs)?;

        // MSE: mean((sim - confidence)^2)
        let diff = sim.sub(confidences).map_err(map_candle)?;
        let sq = diff.sqr().map_err(map_candle)?;
        sq.mean_all().map_err(map_candle)
    }

    /// Compute the combined loss.
    ///
    /// L = λ_c * L_contrastive + λ_d * L_directional + λ_s * L_separation + λ_soft * L_soft
    ///
    /// # Arguments
    /// * `cause_vecs` - Cause embeddings [N, D] (L2-normalized)
    /// * `effect_vecs` - Effect embeddings [N, D] (L2-normalized)
    /// * `confidences` - LLM confidence scores [N]
    ///
    /// # Returns
    /// (total_loss_tensor, LossComponents for logging)
    pub fn compute(
        &self,
        cause_vecs: &Tensor,
        effect_vecs: &Tensor,
        confidences: &Tensor,
    ) -> EmbeddingResult<(Tensor, LossComponents)> {
        let l_contrastive = self.info_nce_loss(cause_vecs, effect_vecs)?;
        let l_directional = self.directional_margin_loss(cause_vecs, effect_vecs)?;
        let l_separation = self.separation_loss(cause_vecs, effect_vecs)?;
        let l_soft = self.soft_label_loss(cause_vecs, effect_vecs, confidences)?;

        // Weighted combination
        let total = l_contrastive
            .affine(self.config.lambda_contrastive as f64, 0.0)
            .map_err(map_candle)?
            .add(
                &l_directional
                    .affine(self.config.lambda_directional as f64, 0.0)
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?
            .add(
                &l_separation
                    .affine(self.config.lambda_separation as f64, 0.0)
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?
            .add(
                &l_soft
                    .affine(self.config.lambda_soft as f64, 0.0)
                    .map_err(map_candle)?,
            )
            .map_err(map_candle)?;

        // Extract scalar values for logging
        let components = LossComponents {
            contrastive: tensor_to_f32(&l_contrastive)?,
            directional: tensor_to_f32(&l_directional)?,
            separation: tensor_to_f32(&l_separation)?,
            soft_label: tensor_to_f32(&l_soft)?,
            total: tensor_to_f32(&total)?,
        };

        Ok((total, components))
    }

    /// Get the loss configuration.
    pub fn config(&self) -> &LossConfig {
        &self.config
    }
}

/// Compute batch cosine similarity between paired vectors.
/// Returns [N] where result[i] = cos(a[i], b[i]).
fn batch_cosine_similarity(a: &Tensor, b: &Tensor) -> EmbeddingResult<Tensor> {
    // Element-wise multiply and sum over last dimension
    let dot = (a * b).map_err(map_candle)?.sum(1).map_err(map_candle)?;

    // Norms
    let norm_a = a
        .sqr()
        .map_err(map_candle)?
        .sum(1)
        .map_err(map_candle)?
        .sqrt()
        .map_err(map_candle)?;
    let norm_b = b
        .sqr()
        .map_err(map_candle)?
        .sum(1)
        .map_err(map_candle)?
        .sqrt()
        .map_err(map_candle)?;

    let denom = (norm_a * norm_b).map_err(map_candle)?;

    // Add epsilon to avoid division by zero
    let eps = Tensor::ones_like(&denom)
        .map_err(map_candle)?
        .affine(1e-8, 0.0)
        .map_err(map_candle)?;
    let safe_denom = denom.add(&eps).map_err(map_candle)?;

    dot.div(&safe_denom).map_err(map_candle)
}

/// Extract a scalar f32 from a 0-dim or 1-element tensor.
fn tensor_to_f32(t: &Tensor) -> EmbeddingResult<f32> {
    let flat = t.flatten_all().map_err(map_candle)?;
    let val: f32 = flat.to_vec1::<f32>().map_err(map_candle)?[0];
    Ok(val)
}

/// Map candle errors to EmbeddingError.
fn map_candle(e: candle_core::Error) -> EmbeddingError {
    EmbeddingError::GpuError {
        message: format!("Loss computation error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    fn make_test_vecs(n: usize, d: usize) -> (Tensor, Tensor) {
        let device = Device::Cpu;
        // Create normalized random vectors
        let cause_data: Vec<f32> = (0..n * d).map(|i| (i as f32 * 0.1).sin()).collect();
        let effect_data: Vec<f32> = (0..n * d).map(|i| (i as f32 * 0.2 + 1.0).cos()).collect();

        let cause = Tensor::from_slice(&cause_data, (n, d), &device).unwrap();
        let effect = Tensor::from_slice(&effect_data, (n, d), &device).unwrap();

        // L2 normalize
        let cause_norm = cause
            .sqr()
            .unwrap()
            .sum(1)
            .unwrap()
            .sqrt()
            .unwrap()
            .unsqueeze(1)
            .unwrap();
        let effect_norm = effect
            .sqr()
            .unwrap()
            .sum(1)
            .unwrap()
            .sqrt()
            .unwrap()
            .unsqueeze(1)
            .unwrap();

        let cause = cause.broadcast_div(&cause_norm).unwrap();
        let effect = effect.broadcast_div(&effect_norm).unwrap();

        (cause, effect)
    }

    #[test]
    fn test_info_nce_loss_positive() {
        let loss_fn = DirectionalContrastiveLoss::default_config();
        let (cause, effect) = make_test_vecs(4, 16);

        let loss = loss_fn.info_nce_loss(&cause, &effect).unwrap();
        let val: f32 = loss.flatten_all().unwrap().to_vec1().unwrap()[0];

        // Loss should be positive (it's a cross-entropy)
        assert!(val > 0.0, "InfoNCE loss should be positive, got {}", val);
    }

    #[test]
    fn test_directional_margin_loss() {
        let loss_fn = DirectionalContrastiveLoss::default_config();
        let (cause, effect) = make_test_vecs(4, 16);

        let loss = loss_fn.directional_margin_loss(&cause, &effect).unwrap();
        let val: f32 = loss.flatten_all().unwrap().to_vec1().unwrap()[0];

        // Loss should be non-negative (ReLU)
        assert!(val >= 0.0, "Directional loss should be >= 0, got {}", val);
    }

    #[test]
    fn test_separation_loss() {
        let loss_fn = DirectionalContrastiveLoss::default_config();
        let (cause, effect) = make_test_vecs(4, 16);

        let loss = loss_fn.separation_loss(&cause, &effect).unwrap();
        let val: f32 = loss.flatten_all().unwrap().to_vec1().unwrap()[0];

        // Separation loss is mean cosine similarity, should be in [-1, 1]
        assert!(
            (-1.0..=1.0).contains(&val),
            "Separation loss should be in [-1,1], got {}",
            val
        );
    }

    #[test]
    fn test_soft_label_loss() {
        let loss_fn = DirectionalContrastiveLoss::default_config();
        let (cause, effect) = make_test_vecs(4, 16);
        let confidences = Tensor::from_slice(&[0.9f32, 0.8, 0.7, 0.1], 4, &Device::Cpu).unwrap();

        let loss = loss_fn
            .soft_label_loss(&cause, &effect, &confidences)
            .unwrap();
        let val: f32 = loss.flatten_all().unwrap().to_vec1().unwrap()[0];

        // MSE should be non-negative
        assert!(val >= 0.0, "Soft label loss should be >= 0, got {}", val);
    }

    #[test]
    fn test_combined_loss() {
        let loss_fn = DirectionalContrastiveLoss::default_config();
        let (cause, effect) = make_test_vecs(4, 16);
        let confidences = Tensor::from_slice(&[0.9f32, 0.8, 0.7, 0.1], 4, &Device::Cpu).unwrap();

        let (total, components) = loss_fn.compute(&cause, &effect, &confidences).unwrap();
        let total_val: f32 = total.flatten_all().unwrap().to_vec1().unwrap()[0];

        assert!(total_val > 0.0, "Total loss should be positive");
        assert!(
            components.contrastive > 0.0,
            "Contrastive component should be positive"
        );
        assert!(components.total > 0.0, "Logged total should match");
    }

    #[test]
    fn test_loss_config_default() {
        let config = LossConfig::default();
        assert_eq!(config.temperature, 0.05);
        assert_eq!(config.margin, 0.2);
        assert_eq!(config.lambda_contrastive, 1.0);
        assert_eq!(config.lambda_directional, 0.3);
        assert_eq!(config.lambda_separation, 0.1);
        assert_eq!(config.lambda_soft, 0.2);
    }

    #[test]
    fn test_info_nce_gradient_connected() {
        use candle_core::Var;

        // Create trainable Var tensors to verify gradient flows
        let cause_data: Vec<f32> = (0..4 * 8).map(|i| (i as f32 * 0.1).sin()).collect();
        let effect_data: Vec<f32> = (0..4 * 8).map(|i| (i as f32 * 0.2 + 1.0).cos()).collect();
        let cause_t = Tensor::from_slice(&cause_data, (4, 8), &Device::Cpu).unwrap();
        let effect_t = Tensor::from_slice(&effect_data, (4, 8), &Device::Cpu).unwrap();

        let cause_var = Var::from_tensor(&cause_t).unwrap();
        let effect_var = Var::from_tensor(&effect_t).unwrap();

        let loss_fn = DirectionalContrastiveLoss::default_config();
        let loss = loss_fn
            .info_nce_loss(cause_var.as_tensor(), effect_var.as_tensor())
            .unwrap();

        // backward() should succeed and produce non-zero gradients
        let grads = loss.backward().unwrap();
        let cause_grad = grads
            .get(cause_var.as_tensor())
            .expect("cause gradient must exist");
        let effect_grad = grads
            .get(effect_var.as_tensor())
            .expect("effect gradient must exist");

        // Gradients must be non-zero (not disconnected)
        let cause_grad_norm: f32 = cause_grad
            .sqr()
            .unwrap()
            .sum_all()
            .unwrap()
            .to_scalar()
            .unwrap();
        let effect_grad_norm: f32 = effect_grad
            .sqr()
            .unwrap()
            .sum_all()
            .unwrap()
            .to_scalar()
            .unwrap();

        assert!(
            cause_grad_norm > 1e-10,
            "Cause gradient must be non-zero, got {}",
            cause_grad_norm
        );
        assert!(
            effect_grad_norm > 1e-10,
            "Effect gradient must be non-zero, got {}",
            effect_grad_norm
        );
    }

    #[test]
    fn test_directional_loss_not_constant() {
        // Verify directional loss varies with input (not stuck at margin constant)
        let loss_fn = DirectionalContrastiveLoss::default_config();

        // Test 1: well-separated vectors (high diagonal, low off-diagonal → loss should be low)
        let d = 16;
        let cause1 = Tensor::from_slice(
            &(0..4 * d)
                .map(|i| if i % d == (i / d) { 1.0f32 } else { 0.0 })
                .collect::<Vec<_>>(),
            (4, d),
            &Device::Cpu,
        )
        .unwrap();
        let effect1 = Tensor::from_slice(
            &(0..4 * d)
                .map(|i| if i % d == (i / d) { 0.9f32 } else { 0.1 })
                .collect::<Vec<_>>(),
            (4, d),
            &Device::Cpu,
        )
        .unwrap();
        // L2 normalize
        let cn1 = cause1
            .sqr()
            .unwrap()
            .sum(1)
            .unwrap()
            .sqrt()
            .unwrap()
            .unsqueeze(1)
            .unwrap();
        let en1 = effect1
            .sqr()
            .unwrap()
            .sum(1)
            .unwrap()
            .sqrt()
            .unwrap()
            .unsqueeze(1)
            .unwrap();
        let cause1 = cause1.broadcast_div(&cn1).unwrap();
        let effect1 = effect1.broadcast_div(&en1).unwrap();

        let loss1 = loss_fn.directional_margin_loss(&cause1, &effect1).unwrap();
        let val1: f32 = loss1.flatten_all().unwrap().to_vec1().unwrap()[0];

        // Test 2: random vectors (lower gap → loss should be higher)
        let (cause2, effect2) = make_test_vecs(4, d);
        let loss2 = loss_fn.directional_margin_loss(&cause2, &effect2).unwrap();
        let val2: f32 = loss2.flatten_all().unwrap().to_vec1().unwrap()[0];

        // The two configurations should produce different loss values
        assert!(
            (val1 - val2).abs() > 1e-6 || val1 != 0.2,
            "Directional loss should vary with input, got val1={}, val2={} (old bug: constant 0.2)",
            val1,
            val2
        );
    }
}
