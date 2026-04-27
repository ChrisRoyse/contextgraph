//! Learned weight projection model implementation.
//!
//! This model uses Candle for GPU/CPU inference on the trained MLP weights.

use std::path::Path;

use candle_core::{DType, Device, Tensor};
use safetensors::SafeTensors;
use tracing::{debug, info, warn};

use crate::error::{EmbeddingError, EmbeddingResult};

use super::constants::{
    DEFAULT_CATEGORY_WEIGHTS, HIDDEN_DIM_1, HIDDEN_DIM_2, MAX_WEIGHTED_AGREEMENT, NUM_EMBEDDERS,
    OUTPUT_DIM,
};

/// Learned weight projection for graph edge weights.
///
/// Projects 13 embedder similarity scores to a single edge weight [0, 1]
/// using a trained 3-layer MLP.
///
/// # Thread Safety
///
/// This struct is `Send + Sync` and can be shared across threads.
/// All tensors are immutable after loading.
#[derive(Clone)]
pub struct LearnedWeightProjection {
    /// Learned category weights (initialized from constitution, then trained).
    category_weights: Tensor,

    /// Layer 1: Linear(13, 64)
    layer1_weight: Tensor,
    layer1_bias: Tensor,

    /// Layer 1: LayerNorm(64)
    layer_norm_weight: Tensor,
    layer_norm_bias: Tensor,

    /// Layer 2: Linear(64, 32)
    layer2_weight: Tensor,
    layer2_bias: Tensor,

    /// Layer 3: Linear(32, 1)
    layer3_weight: Tensor,
    layer3_bias: Tensor,

    /// Device for computation.
    device: Device,

    /// Whether the model is properly loaded.
    loaded: bool,
}

impl LearnedWeightProjection {
    /// Load the model from a SafeTensors file.
    ///
    /// # Arguments
    ///
    /// * `weights_path` - Path to the SafeTensors weights file
    /// * `device` - Device to load tensors to (CPU or CUDA)
    ///
    /// # Errors
    ///
    /// Returns error if the file cannot be read or tensors are invalid.
    pub fn load<P: AsRef<Path>>(weights_path: P, device: &Device) -> EmbeddingResult<Self> {
        let weights_path = weights_path.as_ref();
        info!("Loading learned weight projection from {:?}", weights_path);

        // Read the SafeTensors file
        let data = std::fs::read(weights_path)?;

        let safetensors =
            SafeTensors::deserialize(&data).map_err(|e| EmbeddingError::ConfigError {
                message: format!("Failed to parse SafeTensors from {:?}: {}", weights_path, e),
            })?;

        // Load tensors
        let category_weights = Self::load_tensor(&safetensors, "category_weights", device)?;
        let layer1_weight = Self::load_tensor(&safetensors, "projection_0_weight", device)?;
        let layer1_bias = Self::load_tensor(&safetensors, "projection_0_bias", device)?;
        let layer_norm_weight = Self::load_tensor(&safetensors, "projection_1_weight", device)?;
        let layer_norm_bias = Self::load_tensor(&safetensors, "projection_1_bias", device)?;
        let layer2_weight = Self::load_tensor(&safetensors, "projection_4_weight", device)?;
        let layer2_bias = Self::load_tensor(&safetensors, "projection_4_bias", device)?;
        let layer3_weight = Self::load_tensor(&safetensors, "projection_6_weight", device)?;
        let layer3_bias = Self::load_tensor(&safetensors, "projection_6_bias", device)?;

        debug!(
            "Loaded weight projection: category_weights={:?}, layer1={:?}x{:?}",
            category_weights.shape(),
            layer1_weight.shape(),
            layer1_bias.shape()
        );

        Ok(Self {
            category_weights,
            layer1_weight,
            layer1_bias,
            layer_norm_weight,
            layer_norm_bias,
            layer2_weight,
            layer2_bias,
            layer3_weight,
            layer3_bias,
            device: device.clone(),
            loaded: true,
        })
    }

