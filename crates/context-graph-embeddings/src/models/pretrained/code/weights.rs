//! Weight structures for Qwen2 model (Qodo-Embed-1-1.5B).
//!
//! Contains tensor weight definitions for Grouped-Query Attention,
//! SwiGLU feed-forward networks, and Qwen2 decoder layers.

use std::path::Path;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelId;

use super::config::QwenConfig;

/// Qwen2-style self-attention weights with GQA support.
#[derive(Debug)]
pub struct QwenAttentionWeights {
    /// Query projection: [hidden_size, hidden_size]
    pub q_proj_weight: Tensor,
    /// Query projection bias: [hidden_size]
    pub q_proj_bias: Tensor,
    /// Key projection: [hidden_size, num_kv_heads * head_dim]
    pub k_proj_weight: Tensor,
    /// Key projection bias: [num_kv_heads * head_dim]
    pub k_proj_bias: Tensor,
    /// Value projection: [hidden_size, num_kv_heads * head_dim]
    pub v_proj_weight: Tensor,
    /// Value projection bias: [num_kv_heads * head_dim]
    pub v_proj_bias: Tensor,
    /// Output projection: [hidden_size, hidden_size]
    pub o_proj_weight: Tensor,
}

/// Qwen2-style SwiGLU FFN weights.
#[derive(Debug)]
pub struct QwenMlpWeights {
    /// Gate projection: [intermediate_size, hidden_size]
    pub gate_proj_weight: Tensor,
    /// Up projection: [intermediate_size, hidden_size]
    pub up_proj_weight: Tensor,
    /// Down projection: [hidden_size, intermediate_size]
    pub down_proj_weight: Tensor,
}

/// Qwen2-style decoder layer weights.
#[derive(Debug)]
pub struct QwenLayerWeights {
    /// Self-attention weights.
    pub attention: QwenAttentionWeights,
    /// Input layer norm weight (before attention): [hidden_size]
    pub input_layernorm_weight: Tensor,
    /// Post-attention layer norm weight (before FFN): [hidden_size]
    pub post_attention_layernorm_weight: Tensor,
    /// MLP weights.
    pub mlp: QwenMlpWeights,
}

/// Complete Qwen2 weights for Qodo-Embed model.
#[derive(Debug)]
pub struct QwenWeights {
    /// Model configuration.
    pub config: QwenConfig,
    /// Token embeddings: [vocab_size, hidden_size]
    pub embed_tokens: Tensor,
    /// Decoder layers.
    pub layers: Vec<QwenLayerWeights>,
    /// Final RMSNorm weight: [hidden_size]
    pub norm_weight: Tensor,
    /// GPU device reference.
    pub device: &'static Device,
}

