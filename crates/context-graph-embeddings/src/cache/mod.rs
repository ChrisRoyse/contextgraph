//! High-performance embedding cache using moka.
//!
//! # Performance Targets
//! - Cache lookup: <100us
//! - Cache miss + store: <500us overhead
//! - Memory efficiency: ~6KB per entry (13 embeddings at native dimensions)
//!
//! # Architecture
//!
//! This cache stores complete `MultiArrayEmbedding` instances keyed by xxhash64
//! of the input content. Uses moka's concurrent cache for thread-safe access
//! with configurable eviction policies and TTL.
//!
//! # Critical Requirements (NO FALLBACKS)
//!
//! - If cache operations fail, errors propagate (no silent degradation)
//! - Real moka cache backend (no stubs or mocks in production)
//! - Fail-fast on invalid configuration

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use moka::sync::Cache;

use crate::config::CacheConfig;
use crate::error::EmbeddingResult;
use crate::types::dimensions::{MODEL_COUNT, TOTAL_DIMENSION};
use crate::types::MultiArrayEmbedding;

// ============================================================================
// CACHE KEY
// ============================================================================

/// Cache key: xxhash64 of input content.
///
/// Uses the xxhash-rust crate (already a dependency) for fast hashing.
/// The hash is computed from the input text/content string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey(pub u64);

impl CacheKey {
    /// Create key from content string using xxhash64.
    ///
    /// # Arguments
    /// * `content` - The input text to hash
    ///
    /// # Returns
    /// A deterministic cache key based on content hash
    #[inline]
    pub fn from_content(content: &str) -> Self {
        let hash = xxhash_rust::xxh64::xxh64(content.as_bytes(), 0);
        Self(hash)
    }

    /// Create key from raw bytes using xxhash64.
    ///
    /// # Arguments
    /// * `bytes` - The raw bytes to hash
    ///
    /// # Returns
    /// A deterministic cache key based on content hash
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let hash = xxhash_rust::xxh64::xxh64(bytes, 0);
        Self(hash)
    }

    /// Create key from pre-computed hash value.
    ///
    /// Use this when the hash is already available (e.g., from MultiArrayEmbedding).
    #[inline]
    pub const fn from_hash(hash: u64) -> Self {
        Self(hash)
    }

    /// Get the raw hash value.
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

// ============================================================================
// CACHE STATISTICS
// ============================================================================

/// Cache hit/miss statistics for monitoring.
///
/// Thread-safe counters using atomic operations.
#[derive(Debug, Default)]
pub struct CacheStats {
    /// Total cache hits
    hits: AtomicU64,
    /// Total cache misses
    misses: AtomicU64,
    /// Total insertions
    insertions: AtomicU64,
    /// Total evictions (tracked by moka internally, we estimate)
    estimated_evictions: AtomicU64,
}