    /// Create a fallback model using constitution weights (no trained weights).
    ///
    /// This uses the default category weights from the constitution to compute
    /// edge weights via weighted agreement.
    pub fn fallback(device: &Device) -> EmbeddingResult<Self> {
        warn!("Creating fallback weight projection (no trained weights)");

        // Create default category weights tensor
        let category_weights = Tensor::from_slice(&DEFAULT_CATEGORY_WEIGHTS, NUM_EMBEDDERS, device)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to create category weights tensor: {}", e),
            })?;

        // Create dummy tensors for the MLP layers (won't be used in fallback mode)
        let layer1_weight = Tensor::zeros((HIDDEN_DIM_1, NUM_EMBEDDERS), DType::F32, device)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to create layer1_weight: {}", e),
            })?;
        let layer1_bias = Tensor::zeros(HIDDEN_DIM_1, DType::F32, device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create layer1_bias: {}", e),
            }
        })?;
        let layer_norm_weight = Tensor::ones(HIDDEN_DIM_1, DType::F32, device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create layer_norm_weight: {}", e),
            }
        })?;
        let layer_norm_bias = Tensor::zeros(HIDDEN_DIM_1, DType::F32, device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create layer_norm_bias: {}", e),
            }
        })?;
        let layer2_weight = Tensor::zeros((HIDDEN_DIM_2, HIDDEN_DIM_1), DType::F32, device)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to create layer2_weight: {}", e),
            })?;
        let layer2_bias = Tensor::zeros(HIDDEN_DIM_2, DType::F32, device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create layer2_bias: {}", e),
            }
        })?;
        let layer3_weight =
            Tensor::zeros((OUTPUT_DIM, HIDDEN_DIM_2), DType::F32, device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create layer3_weight: {}", e),
                }
            })?;
        let layer3_bias = Tensor::zeros(OUTPUT_DIM, DType::F32, device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create layer3_bias: {}", e),
            }
        })?;

        Ok(Self {
            category_weights,
            layer1_weight,
            layer1_bias,
            layer_norm_weight,
            layer_norm_bias,
            layer2_weight,
            layer2_bias,
            layer3_weight,
            layer3_bias,
            device: device.clone(),
            loaded: false,
        })
    }

    /// Project embedder scores to edge weight.
    ///
    /// # Arguments
    ///
    /// * `embedder_scores` - Array of 13 similarity scores per embedder
    ///
    /// # Returns
    ///
    /// Edge weight in [0, 1].
    pub fn project(&self, embedder_scores: &[f32; NUM_EMBEDDERS]) -> EmbeddingResult<f32> {
        if !self.loaded {
            // Fallback: use weighted agreement
            return Ok(self.weighted_agreement_fallback(embedder_scores));
        }

        // Create input tensor [1, 13]
        let input =
            Tensor::from_slice(embedder_scores, (1, NUM_EMBEDDERS), &self.device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create input tensor: {}", e),
                }
            })?;

        // Apply learned category weights
        let category_weights =
            self.category_weights
                .unsqueeze(0)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to unsqueeze category weights: {}", e),
                })?;
        let weighted_input = (input * category_weights).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to apply category weights: {}", e),
        })?;

        // Layer 1: Linear + LayerNorm + GELU
        let x = self.linear(&weighted_input, &self.layer1_weight, &self.layer1_bias)?;
        let x = self.layer_norm(&x)?;
        let x = self.gelu(&x)?;

        // Layer 2: Linear + GELU
        let x = self.linear(&x, &self.layer2_weight, &self.layer2_bias)?;
        let x = self.gelu(&x)?;

        // Layer 3: Linear + Sigmoid
        let x = self.linear(&x, &self.layer3_weight, &self.layer3_bias)?;
        let x = self.sigmoid(&x)?;

        // Extract scalar result
        let result: Vec<f32> = x
            .flatten_all()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to flatten output: {}", e),
            })?
            .to_vec1()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to convert to vec: {}", e),
            })?;

        Ok(result.first().copied().unwrap_or(0.5))
    }

    /// Batch project multiple score sets.
    ///
    /// # Arguments
    ///
    /// * `batch_scores` - Vector of score arrays
    ///
    /// # Returns
    ///
    /// Vector of edge weights.
    pub fn project_batch(
        &self,
        batch_scores: &[[f32; NUM_EMBEDDERS]],
    ) -> EmbeddingResult<Vec<f32>> {
        if batch_scores.is_empty() {
            return Ok(Vec::new());
        }

        if !self.loaded {
            // Fallback: use weighted agreement
            return Ok(batch_scores
                .iter()
                .map(|s| self.weighted_agreement_fallback(s))
                .collect());
        }

        let batch_size = batch_scores.len();

        // Flatten to [batch * 13]
        let flat_scores: Vec<f32> = batch_scores
            .iter()
            .flat_map(|s| s.iter().copied())
            .collect();

        // Create input tensor [batch, 13]
        let input = Tensor::from_slice(&flat_scores, (batch_size, NUM_EMBEDDERS), &self.device)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to create batch input tensor: {}", e),
            })?;

        // Apply learned category weights (broadcast)
        let category_weights = self
            .category_weights
            .unsqueeze(0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to unsqueeze category weights: {}", e),
            })?
            .broadcast_as((batch_size, NUM_EMBEDDERS))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to broadcast category weights: {}", e),
            })?;

        let weighted_input = (input * category_weights).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to apply category weights: {}", e),
        })?;

        // Forward pass
        let x = self.linear(&weighted_input, &self.layer1_weight, &self.layer1_bias)?;
        let x = self.layer_norm(&x)?;
        let x = self.gelu(&x)?;

        let x = self.linear(&x, &self.layer2_weight, &self.layer2_bias)?;
        let x = self.gelu(&x)?;

        let x = self.linear(&x, &self.layer3_weight, &self.layer3_bias)?;
        let x = self.sigmoid(&x)?;

        // Extract results
        let results: Vec<f32> = x
            .flatten_all()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to flatten batch output: {}", e),
            })?
            .to_vec1()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to convert batch to vec: {}", e),
            })?;

        Ok(results)
    }

    /// Get the learned category weights.
    pub fn category_weights(&self) -> EmbeddingResult<[f32; NUM_EMBEDDERS]> {
        let weights: Vec<f32> =
            self.category_weights
                .to_vec1()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to get category weights: {}", e),
                })?;

        Ok(weights.try_into().unwrap_or(DEFAULT_CATEGORY_WEIGHTS))
    }

    /// Check if the model was loaded from trained weights.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    // Helper methods

    fn load_tensor(
        safetensors: &SafeTensors,
        name: &str,
        device: &Device,
    ) -> EmbeddingResult<Tensor> {
        let view = safetensors
            .tensor(name)
            .map_err(|e| EmbeddingError::ConfigError {
                message: format!("Tensor '{}' not found in weights file: {}", name, e),
            })?;

        let shape: Vec<usize> = view.shape().to_vec();
        let data: Vec<f32> = view
            .data()
            .chunks(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        Tensor::from_slice(&data, &shape[..], device).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to create tensor '{}': {}", name, e),
        })
    }

    fn linear(&self, input: &Tensor, weight: &Tensor, bias: &Tensor) -> EmbeddingResult<Tensor> {
        // y = x @ W^T + b
        let weight_t = weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to transpose weight: {}", e),
        })?;

        let matmul = input
            .matmul(&weight_t)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed matmul: {}", e),
            })?;

        let bias_broadcast = bias
            .unsqueeze(0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to unsqueeze bias: {}", e),
            })?
            .broadcast_as(matmul.shape())
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to broadcast bias: {}", e),
            })?;

        (matmul + bias_broadcast).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to add bias: {}", e),
        })
    }

    fn layer_norm(&self, input: &Tensor) -> EmbeddingResult<Tensor> {
        let eps = 1e-5;

        // Compute mean and variance over last dim
        let mean =
            input
                .mean_keepdim(candle_core::D::Minus1)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("LayerNorm mean failed: {}", e),
                })?;

        let centered = (input - mean).map_err(|e| EmbeddingError::GpuError {
            message: format!("LayerNorm center failed: {}", e),
        })?;

        let variance = centered
            .sqr()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm sqr failed: {}", e),
            })?
            .mean_keepdim(candle_core::D::Minus1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm variance failed: {}", e),
            })?;

        let std = (variance + eps)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm eps add failed: {}", e),
            })?
            .sqrt()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm sqrt failed: {}", e),
            })?;

        let normalized = (centered / std).map_err(|e| EmbeddingError::GpuError {
            message: format!("LayerNorm div failed: {}", e),
        })?;

        // Apply weight and bias
        let weight_broadcast = self
            .layer_norm_weight
            .unsqueeze(0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm weight unsqueeze failed: {}", e),
            })?
            .broadcast_as(normalized.shape())
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm weight broadcast failed: {}", e),
            })?;

        let bias_broadcast = self
            .layer_norm_bias
            .unsqueeze(0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm bias unsqueeze failed: {}", e),
            })?
            .broadcast_as(normalized.shape())
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LayerNorm bias broadcast failed: {}", e),
            })?;

        let scaled = (normalized * weight_broadcast).map_err(|e| EmbeddingError::GpuError {
            message: format!("LayerNorm scale failed: {}", e),
        })?;

        (scaled + bias_broadcast).map_err(|e| EmbeddingError::GpuError {
            message: format!("LayerNorm shift failed: {}", e),
        })
    }

    fn gelu(&self, input: &Tensor) -> EmbeddingResult<Tensor> {
        // GELU(x) = x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
        // Simplified approximation: candle has built-in gelu
        input.gelu_erf().map_err(|e| EmbeddingError::GpuError {
            message: format!("GELU failed: {}", e),
        })
    }

    fn sigmoid(&self, input: &Tensor) -> EmbeddingResult<Tensor> {
        candle_nn::ops::sigmoid(input).map_err(|e| EmbeddingError::GpuError {
            message: format!("Sigmoid failed: {}", e),
        })
    }

    /// Fallback computation using weighted agreement from constitution.
    fn weighted_agreement_fallback(&self, scores: &[f32; NUM_EMBEDDERS]) -> f32 {
        let mut weighted_sum = 0.0f32;

        for (i, &score) in scores.iter().enumerate() {
            // Apply default thresholds (0.5 for simplicity)
            if score >= 0.5 {
                weighted_sum += DEFAULT_CATEGORY_WEIGHTS[i];
            }
        }

        // Normalize to [0, 1]
        (weighted_sum / MAX_WEIGHTED_AGREEMENT).clamp(0.0, 1.0)
    }
}

