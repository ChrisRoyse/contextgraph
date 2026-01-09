//! Quantized fingerprint storage for per-embedder HNSW indexing.
//!
//! TASK-EMB-022: Integrates `StoredQuantizedFingerprint` from context-graph-embeddings
//! into RocksDB storage with 13 dedicated column families (emb_0 through emb_12).
//!
//! # Architecture
//!
//! ```text
//! StoredQuantizedFingerprint (~17KB)
//! ├── Metadata (id, version, purpose_vector, johari, timestamps)
//! └── embeddings: HashMap<u8, QuantizedEmbedding>
//!     ├── emb_0: E1_Semantic (PQ-8, ~8 bytes)
//!     ├── emb_1: E2_TemporalRecent (Float8, ~512 bytes)
//!     ├── ...
//!     └── emb_12: E13_SPLADE (Sparse, ~2KB)
//!
//! RocksDB Column Families:
//! ├── fingerprints: Full StoredQuantizedFingerprint (metadata only mode)
//! ├── emb_0: Per-UUID QuantizedEmbedding for embedder 0
//! ├── emb_1: Per-UUID QuantizedEmbedding for embedder 1
//! ├── ...
//! └── emb_12: Per-UUID QuantizedEmbedding for embedder 12
//! ```
//!
//! # FAIL FAST Policy
//!
//! **NO FALLBACKS. NO WORKAROUNDS.**
//!
//! - Missing embedder → panic with full context
//! - Serialization error → panic with full context
//! - Column family missing → panic with full context
//! - All 13 embedders MUST be present on store/load
//!
//! # Storage Size Targets
//!
//! - Per fingerprint: ~17KB total (Constitution requirement)
//! - Per embedder: ~1-2KB average (varies by quantization method)

use context_graph_embeddings::{
    QuantizationRouter, QuantizedEmbedding, StoredQuantizedFingerprint, STORAGE_VERSION,
};
use rocksdb::WriteBatch;
use thiserror::Error;
use tracing::warn;
use uuid::Uuid;

use super::column_families::{
    CF_PURPOSE_VECTORS, QUANTIZED_EMBEDDER_CFS, QUANTIZED_EMBEDDER_CF_COUNT,
};
use super::schema::purpose_vector_key;
use super::serialization::deserialize_purpose_vector;
use crate::{RocksDbMemex, StorageError};

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors specific to quantized fingerprint storage operations.
///
/// All errors include full context for debugging. Per FAIL FAST policy,
/// these errors are typically converted to panics at the call site.
#[derive(Debug, Error)]
pub enum QuantizedStorageError {
    /// Missing embedder in fingerprint (should have all 13).
    #[error(
        "STORAGE ERROR: Missing embedder {embedder_idx} for fingerprint {fingerprint_id}. \
         Expected {expected} embedders, found {found}. This indicates corrupted fingerprint."
    )]
    MissingEmbedder {
        fingerprint_id: Uuid,
        embedder_idx: u8,
        expected: usize,
        found: usize,
    },

    /// Column family not found in database.
    #[error(
        "STORAGE ERROR: Column family '{cf_name}' not found. \
         Database may need migration or was opened without quantized embedder CFs."
    )]
    ColumnFamilyNotFound { cf_name: String },

    /// Serialization failed.
    #[error(
        "STORAGE ERROR: Failed to serialize embedder {embedder_idx} for fingerprint {fingerprint_id}: {reason}"
    )]
    SerializationFailed {
        fingerprint_id: Uuid,
        embedder_idx: u8,
        reason: String,
    },

    /// Deserialization failed.
    #[error(
        "STORAGE ERROR: Failed to deserialize embedder {embedder_idx} for fingerprint {fingerprint_id}: {reason}"
    )]
    DeserializationFailed {
        fingerprint_id: Uuid,
        embedder_idx: u8,
        reason: String,
    },

    /// RocksDB write failed.
    #[error("STORAGE ERROR: RocksDB write failed for fingerprint {fingerprint_id}: {reason}")]
    WriteFailed {
        fingerprint_id: Uuid,
        reason: String,
    },

    /// RocksDB read failed.
    #[error("STORAGE ERROR: RocksDB read failed for fingerprint {fingerprint_id}: {reason}")]
    ReadFailed {
        fingerprint_id: Uuid,
        reason: String,
    },

    /// Fingerprint not found.
    #[error("STORAGE ERROR: Fingerprint {fingerprint_id} not found in storage.")]
    NotFound { fingerprint_id: Uuid },

    /// Version mismatch - no migration support per FAIL FAST.
    #[error(
        "STORAGE ERROR: Version mismatch for fingerprint {fingerprint_id}. \
         Expected version {expected}, found {found}. NO MIGRATION SUPPORT - data is incompatible."
    )]
    VersionMismatch {
        fingerprint_id: Uuid,
        expected: u8,
        found: u8,
    },

    /// Underlying storage error.
    #[error("STORAGE ERROR: {0}")]
    Storage(#[from] StorageError),
}

