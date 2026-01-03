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
sequence: 31
depends_on:
  - "M04-T05"
spec_refs:
  - "TECH-GRAPH-004 Section 10.1"
files_to_create:
  - path: "crates/context-graph-cuda/kernels/poincare_distance.cu"
    description: "CUDA kernel for batch Poincare ball distance"
  - path: "crates/context-graph-cuda/src/poincare.rs"
    description: "Rust FFI wrapper for Poincare CUDA kernel"
files_to_modify:
  - path: "crates/context-graph-cuda/src/lib.rs"
    description: "Add poincare module"
  - path: "crates/context-graph-cuda/build.rs"
    description: "Add kernel compilation"
test_file: "crates/context-graph-graph/tests/cuda_tests.rs"
---

## Context

The Poincare ball distance computation is a fundamental operation for hyperbolic geometry in the knowledge graph. While the CPU implementation (M04-T05) is suitable for single-pair computations, batch operations require GPU acceleration to meet performance targets. This kernel computes the distance matrix between query points and database points in the Poincare ball model.

The Poincare ball distance formula:
```
d(x,y) = (2/sqrt(|c|)) * arctanh(sqrt(|c| * ||x - y||^2 / ((1 - |c|*||x||^2)(1 - |c|*||y||^2))))
```

## RTX 5090 Optimizations

- **Compute Capability 12.0**: Use latest CUDA features
- **Green Contexts**: Enable for multi-stream execution if available
- **Shared Memory**: 64KB per SM for query caching
- **Warp-Level Primitives**: Use `__shfl_down_sync` for reductions
- **FP16 Computation**: Consider for intermediate computations where precision allows

## Scope

### In Scope
- CUDA kernel for batch Poincare distance
- Shared memory optimization for query caching
- Rust FFI wrapper
- Build system integration

### Out of Scope
- CPU fallback implementation (exists in M04-T05)
- Multi-GPU support
- Stream pipelining (future optimization)
- FP8/FP4 quantization (future optimization)

## Definition of Done

### Signatures

```cuda
// In crates/context-graph-cuda/kernels/poincare_distance.cu

#include <cuda_runtime.h>
#include <cuda_fp16.h>
#include <math.h>

// Kernel configuration
constexpr int BLOCK_DIM_X = 32;      // Threads per block (X dimension)
constexpr int BLOCK_DIM_Y = 8;       // Threads per block (Y dimension)
constexpr int POINT_DIM = 64;        // Poincare ball dimension
constexpr int QUERIES_PER_BLOCK = 8; // Queries cached in shared memory

/**
 * Compute batch Poincare ball distances
 *
 * Each thread block processes QUERIES_PER_BLOCK query points against
 * a tile of database points. Shared memory caches query vectors for
 * efficient reuse.
 *
 * @param queries     Query points [n_queries][POINT_DIM], row-major
 * @param database    Database points [n_database][POINT_DIM], row-major
 * @param distances   Output distances [n_queries][n_database], row-major
 * @param n_queries   Number of query points
 * @param n_database  Number of database points
 * @param curvature   Poincare ball curvature (negative, typically -1.0)
 */
__global__ void poincare_distance_kernel(
    const float* __restrict__ queries,
    const float* __restrict__ database,
    float* __restrict__ distances,
    int n_queries,
    int n_database,
    float curvature
) {
    // Shared memory for query vectors
    __shared__ float shared_queries[QUERIES_PER_BLOCK][POINT_DIM];
    __shared__ float shared_query_norms[QUERIES_PER_BLOCK];

    // Thread indices
    const int tx = threadIdx.x;  // Used for dimension reduction
    const int ty = threadIdx.y;  // Used for query index within block
    const int bx = blockIdx.x;   // Database block index
    const int by = blockIdx.y;   // Query block index

    // Global indices
    const int query_idx = by * QUERIES_PER_BLOCK + ty;
    const int db_idx = bx * BLOCK_DIM_X + tx;

    // Curvature magnitude
    const float c = fabsf(curvature);
    const float sqrt_c = sqrtf(c);
    const float inv_2_sqrt_c = 2.0f / sqrt_c;

    // Load queries into shared memory
    if (ty < QUERIES_PER_BLOCK && query_idx < n_queries) {
        // Each thread in ty loads one element
        for (int d = tx; d < POINT_DIM; d += BLOCK_DIM_X) {
            shared_queries[ty][d] = queries[query_idx * POINT_DIM + d];
        }
    }
    __syncthreads();

    // Compute query norms (parallel reduction)
    if (ty < QUERIES_PER_BLOCK && query_idx < n_queries) {
        float norm_sq = 0.0f;
        for (int d = 0; d < POINT_DIM; d++) {
            float val = shared_queries[ty][d];
            norm_sq += val * val;
        }
        if (tx == 0) {
            shared_query_norms[ty] = norm_sq;
        }
    }
    __syncthreads();

    // Process each query-database pair
    if (query_idx < n_queries && db_idx < n_database) {
        // Load database point
        float db_point[POINT_DIM];
        float db_norm_sq = 0.0f;

        for (int d = 0; d < POINT_DIM; d++) {
            db_point[d] = database[db_idx * POINT_DIM + d];
            db_norm_sq += db_point[d] * db_point[d];
        }

        // Get query norm from shared memory
        float query_norm_sq = shared_query_norms[ty];

        // Compute ||x - y||^2
        float diff_norm_sq = 0.0f;
        for (int d = 0; d < POINT_DIM; d++) {
            float diff = shared_queries[ty][d] - db_point[d];
            diff_norm_sq += diff * diff;
        }

        // Denominators: (1 - c*||x||^2)(1 - c*||y||^2)
        float denom_x = 1.0f - c * query_norm_sq;
        float denom_y = 1.0f - c * db_norm_sq;

        // Handle boundary cases
        denom_x = fmaxf(denom_x, 1e-7f);
        denom_y = fmaxf(denom_y, 1e-7f);

        // Argument to arctanh
        float arg = sqrtf(c * diff_norm_sq / (denom_x * denom_y));
        arg = fminf(arg, 1.0f - 1e-7f);  // Clamp to valid range

        // Poincare distance: (2/sqrt(c)) * arctanh(arg)
        float dist = inv_2_sqrt_c * atanhf(arg);

        // Write result
        distances[query_idx * n_database + db_idx] = dist;
    }
}

/**
 * Host function to launch Poincare distance kernel
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
    // Calculate grid dimensions
    dim3 block(BLOCK_DIM_X, BLOCK_DIM_Y);
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
 * Single-pair Poincare distance (convenience function)
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

```rust
// In crates/context-graph-cuda/src/poincare.rs

