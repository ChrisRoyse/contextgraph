# TASK-L001: Multi-Embedding Query Executor

```yaml
metadata:
  id: "TASK-L001"
  title: "Multi-Embedding Query Executor"
  layer: "logic"
  priority: "P0"
  estimated_hours: 12
  created: "2026-01-04"
  updated: "2026-01-05"
  status: "pending"
  dependencies:
    - "TASK-F001"  # SemanticFingerprint struct (COMPLETE)
    - "TASK-F005"  # HNSW indexes (COMPLETE)
    - "TASK-F007"  # MultiArrayEmbeddingProvider trait (COMPLETE)
  spec_refs:
    - "constitution.yaml:retrieval-pipeline"
    - "contextprd.md:5-stage-pipeline"
    - "learntheory.md:UTL-formula"
```

## Problem Statement

Implement a query executor that searches across all 13 embedding spaces in parallel, aggregating results with RRF (Reciprocal Rank Fusion) as the primary fusion method, supporting the 5-stage retrieval pipeline with <60ms latency target at 1M memories.

## Context

The Multi-Array Teleological Fingerprint architecture stores 13 separate embedding spaces (E1-E13). Unlike fusion-based approaches, each space must be searched independently and results combined using RRF. This enables:

- Selective space activation via bitmask (skip irrelevant spaces)
- Per-space confidence weighting from purpose vector alignment
- Explainable retrieval (which spaces contributed to ranking)
- Graceful degradation (missing embeddings handled with fail-fast error reporting)
- **5-Stage Pipeline Integration** per `constitution.yaml` and `contextprd.md`

### 5-Stage Pipeline Architecture (Source of Truth: constitution.yaml)

| Stage | Name | Component | Target Latency | Candidates |
|-------|------|-----------|----------------|------------|
| 1 | Recall | SPLADE Sparse (E13) | <5ms | 1000 |
| 2 | Dense Filter | Matryoshka 128D (E1[:128]) | <10ms | 200 |
| 3 | Precision | Full 13-space HNSW | <20ms | 100 |
| 4 | Teleological | Purpose alignment filter | <10ms | 50 |
| 5 | Late Interaction | ColBERT MaxSim (E12) | <15ms | Final |

**Total Target: <60ms @ 1M memories**

### 13 Embedding Spaces (Source of Truth: SemanticFingerprint)

```
E1:  Semantic      (1024D) - e5-large-v2
E2:  Temporal-Recent (512D) - exponential decay
E3:  Temporal-Periodic (512D) - Fourier features
E4:  Temporal-Positional (512D) - sinusoidal PE
E5:  Causal        (768D) - Longformer
E6:  Sparse        (variable) - SPLADE
E7:  Code          (256D) - CodeT5p
E8:  Graph         (384D) - MiniLM
E9:  HDC           (10000D) - Hyperdimensional
E10: Multimodal    (768D) - CLIP
E11: Entity        (384D) - MiniLM
E12: Late-Interaction (128D per token) - ColBERT
E13: SPLADE        (variable) - SPLADE v3
```

## Technical Specification

### Existing Infrastructure (VERIFIED)

**Traits - `crates/context-graph-core/src/traits/`:**
- `multi_array_embedding.rs` - `MultiArrayEmbeddingProvider`, `SingleEmbedder`, `SparseEmbedder`, `TokenEmbedder`, `MultiArrayEmbeddingOutput`
- `teleological_memory_store.rs` - `TeleologicalMemoryStore`, `TeleologicalSearchOptions`, `TeleologicalSearchResult`

**Types - `crates/context-graph-core/src/types/fingerprint/`:**
- `semantic.rs` - `SemanticFingerprint` with all 13 embeddings
- `sparse.rs` - `SparseVector` for E6 and E13
- `purpose.rs` - `PurposeVector` (13D alignment)
- `teleological.rs` - `TeleologicalFingerprint`

**Stubs - `crates/context-graph-core/src/stubs/`:**
- `multi_array_stub.rs` - `StubMultiArrayProvider`
- `teleological_store_stub.rs` - `InMemoryTeleologicalStore`

**Error Types - `crates/context-graph-core/src/error.rs`:**
- `CoreError::StorageError` - Storage operations
- `CoreError::IndexError` - HNSW/index failures
- `CoreError::ValidationError` - Input validation
- `CoreError::Embedding` - Embedding generation
- `CoreError::Internal` - Invariant violations

### Data Structures (TO CREATE)

