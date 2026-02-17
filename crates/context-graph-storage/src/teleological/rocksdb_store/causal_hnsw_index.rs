//! HNSW index for E11 entity embeddings on causal relationships.
//!
//! This module provides O(log n) search for causal relationships using E11 KEPLER
//! entity embeddings. Replaces O(n) brute-force RocksDB scan.
//!
//! # Performance Targets
//!
//! | Relationships | Brute Force | HNSW Target | Speedup |
//! |---------------|-------------|-------------|---------|
//! | 100           | 395 µs      | 200 µs      | 2x      |
//! | 500           | 2.05 ms     | 400 µs      | 5x      |
//! | 1000          | 4.88 ms     | 500 µs      | 10x     |
//! | 5000          | 42.6 ms     | 1 ms        | 42x     |
//! | 50000         | ~400 ms     | 2 ms        | 200x    |
//!
//! # Constitution Compliance
//!
//! - ARCH-02: Apples-to-apples - E11↔E11 comparisons only
//! - ARCH-12: E1 is foundation - E11 is enhancer, not replacement
//! - E11 def: V_factuality 768D per constitution.yaml

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
// HIGH-17 FIX: parking_lot::RwLock is non-poisonable.
use parking_lot::RwLock;

use tracing::{debug, error, info, warn};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};

/// E11 KEPLER embedding dimension (768D per constitution.yaml).
pub const E11_DIM: usize = 768;

/// HNSW connectivity parameter (M).
/// Higher M = better recall, more memory.
const HNSW_M: usize = 16;

/// HNSW construction expansion factor (ef_construction).
/// Higher = better index quality, slower construction.
const HNSW_EF_CONSTRUCTION: usize = 200;

/// HNSW search expansion factor (ef_search).
/// Higher = better recall, slower search.
const HNSW_EF_SEARCH: usize = 100;

/// Initial capacity for the HNSW index.
/// Will grow automatically when exceeded.
const INITIAL_CAPACITY: usize = 1024;

/// HNSW index for E11 entity embeddings on causal relationships.
///
/// Uses usearch with 768D cosine similarity for O(log n) search.
/// Stores UUID associations for mapping back to causal relationships.
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability. Multiple readers can access
/// concurrently, but writes are exclusive.
///
/// # Example
///
/// ```ignore
/// let index = CausalE11Index::new();
///
/// // Insert embedding
/// let id = Uuid::new_v4();
/// let embedding = vec![0.1f32; 768];
/// index.insert(id, &embedding)?;
///
/// // Search
/// let results = index.search(&embedding, 10)?;
/// ```
pub struct CausalE11Index {
    /// usearch HNSW index - provides O(log n) graph traversal
    index: RwLock<Index>,
    /// UUID to usearch key mapping
    id_to_key: RwLock<HashMap<Uuid, u64>>,
    /// usearch key to UUID mapping (for result conversion)
    key_to_id: RwLock<HashMap<u64, Uuid>>,
    /// Next available key for usearch (monotonically increasing)
    next_key: RwLock<u64>,
    /// HIGH-2 FIX: Count of removed vectors still orphaned in usearch index.
    /// When removed_count / total_count > COMPACTION_RATIO, compaction is needed.
    removed_count: AtomicUsize,
}

impl CausalE11Index {
    /// Create a new E11 HNSW index for causal relationships.
    ///
    /// Initializes usearch Index with:
    /// - 768D dimension (E11 KEPLER)
    /// - Cosine metric
    /// - M=16 connectivity
    /// - ef_construction=200
    /// - ef_search=100
    ///
    /// # Panics
    ///
    /// Panics if usearch Index creation fails.
    pub fn new() -> Self {
        let options = IndexOptions {
            dimensions: E11_DIM,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: HNSW_M,
            expansion_add: HNSW_EF_CONSTRUCTION,
            expansion_search: HNSW_EF_SEARCH,
            ..Default::default()
        };

        let index = Index::new(&options).unwrap_or_else(|e| {
            panic!(
                "FAIL FAST: Failed to create usearch index for CausalE11Index: {}",
                e
            )
        });

        // Reserve initial capacity - usearch requires this before adding vectors
        index.reserve(INITIAL_CAPACITY).unwrap_or_else(|e| {
            panic!(
                "FAIL FAST: Failed to reserve capacity for CausalE11Index: {}",
                e
            )
        });

        info!(
            dimension = E11_DIM,
            m = HNSW_M,
            ef_construction = HNSW_EF_CONSTRUCTION,
            ef_search = HNSW_EF_SEARCH,
            "Created CausalE11Index HNSW index"
        );

        Self {
            index: RwLock::new(index),
            id_to_key: RwLock::new(HashMap::new()),
            key_to_id: RwLock::new(HashMap::new()),
            next_key: RwLock::new(0),
            removed_count: AtomicUsize::new(0),
        }
    }