impl std::fmt::Debug for LearnedWeightProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LearnedWeightProjection")
            .field("loaded", &self.loaded)
            .field("device", &self.device)
            .finish()
    }
}

// Implement WeightProjector trait for integration with EdgeBuilder
impl context_graph_core::graph_linking::WeightProjector for LearnedWeightProjection {
    fn project(&self, scores: &[f32; NUM_EMBEDDERS]) -> f32 {
        // Call the internal project method, returning fallback value on error
        self.project(scores).unwrap_or_else(|e| {
            tracing::warn!("Weight projection failed, using fallback: {}", e);
            self.weighted_agreement_fallback(scores)
        })
    }

    fn project_batch(&self, batch_scores: &[[f32; NUM_EMBEDDERS]]) -> Vec<f32> {
        // Call the internal batch method, falling back on error
        self.project_batch(batch_scores).unwrap_or_else(|e| {
            tracing::warn!("Batch weight projection failed, using fallback: {}", e);
            batch_scores
                .iter()
                .map(|s| self.weighted_agreement_fallback(s))
                .collect()
        })
    }

    fn is_learned(&self) -> bool {
        self.loaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_projection() {
        let device = Device::Cpu;
        let proj = LearnedWeightProjection::fallback(&device).unwrap();

        // Test with scores above threshold
        let scores = [
            0.8, 0.0, 0.0, 0.0, 0.7, 0.6, 0.9, 0.6, 0.5, 0.7, 0.6, 0.5, 0.6, 0.7,
        ];
        let weight = proj.project(&scores).unwrap();

        assert!((0.0..=1.0).contains(&weight));
        assert!(weight > 0.5); // Should be high with many scores above threshold
    }

    #[test]
    fn test_fallback_low_scores() {
        let device = Device::Cpu;
        let proj = LearnedWeightProjection::fallback(&device).unwrap();

        // Test with scores below threshold
        let scores = [
            0.1, 0.0, 0.0, 0.0, 0.2, 0.1, 0.3, 0.2, 0.1, 0.2, 0.1, 0.2, 0.1, 0.1,
        ];
        let weight = proj.project(&scores).unwrap();

        assert!((0.0..=1.0).contains(&weight));
        assert!(weight < 0.2); // Should be low with no scores above threshold
    }

    #[test]
    fn test_batch_projection() {
        let device = Device::Cpu;
        let proj = LearnedWeightProjection::fallback(&device).unwrap();

        let batch = vec![
            [
                0.8, 0.0, 0.0, 0.0, 0.7, 0.6, 0.9, 0.6, 0.5, 0.7, 0.6, 0.5, 0.6, 0.7,
            ],
            [
                0.1, 0.0, 0.0, 0.0, 0.2, 0.1, 0.3, 0.2, 0.1, 0.2, 0.1, 0.2, 0.1, 0.1,
            ],
        ];

        let weights = proj.project_batch(&batch).unwrap();

        assert_eq!(weights.len(), 2);
        assert!(weights[0] > weights[1]); // First should be higher
    }
}
