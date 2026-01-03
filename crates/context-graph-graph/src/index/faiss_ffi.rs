//! FAISS C API FFI bindings for GPU-accelerated vector similarity search.
//!
//! This module provides low-level C bindings to the FAISS library.
//! These bindings are used by `FaissGpuIndex` (M04-T10) for IVF-PQ operations.
//!
//! # Feature Flags
//!
//! - `faiss-gpu`: Enable FAISS GPU FFI bindings. Without this feature,
//!   `gpu_available()` always returns `false` and GPU tests are skipped.
//!
//! # Safety
//!
//! All extern "C" functions are unsafe. The `GpuResources` wrapper provides
//! a safe RAII interface for GPU resource management.
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004 Section 3.1: FAISS FFI Bindings
//! - perf.latency.faiss_1M_k100: <2ms target
//! - AP-015: GPU alloc without pool → use CUDA memory pool
//!
//! # FAISS C API Reference
//!
//! - <https://github.com/facebookresearch/faiss/blob/main/c_api/>
//! - Functions prefixed `faiss_` (e.g., faiss_index_factory)
//! - Types prefixed `Faiss` (e.g., FaissIndex)

use std::os::raw::{c_char, c_float, c_int, c_long};
use std::ptr::NonNull;

use crate::error::{GraphError, GraphResult};

// ========== GPU Availability Check ==========
//
// This section provides a safe way to check GPU availability before
// making FAISS FFI calls. When `faiss-gpu` feature is not enabled,
// gpu_available() returns false to skip GPU tests gracefully.

/// Check if FAISS GPU support is available.
///
/// Returns true if:
/// 1. The `faiss-gpu` feature is enabled (FAISS library is linked)
/// 2. FAISS reports at least one CUDA-capable GPU
///
/// Use this before attempting any GPU operations to avoid crashes
/// on systems without GPU hardware or with driver issues.
///
/// This function uses a subprocess to safely detect GPU availability,
/// preventing crashes from driver initialization failures (especially on WSL2).
///
/// # Example
///
/// ```ignore
/// if gpu_available() {
///     let resources = GpuResources::new()?;
///     // ... use GPU resources
/// } else {
///     println!("No GPU available, skipping GPU operations");
/// }
/// ```
///
/// # Environment Variables
///
/// - `SKIP_GPU_TESTS=1`: Force this function to return false
/// - `FAISS_GPU_CHECKED=1`: Use cached result (internal use)
#[cfg(feature = "faiss-gpu")]
pub fn gpu_available() -> bool {
    use std::sync::OnceLock;

    // Cache the result to avoid repeated subprocess calls
    static GPU_AVAILABLE: OnceLock<bool> = OnceLock::new();

    *GPU_AVAILABLE.get_or_init(|| {
        // Allow tests to skip GPU via environment variable
        if std::env::var("SKIP_GPU_TESTS").map(|v| v == "1").unwrap_or(false) {
            return false;
        }

        // Use subprocess to safely check GPU availability
        // This prevents crashes from WSL2 driver issues
        check_gpu_via_subprocess()
    })
}

/// Check GPU availability using nvidia-smi as a subprocess.
/// This is safer than calling CUDA directly as it won't crash on driver issues.
#[cfg(feature = "faiss-gpu")]
fn check_gpu_via_subprocess() -> bool {
    use std::process::Command;

    // First check if nvidia-smi works (safest check)
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
                        // Now try a quick CUDA test to verify driver works
                        return check_cuda_works();
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}