    /// Insert a causal relationship's E11 embedding into the index.
    ///
    /// # Arguments
    /// * `id` - UUID of the causal relationship
    /// * `embedding` - E11 768D KEPLER embedding
    ///
    /// # Errors
    /// Returns error if embedding dimension is wrong or usearch operation fails.
    ///
    /// # Note
    /// If the ID already exists, the old mapping is removed and replaced.
    #[allow(clippy::readonly_write_lock)] // usearch uses interior mutability via C++ FFI
    pub fn insert(&self, id: Uuid, embedding: &[f32]) -> CoreResult<()> {
        // Validate dimension
        if embedding.len() != E11_DIM {
            return Err(CoreError::Internal(format!(
                "E11 dimension mismatch: expected {}, got {}",
                E11_DIM,
                embedding.len()
            )));
        }

        // Validate no NaN/Inf
        for (i, &v) in embedding.iter().enumerate() {
            if v.is_nan() || v.is_infinite() {
                return Err(CoreError::Internal(format!(
                    "E11 embedding contains invalid value at index {}: {}",
                    i, v
                )));
            }
        }

        let mut id_to_key = self.id_to_key.write();
        let mut key_to_id = self.key_to_id.write();
        let index = self.index.write();
        let mut next_key = self.next_key.write();

        // Handle duplicate - remove old mapping
        if let Some(&old_key) = id_to_key.get(&id) {
            key_to_id.remove(&old_key);
            // Note: usearch doesn't support deletion, so the old vector remains
            // orphaned in the index. Track for compaction threshold (HIGH-2).
            self.removed_count.fetch_add(1, Ordering::Relaxed);
            debug!(id = %id, old_key = old_key, "Replacing existing E11 embedding (old vector orphaned)");
        }

        // Ensure capacity - grow if needed
        let current_size = index.size();
        let current_capacity = index.capacity();
        if current_size >= current_capacity {
            let new_capacity = (current_capacity * 2).max(INITIAL_CAPACITY);
            index.reserve(new_capacity).map_err(|e| {
                error!("Failed to reserve capacity for CausalE11Index: {}", e);
                CoreError::Internal(format!("usearch reserve failed: {}", e))
            })?;
            debug!(
                old_capacity = current_capacity,
                new_capacity = new_capacity,
                "Grew CausalE11Index capacity"
            );
        }

        // Allocate new key
        let key = *next_key;
        *next_key += 1;

        // Update mappings
        id_to_key.insert(id, key);
        key_to_id.insert(key, id);

        // Add to usearch index - O(log n) HNSW graph insertion
        index.add(key, embedding).map_err(|e| {
            error!("Failed to add to CausalE11Index: {}", e);
            CoreError::Internal(format!("usearch add failed: {}", e))
        })?;

        Ok(())
    }

