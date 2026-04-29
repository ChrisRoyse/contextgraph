# CUDA Acceleration

**Source files covered:**
- `crates/context-graph-cuda/src/**/*.rs`
- `crates/*/Cargo.toml`

## 1. Scope
`context-graph-cuda` exposes FFI bindings, device context handling, similarity kernels, HDBSCAN, allocation, memory pool, and test stubs. Targets RTX 5090 (Blackwell) via candle 0.9.2-alpha. Crates `mcp`, `cli`, `storage`, `embeddings` enable CUDA through default features (`cuda`).

## 2. Module Tree
| Path | Purpose |
| --- | --- |
| `src/ffi/cuda_driver.rs` | CUresult enum, device attribute IDs |
| `src/ffi/faiss.rs` | FAISS C ABI |
| `src/context/` | Device init, current device, green contexts (Blackwell partitioning) |
| `src/similarity.rs` | Batched cosine; dense embedder GPU path |
| `src/hdbscan/` | GPU HDBSCAN clustering |
| `src/allocation.rs` | VRAM allocator (live_tensors registry; tensor → cudaFree) |
| `src/memory.rs` | Memory pool helpers |
| `src/test_stub.rs` | CPU stand-in when CUDA absent |

## 3. Constants
| Source | Constant | Value |
| --- | --- | --- |
| `similarity.rs` | DENSE_EMBEDDER_INDICES | `[0, 1, 2, 3, 4, 6, 7, 8, 9, 10]` |
| `similarity.rs` | GPU_BATCH_THRESHOLD | 100 |
| `ffi/faiss.rs` | FAISS_OK | 0 |
| `ffi/cuda_driver.rs` | CUDA_SUCCESS | 0 |
| `ffi/cuda_driver.rs` | CUDA_ERROR_NOT_INITIALIZED | 3 |
| `ffi/cuda_driver.rs` | CUDA_ERROR_INVALID_DEVICE | 101 |
| `ffi/cuda_driver.rs` | CUDA_ERROR_NO_DEVICE | 100 |
| `ffi/cuda_driver.rs` | CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR | 75 |
| `ffi/cuda_driver.rs` | CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR | 76 |
| `ffi/cuda_driver.rs` | CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK | 1 |
| `ffi/cuda_driver.rs` | CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X | 2 |
| `ffi/cuda_driver.rs` | CU_DEVICE_ATTRIBUTE_WARP_SIZE | 10 |
| `context/green_contexts.rs` | GREEN_CONTEXTS_MIN_COMPUTE_MAJOR / MINOR | 7 / 0 |
| `context/green_contexts.rs` | INFERENCE_PARTITION_PERCENT | 0.70 |
| `context/green_contexts.rs` | BACKGROUND_PARTITION_PERCENT | 0.30 |
| `context/green_contexts.rs` | MIN_SMS_FOR_PARTITIONING | 8 |

## 4. Build Notes
- candle features: `cuda` enables candle-core/candle-nn CUDA backends.
- `cargo build --features cuda` requires CUDA toolkit installed; benches under `crates/context-graph-benchmark/src/bin/gpu_bench.rs`.
- Helper scripts: `scripts/v4_2/cuda13_blackwell.py`, `scripts/v4_2/cuda13_wsl_fsv.py`.

## 5. Runtime Boundary
This document records source constants only. Actual CUDA availability is checked at runtime in `context::current_device()`; downstream code uses `test_stub.rs` when no device is detected.
