# CUDA Acceleration

Source files covered:

- `crates/context-graph-cuda/Cargo.toml`
- `crates/context-graph-cuda/build.rs`
- `crates/context-graph-cuda/src/lib.rs`
- `crates/context-graph-cuda/src/error.rs`
- `crates/context-graph-cuda/src/ops.rs`
- `crates/context-graph-cuda/src/similarity.rs`
- `crates/context-graph-cuda/src/stub.rs`
- `crates/context-graph-cuda/src/context/{mod,green_contexts}.rs`
- `crates/context-graph-cuda/src/ffi/{mod,cuda_driver,knn,faiss}.rs`
- `crates/context-graph-cuda/src/hdbscan/{mod,clusterer,gpu_knn,error}.rs`
- `crates/context-graph-cuda/src/safe/{mod,device}.rs`
- `crates/context-graph-cuda/kernels/knn.cu`
- `.cargo/config.toml`, `rust-toolchain.toml`

Related: [05_embedding_subsystem.md](05_embedding_subsystem.md), [12_clustering_and_topics.md](12_clustering_and_topics.md).

---

## 1. Target Hardware & Toolchain

| Item | Value |
|---|---|
| GPU | NVIDIA RTX 5090 (Blackwell, 32 GB GDDR7) |
| Compute capability | 12.0 (`sm_120` / `sm_120a`) |
| CUDA toolkit (pinned) | 13.1 (`/usr/local/cuda-13.1`) |
| Driver lib | `libcuda.so` via WSL2 (`/usr/lib/wsl/lib`) or `$CUDA_HOME/lib64` |
| Kernel compile target | `sm_120a` (default, override via `CUDA_ARCH`) |
| Virtual arch for PTX | `compute_120a` (derived from `CUDA_ARCH`) |
| Rust toolchain | stable, `x86_64-unknown-linux-gnu` |
| Linker | `clang` with `-fuse-ld=lld` |
| `cudarc` (transitive via candle) | 0.17.8 and 0.18.2 in lockfile |
| `candle-core` (transitive) | 0.9.2-alpha.2 |
| `cudarc` ABI override | `CUDARC_CUDA_VERSION=13000` (forces 13.0 ABI across the dep graph) |

The crate itself declares **no** `cudarc` or `candle` dependency. It links `libcuda.so` directly and compiles PTX via `nvcc`. `cudarc` only appears via other crates (e.g. `candle-core`); the `CUDARC_CUDA_VERSION=13000` env in `.cargo/config.toml` pins those consumers to the 13.0 ABI (forward-compatible with the 13.1 runtime).

---

## 2. Crate Layout

| Module | Purpose |
|---|---|
| `ffi/cuda_driver` | Raw `extern "C"` bindings to CUDA Driver API (`cuInit`, `cuDeviceGet*`, `cuCtx*`, `cuMemGetInfo_v2`). |
| `ffi/knn` | Driver API bindings for module/kernel/memory (`cuModuleLoadData`, `cuMemAlloc_v2`, `cuLaunchKernel`, …) plus safe wrappers `GpuBuffer`, `CudaContext`, `CudaModule`, and host entry points `compute_core_distances_gpu` / `compute_pairwise_distances_gpu`. |
| `ffi/faiss` | FAISS C API bindings (`faiss_index_factory`, `faiss_Index_{train,add_with_ids,search}`, `faiss_StandardGpuResources_*`, `faiss_get_num_gpus`) and `GpuResources` RAII. Compiled only with `faiss-working`. |
| `safe/device` | `GpuDevice` RAII owning a CUDA primary context; queries name, compute capability, memory info. |
| `context/green_contexts` | SM partitioning manager: 70 % inference / 30 % background; capability-gated on Volta+ (SM ≥ 7.0). |
| `ops` | `VectorOps` async trait (contract only): cosine, dot, normalize, batch cosine, matmul, softmax. |
| `similarity` | CPU SIMD-style batched cosine grouped by dimension (1024 / 768 / 512 / 1536) for the 10 dense embedders. |
| `hdbscan` | GPU-accelerated HDBSCAN: GPU k-NN core distances + CPU mutual-reachability / MST / cluster extraction. |
| `stub` | `#[cfg(test)]`-only CPU `VectorOps` impl; gated out of production builds. |
| `error` | `CudaError` + `CudaResult<T>`. |