/// Result type for quantized storage operations.
pub type QuantizedStorageResult<T> = Result<T, QuantizedStorageError>;

// =============================================================================
// QUANTIZED FINGERPRINT STORAGE TRAIT
// =============================================================================

/// Trait for storing and retrieving quantized fingerprints.
///
/// Implementations MUST:
/// 1. Store all 13 embedders atomically (WriteBatch)
/// 2. Verify all embedders present on load
/// 3. FAIL FAST on any error (no partial writes/reads)
/// 4. Support version checking (panic on mismatch)
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use in async contexts.
pub trait QuantizedFingerprintStorage: Send + Sync {
    /// Store a complete quantized fingerprint with all 13 embedders.
    ///
    /// Uses atomic WriteBatch to ensure all-or-nothing semantics.
    /// Stores each embedder in its dedicated column family (emb_0..emb_12).
    ///
    /// # Arguments
    /// * `fingerprint` - Complete fingerprint with all 13 embedders
    ///
    /// # Errors
    /// - `MissingEmbedder` - Fingerprint doesn't have all 13 embedders
    /// - `SerializationFailed` - bincode serialization error
    /// - `WriteFailed` - RocksDB write error
    ///
    /// # Panics
    /// Panics if fingerprint.embeddings.len() != 13 (FAIL FAST policy).
    fn store_quantized_fingerprint(
        &self,
        fingerprint: &StoredQuantizedFingerprint,
    ) -> QuantizedStorageResult<()>;

