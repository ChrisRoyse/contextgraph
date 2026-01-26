//! Tensor operations for Longformer model.
//!
//! This module contains LayerNorm, mean pooling, and L2 normalization.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

/// Apply LayerNorm.
pub fn layer_norm(x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f64) -> EmbeddingResult<Tensor> {
    let mean = x
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm mean failed: {}", e),
        })?;

    let x_centered = x
        .broadcast_sub(&mean)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm center failed: {}", e),
        })?;

    let var = x_centered
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm sqr failed: {}", e),
        })?
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm var mean failed: {}", e),
        })?;

    let eps_tensor = Tensor::ones_like(&var)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm create eps ones failed: {}", e),
        })?
        .affine(eps, 0.0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm eps scale failed: {}", e),
        })?;

    let std = var
        .broadcast_add(&eps_tensor)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm var add eps failed: {}", e),
        })?
        .sqrt()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm sqrt failed: {}", e),
        })?;

    let normalized = x_centered
        .broadcast_div(&std)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm div failed: {}", e),
        })?;

    let scaled = normalized
        .broadcast_mul(weight)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm scale failed: {}", e),
        })?;

    scaled
        .broadcast_add(bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel LayerNorm bias failed: {}", e),
        })
}

/// Mean pooling over sequence dimension.
pub fn mean_pooling(hidden_states: &Tensor, attention_mask: &Tensor) -> EmbeddingResult<Tensor> {
    let mask_expanded = attention_mask
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel mask expand failed: {}", e),
        })?
        .broadcast_as(hidden_states.shape())
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel mask broadcast failed: {}", e),
        })?;

    let masked_hidden = (hidden_states * mask_expanded).map_err(|e| EmbeddingError::GpuError {
        message: format!("CausalModel mask apply failed: {}", e),
    })?;

    let sum_hidden = masked_hidden.sum(1).map_err(|e| EmbeddingError::GpuError {
        message: format!("CausalModel sum pooling failed: {}", e),
    })?;

    let mask_sum = attention_mask
        .sum_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel mask sum failed: {}", e),
        })?;

    sum_hidden
        .broadcast_div(&mask_sum)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel mean div failed: {}", e),
        })
}

/// Marker-weighted pooling over sequence dimension.
///
/// Similar to mean pooling, but applies per-token weights to emphasize
/// causal marker tokens. This creates differentiated cause/effect embeddings.
///
/// # Arguments
///
/// * `hidden_states` - Hidden states tensor [batch, seq_len, hidden_size]
/// * `attention_mask` - Attention mask [batch, seq_len]
/// * `marker_weights` - Per-token weights [seq_len], e.g., 2.5 for markers, 1.0 for normal
///
/// # Returns
///
/// Pooled tensor [batch, hidden_size]
pub fn marker_weighted_pooling(
    hidden_states: &Tensor,
    attention_mask: &Tensor,
    marker_weights: &[f32],
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Marker pool get dims failed: {}", e),
            })?;

    // Create marker weights tensor [1, seq_len]
    let device = hidden_states.device();
    let marker_tensor = Tensor::from_slice(marker_weights, (1, seq_len), device)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool create weights tensor failed: {}", e),
        })?;

    // Combined weights: attention_mask * marker_weights [batch, seq_len]
    let combined_weights = attention_mask
        .broadcast_mul(&marker_tensor)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool combine weights failed: {}", e),
        })?;

    // Expand to hidden dimension: [batch, seq_len] -> [batch, seq_len, hidden_size]
    let weights_expanded = combined_weights
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool weights expand failed: {}", e),
        })?
        .broadcast_as((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool weights broadcast failed: {}", e),
        })?;

    // Weighted hidden states
    let weighted_hidden =
        (hidden_states * &weights_expanded).map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool apply weights failed: {}", e),
        })?;

    // Sum over sequence dimension
    let sum_hidden = weighted_hidden.sum(1).map_err(|e| EmbeddingError::GpuError {
        message: format!("Marker pool sum hidden failed: {}", e),
    })?;

    // Sum of weights for normalization
    let weight_sum = combined_weights.sum(1).map_err(|e| EmbeddingError::GpuError {
        message: format!("Marker pool weight sum failed: {}", e),
    })?;

    // Expand weight_sum to match hidden_size for division
    let weight_sum_expanded = weight_sum
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool weight sum expand failed: {}", e),
        })?
        .broadcast_as((batch_size, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool weight sum broadcast failed: {}", e),
        })?;

    // Divide for weighted average
    sum_hidden
        .broadcast_div(&weight_sum_expanded)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Marker pool div failed: {}", e),
        })
}

/// L2 normalize a tensor.
pub fn l2_normalize(tensor: &Tensor) -> EmbeddingResult<Tensor> {
    let norm = tensor
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel sqr failed: {}", e),
        })?
        .sum_keepdim(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel norm sum failed: {}", e),
        })?
        .sqrt()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel sqrt failed: {}", e),
        })?;

    let eps_tensor = Tensor::ones_like(&norm)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel create eps ones failed: {}", e),
        })?
        .affine(1e-12, 0.0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel eps scale failed: {}", e),
        })?;

    tensor
        .broadcast_div(
            &norm
                .broadcast_add(&eps_tensor)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("CausalModel norm eps add failed: {}", e),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel normalize div failed: {}", e),
        })
}
