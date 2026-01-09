#![deny(deprecated)]

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
//! - **search**: Semantic search with filters and metadata enrichment
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

// ============================================================================
// GPU ENFORCEMENT - AP-007 CRITICAL
// ============================================================================
//
// This compile_error! ensures GPU is available at compile time.
// NO CPU FALLBACK. NO WORKAROUNDS. NO EXCEPTIONS.
//
// The Constitution mandates GPU acceleration for vector search performance.
// Without FAISS GPU, the <2ms latency target (perf.latency.faiss_1M_k100)
// cannot be achieved.
//
// Constitution Reference: stack.gpu, AP-007, TECH-GRAPH-004
// ============================================================================

#[cfg(not(feature = "faiss-gpu"))]
compile_error!(
    "[GRAPH-E001] FAISS_GPU_REQUIRED: The 'faiss-gpu' feature MUST be enabled.

    Context Graph's knowledge graph requires GPU-accelerated vector search.

    Requirements:
        - GPU: RTX 5090 (Blackwell architecture) or compatible
        - CUDA: 13.1+
        - libfaiss_c: GPU-enabled build

    Build with: cargo build --features faiss-gpu

    Constitution Reference: stack.gpu, AP-007, TECH-GRAPH-004
    Performance Target: <2ms for 1M vectors @ k=100 (perf.latency.faiss_1M_k100)
    Exit Code: 102 (FAISS_GPU_UNAVAILABLE)"
);

pub mod config;
pub mod contradiction;
pub mod entailment;
pub mod error;
pub mod hyperbolic;
pub mod index;
pub mod marblestone;
pub mod query;
pub mod search;
pub mod storage;
pub mod traversal;

// Re-exports for convenience
pub use config::{ConeConfig, HyperbolicConfig, IndexConfig};
pub use contradiction::{
    check_contradiction, contradiction_detect, get_contradictions, mark_contradiction,
    ContradictionParams, ContradictionResult, ContradictionType,
};
pub use entailment::{
    entailment_check_batch, entailment_query, entailment_score, is_entailed_by,
    lowest_common_ancestor, BatchEntailmentResult, EntailmentCone, EntailmentDirection,
    EntailmentQueryParams, EntailmentResult, LcaResult,
};
pub use error::{GraphError, GraphResult};
pub use hyperbolic::{PoincareBall, PoincarePoint};
pub use index::{
    AllocationHandle, FaissGpuIndex, GpuMemoryConfig, GpuMemoryManager, GpuResources,
    MemoryCategory, MemoryStats, MetricType, SearchResult, SearchResultItem,
};
pub use search::{
    semantic_search, semantic_search_batch, semantic_search_batch_simple, semantic_search_simple,
    BatchSemanticSearchResult, NoMetadataProvider, NodeMetadataProvider, SearchFilters,
    SearchStats, SemanticSearchResult, SemanticSearchResultItem,
};
pub use storage::{
    get_column_family_descriptors, get_db_options, StorageConfig, ALL_COLUMN_FAMILIES,
    CF_ADJACENCY, CF_CONES, CF_FAISS_IDS, CF_HYPERBOLIC, CF_METADATA, CF_NODES,
};

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
