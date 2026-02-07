//! Provenance storage operations for RocksDbTeleologicalStore.
//!
//! Provides storage methods for Phase 4-6 provenance column families:
//! - CF_MERGE_HISTORY: Permanent merge lineage tracking
//! - CF_IMPORTANCE_HISTORY: Permanent importance change audit trail
//! - CF_EMBEDDING_REGISTRY: Embedding model version tracking per fingerprint
//!
//! # Serialization
//!
//! All records use JSON serialization (NOT bincode) for consistency with
//! the audit log (CF_AUDIT_LOG) and to avoid bincode's DeserializeAnyNotSupported
//! issues with complex types.
//!
//! # FAIL FAST Policy
//!
//! All RocksDB operations return detailed errors with operation name, CF, and key context.
//! No fallbacks, no mock data, no silent failures.

use tracing::{debug, error};
use uuid::Uuid;

use context_graph_core::types::audit::{
    EmbeddingVersionRecord, ImportanceChangeRecord, MergeRecord,
};

use crate::teleological::column_families::{
    CF_EMBEDDING_REGISTRY, CF_IMPORTANCE_HISTORY, CF_MERGE_HISTORY,
};

use super::store::RocksDbTeleologicalStore;
use super::types::{TeleologicalStoreError, TeleologicalStoreResult};

// Minimal hex encoding for error messages
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// ============================================================================
// CF_MERGE_HISTORY Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Append a merge record to the permanent merge history.
    ///
    /// Writes to CF_MERGE_HISTORY. Unlike ReversalRecords (30-day), merge
    /// history is PERMANENT -- never expires, never deleted.
    ///
    /// # Key Format
    /// `{merged_uuid_bytes}_{timestamp_nanos_be}` (24 bytes)
    pub fn append_merge_record(
        &self,
        record: &MergeRecord,
    ) -> TeleologicalStoreResult<()> {
        let key = record.storage_key();

        let bytes = serde_json::to_vec(record).map_err(|e| {
            error!(
                "FAIL FAST: Failed to serialize MergeRecord {}: {}",
                record.id, e
            );
            TeleologicalStoreError::Serialization {
                id: Some(record.id),
                message: format!("MergeRecord serialization failed: {}", e),
            }
        })?;

        let cf = self.get_cf(CF_MERGE_HISTORY)?;
        self.db.put_cf(cf, key, &bytes).map_err(|e| {
            error!(
                "FAIL FAST: Failed to write MergeRecord {} to CF '{}': {}",
                record.id, CF_MERGE_HISTORY, e
            );
            TeleologicalStoreError::rocksdb_op("put", CF_MERGE_HISTORY, Some(record.id), e)
        })?;

        debug!(
            "Appended merge record {} ({} bytes): merged_id={}, sources={}",
            record.id,
            bytes.len(),
            record.merged_id,
            record.source_ids.len(),
        );

        Ok(())
    }

    /// Query merge history for a specific merged fingerprint ID.
    ///
    /// Uses prefix scan on CF_MERGE_HISTORY with the merged_id UUID prefix.
    /// Returns records in chronological order (oldest first).
    pub fn get_merge_history(
        &self,
        merged_id: Uuid,
        limit: usize,
    ) -> TeleologicalStoreResult<Vec<MergeRecord>> {
        let cf = self.get_cf(CF_MERGE_HISTORY)?;
        let prefix = MergeRecord::prefix_key(&merged_id);
        let iter = self.db.prefix_iterator_cf(cf, prefix);

        let mut records = Vec::new();
        let effective_limit = if limit == 0 { usize::MAX } else { limit };

        for item in iter {
            if records.len() >= effective_limit {
                break;
            }

            let (key, value) = item.map_err(|e| {
                error!(
                    "FAIL FAST: RocksDB iteration failed on CF '{}' for merged_id {}: {}",
                    CF_MERGE_HISTORY, merged_id, e
                );
                TeleologicalStoreError::rocksdb_op(
                    "prefix_iterate",
                    CF_MERGE_HISTORY,
                    None,
                    e,
                )
            })?;

            // Verify key prefix matches (prefix_iterator may overshoot)
            if key.len() < 16 || &key[..16] != merged_id.as_bytes() {
                break;
            }

            let record: MergeRecord = serde_json::from_slice(&value).map_err(|e| {
                error!(
                    "FAIL FAST: Failed to deserialize MergeRecord from CF '{}': {}",
                    CF_MERGE_HISTORY, e
                );
                TeleologicalStoreError::Deserialization {
                    key: format!("merge_history:{}", hex::encode(&*key)),
                    message: format!("MergeRecord deserialization failed: {}", e),
                }
            })?;
            records.push(record);
        }

        debug!(
            "Retrieved {} merge records for merged_id {} (limit={})",
            records.len(),
            merged_id,
            limit
        );

        Ok(records)
    }
}

