# CUDA Acceleration

**Source files covered:**
- `crates/context-graph-cuda/Cargo.toml` — `default = ["cuda"]`, optional `faiss-working` feature
- `crates/context-graph-cuda/build.rs` — nvcc 13.2 validation + cubin compilation + driver linking
- `crates/context-graph-cuda/kernels/knn.cu` — k-NN kernels compiled to native sm_120a cubin
- `crates/context-graph-cuda/src/lib.rs` — public surface (`#![deny(deprecated)]`); test-only `stub` module
- `crates/context-graph-cuda/src/error.rs` — `CudaError` (12 variants) + `CudaResult<T>`
- `crates/context-graph-cuda/src/ops.rs` — `VectorOps` async trait
- `crates/context-graph-cuda/src/stub.rs` — `StubVectorOps` (`#[cfg(test)]` only)
- `crates/context-graph-cuda/src/similarity.rs` — `BatchedQueryContext`, `DimensionGroup`, batched cosine
- `crates/context-graph-cuda/src/ffi/{mod,cuda_driver,knn,faiss}.rs` — single source of truth for CUDA / FAISS extern "C"
- `crates/context-graph-cuda/src/safe/{mod,device}.rs` — RAII `GpuDevice`
- `crates/context-graph-cuda/src/context/{mod,green_contexts}.rs` — Volta+ SM partitioning
- `crates/context-graph-cuda/src/hdbscan/{mod,clusterer,gpu_knn,error,tests}.rs` — GPU HDBSCAN
- `crates/context-graph-cuda/tests/native_cubin_smoke.rs` — end-to-end cubin smoke test
- `crates/context-graph-embeddings/src/gpu/{device,memory,model_loader,tensor,ops,mod}.rs` — Candle-driven GPU helpers
- `crates/context-graph-embeddings/src/warm/cuda_alloc/{mod,allocation,allocator,allocator_cuda,gpu_info,constants,helpers,tests}.rs` — non-evictable model-weight allocator
- workspace `Cargo.toml` — candle-core / candle-nn `0.10.2` with `cuda` feature (cudarc 0.19.x for CUDA 13.2 / Blackwell)

Cross-references: see [05](05_embedding_subsystem.md) §3, §9 for embedder dimensions consumed by the GPU similarity batch path; [06](06_search_and_fusion.md) for fusion strategies that schedule batched cosines; [09](09_storage_layer.md) for quantized vectors loaded from VRAM; [12](12_clustering_and_topics.md) for the HDBSCAN consumer; [16](16_training_subsystem.md) for LoRA training paths sharing the same device singleton; [17](17_constellation_subsystem.md) for the Welford pipeline that consumes GPU-resident fingerprints.

---

## 1. Scope

`context-graph-cuda` is the **single source of truth** for CUDA FFI bindings (ARCH-06: every `extern "C"` CUDA declaration must live in this crate). Two complementary GPU paths exist in the workspace:

- **Direct CUDA Driver API** (this crate): owns the kernels we wrote ourselves (k-NN core / pairwise distances), loads them from native `sm_120a` cubin, and exposes RAII context / device wrappers.
- **Candle-managed CUDA** (`candle-core` / `candle-nn` 0.10.2 with `cuda` via cudarc 0.19.x): used by the warm model loader, embedding inference, and LoRA training. Workspace `Cargo.toml` pin: *"Using 0.10.2 for cudarc 0.19.x CUDA 13.2 bindings (RTX 5090 Blackwell)"*.

This crate does NOT duplicate Candle's kernels — it provides the primitives Candle does not (custom HDBSCAN k-NN, Green Contexts, raw cubin loading), plus the FFI shim that the rest of the workspace links against.

### 1.1 Target hardware

| Property | Value | Source |
|---|---|---|
| GPU | NVIDIA RTX 5090 (Blackwell) | `lib.rs:18-22` |
| VRAM | 32 GB GDDR7 (1.8 TB/s) | `lib.rs:18-22` |
| Compute capability (required) | `12.0` | `warm/cuda_alloc/constants.rs:9-12` (`REQUIRED_COMPUTE_MAJOR=12`, `REQUIRED_COMPUTE_MINOR=0`) |
| CUDA toolkit | 13.2 (validated at build time) | `lib.rs:21`, `build.rs::validate_nvcc_cuda_13_2` (output must contain `release 13.2`) |
| `CUDA_ARCH` (default; only allowed value) | `sm_120a` | `build.rs:62-69` (the `a` suffix unlocks Blackwell-family-specific SASS) |
| Workspace Candle | 0.10.2 + `cuda` feature | root `Cargo.toml` |
| Workspace cudarc | 0.19.x (CUDA 13.2 bindings) | provided through Candle's `cuda` feature |
| Minimum VRAM gate | 31 GiB (`MINIMUM_VRAM_BYTES`) | `warm/cuda_alloc/constants.rs:26` |

