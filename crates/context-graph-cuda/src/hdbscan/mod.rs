//! GPU-accelerated HDBSCAN clustering.
//!
//! Uses FAISS GPU for k-NN (core distance computation) to accelerate HDBSCAN.
//!
//! # Constitution Compliance
//!
//! - ARCH-GPU-05: HDBSCAN clustering runs on GPU
//! - AP-GPU-04: NEVER use sklearn HDBSCAN - use GPU implementation
//! - Performance target: topic_detection < 20ms
//!
//! # Algorithm
//!
//! HDBSCAN has O(n²) complexity in the naive case due to:
//! 1. Core distance computation: For each point, find k-th nearest neighbor
//! 2. Mutual reachability: For each pair, compute MR(a,b) = max(core_a, core_b, dist(a,b))
//!
//! GPU acceleration strategy:
//! 1. Use FAISS GPU Flat index for batch k-NN (O(n) GPU operations vs O(n²k) CPU)
//! 2. Mutual reachability stays on CPU (fast with precomputed core distances)
//! 3. MST + cluster extraction on CPU (already O(n log n), fast)
//!
//! # Error Handling
//!
//! No fallbacks - fails fast with detailed error messages if GPU unavailable.

mod error;
mod gpu_knn;
mod clusterer;

pub use error::{GpuHdbscanError, GpuHdbscanResult};
pub use clusterer::{ClusterMembership, ClusterSelectionMethod, GpuHdbscanClusterer, HdbscanParams};
pub use gpu_knn::GpuKnnIndex;

#[cfg(test)]
mod tests;
