//! CUDA Driver API FFI bindings.
//!
//! Low-level bindings to libcuda.so. These are the ONLY CUDA FFI
//! declarations in the entire codebase.
//!
//! # Why Driver API (not Runtime API)?
//!
//! CUDA 13.2 on WSL2 with RTX 5090 (Blackwell) has a bug where
//! cudaGetDeviceProperties (Runtime API) segfaults. The Driver API
//! (cuDeviceGetAttribute) works correctly and is also faster.
//!
//! Reference: NVIDIA Pro Tip - cuDeviceGetAttribute is orders of
//! magnitude faster than cudaGetDeviceProperties.

// FFI types match official CUDA API naming conventions
#![allow(non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_uint};

// =============================================================================
// TYPE ALIASES
// =============================================================================

/// CUDA result code. 0 = success, non-zero = error.
pub type CUresult = c_int;

/// CUDA device handle (ordinal-based).
pub type CUdevice = c_int;

/// CUDA device attribute enumeration.
pub type CUdevice_attribute = c_int;

/// CUDA context handle (opaque pointer).
pub type CUcontext = *mut c_void;

// =============================================================================
// RESULT CODES
// =============================================================================

/// CUDA operation completed successfully.
pub const CUDA_SUCCESS: CUresult = 0;

/// CUDA driver not initialized. Call cuInit() first.
pub const CUDA_ERROR_NOT_INITIALIZED: CUresult = 3;

/// Invalid device ordinal passed to cuDeviceGet.
pub const CUDA_ERROR_INVALID_DEVICE: CUresult = 101;

/// No CUDA-capable device is available.
pub const CUDA_ERROR_NO_DEVICE: CUresult = 100;

// =============================================================================
// DEVICE ATTRIBUTE CONSTANTS
// =============================================================================

/// Compute capability major version (e.g., 12 for RTX 5090).
pub const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR: CUdevice_attribute = 75;

/// Compute capability minor version (e.g., 0 for RTX 5090).
pub const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR: CUdevice_attribute = 76;

/// Maximum threads per block.
pub const CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK: CUdevice_attribute = 1;

/// Maximum block dimension X.
pub const CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X: CUdevice_attribute = 2;

/// Warp size in threads.
pub const CU_DEVICE_ATTRIBUTE_WARP_SIZE: CUdevice_attribute = 10;

/// Number of streaming multiprocessors on the device.
pub const CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT: CUdevice_attribute = 16;

// =============================================================================
// FFI DECLARATIONS
// =============================================================================