### 1.2 AP-007: no production stub

Per `lib.rs:10-16` and `lib.rs:46-48`: there is **no production stub**. `StubVectorOps` is gated `#[cfg(test)]`, and `lib.rs` sets `#![deny(deprecated)]` so any non-test code that imports it fails to compile. `is_gpu_available()` returns `false` for the stub and `device_name()` returns `"CPU (Stub)"`.

Crates that turn on CUDA via `default-features = ["cuda"]`: `context-graph-cuda`, `context-graph-embeddings` (`cuda` is an alias for `candle`), `context-graph-mcp`, `context-graph-cli`, `context-graph-storage`. The `context-graph-causal-agent` crate gates `cuda` behind its `llm` feature so the ~500 MB llama-cpp-2 dependency is opt-in (see §15).

---

## 2. Module tree

| Path | Purpose |
|---|---|
| `src/error.rs` | `CudaError` (12 variants) + `CudaResult<T>` |
| `src/ops.rs` | `VectorOps` async trait (cosine, dot, normalize, batch cosine, matmul, softmax, `is_gpu_available`, `device_name`) |
| `src/stub.rs` | `StubVectorOps` — TEST-ONLY (`#[cfg(test)]`); production code paths cannot link it |
| `src/ffi/mod.rs` | Re-exports + `is_faiss_gpu_available()`, `faiss_status()` |
| `src/ffi/cuda_driver.rs` | Driver API extern "C": `cuInit`, `cuDeviceGet`, `cuCtxCreate_v2`, `cuCtxDestroy_v2`, `cuCtxSetCurrent`, `cuCtxGetCurrent`, `cuDeviceGetAttribute`, `cuDeviceGetName`, `cuDeviceTotalMem_v2`, `cuDeviceGetCount`, `cuDriverGetVersion`, `cuMemGetInfo_v2` plus CUresult / device-attribute IDs |
| `src/ffi/knn.rs` | Kernel host wrappers + `GpuBuffer`/`CudaContext`/`CudaModule` RAII; loads embedded cubin via `cuModuleLoadData` (image is `IMAGE` from `OUT_DIR/knn_image.rs`) |
| `src/ffi/faiss.rs` | FAISS C-ABI bindings (only when `faiss-working` feature on); subprocess `gpu_available()` to dodge WSL2 cudart static-init crash |
| `src/context/{mod,green_contexts}.rs` | Green Contexts — Volta+ (CC ≥ 7.0); 70/30 inference/background SM split |
| `src/safe/device.rs` | `GpuDevice` RAII: owns context, calls `cuCtxDestroy_v2` on drop, `Send` but NOT `Sync`. Free function `gpu_memory_usage_percent()` for one-shot polling |
| `src/similarity.rs` | Batched cosine grouped by `DimensionGroup`; `DENSE_EMBEDDER_INDICES`; `BatchedQueryContext` |
| `src/hdbscan/clusterer.rs` | `GpuHdbscanClusterer`, `HdbscanParams`, `ClusterMembership`, `ClusterSelectionMethod` |
| `src/hdbscan/gpu_knn.rs` | `GpuKnnIndex`: vector store + `compute_core_distances` via the custom k-NN kernel |
| `src/hdbscan/error.rs` | `GpuHdbscanError`, `GpuHdbscanResult` |

CUDA-backed embedding helpers live one crate over in `context-graph-embeddings/src/gpu/`:

| Path | Purpose |
|---|---|
| `gpu/mod.rs` | `GpuInfo`, `gpu_info()` |
| `gpu/device/` | Singleton CUDA `Device` (Candle); `init_gpu`, `device`, `default_dtype`, `is_gpu_available`, `require_gpu`, `get_gpu_info` |
| `gpu/memory/budget.rs` | `MemoryBudget::rtx_5090()` defaults |
| `gpu/memory/slots.rs` | `ModelSlot`, `ModelSlotManager`, `MODEL_BUDGET_BYTES = 8 GiB` |
| `gpu/memory/{pool,pressure,stats,tracker,error}.rs` | Pool / pressure tiers / `MemoryStats` / `VramTracker` / `MemoryError` |
| `gpu/model_loader/` | Candle safetensors model loader (`GpuModelLoader`, `UnifiedModelLoader`, BERT weights structs) |
| `gpu/tensor/` | `GpuTensor` typed wrapper around `candle::Tensor` |
| `gpu/ops.rs` | `normalize_gpu` (L2 normalization on device) |

