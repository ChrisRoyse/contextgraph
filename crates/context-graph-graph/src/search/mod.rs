//! Semantic search module for knowledge graph.
//!
//! Provides high-level semantic search operations that combine:
//! - FAISS GPU vector similarity search (M04-T10)
//! - Search result handling (M04-T11)
//! - Domain-aware filtering and relevance scoring
//! - Batch processing for multiple queries
//!
//! # Architecture
//!
//! ```text
//! Query Vector(s)
//!       |
//!       v
//! FaissGpuIndex::search()
//!       |
//!       v
//! SearchResult (raw FAISS output)
//!       |
//!       v
//! Apply SearchFilters (domain, similarity, exclusions)
//!       |
//!       v
//! Resolve metadata via NodeMetadataProvider
//!       |
//!       v
//! SemanticSearchResult (enriched output)
//! ```
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004: Semantic search specification
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - perf.latency.faiss_1M_k100: <2ms target

pub mod domain_search;
pub mod filters;
pub mod result;

// Re-exports for convenience
pub use domain_search::{domain_aware_search, DomainSearchResult, DomainSearchResults};
pub use filters::{Domain, SearchFilters};
pub use result::{
    BatchSemanticSearchResult, SearchStats, SemanticSearchResult, SemanticSearchResultItem,
};

use std::time::Instant;
use uuid::Uuid;

use crate::error::{GraphError, GraphResult};
use crate::index::{FaissGpuIndex, SearchResult, SearchResultItem};

/// Trait for resolving node metadata from FAISS IDs.
///
/// Implementations provide the mapping from FAISS internal IDs to
/// node UUIDs and domain information.
///
/// # Example Implementation
///
/// ```ignore
/// impl NodeMetadataProvider for GraphStorage {
///     fn get_node_uuid(&self, faiss_id: i64) -> Option<Uuid> {
///         // Look up in faiss_ids column family
///         self.get_node_by_faiss_id(faiss_id).ok().flatten()
///     }
///
///     fn get_node_domain(&self, faiss_id: i64) -> Option<Domain> {
///         // Look up domain from node metadata
///         self.get_node_domain(faiss_id).ok().flatten()
///     }
/// }
/// ```
pub trait NodeMetadataProvider {
    /// Get the node UUID for a FAISS internal ID.
    ///
    /// Returns None if the mapping doesn't exist.
    fn get_node_uuid(&self, faiss_id: i64) -> Option<Uuid>;

    /// Get the domain for a FAISS internal ID.
    ///
    /// Returns None if domain is unknown.
    fn get_node_domain(&self, faiss_id: i64) -> Option<Domain>;

    /// Batch get node UUIDs for multiple FAISS IDs.
    ///
    /// Default implementation calls get_node_uuid for each ID.
    /// Override for optimized batch lookups.
    fn get_node_uuids(&self, faiss_ids: &[i64]) -> Vec<Option<Uuid>> {
        faiss_ids.iter().map(|&id| self.get_node_uuid(id)).collect()
    }

    /// Batch get domains for multiple FAISS IDs.
    ///
    /// Default implementation calls get_node_domain for each ID.
    /// Override for optimized batch lookups.
    fn get_node_domains(&self, faiss_ids: &[i64]) -> Vec<Option<Domain>> {
        faiss_ids
            .iter()
            .map(|&id| self.get_node_domain(id))
            .collect()
    }
}

/// No-op metadata provider that returns None for all lookups.
///
/// Useful for testing or when metadata resolution is not needed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMetadataProvider;

impl NodeMetadataProvider for NoMetadataProvider {
    fn get_node_uuid(&self, _faiss_id: i64) -> Option<Uuid> {
        None
    }

    fn get_node_domain(&self, _faiss_id: i64) -> Option<Domain> {
        None
    }
}

