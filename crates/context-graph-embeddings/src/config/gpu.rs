//! GPU configuration.
//!
//! Target hardware: RTX 5090 (32GB GDDR7, Compute 12.0, CUDA 13.2)

use serde::{Deserialize, Serialize};

use crate::error::{EmbeddingError, EmbeddingResult};

// ============================================================================
// DEFAULT FUNCTIONS
// ============================================================================

fn default_gpu_enabled() -> bool {
    true
}

fn default_device_ids() -> Vec<u32> {
    vec![0]
}

fn default_memory_fraction() -> f32 {
    0.9
}

fn default_use_cuda_graphs() -> bool {
    true
}

fn default_mixed_precision() -> bool {
    true
}

// ============================================================================
// GPU CONFIG
// ============================================================================

/// Configuration for GPU usage.
///
/// Target hardware: RTX 5090 (32GB GDDR7, Compute 12.0, CUDA 13.2)
///
/// # Key Features
/// - Green Contexts: Static SM partitioning for deterministic latency
/// - Mixed Precision: FP16/BF16 for 2x throughput
/// - CUDA Graphs: Kernel fusion for reduced launch overhead
/// - GPU Direct Storage: 25+ GB/s model loading vs ~6 GB/s via CPU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    /// Whether GPU acceleration is enabled.
    /// Default: true
    #[serde(default = "default_gpu_enabled")]
    pub enabled: bool,

    /// CUDA device IDs to use.
    /// Empty means auto-select first available device.
    /// Default: `[0]`
    #[serde(default = "default_device_ids")]
    pub device_ids: Vec<u32>,

    /// Fraction of GPU memory to use (0.0-1.0].
    /// Constitution spec: <24GB of 32GB = 0.75 max, default 0.9
    /// Reserve 10% for other operations.
    /// Default: 0.9
    #[serde(default = "default_memory_fraction")]
    pub memory_fraction: f32,

    /// Use CUDA graphs for kernel fusion.
    /// Reduces kernel launch overhead.
    /// Default: true
    #[serde(default = "default_use_cuda_graphs")]
    pub use_cuda_graphs: bool,

    /// Enable mixed precision (FP16/BF16) inference.
    /// Provides 2x throughput with minimal accuracy loss.
    /// Default: true
    #[serde(default = "default_mixed_precision")]
    pub mixed_precision: bool,

    /// Use CUDA 13.2 green contexts for power efficiency.
    /// Provides static SM partitioning for deterministic latency.
    /// Requires: CUDA 13.2+, Blackwell architecture (Compute 12.0)
    /// Default: false (requires explicit opt-in)
    #[serde(default)]
    pub green_contexts: bool,

    /// Whether to enable GPU Direct Storage (GDS) for fast model loading.
    /// Provides 25+ GB/s vs ~6 GB/s via CPU path.
    /// Requires: GDS driver, NVMe SSD
    /// Default: false
    #[serde(default)]
    pub gds_enabled: bool,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            device_ids: vec![0],
            memory_fraction: 0.9,
            use_cuda_graphs: true,
            mixed_precision: true,
            green_contexts: false,
            gds_enabled: false,
        }
    }
}

impl GpuConfig {
    /// Validate GPU configuration.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if:
    /// - enabled && device_ids.is_empty()
    /// - memory_fraction <= 0.0 or > 1.0 or is NaN
    pub fn validate(&self) -> EmbeddingResult<()> {
        if self.enabled && self.device_ids.is_empty() {
            return Err(EmbeddingError::ConfigError {
                message: "device_ids cannot be empty when GPU enabled".to_string(),
            });
        }
        if self.memory_fraction <= 0.0
            || self.memory_fraction > 1.0
            || self.memory_fraction.is_nan()
        {
            return Err(EmbeddingError::ConfigError {
                message: format!(
                    "memory_fraction must be in (0.0, 1.0], got {}",
                    self.memory_fraction
                ),
            });
        }
        Ok(())
    }

    /// Check if this config uses GPU acceleration.
    pub fn is_gpu_enabled(&self) -> bool {
        self.enabled && !self.device_ids.is_empty()
    }
}
