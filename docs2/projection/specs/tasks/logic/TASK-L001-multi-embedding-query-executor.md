# TASK-L001: Multi-Embedding Query Executor

```yaml
metadata:
  id: "TASK-L001"
  title: "Multi-Embedding Query Executor"
  layer: "logic"
  priority: "P0"
  estimated_hours: 8
  created: "2026-01-04"
  status: "pending"
  dependencies:
    - "TASK-F001"  # SemanticFingerprint struct
    - "TASK-F005"  # HNSW indexes
    - "TASK-F007"  # EmbeddingProvider trait
  spec_refs:
    - "projectionplan1.md:retrieval-pipeline"
    - "projectionplan2.md:multi-embedding-search"
```

## Problem Statement

Implement a query executor that searches across all 13 embedding spaces in parallel, aggregating results with configurable weighting strategies and supporting the 5-stage retrieval pipeline.

## Context

The Multi-Array Teleological Fingerprint architecture stores 13 separate embedding spaces (E1-E13), including E13 SPLADE for sparse lexical retrieval. Unlike fusion-based approaches, each space must be searched independently and results combined using RRF (Reciprocal Rank Fusion). This enables:
- Selective space activation (skip irrelevant spaces)
- Per-space confidence weighting
- Explainable retrieval (which spaces contributed)
- Graceful degradation (missing embeddings handled)
- **5-Stage Pipeline Integration**:
  - Stage 1: SPLADE sparse retrieval (E13) for initial candidate set
  - Stage 2: Matryoshka 128D dense retrieval for fast filtering
  - Stage 3: Full embedding search across active spaces
  - Stage 4: Cross-encoder reranking
  - Stage 5: RRF fusion for final ranking

## Technical Specification

### Data Structures

```rust
/// Query configuration for multi-embedding search
pub struct MultiEmbeddingQuery {
    /// The query text to embed
    pub query_text: String,

    /// Which embedding spaces to search (bitmask or vec)
    pub active_spaces: EmbeddingSpaceMask,

    /// Per-space weight overrides (defaults to equal weighting)
    pub space_weights: Option<[f32; 13]>,

    /// Maximum results per space before aggregation
    pub per_space_limit: usize,

    /// Final result limit after aggregation
    pub final_limit: usize,

    /// Minimum similarity threshold per space
    pub min_similarity: f32,

    /// Whether to include space-level debug info
    pub include_space_breakdown: bool,

    /// Pipeline stage configuration
    pub pipeline_config: Option<PipelineStageConfig>,
}

/// Configuration for 5-stage retrieval pipeline
#[derive(Clone, Debug)]
pub struct PipelineStageConfig {
    /// Stage 1: SPLADE sparse retrieval candidate count
    pub splade_candidates: usize,

    /// Stage 2: Matryoshka 128D filter count
    pub matryoshka_128d_limit: usize,

    /// Stage 3: Full embedding search
    pub full_search_limit: usize,

    /// Stage 4: Cross-encoder rerank count
    pub rerank_limit: usize,

    /// RRF k parameter (default: 60)
    pub rrf_k: f32,
}

impl Default for PipelineStageConfig {
    fn default() -> Self {
        Self {
            splade_candidates: 1000,
            matryoshka_128d_limit: 200,
            full_search_limit: 100,
            rerank_limit: 20,
            rrf_k: 60.0,
        }
    }
}

/// Bitmask for active embedding spaces
#[derive(Clone, Copy, Debug)]
pub struct EmbeddingSpaceMask(pub u16);

impl EmbeddingSpaceMask {
    pub const ALL: Self = Self(0x1FFF);  // All 13 spaces (bits 0-12)
    pub const ALL_DENSE: Self = Self(0x0FFF);  // All 12 dense spaces (E1-E12)
    pub const SEMANTIC_ONLY: Self = Self(0x0001);  // E1 only
    pub const TEXT_CORE: Self = Self(0x0007);  // E1, E2, E3
    pub const SPLADE_ONLY: Self = Self(0x1000);  // E13 SPLADE only
    pub const HYBRID: Self = Self(0x1001);  // E1 semantic + E13 SPLADE

    pub fn is_active(&self, space_idx: usize) -> bool {
        (self.0 & (1 << space_idx)) != 0
    }

    pub fn active_count(&self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn includes_splade(&self) -> bool {
        self.is_active(12)  // E13 is index 12
    }
}

/// Result from a single embedding space
pub struct SpaceSearchResult {
    pub space_index: usize,
    pub space_name: &'static str,
    pub matches: Vec<ScoredMatch>,
    pub search_time_ms: f32,
    pub index_size: usize,
}

/// A scored match from search
pub struct ScoredMatch {
    pub memory_id: MemoryId,
    pub similarity: f32,
    pub vector_offset: usize,
}

/// Aggregated multi-space result
pub struct MultiEmbeddingResult {
    /// Final ranked results
    pub results: Vec<AggregatedMatch>,

    /// Per-space breakdown (if requested)
    pub space_breakdown: Option<Vec<SpaceSearchResult>>,

    /// Total search time
    pub total_time_ms: f32,

    /// Spaces actually searched
    pub spaces_searched: usize,
}

/// A result aggregated across multiple spaces
pub struct AggregatedMatch {
    pub memory_id: MemoryId,

    /// Weighted average similarity across spaces
    pub aggregate_similarity: f32,

    /// How many spaces this memory appeared in
    pub space_count: usize,

    /// Per-space similarities (sparse, only matched spaces)
    pub space_similarities: Vec<(usize, f32)>,
}
```

