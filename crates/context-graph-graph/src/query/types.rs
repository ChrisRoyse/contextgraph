//! Query types and options for high-level graph operations.
//!
//! This module defines the core types used by the query module including:
//! - Search options and results
//! - Query configuration
//! - Graph statistics
//!
//! # Constitution Reference
//!
//! - ARCH-12: E1 is foundation - all retrieval starts with E1
//! - perf.latency.faiss_1M_k100: <2ms target
//! - AP-001: Never unwrap() in prod - all errors properly typed

use std::time::Duration;
use uuid::Uuid;

use crate::error::{GraphError, GraphResult};
use crate::search::Domain;
use crate::storage::edges::EdgeType;

/// Options for semantic search queries.
///
/// Controls search behavior including top-k, similarity threshold,
/// filtering by edge types and domain.
///
/// # Example
///
/// ```ignore
/// let options = SemanticSearchOptions::default()
///     .with_top_k(50)
///     .with_min_similarity(0.7)
///     .with_domain(Domain::Code);
/// ```
#[derive(Debug, Clone)]
pub struct SemanticSearchOptions {
    /// Maximum number of results to return.
    pub top_k: usize,

    /// Minimum similarity threshold [0.0, 1.0].
    /// Results below this are filtered out.
    pub min_similarity: f32,

    /// Filter by specific edge types (for graph traversal).
    /// None means all edge types.
    pub edge_types: Option<Vec<EdgeType>>,

    /// Filter by domain.
    /// None means all domains.
    pub domain: Option<Domain>,

    /// Whether to include graph context in results.
    pub include_graph_context: bool,

    /// Maximum BFS depth for graph context enrichment.
    pub graph_context_depth: usize,
}

impl Default for SemanticSearchOptions {
    fn default() -> Self {
        Self {
            top_k: 100,
            min_similarity: 0.0,
            edge_types: None,
            domain: None,
            include_graph_context: false,
            graph_context_depth: 1,
        }
    }
}

impl SemanticSearchOptions {
    /// Create options with custom top-k.
    #[must_use]
    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = k;
        self
    }

    /// Set minimum similarity threshold.
    #[must_use]
    pub fn with_min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = threshold;
        self
    }

    /// Filter by specific edge types.
    #[must_use]
    pub fn with_edge_types(mut self, types: Vec<EdgeType>) -> Self {
        self.edge_types = Some(types);
        self
    }

    /// Filter by domain.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Enable graph context enrichment.
    #[must_use]
    pub fn with_graph_context(mut self, depth: usize) -> Self {
        self.include_graph_context = true;
        self.graph_context_depth = depth;
        self
    }

    /// Validate options - FAIL FAST.
    pub fn validate(&self) -> GraphResult<()> {
        if self.top_k == 0 {
            return Err(GraphError::InvalidInput("top_k must be > 0".to_string()));
        }
        if !(0.0..=1.0).contains(&self.min_similarity) {
            return Err(GraphError::InvalidInput(format!(
                "min_similarity must be in [0.0, 1.0], got {}",
                self.min_similarity
            )));
        }
        Ok(())
    }
}

/// A single search result with node information and score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Node UUID.
    pub node_id: Uuid,

    /// FAISS internal ID for low-level operations.
    pub faiss_id: i64,

    /// Similarity score [0.0, 1.0].
    pub similarity: f32,

    /// L2 squared distance from FAISS.
    pub distance: f32,

    /// Node domain (if known).
    pub domain: Option<Domain>,

    /// Connected nodes (if graph context enabled).
    pub connected_nodes: Vec<Uuid>,

    /// Edge types to connected nodes.
    pub edge_types: Vec<EdgeType>,
}

impl SearchResult {
    /// Create a new search result.
    pub fn new(node_id: Uuid, faiss_id: i64, similarity: f32, distance: f32) -> Self {
        Self {
            node_id,
            faiss_id,
            similarity,
            distance,
            domain: None,
            connected_nodes: Vec::new(),
            edge_types: Vec::new(),
        }
    }

    /// Set domain.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Add graph context.
    #[must_use]
    pub fn with_graph_context(mut self, nodes: Vec<Uuid>, types: Vec<EdgeType>) -> Self {
        self.connected_nodes = nodes;
        self.edge_types = types;
        self
    }
}

/// Result of a graph query operation.
///
/// Contains matched nodes, statistics, and timing information.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Matched search results, ordered by similarity (descending).
    pub results: Vec<SearchResult>,

    /// Total number of hits before filtering.
    pub total_hits: usize,

    /// Query latency.
    pub latency: Duration,

    /// Statistics about the query execution.
    pub stats: QueryStats,
}

impl QueryResult {
    /// Create a new query result.
    pub fn new(results: Vec<SearchResult>, total_hits: usize, latency: Duration) -> Self {
        Self {
            results,
            total_hits,
            latency,
            stats: QueryStats::default(),
        }
    }

    /// Set query statistics.
    #[must_use]
    pub fn with_stats(mut self, stats: QueryStats) -> Self {
        self.stats = stats;
        self
    }

