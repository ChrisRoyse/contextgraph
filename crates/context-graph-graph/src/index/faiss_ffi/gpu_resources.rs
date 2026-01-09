//! RAII wrapper for FAISS GPU resources.
//!
//! This module provides a safe, RAII-managed wrapper for FAISS
//! GPU resource allocation and deallocation.
//!
//! # Safety
//!
//! `GpuResources` provides a safe interface over unsafe FFI calls.
//! Resources are automatically freed when dropped.
//!
//! # Constitution Reference
//!
//! - AP-015: GPU alloc without pool -> use CUDA memory pool

use std::ptr::NonNull;

use super::bindings::{faiss_StandardGpuResources_free, faiss_StandardGpuResources_new};
use super::types::{FaissGpuResourcesProvider, FaissStandardGpuResources};
use crate::error::{GraphError, GraphResult};

/// RAII wrapper for FAISS GPU resources.
///
/// Automatically frees GPU resources when dropped.
/// Safe to share across threads (Send + Sync).
///
/// # Example
///
/// ```ignore
/// let resources = GpuResources::new()?;
/// let provider = resources.as_provider();
/// // Use provider for cpu_to_gpu transfer...
/// // Resources automatically freed on drop
/// ```
pub struct GpuResources {
    ptr: NonNull<FaissStandardGpuResources>,
}

impl GpuResources {
    /// Allocate new GPU resources.
    ///
    /// # Errors
    ///
    /// Returns `GraphError::GpuResourceAllocation` if:
    /// - No GPU available
    /// - GPU memory allocation fails
    /// - FAISS library not linked
    ///
    /// # Constitution Reference
    ///
    /// AP-015: GPU alloc without pool -> use CUDA memory pool
    pub fn new() -> GraphResult<Self> {
        let mut res_ptr: *mut FaissStandardGpuResources = std::ptr::null_mut();

        // SAFETY: FFI call with valid output pointer
        let result = unsafe { faiss_StandardGpuResources_new(&mut res_ptr) };

        if result != 0 {
            return Err(GraphError::GpuResourceAllocation(format!(
                "faiss_StandardGpuResources_new failed with error code: {}",
                result
            )));
        }

        NonNull::new(res_ptr)
            .map(|ptr| GpuResources { ptr })
            .ok_or_else(|| {
                GraphError::GpuResourceAllocation(
                    "faiss_StandardGpuResources_new returned null pointer".to_string(),
                )
            })
    }

    /// Get the raw pointer for FFI calls.
    ///
    /// # Safety
    ///
    /// The returned pointer is valid for the lifetime of this GpuResources.
    /// Do NOT call `faiss_StandardGpuResources_free` on it manually.
    #[inline]
    pub fn as_ptr(&self) -> *mut FaissStandardGpuResources {
        self.ptr.as_ptr()
    }

    /// Get as GpuResourcesProvider for cpu_to_gpu transfer.
    ///
    /// Required by `faiss_index_cpu_to_gpu`.
    ///
    /// # Safety Note
    ///
    /// FAISS C API uses `FAISS_DECLARE_CLASS_INHERITED(StandardGpuResources, GpuResourcesProvider)`
    /// which creates: `typedef struct FaissGpuResourcesProvider_H FaissStandardGpuResources;`
    /// This makes the types structurally identical, so direct pointer cast is correct.
    #[inline]
    pub fn as_provider(&self) -> *mut FaissGpuResourcesProvider {
        // SAFETY: FaissStandardGpuResources and FaissGpuResourcesProvider are
        // typedef aliases in FAISS C API (via FAISS_DECLARE_CLASS_INHERITED).
        // Direct cast is safe and matches FAISS design.
        self.ptr.as_ptr() as *mut FaissGpuResourcesProvider
    }
}

impl Drop for GpuResources {
    fn drop(&mut self) {
        // SAFETY: ptr was allocated by faiss_StandardGpuResources_new
        // and has not been freed yet (RAII guarantees single ownership)
        unsafe {
            faiss_StandardGpuResources_free(self.ptr.as_ptr());
        }
    }
}

// SAFETY: GpuResources wraps a pointer to GPU resources allocated by FAISS.
// The underlying FAISS StandardGpuResources implementation is designed to be
// thread-safe - it uses internal synchronization for GPU memory management.
// We ensure single ownership via NonNull and RAII cleanup.
// Multiple threads can use the same GPU resources for different operations.
unsafe impl Send for GpuResources {}
unsafe impl Sync for GpuResources {}

impl std::fmt::Debug for GpuResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuResources")
            .field("ptr", &self.ptr)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::faiss_ffi::gpu_detection::gpu_available;

    #[test]
    fn test_gpu_resources_is_send_sync() {
        // Compile-time verification that GpuResources is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuResources>();
    }

    #[test]
    fn test_gpu_resources_allocation() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let resources = GpuResources::new();
        match resources {
            Ok(res) => {
                // Verify pointer is valid
                assert!(!res.as_ptr().is_null());
                assert!(!res.as_provider().is_null());
                println!("GPU resources allocated: {:?}", res);
            }
            Err(e) => {
                panic!("GPU resources allocation failed with GPU available: {}", e);
            }
        }
    }

    #[test]
    fn test_gpu_resources_drop() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        // Test that drop doesn't crash
        {
            let resources = GpuResources::new();
            match resources {
                Ok(res) => {
                    println!("Allocated GPU resources, will drop...");
                    drop(res);
                    println!("Drop completed without crash");
                }
                Err(e) => {
                    panic!("GPU resources allocation failed with GPU available: {}", e);
                }
            }
        }
        // If we reach here, drop worked correctly
    }
}
