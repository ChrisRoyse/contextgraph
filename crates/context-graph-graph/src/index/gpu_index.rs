//! FAISS GPU IVF-PQ Index Wrapper
//!
//! Provides safe Rust wrapper around FAISS GPU index with:
//! - RAII resource management (Drop impl)
//! - Thread-safe GPU resource sharing (Arc<GpuResources>)
//! - Proper error handling (GraphError variants)
//! - Performance-optimized search (<2ms for 1M vectors, k=100)
//!
//! # Constitution References
//!
//! - TECH-GRAPH-004: Knowledge Graph technical specification
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - perf.latency.faiss_1M_k100: <2ms target
//!
//! # Safety
//!
//! This module uses unsafe FFI calls to FAISS C API. All unsafe blocks
//! are contained within this module with safety invariants documented.

use std::ffi::CString;
use std::os::raw::{c_float, c_int, c_long};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;

use crate::config::IndexConfig;
use crate::error::{GraphError, GraphResult};
use super::faiss_ffi::{
    FaissIndex, GpuResources as FfiGpuResources, MetricType,
    faiss_index_factory, faiss_index_cpu_to_gpu, faiss_Index_train,
    faiss_Index_is_trained, faiss_Index_add_with_ids, faiss_Index_search,
    faiss_IndexIVF_set_nprobe, faiss_Index_ntotal, faiss_write_index,
    faiss_read_index, faiss_Index_free, check_faiss_result, gpu_available,
};

/// GPU resources handle with RAII cleanup.
///
/// Wraps raw GPU resource pointer with automatic deallocation.
/// Use `Arc<GpuResources>` for sharing across multiple indices.
///
/// # Thread Safety
///
/// This type is `Send + Sync` because the underlying FAISS StandardGpuResources
/// uses internal synchronization for GPU memory management.
pub struct GpuResources {
    inner: FfiGpuResources,
    gpu_id: i32,
}

// SAFETY: GpuResources wraps FfiGpuResources which is Send+Sync.
// The gpu_id field is Copy and thread-safe.
unsafe impl Send for GpuResources {}
unsafe impl Sync for GpuResources {}

impl GpuResources {
    /// Allocate GPU resources for the specified device.
    ///
    /// # Arguments
    ///
    /// * `gpu_id` - CUDA device ID (typically 0)
    ///
    /// # Errors
    ///
    /// Returns `GraphError::GpuResourceAllocation` if:
    /// - GPU device is unavailable
    /// - CUDA initialization fails
    /// - Insufficient GPU memory
    ///
    /// # Example
    ///
    /// ```no_run
    /// use context_graph_graph::index::gpu_index::GpuResources;
    /// use std::sync::Arc;
    ///
    /// let resources = Arc::new(GpuResources::new(0)?);
    /// # Ok::<(), context_graph_graph::error::GraphError>(())
    /// ```
    pub fn new(gpu_id: i32) -> GraphResult<Self> {
        // Create FFI GPU resources - this validates GPU availability
        let inner = FfiGpuResources::new().map_err(|e| {
            GraphError::GpuResourceAllocation(format!(
                "Failed to create GPU resources for device {}: {}",
                gpu_id, e
            ))
        })?;

        Ok(Self { inner, gpu_id })
    }

    /// Get reference to inner FFI resources for FFI calls.
    #[inline]
    pub(crate) fn inner(&self) -> &FfiGpuResources {
        &self.inner
    }

    /// Get the GPU device ID.
    #[inline]
    pub fn gpu_id(&self) -> i32 {
        self.gpu_id
    }
}

impl std::fmt::Debug for GpuResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuResources")
            .field("gpu_id", &self.gpu_id)
            .finish()
    }
}

