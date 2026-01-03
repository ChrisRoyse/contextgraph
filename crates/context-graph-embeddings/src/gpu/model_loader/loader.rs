//! GPU model loader for loading pretrained BERT models from safetensors.
//!
//! Provides GPU-accelerated model loading via Candle's VarBuilder.

use candle_core::{DType, Device};
use candle_nn::VarBuilder;
use std::collections::HashMap;
use std::path::Path;

use super::batch_loader;
use super::config::BertConfig;
use super::embedding_loader::{load_embeddings, load_pooler};
use super::error::ModelLoadError;
use super::layer_loader::load_encoder_layer;
use super::weights::BertWeights;
use crate::gpu::init_gpu;

/// GPU model loader for loading pretrained models from safetensors.
///
/// # Example
///
/// ```rust,no_run
/// use context_graph_embeddings::gpu::GpuModelLoader;
/// use candle_core::DType;
///
/// // Create loader (initializes GPU)
/// let loader = GpuModelLoader::new().expect("GPU init");
///
/// // Verify device and dtype
/// assert!(loader.device().is_cuda());
/// assert_eq!(loader.dtype(), DType::F32);
/// ```
pub struct GpuModelLoader {
    /// Reference to the GPU device singleton.
    device: &'static Device,
    /// Default dtype for loading weights.
    dtype: DType,
}

impl GpuModelLoader {
    /// Create a new GPU model loader.
    ///
    /// Initializes the GPU device if not already initialized.
    ///
    /// # Errors
    ///
    /// Returns error if GPU initialization fails (no CUDA, no GPU hardware).
    pub fn new() -> Result<Self, ModelLoadError> {
        let device = init_gpu().map_err(|e| ModelLoadError::GpuInitError {
            message: e.to_string(),
        })?;

        Ok(Self {
            device,
            dtype: DType::F32,
        })
    }

    /// Create a loader with a specific dtype.
    pub fn with_dtype(dtype: DType) -> Result<Self, ModelLoadError> {
        let device = init_gpu().map_err(|e| ModelLoadError::GpuInitError {
            message: e.to_string(),
        })?;

        Ok(Self { device, dtype })
    }

