---
id: "M04-T23"
title: "Implement Poincare Distance CUDA Kernel"
description: |
  Implement poincare_distance_batch CUDA kernel for GPU-accelerated hyperbolic distance.
  Input: queries[n_q][64], database[n_db][64], curvature c
  Output: distances[n_q][n_db]
  Use shared memory for query caching.
  Performance: <1ms for 1K x 1K distance matrix.
layer: "surface"
status: "pending"
priority: "high"
estimated_hours: 4
sequence: 32
depends_on:
  - "M04-T05"  # PoincareBall Mobius operations (COMPLETE)
spec_refs:
  - "TECH-GRAPH-004 Section 10.1"
  - "constitution.yaml stack.gpu"
  - "docs2/CUDA-13-1-RTX-5090-Report.md"
files_to_create:
  - path: "crates/context-graph-cuda/kernels/poincare_distance.cu"
    description: "CUDA kernel for batch Poincare ball distance"
  - path: "crates/context-graph-cuda/src/poincare.rs"
    description: "Rust FFI wrapper for Poincare CUDA kernel"
files_to_modify:
  - path: "crates/context-graph-cuda/src/lib.rs"
    description: "Add poincare module export"
  - path: "crates/context-graph-cuda/Cargo.toml"
    description: "Add cudarc dependency, update features"
  - path: "crates/context-graph-cuda/build.rs"
    description: "Create build.rs for CUDA kernel compilation (file does not exist yet)"
test_file: "crates/context-graph-cuda/src/poincare.rs (inline #[cfg(test)])"
integration_test: "crates/context-graph-graph/tests/cuda_poincare_test.rs"
---

## CRITICAL: Codebase State Audit (2026-01-04)

### Verified Current State

**Dependency M04-T05 (PoincareBall) - COMPLETE:**
- Location: `crates/context-graph-graph/src/hyperbolic/mobius.rs`
- CPU `distance()` function exists at line 207-232
- Formula: `d(x,y) = (2/sqrt(c)) * arctanh(sqrt(c) * ||(-x) ⊕ y||)`
- Uses Mobius subtraction: `neg_x = -x`, then `mobius_add(neg_x, y)`
- 35 passing tests verify correctness
- Performance: <10μs per pair (CPU target met)

**PoincarePoint Structure - COMPLETE:**
- Location: `crates/context-graph-graph/src/hyperbolic/poincare.rs`
- Size: 256 bytes (64 × f32)
- Alignment: 64 bytes (cache-line aligned, SIMD-ready)
- repr(C): FFI-compatible for CUDA kernels (explicitly noted in comments)
- 30 passing tests

**CUDA Crate Current State:**
- Location: `crates/context-graph-cuda/`
- Cargo.toml: Has `stub` and placeholder `cuda` features
- src/lib.rs: Exports `StubVectorOps`, `VectorOps`, `CudaError`, `CudaResult`
- src/stub.rs: CPU fallback implementations
- src/error.rs: Basic error types (6 variants)
- **NO build.rs exists** - must be created
- **NO kernels/ directory exists** - must be created
- **NO .cu files exist** - this is the first CUDA kernel

**HyperbolicConfig:**
- Location: `crates/context-graph-graph/src/config.rs`
- Fields: dim=64, curvature=-1.0, max_norm=0.99999, eps=1e-7
- Method: `abs_curvature()` returns |curvature|

### File Paths Verified (EXACT paths)

| File | Exists | Action |
|------|--------|--------|
| `crates/context-graph-cuda/Cargo.toml` | YES | MODIFY |
| `crates/context-graph-cuda/src/lib.rs` | YES | MODIFY |
| `crates/context-graph-cuda/src/error.rs` | YES | MODIFY |
| `crates/context-graph-cuda/build.rs` | NO | CREATE |
| `crates/context-graph-cuda/kernels/` | NO | CREATE DIR |
| `crates/context-graph-cuda/kernels/poincare_distance.cu` | NO | CREATE |
| `crates/context-graph-cuda/src/poincare.rs` | NO | CREATE |

---

## Context

The Poincare ball distance computation is fundamental for hyperbolic geometry in the knowledge graph. While the CPU implementation (M04-T05) handles single-pair computations at <10μs, batch operations on 1K+ points require GPU acceleration to meet <1ms performance targets.

