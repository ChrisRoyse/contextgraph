//! Sliding window attention — O(n*w) complexity.
//!
//! Each query token attends only to keys within a fixed window around it.
//! This reduces both memory and compute from O(n^2) to O(n*w) where
//! w is the window size.
//!
//! For bidirectional models (E5 Causal / NomicBERT), each query at position i
//! attends to keys in [max(0, i - w/2), min(n, i + w/2)].
//!
//! When window_size >= seq_len, this produces identical output to dense attention.

use candle_core::{DType, Tensor};

use crate::error::{EmbeddingError, EmbeddingResult};

use super::AttentionStrategy;

/// Sliding window attention with fixed window size.
///
/// Complexity: O(n * w) instead of O(n^2).
/// Each token attends to at most `window_size` neighboring tokens.
pub struct SlidingWindowAttention {
    window_size: usize,
}

impl SlidingWindowAttention {
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }
}

impl AttentionStrategy for SlidingWindowAttention {
    fn forward(
        &self,
        q: &Tensor,
        k: &Tensor,
        v: &Tensor,
        mask: &Tensor,
        scale: f64,
    ) -> EmbeddingResult<Tensor> {
        let (_batch_size, _num_heads, seq_len, _head_dim) =
            q.dims4().map_err(|e| EmbeddingError::GpuError {
                message: format!("SlidingWindowAttention Q dims failed: {}", e),
            })?;

        // If window covers the full sequence, use dense attention
        if self.window_size >= seq_len {
            return super::dense::DenseAttention.forward(q, k, v, mask, scale);
        }

        // Create a sliding window mask that restricts attention to local context.
        // For position i, allow attending to [i - w/2, i + w/2].
        // This is implemented by creating a [seq_len, seq_len] mask with
        // -10000 outside the window and 0 inside, then adding it to the
        // existing mask.
        let device = q.device();
        let half_w = self.window_size / 2;

        // Build window mask: 0 inside window, -10000 outside
        let mut window_mask_data = vec![-10000.0f32; seq_len * seq_len];
        for i in 0..seq_len {
            let start = i.saturating_sub(half_w);
            let end = (i + half_w + 1).min(seq_len);
            for j in start..end {
                window_mask_data[i * seq_len + j] = 0.0;
            }
        }

        let window_mask = Tensor::from_slice(&window_mask_data, (1, 1, seq_len, seq_len), device)
            .map_err(|e| EmbeddingError::GpuError {
            message: format!("SlidingWindowAttention window mask create failed: {}", e),
        })?;

        // Convert window mask to match input dtype
        let window_mask =
            window_mask
                .to_dtype(mask.dtype())
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("SlidingWindowAttention window mask dtype failed: {}", e),
                })?;

        // Combine with existing padding mask by adding both
        // (both use 0 for allowed and -10000 for blocked)
        let combined_mask =
            mask.broadcast_add(&window_mask)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("SlidingWindowAttention mask combine failed: {}", e),
                })?;

        // K^T: [batch, heads, head_dim, seq_len]
        let k_t = k
            .transpose(2, 3)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("SlidingWindowAttention K transpose failed: {}", e),
            })?
            .contiguous()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("SlidingWindowAttention K^T contiguous failed: {}", e),
            })?;

        // QK^T: [batch, heads, seq_len, seq_len]
        let scores = q.matmul(&k_t).map_err(|e| EmbeddingError::GpuError {
            message: format!("SlidingWindowAttention QK matmul failed: {}", e),
        })?;

        // Scale
        let scores = (scores / scale).map_err(|e| EmbeddingError::GpuError {
            message: format!("SlidingWindowAttention scale failed: {}", e),
        })?;

        // Apply combined mask (padding + window)
        let scores =
            scores
                .broadcast_add(&combined_mask)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("SlidingWindowAttention mask add failed: {}", e),
                })?;

        // Softmax: window-masked positions get ~0 probability
        // Need F32 for softmax stability with large negative values
        let scores_f32 = scores
            .to_dtype(DType::F32)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("SlidingWindowAttention scores to F32 failed: {}", e),
            })?;

        let attn_probs =
            candle_nn::ops::softmax(&scores_f32, candle_core::D::Minus1).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("SlidingWindowAttention softmax failed: {}", e),
                }
            })?;

        // Convert back to original dtype for V matmul
        let attn_probs = attn_probs
            .to_dtype(v.dtype())
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("SlidingWindowAttention probs dtype failed: {}", e),
            })?;

        // Context: [batch, heads, seq_len, head_dim]
        attn_probs.matmul(v).map_err(|e| EmbeddingError::GpuError {
            message: format!("SlidingWindowAttention context matmul failed: {}", e),
        })
    }

    fn name(&self) -> &str {
        "sliding_window"
    }
}
