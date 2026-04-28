# CUDA Acceleration

**Source files covered:**
- `crates/context-graph-cuda/src/**/*.rs`
- `crates/*/Cargo.toml`

## 1. Scope
`context-graph-cuda` exposes ffi, safe/context, similarity, hdbscan, allocation, memory, and test stub modules. Crates enable CUDA through default features where declared.

## 2. Constants
| Source | Constant | Type | Value |
| --- | --- | --- | --- |
| crates/context-graph-cuda/src/similarity.rs | DENSE_EMBEDDER_INDICES | [usize; 10] | [0, 1, 2, 3, 4, 6, 7, 8, 9, 10] |
| crates/context-graph-cuda/src/similarity.rs | GPU_BATCH_THRESHOLD | usize | 100 |
| crates/context-graph-cuda/src/ffi/faiss.rs | FAISS_OK | c_int | 0 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CUDA_SUCCESS | CUresult | 0 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CUDA_ERROR_NOT_INITIALIZED | CUresult | 3 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CUDA_ERROR_INVALID_DEVICE | CUresult | 101 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CUDA_ERROR_NO_DEVICE | CUresult | 100 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR | CUdevice_attribute | 75 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR | CUdevice_attribute | 76 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK | CUdevice_attribute | 1 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X | CUdevice_attribute | 2 |
| crates/context-graph-cuda/src/ffi/cuda_driver.rs | CU_DEVICE_ATTRIBUTE_WARP_SIZE | CUdevice_attribute | 10 |
| crates/context-graph-cuda/src/context/green_contexts.rs | GREEN_CONTEXTS_MIN_COMPUTE_MAJOR | u32 | 7 |
| crates/context-graph-cuda/src/context/green_contexts.rs | GREEN_CONTEXTS_MIN_COMPUTE_MINOR | u32 | 0 |
| crates/context-graph-cuda/src/context/green_contexts.rs | INFERENCE_PARTITION_PERCENT | f32 | 0.70 |
| crates/context-graph-cuda/src/context/green_contexts.rs | BACKGROUND_PARTITION_PERCENT | f32 | 0.30 |
| crates/context-graph-cuda/src/context/green_contexts.rs | MIN_SMS_FOR_PARTITIONING | u32 | 8 |

## 3. Runtime Claim Boundary
This document does not claim CUDA is available on the host; it only documents source constants and features.
