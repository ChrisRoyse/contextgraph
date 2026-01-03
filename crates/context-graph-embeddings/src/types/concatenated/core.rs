//! Core struct and basic operations for ConcatenatedEmbedding.
//!
//! Contains the `ConcatenatedEmbedding` struct definition and fundamental
//! methods for managing the collection of model embeddings.

use crate::types::dimensions::MODEL_COUNT;
use crate::types::{ModelEmbedding, ModelId};

/// Aggregates outputs from all 12 embedding models.
///
/// This struct collects individual `ModelEmbedding` outputs and concatenates
/// them into a single 8320D vector for FuseMoE input.
///
/// # Invariants
/// - `embeddings` is indexed by `ModelId as u8` (0-11)
/// - `is_complete()` returns true only when all 12 slots are filled
/// - `concatenated` vector is built in model order (E1-E12)
/// - `content_hash` is deterministic: same embeddings → same hash
///
/// # Fail-Fast Behavior
/// - `set()` panics if embedding dimension doesn't match projected dimension
/// - `concatenate()` panics if not all 12 slots are filled
/// - No fallbacks or workarounds - errors must be addressed
#[derive(Debug, Clone)]
pub struct ConcatenatedEmbedding {
    /// Individual model embeddings indexed by `ModelId as u8`.
    /// Array of 12 slots, each `Option<ModelEmbedding>`.
    pub embeddings: [Option<ModelEmbedding>; MODEL_COUNT],

    /// The concatenated 8320D vector (built by `concatenate()`).
    /// Empty until `concatenate()` is called.
    pub concatenated: Vec<f32>,

    /// Sum of all individual model latencies in microseconds.
    /// Updated incrementally as embeddings are set.
    pub total_latency_us: u64,

    /// xxHash64 of concatenated vector bytes for caching.
    /// Zero until `concatenate()` is called.
    pub content_hash: u64,
}

impl ConcatenatedEmbedding {
    /// Creates a new `ConcatenatedEmbedding` with all slots empty.
    ///
    /// # Returns
    /// A new instance with:
    /// - All 12 embedding slots set to `None`
    /// - Empty `concatenated` vector
    /// - `total_latency_us = 0`
    /// - `content_hash = 0`
    #[must_use]
    pub fn new() -> Self {
        Self {
            embeddings: std::array::from_fn(|_| None),
            concatenated: Vec::new(),
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
    ///
    /// # Example
    /// ```
    /// use context_graph_embeddings::types::{ConcatenatedEmbedding, ModelEmbedding, ModelId};
    ///
    /// let mut concat = ConcatenatedEmbedding::new();
    /// let model_id = ModelId::Semantic;
    /// let dim = model_id.projected_dimension();
    /// let mut emb = ModelEmbedding::new(model_id, vec![0.1; dim], 1000);
    /// emb.set_projected(true);
    /// concat.set(emb);
    /// assert_eq!(concat.filled_count(), 1);
    /// ```
    pub fn set(&mut self, embedding: ModelEmbedding) {
        let model_id = embedding.model_id;
        let expected_dim = model_id.projected_dimension();
        let actual_dim = embedding.vector.len();

        // Fail-fast: dimension must match projected dimension
        assert!(
            actual_dim == expected_dim,
            "Dimension mismatch for {:?}: expected {}, got {}. \
             Embeddings must be projected to projected_dimension() before concatenation.",
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

    /// Returns `true` only if all 12 slots are filled.
    ///
    /// This is a prerequisite for calling `concatenate()`.
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

    /// Returns the count of filled slots (0-12).
    #[inline]
    #[must_use]
    pub fn filled_count(&self) -> usize {
        self.embeddings.iter().filter(|e| e.is_some()).count()
    }
}

impl Default for ConcatenatedEmbedding {
    fn default() -> Self {
        Self::new()
    }
}
