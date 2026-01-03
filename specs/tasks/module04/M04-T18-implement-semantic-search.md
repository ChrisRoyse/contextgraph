---
id: "M04-T18"
title: "Implement Semantic Search Operation"
description: |
  Implement semantic_search(query, k, filters) on KnowledgeGraph.
  Uses FAISS GPU index for initial k-NN retrieval.
  Applies SearchFilters: min_importance, johari_quadrants, created_after, agent_id.
  Returns Vec<SearchResult> with node, similarity, distance.
  Performance: <10ms for k=100 on 10M vectors.
layer: "surface"
status: "pending"
priority: "critical"
estimated_hours: 3
sequence: 26
depends_on:
  - "M04-T10"
  - "M04-T11"
spec_refs:
  - "TECH-GRAPH-004 Section 8"
  - "REQ-KG-060"
files_to_create:
  - path: "crates/context-graph-graph/src/search/mod.rs"
    description: "Search module with semantic search implementation"
  - path: "crates/context-graph-graph/src/search/filters.rs"
    description: "SearchFilters struct and filter logic"
files_to_modify:
  - path: "crates/context-graph-graph/src/lib.rs"
    description: "Add search module and semantic_search to KnowledgeGraph"
test_file: "crates/context-graph-graph/tests/search_tests.rs"
---

## Context

Semantic search is the primary retrieval mechanism for the Knowledge Graph. It leverages the FAISS GPU index (IVF-PQ) with nprobe=128 for high recall k-NN search over 1536-dimensional embedding vectors. Post-filtering applies business logic (importance, Johari quadrants, recency, agent ownership) to the candidates. The system must achieve <10ms latency for k=100 on 10M vectors to meet NFR-KG-060.

## Scope

### In Scope
- `semantic_search()` function on KnowledgeGraph
- SearchFilters struct with all filter fields
- L2 distance to cosine similarity conversion
- Post-filtering of FAISS results
- SemanticSearchResult struct with node, similarity, distance

### Out of Scope
- Domain-aware modulation (see M04-T19)
- Entailment-based search (see M04-T20)
- Hybrid search combining multiple strategies
- CUDA-accelerated post-filtering

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/search/mod.rs

use crate::error::{GraphError, GraphResult};
use crate::index::gpu_index::{FaissGpuIndex, SearchResult as FaissSearchResult};
use crate::storage::rocksdb::{GraphStorage, NodeId};
use crate::Vector1536;

pub mod filters;
pub use filters::SearchFilters;

/// Result from semantic search
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    /// Node identifier
    pub node_id: NodeId,

    /// Cosine similarity score [0, 1] (higher = more similar)
    pub similarity: f32,

    /// L2 distance from query (lower = more similar)
    pub distance: f32,

    /// Rank in result set (0 = best match)
    pub rank: usize,
}

impl SemanticSearchResult {
    /// Create from FAISS result with distance-to-similarity conversion
    pub fn from_faiss(node_id: NodeId, distance: f32, rank: usize) -> Self {
        // Convert L2 distance to cosine similarity
        // For normalized vectors: cos_sim = 1 - (d^2 / 2)
        let similarity = (1.0 - distance / 2.0).max(0.0).min(1.0);

        Self {
            node_id,
            similarity,
            distance,
            rank,
        }
    }
}

