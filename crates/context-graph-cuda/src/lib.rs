#![deny(deprecated)]

//! CUDA acceleration for Context Graph.
//!
//! This crate provides GPU-accelerated operations for:
//! - Vector similarity search (cosine, dot product)
//! - Neural attention mechanisms
//! - GPU-accelerated HDBSCAN clustering (ARCH-GPU-05)
//!
//! # Constitution AP-007 Compliance
//!
//! **CUDA is ALWAYS required - no stub implementations in production.**
//!
//! The `StubVectorOps` type is available ONLY in test builds (`#[cfg(test)]`)
//! and must NOT be used in production code paths. All production code must
//! use real CUDA implementations.
//!
//! # Target Hardware
//!
//! - RTX 5090 (32GB GDDR7, 1.8 TB/s bandwidth)
//! - CUDA 13.2 with Compute Capability 12.0
//! - Blackwell architecture optimizations
//!
//! # Example (Test Only)
//!
//! ```ignore
//! // StubVectorOps is only available in #[cfg(test)] builds
//! #[cfg(test)]
//! use context_graph_cuda::{StubVectorOps, VectorOps};
//!
//! #[cfg(test)]
//! fn test_example() {
//!     let ops = StubVectorOps::new();
//!     assert!(!ops.is_gpu_available());
//! }
//! ```

pub mod context;
pub mod error;
pub mod ffi;
pub mod hdbscan;
pub mod ops;
pub mod safe;
pub mod similarity;

// AP-007: StubVectorOps is TEST ONLY - not available in production builds
#[cfg(test)]
pub mod stub;

pub use error::{CudaError, CudaResult};
pub use ffi::{
    compute_core_distances_gpu,
    // Custom k-NN kernel
    compute_hdc_embeddings_gpu,
    compute_pairwise_distances_gpu,
    // CUDA Driver API exports
    cuCtxCreate_v2,
    cuCtxDestroy_v2,
    cuCtxGetCurrent,
    cuCtxSetCurrent,
    cuDeviceGet,
    cuDeviceGetAttribute,
    cuDeviceGetCount,
    cuDeviceGetName,
    cuDeviceTotalMem_v2,
    cuDriverGetVersion,
    cuInit,
    cuMemGetInfo_v2,
    cuda_available,
    cuda_device_count,
    cuda_result_to_string,
    decode_driver_version,
    faiss_status,
    is_cuda_success,
    // FAISS GPU status
    is_faiss_gpu_available,
    CUcontext,
    CUdevice,
    CUdevice_attribute,
    CUresult,
    CUDA_ERROR_INVALID_DEVICE,
    CUDA_ERROR_NOT_INITIALIZED,
    CUDA_ERROR_NO_DEVICE,
    CUDA_SUCCESS,
    CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
    CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
    CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X,
    CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK,
    CU_DEVICE_ATTRIBUTE_WARP_SIZE,
};
// Re-export FAISS types when faiss-working feature is enabled
#[cfg(feature = "faiss-working")]
pub use ffi::{
    check_faiss_result, faiss_gpu_available, FaissGpuResources, FaissGpuResourcesProvider,
    FaissIndex, FaissStandardGpuResources, MetricType, FAISS_OK,
};
// Safe RAII wrappers (TASK-04)
pub use safe::{gpu_memory_usage_percent, GpuDevice};
// Green Contexts GPU partitioning (TASK-13)
pub use context::{
    should_enable_green_contexts, should_enable_green_contexts_with_config, GreenContext,
    GreenContexts, GreenContextsConfig, BACKGROUND_PARTITION_PERCENT,
    GREEN_CONTEXTS_MIN_COMPUTE_MAJOR, GREEN_CONTEXTS_MIN_COMPUTE_MINOR,
    INFERENCE_PARTITION_PERCENT, MIN_SMS_FOR_PARTITIONING,
};
pub use ops::VectorOps;
// GPU-batched similarity (ARCH-GPU-06: batch operations preferred)
// Note: CUDA 13.2 / sm_120 support is provided by the workspace Candle + cudarc stack.
pub use similarity::{
    compute_batch_cosine_similarity, compute_batch_cosine_similarity_chunked, embedder_to_group,
    should_use_gpu_batch, BatchedQueryContext, DimensionGroup, DENSE_EMBEDDER_INDICES,
    GPU_BATCH_THRESHOLD,
};
// GPU HDBSCAN clustering (ARCH-GPU-05)
pub use hdbscan::{
    ClusterMembership, ClusterSelectionMethod, GpuHdbscanClusterer, GpuHdbscanError,
    GpuHdbscanResult, GpuKnnIndex, HdbscanParams,
};

// AP-007: StubVectorOps export is gated to test-only builds
// Allow deprecated usage in tests - the deprecation warning is intentional for production
#[cfg(test)]
#[allow(deprecated)]
pub use stub::StubVectorOps;
