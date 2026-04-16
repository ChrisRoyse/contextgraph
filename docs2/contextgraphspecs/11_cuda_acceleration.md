# CUDA Acceleration

Source files covered:

- `crates/context-graph-cuda/Cargo.toml`
- `crates/context-graph-cuda/build.rs`
- `crates/context-graph-cuda/src/lib.rs`
- `crates/context-graph-cuda/src/error.rs`
- `crates/context-graph-cuda/src/ops.rs`
- `crates/context-graph-cuda/src/similarity.rs`
- `crates/context-graph-cuda/src/stub.rs`
- `crates/context-graph-cuda/src/context/mod.rs`
- `crates/context-graph-cuda/src/context/green_contexts.rs`
- `crates/context-graph-cuda/src/ffi/mod.rs`
- `crates/context-graph-cuda/src/ffi/cuda_driver.rs`
- `crates/context-graph-cuda/src/ffi/knn.rs`
- `crates/context-graph-cuda/src/ffi/faiss.rs`
- `crates/context-graph-cuda/src/hdbscan/mod.rs`
- `crates/context-graph-cuda/src/hdbscan/clusterer.rs`
- `crates/context-graph-cuda/src/hdbscan/error.rs`
- `crates/context-graph-cuda/src/hdbscan/gpu_knn.rs`
- `crates/context-graph-cuda/src/hdbscan/tests.rs`
- `crates/context-graph-cuda/src/safe/mod.rs`
- `crates/context-graph-cuda/src/safe/device.rs`
- `crates/context-graph-cuda/kernels/knn.cu`

---

## 1. CUDA Subsystem Architecture

The `context-graph-cuda` crate provides GPU-accelerated operations for the Context Graph system. It is the **single source of truth** for all CUDA FFI declarations (constitution rule ARCH-06). No other crate in the workspace may declare CUDA FFI bindings.

**Post-cleanup note**: As of commit 8356e7b0 ("refactor: delete 7K lines dead code"), the hyperbolic (Poincare / cone) CUDA code was deleted from the CUDA crate. The surviving modules are those listed below.

**Build status (dev env, 2026-04)**: `cargo build -p context-graph-cuda` succeeds against the CUDA 13.2 toolkit with `sm_120` PTX. The candle-core integration is still deferred (see Section 10) because candle's `cudarc` backend does not yet support CUDA 13.1+ / compute capability 12.0; the batch similarity module runs as SIMD-optimized CPU in the meantime.

The crate is organized into six modules:

| Module | Purpose |
|--------|---------|
| `ffi/` | Raw FFI bindings to CUDA Driver API, custom k-NN kernel, and FAISS C API |
| `safe/` | RAII wrappers around unsafe CUDA operations with automatic resource cleanup |
| `context/` | Green Contexts GPU SM partitioning for workload isolation |
| `ops` | `VectorOps` trait defining the GPU-accelerated vector operations interface |
| `similarity` | Batched cosine similarity computation grouped by embedding dimension |
| `hdbscan/` | GPU-accelerated HDBSCAN clustering with k-NN core distance on GPU |

Additional modules:

| Module | Purpose |
|--------|---------|
| `error` | `CudaError` enum and `CudaResult<T>` type alias |
| `stub` | Test-only CPU stub (`StubVectorOps`), gated behind `#[cfg(test)]` |

### Dependency Graph

The CUDA crate depends only on `context-graph-core` and is consumed by three upstream crates:

- `context-graph-embeddings` (embedding model inference)
- `context-graph-storage` (vector index operations)
- `context-graph-mcp` (MCP server, top-level integration)

### Constitution Compliance

The crate enforces several constitution rules:

| Rule | Description |
|------|-------------|
| AP-007 | CUDA is always required; no stub implementations in production |
| ARCH-GPU-01 | GPU is mandatory; no CPU fallback for batch similarity |
| ARCH-GPU-05 | HDBSCAN clustering runs on GPU |
| ARCH-GPU-06 | Batch operations preferred; minimize kernel launches |
| ARCH-GPU-07 | FP16 for Tensor Core acceleration (planned, pending candle support) |
| AP-GPU-04 | Never use sklearn HDBSCAN; use GPU implementation |
| AP-GPU-07 | No per-item serialization; full batch GPU processing |

---

## 2. GPU Device Management and Context

### 2.1 GpuDevice (RAII Wrapper)

**File:** `src/safe/device.rs`

`GpuDevice` is the primary safe abstraction for CUDA device interaction. It manages the full lifecycle of a CUDA device handle and context.

```rust
pub struct GpuDevice {
    device: CUdevice,     // CUDA device handle
    context: CUcontext,   // CUDA context handle (owned)
    ordinal: i32,         // Device index
}
```

**Lifecycle:**

1. **Initialization**: `GpuDevice::new(ordinal)` calls `cuInit(0)` (once per process via `std::sync::Once`), then `cuDeviceGet` and `cuCtxCreate_v2`
2. **Query**: Methods like `name()`, `compute_capability()`, `memory_info()`, `memory_usage_percent()` query device properties
3. **Cleanup**: `Drop` calls `cuCtxDestroy_v2`. Drop never panics; errors are logged to stderr

**Thread safety:**

- `Send`: yes (can be moved between threads)
- `Sync`: no (CUDA contexts are thread-bound)

