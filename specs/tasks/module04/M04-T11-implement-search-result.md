# M04-T11: Implement SearchResult Struct

| Field | Value |
|-------|-------|
| **Task ID** | M04-T11 |
| **Module** | context-graph-graph |
| **Status** | Ready |
| **Priority** | P0 (Critical Path) |
| **Depends On** | M04-T10 (FaissGpuIndex - COMPLETE) |
| **Estimated Effort** | 2-3 hours |
| **Constitution Refs** | TECH-GRAPH-004 Section 3.2, perf.latency.faiss_1M_k100 |

---

## Executive Summary

Implement `SearchResult` and `SearchResultItem` structs to encapsulate FAISS k-NN search output. FAISS returns flat arrays of IDs and distances for all queries combined. This task provides:
- Structured result container with per-query slicing
- Automatic filtering of `-1` sentinel IDs (no match found)
- L2 distance to cosine similarity conversion
- Helper methods for common operations

**CRITICAL RULES**:
- **NO BACKWARDS COMPATIBILITY** - System must work correctly or fail fast
- **NO MOCK DATA IN TESTS** - All tests use real data and verify actual behavior
- **NEVER unwrap() IN PROD** - Use `expect()` only in tests

---

## Current Codebase State (Verified 2026-01-03)

### Completed Dependencies

| Task | Description | Status | File Location |
|------|-------------|--------|---------------|
| M04-T00 | Crate scaffold | ✅ Complete | `crates/context-graph-graph/` |
| M04-T01 | IndexConfig | ✅ Complete | `src/config.rs` |
| M04-T08 | GraphError | ✅ Complete | `src/error.rs` |
| M04-T09 | FAISS FFI | ✅ Complete | `src/index/faiss_ffi.rs` |
| M04-T10 | FaissGpuIndex | ✅ Complete | `src/index/gpu_index.rs` |

### Existing File Structure

```
crates/context-graph-graph/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Crate root with re-exports
│   ├── config.rs                 # IndexConfig, HyperbolicConfig, ConeConfig
│   ├── error.rs                  # GraphError enum (32 variants)
│   ├── index/
│   │   ├── mod.rs                # Module exports (currently exports faiss_ffi, gpu_index)
│   │   ├── faiss_ffi.rs          # Raw FFI bindings (COMPLETE)
│   │   └── gpu_index.rs          # FaissGpuIndex, GpuResources (COMPLETE)
│   ├── hyperbolic/
│   ├── entailment/
│   ├── storage/
│   ├── traversal/
│   ├── marblestone/
│   └── query/
```

### FaissGpuIndex::search() Return Type (from gpu_index.rs:393)

```rust
/// Returns Tuple of (distances, indices) where each has length n_queries * k.
/// Distances are L2 squared distances. Indices are -1 for unfilled slots.
pub fn search(&self, queries: &[f32], k: usize) -> GraphResult<(Vec<f32>, Vec<i64>)>
```

**Key Insight**: The `search()` method already exists and returns raw `(Vec<f32>, Vec<i64>)`. This task wraps that output in a structured `SearchResult` type.

---

## File to Create

| File | Purpose |
|------|---------|
| `src/index/search_result.rs` | SearchResult and SearchResultItem structs |

### Files to Modify

| File | Change |
|------|--------|
| `src/index/mod.rs` | Add `pub mod search_result;` and re-export types |
| `src/lib.rs` | Add re-export for `SearchResult`, `SearchResultItem` |

---

## Implementation Specification

### Target File: `crates/context-graph-graph/src/index/search_result.rs`

```rust
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
```

---

## Module Integration

### Update: `crates/context-graph-graph/src/index/mod.rs`

Current content (from reading the file):
```rust
pub mod faiss_ffi;
pub mod gpu_index;

pub use faiss_ffi::{check_faiss_result, MetricType};
pub use gpu_index::{FaissGpuIndex, GpuResources};

// TODO: M04-T11 - Implement SearchResult
// pub mod search_result;
// pub use search_result::SearchResult;
```

