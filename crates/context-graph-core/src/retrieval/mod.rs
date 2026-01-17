//! Multi-embedding query executor module.
//!
//! This module provides the core retrieval infrastructure for searching across
//! the 13 embedding spaces defined in `SemanticFingerprint`. It implements
//! a 5-stage retrieval pipeline with RRF (Reciprocal Rank Fusion) aggregation.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                     Multi-Embedding Query Executor                       │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  Query Input                                                             │
//! │  ├── query_text: String                                                  │
//! │  ├── active_spaces: EmbeddingSpaceMask (bitmask 0x1FFF)                 │
//! │  └── pipeline_config: PipelineStageConfig                               │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  5-Stage Pipeline                                                        │
//! │  ├── Stage 1: SPLADE Recall (<5ms, 1000 candidates)                     │
//! │  ├── Stage 2: Matryoshka 128D Filter (<10ms, 200 candidates)            │
//! │  ├── Stage 3: Full 13-Space HNSW (<20ms, 100 candidates)                │
//! │  ├── Stage 4: Teleological Alignment (<10ms, 50 candidates)             │
//! │  └── Stage 5: Late Interaction Rerank (<15ms, final ranking)            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  Aggregation Strategy                                                    │
//! │  └── RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1), k=60                            │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 13 Embedding Spaces
//!
//! | Index | Name | Dimension | Type |
//! |-------|------|-----------|------|
//! | 0 | E1_Semantic | 1024 | Dense HNSW |
//! | 1 | E2_Temporal_Recent | 512 | Dense HNSW |
//! | 2 | E3_Temporal_Periodic | 512 | Dense HNSW |
//! | 3 | E4_Temporal_Positional | 512 | Dense HNSW |
//! | 4 | E5_Causal | 768 | Dense HNSW |
//! | 5 | E6_Sparse | variable | Sparse Inverted |
//! | 6 | E7_Code | 1536 | Dense HNSW |
//! | 7 | E8_Graph | 384 | Dense HNSW |
//! | 8 | E9_HDC | 1024 | Dense HNSW |
//! | 9 | E10_Multimodal | 768 | Dense HNSW |
//! | 10 | E11_Entity | 384 | Dense HNSW |
//! | 11 | E12_Late_Interaction | 128×N | ColBERT MaxSim |
//! | 12 | E13_SPLADE | 30522 | Sparse Inverted |
//!
//! # Performance Targets (constitution.yaml)
//!
//! - Total latency: <60ms @ 1M memories
//! - Query embedding: <30ms
//! - Each stage has specific latency targets (see pipeline above)
//!
//! # Example
//!
//! ```ignore
//! use context_graph_core::retrieval::{
//!     MultiEmbeddingQuery, EmbeddingSpaceMask, MultiEmbeddingQueryExecutor,
//!     InMemoryMultiEmbeddingExecutor,
//! };
//! use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};
//!
//! // Create executor
//! let store = InMemoryTeleologicalStore::new();
//! let provider = StubMultiArrayProvider::new();
//! let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);
//!
//! // Build query
//! let query = MultiEmbeddingQuery {
//!     query_text: "How does memory consolidation work?".to_string(),
//!     active_spaces: EmbeddingSpaceMask::ALL,
//!     final_limit: 10,
//!     ..Default::default()
//! };
//!
//! // Execute
//! let result = executor.execute(query).await?;
//! assert!(result.within_latency_target());
//! ```

mod aggregation;
pub mod config;
pub mod detector;
pub mod distance;
pub mod divergence;
mod executor;
pub mod multi_space;
mod pipeline;
mod query;
mod result;
pub mod similarity;
mod teleological_query;
mod teleological_result;

// AP-007: In-memory executor uses stubs and is TEST ONLY
#[cfg(test)]
mod in_memory_executor;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use aggregation::AggregationStrategy;
pub use executor::{IndexType, MultiEmbeddingQueryExecutor, SpaceInfo};
pub use pipeline::{DefaultTeleologicalPipeline, PipelineHealth, TeleologicalRetrievalPipeline};

// AP-007: InMemoryMultiEmbeddingExecutor is TEST ONLY - uses stubs
#[cfg(test)]
pub use in_memory_executor::InMemoryMultiEmbeddingExecutor;
pub use query::{EmbeddingSpaceMask, MultiEmbeddingQuery, PipelineStageConfig};
pub use result::{
    AggregatedMatch, MultiEmbeddingResult, PipelineStageTiming, ScoredMatch, SpaceContribution,
    SpaceSearchResult,
};
pub use teleological_query::TeleologicalQuery;
pub use teleological_result::{
    AlignmentLevel, PipelineBreakdown, ScoredMemory, TeleologicalRetrievalResult,
};

// Similarity types for retrieval
pub use similarity::{PerSpaceScores, SimilarityResult, NUM_SPACES};

// Divergence types for topic drift detection
pub use divergence::{
    DivergenceAlert, DivergenceReport, DivergenceSeverity,
    DIVERGENCE_SPACES, MAX_SUMMARY_LEN, truncate_summary,
};

// Threshold and weight configuration types
pub use config::{
    high_thresholds, low_thresholds, default_weights,
    PerSpaceThresholds, SimilarityThresholds, SpaceWeights,
    RECENT_LOOKBACK_SECS, MAX_RECENT_MEMORIES, SPACE_WEIGHTS, TOTAL_WEIGHT,
};

// Distance and similarity computation functions
pub use distance::{
    compute_all_similarities, compute_similarity_for_space, cosine_similarity,
    hamming_similarity, jaccard_similarity, max_sim, transe_similarity,
};

// Multi-space similarity service
pub use multi_space::{
    compute_similarities_batch, filter_relevant, sort_by_relevance, MultiSpaceSimilarity,
};

// Divergence detection service
pub use detector::{
    DivergenceDetector, RecentMemory, is_divergence_space,
};
