---
id: "M04-T24"
title: "Implement Cone Membership CUDA Kernel"
description: |
  Implement cone_check_batch CUDA kernel for batch entailment cone membership.
  Input: cones[n_cones][65] (64 apex coords + 1 aperture), points[n_pts][64]
  Output: scores[n_cones][n_pts]
  Performance: <2ms for 1K x 1K membership matrix.
layer: "surface"
status: "pending"
priority: "high"
estimated_hours: 4
sequence: 32
depends_on:
  - "M04-T07"
spec_refs:
  - "TECH-GRAPH-004 Section 10.2"
files_to_create:
  - path: "crates/context-graph-cuda/kernels/cone_check.cu"
    description: "CUDA kernel for batch cone membership checking"
  - path: "crates/context-graph-cuda/src/cone.rs"
    description: "Rust FFI wrapper for cone CUDA kernel"
files_to_modify:
  - path: "crates/context-graph-cuda/src/lib.rs"
    description: "Add cone module"
  - path: "crates/context-graph-cuda/build.rs"
    description: "Add kernel compilation"
test_file: "crates/context-graph-graph/tests/cuda_tests.rs"
---

## Context

Entailment cone membership checking is the core operation for IS-A hierarchy queries. While individual containment checks are O(1) on CPU, batch operations for knowledge graph queries benefit from GPU acceleration. This kernel computes a membership score matrix where each element indicates how strongly a point is contained by a cone.

The CANONICAL membership score formula (from M04-T07):
- Compute angle between point direction and cone axis (origin direction)
- If angle <= effective_aperture: score = 1.0
- If angle > effective_aperture: score = exp(-2.0 * (angle - aperture))

## RTX 5090 Optimizations

- **Compute Capability 12.0**: Use latest CUDA features
- **Shared Memory**: Cache cone data for efficient access
- **Warp-Level Primitives**: Use `__shfl_down_sync` for dot product reduction
- **FP16**: Consider for angle computation where precision allows
- **Green Contexts**: Enable for multi-stream execution

## Scope

### In Scope
- CUDA kernel for batch cone membership
- Shared memory optimization for cone caching
- Soft membership score output [0, 1]
- Rust FFI wrapper
- Build system integration

### Out of Scope
- Boolean containment output (derived from score threshold)
- CPU fallback (exists in M04-T07)
- Multi-GPU support
- Cone training/updates on GPU

## Definition of Done

### Signatures

