//! Self-attention with Rotary Position Embeddings for NomicBERT.
//!
//! Implements fused QKV projection and non-interleaved RoPE.
//! Key differences from standard BERT attention:
//! - Single Wqkv [3*hidden, hidden] instead of separate Q/K/V weights
//! - No biases in QKV or output projections
//! - Rotary position embeddings (base=1000, fraction=1.0) instead of learned position embeddings

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::super::config::NomicConfig;
use super::super::weights::NomicAttentionWeights;

/// Precompute rotary embedding cos/sin tables for the given sequence length.
///
/// Returns (cos, sin) tensors each of shape [seq_len, rotary_dim/2].
pub fn compute_rotary_freqs(
    seq_len: usize,
    head_dim: usize,
    base: f64,
    fraction: f64,
    device: &candle_core::Device,
) -> EmbeddingResult<(Tensor, Tensor)> {
    let rotary_dim = (head_dim as f64 * fraction) as usize;
    let half_dim = rotary_dim / 2;

    // Inverse frequencies: 1 / (base^(2i/rotary_dim)) for i in 0..half_dim
    let inv_freq: Vec<f32> = (0..half_dim)
        .map(|i| 1.0 / base.powf(2.0 * i as f64 / rotary_dim as f64) as f32)
        .collect();

    // Compute angles: angles[p][i] = p * inv_freq[i]
    let mut angles = vec![0.0f32; seq_len * half_dim];
    for p in 0..seq_len {
        for i in 0..half_dim {
            angles[p * half_dim + i] = p as f32 * inv_freq[i];
        }
    }

    let angle_tensor =
        Tensor::from_slice(&angles, (seq_len, half_dim), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("RoPE angle tensor failed: {}", e),
            }
        })?;

    let cos = angle_tensor.cos().map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE cos failed: {}", e),
    })?;

    let sin = angle_tensor.sin().map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE sin failed: {}", e),
    })?;

    Ok((cos, sin))
}

/// Apply rotary position embeddings (non-interleaved half-split).
///
/// x: [batch, heads, seq_len, head_dim]
/// cos/sin: [seq_len, half_dim]
///
/// Non-interleaved: split head_dim into first and second halves:
///   new[..., :half] = x[..., :half] * cos - x[..., half:] * sin
///   new[..., half:] = x[..., half:] * cos + x[..., :half] * sin
fn apply_rotary_emb(
    x: &Tensor,
    cos: &Tensor,
    sin: &Tensor,
) -> EmbeddingResult<Tensor> {
    let head_dim = x.dim(3).map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE get head_dim failed: {}", e),
    })?;
    let half = head_dim / 2;

    let x1 = x.narrow(3, 0, half).map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE narrow x1 failed: {}", e),
    })?;
    let x2 = x
        .narrow(3, half, half)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE narrow x2 failed: {}", e),
        })?;

    // Reshape cos/sin from [seq_len, half] to [1, 1, seq_len, half]
    let cos_4d = cos
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE cos unsqueeze 0 failed: {}", e),
        })?
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE cos unsqueeze 1 failed: {}", e),
        })?;
    let sin_4d = sin
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE sin unsqueeze 0 failed: {}", e),
        })?
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE sin unsqueeze 1 failed: {}", e),
        })?;

    // o1 = x1 * cos - x2 * sin
    let o1 = x1
        .broadcast_mul(&cos_4d)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE o1 mul cos failed: {}", e),
        })?
        .broadcast_sub(
            &x2.broadcast_mul(&sin_4d)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("RoPE o1 mul sin failed: {}", e),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE o1 sub failed: {}", e),
        })?;

    // o2 = x2 * cos + x1 * sin
    let o2 = x2
        .broadcast_mul(&cos_4d)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE o2 mul cos failed: {}", e),
        })?
        .broadcast_add(
            &x1.broadcast_mul(&sin_4d)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("RoPE o2 mul sin failed: {}", e),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE o2 add failed: {}", e),
        })?;

    Tensor::cat(&[&o1, &o2], 3).map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE cat failed: {}", e),
    })
}

