//! Graph Relationship Module
//!
//! Implements directional graph embeddings and asymmetric similarity for E8.
//!
//! ## Constitution Reference
//!
//! See E8 Graph Embedder specification in architecture documents.
//!
//! ## Features
//!
//! - **E8 Asymmetric Similarity**: Constitution-specified graph similarity
//!   with direction modifiers (source→target=1.2, target→source=0.8)
//! - **Connectivity Context**: Overlap computation for structural relationships
//! - **Query Intent Detection**: Automatic source/target query classification
//!
//! ## NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING

pub mod asymmetric;

pub use asymmetric::{
    compute_e8_asymmetric_fingerprint_similarity, compute_e8_asymmetric_full,
    compute_graph_asymmetric_similarity, compute_graph_asymmetric_similarity_simple,
    detect_graph_query_intent, adjust_batch_graph_similarities,
    ConnectivityContext, GraphDirection,
};
