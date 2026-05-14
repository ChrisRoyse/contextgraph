//! Embedding layer computation for the sparse SPLADE model.
//!
//! This module handles the initial token embedding computation including
//! word, position, and token type embeddings with layer normalization.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertWeights;

use super::encoder::layer_norm;

/// Compute token embeddings (word + position + token_type).
pub(crate) fn compute_embeddings(
    input_ids: &Tensor,
    position_tensor: &Tensor,
    token_type_tensor: &Tensor,
    weights: &BertWeights,
    config: &crate::gpu::BertConfig,
    batch_size: usize,
    seq_len: usize,
) -> EmbeddingResult<Tensor> {
    let word_embeds = weights
        .embeddings
        .word_embeddings
        .index_select(
            &input_ids
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("SparseModel flatten input_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel word embedding lookup failed: {}", e),
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel word embedding reshape failed: {}", e),
        })?;

    let position_embeds = weights
        .embeddings
        .position_embeddings
        .index_select(
            &position_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("SparseModel flatten position_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel position embedding lookup failed: {}", e),
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel position embedding reshape failed: {}", e),
        })?;

    let token_type_embeds = weights
        .embeddings
        .token_type_embeddings
        .index_select(
            &token_type_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("SparseModel flatten token_type_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel token_type embedding lookup failed: {}", e),
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel token_type embedding reshape failed: {}", e),
        })?;

    // Sum embeddings
    let embeddings = ((word_embeds + position_embeds).map_err(|e| EmbeddingError::GpuError {
        message: format!("SparseModel embedding add 1 failed: {}", e),
    })? + token_type_embeds)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel embedding add 2 failed: {}", e),
        })?;

    // Apply LayerNorm to embeddings
    layer_norm(
        &embeddings,
        &weights.embeddings.layer_norm_weight,
        &weights.embeddings.layer_norm_bias,
        config.layer_norm_eps,
    )
}

/// Run through all encoder layers.
pub(crate) fn run_encoder(
    embeddings: Tensor,
    attention_mask_tensor: &Tensor,
    weights: &BertWeights,
    config: &crate::gpu::BertConfig,
) -> EmbeddingResult<Tensor> {
    let mut hidden_states = embeddings;

    // Create attention mask for broadcasting
    let extended_attention_mask = attention_mask_tensor
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel attention mask unsqueeze 1 failed: {}", e),
        })?
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel attention mask unsqueeze 2 failed: {}", e),
        })?;

    // Convert mask: 1.0 -> 0.0, 0.0 -> -10000.0
    let extended_attention_mask =
        ((extended_attention_mask * (-1.0)).map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel attention mask mul failed: {}", e),
        })? + 1.0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("SparseModel attention mask add failed: {}", e),
            })?
            * (-10000.0f64);

    let extended_attention_mask =
        extended_attention_mask.map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel attention mask scale failed: {}", e),
        })?;

    for (layer_idx, layer) in weights.encoder_layers.iter().enumerate() {
        hidden_states = super::encoder::encoder_layer_forward(
            &hidden_states,
            layer,
            &extended_attention_mask,
            config,
            layer_idx,
        )?;
    }

    Ok(hidden_states)
}
