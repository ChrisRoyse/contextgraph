//! Embedding cache system for O(1) lookup of FusedEmbedding results.
//!
//! This module provides caching infrastructure for the embedding pipeline,
//! using xxHash64 content hashes for fast key lookup.
//!
//! # Types
//!
//! - [`CacheKey`]: 8-byte content hash for HashMap key usage
//! - [`CacheEntry`]: Cached embedding with LRU/LFU metadata
//! - [`CacheManager`]: LRU-based cache with thread-safe operations
//! - [`CacheMetrics`]: Atomic counters for cache statistics
//!
//! # Example
//!
//! ```
//! use context_graph_embeddings::cache::CacheKey;
//! use context_graph_embeddings::config::CacheConfig;
//!
//! // Create cache key from content
//! let key = CacheKey::from_content("Hello world");
//! assert!(key.content_hash != 0);
//!
//! // Verify cache config defaults
//! let config = CacheConfig::default();
//! assert!(config.enabled);
//! assert!(config.max_entries > 0);
//! ```

pub mod manager;
mod types;

pub use manager::{CacheManager, CacheMetrics};
pub use types::{CacheEntry, CacheKey};
