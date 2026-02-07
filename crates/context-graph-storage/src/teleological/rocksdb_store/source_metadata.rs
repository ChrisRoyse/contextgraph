//! Source metadata storage operations for RocksDbTeleologicalStore.
//!
//! Contains methods for storing and retrieving source metadata (provenance tracking).
//!
//! # Concurrency
//!
//! Batch and O(n) scan operations use `spawn_blocking` to avoid blocking the
//! Tokio async runtime. Single-key operations use sync RocksDB calls directly
//! since they're typically fast (<1ms).

use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::types::SourceMetadata;

use crate::teleological::column_families::CF_SOURCE_METADATA;
use crate::teleological::schema::source_metadata_key;

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

impl RocksDbTeleologicalStore {
    /// Deserialize source metadata, trying JSON first then bincode for migration.
    ///
    /// JSON is the canonical format (bincode is incompatible with skip_serializing_if).
    /// Bincode fallback handles data written before the JSON migration.
    fn deserialize_source_metadata(bytes: &[u8], id: Uuid) -> CoreResult<SourceMetadata> {
        // Try JSON first (canonical format)
        if let Ok(metadata) = serde_json::from_slice::<SourceMetadata>(bytes) {
            return Ok(metadata);
        }

        // Try bincode for pre-migration data
        if let Ok(metadata) = bincode::deserialize::<SourceMetadata>(bytes) {
            return Ok(metadata);
        }

        // Both failed - genuine corruption
        error!(
            "METADATA ERROR: Failed to deserialize source metadata for fingerprint {}. \
             Bytes length: {}. Neither JSON nor bincode could parse. Data corrupted.",
            id, bytes.len()
        );
        Err(CoreError::Internal(format!(
            "Failed to deserialize source metadata for {}: data corruption ({}B, not valid JSON or bincode)",
            id, bytes.len()
        )))
    }

    /// Store source metadata for a fingerprint (internal async wrapper).
    ///
    /// Uses JSON serialization (not bincode) because SourceMetadata uses
    /// `skip_serializing_if` which is incompatible with bincode.
    pub(crate) async fn store_source_metadata_async(
        &self,
        id: Uuid,
        metadata: &SourceMetadata,
    ) -> CoreResult<()> {
        // Serialize metadata using JSON (NOT bincode - skip_serializing_if is incompatible)
        let bytes = serde_json::to_vec(metadata).map_err(|e| {
            error!(
                "METADATA ERROR: Failed to serialize source metadata for fingerprint {}: {}",
                id, e
            );
            CoreError::Internal(format!(
                "Failed to serialize source metadata for {}: {}",
                id, e
            ))
        })?;

        let cf = self.cf_source_metadata();
        let key = source_metadata_key(&id);

        self.db.put_cf(cf, key, &bytes).map_err(|e| {
            error!(
                "ROCKSDB ERROR: Failed to store source metadata for fingerprint {}: {}",
                id, e
            );
            TeleologicalStoreError::rocksdb_op("put_source_metadata", CF_SOURCE_METADATA, Some(id), e)
        })?;

        info!(
            "Stored source metadata for fingerprint {} ({} bytes JSON, type: {:?})",
            id,
            bytes.len(),
            metadata.source_type
        );
        Ok(())
    }