impl QwenWeights {
    /// Load Qwen2 weights from sharded safetensors files.
    pub fn from_path(model_path: &Path, device: &'static Device) -> EmbeddingResult<Self> {
        // Check for sharded model files
        let shard1_path = model_path.join("model-00001-of-00002.safetensors");
        let shard2_path = model_path.join("model-00002-of-00002.safetensors");
        let single_path = model_path.join("model.safetensors");

        let safetensor_paths: Vec<std::path::PathBuf> = if shard1_path.exists()
            && shard2_path.exists()
        {
            vec![shard1_path, shard2_path]
        } else if single_path.exists() {
            vec![single_path]
        } else {
            return Err(EmbeddingError::ModelLoadError {
                model_id: ModelId::Code,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "No safetensors found at {}. Expected model-00001-of-00002.safetensors and model-00002-of-00002.safetensors or model.safetensors",
                        model_path.display()
                    ),
                )),
            });
        };

        // Parse config.json for model dimensions
        let config = QwenConfig::from_path(model_path)?;

        tracing::info!(
            "Loading Qwen2 model: {} layers, hidden_size={}, {} attention heads, {} KV heads",
            config.num_hidden_layers,
            config.hidden_size,
            config.num_attention_heads,
            config.num_key_value_heads
        );

        // Load safetensors with FP16 for memory efficiency on modern GPUs
        let safetensor_refs: Vec<&Path> = safetensor_paths.iter().map(|p| p.as_path()).collect();
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&safetensor_refs, DType::F16, device).map_err(
                |e| EmbeddingError::GpuError {
                    message: format!("Qwen2 safetensors load failed: {}", e),
                },
            )?
        };

        // Load token embeddings
        let embed_tokens = vb
            .get(
                (config.vocab_size, config.hidden_size),
                "embed_tokens.weight",
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 embed_tokens.weight load failed: {}", e),
            })?;

        // Load decoder layers
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for layer_idx in 0..config.num_hidden_layers {
            let layer = Self::load_layer(&vb, &config, layer_idx)?;
            layers.push(layer);
        }

        // Load final layer norm
        let norm_weight =
            vb.get((config.hidden_size,), "norm.weight")
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 norm.weight load failed: {}", e),
                })?;

        Ok(QwenWeights {
            config,
            embed_tokens,
            layers,
            norm_weight,
            device,
        })
    }

    /// Load a single decoder layer.
    fn load_layer(
        vb: &VarBuilder,
        config: &QwenConfig,
        layer_idx: usize,
    ) -> EmbeddingResult<QwenLayerWeights> {
        let prefix = format!("layers.{}", layer_idx);
        let kv_dim = config.num_key_value_heads * config.head_dim;

        // Load attention weights
        let attention = QwenAttentionWeights {
            q_proj_weight: vb
                .get(
                    (config.hidden_size, config.hidden_size),
                    &format!("{}.self_attn.q_proj.weight", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} q_proj.weight load failed: {}", layer_idx, e),
                })?,
            q_proj_bias: vb
                .get(
                    (config.hidden_size,),
                    &format!("{}.self_attn.q_proj.bias", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} q_proj.bias load failed: {}", layer_idx, e),
                })?,
            k_proj_weight: vb
                .get(
                    (kv_dim, config.hidden_size),
                    &format!("{}.self_attn.k_proj.weight", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} k_proj.weight load failed: {}", layer_idx, e),
                })?,
            k_proj_bias: vb
                .get((kv_dim,), &format!("{}.self_attn.k_proj.bias", prefix))
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} k_proj.bias load failed: {}", layer_idx, e),
                })?,
            v_proj_weight: vb
                .get(
                    (kv_dim, config.hidden_size),
                    &format!("{}.self_attn.v_proj.weight", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} v_proj.weight load failed: {}", layer_idx, e),
                })?,
            v_proj_bias: vb
                .get((kv_dim,), &format!("{}.self_attn.v_proj.bias", prefix))
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} v_proj.bias load failed: {}", layer_idx, e),
                })?,
            o_proj_weight: vb
                .get(
                    (config.hidden_size, config.hidden_size),
                    &format!("{}.self_attn.o_proj.weight", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 layer {} o_proj.weight load failed: {}", layer_idx, e),
                })?,
        };

        // Load layer norms
        let input_layernorm_weight = vb
            .get(
                (config.hidden_size,),
                &format!("{}.input_layernorm.weight", prefix),
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "Qwen2 layer {} input_layernorm.weight load failed: {}",
                    layer_idx, e
                ),
            })?;

        let post_attention_layernorm_weight = vb
            .get(
                (config.hidden_size,),
                &format!("{}.post_attention_layernorm.weight", prefix),
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "Qwen2 layer {} post_attention_layernorm.weight load failed: {}",
                    layer_idx, e
                ),
            })?;

        // Load MLP weights
        let mlp = QwenMlpWeights {
            gate_proj_weight: vb
                .get(
                    (config.intermediate_size, config.hidden_size),
                    &format!("{}.mlp.gate_proj.weight", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "Qwen2 layer {} mlp.gate_proj.weight load failed: {}",
                        layer_idx, e
                    ),
                })?,
            up_proj_weight: vb
                .get(
                    (config.intermediate_size, config.hidden_size),
                    &format!("{}.mlp.up_proj.weight", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "Qwen2 layer {} mlp.up_proj.weight load failed: {}",
                        layer_idx, e
                    ),
                })?,
            down_proj_weight: vb
                .get(
                    (config.hidden_size, config.intermediate_size),
                    &format!("{}.mlp.down_proj.weight", prefix),
                )
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "Qwen2 layer {} mlp.down_proj.weight load failed: {}",
                        layer_idx, e
                    ),
                })?,
        };

        Ok(QwenLayerWeights {
            attention,
            input_layernorm_weight,
            post_attention_layernorm_weight,
            mlp,
        })
    }
}
