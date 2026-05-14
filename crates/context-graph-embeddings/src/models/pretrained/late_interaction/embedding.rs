//! Token embedding and pooling methods for LateInteractionModel.

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::gpu_forward::{gpu_forward_tokens, gpu_forward_tokens_batch};
use super::model::LateInteractionModel;
use super::types::{ModelState, TokenEmbeddings, LATE_INTERACTION_DIMENSION};

impl LateInteractionModel {
    /// Get full per-token embeddings for MaxSim scoring.
    ///
    /// # Arguments
    /// * `text` - Input text to tokenize and embed
    ///
    /// # Returns
    /// `TokenEmbeddings` with per-token 128D vectors
    ///
    /// # Errors
    /// - `EmbeddingError::NotInitialized` if model not loaded
    /// - `EmbeddingError::EmptyInput` if text is empty
    /// - `EmbeddingError::InputTooLong` if tokens exceed limit
    pub async fn embed_tokens(&self, text: &str) -> EmbeddingResult<TokenEmbeddings> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        // Get loaded state
        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("LateInteractionModel failed to acquire read lock: {}", e),
            })?;

        let (weights, projection, tokenizer) = match &*state {
            ModelState::Loaded {
                weights,
                projection,
                tokenizer,
            } => (weights, projection, tokenizer),
            _ => {
                return Err(EmbeddingError::NotInitialized {
                    model_id: ModelId::LateInteraction,
                });
            }
        };

        // Run GPU-accelerated ColBERT forward pass
        gpu_forward_tokens(trimmed, weights, projection, tokenizer)
    }

    /// Get full per-token embeddings for a true padded ColBERT CUDA batch.
    pub async fn embed_tokens_batch(
        &self,
        texts: &[String],
    ) -> EmbeddingResult<Vec<TokenEmbeddings>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        if texts.is_empty() {
            return Err(EmbeddingError::TrueBatchEmpty {
                model_id: ModelId::LateInteraction,
                recovery_hint: "submit at least one ColBERT input; empty batches are invalid"
                    .to_string(),
            });
        }

        let mut prepared = Vec::with_capacity(texts.len());
        for (idx, text) in texts.iter().enumerate() {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Err(EmbeddingError::EmptyInput);
            }
            prepared.push(trimmed.to_string());
            tracing::trace!(batch_index = idx, "ColBERT true-batch input accepted");
        }

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("LateInteractionModel failed to acquire read lock: {}", e),
            })?;

        let (weights, projection, tokenizer) = match &*state {
            ModelState::Loaded {
                weights,
                projection,
                tokenizer,
            } => (weights, projection, tokenizer),
            _ => {
                return Err(EmbeddingError::NotInitialized {
                    model_id: ModelId::LateInteraction,
                });
            }
        };

        let rows = gpu_forward_tokens_batch(&prepared, weights, projection, tokenizer)?;
        if rows.len() != prepared.len() {
            return Err(EmbeddingError::TrueBatchOutputCountMismatch {
                model_id: ModelId::LateInteraction,
                expected: prepared.len(),
                actual: rows.len(),
                recovery_hint: "ColBERT true-batch token output count must match input batch size"
                    .to_string(),
            });
        }
        for (idx, row) in rows.iter().enumerate() {
            row.validate_true_batch_output(ModelId::LateInteraction, idx)?;
        }
        Ok(rows)
    }

    /// Pool token embeddings to single 128D vector for fusion.
    ///
    /// Uses mean pooling over valid (non-padding) tokens,
    /// then L2 normalizes the result.
    ///
    /// # Arguments
    /// * `token_embs` - Per-token embeddings from `embed_tokens`
    ///
    /// # Returns
    /// Single 128D L2-normalized vector suitable for multi-array storage
    pub fn pool_tokens(&self, token_embs: &TokenEmbeddings) -> Vec<f32> {
        // Mean pooling over valid tokens
        let valid_vectors: Vec<&Vec<f32>> = token_embs
            .vectors
            .iter()
            .zip(token_embs.mask.iter())
            .filter(|(_, &valid)| valid)
            .map(|(v, _)| v)
            .collect();

        if valid_vectors.is_empty() {
            return vec![0.0f32; LATE_INTERACTION_DIMENSION];
        }

        let n = valid_vectors.len() as f32;
        let mut pooled = vec![0.0f32; LATE_INTERACTION_DIMENSION];

        for v in valid_vectors {
            for (i, val) in v.iter().enumerate() {
                pooled[i] += val / n;
            }
        }

        // L2 normalize
        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            pooled.iter_mut().for_each(|x| *x /= norm);
        }

        pooled
    }

    /// True CUDA batch processing using one padded ColBERT tensor forward pass.
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        if inputs.is_empty() {
            return Err(EmbeddingError::TrueBatchEmpty {
                model_id: ModelId::LateInteraction,
                recovery_hint: "submit at least one ColBERT input; empty batches are invalid"
                    .to_string(),
            });
        }
        if inputs.len() > self.config.max_batch_size {
            return Err(EmbeddingError::ConfigError {
                message: format!(
                    "ColBERT true-batch size {} exceeds max_batch_size guard {}; reduce batch size before CUDA forward",
                    inputs.len(),
                    self.config.max_batch_size
                ),
            });
        }

        let mut prepared = Vec::with_capacity(inputs.len());
        for input in inputs {
            self.validate_input(input)?;
            prepared.push(Self::extract_content(input)?);
        }

        let start = std::time::Instant::now();
        let token_rows = self.embed_tokens_batch(&prepared).await?;
        if token_rows.len() != inputs.len() {
            return Err(EmbeddingError::TrueBatchOutputCountMismatch {
                model_id: ModelId::LateInteraction,
                expected: inputs.len(),
                actual: token_rows.len(),
                recovery_hint: "ColBERT pooled embedding count must match input batch size"
                    .to_string(),
            });
        }

        let latency_us = ((start.elapsed().as_micros() as u64) / inputs.len() as u64).max(1);
        token_rows
            .iter()
            .map(|token_embs| {
                let vector = self.pool_tokens(token_embs);
                let embedding = ModelEmbedding::new(ModelId::LateInteraction, vector, latency_us);
                embedding.validate()?;
                Ok(embedding)
            })
            .collect()
    }

    /// Extract text content from model input for embedding.
    pub(crate) fn extract_content(input: &ModelInput) -> EmbeddingResult<String> {
        match input {
            ModelInput::Text {
                content,
                instruction,
            } => {
                let mut full = content.clone();
                if let Some(inst) = instruction {
                    full = format!("{} {}", inst, full);
                }
                Ok(full)
            }
            _ => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::LateInteraction,
                input_type: InputType::from(input),
            }),
        }
    }
}
