//! GPU Resources management for FAISS.
//!
//! Provides RAII wrapper around FAISS GPU resources with automatic cleanup.

use std::sync::Arc;

use super::super::faiss_ffi::GpuResources as FfiGpuResources;
use crate::error::{GraphError, GraphResult};

/// GPU resources handle with RAII cleanup.
///
/// Wraps raw GPU resource pointer with automatic deallocation.
/// Use `Arc<GpuResources>` for sharing across multiple indices.
///
/// # Thread Safety
///
/// This type is `Send + Sync` because the underlying FAISS StandardGpuResources
/// uses internal synchronization for GPU memory management.
pub struct GpuResources {
    inner: FfiGpuResources,
    gpu_id: i32,
}

// SAFETY: GpuResources wraps FfiGpuResources which is Send+Sync.
// The gpu_id field is Copy and thread-safe.
unsafe impl Send for GpuResources {}
unsafe impl Sync for GpuResources {}

impl GpuResources {
    /// Allocate GPU resources for the specified device.
    ///
    /// # Arguments
    ///
    /// * `gpu_id` - CUDA device ID (typically 0)
    ///
    /// # Errors
    ///
    /// Returns `GraphError::GpuResourceAllocation` if:
    /// - GPU device is unavailable
    /// - CUDA initialization fails
    /// - Insufficient GPU memory
    ///
    /// # Example
    ///
    /// ```no_run
    /// use context_graph_graph::index::gpu_index::GpuResources;
    /// use std::sync::Arc;
    ///
    /// let resources = Arc::new(GpuResources::new(0)?);
    /// # Ok::<(), context_graph_graph::error::GraphError>(())
    /// ```
    pub fn new(gpu_id: i32) -> GraphResult<Self> {
        // Create FFI GPU resources - this validates GPU availability
        let inner = FfiGpuResources::new().map_err(|e| {
            GraphError::GpuResourceAllocation(format!(
                "Failed to create GPU resources for device {}: {}",
                gpu_id, e
            ))
        })?;

        Ok(Self { inner, gpu_id })
    }

    /// Get reference to inner FFI resources for FFI calls.
    #[inline]
    pub(crate) fn inner(&self) -> &FfiGpuResources {
        &self.inner
    }

    /// Get the GPU device ID.
    #[inline]
    pub fn gpu_id(&self) -> i32 {
        self.gpu_id
    }
}

impl std::fmt::Debug for GpuResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuResources")
            .field("gpu_id", &self.gpu_id)
            .finish()
    }
}

/// Create a shared GPU resources handle.
///
/// Convenience function for creating `Arc<GpuResources>`.
///
/// # Arguments
///
/// * `gpu_id` - CUDA device ID (typically 0)
///
/// # Errors
///
/// Returns error if GPU resources cannot be allocated.
pub fn create_shared_resources(gpu_id: i32) -> GraphResult<Arc<GpuResources>> {
    Ok(Arc::new(GpuResources::new(gpu_id)?))
}
