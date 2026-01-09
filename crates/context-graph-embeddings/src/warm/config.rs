//! Configuration for the warm model loading system.
//!
//! # Overview
//!
//! `WarmConfig` controls VRAM budgets, model paths, CUDA device selection,
//! and loading behavior for the warm model loading system.
//!
//! # Environment Variable Overrides
//!
//! All configuration can be loaded from environment variables via [`WarmConfig::from_env()`]:
//!
//! | Field | Environment Variable | Default |
//! |-------|---------------------|---------|
//! | `vram_budget_bytes` | `WARM_VRAM_BUDGET_BYTES` | 25,769,803,776 (24GB) |
//! | `vram_headroom_bytes` | `WARM_VRAM_HEADROOM_BYTES` | 8,589,934,592 (8GB) |
//! | `model_weights_path` | `WARM_MODEL_WEIGHTS_PATH` | "./models" |
//! | `diagnostic_dump_path` | `WARM_DIAGNOSTIC_DUMP_PATH` | "/var/log/context-graph" |
//! | `cuda_device_id` | `CUDA_VISIBLE_DEVICES` | 0 |
//! | `enable_test_inference` | `WARM_ENABLE_TEST_INFERENCE` | true |
//! | `max_load_time_per_model_ms` | `WARM_MAX_LOAD_TIME_MS` | 30,000 (30s) |
//!
//! # Requirements Implemented
//!
//! - REQ-WARM-012: VRAM budget enforcement (24GB default)
//! - REQ-WARM-013: VRAM usage reporting

use std::path::PathBuf;

use super::error::WarmError;

/// One gigabyte in bytes.
const GB: usize = 1024 * 1024 * 1024;

/// Quantization modes for model loading.
///
/// Lower precision modes reduce VRAM usage at the cost of some accuracy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuantizationMode {
    /// Full precision (FP32) - ~7.2GB total for all models.
    Fp32,
    /// Half precision (FP16) - ~3.6GB total (recommended).
    #[default]
    Fp16,
    /// Quarter precision (FP8) - ~1.8GB total.
    Fp8,
}

impl QuantizationMode {
    /// Get the memory multiplier relative to FP32.
    #[must_use]
    pub fn memory_multiplier(&self) -> f32 {
        match self {
            Self::Fp32 => 1.0,
            Self::Fp16 => 0.5,
            Self::Fp8 => 0.25,
        }
    }

    /// Get the human-readable name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fp32 => "FP32",
            Self::Fp16 => "FP16",
            Self::Fp8 => "FP8",
        }
    }
}

/// Configuration for the warm loading system.
///
/// Controls VRAM allocation, model paths, and loading behavior.
///
/// # Example
///
/// ```rust,no_run
/// use context_graph_embeddings::warm::config::WarmConfig;
///
/// // Load from environment variables
/// let config = WarmConfig::from_env();
///
/// // Or use defaults
/// let config = WarmConfig::default();
///
/// // Validate before use
/// config.validate()?;
/// # Ok::<(), context_graph_embeddings::warm::WarmError>(())
/// ```
#[derive(Debug, Clone)]
pub struct WarmConfig {
    /// Maximum VRAM for model weights (default: 24GB).
    ///
    /// This budget is enforced during model loading. Models are loaded
    /// largest-first to detect VRAM issues early.
    pub vram_budget_bytes: usize,

    /// Reserved VRAM for working memory (default: 8GB).
    ///
    /// This headroom is kept free for inference activations and
    /// intermediate tensors during forward passes.
    pub vram_headroom_bytes: usize,

    /// Path to model weight files.
    ///
    /// Directory containing SafeTensors model files.
    pub model_weights_path: PathBuf,

    /// Path for diagnostic dump files.
    ///
    /// On fatal startup errors, diagnostic JSON is written here.
    pub diagnostic_dump_path: PathBuf,

    /// CUDA device ID to use.
    ///
    /// Typically 0 for single-GPU systems.
    pub cuda_device_id: u32,

    /// Whether to run test inference during health check.
    ///
    /// Enabled by default for validation.
    pub enable_test_inference: bool,

    /// Maximum time (ms) to load a single model.
    ///
    /// Prevents indefinite hangs on corrupted weight files.
    pub max_load_time_per_model_ms: u64,

    /// Quantization mode for loading.
    ///
    /// FP16 recommended for balance of memory and accuracy.
    pub quantization: QuantizationMode,
}

impl Default for WarmConfig {
    fn default() -> Self {
        Self {
            vram_budget_bytes: 24 * GB,
            vram_headroom_bytes: 8 * GB,
            model_weights_path: PathBuf::from("./models"),
            diagnostic_dump_path: PathBuf::from("/var/log/context-graph"),
            cuda_device_id: 0,
            enable_test_inference: true,
            max_load_time_per_model_ms: 30_000,
            quantization: QuantizationMode::Fp16,
        }
    }
}

impl WarmConfig {
    /// Load configuration from environment variables.
    ///
    /// Falls back to default values for any unset variables.
    /// Invalid values are silently ignored (defaults used).
    #[must_use]
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("WARM_VRAM_BUDGET_BYTES") {
            if let Ok(bytes) = val.parse::<usize>() {
                config.vram_budget_bytes = bytes;
            }
        }

        if let Ok(val) = std::env::var("WARM_VRAM_HEADROOM_BYTES") {
            if let Ok(bytes) = val.parse::<usize>() {
                config.vram_headroom_bytes = bytes;
            }
        }

        if let Ok(val) = std::env::var("WARM_MODEL_WEIGHTS_PATH") {
            config.model_weights_path = PathBuf::from(val);
        }

