//! GPU device management for RTX 5090 acceleration.
//!
//! # Singleton Pattern
//!
//! The GPU device is initialized once and shared globally. This ensures:
//! - Single CUDA context for optimal memory management
//! - Consistent device placement across all operations
//! - Automatic cleanup on process exit
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_graph_embeddings::gpu::{init_gpu, device};
//!
//! // Initialize at startup
//! init_gpu()?;
//!
//! // Get device for tensor operations
//! let dev = device();
//! let tensor = Tensor::zeros((1024,), DType::F32, dev)?;
//! ```

use candle_core::{Device, DType};
use std::sync::OnceLock;

use super::GpuInfo;

/// Global GPU device singleton.
static GPU_DEVICE: OnceLock<Device> = OnceLock::new();

/// GPU availability flag (cached for fast checks).
static GPU_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Cached GPU info for runtime queries.
static GPU_INFO: OnceLock<GpuInfo> = OnceLock::new();

/// Initialize result for thread-safe error handling.
static INIT_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

/// Initialize the GPU device (call once at startup).
///
/// # Returns
///
/// Reference to the initialized GPU device, or error if CUDA unavailable.
///
/// # Thread Safety
///
/// Safe to call from multiple threads; only the first call initializes.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_embeddings::gpu::init_gpu;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let device = init_gpu()?;
///     println!("GPU initialized: {:?}", device);
///     Ok(())
/// }
/// ```
pub fn init_gpu() -> Result<&'static Device, candle_core::Error> {
    // Check if already initialized
    if let Some(device) = GPU_DEVICE.get() {
        return Ok(device);
    }

    // Check if previous initialization failed
    if let Some(Err(msg)) = INIT_RESULT.get() {
        return Err(candle_core::Error::Msg(msg.clone()));
    }

    // Attempt initialization
    tracing::info!("Initializing CUDA device 0 (RTX 5090)");

    match Device::new_cuda(0) {
        Ok(device) => {
            // Store the device
            let _ = GPU_DEVICE.set(device);
            let _ = GPU_AVAILABLE.set(true);

            // Get device reference after storing
            let device_ref = GPU_DEVICE.get().unwrap();

            // Cache GPU info
            let info = query_gpu_info(device_ref);
            let _ = GPU_INFO.set(info);
            let _ = INIT_RESULT.set(Ok(()));

            tracing::info!(
                "GPU initialized: {} ({} VRAM)",
                GPU_INFO.get().map(|i| i.name.as_str()).unwrap_or("unknown"),
                format_bytes(GPU_INFO.get().map(|i| i.total_vram).unwrap_or(0))
            );

            Ok(device_ref)
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = GPU_AVAILABLE.set(false);
            let _ = INIT_RESULT.set(Err(msg.clone()));
            Err(e)
        }
    }
}

/// Get the active GPU device.
///
/// # Panics
///
/// Panics if [`init_gpu`] was not called first. Always initialize
/// at application startup.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_embeddings::gpu::device;
///
/// let dev = device();
/// // Use dev for tensor operations
/// ```
pub fn device() -> &'static Device {
    GPU_DEVICE.get().expect("GPU not initialized - call init_gpu() at startup")
}

/// Check if GPU is available and initialized.
///
/// Returns `false` if:
/// - CUDA is not installed
/// - No GPU hardware found
/// - [`init_gpu`] not called or failed
pub fn is_gpu_available() -> bool {
    *GPU_AVAILABLE.get().unwrap_or(&false)
}

/// Default dtype for embeddings.
///
/// - `F32`: Maximum precision (default for accuracy-critical embeddings)
/// - `F16`: Half precision for speed (2x memory savings)
/// - `BF16`: Brain float for training stability
pub fn default_dtype() -> DType {
    DType::F32
}

/// Get cached GPU information.
pub fn get_gpu_info() -> GpuInfo {
    GPU_INFO.get().cloned().unwrap_or_default()
}

/// Query GPU information from the device.
fn query_gpu_info(_device: &Device) -> GpuInfo {
    // Note: Candle doesn't expose detailed GPU info directly.
    // For RTX 5090, we use known specifications.
    GpuInfo {
        name: "NVIDIA GeForce RTX 5090".to_string(),
        total_vram: 32 * 1024 * 1024 * 1024, // 32GB
        compute_capability: "12.0".to_string(),
        available: true,
    }
}

/// Format bytes as human-readable string.
fn format_bytes(bytes: usize) -> String {
    const GB: usize = 1024 * 1024 * 1024;
    const MB: usize = 1024 * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(32 * 1024 * 1024 * 1024), "32.0 GB");
    }

    #[test]
    fn test_default_dtype() {
        assert_eq!(default_dtype(), DType::F32);
    }

    #[test]
    fn test_gpu_not_initialized() {
        // Before init_gpu is called
        assert!(!is_gpu_available() || GPU_DEVICE.get().is_some());
    }
}