`lib.rs` re-exports the public surface and excludes `stub` from non-test builds per AP-007.

---

## 3. Safe Wrapper API

### `GpuDevice` (`safe/device.rs`)

| Item | Signature | Purpose |
|---|---|---|
| `GpuDevice::new` | `fn(ordinal: i32) -> CudaResult<Self>` | One-time `cuInit(0)` via `Once`, `cuDeviceGet`, `cuCtxCreate_v2`. Owns the context. |
| `compute_capability` | `fn(&self) -> (u32, u32)` | `cuDeviceGetAttribute` for `COMPUTE_CAPABILITY_{MAJOR,MINOR}`. RTX 5090 → `(12, 0)`. |
| `name` | `fn(&self) -> String` | `cuDeviceGetName` into 256-byte buffer. |
| `memory_info` | `fn(&self) -> CudaResult<(usize, usize)>` | Sets context current, calls `cuMemGetInfo_v2` → `(free, total)`. |
| `memory_usage_percent` | `fn(&self) -> f32` | `1.0 - free/total`; returns 0.0 on error. |
| `ordinal` | `fn(&self) -> i32` | Ordinal passed to `new`. |
| `Drop` | `cuCtxDestroy_v2`; logs on failure, never panics. | C2 RAII. |
| Thread safety | `Send` (moveable); not `Sync` (contexts are thread-bound). | |
| Module helper | `pub fn gpu_memory_usage_percent() -> CudaResult<f32>` (in `safe/mod.rs`) | Creates a throwaway `GpuDevice::new(0)` and reads `memory_usage_percent`. |

### `GpuBuffer` (`ffi/knn.rs`)

| Item | Signature | Purpose |
|---|---|---|
| `GpuBuffer::new` | `fn(size: usize) -> CudaResult<Self>` | `cuMemAlloc_v2`. |
| `copy_from_host` | `fn(&self, src: &[u8]) -> CudaResult<()>` | `cuMemcpyHtoD_v2`, bounds-checked against `size`. |
| `copy_to_host` | `fn(&self, dst: &mut [u8]) -> CudaResult<()>` | `cuMemcpyDtoH_v2`, bounds-checked. |
| `ptr` / `size` | accessors | `CUdeviceptr` and byte size. |
| `Drop` | `cuMemFree_v2`. | |
| Thread safety | `unsafe impl Send` (owns the allocation exclusively). | |

### `CudaContext` / `CudaModule` (`ffi/knn.rs`, private)

- `CudaContext::new(device: i32)` → `cuDeviceGet` + `cuCtxCreate_v2`; drop calls `cuCtxDestroy_v2`.
- `CudaModule::load_ptx(ptx: &str)` → `cuModuleLoadData`; `get_function(name)` → `cuModuleGetFunction`; drop calls `cuModuleUnload`.

### Top-level CUDA helpers (`ffi/knn.rs`)

| Function | Signature |
|---|---|
| `cuda_available` | `fn() -> bool` — `cuInit` + `cuDeviceGetCount` > 0 |
| `cuda_device_count` | `fn() -> CudaResult<i32>` |
| `compute_core_distances_gpu` | `fn(&[f32], n_points, dimension, k) -> CudaResult<Vec<f32>>` |
| `compute_pairwise_distances_gpu` | `fn(&[f32], n_points, dimension) -> CudaResult<Vec<f32>>` |

All three do: lazy `cuInit` (via `Once`), create `CudaContext`, load embedded PTX, launch kernel with `blockDim=256`, `cuCtxSynchronize`, copy results back. All `usize → i32` casts are bounds-checked against `i32::MAX`; block-count casts are bounds-checked against `u32::MAX`.

### `CudaError` variants (`error.rs`)

