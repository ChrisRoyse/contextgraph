//! FFI bindings for GPU k-NN kernel using CUDA Driver API.
//!
//! This provides a FAISS-free GPU k-NN implementation for HDBSCAN.
//! Uses PTX loaded via Driver API to avoid cudart static initialization
//! bugs on WSL2 with CUDA 13.1.
//!
//! # Constitution Compliance
//!
//! - ARCH-GPU-05: k-NN runs on GPU
//! - ARCH-GPU-06: Batch operations preferred

use std::ffi::{c_void, CString};
use std::os::raw::c_int;
use std::sync::Once;

use crate::error::{CudaError, CudaResult};
use crate::ffi::cuda_driver::{cuInit, CUcontext, CUresult, CUDA_SUCCESS};

// Include generated PTX
include!(concat!(env!("OUT_DIR"), "/knn_ptx.rs"));

// =============================================================================
// CUDA DRIVER API BINDINGS
// =============================================================================

/// CUDA module handle (opaque pointer).
pub type CUmodule = *mut c_void;

/// CUDA function handle (opaque pointer).
pub type CUfunction = *mut c_void;

/// CUDA device pointer (opaque, 64-bit address).
pub type CUdeviceptr = u64;

#[link(name = "cuda")]
extern "C" {
    fn cuModuleLoadData(module: *mut CUmodule, image: *const c_void) -> CUresult;
    fn cuModuleGetFunction(
        hfunc: *mut CUfunction,
        hmod: CUmodule,
        name: *const std::os::raw::c_char,
    ) -> CUresult;
    fn cuModuleUnload(hmod: CUmodule) -> CUresult;

    fn cuMemAlloc_v2(dptr: *mut CUdeviceptr, bytesize: usize) -> CUresult;
    fn cuMemFree_v2(dptr: CUdeviceptr) -> CUresult;
    fn cuMemcpyHtoD_v2(dstDevice: CUdeviceptr, srcHost: *const c_void, byteCount: usize)
        -> CUresult;
    fn cuMemcpyDtoH_v2(
        dstHost: *mut c_void,
        srcDevice: CUdeviceptr,
        byteCount: usize,
    ) -> CUresult;

    fn cuLaunchKernel(
        f: CUfunction,
        gridDimX: u32,
        gridDimY: u32,
        gridDimZ: u32,
        blockDimX: u32,
        blockDimY: u32,
        blockDimZ: u32,
        sharedMemBytes: u32,
        hStream: *mut c_void, // CUstream, null for default stream
        kernelParams: *mut *mut c_void,
        extra: *mut *mut c_void,
    ) -> CUresult;

    fn cuCtxSynchronize() -> CUresult;

    fn cuDeviceGet(device: *mut c_int, ordinal: c_int) -> CUresult;
    fn cuCtxCreate_v2(pctx: *mut CUcontext, flags: u32, dev: c_int) -> CUresult;
    fn cuCtxDestroy_v2(ctx: CUcontext) -> CUresult;
    fn cuDeviceGetCount(count: *mut c_int) -> CUresult;
}

// =============================================================================
// INITIALIZATION
// =============================================================================

static INIT: Once = Once::new();
static mut INIT_RESULT: CUresult = CUDA_SUCCESS;

/// Initialize CUDA driver. Thread-safe, idempotent.
fn ensure_cuda_initialized() -> CudaResult<()> {
    INIT.call_once(|| {
        let result = unsafe { cuInit(0) };
        unsafe {
            INIT_RESULT = result;
        }
    });

    let result = unsafe { INIT_RESULT };
    if result == CUDA_SUCCESS {
        Ok(())
    } else {
        Err(CudaError::CudaRuntimeError {
            operation: "cuInit".to_string(),
            code: result,
        })
    }
}

// =============================================================================
// SAFE WRAPPERS
// =============================================================================

/// Check if CUDA is available.
pub fn cuda_available() -> bool {
    if ensure_cuda_initialized().is_err() {
        return false;
    }

    let mut count: c_int = 0;
    let ret = unsafe { cuDeviceGetCount(&mut count) };
    ret == CUDA_SUCCESS && count > 0
}

/// Get the number of CUDA devices.
pub fn cuda_device_count() -> CudaResult<i32> {
    ensure_cuda_initialized()?;

    let mut count: c_int = 0;
    let ret = unsafe { cuDeviceGetCount(&mut count) };
    if ret == CUDA_SUCCESS {
        Ok(count)
    } else {
        Err(CudaError::CudaRuntimeError {
            operation: "cuDeviceGetCount".to_string(),
            code: ret,
        })
    }
}

/// RAII wrapper for GPU memory allocation.
pub struct GpuBuffer {
    ptr: CUdeviceptr,
    size: usize,
}