    /// Search for similar causal relationships using E11 embeddings.
    ///
    /// Uses O(log n) HNSW graph traversal instead of O(n) brute force.
    ///
    /// # Arguments
    /// * `query_embedding` - E11 768D query embedding
    /// * `k` - Number of results to return
    ///
    /// # Returns
    /// Vector of (causal_id, distance) tuples, sorted by distance ascending.
    /// Distance is cosine distance (0 = identical, 2 = opposite).
    ///
    /// # Errors
    /// Returns error if embedding dimension is wrong or usearch operation fails.
    pub fn search(&self, query_embedding: &[f32], k: usize) -> CoreResult<Vec<(Uuid, f32)>> {
        // Validate dimension
        if query_embedding.len() != E11_DIM {
            return Err(CoreError::Internal(format!(
                "E11 query dimension mismatch: expected {}, got {}",
                E11_DIM,
                query_embedding.len()
            )));
        }

        let index = self.index.read();
        let key_to_id = self.key_to_id.read();

        if key_to_id.is_empty() {
            return Ok(Vec::new());
        }

        // Compute effective k - can't return more than we have
        let active_count = key_to_id.len();
        let request_k = if k > active_count {
            // Request all vectors plus buffer for potentially removed entries
            index.size().max(k)
        } else {
            // Request k + buffer for potentially removed entries
            k * 2
        };

        // O(log n) HNSW graph traversal - NOT brute force!
        let results = index.search(query_embedding, request_k).map_err(|e| {
            error!("CausalE11Index search failed: {}", e);
            CoreError::Internal(format!("usearch search failed: {}", e))
        })?;

        // Map keys back to UUIDs, filtering removed entries
        let mut output = Vec::with_capacity(k.min(active_count));
        for (key, distance) in results.keys.iter().zip(results.distances.iter()) {
            if let Some(&id) = key_to_id.get(key) {
                // Convert distance to similarity: similarity = 1 - distance
                // usearch cosine returns distance in [0, 2]
                let similarity = 1.0 - distance.min(1.0);
                output.push((id, similarity));
                if output.len() >= k {
                    break;
                }
            }
        }

        Ok(output)
    }

    /// Remove a causal relationship from the index.
    ///
    /// # Arguments
    /// * `id` - UUID of the causal relationship to remove
    ///
    /// # Returns
    /// True if the relationship was removed, false if it wasn't in the index.
    ///
    /// # Note
    /// The vector remains in the usearch index (usearch doesn't support deletion)
    /// but won't be returned in search results.
    /// HIGH-2 FIX: Tracks removal count for compaction threshold monitoring.
    pub fn remove(&self, id: Uuid) -> bool {
        let mut id_to_key = self.id_to_key.write();
        let mut key_to_id = self.key_to_id.write();

        if let Some(key) = id_to_key.remove(&id) {
            // Remove from key_to_id so search won't return this ID.
            // HIGH-2 FIX: Vector remains orphaned in usearch (no deletion support).
            // Track removal count for compaction threshold monitoring.
            key_to_id.remove(&key);
            self.removed_count.fetch_add(1, Ordering::Relaxed);
            debug!(id = %id, "Removed from CausalE11Index");
            true
        } else {
            false
        }
    }