| Variant | Fields |
|---|---|
| `DeviceInitError` | `String` |
| `MemoryError` | `String` |
| `KernelError` | `String` |
| `DimensionMismatch` | `{ expected, actual }` |
| `NoDevice` | — |
| `NotImplemented` | `String` |
| `InvalidConfig` | `String` |
| `FaissError` | `{ operation, code }` |
| `CudaRuntimeError` | `{ operation, code }` |
| `InvalidArgument` | `{ argument, reason }` |
| `TensorError` | `String` |
| `BatchTooLarge` | `{ size, max }` |

`pub type CudaResult<T> = Result<T, CudaError>`.

---

## 4. FFI Layer

### CUDA Driver API (`ffi/cuda_driver.rs`, `ffi/knn.rs`)

Driver API is used throughout; Runtime API is deliberately avoided (the Runtime API path has exhibited segfaults on WSL2 for this hardware/toolkit combination). `#[link(name = "cuda")]` pulls `libcuda.so`.

| Symbol group | Functions declared |
|---|---|
| Init / device | `cuInit`, `cuDeviceGetCount`, `cuDeviceGet`, `cuDeviceGetAttribute`, `cuDeviceGetName`, `cuDeviceTotalMem_v2`, `cuDriverGetVersion` |
| Context | `cuCtxCreate_v2`, `cuCtxDestroy_v2`, `cuCtxSetCurrent`, `cuCtxGetCurrent`, `cuCtxSynchronize`, `cuMemGetInfo_v2` |
| Module | `cuModuleLoadData`, `cuModuleGetFunction`, `cuModuleUnload` |
| Memory | `cuMemAlloc_v2`, `cuMemFree_v2`, `cuMemcpyHtoD_v2`, `cuMemcpyDtoH_v2` |
| Launch | `cuLaunchKernel` |
| Types / constants | `CUresult`, `CUdevice`, `CUcontext`, `CUmodule`, `CUfunction`, `CUdeviceptr`, `CUdevice_attribute`, `CUDA_SUCCESS`, `CUDA_ERROR_NOT_INITIALIZED` (3), `CUDA_ERROR_NO_DEVICE` (100), `CUDA_ERROR_INVALID_DEVICE` (101), `CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_{MAJOR,MINOR}`, `CU_DEVICE_ATTRIBUTE_MAX_{THREADS_PER_BLOCK,BLOCK_DIM_X}`, `CU_DEVICE_ATTRIBUTE_WARP_SIZE` |
| Helpers | `decode_driver_version(encoded) -> (major, minor)`, `is_cuda_success`, `cuda_result_to_string` |

Driver encodes version as `major * 1000 + minor * 10` (e.g. 13010 = 13.1).

cuBLAS / cuBLASLt / cuDNN / cuSolver / cuSparse are **not** linked by this crate. The only NVIDIA libraries linked are `libcuda.so` and (optionally) `libfaiss_c.so` + `libfaiss.so`.

### FAISS C API (`ffi/faiss.rs`, feature-gated)

`#[link(name = "faiss_c")]`.

| Group | Functions |
|---|---|
| Index factory | `faiss_index_factory`, `faiss_Index_free` |
| GPU resources | `faiss_StandardGpuResources_new`, `faiss_StandardGpuResources_free` |
| CPU↔GPU transfer | `faiss_index_cpu_to_gpu` |
| Training / search | `faiss_Index_train`, `faiss_Index_is_trained`, `faiss_Index_add_with_ids`, `faiss_Index_search`, `faiss_IndexIVF_set_nprobe`, `faiss_Index_ntotal` |
| Persistence | `faiss_write_index`, `faiss_read_index` |
| GPU detection | `faiss_get_num_gpus` |

Types: `MetricType::{InnerProduct=0, L2=1}`; opaque handles `FaissIndex`, `FaissGpuResourcesProvider`, `FaissStandardGpuResources`; RAII wrapper `GpuResources` (`Send + Sync`).

GPU detection uses a subprocess probe (`nvidia-smi` count + optional compiled C test binary at `/tmp/faiss_gpu_test_bin`) to avoid crashing the parent on driver/ABI mismatch. Results cached in `OnceLock<bool>`. `SKIP_GPU_TESTS=1` forces `false`.