// ============================================================================
// CF_IMPORTANCE_HISTORY Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Append an importance change record to the permanent history.
    ///
    /// Writes to CF_IMPORTANCE_HISTORY. PERMANENT -- never expires, never deleted.
    ///
    /// # Key Format
    /// `{memory_uuid_bytes}_{timestamp_nanos_be}` (24 bytes)
    pub fn append_importance_change(
        &self,
        record: &ImportanceChangeRecord,
    ) -> TeleologicalStoreResult<()> {
        let key = record.storage_key();

        let bytes = serde_json::to_vec(record).map_err(|e| {
            error!(
                "FAIL FAST: Failed to serialize ImportanceChangeRecord for memory {}: {}",
                record.memory_id, e
            );
            TeleologicalStoreError::Serialization {
                id: Some(record.memory_id),
                message: format!("ImportanceChangeRecord serialization failed: {}", e),
            }
        })?;

        let cf = self.get_cf(CF_IMPORTANCE_HISTORY)?;
        self.db.put_cf(cf, key, &bytes).map_err(|e| {
            error!(
                "FAIL FAST: Failed to write ImportanceChangeRecord to CF '{}': {}",
                CF_IMPORTANCE_HISTORY, e
            );
            TeleologicalStoreError::rocksdb_op(
                "put",
                CF_IMPORTANCE_HISTORY,
                Some(record.memory_id),
                e,
            )
        })?;

        debug!(
            "Appended importance change for memory {}: {:.2} -> {:.2} (delta={:.2})",
            record.memory_id, record.old_value, record.new_value, record.delta,
        );

        Ok(())
    }

    /// Query importance change history for a specific memory.
    ///
    /// Uses prefix scan on CF_IMPORTANCE_HISTORY with the memory_id UUID prefix.
    /// Returns records in chronological order (oldest first).
    pub fn get_importance_history(
        &self,
        memory_id: Uuid,
        limit: usize,
    ) -> TeleologicalStoreResult<Vec<ImportanceChangeRecord>> {
        let cf = self.get_cf(CF_IMPORTANCE_HISTORY)?;
        let prefix = ImportanceChangeRecord::prefix_key(&memory_id);
        let iter = self.db.prefix_iterator_cf(cf, prefix);

        let mut records = Vec::new();
        let effective_limit = if limit == 0 { usize::MAX } else { limit };

        for item in iter {
            if records.len() >= effective_limit {
                break;
            }

            let (key, value) = item.map_err(|e| {
                error!(
                    "FAIL FAST: RocksDB iteration failed on CF '{}' for memory {}: {}",
                    CF_IMPORTANCE_HISTORY, memory_id, e
                );
                TeleologicalStoreError::rocksdb_op(
                    "prefix_iterate",
                    CF_IMPORTANCE_HISTORY,
                    None,
                    e,
                )
            })?;

            // Verify key prefix matches
            if key.len() < 16 || &key[..16] != memory_id.as_bytes() {
                break;
            }

            let record: ImportanceChangeRecord =
                serde_json::from_slice(&value).map_err(|e| {
                    error!(
                        "FAIL FAST: Failed to deserialize ImportanceChangeRecord from CF '{}': {}",
                        CF_IMPORTANCE_HISTORY, e
                    );
                    TeleologicalStoreError::Deserialization {
                        key: format!("importance_history:{}", hex::encode(&*key)),
                        message: format!(
                            "ImportanceChangeRecord deserialization failed: {}",
                            e
                        ),
                    }
                })?;
            records.push(record);
        }

        debug!(
            "Retrieved {} importance change records for memory {} (limit={})",
            records.len(),
            memory_id,
            limit
        );

        Ok(records)
    }
}