### Mathematics

**Poincare Ball Distance Formula:**
```
d(x,y) = (2/sqrt(|c|)) * arctanh(sqrt(|c|) * ||(-x) ⊕ y||)
```

Where:
- `c` = curvature (negative, default -1.0)
- `||(-x) ⊕ y||` = norm of Mobius difference
- `⊕` = Mobius addition in Poincare ball

**Mobius Addition:**
```
x ⊕ y = ((1 + 2c<x,y> + c||y||²)x + (1 - c||x||²)y) /
        (1 + 2c<x,y> + c²||x||²||y||²)
```

**Simplified GPU Approach (avoid Mobius add):**
For CUDA kernel efficiency, use the direct formula:
```
d(x,y) = (2/sqrt(c)) * arctanh(sqrt(c * ||x-y||² / ((1-c*||x||²)(1-c*||y||²))))
```

This is mathematically equivalent but more GPU-friendly (avoids intermediate allocation).

---

## RTX 5090 / CUDA 13.1 / Compute 12.0 Optimizations

From `docs2/CUDA-13-1-RTX-5090-Report.md`:

| Feature | Application |
|---------|-------------|
| **170 SMs, 21760 CUDA cores** | Massive parallelism for batch distance |
| **32GB GDDR7, 1792 GB/s BW** | 78% more bandwidth than 4090 |
| **L2 Cache: 98MB** | Cache database points for reuse |
| **Shared Memory: 64KB/SM** | Cache 8 query vectors (8×256 = 2KB) |
| **FP32 Tensor Cores** | Not used for this kernel (arctanh not tensor-friendly) |
| **Green Contexts** | Future: isolate Poincare compute from other GPU work |

### Kernel Optimization Strategy

1. **Shared Memory**: Cache query vectors (8 queries × 64 floats × 4 bytes = 2KB)
2. **Coalesced Access**: Database read is coalesced across threads
3. **Warp Shuffle**: Use `__shfl_down_sync` for norm reduction
4. **Register Pressure**: Keep db_point[64] in registers (256 bytes per thread)
5. **L2 Persistence**: Consider `cudaAccessPolicyWindow` for large databases

---

## Scope

### In Scope
- CUDA kernel `poincare_distance_kernel` for batch distance computation
- Host launcher function `launch_poincare_distance`
- Rust FFI wrapper `poincare.rs` with safe interface
- build.rs for nvcc compilation
- Integration with existing context-graph-cuda error types
- CPU fallback when GPU unavailable

### Out of Scope
- Multi-GPU support (single GPU only)
- Stream pipelining (future M04-T28 GPU memory manager)
- FP16/FP8 quantization (future optimization)
- cudarc binding alternative (use raw FFI for now)
- Modifying context-graph-graph crate (use existing PoincareBall as reference)

---

## Definition of Done

### 1. CUDA Kernel File

**Create: `crates/context-graph-cuda/kernels/poincare_distance.cu`**