`warm/cuda_alloc/` (in `context-graph-embeddings`) builds the non-evictable allocator that holds model weights for the lifetime of the warm provider — see §6.

---

## 3. Build pipeline (`build.rs`)

| Phase | Behaviour |
|---|---|
| Cargo rerun gating | `cargo:rerun-if-changed=build.rs`, `kernels/`; `rerun-if-env-changed=CUDA_PATH`, `CUDA_HOME`, `CUDA_ARCH`, `NVCC_FLAGS` |
| Feature gate | When `cuda` is **disabled**, `build.rs` PANICS — there is no non-CUDA fallback (`build.rs:49-52`) |
| Arch enforcement | Reads `CUDA_ARCH` (default `sm_120a`); panics on any other value (`build.rs:62-69`) |
| nvcc discovery | Tries `CUDA_PATH/bin/nvcc`, then `CUDA_HOME/bin/nvcc`, then `/usr/local/cuda-13.2/bin/nvcc`, then `/usr/local/cuda/bin/nvcc`, finally `which nvcc`. Panics if not found |
| nvcc validation | Runs `nvcc --version`; panics unless output contains `release 13.2` (`build.rs:215-233`) |
| Kernel compile | `nvcc --cubin -O3 -arch sm_120a -Xptxas -O3 --use_fast_math --restrict -DNDEBUG <NVCC_FLAGS> -o $OUT_DIR/<name>.cubin <source>`. Panics if nvcc fails or output is empty |
| Image embed | Generates `OUT_DIR/<name>_image.rs` with `pub const IMAGE: &[u8] = include_bytes!("…cubin")`; `ffi/knn.rs:21` does `include!()` on it |
| Driver linking | `cargo:rustc-link-lib=cuda`; adds `/usr/lib/wsl/lib`, `$CUDA_PATH/lib64`, `/usr/local/cuda/lib64`, `/usr/local/cuda/lib`, `/opt/cuda/lib64` to search paths |
| FAISS linking | Only when `faiss-working`: searches `$HOME/.local/lib`, `/usr/local/lib`, `/usr/lib`, `/usr/lib/x86_64-linux-gnu` for `libfaiss_c.so`; FAILS FAST if missing |

Compile flags rationale:

| Flag | Reason |
|---|---|
| `--cubin` | Native Blackwell SASS, no PTX JIT |
| `-O3` / `-Xptxas -O3` | Highest optimization at both nvcc and PTX-assembler stages |
| `-arch sm_120a` | RTX 5090 consumer Blackwell — `a` suffix unlocks SASS-specific instructions |
| `--use_fast_math` | Acceptable for embedding workloads (cosines, L2 norms — 1-ULP tolerance) |
| `--restrict` | Strict aliasing analysis on `__restrict__` pointers |
| `-DNDEBUG` | Strip asserts |

The cubin is embedded into the binary (`include_bytes!`) and loaded at runtime via `cuModuleLoadData`. This avoids cudart's static-init bug on WSL2 with CUDA 13.x and skips PTX JIT cost on first launch.

---

## 4. Custom CUDA kernels

`kernels/knn.cu` compiled to native sm_120a cubin. `BLOCK_SIZE = 256` (`ffi/knn.rs:329`).

| Kernel symbol | Grid layout | Inputs | Output | Notes |
|---|---|---|---|---|
| `compute_core_distances_kernel` | `n_points` threads, one per query | `vectors[n_points × dim] (f32)`, `n_points`, `dim`, `k` | `core_dists[n_points]` (sqrt of L2² to k-th neighbour) | Stack-resident `top_k[16]`; `k` capped at 16 for register pressure; sorted-insert maintenance |
| `compute_pairwise_distances_kernel` | `total_pairs = n·(n-1)/2` threads | `vectors[n_points × dim] (f32)`, `n_points`, `dim` | `distances[total_pairs]` upper-triangular (i<j) | Linear-to-pair index conversion via `j = round(0.5 + sqrt(0.25 + 2t))`, `i = t - j·(j-1)/2`; bounds-check correction |

Both kernels compute L2 squared distance via `__device__ __forceinline__ float l2_distance_squared(...)`.

Host wrappers (`src/ffi/knn.rs::compute_core_distances_gpu`, `compute_pairwise_distances_gpu`) perform: `cuInit`-once → `CudaContext::new(0)` → `CudaModule::load_image(IMAGE)` → `cuModuleGetFunction` → `GpuBuffer::new` (alloc) → `copy_from_host` → `cuLaunchKernel` → `cuCtxSynchronize` → `copy_to_host`.

