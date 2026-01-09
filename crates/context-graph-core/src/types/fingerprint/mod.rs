//! Fingerprint types for the Context Graph system.
//!
//! This module provides the complete teleological fingerprint hierarchy:
//! - SemanticFingerprint: 13-embedding array (TASK-F001) ✅
//! - SparseVector: SPLADE sparse vector for E6 and E13 (TASK-F001) ✅
//! - PurposeVector: 13D alignment to North Star (TASK-F002) ✅
//! - JohariFingerprint: Per-embedder awareness classification (TASK-F003) ✅
//! - TeleologicalFingerprint: Complete node representation (TASK-F002) ✅
//!
//! # Design Philosophy
//!
//! **NO FUSION**: Each embedding space is preserved independently for:
//! 1. Per-space similarity search (13x HNSW indexes)
//! 2. Per-space Johari quadrant classification
//! 3. Per-space teleological alignment computation
//! 4. Full semantic information preservation (~46KB vs 6KB fused = 67% info loss avoided)
//!
//! # Example
//!
//! ```ignore
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

mod evolution;
mod johari;
mod purpose;
mod semantic;
mod sparse;
mod teleological;

// Re-export SemanticFingerprint types (TASK-F001, TASK-CORE-003)
pub use semantic::{
    EmbeddingRef, EmbeddingSlice, SemanticFingerprint, TeleologicalArray, ValidationError, E10_DIM,
    E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM,
    E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM, NUM_EMBEDDERS, TOTAL_DENSE_DIMS,
};

// Re-export SparseVector types (TASK-F001)
pub use sparse::{SparseVector, SparseVectorError, MAX_SPARSE_ACTIVE, SPARSE_VOCAB_SIZE};

// Re-export Purpose types (TASK-F002)
pub use purpose::{AlignmentThreshold, PurposeVector};

// Re-export Evolution types (TASK-F002)
pub use evolution::{EvolutionTrigger, PurposeSnapshot};

// Re-export Johari types (TASK-F003) ✅
pub use johari::JohariFingerprint;

// Re-export TeleologicalFingerprint (TASK-F002)
pub use teleological::TeleologicalFingerprint;