/// Perform semantic search for a single query vector.
///
/// Searches the FAISS index for k nearest neighbors, applies filters,
/// enriches results with metadata, and returns a structured result.
///
/// # Arguments
///
/// * `index` - Trained FAISS GPU index
/// * `query` - Query vector (must match index dimension)
/// * `k` - Number of neighbors to retrieve
/// * `filters` - Optional search filters
/// * `metadata` - Optional metadata provider for enrichment
///
/// # Returns
///
/// `SemanticSearchResult` with filtered, enriched results.
///
/// # Errors
///
/// - `GraphError::IndexNotTrained` if index is not trained
/// - `GraphError::DimensionMismatch` if query dimension doesn't match
/// - `GraphError::FaissSearchFailed` if FAISS search fails
/// - `GraphError::InvalidConfig` if filters are invalid
///
/// # Example
///
/// ```ignore
/// use context_graph_graph::search::{semantic_search, SearchFilters, Domain};
/// use context_graph_graph::index::FaissGpuIndex;
///
/// let index: FaissGpuIndex = /* trained index */;
/// let query: Vec<f32> = /* 1536D embedding */;
///
/// let filters = SearchFilters::new()
///     .with_domain(Domain::Code)
///     .with_min_similarity(0.7)
///     .with_max_results(50);
///
/// let result = semantic_search(&index, &query, 100, Some(filters), None)?;
/// println!("Found {} results in {}us", result.total_hits, result.latency_us);
/// ```
pub fn semantic_search<M: NodeMetadataProvider>(
    index: &FaissGpuIndex,
    query: &[f32],
    k: usize,
    filters: Option<SearchFilters>,
    metadata: Option<&M>,
) -> GraphResult<SemanticSearchResult> {
    // Validate filters if provided
    if let Some(ref f) = filters {
        f.validate()?;
    }

    // Determine effective k
    let effective_k = filters.as_ref().map(|f| f.effective_k(k)).unwrap_or(k);

    // Perform FAISS search
    let start = Instant::now();
    let (distances, ids) = index.search(query, effective_k)?;
    let search_latency = start.elapsed();

    // Convert raw results to SearchResult
    let raw_result = SearchResult::new(ids, distances, effective_k, 1);

    // Process results for query 0
    let items = process_query_results(&raw_result, 0, filters.as_ref(), metadata);

    Ok(SemanticSearchResult::new(
        items,
        effective_k,
        search_latency.as_micros() as u64,
    ))
}

/// Perform semantic search for multiple query vectors in batch.
///
/// More efficient than calling semantic_search repeatedly as it
/// batches the FAISS search operation.
///
/// # Arguments
///
/// * `index` - Trained FAISS GPU index
/// * `queries` - Query vectors (flattened, row-major: n_queries * dimension)
/// * `k` - Number of neighbors per query
/// * `filters` - Optional search filters (applied to all queries)
/// * `metadata` - Optional metadata provider for enrichment
///
/// # Returns
///
/// `BatchSemanticSearchResult` with results for each query.
///
/// # Errors
///
/// Same as `semantic_search`, plus:
/// - `GraphError::InvalidConfig` if queries length is not a multiple of dimension
///
/// # Example
///
/// ```ignore
/// use context_graph_graph::search::{semantic_search_batch, SearchFilters};
///
/// // 3 queries of dimension 1536
/// let queries: Vec<f32> = vec![/* 3 * 1536 floats */];
///
/// let results = semantic_search_batch(&index, &queries, 50, None, None)?;
///
/// for (i, result) in results.iter().enumerate() {
///     println!("Query {}: {} hits", i, result.total_hits);
/// }
/// ```
pub fn semantic_search_batch<M: NodeMetadataProvider>(
    index: &FaissGpuIndex,
    queries: &[f32],
    k: usize,
    filters: Option<SearchFilters>,
    metadata: Option<&M>,
) -> GraphResult<BatchSemanticSearchResult> {
    // Validate filters if provided
    if let Some(ref f) = filters {
        f.validate()?;
    }

    // Calculate number of queries
    let dimension = index.dimension();
    if queries.len() % dimension != 0 {
        return Err(GraphError::DimensionMismatch {
            expected: dimension,
            actual: queries.len() % dimension,
        });
    }
    let num_queries = queries.len() / dimension;

    if num_queries == 0 {
        return Ok(BatchSemanticSearchResult::empty());
    }

    // Determine effective k
    let effective_k = filters.as_ref().map(|f| f.effective_k(k)).unwrap_or(k);

    // Perform batch FAISS search
    let start = Instant::now();
    let (distances, ids) = index.search(queries, effective_k)?;
    let total_latency = start.elapsed();

    // Convert raw results
    let raw_result = SearchResult::new(ids, distances, effective_k, num_queries);

    // Process results for each query
    let mut results = Vec::with_capacity(num_queries);
    let per_query_latency = total_latency.as_micros() as u64 / num_queries as u64;

    for query_idx in 0..num_queries {
        let items = process_query_results(&raw_result, query_idx, filters.as_ref(), metadata);

        let result = SemanticSearchResult::new(items, effective_k, per_query_latency)
            .with_query_index(query_idx);

        results.push(result);
    }

    Ok(BatchSemanticSearchResult::new(
        results,
        total_latency.as_micros() as u64,
    ))
}

