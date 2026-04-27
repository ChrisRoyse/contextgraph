//! NomicBERT weight structures for the causal embedding model (E5).
//!
//! This module contains tensor weight structures for the nomic-embed-text-v1.5
//! model: embeddings, fused QKV attention, SwiGLU FFN, and layer norms.
//!
//! # Causal Projection Weights
//!
//! The `CausalProjectionWeights` struct provides learned projection matrices
//! for creating asymmetric cause/effect embeddings during fine-tuning.
//! For the base nomic model, asymmetry comes from instruction prefixes instead.

use candle_core::{DType, Device, Tensor};
use rand::Rng;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::config::NomicConfig;

/// Seed for causal projection initialization (deterministic).
pub const CAUSAL_PROJECTION_SEED: u64 = 0xCA05A1;

/// NomicBERT embedding weights.
///
/// No position embeddings — nomic uses rotary PE applied in the attention layer.
#[derive(Debug)]
pub struct NomicEmbeddingWeights {
    /// Word embeddings: [vocab_size, hidden_size]
    pub word_embeddings: Tensor,
    /// Token type embeddings: [type_vocab_size, hidden_size]
    pub token_type_embeddings: Tensor,
    /// Embedding LayerNorm weight: [hidden_size]
    pub layer_norm_weight: Tensor,
    /// Embedding LayerNorm bias: [hidden_size]
    pub layer_norm_bias: Tensor,
}

/// NomicBERT attention weights with fused QKV.
///
/// Uses a single fused Wqkv projection [3*hidden_size, hidden_size] instead of
/// separate Q, K, V matrices. This is split at runtime into Q, K, V.
#[derive(Debug)]
pub struct NomicAttentionWeights {
    /// Fused Q+K+V projection: [3*hidden_size, hidden_size]
    pub wqkv_weight: Tensor,
    /// Output projection: [hidden_size, hidden_size]
    pub out_proj_weight: Tensor,
    /// Attention LayerNorm weight (norm1): [hidden_size]
    pub norm1_weight: Tensor,
    /// Attention LayerNorm bias (norm1): [hidden_size]
    pub norm1_bias: Tensor,
}

/// NomicBERT SwiGLU FFN weights.
///
/// SwiGLU uses two parallel projections (gate and up), then:
///   output = fc2(SiLU(fc11(x)) * fc12(x))
#[derive(Debug)]
pub struct NomicFfnWeights {
    /// Gate projection (SwiGLU): [intermediate_size, hidden_size]
    pub fc11_weight: Tensor,
    /// Up projection (SwiGLU): [intermediate_size, hidden_size]
    pub fc12_weight: Tensor,
    /// Down projection: [hidden_size, intermediate_size]
    pub fc2_weight: Tensor,
    /// FFN LayerNorm weight (norm2): [hidden_size]
    pub norm2_weight: Tensor,
    /// FFN LayerNorm bias (norm2): [hidden_size]
    pub norm2_bias: Tensor,
}

/// NomicBERT encoder layer weights.
#[derive(Debug)]
pub struct NomicEncoderLayerWeights {
    /// Attention weights (fused QKV + output + norm1).
    pub attention: NomicAttentionWeights,
    /// FFN weights (SwiGLU fc11/fc12/fc2 + norm2).
    pub ffn: NomicFfnWeights,
}

/// Complete NomicBERT model weights.
#[derive(Debug)]
pub struct NomicWeights {
    /// Model configuration.
    pub config: NomicConfig,
    /// Embedding layer weights (word + token_type + LayerNorm, no position).
    pub embeddings: NomicEmbeddingWeights,
    /// Encoder layer weights.
    pub encoder_layers: Vec<NomicEncoderLayerWeights>,
    /// GPU device reference.
    pub(crate) device: &'static Device,
}

// =============================================================================
// Causal Projection Weights for Asymmetric Embeddings
// =============================================================================

/// Standard deviation for initializing projection weight perturbations.
const PROJECTION_INIT_STD: f64 = 0.02;

/// Learned projection weights for asymmetric cause/effect embeddings.
///
/// These projections transform the base embedding into cause-role and
/// effect-role vectors. For the base nomic model, asymmetry comes from
/// instruction prefixes instead. These projections are available for
/// future fine-tuning of dedicated projection heads.
///
/// Initialized as perturbed identity matrices (I + N(0, 0.02)) to create
/// immediate asymmetry without requiring fine-tuning.
#[derive(Debug)]
pub struct CausalProjectionWeights {
    /// Cause projection matrix: [hidden_size, hidden_size]
    pub cause_projection: Tensor,
    /// Cause projection bias: [hidden_size]
    pub cause_bias: Tensor,
    /// Effect projection matrix: [hidden_size, hidden_size]
    pub effect_projection: Tensor,
    /// Effect projection bias: [hidden_size]
    pub effect_bias: Tensor,
}

