//! GPU device utility functions.
//!
//! Internal utilities for GPU information queries and formatting.
//!
//! # CUDA Driver API Usage
//!
//! This module uses the CUDA Driver API (NOT Runtime API) for device queries.
//! This is required because the Runtime API (cudaGetDeviceProperties) segfaults
//! in CUDA 13.1 on WSL2 with RTX 5090 (Blackwell) GPUs.
//!
//! # References
//!
//! - NVIDIA CUDA Pro Tip: https://developer.nvidia.com/blog/cuda-pro-tip-the-fast-way-to-query-device-properties/
//! - cudaDeviceGetAttribute is orders of magnitude faster than cudaGetDeviceProperties

use candle_core::Device;

use crate::gpu::GpuInfo;

// CUDA error codes
const CUDA_SUCCESS: i32 = 0;

// CUDA device attribute constants (from cuda.h / CUDA Driver API)
// See: https://docs.nvidia.com/cuda/cuda-driver-api/group__CUDA__DEVICE.html
const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR: i32 = 75;
const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR: i32 = 76;

// FFI bindings to CUDA Driver API
extern "C" {
    /// Initialize the CUDA driver (Driver API).
    /// Must be called before any other Driver API call.
    fn cuInit(flags: std::os::raw::c_uint) -> i32;

    /// Get a CUDA device handle by ordinal.
    fn cuDeviceGet(device: *mut i32, ordinal: i32) -> i32;

    /// Get the device name as a null-terminated string.
    /// max_len includes the null terminator.
    fn cuDeviceGetName(name: *mut std::os::raw::c_char, len: i32, dev: i32) -> i32;

    /// Get total memory on the device in bytes.
    fn cuDeviceTotalMem_v2(bytes: *mut usize, dev: i32) -> i32;

    /// Get a specific device attribute value.
    fn cuDeviceGetAttribute(value: *mut i32, attrib: i32, dev: i32) -> i32;

    /// Get the CUDA driver version.
    /// Returns version as (major * 1000 + minor * 10).
    fn cuDriverGetVersion(version: *mut i32) -> i32;
}

/// Query REAL GPU information using CUDA Driver API.
///
/// This function queries actual hardware properties. NO hardcoded fallbacks.
/// If any query fails, appropriate error information is logged.
///
/// # Implementation Notes
///
/// Uses CUDA Driver API instead of Runtime API to avoid CUDA 13.1 WSL2 segfault bug.
/// Uses `cuDeviceGetAttribute` instead of `cudaGetDeviceProperties` for performance
/// (nanoseconds vs milliseconds per NVIDIA recommendations).
///
/// # Arguments
///
/// * `_device` - Candle device reference (used for API consistency; ordinal 0 assumed)
///
/// # Returns
///
/// `GpuInfo` with real hardware properties queried from the GPU.
/// If queries fail, returns best-effort partial information with errors logged.
pub(crate) fn query_gpu_info(_device: &Device) -> GpuInfo {
    query_gpu_info_real(0)
}

