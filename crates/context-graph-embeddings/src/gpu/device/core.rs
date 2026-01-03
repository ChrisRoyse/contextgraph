//! Core GPU device initialization for RTX 5090 acceleration.
//!
//! # GPU-ONLY Architecture
//!
//! This module is **strictly GPU-only** with NO CPU fallback. If a CUDA-capable
//! GPU is not available, initialization will fail with a clear error.
//!
//! # Requirements
//!
//! - **Hardware**: NVIDIA CUDA-capable GPU (target: RTX 5090 / Blackwell GB202)
//! - **Driver**: CUDA 13.1+ with compatible NVIDIA drivers
//! - **Memory**: Minimum 16GB VRAM recommended (32GB for RTX 5090)
//!
//! # Singleton Pattern
//!
//! The GPU device is initialized once and shared globally. This ensures:
//! - Single CUDA context for optimal memory management
//! - Consistent device placement across all operations
//! - Automatic cleanup on process exit

use candle_core::Device;
use std::sync::OnceLock;

use crate::gpu::GpuInfo;
use super::utils::query_gpu_info;

/// Global GPU device singleton.
pub(crate) static GPU_DEVICE: OnceLock<Device> = OnceLock::new();

/// GPU availability flag (cached for fast checks).
pub(crate) static GPU_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Cached GPU info for runtime queries.
pub(crate) static GPU_INFO: OnceLock<GpuInfo> = OnceLock::new();

/// Initialize result for thread-safe error handling.
pub(crate) static INIT_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

/// Initialize the GPU device (call once at startup).
///
/// # GPU-Only Requirement
///
/// This function **requires** a CUDA-capable GPU. There is NO CPU fallback.
/// If no GPU is available, this function returns an error with detailed
/// diagnostic information.
///
/// # Target Hardware
///
/// - Primary target: NVIDIA RTX 5090 (Blackwell GB202, 32GB VRAM)
/// - Minimum requirement: Any CUDA-capable GPU with compute capability 6.0+
/// - Required driver: CUDA 13.1+ recommended
///
/// # Returns
///
/// Reference to the initialized GPU device, or error if CUDA unavailable.
///
/// # Errors
///
/// Returns [`candle_core::Error`] if:
/// - No CUDA-capable GPU is detected
/// - CUDA drivers are not installed or incompatible
/// - GPU is in use by another process with exclusive access
/// - Insufficient GPU memory for initialization
///
/// # Thread Safety
///
/// Safe to call from multiple threads; only the first call initializes.
///
/// # Example
///
/// ```
/// use context_graph_embeddings::gpu::init_gpu;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // GPU is available - RTX 5090 with CUDA 13.1
///     let device = init_gpu()?;
///     println!("GPU initialized: {:?}", device);
///     Ok(())
/// }
/// ```
pub fn init_gpu() -> Result<&'static Device, candle_core::Error> {
    // Check if already initialized
    if let Some(device) = GPU_DEVICE.get() {
        tracing::debug!("GPU already initialized, returning cached device");
        return Ok(device);
    }

    // Check if previous initialization failed
    if let Some(Err(msg)) = INIT_RESULT.get() {
        tracing::error!("GPU initialization previously failed: {}", msg);
        return Err(candle_core::Error::Msg(msg.clone()));
    }

    // Log initialization attempt with full context
    tracing::info!("=== GPU Initialization Starting ===");
    tracing::info!("Target hardware: NVIDIA RTX 5090 / Blackwell GB202");
    tracing::info!("Target CUDA version: 13.1+");
    tracing::info!("Attempting CUDA device 0 initialization...");

    match Device::new_cuda(0) {
        Ok(device) => {
            init_success(device)
        }
        Err(e) => {
            init_failure(e)
        }
    }
}

/// Handle successful GPU initialization.
fn init_success(device: Device) -> Result<&'static Device, candle_core::Error> {
    // Store the device
    let _ = GPU_DEVICE.set(device);
    let _ = GPU_AVAILABLE.set(true);

    // Get device reference after storing
    let device_ref = GPU_DEVICE.get().unwrap();

    // Cache GPU info
    let info = query_gpu_info(device_ref);
    let _ = GPU_INFO.set(info.clone());
    let _ = INIT_RESULT.set(Ok(()));

    // Log success with comprehensive details
    tracing::info!("=== GPU Initialization SUCCESS ===");
    tracing::info!("  Device: {}", info.name);
    tracing::info!("  VRAM: {}", super::utils::format_bytes(info.total_vram));
    tracing::info!("  Compute Capability: {}", info.compute_capability);
    tracing::info!("  Status: Ready for tensor operations");

    Ok(device_ref)
}

/// Handle GPU initialization failure with detailed error logging.
fn init_failure(e: candle_core::Error) -> Result<&'static Device, candle_core::Error> {
    let msg = e.to_string();
    let _ = GPU_AVAILABLE.set(false);
    let _ = INIT_RESULT.set(Err(msg.clone()));

    // ROBUST ERROR LOGGING - provide actionable information
    tracing::error!("=== GPU Initialization FAILED ===");
    tracing::error!("Error: {}", msg);
    tracing::error!("");
    tracing::error!("This crate REQUIRES a CUDA-capable GPU. NO CPU FALLBACK.");
    tracing::error!("");
    tracing::error!("Troubleshooting steps:");
    tracing::error!("  1. Verify NVIDIA GPU is present: nvidia-smi");
    tracing::error!("  2. Check CUDA installation: nvcc --version");
    tracing::error!("  3. Verify driver compatibility with CUDA 13.1+");
    tracing::error!("  4. Ensure GPU is not in exclusive compute mode");
    tracing::error!(
        "  5. Check available GPU memory: nvidia-smi --query-gpu=memory.free --format=csv"
    );
    tracing::error!("");
    tracing::error!("Target hardware: RTX 5090 (32GB VRAM, Compute 12.0)");
    tracing::error!("Minimum hardware: Any CUDA GPU with Compute 6.0+");

    Err(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_gpu_available_before_init() {
        // Before init_gpu is called, GPU should not be available
        // (or it was already initialized by another test)
        let available = GPU_AVAILABLE.get().copied().unwrap_or(false);
        let initialized = GPU_DEVICE.get().is_some();
        // Either not available and not initialized, or both are true
        assert!(available == initialized || !available);
    }

    #[test]
    fn test_init_gpu_succeeds_on_cuda_hardware() {
        let result = init_gpu();
        assert!(result.is_ok(), "GPU init should succeed on CUDA hardware");
        assert!(*GPU_AVAILABLE.get().unwrap_or(&false));
    }
}
