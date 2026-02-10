//! Encoder layer and SwiGLU FFN forward pass for NomicBERT model.
//!
//! Key differences from standard BERT:
//! - SwiGLU FFN: output = fc2(SiLU(fc11(x)) * fc12(x)) instead of GELU(fc1(x)) * fc2(x)
//! - No biases in FFN projections
//! - Rotary position embeddings applied in attention (not here)

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::super::weights::{NomicEncoderLayerWeights, NomicFfnWeights, NomicWeights};
use super::attention::{compute_rotary_freqs, self_attention_forward, self_attention_forward_with_lora};
use super::ops::layer_norm;

/// Run all encoder layers.
pub fn run_encoder(
    embeddings: Tensor,
    attention_mask_tensor: &Tensor,
    weights: &NomicWeights,
) -> EmbeddingResult<Tensor> {
    let config = &weights.config;
    let seq_len = embeddings.dim(1).map_err(|e| EmbeddingError::GpuError {
        message: format!("CausalModel get seq_len failed: {}", e),
    })?;

    let head_dim = config.hidden_size / config.num_attention_heads;

    // Precompute rotary freqs once for all layers
    let (cos, sin) = compute_rotary_freqs(
        seq_len,
        head_dim,
        config.rotary_emb_base,
        config.rotary_emb_fraction,
        weights.device,
    )?;

    // Create extended attention mask: [batch, 1, 1, seq] with -10000 for padding
    let extended_attention_mask = attention_mask_tensor
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel attention mask unsqueeze 1 failed: {}", e),
        })?
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel attention mask unsqueeze 2 failed: {}", e),
        })?;

    let ones =
        Tensor::ones_like(&extended_attention_mask).map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel create ones tensor failed: {}", e),
        })?;

    let inverted_mask =
        ones.broadcast_sub(&extended_attention_mask)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("CausalModel attention mask invert failed: {}", e),
            })?;

    let extended_attention_mask =
        (inverted_mask * (-10000.0f64)).map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel attention mask scale failed: {}", e),
        })?;

    let mut hidden_states = embeddings;

    for (layer_idx, layer) in weights.encoder_layers.iter().enumerate() {
        hidden_states = encoder_layer_forward(
            &hidden_states,
            layer,
            &extended_attention_mask,
            config,
            layer_idx,
            &cos,
            &sin,
        )?;
    }

    Ok(hidden_states)
}

/// Run single encoder layer: attention + SwiGLU FFN with post-norm.
fn encoder_layer_forward(
    hidden_states: &Tensor,
    layer: &NomicEncoderLayerWeights,
    attention_mask: &Tensor,
    config: &super::super::config::NomicConfig,
    layer_idx: usize,
    cos: &Tensor,
    sin: &Tensor,
) -> EmbeddingResult<Tensor> {
    // Self-attention with RoPE
    let attention_output = self_attention_forward(
        hidden_states,
        &layer.attention,
        attention_mask,
        config,
        layer_idx,
        cos,
        sin,
    )?;

    // Add & Norm (post-norm: residual + attention, then norm1)
    let attention_output =
        hidden_states
            .add(&attention_output)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "CausalModel layer {} attention residual failed: {}",
                    layer_idx, e
                ),
            })?;

    let attention_output = layer_norm(
        &attention_output,
        &layer.attention.norm1_weight,
        &layer.attention.norm1_bias,
        config.layer_norm_eps,
    )?;

    // SwiGLU FFN
    let ffn_output = ffn_forward(&attention_output, &layer.ffn, layer_idx)?;

    // Add & Norm (post-norm: residual + FFN, then norm2)
    let output = attention_output
        .add(&ffn_output)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} FFN residual failed: {}", layer_idx, e),
        })?;

    layer_norm(
        &output,
        &layer.ffn.norm2_weight,
        &layer.ffn.norm2_bias,
        config.layer_norm_eps,
    )
}