/// Semantic search over the knowledge graph
///
/// Uses FAISS GPU index for k-NN retrieval, then applies post-filters.
/// Performance target: <10ms for k=100 on 10M vectors.
///
/// # Arguments
/// * `index` - FAISS GPU index for vector search
/// * `storage` - Graph storage for node metadata
/// * `query` - Query embedding vector (1536D)
/// * `k` - Number of results to return
/// * `filters` - Optional post-filters to apply
///
/// # Returns
/// * Vector of search results, sorted by similarity descending
///
/// # Errors
/// * `GraphError::IndexNotTrained` if index not trained
/// * `GraphError::FaissSearchFailed` on FAISS error
///
/// # Example
/// ```rust
/// let query: Vector1536 = embedding_model.encode("find similar documents")?;
/// let results = semantic_search(
///     &index,
///     &storage,
///     &query,
///     10,
///     Some(SearchFilters::default().min_importance(0.5)),
/// )?;
/// for result in results {
///     println!("Node {} similarity: {:.3}", result.node_id, result.similarity);
/// }
/// ```
pub fn semantic_search(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    query: &Vector1536,
    k: usize,
    filters: Option<SearchFilters>,
) -> GraphResult<Vec<SemanticSearchResult>> {
    // Early return for empty index
    if index.ntotal() == 0 {
        return Ok(Vec::new());
    }

    // Check if index is trained
    if !index.is_trained() {
        return Err(GraphError::IndexNotTrained);
    }

    // Over-fetch if filters are present (3x candidates)
    let fetch_k = if filters.is_some() {
        (k * 3).min(index.ntotal() as usize)
    } else {
        k.min(index.ntotal() as usize)
    };

    // Perform FAISS search
    let faiss_result = index.search(&[query.clone()], fetch_k)?;

    // Convert to SemanticSearchResult
    let mut results: Vec<SemanticSearchResult> = faiss_result
        .query_results(0)
        .enumerate()
        .filter_map(|(rank, (id, distance))| {
            if id >= 0 {
                Some(SemanticSearchResult::from_faiss(id as NodeId, distance, rank))
            } else {
                None  // Filter out -1 sentinel values
            }
        })
        .collect();

    // Apply filters if present
    if let Some(ref filters) = filters {
        results = apply_filters(storage, results, filters)?;
    }

    // Re-rank and truncate to k
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    for (i, result) in results.iter_mut().enumerate() {
        result.rank = i;
    }
    results.truncate(k);

    Ok(results)
}

/// Apply post-filters to search results
fn apply_filters(
    storage: &GraphStorage,
    results: Vec<SemanticSearchResult>,
    filters: &SearchFilters,
) -> GraphResult<Vec<SemanticSearchResult>> {
    let mut filtered = Vec::with_capacity(results.len());

    for result in results {
        // Get node metadata for filtering
        let node = match storage.get_node(result.node_id)? {
            Some(n) => n,
            None => continue,  // Skip missing nodes
        };

        // Apply min_importance filter
        if let Some(min_imp) = filters.min_importance {
            if node.importance < min_imp {
                continue;
            }
        }

        // Apply johari_quadrants filter
        if let Some(ref quadrants) = filters.johari_quadrants {
            if !quadrants.contains(&node.johari_quadrant) {
                continue;
            }
        }

        // Apply created_after filter
        if let Some(after) = filters.created_after {
            if node.created_at < after {
                continue;
            }
        }

        // Apply agent_id filter
        if let Some(ref agent_id) = filters.agent_id {
            if node.agent_id.as_ref() != Some(agent_id) {
                continue;
            }
        }

        filtered.push(result);
    }

    Ok(filtered)
}

/// Batch semantic search for multiple queries
///
/// More efficient than multiple single searches due to batched GPU operations.
///
/// # Arguments
/// * `index` - FAISS GPU index
/// * `queries` - Vector of query embeddings
/// * `k` - Number of results per query
///
/// # Returns
/// * Vector of result vectors, one per query
pub fn semantic_search_batch(
    index: &FaissGpuIndex,
    queries: &[Vector1536],
    k: usize,
) -> GraphResult<Vec<Vec<SemanticSearchResult>>> {
    if queries.is_empty() || index.ntotal() == 0 {
        return Ok(vec![Vec::new(); queries.len()]);
    }

    if !index.is_trained() {
        return Err(GraphError::IndexNotTrained);
    }

    let fetch_k = k.min(index.ntotal() as usize);
    let faiss_results = index.search(queries, fetch_k)?;

    let mut all_results = Vec::with_capacity(queries.len());

    for query_idx in 0..faiss_results.num_queries {
        let results: Vec<SemanticSearchResult> = faiss_results
            .query_results(query_idx)
            .enumerate()
            .filter_map(|(rank, (id, distance))| {
                if id >= 0 {
                    Some(SemanticSearchResult::from_faiss(id as NodeId, distance, rank))
                } else {
                    None
                }
            })
            .collect();

        all_results.push(results);
    }

    Ok(all_results)
}
```

```rust
// In crates/context-graph-graph/src/search/filters.rs

