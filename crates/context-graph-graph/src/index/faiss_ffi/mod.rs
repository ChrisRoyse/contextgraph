//! FAISS FFI bindings for GPU vector indexing.
//!
//! # Current Status: STUB IMPLEMENTATION
//!
//! FAISS GPU is disabled on WSL2 with CUDA 13.1 due to static initialization
//! crashes in the CUDA runtime. The custom GPU k-NN in context-graph-cuda
//! uses the CUDA Driver API which works correctly.
//!
//! ## To Enable Real FAISS GPU
//!
//! 1. Rebuild FAISS from source with lazy CUDA initialization:
//!    ```bash
//!    git clone https://github.com/facebookresearch/faiss
//!    cd faiss
//!    cmake -B build \
//!        -DFAISS_ENABLE_GPU=ON \
//!        -DCMAKE_CUDA_ARCHITECTURES=120 \
//!        -DBUILD_SHARED_LIBS=ON \
//!        -DFAISS_ENABLE_C_API=ON
//!    cmake --build build -j$(nproc)
//!    cmake --install build --prefix ~/.local
//!    ```
//!
//! 2. Add `faiss-working` feature to context-graph-cuda/Cargo.toml
//!
//! 3. Enable the feature when building:
//!    ```bash
//!    cargo build --features faiss-working
//!    ```
//!
//! # Constitution Compliance
//!
//! - ARCH-06: CUDA FFI only in context-graph-cuda
//! - AP-001: Fail fast, never unwrap() in prod

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_long};

use crate::error::{GraphError, GraphResult};

// =============================================================================
// TYPE DEFINITIONS (Stub)
// =============================================================================

/// Stub FAISS index pointer.
pub type FaissIndex = c_void;

/// Stub FAISS GPU resources provider pointer.
pub type FaissGpuResourcesProvider = c_void;

/// Stub FAISS standard GPU resources pointer.
pub type FaissStandardGpuResources = c_void;

/// Metric type for FAISS indexes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    /// Inner product (cosine similarity when normalized).
    InnerProduct = 0,
    /// L2 (Euclidean) distance.
    L2 = 1,
}

impl Default for MetricType {
    fn default() -> Self {
        MetricType::L2
    }
}

/// FAISS success code.
pub const FAISS_OK: c_int = 0;

// =============================================================================
// GPU AVAILABILITY
// =============================================================================

/// Check if FAISS GPU is available.
///
/// Returns false on WSL2 due to CUDA runtime static initialization issues.
/// Use `cuda_driver_available()` for checking raw CUDA availability.
pub fn gpu_available() -> bool {
    // FAISS GPU disabled on WSL2 - use custom k-NN instead
    // See module docs for instructions to enable
    false
}

/// Check if CUDA is available via driver API.
///
/// This works even when FAISS GPU crashes due to cudart issues.
pub fn cuda_driver_available() -> bool {
    // Check via context-graph-cuda driver API
    context_graph_cuda::ffi::cuda_available()
}

// =============================================================================
// GPU RESOURCES (Stub)
// =============================================================================

/// Stub GPU resources wrapper.
///
/// Real implementation requires FAISS GPU to be enabled.
pub struct GpuResources {
    _phantom: std::marker::PhantomData<()>,
}

impl GpuResources {
    /// Create new GPU resources.
    ///
    /// # Errors
    ///
    /// Always returns error on WSL2 - FAISS GPU is disabled.
    pub fn new() -> Result<Self, String> {
        Err("FAISS GPU disabled on WSL2 - see index/faiss_ffi/mod.rs for instructions".to_string())
    }

    /// Get the provider pointer.
    pub fn as_provider(&self) -> *mut FaissGpuResourcesProvider {
        std::ptr::null_mut()
    }
}

// =============================================================================
// RESULT CHECKING
// =============================================================================

/// Check FAISS result code.
pub fn check_faiss_result(code: c_int, operation: &str) -> GraphResult<()> {
    if code == FAISS_OK {
        Ok(())
    } else {
        Err(GraphError::FaissIndexCreation(format!(
            "FAISS {} failed with code {}",
            operation, code
        )))
    }
}

// =============================================================================
// FFI STUBS (Not functional - return errors)
// =============================================================================

/// Stub - not functional.
pub unsafe fn faiss_index_factory(
    _p_index: *mut *mut FaissIndex,
    _d: c_int,
    _description: *const c_char,
    _metric: MetricType,
) -> c_int {
    -1 // Error
}

/// Stub - not functional.
pub unsafe fn faiss_index_cpu_to_gpu(
    _provider: *mut FaissGpuResourcesProvider,
    _device: c_int,
    _index: *mut FaissIndex,
    _p_out: *mut *mut FaissIndex,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_free(_index: *mut FaissIndex) {}

/// Stub - not functional.
pub unsafe fn faiss_Index_add_with_ids(
    _index: *mut FaissIndex,
    _n: c_long,
    _x: *const f32,
    _xids: *const c_long,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_search(
    _index: *mut FaissIndex,
    _n: c_long,
    _x: *const f32,
    _k: c_long,
    _distances: *mut f32,
    _labels: *mut c_long,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_train(
    _index: *mut FaissIndex,
    _n: c_long,
    _x: *const f32,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_is_trained(_index: *mut FaissIndex) -> c_int {
    0 // Not trained
}

/// Stub - not functional.
pub unsafe fn faiss_Index_ntotal(_index: *mut FaissIndex) -> c_long {
    0
}

/// Stub - not functional.
pub unsafe fn faiss_IndexIVF_set_nprobe(_index: *mut FaissIndex, _nprobe: usize) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_StandardGpuResources_new(
    _p_res: *mut *mut FaissStandardGpuResources,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_StandardGpuResources_free(_res: *mut FaissStandardGpuResources) {}

/// Stub - not functional.
pub unsafe fn faiss_get_num_gpus() -> c_int {
    0
}

/// Stub - not functional.
pub unsafe fn faiss_read_index(
    _fname: *const c_char,
    _io_flags: c_int,
    _p_out: *mut *mut FaissIndex,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_write_index(_index: *const FaissIndex, _fname: *const c_char) -> c_int {
    -1
}

#[cfg(feature = "faiss-gpu")]
pub fn gpu_count_direct() -> c_int {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_available_returns_false() {
        // FAISS GPU disabled on WSL2
        assert!(!gpu_available());
    }

    #[test]
    fn test_cuda_driver_available() {
        // CUDA driver API should work even when FAISS doesn't
        let available = cuda_driver_available();
        println!("CUDA driver available: {}", available);
        // This should be true on systems with NVIDIA GPU
    }

    #[test]
    fn test_metric_type_default() {
        assert_eq!(MetricType::default(), MetricType::L2);
    }

    #[test]
    fn test_check_faiss_result_ok() {
        assert!(check_faiss_result(FAISS_OK, "test").is_ok());
    }

    #[test]
    fn test_check_faiss_result_error() {
        assert!(check_faiss_result(-1, "test").is_err());
    }
}
