---
id: "M04-T11"
title: "Implement SearchResult Struct"
description: |
  Implement SearchResult struct for FAISS query results.
  Fields: ids (Vec<i64>), distances (Vec<f32>), k (usize), num_queries (usize).
  Include query_results(idx) iterator method for extracting per-query results.
  Handle -1 sentinel IDs (no match found).
layer: "logic"
status: "pending"
priority: "high"
estimated_hours: 1.5
sequence: 15
depends_on:
  - "M04-T10"
spec_refs:
  - "TECH-GRAPH-004 Section 3.2"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/index/gpu_index.rs"
    description: "Add SearchResult struct and associated methods"
test_file: "crates/context-graph-graph/tests/search_result_tests.rs"
---

## Context

SearchResult encapsulates the output of FAISS k-NN search operations. FAISS returns flat arrays of IDs and distances for all queries combined, requiring slicing to extract per-query results. The -1 sentinel value indicates fewer than k matches were found for a query, which must be filtered out for consumer code.

## Scope

### In Scope
- SearchResult struct with ids, distances, k, num_queries
- query_results(idx) method returning iterator of (id, distance) pairs
- Filtering of -1 sentinel IDs
- Clone, Debug traits
- Helper methods for common operations

### Out of Scope
- FAISS search execution (see M04-T10)
- Post-processing like re-ranking (see M04-T19)
- Conversion to domain types (see M04-T18)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/index/gpu_index.rs (after FaissGpuIndex)

/// Result from FAISS k-NN search
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// Vector IDs for all queries (flattened, k per query)
    /// -1 indicates no match found for that position
    pub ids: Vec<i64>,
    /// Distances for all queries (flattened, k per query)
    /// L2 distance: lower = more similar
    pub distances: Vec<f32>,
    /// Number of neighbors requested per query
    pub k: usize,
    /// Number of queries in this result
    pub num_queries: usize,
}

impl SearchResult {
    /// Create a new SearchResult
    pub fn new(ids: Vec<i64>, distances: Vec<f32>, k: usize, num_queries: usize) -> Self {
        debug_assert_eq!(ids.len(), k * num_queries);
        debug_assert_eq!(distances.len(), k * num_queries);
        Self { ids, distances, k, num_queries }
    }

    /// Get results for a specific query index
    ///
    /// # Arguments
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Returns
    /// Iterator of (id, distance) pairs, filtering out -1 sentinel IDs
    ///
    /// # Panics
    /// Panics if query_idx >= num_queries
    pub fn query_results(&self, query_idx: usize) -> impl Iterator<Item = (i64, f32)> + '_ {
        assert!(query_idx < self.num_queries, "query_idx out of bounds");

        let start = query_idx * self.k;
        let end = start + self.k;