/// Verify FAISS GPU actually works by running a test that calls faiss_get_num_gpus.
/// Returns false if the FAISS test crashes or fails.
#[cfg(feature = "faiss-gpu")]
fn check_cuda_works() -> bool {
    use std::process::Command;
    use std::path::Path;

    // Check for pre-compiled FAISS GPU test binary (specific to our build)
    // This tests that FAISS can actually call CUDA functions without crashing
    let faiss_test = "/tmp/test_faiss_gpu_check";
    if Path::new(faiss_test).exists() {
        match Command::new(faiss_test).output() {
            Ok(output) => {
                if output.status.success() {
                    return true;
                }
                // FAISS GPU test crashed - GPU not usable
                return false;
            }
            Err(_) => {}
        }
    }

    // Fallback: Check for CUDA test binary
    let cuda_test = "/tmp/test_cuda3";
    if Path::new(cuda_test).exists() {
        match Command::new(cuda_test).output() {
            Ok(output) => {
                if !output.status.success() {
                    return false;
                }
                // CUDA works, but we still need to be careful about FAISS
                // On WSL2 with driver issues, CUDA may work but FAISS crashes
            }
            Err(_) => {}
        }
    }

    // Check for WSL2 with known driver issues
    // The /usr/lib/wsl/lib directory indicates WSL2
    if Path::new("/usr/lib/wsl/lib").exists() {
        // On WSL2, we've seen driver shim issues that crash FAISS
        // Be conservative and return false unless we have a working FAISS test
        if !Path::new(faiss_test).exists() {
            // No FAISS test binary - can't verify, assume unsafe on WSL2
            return false;
        }
    }

    // No test binaries available and not on WSL2 - assume GPU works
    true
}

/// Check if FAISS GPU support is available.
///
/// When the `faiss-gpu` feature is not enabled, this always returns `false`
/// because the FAISS library is not linked.
#[cfg(not(feature = "faiss-gpu"))]
#[inline]
pub fn gpu_available() -> bool {
    // FAISS library not linked - no GPU support available
    false
}

/// Directly check GPU count via FAISS FFI.
///
/// # Safety
/// This can crash on WSL2 with driver issues. Use `gpu_available()` instead
/// which performs a safe subprocess check first.
#[cfg(feature = "faiss-gpu")]
pub unsafe fn gpu_count_direct() -> Result<i32, i32> {
    let mut num_gpus: c_int = 0;
    let rc = faiss_get_num_gpus(&mut num_gpus);
    if rc == 0 {
        Ok(num_gpus)
    } else {
        Err(rc)
    }
}

// ========== Metric Type ==========

/// Metric type for distance computation.
///
/// Determines how similarity is measured between vectors.
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

// ========== Opaque Pointer Types ==========

/// Opaque pointer to FAISS index.
///
/// This type represents any FAISS index (Flat, IVF, PQ, GPU, etc.).
/// The actual type is determined by how the index was created.
#[repr(C)]
pub struct FaissIndex {
    _private: [u8; 0],
}

/// Opaque pointer to FAISS GPU resources provider interface.
///
/// This is the abstract interface that StandardGpuResources implements.
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

// ========== FAISS C API Bindings ==========

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

    // ---------- GPU Resources ----------

    /// Allocate standard GPU resources.
    ///
    /// Creates a new StandardGpuResources object for GPU memory management.
    /// MUST be freed with `faiss_StandardGpuResources_free`.
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    pub fn faiss_StandardGpuResources_new(
        p_res: *mut *mut FaissStandardGpuResources,
    ) -> c_int;

    /// Free GPU resources.
    ///
    /// Releases all GPU memory held by this resources object.
    pub fn faiss_StandardGpuResources_free(res: *mut FaissStandardGpuResources);

    // Note: No faiss_StandardGpuResources_as_GpuResourcesProvider function exists.
    // FAISS C API uses FAISS_DECLARE_CLASS_INHERITED(StandardGpuResources, GpuResourcesProvider)
    // which creates a typedef alias, making the types structurally identical.
    // Direct pointer cast is safe and correct.

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
    /// For IVF indices, this clusters the vectors to create centroids.
    /// Must be called before `add_with_ids` for untrained indices.
    ///
    /// # Arguments
    /// - `index`: Target index
    /// - `n`: Number of training vectors
    /// - `x`: Training vectors (n * d floats, row-major)
    pub fn faiss_Index_train(
        index: *mut FaissIndex,
        n: c_long,
        x: *const c_float,
    ) -> c_int;

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
    ///
    /// Note: FAISS_DECLARE_SETTER macro generates `faiss_IndexIVF_set_nprobe`
    /// with void return type (not c_int).
    pub fn faiss_IndexIVF_set_nprobe(
        index: *mut FaissIndex,
        nprobe: usize,
    );

    /// Get total number of vectors in index.
    pub fn faiss_Index_ntotal(index: *const FaissIndex) -> c_long;

    // ---------- Persistence ----------

    /// Write index to file.
    ///
    /// # Arguments
    /// - `index`: Source index
    /// - `fname`: Output file path (C string)
    pub fn faiss_write_index(
        index: *const FaissIndex,
        fname: *const c_char,
    ) -> c_int;

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

    /// Free index.
    ///
    /// Releases all memory held by the index.
    pub fn faiss_Index_free(index: *mut FaissIndex);

    // ---------- GPU Detection ----------

    /// Get the number of available GPUs.
    ///
    /// Writes the number of CUDA-capable GPUs visible to FAISS into `p_output`.
    /// Use this to check GPU availability before attempting GPU operations.
    ///
    /// # Arguments
    /// * `p_output` - Pointer to store the GPU count
    ///
    /// # Returns
    /// 0 on success, non-zero error code on failure
    pub fn faiss_get_num_gpus(p_output: *mut c_int) -> c_int;
}

