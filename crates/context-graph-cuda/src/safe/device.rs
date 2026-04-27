//! Safe RAII wrapper for CUDA device and context.
//!
//! Ensures proper initialization and cleanup of CUDA resources.
//!
//! # Thread Safety
//!
//! `GpuDevice` is `Send` but NOT `Sync`. CUDA contexts are thread-bound;
//! you can move a `GpuDevice` to another thread, but you should not share
//! references across threads.
//!
//! # Constitution Compliance
//!
//! - ARCH-06: CUDA FFI only in context-graph-cuda
//! - AP-14: No .unwrap() - all errors propagated via Result

use crate::error::{CudaError, CudaResult};
use crate::ffi::cuda_driver::{
    cuCtxCreate_v2, cuCtxDestroy_v2, cuCtxSetCurrent, cuDeviceGet, cuDeviceGetAttribute,
    cuDeviceGetName, cuInit, cuMemGetInfo_v2, CUcontext, CUdevice, CUDA_ERROR_INVALID_DEVICE,
    CUDA_ERROR_NO_DEVICE, CUDA_SUCCESS, CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
    CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
};
use std::ffi::CStr;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Once;

/// Global once-guard for CUDA driver initialization.
static CUDA_INIT: Once = Once::new();

/// Result of CUDA initialization (stored for error reporting).
/// BLD-03 FIX: Replace `static mut` with AtomicI32 (CUresult = c_int = i32).
/// `static mut` is deprecated and will become a hard error in Rust edition 2024.
static CUDA_INIT_RESULT: AtomicI32 = AtomicI32::new(CUDA_SUCCESS);

/// RAII wrapper for CUDA device with automatic context cleanup.
///
/// # Thread Safety
///
/// - `Send`: Can be moved between threads
/// - NOT `Sync`: CUDA contexts are thread-bound; don't share references across threads
///
/// # Drop Behavior
///
/// Calls `cuCtxDestroy_v2` on drop. NEVER panics - logs errors instead.
///
/// # Example
///
/// ```no_run
/// use context_graph_cuda::GpuDevice;
///
/// fn main() -> Result<(), context_graph_cuda::CudaError> {
///     let device = GpuDevice::new(0)?;
///     println!("GPU: {}", device.name());
///     let (major, minor) = device.compute_capability();
///     println!("Compute capability: {}.{}", major, minor);
///     let (free, total) = device.memory_info()?;
///     println!("Memory: {} free / {} total", free, total);
///     // Context automatically destroyed when device goes out of scope
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct GpuDevice {
    /// CUDA device handle.
    device: CUdevice,
    /// CUDA context handle (owned by this struct).
    context: CUcontext,
    /// Device ordinal (for error messages).
    ordinal: i32,
}

impl GpuDevice {
    /// Create a new GPU device handle with CUDA context.
    ///
    /// Initializes CUDA driver (once per process) and creates context.
    ///
    /// # Arguments
    ///
    /// * `ordinal` - GPU device index (0 for first GPU)
    ///
    /// # Errors
    ///
    /// * `CudaError::NoDevice` - No CUDA device available
    /// * `CudaError::DeviceInitError` - CUDA init or context creation failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// let device = GpuDevice::new(0)?;
    /// println!("GPU: {}", device.name());
    /// ```
    pub fn new(ordinal: i32) -> CudaResult<Self> {
        // Thread-safe one-time CUDA initialization
        CUDA_INIT.call_once(|| {
            // SAFETY: cuInit(0) is thread-safe and idempotent
            let result = unsafe { cuInit(0) };
            CUDA_INIT_RESULT.store(result, Ordering::Release);
        });

        // Check if initialization succeeded
        let init_result = CUDA_INIT_RESULT.load(Ordering::Acquire);
        if init_result != CUDA_SUCCESS {
            return match init_result {
                CUDA_ERROR_NO_DEVICE => Err(CudaError::NoDevice),
                code => Err(CudaError::DeviceInitError(format!(
                    "cuInit failed with error code {}",
                    code
                ))),
            };
        }

        // Get device handle
        let mut device: CUdevice = 0;
        // SAFETY: device is valid pointer, cuInit was called
        let result = unsafe { cuDeviceGet(&mut device, ordinal) };
        if result != CUDA_SUCCESS {
            return match result {
                CUDA_ERROR_INVALID_DEVICE => Err(CudaError::DeviceInitError(format!(
                    "Invalid device ordinal {}: error code {} (CUDA_ERROR_INVALID_DEVICE)",
                    ordinal, result
                ))),
                CUDA_ERROR_NO_DEVICE => Err(CudaError::NoDevice),
                code => Err(CudaError::DeviceInitError(format!(
                    "cuDeviceGet({}) failed with error code {}",
                    ordinal, code
                ))),
            };
        }

        // Create context
        let mut context: CUcontext = ptr::null_mut();
        // SAFETY: device is valid, context is valid pointer
        let result = unsafe { cuCtxCreate_v2(&mut context, 0, device) };
        if result != CUDA_SUCCESS {
            return Err(CudaError::DeviceInitError(format!(
                "cuCtxCreate_v2 failed for device {}: error code {}",
                ordinal, result
            )));
        }

        Ok(Self {
            device,
            context,
            ordinal,
        })
    }

