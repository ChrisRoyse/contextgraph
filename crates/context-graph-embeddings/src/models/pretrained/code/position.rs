//! Rotary Position Embedding (RoPE) for Qwen2 attention.
//!
//! Implements the rotary position embedding used in Qwen2 and similar
//! models for encoding position information into attention.

use candle_core::{DType, Device, Tensor};

use crate::error::{EmbeddingError, EmbeddingResult};

/// Precomputed RoPE frequencies for efficient position encoding.
#[derive(Debug)]
pub struct RopeCache {
    /// Cosine frequencies: [max_seq_len, head_dim/2]
    pub cos: Tensor,
    /// Sine frequencies: [max_seq_len, head_dim/2]
    pub sin: Tensor,
}

impl RopeCache {
    /// Create RoPE frequency cache for the given configuration.
    pub fn new(
        max_seq_len: usize,
        head_dim: usize,
        rope_theta: f64,
        device: &Device,
        dtype: DType,
    ) -> EmbeddingResult<Self> {
        // Compute inverse frequencies: 1 / (theta^(2i/d))
        let half_dim = head_dim / 2;
        let mut inv_freq = Vec::with_capacity(half_dim);
        for i in 0..half_dim {
            let freq = 1.0 / rope_theta.powf(2.0 * (i as f64) / (head_dim as f64));
            inv_freq.push(freq as f32);
        }

        // Create position indices: [0, 1, 2, ..., max_seq_len-1]
        let positions: Vec<f32> = (0..max_seq_len).map(|i| i as f32).collect();

        // Compute outer product: positions * inv_freq -> [max_seq_len, half_dim]
        let positions_tensor =
            Tensor::from_slice(&positions, (max_seq_len, 1), device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("RoPE positions tensor creation failed: {}", e),
                }
            })?;

        let inv_freq_tensor =
            Tensor::from_slice(&inv_freq, (1, half_dim), device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("RoPE inv_freq tensor creation failed: {}", e),
                }
            })?;

        let freqs =
            positions_tensor
                .matmul(&inv_freq_tensor)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("RoPE frequency computation failed: {}", e),
                })?;

        // Compute cos and sin
        let cos = freqs.cos().map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE cos computation failed: {}", e),
        })?;

        let sin = freqs.sin().map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE sin computation failed: {}", e),
        })?;

        // Convert to target dtype
        let cos = cos.to_dtype(dtype).map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE cos dtype conversion failed: {}", e),
        })?;

        let sin = sin.to_dtype(dtype).map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE sin dtype conversion failed: {}", e),
        })?;

        Ok(RopeCache { cos, sin })
    }
}

/// Apply RoPE to query and key tensors.
///
/// # Arguments
/// * `q` - Query tensor: [batch, num_heads, seq_len, head_dim]
/// * `k` - Key tensor: [batch, num_kv_heads, seq_len, head_dim]
/// * `cos` - Cosine frequencies: [max_seq_len, head_dim/2]
/// * `sin` - Sine frequencies: [max_seq_len, head_dim/2]
/// * `seq_len` - Current sequence length
///
/// # Returns
/// Tuple of (rotated_q, rotated_k)
pub fn apply_rope(
    q: &Tensor,
    k: &Tensor,
    cos: &Tensor,
    sin: &Tensor,
    seq_len: usize,
) -> EmbeddingResult<(Tensor, Tensor)> {
    // Slice cos/sin to current sequence length
    let cos = cos
        .narrow(0, 0, seq_len)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE cos slice failed: {}", e),
        })?;
    let sin = sin
        .narrow(0, 0, seq_len)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE sin slice failed: {}", e),
        })?;

    let rotated_q = apply_rope_single(q, &cos, &sin)?;
    let rotated_k = apply_rope_single(k, &cos, &sin)?;

    Ok((rotated_q, rotated_k))
}

/// Apply RoPE to a single tensor.
///
/// Uses the "rotate_half" approach:
/// - Split tensor into two halves along last dimension
/// - Rotate by applying: x * cos + rotate_half(x) * sin
fn apply_rope_single(x: &Tensor, cos: &Tensor, sin: &Tensor) -> EmbeddingResult<Tensor> {
    let dims = x.dims();
    if dims.len() != 4 {
        return Err(EmbeddingError::GpuError {
            message: format!("RoPE expects 4D tensor, got {}D", dims.len()),
        });
    }

    let (_batch, _num_heads, _seq_len, head_dim) = (dims[0], dims[1], dims[2], dims[3]);
    let half_dim = head_dim / 2;

    // Split into first and second half
    let x1 = x
        .narrow(3, 0, half_dim)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE x1 narrow failed: {}", e),
        })?;
    let x2 = x
        .narrow(3, half_dim, half_dim)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE x2 narrow failed: {}", e),
        })?;

    // Reshape cos/sin for broadcasting: [seq_len, half_dim] -> [1, 1, seq_len, half_dim]
    let cos = cos
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE cos unsqueeze 0 failed: {}", e),
        })?
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE cos unsqueeze 1 failed: {}", e),
        })?;

    let sin = sin
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE sin unsqueeze 0 failed: {}", e),
        })?
        .unsqueeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE sin unsqueeze 1 failed: {}", e),
        })?;

    // Apply rotation: [x1, x2] * cos + [-x2, x1] * sin
    let x1_cos = x1
        .broadcast_mul(&cos)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE x1*cos failed: {}", e),
        })?;
    let x2_cos = x2
        .broadcast_mul(&cos)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE x2*cos failed: {}", e),
        })?;

    let neg_x2 = x2.neg().map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE -x2 failed: {}", e),
    })?;
    let neg_x2_sin = neg_x2
        .broadcast_mul(&sin)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE -x2*sin failed: {}", e),
        })?;
    let x1_sin = x1
        .broadcast_mul(&sin)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE x1*sin failed: {}", e),
        })?;

    // o1 = x1 * cos - x2 * sin
    // o2 = x2 * cos + x1 * sin
    let o1 = x1_cos
        .add(&neg_x2_sin)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("RoPE o1 add failed: {}", e),
        })?;
    let o2 = x2_cos.add(&x1_sin).map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE o2 add failed: {}", e),
    })?;

    // Concatenate back: [batch, heads, seq_len, head_dim]
    Tensor::cat(&[&o1, &o2], 3).map_err(|e| EmbeddingError::GpuError {
        message: format!("RoPE concat failed: {}", e),
    })
}