    /// Get the number of active entries in the index.
    ///
    /// This counts only non-removed entries. The usearch index may contain
    /// more vectors due to soft-deletion.
    pub fn len(&self) -> usize {
        self.key_to_id.read().len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a causal relationship ID exists in the index.
    pub fn contains(&self, id: Uuid) -> bool {
        self.id_to_key.read().contains_key(&id)
    }

    // =========================================================================
    // HIGH-2 FIX: COMPACTION TRACKING (mirrors HnswEmbedderIndex pattern)
    // =========================================================================

    /// HIGH-2 FIX: Number of orphaned vectors in usearch index (removed from maps but still in graph).
    pub fn removed_count(&self) -> usize {
        self.removed_count.load(Ordering::Relaxed)
    }

    /// HIGH-2 FIX: Total vectors in usearch index (including orphaned).
    pub fn usearch_size(&self) -> usize {
        self.index.read().size()
    }

    /// HIGH-2 FIX: Check if compaction is needed (removed/total > 25%).
    /// Compaction rebuilds the index, eliminating orphaned vectors.
    pub fn needs_compaction(&self) -> bool {
        let total = self.usearch_size();
        if total == 0 {
            return false;
        }
        let removed = self.removed_count();
        // Compact when >25% of vectors are orphaned
        removed * 4 > total
    }

    /// HIGH-2 FIX: Rebuild the index from the currently active vectors.
    ///
    /// Creates a new usearch index containing only the active (non-removed) vectors.
    /// This eliminates orphaned vectors that accumulate from soft-delete operations.
    ///
    /// # Arguments
    /// * `vectors` - Iterator of (id, embedding) pairs for all active entries.
    ///   The caller must provide vectors from the authoritative store (e.g., RocksDB).
    ///
    /// # Returns
    /// Number of vectors in the rebuilt index.
    ///
    /// # Errors
    /// Returns error if usearch operations fail during rebuild.
    pub fn rebuild(&self, vectors: &[(Uuid, Vec<f32>)]) -> CoreResult<usize> {
        let old_removed = self.removed_count();
        let old_total = self.usearch_size();

        info!(
            old_total = old_total,
            old_removed = old_removed,
            active = vectors.len(),
            "Rebuilding CausalE11Index to eliminate orphaned vectors"
        );

        // Create new index with same configuration
        let options = IndexOptions {
            dimensions: E11_DIM,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: HNSW_M,
            expansion_add: HNSW_EF_CONSTRUCTION,
            expansion_search: HNSW_EF_SEARCH,
            ..Default::default()
        };

        let new_index = Index::new(&options).map_err(|e| {
            error!("Failed to create new usearch index during CausalE11Index rebuild: {}", e);
            CoreError::Internal(format!("usearch rebuild failed: {}", e))
        })?;

        let capacity = vectors.len().max(INITIAL_CAPACITY);
        new_index.reserve(capacity).map_err(|e| {
            error!("Failed to reserve capacity during CausalE11Index rebuild: {}", e);
            CoreError::Internal(format!("usearch reserve failed during rebuild: {}", e))
        })?;

        // Build new mappings and insert all active vectors
        let mut new_id_to_key = HashMap::with_capacity(vectors.len());
        let mut new_key_to_id = HashMap::with_capacity(vectors.len());
        let mut next_key: u64 = 0;

        for (id, embedding) in vectors {
            if embedding.len() != E11_DIM {
                warn!(
                    id = %id,
                    dim = embedding.len(),
                    "Skipping vector with wrong dimension during CausalE11Index rebuild"
                );
                continue;
            }

            new_id_to_key.insert(*id, next_key);
            new_key_to_id.insert(next_key, *id);

            new_index.add(next_key, embedding).map_err(|e| {
                error!(id = %id, key = next_key, "Failed to add vector during CausalE11Index rebuild: {}", e);
                CoreError::Internal(format!("usearch add failed during rebuild: {}", e))
            })?;

            next_key += 1;
        }

        // Swap in the new index atomically (lock order: id_to_key -> key_to_id -> index -> next_key)
        let mut id_to_key = self.id_to_key.write();
        let mut key_to_id = self.key_to_id.write();
        let mut index = self.index.write();
        let mut next_key_lock = self.next_key.write();

        *index = new_index;
        *id_to_key = new_id_to_key;
        *key_to_id = new_key_to_id;
        *next_key_lock = next_key;

        // Reset removed count after successful rebuild
        self.removed_count.store(0, Ordering::Relaxed);

        let rebuilt_count = next_key as usize;
        info!(
            rebuilt_count = rebuilt_count,
            eliminated = old_total.saturating_sub(rebuilt_count),
            "CausalE11Index rebuild complete"
        );

        Ok(rebuilt_count)
    }

    /// HIGH-2 FIX: Reset removed count (e.g., after external compaction).
    pub fn reset_removed_count(&self) {
        self.removed_count.store(0, Ordering::Relaxed);
    }

    /// Get memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        let index = self.index.read();
        let id_to_key = self.id_to_key.read();
        let key_to_id = self.key_to_id.read();

        let usearch_memory = index.memory_usage();
        let overhead = std::mem::size_of::<CausalE11Index>();
        let id_map_bytes = id_to_key.capacity() * (16 + 8); // UUID (16) + u64 (8)
        let key_map_bytes = key_to_id.capacity() * (8 + 16); // u64 (8) + UUID (16)

        usearch_memory + overhead + id_map_bytes + key_map_bytes
    }