### 4.1 Bounds-check fixes (BLD-02 / BLD-04 / BLD-08 / CRIT-07)

Host wrappers explicitly bounds-check every parameter before casting to `i32`/`u32` to prevent silent truncation:

- BLD-08: `compute_core_distances_gpu` validates `n_points`, `dimension`, **and** `k` against `i32::MAX`.
- BLD-02: `compute_pairwise_distances_gpu` validates `n_points` and `dimension` against `i32::MAX`.
- BLD-04: `num_pairs = n_points · (n_points - 1) / 2` uses `checked_mul().and_then(checked_div)` to prevent `usize` overflow on 32-bit platforms.
- CRIT-07: `total_pairs` is checked against `u32::MAX` before being narrowed for the grid-launch dimension.

`tests/native_cubin_smoke.rs` validates the kernels end-to-end on a 3-point / 2-D example, asserts CUDA is available (no CPU fallback), and exercises the empty-input and invalid-vector-length error paths.

---

## 5. CUDA Driver API constants

`src/ffi/cuda_driver.rs`:

| Constant | Value |
|---|---|
| `CUDA_SUCCESS` | 0 |
| `CUDA_ERROR_NOT_INITIALIZED` | 3 |
| `CUDA_ERROR_NO_DEVICE` | 100 |
| `CUDA_ERROR_INVALID_DEVICE` | 101 |
| `CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK` | 1 |
| `CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X` | 2 |
| `CU_DEVICE_ATTRIBUTE_WARP_SIZE` | 10 |
| `CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR` | 75 |
| `CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR` | 76 |

`decode_driver_version(encoded) -> (major, minor)` decodes `(major × 1000 + minor × 10)` (e.g. `13020 → (13, 2)`); `is_cuda_success(code)` is a `const fn`; `cuda_result_to_string` formats the four named error codes plus `CUDA_ERROR_UNKNOWN` for everything else. The init result is cached in `static CUDA_INIT_RESULT: AtomicI32` (BLD-03 fix; replaced deprecated `static mut`). The same pattern lives in `safe/device.rs:33-34` and `ffi/knn.rs:84`.

### 5.1 Why Driver API (not Runtime API)

`ffi/cuda_driver.rs:6-13`: CUDA 13.2 on WSL2 with RTX 5090 has a bug where `cudaGetDeviceProperties` (Runtime API) segfaults. The Driver API (`cuDeviceGetAttribute`) works correctly and is also faster (nanoseconds vs milliseconds).

---

## 6. VRAM allocation lifecycle (`warm/cuda_alloc/`)

The warm subsystem is the only path that allocates non-evictable VRAM for model weights. Every model weight is held by a Candle `Tensor` whose `Drop` calls `cudaFree`.

`WarmCudaAllocator` fields (`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator.rs:50-74`):

| Field | Type | Purpose |
|---|---|---|
| `device_id` | `u32` | CUDA ordinal |
| `gpu_info` | `Option<GpuInfo>` | Cached compute-cap / vram |
| `total_allocated_bytes` | `usize` | Accounting |
| `allocation_history` | `Vec<String>` (capped at `MAX_ALLOCATION_HISTORY = 100`) | Diagnostics ring |
| `next_alloc_id` | `u64` | Monotonic opaque handle |
| `live_tensors` | `Vec<(u64, Box<dyn Any + Send>)>` | **Owns the candle::Tensor that holds the GPU allocation** |

### 6.1 `allocate_protected(size_bytes)` (`allocator_cuda.rs:231-289`)

1. Acquire CUDA `Device::new_cuda(device_id)`.
2. `Tensor::zeros((num_elements,), DType::F32, &device)` where `num_elements = size_bytes.div_ceil(4)`.
3. Increment `next_alloc_id` (saturating; never returns 0).
4. Push `(alloc_id, Box::new(tensor))` into `live_tensors` so the tensor stays alive.
5. Return `VramAllocation::new_protected(alloc_id, size_bytes, device_id)` — `ptr` carries `alloc_id`, **not** a raw CUDA pointer.
6. Update `total_allocated_bytes` and `allocation_history` (ring-trimmed at `MAX_ALLOCATION_HISTORY`).

### 6.2 `free_protected(&VramAllocation)` (`allocator_cuda.rs:299-342`)

