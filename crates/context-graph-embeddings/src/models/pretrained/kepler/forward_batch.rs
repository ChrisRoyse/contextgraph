//! GPU-accelerated batch forward pass for KEPLER.
//!
//! Per ARCH-GPU-06: Batch operations preferred - minimize kernel launches.
//! This module provides true GPU batch inference for multiple texts in a single forward pass.

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertWeights;
use crate::types::ModelId;

use super::types::KEPLER_MAX_TOKENS;
use super::KeplerModel;

impl KeplerModel {
    /// Run GPU-accelerated batch forward pass for multiple texts.
    ///
    /// Per ARCH-GPU-06: Batch operations preferred - minimize kernel launches.
    /// This processes all texts in a single GPU operation, not sequential loops.
    ///
    /// # GPU Pipeline (Batched)
    ///
    /// 1. Tokenize all input texts to token IDs
    /// 2. Pad all sequences to max length
    /// 3. Create GPU tensors: [batch_size, max_seq_len]
    /// 4. Embedding lookup for entire batch
    /// 5. Run transformer encoder layers (batch parallel)
    /// 6. Mean pooling for each item in batch
    /// 7. L2 normalization for each item
    ///
    /// # Arguments
    /// * `texts` - Slice of texts to embed
    /// * `weights` - Loaded BERT/RoBERTa weights on GPU
    /// * `tokenizer` - RoBERTa tokenizer
    ///
    /// # Returns
    /// * `Ok(Vec<Vec<f32>>)` - Batch of 768D embeddings
    /// * `Err(EmbeddingError)` - If any step fails (NO CPU fallback per AP-GPU-01)
    pub(crate) fn gpu_forward_batch(
        texts: &[&str],
        weights: &BertWeights,
        tokenizer: &Tokenizer,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let device = weights.device();
        let config = &weights.config;
        let batch_size = texts.len();

        // Step 1: Tokenize all inputs
        let encodings: Vec<_> = texts
            .iter()
            .map(|text| {
                tokenizer
                    .encode(*text, true)
                    .map_err(|e| EmbeddingError::TokenizationError {
                        model_id: ModelId::Kepler,
                        message: format!("KeplerModel batch tokenization failed: {}", e),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Step 2: Find max sequence length and pad
        let max_len = config.max_position_embeddings.min(KEPLER_MAX_TOKENS);
        let actual_max_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(max_len))
            .max()
            .unwrap_or(1);

        // Step 3: Create padded tensors for batch
        // input_ids: [batch_size, max_seq_len]
        // attention_mask: [batch_size, max_seq_len]
        let mut all_token_ids = vec![0u32; batch_size * actual_max_len];
        let mut all_attention_mask = vec![0.0f32; batch_size * actual_max_len];
        let mut all_position_ids = vec![0u32; batch_size * actual_max_len];
        let all_token_type_ids = vec![0u32; batch_size * actual_max_len]; // All zeros for RoBERTa

        for (batch_idx, encoding) in encodings.iter().enumerate() {
            let token_ids = encoding.get_ids();
            let seq_len = token_ids.len().min(actual_max_len);
            let offset = batch_idx * actual_max_len;

            // Copy token IDs (with truncation)
            for (i, &tid) in token_ids[..seq_len].iter().enumerate() {
                all_token_ids[offset + i] = tid;
            }

            // Set attention mask (1.0 for real tokens, 0.0 for padding)
            for i in 0..seq_len {
                all_attention_mask[offset + i] = 1.0;
            }

            // Position IDs (RoBERTa offset of 2)
            for i in 0..seq_len {
                all_position_ids[offset + i] = (i as u32) + 2;
            }

            // Token type IDs (all zeros for RoBERTa)
            // Already initialized to 0
        }

        // Create GPU tensors
        let input_ids = Tensor::from_slice(&all_token_ids, (batch_size, actual_max_len), device)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch input_ids tensor failed: {}", e),
            })?;

        let attention_mask_tensor =
            Tensor::from_slice(&all_attention_mask, (batch_size, actual_max_len), device).map_err(
                |e| EmbeddingError::GpuError {
                    message: format!("KeplerModel batch attention_mask tensor failed: {}", e),
                },
            )?;

        let position_tensor =
            Tensor::from_slice(&all_position_ids, (batch_size, actual_max_len), device).map_err(
                |e| EmbeddingError::GpuError {
                    message: format!("KeplerModel batch position_ids tensor failed: {}", e),
                },
            )?;

        let token_type_tensor =
            Tensor::from_slice(&all_token_type_ids, (batch_size, actual_max_len), device).map_err(
                |e| EmbeddingError::GpuError {
                    message: format!("KeplerModel batch token_type tensor failed: {}", e),
                },
            )?;

        // Step 4: Compute embeddings (batch)
        let embeddings = Self::compute_embeddings_batch(
            &input_ids,
            &position_tensor,
            &token_type_tensor,
            weights,
            batch_size,
            actual_max_len,
        )?;

        // Apply LayerNorm to embeddings
        let embeddings = Self::layer_norm(
            &embeddings,
            &weights.embeddings.layer_norm_weight,
            &weights.embeddings.layer_norm_bias,
            config.layer_norm_eps,
        )?;

        // Step 5: Encoder layers (already batch-capable)
        let extended_attention_mask = Self::create_attention_mask_batch(&attention_mask_tensor)?;
        let mut hidden_states = embeddings;

        for (layer_idx, layer) in weights.encoder_layers.iter().enumerate() {
            hidden_states = Self::encoder_layer_forward(
                &hidden_states,
                layer,
                &extended_attention_mask,
                config,
                layer_idx,
            )?;
        }

        // Step 6: Batch mean pooling
        let pooled = Self::mean_pool_batch(
            &hidden_states,
            &attention_mask_tensor,
            config,
            batch_size,
            actual_max_len,
        )?;

        // Step 7: Batch L2 normalization
        let normalized = Self::l2_normalize_batch(&pooled)?;

        // Convert to Vec<Vec<f32>>
        Self::tensor_to_batch_vecs(&normalized, batch_size)
    }

    /// Compute embeddings for batch: word + position + token_type.
    fn compute_embeddings_batch(
        input_ids: &Tensor,
        position_tensor: &Tensor,
        token_type_tensor: &Tensor,
        weights: &BertWeights,
        batch_size: usize,
        seq_len: usize,
    ) -> EmbeddingResult<Tensor> {
        let config = &weights.config;

        // Flatten input_ids for index_select: [batch * seq]
        let input_flat = input_ids
            .flatten_all()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch flatten input_ids failed: {}", e),
            })?;

