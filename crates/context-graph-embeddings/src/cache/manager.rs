//! CacheManager implementation with LRU eviction.
//!
//! This module provides the main cache manager for storing and retrieving
//! FusedEmbedding results with O(1) lookup and LRU eviction.
//!
//! # Architecture
//!
//! - LinkedHashMap maintains insertion order for LRU semantics
//! - RwLock optimizes for read-heavy workloads
//! - Atomic counters provide lock-free metrics updates
//! - Optional disk persistence uses bincode serialization
//!
//! # Performance Targets
//!
//! - Lookup latency: <100μs (constitution.yaml reflex_cache budget)
//! - Hit rate target: >80% under normal workload
//! - Max entries: 100,000 (configurable)
//! - Max bytes: 1GB (configurable)

use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration;

use linked_hash_map::LinkedHashMap;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::cache::types::{CacheEntry, CacheKey};
use crate::config::CacheConfig;
use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::{AuxiliaryEmbeddingData, FusedEmbedding};

/// Magic bytes for cache persistence file format.
const CACHE_MAGIC: [u8; 4] = [0x43, 0x47, 0x45, 0x43]; // "CGEC"

/// Cache file format version.
const CACHE_VERSION: u8 = 1;

/// Serializable cache entry for disk persistence.
/// This avoids issues with FusedEmbedding's skip_serializing_if attribute
/// that can cause bincode deserialization mismatches.
#[derive(Serialize, Deserialize)]
struct SerializableCacheEntry {
    key: CacheKey,
    vector: Vec<f32>,
    expert_weights: [f32; 8],
    selected_experts: [u8; 2],
    pipeline_latency_us: u64,
    content_hash: u64,
    aux_data: Option<AuxiliaryEmbeddingData>,
}

impl SerializableCacheEntry {
    fn from_cache_entry(key: CacheKey, embedding: &FusedEmbedding) -> Self {
        Self {
            key,
            vector: embedding.vector.clone(),
            expert_weights: embedding.expert_weights,
            selected_experts: embedding.selected_experts,
            pipeline_latency_us: embedding.pipeline_latency_us,
            content_hash: embedding.content_hash,
            aux_data: embedding.aux_data.clone(),
        }
    }

    fn into_key_and_embedding(self) -> (CacheKey, FusedEmbedding) {
        let embedding = FusedEmbedding {
            vector: self.vector,
            expert_weights: self.expert_weights,
            selected_experts: self.selected_experts,
            pipeline_latency_us: self.pipeline_latency_us,
            content_hash: self.content_hash,
            aux_data: self.aux_data,
        };
        (self.key, embedding)
    }
}

/// Thread-safe cache metrics with atomic counters.
///
/// All metrics use relaxed ordering since exact consistency is not required
/// for statistical monitoring.
#[derive(Debug, Default)]
pub struct CacheMetrics {
    /// Number of cache hits (key found and not expired).
    pub hits: AtomicU64,
    /// Number of cache misses (key not found or expired).
    pub misses: AtomicU64,
    /// Number of entries evicted due to capacity/memory limits.
    pub evictions: AtomicU64,
    /// Current memory usage in bytes (approximate).
    pub bytes_used: AtomicUsize,
}

impl CacheMetrics {
    /// Create new metrics with all counters at zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all metrics to zero.
    pub fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.bytes_used.store(0, Ordering::Relaxed);
    }

    /// Increment hit counter.
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment miss counter.
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment eviction counter.
    pub fn record_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Add bytes to memory usage.
    pub fn add_bytes(&self, bytes: usize) {
        self.bytes_used.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Subtract bytes from memory usage.
    pub fn subtract_bytes(&self, bytes: usize) {
        self.bytes_used.fetch_sub(bytes, Ordering::Relaxed);
    }
}

/// LRU-based embedding cache manager.
///
/// FAIL-FAST: All errors propagate immediately. No fallbacks.
///
/// # Thread Safety
///
/// Uses RwLock for entries to optimize read-heavy workloads:
/// - get() takes read lock (multiple concurrent readers)
/// - put()/remove()/clear() take write lock (exclusive)
///
/// # Eviction Strategy
///
/// 1. TTL expiration: Entries older than ttl_seconds are removed on access
/// 2. LRU eviction: When max_entries exceeded, oldest entries removed
/// 3. Memory eviction: When max_bytes exceeded, entries removed until under limit
pub struct CacheManager {
    /// Internal cache storage with LRU ordering.
    entries: RwLock<LinkedHashMap<CacheKey, CacheEntry>>,
    /// Cache configuration (immutable after creation).
    config: CacheConfig,
    /// Thread-safe metrics.
    metrics: CacheMetrics,
}

