//! Pre-flight checks and CUDA initialization for warm model loading.
//!
//! CRITICAL: This module has NO stub mode. CUDA is REQUIRED.
//! Constitution Reference: v4.0.0, stack.gpu, AP-007

// CRITICAL: CUDA feature is REQUIRED. No stub mode.
#[cfg(not(feature = "cuda"))]
compile_error!(
    "[EMB-E001] CUDA_UNAVAILABLE: The 'cuda' feature MUST be enabled.

    Context Graph embeddings require GPU acceleration.
    There is NO CPU fallback and NO stub mode.

    Target Hardware (Constitution v4.0.0):
    - GPU: RTX 5090 (Blackwell architecture)
    - CUDA: 13.1+
    - VRAM: 32GB minimum

    Remediation:
    1. Install CUDA 13.1+
    2. Ensure RTX 5090 or compatible GPU is available
    3. Build with: cargo build --features cuda

    Constitution Reference: stack.gpu, AP-007
    Exit Code: 101 (CUDA_UNAVAILABLE)"
);

#[cfg(feature = "cuda")]
use super::constants::{GB, MODEL_SIZES};
#[cfg(feature = "cuda")]
use super::helpers::format_bytes;
#[cfg(feature = "cuda")]
use crate::warm::config::WarmConfig;
#[cfg(feature = "cuda")]
use crate::warm::cuda_alloc::{
    GpuInfo, WarmCudaAllocator, MINIMUM_VRAM_BYTES, REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR,
};
#[cfg(feature = "cuda")]
use crate::warm::error::{WarmError, WarmResult};

/// Run pre-flight checks before loading.
///
/// Verifies:
/// - GPU meets compute capability requirements (12.0+ for RTX 5090)
/// - Sufficient VRAM available (32GB)
/// - CUDA context is valid
/// - GPU is NOT simulated/stub
///
/// # Errors
///
/// Returns `WarmError::CudaUnavailable` with EMB-E001 if:
/// - No CUDA device found
/// - GPU is simulated/stub (contains "Simulated" or "Stub" in name)
///
/// Returns `WarmError::CudaCapabilityInsufficient` with EMB-E002 if:
/// - Compute capability below 12.0
///
/// Returns `WarmError::VramInsufficientTotal` with EMB-E003 if:
/// - Total VRAM below 32GB
#[cfg(feature = "cuda")]
pub fn run_preflight_checks(config: &WarmConfig, gpu_info: &mut Option<GpuInfo>) -> WarmResult<()> {
    tracing::info!("Running pre-flight checks...");

    // Try to create a temporary allocator to query GPU info
    let allocator = WarmCudaAllocator::new(config.cuda_device_id)?;
    let info = allocator.get_gpu_info()?;

    // CRITICAL: Verify this is NOT a simulated GPU
    let name_lower = info.name.to_lowercase();
    if name_lower.contains("simulated")
        || name_lower.contains("stub")
        || name_lower.contains("fake")
    {
        tracing::error!(
            "[EMB-E001] DETECTED SIMULATED GPU: '{}' - Real GPU required!",
            info.name
        );
        return Err(WarmError::CudaUnavailable {
            message: format!(
                "[EMB-E001] CUDA_UNAVAILABLE: Detected simulated/stub GPU: '{}'. \
                 Real RTX 5090 (Blackwell, CC 12.0) required. \
                 Constitution Reference: stack.gpu, AP-007",
                info.name
            ),
        });
    }

    tracing::info!(
        "GPU detected: {} (CC {}, {} VRAM)",
        info.name,
        info.compute_capability_string(),
        format_bytes(info.total_memory_bytes)
    );

    // Check compute capability
    if !info.meets_compute_requirement(REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR) {
        tracing::error!(
            "[EMB-E002] GPU compute capability {}.{} below required {}.{}",
            info.compute_capability.0,
            info.compute_capability.1,
            REQUIRED_COMPUTE_MAJOR,
            REQUIRED_COMPUTE_MINOR
        );
        return Err(WarmError::CudaCapabilityInsufficient {
            actual_cc: info.compute_capability_string(),
            required_cc: format!("{}.{}", REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR),
            gpu_name: info.name.clone(),
        });
    }

    // Check VRAM
    if info.total_memory_bytes < MINIMUM_VRAM_BYTES {
        let required_gb = MINIMUM_VRAM_BYTES as f64 / GB as f64;
        let available_gb = info.total_memory_bytes as f64 / GB as f64;
        tracing::error!(
            "[EMB-E003] Insufficient VRAM: {:.1} GB available, {:.1} GB required",
            available_gb,
            required_gb
        );
        return Err(WarmError::VramInsufficientTotal {
            required_bytes: MINIMUM_VRAM_BYTES,
            available_bytes: info.total_memory_bytes,
            required_gb,
            available_gb,
            model_breakdown: MODEL_SIZES
                .iter()
                .map(|(id, size)| (id.to_string(), *size))
                .collect(),
        });
    }

    *gpu_info = Some(info);
    tracing::info!("Pre-flight checks passed - Real GPU verified");
    Ok(())
}

/// Initialize the CUDA allocator.
///
/// # Errors
///
/// Returns `WarmError::CudaUnavailable` if CUDA device cannot be initialized.
#[cfg(feature = "cuda")]
pub fn initialize_cuda_allocator(config: &WarmConfig) -> WarmResult<WarmCudaAllocator> {
    tracing::info!(
        "Initializing CUDA allocator for device {}",
        config.cuda_device_id
    );

    let allocator = WarmCudaAllocator::new(config.cuda_device_id)?;
    tracing::info!("CUDA allocator initialized successfully");
    Ok(allocator)
}
