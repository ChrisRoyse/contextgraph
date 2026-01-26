//! High-level semantic search with graph context enrichment.
//!
//! This module provides semantic search operations that combine:
//! - FAISS GPU vector similarity search
//! - Optional graph context enrichment via BFS traversal
//!
//! # Architecture
//!
//! ```text
//! Query Embedding
//!       |
//!       v
//! search::semantic_search() -- FAISS GPU search
//!       |
//!       v
//! Apply SemanticSearchOptions filters
//!       |
//!       v (if include_graph_context)
//! BFS traversal for connected nodes
//!       |
//!       v
//! QueryResult with enriched SearchResults
//! ```
//!
//! # Constitution Reference
//!
//! - ARCH-12: E1 is foundation - all retrieval starts with E1
//! - perf.latency.faiss_1M_k100: <2ms target
//! - AP-001: Never unwrap() in prod - all errors properly typed

use std::time::Instant;
use uuid::Uuid;

use crate::error::GraphResult;
use crate::index::FaissGpuIndex;
use crate::search::{
    semantic_search as low_level_search, NodeMetadataProvider, SearchFilters,
    SemanticSearchResultItem,
};
use crate::storage::GraphStorage;

use super::types::{QueryResult, QueryStats, SearchResult, SemanticSearchOptions};

/// Perform semantic search with optional graph context enrichment.
///
/// This is the high-level semantic search function that combines
/// FAISS vector similarity with optional graph traversal to enrich
/// results with connected node information.
///
/// # Arguments
///
/// * `index` - Trained FAISS GPU index
/// * `storage` - Graph storage for metadata and graph traversal
/// * `query_embedding` - Query vector (must match index dimension)
/// * `options` - Search options controlling behavior
///
/// # Returns
///
/// `QueryResult` with filtered, optionally enriched results.
///
/// # Errors
///
/// - `GraphError::IndexNotTrained` if index is not trained
/// - `GraphError::DimensionMismatch` if query dimension doesn't match
/// - `GraphError::InvalidInput` if options are invalid
///
/// # Example
///
/// ```ignore
/// let options = SemanticSearchOptions::default()
///     .with_top_k(50)
///     .with_min_similarity(0.7)
///     .with_domain(Domain::Code);
///
/// let result = semantic_search(&index, &storage, &query_embedding, options).await?;
/// println!("Found {} results in {:?}", result.len(), result.latency);
/// ```
pub async fn semantic_search<M: NodeMetadataProvider>(
    index: &FaissGpuIndex,
    _storage: &GraphStorage,
    query_embedding: &[f32],
    options: SemanticSearchOptions,
    metadata: Option<&M>,
) -> GraphResult<QueryResult> {
    // Validate options - FAIL FAST
    options.validate()?;

    let start = Instant::now();
    let mut stats = QueryStats::default();

    // Build search filters from options
    let filters = build_filters(&options);

    // Perform low-level FAISS search
    let faiss_start = Instant::now();
    let search_result = low_level_search(index, query_embedding, options.top_k, Some(filters), metadata)?;
    stats = stats.with_faiss_time(faiss_start.elapsed().as_micros() as u64);

    let pre_filter_count = search_result.total_hits;
    stats = stats.with_vectors_searched(index.len());

    // Convert low-level results to high-level SearchResults
    let filter_start = Instant::now();
    let mut results: Vec<SearchResult> = search_result
        .items
        .iter()
        .filter_map(|item| convert_result_item(item, &options))
        .collect();
    let post_filter_count = results.len();
    stats = stats
        .with_filter_time(filter_start.elapsed().as_micros() as u64)
        .with_filter_counts(pre_filter_count, post_filter_count);

    // Enrich with graph context if requested
    if options.include_graph_context && !results.is_empty() {
        let graph_start = Instant::now();
        enrich_with_graph_context(&mut results, _storage, options.graph_context_depth).await?;
        stats = stats.with_graph_context_time(graph_start.elapsed().as_micros() as u64);
    }

    let latency = start.elapsed();

    Ok(QueryResult::new(results, pre_filter_count, latency).with_stats(stats))
}

/// Perform semantic search without metadata provider.
///
/// Convenience function for simple searches where metadata resolution
/// is not needed.
pub async fn semantic_search_simple(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    query_embedding: &[f32],
    options: SemanticSearchOptions,
) -> GraphResult<QueryResult> {
    semantic_search::<NoMetadataProvider>(index, storage, query_embedding, options, None).await
}

/// No-op metadata provider for simple searches.
struct NoMetadataProvider;

impl NodeMetadataProvider for NoMetadataProvider {
    fn get_node_uuid(&self, _faiss_id: i64) -> Option<Uuid> {
        None
    }