impl CacheStats {
    /// Create new stats instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a cache hit.
    #[inline]
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss.
    #[inline]
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an insertion.
    #[inline]
    pub fn record_insertion(&self) {
        self.insertions.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total hits.
    #[inline]
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Get total misses.
    #[inline]
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Get total insertions.
    #[inline]
    pub fn insertions(&self) -> u64 {
        self.insertions.load(Ordering::Relaxed)
    }

    /// Get hit ratio (0.0 to 1.0).
    ///
    /// Returns 0.0 if no lookups have been performed.
    pub fn hit_ratio(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Reset all statistics to zero.
    pub fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.insertions.store(0, Ordering::Relaxed);
        self.estimated_evictions.store(0, Ordering::Relaxed);
    }
}

impl Clone for CacheStats {
    fn clone(&self) -> Self {
        Self {
            hits: AtomicU64::new(self.hits.load(Ordering::Relaxed)),
            misses: AtomicU64::new(self.misses.load(Ordering::Relaxed)),
            insertions: AtomicU64::new(self.insertions.load(Ordering::Relaxed)),
            estimated_evictions: AtomicU64::new(self.estimated_evictions.load(Ordering::Relaxed)),
        }
    }
}

// ============================================================================
// MEMORY ESTIMATION
// ============================================================================

/// Estimate memory usage of a MultiArrayEmbedding in bytes.
///
/// # Memory Layout
/// - Each f32 = 4 bytes
/// - 13 embeddings at projected dimensions = TOTAL_DIMENSION floats
/// - Struct overhead: ~200 bytes (Options, latency, hash, etc.)
///
/// # Returns
/// Estimated bytes for the embedding (NOT including Arc overhead)
#[inline]
pub fn estimate_embedding_memory(embedding: &MultiArrayEmbedding) -> u32 {
    let mut total_bytes: usize = 0;

    // Count actual vector bytes
    for opt_emb in &embedding.embeddings {
        if let Some(emb) = opt_emb {
            // Vector data: dimension * sizeof(f32)
            total_bytes += emb.vector.len() * std::mem::size_of::<f32>();
            // Attention weights if present
            if let Some(ref attn) = &emb.attention_weights {
                let attn_vec: &Vec<f32> = attn;
                total_bytes += attn_vec.len() * std::mem::size_of::<f32>();
            }
            // ModelEmbedding struct overhead
            total_bytes += std::mem::size_of::<crate::types::ModelEmbedding>();
        }
    }

    // MultiArrayEmbedding struct overhead
    total_bytes += std::mem::size_of::<MultiArrayEmbedding>();

    // Cap at u32::MAX for moka weigher
    total_bytes.min(u32::MAX as usize) as u32
}

/// Estimate memory for a complete MultiArrayEmbedding (all 13 slots filled).
///
/// Used for capacity planning.
pub const fn estimate_complete_embedding_memory() -> usize {
    // All 13 embeddings at projected dimensions
    // TOTAL_DIMENSION floats * 4 bytes per f32
    let vector_bytes = TOTAL_DIMENSION * std::mem::size_of::<f32>();
    // Struct overhead per embedding (~64 bytes each)
    let per_embedding_overhead = MODEL_COUNT * 64;
    // MultiArrayEmbedding struct
    let container_overhead = 128;

    vector_bytes + per_embedding_overhead + container_overhead
}

// ============================================================================
// EMBEDDING CACHE
// ============================================================================

/// High-performance embedding cache using moka.
///
/// Thread-safe, concurrent cache with configurable eviction and TTL.
/// Stores `Arc<MultiArrayEmbedding>` to allow cheap clones on retrieval.
///
/// # Design Principles
/// - **NO FALLBACKS**: Cache operations propagate errors
/// - **REAL DATA**: No stubs or mocks in production
/// - **FAIL FAST**: Invalid configuration rejected at construction
///
/// # Performance
/// - O(1) amortized lookup/insert
/// - Lock-free reads in common case
/// - Background eviction (non-blocking)
pub struct EmbeddingCache {
    /// The moka cache backend
    inner: Cache<CacheKey, Arc<MultiArrayEmbedding>>,
    /// Cache configuration (immutable after creation)
    config: CacheConfig,
    /// Runtime statistics
    stats: Arc<CacheStats>,
    /// Whether caching is enabled
    enabled: bool,
}

impl EmbeddingCache {
    /// Create a new embedding cache from configuration.
    ///
    /// # Arguments
    /// * `config` - Cache configuration (validated before use)
    ///
    /// # Returns
    /// * `Ok(EmbeddingCache)` - Successfully created cache
    /// * `Err(EmbeddingError)` - Configuration invalid
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if:
    /// - `enabled && max_entries == 0`
    /// - `enabled && max_bytes == 0`
    /// - `persist_to_disk && disk_path.is_none()`
    pub fn new(config: CacheConfig) -> EmbeddingResult<Self> {
        // Validate configuration (fail fast)
        config.validate()?;

        let enabled = config.enabled;

        if !enabled {
            tracing::info!(
                "Embedding cache DISABLED by config. \
                 All lookups will return None, all inserts will be no-ops."
            );
        }

        // Build moka cache with configuration
        let mut builder = Cache::builder()
            .max_capacity(config.max_entries as u64)
            .weigher(|_key: &CacheKey, value: &Arc<MultiArrayEmbedding>| {
                estimate_embedding_memory(value)
            });

        // Add TTL if configured
        if let Some(ttl_secs) = config.ttl_seconds {
            if ttl_secs > 0 {
                builder = builder.time_to_live(Duration::from_secs(ttl_secs));
                tracing::debug!(ttl_secs, "Cache TTL configured");
            }
        }

        let inner = builder.build();

        tracing::info!(
            enabled,
            max_entries = config.max_entries,
            max_bytes = config.max_bytes,
            ttl_seconds = ?config.ttl_seconds,
            eviction_policy = %config.eviction_policy.as_str(),
            bytes_per_entry = config.bytes_per_entry(),
            "Embedding cache initialized"
        );

        Ok(Self {
            inner,
            config,
            stats: Arc::new(CacheStats::new()),
            enabled,
        })
    }

