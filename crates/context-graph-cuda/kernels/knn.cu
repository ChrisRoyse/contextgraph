//! GPU k-NN kernel using brute-force L2 distance computation.
//!
//! This kernel computes all-pairs L2 distances and finds k nearest neighbors
//! for each query point. Used for HDBSCAN core distance computation.
//!
//! # Constitution Compliance
//!
//! - ARCH-GPU-05: k-NN runs on GPU
//! - RTX 5090 (SM 12.0) optimized
//!
//! # Driver API
//!
//! This kernel is compiled to PTX and loaded via CUDA Driver API.
//! No cudart dependency to avoid WSL2 static initialization bugs.

#include <float.h>
#include <stdint.h>

// Block size for kernels - 256 is optimal for RTX 5090
#define BLOCK_SIZE 256

//! Compute L2 squared distance between two vectors.
__device__ __forceinline__ float l2_distance_squared(
    const float* __restrict__ a,
    const float* __restrict__ b,
    int dimension
) {
    float sum = 0.0f;
    for (int d = 0; d < dimension; d++) {
        float diff = a[d] - b[d];
        sum += diff * diff;
    }
    return sum;
}

//! Kernel: Compute core distances for all points.
//! Each thread handles one point.
//!
//! # Arguments
//! - vectors: Device pointer to input vectors (n_points * dimension floats)
//! - n_points: Number of points
//! - dimension: Vector dimension
//! - k: Number of neighbors for core distance
//! - core_dists: Device pointer to output core distances (n_points floats)
extern "C" __global__ void compute_core_distances_kernel(
    const float* __restrict__ vectors,
    int n_points,
    int dimension,
    int k,
    float* __restrict__ core_dists
) {
    int tid = blockIdx.x * blockDim.x + threadIdx.x;
    if (tid >= n_points) return;

    const float* query = vectors + tid * dimension;

    // For small k, use a simple O(n*k) selection
    // Keep track of top-(k+1) smallest distances (including self)
    int k_search = k + 1;
    if (k_search > n_points) k_search = n_points;

    // Use local registers for top-k tracking (max k=16 for register pressure)
    float top_k[16];
    int actual_k = k_search;
    if (actual_k > 16) actual_k = 16;

    // Initialize with max values
    #pragma unroll
    for (int i = 0; i < 16; i++) {
        top_k[i] = FLT_MAX;
    }

    // Stream through all points
    for (int j = 0; j < n_points; j++) {
        const float* target = vectors + j * dimension;
        float dist = l2_distance_squared(query, target, dimension);

        // Insert into sorted top-k if smaller
        if (dist < top_k[actual_k - 1]) {
            int pos = actual_k - 1;
            while (pos > 0 && dist < top_k[pos - 1]) {
                top_k[pos] = top_k[pos - 1];
                pos--;
            }
            top_k[pos] = dist;
        }
    }

    // k-th neighbor distance (index k gives k-th non-self, since index 0 is self with dist=0)
    int kth_idx = k;
    if (kth_idx >= actual_k) kth_idx = actual_k - 1;
    float kth_dist = top_k[kth_idx];
    core_dists[tid] = sqrtf(kth_dist);
}

//! Kernel: Compute pairwise L2 distances.
//! Each thread handles one pair (i, j) where i < j.
//!
//! # Arguments
//! - vectors: Device pointer to input vectors (n_points * dimension floats)
//! - n_points: Number of points
//! - dimension: Vector dimension
//! - distances: Device pointer to output distances (n*(n-1)/2 floats)
extern "C" __global__ void compute_pairwise_distances_kernel(
    const float* __restrict__ vectors,
    int n_points,
    int dimension,
    float* __restrict__ distances
) {
    int64_t tid = (int64_t)blockIdx.x * blockDim.x + threadIdx.x;
    int64_t total_pairs = (int64_t)n_points * (n_points - 1) / 2;

    if (tid >= total_pairs) return;

    // Convert linear index to (i, j) pair where i < j
    // Using: tid = j*(j-1)/2 + i
    int64_t j = (int64_t)(0.5 + sqrtf(0.25f + 2.0f * (float)tid));
    int64_t i = tid - j * (j - 1) / 2;

    // Bounds check
    if (j >= n_points || i >= j) {
        j++;
        i = tid - j * (j - 1) / 2;
    }

    if (i >= 0 && i < j && j < n_points) {
        const float* vec_i = vectors + i * dimension;
        const float* vec_j = vectors + j * dimension;
        distances[tid] = sqrtf(l2_distance_squared(vec_i, vec_j, dimension));
    }
}