// =============================================================================
// Trainable Projection Weights (for fine-tuning)
// =============================================================================

/// Trainable projection weights that wrap Candle `Var` for autograd support.
///
/// Holds both trainable `Var` tensors (for gradient computation) and inference
/// `Tensor` views. During training, gradients flow through the `Var` tensors.
/// For inference, `as_inference()` returns the standard `CausalProjectionWeights`.
///
/// # Loading Priority
///
/// `trained/projection_v1.safetensors` > perturbed identity fallback
#[derive(Debug)]
pub struct TrainableProjection {
    /// Trainable cause projection [hidden_size, hidden_size].
    pub cause_projection_var: candle_core::Var,
    /// Trainable cause bias [hidden_size].
    pub cause_bias_var: candle_core::Var,
    /// Trainable effect projection [hidden_size, hidden_size].
    pub effect_projection_var: candle_core::Var,
    /// Trainable effect bias [hidden_size].
    pub effect_bias_var: candle_core::Var,
    /// Hidden size (768 for nomic-embed).
    pub hidden_size: usize,
}

impl TrainableProjection {
    /// Create from existing (static) projection weights by wrapping in Var.
    pub fn from_inference(
        weights: &CausalProjectionWeights,
    ) -> crate::error::EmbeddingResult<Self> {
        let hidden_size = weights.cause_projection.dim(0).map_err(|e| {
            crate::error::EmbeddingError::GpuError {
                message: format!("Failed to get projection dim: {}", e),
            }
        })?;

        let cause_projection_var = candle_core::Var::from_tensor(&weights.cause_projection)
            .map_err(|e| crate::error::EmbeddingError::GpuError {
                message: format!("Failed to create trainable cause projection: {}", e),
            })?;
        let cause_bias_var = candle_core::Var::from_tensor(&weights.cause_bias).map_err(|e| {
            crate::error::EmbeddingError::GpuError {
                message: format!("Failed to create trainable cause bias: {}", e),
            }
        })?;
        let effect_projection_var = candle_core::Var::from_tensor(&weights.effect_projection)
            .map_err(|e| crate::error::EmbeddingError::GpuError {
                message: format!("Failed to create trainable effect projection: {}", e),
            })?;
        let effect_bias_var = candle_core::Var::from_tensor(&weights.effect_bias).map_err(|e| {
            crate::error::EmbeddingError::GpuError {
                message: format!("Failed to create trainable effect bias: {}", e),
            }
        })?;

        Ok(Self {
            cause_projection_var,
            cause_bias_var,
            effect_projection_var,
            effect_bias_var,
            hidden_size,
        })
    }

    /// Create a new trainable projection with perturbed identity initialization.
    pub fn new(hidden_size: usize, device: &Device) -> crate::error::EmbeddingResult<Self> {
        let weights =
            CausalProjectionWeights::initialize(hidden_size, device, CAUSAL_PROJECTION_SEED)?;
        Self::from_inference(&weights)
    }

    /// Create inference weights view (borrows tensor data from Var).
    pub fn as_inference(&self) -> CausalProjectionWeights {
        CausalProjectionWeights {
            cause_projection: self.cause_projection_var.as_tensor().clone(),
            cause_bias: self.cause_bias_var.as_tensor().clone(),
            effect_projection: self.effect_projection_var.as_tensor().clone(),
            effect_bias: self.effect_bias_var.as_tensor().clone(),
        }
    }

    /// Get all trainable Var tensors (for optimizer registration).
    pub fn trainable_vars(&self) -> Vec<&candle_core::Var> {
        vec![
            &self.cause_projection_var,
            &self.cause_bias_var,
            &self.effect_projection_var,
            &self.effect_bias_var,
        ]
    }