### Core Trait

```rust
/// Executes queries across multiple embedding spaces
#[async_trait]
pub trait MultiEmbeddingQueryExecutor: Send + Sync {
    /// Execute a multi-embedding query
    async fn execute(
        &self,
        query: MultiEmbeddingQuery,
    ) -> Result<MultiEmbeddingResult, QueryError>;

    /// Execute with pre-computed query embeddings (skip embedding step)
    async fn execute_with_embeddings(
        &self,
        embeddings: &SemanticFingerprint,
        config: QueryConfig,
    ) -> Result<MultiEmbeddingResult, QueryError>;

    /// Get available embedding spaces and their status
    fn available_spaces(&self) -> &[SpaceInfo];

    /// Warm up specific spaces (pre-load indexes)
    async fn warm_up(&self, spaces: EmbeddingSpaceMask) -> Result<(), QueryError>;
}

pub struct SpaceInfo {
    pub index: usize,
    pub name: &'static str,
    pub dimension: usize,
    pub index_size: usize,
    pub is_loaded: bool,
}
```

### Aggregation Strategies

```rust
pub enum AggregationStrategy {
    /// Weighted average of similarities
    WeightedAverage {
        weights: [f32; 13],
        require_all: bool,
    },

    /// Maximum similarity across spaces
    MaxPooling,

    /// Reciprocal Rank Fusion: RRF(d) = SUM_i 1/(k + rank_i(d))
    /// Default k=60 as per standard RRF literature
    RRF { k: f32 },

    /// Learned combination (from purpose alignment)
    PurposeWeighted {
        purpose_vector: PurposeVector,
    },
}

impl AggregationStrategy {
    pub fn aggregate(&self, matches: &[(usize, f32)]) -> f32 {
        match self {
            Self::WeightedAverage { weights, require_all } => {
                let (sum, weight_sum) = matches.iter()
                    .map(|(idx, sim)| (sim * weights[*idx], weights[*idx]))
                    .fold((0.0, 0.0), |(s, w), (sim, wt)| (s + sim, w + wt));
                if weight_sum > 0.0 { sum / weight_sum } else { 0.0 }
            }
            Self::MaxPooling => {
                matches.iter().map(|(_, sim)| *sim).fold(0.0, f32::max)
            }
            Self::RRF { k } => {
                // Reciprocal Rank Fusion: sum(1 / (k + rank_i))
                // Note: This requires rank information, not just similarity
                unimplemented!("RRF requires rank-based input - use aggregate_rrf")
            }
            Self::PurposeWeighted { purpose_vector } => {
                // Weight by purpose alignment per space
                let (sum, weight_sum) = matches.iter()
                    .map(|(idx, sim)| {
                        let weight = purpose_vector.alignment[*idx];
                        (sim * weight, weight)
                    })
                    .fold((0.0, 0.0), |(s, w), (sim, wt)| (s + sim, w + wt));
                if weight_sum > 0.0 { sum / weight_sum } else { 0.0 }
            }
        }
    }

    /// Aggregate using Reciprocal Rank Fusion across ranked lists
    /// RRF(d) = SUM_i 1/(k + rank_i(d)) where k=60 by default
    ///
    /// # Arguments
    /// * `ranked_lists` - Vec of (space_index, ranked_memory_ids) per space
    /// * `k` - RRF constant (typically 60)
    ///
    /// # Returns
    /// HashMap of memory_id -> RRF score
    pub fn aggregate_rrf(
        ranked_lists: &[(usize, Vec<MemoryId>)],
        k: f32,
    ) -> HashMap<MemoryId, f32> {
        let mut scores: HashMap<MemoryId, f32> = HashMap::new();

        for (_space_idx, ranked_ids) in ranked_lists {
            for (rank, memory_id) in ranked_ids.iter().enumerate() {
                let rrf_contribution = 1.0 / (k + (rank as f32) + 1.0);
                *scores.entry(memory_id.clone()).or_insert(0.0) += rrf_contribution;
            }
        }

        scores
    }
}
```