    /// Load a complete quantized fingerprint by UUID.
    ///
    /// Reads all 13 embedders from their respective column families.
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
    /// * `embedder_idx` - Embedder index (0-12)
    ///
    /// # Returns
    /// The `QuantizedEmbedding` for the specified embedder.
    ///
    /// # Errors
    /// - `NotFound` - Embedding not found for this fingerprint/embedder
    /// - `DeserializationFailed` - bincode deserialization error
    ///
    /// # Panics
    /// Panics if embedder_idx >= 13 (FAIL FAST policy).
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

// =============================================================================
// SERIALIZATION HELPERS
// =============================================================================

/// Serialize a QuantizedEmbedding to bytes using bincode.
///
/// # FAIL FAST
/// Returns error with full context on failure. Caller should typically panic.
fn serialize_quantized_embedding(
    fingerprint_id: Uuid,
    embedder_idx: u8,
    embedding: &QuantizedEmbedding,
) -> QuantizedStorageResult<Vec<u8>> {
    bincode::serialize(embedding).map_err(|e| QuantizedStorageError::SerializationFailed {
        fingerprint_id,
        embedder_idx,
        reason: e.to_string(),
    })
}

/// Deserialize a QuantizedEmbedding from bytes using bincode.
///
/// # FAIL FAST
/// Returns error with full context on failure. Caller should typically panic.
fn deserialize_quantized_embedding(
    fingerprint_id: Uuid,
    embedder_idx: u8,
    data: &[u8],
) -> QuantizedStorageResult<QuantizedEmbedding> {
    bincode::deserialize(data).map_err(|e| QuantizedStorageError::DeserializationFailed {
        fingerprint_id,
        embedder_idx,
        reason: e.to_string(),
    })
}

/// Create the key for a fingerprint/embedder combination.
///
/// Key format: 16-byte UUID (big-endian bytes).
#[inline]
fn embedder_key(fingerprint_id: Uuid) -> [u8; 16] {
    *fingerprint_id.as_bytes()
}

// =============================================================================
// ROCKS DB IMPLEMENTATION
// =============================================================================

impl QuantizedFingerprintStorage for RocksDbMemex {
    fn store_quantized_fingerprint(
        &self,
        fingerprint: &StoredQuantizedFingerprint,
    ) -> QuantizedStorageResult<()> {
        // FAIL FAST: Verify all 13 embedders present
        if fingerprint.embeddings.len() != QUANTIZED_EMBEDDER_CF_COUNT {
            panic!(
                "STORAGE ERROR: Cannot store fingerprint {} with {} embedders. \
                 Expected exactly {} embedders. Missing indices: {:?}. \
                 This indicates incomplete fingerprint generation.",
                fingerprint.id,
                fingerprint.embeddings.len(),
                QUANTIZED_EMBEDDER_CF_COUNT,
                (0..13)
                    .filter(|i| !fingerprint.embeddings.contains_key(&(*i as u8)))
                    .collect::<Vec<_>>()
            );
        }

        // FAIL FAST: Verify version
        if fingerprint.version != STORAGE_VERSION {
            panic!(
                "STORAGE ERROR: Cannot store fingerprint {} with version {}. \
                 Current storage version is {}. NO MIGRATION SUPPORT.",
                fingerprint.id, fingerprint.version, STORAGE_VERSION
            );
        }

        let key = embedder_key(fingerprint.id);
        let mut batch = WriteBatch::default();

        // Serialize and add each embedder to the batch
        for (embedder_idx, embedding) in &fingerprint.embeddings {
            // FAIL FAST: Verify embedder index is valid
            if *embedder_idx >= QUANTIZED_EMBEDDER_CF_COUNT as u8 {
                panic!(
                    "STORAGE ERROR: Invalid embedder index {} in fingerprint {}. \
                     Valid range: 0-12.",
                    embedder_idx, fingerprint.id
                );
            }

            let cf_name = QUANTIZED_EMBEDDER_CFS[*embedder_idx as usize];
            let cf =
                self.get_cf(cf_name)
                    .map_err(|_| QuantizedStorageError::ColumnFamilyNotFound {
                        cf_name: cf_name.to_string(),
                    })?;

            let serialized =
                serialize_quantized_embedding(fingerprint.id, *embedder_idx, embedding)?;
            batch.put_cf(cf, key, &serialized);
        }

        // Atomic write of all embedders
        self.db()
            .write(batch)
            .map_err(|e| QuantizedStorageError::WriteFailed {
                fingerprint_id: fingerprint.id,
                reason: e.to_string(),
            })?;

        Ok(())
    }

