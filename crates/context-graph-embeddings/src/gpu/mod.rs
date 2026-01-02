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
//! # Feature Gating
//!
//! All GPU functionality requires the `candle` feature:
//! ```toml
//! [dependencies]
//! context-graph-embeddings = { version = "0.1", features = ["cuda"] }
//! ```

#[cfg(feature = "candle")]
mod device;
#[cfg(feature = "candle")]
mod memory;
#[cfg(feature = "candle")]
mod ops;
#[cfg(feature = "candle")]
mod tensor;

#[cfg(feature = "candle")]
pub use device::{init_gpu, device, default_dtype, is_gpu_available, get_gpu_info};
#[cfg(feature = "candle")]
pub use memory::{GpuMemoryPool, MemoryStats, VramTracker};
#[cfg(feature = "candle")]
pub use ops::{l2_norm_gpu, normalize_gpu, cosine_similarity_gpu, matmul_gpu, softmax_gpu};
#[cfg(feature = "candle")]
pub use tensor::GpuTensor;

/// Stub exports when candle feature is not enabled.
/// These allow compile-time checking without GPU hardware.
#[cfg(not(feature = "candle"))]
pub fn is_gpu_available() -> bool {
    false
}

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
#[cfg(feature = "candle")]
pub fn gpu_info() -> GpuInfo {
    device::get_gpu_info()
}

#[cfg(not(feature = "candle"))]
pub fn gpu_info() -> GpuInfo {
    GpuInfo::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_info_stub() {
        #[cfg(not(feature = "candle"))]
        {
            let info = gpu_info();
            assert!(!info.available);
            assert_eq!(info.name, "No GPU");
        }
    }
}
