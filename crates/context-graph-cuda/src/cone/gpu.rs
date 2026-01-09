//! GPU functions for cone operations.
//!
//! Contains GPU availability checking and CUDA kernel wrappers.
//!
//! # CUDA Required
//!
//! This module requires CUDA support (RTX 5090 / Blackwell).
//! There are NO fallback stubs - the system will fail fast if CUDA is unavailable.

use std::ffi::c_void;

use super::config::ConeCudaConfig;
use super::ffi;
use super::types::ConeKernelInfo;
use crate::error::{CudaError, CudaResult};

// ============================================================================
// Kernel Info
// ============================================================================

/// Get kernel configuration info.
///
/// Returns None if CUDA feature is disabled.
///
/// # Example
///
/// ```
/// use context_graph_cuda::cone::get_cone_kernel_info;
///
/// if let Some(info) = get_cone_kernel_info() {
///     println!("Block size: {}x{}", info.block_dim_x, info.block_dim_y);
///     println!("Shared memory: {} bytes", info.shared_mem_bytes);
/// }
/// ```
pub fn get_cone_kernel_info() -> Option<ConeKernelInfo> {
    let mut block_dim_x: i32 = 0;
    let mut block_dim_y: i32 = 0;
    let mut point_dim: i32 = 0;
    let mut cone_data_dim: i32 = 0;
    let mut shared_mem: i32 = 0;

    unsafe {
        ffi::get_cone_kernel_config(
            &mut block_dim_x,
            &mut block_dim_y,
            &mut point_dim,
            &mut cone_data_dim,
            &mut shared_mem,
        );
    }

    Some(ConeKernelInfo {
        block_dim_x,
        block_dim_y,
        point_dim,
        cone_data_dim,
        shared_mem_bytes: shared_mem,
    })
}

// ============================================================================
// GPU Availability
// ============================================================================

/// Check if CUDA Cone kernels are available.
///
/// Returns true if:
/// 1. The `cuda` feature is enabled at compile time
/// 2. The system has a CUDA-capable GPU
/// 3. The CUDA driver is available and initialized
///
/// # Implementation Note
///
/// Uses CUDA Driver API (cuInit, cuDeviceGetCount) instead of Runtime API
/// (cudaGetDeviceCount) because Runtime API segfaults in CUDA 13.1 on WSL2
/// with RTX 5090 (Blackwell) GPUs. This is a known NVIDIA driver bug.
///
/// # Example
///
/// ```
/// use context_graph_cuda::cone::is_cone_gpu_available;
///
/// if is_cone_gpu_available() {
///     println!("GPU acceleration available!");
/// } else {
///     panic!("RTX 5090 required but CUDA unavailable");
/// }
/// ```
pub fn is_cone_gpu_available() -> bool {
    // Use CUDA Driver API instead of Runtime API to avoid CUDA 13.1 WSL2 segfault bug
    // Reference: cudaGetDeviceCount (Runtime) segfaults, cuDeviceGetCount (Driver) works
    extern "C" {
        /// Initialize the CUDA driver (Driver API)
        fn cuInit(flags: std::os::raw::c_uint) -> i32;
        /// Get CUDA device count (Driver API)
        fn cuDeviceGetCount(count: *mut i32) -> i32;
    }

    unsafe {
        // Initialize CUDA driver first (required before any Driver API call)
        let init_result = cuInit(0);
        if init_result != 0 {
            tracing::debug!("cuInit failed with error code {}", init_result);
            return false;
        }

        // Get device count using Driver API
        let mut device_count: i32 = 0;
        let result = cuDeviceGetCount(&mut device_count);
        // CUDA_SUCCESS = 0
        if result != 0 {
            tracing::debug!("cuDeviceGetCount failed with error code {}", result);
            return false;
        }

        device_count > 0
    }
}

// ============================================================================
// GPU Functions
// ============================================================================

/// Compute batch cone membership scores on GPU.
///
/// # Safety
///
/// - `d_cones`, `d_points`, `d_scores` must be valid device pointers
/// - Arrays must be properly sized: cones\[n_cones\]\[65\], points\[n_points\]\[64\]
/// - Output scores\[n_cones\]\[n_points\] must be pre-allocated
/// - Pointers must be aligned for float32 access
///
/// # Arguments
///
/// * `d_cones` - Device pointer to cone data \[n_cones\]\[65\]
/// * `d_points` - Device pointer to point vectors \[n_points\]\[64\]
/// * `d_scores` - Device pointer to output matrix \[n_cones\]\[n_points\]
/// * `n_cones` - Number of cones
/// * `n_points` - Number of points
/// * `config` - Cone configuration
/// * `stream` - CUDA stream (None for default stream)
///
/// # Errors
///
/// Returns `CudaError` if:
/// - Configuration is invalid
/// - Kernel launch fails
/// - CUDA runtime error occurs
///
/// # Performance
///
/// Target: <2ms for 1K x 1K membership matrix on RTX 5090.
pub unsafe fn cone_check_batch_gpu(
    d_cones: *const f32,
    d_points: *const f32,
    d_scores: *mut f32,
    n_cones: usize,
    n_points: usize,
    config: &ConeCudaConfig,
    stream: Option<*mut c_void>,
) -> CudaResult<()> {
    // Validate configuration
    config.validate()?;

    // Validate pointer arguments
    if d_cones.is_null() {
        return Err(CudaError::InvalidConfig(
            "d_cones pointer is null".to_string(),
        ));
    }
    if d_points.is_null() {
        return Err(CudaError::InvalidConfig(
            "d_points pointer is null".to_string(),
        ));
    }
    if d_scores.is_null() {
        return Err(CudaError::InvalidConfig(
            "d_scores pointer is null".to_string(),
        ));
    }

    // Validate sizes
    if n_cones == 0 || n_points == 0 {
        return Err(CudaError::InvalidConfig(
            "n_cones and n_points must be positive".to_string(),
        ));
    }

    let stream_ptr = stream.unwrap_or(std::ptr::null_mut());

    let result = ffi::launch_cone_check(
        d_cones,
        d_points,
        d_scores,
        n_cones as i32,
        n_points as i32,
        config.curvature,
        stream_ptr,
    );

    if result != 0 {
        return Err(CudaError::KernelError(format!(
            "Cone check kernel failed with CUDA error code {}",
            result
        )));
    }

    Ok(())
}

/// Compute single cone membership score on GPU.
///
/// # Safety
///
/// Same requirements as `cone_check_batch_gpu`.
///
/// # Note
///
/// For efficiency, prefer `cone_check_batch_gpu` when computing
/// multiple scores. Single-pair calls have significant kernel launch overhead.
pub unsafe fn cone_check_single_gpu(
    d_cone: *const f32,
    d_point: *const f32,
    d_score: *mut f32,
    config: &ConeCudaConfig,
    stream: Option<*mut c_void>,
) -> CudaResult<()> {
    config.validate()?;

    if d_cone.is_null() || d_point.is_null() || d_score.is_null() {
        return Err(CudaError::InvalidConfig(
            "Device pointers cannot be null".to_string(),
        ));
    }

    let stream_ptr = stream.unwrap_or(std::ptr::null_mut());

    let result = ffi::cone_check_single(d_cone, d_point, d_score, config.curvature, stream_ptr);

    if result != 0 {
        return Err(CudaError::KernelError(format!(
            "Cone check single kernel failed with CUDA error code {}",
            result
        )));
    }

    Ok(())
}