### Custom k-NN kernel (`kernels/knn.cu`)

| Kernel | Signature | Block | Notes |
|---|---|---|---|
| `compute_core_distances_kernel` | `(const float* vectors, int n_points, int dimension, int k, float* core_dists)` | 256 threads/block | One thread per point; streams all points; keeps top-(k+1) in a 16-register array; emits `sqrtf(kth_dist)`. |
| `compute_pairwise_distances_kernel` | `(const float* vectors, int n_points, int dimension, float* distances)` | 256 threads/block | One thread per upper-triangular pair (i,j); 64-bit linear-to-pair index via `sqrtf`; writes `n*(n-1)/2` distances. |

Both kernels use `__restrict__`, `#pragma unroll` on the top-k init, and are built with `--use_fast_math`. They are compiled to PTX and loaded at runtime via `cuModuleLoadData`.

---

## 5. Context & Memory Management

### Process-wide initialization

Both `safe::device::GpuDevice::new` and `ffi::knn::ensure_cuda_initialized` guard `cuInit(0)` behind `std::sync::Once` with an `AtomicI32` to cache the result (no `static mut`). Safe to call concurrently from any thread.

### Per-`GpuDevice` ownership

`GpuDevice` owns one `CUcontext` created via `cuCtxCreate_v2(&mut ctx, 0, dev)`. `Drop` calls `cuCtxDestroy_v2`. The struct is `Send` (can move threads) but **not** `Sync` (CUDA contexts are thread-bound).

### Per-call ephemeral contexts

`compute_core_distances_gpu` and `compute_pairwise_distances_gpu` create a **private** `CudaContext` for the duration of the call via the lightweight wrapper in `ffi/knn.rs`. This gives each kernel invocation an isolated context. For long-lived usage, callers should hold a `GpuDevice` instead and re-enter via `cuCtxSetCurrent`.

### GPU buffer lifetimes

`GpuBuffer` owns one `CUdeviceptr` via `cuMemAlloc_v2`; `Drop` → `cuMemFree_v2`. `unsafe impl Send`. Bounds-checked HtoD / DtoH copies. The crate does **not** implement a device-memory pool — pooling lives in the embeddings crate's allocator (`context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs`) which uses the FFI symbols re-exported from here.

### FAISS GPU resources

`GpuResources` wraps `FaissStandardGpuResources *` with `NonNull`; `Drop` → `faiss_StandardGpuResources_free`. `unsafe impl Send + Sync` (FAISS uses internal synchronization). Exposed as `as_ptr()` or `as_provider()` (`*mut FaissGpuResourcesProvider`) for `faiss_index_cpu_to_gpu`.

### Green Contexts (SM partitioning)

`GreenContexts::new(&GpuDevice, GreenContextsConfig)` auto-detects capability and allocates:

| Partition | Percentage | Constant |
|---|---|---|
| Inference (`id=0`) | 0.70 | `INFERENCE_PARTITION_PERCENT` |
| Background (`id=1`) | 0.30 | `BACKGROUND_PARTITION_PERCENT` |

SM count estimation is a hardcoded table keyed by `(major, minor)`:

| Compute | SMs | GPU family |
|---|---|---|
| (12, x) | 170 | Blackwell / RTX 5090 |
| (9, 0) | 132 | Hopper / H100 |
| (8, 9) | 128 | Ada / RTX 4090 |
| (8, 6) | 84 | Ada / RTX 4080 |
| (8, 0) | 108 | Ampere / A100 |
| (7, 5) | 72 | Turing / RTX 2080 Ti |
| (7, 0) | 80 | Volta / V100 |
| otherwise | 32 | fallback |

Gates: minimum compute capability `(7, 0)` (configurable); `MIN_SMS_FOR_PARTITIONING = 8`. On unsupported hardware `is_enabled() == false` and `{inference,background}_context()` return `None` (graceful degradation — not an error).

`GreenContextsConfig::validate()` rejects: partition sum > 1.0, negative partitions, zero-major compute.

---

## 6. GPU Kernels & Operations

### `VectorOps` trait (`ops.rs`)

