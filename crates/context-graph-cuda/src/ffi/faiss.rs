//! FAISS C API FFI bindings - SINGLE SOURCE OF TRUTH.
//!
//! ALL FAISS extern "C" declarations MUST be in this module.
//! No other crate may declare FAISS FFI bindings.
//!
//! # Constitution Compliance
//!
//! - ARCH-06: CUDA FFI only in context-graph-cuda
//! - AP-08: No sync I/O in async context (these are blocking calls)
//! - AP-15: GPU alloc without pool -> use CUDA memory pool
//!
//! # Safety
//!
//! All functions in this module are unsafe FFI. Callers must ensure:
//! - GPU resources allocated before use
//! - Index trained before search
//! - Proper buffer sizes for output arrays
//!
//! # FAISS C API Reference
//!
//! <https://github.com/facebookresearch/faiss/blob/main/c_api/>

use std::os::raw::{c_char, c_float, c_int, c_long};
use std::ptr::NonNull;
use std::sync::OnceLock;

use crate::error::{CudaError, CudaResult};

// =============================================================================
// TYPE DEFINITIONS
// =============================================================================

/// Metric type for distance computation.
///
/// Must match FAISS MetricType enum values exactly.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MetricType {
    /// Inner product (cosine similarity when normalized).
    /// Higher values = more similar.
    InnerProduct = 0,

    /// L2 (Euclidean) distance.
    /// Lower values = more similar.
    #[default]
    L2 = 1,
}

/// Opaque pointer to FAISS index.
///
/// Represents any FAISS index (Flat, IVF, PQ, GPU, etc.).
#[repr(C)]
pub struct FaissIndex {
    _private: [u8; 0],
}

/// Opaque pointer to FAISS GPU resources provider interface.
#[repr(C)]
pub struct FaissGpuResourcesProvider {
    _private: [u8; 0],
}

/// Opaque pointer to FAISS standard GPU resources.
///
/// Manages GPU memory allocation for FAISS operations.
/// Must be freed with `faiss_StandardGpuResources_free`.
#[repr(C)]
pub struct FaissStandardGpuResources {
    _private: [u8; 0],
}

// =============================================================================
// FFI DECLARATIONS
// =============================================================================

