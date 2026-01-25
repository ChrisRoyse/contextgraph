//! GPU-accelerated k-NN using custom CUDA kernel.
//!
//! Provides efficient batch k-NN computation on GPU for HDBSCAN core distance.
//! Uses Driver API to avoid WSL2 cudart static initialization bugs.

use std::time::Instant;

use tracing::{debug, info, instrument};

use crate::ffi::knn::{compute_core_distances_gpu, cuda_available};

use super::error::{GpuHdbscanError, GpuHdbscanResult};

/// GPU k-NN index for HDBSCAN core distance computation.
///
/// Uses custom CUDA kernel for brute-force exact k-NN.
/// No training required.
///
/// # Constitution Compliance
///
/// - ARCH-GPU-05: k-NN runs on GPU
/// - ARCH-GPU-06: Batch operations preferred
pub struct GpuKnnIndex {
    /// Stored vectors (flattened)
    vectors: Vec<f32>,
    /// Vector dimension
    dimension: usize,
    /// Number of vectors in index
    vector_count: usize,
}

impl GpuKnnIndex {
    /// Create a new GPU k-NN index.
    ///
    /// # Arguments
    ///
    /// * `dimension` - Vector dimension (must be > 0)
    ///
    /// # Errors
    ///
    /// - `GpuNotAvailable` if no GPU detected
    #[instrument(skip_all, fields(dimension))]
    pub fn new(dimension: usize) -> GpuHdbscanResult<Self> {
        // Validate dimension
        if dimension == 0 {
            return Err(GpuHdbscanError::invalid_dimension(dimension));
        }

        // Check GPU availability - no fallback
        if !cuda_available() {
            return Err(GpuHdbscanError::gpu_not_available(
                "CUDA GPU not available. Check CUDA installation and nvidia-smi.",
            ));
        }

        debug!(dimension, "Creating GPU k-NN index");

        Ok(Self {
            vectors: Vec::new(),
            dimension,
            vector_count: 0,
        })
    }

    /// Add vectors to the index.
    ///
    /// # Arguments
    ///
    /// * `vectors` - Vectors to add, each must be `dimension` elements
    ///
    /// # Errors
    ///
    /// - `DimensionMismatch` if any vector has wrong dimension
    /// - `NonFiniteValue` if any value is NaN or Infinity
    #[instrument(skip_all, fields(n_vectors = vectors.len()))]
    pub fn add(&mut self, vectors: &[Vec<f32>]) -> GpuHdbscanResult<()> {
        if vectors.is_empty() {
            return Ok(());
        }

        let n = vectors.len();
        debug!(n_vectors = n, dimension = self.dimension, "Adding vectors to index");

        // Validate dimensions and values
        for (i, vec) in vectors.iter().enumerate() {
            if vec.len() != self.dimension {
                return Err(GpuHdbscanError::InvalidParameter {
                    parameter: format!("vectors[{}].len()", i),
                    value: vec.len().to_string(),
                    requirement: format!("must equal dimension {}", self.dimension),
                });
            }

            for (j, &val) in vec.iter().enumerate() {
                if !val.is_finite() {
                    return Err(GpuHdbscanError::non_finite_value(i * self.dimension + j, val));
                }
            }
        }

        // Flatten vectors (row-major)
        for vec in vectors {
            self.vectors.extend(vec);
        }
        self.vector_count += n;

        debug!(total_vectors = self.vector_count, "Vectors added to index");
        Ok(())
    }

    /// Compute core distances for all vectors using batch k-NN.
    ///
    /// Core distance = distance to k-th nearest neighbor.
    /// This is the GPU-accelerated bottleneck of HDBSCAN.
    ///
    /// # Arguments
    ///
    /// * `k` - Number of neighbors (typically min_samples)
    ///
    /// # Returns
    ///
    /// Vector of core distances, one per vector in index.
    /// Core distance is the L2 distance to the k-th nearest neighbor.
    ///
    /// # Errors
    ///
    /// - `InsufficientData` if index has fewer than k+1 vectors
    /// - `CudaError` if GPU computation fails
    #[instrument(skip_all, fields(k, n_vectors = self.vector_count))]
    pub fn compute_core_distances(&self, k: usize) -> GpuHdbscanResult<Vec<f32>> {
        if self.vector_count == 0 {
            return Ok(vec![]);
        }

        // Need k+1 neighbors (including self) to get k-th neighbor distance
        let k_search = k.min(self.vector_count - 1) + 1;

        if self.vector_count < k_search {
            return Err(GpuHdbscanError::insufficient_data(k_search, self.vector_count));
        }

        let start = Instant::now();
        info!(
            k,
            k_search,
            n_vectors = self.vector_count,
            "Computing core distances on GPU"
        );

        // Run GPU k-NN kernel
        let core_distances = compute_core_distances_gpu(
            &self.vectors,
            self.vector_count,
            self.dimension,
            k,
        )
        .map_err(|e| GpuHdbscanError::internal("compute_core_distances_gpu", e.to_string()))?;

        let elapsed = start.elapsed();
        info!(
            elapsed_us = elapsed.as_micros(),
            avg_core_dist = core_distances.iter().sum::<f32>() / self.vector_count as f32,
            "Core distances computed on GPU"
        );

        Ok(core_distances)
    }

    /// Compute core distances with explicit vectors.
    ///
    /// # Arguments
    ///
    /// * `vectors` - The vectors (must match what was added)
    /// * `k` - Number of neighbors (typically min_samples)
    ///
    /// # Returns
    ///
    /// Vector of core distances (L2 distances to k-th neighbor).
    #[instrument(skip_all, fields(k, n_vectors = vectors.len()))]
    pub fn compute_core_distances_with_vectors(
        &self,
        vectors: &[Vec<f32>],
        k: usize,
    ) -> GpuHdbscanResult<Vec<f32>> {
        let n = vectors.len();
        if n == 0 {
            return Ok(vec![]);
        }

        if n != self.vector_count {
            return Err(GpuHdbscanError::dimension_mismatch(self.vector_count, n));
        }

        // Just delegate to compute_core_distances since we have the vectors stored
        self.compute_core_distances(k)
    }

    /// Get the number of vectors in the index.
    pub fn len(&self) -> usize {
        self.vector_count
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.vector_count == 0
    }

    /// Get the vector dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

// SAFETY: GpuKnnIndex owns its data exclusively
unsafe impl Send for GpuKnnIndex {}
