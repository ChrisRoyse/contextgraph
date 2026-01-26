//! FAISS FFI bindings for GPU vector indexing.
//!
//! # Architecture
//!
//! This module serves as a bridge to the FAISS FFI bindings in `context-graph-cuda`.
//! All FAISS FFI declarations are centralized in `context-graph-cuda/src/ffi/faiss.rs`
//! per Constitution ARCH-06 (CUDA FFI only in context-graph-cuda).
//!
//! # FAISS GPU Availability
//!
//! FAISS GPU requires the `faiss-working` feature to be enabled in `context-graph-cuda`.
//! Without this feature, FAISS operations will fail fast with clear error messages.
//!
//! ## To Enable FAISS GPU
//!
//! 1. Rebuild FAISS from source with CUDA 13.1+ and sm_120 support:
//!    ```bash
//!    ./scripts/rebuild_faiss_gpu.sh
//!    ```
//!
//! 2. Build the workspace with the faiss-working feature:
//!    ```bash
//!    cargo build -p context-graph-cuda --features faiss-working
//!    ```
//!
//! # Constitution Compliance
//!
//! - ARCH-06: CUDA FFI only in context-graph-cuda (this module re-exports from there)
//! - ARCH-GPU-04: FAISS indexes use GPU (faiss-gpu) not CPU
//! - AP-001: Fail fast, never unwrap() in prod
//! - AP-GPU-03: NEVER use CPU FAISS when GPU FAISS available

#![allow(non_snake_case)]

use crate::error::{GraphError, GraphResult};

// =============================================================================
// TYPE DEFINITIONS - Re-exported from context-graph-cuda when available
// =============================================================================

/// Metric type for FAISS indexes.
///
/// Determines how distances are computed between vectors.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetricType {
    /// Inner product (cosine similarity when normalized).
    /// Higher values = more similar.
    InnerProduct = 0,

    /// L2 (Euclidean) distance.
    /// Lower values = more similar.
    #[default]
    L2 = 1,
}

/// FAISS success code.
pub const FAISS_OK: i32 = 0;

// =============================================================================
// TYPE ALIASES for FFI compatibility
// =============================================================================

/// Opaque FAISS index pointer.
pub type FaissIndex = std::ffi::c_void;

/// Opaque FAISS GPU resources provider pointer.
pub type FaissGpuResourcesProvider = std::ffi::c_void;

/// Opaque FAISS standard GPU resources pointer.
pub type FaissStandardGpuResources = std::ffi::c_void;

// =============================================================================
// GPU AVAILABILITY CHECKING
// =============================================================================

/// Check if FAISS GPU is available.
///
/// Returns `true` only when:
/// 1. `context-graph-cuda` was built with `faiss-working` feature
/// 2. FAISS reports at least one GPU
/// 3. GPU actually works (verified at runtime)
///
/// # Example
///
/// ```no_run
/// use context_graph_graph::index::faiss_ffi::gpu_available;
///
/// if gpu_available() {
///     println!("FAISS GPU is available!");
/// } else {
///     println!("FAISS GPU is not available - see logs for details");
/// }
/// ```
pub fn gpu_available() -> bool {
    context_graph_cuda::is_faiss_gpu_available()
}

/// Check if CUDA is available via driver API.
///
/// This works even when FAISS GPU crashes due to cudart issues.
/// The driver API is used by the custom k-NN implementation.
pub fn cuda_driver_available() -> bool {
    context_graph_cuda::ffi::cuda_available()
}

/// Get a human-readable status of FAISS GPU availability.
pub fn faiss_status() -> &'static str {
    context_graph_cuda::faiss_status()
}

// =============================================================================
// GPU RESOURCES
// =============================================================================

/// GPU resources wrapper for FAISS operations.
///
/// This is only functional when `context-graph-cuda` is built with `faiss-working`.
/// Otherwise, creating `GpuResources` will return an error.
pub struct GpuResources {
    /// Marker to prevent direct construction
    _private: (),
}