Async contract; no production implementor in this crate (the stub in `stub.rs` is test-only). Returns raw cosine `[-1, 1]`; callers needing `[0, 1]` must apply SRC-3 normalization `(raw + 1.0) / 2.0`.

| Method | Signature |
|---|---|
| `cosine_similarity` | `async fn(&self, a: &[f32], b: &[f32]) -> CudaResult<f32>` |
| `dot_product` | `async fn(&self, a: &[f32], b: &[f32]) -> CudaResult<f32>` |
| `normalize` | `async fn(&self, v: &[f32]) -> CudaResult<Vec<f32>>` |
| `batch_cosine_similarity` | `async fn(&self, query: &[f32], vectors: &[Vec<f32>]) -> CudaResult<Vec<f32>>` |
| `matmul` | `async fn(&self, a: &[f32], b: &[f32], m, n, k) -> CudaResult<Vec<f32>>` |
| `softmax` | `async fn(&self, v: &[f32]) -> CudaResult<Vec<f32>>` |
| `is_gpu_available` | `fn(&self) -> bool` |
| `device_name` | `fn(&self) -> &str` |

### Batched similarity (`similarity.rs`)

Groups the 10 dense embedders by dimension (sparse E6/E13 and MaxSim E12 are excluded):

| `DimensionGroup` | Dim | Embedder indices | Embedders |
|---|---|---|---|
| `Dim1024` | 1024 | (0, 7, 8) | E1, E8, E9 |
| `Dim768` | 768 | (4, 9, 10) | E5, E10, E11 |
| `Dim512` | 512 | (1, 2, 3) | E2, E3, E4 |
| `Dim1536` | 1536 | (6,) | E7 |

`DENSE_EMBEDDER_INDICES = [0, 1, 2, 3, 4, 6, 7, 8, 9, 10]`.

| Item | Signature |
|---|---|
| `embedder_to_group` | `const fn(usize) -> Option<DimensionGroup>` |
| `BatchedQueryContext::new` | `fn(&[&[f32]; 10]) -> CudaResult<Self>` — L2-normalizes once per query into a `[[f32; 1536]; 10]` |
| `BatchedQueryContext::get_normalized` | `fn(&self, idx: usize) -> &[f32]` |
| `compute_batch_cosine_similarity` | `fn(&BatchedQueryContext, &[[&[f32]; 10]]) -> CudaResult<Vec<[f32; 10]>>` — 4-wide manual unroll over pre-normalized query + candidate, single-pass dot + `cand_norm_sq`, clamped `[-1, 1]` |
| `compute_batch_cosine_similarity_chunked` | same signature; splits into 2048-candidate cache-friendly batches |
| `GPU_BATCH_THRESHOLD` | `const usize = 100` |
| `should_use_gpu_batch(count) -> bool` | `count >= GPU_BATCH_THRESHOLD` |

The body is CPU-resident (cache-friendly, 4-wide manual unroll). The `DimensionGroup` scaffolding, `BatchedQueryContext` layout, and `should_use_gpu_batch` threshold are prepared for a drop-in GPU matmul backend when the dependency graph can provide one; the public surface won't change.

### GPU HDBSCAN (`hdbscan/`)

`GpuHdbscanClusterer::fit(embeddings: &[Vec<f32>], memory_ids: &[Uuid]) -> GpuHdbscanResult<Vec<ClusterMembership>>`.

Pipeline:

| Step | Location | Algorithm |
|---|---|---|
| 1. Core distances | GPU | `GpuKnnIndex::compute_core_distances_with_vectors` → `compute_core_distances_gpu` (brute-force k-NN PTX kernel) |
| 2. Mutual reachability | CPU | `MR(a,b) = max(core_a, core_b, L2(a,b))`, full `n×n` matrix |
| 3. MST | CPU | Prim's algorithm; edges sorted by weight |
| 4. Cluster extraction | CPU | Union-Find with gap detection; gap = 2× edge-weight jump, else semantic threshold 0.5 |
| 5. Core-point ID | CPU | Points at or below cluster-median core distance are core; others are border |

