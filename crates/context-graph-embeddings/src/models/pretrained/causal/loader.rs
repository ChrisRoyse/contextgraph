//! Weight loading functions for NomicBERT (nomic-embed-text-v1.5).
//!
//! This module handles loading model weights from safetensors files
//! and parsing configuration from config.json.
//!
//! Tensor name format (112 tensors total):
//!   - embeddings.word_embeddings.weight [30528, 768]
//!   - embeddings.token_type_embeddings.weight [2, 768]
//!   - emb_ln.weight [768], emb_ln.bias [768]
//!   - encoder.layers.{i}.attn.Wqkv.weight [2304, 768]
//!   - encoder.layers.{i}.attn.out_proj.weight [768, 768]
//!   - encoder.layers.{i}.norm1.weight [768], norm1.bias [768]
//!   - encoder.layers.{i}.norm2.weight [768], norm2.bias [768]
//!   - encoder.layers.{i}.mlp.fc11.weight [3072, 768]
//!   - encoder.layers.{i}.mlp.fc12.weight [3072, 768]
//!   - encoder.layers.{i}.mlp.fc2.weight [768, 3072]

use std::path::Path;

use candle_core::{DType, Device};
use candle_nn::VarBuilder;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelId;

use super::config::NomicConfig;
use super::weights::{
    NomicAttentionWeights, NomicEmbeddingWeights, NomicEncoderLayerWeights, NomicFfnWeights,
    NomicWeights,
};

/// Load NomicBERT weights from safetensors file.
pub fn load_nomic_weights(
    model_path: &Path,
    device: &'static Device,
) -> EmbeddingResult<NomicWeights> {
    let safetensors_path = model_path.join("model.safetensors");
    if !safetensors_path.exists() {
        return Err(EmbeddingError::ModelLoadError {
            model_id: ModelId::Causal,
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "CausalModel model.safetensors not found at {}",
                    safetensors_path.display()
                ),
            )),
        });
    }

    let config = load_config(model_path)?;

    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[&safetensors_path], DType::F32, device).map_err(
            |e| EmbeddingError::GpuError {
                message: format!(
                    "CausalModel (nomic-embed) safetensors load failed at {}: {}",
                    safetensors_path.display(),
                    e
                ),
            },
        )?
    };

    let embeddings = load_embeddings(&vb, &config)?;

    let mut encoder_layers = Vec::with_capacity(config.num_hidden_layers);
    for layer_idx in 0..config.num_hidden_layers {
        let layer = load_encoder_layer(&vb, &config, layer_idx)?;
        encoder_layers.push(layer);
    }

    tracing::info!(
        "CausalModel loaded: nomic-embed-text-v1.5, {} layers, hidden_size={}, rotary_base={}, SwiGLU FFN",
        config.num_hidden_layers,
        config.hidden_size,
        config.rotary_emb_base
    );

    Ok(NomicWeights {
        config,
        embeddings,
        encoder_layers,
        device,
    })
}

/// Load NomicBERT config from config.json.
pub fn load_config(model_path: &Path) -> EmbeddingResult<NomicConfig> {
    let config_path = model_path.join("config.json");
    let config_content =
        std::fs::read_to_string(&config_path).map_err(|e| EmbeddingError::ModelLoadError {
            model_id: ModelId::Causal,
            source: Box::new(e),
        })?;

    #[derive(serde::Deserialize)]
    struct RawConfig {
        #[serde(default = "default_vocab")]
        vocab_size: usize,
        #[serde(alias = "n_embd")]
        hidden_size: Option<usize>,
        #[serde(alias = "n_layer")]
        num_hidden_layers: Option<usize>,
        #[serde(alias = "n_head")]
        num_attention_heads: Option<usize>,
        #[serde(alias = "n_inner")]
        intermediate_size: Option<usize>,
        #[serde(alias = "n_positions")]
        max_position_embeddings: Option<usize>,
        #[serde(default = "default_type_vocab")]
        type_vocab_size: usize,
        #[serde(default = "default_layer_norm_eps")]
        layer_norm_epsilon: f64,
        #[serde(default = "default_rotary_base")]
        rotary_emb_base: f64,
        #[serde(default = "default_rotary_fraction")]
        rotary_emb_fraction: f64,
        #[serde(default)]
        rotary_emb_interleaved: bool,
        #[serde(default)]
        qkv_proj_bias: bool,
        #[serde(default)]
        mlp_fc1_bias: bool,
        #[serde(default)]
        mlp_fc2_bias: bool,
    }

    fn default_vocab() -> usize {
        30528
    }
    fn default_type_vocab() -> usize {
        2
    }
    fn default_layer_norm_eps() -> f64 {
        1e-12
    }
    fn default_rotary_base() -> f64 {
        1000.0
    }
    fn default_rotary_fraction() -> f64 {
        1.0
    }

    let raw: RawConfig =
        serde_json::from_str(&config_content).map_err(|e| EmbeddingError::ConfigError {
            message: format!(
                "CausalModel (nomic-embed) config parse failed at {}: {}",
                config_path.display(),
                e
            ),
        })?;

    let hidden_size = raw.hidden_size.unwrap_or(768);
    let num_hidden_layers = raw.num_hidden_layers.unwrap_or(12);
    let num_attention_heads = raw.num_attention_heads.unwrap_or(12);
    let intermediate_size = raw.intermediate_size.unwrap_or(3072);

    if hidden_size != 768 {
        return Err(EmbeddingError::ConfigError {
            message: format!(
                "CausalModel (nomic-embed) expected hidden_size=768 for E5 (768D), got {}. \
                 Model dimension must match CAUSAL_DIMENSION.",
                hidden_size
            ),
        });
    }

    // Reject interleaved RoPE — our forward pass implements non-interleaved half-split only
    if raw.rotary_emb_interleaved {
        return Err(EmbeddingError::ConfigError {
            message: "CausalModel (nomic-embed) rotary_emb_interleaved=true is not supported. \
                      Only non-interleaved (half-split) RoPE is implemented."
                .to_string(),
        });
    }

    Ok(NomicConfig {
        vocab_size: raw.vocab_size,
        hidden_size,
        num_hidden_layers,
        num_attention_heads,
        intermediate_size,
        max_position_embeddings: raw.max_position_embeddings.unwrap_or(8192),
        type_vocab_size: raw.type_vocab_size,
        layer_norm_eps: raw.layer_norm_epsilon,
        rotary_emb_base: raw.rotary_emb_base,
        rotary_emb_fraction: raw.rotary_emb_fraction,
        rotary_emb_interleaved: raw.rotary_emb_interleaved,
        qkv_proj_bias: raw.qkv_proj_bias,
        mlp_fc1_bias: raw.mlp_fc1_bias,
        mlp_fc2_bias: raw.mlp_fc2_bias,
    })
}