## Implementation Requirements

### Prerequisites

- [ ] TASK-F001 complete (SemanticFingerprint available)
- [ ] TASK-F005 complete (HNSW indexes configured)
- [ ] TASK-F007 complete (EmbeddingProvider for query embedding)

### Scope

#### In Scope

- Multi-space parallel query execution (13 spaces)
- Configurable space selection (bitmask)
- Multiple aggregation strategies including RRF fusion
- Per-space result limits and thresholds
- Search timing and diagnostics
- Graceful handling of missing/failed spaces
- **E13 SPLADE sparse query execution** (lexical retrieval)
- **5-stage pipeline support** with configurable limits per stage

#### Out of Scope

- Purpose-aware retrieval (TASK-L008)
- Goal alignment scoring (TASK-L003)
- Index building (TASK-L005)

### Constraints

- Query latency < 50ms for typical queries (with warm indexes)
- Memory overhead < 1MB per active query
- Thread-safe for concurrent queries
- Graceful degradation when spaces unavailable

## Pseudo Code

```
FUNCTION execute_multi_embedding_query(query: MultiEmbeddingQuery):
    // Step 1: Generate query embeddings for active spaces
    active_spaces = query.active_spaces.to_list()
    query_embeddings = embedding_provider.embed_for_spaces(query.query_text, active_spaces)

    // Step 2: Execute parallel searches across spaces
    search_futures = []
    FOR space_idx IN active_spaces:
        IF query_embeddings[space_idx] IS NOT NULL:
            future = async search_space(
                space_idx,
                query_embeddings[space_idx],
                query.per_space_limit,
                query.min_similarity
            )
            search_futures.push((space_idx, future))

    // Step 3: Await all searches
    space_results = await_all(search_futures)

    // Step 4: Aggregate results across spaces
    memory_scores = HashMap::new()
    FOR (space_idx, result) IN space_results:
        FOR match IN result.matches:
            entry = memory_scores.entry(match.memory_id).or_default()
            entry.push((space_idx, match.similarity))

    // Step 5: Compute aggregate scores
    aggregated = []
    FOR (memory_id, space_similarities) IN memory_scores:
        score = aggregation_strategy.aggregate(space_similarities)
        aggregated.push(AggregatedMatch {
            memory_id,
            aggregate_similarity: score,
            space_count: space_similarities.len(),
            space_similarities
        })

    // Step 6: Sort and limit
    aggregated.sort_by_key(|m| -m.aggregate_similarity)
    aggregated.truncate(query.final_limit)

    RETURN MultiEmbeddingResult {
        results: aggregated,
        space_breakdown: if query.include_space_breakdown { Some(space_results) } else { None },
        total_time_ms: elapsed,
        spaces_searched: active_spaces.len()
    }
```

## Definition of Done

### Implementation Checklist

- [ ] `MultiEmbeddingQueryExecutor` trait defined
- [ ] `EmbeddingSpaceMask` with common presets (13 spaces)
- [ ] Parallel space search implementation
- [ ] WeightedAverage aggregation (13D weights)
- [ ] MaxPooling aggregation
- [ ] **RRF aggregation: RRF(d) = SUM_i 1/(k + rank_i(d)) where k=60**
- [ ] PurposeWeighted aggregation (13D purpose vector)
- [ ] Query timing instrumentation
- [ ] Graceful space failure handling
- [ ] **E13 SPLADE sparse query execution**
- [ ] **5-stage pipeline configuration**
- [ ] **PipelineStageConfig with configurable limits**

