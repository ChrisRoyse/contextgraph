//! Dense (full O(n^2)) scaled dot-product attention.
//!
//! This is the traditional attention implementation that materializes the
//! full [seq_len, seq_len] score matrix. It wraps the exact operations
//! from the original causal and code attention implementations for
//! bit-exact backward compatibility.
//!
//! Memory: O(n^2) for the score matrix
//! Compute: O(n^2 * d) for matmul

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::AttentionStrategy;

/// Dense attention: full O(n^2) score matrix materialization.
///
/// This is the default fallback and produces bit-exact results with the
/// legacy inline attention code.
pub struct DenseAttention;

impl AttentionStrategy for DenseAttention {
    fn forward(
        &self,
        q: &Tensor,
        k: &Tensor,
        v: &Tensor,
        mask: &Tensor,
        scale: f64,
    ) -> EmbeddingResult<Tensor> {
        // K^T: [batch, heads, head_dim, seq_len]
        let k_t = k
            .transpose(2, 3)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("DenseAttention K transpose failed: {}", e),
            })?
            .contiguous()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("DenseAttention K^T contiguous failed: {}", e),
            })?;

        // QK^T: [batch, heads, seq_len, seq_len]
        let scores = q.matmul(&k_t).map_err(|e| EmbeddingError::GpuError {
            message: format!("DenseAttention QK matmul failed: {}", e),
        })?;

        // Scale by 1/sqrt(head_dim)
        let scores = (scores / scale).map_err(|e| EmbeddingError::GpuError {
            message: format!("DenseAttention scale failed: {}", e),
        })?;

        // Add attention mask
        let scores = scores
            .broadcast_add(mask)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("DenseAttention mask add failed: {}", e),
            })?;

        // Softmax over last dimension
        let attn_probs = candle_nn::ops::softmax(&scores, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("DenseAttention softmax failed: {}", e),
            }
        })?;

        // Weighted sum: [batch, heads, seq_len, head_dim]
        attn_probs.matmul(v).map_err(|e| EmbeddingError::GpuError {
            message: format!("DenseAttention context matmul failed: {}", e),
        })
    }

    fn name(&self) -> &str {
        "dense"
    }
}