impl GpuResources {
    /// Create new GPU resources.
    ///
    /// # Errors
    ///
    /// Returns `GraphError::FaissGpuUnavailable` if FAISS GPU is not available.
    /// This happens when:
    /// - `faiss-working` feature is not enabled in context-graph-cuda
    /// - FAISS library is not installed
    /// - No GPU available
    ///
    /// # Example
    ///
    /// ```no_run
    /// use context_graph_graph::index::faiss_ffi::GpuResources;
    ///
    /// match GpuResources::new() {
    ///     Ok(res) => println!("GPU resources allocated"),
    ///     Err(e) => eprintln!("FAISS GPU unavailable: {}", e),
    /// }
    /// ```
    pub fn new() -> GraphResult<Self> {
        if !gpu_available() {
            return Err(GraphError::FaissGpuUnavailable {
                reason: faiss_status().to_string(),
                help: "Run ./scripts/rebuild_faiss_gpu.sh and build with --features faiss-working"
                    .to_string(),
            });
        }

        // When faiss-working is enabled, we can create real GPU resources
        // For now, this is a placeholder - the actual FAISS operations
        // should be performed through context-graph-cuda's FAISS module
        Ok(Self { _private: () })
    }

    /// Get the provider pointer for FFI calls.
    ///
    /// # Returns
    ///
    /// Returns null pointer if FAISS GPU is not available.
    /// When FAISS GPU is enabled, this would return the actual provider pointer.
    pub fn as_provider(&self) -> *mut FaissGpuResourcesProvider {
        if !gpu_available() {
            tracing::error!(
                target: "context_graph::faiss",
                "GpuResources::as_provider called but FAISS GPU is not available"
            );
            return std::ptr::null_mut();
        }

        // When faiss-working is enabled in context-graph-cuda,
        // this would delegate to the real GPU resources
        std::ptr::null_mut()
    }
}

impl std::fmt::Debug for GpuResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuResources")
            .field("available", &gpu_available())
            .finish()
    }
}

// =============================================================================
// RESULT CHECKING
// =============================================================================

/// Check FAISS result code and convert to GraphResult.
///
/// # Arguments
///
/// * `code` - FAISS return code (0 = success)
/// * `operation` - Description of the operation for error messages
///
/// # Returns
///
/// * `Ok(())` if code is 0
/// * `Err(GraphError::FaissIndexCreation)` otherwise
pub fn check_faiss_result(code: i32, operation: &str) -> GraphResult<()> {
    if code == FAISS_OK {
        Ok(())
    } else {
        Err(GraphError::FaissIndexCreation(format!(
            "FAISS {} failed with code {} - ensure FAISS GPU is properly installed",
            operation, code
        )))
    }
}

// =============================================================================
// FFI FUNCTIONS - FAIL FAST when FAISS is unavailable
// =============================================================================

/// Log an error when a FAISS function is called but GPU is unavailable.
fn log_faiss_unavailable(function_name: &str) {
    tracing::error!(
        target: "context_graph::faiss",
        "{} called but FAISS GPU is not available. Status: {}",
        function_name,
        faiss_status()
    );
}

/// Create index from factory string.
///
/// # Safety
///
/// - `p_index` must be a valid pointer to store the result
/// - `description` must be a valid C string
///
/// # Errors
///
/// Returns -1 if FAISS GPU is not available. Use `gpu_available()` to check first.
pub unsafe fn faiss_index_factory(
    _p_index: *mut *mut std::ffi::c_void,
    _d: i32,
    _description: *const std::os::raw::c_char,
    _metric: MetricType,
) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_index_factory");
        return -1;
    }
    -1 // FAISS GPU delegation not yet implemented
}

/// Transfer index from CPU to GPU.
///
/// # Safety
///
/// All pointers must be valid.
///
/// # Errors
///
/// Returns -1 if FAISS GPU is not available.
pub unsafe fn faiss_index_cpu_to_gpu(
    _provider: *mut std::ffi::c_void,
    _device: i32,
    _index: *mut std::ffi::c_void,
    _p_out: *mut *mut std::ffi::c_void,
) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_index_cpu_to_gpu");
        return -1;
    }
    -1
}

/// Free index and release memory.
pub unsafe fn faiss_Index_free(_index: *mut std::ffi::c_void) {
    // No-op when FAISS unavailable
}

