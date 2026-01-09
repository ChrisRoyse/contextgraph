//! Teleological module: Cross-embedding synergy and fusion capabilities.
//!
//! This module implements the TELEO Foundation Layer for the 13-embedder system,
//! providing types and structures for teleological vector fusion, meaning extraction,
//! and task-specific retrieval profiles.
//!
//! # Architecture
//!
//! From teleoplan.md: "The 13 embeddings are not just parallel storage - they are
//! 13 distinct **knowledge lenses** that, when combined into a teleological vector,
//! create a representation with **exponentially richer meaning** than any single embedding."
//!
//! ## Key Components
//!
//! - **SynergyMatrix**: 13x13 cross-embedding synergy weights
//! - **TeleologicalVector**: Fused multi-embedding representation
//! - **GroupAlignments**: 6D hierarchical group aggregation
//! - **MultiResolutionHierarchy**: 4-level purpose vector hierarchy
//! - **MeaningExtractionConfig**: Configuration for semantic extraction
//! - **TeleologicalProfile**: Task-specific fusion configurations
//!
//! ## Embedding Groups (from teleoplan.md)
//!
//! | Group | Embeddings | Captures |
//! |-------|------------|----------|
//! | Factual | E1, E12, E13 | what IS |
//! | Temporal | E2, E3 | when/sequence |
//! | Causal | E4, E7 | why/how |
//! | Relational | E5, E8, E9 | like/where/who |
//! | Qualitative | E10, E11 | feel/principle |
//! | Implementation | E6 | code |
//!
//! ## Multi-Resolution Hierarchy
//!
//! - Level 0: Full 13D Purpose Vector
//! - Level 1: 6D Group Purpose Vector
//! - Level 2: 3D Core Purpose Vector (What/How/Why)
//! - Level 3: 1D Purpose Alignment Score
//!
//! ## Example
//!
//! ```
//! use context_graph_core::teleological::{
//!     SynergyMatrix, TeleologicalProfile, TaskType,
//!     GroupAlignments, MultiResolutionHierarchy,
//! };
//!
//! // Create a synergy matrix with base values
//! let synergy = SynergyMatrix::with_base_synergies();
//! assert!(synergy.get_synergy(0, 4) > 0.8); // E1 + E5 = strong synergy
//!
//! // Create a task-specific profile
//! let profile = TeleologicalProfile::code_implementation();
//! assert!(profile.get_weight(5) > 0.2); // E6 (Code) boosted
//!
//! // Build multi-resolution hierarchy
//! let alignments = [0.8f32; 13];
//! let hierarchy = MultiResolutionHierarchy::from_raw(alignments);
//! assert!(hierarchy.quick_score() > 0.7);
//! ```

// Canonical Embedder enumeration (SINGLE source of truth for embedder types)
pub mod embedder;

// Core type definitions
pub mod types;

// 13x13 synergy matrix
pub mod synergy_matrix;

// Group alignments and hierarchical aggregation
pub mod groups;

// Multi-resolution purpose vector hierarchy
pub mod resolution;

// TeleologicalVector: fused representation
pub mod vector;

// Meaning extraction configuration
pub mod meaning;

// Task-specific profiles
pub mod profile;

// Matrix search: cross-correlation search across all 13 embedders
pub mod matrix_search;

// Comparison validation errors (TASK-CORE-004)
pub mod comparison_error;

// Teleological services (TELEO-007 through TELEO-015)
pub mod services;

// Re-exports for convenience
pub use comparison_error::{ComparisonValidationError, ComparisonValidationResult, WeightValues};
pub use embedder::{Embedder, EmbedderDims, EmbedderGroup, EmbedderMask};
pub use groups::{GroupAlignments, GroupType};
pub use matrix_search::{
    embedder_names, ComparisonScope, ComponentWeights, ComprehensiveComparison, MatrixSearchConfig,
    SearchStrategy, SimilarityBreakdown, TeleologicalMatrixSearch,
};
pub use meaning::{
    CrossEmbeddingAnalysis, ExtractedMeaning, FusionMethod, MeaningExtractionConfig,
    NuanceDimension,
};
pub use profile::{FusionStrategy, ProfileMetrics, TaskType, TeleologicalProfile};
pub use resolution::{DomainAlignments, DomainType, MultiResolutionHierarchy, ResolutionView};
pub use synergy_matrix::{SynergyMatrix, CROSS_CORRELATION_COUNT, SYNERGY_DIM};
pub use types::{ProfileId, TuckerCore, EMBEDDING_DIM, NUM_EMBEDDERS};
pub use vector::TeleologicalVector;
