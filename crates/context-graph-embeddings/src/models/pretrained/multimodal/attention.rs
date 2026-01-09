//! Multi-head self-attention implementation for CLIP.
//!
//! Uses flatten/reshape pattern for Candle matmul compatibility (requires 2D tensors).
//! All transposes followed by contiguous() for memory layout compatibility.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

/// Multi-head self-attention.
///
/// Uses flatten/reshape pattern for Candle matmul compatibility (requires 2D tensors).
/// All transposes followed by contiguous() for memory layout compatibility.
#[allow(clippy::too_many_arguments)]
pub fn self_attention(
    hidden_states: &Tensor,
    q_weight: &Tensor,
    q_bias: &Tensor,
    k_weight: &Tensor,
    k_bias: &Tensor,
    v_weight: &Tensor,
    v_bias: &Tensor,
    out_weight: &Tensor,
    out_bias: &Tensor,
    num_heads: usize,
    mask: Option<&Tensor>,
) -> EmbeddingResult<Tensor> {
    let (batch, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Dims3 failed: {}", e),
            })?;
    let head_dim = hidden_size / num_heads;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Flatten hidden failed: {}", e),
        })?;

    // Q, K, V projections with flatten/reshape pattern
    let q = project_qkv(
        &hidden_flat,
        q_weight,
        q_bias,
        batch,
        seq_len,
        hidden_size,
        "Q",
    )?;
    let k = project_qkv(
        &hidden_flat,
        k_weight,
        k_bias,
        batch,
        seq_len,
        hidden_size,
        "K",
    )?;
    let v = project_qkv(
        &hidden_flat,
        v_weight,
        v_bias,
        batch,
        seq_len,
        hidden_size,
        "V",
    )?;

    // Reshape for multi-head: [batch, seq, heads, head_dim]
    let q = reshape_for_heads(&q, batch, seq_len, num_heads, head_dim, "Q")?;
    let k = reshape_for_heads(&k, batch, seq_len, num_heads, head_dim, "K")?;
    let v = reshape_for_heads(&v, batch, seq_len, num_heads, head_dim, "V")?;

    // Transpose to [batch, heads, seq, head_dim] with contiguous() for Candle
    let q = transpose_for_attention(&q, "Q")?;
    let k = transpose_for_attention(&k, "K")?;
    let v = transpose_for_attention(&v, "V")?;

    // K^T with contiguous() for matmul
    let k_t = k
        .transpose(2, 3)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("K transpose 2,3 failed: {}", e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("K^T contiguous failed: {}", e),
        })?;

    // Attention scores: Q @ K^T / sqrt(head_dim)
    let scale = (head_dim as f64).sqrt();
    let attn_weights = q.matmul(&k_t).map_err(|e| EmbeddingError::GpuError {
        message: format!("QK matmul failed: {}", e),
    })?;
    let attn_weights = (attn_weights / scale).map_err(|e| EmbeddingError::GpuError {
        message: format!("Scale divide failed: {}", e),
    })?;

    // Apply mask if provided with broadcast_add
    let attn_weights = if let Some(m) = mask {
        attn_weights
            .broadcast_add(m)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Mask addition failed: {}", e),
            })?
    } else {
        attn_weights
    };

    // Softmax
    let attn_weights =
        candle_nn::ops::softmax(&attn_weights, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Softmax failed: {}", e),
            }
        })?;

    // Attention output: attn_weights @ V
    let attn_output = attn_weights
        .matmul(&v)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Attention V matmul failed: {}", e),
        })?;

    // Transpose back: [batch, heads, seq, head_dim] -> [batch, seq, heads, head_dim]
    let attn_output = attn_output
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Output transpose failed: {}", e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Output contiguous failed: {}", e),
        })?;

    // Reshape: [batch, seq, hidden_size]
    let attn_output = attn_output
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Output reshape failed: {}", e),
        })?;

    // Output projection with flatten/reshape pattern
    let attn_flat = attn_output
        .reshape((batch * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Output flatten failed: {}", e),
        })?;

    let output = attn_flat
        .matmul(&out_weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("Out weight transpose failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Output projection failed: {}", e),
        })?
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Output reshape failed: {}", e),
        })?;
    let output = output
        .broadcast_add(out_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Output bias failed: {}", e),
        })?;

    Ok(output)
}

/// Project Q, K, or V with weight and bias.
fn project_qkv(
    hidden_flat: &Tensor,
    weight: &Tensor,
    bias: &Tensor,
    batch: usize,
    seq_len: usize,
    hidden_size: usize,
    name: &str,
) -> EmbeddingResult<Tensor> {
    let proj = hidden_flat
        .matmul(&weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("{} weight transpose failed: {}", name, e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("{} projection failed: {}", name, e),
        })?
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("{} reshape failed: {}", name, e),
        })?;
    proj.broadcast_add(bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("{} bias failed: {}", name, e),
        })
}

/// Reshape tensor for multi-head attention.
fn reshape_for_heads(
    tensor: &Tensor,
    batch: usize,
    seq_len: usize,
    num_heads: usize,
    head_dim: usize,
    name: &str,
) -> EmbeddingResult<Tensor> {
    tensor
        .reshape((batch, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("{} head reshape failed: {}", name, e),
        })
}

/// Transpose tensor for attention computation.
fn transpose_for_attention(tensor: &Tensor, name: &str) -> EmbeddingResult<Tensor> {
    tensor
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("{} transpose failed: {}", name, e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("{} contiguous failed: {}", name, e),
        })
}
