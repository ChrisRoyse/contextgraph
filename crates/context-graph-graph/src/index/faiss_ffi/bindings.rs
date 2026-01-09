//! FAISS C API FFI bindings.
//!
//! This module contains the raw extern "C" function declarations
//! for interfacing with the FAISS library.
//!
//! # Safety
//!
//! All extern "C" functions are unsafe. Use the safe wrappers
//! in the `gpu_resources` module for RAII-managed GPU resources.
//!
//! # FAISS C API Reference
//!
//! - <https://github.com/facebookresearch/faiss/blob/main/c_api/>
//! - Functions prefixed `faiss_` (e.g., faiss_index_factory)
//! - Types prefixed `Faiss` (e.g., FaissIndex)
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004 Section 3.1: FAISS FFI Bindings

use std::os::raw::{c_char, c_float, c_int, c_long};

use super::types::{FaissGpuResourcesProvider, FaissIndex, FaissStandardGpuResources, MetricType};

#[link(name = "faiss_c")]
extern "C" {
    // ---------- Index Factory ----------

    /// Create index from factory string.
    ///
    /// # Arguments
    /// - `p_index`: Output pointer to created index
    /// - `d`: Vector dimension
    /// - `description`: Factory string (e.g., "IVF16384,PQ64x8")
    /// - `metric`: Distance metric type
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    pub fn faiss_index_factory(
        p_index: *mut *mut FaissIndex,
        d: c_int,
        description: *const c_char,
        metric: MetricType,
    ) -> c_int;

    // ---------- GPU Resources ----------

    /// Allocate standard GPU resources.
    ///
    /// Creates a new StandardGpuResources object for GPU memory management.
    /// MUST be freed with `faiss_StandardGpuResources_free`.
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    pub fn faiss_StandardGpuResources_new(p_res: *mut *mut FaissStandardGpuResources) -> c_int;

    /// Free GPU resources.
    ///
    /// Releases all GPU memory held by this resources object.
    pub fn faiss_StandardGpuResources_free(res: *mut FaissStandardGpuResources);

    // Note: No faiss_StandardGpuResources_as_GpuResourcesProvider function exists.
    // FAISS C API uses FAISS_DECLARE_CLASS_INHERITED(StandardGpuResources, GpuResourcesProvider)
    // which creates a typedef alias, making the types structurally identical.
    // Direct pointer cast is safe and correct.

    // ---------- CPU to GPU Transfer ----------

    /// Transfer index from CPU to GPU.
    ///
    /// # Arguments
    /// - `provider`: GPU resources provider
    /// - `device`: GPU device ID (usually 0)
    /// - `index`: Source CPU index
    /// - `p_out`: Output pointer to GPU index
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    pub fn faiss_index_cpu_to_gpu(
        provider: *mut FaissGpuResourcesProvider,
        device: c_int,
        index: *const FaissIndex,
        p_out: *mut *mut FaissIndex,
    ) -> c_int;

    // ---------- Index Operations ----------

    /// Train the index with vectors.
    ///
    /// For IVF indices, this clusters the vectors to create centroids.
    /// Must be called before `add_with_ids` for untrained indices.
    ///
    /// # Arguments
    /// - `index`: Target index
    /// - `n`: Number of training vectors
    /// - `x`: Training vectors (n * d floats, row-major)
    pub fn faiss_Index_train(index: *mut FaissIndex, n: c_long, x: *const c_float) -> c_int;

    /// Check if index is trained.
    ///
    /// # Returns
    /// Non-zero if trained, 0 if not trained
    pub fn faiss_Index_is_trained(index: *const FaissIndex) -> c_int;

    /// Add vectors with IDs to the index.
    ///
    /// # Arguments
    /// - `index`: Target index
    /// - `n`: Number of vectors
    /// - `x`: Vectors to add (n * d floats, row-major)
    /// - `xids`: Vector IDs (n longs)
    pub fn faiss_Index_add_with_ids(
        index: *mut FaissIndex,
        n: c_long,
        x: *const c_float,
        xids: *const c_long,
    ) -> c_int;

    /// Search for k nearest neighbors.
    ///
    /// # Arguments
    /// - `index`: Source index
    /// - `n`: Number of query vectors
    /// - `x`: Query vectors (n * d floats, row-major)
    /// - `k`: Number of neighbors to return
    /// - `distances`: Output distances (n * k floats)
    /// - `labels`: Output IDs (n * k longs, -1 for missing)
    pub fn faiss_Index_search(
        index: *const FaissIndex,
        n: c_long,
        x: *const c_float,
        k: c_long,
        distances: *mut c_float,
        labels: *mut c_long,
    ) -> c_int;

    /// Set nprobe parameter for IVF index.
    ///
    /// Controls search quality vs speed tradeoff.
    /// Higher values = more accurate but slower.
    ///
    /// Note: FAISS_DECLARE_SETTER macro generates `faiss_IndexIVF_set_nprobe`
    /// with void return type (not c_int).
    pub fn faiss_IndexIVF_set_nprobe(index: *mut FaissIndex, nprobe: usize);

    /// Get total number of vectors in index.
    pub fn faiss_Index_ntotal(index: *const FaissIndex) -> c_long;

    // ---------- Persistence ----------

    /// Write index to file.
    ///
    /// # Arguments
    /// - `index`: Source index
    /// - `fname`: Output file path (C string)
    pub fn faiss_write_index(index: *const FaissIndex, fname: *const c_char) -> c_int;

    /// Read index from file.
    ///
    /// # Arguments
    /// - `fname`: Input file path (C string)
    /// - `io_flags`: IO flags (usually 0)
    /// - `p_out`: Output pointer to loaded index
    pub fn faiss_read_index(
        fname: *const c_char,
        io_flags: c_int,
        p_out: *mut *mut FaissIndex,
    ) -> c_int;

    /// Free index.
    ///
    /// Releases all memory held by the index.
    pub fn faiss_Index_free(index: *mut FaissIndex);

    // ---------- GPU Detection ----------

    /// Get the number of available GPUs.
    ///
    /// Writes the number of CUDA-capable GPUs visible to FAISS into `p_output`.
    /// Use this to check GPU availability before attempting GPU operations.
    ///
    /// # Arguments
    /// * `p_output` - Pointer to store the GPU count
    ///
    /// # Returns
    /// 0 on success, non-zero error code on failure
    pub fn faiss_get_num_gpus(p_output: *mut c_int) -> c_int;
}
