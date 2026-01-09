//! Semantic search result types.
//!
//! Provides structured wrappers for semantic search output that enriches
//! raw FAISS results with node metadata, domain information, and similarity scores.
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004: Semantic search specification
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - perf.latency.faiss_1M_k100: <2ms target

use uuid::Uuid;

use crate::index::SearchResultItem;
use crate::storage::edges::Domain;

/// Single result item from semantic search.
///
/// Enriches the raw FAISS result with node metadata and domain information.
/// Provides both distance (L2) and similarity (cosine) for flexibility.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticSearchResultItem {
    /// FAISS internal ID (maps to node)
    pub faiss_id: i64,
    /// Node UUID (if resolved via metadata provider)
    pub node_id: Option<Uuid>,
    /// L2 squared distance from query (lower = more similar)
    pub distance: f32,
    /// Cosine similarity derived from L2 (higher = more similar)
    /// For normalized vectors: similarity = 1 - (distance / 2)
    pub similarity: f32,
    /// Node domain if available from metadata
    pub domain: Option<Domain>,
    /// Optional relevance score incorporating domain boost
    pub relevance_score: Option<f32>,
}

impl SemanticSearchResultItem {
    /// Create from raw FAISS result item.
    ///
    /// # Arguments
    ///
    /// * `item` - Raw SearchResultItem from FAISS
    #[inline]
    pub fn from_search_result_item(item: &SearchResultItem) -> Self {
        Self {
            faiss_id: item.id,
            node_id: None,
            distance: item.distance,
            similarity: item.similarity,
            domain: None,
            relevance_score: None,
        }
    }

    /// Create with explicit values.
    #[inline]
    pub fn new(faiss_id: i64, distance: f32, similarity: f32) -> Self {
        Self {
            faiss_id,
            node_id: None,
            distance,
            similarity,
            domain: None,
            relevance_score: None,
        }
    }

    /// Set the node UUID.
    #[inline]
    #[must_use]
    pub fn with_node_id(mut self, node_id: Uuid) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Set the domain.
    #[inline]
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Set the relevance score.
    #[inline]
    #[must_use]
    pub fn with_relevance_score(mut self, score: f32) -> Self {
        self.relevance_score = Some(score);
        self
    }

    /// Compute relevance score with domain boost.
    ///
    /// When query domain matches result domain, applies a boost factor.
    ///
    /// # Arguments
    ///
    /// * `query_domain` - The domain of the query
    /// * `boost_factor` - Boost multiplier for matching domains (e.g., 1.2)
    ///
    /// # Returns
    ///
    /// Relevance score = similarity * boost (if domains match) or similarity
    #[inline]
    pub fn compute_relevance(&mut self, query_domain: Option<Domain>, boost_factor: f32) {
        let base_score = self.similarity;

        let boosted = match (query_domain, self.domain) {
            (Some(qd), Some(rd)) if qd == rd => base_score * boost_factor,
            _ => base_score,
        };

        // Clamp to [0, 1] range
        self.relevance_score = Some(boosted.clamp(0.0, 1.0));
    }
}

/// Result from semantic search operation.
///
/// Contains search results with metadata, timing information, and statistics.
/// Designed for both single-query and batch operations.
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    /// Search result items, sorted by relevance (highest first)
    pub items: Vec<SemanticSearchResultItem>,
    /// Number of results requested (k)
    pub k: usize,
    /// Actual number of valid results returned
    pub total_hits: usize,
    /// Search latency in microseconds
    pub latency_us: u64,
    /// Query index for batch operations (0 for single query)
    pub query_index: usize,
}

impl SemanticSearchResult {
    /// Create new search result.
    ///
    /// # Arguments
    ///
    /// * `items` - Search result items
    /// * `k` - Number of results requested
    /// * `latency_us` - Search latency in microseconds
    #[inline]
    pub fn new(items: Vec<SemanticSearchResultItem>, k: usize, latency_us: u64) -> Self {
        let total_hits = items.len();
        Self {
            items,
            k,
            total_hits,
            latency_us,
            query_index: 0,
        }
    }

    /// Create empty result (no matches found).
    #[inline]
    pub fn empty(k: usize, latency_us: u64) -> Self {
        Self {
            items: Vec::new(),
            k,
            total_hits: 0,
            latency_us,
            query_index: 0,
        }
    }

