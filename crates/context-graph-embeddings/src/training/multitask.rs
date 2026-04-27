//! Multi-task training heads for causal embedder.
//!
//! Joint objectives sharing the NomicBERT encoder:
//! - **Task A**: Contrastive causal pair matching (primary — handled by loss.rs)
//! - **Task B**: Direction classification (cause→effect vs effect→cause vs none)
//! - **Task C**: Mechanism type prediction (biological, economic, etc.)
//!
//! These auxiliary heads provide additional gradient signal that helps the
//! encoder learn richer causal representations.

use candle_core::{DType, Device, Tensor, Var};

use crate::error::{EmbeddingError, EmbeddingResult};

/// Direction classification labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionLabel {
    /// A causes B (forward).
    Forward = 0,
    /// B causes A (backward).
    Backward = 1,
    /// No causal relationship.
    None = 2,
}

impl DirectionLabel {
    /// Convert to one-hot index.
    pub fn index(self) -> u32 {
        self as u32
    }

    /// Number of classes.
    pub const NUM_CLASSES: usize = 3;
}

/// Mechanism type labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MechanismLabel {
    Biological = 0,
    Economic = 1,
    Physical = 2,
    Technical = 3,
    Social = 4,
    Ecological = 5,
    Other = 6,
}

impl MechanismLabel {
    /// Convert to one-hot index.
    pub fn index(self) -> u32 {
        self as u32
    }

    /// Parse from string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "biological" | "bio" | "medical" | "health" => Self::Biological,
            "economic" | "financial" | "monetary" => Self::Economic,
            "physical" | "chemical" | "physics" => Self::Physical,
            "technical" | "software" | "engineering" => Self::Technical,
            "social" | "political" | "cultural" => Self::Social,
            "ecological" | "environmental" => Self::Ecological,
            _ => Self::Other,
        }
    }

    /// Number of classes.
    pub const NUM_CLASSES: usize = 7;
}

/// Configuration for multi-task heads.
#[derive(Debug, Clone)]
pub struct MultiTaskConfig {
    /// Input dimension from encoder (default: 768).
    pub input_dim: usize,
    /// Hidden dimension for classification MLPs (default: 256).
    pub hidden_dim: usize,
    /// Weight for direction classification loss (default: 0.2).
    pub lambda_direction: f32,
    /// Weight for mechanism classification loss (default: 0.1).
    pub lambda_mechanism: f32,
}

impl Default for MultiTaskConfig {
    fn default() -> Self {
        Self {
            input_dim: 768,
            hidden_dim: 256,
            lambda_direction: 0.2,
            lambda_mechanism: 0.1,
        }
    }
}

/// A simple 2-layer MLP classification head.
pub struct ClassificationHead {
    /// First layer weights [input_dim, hidden_dim].
    w1: Var,
    /// First layer bias [hidden_dim].
    b1: Var,
    /// Second layer weights [hidden_dim, num_classes].
    w2: Var,
    /// Second layer bias [num_classes].
    b2: Var,
}

impl ClassificationHead {
    /// Create a new classification head with Xavier initialization.
    pub fn new(
        input_dim: usize,
        hidden_dim: usize,
        num_classes: usize,
        device: &Device,
    ) -> EmbeddingResult<Self> {
        let std1 = (2.0 / (input_dim + hidden_dim) as f64).sqrt() as f32;
        let std2 = (2.0 / (hidden_dim + num_classes) as f64).sqrt() as f32;

        let w1_data: Vec<f32> = (0..input_dim * hidden_dim)
            .map(|i| ((i as f32 * 0.618_034 + 0.5) % 1.0 * 2.0 - 1.0) * std1)
            .collect();
        let w1 = Var::from_tensor(
            &Tensor::from_slice(&w1_data, (input_dim, hidden_dim), device).map_err(map_candle)?,
        )
        .map_err(map_candle)?;

        let b1 = Var::from_tensor(
            &Tensor::zeros((hidden_dim,), DType::F32, device).map_err(map_candle)?,
        )
        .map_err(map_candle)?;

        let w2_data: Vec<f32> = (0..hidden_dim * num_classes)
            .map(|i| ((i as f32 * 0.414_213_57 + 0.7) % 1.0 * 2.0 - 1.0) * std2)
            .collect();
        let w2 = Var::from_tensor(
            &Tensor::from_slice(&w2_data, (hidden_dim, num_classes), device).map_err(map_candle)?,
        )
        .map_err(map_candle)?;

        let b2 = Var::from_tensor(
            &Tensor::zeros((num_classes,), DType::F32, device).map_err(map_candle)?,
        )
        .map_err(map_candle)?;

        Ok(Self { w1, b1, w2, b2 })
    }