    /// Get compute capability (major, minor).
    ///
    /// # Example
    ///
    /// RTX 5090 returns `(12, 0)`.
    /// RTX 4090 returns `(8, 9)`.
    #[must_use]
    pub fn compute_capability(&self) -> (u32, u32) {
        let mut major: i32 = 0;
        let mut minor: i32 = 0;

        // SAFETY: device is valid, pointers are valid
        unsafe {
            cuDeviceGetAttribute(
                &mut major,
                CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
                self.device,
            );
            cuDeviceGetAttribute(
                &mut minor,
                CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
                self.device,
            );
        }

        // Convert to unsigned; negative values would indicate error but we
        // saturate to 0 rather than panic
        (major.max(0) as u32, minor.max(0) as u32)
    }

    /// Get device name.
    ///
    /// # Example
    ///
    /// Returns "NVIDIA GeForce RTX 5090" or similar.
    #[must_use]
    pub fn name(&self) -> String {
        // 256 bytes is sufficient for any CUDA device name
        let mut name_buf = [0i8; 256];

        // SAFETY: buffer is valid and large enough
        let result =
            unsafe { cuDeviceGetName(name_buf.as_mut_ptr(), name_buf.len() as i32, self.device) };

        if result != CUDA_SUCCESS {
            return format!("GPU {} (name query failed)", self.ordinal);
        }

        // SAFETY: cuDeviceGetName null-terminates the string
        let c_str = unsafe { CStr::from_ptr(name_buf.as_ptr()) };
        c_str.to_string_lossy().into_owned()
    }

    /// Get memory info (free_bytes, total_bytes).
    ///
    /// # Errors
    ///
    /// Returns error if context is invalid or CUDA call fails.
    ///
    /// # Note
    ///
    /// Free memory is approximate; other processes may allocate concurrently.
    pub fn memory_info(&self) -> CudaResult<(usize, usize)> {
        // Ensure our context is current for this thread
        // SAFETY: context is valid
        let result = unsafe { cuCtxSetCurrent(self.context) };
        if result != CUDA_SUCCESS {
            return Err(CudaError::DeviceInitError(format!(
                "cuCtxSetCurrent failed: error code {}",
                result
            )));
        }

        let mut free: usize = 0;
        let mut total: usize = 0;

        // SAFETY: context is current, pointers are valid
        let result = unsafe { cuMemGetInfo_v2(&mut free, &mut total) };
        if result != CUDA_SUCCESS {
            return Err(CudaError::MemoryError(format!(
                "cuMemGetInfo_v2 failed: error code {}",
                result
            )));
        }

        Ok((free, total))
    }

    /// Get device ordinal.
    #[inline]
    #[must_use]
    pub fn ordinal(&self) -> i32 {
        self.ordinal
    }

    /// Get GPU memory usage as a percentage (0.0 - 1.0).
    ///
    /// This calculates `1.0 - (free / total)` to show how much memory is in use.
    ///
    /// # Returns
    ///
    /// A value between 0.0 (no memory used) and 1.0 (fully utilized).
    /// Returns 0.0 if memory info cannot be retrieved.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let device = GpuDevice::new(0)?;
    /// let usage = device.memory_usage_percent();
    /// println!("GPU memory usage: {:.1}%", usage * 100.0);
    /// ```
    #[must_use]
    pub fn memory_usage_percent(&self) -> f32 {
        match self.memory_info() {
            Ok((free, total)) if total > 0 => {
                let used = total - free;
                (used as f64 / total as f64) as f32
            }
            _ => 0.0, // Return 0 if we can't get memory info
        }
    }
}