**Key methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `new(ordinal)` | `CudaResult<Self>` | Create device handle with CUDA context |
| `compute_capability()` | `(u32, u32)` | Compute capability (e.g., `(12, 0)` for RTX 5090) |
| `name()` | `String` | Device name (e.g., "NVIDIA GeForce RTX 5090") |
| `memory_info()` | `CudaResult<(usize, usize)>` | `(free_bytes, total_bytes)` |
| `memory_usage_percent()` | `f32` | Memory usage as 0.0-1.0 |
| `ordinal()` | `i32` | Device index |

### 2.2 CUDA Driver Initialization

CUDA driver initialization is thread-safe and idempotent, guarded by `std::sync::Once`. The initialization result is stored in an `AtomicI32` (replacing deprecated `static mut` for Rust edition 2024 compliance, fix BLD-03).

### 2.3 Why Driver API (Not Runtime API)

The crate uses the CUDA Driver API (`cuInit`, `cuDeviceGetAttribute`, etc.) instead of the Runtime API (`cudaGetDeviceProperties`, etc.) because:

1. CUDA 13.1 on WSL2 with RTX 5090 (Blackwell) has a bug where `cudaGetDeviceProperties` (Runtime API) segfaults
2. `cuDeviceGetAttribute` (Driver API) is orders of magnitude faster than `cudaGetDeviceProperties`
3. PTX loading via Driver API avoids cudart static initialization bugs on WSL2

---

## 3. VRAM Allocation and Memory Management

### 3.1 GpuBuffer (RAII GPU Memory)

**File:** `src/ffi/knn.rs`

`GpuBuffer` provides RAII-wrapped GPU memory allocation using the CUDA Driver API.

```rust
pub struct GpuBuffer {
    ptr: CUdeviceptr,  // Device pointer (64-bit address)
    size: usize,       // Buffer size in bytes
}
```

**Methods:**

| Method | Description |
|--------|-------------|
| `new(size)` | Allocate `size` bytes on GPU via `cuMemAlloc_v2` |
| `copy_from_host(src)` | Copy `src` bytes from host to device via `cuMemcpyHtoD_v2` |
| `copy_to_host(dst)` | Copy from device to host buffer via `cuMemcpyDtoH_v2` |
| `ptr()` | Get raw device pointer |
| `size()` | Get buffer size in bytes |

**Safety:**

- `Drop` calls `cuMemFree_v2` to release GPU memory
- `Send` is implemented (GPU memory is owned exclusively)
- Bounds checking on `copy_from_host` and `copy_to_host` prevents buffer overflows

### 3.2 CudaContext and CudaModule (Internal RAII)

Two additional RAII wrappers in `ffi/knn.rs` manage kernel execution resources:

- **`CudaContext`**: Wraps `cuCtxCreate_v2`/`cuCtxDestroy_v2` for kernel launch contexts
- **`CudaModule`**: Wraps `cuModuleLoadData`/`cuModuleUnload` for loading compiled PTX, with `get_function()` to retrieve kernel entry points via `cuModuleGetFunction`

### 3.3 FAISS GPU Resources

**File:** `src/ffi/faiss.rs` (requires `faiss-working` feature)

`GpuResources` wraps FAISS `StandardGpuResources` for GPU memory management in FAISS operations.

```rust
pub struct GpuResources {
    ptr: NonNull<FaissStandardGpuResources>,
}
```

- `new()` allocates via `faiss_StandardGpuResources_new`
- `Drop` frees via `faiss_StandardGpuResources_free`
- `Send + Sync` (FAISS uses internal synchronization)
- `as_provider()` casts to `FaissGpuResourcesProvider` for index transfers

### 3.4 Memory Monitoring

The convenience function `gpu_memory_usage_percent()` in `src/safe/mod.rs` creates a temporary `GpuDevice` to query current VRAM usage as a float in [0.0, 1.0].

---

## 4. Batch Similarity Computation on GPU

**File:** `src/similarity.rs`

The batch similarity module provides high-performance cosine similarity computation for the 13-embedder system, batching by dimension group to minimize kernel launches (ARCH-GPU-06).

### 4.1 Dimension Groups

Embedders are grouped by vector dimension for efficient batched matrix multiplication (`crates/context-graph-cuda/src/similarity.rs:47-117`):

| Group | Dimension | Embedders | Count |
|-------|-----------|-----------|-------|
| `Dim1024` | 1024 | E1 (semantic), E8 (graph, e5-large-v2), E9 (HDC) | 3 |
| `Dim768` | 768 | E5 (causal), E10 (multimodal), E11 (entity, disabled by default) | 3 |
| `Dim512` | 512 | E2 (temporal recent), E3 (temporal periodic), E4 (temporal positional) | 3 |
| `Dim1536` | 1536 | E7 (code) | 1 |

**Total:** 10 dense embedders participate in GPU batching (`DENSE_EMBEDDER_INDICES = [0, 1, 2, 3, 4, 6, 7, 8, 9, 10]`, `similarity.rs:121`). Three embedders are excluded:

| Excluded | Reason |
|----------|--------|
| E6 (sparse, 30K) | Inverted index, not dense vector |
| E12 (token-level ColBERT) | MaxSim computation, not standard cosine |
| E13 (sparse SPLADE) | Inverted index, not dense vector |

The mapping from embedder index to dimension group is provided by `embedder_to_group(embedder_idx: usize) -> Option<DimensionGroup>` (`similarity.rs:108`). When E11 is disabled via `E11_ENTITY_ENABLED = false`, the E11 slot is still batched but populated with a zero vector, yielding near-0.5 normalized similarity for every candidate (harmless for fusion since E11 weight is 0 in all default profiles).