1. If allocation invalid (`ptr == 0`) → `Ok(())`.
2. Find `(id, _)` in `live_tensors` matching `allocation.ptr`.
3. `swap_remove` and drop the `Box<dyn Any>` — drops the inner `candle::Tensor`, which calls `cudaFree` via Candle's destructor.
4. Decrement `total_allocated_bytes` (saturating).
5. Append `FREE: …` to history.

### 6.3 The H3 root-cause fix (load-bearing)

The previous implementation used `mem::forget(tensor)` + `std::ptr::addr_of!(tensor) as u64` to fabricate a "pointer." The address captured a **stack frame** that became invalid after return, so `free_protected()` could never reach the right tensor — every model weight leaked until process exit.

The fix:

- Replaced `mem::forget` + stack-address with `live_tensors: Vec<(u64, Box<dyn Any + Send>)>` keyed by a monotonic `alloc_id`.
- `VramAllocation.ptr` carries the opaque `alloc_id`, not a raw GPU address.
- `free_protected()` removes the entry; dropping the boxed `candle::Tensor` lets Candle's destructor run `cudaFree`.
- `VramAllocation` no longer derives `Clone` — duplicate handles would each try to free the same tensor.
- `WarmCudaAllocator` is intentionally not internally synchronized (`allocator.rs:40-43`); the warm singleton wraps it in `Arc<Mutex<...>>`.

### 6.4 Fake-pointer detection (AP-007)

`is_fake_pointer(ptr) -> bool` flags addresses inside the synthetic mock range `0x7f80_0000_0000` ± 256 GiB. Constants in `warm/cuda_alloc/constants.rs:43-58`:

| Constant | Value |
|---|---|
| `FAKE_ALLOCATION_BASE_PATTERN` | `0x7f80_0000_0000` |
| `FAKE_ALLOCATION_BASE_MASK` | `0xFFFF_F000_0000_0000` |
| `SIN_WAVE_ENERGY_THRESHOLD` | 0.80 (80 % FFT energy concentration trips the fake-sin-wave detector) |
| `GOLDEN_SIMILARITY_THRESHOLD` | 0.99 (cosine to golden reference) |

`allocate_protected_with_verification(size, name)` is the sanctioned entry-point that runs `verify_real_allocation` after allocation; on a fake pointer it logs `EMB-E009` with full diagnostic context and returns `WarmError::FakeAllocationDetected`, which the binary surfaces as exit code 109.

---

## 7. GPU device singleton (Candle path)

`context-graph-embeddings/src/gpu/device/`:

- `init_gpu()` initializes the singleton; idempotent.
- `device() -> Device` returns the global Candle device (`Device::Cuda(0)` when GPU available).
- `default_dtype() -> DType` typically `DType::F16` (Tensor Core acceleration per ARCH-GPU-07).
- `is_gpu_available() -> bool`, `require_gpu() -> Result<()>`.
- `get_gpu_info() -> GpuInfo` returns `{ name, total_vram, compute_capability, available }`.

---

## 8. Memory budget (`gpu/memory/budget.rs`)

`MemoryBudget::rtx_5090()` (32 GiB total):

| Pool | Bytes | GiB |
|---|---|---|
| `model_weights` | `16 * GB` | 16 |
| `activation_cache` | `8 * GB` | 8 |
| `working_memory` | `6 * GB` | 6 |
| `reserved` | `2 * GB` | 2 |

`MODEL_BUDGET_BYTES` (`gpu/memory/slots.rs:43`) = **8 GiB** — the per-slot soft cap used by `ModelSlotManager` for LRU bookkeeping (separate from the 16 GiB weights pool).

`MemoryPressure` (`memory/pressure.rs`): nominal / elevated / critical, based on watermarks. Pools enforce budgets and refuse over-allocation rather than silently degrading. `VramTracker` keeps per-component byte counts (`model_weights`, `activation_cache`, `working_memory`, …) and exposes `MemoryStats` snapshots.

---

## 9. GPU-batched cosine similarity (`src/similarity.rs`)

| Symbol | Purpose |
|---|---|
| `DimensionGroup` enum (`Dim1024`, `Dim768`, `Dim512`, `Dim1536`) | Partitions dense embedders by dimension |
| `embedder_to_group(idx) -> Option<DimensionGroup>` | E1/E8/E9 → 1024D, E5/E10/E11 → 768D, E2/E3/E4 → 512D, E7 → 1536D, E6/E12/E13 → `None` |
| `DENSE_EMBEDDER_INDICES: [usize; 10]` | `[0, 1, 2, 3, 4, 6, 7, 8, 9, 10]` — embedders that participate in batched GPU similarity |
| `GPU_BATCH_THRESHOLD: usize` | 100 — below this, CPU path is used |
| `should_use_gpu_batch(n_candidates) -> bool` | Gate |
| `BatchedQueryContext` | Pre-normalized query tensors grouped by dimension; max-dim 1536 storage `[[f32; 1536]; 10]` |
| `compute_batch_cosine_similarity(...)` | One matmul per dimension group (4 groups → 4 kernel launches for the full 10-embedder batch) |
| `compute_batch_cosine_similarity_chunked(...)` | Chunked variant for memory-bounded large candidate sets; `BATCH_SIZE = 2048` |

