//! Index module for multi-space HNSW and inverted indexes.
//!
//! # Architecture
//!
//! This module implements the index layer for the 5-stage retrieval pipeline:
//!
//! | Stage | Index Type | Purpose |
//! |-------|------------|---------|
//! | 1 | SPLADE Inverted | BM25+SPLADE hybrid recall |
//! | 2 | Matryoshka 128D HNSW | Fast ANN filtering |
//! | 3 | Multi-space HNSW | Dense semantic search |
//! | 4 | Purpose Vector HNSW | Teleological alignment |
//! | 5 | ColBERT MaxSim | Late interaction reranking |
//!
//! # Index Counts
//!
//! - 10 dense HNSW indexes (E1-E5, E7-E11)
//! - 1 Matryoshka 128D HNSW (E1 truncated for Stage 2)
//! - 1 PurposeVector 13D HNSW (Stage 4)
//! - 1 SPLADE inverted index (Stage 1)
//!
//! Total: 12 HNSW + 1 inverted = 13 indexes managed
//!
//! # Usage
//!
//! ```ignore
//! use context_graph_core::index::{HnswMultiSpaceIndex, MultiSpaceIndexManager};
//!
//! let mut manager = HnswMultiSpaceIndex::new();
//! manager.initialize().await?;
//!
//! // Add fingerprint to all indexes
//! manager.add_fingerprint(memory_id, &fingerprint).await?;
//!
//! // Stage 1: SPLADE recall
//! let candidates = manager.search_splade(&sparse_query, 10000).await?;
//!
//! // Stage 2: Matryoshka filtering
//! let filtered = manager.search_matryoshka(&query_128d, 1000).await?;
//!
//! // Stage 3: Dense semantic search
//! let results = manager.search(EmbedderIndex::E1Semantic, &query, 100).await?;
//!
//! // Stage 4: Purpose alignment
//! let aligned = manager.search_purpose(&purpose_query, 50).await?;
//! ```
//!
//! # Error Handling
//!
//! All operations use fail-fast semantics:
//! - Dimension mismatches error immediately
//! - Invalid embedder operations error immediately
//! - NO fallbacks or silent failures

pub mod config;
pub mod error;
pub mod hnsw_impl;
pub mod manager;
pub mod splade_impl;
pub mod status;

// Re-exports for convenient access
pub use error::{IndexError, IndexResult};
pub use hnsw_impl::{HnswMultiSpaceIndex, SimpleHnswIndex};
pub use manager::MultiSpaceIndexManager;
pub use splade_impl::SpladeInvertedIndex;
pub use status::{IndexHealth, IndexStatus, MultiIndexHealth};

// Re-export key types from local config (avoiding cyclic dependency with context-graph-storage)
pub use config::{
    DistanceMetric, EmbedderIndex, HnswConfig, InvertedIndexConfig,
    E1_DIM, E1_MATRYOSHKA_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E6_SPARSE_VOCAB,
    E7_DIM, E8_DIM, E9_DIM, E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB,
    NUM_EMBEDDERS, PURPOSE_VECTOR_DIM,
};
