//! Qwen2 decoder layer implementation.
//!
//! Contains:
//! - RMSNorm (Qwen2-style layer normalization)
//! - SwiGLU feed-forward network
//! - Decoder layer forward pass

use candle_core::{DType, Tensor};

use crate::error::{EmbeddingError, EmbeddingResult};

use super::attention::gqa_forward;
use super::config::QwenConfig;
use super::position::RopeCache;
use super::weights::{QwenLayerWeights, QwenMlpWeights};

/// Apply RMSNorm (Root Mean Square Layer Normalization).
///
/// RMSNorm normalizes the input by dividing by the RMS of the elements,
/// then multiplies by the learned scale (weight).
///
/// Unlike LayerNorm, RMSNorm does not center the input (no bias subtraction)
/// and uses RMS instead of variance.
///
/// Computes in FP32 to prevent precision loss for large values, then converts back to FP16.
pub fn rms_norm(x: &Tensor, weight: &Tensor, eps: f64) -> EmbeddingResult<Tensor> {
    let original_dtype = x.dtype();

    // Convert to F32 for numerical stability with large values
    let x_f32 = x
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm x to F32 failed: {}", e),
        })?;
    let weight_f32 = weight
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm weight to F32 failed: {}", e),
        })?;

    // Compute RMS: sqrt(mean(x^2))
    let variance = x_f32
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm sqr failed: {}", e),
        })?
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm mean failed: {}", e),
        })?;

    // Add epsilon for numerical stability
    let eps_tensor = Tensor::ones_like(&variance)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm create eps ones failed: {}", e),
        })?
        .affine(eps, 0.0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm eps scale failed: {}", e),
        })?;

    // Normalize: x / sqrt(variance + eps)
    let normalized = x_f32
        .broadcast_div(
            &variance
                .broadcast_add(&eps_tensor)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 RMSNorm eps add failed: {}", e),
                })?
                .sqrt()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 RMSNorm sqrt failed: {}", e),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm div failed: {}", e),
        })?;

    // Scale by learned weight
    let result = normalized
        .broadcast_mul(&weight_f32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm mul failed: {}", e),
        })?;

    // Convert back to original dtype
    result
        .to_dtype(original_dtype)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 RMSNorm to original dtype failed: {}", e),
        })
}

/// Run single decoder layer forward pass.
///
/// Qwen2 layer structure:
/// 1. RMSNorm (input_layernorm)
/// 2. Grouped-Query Attention
/// 3. Residual connection
/// 4. RMSNorm (post_attention_layernorm)
/// 5. SwiGLU FFN
/// 6. Residual connection
pub fn decoder_layer_forward(
    hidden_states: &Tensor,
    layer: &QwenLayerWeights,
    attention_mask: &Tensor,
    rope_cache: &RopeCache,
    config: &QwenConfig,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    // Pre-norm: apply RMSNorm before attention
    let normed = rms_norm(
        hidden_states,
        &layer.input_layernorm_weight,
        config.rms_norm_eps,
    )?;

    // Self-attention with GQA and RoPE
    let attention_output = gqa_forward(
        &normed,
        &layer.attention,
        attention_mask,
        rope_cache,
        config,
        layer_idx,
    )?;

    // Residual connection
    let hidden_states =
        hidden_states
            .add(&attention_output)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} attention residual failed: {}", layer_idx, e),
            })?;

    // Pre-norm: apply RMSNorm before FFN
    let normed = rms_norm(
        &hidden_states,
        &layer.post_attention_layernorm_weight,
        config.rms_norm_eps,
    )?;

    // SwiGLU FFN
    let ffn_output = swiglu_ffn_forward(&normed, &layer.mlp, config, layer_idx)?;

    // Residual connection
    hidden_states
        .add(&ffn_output)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} FFN residual failed: {}", layer_idx, e),
        })
}

/// Run SwiGLU FFN forward pass.
///
/// SwiGLU (Swish-Gated Linear Unit) combines:
/// - Gate: silu(x @ gate_proj)
/// - Up: x @ up_proj
/// - Output: (gate * up) @ down_proj
///
/// SiLU is also known as Swish: silu(x) = x * sigmoid(x)
pub fn swiglu_ffn_forward(
    hidden_states: &Tensor,
    mlp: &QwenMlpWeights,
    _config: &QwenConfig,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 layer {} FFN get dims failed: {}", layer_idx, e),
            })?;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} FFN flatten hidden failed: {}", layer_idx, e),
        })?;

    // Gate projection: [batch*seq, hidden] @ [intermediate, hidden]^T -> [batch*seq, intermediate]
    let gate = hidden_flat
        .matmul(
            &mlp.gate_proj_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "Qwen2 layer {} FFN gate_proj transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "Qwen2 layer {} FFN gate_proj matmul failed: {}",
                layer_idx, e
            ),
        })?;

    // Up projection: [batch*seq, hidden] @ [intermediate, hidden]^T -> [batch*seq, intermediate]
    let up = hidden_flat
        .matmul(
            &mlp.up_proj_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "Qwen2 layer {} FFN up_proj transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} FFN up_proj matmul failed: {}", layer_idx, e),
        })?;

    // SiLU activation on gate: silu(x) = x * sigmoid(x)
    let gate_activated = candle_nn::ops::silu(&gate).map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 layer {} SiLU failed: {}", layer_idx, e),
    })?;

    // Element-wise multiply: gate * up
    let intermediate = gate_activated
        .mul(&up)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} FFN gate*up failed: {}", layer_idx, e),
        })?;

    // Down projection: [batch*seq, intermediate] @ [hidden, intermediate]^T -> [batch*seq, hidden]
    intermediate
        .matmul(
            &mlp.down_proj_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "Qwen2 layer {} FFN down_proj transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "Qwen2 layer {} FFN down_proj matmul failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 layer {} FFN reshape output failed: {}", layer_idx, e),
        })
}
