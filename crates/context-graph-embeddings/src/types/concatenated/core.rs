//! Core struct for Multi-Array Embedding Storage (Teleological Fingerprints).
//!
//! Contains the `MultiArrayEmbedding` struct and fundamental methods for
//! managing the collection of model embeddings as separate arrays.

use crate::types::dimensions::MODEL_COUNT;
use crate::types::{ModelEmbedding, ModelId};

/// Multi-Array Storage for all 14 embedding models (E1-E13 + Kepler).
///
/// This struct collects individual `ModelEmbedding` outputs and stores them
/// as SEPARATE arrays for multi-array teleological storage. Each embedding
/// maintains its native dimension for per-space indexing and similarity.
///
/// # Architecture
/// - Each embedding is stored in `embeddings[model_id]` as a separate vector
/// - Per-space indexing: Each space has its own HNSW index
/// - Similarity: RRF fusion of per-space scores
/// - The 13-embedding array IS the teleological vector
///
/// # Invariants
/// - `embeddings` is indexed by `ModelId as u8` (0-13)
/// - `is_complete()` returns true only when all 14 slots are filled
/// - `content_hash` is deterministic: same embeddings → same hash
///
/// # Fail-Fast Behavior
/// - `set()` panics if embedding dimension doesn't match projected dimension
/// - No fallbacks or workarounds - errors must be addressed
#[derive(Debug, Clone)]
pub struct MultiArrayEmbedding {
    /// Individual model embeddings indexed by `ModelId as u8`.
    /// Array of 14 slots, each `Option<ModelEmbedding>`.
    /// Each embedding is stored SEPARATELY at its native dimension.
    pub embeddings: [Option<ModelEmbedding>; MODEL_COUNT],

    /// Sum of all individual model latencies in microseconds.
    /// Updated incrementally as embeddings are set.
    pub total_latency_us: u64,

    /// xxHash64 of embeddings for caching and deduplication.
    pub content_hash: u64,
}

impl MultiArrayEmbedding {
    /// Creates a new `MultiArrayEmbedding` with all slots empty.
    ///
    /// # Returns
    /// A new instance with:
    /// - All 14 embedding slots set to `None`
    /// - `total_latency_us = 0`
    /// - `content_hash = 0`
    #[must_use]
    pub fn new() -> Self {
        Self {
            embeddings: std::array::from_fn(|_| None),
            total_latency_us: 0,
            content_hash: 0,
        }
    }

    /// Sets the embedding at the index matching `embedding.model_id`.
    ///
    /// Updates `total_latency_us` by adding the embedding's latency.
    /// If overwriting an existing embedding, the old latency is subtracted first.
    ///
    /// # Arguments
    /// * `embedding` - The model embedding to store. Must have `is_projected = true`
    ///   and vector length matching `model_id.projected_dimension()`.
    ///
    /// # Panics
    /// Panics if `embedding.vector.len() != embedding.model_id.projected_dimension()`.
    /// This is a fail-fast design - incorrect dimensions must be fixed, not worked around.
    pub fn set(&mut self, embedding: ModelEmbedding) {
        let model_id = embedding.model_id;
        let expected_dim = model_id.projected_dimension();
        let actual_dim = embedding.vector.len();

        // Fail-fast: dimension must match projected dimension
        assert!(
            actual_dim == expected_dim,
            "Dimension mismatch for {:?}: expected {}, got {}. \
             Embeddings must be projected to projected_dimension() before storage.",
            model_id,
            expected_dim,
            actual_dim
        );

        let index = model_id as u8 as usize;

        // If overwriting, subtract old latency first
        if let Some(ref old_emb) = self.embeddings[index] {
            self.total_latency_us = self.total_latency_us.saturating_sub(old_emb.latency_us);
        }

        // Add new latency (with saturating add to handle u64::MAX)
        self.total_latency_us = self.total_latency_us.saturating_add(embedding.latency_us);

        self.embeddings[index] = Some(embedding);
    }

    /// Gets the embedding for the specified model, if present.
    ///
    /// # Arguments
    /// * `model_id` - The model to retrieve
    ///
    /// # Returns
    /// - `Some(&ModelEmbedding)` if the model's embedding has been set
    /// - `None` if not yet set
    #[inline]
    #[must_use]
    pub fn get(&self, model_id: ModelId) -> Option<&ModelEmbedding> {
        let index = model_id as u8 as usize;
        self.embeddings[index].as_ref()
    }

    /// Returns `true` only if all 14 slots are filled.
    #[inline]
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.embeddings.iter().all(|e| e.is_some())
    }

    /// Returns the list of `ModelId` variants not yet set.
    ///
    /// Useful for error reporting and timeout handling.
    ///
    /// # Returns
    /// A vector of missing model IDs. Empty if `is_complete()` is true.
    #[must_use]
    pub fn missing_models(&self) -> Vec<ModelId> {
        ModelId::all()
            .iter()
            .copied()
            .filter(|model_id| self.embeddings[*model_id as u8 as usize].is_none())
            .collect()
    }

    /// Returns the count of filled slots (0-14).
    #[inline]
    #[must_use]
    pub fn filled_count(&self) -> usize {
        self.embeddings.iter().filter(|e| e.is_some()).count()
    }
}

impl Default for MultiArrayEmbedding {
    fn default() -> Self {
        Self::new()
    }
}
