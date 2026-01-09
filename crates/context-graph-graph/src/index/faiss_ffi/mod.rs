//! FAISS C API FFI bindings for GPU-accelerated vector similarity search.
//!
//! This module provides low-level C bindings to the FAISS library.
//! These bindings are used by `FaissGpuIndex` (M04-T10) for IVF-PQ operations.
//!
//! # Module Structure
//!
//! - `types`: Metric type enums and opaque pointer types
//! - `bindings`: Raw extern "C" FFI declarations
//! - `gpu_detection`: Safe GPU availability checking
//! - `gpu_resources`: RAII wrapper for GPU resource management
//! - `helpers`: Utility functions for error handling
//!
//! # Feature Flags
//!
//! - `faiss-gpu`: Enable FAISS GPU FFI bindings. Without this feature,
//!   `gpu_available()` always returns `false` and GPU tests are skipped.
//!
//! # Safety
//!
//! All extern "C" functions are unsafe. The `GpuResources` wrapper provides
//! a safe RAII interface for GPU resource management.
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004 Section 3.1: FAISS FFI Bindings
//! - perf.latency.faiss_1M_k100: <2ms target
//! - AP-015: GPU alloc without pool -> use CUDA memory pool
//!
//! # FAISS C API Reference
//!
//! - <https://github.com/facebookresearch/faiss/blob/main/c_api/>
//! - Functions prefixed `faiss_` (e.g., faiss_index_factory)
//! - Types prefixed `Faiss` (e.g., FaissIndex)

// ========== Submodules ==========

pub mod bindings;
pub mod gpu_detection;
pub mod gpu_resources;
pub mod helpers;
pub mod types;

// ========== Re-exports for backwards compatibility ==========

// Types
pub use types::{FaissGpuResourcesProvider, FaissIndex, FaissStandardGpuResources, MetricType};

// FFI bindings - re-export all extern functions
pub use bindings::{
    faiss_IndexIVF_set_nprobe, faiss_Index_add_with_ids, faiss_Index_free, faiss_Index_is_trained,
    faiss_Index_ntotal, faiss_Index_search, faiss_Index_train, faiss_StandardGpuResources_free,
    faiss_StandardGpuResources_new, faiss_get_num_gpus, faiss_index_cpu_to_gpu,
    faiss_index_factory, faiss_read_index, faiss_write_index,
};

// GPU detection
pub use gpu_detection::gpu_available;

#[cfg(feature = "faiss-gpu")]
pub use gpu_detection::gpu_count_direct;

// GPU resources wrapper
pub use gpu_resources::GpuResources;

// Helper functions
pub use helpers::check_faiss_result;