```rust
// Location: crates/context-graph-core/src/retrieval/query.rs

use crate::error::{CoreError, CoreResult};
use crate::types::fingerprint::{PurposeVector, SemanticFingerprint, SparseVector, NUM_EMBEDDERS};
use uuid::Uuid;
use std::time::Duration;

/// Query configuration for multi-embedding search.
///
/// # Performance Targets (constitution.yaml)
/// - Total latency: <60ms @ 1M memories
/// - Single embedding: <30ms
///
/// # Fail-Fast Behavior
/// - Empty query_text: Returns `CoreError::ValidationError`
/// - Invalid space indices: Returns `CoreError::ValidationError`
/// - All spaces disabled: Returns `CoreError::ValidationError`
#[derive(Clone, Debug)]
pub struct MultiEmbeddingQuery {
    /// The query text to embed. MUST be non-empty.
    pub query_text: String,

    /// Which embedding spaces to search (bitmask).
    /// Default: ALL (0x1FFF = all 13 spaces)
    pub active_spaces: EmbeddingSpaceMask,

    /// Per-space weight overrides [0.0, 1.0].
    /// None = equal weighting (1.0 for all active spaces)
    pub space_weights: Option<[f32; NUM_EMBEDDERS]>,

    /// Maximum results per space before aggregation.
    /// Default: 100, Range: [1, 1000]
    pub per_space_limit: usize,

    /// Final result limit after aggregation.
    /// Default: 10, Range: [1, 1000]
    pub final_limit: usize,

    /// Minimum similarity threshold per space [0.0, 1.0].
    /// Results below threshold are filtered. Default: 0.0
    pub min_similarity: f32,

    /// Include per-space breakdown in results.
    /// Default: false (reduces response size)
    pub include_space_breakdown: bool,

    /// Pipeline stage configuration.
    /// None = use defaults from PipelineStageConfig::default()
    pub pipeline_config: Option<PipelineStageConfig>,

    /// Aggregation strategy.
    /// Default: RRF with k=60
    pub aggregation: AggregationStrategy,
}

impl Default for MultiEmbeddingQuery {
    fn default() -> Self {
        Self {
            query_text: String::new(),
            active_spaces: EmbeddingSpaceMask::ALL,
            space_weights: None,
            per_space_limit: 100,
            final_limit: 10,
            min_similarity: 0.0,
            include_space_breakdown: false,
            pipeline_config: None,
            aggregation: AggregationStrategy::RRF { k: 60.0 },
        }
    }
}

impl MultiEmbeddingQuery {
    /// Validate query configuration.
    ///
    /// # Errors
    /// - `CoreError::ValidationError` if query_text is empty
    /// - `CoreError::ValidationError` if no spaces are active
    /// - `CoreError::ValidationError` if limits are out of range
    pub fn validate(&self) -> CoreResult<()> {
        if self.query_text.is_empty() {
            return Err(CoreError::ValidationError {
                field: "query_text".to_string(),
                message: "Query text must not be empty".to_string(),
            });
        }

        if self.active_spaces.active_count() == 0 {
            return Err(CoreError::ValidationError {
                field: "active_spaces".to_string(),
                message: "At least one embedding space must be active".to_string(),
            });
        }

        if self.per_space_limit == 0 || self.per_space_limit > 1000 {
            return Err(CoreError::ValidationError {
                field: "per_space_limit".to_string(),
                message: format!("per_space_limit must be in [1, 1000], got {}", self.per_space_limit),
            });
        }

        if self.final_limit == 0 || self.final_limit > 1000 {
            return Err(CoreError::ValidationError {
                field: "final_limit".to_string(),
                message: format!("final_limit must be in [1, 1000], got {}", self.final_limit),
            });
        }

        if self.min_similarity < 0.0 || self.min_similarity > 1.0 {
            return Err(CoreError::ValidationError {
                field: "min_similarity".to_string(),
                message: format!("min_similarity must be in [0.0, 1.0], got {}", self.min_similarity),
            });
        }

        Ok(())
    }
}

/// Configuration for 5-stage retrieval pipeline.
///
/// # Stage Targets (constitution.yaml)
/// - Stage 1 (SPLADE): <5ms, 1000 candidates
/// - Stage 2 (Matryoshka): <10ms, 200 candidates
/// - Stage 3 (Full HNSW): <20ms, 100 candidates
/// - Stage 4 (Teleological): <10ms, 50 candidates
/// - Stage 5 (Late Interaction): <15ms, final ranking
#[derive(Clone, Debug)]
pub struct PipelineStageConfig {
    /// Stage 1: SPLADE sparse retrieval candidate count.
    /// Default: 1000
    pub splade_candidates: usize,

    /// Stage 2: Matryoshka 128D filter count.
    /// Default: 200
    pub matryoshka_128d_limit: usize,

    /// Stage 3: Full 13-space embedding search limit.
    /// Default: 100
    pub full_search_limit: usize,

    /// Stage 4: Teleological alignment filter limit.
    /// Default: 50
    pub teleological_limit: usize,

    /// Stage 5: Late interaction rerank count.
    /// Default: 20
    pub late_interaction_limit: usize,

    /// RRF k parameter (default: 60 per constitution.yaml).
    /// Formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d))
    pub rrf_k: f32,

    /// Minimum alignment threshold for Stage 4 teleological filter.
    /// Default: 0.55 (critical threshold per constitution.yaml)
    pub min_alignment_threshold: f32,
}

impl Default for PipelineStageConfig {
    fn default() -> Self {
        Self {
            splade_candidates: 1000,
            matryoshka_128d_limit: 200,
            full_search_limit: 100,
            teleological_limit: 50,
            late_interaction_limit: 20,
            rrf_k: 60.0,
            min_alignment_threshold: 0.55,
        }
    }
}

/// Bitmask for active embedding spaces (0-12).
///
/// # Bit Layout
/// - Bit 0: E1 Semantic
/// - Bit 1: E2 Temporal-Recent
/// - Bit 2: E3 Temporal-Periodic
/// - Bit 3: E4 Temporal-Positional
/// - Bit 4: E5 Causal
/// - Bit 5: E6 Sparse
/// - Bit 6: E7 Code
/// - Bit 7: E8 Graph
/// - Bit 8: E9 HDC
/// - Bit 9: E10 Multimodal
/// - Bit 10: E11 Entity
/// - Bit 11: E12 Late-Interaction
/// - Bit 12: E13 SPLADE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbeddingSpaceMask(pub u16);

impl EmbeddingSpaceMask {
    /// All 13 spaces active (bits 0-12).
    pub const ALL: Self = Self(0x1FFF);

    /// All 12 dense spaces (E1-E12, bits 0-11).
    pub const ALL_DENSE: Self = Self(0x0FFF);

    /// E1 semantic only.
    pub const SEMANTIC_ONLY: Self = Self(0x0001);

    /// Text core: E1, E2, E3.
    pub const TEXT_CORE: Self = Self(0x0007);

    /// E13 SPLADE only (bit 12).
    pub const SPLADE_ONLY: Self = Self(0x1000);

    /// Hybrid: E1 semantic + E13 SPLADE.
    pub const HYBRID: Self = Self(0x1001);

    /// Stage 2 fast filter: E1 only (Matryoshka 128D).
    pub const MATRYOSHKA_FILTER: Self = Self(0x0001);

    /// Code-focused: E1, E7 Code, E13 SPLADE.
    pub const CODE_FOCUSED: Self = Self(0x1041);

    /// Check if a specific space is active.
    #[inline]
    pub fn is_active(&self, space_idx: usize) -> bool {
        if space_idx >= NUM_EMBEDDERS {
            return false;
        }
        (self.0 & (1 << space_idx)) != 0
    }

    /// Count number of active spaces.
    #[inline]
    pub fn active_count(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// Check if E13 SPLADE is active (for Stage 1).
    #[inline]
    pub fn includes_splade(&self) -> bool {
        self.is_active(12)
    }

    /// Check if E12 Late-Interaction is active (for Stage 5).
    #[inline]
    pub fn includes_late_interaction(&self) -> bool {
        self.is_active(11)
    }

    /// Get list of active space indices.
    pub fn active_indices(&self) -> Vec<usize> {
        (0..NUM_EMBEDDERS)
            .filter(|&i| self.is_active(i))
            .collect()
    }

    /// Get embedding space name by index.
    pub const fn space_name(idx: usize) -> &'static str {
        match idx {
            0 => "E1_Semantic",
            1 => "E2_Temporal_Recent",
            2 => "E3_Temporal_Periodic",
            3 => "E4_Temporal_Positional",
            4 => "E5_Causal",
            5 => "E6_Sparse",
            6 => "E7_Code",
            7 => "E8_Graph",
            8 => "E9_HDC",
            9 => "E10_Multimodal",
            10 => "E11_Entity",
            11 => "E12_Late_Interaction",
            12 => "E13_SPLADE",
            _ => "Unknown",
        }
    }
}

impl Default for EmbeddingSpaceMask {
    fn default() -> Self {
        Self::ALL
    }
}
```

