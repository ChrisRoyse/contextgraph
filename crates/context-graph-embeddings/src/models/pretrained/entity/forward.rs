//! GPU-accelerated BERT forward pass.
//!
//! Contains the core inference logic including tokenization,
//! embedding lookup, encoder execution, and pooling orchestration.

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertWeights;
use crate::types::ModelId;

use super::types::ENTITY_MAX_TOKENS;
use super::EntityModel;

impl EntityModel {
    /// Run GPU-accelerated BERT forward pass.
    ///
    /// # GPU Pipeline
    ///
    /// 1. Tokenize input text to token IDs
    /// 2. Create GPU tensors for input_ids, attention_mask, token_type_ids
    /// 3. Embedding lookup: word + position + token_type
    /// 4. Apply LayerNorm to embeddings
    /// 5. Run transformer encoder layers (6 layers for MiniLM)
    /// 6. Mean pooling over sequence length
    /// 7. L2 normalization
    pub(crate) fn gpu_forward(
        text: &str,
        weights: &BertWeights,
        tokenizer: &Tokenizer,
    ) -> EmbeddingResult<Vec<f32>> {
        let device = weights.device();
        let config = &weights.config;

        // Tokenize input text
        let encoding =
            tokenizer
                .encode(text, true)
                .map_err(|e| EmbeddingError::TokenizationError {
                    model_id: ModelId::Entity,
                    message: format!("EntityModel tokenization failed: {}", e),
                })?;

        let token_ids: Vec<u32> = encoding.get_ids().to_vec();
        let attention_mask: Vec<f32> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as f32)
            .collect();

        // Truncate to max_position_embeddings if needed
        let max_len = config.max_position_embeddings.min(ENTITY_MAX_TOKENS);
        let seq_len = token_ids.len().min(max_len);
        let token_ids = &token_ids[..seq_len];
        let attention_mask = &attention_mask[..seq_len];

        // Create GPU tensors
        let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("EntityModel input_ids tensor failed: {}", e),
            }
        })?;

        let attention_mask_tensor = Tensor::from_slice(attention_mask, (1, seq_len), device)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel attention_mask tensor failed: {}", e),
            })?;

        // Token type IDs (all zeros for single sentence)
        let token_type_ids: Vec<u32> = vec![0u32; seq_len];
        let token_type_tensor =
            Tensor::from_slice(&token_type_ids, (1, seq_len), device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("EntityModel token_type tensor failed: {}", e),
                }
            })?;

        // Position IDs
        let position_ids: Vec<u32> = (0..seq_len as u32).collect();
        let position_tensor =
            Tensor::from_slice(&position_ids, (1, seq_len), device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("EntityModel position_ids tensor failed: {}", e),
                }
            })?;

        // === EMBEDDING LAYER ===
        let embeddings = Self::compute_embeddings(
            &input_ids,
            &position_tensor,
            &token_type_tensor,
            weights,
            seq_len,
        )?;

        // Apply LayerNorm to embeddings
        let embeddings = Self::layer_norm(
            &embeddings,
            &weights.embeddings.layer_norm_weight,
            &weights.embeddings.layer_norm_bias,
            config.layer_norm_eps,
        )?;

        // === ENCODER LAYERS ===
        let extended_attention_mask = Self::create_attention_mask(&attention_mask_tensor)?;
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

        // === POOLING ===
        let pooled = Self::mean_pool(&hidden_states, &attention_mask_tensor, config, seq_len)?;
        let normalized = Self::l2_normalize(&pooled)?;
        Self::tensor_to_vec(&normalized)
    }

    /// Compute combined embeddings: word + position + token_type.
    fn compute_embeddings(
        input_ids: &Tensor,
        position_tensor: &Tensor,
        token_type_tensor: &Tensor,
        weights: &BertWeights,
        seq_len: usize,
    ) -> EmbeddingResult<Tensor> {
        let config = &weights.config;

        let word_embeds = weights
            .embeddings
            .word_embeddings
            .index_select(
                &input_ids
                    .flatten_all()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("EntityModel flatten input_ids failed: {}", e),
                    })?,
                0,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel word embedding lookup failed: {}", e),
            })?
            .reshape((1, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel word embedding reshape failed: {}", e),
            })?;

        let position_embeds = weights
            .embeddings
            .position_embeddings
            .index_select(
                &position_tensor
                    .flatten_all()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("EntityModel flatten position_ids failed: {}", e),
                    })?,
                0,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel position embedding lookup failed: {}", e),
            })?
            .reshape((1, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel position embedding reshape failed: {}", e),
            })?;

        let token_type_embeds = weights
            .embeddings
            .token_type_embeddings
            .index_select(
                &token_type_tensor
                    .flatten_all()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("EntityModel flatten token_type_ids failed: {}", e),
                    })?,
                0,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel token_type embedding lookup failed: {}", e),
            })?
            .reshape((1, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel token_type embedding reshape failed: {}", e),
            })?;

        // Sum embeddings
        let combined = ((word_embeds + position_embeds).map_err(|e| EmbeddingError::GpuError {
            message: format!("EntityModel embedding add 1 failed: {}", e),
        })? + token_type_embeds)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel embedding add 2 failed: {}", e),
            })?;

        Ok(combined)
    }

    /// Create extended attention mask for broadcasting: [batch, 1, 1, seq_len].
    fn create_attention_mask(attention_mask_tensor: &Tensor) -> EmbeddingResult<Tensor> {
        let extended = attention_mask_tensor
            .unsqueeze(1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel attention mask unsqueeze 1 failed: {}", e),
            })?
            .unsqueeze(2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel attention mask unsqueeze 2 failed: {}", e),
            })?;

        // Convert mask: 1.0 -> 0.0, 0.0 -> -10000.0
        let inverted = ((extended * (-1.0)).map_err(|e| EmbeddingError::GpuError {
            message: format!("EntityModel attention mask mul failed: {}", e),
        })? + 1.0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("EntityModel attention mask add failed: {}", e),
            })?
            * (-10000.0f64);

        inverted.map_err(|e| EmbeddingError::GpuError {
            message: format!("EntityModel attention mask scale failed: {}", e),
        })
    }
}
