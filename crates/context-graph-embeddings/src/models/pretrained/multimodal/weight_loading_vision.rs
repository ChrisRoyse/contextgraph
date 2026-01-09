//! Vision encoder weight loading for CLIP model.

use candle_nn::VarBuilder;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::config::ClipVisionConfig;
use super::model::MultimodalModel;
use super::weights::{
    ClipVisionAttentionWeights, ClipVisionLayerWeights, ClipVisionMlpWeights, ClipVisionWeights,
};

impl MultimodalModel {
    /// Load vision encoder weights.
    pub(crate) fn load_vision_weights(
        vb: &VarBuilder,
        config: &ClipVisionConfig,
    ) -> EmbeddingResult<ClipVisionWeights> {
        let prefix = "vision_model";
        let h = config.hidden_size;
        let i = config.intermediate_size;
        let p = config.patch_size;
        let num_patches = (config.image_size / p) * (config.image_size / p);

        // Patch embedding (conv2d weight): [hidden_size, 3, patch_size, patch_size]
        let patch_embedding_weight = Self::get_tensor(
            vb,
            &format!("{}.embeddings.patch_embedding.weight", prefix),
            &[h, 3, p, p],
        )?;

        // Class embedding: [hidden_size]
        let class_embedding =
            Self::get_tensor(vb, &format!("{}.embeddings.class_embedding", prefix), &[h])?;

        // Position embeddings: [num_patches+1, hidden_size]
        let position_embedding = Self::get_tensor(
            vb,
            &format!("{}.embeddings.position_embedding.weight", prefix),
            &[num_patches + 1, h],
        )?;

        // Pre-LayerNorm
        let pre_layernorm_weight =
            Self::get_tensor(vb, &format!("{}.pre_layrnorm.weight", prefix), &[h])?;
        let pre_layernorm_bias =
            Self::get_tensor(vb, &format!("{}.pre_layrnorm.bias", prefix), &[h])?;

        // Encoder layers
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for layer_idx in 0..config.num_hidden_layers {
            let layer_prefix = format!("{}.encoder.layers.{}", prefix, layer_idx);
            let layer = Self::load_vision_layer(vb, &layer_prefix, h, i)?;
            layers.push(layer);
        }

        // Post-LayerNorm
        let post_layernorm_weight =
            Self::get_tensor(vb, &format!("{}.post_layernorm.weight", prefix), &[h])?;
        let post_layernorm_bias =
            Self::get_tensor(vb, &format!("{}.post_layernorm.bias", prefix), &[h])?;

        // Visual projection - stored as [projection_dim, hidden_size], transpose for matmul
        let visual_projection_raw =
            Self::get_tensor(vb, "visual_projection.weight", &[config.projection_dim, h])?;
        let visual_projection = visual_projection_raw
            .t()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to transpose visual_projection: {}", e),
            })?
            .contiguous()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to make visual_projection contiguous: {}", e),
            })?;

        Ok(ClipVisionWeights {
            config: config.clone(),
            patch_embedding_weight,
            class_embedding,
            position_embedding,
            pre_layernorm_weight,
            pre_layernorm_bias,
            layers,
            post_layernorm_weight,
            post_layernorm_bias,
            visual_projection,
        })
    }

    /// Load a single vision encoder layer.
    pub(crate) fn load_vision_layer(
        vb: &VarBuilder,
        prefix: &str,
        h: usize,
        i: usize,
    ) -> EmbeddingResult<ClipVisionLayerWeights> {
        // Self-attention
        let attention = ClipVisionAttentionWeights {
            q_proj_weight: Self::get_tensor(
                vb,
                &format!("{}.self_attn.q_proj.weight", prefix),
                &[h, h],
            )?,
            q_proj_bias: Self::get_tensor(vb, &format!("{}.self_attn.q_proj.bias", prefix), &[h])?,
            k_proj_weight: Self::get_tensor(
                vb,
                &format!("{}.self_attn.k_proj.weight", prefix),
                &[h, h],
            )?,
            k_proj_bias: Self::get_tensor(vb, &format!("{}.self_attn.k_proj.bias", prefix), &[h])?,
            v_proj_weight: Self::get_tensor(
                vb,
                &format!("{}.self_attn.v_proj.weight", prefix),
                &[h, h],
            )?,
            v_proj_bias: Self::get_tensor(vb, &format!("{}.self_attn.v_proj.bias", prefix), &[h])?,
            out_proj_weight: Self::get_tensor(
                vb,
                &format!("{}.self_attn.out_proj.weight", prefix),
                &[h, h],
            )?,
            out_proj_bias: Self::get_tensor(
                vb,
                &format!("{}.self_attn.out_proj.bias", prefix),
                &[h],
            )?,
        };

        // MLP
        let mlp = ClipVisionMlpWeights {
            fc1_weight: Self::get_tensor(vb, &format!("{}.mlp.fc1.weight", prefix), &[i, h])?,
            fc1_bias: Self::get_tensor(vb, &format!("{}.mlp.fc1.bias", prefix), &[i])?,
            fc2_weight: Self::get_tensor(vb, &format!("{}.mlp.fc2.weight", prefix), &[h, i])?,
            fc2_bias: Self::get_tensor(vb, &format!("{}.mlp.fc2.bias", prefix), &[h])?,
        };

        // Layer norms
        let layer_norm1_weight =
            Self::get_tensor(vb, &format!("{}.layer_norm1.weight", prefix), &[h])?;
        let layer_norm1_bias = Self::get_tensor(vb, &format!("{}.layer_norm1.bias", prefix), &[h])?;
        let layer_norm2_weight =
            Self::get_tensor(vb, &format!("{}.layer_norm2.weight", prefix), &[h])?;
        let layer_norm2_bias = Self::get_tensor(vb, &format!("{}.layer_norm2.bias", prefix), &[h])?;

        Ok(ClipVisionLayerWeights {
            attention,
            mlp,
            layer_norm1_weight,
            layer_norm1_bias,
            layer_norm2_weight,
            layer_norm2_bias,
        })
    }
}