    /// Save trained weights to safetensors format.
    pub fn save_trained(&self, path: &std::path::Path) -> crate::error::EmbeddingResult<()> {
        use std::collections::HashMap;

        let mut tensors: HashMap<String, Tensor> = HashMap::new();
        tensors.insert(
            "cause_projection".to_string(),
            self.cause_projection_var.as_tensor().clone(),
        );
        tensors.insert(
            "cause_bias".to_string(),
            self.cause_bias_var.as_tensor().clone(),
        );
        tensors.insert(
            "effect_projection".to_string(),
            self.effect_projection_var.as_tensor().clone(),
        );
        tensors.insert(
            "effect_bias".to_string(),
            self.effect_bias_var.as_tensor().clone(),
        );

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::error::EmbeddingError::InternalError {
                    message: format!("Failed to create checkpoint dir: {}", e),
                }
            })?;
        }

        let tensor_data: Vec<(String, Vec<f32>, Vec<usize>)> = tensors
            .iter()
            .map(|(k, v)| {
                let data: Vec<f32> = v
                    .flatten_all()
                    .map_err(|e| crate::error::EmbeddingError::InternalError {
                        message: format!("Flatten tensor '{}' failed: {}", k, e),
                    })?
                    .to_vec1()
                    .map_err(|e| crate::error::EmbeddingError::InternalError {
                        message: format!("to_vec1 tensor '{}' failed: {}", k, e),
                    })?;
                let shape: Vec<usize> = v.shape().dims().to_vec();
                Ok((k.clone(), data, shape))
            })
            .collect::<Result<Vec<_>, crate::error::EmbeddingError>>()?;

        let views: Vec<(String, safetensors::tensor::TensorView<'_>)> = tensor_data
            .iter()
            .map(|(k, data, shape)| {
                let view = safetensors::tensor::TensorView::new(
                    safetensors::Dtype::F32,
                    shape.clone(),
                    bytemuck::cast_slice(data.as_slice()),
                )
                .map_err(|e| crate::error::EmbeddingError::InternalError {
                    message: format!("TensorView for '{}' failed: {}", k, e),
                })?;
                Ok((k.clone(), view))
            })
            .collect::<Result<Vec<_>, crate::error::EmbeddingError>>()?;

        safetensors::tensor::serialize_to_file(
            views.iter().map(|(k, v)| (k.clone(), v.clone())),
            &None::<HashMap<String, String>>,
            path,
        )
        .map_err(|e| crate::error::EmbeddingError::InternalError {
            message: format!("Failed to save trained weights: {}", e),
        })?;

        tracing::info!("Saved trained projection weights to {}", path.display());
        Ok(())
    }

    /// Load trained weights from safetensors format.
    pub fn load_trained(
        path: &std::path::Path,
        device: &Device,
    ) -> crate::error::EmbeddingResult<Self> {
        let data =
            std::fs::read(path).map_err(|e| crate::error::EmbeddingError::InternalError {
                message: format!("Failed to read checkpoint: {}", e),
            })?;

        let safetensors = safetensors::SafeTensors::deserialize(&data).map_err(|e| {
            crate::error::EmbeddingError::InternalError {
                message: format!("Failed to deserialize checkpoint: {}", e),
            }
        })?;

        let load_tensor = |name: &str| -> crate::error::EmbeddingResult<Tensor> {
            let view = safetensors.tensor(name).map_err(|e| {
                crate::error::EmbeddingError::InternalError {
                    message: format!("Missing tensor '{}': {}", name, e),
                }
            })?;
            let shape: Vec<usize> = view.shape().to_vec();
            let float_data: &[f32] = bytemuck::cast_slice(view.data());
            Tensor::from_slice(float_data, shape, device).map_err(|e| {
                crate::error::EmbeddingError::GpuError {
                    message: format!("Failed to create tensor '{}': {}", name, e),
                }
            })
        };

        let cause_proj = load_tensor("cause_projection")?;
        let cause_bias = load_tensor("cause_bias")?;
        let effect_proj = load_tensor("effect_projection")?;
        let effect_bias = load_tensor("effect_bias")?;

        let hidden_size =
            cause_proj
                .dim(0)
                .map_err(|e| crate::error::EmbeddingError::GpuError {
                    message: format!("Failed to get dim: {}", e),
                })?;

        Ok(Self {
            cause_projection_var: candle_core::Var::from_tensor(&cause_proj).map_err(|e| {
                crate::error::EmbeddingError::GpuError {
                    message: format!("Var from tensor: {}", e),
                }
            })?,
            cause_bias_var: candle_core::Var::from_tensor(&cause_bias).map_err(|e| {
                crate::error::EmbeddingError::GpuError {
                    message: format!("Var from tensor: {}", e),
                }
            })?,
            effect_projection_var: candle_core::Var::from_tensor(&effect_proj).map_err(|e| {
                crate::error::EmbeddingError::GpuError {
                    message: format!("Var from tensor: {}", e),
                }
            })?,
            effect_bias_var: candle_core::Var::from_tensor(&effect_bias).map_err(|e| {
                crate::error::EmbeddingError::GpuError {
                    message: format!("Var from tensor: {}", e),
                }
            })?,
            hidden_size,
        })
    }

    /// Apply cause projection using trainable Var (gradients flow through).
    pub fn project_cause_trainable(
        &self,
        embedding: &Tensor,
    ) -> crate::error::EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(&self.cause_projection_var.as_tensor().t().map_err(|e| {
                crate::error::EmbeddingError::GpuError {
                    message: format!("Cause projection transpose: {}", e),
                }
            })?)
            .map_err(|e| crate::error::EmbeddingError::GpuError {
                message: format!("Cause projection matmul: {}", e),
            })?;

        projected
            .broadcast_add(self.cause_bias_var.as_tensor())
            .map_err(|e| crate::error::EmbeddingError::GpuError {
                message: format!("Cause bias add: {}", e),
            })
    }

    /// Apply effect projection using trainable Var (gradients flow through).
    pub fn project_effect_trainable(
        &self,
        embedding: &Tensor,
    ) -> crate::error::EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(&self.effect_projection_var.as_tensor().t().map_err(|e| {
                crate::error::EmbeddingError::GpuError {
                    message: format!("Effect projection transpose: {}", e),
                }
            })?)
            .map_err(|e| crate::error::EmbeddingError::GpuError {
                message: format!("Effect projection matmul: {}", e),
            })?;

        projected
            .broadcast_add(self.effect_bias_var.as_tensor())
            .map_err(|e| crate::error::EmbeddingError::GpuError {
                message: format!("Effect bias add: {}", e),
            })
    }
}