impl GpuBuffer {
    /// Allocate GPU memory.
    pub fn new(size: usize) -> CudaResult<Self> {
        let mut ptr: CUdeviceptr = 0;
        let ret = unsafe { cuMemAlloc_v2(&mut ptr, size) };
        if ret == CUDA_SUCCESS && ptr != 0 {
            Ok(Self { ptr, size })
        } else {
            Err(CudaError::CudaRuntimeError {
                operation: format!("cuMemAlloc_v2({})", size),
                code: ret,
            })
        }
    }

    /// Copy data from host to device.
    pub fn copy_from_host(&self, src: &[u8]) -> CudaResult<()> {
        if src.len() > self.size {
            return Err(CudaError::InvalidArgument {
                argument: "src".to_string(),
                reason: format!("Source size {} exceeds buffer size {}", src.len(), self.size),
            });
        }

        let ret = unsafe { cuMemcpyHtoD_v2(self.ptr, src.as_ptr() as *const c_void, src.len()) };

        if ret == CUDA_SUCCESS {
            Ok(())
        } else {
            Err(CudaError::CudaRuntimeError {
                operation: "cuMemcpyHtoD_v2".to_string(),
                code: ret,
            })
        }
    }

    /// Copy data from device to host.
    pub fn copy_to_host(&self, dst: &mut [u8]) -> CudaResult<()> {
        if dst.len() > self.size {
            return Err(CudaError::InvalidArgument {
                argument: "dst".to_string(),
                reason: format!(
                    "Destination size {} exceeds buffer size {}",
                    dst.len(),
                    self.size
                ),
            });
        }

        let ret =
            unsafe { cuMemcpyDtoH_v2(dst.as_mut_ptr() as *mut c_void, self.ptr, dst.len()) };

        if ret == CUDA_SUCCESS {
            Ok(())
        } else {
            Err(CudaError::CudaRuntimeError {
                operation: "cuMemcpyDtoH_v2".to_string(),
                code: ret,
            })
        }
    }

    /// Get the device pointer.
    pub fn ptr(&self) -> CUdeviceptr {
        self.ptr
    }

    /// Get buffer size in bytes.
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for GpuBuffer {
    fn drop(&mut self) {
        if self.ptr != 0 {
            unsafe {
                cuMemFree_v2(self.ptr);
            }
        }
    }
}

// SAFETY: GpuBuffer owns the GPU memory exclusively
unsafe impl Send for GpuBuffer {}

/// RAII wrapper for CUDA context.
struct CudaContext {
    ctx: CUcontext,
}

impl CudaContext {
    fn new(device: i32) -> CudaResult<Self> {
        let mut dev: c_int = 0;
        let ret = unsafe { cuDeviceGet(&mut dev, device as c_int) };
        if ret != CUDA_SUCCESS {
            return Err(CudaError::CudaRuntimeError {
                operation: format!("cuDeviceGet({})", device),
                code: ret,
            });
        }

        let mut ctx: CUcontext = std::ptr::null_mut();
        let ret = unsafe { cuCtxCreate_v2(&mut ctx, 0, dev) };
        if ret == CUDA_SUCCESS && !ctx.is_null() {
            Ok(Self { ctx })
        } else {
            Err(CudaError::CudaRuntimeError {
                operation: format!("cuCtxCreate_v2(device={})", device),
                code: ret,
            })
        }
    }
}

impl Drop for CudaContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe {
                cuCtxDestroy_v2(self.ctx);
            }
        }
    }
}

/// RAII wrapper for CUDA module.
struct CudaModule {
    module: CUmodule,
}

impl CudaModule {
    fn load_ptx(ptx: &str) -> CudaResult<Self> {
        let ptx_cstring = CString::new(ptx).map_err(|_| CudaError::InvalidArgument {
            argument: "ptx".to_string(),
            reason: "PTX contains null bytes".to_string(),
        })?;

        let mut module: CUmodule = std::ptr::null_mut();
        let ret = unsafe { cuModuleLoadData(&mut module, ptx_cstring.as_ptr() as *const c_void) };

        if ret == CUDA_SUCCESS && !module.is_null() {
            Ok(Self { module })
        } else {
            Err(CudaError::CudaRuntimeError {
                operation: "cuModuleLoadData".to_string(),
                code: ret,
            })
        }
    }