        self.ids[start..end]
            .iter()
            .zip(&self.distances[start..end])
            .filter(|(&id, _)| id != -1)
            .map(|(&id, &dist)| (id, dist))
    }

    /// Get results for a specific query as a collected Vec
    ///
    /// # Arguments
    /// * `query_idx` - Index of the query (0-based)
    ///
    /// # Returns
    /// Vec of (id, distance) pairs, excluding -1 sentinels
    pub fn query_results_vec(&self, query_idx: usize) -> Vec<(i64, f32)> {
        self.query_results(query_idx).collect()
    }

    /// Get the number of valid results for a query (excluding -1 sentinels)
    pub fn num_valid_results(&self, query_idx: usize) -> usize {
        self.query_results(query_idx).count()
    }

    /// Check if any results were found for a query
    pub fn has_results(&self, query_idx: usize) -> bool {
        self.num_valid_results(query_idx) > 0
    }

    /// Total number of valid results across all queries
    pub fn total_valid_results(&self) -> usize {
        self.ids.iter().filter(|&&id| id != -1).count()
    }

    /// Check if result is empty (no valid matches for any query)
    pub fn is_empty(&self) -> bool {
        self.total_valid_results() == 0
    }

    /// Get the top-1 result for a query if available
    pub fn top_result(&self, query_idx: usize) -> Option<(i64, f32)> {
        self.query_results(query_idx).next()
    }

    /// Get all results as iterator of (query_idx, id, distance) triples
    pub fn all_results(&self) -> impl Iterator<Item = (usize, i64, f32)> + '_ {
        (0..self.num_queries)
            .flat_map(move |q| {
                self.query_results(q).map(move |(id, dist)| (q, id, dist))
            })
    }

    /// Get the minimum distance found across all queries
    pub fn min_distance(&self) -> Option<f32> {
        self.ids
            .iter()
            .zip(&self.distances)
            .filter(|(&id, _)| id != -1)
            .map(|(_, &dist)| dist)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get the maximum distance found across all queries
    pub fn max_distance(&self) -> Option<f32> {
        self.ids
            .iter()
            .zip(&self.distances)
            .filter(|(&id, _)| id != -1)
            .map(|(_, &dist)| dist)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
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

/// Single search result item with additional metadata
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResultItem {
    /// Vector ID
    pub id: i64,
    /// L2 distance from query
    pub distance: f32,
    /// Cosine similarity (derived from L2 for normalized vectors)
    pub similarity: f32,
}

impl SearchResultItem {
    /// Create from ID and L2 distance
    ///
    /// Converts L2 distance to cosine similarity assuming normalized vectors:
    /// similarity = 1 - (distance^2 / 2)
    pub fn from_l2(id: i64, distance: f32) -> Self {
        let similarity = 1.0 - (distance * distance / 2.0);
        Self { id, distance, similarity }
    }

    /// Create from ID and cosine similarity
    pub fn from_similarity(id: i64, similarity: f32) -> Self {
        let distance = (2.0 * (1.0 - similarity)).sqrt();
        Self { id, distance, similarity }
    }
}

impl SearchResult {
    /// Convert to SearchResultItems for a single query
    pub fn to_items(&self, query_idx: usize) -> Vec<SearchResultItem> {
        self.query_results(query_idx)
            .map(|(id, dist)| SearchResultItem::from_l2(id, dist))
            .collect()
    }
}
```

### Constraints
- -1 sentinel IDs MUST be filtered from query_results()
- query_results() MUST panic if query_idx >= num_queries
- L2 to cosine conversion assumes normalized vectors
- Must handle empty results gracefully

### Acceptance Criteria
- [ ] SearchResult struct with 4 fields
- [ ] query_results(idx) returns iterator of (id, distance) pairs
- [ ] Filters out -1 sentinel values
- [ ] Clone, Debug implemented
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. query_results(idx):
   - Calculate start = idx * k, end = start + k
   - Slice ids and distances
   - Zip together
   - Filter where id != -1
   - Map to (id, distance) tuples

2. L2 to cosine conversion:
   - For normalized vectors: ||a - b||^2 = 2 - 2*cos(a,b)
   - cos(a,b) = 1 - ||a - b||^2 / 2

### Edge Cases
- All -1 sentinels: Return empty iterator
- Zero queries: Return empty
- k=0: Return empty
- NaN distances: Handle via partial_cmp

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph search_result
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] query_results filters -1 correctly
- [ ] Iteration order preserved
- [ ] L2 to similarity conversion accurate

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_filter_sentinel() {
        let result = SearchResult::new(
            vec![1, -1, 3, -1, -1, -1],
            vec![0.1, 0.0, 0.3, 0.0, 0.0, 0.0],
            3, 2,
        );

        let q0: Vec<_> = result.query_results(0).collect();
        assert_eq!(q0, vec![(1, 0.1), (3, 0.3)]);

        let q1: Vec<_> = result.query_results(1).collect();
        assert!(q1.is_empty());
    }

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
    fn test_l2_to_similarity() {
        let item = SearchResultItem::from_l2(1, 0.0);
        assert!((item.similarity - 1.0).abs() < 1e-6);

        let item = SearchResultItem::from_l2(1, 1.414);
        assert!((item.similarity - 0.0).abs() < 0.01);
    }

    #[test]
    #[should_panic]
    fn test_query_idx_out_of_bounds() {
        let result = SearchResult::new(vec![1, 2], vec![0.1, 0.2], 2, 1);
        let _ = result.query_results(1).collect::<Vec<_>>();
    }
}
```