// ============================================================================
// CF_EMBEDDING_REGISTRY Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Store an embedding version record for a fingerprint.
    ///
    /// Writes to CF_EMBEDDING_REGISTRY. Overwrites any existing record for
    /// the same fingerprint (re-embedding updates the version record).
    ///
    /// # Key Format
    /// `fingerprint_uuid_bytes` (16 bytes)
    pub fn store_embedding_version(
        &self,
        record: &EmbeddingVersionRecord,
    ) -> TeleologicalStoreResult<()> {
        let key = record.fingerprint_id.as_bytes();

        let bytes = serde_json::to_vec(record).map_err(|e| {
            error!(
                "FAIL FAST: Failed to serialize EmbeddingVersionRecord for fingerprint {}: {}",
                record.fingerprint_id, e
            );
            TeleologicalStoreError::Serialization {
                id: Some(record.fingerprint_id),
                message: format!("EmbeddingVersionRecord serialization failed: {}", e),
            }
        })?;

        let cf = self.get_cf(CF_EMBEDDING_REGISTRY)?;
        self.db.put_cf(cf, key, &bytes).map_err(|e| {
            error!(
                "FAIL FAST: Failed to write EmbeddingVersionRecord to CF '{}': {}",
                CF_EMBEDDING_REGISTRY, e
            );
            TeleologicalStoreError::rocksdb_op(
                "put",
                CF_EMBEDDING_REGISTRY,
                Some(record.fingerprint_id),
                e,
            )
        })?;

        debug!(
            "Stored embedding version for fingerprint {}: {} embedders tracked",
            record.fingerprint_id,
            record.embedder_versions.len(),
        );

        Ok(())
    }

    /// Retrieve the embedding version record for a fingerprint.
    ///
    /// Returns None if no version record exists (fingerprint hasn't been tracked yet).
    pub fn get_embedding_version(
        &self,
        fingerprint_id: Uuid,
    ) -> TeleologicalStoreResult<Option<EmbeddingVersionRecord>> {
        let cf = self.get_cf(CF_EMBEDDING_REGISTRY)?;
        let key = fingerprint_id.as_bytes();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => {
                let record: EmbeddingVersionRecord =
                    serde_json::from_slice(&bytes).map_err(|e| {
                        error!(
                            "FAIL FAST: Failed to deserialize EmbeddingVersionRecord for {}: {}",
                            fingerprint_id, e
                        );
                        TeleologicalStoreError::Deserialization {
                            key: format!("embedding_registry:{}", fingerprint_id),
                            message: format!(
                                "EmbeddingVersionRecord deserialization failed: {}",
                                e
                            ),
                        }
                    })?;
                Ok(Some(record))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                error!(
                    "FAIL FAST: Failed to read from CF '{}' for fingerprint {}: {}",
                    CF_EMBEDDING_REGISTRY, fingerprint_id, e
                );
                Err(TeleologicalStoreError::rocksdb_op(
                    "get",
                    CF_EMBEDDING_REGISTRY,
                    Some(fingerprint_id),
                    e,
                ))
            }
        }
    }
}