### Aggregation Strategies

```rust
// Location: crates/context-graph-core/src/retrieval/aggregation.rs

use crate::types::fingerprint::{PurposeVector, NUM_EMBEDDERS};
use std::collections::HashMap;
use uuid::Uuid;

/// Aggregation strategy for combining multi-space search results.
///
/// # Primary Strategy: RRF (per constitution.yaml)
/// RRF(d) = Σᵢ 1/(k + rankᵢ(d)) where k=60
#[derive(Clone, Debug)]
pub enum AggregationStrategy {
    /// Reciprocal Rank Fusion - PRIMARY STRATEGY.
    /// Formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d))
    ///
    /// # Parameters
    /// - k: Ranking constant (default: 60 per RRF literature)
    RRF { k: f32 },

    /// Weighted average of similarities.
    /// Score = Σ(wᵢ × simᵢ) / Σwᵢ
    WeightedAverage {
        weights: [f32; NUM_EMBEDDERS],
        require_all: bool,
    },

    /// Maximum similarity across spaces.
    /// Score = max(simᵢ)
    MaxPooling,

    /// Purpose-weighted aggregation using 13D purpose vector.
    /// Score = Σ(τᵢ × simᵢ) / Στᵢ where τ = purpose_vector.alignment
    PurposeWeighted {
        purpose_vector: PurposeVector,
    },
}

impl Default for AggregationStrategy {
    fn default() -> Self {
        Self::RRF { k: 60.0 }
    }
}

impl AggregationStrategy {
    /// Aggregate similarity scores (for non-RRF strategies).
    ///
    /// # Arguments
    /// - matches: Vec of (space_index, similarity) pairs
    ///
    /// # Returns
    /// Aggregated similarity score [0.0, 1.0]
    ///
    /// # Panics
    /// Panics if called with RRF strategy (use aggregate_rrf instead)
    pub fn aggregate(&self, matches: &[(usize, f32)]) -> f32 {
        match self {
            Self::RRF { .. } => {
                panic!("RRF requires rank-based input - use aggregate_rrf()");
            }
            Self::WeightedAverage { weights, require_all } => {
                if *require_all && matches.len() < NUM_EMBEDDERS {
                    return 0.0;
                }
                let (sum, weight_sum) = matches
                    .iter()
                    .filter(|(idx, _)| *idx < NUM_EMBEDDERS)
                    .map(|(idx, sim)| (sim * weights[*idx], weights[*idx]))
                    .fold((0.0, 0.0), |(s, w), (sim, wt)| (s + sim, w + wt));
                if weight_sum > f32::EPSILON {
                    sum / weight_sum
                } else {
                    0.0
                }
            }
            Self::MaxPooling => {
                matches
                    .iter()
                    .map(|(_, sim)| *sim)
                    .fold(0.0_f32, f32::max)
            }
            Self::PurposeWeighted { purpose_vector } => {
                let (sum, weight_sum) = matches
                    .iter()
                    .filter(|(idx, _)| *idx < NUM_EMBEDDERS)
                    .map(|(idx, sim)| {
                        let weight = purpose_vector.alignment[*idx];
                        (sim * weight, weight)
                    })
                    .fold((0.0, 0.0), |(s, w), (sim, wt)| (s + sim, w + wt));
                if weight_sum > f32::EPSILON {
                    sum / weight_sum
                } else {
                    0.0
                }
            }
        }
    }

    /// Aggregate using Reciprocal Rank Fusion across ranked lists.
    ///
    /// # Formula
    /// RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)
    ///
    /// # Arguments
    /// - ranked_lists: Vec of (space_index, Vec<memory_id>) per space
    /// - k: RRF constant (default: 60)
    ///
    /// # Returns
    /// HashMap of memory_id -> RRF score
    ///
    /// # Example
    /// ```ignore
    /// // Document d appears at ranks 0, 2, 1 across 3 spaces
    /// // RRF(d) = 1/(60+1) + 1/(60+3) + 1/(60+2) = 1/61 + 1/63 + 1/62 ≈ 0.0492
    /// ```
    pub fn aggregate_rrf(
        ranked_lists: &[(usize, Vec<Uuid>)],
        k: f32,
    ) -> HashMap<Uuid, f32> {
        let mut scores: HashMap<Uuid, f32> = HashMap::new();

        for (_space_idx, ranked_ids) in ranked_lists {
            for (rank, memory_id) in ranked_ids.iter().enumerate() {
                // RRF: 1 / (k + rank + 1) - rank is 0-indexed
                let rrf_contribution = 1.0 / (k + (rank as f32) + 1.0);
                *scores.entry(*memory_id).or_insert(0.0) += rrf_contribution;
            }
        }

        scores
    }

    /// Aggregate RRF with per-space weighting.
    ///
    /// # Formula
    /// RRF_weighted(d) = Σᵢ wᵢ/(k + rankᵢ(d) + 1)
    pub fn aggregate_rrf_weighted(
        ranked_lists: &[(usize, Vec<Uuid>)],
        k: f32,
        weights: &[f32; NUM_EMBEDDERS],
    ) -> HashMap<Uuid, f32> {
        let mut scores: HashMap<Uuid, f32> = HashMap::new();

        for (space_idx, ranked_ids) in ranked_lists {
            let weight = if *space_idx < NUM_EMBEDDERS {
                weights[*space_idx]
            } else {
                1.0
            };

            for (rank, memory_id) in ranked_ids.iter().enumerate() {
                let rrf_contribution = weight / (k + (rank as f32) + 1.0);
                *scores.entry(*memory_id).or_insert(0.0) += rrf_contribution;
            }
        }

        scores
    }
}
```

### Result Types

```rust
// Location: crates/context-graph-core/src/retrieval/result.rs

