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
//!
//! # Module Organization
//!
//! - `resources` - GPU resource management (GpuResources)
//! - `index` - Core index structure and creation (FaissGpuIndex)
//! - `operations` - Train, search, and add operations
//! - `persistence` - Save and load functionality

mod index;
mod operations;
mod persistence;
mod resources;

#[cfg(test)]
mod tests;

// Re-export all public items
pub use index::FaissGpuIndex;
pub use resources::{create_shared_resources, GpuResources};
