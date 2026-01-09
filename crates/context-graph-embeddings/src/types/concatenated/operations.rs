//! Operations for MultiArrayEmbedding: validation, hashing, and per-space access.
//!
//! Contains methods that operate on the collected embeddings for validation
//! and content hashing.

use crate::error::EmbeddingResult;
use crate::types::dimensions::MODEL_COUNT;
use crate::types::{ModelEmbedding, ModelId};
use xxhash_rust::xxh64::xxh64;

use super::MultiArrayEmbedding;

impl MultiArrayEmbedding {
    /// Computes and stores the content hash from all embeddings.
    ///
    /// After this call, `self.content_hash` contains xxHash64 of all embedding data.
    /// This hash is deterministic: same embeddings → same hash.
    ///
    /// # Panics
    /// Panics if `is_complete() == false`. All 13 models must be present.
    pub fn compute_hash(&mut self) {
        assert!(
            self.is_complete(),
            "Cannot compute hash: {} of {} models missing. Missing: {:?}",
            MODEL_COUNT - self.filled_count(),
            MODEL_COUNT,
            self.missing_models()
        );

        // Hash all embeddings in order
        let mut all_bytes: Vec<u8> = Vec::new();
        for model_id in ModelId::all() {
            let embedding = self.embeddings[*model_id as u8 as usize]
                .as_ref()
                .expect("Embedding should exist after is_complete() check");

            // Convert f32 slice to bytes
            let bytes: &[u8] = unsafe {
                std::slice::from_raw_parts(
                    embedding.vector.as_ptr() as *const u8,
                    std::mem::size_of_val(embedding.vector.as_slice()),
                )
            };
            all_bytes.extend_from_slice(bytes);
        }

        self.content_hash = xxh64(&all_bytes, 0);
    }

    /// Returns the total dimension across all embeddings.
    ///
    /// Returns the sum of all filled embedding dimensions.
    #[must_use]
    pub fn total_dimension(&self) -> usize {
        self.embeddings
            .iter()
            .filter_map(|e| e.as_ref())
            .map(|e| e.vector.len())
            .sum()
    }

    /// Validates all embeddings against their model requirements.
    ///
    /// Calls `validate()` on each present embedding.
    ///
    /// # Errors
    /// Returns the first validation error encountered.
    /// Does not validate missing embeddings (use `is_complete()` for that).
    pub fn validate(&self) -> EmbeddingResult<()> {
        for embedding in self.embeddings.iter().flatten() {
            embedding.validate()?;
        }
        Ok(())
    }

    /// Returns the embedding vector for a specific model.
    ///
    /// # Arguments
    /// * `model_id` - The model whose embedding to retrieve
    ///
    /// # Returns
    /// - `Some(&[f32])` if the embedding exists
    /// - `None` if not yet set
    #[must_use]
    pub fn get_vector(&self, model_id: ModelId) -> Option<&[f32]> {
        self.get(model_id).map(|e| e.vector.as_slice())
    }

    /// Returns an iterator over all present embeddings with their model IDs.
    pub fn iter(&self) -> impl Iterator<Item = (ModelId, &ModelEmbedding)> {
        ModelId::all()
            .iter()
            .filter_map(|&model_id| self.get(model_id).map(|emb| (model_id, emb)))
    }
}