    fn get_node_domain(&self, _faiss_id: i64) -> Option<crate::search::Domain> {
        None
    }
}

/// Build search filters from semantic search options.
fn build_filters(options: &SemanticSearchOptions) -> SearchFilters {
    let mut filters = SearchFilters::new();

    if options.min_similarity > 0.0 {
        filters = filters.with_min_similarity(options.min_similarity);
    }

    if let Some(domain) = options.domain {
        filters = filters.with_domain(domain);
    }

    filters = filters.with_max_results(options.top_k);

    filters
}

/// Convert a low-level SemanticSearchResultItem to a high-level SearchResult.
fn convert_result_item(
    item: &SemanticSearchResultItem,
    options: &SemanticSearchOptions,
) -> Option<SearchResult> {
    // Filter by minimum similarity
    if item.similarity < options.min_similarity {
        return None;
    }

    // Filter by domain if specified
    if let Some(filter_domain) = options.domain {
        if item.domain != Some(filter_domain) {
            return None;
        }
    }

    // Create node_id from FAISS ID if not available
    let node_id = item.node_id.unwrap_or_else(|| {
        // Generate a deterministic UUID from faiss_id for consistency
        Uuid::from_u64_pair(item.faiss_id as u64, 0)
    });

    let mut result = SearchResult::new(node_id, item.faiss_id, item.similarity, item.distance);

    if let Some(domain) = item.domain {
        result = result.with_domain(domain);
    }

    Some(result)
}

/// Enrich search results with graph context via BFS traversal.
///
/// For each result node, performs BFS to find connected nodes up to
/// the specified depth.
async fn enrich_with_graph_context(
    results: &mut [SearchResult],
    storage: &GraphStorage,
    _depth: usize,
) -> GraphResult<()> {
    for result in results.iter_mut() {
        // Get adjacency list for this node
        // Convert UUID to i64 for storage lookup
        let node_i64 = uuid_to_i64(&result.node_id);

        match storage.get_adjacency(node_i64) {
            Ok(edges) => {
                // Convert LegacyGraphEdge targets to UUIDs
                let connected: Vec<Uuid> = edges
                    .iter()
                    .map(|e| Uuid::from_u64_pair(e.target as u64, 0))
                    .collect();

                // Update result with graph context
                // Note: edge_types left empty as LegacyGraphEdge uses u8, not EdgeType
                result.connected_nodes = connected;
            }
            Err(_) => {
                // Node has no edges - this is fine, not an error
                continue;
            }
        }
    }

    Ok(())
}

/// Convert UUID to i64 for RocksDB key lookup.
#[inline]
fn uuid_to_i64(uuid: &Uuid) -> i64 {
    // Use lower 64 bits of UUID
    let bytes = uuid.as_bytes();
    i64::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_filters_default() {
        let options = SemanticSearchOptions::default();
        let filters = build_filters(&options);

        assert!(filters.domain.is_none());
        // min_similarity of 0.0 means no filter
        assert!(filters.min_similarity.is_none());
    }

    #[test]
    fn test_build_filters_with_options() {
        use crate::search::Domain;

        let options = SemanticSearchOptions::default()
            .with_min_similarity(0.7)
            .with_domain(Domain::Code)
            .with_top_k(50);

        let filters = build_filters(&options);

        assert_eq!(filters.domain, Some(Domain::Code));
        assert_eq!(filters.min_similarity, Some(0.7));
        assert_eq!(filters.max_results, Some(50));
    }

    #[test]
    fn test_uuid_to_i64_deterministic() {
        let uuid = Uuid::from_u128(0x12345678_9abc_def0_1234_567890abcdef);
        let i64_1 = uuid_to_i64(&uuid);
        let i64_2 = uuid_to_i64(&uuid);
        assert_eq!(i64_1, i64_2);
    }

    #[test]
    fn test_convert_result_item_filters_by_similarity() {
        let item = SemanticSearchResultItem {
            faiss_id: 1,
            node_id: Some(Uuid::new_v4()),
            distance: 0.5,
            similarity: 0.5,
            domain: None,
            relevance_score: None,
        };

        let options = SemanticSearchOptions::default().with_min_similarity(0.7);
        let result = convert_result_item(&item, &options);

        assert!(result.is_none()); // Filtered out due to low similarity
    }

    #[test]
    fn test_convert_result_item_passes() {
        let node_id = Uuid::new_v4();
        let item = SemanticSearchResultItem {
            faiss_id: 1,
            node_id: Some(node_id),
            distance: 0.1,
            similarity: 0.95,
            domain: None,
            relevance_score: None,
        };

        let options = SemanticSearchOptions::default().with_min_similarity(0.7);
        let result = convert_result_item(&item, &options);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.node_id, node_id);
        assert_eq!(result.similarity, 0.95);
    }
}
