//! Search module for HNSW indexes.
//!
//! # Overview
//!
//! Provides k-nearest-neighbor search against individual and multiple embedder indexes.
//! Supports both single-embedder and multi-embedder parallel search.
//!
//! # Components
//!
//! - **single**: Single embedder HNSW search (Stage 2/3 of pipeline)
//! - **multi**: Multi-embedder parallel search with aggregation
//!
//! # Supported Embedders
//!
//! 11 HNSW-capable embedders:
//! - E1Semantic (1024D) - Primary semantic embeddings
//! - E1Matryoshka128 (128D) - Truncated Matryoshka for fast filtering
//! - E2TemporalRecent (512D) - Recent event emphasis
//! - E3TemporalPeriodic (512D) - Periodic pattern detection
//! - E4TemporalPositional (512D) - Position-based temporal
//! - E5Causal (768D) - Causal relationship modeling
//! - E7Code (1536D) - Code-specific embeddings
//! - E8Graph (1024D) - Graph structure embeddings
//! - E9HDC (1024D) - Hyperdimensional computing
//! - E10Multimodal (768D) - Cross-modal embeddings
//! - E11Entity (768D) - Named entity embeddings
//!
//! # NOT Supported (Different Algorithms)
//!
//! - E6Sparse - Requires inverted index with BM25
//! - E12LateInteraction - Requires ColBERT MaxSim token-level
//! - E13Splade - Requires inverted index with learned expansion
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! All errors are fatal. No recovery attempts. This ensures:
//! - Bugs are caught early in development
//! - Data integrity is preserved
//! - Clear error messages for debugging
//!
//! # Example
//!
//! ```no_run
//! use context_graph_storage::teleological::search::{
//!     // Single embedder search
//!     SingleEmbedderSearch, SingleEmbedderSearchConfig,
//!     // Multi-embedder search
//!     MultiEmbedderSearch, MultiSearchBuilder,
//!     NormalizationStrategy, AggregationStrategy,
//! };
//! use context_graph_storage::teleological::indexes::{
//!     EmbedderIndex, EmbedderIndexRegistry,
//! };
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! let registry = Arc::new(EmbedderIndexRegistry::new());
//!
//! // Single embedder search
//! let single_search = SingleEmbedderSearch::new(Arc::clone(&registry));
//! let query = vec![0.5f32; 1024];
//! let results = single_search.search(EmbedderIndex::E1Semantic, &query, 10, None);
//!
//! // Multi-embedder parallel search
//! let multi_search = MultiEmbedderSearch::new(registry);
//! let queries: HashMap<EmbedderIndex, Vec<f32>> = [
//!     (EmbedderIndex::E1Semantic, vec![0.5f32; 1024]),
//!     (EmbedderIndex::E8Graph, vec![0.5f32; 1024]),
//! ].into_iter().collect();
//!
//! let results = MultiSearchBuilder::new(queries)
//!     .k(10)
//!     .normalization(NormalizationStrategy::MinMax)
//!     .aggregation(AggregationStrategy::Max)
//!     .execute(&multi_search);
//! ```

mod error;
mod matrix;
mod maxsim;
mod multi;
mod pipeline;
mod result;
mod single;
pub mod temporal_boost;
mod token_storage;

// Re-export error types
pub use error::{SearchError, SearchResult};

// Re-export result types (single embedder)
pub use result::{EmbedderSearchHit, SingleEmbedderSearchResults};

// Re-export single embedder search types
pub use single::{SingleEmbedderSearch, SingleEmbedderSearchConfig};

// Re-export multi-embedder search types
pub use multi::{
    // Result types
    AggregatedHit,
    AggregationStrategy,
    // Search struct and builder
    MultiEmbedderSearch,
    // Configuration
    MultiEmbedderSearchConfig,
    MultiEmbedderSearchResults,
    MultiSearchBuilder,
    // Strategy enums
    NormalizationStrategy,
    PerEmbedderResults,
};

// Re-export matrix strategy search types
pub use matrix::{
    // Correlation types
    CorrelationAnalysis,
    CorrelationPattern,
    MatrixAnalysis,
    MatrixSearchBuilder,
    // Result types
    MatrixSearchResults,
    // Search struct and builder
    MatrixStrategySearch,
    // Matrix types
    SearchMatrix,
};

// Re-export pipeline types
pub use pipeline::{
    InMemorySpladeIndex,
    // In-memory implementations for testing
    InMemoryTokenStorage,
    PipelineBuilder,
    PipelineCandidate,
    // Configuration
    PipelineConfig,
    // Error types
    PipelineError,
    // Result types
    PipelineResult,
    // Stage enum
    PipelineStage,
    // Pipeline struct and builder
    RetrievalPipeline,
    SpladeIndex,
    StageConfig,
    StageResult,
    // Storage traits
    TokenStorage,
};

// Re-export MaxSim scorer types (TASK-STORAGE-P2-001)
pub use maxsim::{
    // Standalone MaxSim computation
    compute_maxsim_direct,
    // SIMD-optimized cosine similarity
    cosine_similarity_128d,
    // Scorer struct
    MaxSimScorer,
    // Constants
    E12_TOKEN_DIM,
};

// Re-export RocksDB token storage (TASK-STORAGE-P2-001)
pub use token_storage::{
    // Storage struct
    RocksDbTokenStorage,
    // Error types
    TokenStorageError,
    TokenStorageResult,
    // Constants
    MAX_TOKENS_PER_MEMORY,
};