```cuda
// In crates/context-graph-cuda/kernels/cone_check.cu

#include <cuda_runtime.h>
#include <math.h>

// Kernel configuration
constexpr int BLOCK_DIM_X = 32;      // Threads per block (X dimension)
constexpr int BLOCK_DIM_Y = 8;       // Threads per block (Y dimension)
constexpr int POINT_DIM = 64;        // Poincare ball dimension
constexpr int CONE_DATA_DIM = 65;    // 64 apex coords + 1 aperture
constexpr int CONES_PER_BLOCK = 8;   // Cones cached in shared memory

// Small epsilon for numerical stability
constexpr float EPS = 1e-7f;

/**
 * Compute log map from apex to point (tangent vector)
 *
 * log_apex(point) returns tangent vector at apex pointing toward point
 */
__device__ void log_map(
    const float* apex,
    const float* point,
    float* tangent,
    float curvature
) {
    const float c = fabsf(curvature);

    // Compute ||apex||^2
    float apex_norm_sq = 0.0f;
    for (int d = 0; d < POINT_DIM; d++) {
        apex_norm_sq += apex[d] * apex[d];
    }

    // Compute Mobius subtraction: point (-) apex
    // gyro subtraction in Poincare ball
    float lambda_apex = 1.0f / fmaxf(1.0f - c * apex_norm_sq, EPS);

    // Simplified log map for tangent direction
    // (Full formula involves Mobius operations, this is approximation for direction)
    float diff[POINT_DIM];
    float diff_norm_sq = 0.0f;
    for (int d = 0; d < POINT_DIM; d++) {
        diff[d] = point[d] - apex[d];
        diff_norm_sq += diff[d] * diff[d];
    }

    float diff_norm = sqrtf(fmaxf(diff_norm_sq, EPS));

    for (int d = 0; d < POINT_DIM; d++) {
        tangent[d] = diff[d] / diff_norm;
    }
}

/**
 * Compute entailment cone membership score
 *
 * CANONICAL FORMULA:
 * - If angle <= aperture: score = 1.0
 * - If angle > aperture: score = exp(-2.0 * (angle - aperture))
 */
__device__ float cone_membership_score(
    const float* apex,
    float aperture,
    const float* point,
    float curvature
) {
    // Compute tangent from apex to point
    float tangent[POINT_DIM];
    log_map(apex, point, tangent, curvature);

    // Compute tangent from apex to origin (direction toward origin)
    float origin[POINT_DIM];
    for (int d = 0; d < POINT_DIM; d++) {
        origin[d] = 0.0f;
    }

    float to_origin[POINT_DIM];
    log_map(apex, origin, to_origin, curvature);

    // Compute angle between tangent vectors
    float dot = 0.0f;
    float tangent_norm = 0.0f;
    float origin_norm = 0.0f;

    for (int d = 0; d < POINT_DIM; d++) {
        dot += tangent[d] * to_origin[d];
        tangent_norm += tangent[d] * tangent[d];
        origin_norm += to_origin[d] * to_origin[d];
    }

    tangent_norm = sqrtf(fmaxf(tangent_norm, EPS));
    origin_norm = sqrtf(fmaxf(origin_norm, EPS));

    float cos_angle = dot / (tangent_norm * origin_norm);
    cos_angle = fmaxf(-1.0f, fminf(1.0f, cos_angle));  // Clamp to valid range

    float angle = acosf(cos_angle);

    // CANONICAL FORMULA for membership score
    if (angle <= aperture) {
        return 1.0f;
    } else {
        return expf(-2.0f * (angle - aperture));
    }
}

/**
 * Batch cone membership kernel
 *
 * @param cones      Cone data [n_cones][65] (64 apex + 1 aperture)
 * @param points     Point data [n_points][64]
 * @param scores     Output scores [n_cones][n_points]
 * @param n_cones    Number of cones
 * @param n_points   Number of points
 * @param curvature  Poincare ball curvature
 */
__global__ void cone_check_kernel(
    const float* __restrict__ cones,
    const float* __restrict__ points,
    float* __restrict__ scores,
    int n_cones,
    int n_points,
    float curvature
) {
    // Shared memory for cone data
    __shared__ float shared_cones[CONES_PER_BLOCK][CONE_DATA_DIM];

    const int tx = threadIdx.x;
    const int ty = threadIdx.y;
    const int bx = blockIdx.x;
    const int by = blockIdx.y;

    // Global indices
    const int cone_idx = by * CONES_PER_BLOCK + ty;
    const int point_idx = bx * BLOCK_DIM_X + tx;

    // Load cones into shared memory
    if (ty < CONES_PER_BLOCK && cone_idx < n_cones) {
        for (int d = tx; d < CONE_DATA_DIM; d += BLOCK_DIM_X) {
            shared_cones[ty][d] = cones[cone_idx * CONE_DATA_DIM + d];
        }
    }
    __syncthreads();

    // Compute membership score
    if (cone_idx < n_cones && point_idx < n_points) {
        // Extract apex and aperture from shared memory
        const float* apex = shared_cones[ty];
        float aperture = shared_cones[ty][POINT_DIM];  // Last element

        // Load point from global memory
        float point[POINT_DIM];
        for (int d = 0; d < POINT_DIM; d++) {
            point[d] = points[point_idx * POINT_DIM + d];
        }

        // Compute score
        float score = cone_membership_score(apex, aperture, point, curvature);

        // Write result
        scores[cone_idx * n_points + point_idx] = score;
    }
}

/**
 * Host function to launch cone check kernel
 */
extern "C" cudaError_t launch_cone_check(
    const float* d_cones,
    const float* d_points,
    float* d_scores,
    int n_cones,
    int n_points,
    float curvature,
    cudaStream_t stream
) {
    dim3 block(BLOCK_DIM_X, BLOCK_DIM_Y);
    dim3 grid(
        (n_points + BLOCK_DIM_X - 1) / BLOCK_DIM_X,
        (n_cones + CONES_PER_BLOCK - 1) / CONES_PER_BLOCK
    );

    cone_check_kernel<<<grid, block, 0, stream>>>(
        d_cones, d_points, d_scores,
        n_cones, n_points, curvature
    );

    return cudaGetLastError();
}

/**
 * Boolean containment check (threshold at 0.5)
 */
extern "C" cudaError_t launch_cone_contains(
    const float* d_cones,
    const float* d_points,
    bool* d_contains,
    int n_cones,
    int n_points,
    float curvature,
    float threshold,
    cudaStream_t stream
);  // Implemented with threshold comparison kernel
```