use crate::types::fingerprint::NUM_EMBEDDERS;
use uuid::Uuid;
use std::time::Duration;

/// Result from a single embedding space search.
#[derive(Clone, Debug)]
pub struct SpaceSearchResult {
    /// Space index (0-12).
    pub space_index: usize,

    /// Space name (e.g., "E1_Semantic").
    pub space_name: &'static str,

    /// Matches from this space, ranked by similarity.
    pub matches: Vec<ScoredMatch>,

    /// Search time for this space.
    pub search_time: Duration,

    /// Number of items in index for this space.
    pub index_size: usize,

    /// Whether this space search succeeded.
    pub success: bool,

    /// Error message if search failed (for graceful degradation).
    pub error: Option<String>,
}

/// A scored match from a single space.
#[derive(Clone, Debug)]
pub struct ScoredMatch {
    /// Memory/fingerprint UUID.
    pub memory_id: Uuid,

    /// Similarity score [0.0, 1.0].
    pub similarity: f32,

    /// Rank in this space's results (0-indexed).
    pub rank: usize,
}

/// Aggregated multi-space search result.
#[derive(Clone, Debug)]
pub struct MultiEmbeddingResult {
    /// Final ranked results after aggregation.
    pub results: Vec<AggregatedMatch>,

    /// Per-space breakdown (if include_space_breakdown=true).
    pub space_breakdown: Option<Vec<SpaceSearchResult>>,

    /// Total end-to-end search time.
    pub total_time: Duration,

    /// Number of spaces actually searched successfully.
    pub spaces_searched: usize,

    /// Number of spaces that failed (for graceful degradation tracking).
    pub spaces_failed: usize,

    /// Pipeline stage timings (if pipeline mode enabled).
    pub stage_timings: Option<PipelineStageTiming>,
}

/// Timing breakdown for 5-stage pipeline.
#[derive(Clone, Debug)]
pub struct PipelineStageTiming {
    /// Stage 1: SPLADE sparse retrieval.
    pub stage1_splade: Duration,

    /// Stage 2: Matryoshka 128D dense filter.
    pub stage2_matryoshka: Duration,

    /// Stage 3: Full 13-space HNSW search.
    pub stage3_full_hnsw: Duration,

    /// Stage 4: Teleological alignment filter.
    pub stage4_teleological: Duration,

    /// Stage 5: Late interaction reranking.
    pub stage5_late_interaction: Duration,

    /// Candidates after each stage.
    pub candidates_per_stage: [usize; 5],
}

impl PipelineStageTiming {
    /// Check if all stages met their latency targets (constitution.yaml).
    pub fn all_stages_within_target(&self) -> bool {
        self.stage1_splade.as_millis() < 5
            && self.stage2_matryoshka.as_millis() < 10
            && self.stage3_full_hnsw.as_millis() < 20
            && self.stage4_teleological.as_millis() < 10
            && self.stage5_late_interaction.as_millis() < 15
    }

    /// Total pipeline time.
    pub fn total(&self) -> Duration {
        self.stage1_splade
            + self.stage2_matryoshka
            + self.stage3_full_hnsw
            + self.stage4_teleological
            + self.stage5_late_interaction
    }
}

/// A result aggregated across multiple embedding spaces.
#[derive(Clone, Debug)]
pub struct AggregatedMatch {
    /// Memory/fingerprint UUID.
    pub memory_id: Uuid,

    /// Aggregated score (RRF or weighted average).
    pub aggregate_score: f32,

    /// Number of spaces this memory appeared in.
    pub space_count: usize,

    /// Per-space scores (space_index, similarity, rank).
    pub space_contributions: Vec<SpaceContribution>,

    /// Purpose alignment score (from Stage 4 if available).
    pub purpose_alignment: Option<f32>,
}

/// Contribution from a single space to the aggregated score.
#[derive(Clone, Debug)]
pub struct SpaceContribution {
    /// Space index (0-12).
    pub space_index: usize,

    /// Similarity in this space.
    pub similarity: f32,

    /// Rank in this space's results.
    pub rank: usize,

    /// RRF contribution: 1/(k + rank + 1).
    pub rrf_contribution: f32,
}
```

### Core Trait

```rust
// Location: crates/context-graph-core/src/retrieval/executor.rs

use async_trait::async_trait;
use crate::error::CoreResult;
use crate::types::fingerprint::SemanticFingerprint;
use super::{
    MultiEmbeddingQuery, MultiEmbeddingResult, EmbeddingSpaceMask, SpaceInfo,
};

/// Multi-embedding query executor trait.
///
/// Executes queries across 13 embedding spaces in parallel,
/// aggregating results using the configured strategy (default: RRF).
///
/// # Performance Targets (constitution.yaml)
/// - Total latency: <60ms @ 1M memories
/// - Query embedding: <30ms
///
/// # Thread Safety
/// Required to be `Send + Sync` for concurrent query execution.
///
/// # Fail-Fast Behavior
/// All methods return `CoreError` on failure with detailed context.
/// No silent failures or fallback to default values.
#[async_trait]
pub trait MultiEmbeddingQueryExecutor: Send + Sync {
    /// Execute a multi-embedding query.
    ///
    /// # Arguments
    /// - query: Query configuration (validated internally)
    ///
    /// # Returns
    /// `MultiEmbeddingResult` with ranked results and timing
    ///
    /// # Errors
    /// - `CoreError::ValidationError` - Invalid query parameters
    /// - `CoreError::Embedding` - Embedding generation failed
    /// - `CoreError::IndexError` - HNSW/index search failed
    /// - `CoreError::StorageError` - Storage backend failure
    ///
    /// # Example
    /// ```ignore
    /// let query = MultiEmbeddingQuery {
    ///     query_text: "How does memory consolidation work?".to_string(),
    ///     active_spaces: EmbeddingSpaceMask::ALL,
    ///     final_limit: 10,
    ///     ..Default::default()
    /// };
    /// let result = executor.execute(query).await?;
    /// assert!(result.total_time.as_millis() < 60);
    /// ```
    async fn execute(
        &self,
        query: MultiEmbeddingQuery,
    ) -> CoreResult<MultiEmbeddingResult>;

