//! Embedding cache configuration.
//!
//! The cache stores computed embeddings keyed by content hash (xxhash64).
//! Provides <100us lookup vs ~200ms recomputation for cache hits.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{EmbeddingError, EmbeddingResult};

// ============================================================================
// EVICTION POLICY ENUM
// ============================================================================

/// Cache eviction policy.
///
/// Determines how entries are removed when the cache reaches capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvictionPolicy {
    /// Least Recently Used - evict oldest access.
    /// Best for: temporal locality workloads
    #[default]
    Lru,

    /// Least Frequently Used - evict lowest access count.
    /// Best for: frequency-based access patterns
    Lfu,

    /// LRU with TTL consideration.
    /// Prioritizes expired entries for eviction.
    TtlLru,

    /// Adaptive Replacement Cache - balanced LRU/LFU hybrid.
    /// Best for: mixed workloads with unknown access patterns
    Arc,
}

impl EvictionPolicy {
    /// Returns all available eviction policies.
    pub fn all() -> &'static [EvictionPolicy] {
        &[
            EvictionPolicy::Lru,
            EvictionPolicy::Lfu,
            EvictionPolicy::TtlLru,
            EvictionPolicy::Arc,
        ]
    }

    /// Returns the policy name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            EvictionPolicy::Lru => "lru",
            EvictionPolicy::Lfu => "lfu",
            EvictionPolicy::TtlLru => "ttl_lru",
            EvictionPolicy::Arc => "arc",
        }
    }
}

// ============================================================================
// DEFAULT FUNCTIONS
// ============================================================================

fn default_cache_enabled() -> bool {
    true
}

fn default_max_entries() -> usize {
    100_000
}

fn default_max_bytes() -> usize {
    1_073_741_824 // 1 GB
}

// ============================================================================
// CACHE CONFIG
// ============================================================================

/// Configuration for embedding cache.
///
/// The cache stores computed embeddings keyed by content hash (xxhash64).
/// Provides <100us lookup vs ~200ms recomputation for cache hits.
///
/// # Capacity Calculation
/// ```text
/// Multi-Array Storage: 13 embeddings at native dimensions
/// Average entry: ~6KB (varies by embedding combination)
/// 100K entries: ~600 MB
/// With metadata overhead: ~1 GB
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled.
    /// Default: true
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,

    /// Maximum number of cached embeddings.
    /// Constitution spec: 100,000 entries
    /// Default: 100_000
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,

    /// Maximum cache size in bytes.
    /// Default: 1GB (1_073_741_824 bytes)
    /// This is the primary memory budget constraint.
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,

    /// Time-to-live for cached entries in seconds.
    /// None = no expiration (entries evicted only by policy).
    /// Default: None
    #[serde(default)]
    pub ttl_seconds: Option<u64>,

    /// Eviction policy when cache is full.
    /// Default: Lru
    #[serde(default)]
    pub eviction_policy: EvictionPolicy,

    /// Whether to persist cache to disk on shutdown.
    /// Default: false
    #[serde(default)]
    pub persist_to_disk: bool,

    /// Path for disk persistence (required if persist_to_disk is true).
    /// Default: None
    #[serde(default)]
    pub disk_path: Option<PathBuf>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 100_000,
            max_bytes: 1_073_741_824, // 1 GB
            ttl_seconds: None,
            eviction_policy: EvictionPolicy::Lru,
            persist_to_disk: false,
            disk_path: None,
        }
    }
}

impl CacheConfig {
    /// Validate cache configuration.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if:
    /// - enabled && max_entries == 0
    /// - enabled && max_bytes == 0
    /// - persist_to_disk && disk_path.is_none()
    pub fn validate(&self) -> EmbeddingResult<()> {
        if self.enabled && self.max_entries == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "max_entries must be > 0 when cache enabled".to_string(),
            });
        }
        if self.enabled && self.max_bytes == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "max_bytes must be > 0 when cache enabled".to_string(),
            });
        }
        if self.persist_to_disk && self.disk_path.is_none() {
            return Err(EmbeddingError::ConfigError {
                message: "disk_path required when persist_to_disk enabled".to_string(),
            });
        }
        Ok(())
    }

    /// Create a disabled cache configuration.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Calculate the average bytes per entry based on current configuration.
    pub fn bytes_per_entry(&self) -> usize {
        self.max_bytes.checked_div(self.max_entries).unwrap_or(0)
    }
}
