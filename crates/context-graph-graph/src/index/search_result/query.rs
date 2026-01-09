//! Query access methods for SearchResult.
//!
//! Methods for extracting per-query results and filtering sentinels.

use super::types::{SearchResult, SearchResultItem};

impl SearchResult {
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
            .flat_map(move |q| self.query_results(q).map(move |(id, dist)| (q, id, dist)))
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
}