```cuda
// CUDA kernel for batch Poincare ball distance computation
// Target: RTX 5090 (Compute Capability 12.0, CUDA 13.1)
// Performance: <1ms for 1K x 1K distance matrix

#include <cuda_runtime.h>
#include <math.h>

// Kernel configuration - tuned for RTX 5090
constexpr int BLOCK_DIM_X = 32;       // Warp size for coalesced access
constexpr int BLOCK_DIM_Y = 8;        // 8 queries per block
constexpr int POINT_DIM = 64;         // Poincare ball dimension (fixed)
constexpr int QUERIES_PER_BLOCK = 8;  // Matches BLOCK_DIM_Y

/**
 * Compute batch Poincare ball distances on GPU.
 *
 * Grid: (ceil(n_database/32), ceil(n_queries/8))
 * Block: (32, 8) = 256 threads
 *
 * Each thread computes one (query, database) distance pair.
 * Shared memory caches query vectors for reuse across database points.
 *
 * @param queries     [n_queries][64] row-major, device memory
 * @param database    [n_database][64] row-major, device memory
 * @param distances   [n_queries][n_database] row-major, device memory (OUTPUT)
 * @param n_queries   Number of query points
 * @param n_database  Number of database points
 * @param curvature   Poincare ball curvature (must be negative, typically -1.0)
 */
__global__ void poincare_distance_kernel(
    const float* __restrict__ queries,
    const float* __restrict__ database,
    float* __restrict__ distances,
    int n_queries,
    int n_database,
    float curvature
) {
    // Shared memory for query vectors and their squared norms
    __shared__ float shared_queries[QUERIES_PER_BLOCK][POINT_DIM];
    __shared__ float shared_query_norms_sq[QUERIES_PER_BLOCK];

    const int tx = threadIdx.x;  // 0-31: database point index within tile
    const int ty = threadIdx.y;  // 0-7: query index within block
    const int bx = blockIdx.x;   // Database tile index
    const int by = blockIdx.y;   // Query tile index

    const int query_idx = by * QUERIES_PER_BLOCK + ty;
    const int db_idx = bx * BLOCK_DIM_X + tx;

    // Curvature magnitude and precomputed constants
    const float c = fabsf(curvature);
    const float sqrt_c = sqrtf(c);
    const float two_over_sqrt_c = 2.0f / sqrt_c;

    // Phase 1: Load query vectors into shared memory
    // Each thread in a row loads part of the query vector
    if (query_idx < n_queries) {
        for (int d = tx; d < POINT_DIM; d += BLOCK_DIM_X) {
            shared_queries[ty][d] = queries[query_idx * POINT_DIM + d];
        }
    }
    __syncthreads();

    // Phase 2: Compute query norms (parallel reduction per query)
    if (query_idx < n_queries && tx == 0) {
        float norm_sq = 0.0f;
        #pragma unroll 8
        for (int d = 0; d < POINT_DIM; d++) {
            float val = shared_queries[ty][d];
            norm_sq += val * val;
        }
        shared_query_norms_sq[ty] = norm_sq;
    }
    __syncthreads();

    // Phase 3: Compute distance for this (query, database) pair
    if (query_idx < n_queries && db_idx < n_database) {
        // Load database point into registers
        float db_point[POINT_DIM];
        float db_norm_sq = 0.0f;

        #pragma unroll 8
        for (int d = 0; d < POINT_DIM; d++) {
            db_point[d] = database[db_idx * POINT_DIM + d];
            db_norm_sq += db_point[d] * db_point[d];
        }

        // Get cached query norm
        float query_norm_sq = shared_query_norms_sq[ty];

        // Compute ||x - y||²
        float diff_norm_sq = 0.0f;
        #pragma unroll 8
        for (int d = 0; d < POINT_DIM; d++) {
            float diff = shared_queries[ty][d] - db_point[d];
            diff_norm_sq += diff * diff;
        }

        // Denominators: (1 - c*||x||²) and (1 - c*||y||²)
        // Clamp to avoid division by zero at boundary
        float denom_x = fmaxf(1.0f - c * query_norm_sq, 1e-7f);
        float denom_y = fmaxf(1.0f - c * db_norm_sq, 1e-7f);

        // Compute arctanh argument: sqrt(c * ||x-y||² / (denom_x * denom_y))
        float arg = sqrtf(c * diff_norm_sq / (denom_x * denom_y));

        // Clamp to valid arctanh domain: (-1, 1)
        arg = fminf(arg, 1.0f - 1e-7f);

        // Poincare distance: (2/sqrt(c)) * arctanh(arg)
        float dist = two_over_sqrt_c * atanhf(arg);

        // Write result
        distances[query_idx * n_database + db_idx] = dist;
    }
}

/**
 * Host function to launch Poincare distance kernel.
 *
 * @param d_queries    Device pointer to query vectors
 * @param d_database   Device pointer to database vectors
 * @param d_distances  Device pointer to output distance matrix
 * @param n_queries    Number of query points
 * @param n_database   Number of database points
 * @param curvature    Poincare ball curvature (negative)
 * @param stream       CUDA stream (nullptr for default stream)
 * @return cudaError_t Error code (cudaSuccess on success)
 */
extern "C" cudaError_t launch_poincare_distance(
    const float* d_queries,
    const float* d_database,
    float* d_distances,
    int n_queries,
    int n_database,
    float curvature,
    cudaStream_t stream
) {
    // Validate curvature is negative
    if (curvature >= 0.0f) {
        return cudaErrorInvalidValue;
    }

    // Calculate grid dimensions
    dim3 block(BLOCK_DIM_X, BLOCK_DIM_Y);  // 32 x 8 = 256 threads
    dim3 grid(
        (n_database + BLOCK_DIM_X - 1) / BLOCK_DIM_X,
        (n_queries + QUERIES_PER_BLOCK - 1) / QUERIES_PER_BLOCK
    );

    // Launch kernel
    poincare_distance_kernel<<<grid, block, 0, stream>>>(
        d_queries, d_database, d_distances,
        n_queries, n_database, curvature
    );

    return cudaGetLastError();
}

/**
 * Single-pair Poincare distance (for API completeness).
 * Prefer batch version for efficiency.
 */
extern "C" cudaError_t poincare_distance_single(
    const float* d_point_a,
    const float* d_point_b,
    float* d_distance,
    float curvature,
    cudaStream_t stream
) {
    return launch_poincare_distance(
        d_point_a, d_point_b, d_distance,
        1, 1, curvature, stream
    );
}
```

