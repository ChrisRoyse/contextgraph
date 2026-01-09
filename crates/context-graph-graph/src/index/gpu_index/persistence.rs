//! FAISS GPU Index persistence: save and load operations.
//!
//! Provides serialization and deserialization for GPU indices.

use std::ffi::CString;
use std::os::raw::c_int;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;

use super::super::faiss_ffi::{
    check_faiss_result, faiss_Index_free, faiss_Index_is_trained, faiss_Index_ntotal,
    faiss_index_cpu_to_gpu, faiss_read_index, faiss_write_index, FaissIndex,
};
use super::index::FaissGpuIndex;
use super::resources::GpuResources;
use crate::config::IndexConfig;
use crate::error::{GraphError, GraphResult};

impl FaissGpuIndex {
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
        let c_path = CString::new(path_str.as_ref()).map_err(|e| {
            GraphError::InvalidConfig(format!("Invalid path '{}': {}", path_str, e))
        })?;

        // SAFETY: index_ptr is valid, c_path is valid null-terminated string.
        let ret = unsafe { faiss_write_index(self.index_ptr.as_ptr(), c_path.as_ptr()) };

        check_faiss_result(ret, "faiss_write_index").map_err(|e| {
            GraphError::Serialization(format!("Failed to save index to '{}': {}", path_str, e))
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
        let c_path = CString::new(path_str.as_ref()).map_err(|e| {
            GraphError::InvalidConfig(format!("Invalid path '{}': {}", path_str, e))
        })?;

        // Load CPU index from file
        let mut cpu_index: *mut FaissIndex = std::ptr::null_mut();

        // SAFETY: c_path is valid null-terminated string.
        let ret = unsafe { faiss_read_index(c_path.as_ptr(), 0, &mut cpu_index) };

        check_faiss_result(ret, "faiss_read_index").map_err(|e| {
            GraphError::Deserialization(format!("Failed to load index from '{}': {}", path_str, e))
        })?;

        if cpu_index.is_null() {
            return Err(GraphError::Deserialization(format!(
                "Loaded index pointer is null for '{}'",
                path_str
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
                "Failed to transfer loaded index to GPU {}: {}",
                config.gpu_id, e
            ))
        })?;

        if gpu_index.is_null() {
            return Err(GraphError::GpuResourceAllocation(
                "Loaded GPU index pointer is null after transfer".to_string(),
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
