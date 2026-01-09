//! FAISS GPU IVF-PQ Index core implementation.
//!
//! Provides the main index structure with creation and configuration.

use std::ffi::CString;
use std::os::raw::c_int;
use std::ptr::NonNull;
use std::sync::Arc;

use super::super::faiss_ffi::{
    check_faiss_result, faiss_Index_free, faiss_Index_ntotal, faiss_index_cpu_to_gpu,
    faiss_index_factory, FaissIndex, MetricType,
};
use super::resources::GpuResources;
use crate::config::IndexConfig;
use crate::error::{GraphError, GraphResult};

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
    pub(crate) index_ptr: NonNull<FaissIndex>,
    /// Shared GPU resources
    pub(crate) gpu_resources: Arc<GpuResources>,
    /// Index configuration
    pub(crate) config: IndexConfig,
    /// Whether the index has been trained
    pub(crate) is_trained: bool,
    /// Number of vectors in the index (tracked by wrapper)
    pub(crate) vector_count: usize,
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
    pub fn with_resources(
        config: IndexConfig,
        gpu_resources: Arc<GpuResources>,
    ) -> GraphResult<Self> {
        // Validate configuration
        validate_config(&config)?;

        // Create factory string
        let factory_string = config.factory_string();
        let c_factory = CString::new(factory_string.clone()).map_err(|e| {
            GraphError::InvalidConfig(format!(
                "Invalid factory string '{}': {}",
                factory_string, e
            ))
        })?;

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
                "Failed to create CPU index '{}': {}",
                factory_string, e
            ))
        })?;

        if cpu_index.is_null() {
            return Err(GraphError::FaissIndexCreation(
                "CPU index pointer is null after factory creation".to_string(),
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
                "Failed to transfer index to GPU {}: {}",
                config.gpu_id, e
            ))
        })?;

        if gpu_index.is_null() {
            return Err(GraphError::GpuResourceAllocation(
                "GPU index pointer is null after transfer".to_string(),
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

/// Validate index configuration.
fn validate_config(config: &IndexConfig) -> GraphResult<()> {
    if config.dimension == 0 {
        return Err(GraphError::InvalidConfig(
            "dimension must be > 0".to_string(),
        ));
    }
    if config.nlist == 0 {
        return Err(GraphError::InvalidConfig("nlist must be > 0".to_string()));
    }
    if config.pq_segments == 0 {
        return Err(GraphError::InvalidConfig(
            "pq_segments must be > 0".to_string(),
        ));
    }
    if config.dimension % config.pq_segments != 0 {
        return Err(GraphError::InvalidConfig(format!(
            "pq_segments ({}) must divide dimension ({}) evenly",
            config.pq_segments, config.dimension
        )));
    }
    Ok(())
}