/// Add vectors with IDs to the index.
pub unsafe fn faiss_Index_add_with_ids(
    _index: *mut std::ffi::c_void,
    _n: i64,
    _x: *const f32,
    _xids: *const i64,
) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_Index_add_with_ids");
        return -1;
    }
    -1
}

/// Search for k nearest neighbors.
pub unsafe fn faiss_Index_search(
    _index: *const std::ffi::c_void,
    _n: i64,
    _x: *const f32,
    _k: i64,
    _distances: *mut f32,
    _labels: *mut i64,
) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_Index_search");
        return -1;
    }
    -1
}

/// Train the index with vectors.
pub unsafe fn faiss_Index_train(
    _index: *mut std::ffi::c_void,
    _n: i64,
    _x: *const f32,
) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_Index_train");
        return -1;
    }
    -1
}

/// Check if index is trained.
pub unsafe fn faiss_Index_is_trained(_index: *const std::ffi::c_void) -> i32 {
    0
}

/// Get total number of vectors in index.
pub unsafe fn faiss_Index_ntotal(_index: *const std::ffi::c_void) -> i64 {
    0
}

/// Set nprobe parameter for IVF index.
pub unsafe fn faiss_IndexIVF_set_nprobe(_index: *mut std::ffi::c_void, _nprobe: usize) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_IndexIVF_set_nprobe");
        return -1;
    }
    -1
}

/// Allocate standard GPU resources.
pub unsafe fn faiss_StandardGpuResources_new(_p_res: *mut *mut std::ffi::c_void) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_StandardGpuResources_new");
        return -1;
    }
    -1
}

/// Free GPU resources.
pub unsafe fn faiss_StandardGpuResources_free(_res: *mut std::ffi::c_void) {
    // No-op when FAISS unavailable
}

/// Get number of GPUs.
pub unsafe fn faiss_get_num_gpus() -> i32 {
    if gpu_available() { 1 } else { 0 }
}

/// Read index from file.
pub unsafe fn faiss_read_index(
    _fname: *const std::os::raw::c_char,
    _io_flags: i32,
    _p_out: *mut *mut std::ffi::c_void,
) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_read_index");
        return -1;
    }
    -1
}

/// Write index to file.
pub unsafe fn faiss_write_index(
    _index: *const std::ffi::c_void,
    _fname: *const std::os::raw::c_char,
) -> i32 {
    if !gpu_available() {
        log_faiss_unavailable("faiss_write_index");
        return -1;
    }
    -1
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_available_returns_bool() {
        // This should return false unless faiss-working is enabled
        let available = gpu_available();
        println!("FAISS GPU available: {}", available);
        println!("FAISS status: {}", faiss_status());
    }

    #[test]
    fn test_cuda_driver_available() {
        // CUDA driver API should work even when FAISS doesn't
        let available = cuda_driver_available();
        println!("CUDA driver available: {}", available);
    }

    #[test]
    fn test_metric_type_default() {
        assert_eq!(MetricType::default(), MetricType::L2);
    }

    #[test]
    fn test_metric_type_values() {
        assert_eq!(MetricType::InnerProduct as i32, 0);
        assert_eq!(MetricType::L2 as i32, 1);
    }

    #[test]
    fn test_check_faiss_result_ok() {
        assert!(check_faiss_result(FAISS_OK, "test").is_ok());
    }

    #[test]
    fn test_check_faiss_result_error() {
        let result = check_faiss_result(-1, "test");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("FAISS"));
    }

    #[test]
    fn test_gpu_resources_error_when_unavailable() {
        // When FAISS is not available, GpuResources::new() should return an error
        if !gpu_available() {
            let result = GpuResources::new();
            assert!(result.is_err(), "Expected error when FAISS GPU unavailable");
            let err = result.unwrap_err();
            assert!(
                err.to_string().contains("FAISS") || err.to_string().contains("faiss"),
                "Error should mention FAISS"
            );
            println!("Got expected error: {}", err);
        } else {
            let result = GpuResources::new();
            assert!(result.is_ok(), "Should succeed when FAISS GPU is available");
        }
    }
}