use serde::{Deserialize, Serialize};

/// Johari quadrant for node visibility classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JohariQuadrant {
    /// Known to self and others
    Open,
    /// Known to self, hidden from others
    Hidden,
    /// Unknown to self, visible to others
    Blind,
    /// Unknown to self and others
    Unknown,
}

/// Filters for semantic search post-processing
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Minimum importance score [0, 1]
    pub min_importance: Option<f32>,

    /// Filter to specific Johari quadrants
    pub johari_quadrants: Option<Vec<JohariQuadrant>>,

    /// Only include nodes created after this timestamp (Unix epoch seconds)
    pub created_after: Option<u64>,

    /// Filter to nodes owned by specific agent
    pub agent_id: Option<String>,

    /// Minimum similarity score [0, 1]
    pub min_similarity: Option<f32>,

    /// Exclude specific node IDs
    pub exclude_nodes: Option<Vec<i64>>,
}

impl SearchFilters {
    /// Create empty filters (no filtering)
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set minimum importance
    pub fn min_importance(mut self, min: f32) -> Self {
        self.min_importance = Some(min.max(0.0).min(1.0));
        self
    }

    /// Builder: set Johari quadrant filter
    pub fn johari_quadrants(mut self, quadrants: Vec<JohariQuadrant>) -> Self {
        self.johari_quadrants = Some(quadrants);
        self
    }

    /// Builder: only Open quadrant
    pub fn open_only(self) -> Self {
        self.johari_quadrants(vec![JohariQuadrant::Open])
    }

    /// Builder: set created_after filter
    pub fn created_after(mut self, timestamp: u64) -> Self {
        self.created_after = Some(timestamp);
        self
    }

    /// Builder: set agent filter
    pub fn agent(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Builder: set minimum similarity
    pub fn min_similarity(mut self, min: f32) -> Self {
        self.min_similarity = Some(min.max(0.0).min(1.0));
        self
    }

    /// Builder: exclude specific nodes
    pub fn exclude(mut self, nodes: Vec<i64>) -> Self {
        self.exclude_nodes = Some(nodes);
        self
    }

    /// Check if any filters are active
    pub fn is_active(&self) -> bool {
        self.min_importance.is_some()
            || self.johari_quadrants.is_some()
            || self.created_after.is_some()
            || self.agent_id.is_some()
            || self.min_similarity.is_some()
            || self.exclude_nodes.is_some()
    }
}
```

### Constraints
- MUST use real FAISS GPU index (no mocks per REQ-KG-TEST)
- Performance: <10ms for k=100 on 10M vectors with nprobe=128
- Over-fetch 3x candidates when filters are present
- L2 to cosine: similarity = 1 - (d^2 / 2) for normalized vectors
- Filter out -1 sentinel values from FAISS results

### Acceptance Criteria
- [ ] semantic_search() calls FAISS search internally
- [ ] Converts L2 distance to cosine similarity
- [ ] Applies post-filters to results
- [ ] Returns empty vec if index not trained
- [ ] Performance meets <10ms target
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
semantic_search(index, storage, query, k, filters):
    if index.ntotal == 0:
        return []

    if not index.is_trained:
        return Error(IndexNotTrained)

    # Over-fetch if filtering
    fetch_k = k * 3 if filters else k
    fetch_k = min(fetch_k, index.ntotal)

    # GPU FAISS search
    faiss_results = index.search([query], fetch_k)

    # Convert to SemanticSearchResult
    results = []
    for rank, (id, distance) in faiss_results.query_results(0):
        if id >= 0:  # Filter sentinel
            similarity = 1.0 - (distance / 2.0)  # L2 to cosine
            results.append(SemanticSearchResult{id, similarity, distance, rank})

    # Apply post-filters
    if filters:
        results = apply_filters(storage, results, filters)

    # Re-rank and truncate
    results.sort_by(similarity, descending)
    for i, r in results:
        r.rank = i
    return results[:k]
```

### RTX 5090 Optimizations
- Compute Capability 12.0 for FAISS GPU operations
- Green Contexts for multi-stream search if available
- Consider FP16 for similarity comparisons in post-processing

### Edge Cases
- Empty index: Return empty vector
- Index not trained: Return IndexNotTrained error
- All results filtered out: Return empty vector
- k > ntotal: Return all available results
- -1 sentinel IDs: Filter out (no match found for that slot)

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph semantic_search
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Search returns k results in <10ms
- [ ] Similarity scores in [0, 1]
- [ ] Filters correctly applied
- [ ] Results sorted by similarity descending

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // NOTE: These tests require real FAISS GPU index per REQ-KG-TEST
    // Skip on CI without GPU using #[requires_gpu]

