//! Cache key and entry types for the embedding cache system.
//!
//! This module provides the core types for caching FusedEmbedding results:
//! - [`CacheKey`]: Unique key derived from xxHash64 content hash
//! - [`CacheEntry`]: Cached embedding with LRU/LFU metadata

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use xxhash_rust::xxh64::xxh64;

use crate::types::{FusedEmbedding, ModelInput};

/// Process start instant for relative timestamp storage.
/// Using nanos since start allows compact u64 atomic storage.
static START_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);

/// Cache key derived from xxHash64 content hash.
///
/// # Design Rationale
/// - `Copy` + `Eq` + `Hash` enables direct HashMap key usage
/// - 8 bytes = single register, no allocation
/// - xxHash64 collision probability: ~1/2^64
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// xxHash64 of content (from ModelInput::content_hash() or FusedEmbedding.content_hash)
    pub content_hash: u64,
}

impl CacheKey {
    /// Create key from raw text content.
    /// Uses xxHash64 internally (same as ModelInput::content_hash).
    #[must_use]
    pub fn from_content(content: &str) -> Self {
        Self {
            content_hash: xxh64(content.as_bytes(), 0),
        }
    }

    /// Create key from ModelInput.
    /// Simply wraps ModelInput::content_hash() which already uses xxHash64.
    #[must_use]
    pub fn from_input(input: &ModelInput) -> Self {
        Self {
            content_hash: input.content_hash(),
        }
    }

    /// Create key from FusedEmbedding.
    /// Uses the pre-computed content_hash field.
    #[must_use]
    pub fn from_embedding(embedding: &FusedEmbedding) -> Self {
        Self {
            content_hash: embedding.content_hash,
        }
    }
}

impl From<u64> for CacheKey {
    fn from(hash: u64) -> Self {
        Self { content_hash: hash }
    }
}

/// Cached embedding with LRU/LFU metadata.
///
/// # Memory Layout (estimated)
/// - FusedEmbedding: ~6198 bytes (1536 f32s + metadata)
/// - Instant: 16 bytes
/// - AtomicU64: 8 bytes
/// - AtomicU32: 4 bytes
/// - Total: ~6226 bytes per entry
///
/// # Thread Safety
/// - `last_accessed` and `access_count` use atomics for lock-free updates
/// - `embedding` is read-only after creation
#[derive(Debug)]
pub struct CacheEntry {
    /// The cached fused embedding (immutable after creation)
    pub embedding: FusedEmbedding,
    /// Creation timestamp for TTL expiration
    created_at: Instant,
    /// Last access time as nanos since process start (for LRU)
    last_accessed: AtomicU64,
    /// Access count (for LFU)
    access_count: AtomicU32,
}

/// Metadata size for CacheEntry (Instant + AtomicU64 + AtomicU32).
const CACHE_ENTRY_METADATA_SIZE: usize = 16 + 8 + 4;

impl CacheEntry {
    /// Create new cache entry with current timestamp.
    /// Sets `last_accessed` to now, `access_count` to 1.
    #[must_use]
    pub fn new(embedding: FusedEmbedding) -> Self {
        let now = START_INSTANT.elapsed().as_nanos() as u64;
        Self {
            embedding,
            created_at: Instant::now(),
            last_accessed: AtomicU64::new(now),
            access_count: AtomicU32::new(1),
        }
    }

    /// Update last_accessed timestamp (for LRU policy).
    /// Uses Ordering::Relaxed - eventual consistency is acceptable.
    pub fn touch(&self) {
        let now = START_INSTANT.elapsed().as_nanos() as u64;
        self.last_accessed.store(now, Ordering::Relaxed);
    }

    /// Increment access count (for LFU policy).
    /// Uses Ordering::Relaxed.
    pub fn increment_access(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current access count.
    #[must_use]
    pub fn access_count(&self) -> u32 {
        self.access_count.load(Ordering::Relaxed)
    }

    /// Time since creation.
    #[must_use]
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Check if entry has expired based on TTL.
    #[must_use]
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.age() >= ttl
    }