/// Run all encoder layers with LoRA adapters.
///
/// Same as `run_encoder()` but applies LoRA deltas to Q, V attention projections
/// in each layer. Used during training — inference uses `run_encoder()`.
pub fn run_encoder_with_lora(
    embeddings: Tensor,
    attention_mask_tensor: &Tensor,
    weights: &NomicWeights,
    lora_layers: &crate::training::lora::LoraLayers,
) -> EmbeddingResult<Tensor> {
    let config = &weights.config;
    let seq_len = embeddings.dim(1).map_err(|e| EmbeddingError::GpuError {
        message: format!("CausalModel get seq_len failed: {}", e),
    })?;

    let head_dim = config.hidden_size / config.num_attention_heads;

    let (cos, sin) = compute_rotary_freqs(
        seq_len,
        head_dim,
        config.rotary_emb_base,
        config.rotary_emb_fraction,
        weights.device,
    )?;

    let extended_attention_mask = attention_mask_tensor
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel attention mask unsqueeze 1 failed: {}", e),
        })?
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel attention mask unsqueeze 2 failed: {}", e),
        })?;

    let ones =
        Tensor::ones_like(&extended_attention_mask).map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel create ones tensor failed: {}", e),
        })?;

    let inverted_mask =
        ones.broadcast_sub(&extended_attention_mask)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("CausalModel attention mask invert failed: {}", e),
            })?;

    let extended_attention_mask =
        (inverted_mask * (-10000.0f64)).map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel attention mask scale failed: {}", e),
        })?;

    let mut hidden_states = embeddings;

    for (layer_idx, layer) in weights.encoder_layers.iter().enumerate() {
        hidden_states = encoder_layer_forward_with_lora(
            &hidden_states,
            layer,
            &extended_attention_mask,
            config,
            layer_idx,
            &cos,
            &sin,
            lora_layers,
        )?;
    }

    Ok(hidden_states)
}

/// Run single encoder layer with LoRA: attention + SwiGLU FFN with post-norm.
fn encoder_layer_forward_with_lora(
    hidden_states: &Tensor,
    layer: &NomicEncoderLayerWeights,
    attention_mask: &Tensor,
    config: &super::super::config::NomicConfig,
    layer_idx: usize,
    cos: &Tensor,
    sin: &Tensor,
    lora_layers: &crate::training::lora::LoraLayers,
) -> EmbeddingResult<Tensor> {
    let attention_output = self_attention_forward_with_lora(
        hidden_states,
        &layer.attention,
        attention_mask,
        config,
        layer_idx,
        cos,
        sin,
        lora_layers,
    )?;

    let attention_output =
        hidden_states
            .add(&attention_output)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "CausalModel layer {} attention residual failed: {}",
                    layer_idx, e
                ),
            })?;

    let attention_output = layer_norm(
        &attention_output,
        &layer.attention.norm1_weight,
        &layer.attention.norm1_bias,
        config.layer_norm_eps,
    )?;

    let ffn_output = ffn_forward(&attention_output, &layer.ffn, layer_idx)?;

    let output = attention_output
        .add(&ffn_output)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} FFN residual failed: {}", layer_idx, e),
        })?;

    layer_norm(
        &output,
        &layer.ffn.norm2_weight,
        &layer.ffn.norm2_bias,
        config.layer_norm_eps,
    )
}

/// SwiGLU FFN forward pass.
///
/// output = fc2(SiLU(fc11(x)) * fc12(x))
/// where SiLU(x) = x * sigmoid(x)
fn ffn_forward(
    hidden_states: &Tensor,
    ffn: &NomicFfnWeights,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("CausalModel layer {} FFN get dims failed: {}", layer_idx, e),
            })?;

    let intermediate_size =
        ffn.fc11_weight
            .dim(0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "CausalModel layer {} FFN get intermediate_size failed: {}",
                    layer_idx, e
                ),
            })?;

    // Flatten: [batch*seq, hidden]
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN flatten failed: {}",
                layer_idx, e
            ),
        })?;

    // Gate projection: fc11(x) — no bias
    let gate = hidden_flat
        .matmul(
            &ffn.fc11_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "CausalModel layer {} FFN fc11 transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN fc11 matmul failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, intermediate_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN fc11 reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // Up projection: fc12(x) — no bias
    let up = hidden_flat
        .matmul(
            &ffn.fc12_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "CausalModel layer {} FFN fc12 transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN fc12 matmul failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, intermediate_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN fc12 reshape failed: {}",
                layer_idx, e
            ),
        })?;

    // SiLU(gate) * up
    let activated = gate
        .silu()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel layer {} SiLU failed: {}", layer_idx, e),
        })?
        .mul(&up)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} SwiGLU mul failed: {}",
                layer_idx, e
            ),
        })?;

    // Down projection: fc2(activated) — no bias
    let activated_flat = activated
        .reshape((batch_size * seq_len, intermediate_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN fc2 flatten failed: {}",
                layer_idx, e
            ),
        })?;

    activated_flat
        .matmul(
            &ffn.fc2_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "CausalModel layer {} FFN fc2 transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN fc2 matmul failed: {}",
                layer_idx, e
            ),
        })?
        .reshape((batch_size, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} FFN fc2 reshape failed: {}",
                layer_idx, e
            ),
        })
}