### 2. Rust FFI Wrapper

**Create: `crates/context-graph-cuda/src/poincare.rs`**

```rust
//! CUDA-accelerated Poincare ball distance computation.
//!
//! Provides GPU-accelerated batch distance computation for hyperbolic geometry.
//! Falls back to CPU when GPU is unavailable.
//!
//! # Performance
//!
//! - GPU (RTX 5090): <1ms for 1K × 1K distance matrix
//! - CPU fallback: Uses context-graph-graph PoincareBall
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004 Section 10.1: Poincare CUDA Kernel
//! - perf.latency.poincare_distance_gpu: <1ms for 1K × 1K

use std::ffi::c_void;

use crate::error::{CudaError, CudaResult};

/// Default Poincare ball dimension (fixed for SIMD alignment).
pub const POINCARE_DIM: usize = 64;

/// Default curvature (negative, per hyperbolic geometry).
pub const DEFAULT_CURVATURE: f32 = -1.0;

// FFI declarations for CUDA kernel
mod ffi {
    use std::ffi::c_void;
    use std::os::raw::c_int;

    #[link(name = "poincare_distance", kind = "static")]
    extern "C" {
        pub fn launch_poincare_distance(
            d_queries: *const f32,
            d_database: *const f32,
            d_distances: *mut f32,
            n_queries: c_int,
            n_database: c_int,
            curvature: f32,
            stream: *mut c_void,
        ) -> c_int;

        pub fn poincare_distance_single(
            d_point_a: *const f32,
            d_point_b: *const f32,
            d_distance: *mut f32,
            curvature: f32,
            stream: *mut c_void,
        ) -> c_int;
    }
}

/// Configuration for Poincare CUDA operations.
#[derive(Debug, Clone)]
pub struct PoincareCudaConfig {
    /// Dimension of Poincare ball (must be 64).
    pub dim: usize,
    /// Curvature (must be negative, default -1.0).
    pub curvature: f32,
}

impl Default for PoincareCudaConfig {
    fn default() -> Self {
        Self {
            dim: POINCARE_DIM,
            curvature: DEFAULT_CURVATURE,
        }
    }
}

impl PoincareCudaConfig {
    /// Create config with custom curvature.
    ///
    /// # Errors
    /// Returns error if curvature is not negative.
    pub fn with_curvature(curvature: f32) -> CudaResult<Self> {
        if curvature >= 0.0 {
            return Err(CudaError::InvalidConfig(
                "Poincare curvature must be negative".to_string(),
            ));
        }
        Ok(Self {
            dim: POINCARE_DIM,
            curvature,
        })
    }

    /// Validate configuration.
    pub fn validate(&self) -> CudaResult<()> {
        if self.dim != POINCARE_DIM {
            return Err(CudaError::InvalidConfig(format!(
                "Poincare dimension must be {}, got {}",
                POINCARE_DIM, self.dim
            )));
        }
        if self.curvature >= 0.0 {
            return Err(CudaError::InvalidConfig(
                "Poincare curvature must be negative".to_string(),
            ));
        }
        Ok(())
    }
}

/// Compute batch Poincare distances on GPU.
///
/// # Safety
///
/// - `d_queries`, `d_database`, `d_distances` must be valid device pointers
/// - Arrays must be properly sized: queries[n_queries][64], database[n_database][64]
/// - Output distances[n_queries][n_database] must be pre-allocated
///
/// # Arguments
///
/// * `d_queries` - Device pointer to query vectors [n_queries][64]
/// * `d_database` - Device pointer to database vectors [n_database][64]
/// * `d_distances` - Device pointer to output matrix [n_queries][n_database]
/// * `n_queries` - Number of query points
/// * `n_database` - Number of database points
/// * `config` - Poincare configuration
/// * `stream` - CUDA stream (None for default stream)
///
/// # Errors
///
/// Returns `CudaError` if kernel launch fails.
///
/// # Performance
///
/// Target: <1ms for 1K × 1K distance matrix on RTX 5090.
pub unsafe fn poincare_distance_batch_gpu(
    d_queries: *const f32,
    d_database: *const f32,
    d_distances: *mut f32,
    n_queries: usize,
    n_database: usize,
    config: &PoincareCudaConfig,
    stream: Option<*mut c_void>,
) -> CudaResult<()> {
    config.validate()?;

    let stream_ptr = stream.unwrap_or(std::ptr::null_mut());

    let result = ffi::launch_poincare_distance(
        d_queries,
        d_database,
        d_distances,
        n_queries as i32,
        n_database as i32,
        config.curvature,
        stream_ptr,
    );

    if result != 0 {
        return Err(CudaError::KernelError(format!(
            "Poincare distance kernel failed with CUDA error code {}",
            result
        )));
    }

    Ok(())
}

/// Check if CUDA Poincare kernels are available.
///
/// Returns true if the system has a CUDA-capable GPU and the
/// poincare_distance library is linked.
#[cfg(feature = "cuda")]
pub fn is_poincare_gpu_available() -> bool {
    // Try to query CUDA device count
    // This will fail if no CUDA runtime is available
    unsafe {
        let mut device_count: i32 = 0;
        let result = cuda_runtime_sys::cudaGetDeviceCount(&mut device_count);
        result == 0 && device_count > 0
    }
}

#[cfg(not(feature = "cuda"))]
pub fn is_poincare_gpu_available() -> bool {
    false
}

// ============================================================================
// TESTS - REAL DATA ONLY, NO MOCKS (per constitution REQ-KG-TEST)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = PoincareCudaConfig::default();
        assert_eq!(config.dim, 64);
        assert!((config.curvature - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_config_with_curvature_valid() {
        let config = PoincareCudaConfig::with_curvature(-0.5).unwrap();
        assert!((config.curvature - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn test_config_with_curvature_invalid() {
        let result = PoincareCudaConfig::with_curvature(0.5);
        assert!(result.is_err());

        let result = PoincareCudaConfig::with_curvature(0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validate() {
        let config = PoincareCudaConfig::default();
        assert!(config.validate().is_ok());

        let bad_config = PoincareCudaConfig {
            dim: 128,  // Wrong dimension
            curvature: -1.0,
        };
        assert!(bad_config.validate().is_err());
    }

    #[test]
    fn test_constants() {
        assert_eq!(POINCARE_DIM, 64);
        assert!((DEFAULT_CURVATURE - (-1.0)).abs() < 1e-6);
    }

    // GPU tests require #[requires_gpu] attribute and real hardware
    // See integration_test file for actual GPU tests
}
```

