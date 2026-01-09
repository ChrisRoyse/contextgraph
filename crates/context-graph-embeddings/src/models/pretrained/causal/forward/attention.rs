//! Self-attention forward pass for Longformer model.
//!
//! This module implements multi-head self-attention with Q, K, V projections.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::super::config::LongformerConfig;
use super::super::weights::LongformerAttentionWeights;

/// Dimensions for Q/K/V projection operations.
struct ProjectionDims {
    batch_size: usize,
    seq_len: usize,
    hidden_size: usize,
}

/// Run self-attention forward pass.
pub fn self_attention_forward(
    hidden_states: &Tensor,
    attention: &LongformerAttentionWeights,
    attention_mask: &Tensor,
    config: &LongformerConfig,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("CausalModel layer {} get dims failed: {}", layer_idx, e),
            })?;

    let head_dim = config.hidden_size / config.num_attention_heads;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} flatten hidden failed: {}",
                layer_idx, e
            ),
        })?;

    // Q, K, V projections
    let dims = ProjectionDims {
        batch_size,
        seq_len,
        hidden_size,
    };
    let query = project_qkv(
        &hidden_flat,
        &attention.query_weight,
        &attention.query_bias,
        &dims,
        layer_idx,
        "Q",
    )?;
    let key = project_qkv(
        &hidden_flat,
        &attention.key_weight,
        &attention.key_bias,
        &dims,
        layer_idx,
        "K",
    )?;
    let value = project_qkv(
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
        config.num_attention_heads,
        head_dim,
        layer_idx,
        "Q",
    )?;
    let key = reshape_for_attention(
        &key,
        batch_size,
        seq_len,
        config.num_attention_heads,
        head_dim,
        layer_idx,
        "K",
    )?;
    let value = reshape_for_attention(
        &value,
        batch_size,
        seq_len,
        config.num_attention_heads,
        head_dim,
        layer_idx,
        "V",
    )?;

    // K^T with contiguous() for matmul
    let key_t = key
        .transpose(2, 3)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} K transpose 2,3 failed: {}",
                layer_idx, e
            ),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} K^T contiguous failed: {}",
                layer_idx, e
            ),
        })?;

    // Attention scores
    let scores = query.matmul(&key_t).map_err(|e| EmbeddingError::GpuError {
        message: format!("CausalModel layer {} QK matmul failed: {}", layer_idx, e),
    })?;

    let scale = (head_dim as f64).sqrt();
    let scores = (scores / scale).map_err(|e| EmbeddingError::GpuError {
        message: format!(
            "CausalModel layer {} attention scale failed: {}",
            layer_idx, e
        ),
    })?;

    // Apply attention mask
    let scores = scores
        .broadcast_add(attention_mask)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} attention mask add failed: {}",
                layer_idx, e
            ),
        })?;

    let attention_probs =
        candle_nn::ops::softmax(&scores, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("CausalModel layer {} softmax failed: {}", layer_idx, e),
            }
        })?;

    let context = attention_probs
        .matmul(&value)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context matmul failed: {}",
                layer_idx, e
            ),
        })?;

    // Reshape back
    let context = context
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context transpose failed: {}",
                layer_idx, e
            ),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context contiguous failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // Output projection
    let context_flat = context
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} O flatten failed: {}", layer_idx, e),
        })?;

    let output = context_flat
        .matmul(
            &attention
                .output_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "CausalModel layer {} output transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} output matmul failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} output reshape failed: {}",
                layer_idx, e
            ),
        })?;

    output
        .broadcast_add(&attention.output_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} output bias failed: {}", layer_idx, e),
        })
}

/// Project hidden states to Q, K, or V.
fn project_qkv(
    hidden_flat: &Tensor,
    weight: &Tensor,
    bias: &Tensor,
    dims: &ProjectionDims,
    layer_idx: usize,
    name: &str,
) -> EmbeddingResult<Tensor> {
    let projected = hidden_flat
        .matmul(&weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} transpose failed: {}",
                layer_idx, name, e
            ),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} matmul failed: {}",
                layer_idx, name, e
            ),
        })?
        .reshape((dims.batch_size, dims.seq_len, dims.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} reshape failed: {}",
                layer_idx, name, e
            ),
        })?;

    projected
        .broadcast_add(bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} bias failed: {}",
                layer_idx, name, e
            ),
        })
}

/// Reshape tensor for multi-head attention.
fn reshape_for_attention(
    tensor: &Tensor,
    batch_size: usize,
    seq_len: usize,
    num_heads: usize,
    head_dim: usize,
    layer_idx: usize,
    name: &str,
) -> EmbeddingResult<Tensor> {
    tensor
        .reshape((batch_size, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} head reshape failed: {}",
                layer_idx, name, e
            ),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} transpose 1,2 failed: {}",
                layer_idx, name, e
            ),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} contiguous failed: {}",
                layer_idx, name, e
            ),
        })
}
