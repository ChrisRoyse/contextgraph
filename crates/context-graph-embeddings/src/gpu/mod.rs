//! GPU acceleration module for RTX 5090 (Blackwell GB202).
//!
//! # Architecture
//!
//! This module provides GPU-accelerated operations via Candle (HuggingFace).
//! Target hardware: NVIDIA RTX 5090 32GB with CUDA 13.1
//!
//! | Component | Description |
//! |-----------|-------------|
//! | [`GpuDevice`] | Singleton device manager with automatic CUDA detection |
//! | [`GpuTensor`] | Type-safe tensor wrapper with automatic device placement |
//! | [`ops`] | GPU-accelerated operations (L2 norm, cosine similarity, matmul) |
//! | [`memory`] | VRAM tracking and memory pool management |
//!
//! # Requirements
//!
//! GPU functionality requires CUDA hardware and the candle backend (now mandatory).

mod device;
mod memory;
mod model_loader;
mod ops;
mod tensor;

pub use device::{default_dtype, device, get_gpu_info, init_gpu, is_gpu_available, require_gpu};
pub use memory::{GpuMemoryPool, MemoryBudget, MemoryError, MemoryStats, VramTracker};
pub use model_loader::{
    AttentionWeights, BertConfig, BertWeights, EmbeddingWeights, EncoderLayerWeights, FfnWeights,
    GpuModelLoader, ModelLoadError, PoolerWeights,
};
pub use ops::{
    cosine_similarity_gpu, l2_norm_gpu, matmul_gpu, normalize_gpu, softmax_gpu,
    softmax_with_temperature_gpu,
};
pub use tensor::GpuTensor;

/// GPU device information for runtime queries.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPU device name (e.g., "NVIDIA GeForce RTX 5090")
    pub name: String,
    /// Total VRAM in bytes
    pub total_vram: usize,
    /// CUDA compute capability (e.g., "12.0")
    pub compute_capability: String,
    /// Whether the GPU is available and initialized
    pub available: bool,
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            name: "No GPU".to_string(),
            total_vram: 0,
            compute_capability: "0.0".to_string(),
            available: false,
        }
    }
}

/// Get GPU information at runtime.
pub fn gpu_info() -> GpuInfo {
    device::get_gpu_info()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_info() {
        let info = gpu_info();
        // GPU may or may not be available depending on hardware
        // but the function should always return valid info
        assert!(!info.name.is_empty());
    }
}
