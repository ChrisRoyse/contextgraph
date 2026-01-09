//! Self-attention computation for ColBERT transformer.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::AttentionWeights;

use super::gpu_attention_ops::{
    compute_attention, compute_qkv_projection, reshape_for_attention, ProjectionDims,
};

/// Run self-attention forward pass.
pub(crate) fn self_attention_forward(
    hidden_states: &Tensor,
    attention: &AttentionWeights,
    attention_mask: &Tensor,
    hidden_size: usize,
    num_attention_heads: usize,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, _hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "LateInteractionModel layer {} get dims failed: {}",
                    layer_idx, e
                ),
            })?;

    let head_dim = hidden_size / num_attention_heads;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} flatten hidden failed: {}",
                layer_idx, e
            ),
        })?;

    // Q, K, V projections
    let dims = ProjectionDims {
        batch_size,
        seq_len,
        hidden_size,
    };
    let query = compute_qkv_projection(
        &hidden_flat,
        &attention.query_weight,
        &attention.query_bias,
        &dims,
        layer_idx,
        "Q",
    )?;
    let key = compute_qkv_projection(
        &hidden_flat,
        &attention.key_weight,
        &attention.key_bias,
        &dims,
        layer_idx,
        "K",
    )?;
    let value = compute_qkv_projection(
        &hidden_flat,
        &attention.value_weight,
        &attention.value_bias,
        &dims,
        layer_idx,
        "V",
    )?;

    // Reshape to [batch, heads, seq_len, head_dim]
    let query = reshape_for_attention(
        &query,
        batch_size,
        seq_len,
        num_attention_heads,
        head_dim,
        layer_idx,
        "Q",
    )?;
    let key = reshape_for_attention(
        &key,
        batch_size,
        seq_len,
        num_attention_heads,
        head_dim,
        layer_idx,
        "K",
    )?;
    let value = reshape_for_attention(
        &value,
        batch_size,
        seq_len,
        num_attention_heads,
        head_dim,
        layer_idx,
        "V",
    )?;

    // Attention scores
    let context = compute_attention(&query, &key, &value, attention_mask, head_dim, layer_idx)?;

    // Reshape back to [batch, seq_len, hidden_size]
    let context = context
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} context transpose failed: {}",
                layer_idx, e
            ),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} context contiguous failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} context reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // Output projection
    let context_flat = context
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} context flatten failed: {}",
                layer_idx, e
            ),
        })?;

    let output = context_flat
        .matmul(
            &attention
                .output_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "LateInteractionModel layer {} output transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} output matmul failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} output reshape failed: {}",
                layer_idx, e
            ),
        })?;

    output
        .broadcast_add(&attention.output_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel layer {} output bias failed: {}",
                layer_idx, e
            ),
        })
}