**Replace with:**
```rust
//! FAISS GPU index wrapper for vector similarity search.
//!
//! This module provides a Rust wrapper around FAISS GPU for efficient
//! similarity search on 1M+ vectors with <2ms latency target.
//!
//! # Architecture
//!
//! ```text
//! faiss_ffi.rs     - Low-level C FFI bindings (M04-T09)
//! gpu_index.rs     - High-level FaissGpuIndex wrapper (M04-T10)
//! search_result.rs - Search result types (M04-T11)
//! ```

pub mod faiss_ffi;
pub mod gpu_index;
pub mod search_result;

// Re-exports for convenience
pub use faiss_ffi::{check_faiss_result, MetricType};
pub use gpu_index::{FaissGpuIndex, GpuResources};
pub use search_result::{SearchResult, SearchResultItem};
```

### Update: `crates/context-graph-graph/src/lib.rs`

Add to crate root re-exports (after existing re-exports around line 53):

```rust
pub use index::{FaissGpuIndex, GpuResources, MetricType, SearchResult, SearchResultItem};
```

---

## Acceptance Criteria

### Functional Requirements

- [ ] `SearchResult::new(ids, distances, k, num_queries)` creates result
- [ ] `query_results(idx)` returns iterator of `(id, distance)` pairs
- [ ] `query_results()` automatically filters `-1` sentinel IDs
- [ ] `query_results(idx)` panics if `idx >= num_queries`
- [ ] `query_results_vec(idx)` returns collected Vec
- [ ] `num_valid_results(idx)` counts non-sentinel results
- [ ] `has_results(idx)` returns bool
- [ ] `total_valid_results()` counts across all queries
- [ ] `is_empty()` returns true if no valid results
- [ ] `top_result(idx)` returns first valid result or None
- [ ] `all_results()` iterates (query_idx, id, distance) triples
- [ ] `min_distance()` and `max_distance()` return Option<f32>
- [ ] `to_items(idx)` converts to `Vec<SearchResultItem>`
- [ ] `SearchResultItem::from_l2(id, distance)` converts L2 to similarity
- [ ] `SearchResultItem::from_similarity(id, similarity)` converts back
- [ ] `Clone`, `Debug` derived for both structs
- [ ] `PartialEq` derived for `SearchResultItem`
- [ ] `Default` implemented for `SearchResult`

### Code Quality

- [ ] No `unwrap()` or `expect()` in non-test code
- [ ] All public items have doc comments
- [ ] All tests pass with `cargo test -p context-graph-graph search_result`
- [ ] No clippy warnings with `cargo clippy -p context-graph-graph -- -D warnings`
- [ ] Compiles with `cargo build -p context-graph-graph`

---

## Full State Verification

### Source of Truth

The SearchResult is an in-memory data structure. The source of truth is:
1. The `SearchResult` instance created by the caller
2. The test assertions that verify behavior

### Execute & Inspect

After implementation, run these commands:

```bash
# Verify file exists
ls -la crates/context-graph-graph/src/index/search_result.rs

# Verify module export
grep "pub mod search_result" crates/context-graph-graph/src/index/mod.rs

# Verify re-exports
grep "SearchResult" crates/context-graph-graph/src/index/mod.rs
grep "SearchResult" crates/context-graph-graph/src/lib.rs

# Compile
cargo build -p context-graph-graph

# Run tests with output
cargo test -p context-graph-graph search_result -- --nocapture

# Clippy check
cargo clippy -p context-graph-graph -- -D warnings
```

### Boundary & Edge Case Audit

Manually simulate these edge cases and verify output:

| # | Edge Case | Input | Expected Output |
|---|-----------|-------|-----------------|
| 1 | All sentinels | `ids=[-1,-1,-1]` | `is_empty()=true`, `total_valid_results()=0` |
| 2 | Mixed sentinels | `ids=[1,-1,3]` | `query_results(0)` yields `[(1,_),(3,_)]` |
| 3 | k=0, num_queries=0 | Empty vecs | No panic, `is_empty()=true` |
| 4 | L2=0 (identical) | `from_l2(_, 0.0)` | `similarity=1.0` |
| 5 | L2=2 (orthogonal) | `from_l2(_, 2.0)` | `similarity=0.0` |
| 6 | L2=4 (opposite) | `from_l2(_, 4.0)` | `similarity=-1.0` |
| 7 | query_idx >= num | `query_results(5)` on 2 queries | Panic with clear message |

### Evidence of Success

After implementation, these must exist:

```
crates/context-graph-graph/src/index/search_result.rs   # Implementation
```

Test output must show:
```
running X tests
test index::search_result::tests::test_new_creates_valid_result ... ok
test index::search_result::tests::test_query_results_basic ... ok
test index::search_result::tests::test_filter_sentinel_ids ... ok
... (all tests pass)
test result: ok. X passed; 0 failed
```

---

## Sherlock-Holmes Verification Step

After implementation is complete, spawn a `sherlock-holmes` subagent:

```
Task: Forensic verification of M04-T11 SearchResult implementation

