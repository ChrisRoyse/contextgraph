//! Single embedder HNSW search implementation.

use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexOps, EmbedderIndexRegistry};
use crate::teleological::search::error::{SearchError, SearchResult};
use crate::teleological::search::result::{EmbedderSearchHit, SingleEmbedderSearchResults};

use super::config::SingleEmbedderSearchConfig;

/// Single embedder HNSW search.
///
/// Queries ONE of the 12 HNSW-capable indexes and returns ranked results
/// with similarity scores.
///
/// # Thread Safety
///
/// The search is thread-safe. Multiple threads can search concurrently
/// using the same instance.
///
/// # Example
///
/// ```no_run
/// use context_graph_storage::teleological::search::SingleEmbedderSearch;
/// use context_graph_storage::teleological::indexes::{
///     EmbedderIndex, EmbedderIndexRegistry,
/// };
/// use std::sync::Arc;
///
/// let registry = Arc::new(EmbedderIndexRegistry::new());
/// let search = SingleEmbedderSearch::new(registry);
///
/// // Search with threshold
/// let query = vec![0.5f32; 1024];
/// let results = search.search(
///     EmbedderIndex::E1Semantic,
///     &query,
///     10,
///     Some(0.7),  // Only return similarity >= 0.7
/// );
/// ```
pub struct SingleEmbedderSearch {
    registry: Arc<EmbedderIndexRegistry>,
    config: SingleEmbedderSearchConfig,
}

impl SingleEmbedderSearch {
    /// Create with default configuration.
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry containing all HNSW indexes
    pub fn new(registry: Arc<EmbedderIndexRegistry>) -> Self {
        Self {
            registry,
            config: SingleEmbedderSearchConfig::default(),
        }
    }

    /// Create with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry containing all HNSW indexes
    /// * `config` - Custom search configuration
    pub fn with_config(
        registry: Arc<EmbedderIndexRegistry>,
        config: SingleEmbedderSearchConfig,
    ) -> Self {
        Self { registry, config }
    }

    /// Search a single embedder index.
    ///
    /// # Arguments
    ///
    /// * `embedder` - Which embedder index to search (must be HNSW-capable)
    /// * `query` - Query vector (must match embedder dimension)
    /// * `k` - Number of results to return
    /// * `threshold` - Minimum similarity threshold (None = no threshold)
    ///
    /// # Returns
    ///
    /// Search results sorted by similarity descending.
    ///
    /// # Errors
    ///
    /// - `SearchError::UnsupportedEmbedder` if embedder is E6/E12/E13
    /// - `SearchError::DimensionMismatch` if query dimension wrong
    /// - `SearchError::EmptyQuery` if query is empty
    /// - `SearchError::InvalidVector` if query contains NaN/Inf
    ///
    /// # Example
    ///
    /// ```no_run
    /// use context_graph_storage::teleological::search::SingleEmbedderSearch;
    /// use context_graph_storage::teleological::indexes::{
    ///     EmbedderIndex, EmbedderIndexRegistry,
    /// };
    /// use std::sync::Arc;
    ///
    /// let registry = Arc::new(EmbedderIndexRegistry::new());
    /// let search = SingleEmbedderSearch::new(registry);
    ///
    /// let query = vec![0.5f32; 1024];  // E8Graph is 1024D
    /// let results = search.search(EmbedderIndex::E8Graph, &query, 10, None);
    ///
    /// match results {
    ///     Ok(r) => println!("Found {} results", r.len()),
    ///     Err(e) => eprintln!("Search failed: {}", e),
    /// }
    /// ```
    pub fn search(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
        k: usize,
        threshold: Option<f32>,
    ) -> SearchResult<SingleEmbedderSearchResults> {
        let start = Instant::now();

        // FAIL FAST: Validate embedder type
        if !embedder.uses_hnsw() {
            return Err(SearchError::UnsupportedEmbedder { embedder });
        }

        // FAIL FAST: Validate query
        self.validate_query(embedder, query)?;

        // Get the index from registry
        let index = self.registry.get(embedder).ok_or_else(|| {
            SearchError::Store(format!("Index not found for {:?} in registry", embedder))
        })?;

        // Handle k=0 edge case
        if k == 0 {
            return Ok(SingleEmbedderSearchResults {
                hits: vec![],
                embedder,
                k,
                threshold,
                latency_us: start.elapsed().as_micros() as u64,
            });
        }

        // Execute HNSW search
        let raw_results = index.search(query, k, self.config.ef_search)?;

        // Convert to hits with similarity scores
        let mut hits: Vec<EmbedderSearchHit> = raw_results
            .into_iter()
            .map(|(id, distance)| EmbedderSearchHit::from_hnsw(id, distance, embedder))
            .collect();

        // Apply threshold filter
        if let Some(min_sim) = threshold {
            hits.retain(|h| h.similarity >= min_sim);
        }

        // Sort by similarity descending (HNSW returns by distance ascending,
        // but conversion might have ordering issues with ties)
        hits.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(SingleEmbedderSearchResults {
            hits,
            embedder,
            k,
            threshold,
            latency_us: start.elapsed().as_micros() as u64,
        })
    }

    /// Search with default k from config.
    ///
    /// # Arguments
    ///
    /// * `embedder` - Which embedder index to search
    /// * `query` - Query vector
    ///
    /// # Returns
    ///
    /// Search results using default k and threshold from config.
    pub fn search_default(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
    ) -> SearchResult<SingleEmbedderSearchResults> {
        self.search(
            embedder,
            query,
            self.config.default_k,
            self.config.default_threshold,
        )
    }

    /// Search and return only IDs above threshold.
    ///
    /// More efficient when you only need IDs, not full hit details.
    ///
    /// # Arguments
    ///
    /// * `embedder` - Which embedder index to search
    /// * `query` - Query vector
    /// * `k` - Maximum results
    /// * `min_similarity` - Minimum similarity threshold
    ///
    /// # Returns
    ///
    /// Vector of (id, similarity) pairs sorted by similarity descending.
    pub fn search_ids_above_threshold(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
        k: usize,
        min_similarity: f32,
    ) -> SearchResult<Vec<(Uuid, f32)>> {
        let results = self.search(embedder, query, k, Some(min_similarity))?;
        Ok(results
            .hits
            .into_iter()
            .map(|h| (h.id, h.similarity))
            .collect())
    }

    /// Validate query vector. FAIL FAST on invalid input.
    fn validate_query(&self, embedder: EmbedderIndex, query: &[f32]) -> SearchResult<()> {
        // Check empty
        if query.is_empty() {
            return Err(SearchError::EmptyQuery { embedder });
        }

        // Check dimension
        if let Some(expected_dim) = embedder.dimension() {
            if query.len() != expected_dim {
                return Err(SearchError::DimensionMismatch {
                    embedder,
                    expected: expected_dim,
                    actual: query.len(),
                });
            }
        }

        // Check for NaN/Inf
        for (i, &v) in query.iter().enumerate() {
            if !v.is_finite() {
                return Err(SearchError::InvalidVector {
                    embedder,
                    message: format!("Non-finite value at index {}: {}", i, v),
                });
            }
        }

        Ok(())
    }

    /// Get the underlying registry.
    pub fn registry(&self) -> &Arc<EmbedderIndexRegistry> {
        &self.registry
    }

    /// Get the configuration.
    pub fn config(&self) -> &SingleEmbedderSearchConfig {
        &self.config
    }
}