    /// Clear the index, removing all entries.
    ///
    /// This creates a new usearch index and clears all mappings.
    pub fn clear(&self) {
        let options = IndexOptions {
            dimensions: E11_DIM,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: HNSW_M,
            expansion_add: HNSW_EF_CONSTRUCTION,
            expansion_search: HNSW_EF_SEARCH,
            ..Default::default()
        };

        let new_index = Index::new(&options).unwrap_or_else(|e| {
            panic!("FAIL FAST: Failed to recreate usearch index: {}", e)
        });

        new_index.reserve(INITIAL_CAPACITY).unwrap_or_else(|e| {
            panic!("FAIL FAST: Failed to reserve capacity: {}", e)
        });

        // BLD-09 FIX: Acquire locks in the SAME order as insert() to prevent
        // ABBA deadlock. insert() order: id_to_key → key_to_id → index → next_key.
        // Old clear() order was: index → id_to_key → key_to_id → next_key (inverted).
        let mut id_to_key = self.id_to_key.write();
        let mut key_to_id = self.key_to_id.write();
        let mut index = self.index.write();
        let mut next_key = self.next_key.write();

        *index = new_index;
        id_to_key.clear();
        key_to_id.clear();
        *next_key = 0;

        // HIGH-2 FIX: Reset removed count on clear
        self.removed_count.store(0, Ordering::Relaxed);

        info!("Cleared CausalE11Index");
    }
}

