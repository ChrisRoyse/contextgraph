//! Self-attention layer for the BGE-M3 Dense encoder (XLM-RoBERTa).
//!
//! Architecturally identical to BERT self-attention. The only XLM-R-specific
//! nuance (position embedding indexing) lives in `embeddings.rs`; the attention
//! itself treats the sequence as any other dense transformer.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{AttentionWeights, BertConfig};

/// Dimensions for Q/K/V projection operations.
struct ProjectionDims {
    batch_size: usize,
    seq_len: usize,
    hidden_size: usize,
}

/// Run self-attention forward pass.
pub fn self_attention_forward(
    hidden_states: &Tensor,
    attention: &AttentionWeights,
    attention_mask: &Tensor,
    config: &BertConfig,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, _hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("BgeM3Dense layer {} get dims failed: {}", layer_idx, e),
            })?;

    let head_dim = config.hidden_size / config.num_attention_heads;
    let hidden_size = config.hidden_size;

    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} hidden flatten failed: {}",
                layer_idx, e
            ),
        })?;

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

    let key_t = key
        .transpose(2, 3)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} K transpose 2,3 failed: {}",
                layer_idx, e
            ),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} K^T contiguous failed: {}",
                layer_idx, e
            ),
        })?;

    let scores = query.matmul(&key_t).map_err(|e| EmbeddingError::GpuError {
        message: format!("BgeM3Dense layer {} QK matmul failed: {}", layer_idx, e),
    })?;

    let scale = (head_dim as f64).sqrt();
    let scores = (scores / scale).map_err(|e| EmbeddingError::GpuError {
        message: format!(
            "BgeM3Dense layer {} attention scale failed: {}",
            layer_idx, e
        ),
    })?;

    let scores = scores
        .broadcast_add(attention_mask)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} attention mask add failed: {}",
                layer_idx, e
            ),
        })?;

    let attention_probs =
        candle_nn::ops::softmax(&scores, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("BgeM3Dense layer {} softmax failed: {}", layer_idx, e),
            }
        })?;

    let context = attention_probs
        .matmul(&value)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} context matmul failed: {}",
                layer_idx, e
            ),
        })?;

    let context = context
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} context transpose failed: {}",
                layer_idx, e
            ),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} context contiguous failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} context reshape failed: {}",
                layer_idx, e
            ),
        })?;

    let context_flat = context
        .reshape((batch_size * seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} context flatten failed: {}",
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
                        "BgeM3Dense layer {} output transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense layer {} output matmul failed: {}", layer_idx, e),
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} output reshape failed: {}",
                layer_idx, e
            ),
        })?;

    output
        .broadcast_add(&attention.output_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense layer {} output bias failed: {}", layer_idx, e),
        })
}

fn compute_qkv_projection(
    hidden_flat: &Tensor,
    weight: &Tensor,
    bias: &Tensor,
    dims: &ProjectionDims,
    layer_idx: usize,
    name: &str,
) -> EmbeddingResult<Tensor> {
    let proj = hidden_flat
        .matmul(&weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} {} transpose failed: {}",
                layer_idx, name, e
            ),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} {} matmul failed: {}",
                layer_idx, name, e
            ),
        })?
        .reshape((dims.batch_size, dims.seq_len, dims.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} {} reshape failed: {}",
                layer_idx, name, e
            ),
        })?;

    proj.broadcast_add(bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense layer {} {} bias failed: {}", layer_idx, name, e),
        })
}

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
                "BgeM3Dense layer {} {} reshape failed: {}",
                layer_idx, name, e
            ),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} {} transpose 1,2 failed: {}",
                layer_idx, name, e
            ),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} {} contiguous failed: {}",
                layer_idx, name, e
            ),
        })
}
