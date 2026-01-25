//! GPU-accelerated clustering for topic detection.
//!
//! This module provides GPU-accelerated HDBSCAN clustering using FAISS.
//! Per constitution ARCH-GPU-05: HDBSCAN clustering MUST run on GPU.
//!
//! # Error Handling
//!
//! No CPU fallback is provided. If GPU is unavailable, operations fail with
//! clear error messages per constitution AP-GPU-04.
//!
//! # Integration
//!
//! This module is used by topic_tools to perform GPU-accelerated topic detection.

mod gpu_hdbscan_adapter;

pub use gpu_hdbscan_adapter::{
    GpuClusteringError, GpuClusteringResult, GpuTopicDetector,
};

// Re-export from context-graph-cuda for convenience
pub use context_graph_cuda::hdbscan::ClusterMembership as GpuClusterMembership;