/// Self-attention forward pass with fused QKV and rotary position embeddings.
pub fn self_attention_forward(
    hidden_states: &Tensor,
    attention: &NomicAttentionWeights,
    attention_mask: &Tensor,
    config: &NomicConfig,
    layer_idx: usize,
    cos: &Tensor,
    sin: &Tensor,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("CausalModel layer {} get dims failed: {}", layer_idx, e),
            })?;

    let num_heads = config.num_attention_heads;
    let head_dim = hidden_size / num_heads;

    // Flatten for matmul: [batch*seq, hidden]
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} flatten failed: {}", layer_idx, e),
        })?;

    // Fused QKV: [batch*seq, hidden] x [3*hidden, hidden]^T = [batch*seq, 3*hidden]
    let qkv = hidden_flat
        .matmul(
            &attention
                .wqkv_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "CausalModel layer {} Wqkv transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} Wqkv matmul failed: {}",
                layer_idx, e
            ),
        })?;

    // Reshape to [batch, seq, 3, heads, head_dim]
    let qkv = qkv
        .reshape((batch_size, seq_len, 3, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} QKV reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // Extract Q, K, V → [batch, heads, seq, head_dim]
    let q = extract_qkv_component(&qkv, 0, layer_idx, "Q")?;
    let k = extract_qkv_component(&qkv, 1, layer_idx, "K")?;
    let v = extract_qkv_component(&qkv, 2, layer_idx, "V")?;

    // Apply RoPE to Q and K
    let q = apply_rotary_emb(&q, cos, sin)?;
    let k = apply_rotary_emb(&k, cos, sin)?;

    // Scaled dot-product attention: softmax(QK^T / sqrt(d)) V
    let scale = (head_dim as f64).sqrt();

    let k_t = k
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

    let scores = q
        .matmul(&k_t)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} QK matmul failed: {}",
                layer_idx, e
            ),
        })?;

    let scores = (scores / scale).map_err(|e| EmbeddingError::GpuError {
        message: format!(
            "CausalModel layer {} attention scale failed: {}",
            layer_idx, e
        ),
    })?;

    let scores = scores
        .broadcast_add(attention_mask)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} attention mask failed: {}",
                layer_idx, e
            ),
        })?;

    let attn_probs =
        candle_nn::ops::softmax(&scores, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("CausalModel layer {} softmax failed: {}", layer_idx, e),
            }
        })?;

    let context = attn_probs
        .matmul(&v)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context matmul failed: {}",
                layer_idx, e
            ),
        })?;

    // Reshape back: [batch, heads, seq, head_dim] → [batch, seq, hidden]
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
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // Output projection (no bias for NomicBERT)
    let context_flat = context
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} output flatten failed: {}",
                layer_idx, e
            ),
        })?;

    context_flat
        .matmul(
            &attention
                .out_proj_weight
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
        })
}