/// Query GPU information for a specific device ordinal.
///
/// # Arguments
///
/// * `device_ordinal` - CUDA device ordinal (typically 0 for single-GPU systems)
///
/// # Returns
///
/// `GpuInfo` populated with real hardware values.
///
/// # Errors
///
/// Logs errors but returns partial information with available=true if device exists.
/// This ensures the system continues with best-effort info rather than failing entirely.
fn query_gpu_info_real(device_ordinal: u32) -> GpuInfo {
    // Default values in case of query failures (to detect partial success)
    let name;
    let mut total_vram: usize = 0;
    let mut compute_major: u32 = 0;
    let mut compute_minor: u32 = 0;
    let driver_version_str;
    let available;

    unsafe {
        // Step 1: Initialize CUDA driver
        let init_result = cuInit(0);
        if init_result != CUDA_SUCCESS {
            tracing::error!(
                target: "gpu::device",
                cuda_error_code = init_result,
                "cuInit failed - CUDA driver not initialized"
            );
            return GpuInfo {
                name: "CUDA Init Failed".to_string(),
                total_vram: 0,
                compute_capability: "0.0".to_string(),
                available: false,
            };
        }

        // Step 2: Get device handle
        let mut device_handle: i32 = 0;
        let get_result = cuDeviceGet(&mut device_handle, device_ordinal as i32);
        if get_result != CUDA_SUCCESS {
            tracing::error!(
                target: "gpu::device",
                cuda_error_code = get_result,
                device_ordinal = device_ordinal,
                "cuDeviceGet failed - no device at ordinal"
            );
            return GpuInfo {
                name: format!("Device {} Not Found", device_ordinal),
                total_vram: 0,
                compute_capability: "0.0".to_string(),
                available: false,
            };
        }

        // Device exists, mark as available (even if subsequent queries fail)
        available = true;

        // Step 3: Query device name
        let mut name_buf = [0i8; 256];
        let name_result = cuDeviceGetName(name_buf.as_mut_ptr(), 256, device_handle);
        if name_result == CUDA_SUCCESS {
            // Convert C string to Rust String
            let c_str = std::ffi::CStr::from_ptr(name_buf.as_ptr());
            name = c_str.to_string_lossy().into_owned();
            tracing::debug!(
                target: "gpu::device",
                device_name = %name,
                "GPU name queried successfully"
            );
        } else {
            tracing::warn!(
                target: "gpu::device",
                cuda_error_code = name_result,
                "cuDeviceGetName failed"
            );
            name = format!("CUDA Device {}", device_ordinal);
        }

        // Step 4: Query total memory
        let mem_result = cuDeviceTotalMem_v2(&mut total_vram, device_handle);
        if mem_result == CUDA_SUCCESS {
            tracing::debug!(
                target: "gpu::device",
                total_vram_bytes = total_vram,
                total_vram_gb = total_vram as f64 / (1024.0 * 1024.0 * 1024.0),
                "GPU total memory queried successfully"
            );
        } else {
            tracing::warn!(
                target: "gpu::device",
                cuda_error_code = mem_result,
                "cuDeviceTotalMem_v2 failed"
            );
        }

        // Step 5: Query compute capability (major)
        let mut cc_major: i32 = 0;
        let major_result = cuDeviceGetAttribute(
            &mut cc_major,
            CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
            device_handle,
        );
        if major_result == CUDA_SUCCESS {
            compute_major = cc_major as u32;
        } else {
            tracing::warn!(
                target: "gpu::device",
                cuda_error_code = major_result,
                "cuDeviceGetAttribute(COMPUTE_CAPABILITY_MAJOR) failed"
            );
        }

        // Step 6: Query compute capability (minor)
        let mut cc_minor: i32 = 0;
        let minor_result = cuDeviceGetAttribute(
            &mut cc_minor,
            CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
            device_handle,
        );
        if minor_result == CUDA_SUCCESS {
            compute_minor = cc_minor as u32;
        } else {
            tracing::warn!(
                target: "gpu::device",
                cuda_error_code = minor_result,
                "cuDeviceGetAttribute(COMPUTE_CAPABILITY_MINOR) failed"
            );
        }

        tracing::debug!(
            target: "gpu::device",
            compute_major = compute_major,
            compute_minor = compute_minor,
            "GPU compute capability queried"
        );

        // Step 7: Query driver version
        let mut driver_ver: i32 = 0;
        let driver_result = cuDriverGetVersion(&mut driver_ver);
        if driver_result == CUDA_SUCCESS {
            // Driver version is encoded as (major * 1000 + minor * 10)
            let major = driver_ver / 1000;
            let minor = (driver_ver % 1000) / 10;
            driver_version_str = format!("{}.{}", major, minor);
            tracing::debug!(
                target: "gpu::device",
                driver_version_raw = driver_ver,
                driver_version = %driver_version_str,
                "CUDA driver version queried"
            );
        } else {
            tracing::warn!(
                target: "gpu::device",
                cuda_error_code = driver_result,
                "cuDriverGetVersion failed"
            );
            driver_version_str = "Unknown".to_string();
        }
    }

    // Log comprehensive GPU info summary
    tracing::info!(
        target: "gpu::device",
        gpu_name = %name,
        total_vram_bytes = total_vram,
        total_vram_gb = format!("{:.1} GB", total_vram as f64 / (1024.0 * 1024.0 * 1024.0)),
        compute_capability = format!("{}.{}", compute_major, compute_minor),
        driver_version = %driver_version_str,
        "GPU information queried via CUDA Driver API"
    );

    GpuInfo {
        name,
        total_vram,
        compute_capability: format!("{}.{}", compute_major, compute_minor),
        available,
    }
}

/// Format bytes as human-readable string.
pub(crate) fn format_bytes(bytes: usize) -> String {
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
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 bytes");
    }

    #[test]
    fn test_format_bytes_megabytes() {
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(512 * 1024 * 1024), "512.0 MB");
    }

    #[test]
    fn test_format_bytes_gigabytes() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(32 * 1024 * 1024 * 1024), "32.0 GB");
    }

    #[test]
    fn test_format_bytes_small() {
        assert_eq!(format_bytes(100), "100 bytes");
        assert_eq!(format_bytes(1023), "1023 bytes");
    }
}