### 9.1 Cosine return-range invariant (load-bearing)

Both `VectorOps::cosine_similarity` (`ops.rs:11-22`) and `compute_batch_cosine_similarity` (`similarity.rs:178-274`) return **raw cosine in `[-1.0, 1.0]`**, NOT normalized to `[0, 1]`. Any caller that feeds the result into the fusion pipeline MUST apply `(raw + 1.0) / 2.0` before mixing with other embedder scores (the SRC-3 normalization rule).

The lesson lives in `tasks/lessons.md`: `code/store.rs cosine_similarity_with_norm` was the canonical case where this invariant was forgotten — `CodeStore` returned raw `[-1, 1]` while every other path returned `[0, 1]`, silently corrupting fusion math.

Per ARCH-GPU-06: 10 dense embedders → 4 matmul launches per query (one per dimension group) instead of 10 (one per embedder) or 10·N (one per candidate). Sparse and token-level embedders never enter this path; they have their own retrieval primitives — see [06_search_and_fusion.md](06_search_and_fusion.md).

---

## 10. GPU HDBSCAN (`src/hdbscan/`)

ARCH-GPU-05: HDBSCAN clustering must run on GPU. `AP-GPU-04` forbids sklearn HDBSCAN. Performance target: `< 20 ms` topic detection (`hdbscan/clusterer.rs:22`).

