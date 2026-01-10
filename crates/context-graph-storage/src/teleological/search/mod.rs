//! Single embedder search for HNSW indexes.
//!
//! # Overview
//!
//! Provides k-nearest-neighbor search against individual embedder indexes.
//! This is Stage 2/3 of the 5-stage teleological retrieval pipeline.
//!
//! # Supported Embedders
//!
//! 12 HNSW-capable embedders:
//! - E1Semantic (1024D) - Primary semantic embeddings
//! - E1Matryoshka128 (128D) - Truncated Matryoshka for fast filtering
//! - E2TemporalRecent (512D) - Recent event emphasis
//! - E3TemporalPeriodic (512D) - Periodic pattern detection
//! - E4TemporalPositional (512D) - Position-based temporal
//! - E5Causal (1024D) - Causal relationship modeling
//! - E7Code (1024D) - Code-specific embeddings
//! - E8Graph (384D) - Graph structure embeddings
//! - E9HDC (10000D) - Hyperdimensional computing
//! - E10Multimodal (1024D) - Cross-modal embeddings
//! - E11Entity (384D) - Named entity embeddings
//! - PurposeVector (13D) - Teleological purpose vectors
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

mod error;
mod result;
mod single;

// Re-export error types
pub use error::{SearchError, SearchResult};

// Re-export result types
pub use result::{EmbedderSearchHit, SingleEmbedderSearchResults};

// Re-export search types
pub use single::{SingleEmbedderSearch, SingleEmbedderSearchConfig};
