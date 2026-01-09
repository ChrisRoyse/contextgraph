//! FAISS GPU index wrapper for vector similarity search.
//!
//! This module provides a Rust wrapper around FAISS GPU for efficient
//! similarity search on 1M+ vectors with <2ms latency target.
//!
//! # Architecture
//!
//! ```text
//! faiss_ffi.rs     - Low-level C FFI bindings (M04-T09)
//! gpu_index.rs     - High-level FaissGpuIndex wrapper (M04-T10)
//! search_result.rs - Search result types (M04-T11)
//! gpu_memory.rs    - GPU memory manager for VRAM budget tracking (M04-T28)
//! ```
//!
//! # Index Type
//!
//! Uses IVF-PQ (Inverted File with Product Quantization):
//! - IVF: Partitions vectors into nlist clusters for faster search
//! - PQ: Compresses vectors to reduce memory (64 subquantizers, 8 bits each)
//!
//! # Memory Footprint
//!
//! For 1M 1536D vectors with PQ64x8:
//! - Compressed vectors: 1M * 64 bytes = 64MB
//! - Centroids: 16384 * 1536 * 4 bytes = 100MB
//! - Total GPU memory: ~200MB
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004 Section 3: FAISS Integration
//! - perf.latency.faiss_1M_k100: <2ms
//! - perf.memory.gpu: <24GB (8GB headroom)
//!
//! # GPU Requirements
//!
//! - RTX 5090 with 32GB VRAM (target)
//! - CUDA 13.1
//! - Compute Capability 12.0

pub mod faiss_ffi;
pub mod gpu_index; // Now a directory module
pub mod gpu_memory;
pub mod search_result;

// Re-exports for convenience
pub use faiss_ffi::{check_faiss_result, MetricType};
pub use gpu_index::{FaissGpuIndex, GpuResources};
pub use gpu_memory::{
    AllocationHandle, GpuMemoryConfig, GpuMemoryManager, MemoryCategory, MemoryStats,
};
pub use search_result::{SearchResult, SearchResultItem};