use std::ptr::NonNull;

/// FFI declarations for CUDA Poincare distance kernel
mod ffi {
    use std::os::raw::c_int;

    extern "C" {
        pub fn launch_poincare_distance(
            d_queries: *const f32,
            d_database: *const f32,
            d_distances: *mut f32,
            n_queries: c_int,
            n_database: c_int,
            curvature: f32,
            stream: *mut std::ffi::c_void,
        ) -> c_int;  // cudaError_t
    }
}

/// Error from CUDA Poincare kernel
#[derive(Debug, Clone)]
pub struct PoincareCudaError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for PoincareCudaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CUDA Poincare error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for PoincareCudaError {}

/// Result type for Poincare CUDA operations
pub type PoincareCudaResult<T> = Result<T, PoincareCudaError>;

/// Poincare ball configuration for CUDA operations
#[derive(Debug, Clone)]
pub struct PoincareCudaConfig {
    /// Dimension of Poincare ball (must be 64)
    pub dim: usize,
    /// Curvature (must be negative)
    pub curvature: f32,
}

impl Default for PoincareCudaConfig {
    fn default() -> Self {
        Self {
            dim: 64,
            curvature: -1.0,
        }
    }
}

/// Compute batch Poincare distances on GPU
///
/// # Arguments
/// * `queries` - Query points device pointer [n_queries][64]
/// * `database` - Database points device pointer [n_database][64]
/// * `distances` - Output device pointer [n_queries][n_database]
/// * `n_queries` - Number of query points
/// * `n_database` - Number of database points
/// * `config` - Poincare ball configuration
/// * `stream` - CUDA stream (null for default stream)
///
/// # Safety
/// All pointers must be valid device pointers with correct sizes.
pub unsafe fn poincare_distance_batch(
    queries: NonNull<f32>,
    database: NonNull<f32>,
    distances: NonNull<f32>,
    n_queries: usize,
    n_database: usize,
    config: &PoincareCudaConfig,
    stream: Option<NonNull<std::ffi::c_void>>,
) -> PoincareCudaResult<()> {
    let stream_ptr = stream
        .map(|s| s.as_ptr())
        .unwrap_or(std::ptr::null_mut());

    let result = ffi::launch_poincare_distance(
        queries.as_ptr(),
        database.as_ptr(),
        distances.as_ptr() as *mut f32,
        n_queries as i32,
        n_database as i32,
        config.curvature,
        stream_ptr,
    );

    if result != 0 {
        return Err(PoincareCudaError {
            code: result,
            message: format!("Kernel launch failed with code {}", result),
        });
    }

    Ok(())
}

/// Convenience wrapper for poincare_distance_batch with Vec inputs
pub fn poincare_distance_batch_host(
    queries: &[[f32; 64]],
    database: &[[f32; 64]],
    config: &PoincareCudaConfig,
) -> PoincareCudaResult<Vec<f32>> {
    // This would allocate GPU memory, copy, compute, copy back
    // Full implementation requires CUDA memory management
    unimplemented!("Requires GPU memory allocation - see GpuMemoryManager")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = PoincareCudaConfig::default();
        assert_eq!(config.dim, 64);
        assert!((config.curvature - (-1.0)).abs() < 1e-6);
    }
}
```

### Constraints
- Dimension MUST be 64 (fixed for SIMD alignment)
- Curvature MUST be negative
- Performance target: <1ms for 1K x 1K distance matrix
- Shared memory: QUERIES_PER_BLOCK queries cached
- Boundary handling: Clamp norm near 1.0
- RTX 5090 Compute Capability 12.0 features

### Acceptance Criteria
- [ ] CUDA kernel compiles with nvcc
- [ ] Shared memory used for query vectors
- [ ] Matches CPU implementation within 1e-5 tolerance
- [ ] Performance: <1ms for 1K x 1K
- [ ] Handles boundary cases (points near norm=1)
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```cuda
// Thread block layout: 32 x 8 = 256 threads
// Each block processes 8 queries x 32 database points