    /// Forward pass: x → linear → ReLU → linear → logits.
    pub fn forward(&self, x: &Tensor) -> EmbeddingResult<Tensor> {
        // First layer + ReLU
        let h = x
            .matmul(self.w1.as_tensor())
            .map_err(map_candle)?
            .broadcast_add(self.b1.as_tensor())
            .map_err(map_candle)?
            .relu()
            .map_err(map_candle)?;

        // Second layer (logits)
        h.matmul(self.w2.as_tensor())
            .map_err(map_candle)?
            .broadcast_add(self.b2.as_tensor())
            .map_err(map_candle)
    }

    /// Get trainable variables.
    pub fn trainable_vars(&self) -> Vec<&Var> {
        vec![&self.w1, &self.b1, &self.w2, &self.b2]
    }

    /// Total parameter count.
    pub fn num_params(&self) -> usize {
        self.w1.as_tensor().shape().elem_count()
            + self.b1.as_tensor().shape().elem_count()
            + self.w2.as_tensor().shape().elem_count()
            + self.b2.as_tensor().shape().elem_count()
    }
}

/// Multi-task heads for auxiliary training objectives.
pub struct MultiTaskHeads {
    /// Direction classification head.
    pub direction_head: ClassificationHead,
    /// Mechanism type prediction head.
    pub mechanism_head: ClassificationHead,
    /// Configuration.
    pub config: MultiTaskConfig,
}

impl MultiTaskHeads {
    /// Create multi-task heads.
    pub fn new(config: MultiTaskConfig, device: &Device) -> EmbeddingResult<Self> {
        let direction_head = ClassificationHead::new(
            config.input_dim * 2, // Concatenated cause + effect
            config.hidden_dim,
            DirectionLabel::NUM_CLASSES,
            device,
        )?;

        let mechanism_head = ClassificationHead::new(
            config.input_dim * 2, // Concatenated cause + effect
            config.hidden_dim,
            MechanismLabel::NUM_CLASSES,
            device,
        )?;

        Ok(Self {
            direction_head,
            mechanism_head,
            config,
        })
    }

    /// Compute direction classification logits.
    ///
    /// Input: concatenated cause + effect embeddings [N, 2*D].
    pub fn direction_logits(&self, cause_effect_cat: &Tensor) -> EmbeddingResult<Tensor> {
        self.direction_head.forward(cause_effect_cat)
    }

    /// Compute mechanism prediction logits.
    ///
    /// Input: concatenated cause + effect embeddings [N, 2*D].
    pub fn mechanism_logits(&self, cause_effect_cat: &Tensor) -> EmbeddingResult<Tensor> {
        self.mechanism_head.forward(cause_effect_cat)
    }

    /// Compute cross-entropy loss for direction classification.
    pub fn direction_loss(
        &self,
        cause_effect_cat: &Tensor,
        labels: &Tensor,
    ) -> EmbeddingResult<Tensor> {
        let logits = self.direction_logits(cause_effect_cat)?;
        classification_cross_entropy(&logits, labels)
    }

    /// Compute cross-entropy loss for mechanism prediction.
    pub fn mechanism_loss(
        &self,
        cause_effect_cat: &Tensor,
        labels: &Tensor,
    ) -> EmbeddingResult<Tensor> {
        let logits = self.mechanism_logits(cause_effect_cat)?;
        classification_cross_entropy(&logits, labels)
    }