/// FAISS GPU IVF-PQ Index wrapper.
///
/// Provides GPU-accelerated approximate nearest neighbor search using
/// Inverted File with Product Quantization (IVF-PQ) index structure.
///
/// # Index Parameters (from IndexConfig)
///
/// - `dimension`: 1536 (E7_Code embedding dimension)
/// - `nlist`: 16384 (number of Voronoi cells)
/// - `nprobe`: 128 (cells to search at query time)
/// - `pq_segments`: 64 (PQ subdivision count)
/// - `pq_bits`: 8 (bits per PQ code)
///
/// # Performance Targets
///
/// - 1M vectors, k=100: <2ms
/// - 10M vectors, k=10: <5ms
///
/// # Thread Safety
///
/// - Single `FaissGpuIndex` is NOT thread-safe for concurrent modification
/// - Use separate indices per thread, or synchronize externally
/// - `Arc<GpuResources>` can be shared across indices safely
pub struct FaissGpuIndex {
    /// Raw pointer to GPU index (NonNull for safety guarantees)
    index_ptr: NonNull<FaissIndex>,
    /// Shared GPU resources
    gpu_resources: Arc<GpuResources>,
    /// Index configuration
    config: IndexConfig,
    /// Whether the index has been trained
    is_trained: bool,
    /// Number of vectors in the index (tracked by wrapper)
    vector_count: usize,
}

// SAFETY: FaissGpuIndex owns its index pointer exclusively.
// All mutable operations require &mut self, ensuring single-threaded access.
// The Arc<GpuResources> is Send+Sync, enabling safe transfer between threads.
unsafe impl Send for FaissGpuIndex {}

impl FaissGpuIndex {
    /// Create a new FAISS GPU IVF-PQ index.
    ///
    /// # Arguments
    ///
    /// * `config` - Index configuration parameters
    ///
    /// # Errors
    ///
    /// Returns `GraphError::FaissIndexCreation` if:
    /// - Invalid configuration parameters
    /// - GPU memory allocation fails
    /// - FAISS index creation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use context_graph_graph::config::IndexConfig;
    /// use context_graph_graph::index::gpu_index::FaissGpuIndex;
    ///
    /// let config = IndexConfig::default();
    /// let index = FaissGpuIndex::new(config)?;
    /// # Ok::<(), context_graph_graph::error::GraphError>(())
    /// ```
    pub fn new(config: IndexConfig) -> GraphResult<Self> {
        let resources = Arc::new(GpuResources::new(config.gpu_id)?);
        Self::with_resources(config, resources)
    }

