//! Grouped-Query Attention (GQA) implementation for Qwen2.
//!
//! Implements the multi-head attention with GQA support where
//! multiple query heads share the same key-value heads.

use candle_core::{DType, Tensor};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::models::attention::AttentionStrategy;

use super::config::QwenConfig;
use super::position::{apply_rope, RopeCache};
use super::weights::QwenAttentionWeights;

/// Run Grouped-Query Attention forward pass.
///
/// GQA has num_attention_heads query heads but only num_key_value_heads KV heads.
/// Each group of (num_attention_heads / num_key_value_heads) query heads shares
/// the same key-value pair.
pub fn gqa_forward(
    hidden_states: &Tensor,
    attention: &QwenAttentionWeights,
    attention_mask: &Tensor,
    rope_cache: &RopeCache,
    config: &QwenConfig,
    layer_idx: usize,
    strategy: &dyn AttentionStrategy,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} get dims failed: {}", layer_idx, e),
            })?;

    let num_heads = config.num_attention_heads;
    let num_kv_heads = config.num_key_value_heads;
    let head_dim = config.head_dim;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} flatten hidden failed: {}", layer_idx, e),
        })?;

    // Q projection: [batch*seq, hidden] @ [hidden, hidden]^T + bias
    let query = linear_with_bias(
        &hidden_flat,
        &attention.q_proj_weight,
        &attention.q_proj_bias,
        layer_idx,
        "Q",
    )?;

    // K projection: [batch*seq, hidden] @ [hidden, kv_dim]^T + bias
    let key = linear_with_bias(
        &hidden_flat,
        &attention.k_proj_weight,
        &attention.k_proj_bias,
        layer_idx,
        "K",
    )?;

    // V projection: [batch*seq, hidden] @ [hidden, kv_dim]^T + bias
    let value = linear_with_bias(
        &hidden_flat,
        &attention.v_proj_weight,
        &attention.v_proj_bias,
        layer_idx,
        "V",
    )?;

    // Reshape Q to [batch, num_heads, seq_len, head_dim]
    let query = query
        .reshape((batch_size, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} Q reshape 1 failed: {}", layer_idx, e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} Q transpose failed: {}", layer_idx, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} Q contiguous failed: {}", layer_idx, e),
        })?;

    // Reshape K to [batch, num_kv_heads, seq_len, head_dim]
    let key = key
        .reshape((batch_size, seq_len, num_kv_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} K reshape 1 failed: {}", layer_idx, e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} K transpose failed: {}", layer_idx, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} K contiguous failed: {}", layer_idx, e),
        })?;

    // Reshape V to [batch, num_kv_heads, seq_len, head_dim]
    let value = value
        .reshape((batch_size, seq_len, num_kv_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} V reshape 1 failed: {}", layer_idx, e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} V transpose failed: {}", layer_idx, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} V contiguous failed: {}", layer_idx, e),
        })?;

    // Apply RoPE to Q and K
    let (query, key) = apply_rope(&query, &key, &rope_cache.cos, &rope_cache.sin, seq_len)?;

    // Expand KV heads for GQA: repeat each KV head (num_heads / num_kv_heads) times
    let num_groups = num_heads / num_kv_heads;
    let key = repeat_kv(&key, num_groups, layer_idx)?;
    let value = repeat_kv(&value, num_groups, layer_idx)?;

    // Convert Q, K, V, mask to F32 for numerical stability (FP16 max ~65504).
    // The attention strategy operates in the dtype it receives, so we pass F32
    // and convert the output back to the original dtype afterwards.
    let original_dtype = query.dtype();
    let query_f32 = query
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} Q to F32 failed: {}", layer_idx, e),
        })?;
    let key_f32 = key
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} K to F32 failed: {}", layer_idx, e),
        })?;
    let value_f32 = value
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} V to F32 failed: {}", layer_idx, e),
        })?;
    let attention_mask_f32 =
        attention_mask
            .to_dtype(DType::F32)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} mask to F32 failed: {}", layer_idx, e),
            })?;

    // Dispatch to attention strategy (scale = sqrt(head_dim), strategy divides by it)
    let scale = (head_dim as f64).sqrt();
    let context = strategy.forward(&query_f32, &key_f32, &value_f32, &attention_mask_f32, scale)?;

    // Convert context back to original dtype (FP16)
    let context = context
        .to_dtype(original_dtype)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "Qwen2 layer {} context to original dtype failed: {}",
                layer_idx, e
            ),
        })?;

    // Reshape back: [batch, seq_len, hidden_size]
    let context = context
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} context transpose failed: {}", layer_idx, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} context contiguous failed: {}", layer_idx, e),
        })?
        .reshape((batch_size, seq_len, num_heads * head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} context reshape failed: {}", layer_idx, e),
        })?;

    // Output projection: [batch*seq, hidden] @ [hidden, hidden]^T
    let context_flat = context
        .reshape((batch_size * seq_len, num_heads * head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} O flatten failed: {}", layer_idx, e),
        })?;

    context_flat
        .matmul(
            &attention
                .o_proj_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} O transpose failed: {}", layer_idx, e),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} O matmul failed: {}", layer_idx, e),
        })?
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} O reshape failed: {}", layer_idx, e),
        })
}

/// Linear layer with bias: x @ W^T + b
fn linear_with_bias(
    x: &Tensor,
    weight: &Tensor,
    bias: &Tensor,
    layer_idx: usize,
    name: &str,
) -> EmbeddingResult<Tensor> {
    x.matmul(&weight.t().map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 layer {} {} transpose failed: {}", layer_idx, name, e),
    })?)
    .map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 layer {} {} matmul failed: {}", layer_idx, name, e),
    })?
    .broadcast_add(bias)
    .map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 layer {} {} bias add failed: {}", layer_idx, name, e),
    })
}

/// Repeat KV heads for GQA.
///
/// Expands [batch, num_kv_heads, seq_len, head_dim] to
/// [batch, num_heads, seq_len, head_dim] by repeating each KV head.
fn repeat_kv(x: &Tensor, num_groups: usize, layer_idx: usize) -> EmbeddingResult<Tensor> {
    if num_groups == 1 {
        return Ok(x.clone());
    }

    let dims = x.dims();
    let (batch, num_kv_heads, seq_len, head_dim) = (dims[0], dims[1], dims[2], dims[3]);

    // Expand: [batch, num_kv_heads, seq_len, head_dim] -> [batch, num_kv_heads, num_groups, seq_len, head_dim]
    let expanded = x
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "Qwen2 layer {} KV expand unsqueeze failed: {}",
                layer_idx, e
            ),
        })?
        .expand((batch, num_kv_heads, num_groups, seq_len, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} KV expand failed: {}", layer_idx, e),
        })?;

    // Reshape to [batch, num_heads, seq_len, head_dim]
    expanded
        .reshape((batch, num_kv_heads * num_groups, seq_len, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} KV reshape failed: {}", layer_idx, e),
        })
}
