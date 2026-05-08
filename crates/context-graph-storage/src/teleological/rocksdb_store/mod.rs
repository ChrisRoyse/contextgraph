//! RocksDB-backed TeleologicalMemoryStore implementation.
//!
//! This module provides a persistent storage implementation for TeleologicalFingerprints
//! using RocksDB with 102 column families (11 base + 41 teleological + 14 quantized
//! + 5 code + 2 causal + 29 DynamicJEPA).
//!
//! # Column Families Used
//!
//! - `fingerprints`: Primary storage for ~63KB TeleologicalFingerprints
//! - `topic_profiles`: 13D topic profiles per memory
//! - `e13_splade_inverted`: Inverted index for Stage 1 (Recall) sparse search
//! - `e1_matryoshka_128`: E1 truncated 128D vectors for Stage 2 (Semantic ANN)
//! - `e12_late_interaction`: ColBERT token embeddings for Stage 5 (MaxSim rerank)
//! - `emb_0` through `emb_13`: Per-embedder quantized storage
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

mod anomaly_derivation;
mod audit_log;
mod causal_hnsw_index;
mod causal_relationships;
mod constellation;
mod content;
mod contrastive;
mod crud;
pub mod export;
mod file_index;
mod fusion;
mod helpers;
mod index_ops;
mod inverted_index;
mod learner;
mod learner_training;
mod learning;
mod llm_validation;
mod persistence;
mod provenance_storage;
mod search;
mod source_metadata;
mod store;
mod trait_impl;
pub mod typed_edge_export;
mod typed_edge_keys;
mod types;
mod versioned_bincode;

#[cfg(test)]
mod tests;

// Re-export all public types for backwards compatibility
// Audit-14 STOR-L1 FIX: weighted_rrf_fusion and compute_consensus are #[cfg(test)] only.
pub use anomaly_derivation::{
    classify_edge_as_anomaly, AnomalyDerivationConfig, AnomalyDerivationSummary,
};
pub use export::{decode_training_record, encode_training_record};
pub use fusion::{weighted_rrf_fusion_with_scores, RRF_K};
pub use helpers::{compute_cosine_similarity, hex_encode, hnsw_distance_to_similarity};
pub use learner::{
    decode_goal_centroid, decode_learner_constellation, decode_learner_delta_log,
    decode_learner_fingerprint, decode_learner_goal_state, decode_learner_k_sleep,
    decode_learner_m_trace, decode_learner_profile, decode_learner_retrieval_log,
    decode_learner_state_vector, encode_goal_centroid, encode_learner_audit_entry,
    encode_learner_constellation, encode_learner_delta_log, encode_learner_fingerprint,
    encode_learner_goal_state, encode_learner_k_sleep, encode_learner_m_trace,
    encode_learner_profile, encode_learner_retrieval_log, encode_learner_state_vector,
    learner_audit_key, learner_constellation_key, learner_session_key, learner_skill_key,
    learner_trace_key, learner_trace_ts_key,
};
pub use learner_training::{decode_learner_training_dataset, encode_learner_training_dataset};
pub use learning::{decode_learning_event, encode_learning_event};
pub use llm_validation::{decode_llm_edge_validation, encode_llm_edge_validation};
pub use store::RocksDbTeleologicalStore;
pub use typed_edge_export::{decode_typed_edge_record, encode_typed_edge_record};
pub use typed_edge_keys::typed_edge_record_key;
pub use types::{TeleologicalStoreConfig, TeleologicalStoreError, TeleologicalStoreResult};

// Re-export core file index types for convenience
pub use context_graph_core::types::file_index::{FileIndexEntry, FileWatcherStats};

// Re-export causal HNSW index
pub use causal_hnsw_index::CausalE11Index;