/// Process raw FAISS results for a single query.
///
/// Applies filters, converts to SemanticSearchResultItem, and enriches with metadata.
fn process_query_results<M: NodeMetadataProvider>(
    raw_result: &SearchResult,
    query_idx: usize,
    filters: Option<&SearchFilters>,
    metadata: Option<&M>,
) -> Vec<SemanticSearchResultItem> {
    let mut items = Vec::new();

    for (id, distance) in raw_result.query_results(query_idx) {
        // Create result item from raw FAISS output
        let raw_item = SearchResultItem::from_l2(id, distance);

        // Apply exclusion filter
        if let Some(f) = filters {
            if f.is_excluded(id) {
                continue;
            }
        }

        // Apply distance filter
        if let Some(f) = filters {
            if !f.passes_distance_filter(distance) {
                continue;
            }
        }

        // Create enriched result item
        let mut item = SemanticSearchResultItem::from_search_result_item(&raw_item);

        // Enrich with metadata if provider is available
        if let Some(m) = metadata {
            if let Some(node_id) = m.get_node_uuid(id) {
                item = item.with_node_id(node_id);
            }
            if let Some(domain) = m.get_node_domain(id) {
                item = item.with_domain(domain);
            }
        }

        // Apply similarity filter (after computing similarity)
        if let Some(f) = filters {
            if !f.passes_similarity_filter(item.similarity) {
                continue;
            }

            // Apply domain filter
            if let Some(filter_domain) = f.domain {
                if item.domain != Some(filter_domain) {
                    continue;
                }
            }
        }

        items.push(item);
    }

    // Apply max_results limit if specified
    if let Some(f) = filters {
        if let Some(max) = f.max_results {
            items.truncate(max);
        }
    }

    items
}

/// Convenience function for semantic search without metadata.
///
/// Equivalent to `semantic_search(index, query, k, filters, None::<&NoMetadataProvider>)`.
pub fn semantic_search_simple(
    index: &FaissGpuIndex,
    query: &[f32],
    k: usize,
    filters: Option<SearchFilters>,
) -> GraphResult<SemanticSearchResult> {
    semantic_search(index, query, k, filters, None::<&NoMetadataProvider>)
}