    /// Execute with pre-computed query embeddings (skip embedding step).
    ///
    /// Use when embeddings are already available (e.g., from cache).
    ///
    /// # Arguments
    /// - embeddings: Pre-computed 13-embedding fingerprint
    /// - query: Query configuration (query_text ignored)
    ///
    /// # Errors
    /// Same as `execute()` except no `CoreError::Embedding`
    async fn execute_with_embeddings(
        &self,
        embeddings: &SemanticFingerprint,
        query: MultiEmbeddingQuery,
    ) -> CoreResult<MultiEmbeddingResult>;

    /// Get information about available embedding spaces.
    ///
    /// Returns status for all 13 spaces including:
    /// - Whether index is loaded
    /// - Index size (number of vectors)
    /// - Dimension
    fn available_spaces(&self) -> Vec<SpaceInfo>;

    /// Warm up specific spaces by pre-loading indexes.
    ///
    /// Call before queries for predictable latency.
    ///
    /// # Errors
    /// - `CoreError::IndexError` - Failed to load index
    async fn warm_up(&self, spaces: EmbeddingSpaceMask) -> CoreResult<()>;

    /// Execute 5-stage pipeline query.
    ///
    /// Full pipeline with all stages:
    /// 1. SPLADE sparse recall
    /// 2. Matryoshka 128D filtering
    /// 3. Full 13-space HNSW search
    /// 4. Teleological alignment filter
    /// 5. Late interaction reranking
    ///
    /// # Performance Target
    /// <60ms total @ 1M memories
    async fn execute_pipeline(
        &self,
        query: MultiEmbeddingQuery,
    ) -> CoreResult<MultiEmbeddingResult>;
}

/// Information about a single embedding space.
#[derive(Clone, Debug)]
pub struct SpaceInfo {
    /// Space index (0-12).
    pub index: usize,

    /// Space name (e.g., "E1_Semantic").
    pub name: &'static str,

    /// Embedding dimension (0 for sparse spaces E6, E13).
    pub dimension: usize,

    /// Number of vectors in index.
    pub index_size: usize,

    /// Whether index is loaded in memory.
    pub is_loaded: bool,

    /// Index type (HNSW, Inverted, etc.).
    pub index_type: IndexType,
}

/// Type of index used for a space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexType {
    /// HNSW for dense vectors.
    Hnsw,
    /// Inverted index for sparse vectors (E6, E13).
    Inverted,
    /// No index (linear scan).
    None,
}
```

## Implementation Requirements

### Prerequisites (ALL COMPLETE)

- [x] TASK-F001: `SemanticFingerprint` with 13 embeddings
- [x] TASK-F005: HNSW indexes configured
- [x] TASK-F007: `MultiArrayEmbeddingProvider` trait

### Scope

#### In Scope

1. **Query Structures** (query.rs)
   - `MultiEmbeddingQuery` with validation
   - `PipelineStageConfig` with defaults
   - `EmbeddingSpaceMask` with presets

2. **Aggregation** (aggregation.rs)
   - `AggregationStrategy` enum
   - RRF implementation: `RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)`
   - WeightedAverage, MaxPooling, PurposeWeighted

3. **Results** (result.rs)
   - `SpaceSearchResult` per-space
   - `AggregatedMatch` combined
   - `PipelineStageTiming` metrics

4. **Executor Trait** (executor.rs)
   - `MultiEmbeddingQueryExecutor` trait
   - 5-stage pipeline support
   - Graceful degradation

5. **In-Memory Implementation** (in_memory_executor.rs)
   - Uses `InMemoryTeleologicalStore`
   - Uses `StubMultiArrayProvider`
   - Full test coverage with real data

#### Out of Scope

- Purpose-aware retrieval weighting (TASK-L008)
- Goal alignment scoring (TASK-L003)
- Index building (TASK-L005)
- Production RocksDB executor (separate task)

### Constraints (constitution.yaml)

| Constraint | Value | Validation |
|------------|-------|------------|
| Query latency | <60ms @ 1M memories | Log warning if exceeded |
| Single embedding | <30ms | `MultiArrayEmbeddingOutput::is_within_latency_target()` |
| Memory per query | <1MB | Track allocation |
| Thread safety | Required | `Send + Sync` bounds |
| Graceful degradation | Yes | Continue if spaces fail |

## Pseudo Code

```
FUNCTION execute_multi_embedding_query(query: MultiEmbeddingQuery):
    // Step 0: Validate query (FAIL FAST)
    query.validate()?  // Returns CoreError::ValidationError if invalid

    start_time = Instant::now()

    // Step 1: Generate query embeddings
    query_embeddings = embedding_provider.embed_all(query.query_text).await?
    IF !query_embeddings.is_within_latency_target():
        log::warn!("Embedding exceeded 30ms target: {:?}", query_embeddings.total_latency)

    // Step 2: Execute parallel searches across active spaces
    active_indices = query.active_spaces.active_indices()
    search_futures = Vec::new()

    FOR space_idx IN active_indices:
        embedding = get_embedding_for_space(query_embeddings, space_idx)
        future = search_space_async(
            space_idx,
            embedding,
            query.per_space_limit,
            query.min_similarity
        )
        search_futures.push((space_idx, future))

    // Step 3: Await all searches with graceful degradation
    space_results = Vec::new()
    spaces_failed = 0

    FOR (space_idx, future) IN join_all(search_futures):
        MATCH future.await:
            Ok(result) => space_results.push(result)
            Err(e) => {
                log::error!("Space {} search failed: {}", space_idx, e)
                spaces_failed += 1
                // Continue with other spaces (graceful degradation)
            }

    // Step 4: Build ranked lists for RRF
    ranked_lists: Vec<(usize, Vec<Uuid>)> = space_results
        .iter()
        .map(|r| (r.space_index, r.matches.iter().map(|m| m.memory_id).collect()))
        .collect()

    // Step 5: Aggregate using RRF
    rrf_scores = AggregationStrategy::aggregate_rrf(&ranked_lists, query.pipeline_config.rrf_k)

    // Step 6: Build aggregated matches
    aggregated = Vec::new()
    FOR (memory_id, score) IN rrf_scores:
        contributions = build_space_contributions(memory_id, &space_results, query.pipeline_config.rrf_k)
        aggregated.push(AggregatedMatch {
            memory_id,
            aggregate_score: score,
            space_count: contributions.len(),
            space_contributions: contributions,
            purpose_alignment: None, // Set in Stage 4 if enabled
        })

    // Step 7: Sort by aggregate score and limit
    aggregated.sort_by(|a, b| b.aggregate_score.partial_cmp(&a.aggregate_score).unwrap())
    aggregated.truncate(query.final_limit)

    // Step 8: Build result
    total_time = start_time.elapsed()
    IF total_time.as_millis() > 60:
        log::warn!("Query exceeded 60ms target: {:?}", total_time)

    RETURN MultiEmbeddingResult {
        results: aggregated,
        space_breakdown: IF query.include_space_breakdown { Some(space_results) } ELSE { None },
        total_time,
        spaces_searched: space_results.len(),
        spaces_failed,
        stage_timings: None, // Set in execute_pipeline()
    }
