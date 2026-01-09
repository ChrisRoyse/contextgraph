//! FAISS GPU Index operations: train, search, add.
//!
//! Provides the core operations for the GPU index.

use std::os::raw::{c_float, c_long};

use super::super::faiss_ffi::{
    check_faiss_result, faiss_IndexIVF_set_nprobe, faiss_Index_add_with_ids, faiss_Index_search,
    faiss_Index_train,
};
use super::index::FaissGpuIndex;
use crate::error::{GraphError, GraphResult};

impl FaissGpuIndex {
    /// Train the index with representative vectors.
    ///
    /// IVF-PQ requires training to establish cluster centroids and PQ codebooks.
    /// Training vectors should be representative of the data distribution.
    ///
    /// # Arguments
    ///
    /// * `vectors` - Training vectors (flattened, row-major: n_vectors * dimension f32 values)
    ///
    /// # Errors
    ///
    /// - `GraphError::InsufficientTrainingData` if n_vectors < min_train_vectors (4M)
    /// - `GraphError::DimensionMismatch` if vectors.len() is not a multiple of dimension
    /// - `GraphError::FaissTrainingFailed` on FAISS training error
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use context_graph_graph::index::gpu_index::FaissGpuIndex;
    /// # use context_graph_graph::config::IndexConfig;
    /// # fn example() -> context_graph_graph::error::GraphResult<()> {
    /// let config = IndexConfig::default();
    /// let mut index = FaissGpuIndex::new(config)?;
    ///
    /// // Generate training data (4M+ vectors required)
    /// let training_data: Vec<f32> = generate_training_vectors();
    /// index.train(&training_data)?;
    /// # Ok(())
    /// # }
    /// # fn generate_training_vectors() -> Vec<f32> { vec![] }
    /// ```
    pub fn train(&mut self, vectors: &[f32]) -> GraphResult<()> {
        let remainder = vectors.len() % self.config.dimension;
        if remainder != 0 {
            return Err(GraphError::DimensionMismatch {
                expected: self.config.dimension,
                actual: remainder,
            });
        }

        let n_vectors = vectors.len() / self.config.dimension;

        if n_vectors < self.config.min_train_vectors {
            return Err(GraphError::InsufficientTrainingData {
                required: self.config.min_train_vectors,
                provided: n_vectors,
            });
        }

        // SAFETY: vectors slice contains n_vectors * dimension valid f32 values.
        // index_ptr is valid and points to a FAISS index.
        let ret = unsafe {
            faiss_Index_train(
                self.index_ptr.as_ptr(),
                n_vectors as c_long,
                vectors.as_ptr() as *const c_float,
            )
        };

        check_faiss_result(ret, "faiss_Index_train").map_err(|e| {
            GraphError::FaissTrainingFailed(format!(
                "Training failed with {} vectors: {}",
                n_vectors, e
            ))
        })?;

        // Set nprobe after successful training
        // SAFETY: index_ptr is valid, nprobe value is valid.
        // Note: faiss_IndexIVF_set_nprobe returns void (no error code).
        unsafe {
            faiss_IndexIVF_set_nprobe(self.index_ptr.as_ptr(), self.config.nprobe);
        }

        self.is_trained = true;
        Ok(())
    }

    /// Search for k nearest neighbors.
    ///
    /// # Arguments
    ///
    /// * `queries` - Query vectors (flattened, row-major: n_queries * dimension f32 values)
    /// * `k` - Number of neighbors to return per query
    ///
    /// # Errors
    ///
    /// - `GraphError::IndexNotTrained` if index is not trained
    /// - `GraphError::DimensionMismatch` if queries.len() is not a multiple of dimension
    /// - `GraphError::FaissSearchFailed` on FAISS search error
    ///
    /// # Returns
    ///
    /// Tuple of (distances, indices) where each has length n_queries * k.
    /// Distances are L2 squared distances. Indices are -1 for unfilled slots.
    ///
    /// # Performance
    ///
    /// Target: <2ms for 1M vectors with k=100, <5ms for 10M vectors with k=10
    pub fn search(&self, queries: &[f32], k: usize) -> GraphResult<(Vec<f32>, Vec<i64>)> {
        if !self.is_trained {
            return Err(GraphError::IndexNotTrained);
        }

        let remainder = queries.len() % self.config.dimension;
        if remainder != 0 {
            return Err(GraphError::DimensionMismatch {
                expected: self.config.dimension,
                actual: remainder,
            });
        }

        let n_queries = queries.len() / self.config.dimension;
        let result_size = n_queries * k;

        let mut distances: Vec<f32> = vec![f32::MAX; result_size];
        let mut indices: Vec<i64> = vec![-1; result_size];

        // SAFETY: queries slice contains n_queries * dimension valid f32 values.
        // distances and indices are sized correctly for n_queries * k elements.
        // index_ptr is valid and points to a trained FAISS index.
        let ret = unsafe {
            faiss_Index_search(
                self.index_ptr.as_ptr(),
                n_queries as c_long,
                queries.as_ptr() as *const c_float,
                k as c_long,
                distances.as_mut_ptr() as *mut c_float,
                indices.as_mut_ptr() as *mut c_long,
            )
        };

        check_faiss_result(ret, "faiss_Index_search").map_err(|e| {
            GraphError::FaissSearchFailed(format!(
                "Search failed for {} queries, k={}: {}",
                n_queries, k, e
            ))
        })?;

        Ok((distances, indices))
    }

    /// Add vectors with IDs to the index.
    ///
    /// # Arguments
    ///
    /// * `vectors` - Vectors to add (flattened, row-major: n_vectors * dimension f32 values)
    /// * `ids` - Vector IDs (one per vector, must match n_vectors)
    ///
    /// # Errors
    ///
    /// - `GraphError::IndexNotTrained` if index is not trained
    /// - `GraphError::DimensionMismatch` if vectors.len() is not a multiple of dimension
    /// - `GraphError::InvalidConfig` if vector count doesn't match ID count
    /// - `GraphError::FaissAddFailed` on FAISS add error
    ///
    /// # Note
    ///
    /// Index must be trained before adding vectors.
    pub fn add_with_ids(&mut self, vectors: &[f32], ids: &[i64]) -> GraphResult<()> {
        if !self.is_trained {
            return Err(GraphError::IndexNotTrained);
        }

        let remainder = vectors.len() % self.config.dimension;
        if remainder != 0 {
            return Err(GraphError::DimensionMismatch {
                expected: self.config.dimension,
                actual: remainder,
            });
        }

        let n_vectors = vectors.len() / self.config.dimension;

        if n_vectors != ids.len() {
            return Err(GraphError::InvalidConfig(format!(
                "Vector count ({}) doesn't match ID count ({})",
                n_vectors,
                ids.len()
            )));
        }

        // SAFETY: vectors slice contains n_vectors * dimension valid f32 values.
        // ids slice contains n_vectors valid i64 values.
        // index_ptr is valid and points to a trained FAISS index.
        let ret = unsafe {
            faiss_Index_add_with_ids(
                self.index_ptr.as_ptr(),
                n_vectors as c_long,
                vectors.as_ptr() as *const c_float,
                ids.as_ptr() as *const c_long,
            )
        };

        check_faiss_result(ret, "faiss_Index_add_with_ids").map_err(|e| {
            GraphError::FaissAddFailed(format!("Failed to add {} vectors: {}", n_vectors, e))
        })?;

        self.vector_count += n_vectors;
        Ok(())
    }
}
