//! Core domain types for the Context Graph system.

pub mod fingerprint;
mod graph_edge;
mod johari;
mod memory_node;
mod nervous;
mod pulse;
mod utl;

pub use fingerprint::{
    // SemanticFingerprint and embedding types
    EmbeddingSlice, SemanticFingerprint, E10_DIM, E11_DIM, E12_TOKEN_DIM, E1_DIM, E2_DIM, E3_DIM,
    E4_DIM, E5_DIM, E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM, TOTAL_DENSE_DIMS,
    // SparseVector types
    SparseVector, SparseVectorError, MAX_SPARSE_ACTIVE, SPARSE_VOCAB_SIZE,
};
pub use graph_edge::*;
pub use johari::*;
pub use memory_node::*;
pub use nervous::*;
pub use pulse::*;
pub use utl::*;