```rust
// In crates/context-graph-cuda/src/cone.rs

use std::ptr::NonNull;

/// FFI declarations for CUDA cone membership kernel
mod ffi {
    use std::os::raw::c_int;

    extern "C" {
        pub fn launch_cone_check(
            d_cones: *const f32,
            d_points: *const f32,
            d_scores: *mut f32,
            n_cones: c_int,
            n_points: c_int,
            curvature: f32,
            stream: *mut std::ffi::c_void,
        ) -> c_int;
    }
}

/// Error from CUDA cone kernel
#[derive(Debug, Clone)]
pub struct ConeCudaError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for ConeCudaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CUDA Cone error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for ConeCudaError {}

pub type ConeCudaResult<T> = Result<T, ConeCudaError>;

/// Cone data layout for CUDA kernel
/// [apex_0, apex_1, ..., apex_63, aperture]
pub const CONE_DATA_DIM: usize = 65;

/// Cone data for GPU operations
#[derive(Debug, Clone)]
pub struct ConeData {
    pub apex: [f32; 64],
    pub aperture: f32,
}

impl ConeData {
    /// Pack cone data into flat array for GPU
    pub fn to_gpu_format(&self) -> [f32; CONE_DATA_DIM] {
        let mut data = [0.0f32; CONE_DATA_DIM];
        data[..64].copy_from_slice(&self.apex);
        data[64] = self.aperture;
        data
    }

    /// Unpack from GPU format
    pub fn from_gpu_format(data: &[f32; CONE_DATA_DIM]) -> Self {
        let mut apex = [0.0f32; 64];
        apex.copy_from_slice(&data[..64]);
        Self {
            apex,
            aperture: data[64],
        }
    }
}

/// Configuration for cone CUDA operations
#[derive(Debug, Clone)]
pub struct ConeCudaConfig {
    pub curvature: f32,
}

impl Default for ConeCudaConfig {
    fn default() -> Self {
        Self { curvature: -1.0 }
    }
}

/// Compute batch cone membership scores on GPU
///
/// # Arguments
/// * `cones` - Cone data device pointer [n_cones][65]
/// * `points` - Point data device pointer [n_points][64]
/// * `scores` - Output device pointer [n_cones][n_points]
/// * `n_cones` - Number of cones
/// * `n_points` - Number of points
/// * `config` - Cone configuration
/// * `stream` - CUDA stream
///
/// # Safety
/// All pointers must be valid device pointers.
pub unsafe fn cone_check_batch(
    cones: NonNull<f32>,
    points: NonNull<f32>,
    scores: NonNull<f32>,
    n_cones: usize,
    n_points: usize,
    config: &ConeCudaConfig,
    stream: Option<NonNull<std::ffi::c_void>>,
) -> ConeCudaResult<()> {
    let stream_ptr = stream
        .map(|s| s.as_ptr())
        .unwrap_or(std::ptr::null_mut());

    let result = ffi::launch_cone_check(
        cones.as_ptr(),
        points.as_ptr(),
        scores.as_ptr() as *mut f32,
        n_cones as i32,
        n_points as i32,
        config.curvature,
        stream_ptr,
    );

    if result != 0 {
        return Err(ConeCudaError {
            code: result,
            message: format!("Cone kernel launch failed with code {}", result),
        });
    }

    Ok(())
}

/// Host convenience wrapper
pub fn cone_check_batch_host(
    cones: &[ConeData],
    points: &[[f32; 64]],
    config: &ConeCudaConfig,
) -> ConeCudaResult<Vec<f32>> {
    // Requires GPU memory allocation
    unimplemented!("Requires GPU memory allocation - see GpuMemoryManager")
}

/// Check if point is contained by cone (threshold membership score)
pub fn is_contained_batch_host(
    cones: &[ConeData],
    points: &[[f32; 64]],
    threshold: f32,
    config: &ConeCudaConfig,
) -> ConeCudaResult<Vec<bool>> {
    let scores = cone_check_batch_host(cones, points, config)?;
    Ok(scores.iter().map(|&s| s >= threshold).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cone_data_packing() {
        let cone = ConeData {
            apex: [0.1; 64],
            aperture: 0.5,
        };

        let packed = cone.to_gpu_format();
        assert_eq!(packed.len(), CONE_DATA_DIM);
        assert!((packed[0] - 0.1).abs() < 1e-6);
        assert!((packed[64] - 0.5).abs() < 1e-6);

        let unpacked = ConeData::from_gpu_format(&packed);
        assert!((unpacked.apex[0] - 0.1).abs() < 1e-6);
        assert!((unpacked.aperture - 0.5).abs() < 1e-6);
    }
}
```

