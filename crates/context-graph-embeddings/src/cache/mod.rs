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
//! ```rust,ignore
//! use context_graph_embeddings::cache::{CacheKey, CacheManager};
//! use context_graph_embeddings::config::CacheConfig;
//!
//! let config = CacheConfig::default();
//! let cache = CacheManager::new(config)?;
//!
//! // Cache an embedding
//! let key = CacheKey::from_content("Hello world");
//! cache.put(key, fused_embedding)?;
//!
//! // Retrieve later
//! if let Some(embedding) = cache.get(&key) {
//!     println!("Cache hit! Dimension: {}", embedding.vector.len());
//! }
//!
//! // Check cache stats
//! println!("Hit rate: {:.1}%", cache.hit_rate() * 100.0);
//! ```

pub mod manager;
pub mod types;

pub use manager::{CacheManager, CacheMetrics};
pub use types::{CacheEntry, CacheKey};