    /// Create a disabled cache (for testing or when caching is not wanted).
    pub fn disabled() -> Self {
        let config = CacheConfig::disabled();
        // unwrap is safe: disabled config always validates
        Self::new(config).expect("disabled config should always be valid")
    }

    /// Get an embedding from the cache.
    ///
    /// # Arguments
    /// * `key` - The cache key (xxhash64 of content)
    ///
    /// # Returns
    /// * `Some(Arc<MultiArrayEmbedding>)` - Cache hit
    /// * `None` - Cache miss or cache disabled
    ///
    /// # Performance
    /// Target: <100us lookup time
    pub fn get(&self, key: &CacheKey) -> Option<Arc<MultiArrayEmbedding>> {
        if !self.enabled {
            return None;
        }

        match self.inner.get(key) {
            Some(value) => {
                self.stats.record_hit();
                tracing::trace!(key = %key, "Cache HIT");
                Some(value)
            }
            None => {
                self.stats.record_miss();
                tracing::trace!(key = %key, "Cache MISS");
                None
            }
        }
    }

    /// Insert an embedding into the cache.
    ///
    /// # Arguments
    /// * `key` - The cache key (xxhash64 of content)
    /// * `value` - The embedding to cache (will be wrapped in Arc)
    ///
    /// # Note
    /// This is a no-op if the cache is disabled.
    pub fn insert(&self, key: CacheKey, value: MultiArrayEmbedding) {
        if !self.enabled {
            return;
        }

        let arc_value = Arc::new(value);
        self.insert_arc(key, arc_value);
    }

    /// Insert a pre-wrapped Arc embedding into the cache.
    ///
    /// Use this when you already have an Arc to avoid extra allocation.
    pub fn insert_arc(&self, key: CacheKey, value: Arc<MultiArrayEmbedding>) {
        if !self.enabled {
            return;
        }

        self.stats.record_insertion();
        self.inner.insert(key, value);

        tracing::trace!(
            key = %key,
            entry_count = self.inner.entry_count(),
            "Cache INSERT"
        );
    }

    /// Get or insert an embedding.
    ///
    /// If the key exists, returns the cached value.
    /// Otherwise, calls the factory function to create the value and caches it.
    ///
    /// # Arguments
    /// * `key` - The cache key
    /// * `factory` - Function to create the embedding if not cached
    ///
    /// # Returns
    /// The cached or newly created embedding
    ///
    /// # Errors
    /// Returns error from factory if cache miss and factory fails.
    pub fn get_or_insert_with<F>(
        &self,
        key: CacheKey,
        factory: F,
    ) -> EmbeddingResult<Arc<MultiArrayEmbedding>>
    where
        F: FnOnce() -> EmbeddingResult<MultiArrayEmbedding>,
    {
        // Check cache first
        if let Some(cached) = self.get(&key) {
            return Ok(cached);
        }

        // Cache miss: create new embedding
        let embedding = factory()?;
        let arc_embedding = Arc::new(embedding);

        // Store in cache
        self.insert_arc(key, Arc::clone(&arc_embedding));

        Ok(arc_embedding)
    }

    /// Remove an entry from the cache.
    ///
    /// # Arguments
    /// * `key` - The cache key to remove
    pub fn remove(&self, key: &CacheKey) {
        if !self.enabled {
            return;
        }
        self.inner.invalidate(key);
        tracing::trace!(key = %key, "Cache REMOVE");
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        self.inner.invalidate_all();
        tracing::info!("Cache CLEARED");
    }

    /// Get the current number of entries in the cache.
    #[inline]
    pub fn len(&self) -> u64 {
        self.inner.entry_count()
    }

    /// Check if the cache is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.entry_count() == 0
    }

    /// Check if the cache is enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the cache hit ratio.
    #[inline]
    pub fn hit_ratio(&self) -> f64 {
        self.stats.hit_ratio()
    }