### 4.2 BatchedQueryContext

`BatchedQueryContext` pre-normalizes query vectors (L2 normalization) once per query, then reuses them for all candidate comparisons:

```rust
pub struct BatchedQueryContext {
    normalized_vectors: [[f32; 1536]; 10],  // Max dim (E7) for all
    dimensions: [usize; 10],                // Actual dims per embedder
}
```

Dimension order: `[E1(1024), E2(512), E3(512), E4(512), E5(768), E7(1536), E8(1024), E9(1024), E10(768), E11(768)]`

### 4.3 Core Functions

| Function | Description |
|----------|-------------|
| `compute_batch_cosine_similarity(query_ctx, candidates)` | Compute cosine similarity for all 10 dense embedders per candidate. Returns `Vec<[f32; 10]>` |
| `compute_batch_cosine_similarity_chunked(query_ctx, candidates)` | Same as above but processes in chunks of 2,048 for L2 cache locality |
| `should_use_gpu_batch(count)` | Returns `true` if `count >= GPU_BATCH_THRESHOLD` (100) |
| `embedder_to_group(idx)` | Map embedder index (0-12) to `DimensionGroup` or `None` |

### 4.4 Return Range

All batch similarity functions return **raw cosine similarity in [-1.0, 1.0]**. Callers that feed results into the fusion pipeline must apply SRC-3 normalization: `(raw + 1.0) / 2.0`.

### 4.5 Performance Characteristics

The SIMD-friendly implementation processes vectors in chunks of 4 for potential auto-vectorization by LLVM:

- **CPU scalar**: ~1.3s for 10K candidates across 10 embedders
- **GPU batched (target)**: ~5ms (5 matmul kernels)
- **Current SIMD-optimized CPU**: ~10x faster than scalar

### 4.6 CUDA 13.1 Note

The batch similarity module currently uses a SIMD-optimized CPU implementation because candle-core's cudarc backend does not yet support CUDA 13.1 / compute capability 12.0. The data structures are designed for seamless GPU migration when candle support is added.

---

## 5. HDBSCAN Clustering on GPU

**Files:** `src/hdbscan/clusterer.rs`, `src/hdbscan/gpu_knn.rs`, `src/hdbscan/error.rs`

GPU-accelerated HDBSCAN provides density-based clustering for topic detection with a target latency under 20ms.

### 5.1 Algorithm Pipeline

| Step | Where | Complexity | Description |
|------|-------|------------|-------------|
| 1. Core distances | GPU | O(n) GPU ops | Custom CUDA kernel computes k-th nearest neighbor distance for all points |
| 2. Mutual reachability | CPU | O(n^2) | `MR(a,b) = max(core_a, core_b, dist(a,b))` with precomputed core distances |
| 3. MST | CPU | O(n^2) | Prim's algorithm on mutual reachability graph |
| 4. Cluster extraction | CPU | O(n) | Union-Find with gap detection |
| 5. Core point identification | CPU | O(n) | Points with core distance at or below cluster median |

### 5.2 GpuHdbscanClusterer

```rust
pub struct GpuHdbscanClusterer {
    params: HdbscanParams,
}
```

**HdbscanParams:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `min_cluster_size` | 3 | Minimum points for a valid cluster |
| `min_samples` | 2 | k for core distance computation |
| `cluster_selection_method` | `EOM` | Excess of Mass (default) or `Leaf` |

**Key method:** `fit(embeddings, memory_ids) -> GpuHdbscanResult<Vec<ClusterMembership>>`

Validation performed before clustering:

- Parameter validation (`min_cluster_size >= 2`, `min_samples >= 1`, `min_samples <= min_cluster_size`)
- Input count check (`n >= min_cluster_size`)
- Dimension consistency across all embeddings
- Finite value check (rejects NaN and Infinity)

### 5.3 ClusterMembership

```rust
pub struct ClusterMembership {
    pub memory_id: Uuid,
    pub cluster_id: i32,              // -1 = noise
    pub membership_probability: f32,  // [0.0, 1.0]
    pub is_core: bool,
}
```

### 5.4 GpuKnnIndex

```rust
pub struct GpuKnnIndex {
    vectors: Vec<f32>,    // Flattened row-major
    dimension: usize,
    vector_count: usize,
}
```

The k-NN index stores vectors in flattened format and delegates to the custom CUDA kernel (`compute_core_distances_gpu`) for core distance computation. It requires CUDA availability at creation time; there is no CPU fallback.

### 5.5 Gap Detection

Cluster boundaries are detected by the `detect_gap_threshold` method, which uses two strategies:

1. **Ratio-based**: A 2x jump in consecutive MST edge weights indicates a cluster boundary
2. **Semantic threshold**: A fixed threshold of 0.5 L2 distance (approximately 0.75 cosine similarity) as a fallback

### 5.6 Core Point Identification (CUDA-H1 Fix)

Core points are identified using actual spatial core distances from GPU k-NN rather than global label counting. A point is core if:

1. It is not noise (label != -1)
2. Its core distance is at or below the median core distance for its cluster

### 5.7 Error Types

`GpuHdbscanError` provides explicit error variants:

