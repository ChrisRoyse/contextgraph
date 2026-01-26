//! Graph Linking Types for K-NN Graph Construction and Multi-Relation Edges
//!
//! This module provides types for the Knowledge Graph Linking Enhancements feature:
//! - K-NN graph construction per embedder using NN-Descent algorithm
//! - Multi-relation edge types derived from embedder agreement patterns
//! - Asymmetric similarity handling for E5 (causal) and E8 (graph/emotional) per AP-77
//!
//! # Architecture Reference
//!
//! - ARCH-18: E5/E8 use asymmetric similarity (direction matters)
//! - AP-77: E5 MUST NOT use symmetric cosine - FAIL FAST if attempted
//! - AP-60: Temporal embedders (E2-E4) NEVER count toward edge type detection
//!
//! # Module Structure
//!
//! - `edge_type`: 8 graph linking edge types derived from embedder patterns
//! - `direction`: Directed relation for asymmetric edges (E5, E8)
//! - `embedder_edge`: K-NN graph edges per embedder
//! - `typed_edge`: Multi-relation edges with embedder agreement
//! - `error`: Fail-fast error types for graph linking operations
//! - `thresholds`: Configurable edge detection thresholds
//! - `storage_keys`: Binary key formats for RocksDB storage

mod direction;
mod edge_type;
mod embedder_edge;
mod error;
mod knn_graph;
mod storage_keys;
mod thresholds;
mod typed_edge;

// Re-exports
pub use direction::DirectedRelation;
pub use edge_type::GraphLinkEdgeType;
pub use embedder_edge::EmbedderEdge;
pub use error::{EdgeError, EdgeResult};
pub use knn_graph::{KnnGraph, KnnGraphStats};
pub use storage_keys::{EdgeStorageKey, TypedEdgeStorageKey};
pub use thresholds::{EdgeThresholds, DEFAULT_THRESHOLDS};
pub use typed_edge::TypedEdge;

/// Number of neighbors per node in K-NN graph (k=20 per spec)
pub const KNN_K: usize = 20;

/// Number of NN-Descent iterations (typically 5-10)
pub const NN_DESCENT_ITERATIONS: usize = 8;

/// Sampling rate for NN-Descent (ρ = 0.5 per spec)
pub const NN_DESCENT_SAMPLE_RATE: f32 = 0.5;

/// Minimum similarity threshold for K-NN edges
pub const MIN_KNN_SIMILARITY: f32 = 0.3;
