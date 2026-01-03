//! FAISS Search Result Types
//!
//! Provides structured wrappers for FAISS k-NN search output with:
//! - Per-query result slicing from flat arrays
//! - Automatic -1 sentinel ID filtering
//! - L2 distance to cosine similarity conversion
//! - Helper methods for common operations
//!
//! # Constitution References
//!
//! - TECH-GRAPH-004 Section 3.2: SearchResult specification
//! - perf.latency.faiss_1M_k100: <2ms target
//!
//! # FAISS Return Format
//!
//! FAISS search returns flat arrays:
//! - `ids`: [q0_r0, q0_r1, ..., q0_rk-1, q1_r0, q1_r1, ..., qn_rk-1]
//! - `distances`: Same layout, L2 squared distances
//! - `-1` sentinel indicates fewer than k matches found for that position

use std::cmp::Ordering;

/// Result from FAISS k-NN search.
///
/// Encapsulates the raw output from `FaissGpuIndex::search()` with methods
/// for extracting per-query results and filtering sentinel IDs.
///
/// # Memory Layout
///
/// FAISS returns flat arrays where results for query `i` start at index `i * k`.
/// For n queries with k neighbors each:
/// - `ids.len() == n * k`
/// - `distances.len() == n * k`
///
/// # Sentinel Handling
///
/// FAISS uses `-1` to indicate "no match found" for a given position.
/// This happens when:
/// - The index has fewer than k vectors
/// - Some IVF cells have fewer than nprobe neighbors
///
/// All `query_results*` methods automatically filter out `-1` sentinels.
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// Vector IDs for all queries (flattened, k per query)
    /// -1 indicates no match found for that position
    pub ids: Vec<i64>,
    /// Distances for all queries (flattened, k per query)
    /// L2 squared distance: lower = more similar
    pub distances: Vec<f32>,
    /// Number of neighbors requested per query
    pub k: usize,
    /// Number of queries in this result
    pub num_queries: usize,
}

impl SearchResult {
    /// Create a new SearchResult from raw FAISS output.
    ///
    /// # Arguments
    ///
    /// * `ids` - Vector IDs (flattened, k per query)
    /// * `distances` - L2 squared distances (flattened, k per query)
    /// * `k` - Number of neighbors per query
    /// * `num_queries` - Number of queries
    ///
    /// # Panics (debug only)
    ///
    /// Debug assertions verify array lengths match `k * num_queries`.
    #[inline]
    pub fn new(ids: Vec<i64>, distances: Vec<f32>, k: usize, num_queries: usize) -> Self {
        debug_assert_eq!(
            ids.len(),
            k * num_queries,
            "ids.len() ({}) != k ({}) * num_queries ({})",
            ids.len(),
            k,
            num_queries
        );
        debug_assert_eq!(
            distances.len(),
            k * num_queries,
            "distances.len() ({}) != k ({}) * num_queries ({})",
            distances.len(),
            k,
            num_queries
        );
        Self { ids, distances, k, num_queries }
    }

    /// Get results for a specific query index as an iterator.
    ///
    /// Returns (id, distance) pairs, automatically filtering out -1 sentinel IDs.
    ///
    /// # Arguments
    ///
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `query_idx >= num_queries`.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::index::search_result::SearchResult;
    ///
    /// let result = SearchResult::new(
    ///     vec![1, 2, -1, 4, 5, 6],
    ///     vec![0.1, 0.2, 0.0, 0.4, 0.5, 0.6],
    ///     3, 2,
    /// );
    ///
    /// // Query 0: IDs 1, 2 (sentinel -1 filtered)
    /// let q0: Vec<_> = result.query_results(0).collect();
    /// assert_eq!(q0, vec![(1, 0.1), (2, 0.2)]);
    ///
    /// // Query 1: IDs 4, 5, 6
    /// let q1: Vec<_> = result.query_results(1).collect();
    /// assert_eq!(q1, vec![(4, 0.4), (5, 0.5), (6, 0.6)]);
    /// ```
    pub fn query_results(&self, query_idx: usize) -> impl Iterator<Item = (i64, f32)> + '_ {
        assert!(
            query_idx < self.num_queries,
            "query_idx ({}) >= num_queries ({})",
            query_idx,
            self.num_queries
        );

        let start = query_idx * self.k;
        let end = start + self.k;