```

## Definition of Done

### Implementation Checklist

- [ ] `MultiEmbeddingQuery` with `validate()` method
- [ ] `EmbeddingSpaceMask` with all presets
- [ ] `PipelineStageConfig` with defaults from constitution.yaml
- [ ] `AggregationStrategy` enum with 4 strategies
- [ ] `aggregate_rrf()` function: `RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)`
- [ ] `aggregate_rrf_weighted()` function
- [ ] `SpaceSearchResult`, `AggregatedMatch`, `PipelineStageTiming`
- [ ] `MultiEmbeddingQueryExecutor` trait with 4 methods
- [ ] `InMemoryMultiEmbeddingExecutor` implementation
- [ ] Parallel space search using `tokio::join!`
- [ ] Graceful degradation (continue on space failure)
- [ ] Timing instrumentation with warnings on target exceeded
- [ ] Error propagation with `CoreError` variants

### Testing Requirements (NO MOCK DATA)

All tests use `InMemoryTeleologicalStore` with real `TeleologicalFingerprint` data.

```rust
// Location: crates/context-graph-core/src/retrieval/tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};
    use crate::types::fingerprint::{
        JohariFingerprint, PurposeVector, SemanticFingerprint, SparseVector,
        TeleologicalFingerprint, NUM_EMBEDDERS,
    };

    /// Create real test data - NOT mocks.
    fn create_test_fingerprint(content_hash: [u8; 32]) -> TeleologicalFingerprint {
        TeleologicalFingerprint::new(
            SemanticFingerprint::zeroed(),
            PurposeVector::new([0.75; NUM_EMBEDDERS]),
            JohariFingerprint::zeroed(),
            content_hash,
        )
    }

    /// Create executor with real store and provider.
    async fn create_test_executor() -> impl MultiEmbeddingQueryExecutor {
        let store = InMemoryTeleologicalStore::new();
        let provider = StubMultiArrayProvider::new();

        // Populate with real test data
        for i in 0..100 {
            let mut hash = [0u8; 32];
            hash[0] = i as u8;
            store.store(create_test_fingerprint(hash)).await.unwrap();
        }

        InMemoryMultiEmbeddingExecutor::new(store, provider)
    }

    #[tokio::test]
    async fn test_single_space_query() {
        let executor = create_test_executor().await;
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::SEMANTIC_ONLY,
            per_space_limit: 10,
            final_limit: 5,
            ..Default::default()
        };

        let result = executor.execute(query).await.unwrap();

        assert_eq!(result.spaces_searched, 1);
        assert!(result.results.len() <= 5);
        assert_eq!(result.spaces_failed, 0);

        println!("[VERIFIED] test_single_space_query: spaces_searched={}, results={}",
            result.spaces_searched, result.results.len());
    }

    #[tokio::test]
    async fn test_all_spaces_query() {
        let executor = create_test_executor().await;
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::ALL,
            final_limit: 10,
            ..Default::default()
        };

        let result = executor.execute(query).await.unwrap();

        assert_eq!(result.spaces_searched, 13);

        println!("[VERIFIED] test_all_spaces_query: searched all 13 spaces");
    }

    #[tokio::test]
    async fn test_rrf_aggregation_correctness() {
        // Test RRF formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let ranked_lists = vec![
            (0, vec![id1, id2, id3]), // Space 0: id1=rank0, id2=rank1, id3=rank2
            (1, vec![id2, id1, id3]), // Space 1: id2=rank0, id1=rank1, id3=rank2
            (12, vec![id1, id3, id2]), // Space 12 (SPLADE): id1=rank0, id3=rank1, id2=rank2
        ];

        let scores = AggregationStrategy::aggregate_rrf(&ranked_lists, 60.0);

        // id1 appears at ranks 0, 1, 0 -> 1/61 + 1/62 + 1/61 ≈ 0.0489
        // id2 appears at ranks 1, 0, 2 -> 1/62 + 1/61 + 1/63 ≈ 0.0484
        // id3 appears at ranks 2, 2, 1 -> 1/63 + 1/63 + 1/62 ≈ 0.0479

        let score1 = scores.get(&id1).unwrap();
        let score2 = scores.get(&id2).unwrap();
        let score3 = scores.get(&id3).unwrap();

        assert!(score1 > score2, "id1 should rank higher than id2");
        assert!(score2 > score3, "id2 should rank higher than id3");

        // Verify exact RRF formula
        let expected_id1 = 1.0/61.0 + 1.0/62.0 + 1.0/61.0;
        assert!((score1 - expected_id1).abs() < 0.0001,
            "RRF for id1: expected {}, got {}", expected_id1, score1);

        println!("[VERIFIED] test_rrf_aggregation_correctness: id1={:.4}, id2={:.4}, id3={:.4}",
            score1, score2, score3);
    }

    #[tokio::test]
    async fn test_query_validation_empty_text() {
        let query = MultiEmbeddingQuery {
            query_text: "".into(),
            ..Default::default()
        };

        let result = query.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            CoreError::ValidationError { field, .. } => {
                assert_eq!(field, "query_text");
            }
            _ => panic!("Expected ValidationError"),
        }

        println!("[VERIFIED] test_query_validation_empty_text: Correctly rejects empty query");
    }

    #[tokio::test]
    async fn test_query_validation_no_active_spaces() {
        let query = MultiEmbeddingQuery {
            query_text: "test".into(),
            active_spaces: EmbeddingSpaceMask(0),
            ..Default::default()
        };

        let result = query.validate();
        assert!(result.is_err());

        println!("[VERIFIED] test_query_validation_no_active_spaces: Correctly rejects zero spaces");
    }

    #[tokio::test]
    async fn test_graceful_degradation() {
        // Test that query succeeds even if some spaces fail
        let executor = create_executor_with_failing_space(2).await;
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::ALL,
            ..Default::default()
        };

        let result = executor.execute(query).await.unwrap();

        assert_eq!(result.spaces_searched, 12); // 13 - 1 failed
        assert_eq!(result.spaces_failed, 1);

        println!("[VERIFIED] test_graceful_degradation: Continues with {} spaces despite failure",
            result.spaces_searched);
    }

    #[tokio::test]
    async fn test_latency_within_target() {
        let executor = create_test_executor().await;
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::ALL,
            final_limit: 10,
            ..Default::default()
        };

        let result = executor.execute(query).await.unwrap();

        // In-memory should be well under 60ms
        assert!(result.total_time.as_millis() < 60,
            "Query took {:?}, expected <60ms", result.total_time);

        println!("[VERIFIED] test_latency_within_target: Query completed in {:?}", result.total_time);
    }

    #[tokio::test]
    async fn test_splade_only_query() {
        let executor = create_test_executor().await;
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::SPLADE_ONLY,
            final_limit: 20,
            ..Default::default()
        };

        let result = executor.execute(query).await.unwrap();

        assert_eq!(result.spaces_searched, 1);

        println!("[VERIFIED] test_splade_only_query: SPLADE-only search works");
    }

    #[tokio::test]
    async fn test_hybrid_dense_sparse() {
        let executor = create_test_executor().await;
        let query = MultiEmbeddingQuery {
            query_text: "test query".into(),
            active_spaces: EmbeddingSpaceMask::HYBRID, // E1 + E13
            final_limit: 10,
            ..Default::default()
        };

        let result = executor.execute(query).await.unwrap();

        assert_eq!(result.spaces_searched, 2);

        println!("[VERIFIED] test_hybrid_dense_sparse: Hybrid search works");
    }

    #[tokio::test]
    async fn test_weighted_average_aggregation() {
        let mut weights = [0.0; NUM_EMBEDDERS];
        weights[0] = 1.0;  // E1 weight = 1.0
        weights[1] = 0.5;  // E2 weight = 0.5

        let strategy = AggregationStrategy::WeightedAverage {
            weights,
            require_all: false,
        };

        let matches = vec![(0, 0.8), (1, 0.6)];
        let score = strategy.aggregate(&matches);

        // (0.8 * 1.0 + 0.6 * 0.5) / (1.0 + 0.5) = 1.1 / 1.5 = 0.7333...
        let expected = 1.1 / 1.5;
        assert!((score - expected).abs() < 0.001,
            "Expected {}, got {}", expected, score);

        println!("[VERIFIED] test_weighted_average_aggregation: score={:.4}", score);
    }

    #[tokio::test]
    async fn test_max_pooling_aggregation() {
        let strategy = AggregationStrategy::MaxPooling;
        let matches = vec![(0, 0.8), (1, 0.6), (2, 0.9)];
        let score = strategy.aggregate(&matches);

        assert!((score - 0.9).abs() < 0.001);

        println!("[VERIFIED] test_max_pooling_aggregation: max={:.4}", score);
    }

    #[tokio::test]
    async fn test_purpose_weighted_aggregation() {
        let purpose = PurposeVector::new([0.5; NUM_EMBEDDERS]);
        let strategy = AggregationStrategy::PurposeWeighted {
            purpose_vector: purpose,
        };

        let matches = vec![(0, 0.8), (1, 0.6)];
        let score = strategy.aggregate(&matches);

        // With equal weights: (0.8 * 0.5 + 0.6 * 0.5) / (0.5 + 0.5) = 0.7
        assert!((score - 0.7).abs() < 0.001);

        println!("[VERIFIED] test_purpose_weighted_aggregation: score={:.4}", score);
    }

    #[tokio::test]
    async fn test_embedding_space_mask_presets() {
        assert_eq!(EmbeddingSpaceMask::ALL.active_count(), 13);
        assert_eq!(EmbeddingSpaceMask::ALL_DENSE.active_count(), 12);
        assert_eq!(EmbeddingSpaceMask::SEMANTIC_ONLY.active_count(), 1);
        assert_eq!(EmbeddingSpaceMask::TEXT_CORE.active_count(), 3);
        assert_eq!(EmbeddingSpaceMask::SPLADE_ONLY.active_count(), 1);
        assert_eq!(EmbeddingSpaceMask::HYBRID.active_count(), 2);

        assert!(EmbeddingSpaceMask::ALL.includes_splade());
        assert!(EmbeddingSpaceMask::ALL.includes_late_interaction());
        assert!(!EmbeddingSpaceMask::ALL_DENSE.includes_splade());

        println!("[VERIFIED] test_embedding_space_mask_presets: All presets correct");
    }

    #[tokio::test]
    async fn test_pipeline_stage_config_defaults() {
        let config = PipelineStageConfig::default();

        assert_eq!(config.splade_candidates, 1000);
        assert_eq!(config.matryoshka_128d_limit, 200);
        assert_eq!(config.full_search_limit, 100);
        assert_eq!(config.teleological_limit, 50);
        assert_eq!(config.late_interaction_limit, 20);
        assert!((config.rrf_k - 60.0).abs() < 0.001);
        assert!((config.min_alignment_threshold - 0.55).abs() < 0.001);

        println!("[VERIFIED] test_pipeline_stage_config_defaults: All defaults match constitution.yaml");
    }
}
```

### Verification Commands

```bash
# Build retrieval module
cargo build -p context-graph-core --features retrieval