`HdbscanParams` defaults: `min_cluster_size=3`, `min_samples=2`, `cluster_selection_method=EOM`. Validation requires `min_cluster_size ≥ 2`, `min_samples ≥ 1`, `min_samples ≤ min_cluster_size`. Input validation rejects dimension mismatch, zero dimension, and non-finite values.

`ClusterMembership { memory_id, cluster_id (-1 = noise), membership_probability ∈ [0,1], is_core }`.
`ClusterSelectionMethod::{EOM (default), Leaf}`.
`GpuKnnIndex::new(dimension)` fails fast with `GpuNotAvailable` when `cuda_available()` is false — no CPU fallback.

Also exposed: `GpuHdbscanClusterer::compute_silhouette(&[Vec<f32>], &[i32]) -> f32`.

### `GpuHdbscanError` variants (`hdbscan/error.rs`)

`GpuNotAvailable`, `FaissResourceError`, `FaissIndexError`, `FaissSearchError`, `InvalidParameter`, `InsufficientData`, `DimensionMismatch`, `InvalidDimension`, `NonFiniteValue`, `InternalError`.

---

## 7. Integration Points

| Consumer | Imports | Use |
|---|---|---|
| `context-graph-embeddings` / `gpu/device/utils.rs` | `context_graph_cuda::ffi::{cuInit, cuDeviceGetCount, cuDeviceGetAttribute, CU_DEVICE_ATTRIBUTE_*, …}` | Device enumeration and capability probing for the embeddings subsystem's `GpuDevice` singleton. |
| `context-graph-embeddings` / `warm/cuda_alloc/allocator_cuda.rs` | `context_graph_cuda::ffi::{cuInit, cuCtxCreate_v2, cuMemGetInfo_v2, …}` | VRAM allocator for embedding weights (see [05_embedding_subsystem.md](05_embedding_subsystem.md) §VRAM management). |
| `context-graph-storage` | `context-graph-cuda` as a workspace dep | Transitive wiring only — HNSW rebuild uses the embeddings-layer API, which in turn calls into this crate. |
| `context-graph-mcp` | workspace dep | Transitive. |
| Clustering / topics | `GpuHdbscanClusterer`, `HdbscanParams`, `ClusterMembership` | Consumed by the topic-detection path; see [12_clustering_and_topics.md](12_clustering_and_topics.md). |
| Batched similarity | `BatchedQueryContext`, `compute_batch_cosine_similarity{,_chunked}`, `embedder_to_group`, `DENSE_EMBEDDER_INDICES`, `should_use_gpu_batch` | Multi-space fusion in search/ranking; see [05_embedding_subsystem.md](05_embedding_subsystem.md) and [06_search_and_fusion.md](06_search_and_fusion.md). |

ARCH-06: all CUDA FFI declarations live in this crate only — no other crate may declare `extern "C"` against `libcuda.so` or `libfaiss_c.so`.

---

## 8. Feature Flags

| Feature | Default | Enables | Link deps | Effect |
|---|---|---|---|---|
| `cuda` | yes | `build.rs` compiles `kernels/knn.cu` → PTX; `cargo:rustc-link-lib=cuda`; search paths `/usr/lib/wsl/lib` and `$CUDA_HOME/lib64` (or common fallbacks). | `libcuda.so` | Required — the crate panics at build time (`build.rs`) if `cuda` is absent. |
| `faiss-working` | no | Compiles `src/ffi/faiss.rs`; re-exports `check_faiss_result`, `faiss_gpu_available`, `FaissGpuResourcesProvider`, `FaissGpuResources`, `FaissIndex`, `FaissStandardGpuResources`, `MetricType`, `FAISS_OK`. | `libfaiss_c.so`, `libfaiss.so` (searched in `$HOME/.local/lib`, `/usr/local/lib`, `/usr/lib`, `/usr/lib/x86_64-linux-gnu`) | Requires `cuda`. `build.rs` fails fast with a multi-line error listing searched paths if `libfaiss_c.so` is not found. |

There is **no** non-`cuda` build path: `build.rs` panics with `"CUDA feature is required. RTX 5090 GPU must be available. No fallback stubs."` if the feature is disabled.

