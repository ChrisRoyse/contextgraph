//! Text encoder weight loading for CLIP model.

use candle_nn::VarBuilder;

use crate::error::EmbeddingResult;

use super::config::ClipTextConfig;
use super::model::MultimodalModel;
use super::weights::{
    ClipTextAttentionWeights, ClipTextLayerWeights, ClipTextMlpWeights, ClipTextWeights,
};

impl MultimodalModel {
    /// Load text encoder weights.
    pub(crate) fn load_text_weights(
        vb: &VarBuilder,
        config: &ClipTextConfig,
    ) -> EmbeddingResult<ClipTextWeights> {
        let prefix = "text_model";
        let h = config.hidden_size;
        let i = config.intermediate_size;

        // Embeddings
        let token_embedding = Self::get_tensor(
            vb,
            &format!("{}.embeddings.token_embedding.weight", prefix),
            &[config.vocab_size, h],
        )?;
        let position_embedding = Self::get_tensor(
            vb,
            &format!("{}.embeddings.position_embedding.weight", prefix),
            &[config.max_position_embeddings, h],
        )?;

        // Encoder layers
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for layer_idx in 0..config.num_hidden_layers {
            let layer_prefix = format!("{}.encoder.layers.{}", prefix, layer_idx);
            let layer = Self::load_text_layer(vb, &layer_prefix, h, i)?;
            layers.push(layer);
        }

        // Final layer norm
        let final_layer_norm_weight =
            Self::get_tensor(vb, &format!("{}.final_layer_norm.weight", prefix), &[h])?;
        let final_layer_norm_bias =
            Self::get_tensor(vb, &format!("{}.final_layer_norm.bias", prefix), &[h])?;

        // Text projection
        let text_projection =
            Self::get_tensor(vb, "text_projection.weight", &[h, config.projection_dim])?;

        Ok(ClipTextWeights {
            config: config.clone(),
            token_embedding,
            position_embedding,
            layers,
            final_layer_norm_weight,
            final_layer_norm_bias,
            text_projection,
        })
    }

    /// Load a single text encoder layer.
    pub(crate) fn load_text_layer(
        vb: &VarBuilder,
        prefix: &str,
        h: usize,
        i: usize,
    ) -> EmbeddingResult<ClipTextLayerWeights> {
        // Self-attention
        let attention = ClipTextAttentionWeights {
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
        let mlp = ClipTextMlpWeights {
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

        Ok(ClipTextLayerWeights {
            attention,
            mlp,
            layer_norm1_weight,
            layer_norm1_bias,
            layer_norm2_weight,
            layer_norm2_bias,
        })
    }
}