### Constraints
- Cone data format: [apex_0..apex_63, aperture] = 65 floats
- Point data format: [coord_0..coord_63] = 64 floats
- Performance target: <2ms for 1K x 1K membership matrix
- Uses CANONICAL membership score formula
- Score output in [0, 1]

### Acceptance Criteria
- [ ] CUDA kernel compiles with nvcc
- [ ] Shared memory used for cone data
- [ ] Matches CPU implementation within 1e-5 tolerance
- [ ] Performance: <2ms for 1K x 1K
- [ ] Returns soft membership score [0,1]
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```cuda
// Thread block: 32 x 8 = 256 threads
// Each block processes 8 cones x 32 points

shared float cones[8][65];  // 2.08KB shared memory

// Phase 1: Load cones to shared memory
for d in tx..65 step 32:
    shared_cones[ty][d] = cones[cone_idx * 65 + d]
sync()

// Phase 2: Compute membership for (cone_idx, point_idx)
apex = shared_cones[ty][0..64]
aperture = shared_cones[ty][64]
point[64] = load from global memory

// Compute tangent from apex to point
tangent = log_map(apex, point)

// Compute tangent from apex to origin
to_origin = log_map(apex, origin)

// Angle between tangent vectors
cos_angle = dot(tangent, to_origin) / (||tangent|| * ||to_origin||)
angle = arccos(cos_angle)

// CANONICAL FORMULA
if angle <= aperture:
    score = 1.0
else:
    score = exp(-2.0 * (angle - aperture))

scores[cone_idx * n_points + point_idx] = score
```

### Membership Score Visualization
```
score
  1.0 |--------\
      |         \
  0.5 |          \
      |           \______________
  0.0 |
      +-----------------------------> angle
           aperture
```

### Edge Cases
- Point at apex: Angle undefined, return 1.0
- Apex at origin: to_origin is zero vector, handle specially
- Very narrow cone (aperture ~0): Only apex gets 1.0
- Very wide cone (aperture ~pi/2): Most points contained

