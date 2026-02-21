//! TeleologicalComparator: Apples-to-apples comparison across 13 embedders.
//!
//! Routes to correct similarity function per embedder type, applies weights
//! and synergy matrices, returns detailed comparison results.
//!
//! # Design Philosophy
//!
//! From constitution.yaml ARCH-02: "Compare Only Compatible Embedding Types (Apples-to-Apples)"
//! - E1 compares with E1, E5 with E5, NEVER cross-embedder
//! - Each embedder has a specific output type (dense/sparse/token-level)
//! - The comparator dispatches to the correct similarity function per embedder
//! - Results are aggregated according to SearchStrategy and ComponentWeights
//!
//! # Embedder Type Mapping
//!
//! | Index | Embedder | Type | Similarity Function |
//! |-------|----------|------|---------------------|
//! | 0 | E1 Semantic | Dense | cosine_similarity |
//! | 1 | E2 TemporalRecent | Dense | cosine_similarity |
//! | 2 | E3 TemporalPeriodic | Dense | cosine_similarity |
//! | 3 | E4 TemporalPositional | Dense | cosine_similarity |
//! | 4 | E5 Causal | Dense | cosine_similarity |
//! | 5 | E6 Sparse | Sparse | jaccard_similarity |
//! | 6 | E7 Code | Dense | cosine_similarity |
//! | 7 | E8 Graph | Dense | cosine_similarity |
//! | 8 | E9 HDC | Dense | cosine_similarity |
//! | 9 | E10 Multimodal | Dense | cosine_similarity |
//! | 10 | E11 Entity | Dense | cosine_similarity |
//! | 11 | E12 LateInteraction | TokenLevel | max_sim |
//! | 12 | E13 KeywordSplade | Sparse | jaccard_similarity |

// Submodules
mod batch;
mod result;
mod strategies;
mod teleological;

#[cfg(test)]
mod tests;

// Re-exports for backwards compatibility
pub use batch::BatchComparator;
pub use result::ComparisonResult;
pub use teleological::TeleologicalComparator;