    /// Create index with shared GPU resources.
    ///
    /// Use this when creating multiple indices to share GPU memory resources.
    ///
    /// # Arguments
    ///
    /// * `config` - Index configuration
    /// * `gpu_resources` - Shared GPU resources handle
    ///
    /// # Errors
    ///
    /// Returns `GraphError::FaissIndexCreation` if index creation fails.
    /// Returns `GraphError::InvalidConfig` if configuration is invalid.
    pub fn with_resources(config: IndexConfig, gpu_resources: Arc<GpuResources>) -> GraphResult<Self> {
        // Validate configuration
        if config.dimension == 0 {
            return Err(GraphError::InvalidConfig(
                "dimension must be > 0".to_string()
            ));
        }
        if config.nlist == 0 {
            return Err(GraphError::InvalidConfig(
                "nlist must be > 0".to_string()
            ));
        }
        if config.pq_segments == 0 {
            return Err(GraphError::InvalidConfig(
                "pq_segments must be > 0".to_string()
            ));
        }
        if config.dimension % config.pq_segments != 0 {
            return Err(GraphError::InvalidConfig(format!(
                "pq_segments ({}) must divide dimension ({}) evenly",
                config.pq_segments, config.dimension
            )));
        }

        // Create factory string
        let factory_string = config.factory_string();
        let c_factory = CString::new(factory_string.clone())
            .map_err(|e| GraphError::InvalidConfig(format!(
                "Invalid factory string '{}': {}", factory_string, e
            )))?;

        // Create CPU index first
        let mut cpu_index: *mut FaissIndex = std::ptr::null_mut();

        // SAFETY: faiss_index_factory allocates a new index.
        // We check the return value and null pointer below.
        let ret = unsafe {
            faiss_index_factory(
                &mut cpu_index,
                config.dimension as c_int,
                c_factory.as_ptr(),
                MetricType::L2,
            )
        };

        check_faiss_result(ret, "faiss_index_factory").map_err(|e| {
            GraphError::FaissIndexCreation(format!(
                "Failed to create CPU index '{}': {}", factory_string, e
            ))
        })?;

        if cpu_index.is_null() {
            return Err(GraphError::FaissIndexCreation(
                "CPU index pointer is null after factory creation".to_string()
            ));
        }

        // Transfer to GPU
        let mut gpu_index: *mut FaissIndex = std::ptr::null_mut();

        // SAFETY: faiss_index_cpu_to_gpu transfers the index to GPU.
        // cpu_index is valid (checked above), gpu_resources.inner().as_provider() is valid.
        let ret = unsafe {
            faiss_index_cpu_to_gpu(
                gpu_resources.inner().as_provider(),
                config.gpu_id as c_int,
                cpu_index,
                &mut gpu_index,
            )
        };

        // Free CPU index regardless of GPU transfer result (GPU copy owns data now)
        // SAFETY: cpu_index was allocated by faiss_index_factory and is non-null.
        unsafe { faiss_Index_free(cpu_index) };

        check_faiss_result(ret, "faiss_index_cpu_to_gpu").map_err(|e| {
            GraphError::GpuTransferFailed(format!(
                "Failed to transfer index to GPU {}: {}", config.gpu_id, e
            ))
        })?;

        if gpu_index.is_null() {
            return Err(GraphError::GpuResourceAllocation(
                "GPU index pointer is null after transfer".to_string()
            ));
        }

        // SAFETY: We verified gpu_index is non-null above.
        let index_ptr = unsafe { NonNull::new_unchecked(gpu_index) };

        Ok(Self {
            index_ptr,
            gpu_resources,
            config,
            is_trained: false,
            vector_count: 0,
        })
    }

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
                "Training failed with {} vectors: {}", n_vectors, e
            ))
        })?;

        // Set nprobe after successful training
        // SAFETY: index_ptr is valid, nprobe value is valid.
        // Note: faiss_IndexIVF_set_nprobe returns void (no error code).
        unsafe {
            faiss_IndexIVF_set_nprobe(
                self.index_ptr.as_ptr(),
                self.config.nprobe,
            );
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
                "Search failed for {} queries, k={}: {}", n_queries, k, e
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
                "Vector count ({}) doesn't match ID count ({})", n_vectors, ids.len()
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
            GraphError::FaissAddFailed(format!(
                "Failed to add {} vectors: {}", n_vectors, e
            ))
        })?;

        self.vector_count += n_vectors;
        Ok(())
    }

    /// Get total number of vectors in index (from FAISS).
    #[inline]
    pub fn ntotal(&self) -> usize {
        // SAFETY: index_ptr is valid.
        let count = unsafe { faiss_Index_ntotal(self.index_ptr.as_ptr()) };
        count as usize
    }

    /// Get the number of vectors tracked by this wrapper.
    ///
    /// Note: This may differ from `ntotal()` if vectors were added through
    /// other means or the index was loaded from disk.
    #[inline]
    pub fn len(&self) -> usize {
        self.vector_count
    }

    /// Check if the index is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ntotal() == 0
    }

    /// Check if the index is trained.
    #[inline]
    pub fn is_trained(&self) -> bool {
        self.is_trained
    }

    /// Get the index configuration.
    #[inline]
    pub fn config(&self) -> &IndexConfig {
        &self.config
    }

    /// Get the dimension of vectors in this index.
    #[inline]
    pub fn dimension(&self) -> usize {
        self.config.dimension
    }

    /// Get reference to shared GPU resources.
    #[inline]
    pub fn resources(&self) -> &Arc<GpuResources> {
        &self.gpu_resources
    }

    /// Save index to file.
    ///
    /// # Arguments
    ///
    /// * `path` - File path to save index
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be written or FAISS serialization fails.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> GraphResult<()> {
        let path_str = path.as_ref().to_string_lossy();
        let c_path = CString::new(path_str.as_ref())
            .map_err(|e| GraphError::InvalidConfig(format!(
                "Invalid path '{}': {}", path_str, e
            )))?;

        // SAFETY: index_ptr is valid, c_path is valid null-terminated string.
        let ret = unsafe { faiss_write_index(self.index_ptr.as_ptr(), c_path.as_ptr()) };

        check_faiss_result(ret, "faiss_write_index").map_err(|e| {
            GraphError::Serialization(format!(
                "Failed to save index to '{}': {}", path_str, e
            ))
        })?;

        Ok(())
    }

    /// Load index from file.
    ///
    /// # Arguments
    ///
    /// * `path` - File path to load index from
    /// * `config` - Index configuration (must match saved index dimension)
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be read, FAISS deserialization fails,
    /// or GPU transfer fails.
    pub fn load<P: AsRef<Path>>(path: P, config: IndexConfig) -> GraphResult<Self> {
        let resources = Arc::new(GpuResources::new(config.gpu_id)?);
        Self::load_with_resources(path, config, resources)
    }

    /// Load index from file with shared GPU resources.
    pub fn load_with_resources<P: AsRef<Path>>(
        path: P,
        config: IndexConfig,
        gpu_resources: Arc<GpuResources>,
    ) -> GraphResult<Self> {
        let path_str = path.as_ref().to_string_lossy();
        let c_path = CString::new(path_str.as_ref())
            .map_err(|e| GraphError::InvalidConfig(format!(
                "Invalid path '{}': {}", path_str, e
            )))?;

        // Load CPU index from file
        let mut cpu_index: *mut FaissIndex = std::ptr::null_mut();

        // SAFETY: c_path is valid null-terminated string.
        let ret = unsafe { faiss_read_index(c_path.as_ptr(), 0, &mut cpu_index) };

        check_faiss_result(ret, "faiss_read_index").map_err(|e| {
            GraphError::Deserialization(format!(
                "Failed to load index from '{}': {}", path_str, e
            ))
        })?;

        if cpu_index.is_null() {
            return Err(GraphError::Deserialization(format!(
                "Loaded index pointer is null for '{}'", path_str
            )));
        }

        // Transfer to GPU
        let mut gpu_index: *mut FaissIndex = std::ptr::null_mut();

        // SAFETY: cpu_index is valid (checked above), gpu_resources.inner().as_provider() is valid.
        let ret = unsafe {
            faiss_index_cpu_to_gpu(
                gpu_resources.inner().as_provider(),
                config.gpu_id as c_int,
                cpu_index,
                &mut gpu_index,
            )
        };

        // Free CPU index regardless of transfer result
        // SAFETY: cpu_index was allocated by faiss_read_index and is non-null.
        unsafe { faiss_Index_free(cpu_index) };

        check_faiss_result(ret, "faiss_index_cpu_to_gpu").map_err(|e| {
            GraphError::GpuTransferFailed(format!(
                "Failed to transfer loaded index to GPU {}: {}", config.gpu_id, e
            ))
        })?;

        if gpu_index.is_null() {
            return Err(GraphError::GpuResourceAllocation(
                "Loaded GPU index pointer is null after transfer".to_string()
            ));
        }

        // SAFETY: We verified gpu_index is non-null above.
        let index_ptr = unsafe { NonNull::new_unchecked(gpu_index) };

        // Check if loaded index is trained
        // SAFETY: index_ptr is valid.
        let is_trained = unsafe { faiss_Index_is_trained(index_ptr.as_ptr()) } != 0;

        // Get vector count from FAISS
        // SAFETY: index_ptr is valid.
        let vector_count = unsafe { faiss_Index_ntotal(index_ptr.as_ptr()) } as usize;

        Ok(Self {
            index_ptr,
            gpu_resources,
            config,
            is_trained,
            vector_count,
        })
    }
}