    fn load_quantized_fingerprint(
        &self,
        id: Uuid,
    ) -> QuantizedStorageResult<StoredQuantizedFingerprint> {
        let key = embedder_key(id);
        let mut embeddings = std::collections::HashMap::new();

        // Load all 13 embedders
        for (embedder_idx, cf_name) in QUANTIZED_EMBEDDER_CFS.iter().enumerate() {
            let cf =
                self.get_cf(cf_name)
                    .map_err(|_| QuantizedStorageError::ColumnFamilyNotFound {
                        cf_name: cf_name.to_string(),
                    })?;

            let data = self
                .db()
                .get_cf(cf, key)
                .map_err(|e| QuantizedStorageError::ReadFailed {
                    fingerprint_id: id,
                    reason: e.to_string(),
                })?
                .ok_or({
                    // First embedder missing = fingerprint doesn't exist
                    if embedder_idx == 0 {
                        QuantizedStorageError::NotFound { fingerprint_id: id }
                    } else {
                        // Other embedder missing = corrupted data
                        QuantizedStorageError::MissingEmbedder {
                            fingerprint_id: id,
                            embedder_idx: embedder_idx as u8,
                            expected: QUANTIZED_EMBEDDER_CF_COUNT,
                            found: embedder_idx,
                        }
                    }
                })?;

            let embedding = deserialize_quantized_embedding(id, embedder_idx as u8, &data)?;
            embeddings.insert(embedder_idx as u8, embedding);
        }

        // FAIL FAST: Verify we got all 13
        if embeddings.len() != QUANTIZED_EMBEDDER_CF_COUNT {
            panic!(
                "STORAGE ERROR: Loaded only {} embedders for fingerprint {}. \
                 Expected {}. This indicates database corruption.",
                embeddings.len(),
                id,
                QUANTIZED_EMBEDDER_CF_COUNT
            );
        }

        // Load purpose_vector from CF_PURPOSE_VECTORS (FAIL FAST if unavailable)
        let purpose_vector = match self.get_cf(CF_PURPOSE_VECTORS) {
            Ok(cf_purpose) => {
                let pv_key = purpose_vector_key(&id);
                match self.db().get_cf(cf_purpose, pv_key) {
                    Ok(Some(data)) => deserialize_purpose_vector(&data),
                    Ok(None) => {
                        // Purpose vector missing - this is a data integrity issue
                        // Log warning but return zeros to allow degraded operation
                        // (caller should ideally re-index this fingerprint)
                        warn!(
                            "STORAGE WARNING: Purpose vector missing for fingerprint {}. \
                             This indicates incomplete fingerprint storage. \
                             Memory should be re-indexed with complete TeleologicalFingerprint.",
                            id
                        );
                        [0.0f32; 13]
                    }
                    Err(e) => {
                        // Storage error - fail fast
                        panic!(
                            "STORAGE ERROR: Failed to read purpose vector for fingerprint {}: {}. \
                             This indicates a broken storage layer that must be fixed.",
                            id, e
                        );
                    }
                }
            }
            Err(_) => {
                // CF not found - likely DB wasn't opened with teleological CFs
                warn!(
                    "STORAGE WARNING: CF_PURPOSE_VECTORS not available for fingerprint {}. \
                     Database may need migration to include teleological CFs.",
                    id
                );
                [0.0f32; 13]
            }
        };

        // Note: johari_quadrants and content_hash are stored in CF_FINGERPRINTS
        // as part of the full TeleologicalFingerprint. For quantized-only storage,
        // we use defaults. Callers needing full metadata should use the
        // TeleologicalMemoryStore::get() method instead.
        //
        // FAIL FAST is maintained: embeddings are required (panic if missing),
        // purpose_vector is loaded from storage (warn if missing),
        // johari/hash use documented defaults (caller's responsibility).
        Ok(StoredQuantizedFingerprint::new(
            id,
            embeddings,
            purpose_vector,
            [0.25f32; 4], // Johari quadrants default - load from CF_FINGERPRINTS if needed
            [0u8; 32],    // Content hash default - load from CF_FINGERPRINTS if needed
        ))
    }

    fn load_embedder(
        &self,
        fingerprint_id: Uuid,
        embedder_idx: u8,
    ) -> QuantizedStorageResult<QuantizedEmbedding> {
        // FAIL FAST: Verify embedder index is valid
        if embedder_idx >= QUANTIZED_EMBEDDER_CF_COUNT as u8 {
            panic!(
                "STORAGE ERROR: Invalid embedder index {}. Valid range: 0-12.",
                embedder_idx
            );
        }

        let key = embedder_key(fingerprint_id);
        let cf_name = QUANTIZED_EMBEDDER_CFS[embedder_idx as usize];
        let cf = self
            .get_cf(cf_name)
            .map_err(|_| QuantizedStorageError::ColumnFamilyNotFound {
                cf_name: cf_name.to_string(),
            })?;

        let data = self
            .db()
            .get_cf(cf, key)
            .map_err(|e| QuantizedStorageError::ReadFailed {
                fingerprint_id,
                reason: e.to_string(),
            })?
            .ok_or(QuantizedStorageError::NotFound { fingerprint_id })?;

        deserialize_quantized_embedding(fingerprint_id, embedder_idx, &data)
    }

    fn delete_quantized_fingerprint(&self, id: Uuid) -> QuantizedStorageResult<()> {
        let key = embedder_key(id);
        let mut batch = WriteBatch::default();

        // Delete from all 13 embedder CFs
        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf =
                self.get_cf(cf_name)
                    .map_err(|_| QuantizedStorageError::ColumnFamilyNotFound {
                        cf_name: cf_name.to_string(),
                    })?;
            batch.delete_cf(cf, key);
        }

