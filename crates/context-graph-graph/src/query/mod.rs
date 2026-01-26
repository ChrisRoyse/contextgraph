//! High-level query operations.
//!
//! This module provides the main query interface for the knowledge graph,
//! combining FAISS vector search with graph traversal and NT modulation.
//!
//! # Query Types
//!
//! - **Semantic Search**: Vector similarity with optional graph context enrichment
//! - **Entailment Query**: IS-A hierarchy using entailment cones in hyperbolic space
//! - **Contradiction Detection**: Identify conflicting knowledge
//! - **Query Builder**: Fluent API for complex queries
//! - **Graph API**: High-level CRUD operations
//!
//! # Architecture
//!
//! The query module composes low-level primitives from:
//! - `search/`: FAISS GPU vector similarity
//! - `entailment/`: Hyperbolic entailment cones for IS-A queries
//! - `contradiction/`: Contradiction detection combining semantic + graph
//!
//! # Performance Targets
//!
//! - Semantic search (1M vectors): <25ms P95
//! - Entailment check: <1ms
//! - Contradiction detection: <50ms
//!
//! # Constitution Reference
//!
//! - ARCH-12: E1 is foundation - all retrieval starts with E1
//! - perf.latency.inject_context: P95 <25ms, P99 <50ms
//! - perf.latency.faiss_1M_k100: <2ms
//! - perf.latency.entailment_check: <1ms
//! - AP-001: Never unwrap() in prod - all errors properly typed
//!
//! # Example
//!
//! ```ignore
//! use context_graph_graph::query::{QueryBuilder, Graph};
//! use context_graph_graph::search::Domain;
//!
//! // Using QueryBuilder for semantic search
//! let results = QueryBuilder::semantic(&embedding)
//!     .with_domain(Domain::Code)
//!     .with_min_similarity(0.7)
//!     .with_top_k(50)
//!     .execute(&index, &storage)
//!     .await?;
//!
//! // Using the high-level Graph API
//! let graph = Graph::open("/data/graph.db")?;
//! let results = graph.search(&embedding, 50, 0.7).await?;
//! ```

pub mod builder;
pub mod contradiction;
pub mod entailment;
pub mod graph;
pub mod semantic;
pub mod types;

// ========== Re-exports ==========

// Core types
pub use types::{QueryMode, QueryResult, QueryStats, SearchResult, SemanticSearchOptions};

// Query builder
pub use builder::QueryBuilder;

// High-level Graph API
pub use graph::{Graph, GraphStats};

// Semantic search
pub use semantic::{semantic_search, semantic_search_simple};

// Entailment queries
pub use entailment::{
    batch_check_entailment, cone_membership_score, entailment_membership_score,
    find_lowest_common_ancestor, get_direct_children, get_direct_parents, is_entailed,
    query_entailment, query_entailment_with_params,
};
// Re-export entailment types from the canonical location
pub use crate::entailment::{EntailmentDirection, EntailmentQueryParams, EntailmentResult};

// Contradiction detection
pub use contradiction::{
    check_contradiction_between, detect_contradictions, detect_contradictions_sensitive,
    detect_contradictions_strict, detect_contradictions_with_params, filter_by_type,
    get_known_contradictions, mark_as_contradicting, most_severe,
};
// Re-export contradiction types from the canonical location
pub use crate::contradiction::{ContradictionParams, ContradictionResult, ContradictionType};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_re_exports_available() {
        // Verify all re-exports are accessible
        let _options = SemanticSearchOptions::default();
        let _mode = QueryMode::Semantic;
        let _direction = EntailmentDirection::Ancestors;
    }

    #[test]
    fn test_query_builder_accessible() {
        let embedding = vec![0.0f32; 1536];
        let _builder = QueryBuilder::semantic(&embedding);
    }

    #[test]
    fn test_semantic_search_options_accessible() {
        use crate::search::Domain;

        let options = SemanticSearchOptions::default()
            .with_top_k(50)
            .with_min_similarity(0.7)
            .with_domain(Domain::Code);

        assert_eq!(options.top_k, 50);
    }
}