### 3. Update Error Types

**Modify: `crates/context-graph-cuda/src/error.rs`**

Add new error variant:

```rust
/// Invalid configuration parameter.
#[error("Invalid configuration: {0}")]
InvalidConfig(String),
```

### 4. Update lib.rs

**Modify: `crates/context-graph-cuda/src/lib.rs`**

```rust
pub mod poincare;

pub use poincare::{
    poincare_distance_batch_gpu, is_poincare_gpu_available,
    PoincareCudaConfig, POINCARE_DIM, DEFAULT_CURVATURE,
};
```

### 5. Create build.rs

**Create: `crates/context-graph-cuda/build.rs`**

```rust
//! Build script for CUDA kernel compilation.
//!
//! Compiles .cu files in kernels/ directory using nvcc.
//! Links resulting static library for FFI.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Only compile CUDA kernels if cuda feature is enabled
    #[cfg(feature = "cuda")]
    compile_cuda_kernels();

    // Always tell Cargo to re-run if kernels change
    println!("cargo:rerun-if-changed=kernels/");
}

#[cfg(feature = "cuda")]
fn compile_cuda_kernels() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let cuda_arch = env::var("CUDA_ARCH").unwrap_or_else(|_| "sm_120".to_string());

    // Compile poincare_distance.cu
    let kernel_path = "kernels/poincare_distance.cu";
    let obj_path = out_dir.join("poincare_distance.o");
    let lib_path = out_dir.join("libpoincare_distance.a");

    // Check if nvcc is available
    let nvcc_check = Command::new("nvcc")
        .arg("--version")
        .output();

    if nvcc_check.is_err() {
        panic!("nvcc not found. Install CUDA Toolkit 13.1+ or disable 'cuda' feature.");
    }

    // Compile to object file
    let compile_status = Command::new("nvcc")
        .args(&[
            "-c",
            "-O3",
            "-arch", &cuda_arch,
            "--compiler-options", "-fPIC",
            "-o", obj_path.to_str().unwrap(),
            kernel_path,
        ])
        .status()
        .expect("Failed to run nvcc");

    if !compile_status.success() {
        panic!("CUDA kernel compilation failed");
    }

    // Create static library
    let ar_status = Command::new("ar")
        .args(&["rcs", lib_path.to_str().unwrap(), obj_path.to_str().unwrap()])
        .status()
        .expect("Failed to run ar");

    if !ar_status.success() {
        panic!("Failed to create static library");
    }

    // Tell Cargo to link the library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=poincare_distance");

    // Link CUDA runtime
    println!("cargo:rustc-link-lib=cudart");
}
```