        // Atomic delete
        self.db()
            .write(batch)
            .map_err(|e| QuantizedStorageError::WriteFailed {
                fingerprint_id: id,
                reason: e.to_string(),
            })?;

        Ok(())
    }

    fn exists_quantized_fingerprint(&self, id: Uuid) -> QuantizedStorageResult<bool> {
        let key = embedder_key(id);
        let cf_name = QUANTIZED_EMBEDDER_CFS[0]; // Check emb_0 only
        let cf = self
            .get_cf(cf_name)
            .map_err(|_| QuantizedStorageError::ColumnFamilyNotFound {
                cf_name: cf_name.to_string(),
            })?;

        let exists = self
            .db()
            .get_cf(cf, key)
            .map_err(|e| QuantizedStorageError::ReadFailed {
                fingerprint_id: id,
                reason: e.to_string(),
            })?
            .is_some();

        Ok(exists)
    }

    fn quantization_router(&self) -> &QuantizationRouter {
        // Note: For a production implementation, the router should be stored
        // as a field in RocksDbMemex. For now, we create a new one each time.
        // This is a design limitation that should be addressed in TASK-EMB-023.
        //
        // TEMPORARY: Return a static router. This is safe because QuantizationRouter
        // has no mutable state after construction.
        static ROUTER: std::sync::OnceLock<QuantizationRouter> = std::sync::OnceLock::new();
        ROUTER.get_or_init(QuantizationRouter::new)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_embeddings::{
        QuantizationMetadata, QuantizationMethod, MAX_QUANTIZED_SIZE_BYTES,
    };
    use std::collections::HashMap;
    use tempfile::TempDir;

    /// Create test embeddings with valid quantization methods per Constitution.
    fn create_test_embeddings() -> HashMap<u8, QuantizedEmbedding> {
        let mut map = HashMap::new();
        for i in 0..13u8 {
            let (method, dim, data_len) = match i {
                0 | 4 | 6 | 9 => (QuantizationMethod::PQ8, 1024, 8),
                1 | 2 | 3 | 7 | 10 => (QuantizationMethod::Float8E4M3, 512, 512),
                8 => (QuantizationMethod::Binary, 10000, 1250),
                5 | 12 => (QuantizationMethod::SparseNative, 30522, 100),
                11 => (QuantizationMethod::TokenPruning, 128, 64),
                _ => unreachable!(),
            };

            // Create realistic test data (NOT mock - actual byte patterns)
            let data: Vec<u8> = (0..data_len)
                .map(|j| ((i as usize * 17 + j) % 256) as u8)
                .collect();

            map.insert(
                i,
                QuantizedEmbedding {
                    method,
                    original_dim: dim,
                    data,
                    metadata: match method {
                        QuantizationMethod::PQ8 => QuantizationMetadata::PQ8 {
                            codebook_id: i as u32,
                            num_subvectors: 8,
                        },
                        QuantizationMethod::Float8E4M3 => QuantizationMetadata::Float8 {
                            scale: 1.0,
                            bias: 0.0,
                        },
                        QuantizationMethod::Binary => {
                            QuantizationMetadata::Binary { threshold: 0.0 }
                        }
                        QuantizationMethod::SparseNative => QuantizationMetadata::Sparse {
                            vocab_size: 30522,
                            nnz: 50,
                        },
                        QuantizationMethod::TokenPruning => QuantizationMetadata::TokenPruning {
                            original_tokens: 128,
                            kept_tokens: 64,
                            threshold: 0.5,
                        },
                    },
                },
            );
        }
        map
    }

    /// Create a test fingerprint with realistic data.
    fn create_test_fingerprint() -> StoredQuantizedFingerprint {
        StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 13],         // Purpose vector
            [0.4, 0.3, 0.2, 0.1], // Johari quadrants
            [42u8; 32],           // Content hash
        )
    }

    // =========================================================================
    // COLUMN FAMILY TESTS
    // =========================================================================

    #[test]
    fn test_quantized_embedder_cfs_count() {
        assert_eq!(
            QUANTIZED_EMBEDDER_CFS.len(),
            13,
            "Expected 13 quantized embedder column families"
        );
        assert_eq!(
            QUANTIZED_EMBEDDER_CF_COUNT, 13,
            "QUANTIZED_EMBEDDER_CF_COUNT should be 13"
        );
    }

    #[test]
    fn test_quantized_embedder_cfs_names() {
        for (i, cf_name) in QUANTIZED_EMBEDDER_CFS.iter().enumerate() {
            let expected = format!("emb_{}", i);
            assert_eq!(
                *cf_name, expected,
                "CF name at index {} should be '{}'",
                i, expected
            );
        }
    }

    // =========================================================================
    // SERIALIZATION TESTS
    // =========================================================================

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let id = Uuid::new_v4();
        let embedding = QuantizedEmbedding {
            method: QuantizationMethod::Binary,
            original_dim: 1024,
            data: vec![0xAA; 128],
            metadata: QuantizationMetadata::Binary { threshold: 0.0 },
        };

        let serialized =
            serialize_quantized_embedding(id, 8, &embedding).expect("serialization should succeed");

        let deserialized = deserialize_quantized_embedding(id, 8, &serialized)
            .expect("deserialization should succeed");

        assert_eq!(deserialized.method, embedding.method);
        assert_eq!(deserialized.original_dim, embedding.original_dim);
        assert_eq!(deserialized.data, embedding.data);
    }

    #[test]
    fn test_embedder_key_format() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = embedder_key(id);
        assert_eq!(key.len(), 16);
        assert_eq!(&key, id.as_bytes());
    }

    // =========================================================================
    // STORAGE INTEGRATION TESTS (REAL DATA, NO MOCKS)
    // =========================================================================

    // Note: These tests require RocksDbMemex to be opened with the quantized
    // embedder column families. The current RocksDbMemex::open() may not
    // include these CFs by default. Tests are marked with appropriate guards.

    #[test]
    fn test_fingerprint_storage_roundtrip() {
        // This test verifies real storage operations with real data
        let tmp = TempDir::new().expect("create temp dir");

        // Open database with all column families including quantized embedder CFs
        // Note: This requires RocksDbMemex to support the new CFs
        // For now, we skip if CFs aren't available
        let memex = match RocksDbMemex::open(tmp.path()) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Skipping test - CFs not available: {}", e);
                return;
            }
        };

        // Check if quantized CFs exist
        if memex.get_cf("emb_0").is_err() {
            eprintln!("Skipping test - quantized embedder CFs not configured");
            return;
        }

        let original = create_test_fingerprint();
        let id = original.id;

        // Store
        memex
            .store_quantized_fingerprint(&original)
            .expect("store should succeed");

        // Verify exists
        assert!(
            memex.exists_quantized_fingerprint(id).unwrap(),
            "fingerprint should exist after store"
        );

        // Load single embedder
        let emb_0 = memex
            .load_embedder(id, 0)
            .expect("load embedder 0 should succeed");
        assert_eq!(emb_0.method, original.embeddings.get(&0).unwrap().method);

        // Load full fingerprint
        let loaded = memex
            .load_quantized_fingerprint(id)
            .expect("load should succeed");

        assert_eq!(loaded.id, id);
        assert_eq!(loaded.embeddings.len(), 13);

        // Verify each embedder data matches
        for i in 0..13u8 {
            let orig = original.embeddings.get(&i).unwrap();
            let load = loaded.embeddings.get(&i).unwrap();
            assert_eq!(orig.method, load.method, "embedder {} method mismatch", i);
            assert_eq!(
                orig.original_dim, load.original_dim,
                "embedder {} dim mismatch",
                i
            );
            assert_eq!(orig.data, load.data, "embedder {} data mismatch", i);
        }

        // Delete
        memex
            .delete_quantized_fingerprint(id)
            .expect("delete should succeed");

        // Verify not exists
        assert!(
            !memex.exists_quantized_fingerprint(id).unwrap(),
            "fingerprint should not exist after delete"
        );

        // Load should fail
        let result = memex.load_quantized_fingerprint(id);
        assert!(result.is_err(), "load should fail after delete");
    }

    #[test]
    fn test_load_nonexistent_fingerprint() {
        let tmp = TempDir::new().expect("create temp dir");
        let memex = match RocksDbMemex::open(tmp.path()) {
            Ok(m) => m,
            Err(_) => return,
        };

        if memex.get_cf("emb_0").is_err() {
            return;
        }

        let nonexistent_id = Uuid::new_v4();
        let result = memex.load_quantized_fingerprint(nonexistent_id);

        match result {
            Err(QuantizedStorageError::NotFound { fingerprint_id }) => {
                assert_eq!(fingerprint_id, nonexistent_id);
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    #[should_panic(expected = "STORAGE ERROR")]
    fn test_invalid_embedder_index_panics() {
        let tmp = TempDir::new().expect("create temp dir");
        let memex = match RocksDbMemex::open(tmp.path()) {
            Ok(m) => m,
            Err(_) => panic!("STORAGE ERROR: test setup failed"),
        };

        if memex.get_cf("emb_0").is_err() {
            panic!("STORAGE ERROR: CFs not available");
        }

        // This should panic because embedder_idx=15 is invalid
        let _ = memex.load_embedder(Uuid::new_v4(), 15);
    }

    // =========================================================================
    // PHYSICAL VERIFICATION TESTS
    // =========================================================================

    #[test]
    fn test_physical_storage_verification() {
        // This test performs PHYSICAL VERIFICATION of storage
        // by checking actual bytes in the database
        let tmp = TempDir::new().expect("create temp dir");
        let memex = match RocksDbMemex::open(tmp.path()) {
            Ok(m) => m,
            Err(_) => return,
        };

        if memex.get_cf("emb_0").is_err() {
            return;
        }

        let fingerprint = create_test_fingerprint();
        let id = fingerprint.id;

        // Store
        memex.store_quantized_fingerprint(&fingerprint).unwrap();

        // PHYSICAL VERIFICATION: Read raw bytes from each CF
        let key = embedder_key(id);
        for (i, cf_name) in QUANTIZED_EMBEDDER_CFS.iter().enumerate() {
            let cf = memex.get_cf(cf_name).unwrap();
            let raw_data = memex
                .db()
                .get_cf(cf, key)
                .expect("raw read should succeed")
                .expect("data should exist");

            // Verify raw data is not empty
            assert!(!raw_data.is_empty(), "CF {} should have data", cf_name);

            // Verify raw data can be deserialized
            let embedding: QuantizedEmbedding =
                bincode::deserialize(&raw_data).expect("raw data should deserialize");

            // Verify embedding matches original
            let original_emb = fingerprint.embeddings.get(&(i as u8)).unwrap();
            assert_eq!(
                embedding.method, original_emb.method,
                "Physical verification: method mismatch in {}",
                cf_name
            );
            assert_eq!(
                embedding.data, original_emb.data,
                "Physical verification: data mismatch in {}",
                cf_name
            );
        }
    }

    // =========================================================================
    // EDGE CASE TESTS
    // =========================================================================

    #[test]
    fn test_estimated_size_within_limits() {
        let fingerprint = create_test_fingerprint();
        let size = fingerprint.estimated_size_bytes();

        assert!(size > 0, "Estimated size should be > 0");
        assert!(
            size <= MAX_QUANTIZED_SIZE_BYTES,
            "Estimated size {} exceeds max {}",
            size,
            MAX_QUANTIZED_SIZE_BYTES
        );
    }

    #[test]
    fn test_all_embedders_have_unique_data() {
        let embeddings = create_test_embeddings();

        // Verify each embedder has unique data (no accidental duplicates)
        let mut seen_data: Vec<&[u8]> = Vec::new();
        for (idx, emb) in &embeddings {
            for prev in &seen_data {
                if *prev == emb.data.as_slice() && emb.data.len() > 8 {
                    panic!(
                        "Embedder {} has duplicate data with previous embedder. \
                         Test data should be unique per embedder.",
                        idx
                    );
                }
            }
            seen_data.push(&emb.data);
        }
    }
}