| Variant | Description |
|---------|-------------|
| `GpuNotAvailable` | No GPU detected (references ARCH-GPU-05) |
| `FaissResourceError` | FAISS GPU resource allocation failed |
| `FaissIndexError` | FAISS index creation failed |
| `FaissSearchError` | FAISS k-NN search failed |
| `InvalidParameter` | Invalid HDBSCAN parameter |
| `InsufficientData` | Fewer points than `min_cluster_size` |
| `DimensionMismatch` | Embedding count does not match ID count |
| `InvalidDimension` | Zero-dimensional vectors |
| `NonFiniteValue` | NaN or Infinity in embeddings |
| `InternalError` | Internal algorithm failure |

---

## 6. FFI Bindings to CUDA Kernels

**Files:** `src/ffi/cuda_driver.rs`, `src/ffi/knn.rs`, `src/ffi/faiss.rs`

All FFI declarations are centralized in the `ffi/` module. This is enforced by constitution rule ARCH-06.

### 6.1 CUDA Driver API Bindings

**File:** `src/ffi/cuda_driver.rs`

Binds to `libcuda.so` via `#[link(name = "cuda")]`:

| Function | Purpose |
|----------|---------|
| `cuInit(flags)` | Initialize CUDA driver (must be called first) |
| `cuDeviceGet(device, ordinal)` | Get device handle by index |
| `cuDeviceGetCount(count)` | Get number of CUDA devices |
| `cuDeviceGetAttribute(pi, attrib, dev)` | Query device attribute |
| `cuDeviceGetName(name, len, dev)` | Get device name string |
| `cuDeviceTotalMem_v2(bytes, dev)` | Get total device memory |
| `cuDriverGetVersion(version)` | Get driver version (encoded as `major * 1000 + minor * 10`) |
| `cuCtxCreate_v2(pctx, flags, dev)` | Create CUDA context |
| `cuCtxDestroy_v2(ctx)` | Destroy CUDA context |
| `cuCtxSetCurrent(ctx)` | Set context for calling thread |
| `cuCtxGetCurrent(pctx)` | Get current thread's context |
| `cuMemGetInfo_v2(free, total)` | Get free and total memory |

**Type aliases:**

| Type | C Type | Description |
|------|--------|-------------|
| `CUresult` | `c_int` | CUDA result code (0 = success) |
| `CUdevice` | `c_int` | Device handle (ordinal) |
| `CUdevice_attribute` | `c_int` | Attribute enumeration |
| `CUcontext` | `*mut c_void` | Opaque context handle |

**Result codes:**

| Constant | Value | Meaning |
|----------|-------|---------|
| `CUDA_SUCCESS` | 0 | Operation succeeded |
| `CUDA_ERROR_NOT_INITIALIZED` | 3 | `cuInit()` not called |
| `CUDA_ERROR_NO_DEVICE` | 100 | No CUDA-capable device |
| `CUDA_ERROR_INVALID_DEVICE` | 101 | Invalid device ordinal |

**Device attribute constants:**

| Constant | Value | Description |
|----------|-------|-------------|
| `CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR` | 75 | e.g., 12 for RTX 5090 |
| `CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR` | 76 | e.g., 0 for RTX 5090 |
| `CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK` | 1 | Max threads per block |
| `CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X` | 2 | Max block dimension X |
| `CU_DEVICE_ATTRIBUTE_WARP_SIZE` | 10 | Warp size in threads |

**Helper functions:**

| Function | Description |
|----------|-------------|
| `decode_driver_version(encoded)` | Decode `13010` to `(13, 1)` |
| `is_cuda_success(result)` | Check if result code is `CUDA_SUCCESS` |
| `cuda_result_to_string(result)` | Human-readable error message |

### 6.2 Custom k-NN Kernel Bindings

**File:** `src/ffi/knn.rs`

Additional Driver API bindings for kernel execution (also linked to `libcuda.so`):

| Function | Purpose |
|----------|---------|
| `cuModuleLoadData(module, image)` | Load PTX module from memory |
| `cuModuleGetFunction(hfunc, hmod, name)` | Get kernel function from module |
| `cuModuleUnload(hmod)` | Unload module |
| `cuMemAlloc_v2(dptr, bytesize)` | Allocate device memory |
| `cuMemFree_v2(dptr)` | Free device memory |
| `cuMemcpyHtoD_v2(dst, src, bytes)` | Host-to-device copy |
| `cuMemcpyDtoH_v2(dst, src, bytes)` | Device-to-host copy |
| `cuLaunchKernel(...)` | Launch kernel with grid/block configuration |
| `cuCtxSynchronize()` | Synchronize current context |

**Additional type aliases:**

| Type | Description |
|------|-------------|
| `CUmodule` | `*mut c_void` -- CUDA module handle |
| `CUfunction` | `*mut c_void` -- CUDA function handle |
| `CUdeviceptr` | `u64` -- Device pointer (64-bit address) |

**Public safe wrappers:**

| Function | Description |
|----------|-------------|
| `cuda_available()` | Check if CUDA is available and at least one device exists |
| `cuda_device_count()` | Get number of CUDA devices |
| `compute_core_distances_gpu(vectors, n_points, dimension, k)` | Compute k-NN core distances on GPU |
| `compute_pairwise_distances_gpu(vectors, n_points, dimension)` | Compute all-pairs L2 distances on GPU |

### 6.3 FAISS C API Bindings

**File:** `src/ffi/faiss.rs` (conditionally compiled with `faiss-working` feature)

Binds to `libfaiss_c.so`:

