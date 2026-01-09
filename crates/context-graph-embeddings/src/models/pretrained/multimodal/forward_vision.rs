//! GPU-accelerated vision encoder forward pass for CLIP.
//!
//! Implements the CLIP vision encoder pipeline:
//! 1. Apply patch embedding (14x14 conv2d stride 14) -> 256 patch tokens
//! 2. Prepend [CLS] token, add position embeddings (257 total)
//! 3. Apply pre-LayerNorm
//! 4. Apply 24 transformer layers
//! 5. Extract [CLS] token embedding
//! 6. Apply post-LayerNorm and visual projection
//! 7. L2 normalize to unit sphere

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::normalize_gpu;

use super::config::ClipVisionConfig;
use super::forward_ops::{conv2d_patch_embed, layer_norm, layer_norm_1d, mlp, self_attention};
use super::weights::{ClipVisionLayerWeights, ClipWeights};

/// GPU-accelerated vision encoder forward pass.
///
/// # Pipeline
///
/// 1. Apply patch embedding (14x14 conv2d stride 14) -> 256 patch tokens
/// 2. Prepend [CLS] token, add position embeddings (257 total)
/// 3. Apply pre-LayerNorm
/// 4. Apply 24 transformer layers
/// 5. Extract [CLS] token embedding
/// 6. Apply post-LayerNorm and visual projection
/// 7. L2 normalize to unit sphere
///
/// # Arguments
///
/// * `image_tensor` - Preprocessed image tensor [3, 224, 224] as flat Vec<f32>
/// * `weights` - CLIP model weights on GPU
///
/// # Returns
///
/// 768D normalized embedding vector.
pub fn vision_forward(image_tensor: &[f32], weights: &ClipWeights) -> EmbeddingResult<Vec<f32>> {
    let device = weights.device();
    let config = &weights.vision.config;
    let h = config.hidden_size;
    let p = config.patch_size;
    let img_size = config.image_size;
    let num_patches = (img_size / p) * (img_size / p); // 256

    // Create image tensor: [1, 3, 224, 224]
    let image =
        Tensor::from_slice(image_tensor, (1, 3, img_size, img_size), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Image tensor creation failed: {}", e),
            }
        })?;

    // Patch embedding via 2D convolution
    // Conv2D: [hidden_size, 3, patch_size, patch_size], stride=patch_size
    // Output: [1, hidden_size, num_patches_h, num_patches_w]
    let patch_emb = conv2d_patch_embed(&image, &weights.vision.patch_embedding_weight, p)?;

    // Flatten patches: [1, hidden_size, 16, 16] -> [1, num_patches, hidden_size]
    let patch_emb = patch_emb
        .flatten(2, 3)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Patch flatten failed: {}", e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Patch transpose failed: {}", e),
        })?;

    // Prepend class token: [1, 1, hidden_size]
    let class_token = weights
        .vision
        .class_embedding
        .reshape((1, 1, h))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Class token reshape failed: {}", e),
        })?;

    // Concatenate: [1, num_patches+1, hidden_size]
    let hidden_states =
        Tensor::cat(&[&class_token, &patch_emb], 1).map_err(|e| EmbeddingError::GpuError {
            message: format!("Class token concat failed: {}", e),
        })?;

    // Add position embeddings
    let position_emb = weights
        .vision
        .position_embedding
        .reshape((1, num_patches + 1, h))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Position embedding reshape failed: {}", e),
        })?;

    let mut hidden_states =
        (hidden_states + position_emb).map_err(|e| EmbeddingError::GpuError {
            message: format!("Position embedding addition failed: {}", e),
        })?;

    // Pre-LayerNorm
    hidden_states = layer_norm(
        &hidden_states,
        &weights.vision.pre_layernorm_weight,
        &weights.vision.pre_layernorm_bias,
        config.layer_norm_eps,
    )?;

    // Apply transformer layers (bidirectional attention - no mask needed)
    for layer in &weights.vision.layers {
        hidden_states = vision_transformer_layer(&hidden_states, layer, config)?;
    }

    // Extract [CLS] token: position 0
    let cls_hidden = hidden_states
        .narrow(1, 0, 1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CLS extraction failed: {}", e),
        })?
        .squeeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CLS squeeze failed: {}", e),
        })?;

    // Post-LayerNorm
    let cls_normed = layer_norm_1d(
        &cls_hidden,
        &weights.vision.post_layernorm_weight,
        &weights.vision.post_layernorm_bias,
        config.layer_norm_eps,
    )?;

    // Visual projection: [1, projection_dim]
    let projected = cls_normed
        .matmul(&weights.vision.visual_projection)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Visual projection failed: {}", e),
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

/// Apply a single vision transformer layer (bidirectional attention).
fn vision_transformer_layer(
    hidden_states: &Tensor,
    layer: &ClipVisionLayerWeights,
    config: &ClipVisionConfig,
) -> EmbeddingResult<Tensor> {
    // Pre-norm for attention
    let normed = layer_norm(
        hidden_states,
        &layer.layer_norm1_weight,
        &layer.layer_norm1_bias,
        config.layer_norm_eps,
    )?;

    // Self-attention (no mask - bidirectional)
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
        None,
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