/// Load embedding layer weights (word + token_type + LayerNorm).
fn load_embeddings(
    vb: &VarBuilder,
    config: &NomicConfig,
) -> EmbeddingResult<NomicEmbeddingWeights> {
    let word_embeddings = vb
        .get(
            (config.vocab_size, config.hidden_size),
            "embeddings.word_embeddings.weight",
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel word_embeddings load failed: {}", e),
        })?;

    let token_type_embeddings = vb
        .get(
            (config.type_vocab_size, config.hidden_size),
            "embeddings.token_type_embeddings.weight",
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel token_type_embeddings load failed: {}", e),
        })?;

    let layer_norm_weight = vb
        .get((config.hidden_size,), "emb_ln.weight")
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel emb_ln weight load failed: {}", e),
        })?;

    let layer_norm_bias =
        vb.get((config.hidden_size,), "emb_ln.bias")
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("CausalModel emb_ln bias load failed: {}", e),
            })?;

    Ok(NomicEmbeddingWeights {
        word_embeddings,
        token_type_embeddings,
        layer_norm_weight,
        layer_norm_bias,
    })
}

/// Load a single encoder layer (attention + FFN).
fn load_encoder_layer(
    vb: &VarBuilder,
    config: &NomicConfig,
    layer_idx: usize,
) -> EmbeddingResult<NomicEncoderLayerWeights> {
    let prefix = format!("encoder.layers.{}", layer_idx);

    // Attention: fused QKV + output projection
    let wqkv_weight = vb
        .get(
            (3 * config.hidden_size, config.hidden_size),
            &format!("{}.attn.Wqkv.weight", prefix),
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} Wqkv weight load failed: {}",
                layer_idx, e
            ),
        })?;

    let out_proj_weight = vb
        .get(
            (config.hidden_size, config.hidden_size),
            &format!("{}.attn.out_proj.weight", prefix),
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} out_proj weight load failed: {}",
                layer_idx, e
            ),
        })?;

    let norm1_weight = vb
        .get((config.hidden_size,), &format!("{}.norm1.weight", prefix))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} norm1 weight load failed: {}",
                layer_idx, e
            ),
        })?;

    let norm1_bias = vb
        .get((config.hidden_size,), &format!("{}.norm1.bias", prefix))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} norm1 bias load failed: {}",
                layer_idx, e
            ),
        })?;

    // FFN: SwiGLU (fc11 gate, fc12 up, fc2 down)
    let fc11_weight = vb
        .get(
            (config.intermediate_size, config.hidden_size),
            &format!("{}.mlp.fc11.weight", prefix),
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} fc11 weight load failed: {}",
                layer_idx, e
            ),
        })?;

    let fc12_weight = vb
        .get(
            (config.intermediate_size, config.hidden_size),
            &format!("{}.mlp.fc12.weight", prefix),
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} fc12 weight load failed: {}",
                layer_idx, e
            ),
        })?;

    let fc2_weight = vb
        .get(
            (config.hidden_size, config.intermediate_size),
            &format!("{}.mlp.fc2.weight", prefix),
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} fc2 weight load failed: {}",
                layer_idx, e
            ),
        })?;

    let norm2_weight = vb
        .get((config.hidden_size,), &format!("{}.norm2.weight", prefix))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} norm2 weight load failed: {}",
                layer_idx, e
            ),
        })?;

    let norm2_bias = vb
        .get((config.hidden_size,), &format!("{}.norm2.bias", prefix))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "CausalModel layer {} norm2 bias load failed: {}",
                layer_idx, e
            ),
        })?;

    Ok(NomicEncoderLayerWeights {
        attention: NomicAttentionWeights {
            wqkv_weight,
            out_proj_weight,
            norm1_weight,
            norm1_bias,
        },
        ffn: NomicFfnWeights {
            fc11_weight,
            fc12_weight,
            fc2_weight,
            norm2_weight,
            norm2_bias,
        },
    })
}