impl CausalProjectionWeights {
    /// Initialize projection weights as perturbed identity matrices.
    pub fn initialize(hidden_size: usize, device: &Device, seed: u64) -> EmbeddingResult<Self> {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let cause_data = create_perturbed_identity(hidden_size, &mut rng, PROJECTION_INIT_STD);
        let cause_projection = Tensor::from_slice(&cause_data, (hidden_size, hidden_size), device)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to create cause projection: {}", e),
            })?
            .to_dtype(DType::F32)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to convert cause projection dtype: {}", e),
            })?;

        let effect_data = create_perturbed_identity(hidden_size, &mut rng, PROJECTION_INIT_STD);
        let effect_projection =
            Tensor::from_slice(&effect_data, (hidden_size, hidden_size), device)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to create effect projection: {}", e),
                })?
                .to_dtype(DType::F32)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to convert effect projection dtype: {}", e),
                })?;

        let cause_bias_data: Vec<f32> = (0..hidden_size)
            .map(|_| rng.gen_range(-0.01f32..0.01f32))
            .collect();
        let cause_bias =
            Tensor::from_slice(&cause_bias_data, hidden_size, device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create cause bias: {}", e),
                }
            })?;

        let effect_bias_data: Vec<f32> = (0..hidden_size)
            .map(|_| rng.gen_range(-0.01f32..0.01f32))
            .collect();
        let effect_bias =
            Tensor::from_slice(&effect_bias_data, hidden_size, device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create effect bias: {}", e),
                }
            })?;

        Ok(Self {
            cause_projection,
            cause_bias,
            effect_projection,
            effect_bias,
        })
    }

    /// Apply cause projection to an embedding.
    pub fn project_cause(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .cause_projection
                    .t()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Cause projection transpose failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Cause projection matmul failed: {}", e),
            })?;

        projected
            .broadcast_add(&self.cause_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Cause projection bias add failed: {}", e),
            })
    }

    /// Apply effect projection to an embedding.
    pub fn project_effect(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .effect_projection
                    .t()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Effect projection transpose failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Effect projection matmul failed: {}", e),
            })?;

        projected
            .broadcast_add(&self.effect_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Effect projection bias add failed: {}", e),
            })
    }
}

/// Create a perturbed identity matrix: I + N(0, std)
fn create_perturbed_identity<R: Rng>(size: usize, rng: &mut R, std: f64) -> Vec<f32> {
    let mut data = vec![0.0f32; size * size];

    for i in 0..size {
        for j in 0..size {
            let idx = i * size + j;
            let identity: f32 = if i == j { 1.0 } else { 0.0 };
            let u1: f64 = rng.gen_range(0.0001f64..1.0f64);
            let u2: f64 = rng.gen_range(0.0f64..1.0f64);
            let normal: f64 =
                (-2.0_f64 * u1.ln()).sqrt() * (2.0_f64 * std::f64::consts::PI * u2).cos();
            let perturbation = (normal * std) as f32;

            data[idx] = identity + perturbation;
        }
    }

    data
}