# Run all retrieval tests
cargo test -p context-graph-core retrieval -- --nocapture

# Run specific test
cargo test -p context-graph-core test_rrf_aggregation_correctness -- --nocapture

# Run with timing output
cargo test -p context-graph-core retrieval -- --nocapture 2>&1 | grep "\[VERIFIED\]"

# Benchmark query performance
cargo bench -p context-graph-core -- multi_embedding

# Check for unused code
cargo clippy -p context-graph-core -- -D warnings
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/retrieval/mod.rs` | Module root with public exports |
| `crates/context-graph-core/src/retrieval/query.rs` | `MultiEmbeddingQuery`, `PipelineStageConfig`, `EmbeddingSpaceMask` |
| `crates/context-graph-core/src/retrieval/aggregation.rs` | `AggregationStrategy` with RRF implementation |
| `crates/context-graph-core/src/retrieval/result.rs` | `SpaceSearchResult`, `AggregatedMatch`, `PipelineStageTiming` |
| `crates/context-graph-core/src/retrieval/executor.rs` | `MultiEmbeddingQueryExecutor` trait, `SpaceInfo` |
| `crates/context-graph-core/src/retrieval/in_memory_executor.rs` | `InMemoryMultiEmbeddingExecutor` implementation |
| `crates/context-graph-core/src/retrieval/tests.rs` | Comprehensive test suite with real data |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/lib.rs` | Add `pub mod retrieval;` |
| `crates/context-graph-core/Cargo.toml` | Add `retrieval` feature flag (optional) |

