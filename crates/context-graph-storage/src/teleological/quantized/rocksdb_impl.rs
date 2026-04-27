//! RocksDB implementation of QuantizedFingerprintStorage.
//!
//! Implements storage and retrieval of quantized fingerprints using
//! RocksDB column families (emb_0 through emb_13).

use context_graph_embeddings::{
    QuantizationRouter, QuantizedEmbedding, StoredQuantizedFingerprint, STORAGE_VERSION,
};
use rocksdb::WriteBatch;
use tracing::warn;
use uuid::Uuid;

use super::error::{QuantizedStorageError, QuantizedStorageResult};
use super::helpers::{
    deserialize_quantized_embedding, embedder_key, serialize_quantized_embedding,
};
use super::trait_def::QuantizedFingerprintStorage;
use crate::teleological::column_families::{
    CF_TOPIC_PROFILES, QUANTIZED_EMBEDDER_CFS, QUANTIZED_EMBEDDER_CF_COUNT,
};
use crate::teleological::schema::topic_profile_key;
use crate::teleological::serialization::{deserialize_topic_profile, serialize_topic_profile};
use crate::teleological::RocksDbTeleologicalStore;

// =============================================================================
// ROCKS DB IMPLEMENTATION
// =============================================================================

impl QuantizedFingerprintStorage for RocksDbTeleologicalStore {
    fn store_quantized_fingerprint(
        &self,
        fingerprint: &StoredQuantizedFingerprint,
    ) -> QuantizedStorageResult<()> {
        // FAIL FAST: Verify all quantized embedders are present.
        if fingerprint.embeddings.len() != QUANTIZED_EMBEDDER_CF_COUNT {
            panic!(
                "STORAGE ERROR: Cannot store fingerprint {} with {} embedders. \
                 Expected exactly {} embedders. Missing indices: {:?}. \
                 This indicates incomplete fingerprint generation.",
                fingerprint.id,
                fingerprint.embeddings.len(),
                QUANTIZED_EMBEDDER_CF_COUNT,
                (0..QUANTIZED_EMBEDDER_CF_COUNT)
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
                     Valid range: 0-{}.",
                    embedder_idx,
                    fingerprint.id,
                    QUANTIZED_EMBEDDER_CF_COUNT - 1
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

        // Store the topic profile in its canonical CF in the same batch. Loading
        // a StoredQuantizedFingerprint reads this CF, so writing it here keeps
        // the struct roundtrip physically verifiable instead of returning zeros.
        let topic_cf = self.get_cf(CF_TOPIC_PROFILES).map_err(|_| {
            QuantizedStorageError::ColumnFamilyNotFound {
                cf_name: CF_TOPIC_PROFILES.to_string(),
            }
        })?;
        batch.put_cf(
            topic_cf,
            topic_profile_key(&fingerprint.id),
            serialize_topic_profile(&fingerprint.topic_profile),
        );

        // Atomic write of all embedders and the topic profile.
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

        // Load all quantized embedders.
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

        // FAIL FAST: Verify we got every quantized embedder.
        if embeddings.len() != QUANTIZED_EMBEDDER_CF_COUNT {
            panic!(
                "STORAGE ERROR: Loaded only {} embedders for fingerprint {}. \
                 Expected {}. This indicates database corruption.",
                embeddings.len(),
                id,
                QUANTIZED_EMBEDDER_CF_COUNT
            );
        }

        // Load topic_profile from CF_TOPIC_PROFILES (FAIL FAST if unavailable)
        let topic_profile = match self.get_cf(CF_TOPIC_PROFILES) {
            Ok(cf_purpose) => {
                let pv_key = topic_profile_key(&id);
                match self.db().get_cf(cf_purpose, pv_key) {
                    Ok(Some(data)) => match deserialize_topic_profile(&data) {
                        Ok(profile) => profile,
                        Err(e) => {
                            warn!(
                                "STORAGE WARNING: Corrupted topic profile for fingerprint {}: {}. \
                                 Using zero vector as fallback.",
                                id, e
                            );
                            [0.0f32; 14]
                        }
                    },
                    Ok(None) => {
                        // Topic profile missing - this is a data integrity issue
                        // Log warning but return zeros to allow degraded operation
                        // (caller should ideally re-index this fingerprint)
                        warn!(
                            "STORAGE WARNING: Topic profile missing for fingerprint {}. \
                             This indicates incomplete fingerprint storage. \
                             Memory should be re-indexed with complete TeleologicalFingerprint.",
                            id
                        );
                        [0.0f32; 14]
                    }
                    Err(e) => {
                        // Storage error - fail fast
                        panic!(
                            "STORAGE ERROR: Failed to read topic profile for fingerprint {}: {}. \
                             This indicates a broken storage layer that must be fixed.",
                            id, e
                        );
                    }
                }
            }
            Err(_) => {
                // CF not found - likely DB wasn't opened with teleological CFs
                warn!(
                    "STORAGE WARNING: CF_TOPIC_PROFILES not available for fingerprint {}. \
                     Database may need migration to include teleological CFs.",
                    id
                );
                [0.0f32; 14]
            }
        };

        // Note: content_hash is stored in CF_FINGERPRINTS
        // as part of the full TeleologicalFingerprint. For quantized-only storage,
        // we use defaults. Callers needing full metadata should use the
        // TeleologicalMemoryStore::get() method instead.
        //
        // FAIL FAST is maintained: embeddings are required (panic if missing),
        // topic_profile is loaded from storage (warn if missing),
        // content_hash uses documented default (caller's responsibility).
        Ok(StoredQuantizedFingerprint::new(
            id,
            embeddings,
            topic_profile,
            [0u8; 32], // Content hash default - load from CF_FINGERPRINTS if needed
        ))
    }

    fn load_embedder(
        &self,
        fingerprint_id: Uuid,
        embedder_idx: u8,
    ) -> QuantizedStorageResult<QuantizedEmbedding> {
        // FAIL FAST: Verify embedder index is valid (0..QUANTIZED_EMBEDDER_CF_COUNT).
        // Post-E14: the valid range is 0-13 (14 CFs: emb_0..emb_13).
        if embedder_idx >= QUANTIZED_EMBEDDER_CF_COUNT as u8 {
            panic!(
                "STORAGE ERROR: Invalid embedder index {}. Valid range: 0-{}.",
                embedder_idx,
                QUANTIZED_EMBEDDER_CF_COUNT - 1
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

        // Delete from all quantized embedder CFs.
        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf =
                self.get_cf(cf_name)
                    .map_err(|_| QuantizedStorageError::ColumnFamilyNotFound {
                        cf_name: cf_name.to_string(),
                    })?;
            batch.delete_cf(cf, key);
        }

        let topic_cf = self.get_cf(CF_TOPIC_PROFILES).map_err(|_| {
            QuantizedStorageError::ColumnFamilyNotFound {
                cf_name: CF_TOPIC_PROFILES.to_string(),
            }
        })?;
        batch.delete_cf(topic_cf, topic_profile_key(&id));

        // Atomic delete of all embedder rows and the topic profile row.
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
        // as a field in the store. For now, we create a new one each time.
        // This is a design limitation that should be addressed in TASK-EMB-023.
        //
        // TEMPORARY: Return a static router. This is safe because QuantizationRouter
        // has no mutable state after construction.
        static ROUTER: std::sync::OnceLock<QuantizationRouter> = std::sync::OnceLock::new();
        ROUTER.get_or_init(QuantizationRouter::new)
    }
}