        self.ids[start..end]
            .iter()
            .zip(&self.distances[start..end])
            .filter(|(&id, _)| id != -1)
            .map(|(&id, &dist)| (id, dist))
    }

    /// Get results for a specific query as a collected Vec.
    ///
    /// Convenience method that collects `query_results()` into a Vec.
    ///
    /// # Arguments
    ///
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `query_idx >= num_queries`.
    #[inline]
    pub fn query_results_vec(&self, query_idx: usize) -> Vec<(i64, f32)> {
        self.query_results(query_idx).collect()
    }

    /// Get the number of valid results for a query (excluding -1 sentinels).
    ///
    /// # Arguments
    ///
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `query_idx >= num_queries`.
    #[inline]
    pub fn num_valid_results(&self, query_idx: usize) -> usize {
        self.query_results(query_idx).count()
    }

    /// Check if any results were found for a query.
    ///
    /// # Arguments
    ///
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `query_idx >= num_queries`.
    #[inline]
    pub fn has_results(&self, query_idx: usize) -> bool {
        self.query_results(query_idx).next().is_some()
    }

    /// Total number of valid results across all queries.
    ///
    /// Counts all IDs that are not -1 sentinel values.
    #[inline]
    pub fn total_valid_results(&self) -> usize {
        self.ids.iter().filter(|&&id| id != -1).count()
    }

    /// Check if result is empty (no valid matches for any query).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.total_valid_results() == 0
    }

    /// Get the top-1 result for a query if available.
    ///
    /// Returns the first valid (non-sentinel) result for the query.
    ///
    /// # Arguments
    ///
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `query_idx >= num_queries`.
    #[inline]
    pub fn top_result(&self, query_idx: usize) -> Option<(i64, f32)> {
        self.query_results(query_idx).next()
    }

    /// Get all results as iterator of (query_idx, id, distance) triples.
    ///
    /// Iterates through all queries, yielding valid results with query index.
    pub fn all_results(&self) -> impl Iterator<Item = (usize, i64, f32)> + '_ {
        (0..self.num_queries)
            .flat_map(move |q| {
                self.query_results(q).map(move |(id, dist)| (q, id, dist))
            })
    }

    /// Get the minimum distance found across all queries.
    ///
    /// Returns `None` if no valid results exist.
    pub fn min_distance(&self) -> Option<f32> {
        self.ids
            .iter()
            .zip(&self.distances)
            .filter(|(&id, _)| id != -1)
            .map(|(_, &dist)| dist)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
    }

    /// Get the maximum distance found across all queries.
    ///
    /// Returns `None` if no valid results exist.
    pub fn max_distance(&self) -> Option<f32> {
        self.ids
            .iter()
            .zip(&self.distances)
            .filter(|(&id, _)| id != -1)
            .map(|(_, &dist)| dist)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
    }

    /// Convert to SearchResultItems for a single query.
    ///
    /// Creates `SearchResultItem` instances with L2 to cosine conversion.
    ///
    /// # Arguments
    ///
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `query_idx >= num_queries`.
    pub fn to_items(&self, query_idx: usize) -> Vec<SearchResultItem> {
        self.query_results(query_idx)
            .map(|(id, dist)| SearchResultItem::from_l2(id, dist))
            .collect()
    }

    /// Get the number of queries in this result.
    #[inline]
    pub fn len(&self) -> usize {
        self.num_queries
    }

    /// Get the k value (neighbors per query).
    #[inline]
    pub fn k(&self) -> usize {
        self.k
    }
}

impl Default for SearchResult {
    fn default() -> Self {
        Self {
            ids: Vec::new(),
            distances: Vec::new(),
            k: 0,
            num_queries: 0,
        }
    }
}

/// Single search result item with additional metadata.
///
/// Provides both L2 distance and cosine similarity for convenience.
/// Useful when downstream code needs similarity scores.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResultItem {
    /// Vector ID from the index
    pub id: i64,
    /// L2 distance from query (lower = more similar)
    pub distance: f32,
    /// Cosine similarity (derived from L2 for normalized vectors)
    /// Higher = more similar, range [-1, 1] for normalized vectors
    pub similarity: f32,
}