| Phase | Location | Complexity |
|---|---|---|
| Core distance (k-NN) | GPU via `GpuKnnIndex` (`gpu_knn.rs`) — calls the custom `compute_core_distances_gpu` (the cubin-loaded kernel from §4) | O(n·k·d) on GPU |
| Mutual reachability `MR(a,b) = max(core_a, core_b, dist(a,b))` | CPU (`compute_mutual_reachability`) | O(n²) but trivial |
| MST (Prim's algorithm) | CPU (`build_mst`) | O(n² log n) |
| Cluster extraction (Union-Find + gap detection) | CPU (`extract_clusters`) | O(n α(n)) |
| Core point identification (CUDA-H1 fix) | CPU on GPU-derived core distances | O(n log n) sort per cluster |

Public types:

- `GpuHdbscanClusterer`, `HdbscanParams { min_cluster_size: 3, min_samples: 2, cluster_selection_method }`, `ClusterSelectionMethod { EOM (default), Leaf }`.
- `ClusterMembership { memory_id: Uuid, cluster_id: i32 (-1 = noise), membership_probability ∈ [0,1], is_core: bool }`.
- `GpuKnnIndex` — `new(dimension)`, `add(vectors)`, `compute_core_distances(k)`, `compute_core_distances_with_vectors(vectors, k)`. `cuda_available()` is checked at construction; missing GPU → `GpuHdbscanError::GpuNotAvailable`.
- `GpuHdbscanError` enumerates failures with no fallback to CPU clustering.

### 10.1 Gap detection

`detect_gap_threshold` (`clusterer.rs:472-500`) uses an absolute threshold of `0.5` L2 distance (≈ 0.75 cosine similarity) for semantic embeddings, with a 2× ratio jump as the cluster-boundary heuristic. `extract_clusters` then assigns labels via Union-Find, dropping clusters smaller than `min_cluster_size` to noise (`label = -1`).

### 10.2 CUDA-H1: real spatial core points

`identify_core_points` (`clusterer.rs:512-558`) was previously using global label counts to label "core" vs "border" points. The fix uses the **GPU-computed core distances**: a point is core iff its core distance is at or below the median core distance for its cluster. Points above the cluster median are border points (density-reachable but at the periphery).

The MCP / cluster manager currently routes through the **CPU** `HDBSCANClusterer` (in `context-graph-core`, see [12](12_clustering_and_topics.md)); the GPU clusterer in this crate is exposed as a parallel implementation but is not wired into `MultiSpaceClusterManager::recluster` today.

---

## 11. FAISS GPU (optional)

`faiss-working` Cargo feature, default OFF.

- When ON, `ffi/faiss.rs` is compiled and re-exports `MetricType { InnerProduct = 0, L2 = 1 }`, `FaissIndex`, `FaissStandardGpuResources`, `FaissGpuResourcesProvider`, `FaissGpuResources` (RAII alias for `GpuResources`), `FAISS_OK = 0`, plus the FAISS C-API extern functions (`faiss_index_factory`, `faiss_Index_train`, `faiss_Index_add_with_ids`, `faiss_Index_search`, `faiss_IndexIVF_set_nprobe`, `faiss_index_cpu_to_gpu`, `faiss_StandardGpuResources_new/free`, `faiss_get_num_gpus`, `faiss_write_index`, `faiss_read_index`).
- Linker links `libfaiss_c.so` and `libfaiss.so`; `build.rs` FAILS FAST if `libfaiss_c.so` is missing.
- WSL2 + CUDA 13.2 standard FAISS builds crash at static-init. The crate ships `scripts/rebuild_faiss_gpu.sh` to produce a CUDA 13.2 + sm_120 + lazy-init build.
- Runtime probe `is_faiss_gpu_available()` (`ffi/mod.rs:67-90`) returns `true` only when the feature is on **and** subprocess GPU detection succeeds. On WSL2 (`/usr/lib/wsl/lib` exists), `gpu_available()` first runs `nvidia-smi --query-gpu=count`, then compiles a small C test binary against `libfaiss_c` and runs it in a subprocess to verify FAISS doesn't crash before letting the main process touch it.
- `SKIP_GPU_TESTS=1` env var forces `gpu_available()` to return `false`.
- When OFF, the custom k-NN kernel (§4) is the sole provider. `is_faiss_gpu_available()` always returns `false` (logged once via `tracing::info!` with the enable instructions).
- `faiss_status()` returns: `"FAISS GPU: ENABLED and WORKING"`, `"FAISS GPU: ENABLED but GPU unavailable (check CUDA installation)"`, or `"FAISS GPU: DISABLED (faiss-working feature not enabled)"`.

**Current state.** The default workspace build runs without FAISS — `faiss-working` is OFF, the FAISS module is not compiled, and HDBSCAN core-distance computation goes through the custom k-NN cubin (`compute_core_distances_gpu`), not through `faiss_Index_search`. Earlier docs that describe HDBSCAN as "FAISS-backed" reflect an older design; today the GPU HDBSCAN path uses the in-house Driver-API kernel.

---

## 12. Green Contexts (SM partitioning)

`src/context/green_contexts.rs`:

| Constant | Value |
|---|---|
| `GREEN_CONTEXTS_MIN_COMPUTE_MAJOR` | 7 |
| `GREEN_CONTEXTS_MIN_COMPUTE_MINOR` | 0 |
| `INFERENCE_PARTITION_PERCENT` | 0.70 |
| `BACKGROUND_PARTITION_PERCENT` | 0.30 |
| `MIN_SMS_FOR_PARTITIONING` | 8 |

- Volta+ (CC ≥ 7.0) only.
- 70 % inference / 30 % background SM split (constitution-mandated).
- Below 8 SMs the overhead exceeds benefits, so partitioning is skipped.
- Public API: `GreenContexts`, `GreenContext`, `GreenContextsConfig`, `should_enable_green_contexts(...)`, `should_enable_green_contexts_with_config(...)`.
- On unsupported GPUs partition methods return `None`; this is **not** an error — the system runs without dedicated SM allocation.

`estimate_sm_count` (`green_contexts.rs:392-411`) uses a hardcoded `(major, minor)` lookup: RTX 5090 (12, x) → 170 SMs, RTX 4090 (8, 9) → 128, H100 (9, 0) → 132, A100 (8, 0) → 108, fallback → 32. This is CUDA-L1 (known limitation): non-flagship GPUs miss the table and disable Green Contexts; the proper fix is `cuDeviceGetAttribute(MULTIPROCESSOR_COUNT)` but requires a live CUDA context at initialization.

---

## 13. Safe device wrapper (`src/safe/device.rs`)

`GpuDevice` RAII pattern:

| Method | Notes |
|---|---|
| `GpuDevice::new(ordinal: i32) -> CudaResult<Self>` | One-time `cuInit(0)` via `Once`; `cuDeviceGet`; `cuCtxCreate_v2` |
| `compute_capability() -> (u32, u32)` | RTX 5090 returns `(12, 0)` |
| `name() -> String` | Reads via `cuDeviceGetName` (256-byte buffer) |
| `memory_info() -> CudaResult<(usize, usize)>` | `(free, total)`; sets context current first |
| `memory_usage_percent() -> f32` | `1.0 - free/total`; returns `0.0` when query fails |
| `ordinal() -> i32` | |
| `Drop` | Calls `cuCtxDestroy_v2`; logs but does **NOT** panic on failure (C2: Drop must not panic) |
| `Send` | Yes (CUDA contexts can be moved) |
| `Sync` | NO (contexts are thread-bound) |

`gpu_memory_usage_percent(ordinal)` is a free function for one-shot queries. The compute-cap check pairs with `warm/cuda_alloc/constants.rs::REQUIRED_COMPUTE_MAJOR/MINOR (12, 0)` — anything below RTX 5090 trips the warm allocator's hardware gate.

---

## 14. Stub vs real backend

`VectorOps` (`ops.rs`) defines the abstract API. Exactly two implementations exist:

| Impl | Build gate | Behaviour |
|---|---|---|
| Real CUDA path (workspace Candle + cudarc) | always — `default-features = ["cuda"]` is on | Goes through `BatchedQueryContext` → `compute_batch_cosine_similarity` for batches; per-element calls bottom out in Candle |
| `StubVectorOps` | `#[cfg(test)]` only (`lib.rs:46-48`, `lib.rs:121-123`) | CPU scalar implementation in `stub.rs`; `is_gpu_available()` returns `false`; `device_name()` returns `"CPU (Stub)"` |

`#[deny(deprecated)]` at the top of `lib.rs` and the `#[allow(deprecated)]` on the test-only `StubVectorOps` re-export turn the stub's deprecation warning into a guardrail: any non-test code that imports it fails to compile.

---

## 15. LLM inference (Hermes 2 Pro Mistral 7B)

`context-graph-causal-agent/Cargo.toml`:

| Feature | Wires |
|---|---|
| `llm` | `dep:llama-cpp-2` |
| `cuda` | `llm + llama-cpp-2/cuda` |

The causal-agent crate uses `llama-cpp-2` for grammar-constrained Hermes-2-Pro-Mistral-7B inference (`crates/context-graph-causal-agent/src/llm/mod.rs`). With `cuda` enabled, llama.cpp's CUDA backend (cuBLAS / cuBLASLt) accelerates the LLM. This is independent of the Candle-driven embedding GPU path: embedding inference runs through `candle-core` 0.10.2 with `cuda`; LLM inference runs through `llama-cpp-2` with `cuda`.

The `context-graph-mcp` crate gates `llm` behind its own feature so the ~500 MB llama-cpp-2 dependency is opt-in.

---

## 16. Error variants (`src/error.rs`)

| `CudaError` variant | When raised |
|---|---|
| `DeviceInitError(String)` | `cuInit`, `cuDeviceGet`, `cuCtxCreate_v2` failures |
| `MemoryError(String)` | `cuMemAlloc_v2`, `cuMemGetInfo_v2`, etc. |
| `KernelError(String)` | `cuLaunchKernel` / `cuCtxSynchronize` failures |
| `DimensionMismatch { expected, actual }` | Shape errors |
| `NoDevice` | `CUDA_ERROR_NO_DEVICE` (100) |
| `NotImplemented(String)` | Stub or unimplemented path |
| `InvalidConfig(String)` | Validation |
| `FaissError { operation, code }` | FAISS C-ABI failure |
| `CudaRuntimeError { operation, code }` | Generic Driver API failure with op + code |
| `InvalidArgument { argument, reason }` | Pre-call validation |
| `TensorError(String)` | Candle Tensor op failure |
| `BatchTooLarge { size, max }` | GPU memory budget guard |

`CudaResult<T> = Result<T, CudaError>`.

---

## 17. Cross-references

- Embedder dimensions and HNSW indexes that consume the GPU similarity path: [05_embedding_subsystem.md](05_embedding_subsystem.md) §3, §9.
- Search strategies and weight profiles that schedule batched cosine work: [06_search_and_fusion.md](06_search_and_fusion.md).
- Storage layer column families that hold quantized vectors loaded from VRAM: [09_storage_layer.md](09_storage_layer.md), [13_teleological_subsystem.md](13_teleological_subsystem.md).
- LoRA training paths that consume the same GPU device singleton: [16_training_subsystem.md](16_training_subsystem.md).
- Topic-detection HDBSCAN consumer (uses the CPU clusterer today): [12_clustering_and_topics.md](12_clustering_and_topics.md).
- Welford streaming statistics that consume GPU-resident fingerprints in constellation compilation: [17_constellation_subsystem.md](17_constellation_subsystem.md).
