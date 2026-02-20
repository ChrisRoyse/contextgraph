//! Token embedding and pooling methods for LateInteractionModel.

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::gpu_forward::gpu_forward_tokens;
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

    /// Sequential processing of multiple inputs. Note: processes items one at a
    /// time (no GPU batching). For true batch inference, see Kepler model.
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.embed(input).await?);
        }
        Ok(results)
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