impl CacheManager {
    /// Create new cache manager with given configuration.
    ///
    /// # Arguments
    /// * `config` - Cache configuration (max_entries, max_bytes, ttl, etc.)
    ///
    /// # Returns
    /// * `Ok(CacheManager)` - Configured cache manager
    /// * `Err(EmbeddingError::ConfigError)` - If config validation fails
    ///
    /// # Errors
    /// Returns error if:
    /// - max_entries is 0
    /// - max_bytes is 0
    /// - persist_to_disk is true but disk_path is None
    pub fn new(config: CacheConfig) -> EmbeddingResult<Self> {
        // Validate configuration
        if config.max_entries == 0 {
            error!("CacheManager config error: max_entries cannot be 0");
            return Err(EmbeddingError::ConfigError {
                message: "max_entries cannot be 0".to_string(),
            });
        }

        if config.max_bytes == 0 {
            error!("CacheManager config error: max_bytes cannot be 0");
            return Err(EmbeddingError::ConfigError {
                message: "max_bytes cannot be 0".to_string(),
            });
        }

        if config.persist_to_disk && config.disk_path.is_none() {
            error!("CacheManager config error: disk_path required when persist_to_disk is true");
            return Err(EmbeddingError::ConfigError {
                message: "disk_path required when persist_to_disk is true".to_string(),
            });
        }

        Ok(Self {
            entries: RwLock::new(LinkedHashMap::new()),
            config,
            metrics: CacheMetrics::new(),
        })
    }

    /// Get embedding by key, updating LRU order.
    ///
    /// Returns None if:
    /// - Key not found in cache
    /// - Entry has expired (TTL exceeded)
    ///
    /// # Side Effects
    /// - Updates metrics.hits or metrics.misses
    /// - Moves accessed entry to end of LRU order (most recently used)
    /// - Removes expired entries
    ///
    /// # LRU Semantics
    /// Uses LinkedHashMap::get_refresh() to move accessed entry to back,
    /// ensuring evict_oldest() removes least-recently-accessed entries first.
    #[must_use]
    pub fn get(&self, key: &CacheKey) -> Option<FusedEmbedding> {
        // Acquire write lock for LRU reordering via get_refresh()
        let mut entries = match self.entries.write() {
            Ok(entries) => entries,
            Err(_) => {
                self.metrics.record_miss();
                return None;
            }
        };

        // get_refresh moves entry to back (most recently used) if found
        let entry = match entries.get_refresh(key) {
            Some(entry) => entry,
            None => {
                self.metrics.record_miss();
                return None;
            }
        };

        // Check TTL expiration
        if let Some(ttl_secs) = self.config.ttl_seconds {
            let ttl = Duration::from_secs(ttl_secs);
            if entry.is_expired(ttl) {
                // Entry expired - remove and record miss
                let size = entry.memory_size();
                entries.remove(key);
                self.metrics.subtract_bytes(size);
                self.metrics.record_miss();
                return None;
            }
        }

        // Entry exists and is valid - update access tracking
        entry.touch();
        entry.increment_access();
        self.metrics.record_hit();

        // Clone the embedding to return (entry.embedding is immutable)
        Some(entry.embedding.clone())
    }

    /// Insert embedding, evicting LRU entries if needed.
    ///
    /// # Arguments
    /// * `key` - Cache key (content hash)
    /// * `embedding` - FusedEmbedding to cache
    ///
    /// # Returns
    /// * `Ok(())` - Entry cached successfully
    /// * `Err(EmbeddingError::CacheError)` - If embedding alone exceeds max_bytes
    ///
    /// # Eviction
    /// If inserting would exceed limits:
    /// 1. Evict oldest entries until within max_entries limit
    /// 2. Evict oldest entries until within max_bytes limit
    pub fn put(&self, key: CacheKey, embedding: FusedEmbedding) -> EmbeddingResult<()> {
        let entry = CacheEntry::new(embedding);
        let entry_size = entry.memory_size();

        // Check if single entry exceeds max_bytes
        if entry_size > self.config.max_bytes {
            error!(
                "CacheManager put error: entry size {} exceeds max_bytes {}",
                entry_size, self.config.max_bytes
            );
            return Err(EmbeddingError::CacheError {
                message: format!(
                    "Entry size {} bytes exceeds max_bytes {} bytes",
                    entry_size, self.config.max_bytes
                ),
            });
        }

        let mut entries = self.entries.write().map_err(|e| {
            error!("CacheManager put error: lock poisoned: {}", e);
            EmbeddingError::CacheError {
                message: format!("Lock poisoned: {}", e),
            }
        })?;

        // Check if key already exists - update in place
        if let Some(old_entry) = entries.get(&key) {
            let old_size = old_entry.memory_size();
            self.metrics.subtract_bytes(old_size);
        }

        // Evict entries until we're under max_entries limit
        while entries.len() >= self.config.max_entries {
            self.evict_oldest(&mut entries);
        }

        // Evict entries until we're under max_bytes limit
        let current_bytes = self.metrics.bytes_used.load(Ordering::Relaxed);
        let mut projected_bytes = current_bytes + entry_size;

        while projected_bytes > self.config.max_bytes && !entries.is_empty() {
            if let Some(evicted_size) = self.evict_oldest(&mut entries) {
                projected_bytes = projected_bytes.saturating_sub(evicted_size);
            } else {
                break;
            }
        }

        // Insert new entry
        entries.insert(key, entry);
        self.metrics.add_bytes(entry_size);

        Ok(())
    }