    #[test]
    #[requires_gpu]
    fn test_semantic_search_basic() {
        let dir = tempdir().unwrap();
        let config = IndexConfig::default();
        let mut index = FaissGpuIndex::new(&config).unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        // Generate training data (min 4M vectors for IVF16384)
        let train_data: Vec<Vector1536> = (0..4_194_304)
            .map(|_| random_normalized_vector())
            .collect();
        index.train(&train_data).unwrap();

        // Add test vectors
        let test_vectors: Vec<Vector1536> = (0..1000)
            .map(|_| random_normalized_vector())
            .collect();
        let ids: Vec<i64> = (0..1000).collect();
        index.add_with_ids(&test_vectors, &ids).unwrap();

        // Search
        let query = &test_vectors[0];
        let results = semantic_search(&index, &storage, query, 10, None).unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 10);
        assert_eq!(results[0].node_id, 0);  // Self should be most similar
        assert!(results[0].similarity > 0.99);  // Near-identical
    }

    #[test]
    fn test_search_filters_builder() {
        let filters = SearchFilters::new()
            .min_importance(0.5)
            .open_only()
            .agent("agent-001".to_string());

        assert_eq!(filters.min_importance, Some(0.5));
        assert!(filters.johari_quadrants.is_some());
        assert_eq!(filters.agent_id, Some("agent-001".to_string()));
        assert!(filters.is_active());
    }

    #[test]
    fn test_semantic_search_result_conversion() {
        // L2 distance 0 = cosine similarity 1
        let result = SemanticSearchResult::from_faiss(42, 0.0, 0);
        assert!((result.similarity - 1.0).abs() < 1e-6);

        // L2 distance 2 = cosine similarity 0 (orthogonal for normalized)
        let result = SemanticSearchResult::from_faiss(42, 2.0, 0);
        assert!((result.similarity - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_empty_index_returns_empty() {
        let config = IndexConfig::default();
        let index = FaissGpuIndex::new(&config).unwrap();
        // Don't train - index is empty

        let query = Vector1536::default();
        let result = semantic_search(&index, &mock_storage(), &query, 10, None);

        // Should return empty, not error
        assert!(result.unwrap().is_empty());
    }
}

fn random_normalized_vector() -> Vector1536 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut v: [f32; 1536] = [0.0; 1536];
    let mut norm = 0.0f32;
    for x in v.iter_mut() {
        *x = rng.gen_range(-1.0..1.0);
        norm += *x * *x;
    }
    norm = norm.sqrt();
    for x in v.iter_mut() {
        *x /= norm;
    }
    Vector1536::from(v)
}
```