    /// Get the GPU device.
    pub fn device(&self) -> &'static Device {
        self.device
    }

    /// Get the dtype used for loading.
    pub fn dtype(&self) -> DType {
        self.dtype
    }

    /// Load BERT model configuration from config.json.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Path to model directory containing config.json
    ///
    /// # Errors
    ///
    /// Returns error if config.json is missing or malformed.
    pub fn load_config(&self, model_dir: &Path) -> Result<BertConfig, ModelLoadError> {
        let config_path = model_dir.join("config.json");

        if !config_path.exists() {
            return Err(ModelLoadError::ConfigNotFound {
                path: config_path.display().to_string(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "config.json not found"),
            });
        }

        let config_content =
            std::fs::read_to_string(&config_path).map_err(|e| ModelLoadError::ConfigNotFound {
                path: config_path.display().to_string(),
                source: e,
            })?;

        let config: BertConfig = serde_json::from_str(&config_content).map_err(|e| {
            ModelLoadError::ConfigParseError {
                path: config_path.display().to_string(),
                message: e.to_string(),
            }
        })?;

        // Validate architecture
        if !config.is_supported() {
            return Err(ModelLoadError::UnsupportedArchitecture {
                architecture: config.model_type.clone(),
            });
        }

        tracing::info!(
            "Loaded config for {} model: hidden_size={}, layers={}, heads={}",
            config.model_type,
            config.hidden_size,
            config.num_hidden_layers,
            config.num_attention_heads
        );

        Ok(config)
    }

    /// Load complete BERT weights from safetensors.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Path to model directory containing:
    ///   - config.json
    ///   - model.safetensors
    ///
    /// # Returns
    ///
    /// Complete BertWeights struct with all layers loaded to GPU.
    pub fn load_bert_weights(&self, model_dir: &Path) -> Result<BertWeights, ModelLoadError> {
        self.load_bert_weights_with_prefix(model_dir, "")
    }

    /// Load BERT weights with a custom weight key prefix.
    /// Some models (like SPLADE, ColBERT) use `bert.` as a prefix for all weights.
    ///
    /// # Arguments
    /// * `model_dir` - Path to model directory containing config.json and model.safetensors
    /// * `prefix` - Prefix for weight keys (e.g., "bert." for models with that prefix)
    pub fn load_bert_weights_with_prefix(
        &self,
        model_dir: &Path,
        prefix: &str,
    ) -> Result<BertWeights, ModelLoadError> {
        // Validate model directory exists
        if !model_dir.exists() {
            return Err(ModelLoadError::ModelDirectoryNotFound {
                path: model_dir.display().to_string(),
            });
        }

        // Load config
        let config = self.load_config(model_dir)?;

        // Check for safetensors file
        let safetensors_path = model_dir.join("model.safetensors");
        if !safetensors_path.exists() {
            return Err(ModelLoadError::SafetensorsNotFound {
                path: safetensors_path.display().to_string(),
            });
        }

        tracing::info!(
            "Loading safetensors from: {} (prefix: '{}')",
            safetensors_path.display(),
            prefix
        );

        // Create VarBuilder from safetensors
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&safetensors_path], self.dtype, self.device)
                .map_err(|e| ModelLoadError::SafetensorsLoadError {
                    path: safetensors_path.display().to_string(),
                    message: e.to_string(),
                })?
        };

        // Load embeddings
        let embeddings = load_embeddings(&vb, &config, model_dir, prefix)?;

        // Load encoder layers
        let mut encoder_layers = Vec::with_capacity(config.num_hidden_layers);
        for layer_idx in 0..config.num_hidden_layers {
            let layer = load_encoder_layer(&vb, &config, layer_idx, model_dir, prefix)?;
            encoder_layers.push(layer);
        }

        // Try to load pooler (optional - some models don't have it)
        let pooler = load_pooler(&vb, &config, model_dir, prefix).ok();

        let weights = BertWeights {
            config,
            embeddings,
            encoder_layers,
            pooler,
            device: self.device,
        };

        tracing::info!(
            "Loaded BERT weights: {} parameters, {:.2} MB VRAM",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0)
        );

        Ok(weights)
    }

    /// Load multiple models from a models config.
    ///
    /// # Arguments
    ///
    /// * `models_config_path` - Path to models_config.toml
    ///
    /// # Returns
    ///
    /// Map of model name to loaded BertWeights.
    pub fn load_models_from_config(
        &self,
        models_config_path: &Path,
    ) -> Result<HashMap<String, BertWeights>, ModelLoadError> {
        batch_loader::load_models_from_config(self, models_config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // Integration tests require GPU - skip in CI
    #[test]
    #[ignore = "Requires GPU"]
    fn test_load_semantic_model() {
        let loader = GpuModelLoader::new().expect("GPU init failed");
        let model_dir = Path::new("/home/cabdru/contextgraph/models/semantic");
        let weights = loader.load_bert_weights(model_dir).expect("Load failed");

        // e5-large-v2 specs
        assert_eq!(weights.config.hidden_size, 1024);
        assert_eq!(weights.config.num_hidden_layers, 24);
        assert_eq!(weights.config.num_attention_heads, 16);
        assert!(weights.param_count() > 300_000_000); // ~335M params
    }

    #[test]
    #[ignore = "Requires GPU"]
    fn test_load_entity_model() {
        let loader = GpuModelLoader::new().expect("GPU init failed");
        let model_dir = Path::new("/home/cabdru/contextgraph/models/entity");
        let weights = loader.load_bert_weights(model_dir).expect("Load failed");

        // all-MiniLM-L6-v2 specs
        assert_eq!(weights.config.hidden_size, 384);
        assert_eq!(weights.config.num_hidden_layers, 6);
        assert_eq!(weights.config.num_attention_heads, 12);
    }
}