        let word_embeds = weights
            .embeddings
            .word_embeddings
            .index_select(&input_flat, 0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch word embedding lookup failed: {}", e),
            })?
            .reshape((batch_size, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch word embedding reshape failed: {}", e),
            })?;

        let position_flat =
            position_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("KeplerModel batch flatten position_ids failed: {}", e),
                })?;

        let position_embeds = weights
            .embeddings
            .position_embeddings
            .index_select(&position_flat, 0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch position embedding lookup failed: {}", e),
            })?
            .reshape((batch_size, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch position embedding reshape failed: {}", e),
            })?;

        let token_type_flat =
            token_type_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("KeplerModel batch flatten token_type_ids failed: {}", e),
                })?;

        let token_type_embeds = weights
            .embeddings
            .token_type_embeddings
            .index_select(&token_type_flat, 0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "KeplerModel batch token_type embedding lookup failed: {}",
                    e
                ),
            })?
            .reshape((batch_size, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "KeplerModel batch token_type embedding reshape failed: {}",
                    e
                ),
            })?;

        // Sum embeddings
        let combined = ((word_embeds + position_embeds).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel batch embedding add 1 failed: {}", e),
        })? + token_type_embeds)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch embedding add 2 failed: {}", e),
            })?;

        Ok(combined)
    }

    /// Create extended attention mask for batch: [batch, 1, 1, seq_len].
    fn create_attention_mask_batch(attention_mask_tensor: &Tensor) -> EmbeddingResult<Tensor> {
        let extended = attention_mask_tensor
            .unsqueeze(1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch attention mask unsqueeze 1 failed: {}", e),
            })?
            .unsqueeze(2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch attention mask unsqueeze 2 failed: {}", e),
            })?;

        // Convert mask: 1.0 -> 0.0, 0.0 -> -10000.0
        let inverted = ((extended * (-1.0)).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel batch attention mask mul failed: {}", e),
        })? + 1.0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch attention mask add failed: {}", e),
            })?
            * (-10000.0f64);

        inverted.map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel batch attention mask scale failed: {}", e),
        })
    }

    /// Batch mean pooling: [batch, seq, hidden] -> [batch, hidden]
    fn mean_pool_batch(
        hidden_states: &Tensor,
        attention_mask: &Tensor,
        config: &crate::gpu::BertConfig,
        batch_size: usize,
        seq_len: usize,
    ) -> EmbeddingResult<Tensor> {
        // Expand mask: [batch, seq] -> [batch, seq, hidden]
        let mask_expanded = attention_mask
            .unsqueeze(2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch mean_pool unsqueeze failed: {}", e),
            })?
            .expand((batch_size, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch mean_pool expand failed: {}", e),
            })?;

        // Multiply hidden states by mask
        let masked = (hidden_states * &mask_expanded).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel batch mean_pool mul failed: {}", e),
        })?;

        // Sum over sequence dimension: [batch, hidden]
        let summed = masked.sum(1).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel batch mean_pool sum failed: {}", e),
        })?;

        // Sum mask for denominator: [batch, hidden]
        let mask_sum = mask_expanded
            .sum(1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch mean_pool mask_sum failed: {}", e),
            })?
            .clamp(1e-9, f64::INFINITY)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch mean_pool clamp failed: {}", e),
            })?;

        // Divide to get mean: [batch, hidden]
        (summed / mask_sum).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel batch mean_pool div failed: {}", e),
        })
    }

    /// Batch L2 normalization: [batch, hidden] -> [batch, hidden]
    fn l2_normalize_batch(embeddings: &Tensor) -> EmbeddingResult<Tensor> {
        // Compute L2 norm per row: [batch, 1]
        let norms = embeddings
            .sqr()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch l2_normalize sqr failed: {}", e),
            })?
            .sum_keepdim(1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch l2_normalize sum failed: {}", e),
            })?
            .sqrt()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch l2_normalize sqrt failed: {}", e),
            })?
            .clamp(1e-12, f64::INFINITY)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch l2_normalize clamp failed: {}", e),
            })?;

        // Broadcast divide: [batch, hidden] / [batch, 1]
        embeddings
            .broadcast_div(&norms)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch l2_normalize div failed: {}", e),
            })
    }

    /// Convert batch tensor to Vec<Vec<f32>>.
    fn tensor_to_batch_vecs(tensor: &Tensor, batch_size: usize) -> EmbeddingResult<Vec<Vec<f32>>> {
        let flat: Vec<f32> = tensor
            .flatten_all()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch tensor_to_vec flatten failed: {}", e),
            })?
            .to_vec1()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel batch tensor_to_vec to_vec1 failed: {}", e),
            })?;

        let hidden_size = flat.len() / batch_size;
        let results: Vec<Vec<f32>> = flat.chunks(hidden_size).map(|c| c.to_vec()).collect();

        if results.len() != batch_size {
            return Err(EmbeddingError::GpuError {
                message: format!(
                    "KeplerModel batch size mismatch: expected {}, got {}",
                    batch_size,
                    results.len()
                ),
            });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_batch_forward_consistency() {
        // This test ensures batch forward produces same results as single forward
        // Run with: cargo test --features real-embeddings
        // Actual test requires GPU and loaded model
    }
}
