//! FAISS FFI bindings - DISABLED on WSL2.
//!
//! FAISS GPU crashes on WSL2 with CUDA 13.1 due to static initialization bugs.
//! Use the custom GPU k-NN kernel in context-graph-cuda instead.
//!
//! For HDBSCAN clustering, use:
//! ```ignore
//! use context_graph_cuda::ffi::knn::{compute_core_distances_gpu, cuda_available};
//! ```

// Allow non-snake-case names to match FAISS C API
#![allow(non_snake_case)]

// FAISS module disabled - crashes on WSL2 with CUDA 13.1
// pub use context_graph_cuda::ffi::faiss::*;

// Provide stub types for compilation
use std::ffi::c_void;
use std::os::raw::{c_int, c_long};

/// Stub FAISS index pointer (not functional).
pub type FaissIndex = c_void;

/// Stub FAISS GPU resources provider pointer (not functional).
pub type FaissGpuResourcesProvider = c_void;

/// Stub FAISS standard GPU resources pointer (not functional).
pub type FaissStandardGpuResources = c_void;

/// Metric type for FAISS indexes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    /// L2 (Euclidean) distance.
    L2 = 0,
    /// Inner product (dot product) similarity.
    InnerProduct = 1,
}

/// FAISS success code (0).
pub const FAISS_OK: c_int = 0;

/// Check if GPU is available. Returns false on WSL2 due to FAISS issues.
pub fn gpu_available() -> bool {
    false
}

/// Stub GPU resources wrapper.
pub struct GpuResources;

impl GpuResources {
    /// Create new GPU resources. Always fails on WSL2.
    pub fn new() -> Result<Self, String> {
        Err("FAISS GPU disabled on WSL2".to_string())
    }

    /// Get the provider pointer. Panics - not available.
    pub fn as_provider(&self) -> *mut FaissGpuResourcesProvider {
        panic!("FAISS GPU disabled on WSL2")
    }
}

/// Check FAISS result code. Always returns error on WSL2.
pub fn check_faiss_result(code: c_int, operation: &str) -> Result<(), String> {
    if code == FAISS_OK {
        Ok(())
    } else {
        Err(format!("FAISS {} error code {} (FAISS disabled on WSL2)", operation, code))
    }
}

// Stub FFI functions that panic - FAISS is not available

/// Stub - not functional.
pub unsafe fn faiss_index_factory(
    _: *mut *mut FaissIndex,
    _: c_int,
    _: *const std::os::raw::c_char,
    _: MetricType,
) -> c_int {
    -1 // Error
}

/// Stub - not functional.
pub unsafe fn faiss_index_cpu_to_gpu(
    _: *mut FaissGpuResourcesProvider,
    _: c_int,
    _: *mut FaissIndex,
    _: *mut *mut FaissIndex,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_free(_: *mut FaissIndex) {}

/// Stub - not functional.
pub unsafe fn faiss_Index_add_with_ids(
    _: *mut FaissIndex,
    _: c_long,
    _: *const f32,
    _: *const c_long,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_search(
    _: *mut FaissIndex,
    _: c_long,
    _: *const f32,
    _: c_long,
    _: *mut f32,
    _: *mut c_long,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_train(
    _: *mut FaissIndex,
    _: c_long,
    _: *const f32,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_Index_is_trained(_: *mut FaissIndex) -> c_int {
    0
}

/// Stub - not functional.
pub unsafe fn faiss_Index_ntotal(_: *mut FaissIndex) -> c_long {
    0
}

/// Stub - not functional.
pub unsafe fn faiss_IndexIVF_set_nprobe(
    _: *mut FaissIndex,
    _: usize,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_StandardGpuResources_new(_: *mut *mut FaissStandardGpuResources) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_StandardGpuResources_free(_: *mut FaissStandardGpuResources) {}

/// Stub - not functional.
pub unsafe fn faiss_get_num_gpus() -> c_int {
    0
}

/// Stub - not functional.
pub unsafe fn faiss_read_index(
    _: *const std::os::raw::c_char,
    _: c_int,
    _: *mut *mut FaissIndex,
) -> c_int {
    -1
}

/// Stub - not functional.
pub unsafe fn faiss_write_index(
    _: *const FaissIndex,
    _: *const std::os::raw::c_char,
) -> c_int {
    -1
}

/// Stub - not functional.
#[cfg(feature = "faiss-gpu")]
pub fn gpu_count_direct() -> c_int {
    0
}
