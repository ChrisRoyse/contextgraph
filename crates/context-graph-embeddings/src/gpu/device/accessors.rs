//! GPU device accessor functions.
//!
//! Provides thread-safe access to the initialized GPU device and related
//! information. All functions in this module assume the GPU-only architecture
//! where no CPU fallback is available.

use candle_core::{DType, Device};

use super::core::{GPU_AVAILABLE, GPU_DEVICE, GPU_INFO};
use crate::gpu::GpuInfo;

/// Get the active GPU device.
///
/// # GPU-Only Requirement
///
/// This function returns the initialized CUDA GPU device. There is NO CPU
/// fallback. If the GPU was not initialized or initialization failed, this
/// function will panic.
///
/// # Panics
///
/// Panics if:
/// - [`super::init_gpu`] was not called first
/// - [`super::init_gpu`] was called but failed (no GPU available)
///
/// Always call [`super::init_gpu`] at application startup and handle errors there.
/// Do not catch the panic from this function - fix the initialization instead.
///
/// # Example
///
/// ```
/// use context_graph_embeddings::gpu::{init_gpu, device};
///
/// // GPU is available - RTX 5090 with CUDA 13.1
/// init_gpu().expect("GPU required");
///
/// // Now device() is safe to call
/// let dev = device();
/// assert!(dev.is_cuda());
/// ```
pub fn device() -> &'static Device {
    GPU_DEVICE.get().expect(
        "GPU not initialized - call init_gpu() at startup. \
         This crate requires a CUDA-capable GPU with no CPU fallback.",
    )
}

/// Check if GPU is available and initialized.
///
/// # GPU-Only Architecture
///
/// This function checks if the GPU has been successfully initialized.
/// In the GPU-only architecture, this function returning `false` indicates
/// a critical failure state - the crate cannot function without GPU.
///
/// Returns `false` if:
/// - [`super::init_gpu`] was not called yet
/// - [`super::init_gpu`] was called but CUDA initialization failed
/// - No CUDA-capable GPU hardware found
/// - CUDA drivers not installed or incompatible
///
/// Returns `true` if:
/// - [`super::init_gpu`] was called and CUDA device 0 was successfully initialized
///
/// # Note
///
/// This function is primarily for diagnostic purposes. Application code
/// should call [`super::init_gpu`] and handle the error rather than checking
/// availability first.
pub fn is_gpu_available() -> bool {
    *GPU_AVAILABLE.get().unwrap_or(&false)
}

/// Default dtype for GPU embeddings.
///
/// Returns `F32` for maximum precision, which is optimal for:
/// - Accuracy-critical embedding comparisons
/// - RTX 5090's excellent F32 tensor core performance
///
/// # Alternative DTypes (use with caution)
///
/// - `F16`: Half precision for 2x memory savings (may reduce accuracy)
/// - `BF16`: Brain float for training stability (requires Ampere+)
///
/// # RTX 5090 Optimization
///
/// The RTX 5090 Blackwell architecture has excellent F32 performance,
/// so F32 is preferred over F16 unless memory-constrained.
pub fn default_dtype() -> DType {
    DType::F32
}

/// Get cached GPU information.
///
/// Returns information about the initialized GPU device, including:
/// - Device name (e.g., "NVIDIA GeForce RTX 5090")
/// - Total VRAM in bytes
/// - Compute capability (e.g., "12.0")
/// - Availability status
///
/// # Returns
///
/// Returns cached [`GpuInfo`] if GPU was initialized, or a default
/// "No GPU" info struct if [`super::init_gpu`] was not called or failed.
///
/// # Note
///
/// The returned info is cached at initialization time and does not
/// reflect real-time VRAM usage. Use `nvidia-smi` for live memory stats.
pub fn get_gpu_info() -> GpuInfo {
    GPU_INFO.get().cloned().unwrap_or_default()
}

/// Require GPU to be available, returning an error if not.
///
/// This is a convenience function that combines [`super::init_gpu`] with error
/// transformation to return a structured error type.
///
/// # Usage
///
/// ```
/// use context_graph_embeddings::gpu::require_gpu;
/// use context_graph_embeddings::error::EmbeddingError;
///
/// fn run_embeddings() -> Result<(), EmbeddingError> {
///     require_gpu()?;  // RTX 5090 with CUDA 13.1 is available
///     Ok(())
/// }
/// # run_embeddings().unwrap();
/// ```
pub fn require_gpu() -> Result<&'static Device, crate::error::EmbeddingError> {
    super::init_gpu().map_err(|e| crate::error::EmbeddingError::GpuError {
        message: format!(
            "GPU initialization failed: {}. This crate requires a CUDA-capable GPU (target: RTX 5090). \
             No CPU fallback is available.",
            e
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_dtype_is_f32() {
        // F32 is the optimal dtype for RTX 5090 embeddings
        assert_eq!(default_dtype(), DType::F32);
    }

    #[test]
    fn test_gpu_info_default() {
        // GpuInfo::default() should indicate no GPU
        let info = GpuInfo::default();
        assert_eq!(info.name, "No GPU");
        assert_eq!(info.total_vram, 0);
        assert!(!info.available);
    }

    #[test]
    fn test_device_returns_cuda_device_after_init() {
        let _ = crate::gpu::init_gpu();
        let dev = device();
        // Device should be CUDA, not CPU
        assert!(dev.is_cuda(), "Device must be CUDA, not CPU");
    }

    #[test]
    fn test_gpu_info_after_init() {
        let _ = crate::gpu::init_gpu();
        let info = get_gpu_info();
        assert!(info.available);
        assert!(!info.name.is_empty());
        assert!(info.total_vram > 0);
    }

    #[test]
    fn test_require_gpu_returns_device() {
        let result = require_gpu();
        assert!(result.is_ok());
        let dev = result.unwrap();
        assert!(dev.is_cuda());
    }
}
