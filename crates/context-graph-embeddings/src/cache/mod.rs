//! Embedding cache system for O(1) lookup of FusedEmbedding results.
//!
//! This module provides caching infrastructure for the embedding pipeline,
//! using xxHash64 content hashes for fast key lookup.
//!
//! # Types
//!
//! - [`CacheKey`]: 8-byte content hash for HashMap key usage
//! - [`CacheEntry`]: Cached embedding with LRU/LFU metadata
//!
//! # Example
//!
//! ```rust,ignore
//! use context_graph_embeddings::cache::{CacheKey, CacheEntry};
//! use context_graph_embeddings::types::ModelInput;
//!
//! let input = ModelInput::text("Hello world").unwrap();
//! let key = CacheKey::from_input(&input);
//!
//! // Later, create cache entry from computed embedding
//! let entry = CacheEntry::new(fused_embedding);
//! entry.touch(); // Update LRU timestamp
//! ```

pub mod types;

pub use types::{CacheEntry, CacheKey};