    /// Retrieve source metadata for a fingerprint (internal async wrapper).
    pub(crate) async fn get_source_metadata_async(
        &self,
        id: Uuid,
    ) -> CoreResult<Option<SourceMetadata>> {
        let key = source_metadata_key(&id);
        let cf = self.cf_source_metadata();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => {
                let metadata = Self::deserialize_source_metadata(&bytes, id)?;
                debug!("Retrieved source metadata for fingerprint {}", id);
                Ok(Some(metadata))
            }
            Ok(None) => {
                debug!("No source metadata found for fingerprint {}", id);
                Ok(None)
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to read source metadata for fingerprint {}: {}",
                    id, e
                );
                Err(CoreError::StorageError(format!(
                    "Failed to read source metadata for {}: {}",
                    id, e
                )))
            }
        }
    }

    /// Batch retrieve source metadata (internal async wrapper).
    pub(crate) async fn get_source_metadata_batch_async(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<SourceMetadata>>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = ids.len();
        debug!(
            "Batch retrieving source metadata for {} fingerprints",
            batch_size
        );

        let db = Arc::clone(&self.db);
        let ids = ids.to_vec();

        let metadata_vec = tokio::task::spawn_blocking(move || -> CoreResult<Vec<Option<SourceMetadata>>> {
            let cf = db
                .cf_handle(CF_SOURCE_METADATA)
                .ok_or_else(|| CoreError::Internal("CF_SOURCE_METADATA not found".to_string()))?;

            let keys: Vec<_> = ids
                .iter()
                .map(|id| (cf, source_metadata_key(id).to_vec()))
                .collect();

            let results = db.multi_get_cf(keys);

            let mut metadata_vec = Vec::with_capacity(ids.len());
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(Some(bytes)) => {
                        match Self::deserialize_source_metadata(&bytes, ids[i]) {
                            Ok(metadata) => metadata_vec.push(Some(metadata)),
                            Err(e) => {
                                error!(
                                    "METADATA CORRUPTION: Skipping unreadable source metadata for {} ({}B): {}",
                                    ids[i], bytes.len(), e
                                );
                                metadata_vec.push(None);
                            }
                        }
                    }
                    Ok(None) => metadata_vec.push(None),
                    Err(e) => {
                        error!(
                            "ROCKSDB ERROR: Batch read failed at index {} (fingerprint {}): {}",
                            i, ids[i], e
                        );
                        return Err(CoreError::StorageError(format!(
                            "Failed to read source metadata batch at index {}: {}",
                            i, e
                        )));
                    }
                }
            }

            Ok(metadata_vec)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        let found_count = metadata_vec.iter().filter(|m| m.is_some()).count();
        debug!(
            "Batch source metadata retrieval complete: {} requested, {} found",
            batch_size,
            found_count
        );
        Ok(metadata_vec)
    }

    /// Delete source metadata for a fingerprint (internal async wrapper).
    pub(crate) async fn delete_source_metadata_async(&self, id: Uuid) -> CoreResult<bool> {
        let key = source_metadata_key(&id);
        let cf = self.cf_source_metadata();

        let exists = match self.db.get_cf(cf, key) {
            Ok(Some(_)) => true,
            Ok(None) => {
                debug!("No source metadata to delete for fingerprint {}", id);
                return Ok(false);
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to check source metadata existence for fingerprint {}: {}",
                    id, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to check source metadata existence for {}: {}",
                    id, e
                )));
            }
        };

        if exists {
            self.db.delete_cf(cf, key).map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to delete source metadata for fingerprint {}: {}",
                    id, e
                );
                CoreError::StorageError(format!(
                    "Failed to delete source metadata for {}: {}",
                    id, e
                ))
            })?;
            info!("Deleted source metadata for fingerprint {}", id);
        }

        Ok(exists)
    }

    /// Find all fingerprint IDs that have source metadata matching a file path.
    ///
    /// Scans all source metadata entries and returns UUIDs of fingerprints
    /// whose file_path matches the given path. Used for stale embedding cleanup
    /// when files are modified.
    ///
    /// # Arguments
    /// * `file_path` - The file path to search for
    ///
    /// # Returns
    /// * `Ok(Vec<Uuid>)` - UUIDs of matching fingerprints
    /// * `Err` - If scan fails
    pub(crate) async fn find_fingerprints_by_file_path(
        &self,
        file_path: &str,
    ) -> CoreResult<Vec<Uuid>> {
        let db = Arc::clone(&self.db);
        let file_path_owned = file_path.to_string();
        let file_path_log = file_path_owned.clone();

        let matching_ids = tokio::task::spawn_blocking(move || -> CoreResult<Vec<Uuid>> {
            let cf = db
                .cf_handle(CF_SOURCE_METADATA)
                .ok_or_else(|| CoreError::Internal("CF_SOURCE_METADATA not found".to_string()))?;
            let mut matching_ids = Vec::new();

            // Iterate through all source metadata entries
            // Keys are raw 16-byte UUIDs (from source_metadata_key())
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter {
                match item {
                    Ok((key, value)) => {
                        if key.len() != 16 {
                            continue; // Skip malformed keys
                        }
                        if let Ok(id) = Uuid::from_slice(&key) {
                            // Deserialize metadata (JSON canonical, bincode fallback)
                            let parsed = serde_json::from_slice::<SourceMetadata>(&value)
                                .ok()
                                .or_else(|| bincode::deserialize::<SourceMetadata>(&value).ok());
                            if let Some(metadata) = parsed {
                                if let Some(ref path) = metadata.file_path {
                                    if path == &file_path_owned {
                                        matching_ids.push(id);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error iterating source metadata: {}", e);
                    }
                }
            }

            Ok(matching_ids)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        debug!(
            "Found {} fingerprints for file path {}",
            matching_ids.len(),
            file_path_log
        );
        Ok(matching_ids)
    }
}
