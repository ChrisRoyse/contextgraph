//! Stored quantized fingerprint type for primary storage.

use crate::quantization::{QuantizationMethod, QuantizedEmbedding};
use crate::types::ModelId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::constants::{NUM_EMBEDDERS, STORAGE_VERSION};

/// Complete stored fingerprint with quantized embeddings.
///
/// This struct is used for STORAGE in layer1_primary (RocksDB/ScyllaDB).
/// The actual 13x HNSW indexes (layer2c) use `IndexEntry` for dequantized vectors.
///
/// # Storage Layout
/// Each embedder's quantized embedding is stored separately for:
/// 1. Per-embedder HNSW indexing (requires dequantization)
/// 2. Lazy loading (only fetch needed embedders)
/// 3. Independent quantization per embedder
///
/// # Size Target
/// ~17KB per fingerprint (Constitution requirement)
///
/// # Difference from TeleologicalFingerprint
/// - `TeleologicalFingerprint` (in context-graph-core): ~46KB UNQUANTIZED, includes:
///   - `SemanticFingerprint` with raw f32 arrays
///   - Metadata (timestamps, access_count)
///
/// - `StoredQuantizedFingerprint` (this type): ~17KB QUANTIZED for storage
///   - Uses `QuantizedEmbedding` (compressed bytes)
///   - Temporal history kept in TimescaleDB temporal store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredQuantizedFingerprint {
    /// UUID of the fingerprint (primary key).
    pub id: Uuid,

    /// Storage format version (for future migration detection).
    pub version: u8,

    /// Per-embedder quantized embeddings.
    /// Key: embedder index (0-12)
    /// Value: Quantized embedding with method-specific metadata
    ///
    /// # Invariant
    /// All 13 embedders MUST be present. Missing embedder = panic on load.
    pub embeddings: HashMap<u8, QuantizedEmbedding>,

    /// 13D topic profile (NOT quantized - only 52 bytes).
    /// Each dimension = alignment of that embedder's output to emergent topics.
    /// Represents topic alignment signature across all 13 embedding spaces.
    pub topic_profile: [f32; 14],

    /// SHA-256 content hash (32 bytes).
    /// Used for deduplication and integrity verification.
    pub content_hash: [u8; 32],

    /// Creation timestamp (Unix millis since epoch).
    pub created_at_ms: i64,

    /// Last update timestamp (Unix millis since epoch).
    pub last_updated_ms: i64,

    /// Access count for LRU/importance scoring.
    pub access_count: u64,

    /// Soft-delete flag.
    /// True = marked for deletion but recoverable (30-day window per Constitution).
    pub deleted: bool,
}

impl StoredQuantizedFingerprint {
    /// Create a new StoredQuantizedFingerprint.
    ///
    /// # Arguments
    /// * `id` - UUID for this fingerprint
    /// * `embeddings` - HashMap of quantized embeddings (must have all 13)
    /// * `topic_profile` - 13D topic alignment signature
    /// * `content_hash` - SHA-256 of source content
    ///
    /// # Panics
    /// Panics if `embeddings` doesn't contain exactly 13 entries.
    #[must_use]
    pub fn new(
        id: Uuid,
        embeddings: HashMap<u8, QuantizedEmbedding>,
        topic_profile: [f32; 14],
        content_hash: [u8; 32],
    ) -> Self {
        // FAIL FAST: All 13 embedders required
        if embeddings.len() != NUM_EMBEDDERS {
            panic!(
                "CONSTRUCTION ERROR: StoredQuantizedFingerprint requires exactly {} embeddings, got {}. \
                 Missing embedder indices: {:?}. \
                 This indicates incomplete fingerprint generation.",
                NUM_EMBEDDERS,
                embeddings.len(),
                (0..13).filter(|i| !embeddings.contains_key(&(*i as u8))).collect::<Vec<_>>()
            );
        }

        // Verify all indices are valid (0-12)
        for idx in embeddings.keys() {
            if *idx >= NUM_EMBEDDERS as u8 {
                panic!(
                    "CONSTRUCTION ERROR: Invalid embedder index {}. Valid range: 0-12. \
                     This indicates embedding pipeline bug.",
                    idx
                );
            }
        }

        let now = chrono::Utc::now().timestamp_millis();

        Self {
            id,
            version: STORAGE_VERSION,
            embeddings,
            topic_profile,
            content_hash,
            created_at_ms: now,
            last_updated_ms: now,
            access_count: 0,
            deleted: false,
        }
    }

    /// Compute total storage size in bytes (serialized).
    ///
    /// # Returns
    /// Estimated serialized size. Actual size may vary slightly due to encoding.
    #[must_use]
    pub fn estimated_size_bytes(&self) -> usize {
        let mut size = 0usize;

        // Fixed fields
        size += 16; // id (UUID)
        size += 1; // version
        size += 52; // topic_profile (13 × 4 bytes)
        size += 32; // content_hash
        size += 8; // created_at_ms
        size += 8; // last_updated_ms
        size += 8; // access_count
        size += 1; // deleted

        // Variable fields: embeddings
        for qe in self.embeddings.values() {
            size += 1; // method (enum variant)
            size += 8; // original_dim
            size += qe.data.len(); // compressed data
            size += 32; // metadata (approximate)
        }

        size
    }

    /// Get quantized embedding for a specific embedder.
    ///
    /// # Arguments
    /// * `embedder_idx` - Embedder index (0-12)
    ///
    /// # Panics
    /// Panics if embedder_idx is out of range or embedding is missing.
    #[must_use]
    pub fn get_embedding(&self, embedder_idx: u8) -> &QuantizedEmbedding {
        self.embeddings.get(&embedder_idx).unwrap_or_else(|| {
            panic!(
                "STORAGE ERROR: Missing embedding for embedder {}. \
                 Fingerprint ID: {}. Available embedders: {:?}. \
                 This indicates corrupted fingerprint or storage bug.",
                embedder_idx,
                self.id,
                self.embeddings.keys().collect::<Vec<_>>()
            );
        })
    }

    /// Check if all embeddings use correct quantization methods.
    ///
    /// # Returns
    /// `true` if all embeddings match their Constitution-assigned methods.
    #[must_use]
    pub fn validate_quantization_methods(&self) -> bool {
        for (idx, qe) in &self.embeddings {
            if let Ok(model_id) = ModelId::try_from(*idx) {
                let expected = QuantizationMethod::for_model_id(model_id);
                if qe.method != expected {
                    return false;
                }
            }
        }
        true
    }
}
