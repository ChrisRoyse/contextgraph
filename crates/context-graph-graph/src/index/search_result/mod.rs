//! FAISS Search Result Types
//!
//! Provides structured wrappers for FAISS k-NN search output with:
//! - Per-query result slicing from flat arrays
//! - Automatic -1 sentinel ID filtering
//! - L2 distance to cosine similarity conversion
//! - Helper methods for common operations
//!
//! # Constitution References
//!
//! - TECH-GRAPH-004 Section 3.2: SearchResult specification
//! - perf.latency.faiss_1M_k100: <2ms target
//!
//! # FAISS Return Format
//!
//! FAISS search returns flat arrays:
//! - `ids`: [q0_r0, q0_r1, ..., q0_rk-1, q1_r0, q1_r1, ..., qn_rk-1]
//! - `distances`: Same layout, L2 squared distances
//! - `-1` sentinel indicates fewer than k matches found for that position

mod metrics;
mod query;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types for backwards compatibility
pub use types::{SearchResult, SearchResultItem};