/// Convenience function for batch semantic search without metadata.
///
/// Equivalent to `semantic_search_batch(index, queries, k, filters, None::<&NoMetadataProvider>)`.
pub fn semantic_search_batch_simple(
    index: &FaissGpuIndex,
    queries: &[f32],
    k: usize,
    filters: Option<SearchFilters>,
) -> GraphResult<BatchSemanticSearchResult> {
    semantic_search_batch(index, queries, k, filters, None::<&NoMetadataProvider>)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== NoMetadataProvider Tests ==========

    #[test]
    fn test_no_metadata_provider() {
        let provider = NoMetadataProvider;
        assert!(provider.get_node_uuid(42).is_none());
        assert!(provider.get_node_domain(42).is_none());
    }

    #[test]
    fn test_no_metadata_provider_batch() {
        let provider = NoMetadataProvider;
        let ids = vec![1, 2, 3];
        let uuids = provider.get_node_uuids(&ids);
        let domains = provider.get_node_domains(&ids);

        assert!(uuids.iter().all(|u| u.is_none()));
        assert!(domains.iter().all(|d| d.is_none()));
    }

    // ========== Custom Metadata Provider for Testing ==========

    struct TestMetadataProvider {
        uuids: std::collections::HashMap<i64, Uuid>,
        domains: std::collections::HashMap<i64, Domain>,
    }

    impl TestMetadataProvider {
        fn new() -> Self {
            let mut uuids = std::collections::HashMap::new();
            let mut domains = std::collections::HashMap::new();

            // Set up test data
            uuids.insert(1, Uuid::from_u128(1));
            uuids.insert(2, Uuid::from_u128(2));
            uuids.insert(3, Uuid::from_u128(3));

            domains.insert(1, Domain::Code);
            domains.insert(2, Domain::Research);
            domains.insert(3, Domain::Code);

            Self { uuids, domains }
        }
    }

    impl NodeMetadataProvider for TestMetadataProvider {
        fn get_node_uuid(&self, faiss_id: i64) -> Option<Uuid> {
            self.uuids.get(&faiss_id).copied()
        }

        fn get_node_domain(&self, faiss_id: i64) -> Option<Domain> {
            self.domains.get(&faiss_id).copied()
        }
    }

    #[test]
    fn test_custom_metadata_provider() {
        let provider = TestMetadataProvider::new();

        assert_eq!(provider.get_node_uuid(1), Some(Uuid::from_u128(1)));
        assert_eq!(provider.get_node_domain(1), Some(Domain::Code));
        assert!(provider.get_node_uuid(999).is_none());
    }

    // ========== Process Query Results Tests ==========

    #[test]
    fn test_process_query_results_no_filters() {
        let raw = SearchResult::new(vec![1, 2, 3], vec![0.1, 0.2, 0.3], 3, 1);

        let items = process_query_results::<NoMetadataProvider>(&raw, 0, None, None);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].faiss_id, 1);
        assert_eq!(items[1].faiss_id, 2);
        assert_eq!(items[2].faiss_id, 3);
    }

    #[test]
    fn test_process_query_results_with_exclusions() {
        let raw = SearchResult::new(vec![1, 2, 3], vec![0.1, 0.2, 0.3], 3, 1);

        let filters = SearchFilters::new().with_exclude_ids(vec![2]);

        let items = process_query_results::<NoMetadataProvider>(&raw, 0, Some(&filters), None);

        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.faiss_id != 2));
    }

    #[test]
    fn test_process_query_results_with_similarity_filter() {
        let raw = SearchResult::new(
            vec![1, 2, 3],
            vec![0.0, 1.0, 2.0], // similarities: 1.0, 0.5, 0.0
            3,
            1,
        );

        let filters = SearchFilters::new().with_min_similarity(0.4);

        let items = process_query_results::<NoMetadataProvider>(&raw, 0, Some(&filters), None);

        // Only id=1 (sim=1.0) and id=2 (sim=0.5) pass the 0.4 threshold
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_process_query_results_with_distance_filter() {
        let raw = SearchResult::new(vec![1, 2, 3], vec![0.1, 0.5, 1.5], 3, 1);

        let filters = SearchFilters::new()
            .with_min_distance(0.2)
            .with_max_distance(1.0);

        let items = process_query_results::<NoMetadataProvider>(&raw, 0, Some(&filters), None);

        // Only id=2 (dist=0.5) is within [0.2, 1.0]
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].faiss_id, 2);
    }

    #[test]
    fn test_process_query_results_with_max_results() {
        let raw = SearchResult::new(vec![1, 2, 3, 4, 5], vec![0.1, 0.2, 0.3, 0.4, 0.5], 5, 1);

        let filters = SearchFilters::new().with_max_results(2);

        let items = process_query_results::<NoMetadataProvider>(&raw, 0, Some(&filters), None);

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_process_query_results_with_metadata() {
        let raw = SearchResult::new(vec![1, 2, 3], vec![0.1, 0.2, 0.3], 3, 1);

        let provider = TestMetadataProvider::new();

        let items = process_query_results(&raw, 0, None, Some(&provider));

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].node_id, Some(Uuid::from_u128(1)));
        assert_eq!(items[0].domain, Some(Domain::Code));
        assert_eq!(items[1].node_id, Some(Uuid::from_u128(2)));
        assert_eq!(items[1].domain, Some(Domain::Research));
    }

    #[test]
    fn test_process_query_results_with_domain_filter() {
        let raw = SearchResult::new(vec![1, 2, 3], vec![0.1, 0.2, 0.3], 3, 1);

        let provider = TestMetadataProvider::new();
        let filters = SearchFilters::new().with_domain(Domain::Code);

        let items = process_query_results(&raw, 0, Some(&filters), Some(&provider));

        // Only ids 1 and 3 have Domain::Code
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.domain == Some(Domain::Code)));
    }

    // ========== Filter Sentinel Tests ==========

    #[test]
    fn test_process_query_results_filters_sentinels() {
        let raw = SearchResult::new(
            vec![1, -1, 3], // -1 is sentinel
            vec![0.1, 0.0, 0.3],
            3,
            1,
        );

        let items = process_query_results::<NoMetadataProvider>(&raw, 0, None, None);

        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.faiss_id != -1));
    }

    // ========== Empty Results Tests ==========

    #[test]
    fn test_process_query_results_all_filtered() {
        let raw = SearchResult::new(
            vec![1, 2, 3],
            vec![2.0, 2.0, 2.0], // All similarity = 0.0
            3,
            1,
        );

        let filters = SearchFilters::new().with_min_similarity(0.5);

        let items = process_query_results::<NoMetadataProvider>(&raw, 0, Some(&filters), None);

        assert!(items.is_empty());
    }
}