## Full State Verification

### Source of Truth Identification

| Component | Source of Truth | Location |
|-----------|----------------|----------|
| 13 Embedding Spaces | `SemanticFingerprint` | `crates/context-graph-core/src/types/fingerprint/semantic.rs` |
| Pipeline Latency Targets | `constitution.yaml` | `docs2/constitution.yaml` (retrieval-pipeline section) |
| RRF Formula | `constitution.yaml` | `RRF(d) = Σᵢ 1/(k + rankᵢ(d))` with k=60 |
| Alignment Thresholds | `constitution.yaml` | optimal ≥0.75, critical <0.55 |
| Error Types | `CoreError` | `crates/context-graph-core/src/error.rs` |
| Trait Signatures | Existing traits | `crates/context-graph-core/src/traits/` |

### Execute & Inspect Requirements

After implementation, manually verify:

1. **Module Compilation**
   ```bash
   cargo build -p context-graph-core 2>&1 | head -20
   # Expected: Compiling context-graph-core... Finished
   ```

2. **Test Output Inspection**
   ```bash
   cargo test -p context-graph-core retrieval -- --nocapture 2>&1 | grep "\[VERIFIED\]"
   # Expected: All 15+ tests show [VERIFIED]
   ```

3. **RRF Formula Verification**
   ```bash
   cargo test -p context-graph-core test_rrf_aggregation_correctness -- --nocapture
   # Verify exact values match formula
   ```

### Boundary & Edge Case Audit

| # | Edge Case | Test | Expected Behavior |
|---|-----------|------|-------------------|
| 1 | Empty query text | `test_query_validation_empty_text` | `CoreError::ValidationError` with field="query_text" |
| 2 | Zero active spaces | `test_query_validation_no_active_spaces` | `CoreError::ValidationError` with field="active_spaces" |
| 3 | All spaces fail except one | `test_graceful_degradation` | Returns results from successful space, `spaces_failed=12` |
| 4 | RRF with single space | `test_single_space_query` | Valid RRF scores (1/(k+rank+1)) |
| 5 | Max results exactly at limit | Verify `final_limit` enforced | Never exceeds `final_limit` |
| 6 | Latency at boundary (59ms vs 61ms) | `test_latency_within_target` | Log warning if >60ms |
| 7 | Sparse vector with no overlap | Test E13 SPLADE with disjoint indices | Score = 0.0 |
| 8 | per_space_limit=0 | Validation | `CoreError::ValidationError` |
| 9 | min_similarity=1.0 | Search returns empty | Valid empty result |
| 10 | Duplicate memory_id across spaces | RRF aggregation | Correctly sums contributions |

### Evidence of Success

The implementation is complete when:

1. **All Tests Pass with [VERIFIED] Output**
   ```
   [VERIFIED] test_single_space_query: spaces_searched=1, results=5
   [VERIFIED] test_all_spaces_query: searched all 13 spaces
   [VERIFIED] test_rrf_aggregation_correctness: id1=0.0489, id2=0.0484, id3=0.0479
   [VERIFIED] test_query_validation_empty_text: Correctly rejects empty query
   [VERIFIED] test_graceful_degradation: Continues with 12 spaces despite failure
   [VERIFIED] test_latency_within_target: Query completed in 5ms
   ...
   ```

2. **Cargo Build Success**
   ```bash
   cargo build -p context-graph-core 2>&1 | tail -1
   # Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
   ```

3. **No Clippy Warnings**
   ```bash
   cargo clippy -p context-graph-core -- -D warnings 2>&1 | grep "warning:"
   # (empty output)
   ```

4. **Module Exports Verified**
   ```rust
   // In lib.rs or any consumer:
   use context_graph_core::retrieval::{
       MultiEmbeddingQuery, MultiEmbeddingQueryExecutor,
       EmbeddingSpaceMask, AggregationStrategy,
   };
   ```

## Traceability Matrix

| Requirement | Source | Implementation | Test |
|-------------|--------|----------------|------|
| 13-space parallel search | constitution.yaml | `execute()` with `active_spaces: ALL` | `test_all_spaces_query` |
| RRF fusion (k=60) | constitution.yaml | `aggregate_rrf()` | `test_rrf_aggregation_correctness` |
| <60ms latency | constitution.yaml | Timing instrumentation | `test_latency_within_target` |
| Graceful degradation | constitution.yaml | Continue on space failure | `test_graceful_degradation` |
| SPLADE sparse search | contextprd.md | `EmbeddingSpaceMask::SPLADE_ONLY` | `test_splade_only_query` |
| Matryoshka 128D | contextprd.md | `e1_matryoshka_128()` | Via Stage 2 |
| Purpose-weighted | learntheory.md | `AggregationStrategy::PurposeWeighted` | `test_purpose_weighted_aggregation` |
| Fail-fast validation | CLAUDE.md | `query.validate()` | `test_query_validation_*` |

## Sherlock-Holmes Verification Checklist

Before marking this task complete, run the `sherlock-holmes` subagent with:

```
Task: Verify TASK-L001 Multi-Embedding Query Executor implementation

Verification Points:
1. All 7 files created in crates/context-graph-core/src/retrieval/
2. lib.rs contains `pub mod retrieval;`
3. All 15+ tests pass with [VERIFIED] output
4. RRF formula matches: RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1) with k=60
5. No CoreError::Internal or panics in any test
6. Graceful degradation test shows spaces_failed=1
7. Latency test completes in <60ms
8. EmbeddingSpaceMask::ALL.active_count() == 13
9. PipelineStageConfig::default().rrf_k == 60.0
10. No clippy warnings with -D warnings

Run: cargo test -p context-graph-core retrieval -- --nocapture
Expected: All tests pass, all [VERIFIED] markers present
```

---

*Task created: 2026-01-04*
*Updated: 2026-01-05 (comprehensive audit, fail-fast, real data tests, sherlock verification)*
*Layer: Logic*
*Priority: P0 - Core retrieval functionality*
*Dependencies: TASK-F001 ✅, TASK-F005 ✅, TASK-F007 ✅*