impl Default for CausalE11Index {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_random_embedding(seed: u64) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut embedding = Vec::with_capacity(E11_DIM);
        for i in 0..E11_DIM {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            let hash = hasher.finish();
            embedding.push((hash as f32 / u64::MAX as f32) - 0.5);
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= norm);
        }

        embedding
    }

    #[test]
    fn test_create_index() {
        let index = CausalE11Index::new();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_insert_and_search() {
        let index = CausalE11Index::new();
        let id = Uuid::new_v4();
        let embedding = generate_random_embedding(42);

        // Insert
        index.insert(id, &embedding).expect("Insert failed");
        assert_eq!(index.len(), 1);
        assert!(!index.is_empty());
        assert!(index.contains(id));

        // Search
        let results = index.search(&embedding, 1).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
        assert!(results[0].1 > 0.99, "Self-similarity should be ~1.0");
    }

    #[test]
    fn test_insert_multiple_and_search() {
        let index = CausalE11Index::new();
        let mut ids = Vec::new();

        // Insert 100 embeddings
        for i in 0..100 {
            let id = Uuid::new_v4();
            let embedding = generate_random_embedding(i);
            index.insert(id, &embedding).expect("Insert failed");
            ids.push((id, embedding));
        }

        assert_eq!(index.len(), 100);

        // Search for each and verify self is found
        for (id, embedding) in &ids {
            let results = index.search(embedding, 10).expect("Search failed");
            assert!(!results.is_empty());
            // First result should be self
            assert_eq!(results[0].0, *id);
        }
    }

    #[test]
    fn test_remove() {
        let index = CausalE11Index::new();
        let id = Uuid::new_v4();
        let embedding = generate_random_embedding(42);

        index.insert(id, &embedding).expect("Insert failed");
        assert_eq!(index.len(), 1);

        // Remove
        let removed = index.remove(id);
        assert!(removed);
        assert_eq!(index.len(), 0);
        assert!(!index.contains(id));

        // Remove again should return false
        let removed_again = index.remove(id);
        assert!(!removed_again);

        // Search should not find it
        let results = index.search(&embedding, 1).expect("Search failed");
        assert!(results.is_empty() || results[0].0 != id);
    }

    #[test]
    fn test_duplicate_insert_replaces() {
        let index = CausalE11Index::new();
        let id = Uuid::new_v4();
        let embedding1 = generate_random_embedding(42);
        let embedding2 = generate_random_embedding(43);

        // Insert first
        index.insert(id, &embedding1).expect("Insert failed");
        assert_eq!(index.len(), 1);

        // Insert again with different embedding
        index.insert(id, &embedding2).expect("Insert failed");
        assert_eq!(index.len(), 1); // Still 1

        // Search with new embedding should find it
        let results = index.search(&embedding2, 1).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
    }

    #[test]
    fn test_dimension_mismatch() {
        let index = CausalE11Index::new();
        let id = Uuid::new_v4();

        // Wrong dimension
        let wrong_dim = vec![0.1f32; 512];
        let result = index.insert(id, &wrong_dim);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("dimension mismatch"));

        // Wrong dimension for search
        let result = index.search(&wrong_dim, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_embedding_nan() {
        let index = CausalE11Index::new();
        let id = Uuid::new_v4();

        let mut embedding = generate_random_embedding(42);
        embedding[0] = f32::NAN;

        let result = index.insert(id, &embedding);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid value"));
    }

    #[test]
    fn test_clear() {
        let index = CausalE11Index::new();

        // Insert some
        for i in 0..10 {
            let id = Uuid::new_v4();
            let embedding = generate_random_embedding(i);
            index.insert(id, &embedding).expect("Insert failed");
        }
        assert_eq!(index.len(), 10);

        // Clear
        index.clear();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_memory_bytes() {
        let index = CausalE11Index::new();
        let initial_memory = index.memory_bytes();

        // Insert some
        for i in 0..100 {
            let id = Uuid::new_v4();
            let embedding = generate_random_embedding(i);
            index.insert(id, &embedding).expect("Insert failed");
        }

        let after_memory = index.memory_bytes();
        assert!(after_memory > initial_memory);
    }

    #[test]
    fn test_search_empty() {
        let index = CausalE11Index::new();
        let query = generate_random_embedding(42);

        let results = index.search(&query, 10).expect("Search failed");
        assert!(results.is_empty());
    }

    #[test]
    fn test_compaction_tracking() {
        let index = CausalE11Index::new();

        // Insert 10 embeddings
        let mut ids = Vec::new();
        for i in 0..10 {
            let id = Uuid::new_v4();
            let embedding = generate_random_embedding(i);
            index.insert(id, &embedding).expect("Insert failed");
            ids.push(id);
        }

        assert_eq!(index.len(), 10);
        assert_eq!(index.removed_count(), 0);
        assert!(!index.needs_compaction());

        // Remove 3 entries (30% > 25% threshold)
        for id in &ids[..3] {
            assert!(index.remove(*id));
        }

        assert_eq!(index.len(), 7);
        assert_eq!(index.removed_count(), 3);
        // usearch_size() should still be 10 (orphaned vectors remain)
        assert_eq!(index.usearch_size(), 10);
        // 3/10 = 30% > 25% => needs compaction
        assert!(index.needs_compaction());
    }

    #[test]
    fn test_compaction_rebuild() {
        let index = CausalE11Index::new();

        // Insert 10 embeddings
        let mut all_entries = Vec::new();
        for i in 0..10 {
            let id = Uuid::new_v4();
            let embedding = generate_random_embedding(i);
            index.insert(id, &embedding).expect("Insert failed");
            all_entries.push((id, embedding));
        }

        // Remove 5 entries
        for (id, _) in &all_entries[..5] {
            index.remove(*id);
        }

        assert_eq!(index.removed_count(), 5);
        assert!(index.needs_compaction());

        // Rebuild with only the active vectors
        let active: Vec<(Uuid, Vec<f32>)> = all_entries[5..].to_vec();
        let rebuilt_count = index.rebuild(&active).expect("Rebuild failed");

        assert_eq!(rebuilt_count, 5);
        assert_eq!(index.len(), 5);
        assert_eq!(index.removed_count(), 0);
        assert_eq!(index.usearch_size(), 5);
        assert!(!index.needs_compaction());

        // Verify search still works after rebuild
        let (_, ref embedding) = all_entries[5];
        let results = index.search(embedding, 1).expect("Search failed");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_k_larger_than_index() {
        let index = CausalE11Index::new();

        // Insert 5
        for i in 0..5 {
            let id = Uuid::new_v4();
            let embedding = generate_random_embedding(i);
            index.insert(id, &embedding).expect("Insert failed");
        }

        // Search for 100
        let query = generate_random_embedding(42);
        let results = index.search(&query, 100).expect("Search failed");

        // Should return all 5
        assert_eq!(results.len(), 5);
    }
}