        if let Ok(val) = std::env::var("WARM_DIAGNOSTIC_DUMP_PATH") {
            config.diagnostic_dump_path = PathBuf::from(val);
        }

        if let Ok(val) = std::env::var("CUDA_VISIBLE_DEVICES") {
            // CUDA_VISIBLE_DEVICES can be comma-separated; take the first
            if let Some(first) = val.split(',').next() {
                if let Ok(id) = first.trim().parse::<u32>() {
                    config.cuda_device_id = id;
                }
            }
        }

        if let Ok(val) = std::env::var("WARM_ENABLE_TEST_INFERENCE") {
            config.enable_test_inference =
                matches!(val.to_lowercase().as_str(), "true" | "1" | "yes" | "on");
        }

        if let Ok(val) = std::env::var("WARM_MAX_LOAD_TIME_MS") {
            if let Ok(ms) = val.parse::<u64>() {
                config.max_load_time_per_model_ms = ms;
            }
        }

        config
    }

    /// Total VRAM required for target GPU (budget + headroom).
    ///
    /// For RTX 5090 with 32GB VRAM, default is 32GB total.
    #[must_use]
    pub fn total_vram_required(&self) -> usize {
        self.vram_budget_bytes
            .saturating_add(self.vram_headroom_bytes)
    }

    /// Validate configuration consistency.
    ///
    /// # Errors
    ///
    /// Returns `WarmError::InvalidConfig` if:
    /// - `vram_budget_bytes` is zero
    /// - `model_weights_path` does not exist
    pub fn validate(&self) -> Result<(), WarmError> {
        if self.vram_budget_bytes == 0 {
            return Err(WarmError::InvalidConfig {
                field: "vram_budget_bytes".into(),
                reason: "must be greater than 0".into(),
            });
        }

        if !self.model_weights_path.exists() {
            return Err(WarmError::InvalidConfig {
                field: "model_weights_path".into(),
                reason: format!("path does not exist: {:?}", self.model_weights_path),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config = WarmConfig::default();

        assert_eq!(config.vram_budget_bytes, 24 * GB);
        assert_eq!(config.vram_headroom_bytes, 8 * GB);
        assert_eq!(config.model_weights_path, PathBuf::from("./models"));
        assert_eq!(
            config.diagnostic_dump_path,
            PathBuf::from("/var/log/context-graph")
        );
        assert_eq!(config.cuda_device_id, 0);
        assert!(config.enable_test_inference);
        assert_eq!(config.max_load_time_per_model_ms, 30_000);
        assert_eq!(config.quantization, QuantizationMode::Fp16);
    }

    #[test]
    fn test_total_vram_required() {
        let config = WarmConfig::default();
        assert_eq!(config.total_vram_required(), 32 * GB);
    }

    #[test]
    fn test_from_env_vram_budget() {
        std::env::set_var("WARM_VRAM_BUDGET_BYTES", "16000000000");
        let config = WarmConfig::from_env();
        assert_eq!(config.vram_budget_bytes, 16_000_000_000);
        std::env::remove_var("WARM_VRAM_BUDGET_BYTES");
    }

    #[test]
    fn test_from_env_model_path() {
        std::env::set_var("WARM_MODEL_WEIGHTS_PATH", "/custom/models");
        let config = WarmConfig::from_env();
        assert_eq!(config.model_weights_path, PathBuf::from("/custom/models"));
        std::env::remove_var("WARM_MODEL_WEIGHTS_PATH");
    }

    #[test]
    fn test_from_env_cuda_device() {
        std::env::set_var("CUDA_VISIBLE_DEVICES", "2");
        let config = WarmConfig::from_env();
        assert_eq!(config.cuda_device_id, 2);
        std::env::remove_var("CUDA_VISIBLE_DEVICES");
    }

    #[test]
    fn test_from_env_cuda_device_comma_separated() {
        std::env::set_var("CUDA_VISIBLE_DEVICES", "1,2,3");
        let config = WarmConfig::from_env();
        assert_eq!(config.cuda_device_id, 1);
        std::env::remove_var("CUDA_VISIBLE_DEVICES");
    }

    #[test]
    fn test_from_env_test_inference_variants() {
        for (val, expected) in [
            ("true", true),
            ("TRUE", true),
            ("1", true),
            ("yes", true),
            ("on", true),
            ("false", false),
            ("0", false),
            ("no", false),
        ] {
            std::env::set_var("WARM_ENABLE_TEST_INFERENCE", val);
            let config = WarmConfig::from_env();
            assert_eq!(
                config.enable_test_inference, expected,
                "for value '{}'",
                val
            );
        }
        std::env::remove_var("WARM_ENABLE_TEST_INFERENCE");
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_validate_zero_budget() {
        let mut config = WarmConfig::default();
        config.vram_budget_bytes = 0;

        let result = config.validate();
        assert!(result.is_err());
        if let Err(WarmError::InvalidConfig { field, .. }) = result {
            assert_eq!(field, "vram_budget_bytes");
        }
    }

    #[test]
    fn test_quantization_mode_multiplier() {
        assert_eq!(QuantizationMode::Fp32.memory_multiplier(), 1.0);
        assert_eq!(QuantizationMode::Fp16.memory_multiplier(), 0.5);
        assert_eq!(QuantizationMode::Fp8.memory_multiplier(), 0.25);
    }

    #[test]
    fn test_quantization_mode_as_str() {
        assert_eq!(QuantizationMode::Fp32.as_str(), "FP32");
        assert_eq!(QuantizationMode::Fp16.as_str(), "FP16");
        assert_eq!(QuantizationMode::Fp8.as_str(), "FP8");
    }

    #[test]
    fn test_quantization_mode_default() {
        assert_eq!(QuantizationMode::default(), QuantizationMode::Fp16);
    }
}