### 6. Update Cargo.toml

**Modify: `crates/context-graph-cuda/Cargo.toml`**

```toml
[features]
default = ["stub"]
stub = []
cuda = []  # Enable CUDA kernel compilation

[build-dependencies]
# No external build deps needed - using std::process::Command
```

---

## Constraints

| Constraint | Value | Enforcement |
|------------|-------|-------------|
| Dimension | 64 (fixed) | Compile-time constant in kernel |
| Curvature | Must be negative | Runtime validation, CUDA returns cudaErrorInvalidValue |
| Performance | <1ms for 1K × 1K | Benchmark test |
| Shared memory | 2KB (8 queries × 64 × 4) | QUERIES_PER_BLOCK = 8 |
| Boundary handling | Clamp norms < 1.0 | `fminf(arg, 1.0 - 1e-7)` |
| NaN handling | Propagate from invalid inputs | arctanh clamp prevents NaN |

---

## Acceptance Criteria

- [ ] CUDA kernel compiles with `nvcc -arch=sm_120`
- [ ] Shared memory used for query vector caching
- [ ] Results match CPU PoincareBall::distance() within 1e-5 tolerance
- [ ] Performance: <1ms for 1K × 1K on RTX 5090 (or comparable GPU)
- [ ] Handles boundary cases (points with norm near 0.99999)
- [ ] Handles zero vectors (distance = 0)
- [ ] Compiles with `cargo build -p context-graph-cuda --features cuda`
- [ ] Tests pass with `cargo test -p context-graph-cuda`
- [ ] No clippy warnings
- [ ] Error handling: Returns proper CUDA error codes

---

## Full State Verification Requirements

After completing implementation, you MUST perform these verification steps:

### 1. Source of Truth Definition

The source of truth for this task is:
- **Kernel compilation**: `crates/context-graph-cuda/kernels/poincare_distance.cu` compiled into `libpoincare_distance.a`
- **FFI linkage**: Rust can call `launch_poincare_distance` without linker errors
- **Output data**: GPU-computed distances match CPU reference values

### 2. Execute & Inspect Protocol

After implementation:

```bash
# Step 1: Verify CUDA toolkit
nvcc --version  # Must show CUDA 13.1+

# Step 2: Build with CUDA feature
cd crates/context-graph-cuda
cargo build --features cuda 2>&1 | tee /tmp/cuda_build.log

# Step 3: Check library was created
ls -la target/debug/build/context-graph-cuda-*/out/libpoincare_distance.a

# Step 4: Run tests
cargo test --features cuda 2>&1 | tee /tmp/cuda_test.log

# Step 5: Run integration test (requires GPU)
cargo test -p context-graph-graph cuda_poincare --features faiss-gpu 2>&1 | tee /tmp/integration_test.log
```

### 3. Boundary & Edge Case Audit

You MUST manually test these 3 edge cases with printed state:

#### Edge Case 1: Zero Vectors
```rust
// Query = origin, Database = origin
// Expected: distance = 0.0
let query = [[0.0f32; 64]];
let database = [[0.0f32; 64]];
println!("BEFORE: query_norm={}, db_norm={}", 0.0, 0.0);
// ... call kernel ...
println!("AFTER: distance={}", result[0]);
assert!((result[0] - 0.0).abs() < 1e-6, "Zero vector distance should be 0");
```

#### Edge Case 2: Points Near Boundary (norm = 0.99)
```rust
// Both points near boundary
let scale = 0.99 / (64.0f32).sqrt();
let query = [[scale; 64]];  // norm = 0.99
let database = [[scale; 64]];
println!("BEFORE: query_norm={}, db_norm={}", 0.99, 0.99);
// ... call kernel ...
println!("AFTER: distance={}", result[0]);
assert!(result[0].is_finite(), "Distance must be finite");
assert!(result[0] >= 0.0, "Distance must be non-negative");
```

#### Edge Case 3: Maximum Distance (opposite points)
```rust
// Query at +x, Database at -x (maximally separated)
let mut query = [[0.0f32; 64]];
query[0][0] = 0.9;
let mut database = [[0.0f32; 64]];
database[0][0] = -0.9;
println!("BEFORE: query[0]={}, db[0]={}", 0.9, -0.9);
// ... call kernel ...
println!("AFTER: distance={}", result[0]);
// Large distance expected (hyperbolic space expands near boundary)
assert!(result[0] > 3.0, "Opposite points should have large distance");
```

### 4. Evidence of Success

Provide a log showing:
1. nvcc version output
2. Successful cargo build output
3. Created library file listing
4. Test pass output with actual distance values
5. Benchmark showing <1ms for 1K × 1K

---

## Integration Test

**Create: `crates/context-graph-graph/tests/cuda_poincare_test.rs`**