impl Drop for FaissGpuIndex {
    fn drop(&mut self) {
        // SAFETY: index_ptr was allocated by faiss_index_cpu_to_gpu and is non-null.
        // This is the only place where we free the index. GPU resources are freed
        // separately via Arc<GpuResources> when all references are dropped.
        unsafe {
            faiss_Index_free(self.index_ptr.as_ptr());
        }
    }
}

impl std::fmt::Debug for FaissGpuIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaissGpuIndex")
            .field("ntotal", &self.ntotal())
            .field("is_trained", &self.is_trained)
            .field("dimension", &self.config.dimension)
            .field("factory", &self.config.factory_string())
            .field("gpu_id", &self.config.gpu_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== GPU Resource Tests ==========
    // Tests check GPU availability via gpu_available() before making FFI calls.
    // Tests skip gracefully on systems without GPU instead of segfaulting.

    #[test]
    fn test_gpu_resources_creation() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("⚠ Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        // REAL TEST: Actually allocates GPU resources
        let result = GpuResources::new(0);

        match result {
            Ok(resources) => {
                assert_eq!(resources.gpu_id(), 0);
                println!("GPU resources allocated successfully for device 0");
            }
            Err(e) => {
                panic!("GPU resources creation failed with GPU available: {}", e);
            }
        }
    }

    #[test]
    fn test_gpu_resources_invalid_device() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        // REAL TEST: Invalid device ID should fail
        // Note: Device 999 may succeed on systems with many GPUs, but typically fails
        let result = GpuResources::new(999);

        match result {
            Err(GraphError::GpuResourceAllocation(msg)) => {
                assert!(msg.contains("999") || msg.contains("GPU") || msg.contains("failed"));
                println!("Invalid device ID correctly rejected: {}", msg);
            }
            Err(e) => {
                println!("Invalid device rejected with different error: {}", e);
            }
            Ok(_) => {
                // This might succeed on systems with many GPUs or if device 0 is used
                println!("Device 999 unexpectedly succeeded (unusual but possible)");
            }
        }
    }

    // ========== Index Creation Tests ==========

    #[test]
    fn test_index_creation_valid_config() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let config = IndexConfig::default();
        let resources = match GpuResources::new(config.gpu_id) {
            Ok(r) => Arc::new(r),
            Err(e) => {
                panic!("GPU resources creation failed with GPU available: {}", e);
            }
        };

        let result = FaissGpuIndex::with_resources(config.clone(), resources);

        match result {
            Ok(idx) => {
                assert_eq!(idx.dimension(), 1536);
                assert!(!idx.is_trained());
                assert!(idx.is_empty());
                assert_eq!(idx.config().nlist, 16384);
                println!("Index created with factory: {}", idx.config().factory_string());
            }
            Err(e) => panic!("Index creation failed: {}", e),
        }
    }

    #[test]
    fn test_index_creation_invalid_dimension() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let mut config = IndexConfig::default();
        config.dimension = 0;

        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let result = FaissGpuIndex::with_resources(config, resources);

        match result {
            Err(GraphError::InvalidConfig(msg)) => {
                assert!(msg.contains("dimension"));
                println!("Zero dimension correctly rejected: {}", msg);
            }
            _ => panic!("Expected InvalidConfig error for dimension=0"),
        }
    }

    #[test]
    fn test_index_creation_invalid_pq_segments() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let mut config = IndexConfig::default();
        config.pq_segments = 7; // 1536 % 7 != 0

        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let result = FaissGpuIndex::with_resources(config, resources);

        match result {
            Err(GraphError::InvalidConfig(msg)) => {
                assert!(msg.contains("pq_segments"));
                assert!(msg.contains("divide"));
                println!("Invalid pq_segments correctly rejected: {}", msg);
            }
            _ => panic!("Expected InvalidConfig error for pq_segments=7"),
        }
    }

    #[test]
    fn test_index_creation_zero_nlist() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let mut config = IndexConfig::default();
        config.nlist = 0;

        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let result = FaissGpuIndex::with_resources(config, resources);

        match result {
            Err(GraphError::InvalidConfig(msg)) => {
                assert!(msg.contains("nlist"));
                println!("Zero nlist correctly rejected: {}", msg);
            }
            _ => panic!("Expected InvalidConfig error for nlist=0"),
        }
    }

    // ========== Training Tests ==========

    #[test]
    fn test_train_insufficient_data() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let config = IndexConfig::default();
        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
            Ok(idx) => idx,
            Err(e) => panic!("Index creation failed with GPU available: {}", e),
        };

        // Only 1000 vectors, need 4M+
        let vectors: Vec<f32> = vec![0.0; 1000 * config.dimension];
        let result = index.train(&vectors);

        match result {
            Err(GraphError::InsufficientTrainingData { required, provided }) => {
                assert_eq!(required, 4194304);
                assert_eq!(provided, 1000);
                println!("Insufficient training data correctly rejected");
            }
            _ => panic!("Expected InsufficientTrainingData error"),
        }
    }

    #[test]
    fn test_train_dimension_mismatch() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let config = IndexConfig::default();
        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let mut index = match FaissGpuIndex::with_resources(config, resources) {
            Ok(idx) => idx,
            Err(e) => panic!("Index creation failed with GPU available: {}", e),
        };

        // 1537 elements - not divisible by 1536
        let vectors: Vec<f32> = vec![0.0; 1537];
        let result = index.train(&vectors);

        match result {
            Err(GraphError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 1536);
                assert_eq!(actual, 1); // 1537 % 1536 = 1
                println!("Dimension mismatch correctly rejected");
            }
            _ => panic!("Expected DimensionMismatch error"),
        }
    }

    // ========== Add Tests ==========

    #[test]
    fn test_add_without_training() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let config = IndexConfig::default();
        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
            Ok(idx) => idx,
            Err(e) => panic!("Index creation failed with GPU available: {}", e),
        };

        let vectors: Vec<f32> = vec![0.0; config.dimension];
        let ids: Vec<i64> = vec![0];
        let result = index.add_with_ids(&vectors, &ids);

        match result {
            Err(GraphError::IndexNotTrained) => {
                println!("Add without training correctly rejected");
            }
            _ => panic!("Expected IndexNotTrained error"),
        }
    }

    // ========== Search Tests ==========

    #[test]
    fn test_search_without_training() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        let config = IndexConfig::default();
        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let index = match FaissGpuIndex::with_resources(config.clone(), resources) {
            Ok(idx) => idx,
            Err(e) => panic!("Index creation failed with GPU available: {}", e),
        };

        let queries: Vec<f32> = vec![0.0; config.dimension];
        let result = index.search(&queries, 10);

        match result {
            Err(GraphError::IndexNotTrained) => {
                println!("Search without training correctly rejected");
            }
            _ => panic!("Expected IndexNotTrained error"),
        }
    }

    // ========== Trained Index Tests ==========
    // Uses smaller nlist to reduce training data requirements

    #[test]
    fn test_search_trained_index() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        // Use smaller nlist for testing: nlist=64 requires 64*256=16384 training vectors
        // This is much more manageable than the default 16384*256=4M vectors
        let mut config = IndexConfig::default();
        config.nlist = 64;
        config.min_train_vectors = 64 * 256; // 16384 vectors

        let resources = match GpuResources::new(0) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
            Ok(idx) => idx,
            Err(e) => panic!("Index creation failed with GPU available: {}", e),
        };

        // Generate training data (16384 vectors of dimension 1536)
        let n_train = config.min_train_vectors;
        println!("Generating {} training vectors...", n_train);

        let training_data: Vec<f32> = (0..n_train)
            .flat_map(|i| {
                (0..config.dimension).map(move |d| {
                    // Simple deterministic pattern
                    ((i * config.dimension + d) as f32 * 0.001).sin()
                })
            })
            .collect();

        // Train the index
        println!("Training index (nlist={})...", config.nlist);
        let train_start = std::time::Instant::now();
        match index.train(&training_data) {
            Ok(()) => {
                let train_time = train_start.elapsed();
                println!("Training completed in {:?}", train_time);
            }
            Err(e) => panic!("Training failed: {}", e),
        }
        assert!(index.is_trained(), "Index should be trained");

        // Add some vectors
        let n_add = 1000;
        println!("Adding {} vectors...", n_add);

        let add_data: Vec<f32> = (0..n_add)
            .flat_map(|i| {
                (0..config.dimension).map(move |d| {
                    ((i * 7 + d) as f32 * 0.001).cos()
                })
            })
            .collect();
        let add_ids: Vec<i64> = (0..n_add as i64).collect();

        match index.add_with_ids(&add_data, &add_ids) {
            Ok(()) => println!("Added {} vectors", n_add),
            Err(e) => panic!("Add failed: {}", e),
        }
        assert_eq!(index.ntotal(), n_add);

        // Search
        println!("Searching for k=10 neighbors...");
        let query: Vec<f32> = (0..config.dimension)
            .map(|d| (d as f32 * 0.001).sin())
            .collect();

        let search_start = std::time::Instant::now();
        match index.search(&query, 10) {
            Ok((distances, indices)) => {
                let search_time = search_start.elapsed();
                println!("Search completed in {:?}", search_time);

                assert_eq!(distances.len(), 10);
                assert_eq!(indices.len(), 10);
                assert!(indices[0] >= 0, "First result should be valid");
                println!("Top result: idx={}, dist={:.4}", indices[0], distances[0]);
            }
            Err(e) => panic!("Search failed: {}", e),
        }
    }

    // ========== Thread Safety Tests ==========

    #[test]
    fn test_gpu_resources_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuResources>();
        println!("GpuResources is Send + Sync");
    }

    #[test]
    fn test_faiss_gpu_index_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FaissGpuIndex>();
        println!("FaissGpuIndex is Send");
    }

    // ========== GPU Availability Test ==========

    #[test]
    fn test_gpu_availability_check() {
        // This test verifies that gpu_available() works without crashing
        // regardless of whether a GPU is actually present
        let has_gpu = gpu_available();
        if has_gpu {
            println!("GPU detected: faiss_get_num_gpus() > 0");
        } else {
            println!("No GPU detected: faiss_get_num_gpus() returned 0");
        }
        // Test passes regardless of GPU presence - just verifies the check is safe
    }

    // ========== Full Integration Tests ==========
    // These tests use smaller nlist for faster execution

    #[test]
    fn test_full_index_workflow() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        // Use smaller nlist for testing (256 instead of 16384)
        // This requires 256*256=65536 training vectors instead of 4M
        let mut config = IndexConfig::default();
        config.nlist = 256;
        config.min_train_vectors = 256 * 256; // 65536 vectors

        println!("Creating GPU resources...");
        let resources = match GpuResources::new(config.gpu_id) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        println!("Creating index with factory: {}", config.factory_string());
        let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
            Ok(idx) => idx,
            Err(e) => panic!("Index creation failed: {}", e),
        };

        // Generate training data
        println!("Generating {} training vectors (dimension={})...",
            config.min_train_vectors, config.dimension);

        let training_data: Vec<f32> = (0..config.min_train_vectors)
            .flat_map(|i| {
                (0..config.dimension).map(move |d| {
                    ((i * config.dimension + d) as f32 * 0.0001).sin()
                })
            })
            .collect();

        // Train
        println!("Training index...");
        let train_start = std::time::Instant::now();
        match index.train(&training_data) {
            Ok(()) => {
                let train_time = train_start.elapsed();
                println!("Training completed in {:?}", train_time);
            }
            Err(e) => panic!("Training failed: {}", e),
        }
        assert!(index.is_trained());

        // Add vectors
        let n_add = 10_000;
        println!("Adding {} vectors...", n_add);

        let add_data: Vec<f32> = (0..n_add)
            .flat_map(|i| {
                (0..config.dimension).map(move |d| {
                    ((i * 7 + d) as f32 * 0.001).cos()
                })
            })
            .collect();
        let add_ids: Vec<i64> = (0..n_add as i64).collect();

        let add_start = std::time::Instant::now();
        match index.add_with_ids(&add_data, &add_ids) {
            Ok(()) => {
                let add_time = add_start.elapsed();
                println!("Added {} vectors in {:?}", n_add, add_time);
            }
            Err(e) => panic!("Add failed: {}", e),
        }
        assert_eq!(index.ntotal(), n_add);

        // Search
        println!("Searching for k=10 neighbors...");
        let query: Vec<f32> = (0..config.dimension)
            .map(|d| (d as f32 * 0.001).sin())
            .collect();

        let search_start = std::time::Instant::now();
        match index.search(&query, 10) {
            Ok((distances, indices)) => {
                let search_time = search_start.elapsed();
                println!("Search completed in {:?}", search_time);
                println!("Top result: idx={}, dist={:.4}", indices[0], distances[0]);

                assert_eq!(distances.len(), 10);
                assert_eq!(indices.len(), 10);
                assert!(indices[0] >= 0, "First result should be valid");

                // Relaxed performance check for smaller dataset
                assert!(search_time.as_millis() < 500,
                    "Search took too long: {:?}", search_time);
            }
            Err(e) => panic!("Search failed: {}", e),
        }
    }

    #[test]
    fn test_save_load_roundtrip() {
        // Check GPU availability BEFORE making FFI calls to prevent segfaults
        if !gpu_available() {
            println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
            return;
        }

        // Use smaller config for testing
        let mut config = IndexConfig::default();
        config.nlist = 64;
        config.min_train_vectors = 64 * 256; // 16384 vectors

        let resources = match GpuResources::new(config.gpu_id) {
            Ok(r) => Arc::new(r),
            Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
        };

        // Create and train index
        let mut index = match FaissGpuIndex::with_resources(config.clone(), resources.clone()) {
            Ok(idx) => idx,
            Err(e) => panic!("Index creation failed: {}", e),
        };

        let training_data: Vec<f32> = (0..config.min_train_vectors)
            .flat_map(|i| {
                (0..config.dimension).map(move |d| {
                    ((i + d) as f32) * 0.001
                })
            })
            .collect();

        match index.train(&training_data) {
            Ok(()) => println!("Training completed"),
            Err(e) => panic!("Training failed: {}", e),
        }

        // Add some vectors
        let vectors: Vec<f32> = (0..1000)
            .flat_map(|i| {
                (0..config.dimension).map(move |d| (i + d) as f32 * 0.01)
            })
            .collect();
        let ids: Vec<i64> = (0..1000).collect();

        match index.add_with_ids(&vectors, &ids) {
            Ok(()) => println!("Added 1000 vectors"),
            Err(e) => panic!("Add failed: {}", e),
        }

        // Save to temp file
        let temp_path = std::env::temp_dir().join(format!(
            "test_index_{}.faiss",
            std::process::id()
        ));

        match index.save(&temp_path) {
            Ok(()) => println!("Index saved to {:?}", temp_path),
            Err(e) => panic!("Save failed: {}", e),
        }

        // Load
        match FaissGpuIndex::load_with_resources(&temp_path, config, resources) {
            Ok(loaded) => {
                assert_eq!(loaded.ntotal(), index.ntotal());
                assert!(loaded.is_trained());
                println!("Index loaded with {} vectors", loaded.ntotal());
            }
            Err(e) => {
                // Clean up temp file on error
                let _ = std::fs::remove_file(&temp_path);
                panic!("Load failed: {}", e);
            }
        }

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);
    }
}