    /// Get a reference to the cache statistics.
    #[inline]
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get a clone of the statistics (thread-safe snapshot).
    pub fn stats_snapshot(&self) -> CacheStats {
        self.stats.as_ref().clone()
    }

    /// Get the cache configuration.
    #[inline]
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Run cache maintenance (sync pending operations).
    ///
    /// Moka performs lazy maintenance. Call this to force cleanup.
    pub fn sync(&self) {
        self.inner.run_pending_tasks();
    }

    /// Get estimated memory usage in bytes.
    ///
    /// Note: This is an approximation based on entry count and average size.
    pub fn estimated_memory_bytes(&self) -> usize {
        let entry_count = self.inner.entry_count() as usize;
        let avg_bytes = estimate_complete_embedding_memory();
        entry_count * avg_bytes
    }
}

impl std::fmt::Debug for EmbeddingCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingCache")
            .field("enabled", &self.enabled)
            .field("entry_count", &self.inner.entry_count())
            .field("max_entries", &self.config.max_entries)
            .field("hit_ratio", &self.hit_ratio())
            .field("stats", &self.stats)
            .finish()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ModelEmbedding, ModelId};

    /// Create a test embedding with real data (NOT mocks).
    fn create_test_embedding(filled_models: &[ModelId]) -> MultiArrayEmbedding {
        let mut multi = MultiArrayEmbedding::new();

        for &model_id in filled_models {
            let dim = model_id.projected_dimension();
            // Create real vector data with deterministic values
            let vector: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001).sin()).collect();
            let mut emb = ModelEmbedding::new(model_id, vector, 1000);
            emb.set_projected(true);
            multi.set(emb);
        }

        multi
    }

    /// Create a complete test embedding with all 13 models.
    fn create_complete_embedding() -> MultiArrayEmbedding {
        create_test_embedding(ModelId::all())
    }

    #[test]
    fn test_cache_key_from_content() {
        let key1 = CacheKey::from_content("hello world");
        let key2 = CacheKey::from_content("hello world");
        let key3 = CacheKey::from_content("different content");

        // Same content produces same key
        assert_eq!(key1, key2);
        // Different content produces different key
        assert_ne!(key1, key3);
        // Hash is non-zero
        assert_ne!(key1.as_u64(), 0);
    }

    #[test]
    fn test_cache_key_display() {
        let key = CacheKey::from_content("test");
        let display = format!("{}", key);
        // Should be 16 hex chars
        assert_eq!(display.len(), 16);
        assert!(display.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cache_insert_and_get() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        let key = CacheKey::from_content("test content");
        let embedding = create_complete_embedding();

        cache.insert(key, embedding.clone());

        let retrieved = cache.get(&key);
        assert!(retrieved.is_some(), "Should retrieve inserted embedding");

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.filled_count(), MODEL_COUNT);
    }

    #[test]
    fn test_cache_miss() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        let key = CacheKey::from_content("nonexistent");
        let result = cache.get(&key);

        assert!(result.is_none(), "Should return None for missing key");
        assert_eq!(cache.stats().misses(), 1);
    }

    #[test]
    fn test_cache_hit_ratio() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        // Insert one entry
        let key = CacheKey::from_content("test");
        cache.insert(key, create_complete_embedding());

        // Hit
        let _ = cache.get(&key);
        // Miss
        let _ = cache.get(&CacheKey::from_content("missing"));

        // 1 hit, 1 miss = 50% ratio
        let ratio = cache.hit_ratio();
        assert!((ratio - 0.5).abs() < 0.01, "Hit ratio should be ~0.5");
    }

    #[test]
    fn test_cache_disabled() {
        let cache = EmbeddingCache::disabled();

        assert!(!cache.is_enabled());

        // Insert should be no-op
        let key = CacheKey::from_content("test");
        cache.insert(key, create_complete_embedding());

        // Get should return None even after insert
        assert!(cache.get(&key).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_remove() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        let key = CacheKey::from_content("to_remove");
        cache.insert(key, create_complete_embedding());

        assert!(cache.get(&key).is_some());

        cache.remove(&key);
        cache.sync(); // Force cleanup

        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_clear() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        // Insert multiple entries
        for i in 0..10 {
            let key = CacheKey::from_content(&format!("entry_{}", i));
            cache.insert(key, create_complete_embedding());
        }

        cache.sync();
        assert!(!cache.is_empty());

        cache.clear();
        cache.sync();

        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_config_validation_fails_on_zero_entries() {
        let mut config = CacheConfig::default();
        config.enabled = true;
        config.max_entries = 0;

        let result = EmbeddingCache::new(config);
        assert!(result.is_err(), "Should fail with max_entries=0");
    }

    #[test]
    fn test_cache_config_validation_fails_on_zero_bytes() {
        let mut config = CacheConfig::default();
        config.enabled = true;
        config.max_bytes = 0;

        let result = EmbeddingCache::new(config);
        assert!(result.is_err(), "Should fail with max_bytes=0");
    }

    #[test]
    fn test_cache_get_or_insert_with_hit() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        let key = CacheKey::from_content("existing");
        cache.insert(key, create_complete_embedding());

        let mut factory_called = false;
        let result = cache.get_or_insert_with(key, || {
            factory_called = true;
            Ok(create_complete_embedding())
        });

        assert!(result.is_ok());
        assert!(!factory_called, "Factory should NOT be called on cache hit");
    }

    #[test]
    fn test_cache_get_or_insert_with_miss() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        let key = CacheKey::from_content("new_entry");

        let mut factory_called = false;
        let result = cache.get_or_insert_with(key, || {
            factory_called = true;
            Ok(create_complete_embedding())
        });

        assert!(result.is_ok());
        assert!(factory_called, "Factory SHOULD be called on cache miss");

        // Entry should now be cached
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn test_memory_estimation() {
        let embedding = create_complete_embedding();
        let estimated = estimate_embedding_memory(&embedding);

        // Should be non-zero for complete embedding
        assert!(estimated > 0, "Memory estimate should be positive");

        // Rough sanity check: 13 models * ~1K dims * 4 bytes = ~52KB + overhead
        // We expect something in the 30-80KB range
        assert!(
            estimated > 30_000,
            "Memory estimate seems too low: {}",
            estimated
        );
        assert!(
            estimated < 100_000,
            "Memory estimate seems too high: {}",
            estimated
        );
    }

    #[test]
    fn test_cache_stats_reset() {
        let stats = CacheStats::new();

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();
        stats.record_insertion();

        assert_eq!(stats.hits(), 2);
        assert_eq!(stats.misses(), 1);
        assert_eq!(stats.insertions(), 1);

        stats.reset();

        assert_eq!(stats.hits(), 0);
        assert_eq!(stats.misses(), 0);
        assert_eq!(stats.insertions(), 0);
    }

    #[test]
    fn test_cache_with_ttl() {
        let mut config = CacheConfig::default();
        config.ttl_seconds = Some(3600); // 1 hour TTL

        let cache = EmbeddingCache::new(config).expect("cache with TTL should be valid");

        let key = CacheKey::from_content("ttl_test");
        cache.insert(key, create_complete_embedding());

        // Entry should be retrievable immediately
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn test_partial_embedding_caching() {
        let config = CacheConfig::default();
        let cache = EmbeddingCache::new(config).expect("cache creation should succeed");

        // Create embedding with only some models filled
        let partial = create_test_embedding(&[ModelId::Semantic, ModelId::Code, ModelId::Entity]);

        let key = CacheKey::from_content("partial");
        cache.insert(key, partial);

        let retrieved = cache.get(&key).expect("Should retrieve partial embedding");
        assert_eq!(retrieved.filled_count(), 3);
        assert!(!retrieved.is_complete());
    }

    #[test]
    fn test_cache_concurrent_access() {
        use std::thread;

        let config = CacheConfig::default();
        let cache = Arc::new(EmbeddingCache::new(config).expect("cache creation should succeed"));

        let mut handles = vec![];

        // Spawn multiple threads doing concurrent reads/writes
        for thread_id in 0..4 {
            let cache_clone = Arc::clone(&cache);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = CacheKey::from_content(&format!("thread_{}_entry_{}", thread_id, i));
                    cache_clone.insert(key, create_complete_embedding());
                    let _ = cache_clone.get(&key);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread should not panic");
        }

        // Cache should have entries
        assert!(
            cache.len() > 0,
            "Cache should have entries after concurrent access"
        );
    }
}
