//! Trait definition for quantized fingerprint storage.
//!
//! Defines the `QuantizedFingerprintStorage` trait for storing and retrieving
//! quantized fingerprints with all 14 embedders.

use context_graph_embeddings::{
    QuantizationRouter, QuantizedEmbedding, StoredQuantizedFingerprint,
};
use uuid::Uuid;

use super::error::QuantizedStorageResult;

// =============================================================================
// QUANTIZED FINGERPRINT STORAGE TRAIT
// =============================================================================

/// Trait for storing and retrieving quantized fingerprints.
///
/// Implementations MUST:
/// 1. Store all 14 embedders atomically (WriteBatch)
/// 2. Verify all embedders present on load
/// 3. FAIL FAST on any error (no partial writes/reads)
/// 4. Support version checking (panic on mismatch)
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use in async contexts.
pub trait QuantizedFingerprintStorage: Send + Sync {
    /// Store a complete quantized fingerprint with all 14 embedders.
    ///
    /// Uses atomic WriteBatch to ensure all-or-nothing semantics.
    /// Stores each embedder in its dedicated column family (emb_0..emb_13).
    ///
    /// # Arguments
    /// * `fingerprint` - Complete fingerprint with all 13 embedders
    ///
    /// # Errors
    /// - `MissingEmbedder` - Fingerprint doesn't have all 14 embedders
    /// - `SerializationFailed` - bincode serialization error
    /// - `WriteFailed` - RocksDB write error
    ///
    /// # Panics
    /// Panics if fingerprint.embeddings.len() != 14 (FAIL FAST policy).
    fn store_quantized_fingerprint(
        &self,
        fingerprint: &StoredQuantizedFingerprint,
    ) -> QuantizedStorageResult<()>;

    /// Load a complete quantized fingerprint by UUID.
    ///
    /// Reads all 14 embedders from their respective column families.
    /// Verifies version matches current STORAGE_VERSION.
    ///
    /// # Arguments
    /// * `id` - UUID of the fingerprint to load
    ///
    /// # Returns
    /// Complete `StoredQuantizedFingerprint` with all 13 embedders.
    ///
    /// # Errors
    /// - `NotFound` - Fingerprint doesn't exist
    /// - `MissingEmbedder` - Some embedders missing (corrupted data)
    /// - `DeserializationFailed` - bincode deserialization error
    /// - `VersionMismatch` - Stored version != STORAGE_VERSION
    fn load_quantized_fingerprint(
        &self,
        id: Uuid,
    ) -> QuantizedStorageResult<StoredQuantizedFingerprint>;

    /// Load a single embedder's quantized embedding.
    ///
    /// Useful for lazy loading when only specific embedders are needed.
    ///
    /// # Arguments
    /// * `fingerprint_id` - UUID of the fingerprint
    /// * `embedder_idx` - Embedder index (0-13)
    ///
    /// # Returns
    /// The `QuantizedEmbedding` for the specified embedder.
    ///
    /// # Errors
    /// - `NotFound` - Embedding not found for this fingerprint/embedder
    /// - `DeserializationFailed` - bincode deserialization error
    ///
    /// # Panics
    /// Panics if embedder_idx >= 14 (FAIL FAST policy).
    fn load_embedder(
        &self,
        fingerprint_id: Uuid,
        embedder_idx: u8,
    ) -> QuantizedStorageResult<QuantizedEmbedding>;

    /// Delete a quantized fingerprint and all its embedders.
    ///
    /// Uses atomic WriteBatch to delete all 13 embedder entries.
    ///
    /// # Arguments
    /// * `id` - UUID of the fingerprint to delete
    ///
    /// # Errors
    /// - `WriteFailed` - RocksDB delete error
    fn delete_quantized_fingerprint(&self, id: Uuid) -> QuantizedStorageResult<()>;

    /// Check if a quantized fingerprint exists.
    ///
    /// Checks emb_0 column family only (optimization - if emb_0 exists, all should).
    ///
    /// # Arguments
    /// * `id` - UUID of the fingerprint to check
    ///
    /// # Returns
    /// `true` if the fingerprint exists, `false` otherwise.
    fn exists_quantized_fingerprint(&self, id: Uuid) -> QuantizedStorageResult<bool>;

    /// Get the quantization router for encode/decode operations.
    ///
    /// Returns a reference to the router for callers who need to
    /// quantize/dequantize embeddings outside of storage operations.
    fn quantization_router(&self) -> &QuantizationRouter;
}
