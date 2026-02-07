//! RocksDB-backed TeleologicalMemoryStore implementation.
//!
//! This module provides a persistent storage implementation for TeleologicalFingerprints
//! using RocksDB with 17 column families for efficient indexing and retrieval.
//!
//! # Column Families Used
//!
//! - `fingerprints`: Primary storage for ~63KB TeleologicalFingerprints
//! - `topic_profiles`: 13D topic profiles per memory
//! - `e13_splade_inverted`: Inverted index for Stage 1 (Recall) sparse search
//! - `e1_matryoshka_128`: E1 truncated 128D vectors for Stage 2 (Semantic ANN)
//! - `e12_late_interaction`: ColBERT token embeddings for Stage 5 (MaxSim rerank)
//! - `emb_0` through `emb_12`: Per-embedder quantized storage
//!
//! # FAIL FAST Policy
//!
//! **NO FALLBACKS. NO MOCK DATA. ERRORS ARE FATAL.**
//!
//! Every RocksDB operation that fails returns a detailed error with:
//! - The operation that failed
//! - The column family involved
//! - The key being accessed
//! - The underlying RocksDB error
//!
//! # Thread Safety
//!
//! The store is thread-safe for concurrent reads and writes via RocksDB's internal locking.
//! HNSW indexes are protected by `RwLock` for concurrent query access.
//!
//! # Module Structure
//!
//! - `types`: Error types, configuration, and result aliases
//! - `helpers`: Utility functions for similarity computation
//! - `store`: Core RocksDbTeleologicalStore struct and constructors
//! - `index_ops`: HNSW index add/remove operations
//! - `inverted_index`: SPLADE inverted index operations
//! - `crud`: CRUD operation implementations
//! - `search`: Search operation implementations
//! - `persistence`: Batch, statistics, persistence operations
//! - `content`: Content storage operations
//! - `source_metadata`: Source metadata storage operations
//! - `trait_impl`: TeleologicalMemoryStore trait implementation (thin wrapper)
//! - `tests`: Comprehensive test suite

mod audit_log;
mod causal_hnsw_index;
mod causal_relationships;
mod content;
mod crud;
mod file_index;
mod fusion;
mod helpers;
mod index_ops;
mod inverted_index;
mod persistence;
mod provenance_storage;
mod search;
mod source_metadata;
mod store;
mod trait_impl;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types for backwards compatibility
pub use fusion::{compute_consensus, weighted_rrf_fusion, weighted_rrf_fusion_with_scores, RRF_K};
pub use helpers::compute_cosine_similarity;
pub use store::RocksDbTeleologicalStore;
pub use types::{TeleologicalStoreConfig, TeleologicalStoreError, TeleologicalStoreResult};

// Re-export core file index types for convenience
pub use context_graph_core::types::file_index::{FileIndexEntry, FileWatcherStats};

// Re-export causal HNSW index
pub use causal_hnsw_index::CausalE11Index;