| Function | Purpose |
|----------|---------|
| `faiss_index_factory(p_index, d, description, metric)` | Create index from factory string |
| `faiss_Index_free(index)` | Free index |
| `faiss_StandardGpuResources_new(p_res)` | Allocate GPU resources |
| `faiss_StandardGpuResources_free(res)` | Free GPU resources |
| `faiss_index_cpu_to_gpu(provider, device, index, p_out)` | Transfer CPU index to GPU |
| `faiss_Index_train(index, n, x)` | Train index with vectors |
| `faiss_Index_is_trained(index)` | Check if index is trained |
| `faiss_Index_add_with_ids(index, n, x, xids)` | Add vectors with IDs |
| `faiss_Index_search(index, n, x, k, distances, labels)` | k-NN search |
| `faiss_IndexIVF_set_nprobe(index, nprobe)` | Set IVF search quality |
| `faiss_Index_ntotal(index)` | Get vector count |
| `faiss_write_index(index, fname)` | Write index to file |
| `faiss_read_index(fname, io_flags, p_out)` | Read index from file |
| `faiss_get_num_gpus(p_output)` | Get available GPU count |

**MetricType enum:**

| Variant | Value | Description |
|---------|-------|-------------|
| `InnerProduct` | 0 | Cosine similarity when normalized (higher = more similar) |
| `L2` | 1 | Euclidean distance (lower = more similar, default) |

### 6.4 FAISS GPU Availability Detection

The `is_faiss_gpu_available()` function returns `true` only when:

1. The `faiss-working` feature is enabled
2. FAISS reports at least one GPU
3. GPU actually works (verified at runtime)

On WSL2, FAISS GPU availability is tested via a subprocess to prevent crashes from CUDA version mismatches. The function `check_gpu_via_subprocess()` compiles and runs a minimal C test program that calls `faiss_get_num_gpus`.

---

## 7. Safe Rust Wrappers Around Unsafe CUDA Operations

The crate provides multiple layers of safety over raw CUDA FFI:

### 7.1 RAII Resource Management

| Wrapper | Resource | Allocation | Cleanup |
|---------|----------|------------|---------|
| `GpuDevice` | CUDA context + device | `cuCtxCreate_v2` | `cuCtxDestroy_v2` (in `Drop`) |
| `GpuBuffer` | Device memory | `cuMemAlloc_v2` | `cuMemFree_v2` (in `Drop`) |
| `CudaContext` | CUDA context (internal) | `cuCtxCreate_v2` | `cuCtxDestroy_v2` (in `Drop`) |
| `CudaModule` | PTX module | `cuModuleLoadData` | `cuModuleUnload` (in `Drop`) |
| `GpuResources` | FAISS GPU resources | `faiss_StandardGpuResources_new` | `faiss_StandardGpuResources_free` (in `Drop`) |

All `Drop` implementations are non-panicking. Errors during cleanup are logged but swallowed to maintain RAII safety.

### 7.2 Bounds Checking

All kernel launch functions validate parameters before `i32` casts to prevent silent truncation:

- `n_points`, `dimension`, and `k` are validated against `i32::MAX` before casting (fixes BLD-02, BLD-08)
- `num_pairs` uses checked arithmetic (`checked_mul` and `checked_div`) to prevent overflow on 32-bit platforms (fix BLD-04)
- `total_pairs` is validated against `u32::MAX` before grid dimension calculation (fix CRIT-07)
- `GpuBuffer::copy_from_host` and `copy_to_host` validate buffer sizes

### 7.3 Thread Safety

| Type | `Send` | `Sync` | Rationale |
|------|--------|--------|-----------|
| `GpuDevice` | Yes (unsafe impl) | No | CUDA contexts are thread-bound |
| `GpuBuffer` | Yes (unsafe impl) | No | Owned GPU memory |
| `GpuResources` | Yes (unsafe impl) | Yes (unsafe impl) | FAISS internal synchronization |
| `GpuKnnIndex` | Yes (auto-derived) | Yes (auto-derived) | Contains only `Vec<f32>` and `usize` |
| `GreenContext` | Send | Not Sync | Same as CUDA contexts |

### 7.4 Initialization Safety

CUDA driver initialization uses `std::sync::Once` to ensure `cuInit(0)` is called exactly once per process. The result is stored in an `AtomicI32` for thread-safe error reporting (replacing deprecated `static mut` per BLD-03).

---

## 8. CUDA Kernel Descriptions

**File:** `kernels/knn.cu`

The crate contains one CUDA source file compiled to PTX at build time.

### 8.1 `compute_core_distances_kernel`

```c
extern "C" __global__ void compute_core_distances_kernel(
    const float* __restrict__ vectors,  // n_points * dimension floats
    int n_points,
    int dimension,
    int k,
    float* __restrict__ core_dists      // n_points output floats
)
```

**Purpose:** Compute the core distance (L2 distance to k-th nearest neighbor) for each point. Each thread handles one query point.

**Algorithm:**

1. Each thread loads its query vector from global memory
2. Streams through all points computing L2 squared distances via `l2_distance_squared`
3. Maintains a sorted top-(k+1) list in registers (max 16 entries, insertion sort)
4. Outputs `sqrt(top_k[k])` as the core distance

**Grid configuration:** `blocks = ceil(n_points / 256)`, `threads_per_block = 256`

### 8.2 `compute_pairwise_distances_kernel`

