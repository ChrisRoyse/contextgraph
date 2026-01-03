//! Single model configuration types.
//!
//! Controls device placement, quantization, and inference parameters.

use serde::{Deserialize, Serialize};

use crate::error::{EmbeddingError, EmbeddingResult};

use super::{DevicePlacement, QuantizationMode};

/// Configuration for a single embedding model.
///
/// Controls device placement, quantization, and inference parameters.
///
/// # Example
///
/// ```
/// # use context_graph_embeddings::traits::{SingleModelConfig, DevicePlacement, QuantizationMode};
/// let config = SingleModelConfig {
///     device: DevicePlacement::Cuda(0),
///     quantization: QuantizationMode::Fp16,
///     max_batch_size: 32,
///     use_flash_attention: true,
/// };
/// assert!(config.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleModelConfig {
    /// Device placement for this model.
    #[serde(default)]
    pub device: DevicePlacement,

    /// Quantization mode for reduced memory.
    #[serde(default)]
    pub quantization: QuantizationMode,

    /// Maximum batch size for this model.
    /// Larger batches improve throughput but use more memory.
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,

    /// Whether to use flash attention if available.
    /// Flash attention reduces memory and improves speed.
    #[serde(default = "default_use_flash_attention")]
    pub use_flash_attention: bool,
}

fn default_max_batch_size() -> usize {
    32
}

fn default_use_flash_attention() -> bool {
    true
}

impl Default for SingleModelConfig {
    fn default() -> Self {
        Self {
            device: DevicePlacement::Auto,
            quantization: QuantizationMode::None,
            max_batch_size: 32,
            use_flash_attention: true,
        }
    }
}

impl SingleModelConfig {
    /// Create config for CPU-only inference.
    pub fn cpu() -> Self {
        Self {
            device: DevicePlacement::Cpu,
            ..Default::default()
        }
    }

    /// Create config for CUDA device 0.
    pub fn cuda() -> Self {
        Self {
            device: DevicePlacement::Cuda(0),
            ..Default::default()
        }
    }

    /// Create config with FP16 quantization on CUDA.
    pub fn cuda_fp16() -> Self {
        Self {
            device: DevicePlacement::Cuda(0),
            quantization: QuantizationMode::Fp16,
            ..Default::default()
        }
    }

    /// Validate configuration values.
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if max_batch_size is 0
    pub fn validate(&self) -> EmbeddingResult<()> {
        if self.max_batch_size == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "max_batch_size must be greater than 0".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_model_config_default() {
        let config = SingleModelConfig::default();
        assert_eq!(config.device, DevicePlacement::Auto);
        assert_eq!(config.quantization, QuantizationMode::None);
        assert_eq!(config.max_batch_size, 32);
        assert!(config.use_flash_attention);
    }

    #[test]
    fn test_single_model_config_cpu() {
        let config = SingleModelConfig::cpu();
        assert_eq!(config.device, DevicePlacement::Cpu);
    }

    #[test]
    fn test_single_model_config_cuda() {
        let config = SingleModelConfig::cuda();
        assert_eq!(config.device, DevicePlacement::Cuda(0));
    }

    #[test]
    fn test_single_model_config_cuda_fp16() {
        let config = SingleModelConfig::cuda_fp16();
        assert_eq!(config.device, DevicePlacement::Cuda(0));
        assert_eq!(config.quantization, QuantizationMode::Fp16);
    }

    #[test]
    fn test_single_model_config_validate_success() {
        let config = SingleModelConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_single_model_config_validate_zero_batch_fails() {
        let config = SingleModelConfig {
            max_batch_size: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::ConfigError { message }) => {
                assert!(message.contains("max_batch_size"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = SingleModelConfig {
            device: DevicePlacement::Cuda(1),
            quantization: QuantizationMode::Int8,
            max_batch_size: 64,
            use_flash_attention: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: SingleModelConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.device, config.device);
        assert_eq!(restored.quantization, config.quantization);
        assert_eq!(restored.max_batch_size, config.max_batch_size);
        assert_eq!(restored.use_flash_attention, config.use_flash_attention);
    }
}
