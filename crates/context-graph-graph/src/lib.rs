//! Knowledge Graph with FAISS GPU Vector Search and Hyperbolic Geometry
//!
//! This crate provides the Knowledge Graph layer for the Context Graph system,
//! combining FAISS GPU-accelerated vector similarity search with hyperbolic
//! geometry for hierarchical reasoning.
//!
//! # Architecture
//!
//! - **config**: Index, hyperbolic, and cone configuration types
//! - **error**: Comprehensive error handling with GraphError
//! - **hyperbolic**: Poincare ball model with Mobius operations
//! - **entailment**: Entailment cones for O(1) IS-A queries
//! - **index**: FAISS GPU IVF-PQ index wrapper
//! - **storage**: RocksDB backend for graph persistence
//! - **traversal**: BFS, DFS, and A* graph traversal
//! - **marblestone**: Marblestone NT integration
//! - **query**: High-level query operations
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004: Technical specification
//! - edge_model.nt_weights: Neurotransmitter weighting
//! - perf.latency.faiss_1M_k100: <2ms target
//!
//! # Example
//!
//! ```
//! use context_graph_graph::config::IndexConfig;
//! use context_graph_graph::error::GraphResult;
//!
//! fn example() -> GraphResult<()> {
//!     let config = IndexConfig::default();
//!     assert_eq!(config.dimension, 1536);
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod entailment;
pub mod error;
pub mod hyperbolic;
pub mod index;
pub mod marblestone;
pub mod query;
pub mod storage;
pub mod traversal;

// Re-exports for convenience
pub use config::{ConeConfig, HyperbolicConfig, IndexConfig};
pub use entailment::EntailmentCone;
pub use error::{GraphError, GraphResult};
pub use hyperbolic::{PoincareBall, PoincarePoint};
pub use index::{FaissGpuIndex, GpuResources, MetricType, SearchResult, SearchResultItem};

// Re-export core types for convenience
pub use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
pub use context_graph_core::types::{EmbeddingVector, NodeId, DEFAULT_EMBEDDING_DIM};

/// Re-exported types for embedding operations.
///
/// # Embedding Type Convention
///
/// This crate uses `EmbeddingVector` (= `Vec<f32>`) for all embedding operations.
/// The standard dimension is 1536 per constitution (embeddings.models.E7_Code).
///
/// Use `DEFAULT_EMBEDDING_DIM` constant for dimension validation:
/// ```
/// use context_graph_graph::{EmbeddingVector, DEFAULT_EMBEDDING_DIM};
///
/// fn create_embedding() -> EmbeddingVector {
///     vec![0.0f32; DEFAULT_EMBEDDING_DIM]  // 1536 dimensions
/// }
/// ```

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn test_embedding_vector_reexport() {
        // Verify EmbeddingVector is accessible from crate root
        let embedding: EmbeddingVector = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
        assert_eq!(embedding.len(), 1536);
        assert_eq!(DEFAULT_EMBEDDING_DIM, 1536);
    }

    #[test]
    fn test_node_id_reexport() {
        // Verify NodeId is accessible
        let _id: NodeId = uuid::Uuid::new_v4();
    }

    #[test]
    fn test_embedding_dimension_matches_constitution() {
        // Constitution: embeddings.models.E7_Code = 1536D
        assert_eq!(DEFAULT_EMBEDDING_DIM, 1536);
    }

    #[test]
    fn test_index_config_reexport() {
        // Verify IndexConfig is accessible from crate root
        let config = IndexConfig::default();
        assert_eq!(config.dimension, 1536);
        assert_eq!(config.factory_string(), "IVF16384,PQ64x8");
    }
}