```rust
//! Integration tests for CUDA Poincare distance kernel.
//!
//! Requires GPU hardware and CUDA 13.1+ runtime.
//! Run with: cargo test -p context-graph-graph --features faiss-gpu cuda_poincare

use context_graph_graph::hyperbolic::{PoincareBall, PoincarePoint};
use context_graph_graph::config::HyperbolicConfig;

/// Helper to generate random Poincare point with norm < 0.9.
fn random_poincare_point(seed: u64) -> [f32; 64] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut coords = [0.0f32; 64];
    for i in 0..64 {
        let mut hasher = DefaultHasher::new();
        (seed, i).hash(&mut hasher);
        let hash = hasher.finish();
        // Map hash to [-1, 1]
        coords[i] = ((hash % 2001) as f32 / 1000.0) - 1.0;
    }

    // Normalize to be inside ball with norm < 0.85
    let norm: f32 = coords.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        let scale = 0.85 / norm;
        for c in &mut coords {
            *c *= scale;
        }
    }
    coords
}

#[test]
#[cfg(feature = "faiss-gpu")]
fn test_cuda_poincare_matches_cpu() {
    use context_graph_cuda::{
        poincare_distance_batch_gpu, is_poincare_gpu_available,
        PoincareCudaConfig,
    };

    if !is_poincare_gpu_available() {
        eprintln!("SKIP: No GPU available");
        return;
    }

    let n_queries = 100;
    let n_database = 100;

    // Generate test data
    let queries: Vec<[f32; 64]> = (0..n_queries)
        .map(|i| random_poincare_point(i as u64))
        .collect();
    let database: Vec<[f32; 64]> = (0..n_database)
        .map(|i| random_poincare_point((i + 1000) as u64))
        .collect();

    // CPU reference computation
    let config = HyperbolicConfig::default();
    let ball = PoincareBall::new(config);
    let mut cpu_distances = vec![0.0f32; n_queries * n_database];

    for (i, q) in queries.iter().enumerate() {
        let query_point = PoincarePoint::from_coords(*q);
        for (j, d) in database.iter().enumerate() {
            let db_point = PoincarePoint::from_coords(*d);
            cpu_distances[i * n_database + j] = ball.distance(&query_point, &db_point);
        }
    }

    // GPU computation
    // (Actual GPU memory allocation code here - depends on CUDA runtime bindings)
    // This is a placeholder - actual implementation needs cudarc or cuda-sys

    // Compare results
    // for i in 0..cpu_distances.len() {
    //     assert!((cpu_distances[i] - gpu_distances[i]).abs() < 1e-5);
    // }

    // For now, verify CPU reference is working
    assert!(cpu_distances.iter().all(|&d| d >= 0.0 && !d.is_nan()));
}

#[test]
#[cfg(feature = "faiss-gpu")]
fn test_cuda_poincare_performance() {
    use context_graph_cuda::is_poincare_gpu_available;

    if !is_poincare_gpu_available() {
        eprintln!("SKIP: No GPU available");
        return;
    }

    let n = 1000;  // 1K x 1K = 1M distances

    // Generate test data
    let queries: Vec<[f32; 64]> = (0..n)
        .map(|i| random_poincare_point(i as u64))
        .collect();
    let database: Vec<[f32; 64]> = (0..n)
        .map(|i| random_poincare_point((i + 10000) as u64))
        .collect();

    // Time GPU computation
    let start = std::time::Instant::now();

    // (GPU kernel call here)

    let elapsed = start.elapsed();

    println!("GPU 1Kx1K Poincare distance: {:?}", elapsed);
    // assert!(elapsed.as_millis() < 1, "Performance target: <1ms, actual: {}ms", elapsed.as_millis());
}
```

---

## Verification Checklist (For Sherlock-Holmes Agent)

At task completion, the sherlock-holmes agent MUST verify:

1. **Files Exist:**
   - [ ] `crates/context-graph-cuda/kernels/poincare_distance.cu`
   - [ ] `crates/context-graph-cuda/src/poincare.rs`
   - [ ] `crates/context-graph-cuda/build.rs`

2. **Kernel Correctness:**
   - [ ] Uses shared memory for query caching
   - [ ] Handles curvature parameter correctly (validates negative)
   - [ ] Clamps arctanh argument to valid range
   - [ ] Output matches CPU reference within 1e-5

3. **Rust Integration:**
   - [ ] FFI declarations match C function signatures
   - [ ] Error handling uses CudaError types
   - [ ] Configuration validation implemented
   - [ ] Module exported from lib.rs

4. **Build System:**
   - [ ] build.rs compiles .cu to static library
   - [ ] Cargo.toml has correct features
   - [ ] Links cudart library

5. **Tests:**
   - [ ] Unit tests for configuration
   - [ ] Integration test comparing GPU vs CPU
   - [ ] Performance benchmark test
   - [ ] Edge case tests (zero, boundary, opposite)

6. **No Mock Data:**
   - [ ] All tests use real computed values
   - [ ] No hardcoded expected results that bypass kernel

---

## Notes for Implementer

1. **M04-T05 is complete** - use `PoincareBall::distance()` as the CPU reference implementation. The formula in mobius.rs lines 207-232 is the ground truth.

2. **PoincarePoint is FFI-ready** - it's `#[repr(C, align(64))]` specifically for this kernel. No additional FFI types needed for the point struct.

3. **No cudarc for now** - the task uses raw FFI via `extern "C"`. This keeps dependencies minimal. Future optimization may add cudarc.

4. **Test with RTX 5090 if available** - the kernel is tuned for Compute 12.0. On older GPUs, use appropriate `-arch` flag.

5. **The CUDA crate has NO existing kernels** - you are creating the entire infrastructure (build.rs, kernels/, etc.)

6. **Fallback strategy**: When `cuda` feature is disabled, `is_poincare_gpu_available()` returns false. Users should use CPU PoincareBall::distance() instead.
