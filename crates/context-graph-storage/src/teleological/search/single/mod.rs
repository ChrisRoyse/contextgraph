//! Single embedder HNSW search.
//!
//! Searches ONE of the 11 HNSW-capable indexes for k nearest neighbors.
//!
//! # Supported Embedders (HNSW)
//!
//! | Embedder | Dimension | Use Case |
//! |----------|-----------|----------|
//! | E1Semantic | 1024D | General meaning |
//! | E1Matryoshka128 | 128D | Stage 2 fast filter |
//! | E2TemporalRecent | 512D | Recency |
//! | E3TemporalPeriodic | 512D | Cycles |
//! | E4TemporalPositional | 512D | Who/what |
//! | E5Causal | 768D | Why/because |
//! | E7Code | 1536D | Code/tech |
//! | E8Graph | 1024D | Graph |
//! | E9HDC | 1024D | Structure |
//! | E10Multimodal | 768D | Intent |
//! | E11Entity | 768D | Multi-modal |
//!
//! # NOT Supported (different algorithms)
//!
//! - E6Sparse (inverted index)
//! - E12LateInteraction (MaxSim token-level)
//! - E13Splade (inverted index)
//!
//! # FAIL FAST Policy
//!
//! All validation errors are fatal. No fallbacks.
//!
//! # Example
//!
//! ```no_run
//! use context_graph_storage::teleological::search::{
//!     SingleEmbedderSearch, SingleEmbedderSearchConfig,
//! };
//! use context_graph_storage::teleological::indexes::{
//!     EmbedderIndex, EmbedderIndexRegistry,
//! };
//! use std::sync::Arc;
//!
//! // Create registry and search
//! let registry = Arc::new(EmbedderIndexRegistry::new());
//! let search = SingleEmbedderSearch::new(registry);
//!
//! // Search E1 Semantic (1024D)
//! let query = vec![0.5f32; 1024];
//! let results = search.search(EmbedderIndex::E1Semantic, &query, 10, None);
//! ```

mod config;
mod search;

#[cfg(test)]
mod tests;

// Re-export for backwards compatibility
pub use self::config::SingleEmbedderSearchConfig;
pub use self::search::SingleEmbedderSearch;