#[link(name = "faiss_c")]
extern "C" {
    // ---------- Index Factory ----------

    /// Create index from factory string.
    ///
    /// # Arguments
    /// - `p_index`: Output pointer to created index
    /// - `d`: Vector dimension
    /// - `description`: Factory string (e.g., "IVF16384,PQ64x8")
    /// - `metric`: Distance metric type
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    pub fn faiss_index_factory(
        p_index: *mut *mut FaissIndex,
        d: c_int,
        description: *const c_char,
        metric: MetricType,
    ) -> c_int;

    /// Free index and release memory.
    pub fn faiss_Index_free(index: *mut FaissIndex);

    // ---------- GPU Resources ----------

    /// Allocate standard GPU resources.
    ///
    /// Creates StandardGpuResources for GPU memory management.
    /// MUST be freed with `faiss_StandardGpuResources_free`.
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    pub fn faiss_StandardGpuResources_new(p_res: *mut *mut FaissStandardGpuResources) -> c_int;

    /// Free GPU resources.
    pub fn faiss_StandardGpuResources_free(res: *mut FaissStandardGpuResources);

    // ---------- CPU to GPU Transfer ----------

    /// Transfer index from CPU to GPU.
    ///
    /// # Arguments
    /// - `provider`: GPU resources provider
    /// - `device`: GPU device ID (usually 0)
    /// - `index`: Source CPU index
    /// - `p_out`: Output pointer to GPU index
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    pub fn faiss_index_cpu_to_gpu(
        provider: *mut FaissGpuResourcesProvider,
        device: c_int,
        index: *const FaissIndex,
        p_out: *mut *mut FaissIndex,
    ) -> c_int;

    // ---------- Index Operations ----------

    /// Train the index with vectors.
    ///
    /// For IVF indices, clusters vectors to create centroids.
    /// Must be called before `add_with_ids` for untrained indices.
    ///
    /// # Arguments
    /// - `index`: Target index
    /// - `n`: Number of training vectors
    /// - `x`: Training vectors (n * d floats, row-major)
    pub fn faiss_Index_train(index: *mut FaissIndex, n: c_long, x: *const c_float) -> c_int;

    /// Check if index is trained.
    ///
    /// # Returns
    /// Non-zero if trained, 0 if not trained
    pub fn faiss_Index_is_trained(index: *const FaissIndex) -> c_int;

    /// Add vectors with IDs to the index.
    ///
    /// # Arguments
    /// - `index`: Target index
    /// - `n`: Number of vectors
    /// - `x`: Vectors to add (n * d floats, row-major)
    /// - `xids`: Vector IDs (n longs)
    pub fn faiss_Index_add_with_ids(
        index: *mut FaissIndex,
        n: c_long,
        x: *const c_float,
        xids: *const c_long,
    ) -> c_int;

    /// Search for k nearest neighbors.
    ///
    /// # Arguments
    /// - `index`: Source index
    /// - `n`: Number of query vectors
    /// - `x`: Query vectors (n * d floats, row-major)
    /// - `k`: Number of neighbors to return
    /// - `distances`: Output distances (n * k floats)
    /// - `labels`: Output IDs (n * k longs, -1 for missing)
    pub fn faiss_Index_search(
        index: *const FaissIndex,
        n: c_long,
        x: *const c_float,
        k: c_long,
        distances: *mut c_float,
        labels: *mut c_long,
    ) -> c_int;

    /// Set nprobe parameter for IVF index.
    ///
    /// Controls search quality vs speed tradeoff.
    /// Higher values = more accurate but slower.
    pub fn faiss_IndexIVF_set_nprobe(index: *mut FaissIndex, nprobe: usize);

    /// Get total number of vectors in index.
    pub fn faiss_Index_ntotal(index: *const FaissIndex) -> c_long;

    // ---------- Persistence ----------

    /// Write index to file.
    ///
    /// # Arguments
    /// - `index`: Source index
    /// - `fname`: Output file path (C string)
    pub fn faiss_write_index(index: *const FaissIndex, fname: *const c_char) -> c_int;

    /// Read index from file.
    ///
    /// # Arguments
    /// - `fname`: Input file path (C string)
    /// - `io_flags`: IO flags (usually 0)
    /// - `p_out`: Output pointer to loaded index
    pub fn faiss_read_index(
        fname: *const c_char,
        io_flags: c_int,
        p_out: *mut *mut FaissIndex,
    ) -> c_int;

    // ---------- GPU Detection ----------

    /// Get the number of available GPUs.
    ///
    /// # Arguments
    /// * `p_output` - Pointer to store the GPU count
    ///
    /// # Returns
    /// 0 on success, non-zero error code on failure
    pub fn faiss_get_num_gpus(p_output: *mut c_int) -> c_int;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Check FAISS result code and convert to CudaResult.
///
/// # Arguments
///
/// - `code`: FAISS return code (0 = success)
/// - `operation`: Description of operation for error message
///
/// # Returns
///
/// - `Ok(())` if code is 0
/// - `Err(CudaError::FaissError)` otherwise
///
/// # Example
///
/// ```ignore
/// let result = unsafe { faiss_Index_train(index, n, x) };
/// check_faiss_result(result, "faiss_Index_train")?;
/// ```
#[inline]
pub fn check_faiss_result(code: c_int, operation: &str) -> CudaResult<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(CudaError::FaissError {
            operation: operation.to_string(),
            code,
        })
    }
}

/// FAISS result code constant.
pub const FAISS_OK: c_int = 0;

// =============================================================================
// GPU DETECTION
// =============================================================================

/// Check if FAISS GPU support is available.
///
/// Returns true if:
/// 1. The `cuda` feature is enabled
/// 2. FAISS reports at least one CUDA-capable GPU
/// 3. GPU actually works (verified via subprocess on WSL2)
///
/// Uses subprocess detection to prevent crashes on WSL2 with driver issues.
///
/// # Environment Variables
///
/// - `SKIP_GPU_TESTS=1`: Force this function to return false
///
/// # Example
///
/// ```ignore
/// if gpu_available() {
///     let resources = GpuResources::new()?;
///     // ... use GPU resources
/// } else {
///     println!("No GPU available");
/// }
/// ```
#[cfg(feature = "cuda")]
pub fn gpu_available() -> bool {
    static GPU_AVAILABLE: OnceLock<bool> = OnceLock::new();

    *GPU_AVAILABLE.get_or_init(|| {
        // Allow tests to skip GPU via environment variable
        if std::env::var("SKIP_GPU_TESTS")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            return false;
        }

        // Use subprocess to safely check GPU availability
        check_gpu_via_subprocess()
    })
}