shared float queries[8][64];      // 2KB shared memory
shared float query_norms[8];

// Phase 1: Load queries to shared memory
for d in tx..64 step 32:
    shared_queries[ty][d] = queries[query_idx * 64 + d]
sync()

// Phase 2: Compute query norms (reduction)
norm_sq = sum(shared_queries[ty][d]^2 for d in 0..64)
if tx == 0: shared_query_norms[ty] = norm_sq
sync()

// Phase 3: Compute distance for (query_idx, db_idx)
db_point[64] = load from global memory
db_norm_sq = ||db_point||^2
query_norm_sq = shared_query_norms[ty]
diff_norm_sq = ||shared_queries[ty] - db_point||^2

// Poincare formula
denom_x = max(1 - c * query_norm_sq, 1e-7)
denom_y = max(1 - c * db_norm_sq, 1e-7)
arg = sqrt(c * diff_norm_sq / (denom_x * denom_y))
arg = min(arg, 1 - 1e-7)
distance = (2/sqrt(c)) * arctanh(arg)

distances[query_idx * n_database + db_idx] = distance
```

### RTX 5090 Specific Optimizations
1. **L2 Cache Persistence**: Use `cudaAccessPolicyWindow` for database points
2. **Tensor Core**: Not applicable for this operation
3. **Green Contexts**: Enable for multi-stream
4. **Shared Memory Carve-out**: Request 64KB per SM

### Edge Cases
- Query norm near 1.0: Clamp denominator
- Database norm near 1.0: Clamp denominator
- Zero vector: Distance to origin is well-defined
- Same point: Distance should be 0.0

## Verification

### Test Commands
```bash
cd crates/context-graph-cuda
nvcc --version  # Verify CUDA toolkit
cargo build -p context-graph-cuda
cargo test -p context-graph-graph cuda_poincare
```

### Manual Verification
- [ ] Kernel launches without error
- [ ] Results match CPU reference
- [ ] Performance meets <1ms target
- [ ] No numerical overflow

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[requires_gpu]
    fn test_poincare_distance_batch_gpu() {
        // Generate test data
        let n_queries = 1000;
        let n_database = 1000;

        let queries: Vec<[f32; 64]> = (0..n_queries)
            .map(|_| random_poincare_point())
            .collect();
        let database: Vec<[f32; 64]> = (0..n_database)
            .map(|_| random_poincare_point())
            .collect();

        let config = PoincareCudaConfig::default();

        // Compute on GPU
        let start = std::time::Instant::now();
        let gpu_distances = poincare_distance_batch_host(&queries, &database, &config).unwrap();
        let gpu_time = start.elapsed();

        // Verify performance
        assert!(gpu_time.as_millis() < 1, "GPU took {}ms, expected <1ms", gpu_time.as_millis());

        // Compare with CPU reference
        let ball = PoincareBall::new(&HyperbolicConfig::default());
        for (i, q) in queries.iter().enumerate() {
            for (j, d) in database.iter().enumerate() {
                let cpu_dist = ball.distance(
                    &PoincarePoint::from_coords(q),
                    &PoincarePoint::from_coords(d)
                );
                let gpu_dist = gpu_distances[i * n_database + j];
                assert!((cpu_dist - gpu_dist).abs() < 1e-5,
                    "Mismatch at ({}, {}): CPU={}, GPU={}", i, j, cpu_dist, gpu_dist);
            }
        }
    }

    #[test]
    #[requires_gpu]
    fn test_poincare_distance_boundary() {
        // Test with points near boundary (norm close to 1)
        let near_boundary = |scale: f32| -> [f32; 64] {
            let mut p = [0.0f32; 64];
            let val = scale / (64.0f32).sqrt();
            for x in &mut p { *x = val; }
            p
        };

        let queries = vec![near_boundary(0.99)];
        let database = vec![near_boundary(0.98)];

        let config = PoincareCudaConfig::default();
        let result = poincare_distance_batch_host(&queries, &database, &config);

        assert!(result.is_ok());
        let dist = result.unwrap()[0];
        assert!(dist.is_finite());
        assert!(dist > 0.0);
    }

    fn random_poincare_point() -> [f32; 64] {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut p = [0.0f32; 64];

        // Generate point with norm < 0.9 (safely inside ball)
        loop {
            let mut norm_sq = 0.0f32;
            for x in &mut p {
                *x = rng.gen_range(-1.0..1.0);
                norm_sq += *x * *x;
            }
            if norm_sq < 0.81 {  // 0.9^2
                break;
            }
            // Rescale if needed
            let scale = 0.85 / norm_sq.sqrt();
            for x in &mut p {
                *x *= scale;
            }
            break;
        }
        p
    }
}
```