### Testing Requirements

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_space_query() {
        let executor = create_test_executor();
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::SEMANTIC_ONLY,
            per_space_limit: 10,
            final_limit: 5,
            ..Default::default()
        };
        let result = executor.execute(query).await.unwrap();
        assert!(result.spaces_searched == 1);
        assert!(result.results.len() <= 5);
    }

    #[tokio::test]
    async fn test_multi_space_aggregation() {
        let executor = create_test_executor();
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::TEXT_CORE,
            ..Default::default()
        };
        let result = executor.execute(query).await.unwrap();
        assert!(result.spaces_searched == 3);
        // Verify aggregation correctness
        for match_ in &result.results {
            assert!(match_.space_count <= 3);
        }
    }

    #[tokio::test]
    async fn test_weighted_aggregation() {
        let strategy = AggregationStrategy::WeightedAverage {
            weights: [1.0, 0.5, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            require_all: false,
        };
        let score = strategy.aggregate(&[(0, 0.8), (1, 0.6)]);
        // (0.8 * 1.0 + 0.6 * 0.5) / (1.0 + 0.5) = 1.1 / 1.5 = 0.733...
        assert!((score - 0.7333).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_rrf_aggregation() {
        // Test RRF fusion with k=60
        let ranked_lists = vec![
            (0, vec![MemoryId::from(1), MemoryId::from(2), MemoryId::from(3)]),
            (1, vec![MemoryId::from(2), MemoryId::from(1), MemoryId::from(4)]),
            (12, vec![MemoryId::from(1), MemoryId::from(4), MemoryId::from(2)]), // SPLADE
        ];

        let scores = AggregationStrategy::aggregate_rrf(&ranked_lists, 60.0);

        // Memory 1 appears at ranks 0, 1, 0 -> 1/61 + 1/62 + 1/61
        // Memory 2 appears at ranks 1, 0, 2 -> 1/62 + 1/61 + 1/63
        assert!(scores.get(&MemoryId::from(1)).unwrap() > scores.get(&MemoryId::from(4)).unwrap());
    }

    #[tokio::test]
    async fn test_graceful_space_failure() {
        let executor = create_executor_with_failing_space(2);
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::ALL,
            ..Default::default()
        };
        // Should succeed even if one space fails
        let result = executor.execute(query).await.unwrap();
        assert!(result.spaces_searched == 12); // 13 - 1 failed
    }

    #[tokio::test]
    async fn test_splade_query_execution() {
        let executor = create_test_executor();
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::SPLADE_ONLY,
            per_space_limit: 100,
            final_limit: 20,
            ..Default::default()
        };
        let result = executor.execute(query).await.unwrap();
        assert!(result.spaces_searched == 1);
        // SPLADE should return sparse lexical matches
    }

    #[tokio::test]
    async fn test_hybrid_dense_sparse() {
        let executor = create_test_executor();
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::HYBRID, // E1 + E13
            pipeline_config: Some(PipelineStageConfig::default()),
            ..Default::default()
        };
        let result = executor.execute(query).await.unwrap();
        assert!(result.spaces_searched == 2);
    }
}
```

### Verification Commands

```bash
# Run unit tests
cargo test -p context-graph-core multi_embedding_query

# Run with timing
cargo test -p context-graph-core multi_embedding_query -- --nocapture

# Benchmark query performance
cargo bench -p context-graph-core -- multi_embedding
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/retrieval/mod.rs` | Retrieval module |
| `crates/context-graph-core/src/retrieval/query_executor.rs` | Query executor trait and impl |
| `crates/context-graph-core/src/retrieval/aggregation.rs` | Aggregation strategies |
| `crates/context-graph-core/src/retrieval/space_mask.rs` | EmbeddingSpaceMask utilities |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/lib.rs` | Add `pub mod retrieval` |
| `crates/context-graph-core/src/traits.rs` | Add MultiEmbeddingQueryExecutor trait |

## Traceability

| Requirement | Source | Coverage |
|-------------|--------|----------|
| Multi-embedding search | projectionplan1.md:retrieval | Complete |
| Parallel space queries | projectionplan2.md:performance | Complete |
| Aggregation strategies | projectionplan2.md:ranking | Complete |
| Space selection | projectionplan1.md:selective-search | Complete |

---

*Task created: 2026-01-04*
*Layer: Logic*
*Priority: P0 - Core retrieval functionality*