    /// Get the number of results.
    #[inline]
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Check if results are empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Iterate over results.
    pub fn iter(&self) -> impl Iterator<Item = &SearchResult> {
        self.results.iter()
    }

    /// Get the top result (highest similarity).
    pub fn top(&self) -> Option<&SearchResult> {
        self.results.first()
    }
}

/// Query execution statistics.
#[derive(Debug, Clone, Default)]
pub struct QueryStats {
    /// Time spent in FAISS search.
    pub faiss_time_us: u64,

    /// Time spent in filter application.
    pub filter_time_us: u64,

    /// Time spent in graph context enrichment.
    pub graph_context_time_us: u64,

    /// Number of vectors searched.
    pub vectors_searched: usize,

    /// Number of results before filtering.
    pub pre_filter_count: usize,

    /// Number of results after filtering.
    pub post_filter_count: usize,
}

impl QueryStats {
    /// Create stats with FAISS timing.
    pub fn with_faiss_time(mut self, us: u64) -> Self {
        self.faiss_time_us = us;
        self
    }

    /// Add filter timing.
    pub fn with_filter_time(mut self, us: u64) -> Self {
        self.filter_time_us = us;
        self
    }

    /// Add graph context timing.
    pub fn with_graph_context_time(mut self, us: u64) -> Self {
        self.graph_context_time_us = us;
        self
    }

    /// Set vector count.
    pub fn with_vectors_searched(mut self, count: usize) -> Self {
        self.vectors_searched = count;
        self
    }

    /// Set filter counts.
    pub fn with_filter_counts(mut self, pre: usize, post: usize) -> Self {
        self.pre_filter_count = pre;
        self.post_filter_count = post;
        self
    }

    /// Total query time in microseconds.
    #[inline]
    pub fn total_time_us(&self) -> u64 {
        self.faiss_time_us + self.filter_time_us + self.graph_context_time_us
    }
}

/// Graph operation mode for the QueryBuilder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Pure semantic search (vector similarity only).
    Semantic,

    /// Semantic search with graph context enrichment.
    SemanticWithContext,

    /// Entailment query (IS-A hierarchy).
    Entailment,

    /// Contradiction detection.
    Contradiction,
}

impl Default for QueryMode {
    fn default() -> Self {
        Self::Semantic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_search_options_default() {
        let options = SemanticSearchOptions::default();
        assert_eq!(options.top_k, 100);
        assert_eq!(options.min_similarity, 0.0);
        assert!(options.edge_types.is_none());
        assert!(options.domain.is_none());
        assert!(!options.include_graph_context);
    }

    #[test]
    fn test_semantic_search_options_builder() {
        let options = SemanticSearchOptions::default()
            .with_top_k(50)
            .with_min_similarity(0.7)
            .with_domain(Domain::Code)
            .with_graph_context(2);

        assert_eq!(options.top_k, 50);
        assert_eq!(options.min_similarity, 0.7);
        assert_eq!(options.domain, Some(Domain::Code));
        assert!(options.include_graph_context);
        assert_eq!(options.graph_context_depth, 2);
    }

    #[test]
    fn test_semantic_search_options_validate() {
        // Valid
        assert!(SemanticSearchOptions::default().validate().is_ok());

        // Invalid top_k
        let options = SemanticSearchOptions::default().with_top_k(0);
        assert!(options.validate().is_err());

        // Invalid min_similarity
        let options = SemanticSearchOptions {
            min_similarity: 1.5,
            ..Default::default()
        };
        assert!(options.validate().is_err());
    }

    #[test]
    fn test_search_result() {
        let node_id = Uuid::new_v4();
        let result = SearchResult::new(node_id, 42, 0.95, 0.1)
            .with_domain(Domain::Code);

        assert_eq!(result.node_id, node_id);
        assert_eq!(result.faiss_id, 42);
        assert_eq!(result.similarity, 0.95);
        assert_eq!(result.domain, Some(Domain::Code));
    }

    #[test]
    fn test_query_result() {
        let results = vec![
            SearchResult::new(Uuid::new_v4(), 1, 0.95, 0.1),
            SearchResult::new(Uuid::new_v4(), 2, 0.90, 0.2),
        ];

        let query_result = QueryResult::new(results, 100, Duration::from_micros(500));

        assert_eq!(query_result.len(), 2);
        assert!(!query_result.is_empty());
        assert_eq!(query_result.total_hits, 100);
        assert!(query_result.top().is_some());
        assert_eq!(query_result.top().unwrap().similarity, 0.95);
    }

    #[test]
    fn test_query_stats() {
        let stats = QueryStats::default()
            .with_faiss_time(100)
            .with_filter_time(50)
            .with_graph_context_time(25)
            .with_vectors_searched(1000)
            .with_filter_counts(100, 50);

        assert_eq!(stats.total_time_us(), 175);
        assert_eq!(stats.vectors_searched, 1000);
        assert_eq!(stats.pre_filter_count, 100);
        assert_eq!(stats.post_filter_count, 50);
    }

    #[test]
    fn test_query_mode_default() {
        assert_eq!(QueryMode::default(), QueryMode::Semantic);
    }
}