`is_faiss_gpu_available()` and `faiss_status()` are available unconditionally; they return `false` / `"FAISS GPU: DISABLED (faiss-working feature not enabled)"` when `faiss-working` is off, and are useful for runtime capability reporting.

---

## 9. Stub Mode (Test-Only)

`src/stub.rs` is gated by `#[cfg(test)]` in `lib.rs`:

```rust
#[cfg(test)]
pub mod stub;
#[cfg(test)]
#[allow(deprecated)]
pub use stub::StubVectorOps;
```

`StubVectorOps` is a CPU `VectorOps` impl used only in unit tests and hardware-less CI. Per AP-007 it is **not** available in production builds. All methods run scalar Rust; `is_gpu_available()` returns `false`; `device_name()` returns `"CPU (Stub)"`.

| `VectorOps` method | Fallback implementation |
|---|---|
| `cosine_similarity` | dot / (‖a‖·‖b‖); returns 0.0 if either norm is 0 |
| `dot_product` | Σ aᵢ·bᵢ |
| `normalize` | v / ‖v‖; zero vector returns zeros |
| `batch_cosine_similarity` | Sequential loop over `cosine_similarity` |
| `matmul` | Triple-nested loop |
| `softmax` | Numerically stable (subtract max, exp, normalize) |

---

## 10. Build Environment

### `.cargo/config.toml` env

| Variable | Value | Purpose |
|---|---|---|
| `CUDA_HOME` | `/usr/local/cuda-13.1` | Toolkit root (read by `build.rs`, FAISS build) |
| `CMAKE_CUDA_COMPILER` | `/usr/local/cuda-13.1/bin/nvcc` | CMake-driven FAISS rebuilds |
| `CUDAToolkit_ROOT` | `/usr/local/cuda-13.1` | CMake toolkit discovery |
| `CUDACXX` | `/usr/local/cuda-13.1/bin/nvcc` | CMake fallback |
| `CMAKE_CUDA_ARCHITECTURES` | `120a-real` | Real SM (not virtual) target for FAISS / other CMake consumers |
| `CUDARC_CUDA_VERSION` | `13000` | Forces transitive `cudarc` (0.17.8 / 0.18.2) to use the CUDA 13.0 ABI (forward-compatible at runtime with the 13.1 driver) |
| `NVCC_FLAGS` | `-Xptxas -O3 --use_fast_math --restrict` | Extra flags consumed by this crate's `build.rs` |
| `RUST_TEST_THREADS` | `4` | |

### `build.rs` behavior (cuda feature on)

- Resolves `nvcc` via `which nvcc` → `$CUDA_PATH/bin/nvcc` → common paths (`/usr/local/cuda{,-13.1,-13.0,-12.0}/bin/nvcc`, `/opt/cuda/bin/nvcc`). Panics with a download URL if not found.
- Reads `CUDA_ARCH` (default `sm_120a`); derives virtual arch `compute_120a`.
- Compiles `kernels/knn.cu` → `$OUT_DIR/knn.ptx` with:
  `nvcc --ptx -O3 -arch compute_120a -Xptxas -O3 --use_fast_math --restrict -DNDEBUG [NVCC_FLAGS] -o …`
- Embeds the PTX as a `pub const PTX: &str` in `$OUT_DIR/knn_ptx.rs` (escaped, single literal). Loaded by `ffi::knn` via `include!`.
- `cargo:rustc-link-lib=cuda`; adds `/usr/lib/wsl/lib` if present and `$CUDA_PATH/lib64` / common CUDA lib dirs.
- `cargo:rerun-if-changed` covers `build.rs`, `kernels/`; `cargo:rerun-if-env-changed` covers `CUDA_PATH`, `CUDA_ARCH`, `NVCC_FLAGS`.

### `rust-toolchain.toml`

Stable channel, x86_64-unknown-linux-gnu, components `rustfmt`, `clippy`, `rust-src`. No nightly or pinned version required.

### Workspace linker

`.cargo/config.toml` sets `linker = "clang"` with `-fuse-ld=lld`, and enables `-C target-cpu=native`. `release` profile disables incremental compilation.