#[link(name = "cuda")]
extern "C" {
    /// Initialize the CUDA driver.
    ///
    /// MUST be called before any other CUDA driver function.
    /// Thread-safe if called with same flags (0).
    ///
    /// # Arguments
    ///
    /// * `flags` - Must be 0 (reserved for future use)
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` (0) on success
    /// * `CUDA_ERROR_NO_DEVICE` (100) if no CUDA device available
    /// * Other error codes on failure
    pub fn cuInit(flags: c_uint) -> CUresult;

    /// Get a CUDA device handle by ordinal.
    ///
    /// # Arguments
    ///
    /// * `device` - Output pointer for device handle
    /// * `ordinal` - Device index (0 for first GPU)
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    /// * `CUDA_ERROR_INVALID_DEVICE` if ordinal out of range
    pub fn cuDeviceGet(device: *mut CUdevice, ordinal: c_int) -> CUresult;

    /// Get the number of CUDA devices.
    ///
    /// # Arguments
    ///
    /// * `count` - Output pointer for device count
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    /// * Device count written to `count` pointer
    pub fn cuDeviceGetCount(count: *mut c_int) -> CUresult;

    /// Get a device attribute value.
    ///
    /// Much faster than cudaGetDeviceProperties (nanoseconds vs milliseconds).
    ///
    /// # Arguments
    ///
    /// * `pi` - Output pointer for attribute value
    /// * `attrib` - Attribute to query (CU_DEVICE_ATTRIBUTE_*)
    /// * `dev` - Device handle from cuDeviceGet
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    pub fn cuDeviceGetAttribute(
        pi: *mut c_int,
        attrib: CUdevice_attribute,
        dev: CUdevice,
    ) -> CUresult;

    /// Get the device name as a null-terminated string.
    ///
    /// # Arguments
    ///
    /// * `name` - Output buffer for device name
    /// * `len` - Buffer size including null terminator (recommend 256)
    /// * `dev` - Device handle from cuDeviceGet
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    pub fn cuDeviceGetName(name: *mut c_char, len: c_int, dev: CUdevice) -> CUresult;

    /// Get total memory on the device in bytes.
    ///
    /// Note: This is cuDeviceTotalMem_v2, the versioned API.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Output pointer for total memory in bytes
    /// * `dev` - Device handle from cuDeviceGet
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    pub fn cuDeviceTotalMem_v2(bytes: *mut usize, dev: CUdevice) -> CUresult;

    /// Get the CUDA driver version.
    ///
    /// Version is encoded as (major * 1000 + minor * 10).
    /// Example: CUDA 13.2 = 13020
    ///
    /// # Arguments
    ///
    /// * `version` - Output pointer for version number
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    pub fn cuDriverGetVersion(version: *mut c_int) -> CUresult;

    // =========================================================================
    // CONTEXT MANAGEMENT (for GpuDevice RAII wrapper)
    // =========================================================================

    /// Create a new CUDA context on the specified device.
    ///
    /// # Arguments
    ///
    /// * `pctx` - Output pointer for context handle
    /// * `flags` - Context creation flags (0 for default)
    /// * `dev` - Device handle from cuDeviceGet
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    /// * `CUDA_ERROR_INVALID_DEVICE` if device is invalid
    pub fn cuCtxCreate_v2(pctx: *mut CUcontext, flags: c_uint, dev: CUdevice) -> CUresult;

    /// Destroy a CUDA context.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Context handle to destroy
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    /// * May fail if context has active allocations
    pub fn cuCtxDestroy_v2(ctx: CUcontext) -> CUresult;

    /// Get free and total memory available on current context.
    ///
    /// # Arguments
    ///
    /// * `free` - Output pointer for free bytes
    /// * `total` - Output pointer for total bytes
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    /// * Requires active CUDA context (from cuCtxCreate_v2 or cuCtxSetCurrent)
    pub fn cuMemGetInfo_v2(free: *mut usize, total: *mut usize) -> CUresult;

    /// Set the current CUDA context for the calling thread.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Context to make current (or null to unbind)
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    pub fn cuCtxSetCurrent(ctx: CUcontext) -> CUresult;

    /// Get the current CUDA context for the calling thread.
    ///
    /// # Arguments
    ///
    /// * `pctx` - Output pointer for current context (null if none)
    ///
    /// # Returns
    ///
    /// * `CUDA_SUCCESS` on success
    pub fn cuCtxGetCurrent(pctx: *mut CUcontext) -> CUresult;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Decode CUDA driver version from encoded format.
///
/// # Arguments
///
/// * `encoded` - Version as (major * 1000 + minor * 10)
///
/// # Returns
///
/// Tuple of (major, minor) version numbers.
///
/// # Example
///
/// ```
/// use context_graph_cuda::ffi::decode_driver_version;
/// let (major, minor) = decode_driver_version(13010);
/// assert_eq!(major, 13);
/// assert_eq!(minor, 1);
/// ```
#[inline]
#[must_use]
pub const fn decode_driver_version(encoded: i32) -> (i32, i32) {
    let major = encoded / 1000;
    let minor = (encoded % 1000) / 10;
    (major, minor)
}

/// Check if a CUDA result indicates success.
///
/// # Example
///
/// ```
/// use context_graph_cuda::ffi::{is_cuda_success, CUDA_SUCCESS};
/// assert!(is_cuda_success(CUDA_SUCCESS));
/// assert!(!is_cuda_success(101));
/// ```
#[inline]
#[must_use]
pub const fn is_cuda_success(result: CUresult) -> bool {
    result == CUDA_SUCCESS
}

/// Get human-readable error message for CUDA result codes.
///
/// # Example
///
/// ```
/// use context_graph_cuda::ffi::{cuda_result_to_string, CUDA_ERROR_NO_DEVICE};
/// assert_eq!(cuda_result_to_string(CUDA_ERROR_NO_DEVICE), "CUDA_ERROR_NO_DEVICE (100): No CUDA-capable device");
/// ```
#[must_use]
pub fn cuda_result_to_string(result: CUresult) -> String {
    match result {
        CUDA_SUCCESS => "CUDA_SUCCESS (0): Operation completed successfully".to_string(),
        CUDA_ERROR_NOT_INITIALIZED => {
            "CUDA_ERROR_NOT_INITIALIZED (3): cuInit() not called".to_string()
        }
        CUDA_ERROR_NO_DEVICE => "CUDA_ERROR_NO_DEVICE (100): No CUDA-capable device".to_string(),
        CUDA_ERROR_INVALID_DEVICE => {
            "CUDA_ERROR_INVALID_DEVICE (101): Invalid device ordinal".to_string()
        }
        code => format!("CUDA_ERROR_UNKNOWN ({}): Unknown error code", code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_driver_version() {
        assert_eq!(decode_driver_version(13010), (13, 1));
        assert_eq!(decode_driver_version(12000), (12, 0));
        assert_eq!(decode_driver_version(11080), (11, 8));
    }

    #[test]
    fn test_decode_driver_version_edge_case_zero() {
        assert_eq!(decode_driver_version(0), (0, 0));
    }

    #[test]
    fn test_is_cuda_success() {
        assert!(is_cuda_success(CUDA_SUCCESS));
        assert!(is_cuda_success(0));
        assert!(!is_cuda_success(CUDA_ERROR_NOT_INITIALIZED));
        assert!(!is_cuda_success(CUDA_ERROR_INVALID_DEVICE));
    }

    #[test]
    fn test_cuda_result_to_string() {
        assert!(cuda_result_to_string(CUDA_SUCCESS).contains("CUDA_SUCCESS"));
        assert!(cuda_result_to_string(CUDA_ERROR_NO_DEVICE).contains("100"));
        assert!(cuda_result_to_string(999).contains("UNKNOWN"));
    }

    #[test]
    fn test_constants_match_cuda_header() {
        // These values are from cuda.h and must not change
        assert_eq!(CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR, 75);
        assert_eq!(CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR, 76);
        assert_eq!(CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK, 1);
        assert_eq!(CU_DEVICE_ATTRIBUTE_WARP_SIZE, 10);
    }
}