    fn get_function(&self, name: &str) -> CudaResult<CUfunction> {
        let name_cstring = CString::new(name).map_err(|_| CudaError::InvalidArgument {
            argument: "name".to_string(),
            reason: "Function name contains null bytes".to_string(),
        })?;

        let mut func: CUfunction = std::ptr::null_mut();
        let ret =
            unsafe { cuModuleGetFunction(&mut func, self.module, name_cstring.as_ptr()) };

        if ret == CUDA_SUCCESS && !func.is_null() {
            Ok(func)
        } else {
            Err(CudaError::CudaRuntimeError {
                operation: format!("cuModuleGetFunction({})", name),
                code: ret,
            })
        }
    }
}

impl Drop for CudaModule {
    fn drop(&mut self) {
        if !self.module.is_null() {
            unsafe {
                cuModuleUnload(self.module);
            }
        }
    }
}

// =============================================================================
// K-NN COMPUTATION
// =============================================================================

const BLOCK_SIZE: u32 = 256;

/// Compute core distances for HDBSCAN using GPU k-NN.
///
/// # Arguments
///
/// * `vectors` - Flattened vectors (n_points * dimension elements)
/// * `n_points` - Number of points
/// * `dimension` - Vector dimension
/// * `k` - Number of neighbors (typically min_samples)
///
/// # Returns
///
/// Vector of core distances (n_points elements).
pub fn compute_core_distances_gpu(
    vectors: &[f32],
    n_points: usize,
    dimension: usize,
    k: usize,
) -> CudaResult<Vec<f32>> {
    if n_points == 0 || dimension == 0 {
        return Ok(vec![]);
    }

    if vectors.len() != n_points * dimension {
        return Err(CudaError::InvalidArgument {
            argument: "vectors".to_string(),
            reason: format!(
                "Expected {} elements, got {}",
                n_points * dimension,
                vectors.len()
            ),
        });
    }

    // Initialize CUDA
    ensure_cuda_initialized()?;

    // Create context on device 0
    let _ctx = CudaContext::new(0)?;

    // Load PTX module
    let module = CudaModule::load_ptx(PTX)?;
    let kernel = module.get_function("compute_core_distances_kernel")?;

    // Allocate GPU memory
    let vectors_size = vectors.len() * std::mem::size_of::<f32>();
    let output_size = n_points * std::mem::size_of::<f32>();

    let d_vectors = GpuBuffer::new(vectors_size)?;
    let d_output = GpuBuffer::new(output_size)?;

    // Copy input to GPU
    let vectors_bytes =
        unsafe { std::slice::from_raw_parts(vectors.as_ptr() as *const u8, vectors_size) };
    d_vectors.copy_from_host(vectors_bytes)?;

    // Set up kernel parameters
    let n_points_i32 = n_points as i32;
    let dimension_i32 = dimension as i32;
    let k_i32 = k as i32;
    let d_vectors_ptr = d_vectors.ptr();
    let d_output_ptr = d_output.ptr();

    let mut params: [*mut c_void; 5] = [
        &d_vectors_ptr as *const _ as *mut c_void,
        &n_points_i32 as *const _ as *mut c_void,
        &dimension_i32 as *const _ as *mut c_void,
        &k_i32 as *const _ as *mut c_void,
        &d_output_ptr as *const _ as *mut c_void,
    ];

    // Launch kernel
    let num_blocks = ((n_points as u32) + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let ret = unsafe {
        cuLaunchKernel(
            kernel,
            num_blocks,
            1,
            1,          // grid
            BLOCK_SIZE,
            1,
            1,          // block
            0,          // shared mem
            std::ptr::null_mut(), // stream
            params.as_mut_ptr(),
            std::ptr::null_mut(), // extra
        )
    };

    if ret != CUDA_SUCCESS {
        return Err(CudaError::CudaRuntimeError {
            operation: "cuLaunchKernel(compute_core_distances_kernel)".to_string(),
            code: ret,
        });
    }

    // Synchronize
    let ret = unsafe { cuCtxSynchronize() };
    if ret != CUDA_SUCCESS {
        return Err(CudaError::CudaRuntimeError {
            operation: "cuCtxSynchronize".to_string(),
            code: ret,
        });
    }

    // Copy results back
    let mut output = vec![0.0f32; n_points];
    let output_bytes =
        unsafe { std::slice::from_raw_parts_mut(output.as_mut_ptr() as *mut u8, output_size) };
    d_output.copy_to_host(output_bytes)?;

    Ok(output)
}

/// Compute pairwise distances for all point pairs using GPU.
///
/// # Arguments
///
/// * `vectors` - Flattened vectors (n_points * dimension elements)
/// * `n_points` - Number of points
/// * `dimension` - Vector dimension
///
/// # Returns
///
/// Vector of pairwise distances (n*(n-1)/2 elements, upper triangular).
pub fn compute_pairwise_distances_gpu(
    vectors: &[f32],
    n_points: usize,
    dimension: usize,
) -> CudaResult<Vec<f32>> {
    if n_points < 2 || dimension == 0 {
        return Ok(vec![]);
    }

    if vectors.len() != n_points * dimension {
        return Err(CudaError::InvalidArgument {
            argument: "vectors".to_string(),
            reason: format!(
                "Expected {} elements, got {}",
                n_points * dimension,
                vectors.len()
            ),
        });
    }

    let num_pairs = n_points * (n_points - 1) / 2;

    // Initialize CUDA
    ensure_cuda_initialized()?;

    // Create context on device 0
    let _ctx = CudaContext::new(0)?;

    // Load PTX module
    let module = CudaModule::load_ptx(PTX)?;
    let kernel = module.get_function("compute_pairwise_distances_kernel")?;

    // Allocate GPU memory
    let vectors_size = vectors.len() * std::mem::size_of::<f32>();
    let output_size = num_pairs * std::mem::size_of::<f32>();

    let d_vectors = GpuBuffer::new(vectors_size)?;
    let d_output = GpuBuffer::new(output_size)?;

    // Copy input to GPU
    let vectors_bytes =
        unsafe { std::slice::from_raw_parts(vectors.as_ptr() as *const u8, vectors_size) };
    d_vectors.copy_from_host(vectors_bytes)?;

    // Set up kernel parameters
    let n_points_i32 = n_points as i32;
    let dimension_i32 = dimension as i32;
    let d_vectors_ptr = d_vectors.ptr();
    let d_output_ptr = d_output.ptr();

    let mut params: [*mut c_void; 4] = [
        &d_vectors_ptr as *const _ as *mut c_void,
        &n_points_i32 as *const _ as *mut c_void,
        &dimension_i32 as *const _ as *mut c_void,
        &d_output_ptr as *const _ as *mut c_void,
    ];

    // Launch kernel
    let total_pairs = num_pairs as u64;
    let num_blocks = ((total_pairs as u32) + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let ret = unsafe {
        cuLaunchKernel(
            kernel,
            num_blocks,
            1,
            1,          // grid
            BLOCK_SIZE,
            1,
            1,          // block
            0,          // shared mem
            std::ptr::null_mut(), // stream
            params.as_mut_ptr(),
            std::ptr::null_mut(), // extra
        )
    };

    if ret != CUDA_SUCCESS {
        return Err(CudaError::CudaRuntimeError {
            operation: "cuLaunchKernel(compute_pairwise_distances_kernel)".to_string(),
            code: ret,
        });
    }

    // Synchronize
    let ret = unsafe { cuCtxSynchronize() };
    if ret != CUDA_SUCCESS {
        return Err(CudaError::CudaRuntimeError {
            operation: "cuCtxSynchronize".to_string(),
            code: ret,
        });
    }

    // Copy results back
    let mut output = vec![0.0f32; num_pairs];
    let output_bytes =
        unsafe { std::slice::from_raw_parts_mut(output.as_mut_ptr() as *mut u8, output_size) };
    d_output.copy_to_host(output_bytes)?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cuda_available() {
        let available = cuda_available();
        println!("CUDA available: {}", available);
        // Test should pass regardless of GPU availability
    }

    #[test]
    fn test_cuda_device_count() {
        match cuda_device_count() {
            Ok(count) => println!("CUDA devices: {}", count),
            Err(e) => println!("No CUDA: {}", e),
        }
    }

    #[test]
    fn test_gpu_buffer_allocation() {
        if !cuda_available() {
            println!("No GPU, skipping");
            return;
        }

        // Create context first
        let _ctx = CudaContext::new(0).expect("Failed to create context");

        // Allocate 1MB
        let buffer = GpuBuffer::new(1024 * 1024);
        assert!(buffer.is_ok(), "Failed to allocate GPU buffer");
        println!("Allocated 1MB GPU buffer");
    }

    #[test]
    fn test_core_distances_small() {
        if !cuda_available() {
            println!("No GPU, skipping");
            return;
        }

        // 5 points, 3 dimensions
        let vectors = vec![
            0.0, 0.0, 0.0, // Point 0
            1.0, 0.0, 0.0, // Point 1
            0.0, 1.0, 0.0, // Point 2
            0.0, 0.0, 1.0, // Point 3
            1.0, 1.0, 1.0, // Point 4
        ];

        let result = compute_core_distances_gpu(&vectors, 5, 3, 2);
        match result {
            Ok(distances) => {
                assert_eq!(distances.len(), 5);
                for (i, d) in distances.iter().enumerate() {
                    println!("Core distance {}: {}", i, d);
                    assert!(d.is_finite(), "Non-finite distance at {}", i);
                }
            }
            Err(e) => panic!("GPU computation failed: {}", e),
        }
    }
}