```c
extern "C" __global__ void compute_pairwise_distances_kernel(
    const float* __restrict__ vectors,  // n_points * dimension floats
    int n_points,
    int dimension,
    float* __restrict__ distances       // n*(n-1)/2 output floats
)
```

**Purpose:** Compute L2 distances for all unique pairs. Each thread handles one pair (i, j) where i < j.

**Algorithm:**

1. Thread ID maps to a linear pair index
2. Converts linear index to `(i, j)` pair using `j = floor(0.5 + sqrt(0.25 + 2*tid))`, `i = tid - j*(j-1)/2`
3. Computes `sqrt(l2_distance_squared(vec_i, vec_j))`
4. Output is upper-triangular format: `n*(n-1)/2` elements

**Grid configuration:** `blocks = ceil(num_pairs / 256)`, `threads_per_block = 256`

### 8.3 Helper Device Function

```c
__device__ __forceinline__ float l2_distance_squared(
    const float* __restrict__ a,
    const float* __restrict__ b,
    int dimension
)
```

Computes squared L2 (Euclidean) distance between two vectors with `__restrict__` hints for memory aliasing optimization and `__forceinline__` for inlining into calling kernels.

### 8.4 Kernel Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `BLOCK_SIZE` | 256 | Threads per block (optimal for RTX 5090) |
| Top-k register limit | 16 | Maximum k for register-based insertion sort |

---

## 9. Hardware Targeting (RTX 5090 / Blackwell)

### 9.1 Target Specification

| Property | Value |
|----------|-------|
| GPU | NVIDIA GeForce RTX 5090 |
| Architecture | Blackwell |
| Compute Capability | 12.0 |
| VRAM | 32 GB GDDR7 |
| Memory Bandwidth | 1.8 TB/s |
| Estimated SMs | ~170 (21,760 cores / 128 per SM) |
| CUDA Toolkit | 13.1+ |
| PTX Target | `compute_120` |
| SM Target | `sm_120a` (default; override with `CUDA_ARCH=sm_120`) |

### 9.2 Build Configuration

The build script (`build.rs`) compiles CUDA kernels to PTX with the following defaults (updated in commit `f2ac2f76`):

- Default architecture: `sm_120a` (family-specific Blackwell consumer; unlocks FP4/FP6 Tensor Core operands). Override via `CUDA_ARCH=sm_120` for broader Blackwell family compatibility.
- Optimization: `-O3`, `-Xptxas -O3`, `--use_fast_math`, `--restrict`, `-DNDEBUG`
- PTX virtual architecture: derived from the SM target
- PTX is embedded in the Rust binary at compile time via `include!`
- Compilation uses the Driver API (avoids `cudart` segfaults on WSL2)

### 9.3 SM Count Estimation

The Green Contexts module estimates SM counts based on compute capability:

| Compute Capability | GPU | Estimated SMs |
|-------------------|-----|---------------|
| (12, _) | RTX 5090 (Blackwell) | 170 |
| (9, 0) | H100 (Hopper) | 132 |
| (8, 9) | RTX 4090 (Ada Lovelace) | 128 |
| (8, 6) | RTX 4080 (Ada Lovelace) | 84 |
| (8, 0) | A100 (Ampere) | 108 |
| (7, 5) | RTX 2080 Ti (Turing) | 72 |
| (7, 0) | V100 (Volta) | 80 |
| Other | Fallback | 32 |

**Known limitation (CUDA-L1):** SM count estimation uses a hardcoded lookup table covering only flagship SKUs. Non-flagship GPUs fall through to the default of 32 SMs, which may disable Green Contexts unnecessarily. The proper fix is `cuDeviceGetAttribute(MULTIPROCESSOR_COUNT)` but that requires a live CUDA context at initialization time.

### 9.4 WSL2 Compatibility

The crate includes specific WSL2 workarounds:

- CUDA driver library path: `/usr/lib/wsl/lib` is added to the linker search path when detected
- FAISS GPU availability is tested via subprocess to avoid crash on CUDA version mismatch
- PTX loaded via Driver API to avoid cudart static initialization bugs

---

## 10. Integration with candle ML Framework

### 10.1 Current Status

The candle ML framework integration is **planned but not yet active**. In `Cargo.toml`:

```toml
# TODO: Re-enable when candle-core supports CUDA 13.1 / compute capability 12.0
# candle-core = { version = "0.8", optional = true }
# candle-nn = { version = "0.8", optional = true }
```

The blocking issue is that candle-core's cudarc backend does not yet support CUDA 13.1 / compute capability 12.0.

### 10.2 Planned Usage

When candle support is available, the batch similarity module (`src/similarity.rs`) will switch from SIMD-optimized CPU computation to GPU tensor operations:

- `BatchedQueryContext` will hold GPU tensors instead of CPU arrays
- Dimension-grouped matmul will use candle tensor operations on Tensor Cores (FP16 per ARCH-GPU-07)
- Expected speedup: ~260x over scalar CPU (5ms vs 1.3s for 10K candidates x 10 embedders)

### 10.3 Upstream Integration

The `context-graph-mcp` crate has a `cuda` feature that activates candle:

```toml
cuda = ["context-graph-cuda/cuda", "context-graph-embeddings/cuda", "context-graph-storage/cuda", "candle"]
```

---

## 11. Feature Flags

### 11.1 Cargo Features

| Feature | Default | Description |
|---------|---------|-------------|
| `cuda` | Yes | CUDA support (required; build panics without it) |
| `faiss-working` | No | FAISS GPU support (requires custom-built FAISS with CUDA 13.1+ and sm_120) |

