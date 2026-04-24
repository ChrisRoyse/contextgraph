//! LayerNorm implementation for the BGE-M3 Dense encoder.
//!
//! Identical math to BERT LayerNorm — extracted to keep error messages
//! scoped to this module and to avoid cross-module coupling.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

/// Apply LayerNorm: `(x - mean) / sqrt(var + eps) * weight + bias`.
pub fn layer_norm(x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f64) -> EmbeddingResult<Tensor> {
    let mean = x
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm mean failed: {}", e),
        })?;

    let x_centered = x
        .broadcast_sub(&mean)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm center failed: {}", e),
        })?;

    let var = x_centered
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm sqr failed: {}", e),
        })?
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm var mean failed: {}", e),
        })?;

    let std = (var + eps)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm var add eps failed: {}", e),
        })?
        .sqrt()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm sqrt failed: {}", e),
        })?;

    let normalized = x_centered
        .broadcast_div(&std)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm div failed: {}", e),
        })?;

    let scaled = normalized
        .broadcast_mul(weight)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm scale failed: {}", e),
        })?;

    scaled
        .broadcast_add(bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense LayerNorm bias failed: {}", e),
        })
}