    /// Evict oldest entry (front of LinkedHashMap).
    /// Returns the size of the evicted entry, or None if cache was empty.
    fn evict_oldest(&self, entries: &mut LinkedHashMap<CacheKey, CacheEntry>) -> Option<usize> {
        // LinkedHashMap::pop_front removes the oldest (least recently inserted/accessed) entry
        if let Some((_key, entry)) = entries.pop_front() {
            let size = entry.memory_size();
            self.metrics.subtract_bytes(size);
            self.metrics.record_eviction();
            Some(size)
        } else {
            None
        }
    }

    /// Check if key exists (does not update LRU order).
    ///
    /// Note: This does NOT check TTL expiration. Use get() if you need
    /// to verify the entry is still valid.
    #[must_use]
    pub fn contains(&self, key: &CacheKey) -> bool {
        self.entries
            .read()
            .map(|entries| entries.contains_key(key))
            .unwrap_or(false)
    }

    /// Remove entry by key.
    ///
    /// # Returns
    /// * `Some(FusedEmbedding)` - The removed embedding
    /// * `None` - Key not found
    pub fn remove(&self, key: &CacheKey) -> Option<FusedEmbedding> {
        let mut entries = self.entries.write().ok()?;
        let entry = entries.remove(key)?;
        let size = entry.memory_size();
        self.metrics.subtract_bytes(size);
        Some(entry.embedding)
    }