/// Self-attention forward pass with LoRA adapters on Q, V projections.
///
/// Same as `self_attention_forward()` but adds low-rank updates to query and value:
///   Q_final = Q_base + reshape(LoRA_Q(hidden_flat))
///   V_final = V_base + reshape(LoRA_V(hidden_flat))
///
/// Used during training only — inference uses the base `self_attention_forward()`.
pub fn self_attention_forward_with_lora(
    hidden_states: &Tensor,
    attention: &NomicAttentionWeights,
    attention_mask: &Tensor,
    config: &NomicConfig,
    layer_idx: usize,
    cos: &Tensor,
    sin: &Tensor,
    lora_layers: &crate::training::lora::LoraLayers,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("CausalModel layer {} get dims failed: {}", layer_idx, e),
            })?;

    let num_heads = config.num_attention_heads;
    let head_dim = hidden_size / num_heads;

    // Flatten for matmul: [batch*seq, hidden]
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} flatten failed: {}", layer_idx, e),
        })?;

    // Fused QKV: [batch*seq, hidden] x [3*hidden, hidden]^T = [batch*seq, 3*hidden]
    let qkv = hidden_flat
        .matmul(
            &attention
                .wqkv_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "CausalModel layer {} Wqkv transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} Wqkv matmul failed: {}",
                layer_idx, e
            ),
        })?;

    // Reshape to [batch, seq, 3, heads, head_dim]
    let qkv = qkv
        .reshape((batch_size, seq_len, 3, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} QKV reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // Extract Q, K, V → [batch, heads, seq, head_dim]
    let q = extract_qkv_component(&qkv, 0, layer_idx, "Q")?;
    let k = extract_qkv_component(&qkv, 1, layer_idx, "K")?;
    let v = extract_qkv_component(&qkv, 2, layer_idx, "V")?;

    // Apply LoRA deltas to Q: [batch*seq, hidden] → [batch, heads, seq, head_dim]
    let q_delta = lora_layers.apply_query(layer_idx, &hidden_flat)?;
    let q_delta = q_delta
        .reshape((batch_size, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} LoRA Q reshape failed: {}", layer_idx, e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} LoRA Q transpose failed: {}", layer_idx, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} LoRA Q contiguous failed: {}", layer_idx, e),
        })?;
    let q = q.add(&q_delta).map_err(|e| EmbeddingError::GpuError {
        message: format!("CausalModel layer {} LoRA Q add failed: {}", layer_idx, e),
    })?;

    // Apply LoRA deltas to V: [batch*seq, hidden] → [batch, heads, seq, head_dim]
    let v_delta = lora_layers.apply_value(layer_idx, &hidden_flat)?;
    let v_delta = v_delta
        .reshape((batch_size, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} LoRA V reshape failed: {}", layer_idx, e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} LoRA V transpose failed: {}", layer_idx, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} LoRA V contiguous failed: {}", layer_idx, e),
        })?;
    let v = v.add(&v_delta).map_err(|e| EmbeddingError::GpuError {
        message: format!("CausalModel layer {} LoRA V add failed: {}", layer_idx, e),
    })?;

    // Apply RoPE to Q and K
    let q = apply_rotary_emb(&q, cos, sin)?;
    let k = apply_rotary_emb(&k, cos, sin)?;

    // Scaled dot-product attention: softmax(QK^T / sqrt(d)) V
    let scale = (head_dim as f64).sqrt();

    let k_t = k
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

    let scores = q
        .matmul(&k_t)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} QK matmul failed: {}",
                layer_idx, e
            ),
        })?;

    let scores = (scores / scale).map_err(|e| EmbeddingError::GpuError {
        message: format!(
            "CausalModel layer {} attention scale failed: {}",
            layer_idx, e
        ),
    })?;

    let scores = scores
        .broadcast_add(attention_mask)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} attention mask failed: {}",
                layer_idx, e
            ),
        })?;

    let attn_probs =
        candle_nn::ops::softmax(&scores, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("CausalModel layer {} softmax failed: {}", layer_idx, e),
            }
        })?;

    let context = attn_probs
        .matmul(&v)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context matmul failed: {}",
                layer_idx, e
            ),
        })?;

    // Reshape back: [batch, heads, seq, head_dim] → [batch, seq, hidden]
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
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} context reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // Output projection (no bias for NomicBERT)
    let context_flat = context
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} output flatten failed: {}",
                layer_idx, e
            ),
        })?;

    context_flat
        .matmul(
            &attention
                .out_proj_weight
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
        })
}

/// Extract Q, K, or V from the fused QKV tensor.
///
/// qkv: [batch, seq, 3, heads, head_dim] → component: [batch, heads, seq, head_dim]
fn extract_qkv_component(
    qkv: &Tensor,
    index: usize,
    layer_idx: usize,
    name: &str,
) -> EmbeddingResult<Tensor> {
    qkv.narrow(2, index, 1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} narrow failed: {}",
                layer_idx, name, e
            ),
        })?
        .squeeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} squeeze failed: {}",
                layer_idx, name, e
            ),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} {} transpose failed: {}",
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
