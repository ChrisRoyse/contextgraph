//! Grouped-Query Attention (GQA) implementation for Qwen2.
//!
//! Implements the multi-head attention with GQA support where
//! multiple query heads share the same key-value heads.

use candle_core::{DType, Tensor};

use crate::error::{EmbeddingError, EmbeddingResult};

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
    let _kv_dim = num_kv_heads * head_dim;

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

    // K^T with contiguous() for matmul
    let key_t = key
        .transpose(2, 3)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} K transpose 2,3 failed: {}", layer_idx, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} K^T contiguous failed: {}", layer_idx, e),
        })?;

    // Attention scores in FP32 to prevent overflow (FP16 max ~65504)
    // Q @ K^T can easily overflow in FP16 for longer sequences
    let query_f32 = query
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} Q to F32 failed: {}", layer_idx, e),
        })?;
    let key_t_f32 = key_t
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} K^T to F32 failed: {}", layer_idx, e),
        })?;
    let attention_mask_f32 =
        attention_mask
            .to_dtype(DType::F32)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} mask to F32 failed: {}", layer_idx, e),
            })?;

    // Attention scores: Q @ K^T / sqrt(head_dim) in FP32
    let scale = 1.0 / (head_dim as f64).sqrt();
    let scores = query_f32
        .matmul(&key_t_f32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} QK matmul failed: {}", layer_idx, e),
        })?
        .affine(scale, 0.0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} attention scale failed: {}", layer_idx, e),
        })?;

    // Add attention mask (already in F32)
    let scores =
        scores
            .broadcast_add(&attention_mask_f32)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} attention mask add failed: {}", layer_idx, e),
            })?;

    // Softmax in FP32 (scores already in F32)
    let attention_probs_f32 =
        candle_nn::ops::softmax(&scores, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} softmax failed: {}", layer_idx, e),
            }
        })?;
    // Convert back to FP16 for context matmul
    let attention_probs =
        attention_probs_f32
            .to_dtype(DType::F16)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} softmax to F16 failed: {}", layer_idx, e),
            })?;

    // Apply attention to values: [batch, heads, seq_len, head_dim]
    let context = attention_probs
        .matmul(&value)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} context matmul failed: {}", layer_idx, e),
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
