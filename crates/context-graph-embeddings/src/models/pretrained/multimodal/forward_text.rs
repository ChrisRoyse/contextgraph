//! GPU-accelerated text encoder forward pass for CLIP.
//!
//! Implements the CLIP text encoder pipeline:
//! 1. Tokenize text with CLIP BPE tokenizer (pad/truncate to 77 tokens)
//! 2. Lookup word embeddings + add position embeddings
//! 3. Apply 12 transformer layers with causal attention mask
//! 4. Extract [EOS] token embedding (last non-padding position)
//! 5. Apply final layer norm and text projection
//! 6. L2 normalize to unit sphere

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::normalize_gpu;
use crate::types::ModelId;

use super::config::ClipTextConfig;
use super::forward_ops::{create_causal_mask, layer_norm, mlp, self_attention};
use super::weights::{ClipTextLayerWeights, ClipWeights};

/// GPU-accelerated text encoder forward pass.
///
/// # Pipeline
///
/// 1. Tokenize text with CLIP BPE tokenizer (pad/truncate to 77 tokens)
/// 2. Lookup word embeddings + add position embeddings
/// 3. Apply 12 transformer layers with causal attention mask
/// 4. Extract [EOS] token embedding (last non-padding position)
/// 5. Apply final layer norm and text projection
/// 6. L2 normalize to unit sphere
///
/// # Arguments
///
/// * `text` - Input text string
/// * `weights` - CLIP model weights on GPU
/// * `tokenizer` - HuggingFace tokenizer for CLIP
///
/// # Returns
///
/// 768D normalized embedding vector.
pub fn text_forward(
    text: &str,
    weights: &ClipWeights,
    tokenizer: &Tokenizer,
) -> EmbeddingResult<Vec<f32>> {
    let device = weights.device();
    let config = &weights.text.config;
    let seq_len = config.max_position_embeddings;

    // Tokenize (pad to max length)
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::Multimodal,
            message: format!("CLIP tokenization failed: {}", e),
        })?;

    let mut input_ids: Vec<u32> = encoding.get_ids().to_vec();

    // Truncate if too long
    if input_ids.len() > seq_len {
        input_ids.truncate(seq_len);
    }

    // Find EOS position (last real token) for embedding extraction
    let eos_position = input_ids.len().saturating_sub(1);

    // Pad to max length
    while input_ids.len() < seq_len {
        input_ids.push(0); // Pad token
    }

    // Create input tensor: [1, seq_len]
    let input_ids_i64: Vec<i64> = input_ids.iter().map(|&x| x as i64).collect();
    let input_tensor = Tensor::from_slice(&input_ids_i64, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("Failed to create input tensor: {}", e),
        }
    })?;

    // Token embeddings: [1, seq_len, hidden_size]
    let token_emb = weights
        .text
        .token_embedding
        .index_select(
            &input_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Flatten failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Token embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Reshape failed: {}", e),
        })?;

    // Position embeddings: [1, seq_len, hidden_size]
    let positions: Vec<i64> = (0..seq_len as i64).collect();
    let position_ids = Tensor::from_slice(&positions, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("Position IDs creation failed: {}", e),
        }
    })?;

    let position_emb = weights
        .text
        .position_embedding
        .index_select(
            &position_ids
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Flatten failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Position embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Position reshape failed: {}", e),
        })?;

    // Combine embeddings: hidden_states = token_emb + position_emb
    let mut hidden_states = (token_emb + position_emb).map_err(|e| EmbeddingError::GpuError {
        message: format!("Embedding addition failed: {}", e),
    })?;

    // Create causal attention mask for CLIP text encoder
    // Shape: [1, 1, seq_len, seq_len], lower triangular
    let causal_mask = create_causal_mask(seq_len, device)?;

    // Apply transformer layers
    for layer in &weights.text.layers {
        hidden_states = text_transformer_layer(&hidden_states, layer, config, &causal_mask)?;
    }

    // Final layer norm
    hidden_states = layer_norm(
        &hidden_states,
        &weights.text.final_layer_norm_weight,
        &weights.text.final_layer_norm_bias,
        config.layer_norm_eps,
    )?;

    // Extract [EOS] token embedding: [1, hidden_size]
    let eos_hidden = hidden_states
        .narrow(1, eos_position, 1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("EOS extraction failed: {}", e),
        })?
        .squeeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Squeeze failed: {}", e),
        })?;

    // Text projection: [1, projection_dim]
    let projected = eos_hidden
        .matmul(&weights.text.text_projection)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Text projection failed: {}", e),
        })?;

    // L2 normalize
    let normalized = normalize_gpu(&projected).map_err(|e| EmbeddingError::GpuError {
        message: format!("L2 normalization failed: {}", e),
    })?;

    // Convert to Vec<f32>
    let result: Vec<f32> = normalized
        .squeeze(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Final squeeze failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Tensor to vec failed: {}", e),
        })?;

    Ok(result)
}

/// Apply a single text transformer layer.
fn text_transformer_layer(
    hidden_states: &Tensor,
    layer: &ClipTextLayerWeights,
    config: &ClipTextConfig,
    causal_mask: &Tensor,
) -> EmbeddingResult<Tensor> {
    // Pre-norm for attention
    let normed = layer_norm(
        hidden_states,
        &layer.layer_norm1_weight,
        &layer.layer_norm1_bias,
        config.layer_norm_eps,
    )?;

    // Self-attention with causal mask
    let attn_output = self_attention(
        &normed,
        &layer.attention.q_proj_weight,
        &layer.attention.q_proj_bias,
        &layer.attention.k_proj_weight,
        &layer.attention.k_proj_bias,
        &layer.attention.v_proj_weight,
        &layer.attention.v_proj_bias,
        &layer.attention.out_proj_weight,
        &layer.attention.out_proj_bias,
        config.num_attention_heads,
        Some(causal_mask),
    )?;

    // Residual connection
    let hidden_states = hidden_states
        .add(&attn_output)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Attention residual failed: {}", e),
        })?;

    // Pre-norm for MLP
    let normed = layer_norm(
        &hidden_states,
        &layer.layer_norm2_weight,
        &layer.layer_norm2_bias,
        config.layer_norm_eps,
    )?;

    // MLP: FC1 -> GELU -> FC2
    let mlp_output = mlp(
        &normed,
        &layer.mlp.fc1_weight,
        &layer.mlp.fc1_bias,
        &layer.mlp.fc2_weight,
        &layer.mlp.fc2_bias,
    )?;

    // Residual connection
    let output = (hidden_states + mlp_output).map_err(|e| EmbeddingError::GpuError {
        message: format!("MLP residual failed: {}", e),
    })?;

    Ok(output)
}