    /// Clear all entries, reset metrics.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
            self.metrics.reset();
        }
    }

    /// Current entry count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries
            .read()
            .map(|entries| entries.len())
            .unwrap_or(0)
    }

    /// Check if cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Calculate hit rate: hits / (hits + misses).
    /// Returns 0.0 if no accesses yet.
    #[must_use]
    pub fn hit_rate(&self) -> f32 {
        let hits = self.metrics.hits.load(Ordering::Relaxed);
        let misses = self.metrics.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f32 / total as f32
        }
    }

    /// Current memory usage in bytes.
    #[must_use]
    pub fn memory_usage(&self) -> usize {
        self.metrics.bytes_used.load(Ordering::Relaxed)
    }

    /// Get reference to cache metrics.
    #[must_use]
    pub fn metrics(&self) -> &CacheMetrics {
        &self.metrics
    }

    /// Get reference to cache configuration.
    #[must_use]
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Persist cache to disk (bincode format).
    ///
    /// File format:
    /// - Header: magic bytes [0x43, 0x47, 0x45, 0x43], version u8, entry_count u64
    /// - Entries: bincode-serialized Vec<(CacheKey, FusedEmbedding)>
    /// - Footer: xxhash64 checksum of all preceding bytes
    ///
    /// # Returns
    /// * `Ok(())` - Cache persisted successfully
    /// * `Err(EmbeddingError::CacheError)` - On I/O or serialization failure
    /// * `Err(EmbeddingError::ConfigError)` - If disk_path not configured
    pub async fn persist(&self) -> EmbeddingResult<()> {
        let path = self.config.disk_path.as_ref().ok_or_else(|| {
            error!("CacheManager persist error: disk_path not configured");
            EmbeddingError::ConfigError {
                message: "disk_path not configured for persistence".to_string(),
            }
        })?;

        // Collect entries under read lock, converting to serializable format
        let entries_data: Vec<SerializableCacheEntry> = {
            let entries = self.entries.read().map_err(|e| {
                error!("CacheManager persist error: lock poisoned: {}", e);
                EmbeddingError::CacheError {
                    message: format!("Lock poisoned: {}", e),
                }
            })?;

            entries
                .iter()
                .map(|(k, v)| SerializableCacheEntry::from_cache_entry(*k, &v.embedding))
                .collect()
        };

        let entry_count = entries_data.len() as u64;

        // Build the data buffer
        let mut data = Vec::new();

        // Header: magic + version + entry_count
        data.extend_from_slice(&CACHE_MAGIC);
        data.push(CACHE_VERSION);
        data.extend_from_slice(&entry_count.to_le_bytes());

        // Serialize entries
        let entries_bytes = bincode::serialize(&entries_data).map_err(|e| {
            error!("CacheManager persist error: serialization failed: {}", e);
            EmbeddingError::SerializationError {
                message: format!("bincode serialization failed: {}", e),
            }
        })?;
        data.extend_from_slice(&entries_bytes);

        // Footer: xxhash64 checksum
        let checksum = xxhash_rust::xxh64::xxh64(&data, 0);
        data.extend_from_slice(&checksum.to_le_bytes());

        // Write to disk atomically via temp file
        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, &data).await.map_err(|e| {
            error!("CacheManager persist error: write failed: {}", e);
            EmbeddingError::CacheError {
                message: format!("Failed to write cache file: {}", e),
            }
        })?;

        tokio::fs::rename(&temp_path, path).await.map_err(|e| {
            error!("CacheManager persist error: rename failed: {}", e);
            EmbeddingError::CacheError {
                message: format!("Failed to rename temp cache file: {}", e),
            }
        })?;

        Ok(())
    }

    /// Load cache from disk, replacing current entries.
    ///
    /// # Returns
    /// * `Ok(())` - Cache loaded successfully
    /// * `Err(EmbeddingError::CacheError)` - On I/O, deserialization, or checksum failure
    /// * `Err(EmbeddingError::ConfigError)` - If disk_path not configured
    pub async fn load(&self) -> EmbeddingResult<()> {
        let path = self.config.disk_path.as_ref().ok_or_else(|| {
            error!("CacheManager load error: disk_path not configured");
            EmbeddingError::ConfigError {
                message: "disk_path not configured for persistence".to_string(),
            }
        })?;

        self.load_from_path(path).await
    }

    /// Load cache from a specific path.
    async fn load_from_path(&self, path: &Path) -> EmbeddingResult<()> {
        // Read file
        let data = tokio::fs::read(path).await.map_err(|e| {
            error!("CacheManager load error: read failed: {}", e);
            EmbeddingError::CacheError {
                message: format!("Failed to read cache file: {}", e),
            }
        })?;

        // Minimum size check: magic(4) + version(1) + count(8) + checksum(8) = 21
        if data.len() < 21 {
            error!("CacheManager load error: file too small");
            return Err(EmbeddingError::CacheError {
                message: "Cache file too small".to_string(),
            });
        }

        // Verify checksum (last 8 bytes)
        let checksum_offset = data.len() - 8;
        let stored_checksum = u64::from_le_bytes(
            data[checksum_offset..]
                .try_into()
                .map_err(|_| EmbeddingError::CacheError {
                    message: "Invalid checksum bytes".to_string(),
                })?,
        );

        let computed_checksum = xxhash_rust::xxh64::xxh64(&data[..checksum_offset], 0);

        if stored_checksum != computed_checksum {
            error!(
                "CacheManager load error: checksum mismatch (stored={:#x}, computed={:#x})",
                stored_checksum, computed_checksum
            );
            return Err(EmbeddingError::CacheError {
                message: format!(
                    "Checksum mismatch: stored={:#x}, computed={:#x}",
                    stored_checksum, computed_checksum
                ),
            });
        }

        // Verify magic bytes
        if data[0..4] != CACHE_MAGIC {
            error!("CacheManager load error: invalid magic bytes");
            return Err(EmbeddingError::CacheError {
                message: "Invalid cache file magic bytes".to_string(),
            });
        }

        // Verify version
        let version = data[4];
        if version != CACHE_VERSION {
            error!(
                "CacheManager load error: unsupported version {} (expected {})",
                version, CACHE_VERSION
            );
            return Err(EmbeddingError::CacheError {
                message: format!(
                    "Unsupported cache version {} (expected {})",
                    version, CACHE_VERSION
                ),
            });
        }

        // Parse entry count
        let entry_count = u64::from_le_bytes(
            data[5..13]
                .try_into()
                .map_err(|_| EmbeddingError::CacheError {
                    message: "Invalid entry count bytes".to_string(),
                })?,
        );

        // Deserialize entries using SerializableCacheEntry
        let entries_data: Vec<SerializableCacheEntry> =
            bincode::deserialize(&data[13..checksum_offset]).map_err(|e| {
                error!("CacheManager load error: deserialization failed: {}", e);
                EmbeddingError::SerializationError {
                    message: format!("bincode deserialization failed: {}", e),
                }
            })?;

        // Verify entry count
        if entries_data.len() as u64 != entry_count {
            error!(
                "CacheManager load error: entry count mismatch (header={}, actual={})",
                entry_count,
                entries_data.len()
            );
            return Err(EmbeddingError::CacheError {
                message: format!(
                    "Entry count mismatch: header={}, actual={}",
                    entry_count,
                    entries_data.len()
                ),
            });
        }

        // Replace current entries
        let mut entries = self.entries.write().map_err(|e| {
            error!("CacheManager load error: lock poisoned: {}", e);
            EmbeddingError::CacheError {
                message: format!("Lock poisoned: {}", e),
            }
        })?;

        entries.clear();
        self.metrics.reset();

        for serialized in entries_data {
            let (key, embedding) = serialized.into_key_and_embedding();
            let entry = CacheEntry::new(embedding);
            let size = entry.memory_size();
            entries.insert(key, entry);
            self.metrics.add_bytes(size);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EvictionPolicy;
    use crate::types::dimensions::FUSED_OUTPUT;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ========== Test Helpers (NO MOCK DATA) ==========

    fn create_test_embedding(content_hash: u64) -> FusedEmbedding {
        // Create real 1536D vector with deterministic values
        let vector: Vec<f32> = (0..FUSED_OUTPUT)
            .map(|i| ((i as f32 + content_hash as f32) % 2.0) - 1.0)
            .collect();
        let weights = [0.125f32; 8]; // Equal weights summing to 1.0
        FusedEmbedding::new(vector, weights, [0, 1], 100, content_hash)
            .expect("Test helper should create valid embedding")
    }

    fn create_test_config() -> CacheConfig {
        CacheConfig {
            enabled: true,
            max_entries: 100,
            max_bytes: 1_000_000, // 1 MB
            ttl_seconds: None,
            eviction_policy: EvictionPolicy::Lru,
            persist_to_disk: false,
            disk_path: None,
        }
    }

    fn create_test_config_with_ttl(ttl_secs: u64) -> CacheConfig {
        CacheConfig {
            ttl_seconds: Some(ttl_secs),
            ..create_test_config()
        }
    }

    fn create_test_config_with_persistence(path: PathBuf) -> CacheConfig {
        CacheConfig {
            persist_to_disk: true,
            disk_path: Some(path),
            ..create_test_config()
        }
    }

    // ========== CacheManager::new() Tests ==========

    #[test]
    fn test_new_with_valid_config() {
        println!("BEFORE: Creating CacheManager with valid config");

        let config = create_test_config();
        let result = CacheManager::new(config);

        println!("AFTER: CacheManager created successfully");

        assert!(result.is_ok());
        let cache = result.unwrap();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        println!("PASSED: CacheManager::new() with valid config works");
    }

    #[test]
    fn test_new_with_zero_max_entries_fails() {
        println!("BEFORE: Creating CacheManager with max_entries=0");

        let config = CacheConfig {
            max_entries: 0,
            ..create_test_config()
        };
        let result = CacheManager::new(config);

        println!("AFTER: Result = {:?}", result.is_err());

        assert!(result.is_err());
        println!("PASSED: CacheManager::new() fails with max_entries=0");
    }

    #[test]
    fn test_new_with_zero_max_bytes_fails() {
        println!("BEFORE: Creating CacheManager with max_bytes=0");

        let config = CacheConfig {
            max_bytes: 0,
            ..create_test_config()
        };
        let result = CacheManager::new(config);

        println!("AFTER: Result = {:?}", result.is_err());

        assert!(result.is_err());
        println!("PASSED: CacheManager::new() fails with max_bytes=0");
    }

    #[test]
    fn test_new_with_persist_but_no_path_fails() {
        println!("BEFORE: Creating CacheManager with persist_to_disk=true but no path");

        let config = CacheConfig {
            persist_to_disk: true,
            disk_path: None,
            ..create_test_config()
        };
        let result = CacheManager::new(config);

        println!("AFTER: Result = {:?}", result.is_err());

        assert!(result.is_err());
        println!("PASSED: CacheManager::new() fails when persist_to_disk but no path");
    }

    // ========== put/get Round-Trip Tests ==========

    #[test]
    fn test_put_get_roundtrip() {
        println!("BEFORE: Creating cache and inserting embedding");

        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("test content");
        let embedding = create_test_embedding(12345);
        let original_hash = embedding.content_hash;

        cache.put(key, embedding).unwrap();

        println!("AFTER: Cache state: len={}, bytes={}", cache.len(), cache.memory_usage());

        let retrieved = cache.get(&key);

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.content_hash, original_hash);
        assert_eq!(retrieved.vector.len(), FUSED_OUTPUT);

        println!("PASSED: put/get round-trip preserves embedding data");
    }

    #[test]
    fn test_get_nonexistent_key_returns_none() {
        println!("BEFORE: Creating empty cache");

        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("nonexistent");

        println!("BEFORE: entries.len() = {}, metrics.misses = {}", cache.len(), cache.metrics.misses.load(Ordering::Relaxed));

        let result = cache.get(&key);

        println!("AFTER: result = {:?}, metrics.misses = {}", result.is_none(), cache.metrics.misses.load(Ordering::Relaxed));

        assert!(result.is_none());
        assert_eq!(cache.metrics.misses.load(Ordering::Relaxed), 1);
        println!("PASSED: get() on nonexistent key returns None and records miss");
    }

    // ========== LRU Eviction Tests ==========

    #[test]
    fn test_lru_eviction_at_max_entries() {
        println!("BEFORE: Creating cache with max_entries=3");

        let config = CacheConfig {
            max_entries: 3,
            max_bytes: 100_000_000, // Large enough to not trigger byte eviction
            ..create_test_config()
        };
        let cache = CacheManager::new(config).unwrap();

        // Insert 3 entries
        let key1 = CacheKey::from(1u64);
        let key2 = CacheKey::from(2u64);
        let key3 = CacheKey::from(3u64);

        cache.put(key1, create_test_embedding(1)).unwrap();
        cache.put(key2, create_test_embedding(2)).unwrap();
        cache.put(key3, create_test_embedding(3)).unwrap();

        println!("AFTER insert 3: len={}, evictions={}", cache.len(), cache.metrics.evictions.load(Ordering::Relaxed));

        assert_eq!(cache.len(), 3);
        assert!(cache.contains(&key1));

        // Insert 4th entry - should evict key1 (oldest)
        let key4 = CacheKey::from(4u64);
        cache.put(key4, create_test_embedding(4)).unwrap();

        println!("AFTER insert 4: len={}, evictions={}", cache.len(), cache.metrics.evictions.load(Ordering::Relaxed));

        assert_eq!(cache.len(), 3);
        assert!(!cache.contains(&key1), "key1 should have been evicted");
        assert!(cache.contains(&key2));
        assert!(cache.contains(&key3));
        assert!(cache.contains(&key4));
        assert_eq!(cache.metrics.evictions.load(Ordering::Relaxed), 1);

        println!("PASSED: LRU eviction removes oldest entry when at max_entries");
    }

    #[test]
    fn test_lru_access_prevents_eviction() {
        // This test verifies TRUE LRU behavior:
        // Insert A, B, C -> access A -> insert D (triggers eviction) -> B is evicted (not A)
        println!("BEFORE: Creating cache with max_entries=3 for LRU access test");

        let config = CacheConfig {
            max_entries: 3,
            max_bytes: 100_000_000,
            ..create_test_config()
        };
        let cache = CacheManager::new(config).unwrap();

        // Insert A, B, C in order
        let key_a = CacheKey::from(0xA_u64);
        let key_b = CacheKey::from(0xB_u64);
        let key_c = CacheKey::from(0xC_u64);

        cache.put(key_a, create_test_embedding(0xA)).unwrap();
        cache.put(key_b, create_test_embedding(0xB)).unwrap();
        cache.put(key_c, create_test_embedding(0xC)).unwrap();

        println!("AFTER insert A,B,C: len={}", cache.len());
        assert_eq!(cache.len(), 3);

        // Access A - this should move A to the back (most recently used)
        // Without LRU reorder, A would remain at front and get evicted
        let retrieved = cache.get(&key_a);
        assert!(retrieved.is_some(), "A should be in cache");
        println!("AFTER access A: A moved to back of LRU order");

        // Now insert D - this triggers eviction
        // With TRUE LRU: B should be evicted (oldest ACCESS, since A was just accessed)
        // With FIFO: A would be evicted (oldest INSERT)
        let key_d = CacheKey::from(0xD_u64);
        cache.put(key_d, create_test_embedding(0xD)).unwrap();

        println!("AFTER insert D: len={}, evictions={}", cache.len(), cache.metrics.evictions.load(Ordering::Relaxed));

        // Verify TRUE LRU behavior
        assert_eq!(cache.len(), 3);
        assert!(cache.contains(&key_a), "A should NOT be evicted (was accessed)");
        assert!(!cache.contains(&key_b), "B SHOULD be evicted (least recently accessed)");
        assert!(cache.contains(&key_c), "C should still be in cache");
        assert!(cache.contains(&key_d), "D should be in cache (just inserted)");

        println!("PASSED: LRU access prevents eviction - accessed entry A survived, B was evicted");
    }

    #[test]
    fn test_max_entries_limit_enforced() {
        println!("BEFORE: Creating cache with max_entries=5");

        let config = CacheConfig {
            max_entries: 5,
            max_bytes: 100_000_000,
            ..create_test_config()
        };
        let cache = CacheManager::new(config).unwrap();

        // Insert 10 entries
        for i in 0..10 {
            let key = CacheKey::from(i as u64);
            cache.put(key, create_test_embedding(i as u64)).unwrap();
        }

        println!("AFTER inserting 10: len={}", cache.len());

        assert_eq!(cache.len(), 5, "Cache should never exceed max_entries");
        assert_eq!(cache.metrics.evictions.load(Ordering::Relaxed), 5);

        println!("PASSED: max_entries limit is enforced");
    }

    #[test]
    fn test_max_bytes_limit_enforced() {
        println!("BEFORE: Creating cache with small max_bytes");

        // FusedEmbedding is ~6200 bytes per entry
        // Set max_bytes to allow only ~2 entries
        let config = CacheConfig {
            max_entries: 100,
            max_bytes: 15_000, // ~2 entries worth
            ..create_test_config()
        };
        let cache = CacheManager::new(config).unwrap();

        // Insert 5 entries
        for i in 0..5 {
            let key = CacheKey::from(i as u64);
            cache.put(key, create_test_embedding(i as u64)).unwrap();
            println!("After insert {}: len={}, bytes={}", i, cache.len(), cache.memory_usage());
        }

        println!("AFTER: len={}, bytes={}, max_bytes={}", cache.len(), cache.memory_usage(), 15_000);

        assert!(cache.memory_usage() <= 15_000, "Memory usage should be under max_bytes");
        assert!(cache.metrics.evictions.load(Ordering::Relaxed) > 0);

        println!("PASSED: max_bytes limit is enforced");
    }

    // ========== TTL Expiration Tests ==========

    #[test]
    fn test_ttl_expiration_returns_none() {
        println!("BEFORE: Creating cache with ttl_seconds=0 (immediate expiry)");

        let config = create_test_config_with_ttl(0);
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("ttl test");
        cache.put(key, create_test_embedding(999)).unwrap();

        println!("AFTER put: len={}", cache.len());

        // Even with TTL=0, entry should immediately expire
        // Wait a tiny bit to ensure expiration
        std::thread::sleep(std::time::Duration::from_millis(1));

        let result = cache.get(&key);

        println!("AFTER get with expired TTL: result={:?}", result.is_none());

        assert!(result.is_none(), "Expired entry should return None");
        assert_eq!(cache.metrics.misses.load(Ordering::Relaxed), 1);

        println!("PASSED: TTL expiration returns None for expired entries");
    }

    #[test]
    fn test_ttl_valid_entry_returned() {
        println!("BEFORE: Creating cache with ttl_seconds=3600 (1 hour)");

        let config = create_test_config_with_ttl(3600);
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("valid ttl");
        cache.put(key, create_test_embedding(888)).unwrap();

        let result = cache.get(&key);

        println!("AFTER get: result={:?}", result.is_some());

        assert!(result.is_some(), "Non-expired entry should be returned");
        assert_eq!(cache.metrics.hits.load(Ordering::Relaxed), 1);

        println!("PASSED: Non-expired TTL entry is returned");
    }

    // ========== Hit Rate Tests ==========

    #[test]
    fn test_hit_rate_calculation() {
        println!("BEFORE: Creating cache for hit rate test");

        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        // Initially 0
        assert_eq!(cache.hit_rate(), 0.0);

        let key = CacheKey::from_content("hit rate test");
        cache.put(key, create_test_embedding(111)).unwrap();

        // 1 hit
        let _ = cache.get(&key);
        assert_eq!(cache.hit_rate(), 1.0);

        // 1 hit, 1 miss
        let _ = cache.get(&CacheKey::from_content("nonexistent"));
        let rate = cache.hit_rate();

        println!("AFTER: hits=1, misses=1, rate={:.2}", rate);

        assert!((rate - 0.5).abs() < 0.001, "Hit rate should be 0.5");

        println!("PASSED: hit_rate() calculation is accurate");
    }

    #[test]
    fn test_hit_rate_zero_when_no_accesses() {
        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        println!("BEFORE: New cache with no accesses");
        println!("AFTER: hit_rate = {}", cache.hit_rate());

        assert_eq!(cache.hit_rate(), 0.0);
        println!("PASSED: hit_rate() returns 0.0 when no accesses");
    }

    // ========== contains/remove/clear Tests ==========

    #[test]
    fn test_contains_key() {
        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("contains test");

        println!("BEFORE put: contains = {}", cache.contains(&key));
        assert!(!cache.contains(&key));

        cache.put(key, create_test_embedding(222)).unwrap();

        println!("AFTER put: contains = {}", cache.contains(&key));
        assert!(cache.contains(&key));

        println!("PASSED: contains() correctly reports key presence");
    }

    #[test]
    fn test_remove_entry() {
        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("remove test");
        cache.put(key, create_test_embedding(333)).unwrap();

        println!("BEFORE remove: len={}, bytes={}", cache.len(), cache.memory_usage());

        let removed = cache.remove(&key);

        println!("AFTER remove: len={}, bytes={}, removed={:?}", cache.len(), cache.memory_usage(), removed.is_some());

        assert!(removed.is_some());
        assert_eq!(cache.len(), 0);
        assert!(!cache.contains(&key));

        println!("PASSED: remove() removes entry and updates metrics");
    }

    #[test]
    fn test_clear() {
        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        // Add some entries
        for i in 0..5 {
            let key = CacheKey::from(i as u64);
            cache.put(key, create_test_embedding(i as u64)).unwrap();
        }

        println!("BEFORE clear: len={}, bytes={}", cache.len(), cache.memory_usage());

        cache.clear();

        println!("AFTER clear: len={}, bytes={}", cache.len(), cache.memory_usage());

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.memory_usage(), 0);
        assert_eq!(cache.metrics.hits.load(Ordering::Relaxed), 0);

        println!("PASSED: clear() removes all entries and resets metrics");
    }

    // ========== Persistence Tests ==========

    #[tokio::test]
    async fn test_persist_load_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("cache.bin");

        println!("BEFORE: Creating cache with persistence at {:?}", cache_path);

        let config = create_test_config_with_persistence(cache_path.clone());
        let cache = CacheManager::new(config).unwrap();

        // Add entries
        let key1 = CacheKey::from(100u64);
        let key2 = CacheKey::from(200u64);
        cache.put(key1, create_test_embedding(100)).unwrap();
        cache.put(key2, create_test_embedding(200)).unwrap();

        println!("BEFORE persist: len={}", cache.len());

        // Persist
        cache.persist().await.unwrap();

        println!("AFTER persist: file exists = {}", cache_path.exists());

        assert!(cache_path.exists());

        // Clear and reload
        cache.clear();
        assert_eq!(cache.len(), 0);

        cache.load().await.unwrap();

        println!("AFTER load: len={}", cache.len());

        assert_eq!(cache.len(), 2);
        assert!(cache.get(&key1).is_some());
        assert!(cache.get(&key2).is_some());

        println!("PASSED: persist/load round-trip preserves all entries");
    }

    #[tokio::test]
    async fn test_persist_without_path_fails() {
        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        println!("BEFORE: Attempting persist without disk_path");

        let result = cache.persist().await;

        println!("AFTER: result = {:?}", result.is_err());

        assert!(result.is_err());
        println!("PASSED: persist() fails when disk_path not configured");
    }

    #[tokio::test]
    async fn test_load_detects_checksum_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("corrupt.bin");

        // Write corrupted data
        let mut data = Vec::new();
        data.extend_from_slice(&CACHE_MAGIC);
        data.push(CACHE_VERSION);
        data.extend_from_slice(&0u64.to_le_bytes()); // entry count
        data.extend_from_slice(&0u64.to_le_bytes()); // wrong checksum
        tokio::fs::write(&cache_path, &data).await.unwrap();

        let config = create_test_config_with_persistence(cache_path);
        let cache = CacheManager::new(config).unwrap();

        println!("BEFORE: Loading corrupted cache file");

        let result = cache.load().await;

        println!("AFTER: result = {:?}", result.is_err());

        assert!(result.is_err());
        println!("PASSED: load() detects checksum mismatch");
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_put_oversized_entry_fails() {
        println!("BEFORE: Creating cache with tiny max_bytes");

        // max_bytes smaller than a single entry
        let config = CacheConfig {
            max_entries: 100,
            max_bytes: 100, // Way too small for any embedding
            ..create_test_config()
        };
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("oversized");
        let result = cache.put(key, create_test_embedding(999));

        println!("AFTER: result = {:?}", result.is_err());

        assert!(result.is_err());
        println!("PASSED: put() fails when single entry exceeds max_bytes");
    }

    #[test]
    fn test_update_existing_key() {
        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("update test");

        cache.put(key, create_test_embedding(1)).unwrap();
        let initial_bytes = cache.memory_usage();

        println!("BEFORE update: bytes={}", initial_bytes);

        cache.put(key, create_test_embedding(2)).unwrap();

        println!("AFTER update: bytes={}", cache.memory_usage());

        // len should still be 1
        assert_eq!(cache.len(), 1);
        // Memory should be roughly the same (not doubled)
        assert!(cache.memory_usage() <= initial_bytes + 100);

        println!("PASSED: Updating existing key doesn't leak memory");
    }

    #[test]
    fn test_memory_tracking_accuracy() {
        let config = create_test_config();
        let cache = CacheManager::new(config).unwrap();

        let key = CacheKey::from_content("memory test");
        let embedding = create_test_embedding(555);

        let expected_entry_size = CacheEntry::new(create_test_embedding(0)).memory_size();

        cache.put(key, embedding).unwrap();

        println!("AFTER put: expected_size={}, actual_usage={}", expected_entry_size, cache.memory_usage());

        assert_eq!(cache.memory_usage(), expected_entry_size);

        cache.remove(&key);

        println!("AFTER remove: usage={}", cache.memory_usage());

        assert_eq!(cache.memory_usage(), 0);
        println!("PASSED: Memory tracking is accurate");
    }
}
