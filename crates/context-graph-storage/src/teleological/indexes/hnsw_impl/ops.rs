//! EmbedderIndexOps trait implementation for HnswEmbedderIndex.
//!
//! Implements O(log n) insert and search operations via usearch HNSW graph.

use uuid::Uuid;

use super::super::embedder_index::{validate_vector, EmbedderIndexOps, IndexError, IndexResult};
use super::super::hnsw_config::{EmbedderIndex, HnswConfig};
use super::types::HnswEmbedderIndex;

impl EmbedderIndexOps for HnswEmbedderIndex {
    fn embedder(&self) -> EmbedderIndex {
        self.embedder
    }

    fn config(&self) -> &HnswConfig {
        &self.config
    }

    fn len(&self) -> usize {
        // Return the number of active (non-removed) IDs
        self.key_to_id.read().len()
    }

    #[allow(clippy::readonly_write_lock)] // usearch uses interior mutability via C++ FFI
    fn insert(&self, id: Uuid, vector: &[f32]) -> IndexResult<()> {
        validate_vector(vector, self.config.dimension, self.embedder)?;

        let mut id_to_key = self.id_to_key.write();
        let mut key_to_id = self.key_to_id.write();
        let index = self.index.write();
        let mut next_key = self.next_key.write();

        // Handle duplicate - remove old mapping (usearch may not support true deletion)
        if let Some(&old_key) = id_to_key.get(&id) {
            key_to_id.remove(&old_key);
            // Note: usearch doesn't support deletion, so the old vector remains in index
            // but won't be returned because key_to_id doesn't map it back
        }

        // Ensure capacity - grow if needed
        let current_size = index.size();
        let current_capacity = index.capacity();
        if current_size >= current_capacity {
            // Double capacity when full
            let new_capacity = (current_capacity * 2).max(1024);
            index
                .reserve(new_capacity)
                .map_err(|e| IndexError::OperationFailed {
                    embedder: self.embedder,
                    message: format!("usearch reserve failed: {}", e),
                })?;
        }

        // Allocate new key
        let key = *next_key;
        *next_key += 1;

        // Update mappings
        id_to_key.insert(id, key);
        key_to_id.insert(key, id);

        // Add to usearch index - O(log n) HNSW graph insertion
        index
            .add(key, vector)
            .map_err(|e| IndexError::OperationFailed {
                embedder: self.embedder,
                message: format!("usearch add failed: {}", e),
            })?;

        Ok(())
    }

    /// LOW-17 Note: `HashMap::remove` does not shrink the map's capacity, so
    /// memory usage grows monotonically between compactions. This is expected:
    /// calling `shrink_to_fit()` on every remove would be worse for performance
    /// (repeated reallocation). When the H1 compaction threshold is reached,
    /// the entire index (including its HashMaps) is rebuilt from scratch,
    /// reclaiming all orphaned capacity.
    fn remove(&self, id: Uuid) -> IndexResult<bool> {
        let mut id_to_key = self.id_to_key.write();
        let mut key_to_id = self.key_to_id.write();

        if let Some(key) = id_to_key.remove(&id) {
            // Remove from key_to_id so search won't return this ID.
            // H1 FIX: Vector remains orphaned in usearch (no deletion support).
            // Track removal count for compaction threshold monitoring.
            key_to_id.remove(&key);
            self.removed_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn search(
        &self,
        query: &[f32],
        k: usize,
        _ef_search: Option<usize>,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        validate_vector(query, self.config.dimension, self.embedder)?;

        let index = self.index.read();
        let key_to_id = self.key_to_id.read();

        if key_to_id.is_empty() {
            return Ok(Vec::new());
        }

        // Compute effective k - can't return more than we have
        // Request more from usearch in case some are removed
        let active_count = key_to_id.len();
        let request_k = if k > active_count {
            // If k > active vectors, we need all of them
            // But usearch might have orphaned keys, so request size()
            index.size().max(k)
        } else {
            // Request k + some buffer for potentially removed entries
            k * 2
        };

        // O(log n) HNSW graph traversal - NOT brute force!
        let results = index
            .search(query, request_k)
            .map_err(|e| IndexError::OperationFailed {
                embedder: self.embedder,
                message: format!("usearch search failed: {}", e),
            })?;

        // Map keys back to UUIDs, filtering removed entries
        let mut output = Vec::with_capacity(k.min(active_count));
        for (key, distance) in results.keys.iter().zip(results.distances.iter()) {
            if let Some(&id) = key_to_id.get(key) {
                output.push((id, *distance));
                if output.len() >= k {
                    break;
                }
            }
        }

        Ok(output)
    }

    fn insert_batch(&self, items: &[(Uuid, Vec<f32>)]) -> IndexResult<usize> {
        // Batch insert - could be optimized with usearch batch API if available
        let mut count = 0;
        for (id, vec) in items {
            self.insert(*id, vec)?;
            count += 1;
        }
        Ok(count)
    }

    fn flush(&self) -> IndexResult<()> {
        // In-memory index - nothing to flush
        Ok(())
    }

    fn memory_bytes(&self) -> usize {
        let index = self.index.read();
        let id_to_key = self.id_to_key.read();
        let key_to_id = self.key_to_id.read();

        // usearch memory + our mapping overhead
        let usearch_memory = index.memory_usage();
        let overhead = std::mem::size_of::<HnswEmbedderIndex>();
        let id_map_bytes = id_to_key.capacity() * (16 + 8); // UUID (16) + u64 (8)
        let key_map_bytes = key_to_id.capacity() * (8 + 16); // u64 (8) + UUID (16)

        usearch_memory + overhead + id_map_bytes + key_map_bytes
    }
}