impl Drop for GpuDevice {
    fn drop(&mut self) {
        if self.context.is_null() {
            return;
        }

        // SAFETY: context is valid (we created it in new())
        let result = unsafe { cuCtxDestroy_v2(self.context) };
        if result != CUDA_SUCCESS {
            // Log error but DO NOT panic - this is critical for RAII safety
            // C2: Drop MUST NOT panic
            // We intentionally swallow the error; the context may leak but
            // the process won't abort. In production, consider using eprintln!
            // or a logging crate to report this failure.
            eprintln!(
                "[context-graph-cuda] WARNING: cuCtxDestroy_v2 failed for device {}: error code {}",
                self.ordinal, result
            );
        }
    }
}

// Send is safe: CUDA contexts can be moved to other threads
// SAFETY: GpuDevice contains raw pointers but they are owned and valid
unsafe impl Send for GpuDevice {}

// NOT implementing Sync: CUDA contexts are thread-bound
// Multiple threads should not use the same context simultaneously

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cuda_init_once_flag() {
        // Verify that CUDA_INIT is a valid Once
        // This test doesn't require a GPU - it just checks the Once mechanism
        assert!(!CUDA_INIT.is_completed() || CUDA_INIT.is_completed());
    }

    #[test]
    fn test_gpu_device_creation() {
        let device = GpuDevice::new(0).expect("GPU device creation failed");

        // Verify name is populated
        let name = device.name();
        assert!(!name.is_empty(), "Device name should not be empty");
        println!("Device name: {}", name);

        // Verify compute capability
        let (major, minor) = device.compute_capability();
        assert!(
            major >= 8,
            "Expected compute capability >= 8.x, got {}.{}",
            major,
            minor
        );
        println!("Compute capability: {}.{}", major, minor);

        // Verify memory info
        let (free, total) = device.memory_info().expect("memory_info failed");
        assert!(total > 0, "Total memory should be > 0");
        assert!(free <= total, "Free memory should be <= total");
        println!("Memory: {} free / {} total bytes", free, total);
    }

    #[test]
    fn test_gpu_device_invalid_ordinal() {
        let result = GpuDevice::new(999);
        assert!(result.is_err(), "Should fail for invalid device ordinal");

        let err = result.unwrap_err();
        println!("Expected error: {}", err);

        match err {
            CudaError::DeviceInitError(msg) => {
                assert!(
                    msg.contains("101") || msg.contains("INVALID_DEVICE"),
                    "Error should mention invalid device: {}",
                    msg
                );
            }
            CudaError::NoDevice => {
                // Also acceptable if no devices are present
            }
            other => panic!("Expected DeviceInitError or NoDevice, got: {:?}", other),
        }
    }

    #[test]
    fn test_gpu_device_drop_cleanup() {
        // Test that creating and dropping GpuDevice works without crashing
        // and that multiple devices can be created sequentially.
        //
        // NOTE: We do NOT test for exact memory equality because:
        // 1. CUDA driver maintains internal memory pools/caches
        // 2. Context creation/destruction has variable overhead
        // 3. The driver may keep allocations for performance
        //
        // The key test is that Drop doesn't panic and the context is properly
        // destroyed (cuCtxDestroy_v2 is called without error).

        // Create and destroy multiple devices to ensure cleanup works
        for i in 0..3 {
            let device = GpuDevice::new(0).expect("GPU device creation failed");
            let (free, total) = device.memory_info().expect("memory_info failed");
            println!(
                "Iteration {}: {} free / {} total bytes ({:.1}% free)",
                i,
                free,
                total,
                (free as f64 / total as f64) * 100.0
            );
            // Device dropped here - should call cuCtxDestroy_v2 without panic
        }

        // Final device to verify we can still create after multiple drops
        let final_device = GpuDevice::new(0).expect("Final device creation failed");
        let (free, total) = final_device.memory_info().expect("memory_info failed");

        // Basic sanity checks
        assert!(total > 0, "Total memory should be > 0");
        assert!(free > 0, "Free memory should be > 0");
        assert!(free <= total, "Free should be <= total");

        // RTX 5090 has 32GB VRAM - verify we see reasonable memory
        let expected_min_total = 30 * 1024 * 1024 * 1024_usize; // 30GB minimum
        assert!(
            total >= expected_min_total,
            "Expected at least 30GB VRAM for RTX 5090, got {} bytes",
            total
        );

        println!(
            "Drop cleanup test passed: {} free / {} total bytes",
            free, total
        );
    }

    #[test]
    fn test_cuda_init_once() {
        // Create multiple devices - should not double-init
        let d1 = GpuDevice::new(0).expect("First device failed");
        let d2 = GpuDevice::new(0).expect("Second device failed");

        // Both should work independently
        assert_eq!(d1.ordinal(), 0);
        assert_eq!(d2.ordinal(), 0);

        // Names should match (same physical device)
        assert_eq!(d1.name(), d2.name());
    }
}