    /// Set query index for batch operations.
    #[inline]
    #[must_use]
    pub fn with_query_index(mut self, index: usize) -> Self {
        self.query_index = index;
        self
    }

    /// Check if result is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get number of results.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Get top result if available.
    #[inline]
    pub fn top(&self) -> Option<&SemanticSearchResultItem> {
        self.items.first()
    }

    /// Get result at specific rank (0-indexed).
    #[inline]
    pub fn at(&self, rank: usize) -> Option<&SemanticSearchResultItem> {
        self.items.get(rank)
    }

    /// Iterate over results.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &SemanticSearchResultItem> {
        self.items.iter()
    }

    /// Take top N results.
    ///
    /// Returns a new SemanticSearchResult with at most N items.
    pub fn take(mut self, n: usize) -> Self {
        self.items.truncate(n);
        self.total_hits = self.items.len();
        self
    }

    /// Filter results by minimum similarity.
    ///
    /// Returns a new SemanticSearchResult with only items above threshold.
    pub fn filter_by_similarity(mut self, min_similarity: f32) -> Self {
        self.items.retain(|item| item.similarity >= min_similarity);
        self.total_hits = self.items.len();
        self
    }

    /// Filter results by domain.
    ///
    /// Returns a new SemanticSearchResult with only items matching domain.
    pub fn filter_by_domain(mut self, domain: Domain) -> Self {
        self.items.retain(|item| item.domain == Some(domain));
        self.total_hits = self.items.len();
        self
    }

    /// Sort results by relevance score (highest first).
    ///
    /// Requires relevance_score to be computed on items.
    pub fn sort_by_relevance(&mut self) {
        self.items.sort_by(|a, b| {
            let score_a = a.relevance_score.unwrap_or(a.similarity);
            let score_b = b.relevance_score.unwrap_or(b.similarity);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Sort results by similarity (highest first).
    pub fn sort_by_similarity(&mut self) {
        self.items.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Sort results by distance (lowest first).
    pub fn sort_by_distance(&mut self) {
        self.items.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Get statistics about the results.
    pub fn stats(&self) -> SearchStats {
        if self.items.is_empty() {
            return SearchStats::default();
        }

        let similarities: Vec<f32> = self.items.iter().map(|i| i.similarity).collect();
        let distances: Vec<f32> = self.items.iter().map(|i| i.distance).collect();

        SearchStats {
            count: self.items.len(),
            min_similarity: similarities.iter().cloned().fold(f32::INFINITY, f32::min),
            max_similarity: similarities
                .iter()
                .cloned()
                .fold(f32::NEG_INFINITY, f32::max),
            avg_similarity: similarities.iter().sum::<f32>() / similarities.len() as f32,
            min_distance: distances.iter().cloned().fold(f32::INFINITY, f32::min),
            max_distance: distances.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
            avg_distance: distances.iter().sum::<f32>() / distances.len() as f32,
        }
    }
}

/// Statistics about search results.
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    /// Number of results
    pub count: usize,
    /// Minimum similarity score
    pub min_similarity: f32,
    /// Maximum similarity score
    pub max_similarity: f32,
    /// Average similarity score
    pub avg_similarity: f32,
    /// Minimum L2 distance
    pub min_distance: f32,
    /// Maximum L2 distance
    pub max_distance: f32,
    /// Average L2 distance
    pub avg_distance: f32,
}

/// Batch search result containing results for multiple queries.
#[derive(Debug, Clone)]
pub struct BatchSemanticSearchResult {
    /// Results for each query, indexed by query position
    pub results: Vec<SemanticSearchResult>,
    /// Total search latency for all queries in microseconds
    pub total_latency_us: u64,
    /// Number of queries processed
    pub num_queries: usize,
}

impl BatchSemanticSearchResult {
    /// Create new batch result.
    pub fn new(results: Vec<SemanticSearchResult>, total_latency_us: u64) -> Self {
        let num_queries = results.len();
        Self {
            results,
            total_latency_us,
            num_queries,
        }
    }

    /// Create empty batch result.
    pub fn empty() -> Self {
        Self {
            results: Vec::new(),
            total_latency_us: 0,
            num_queries: 0,
        }
    }

    /// Get result for a specific query index.
    pub fn get(&self, query_idx: usize) -> Option<&SemanticSearchResult> {
        self.results.get(query_idx)
    }

    /// Iterate over all query results.
    pub fn iter(&self) -> impl Iterator<Item = &SemanticSearchResult> {
        self.results.iter()
    }

    /// Check if any query returned results.
    pub fn has_any_results(&self) -> bool {
        self.results.iter().any(|r| !r.is_empty())
    }

    /// Get total number of hits across all queries.
    pub fn total_hits(&self) -> usize {
        self.results.iter().map(|r| r.total_hits).sum()
    }

    /// Average latency per query in microseconds.
    pub fn avg_latency_per_query(&self) -> u64 {
        if self.num_queries == 0 {
            0
        } else {
            self.total_latency_us / self.num_queries as u64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_search_result_item_new() {
        let item = SemanticSearchResultItem::new(42, 0.5, 0.75);
        assert_eq!(item.faiss_id, 42);
        assert!((item.distance - 0.5).abs() < 0.0001);
        assert!((item.similarity - 0.75).abs() < 0.0001);
        assert!(item.node_id.is_none());
        assert!(item.domain.is_none());
    }

    #[test]
    fn test_semantic_search_result_item_builder() {
        let node_id = Uuid::new_v4();
        let item = SemanticSearchResultItem::new(1, 0.2, 0.9)
            .with_node_id(node_id)
            .with_domain(Domain::Code)
            .with_relevance_score(0.95);

        assert_eq!(item.node_id, Some(node_id));
        assert_eq!(item.domain, Some(Domain::Code));
        assert_eq!(item.relevance_score, Some(0.95));
    }

    #[test]
    fn test_compute_relevance_with_domain_match() {
        let mut item = SemanticSearchResultItem::new(1, 0.2, 0.8).with_domain(Domain::Code);

        item.compute_relevance(Some(Domain::Code), 1.2);

        // 0.8 * 1.2 = 0.96
        assert!((item.relevance_score.unwrap() - 0.96).abs() < 0.0001);
    }

    #[test]
    fn test_compute_relevance_no_domain_match() {
        let mut item = SemanticSearchResultItem::new(1, 0.2, 0.8).with_domain(Domain::Code);

        item.compute_relevance(Some(Domain::Research), 1.2);

        // No boost, stays at 0.8
        assert!((item.relevance_score.unwrap() - 0.8).abs() < 0.0001);
    }

    #[test]
    fn test_compute_relevance_clamp() {
        let mut item = SemanticSearchResultItem::new(1, 0.0, 0.95).with_domain(Domain::Code);

        // 0.95 * 1.2 = 1.14, should clamp to 1.0
        item.compute_relevance(Some(Domain::Code), 1.2);
        assert!((item.relevance_score.unwrap() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_semantic_search_result_new() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.1, 0.95),
            SemanticSearchResultItem::new(2, 0.2, 0.90),
        ];
        let result = SemanticSearchResult::new(items, 10, 500);

        assert_eq!(result.k, 10);
        assert_eq!(result.total_hits, 2);
        assert_eq!(result.latency_us, 500);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_semantic_search_result_empty() {
        let result = SemanticSearchResult::empty(10, 100);

        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
        assert!(result.top().is_none());
    }

    #[test]
    fn test_semantic_search_result_top() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.1, 0.95),
            SemanticSearchResultItem::new(2, 0.2, 0.90),
        ];
        let result = SemanticSearchResult::new(items, 10, 500);

        let top = result.top().unwrap();
        assert_eq!(top.faiss_id, 1);
    }

    #[test]
    fn test_semantic_search_result_at() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.1, 0.95),
            SemanticSearchResultItem::new(2, 0.2, 0.90),
        ];
        let result = SemanticSearchResult::new(items, 10, 500);

        assert_eq!(result.at(0).unwrap().faiss_id, 1);
        assert_eq!(result.at(1).unwrap().faiss_id, 2);
        assert!(result.at(2).is_none());
    }

    #[test]
    fn test_semantic_search_result_take() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.1, 0.95),
            SemanticSearchResultItem::new(2, 0.2, 0.90),
            SemanticSearchResultItem::new(3, 0.3, 0.85),
        ];
        let result = SemanticSearchResult::new(items, 10, 500).take(2);

        assert_eq!(result.len(), 2);
        assert_eq!(result.at(0).unwrap().faiss_id, 1);
        assert_eq!(result.at(1).unwrap().faiss_id, 2);
    }

    #[test]
    fn test_semantic_search_result_filter_by_similarity() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.1, 0.95),
            SemanticSearchResultItem::new(2, 0.5, 0.75),
            SemanticSearchResultItem::new(3, 0.8, 0.60),
        ];
        let result = SemanticSearchResult::new(items, 10, 500).filter_by_similarity(0.7);

        assert_eq!(result.len(), 2);
        assert!(result.items.iter().all(|i| i.similarity >= 0.7));
    }

    #[test]
    fn test_semantic_search_result_filter_by_domain() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.1, 0.95).with_domain(Domain::Code),
            SemanticSearchResultItem::new(2, 0.2, 0.90).with_domain(Domain::Research),
            SemanticSearchResultItem::new(3, 0.3, 0.85).with_domain(Domain::Code),
        ];
        let result = SemanticSearchResult::new(items, 10, 500).filter_by_domain(Domain::Code);

        assert_eq!(result.len(), 2);
        assert!(result.items.iter().all(|i| i.domain == Some(Domain::Code)));
    }

    #[test]
    fn test_semantic_search_result_sort_by_similarity() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.3, 0.85),
            SemanticSearchResultItem::new(2, 0.1, 0.95),
            SemanticSearchResultItem::new(3, 0.2, 0.90),
        ];
        let mut result = SemanticSearchResult::new(items, 10, 500);
        result.sort_by_similarity();

        assert_eq!(result.items[0].faiss_id, 2); // 0.95
        assert_eq!(result.items[1].faiss_id, 3); // 0.90
        assert_eq!(result.items[2].faiss_id, 1); // 0.85
    }

    #[test]
    fn test_semantic_search_result_sort_by_distance() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.3, 0.85),
            SemanticSearchResultItem::new(2, 0.1, 0.95),
            SemanticSearchResultItem::new(3, 0.2, 0.90),
        ];
        let mut result = SemanticSearchResult::new(items, 10, 500);
        result.sort_by_distance();

        assert_eq!(result.items[0].faiss_id, 2); // 0.1
        assert_eq!(result.items[1].faiss_id, 3); // 0.2
        assert_eq!(result.items[2].faiss_id, 1); // 0.3
    }

    #[test]
    fn test_semantic_search_result_stats() {
        let items = vec![
            SemanticSearchResultItem::new(1, 0.1, 0.95),
            SemanticSearchResultItem::new(2, 0.2, 0.90),
            SemanticSearchResultItem::new(3, 0.3, 0.85),
        ];
        let result = SemanticSearchResult::new(items, 10, 500);
        let stats = result.stats();

        assert_eq!(stats.count, 3);
        assert!((stats.min_similarity - 0.85).abs() < 0.0001);
        assert!((stats.max_similarity - 0.95).abs() < 0.0001);
        assert!((stats.avg_similarity - 0.9).abs() < 0.0001);
        assert!((stats.min_distance - 0.1).abs() < 0.0001);
        assert!((stats.max_distance - 0.3).abs() < 0.0001);
        assert!((stats.avg_distance - 0.2).abs() < 0.0001);
    }

    #[test]
    fn test_batch_semantic_search_result() {
        let results = vec![
            SemanticSearchResult::new(vec![SemanticSearchResultItem::new(1, 0.1, 0.95)], 10, 200),
            SemanticSearchResult::new(vec![SemanticSearchResultItem::new(2, 0.2, 0.90)], 10, 300),
        ];
        let batch = BatchSemanticSearchResult::new(results, 500);

        assert_eq!(batch.num_queries, 2);
        assert_eq!(batch.total_latency_us, 500);
        assert_eq!(batch.total_hits(), 2);
        assert!(batch.has_any_results());
        assert_eq!(batch.avg_latency_per_query(), 250);
    }

    #[test]
    fn test_batch_semantic_search_result_empty() {
        let batch = BatchSemanticSearchResult::empty();

        assert_eq!(batch.num_queries, 0);
        assert!(!batch.has_any_results());
        assert_eq!(batch.total_hits(), 0);
    }

    #[test]
    fn test_batch_semantic_search_result_get() {
        let results = vec![SemanticSearchResult::new(
            vec![SemanticSearchResultItem::new(1, 0.1, 0.95)],
            10,
            200,
        )];
        let batch = BatchSemanticSearchResult::new(results, 200);

        assert!(batch.get(0).is_some());
        assert!(batch.get(1).is_none());
    }
}
