//! SemanticFingerprint and supporting types for 12-embedding storage.
//!
//! This module provides the foundational data structures for the Teleological Vector Architecture,
//! which stores all 12 embedding types WITHOUT fusion to preserve full semantic information.
//!
//! # Key Types
//!
//! - [`SemanticFingerprint`] - Core 12-embedding storage struct (NO FUSION)
//! - [`EmbeddingSlice`] - Reference type for uniform embedding access
//! - [`SparseVector`] - Memory-efficient sparse vector for E6 (SPLADE) embeddings
//! - [`SparseVectorError`] - Error types for sparse vector validation
//!
//! # Design Philosophy
//!
//! **NO FUSION**: Each embedding space is preserved independently for:
//! 1. Per-space similarity search (12x HNSW indexes)
//! 2. Per-space Johari quadrant classification
//! 3. Per-space teleological alignment computation
//! 4. Full semantic information preservation (~46KB vs 6KB fused = 67% info loss avoided)
//!
//! # Example
//!
//! ```
//! use context_graph_core::types::fingerprint::{SemanticFingerprint, EmbeddingSlice};
//!
//! let fp = SemanticFingerprint::zeroed();
//!
//! // Access embedding by index
//! if let Some(EmbeddingSlice::Dense(slice)) = fp.get_embedding(0) {
//!     assert_eq!(slice.len(), 1024); // E1 semantic dimension
//! }
//!
//! // Check storage size
//! let size = fp.storage_size();
//! assert!(size > 60000); // ~60KB minimum for dense embeddings
//! ```

mod semantic;
mod sparse;

pub use semantic::{
    EmbeddingSlice, SemanticFingerprint, E10_DIM, E11_DIM, E12_TOKEN_DIM, E1_DIM, E2_DIM, E3_DIM,
    E4_DIM, E5_DIM, E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM, TOTAL_DENSE_DIMS,
};
pub use sparse::{SparseVector, SparseVectorError, MAX_SPARSE_ACTIVE, SPARSE_VOCAB_SIZE};