    /// Total memory size in bytes (for max_bytes budget).
    /// Returns: embedding.memory_size() + sizeof(metadata)
    #[must_use]
    pub fn memory_size(&self) -> usize {
        self.embedding.memory_size() + CACHE_ENTRY_METADATA_SIZE
    }

    /// Get creation timestamp.
    #[must_use]
    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    /// Get last access time as duration since process start.
    #[must_use]
    pub fn last_accessed(&self) -> Duration {
        let nanos = self.last_accessed.load(Ordering::Relaxed);
        Duration::from_nanos(nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::dimensions::FUSED_OUTPUT;

    // ========== Helper Functions (NO MOCK DATA) ==========

    fn make_real_fused_embedding() -> FusedEmbedding {
        let vector = vec![0.1f32; FUSED_OUTPUT]; // Real 1536D dimension
        let weights = [0.125f32; 8]; // Real 8 experts, sum = 1.0
        FusedEmbedding::new(vector, weights, [0, 1], 100, 0xDEADBEEF)
            .expect("Test helper should create valid embedding")
    }

    // ========== CacheKey Tests ==========

    #[test]
    fn test_cache_key_from_content_same_content_same_hash() {
        println!("BEFORE: Creating CacheKey from 'Hello, World!'");
        let key1 = CacheKey::from_content("Hello, World!");
        let key2 = CacheKey::from_content("Hello, World!");

        println!("AFTER: key1.content_hash = {:#x}", key1.content_hash);
        println!("AFTER: key2.content_hash = {:#x}", key2.content_hash);

        assert_eq!(key1, key2);
        assert_eq!(key1.content_hash, key2.content_hash);
        println!("PASSED: Same content produces identical hash");
    }

    #[test]
    fn test_cache_key_from_content_different_content_different_hash() {
        println!("BEFORE: Creating CacheKeys from different content");
        let key1 = CacheKey::from_content("Hello");
        let key2 = CacheKey::from_content("World");

        println!("AFTER: key1.content_hash = {:#x}", key1.content_hash);
        println!("AFTER: key2.content_hash = {:#x}", key2.content_hash);

        assert_ne!(key1, key2);
        assert_ne!(key1.content_hash, key2.content_hash);
        println!("PASSED: Different content produces different hash");
    }

    #[test]
    fn test_cache_key_from_input_matches_model_input_content_hash() {
        let input = ModelInput::text("Test content for hashing").unwrap();
        let expected_hash = input.content_hash();

        println!("BEFORE: ModelInput::content_hash() = {:#x}", expected_hash);

        let key = CacheKey::from_input(&input);

        println!("AFTER: CacheKey.content_hash = {:#x}", key.content_hash);

        assert_eq!(key.content_hash, expected_hash);
        println!("PASSED: CacheKey::from_input() matches ModelInput::content_hash()");
    }

    #[test]
    fn test_cache_key_from_embedding_matches_fused_embedding_content_hash() {
        let embedding = make_real_fused_embedding();
        let expected_hash = embedding.content_hash;

        println!(
            "BEFORE: FusedEmbedding.content_hash = {:#x}",
            expected_hash
        );

        let key = CacheKey::from_embedding(&embedding);

        println!("AFTER: CacheKey.content_hash = {:#x}", key.content_hash);

        assert_eq!(key.content_hash, expected_hash);
        println!("PASSED: CacheKey::from_embedding() matches FusedEmbedding.content_hash");
    }

    #[test]
    fn test_cache_key_from_u64() {
        let hash = 0x123456789ABCDEF0_u64;

        println!("BEFORE: Raw hash = {:#x}", hash);

        let key: CacheKey = hash.into();

        println!("AFTER: CacheKey.content_hash = {:#x}", key.content_hash);

        assert_eq!(key.content_hash, hash);
        println!("PASSED: CacheKey::from(u64) works correctly");
    }

    #[test]
    fn test_cache_key_is_copy() {
        let key = CacheKey::from_content("copy test");
        let key_copy = key; // This should work since CacheKey is Copy

        assert_eq!(key, key_copy);
        println!("PASSED: CacheKey is Copy (8 bytes, no allocation)");
    }

    #[test]
    fn test_cache_key_size_is_8_bytes() {
        let size = std::mem::size_of::<CacheKey>();

        println!("CacheKey size = {} bytes", size);

        assert_eq!(size, 8);
        println!("PASSED: CacheKey is exactly 8 bytes");
    }

    // ========== CacheEntry Tests ==========

    #[test]
    fn test_cache_entry_new_sets_access_count_to_1() {
        println!("BEFORE: Creating CacheEntry");

        let entry = CacheEntry::new(make_real_fused_embedding());

        println!("AFTER: access_count = {}", entry.access_count());

        assert_eq!(entry.access_count(), 1);
        println!("PASSED: CacheEntry::new() sets access_count to 1");
    }

    #[test]
    fn test_cache_entry_touch_updates_last_accessed() {
        let entry = CacheEntry::new(make_real_fused_embedding());
        let initial = entry.last_accessed();

        println!("BEFORE: last_accessed = {:?}", initial);

        std::thread::sleep(std::time::Duration::from_millis(10));
        entry.touch();

        let after = entry.last_accessed();

        println!("AFTER: last_accessed = {:?}", after);

        assert!(after > initial, "last_accessed should increase after touch()");
        println!("PASSED: CacheEntry::touch() updates last_accessed");
    }

    #[test]
    fn test_cache_entry_increment_access_increases_count() {
        let entry = CacheEntry::new(make_real_fused_embedding());

        println!("BEFORE: access_count = {}", entry.access_count());

        entry.increment_access();
        entry.increment_access();
        entry.increment_access();

        println!("AFTER: access_count = {}", entry.access_count());

        assert_eq!(entry.access_count(), 4); // 1 initial + 3 increments
        println!("PASSED: CacheEntry::increment_access() increases count");
    }

    #[test]
    fn test_cache_entry_is_expired_with_zero_ttl() {
        let entry = CacheEntry::new(make_real_fused_embedding());

        println!("BEFORE: age = {:?}", entry.age());
        println!("BEFORE: is_expired(0s) = {}", entry.is_expired(Duration::ZERO));

        // With TTL of 0, entry should be expired immediately
        assert!(
            entry.is_expired(Duration::ZERO),
            "Zero TTL should expire immediately"
        );

        // With large TTL, entry should NOT be expired
        assert!(
            !entry.is_expired(Duration::from_secs(3600)),
            "1 hour TTL should not expire"
        );

        println!("AFTER: Expiration logic verified");
        println!("PASSED: Zero TTL expires immediately, large TTL does not");
    }

    #[test]
    fn test_cache_entry_is_expired_after_ttl() {
        let entry = CacheEntry::new(make_real_fused_embedding());

        println!("BEFORE: age = {:?}", entry.age());

        // Wait a bit
        std::thread::sleep(std::time::Duration::from_millis(50));

        let age = entry.age();
        println!("AFTER: age = {:?}", age);

        // Should be expired with 10ms TTL
        assert!(
            entry.is_expired(Duration::from_millis(10)),
            "Should be expired after 50ms with 10ms TTL"
        );

        // Should NOT be expired with 1s TTL
        assert!(
            !entry.is_expired(Duration::from_secs(1)),
            "Should not be expired with 1s TTL"
        );

        println!("PASSED: is_expired() correctly checks against TTL");
    }

    #[test]
    fn test_cache_entry_memory_size_at_least_6200_bytes() {
        let entry = CacheEntry::new(make_real_fused_embedding());

        let size = entry.memory_size();

        println!("BEFORE: Creating CacheEntry");
        println!("AFTER: CacheEntry.memory_size() = {} bytes", size);

        // FusedEmbedding base: 6198 bytes
        // + Instant: 16 bytes
        // + AtomicU64: 8 bytes
        // + AtomicU32: 4 bytes
        // Total: ~6226 bytes minimum
        assert!(size >= 6200, "Memory size should be at least 6200 bytes, got {}", size);
        println!("PASSED: CacheEntry.memory_size() >= 6200 bytes");
    }

    #[test]
    fn test_cache_entry_created_at_returns_instant() {
        let before = Instant::now();
        let entry = CacheEntry::new(make_real_fused_embedding());
        let after = Instant::now();

        let created_at = entry.created_at();

        println!("BEFORE: Instant::now() = {:?}", before);
        println!("AFTER: entry.created_at() = {:?}", created_at);

        assert!(created_at >= before);
        assert!(created_at <= after);
        println!("PASSED: created_at() returns valid Instant");
    }

    // ========== Edge Case Tests with Before/After State Logging ==========

    #[test]
    fn test_edge_case_empty_string_cache_key() {
        println!("BEFORE: Creating CacheKey from empty string");

        let key = CacheKey::from_content("");

        println!("AFTER: CacheKey.content_hash = {:#x}", key.content_hash);

        // xxHash64 of empty string is NOT zero
        assert_ne!(key.content_hash, 0, "Empty string should produce non-zero hash");
        println!("PASSED: Empty string produces valid non-zero hash");
    }

    #[test]
    fn test_edge_case_access_count_saturation() {
        let entry = CacheEntry::new(make_real_fused_embedding());

        println!("BEFORE: access_count = {}", entry.access_count());

        // Simulate many accesses
        for _ in 0..1000 {
            entry.increment_access();
        }

        println!("AFTER: access_count = {}", entry.access_count());

        assert_eq!(
            entry.access_count(),
            1001,
            "Count should be 1 (initial) + 1000"
        );
        println!("PASSED: Access count increments correctly");
    }

    #[test]
    fn test_edge_case_zero_ttl_expires_immediately() {
        let entry = CacheEntry::new(make_real_fused_embedding());

        println!("BEFORE: age = {:?}", entry.age());
        println!(
            "BEFORE: is_expired(0s) = {}",
            entry.is_expired(Duration::ZERO)
        );

        // With TTL of 0, entry should be expired immediately
        assert!(
            entry.is_expired(Duration::ZERO),
            "Zero TTL should expire immediately"
        );

        // With large TTL, entry should NOT be expired
        assert!(
            !entry.is_expired(Duration::from_secs(3600)),
            "1 hour TTL should not expire"
        );

        println!("AFTER: Expiration logic verified");
        println!("PASSED: Zero TTL expires immediately, large TTL does not");
    }

    #[test]
    fn test_edge_case_cache_key_hash_properties() {
        // Test that CacheKey implements Hash correctly
        use std::collections::HashMap;

        let key1 = CacheKey::from_content("test1");
        let key2 = CacheKey::from_content("test2");
        let key1_dup = CacheKey::from_content("test1");

        let mut map: HashMap<CacheKey, &str> = HashMap::new();
        map.insert(key1, "value1");
        map.insert(key2, "value2");

        println!("BEFORE: HashMap with 2 keys");
        println!("AFTER: map.get(key1) = {:?}", map.get(&key1));
        println!("AFTER: map.get(key1_dup) = {:?}", map.get(&key1_dup));
        println!("AFTER: map.get(key2) = {:?}", map.get(&key2));

        assert_eq!(map.get(&key1), Some(&"value1"));
        assert_eq!(map.get(&key1_dup), Some(&"value1")); // Same content = same key
        assert_eq!(map.get(&key2), Some(&"value2"));

        println!("PASSED: CacheKey works correctly as HashMap key");
    }

    #[test]
    fn test_cache_entry_embedding_is_accessible() {
        let original_hash = 0xDEADBEEF_u64;
        let entry = CacheEntry::new(make_real_fused_embedding());

        println!("BEFORE: entry.embedding.content_hash = {:#x}", original_hash);
        println!(
            "AFTER: entry.embedding.content_hash = {:#x}",
            entry.embedding.content_hash
        );

        assert_eq!(entry.embedding.content_hash, original_hash);
        assert_eq!(entry.embedding.vector.len(), FUSED_OUTPUT);
        println!("PASSED: CacheEntry.embedding is publicly accessible");
    }
}