impl SearchResultItem {
    /// Create from ID and L2 distance.
    ///
    /// Converts L2 distance to cosine similarity assuming normalized vectors.
    ///
    /// # Math
    ///
    /// For normalized vectors (||a|| = ||b|| = 1):
    /// - L2 distance: d = ||a - b|| = sqrt(2 - 2*cos(θ))
    /// - Therefore: d² = 2 - 2*cos(θ)
    /// - Solving: cos(θ) = 1 - d²/2
    ///
    /// # Arguments
    ///
    /// * `id` - Vector ID
    /// * `distance` - L2 distance (NOT squared - FAISS returns squared L2)
    ///
    /// # Note
    ///
    /// FAISS IVF-PQ with L2 metric returns squared L2 distances.
    /// The input `distance` should be the raw FAISS output.
    #[inline]
    pub fn from_l2(id: i64, distance: f32) -> Self {
        // FAISS returns squared L2 distance for efficiency
        // For normalized vectors: d² = 2(1 - cos(θ))
        // Therefore: similarity = 1 - d²/2
        let similarity = 1.0 - (distance / 2.0);
        Self { id, distance, similarity }
    }

    /// Create from ID and cosine similarity.
    ///
    /// Computes L2 distance from similarity assuming normalized vectors.
    ///
    /// # Arguments
    ///
    /// * `id` - Vector ID
    /// * `similarity` - Cosine similarity in range [-1, 1]
    #[inline]
    pub fn from_similarity(id: i64, similarity: f32) -> Self {
        // d² = 2(1 - cos(θ))
        let distance = 2.0 * (1.0 - similarity);
        Self { id, distance, similarity }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== SearchResult Basic Tests ==========

    #[test]
    fn test_new_creates_valid_result() {
        let result = SearchResult::new(
            vec![1, 2, 3, 4, 5, 6],
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            3, 2,
        );

        assert_eq!(result.k, 3);
        assert_eq!(result.num_queries, 2);
        assert_eq!(result.ids.len(), 6);
        assert_eq!(result.distances.len(), 6);
    }

    #[test]
    fn test_query_results_basic() {
        let result = SearchResult::new(
            vec![1, 2, 3, 4, 5, 6],
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            3, 2,
        );

        let q0: Vec<_> = result.query_results(0).collect();
        assert_eq!(q0, vec![(1, 0.1), (2, 0.2), (3, 0.3)]);

        let q1: Vec<_> = result.query_results(1).collect();
        assert_eq!(q1, vec![(4, 0.4), (5, 0.5), (6, 0.6)]);
    }

    #[test]
    fn test_query_results_vec() {
        let result = SearchResult::new(
            vec![10, 20, 30],
            vec![1.0, 2.0, 3.0],
            3, 1,
        );

        let items = result.query_results_vec(0);
        assert_eq!(items, vec![(10, 1.0), (20, 2.0), (30, 3.0)]);
    }

    // ========== Sentinel Filtering Tests ==========

    #[test]
    fn test_filter_sentinel_ids() {
        let result = SearchResult::new(
            vec![1, -1, 3, -1, -1, -1],
            vec![0.1, 0.0, 0.3, 0.0, 0.0, 0.0],
            3, 2,
        );

        let q0: Vec<_> = result.query_results(0).collect();
        assert_eq!(q0, vec![(1, 0.1), (3, 0.3)]);

        let q1: Vec<_> = result.query_results(1).collect();
        assert!(q1.is_empty(), "All -1 sentinels should be filtered");
    }

    #[test]
    fn test_all_sentinels_returns_empty() {
        let result = SearchResult::new(
            vec![-1, -1, -1],
            vec![0.0, 0.0, 0.0],
            3, 1,
        );

        assert!(result.query_results(0).next().is_none());
        assert!(!result.has_results(0));
        assert!(result.is_empty());
    }

    #[test]
    fn test_partial_sentinels() {
        let result = SearchResult::new(
            vec![100, -1, -1],
            vec![0.5, 0.0, 0.0],
            3, 1,
        );

        let q: Vec<_> = result.query_results(0).collect();
        assert_eq!(q, vec![(100, 0.5)]);
        assert_eq!(result.num_valid_results(0), 1);
        assert!(result.has_results(0));
    }

    // ========== Count and Check Methods ==========

    #[test]
    fn test_num_valid_results() {
        let result = SearchResult::new(
            vec![1, -1, 3],
            vec![0.1, 0.0, 0.3],
            3, 1,
        );

        assert_eq!(result.num_valid_results(0), 2);
        assert_eq!(result.total_valid_results(), 2);
    }

    #[test]
    fn test_total_valid_results_multiple_queries() {
        let result = SearchResult::new(
            vec![1, 2, -1, 4, -1, 6],  // 4 valid across 2 queries
            vec![0.1, 0.2, 0.0, 0.4, 0.0, 0.6],
            3, 2,
        );

        assert_eq!(result.total_valid_results(), 4);
        assert_eq!(result.num_valid_results(0), 2);  // 1, 2
        assert_eq!(result.num_valid_results(1), 2);  // 4, 6
    }

    #[test]
    fn test_has_results() {
        let result = SearchResult::new(
            vec![1, 2, 3, -1, -1, -1],
            vec![0.1, 0.2, 0.3, 0.0, 0.0, 0.0],
            3, 2,
        );

        assert!(result.has_results(0));
        assert!(!result.has_results(1));
    }

    #[test]
    fn test_is_empty() {
        let empty_result = SearchResult::new(
            vec![-1, -1, -1],
            vec![0.0, 0.0, 0.0],
            3, 1,
        );
        assert!(empty_result.is_empty());

        let non_empty = SearchResult::new(
            vec![1, -1, -1],
            vec![0.1, 0.0, 0.0],
            3, 1,
        );
        assert!(!non_empty.is_empty());
    }

    // ========== Top Result Tests ==========

    #[test]
    fn test_top_result_exists() {
        let result = SearchResult::new(
            vec![42, 43, 44],
            vec![0.5, 0.6, 0.7],
            3, 1,
        );

        let top = result.top_result(0);
        assert_eq!(top, Some((42, 0.5)));
    }

    #[test]
    fn test_top_result_skips_sentinels() {
        let result = SearchResult::new(
            vec![-1, 42, 43],  // First is sentinel
            vec![0.0, 0.5, 0.6],
            3, 1,
        );

        let top = result.top_result(0);
        assert_eq!(top, Some((42, 0.5)));
    }

    #[test]
    fn test_top_result_none_when_all_sentinels() {
        let result = SearchResult::new(
            vec![-1, -1, -1],
            vec![0.0, 0.0, 0.0],
            3, 1,
        );

        assert!(result.top_result(0).is_none());
    }

    // ========== All Results Iterator ==========

    #[test]
    fn test_all_results_iterator() {
        let result = SearchResult::new(
            vec![1, 2, 3, 4],
            vec![0.1, 0.2, 0.3, 0.4],
            2, 2,
        );

        let all: Vec<_> = result.all_results().collect();
        assert_eq!(all, vec![
            (0, 1, 0.1), (0, 2, 0.2),  // Query 0
            (1, 3, 0.3), (1, 4, 0.4),  // Query 1
        ]);
    }

    #[test]
    fn test_all_results_filters_sentinels() {
        let result = SearchResult::new(
            vec![1, -1, 3, -1],
            vec![0.1, 0.0, 0.3, 0.0],
            2, 2,
        );

        let all: Vec<_> = result.all_results().collect();
        assert_eq!(all, vec![
            (0, 1, 0.1),  // Query 0: only ID 1
            (1, 3, 0.3),  // Query 1: only ID 3
        ]);
    }

    // ========== Min/Max Distance ==========

    #[test]
    fn test_min_distance() {
        let result = SearchResult::new(
            vec![1, 2, 3],
            vec![0.5, 0.1, 0.8],
            3, 1,
        );

        assert_eq!(result.min_distance(), Some(0.1));
    }

    #[test]
    fn test_max_distance() {
        let result = SearchResult::new(
            vec![1, 2, 3],
            vec![0.5, 0.1, 0.8],
            3, 1,
        );

        assert_eq!(result.max_distance(), Some(0.8));
    }

    #[test]
    fn test_min_max_ignores_sentinels() {
        let result = SearchResult::new(
            vec![1, -1, 3],
            vec![0.5, 999.0, 0.8],  // 999.0 should be ignored
            3, 1,
        );

        assert_eq!(result.min_distance(), Some(0.5));
        assert_eq!(result.max_distance(), Some(0.8));
    }

    #[test]
    fn test_min_max_empty_result() {
        let result = SearchResult::new(
            vec![-1, -1, -1],
            vec![0.0, 0.0, 0.0],
            3, 1,
        );

        assert!(result.min_distance().is_none());
        assert!(result.max_distance().is_none());
    }

    // ========== L2 to Similarity Conversion ==========

    #[test]
    fn test_l2_to_similarity_zero_distance() {
        // Zero L2 distance = identical vectors = similarity 1.0
        let item = SearchResultItem::from_l2(1, 0.0);
        assert!((item.similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_to_similarity_max_distance() {
        // L2² = 2 for orthogonal normalized vectors -> similarity = 0
        let item = SearchResultItem::from_l2(1, 2.0);
        assert!((item.similarity - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_to_similarity_opposite() {
        // L2² = 4 for opposite normalized vectors -> similarity = -1
        let item = SearchResultItem::from_l2(1, 4.0);
        assert!((item.similarity - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_from_similarity_roundtrip() {
        let original_sim = 0.75;
        let item = SearchResultItem::from_similarity(42, original_sim);

        assert_eq!(item.id, 42);
        assert!((item.similarity - original_sim).abs() < 1e-6);

        // Verify distance conversion
        // d² = 2(1 - 0.75) = 0.5
        assert!((item.distance - 0.5).abs() < 1e-6);
    }

    // ========== to_items Conversion ==========

    #[test]
    fn test_to_items() {
        let result = SearchResult::new(
            vec![10, 20, -1],
            vec![0.0, 2.0, 0.0],  // 0.0 = sim 1.0, 2.0 = sim 0.0
            3, 1,
        );

        let items = result.to_items(0);
        assert_eq!(items.len(), 2);  // -1 filtered

        assert_eq!(items[0].id, 10);
        assert!((items[0].similarity - 1.0).abs() < 1e-6);

        assert_eq!(items[1].id, 20);
        assert!((items[1].similarity - 0.0).abs() < 1e-6);
    }

    // ========== Edge Cases ==========

    #[test]
    fn test_k_zero() {
        let result = SearchResult::new(Vec::new(), Vec::new(), 0, 0);

        assert!(result.is_empty());
        assert_eq!(result.total_valid_results(), 0);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_single_query_single_result() {
        let result = SearchResult::new(vec![42], vec![0.123], 1, 1);

        let q: Vec<_> = result.query_results(0).collect();
        assert_eq!(q, vec![(42, 0.123)]);
    }

    #[test]
    fn test_default() {
        let result = SearchResult::default();
        assert!(result.is_empty());
        assert_eq!(result.k, 0);
        assert_eq!(result.num_queries, 0);
    }

    #[test]
    fn test_len_and_k() {
        let result = SearchResult::new(
            vec![1, 2, 3, 4, 5, 6],
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            3, 2,
        );

        assert_eq!(result.len(), 2);  // num_queries
        assert_eq!(result.k(), 3);
    }

    // ========== Panic Tests ==========

    #[test]
    #[should_panic(expected = "query_idx (1) >= num_queries (1)")]
    fn test_query_idx_out_of_bounds() {
        let result = SearchResult::new(vec![1, 2], vec![0.1, 0.2], 2, 1);
        let _ = result.query_results(1).collect::<Vec<_>>();
    }

    #[test]
    #[should_panic(expected = "query_idx (5) >= num_queries (2)")]
    fn test_query_idx_way_out_of_bounds() {
        let result = SearchResult::new(
            vec![1, 2, 3, 4],
            vec![0.1, 0.2, 0.3, 0.4],
            2, 2,
        );
        let _ = result.query_results(5).collect::<Vec<_>>();
    }

    // ========== SearchResultItem Equality ==========

    #[test]
    fn test_search_result_item_equality() {
        let a = SearchResultItem::from_l2(42, 0.5);
        let b = SearchResultItem::from_l2(42, 0.5);
        assert_eq!(a, b);

        let c = SearchResultItem::from_l2(42, 0.6);
        assert_ne!(a, c);
    }

    #[test]
    fn test_search_result_item_clone() {
        let item = SearchResultItem::from_l2(99, 1.5);
        let cloned = item.clone();
        assert_eq!(item, cloned);
    }

    // ========== Clone and Debug ==========

    #[test]
    fn test_search_result_clone() {
        let result = SearchResult::new(
            vec![1, 2, 3],
            vec![0.1, 0.2, 0.3],
            3, 1,
        );
        let cloned = result.clone();

        assert_eq!(cloned.ids, result.ids);
        assert_eq!(cloned.distances, result.distances);
        assert_eq!(cloned.k, result.k);
        assert_eq!(cloned.num_queries, result.num_queries);
    }

    #[test]
    fn test_search_result_debug() {
        let result = SearchResult::new(vec![1], vec![0.1], 1, 1);
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("SearchResult"));
        assert!(debug_str.contains("ids"));
    }
}