The `cuda` feature is enabled by default and is effectively mandatory. The build script panics with an explicit error message if the `cuda` feature is disabled:

```
CUDA feature is required. RTX 5090 GPU must be available. No fallback stubs.
```

### 11.2 FAISS Activation

The `faiss-working` feature requires manual setup:

1. Run `./scripts/rebuild_faiss_gpu.sh` to build FAISS with CUDA 13.1+ support and `sm_120` architecture
2. Build with: `cargo build --features faiss-working`

The build script searches for `libfaiss_c.so` in these paths (priority order):

1. `$HOME/.local/lib`
2. `/usr/local/lib`
3. `/usr/lib`
4. `/usr/lib/x86_64-linux-gnu`

If `faiss-working` is enabled but `libfaiss_c.so` is not found, the build fails immediately with instructions.

### 11.3 Test-Only Modules

The `stub` module (`StubVectorOps`) is gated behind `#[cfg(test)]` and provides a CPU-only `VectorOps` implementation exclusively for testing. Production builds cannot access it.

---

## 12. Error Handling for GPU Operations

### 12.1 CudaError Enum

**File:** `src/error.rs`

| Variant | Description |
|---------|-------------|
| `DeviceInitError(String)` | CUDA device initialization failed |
| `MemoryError(String)` | GPU memory allocation failed |
| `KernelError(String)` | CUDA kernel execution failed |
| `DimensionMismatch { expected, actual }` | Vector dimension mismatch |
| `NoDevice` | No CUDA device available |
| `NotImplemented(String)` | Feature not implemented |
| `InvalidConfig(String)` | Invalid configuration parameter |
| `FaissError { operation, code }` | FAISS operation failed |
| `CudaRuntimeError { operation, code }` | CUDA runtime API error |
| `InvalidArgument { argument, reason }` | Invalid argument |
| `TensorError(String)` | Tensor operation failed (candle-core) |
| `BatchTooLarge { size, max }` | Batch size exceeds GPU memory budget |

All variants derive `Debug` and implement `Display` via `thiserror::Error`.

### 12.2 Error Propagation Strategy

- **No fallbacks**: GPU operations fail fast with detailed error messages (no silent CPU fallback)
- **No panics in Drop**: RAII wrappers log errors during cleanup but never panic
- **Result types**: `CudaResult<T> = Result<T, CudaError>` and `GpuHdbscanResult<T> = Result<T, GpuHdbscanError>`
- **Constitution rule AP-14**: No `.unwrap()` in production code; all errors propagated via `Result`
- **Bounds checking**: All integer casts from `usize` to `i32`/`u32` are validated before truncation with descriptive error messages

### 12.3 HDBSCAN-Specific Errors

The `GpuHdbscanError` enum (Section 5.7) provides specialized error types with descriptive messages that include:

- Constitution rule references (e.g., "ARCH-GPU-05")
- Parameter names and values
- Required constraints
- Specific non-finite value types (NaN, +Infinity, -Infinity)
- Phase identification for internal errors

---

## Green Contexts: GPU SM Partitioning

**Files:** `src/context/green_contexts.rs`, `src/context/mod.rs`

Green Contexts provide in-process Streaming Multiprocessor (SM) partitioning, dividing the GPU into isolated workload zones.

### Configuration

| Constant | Value | Description |
|----------|-------|-------------|
| `INFERENCE_PARTITION_PERCENT` | 0.70 | 70% of SMs for inference workloads |
| `BACKGROUND_PARTITION_PERCENT` | 0.30 | 30% of SMs for background workloads |
| `GREEN_CONTEXTS_MIN_COMPUTE_MAJOR` | 7 | Minimum compute capability major (Volta) |
| `GREEN_CONTEXTS_MIN_COMPUTE_MINOR` | 0 | Minimum compute capability minor |
| `MIN_SMS_FOR_PARTITIONING` | 8 | Minimum SMs for partitioning to be worthwhile |

### GreenContextsConfig

```rust
pub struct GreenContextsConfig {
    pub min_compute_capability: (u32, u32),  // Default: (7, 0)
    pub inference_partition: f32,             // Default: 0.70
    pub background_partition: f32,            // Default: 0.30
}
```

Validation ensures partition percentages sum to at most 1.0, are non-negative, and compute capability major is non-zero.

### GreenContext (Partition)

```rust
pub struct GreenContext {
    partition_id: u32,   // 0 = inference, 1 = background
    percentage: f32,     // SM allocation percentage
    sm_count: u32,       // Actual SM count
    active: bool,        // Whether partition is active
}
```

### GreenContexts (Manager)

```rust
pub struct GreenContexts {
    config: GreenContextsConfig,
    enabled: bool,
    total_sm_count: u32,
    inference: Option<GreenContext>,
    background: Option<GreenContext>,
}
```

Created via `GreenContexts::new(device, config)`. If the device does not meet compute capability requirements or has fewer than 8 SMs, the manager is created in disabled mode with `is_enabled() = false`. This is intentional graceful degradation, not an error.

### Green Contexts vs MIG

| Feature | Green Contexts | MIG |
|---------|---------------|-----|
| Scope | In-process SM partitioning | System-level GPU partitioning |
| GPUs | Volta+ (compute 7.0+) | A100/H100 only (not RTX) |
| Overhead | Low | Requires GPU reconfiguration |
| Usage | What this crate implements | Not applicable to RTX 5090 |