## Verification

### Test Commands
```bash
cd crates/context-graph-cuda
nvcc --version
cargo build -p context-graph-cuda
cargo test -p context-graph-graph cuda_cone
```

### Manual Verification
- [ ] Kernel launches without error
- [ ] Results match CPU reference
- [ ] Performance meets <2ms target
- [ ] Score gradients are smooth

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[requires_gpu]
    fn test_cone_check_batch_gpu() {
        let n_cones = 1000;
        let n_points = 1000;

        // Generate test data
        let cones: Vec<ConeData> = (0..n_cones)
            .map(|_| random_cone())
            .collect();
        let points: Vec<[f32; 64]> = (0..n_points)
            .map(|_| random_poincare_point())
            .collect();

        let config = ConeCudaConfig::default();

        // Compute on GPU
        let start = std::time::Instant::now();
        let gpu_scores = cone_check_batch_host(&cones, &points, &config).unwrap();
        let gpu_time = start.elapsed();

        // Verify performance
        assert!(gpu_time.as_millis() < 2, "GPU took {}ms, expected <2ms", gpu_time.as_millis());

        // Compare with CPU reference
        for (i, cone) in cones.iter().enumerate() {
            let cpu_cone = EntailmentCone::new(
                PoincarePoint::from_coords(&cone.apex),
                cone.aperture,
                1.0,
                0,
            );
            let ball = PoincareBall::new(&HyperbolicConfig::default());

            for (j, point) in points.iter().enumerate() {
                let cpu_point = PoincarePoint::from_coords(point);
                let cpu_score = cpu_cone.membership_score(&cpu_point, &ball);
                let gpu_score = gpu_scores[i * n_points + j];

                assert!((cpu_score - gpu_score).abs() < 1e-5,
                    "Mismatch at ({}, {}): CPU={}, GPU={}", i, j, cpu_score, gpu_score);
            }
        }
    }

    #[test]
    #[requires_gpu]
    fn test_cone_membership_canonical_formula() {
        // Test that GPU implements canonical formula correctly
        let cone = ConeData {
            apex: [0.0; 64],  // Origin
            aperture: 0.5,   // ~29 degrees
        };

        // Point inside cone (should be 1.0)
        let point_inside = make_point_at_angle(0.3);

        // Point outside cone (should be exp(-2 * (angle - 0.5)))
        let point_outside = make_point_at_angle(0.7);

        let config = ConeCudaConfig::default();
        let scores = cone_check_batch_host(&[cone], &[point_inside, point_outside], &config).unwrap();

        // Inside cone
        assert!((scores[0] - 1.0).abs() < 1e-5);

        // Outside cone: exp(-2 * (0.7 - 0.5)) = exp(-0.4) = 0.67
        let expected = (-2.0f32 * 0.2).exp();
        assert!((scores[1] - expected).abs() < 0.1);  // Approximate due to angle computation
    }

    fn random_cone() -> ConeData {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let apex = random_poincare_point();
        let aperture = rng.gen_range(0.3..1.2);  // Reasonable range

        ConeData { apex, aperture }
    }

    fn random_poincare_point() -> [f32; 64] {
        // Same as in poincare tests
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut p = [0.0f32; 64];

        loop {
            let mut norm_sq = 0.0f32;
            for x in &mut p {
                *x = rng.gen_range(-1.0..1.0);
                norm_sq += *x * *x;
            }
            if norm_sq < 0.81 {
                break;
            }
            let scale = 0.85 / norm_sq.sqrt();
            for x in &mut p { *x *= scale; }
            break;
        }
        p
    }

    fn make_point_at_angle(angle: f32) -> [f32; 64] {
        // Create point at specified angle from origin direction
        let mut p = [0.0f32; 64];
        p[0] = 0.5 * angle.cos();
        p[1] = 0.5 * angle.sin();
        p
    }
}
```
