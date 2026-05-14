//! Types and constants for the late-interaction (ColBERT) embedding model.

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertWeights;
use crate::traits::get_memory_estimate;
use crate::types::ModelId;

/// Native dimension for ColBERT per-token embeddings.
pub const LATE_INTERACTION_DIMENSION: usize = 128;

/// Maximum tokens for ColBERT (standard BERT-family limit).
pub const LATE_INTERACTION_MAX_TOKENS: usize = 512;

/// Latency budget in milliseconds (P95 target).
pub const LATE_INTERACTION_LATENCY_BUDGET_MS: u64 = 8;

/// HuggingFace model repository name.
pub const LATE_INTERACTION_MODEL_NAME: &str = "colbert-ir/colbertv2.0";

/// Per-token embeddings from ColBERT.
///
/// Each token in the input produces a 128D embedding vector.
/// The mask indicates which tokens are valid (non-padding).
#[derive(Debug, Clone)]
pub struct TokenEmbeddings {
    /// Token vectors [num_tokens, 128] - each inner Vec is 128D
    pub vectors: Vec<Vec<f32>>,
    /// Token strings for debugging/analysis
    pub tokens: Vec<String>,
    /// Mask for valid tokens (excludes padding)
    pub mask: Vec<bool>,
}

impl TokenEmbeddings {
    /// Create new token embeddings with validation.
    ///
    /// # Errors
    /// - `EmbeddingError::InvalidDimension` if any vector is not 128D
    /// - `EmbeddingError::EmptyInput` if vectors is empty
    pub fn new(
        vectors: Vec<Vec<f32>>,
        tokens: Vec<String>,
        mask: Vec<bool>,
    ) -> EmbeddingResult<Self> {
        if vectors.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        // Validate all vectors are 128D
        for (i, vec) in vectors.iter().enumerate() {
            if vec.len() != LATE_INTERACTION_DIMENSION {
                tracing::error!(
                    "Token {} has invalid dimension: expected {}, got {}",
                    i,
                    LATE_INTERACTION_DIMENSION,
                    vec.len()
                );
                return Err(EmbeddingError::InvalidDimension {
                    expected: LATE_INTERACTION_DIMENSION,
                    actual: vec.len(),
                });
            }
        }

        // Validate lengths match
        if vectors.len() != tokens.len() || vectors.len() != mask.len() {
            tracing::error!(
                "Length mismatch: vectors={}, tokens={}, mask={}",
                vectors.len(),
                tokens.len(),
                mask.len()
            );
            return Err(EmbeddingError::InvalidDimension {
                expected: vectors.len(),
                actual: tokens.len().min(mask.len()),
            });
        }

        Ok(Self {
            vectors,
            tokens,
            mask,
        })
    }

    /// Count of valid (non-padding) tokens.
    pub fn valid_token_count(&self) -> usize {
        self.mask.iter().filter(|&&v| v).count()
    }

    /// Validate a ColBERT true-batch row before accepting it into durable output.
    pub fn validate_true_batch_output(
        &self,
        model_id: ModelId,
        batch_index: usize,
    ) -> EmbeddingResult<()> {
        if self.vectors.len() != self.tokens.len() || self.vectors.len() != self.mask.len() {
            return Err(EmbeddingError::TrueBatchOutputCountMismatch {
                model_id,
                expected: self.vectors.len(),
                actual: self.tokens.len().min(self.mask.len()),
                recovery_hint: format!(
                    "ColBERT true-batch row {batch_index} has mismatched token/vector/mask lengths"
                ),
            });
        }

        if self.valid_token_count() == 0 {
            return Err(EmbeddingError::InferenceValidationFailed {
                model_id,
                reason: format!(
                    "ColBERT true-batch produced zero valid tokens at batch_index={batch_index}"
                ),
            });
        }

        for (token_idx, vector) in self.vectors.iter().enumerate() {
            if vector.len() != LATE_INTERACTION_DIMENSION {
                return Err(EmbeddingError::ModelDimensionMismatch {
                    model_id,
                    expected: LATE_INTERACTION_DIMENSION,
                    actual: vector.len(),
                });
            }
            if let Some((value_idx, value)) = vector
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| !value.is_finite())
            {
                return Err(EmbeddingError::InferenceValidationFailed {
                    model_id,
                    reason: format!(
                        "ColBERT true-batch produced non-finite value at batch_index={batch_index}, token_index={token_idx}, vector_index={value_idx}: {value}"
                    ),
                });
            }
        }

        Ok(())
    }
}

/// ColBERT projection layer weights for 768D -> 128D.
#[derive(Debug)]
pub struct ColBertProjection {
    /// Linear projection weight: [128, 768]
    pub weight: Tensor,
}

/// VRAM budget plan for a ColBERT true-batch forward pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateInteractionBatchVramPlan {
    /// Model-level conservative VRAM reservation.
    pub model_vram_bytes: usize,
    /// Input and mask tensor bytes needed for this batch.
    pub batch_tensor_bytes: usize,
    /// Total required bytes before CUDA tensor allocation.
    pub required_bytes: usize,
    /// Available GPU memory bytes used for the guard decision.
    pub available_bytes: usize,
}

/// Fail closed if a ColBERT true batch would exceed the available VRAM budget.
pub fn validate_late_interaction_batch_vram_budget(
    batch_tensor_bytes: usize,
    available_bytes: usize,
) -> EmbeddingResult<LateInteractionBatchVramPlan> {
    let model_id = ModelId::LateInteraction;
    let model_vram_bytes = get_memory_estimate(model_id);
    let required_bytes = model_vram_bytes
        .checked_add(batch_tensor_bytes)
        .ok_or_else(|| EmbeddingError::ConfigError {
            message: format!(
                "ColBERT true-batch VRAM plan overflow: model_vram_bytes={model_vram_bytes}, batch_tensor_bytes={batch_tensor_bytes}"
            ),
        })?;

    if available_bytes < required_bytes {
        return Err(EmbeddingError::InsufficientVram {
            required_bytes,
            available_bytes,
            required_gb: bytes_to_gb(required_bytes),
            available_gb: bytes_to_gb(available_bytes),
        });
    }

    Ok(LateInteractionBatchVramPlan {
        model_vram_bytes,
        batch_tensor_bytes,
        required_bytes,
        available_bytes,
    })
}

fn bytes_to_gb(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

/// Internal state for model weights.
#[allow(dead_code)]
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with candle model and tokenizer (GPU-accelerated).
    Loaded {
        /// BERT model weights on GPU (boxed to reduce enum size).
        weights: Box<BertWeights>,
        /// ColBERT projection layer (768D -> 128D).
        projection: ColBertProjection,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
    },
}