### RTX 5090 Example

On an RTX 5090 with ~170 SMs:

- Inference partition: 119 SMs (70%)
- Background partition: 51 SMs (30%)
- Each partition's kernels are guaranteed to use only their allocated SMs

---

## 13. Pluggable Attention Strategies

Introduced in commit b26fe330. Lives in the **embeddings** crate (`crates/context-graph-embeddings/src/models/attention/`), not the CUDA crate, but runs on GPU tensors via candle and shapes the GPU attention workload for E1/E5/E7/E8/E10/E11/E12.

### 13.1 AttentionStrategy Trait

Defined in `crates/context-graph-embeddings/src/models/attention/mod.rs:44`. All strategies compute the same logical operation `output = softmax(QK^T / sqrt(d) + mask) @ V`, differing in memory/compute profile.

```rust
pub trait AttentionStrategy: Send + Sync {
    fn forward(
        &self, q: &Tensor, k: &Tensor, v: &Tensor, mask: &Tensor, scale: f64,
    ) -> EmbeddingResult<Tensor>;
    fn name(&self) -> &str;
}
```

Tensor shape convention: `q`/`k`/`v` are `[batch, heads, seq_len, head_dim]`; `k`/`v` are already GQA-expanded; `mask` broadcasts to `[batch, heads, seq_len, seq_len]`; `scale = sqrt(head_dim)` is the divisor.

### 13.2 AttentionMode Enum

```rust
pub enum AttentionMode {
    Dense,
    MemoryEfficient { tile_size: usize },       // default tile_size = 256
    SlidingWindow  { window_size: usize },      // default window_size = 256
}
```

Default: `AttentionMode::MemoryEfficient { tile_size: 256 }` (`mod.rs:89-95`).

| Mode | Implementation | Memory | Compute | Use case |
|------|----------------|--------|---------|----------|
| `Dense` | `dense::DenseAttention` | O(n^2) | O(n^2) | Legacy fallback, bit-exact with pre-refactor code |
| `MemoryEfficient` | `tiled::TiledAttention::new(tile_size)` | O(n) per tile | O(n^2) with online softmax | Recommended default |
| `SlidingWindow` | `sliding_window::SlidingWindowAttention::new(window_size)` | O(n*w) | O(n*w) | Long-context local-attention models |

### 13.3 Strategy Resolution

`resolve_attention_mode(attention_mode, use_flash_attention)` (`mod.rs:116`) reconciles the new `attention_mode` config with the legacy `use_flash_attention: bool` flag:

1. Explicit `attention_mode` wins.
2. Legacy `use_flash_attention: true` maps to `MemoryEfficient { tile_size: 256 }`.
3. Legacy `use_flash_attention: false` maps to `Dense`.

### 13.4 File Layout

| File | Purpose |
|------|---------|
| `attention/mod.rs` | Trait, enum, factory `create_strategy()` |
| `attention/dense.rs` | Full O(n^2) materialized QK^T |
| `attention/tiled.rs` | Memory-efficient tiled online softmax |
| `attention/sliding_window.rs` | Local-window masked attention |

---

## 14. VRAM Allocator Refactor (Live-Tensor Tracking)

Described in CLAUDE memory as "VRAM allocator: replaced `mem::forget` with `live_tensors` storage; `free_protected()` drops tensor -> cudaFree". Lives in `crates/context-graph-embeddings/src/warm/cuda_alloc/`.

### 14.1 Problem With the Old Design

The previous `allocate_protected()` leaked `mem::forget(tensor)` to keep the CUDA pointer alive, then manually reconstructed a raw `*mut` via `addr_of!`. There was no live reference for `free_protected()` to drop, so allocations could not be reliably freed and the tensor's CUDA memory leaked until process exit. Cloning `VramAllocation` could also produce two owners for the same raw pointer.

### 14.2 New Design

The allocator (`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator.rs:73`) now owns the live tensors directly:

```rust
pub struct WarmCudaAllocator {
    // ...
    pub(crate) live_tensors: Vec<(u64, Box<dyn std::any::Any + Send>)>,
}
```

Each allocation receives a monotonic `alloc_id: u64`. That ID is stored in `VramAllocation.ptr` (now an opaque handle, not a raw CUDA pointer) while the actual `Tensor` is stored as a `Box<dyn Any + Send>` inside `live_tensors`.

Key entry points (`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs`):

| Method | Line | Behaviour |
|--------|------|-----------|
| `allocate_protected(size_bytes)` | 231 | Allocates a CUDA tensor, records it in `live_tensors`, returns a `VramAllocation` keyed by `alloc_id`. |
| `free_protected(&VramAllocation)` | 299 | Looks up `alloc_id` in `live_tensors`, `swap_remove`s the boxed tensor, and the `Drop` impl of the tensor issues `cudaFree`. |

`VramAllocation` is intentionally no longer `Clone` (`allocation.rs:20-28`): the opaque handle is shared via `&VramAllocation` references to prevent double-free.

### 14.3 Impact

- No more `mem::forget` in the protected path.
- `free_protected()` is now the authoritative release point; if the allocation ID is missing (e.g. a test-fabricated `VramAllocation`), it is a no-op rather than UB.
- `VramAllocation` is `Send + Sync` because it only holds primitive data; the actual CUDA pointer lives in the allocator's `live_tensors` vector.
