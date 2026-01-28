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
use std::sync::RwLock;

use tracing::{debug, error, info};
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

        let mut id_to_key = self.id_to_key.write().unwrap();
        let mut key_to_id = self.key_to_id.write().unwrap();
        let index = self.index.write().unwrap();
        let mut next_key = self.next_key.write().unwrap();

        // Handle duplicate - remove old mapping
        if let Some(&old_key) = id_to_key.get(&id) {
            key_to_id.remove(&old_key);
            // Note: usearch doesn't support deletion, so the old vector remains
            // but won't be returned because key_to_id doesn't map it back
            debug!(id = %id, old_key = old_key, "Replacing existing E11 embedding");
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

        let index = self.index.read().unwrap();
        let key_to_id = self.key_to_id.read().unwrap();

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
                let similarity = 1.0 - distance;
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
    pub fn remove(&self, id: Uuid) -> bool {
        let mut id_to_key = self.id_to_key.write().unwrap();
        let mut key_to_id = self.key_to_id.write().unwrap();

        if let Some(key) = id_to_key.remove(&id) {
            key_to_id.remove(&key);
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
        self.key_to_id.read().unwrap().len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a causal relationship ID exists in the index.
    pub fn contains(&self, id: Uuid) -> bool {
        self.id_to_key.read().unwrap().contains_key(&id)
    }

    /// Get memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        let index = self.index.read().unwrap();
        let id_to_key = self.id_to_key.read().unwrap();
        let key_to_id = self.key_to_id.read().unwrap();

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

        *self.index.write().unwrap() = new_index;
        self.id_to_key.write().unwrap().clear();
        self.key_to_id.write().unwrap().clear();
        *self.next_key.write().unwrap() = 0;

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