    /// Get all trainable variables.
    pub fn all_trainable_vars(&self) -> Vec<&Var> {
        let mut vars = self.direction_head.trainable_vars();
        vars.extend(self.mechanism_head.trainable_vars());
        vars
    }

    /// Total parameter count.
    pub fn total_params(&self) -> usize {
        self.direction_head.num_params() + self.mechanism_head.num_params()
    }
}

/// Cross-entropy loss for classification.
fn classification_cross_entropy(logits: &Tensor, labels: &Tensor) -> EmbeddingResult<Tensor> {
    let n = logits.dim(0).map_err(map_candle)?;
    let device = logits.device();

    // Log-softmax
    let max_logits = logits.max_keepdim(1).map_err(map_candle)?;
    let shifted = logits.broadcast_sub(&max_logits).map_err(map_candle)?;
    let exp = shifted.exp().map_err(map_candle)?;
    let sum_exp = exp.sum_keepdim(1).map_err(map_candle)?;
    let log_softmax = shifted
        .broadcast_sub(&sum_exp.log().map_err(map_candle)?)
        .map_err(map_candle)?;

    // Gather at label indices
    let label_vec: Vec<u32> = labels
        .to_dtype(DType::U32)
        .map_err(map_candle)?
        .to_vec1()
        .map_err(map_candle)?;

    let mut nll_sum = 0.0f64;
    for (i, &label) in label_vec.iter().enumerate().take(n) {
        let row = log_softmax.get(i).map_err(map_candle)?;
        let idx = label as usize;
        let log_prob: f32 = row
            .get(idx)
            .map_err(map_candle)?
            .to_scalar()
            .map_err(map_candle)?;
        nll_sum -= log_prob as f64;
    }

    Tensor::new(&[nll_sum as f32 / n as f32], device).map_err(map_candle)
}

/// Map candle errors to EmbeddingError.
fn map_candle(e: candle_core::Error) -> EmbeddingError {
    EmbeddingError::GpuError {
        message: format!("Multi-task head error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_label() {
        assert_eq!(DirectionLabel::Forward.index(), 0);
        assert_eq!(DirectionLabel::Backward.index(), 1);
        assert_eq!(DirectionLabel::None.index(), 2);
    }

    #[test]
    fn test_mechanism_label_parsing() {
        assert_eq!(
            MechanismLabel::from_str("biological"),
            MechanismLabel::Biological
        );
        assert_eq!(
            MechanismLabel::from_str("economic"),
            MechanismLabel::Economic
        );
        assert_eq!(MechanismLabel::from_str("unknown"), MechanismLabel::Other);
    }

    #[test]
    fn test_classification_head_shape() {
        let device = Device::Cpu;
        let head = ClassificationHead::new(16, 8, 3, &device).unwrap();

        let x = Tensor::ones((4, 16), DType::F32, &device).unwrap();
        let logits = head.forward(&x).unwrap();

        assert_eq!(logits.dims(), &[4, 3]); // [batch, num_classes]
    }

    #[test]
    fn test_multitask_heads_creation() {
        let config = MultiTaskConfig {
            input_dim: 8,
            hidden_dim: 4,
            ..Default::default()
        };
        let heads = MultiTaskHeads::new(config, &Device::Cpu).unwrap();

        // Check parameter count > 0
        assert!(heads.total_params() > 0);
        assert!(!heads.all_trainable_vars().is_empty());
    }

    #[test]
    fn test_direction_classification() {
        let device = Device::Cpu;
        let config = MultiTaskConfig {
            input_dim: 8,
            hidden_dim: 4,
            ..Default::default()
        };
        let heads = MultiTaskHeads::new(config, &device).unwrap();

        // Concatenated cause + effect = [N, 2*D]
        let cat = Tensor::ones((3, 16), DType::F32, &device).unwrap();
        let labels = Tensor::from_slice(&[0u32, 1, 2], 3, &device).unwrap();

        let loss = heads.direction_loss(&cat, &labels).unwrap();
        let val: f32 = loss.flatten_all().unwrap().to_vec1().unwrap()[0];
        assert!(val > 0.0, "Classification loss should be positive");
    }
}