#[cfg(feature = "cuda")]
fn check_gpu_via_subprocess() -> bool {
    use std::path::Path;
    use std::process::Command;

    // Check if nvidia-smi works and reports GPUs
    match Command::new("nvidia-smi")
        .arg("--query-gpu=count")
        .arg("--format=csv,noheader")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(count) = stdout.trim().parse::<i32>() {
                    if count > 0 {
                        // nvidia-smi works and found GPUs
                        // On WSL2, FAISS may crash due to CUDA version mismatch
                        // Use subprocess test to verify FAISS works before proceeding
                        if Path::new("/usr/lib/wsl/lib").exists() {
                            // WSL2 detected - test FAISS in subprocess to avoid crash
                            let test_result = test_faiss_gpu_subprocess();
                            if !test_result {
                                tracing::error!(
                                    target: "context_graph::cuda",
                                    "FAISS GPU test failed on WSL2. This is likely due to CUDA version mismatch. \
                                    FAISS must be rebuilt with CUDA 13.1+ for RTX 5090 compatibility. \
                                    Run: cd /path/to/faiss && cmake -B build -DFAISS_ENABLE_GPU=ON \
                                    -DCMAKE_CUDA_ARCHITECTURES=120 && cmake --build build"
                                );
                            }
                            return test_result;
                        }
                        return true;
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}

/// Test FAISS GPU in a subprocess to safely detect crashes.
#[cfg(feature = "cuda")]
fn test_faiss_gpu_subprocess() -> bool {
    use std::fs;
    use std::process::Command;

    // Try to run a simple FAISS GPU test
    // This test binary should call faiss_get_num_gpus() and exit cleanly
    let test_binary = "/tmp/faiss_gpu_test_bin";

    // Check if we have a working test binary
    if std::path::Path::new(test_binary).exists() {
        match Command::new(test_binary).output() {
            Ok(output) => {
                if output.status.success() {
                    return true;
                }
                tracing::debug!(
                    target: "context_graph::cuda",
                    "FAISS GPU test binary returned error: {:?}",
                    output.status.code()
                );
                return false;
            }
            Err(e) => {
                tracing::debug!(
                    target: "context_graph::cuda",
                    "Failed to run FAISS GPU test binary: {}",
                    e
                );
                return false;
            }
        }
    }

    // No test binary - try to create one
    // This requires gcc and libfaiss_c
    let test_source = r#"
#include <stdio.h>
int faiss_get_num_gpus(int* n);
int main() {
    int n = -1;
    int ret = faiss_get_num_gpus(&n);
    if (ret != 0 || n <= 0) return 1;
    return 0;
}
"#;

    let source_path = "/tmp/faiss_gpu_test.c";
    if fs::write(source_path, test_source).is_err() {
        tracing::debug!(
            target: "context_graph::cuda",
            "Failed to write FAISS GPU test source"
        );
        return false;
    }

    // Determine FAISS library path
    // Priority: HOME/.local/lib (user-built), then /usr/local/lib
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/cabdru".to_string());
    let user_lib = format!("{}/.local/lib", home);
    let lib_path = if std::path::Path::new(&user_lib).join("libfaiss_c.so").exists() {
        user_lib
    } else {
        "/usr/local/lib".to_string()
    };

    // Compile test binary
    let compile_result = Command::new("gcc")
        .args([
            source_path,
            "-o",
            test_binary,
            &format!("-L{}", lib_path),
            "-lfaiss_c",
            &format!("-Wl,-rpath,{}", lib_path),
        ])
        .output();

    match compile_result {
        Ok(output) => {
            if !output.status.success() {
                tracing::debug!(
                    target: "context_graph::cuda",
                    "Failed to compile FAISS GPU test: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return false;
            }
        }
        Err(e) => {
            tracing::debug!(
                target: "context_graph::cuda",
                "Failed to run gcc for FAISS GPU test: {}",
                e
            );
            return false;
        }
    }

    // Run the test binary
    match Command::new(test_binary).output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Stub for non-CUDA builds.
#[cfg(not(feature = "cuda"))]
#[inline]
pub fn gpu_available() -> bool {
    false
}

/// Directly check GPU count via FAISS FFI.
///
/// # Safety
/// This can crash on WSL2 with driver issues. Use `gpu_available()` instead.
#[cfg(feature = "cuda")]
pub unsafe fn gpu_count_direct() -> Result<i32, i32> {
    let mut num_gpus: c_int = 0;
    let rc = faiss_get_num_gpus(&mut num_gpus);
    if rc == 0 {
        Ok(num_gpus)
    } else {
        Err(rc)
    }
}

// =============================================================================
// GPU RESOURCES RAII WRAPPER
// =============================================================================

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
    /// Returns `CudaError::FaissError` if:
    /// - No GPU available
    /// - GPU memory allocation fails
    /// - FAISS library not linked
    pub fn new() -> CudaResult<Self> {
        let mut res_ptr: *mut FaissStandardGpuResources = std::ptr::null_mut();

        // SAFETY: FFI call with valid output pointer
        let result = unsafe { faiss_StandardGpuResources_new(&mut res_ptr) };

        if result != 0 {
            return Err(CudaError::FaissError {
                operation: "faiss_StandardGpuResources_new".to_string(),
                code: result,
            });
        }

        NonNull::new(res_ptr)
            .map(|ptr| GpuResources { ptr })
            .ok_or_else(|| CudaError::FaissError {
                operation: "faiss_StandardGpuResources_new".to_string(),
                code: -1, // Null pointer returned
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
    /// FAISS C API uses typedef alias making types structurally identical.
    /// Direct pointer cast is correct.
    #[inline]
    pub fn as_provider(&self) -> *mut FaissGpuResourcesProvider {
        self.ptr.as_ptr() as *mut FaissGpuResourcesProvider
    }
}

impl Drop for GpuResources {
    fn drop(&mut self) {
        // SAFETY: ptr was allocated by faiss_StandardGpuResources_new
        unsafe {
            faiss_StandardGpuResources_free(self.ptr.as_ptr());
        }
    }
}

// SAFETY: GpuResources uses internal synchronization for GPU memory.
unsafe impl Send for GpuResources {}
unsafe impl Sync for GpuResources {}

impl std::fmt::Debug for GpuResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuResources")
            .field("ptr", &self.ptr)
            .finish()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_type_values() {
        // FAISS C API requires exact enum values
        assert_eq!(MetricType::InnerProduct as i32, 0);
        assert_eq!(MetricType::L2 as i32, 1);
    }

    #[test]
    fn test_metric_type_default() {
        assert_eq!(MetricType::default(), MetricType::L2);
    }

    #[test]
    fn test_check_faiss_result_success() {
        let result = check_faiss_result(0, "test_operation");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_faiss_result_failure() {
        let result = check_faiss_result(-1, "test_operation");
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            CudaError::FaissError { operation, code } => {
                assert_eq!(operation, "test_operation");
                assert_eq!(code, -1);
            }
            _ => panic!("Expected FaissError"),
        }
    }

    #[test]
    fn test_opaque_types_zero_size() {
        // Opaque types should have zero size for FFI safety
        assert_eq!(std::mem::size_of::<FaissIndex>(), 0);
        assert_eq!(std::mem::size_of::<FaissGpuResourcesProvider>(), 0);
        assert_eq!(std::mem::size_of::<FaissStandardGpuResources>(), 0);
    }

    #[test]
    fn test_gpu_available_returns_bool() {
        // Verifies gpu_available() works without crashing
        let available = gpu_available();
        println!("FAISS GPU available: {}", available);
        // Test passes if no crash
    }

    #[test]
    fn test_gpu_resources_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuResources>();
    }

    #[test]
    fn test_gpu_resources_allocation() {
        if !gpu_available() {
            println!("Skipping: No GPU available");
            return;
        }

        let resources = GpuResources::new();
        match resources {
            Ok(res) => {
                assert!(!res.as_ptr().is_null());
                assert!(!res.as_provider().is_null());
                println!("GPU resources allocated: {:?}", res);
            }
            Err(e) => {
                panic!("GPU allocation failed with GPU available: {}", e);
            }
        }
    }
}
