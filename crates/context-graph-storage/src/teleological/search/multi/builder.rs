//! Builder pattern for multi-embedder search.
//!
//! Provides a fluent API for constructing and executing multi-embedder searches.

use std::collections::HashMap;

use super::super::super::indexes::EmbedderIndex;
use super::super::error::SearchResult;
use super::executor::MultiEmbedderSearch;
use super::types::{AggregationStrategy, MultiEmbedderSearchResults, NormalizationStrategy};

// ============================================================================
// MULTI-SEARCH BUILDER
// ============================================================================

/// Builder pattern for multi-embedder search.
///
/// Provides a fluent API for constructing and executing multi-embedder searches.
///
/// # Example
///
/// ```no_run
/// use context_graph_storage::teleological::search::{
///     MultiEmbedderSearch, MultiSearchBuilder,
///     NormalizationStrategy, AggregationStrategy,
/// };
/// use context_graph_storage::teleological::indexes::{
///     EmbedderIndex, EmbedderIndexRegistry,
/// };
/// use std::sync::Arc;
/// use std::collections::HashMap;
///
/// let registry = Arc::new(EmbedderIndexRegistry::new());
/// let search = MultiEmbedderSearch::new(registry);
///
/// let queries: HashMap<EmbedderIndex, Vec<f32>> = [
///     (EmbedderIndex::E1Semantic, vec![0.5f32; 1024]),
///     (EmbedderIndex::E8Graph, vec![0.5f32; 1024]),
/// ].into_iter().collect();
///
/// let results = MultiSearchBuilder::new(queries)
///     .k(10)
///     .threshold(0.5)
///     .normalization(NormalizationStrategy::MinMax)
///     .aggregation(AggregationStrategy::Max)
///     .execute(&search);
/// ```
#[derive(Debug, Clone)]
pub struct MultiSearchBuilder {
    pub(crate) queries: HashMap<EmbedderIndex, Vec<f32>>,
    k: usize,
    threshold: Option<f32>,
    normalization: NormalizationStrategy,
    aggregation: AggregationStrategy,
}

impl MultiSearchBuilder {
    /// Create a new builder with queries.
    ///
    /// # Arguments
    ///
    /// * `queries` - Map of embedder -> query vector
    pub fn new(queries: HashMap<EmbedderIndex, Vec<f32>>) -> Self {
        Self {
            queries,
            k: 100,
            threshold: None,
            normalization: NormalizationStrategy::None,
            aggregation: AggregationStrategy::Max,
        }
    }

    /// Set the number of results per embedder.
    pub fn k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set the minimum similarity threshold.
    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// Set the normalization strategy.
    pub fn normalization(mut self, strategy: NormalizationStrategy) -> Self {
        self.normalization = strategy;
        self
    }

    /// Set the aggregation strategy.
    pub fn aggregation(mut self, strategy: AggregationStrategy) -> Self {
        self.aggregation = strategy;
        self
    }

    /// Add a query for an additional embedder.
    pub fn add_query(mut self, embedder: EmbedderIndex, query: Vec<f32>) -> Self {
        self.queries.insert(embedder, query);
        self
    }

    /// Execute the search.
    ///
    /// # Arguments
    ///
    /// * `search` - The MultiEmbedderSearch instance to use
    ///
    /// # Returns
    ///
    /// Search results or error.
    pub fn execute(self, search: &MultiEmbedderSearch) -> SearchResult<MultiEmbedderSearchResults> {
        search.search_with_options(
            self.queries,
            self.k,
            self.threshold,
            self.normalization,
            self.aggregation,
        )
    }
}