INVESTIGATION CHECKLIST:

1. FILE EXISTENCE
   □ Does crates/context-graph-graph/src/index/search_result.rs exist?
   □ Is "pub mod search_result" in src/index/mod.rs?
   □ Is "pub use search_result::{SearchResult, SearchResultItem}" in src/index/mod.rs?
   □ Is SearchResult re-exported in src/lib.rs?

2. STRUCT DEFINITIONS
   □ SearchResult has: ids: Vec<i64>, distances: Vec<f32>, k: usize, num_queries: usize
   □ SearchResultItem has: id: i64, distance: f32, similarity: f32
   □ SearchResult derives Clone, Debug
   □ SearchResultItem derives Clone, Debug, PartialEq
   □ SearchResult implements Default

3. METHOD SIGNATURES
   □ SearchResult::new(Vec<i64>, Vec<f32>, usize, usize) -> Self
   □ query_results(&self, usize) -> impl Iterator<Item = (i64, f32)>
   □ query_results_vec(&self, usize) -> Vec<(i64, f32)>
   □ num_valid_results(&self, usize) -> usize
   □ has_results(&self, usize) -> bool
   □ total_valid_results(&self) -> usize
   □ is_empty(&self) -> bool
   □ top_result(&self, usize) -> Option<(i64, f32)>
   □ all_results(&self) -> impl Iterator<Item = (usize, i64, f32)>
   □ min_distance(&self) -> Option<f32>
   □ max_distance(&self) -> Option<f32>
   □ to_items(&self, usize) -> Vec<SearchResultItem>
   □ SearchResultItem::from_l2(i64, f32) -> Self
   □ SearchResultItem::from_similarity(i64, f32) -> Self

4. SENTINEL FILTERING VERIFICATION
   □ Verify query_results() filters -1 IDs (check implementation)
   □ Verify filter closure is: .filter(|(&id, _)| id != -1)
   □ Run test_filter_sentinel_ids and verify output

5. PANIC CONDITIONS
   □ query_results() panics if query_idx >= num_queries
   □ Panic message includes actual values for debugging
   □ Run test_query_idx_out_of_bounds and verify panic

6. L2 TO SIMILARITY MATH
   □ Verify formula: similarity = 1.0 - (distance / 2.0)
   □ Run test_l2_to_similarity_zero_distance: d=0 -> sim=1
   □ Run test_l2_to_similarity_max_distance: d=2 -> sim=0
   □ Run test_l2_to_similarity_opposite: d=4 -> sim=-1

7. NO UNWRAP CHECK
   □ Search for "unwrap()" outside #[cfg(test)] - MUST find zero
   □ Search for "expect(" outside #[cfg(test)] - MUST find zero

8. COMPILATION & TESTS
   □ cargo build -p context-graph-graph succeeds
   □ cargo test -p context-graph-graph search_result succeeds
   □ cargo clippy -p context-graph-graph -- -D warnings clean

VERDICT FORMAT:
- [PASS] Item - evidence
- [FAIL] Item - what's wrong

FINAL VERDICT:
- INNOCENT: All checks pass
- GUILTY: List failures requiring correction
```

---

## Revision History

| Date | Author | Changes |
|------|--------|---------|
| 2026-01-03 | AI Agent | Complete rewrite with actual codebase audit, verified M04-T10 is complete, added all verification requirements |