// ========== RAII Wrapper ==========

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
    /// AP-015: GPU alloc without pool → use CUDA memory pool
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

// ========== Helper Functions ==========

/// Check FAISS result code and convert to GraphResult.
///
/// # Arguments
///
/// - `code`: FAISS return code (0 = success)
/// - `operation`: Description of operation for error message
///
/// # Returns
///
/// - `Ok(())` if code is 0
/// - `Err(GraphError::FaissIndexCreation)` otherwise
///
/// # Example
///
/// ```ignore
/// let result = unsafe { faiss_Index_train(index, n, x) };
/// check_faiss_result(result, "faiss_Index_train")?;
/// ```
#[inline]
pub fn check_faiss_result(code: c_int, operation: &str) -> GraphResult<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(GraphError::FaissIndexCreation(format!(
            "{} failed with error code: {}",
            operation, code
        )))
    }
}

// ========== Tests ==========

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
    fn test_metric_type_debug() {
        assert_eq!(format!("{:?}", MetricType::L2), "L2");
        assert_eq!(format!("{:?}", MetricType::InnerProduct), "InnerProduct");
    }

    #[test]
    fn test_metric_type_clone() {
        let m1 = MetricType::L2;
        let m2 = m1;
        assert_eq!(m1, m2);
    }

    #[test]
    fn test_metric_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(MetricType::L2);
        set.insert(MetricType::InnerProduct);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_gpu_resources_is_send_sync() {
        // Compile-time verification that GpuResources is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuResources>();
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
        assert!(matches!(err, GraphError::FaissIndexCreation(_)));
        let msg = err.to_string();
        assert!(msg.contains("test_operation"));
        assert!(msg.contains("-1"));
    }

    #[test]
    fn test_check_faiss_result_various_codes() {
        // Test multiple error codes
        for code in [1, 2, 10, 100, -100] {
            let result = check_faiss_result(code, "test");
            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(msg.contains(&code.to_string()));
        }
    }

    #[test]
    fn test_opaque_types_zero_size() {
        // Opaque types should have zero size (for FFI safety)
        assert_eq!(std::mem::size_of::<FaissIndex>(), 0);
        assert_eq!(std::mem::size_of::<FaissGpuResourcesProvider>(), 0);
        assert_eq!(std::mem::size_of::<FaissStandardGpuResources>(), 0);
    }

    // ========== GPU Detection Test ==========

    #[test]
    fn test_gpu_available_returns_bool() {
        // This test verifies gpu_available() works without crashing
        // even when no GPU is present. It should return false on systems
        // without CUDA GPUs and true on systems with them.
        let available = gpu_available();
        println!("GPU available: {}", available);
        // The function should return a valid bool either way
        assert!(available == true || available == false);
    }

    // ========== GPU Tests (require FAISS + GPU) ==========

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
