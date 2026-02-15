//! Content storage operations for RocksDbTeleologicalStore.
//!
//! Contains methods for storing and retrieving raw content text.
//!
//! # Concurrency
//!
//! Batch operations use `spawn_blocking` to avoid blocking the Tokio async
//! runtime. Single-key operations use sync RocksDB calls directly since
//! they're typically fast (<1ms).

use std::sync::Arc;
use sha2::{Digest, Sha256};
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};

use crate::teleological::column_families::CF_CONTENT;
use crate::teleological::schema::content_key;
use crate::teleological::serialization::deserialize_teleological_fingerprint;

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

/// Maximum content size: 1MB
const MAX_CONTENT_SIZE: usize = 1_048_576;

impl RocksDbTeleologicalStore {
    /// Store content for a fingerprint (internal async wrapper).
    pub(crate) async fn store_content_async(&self, id: Uuid, content: &str) -> CoreResult<()> {
        // 1. Validate size - FAIL FAST
        let content_bytes = content.as_bytes();
        if content_bytes.len() > MAX_CONTENT_SIZE {
            error!(
                "CONTENT ERROR: Content size {} bytes exceeds max {} bytes for fingerprint {}",
                content_bytes.len(),
                MAX_CONTENT_SIZE,
                id
            );
            return Err(CoreError::Internal(format!(
                "Content size {} bytes exceeds maximum {} bytes for fingerprint {}",
                content_bytes.len(),
                MAX_CONTENT_SIZE,
                id
            )));
        }

        // 2. Compute SHA-256 hash of content
        let mut hasher = Sha256::new();
        hasher.update(content_bytes);
        let computed_hash: [u8; 32] = hasher.finalize().into();

        // 3. Verify hash matches if fingerprint exists
        if let Some(data) = self.get_fingerprint_raw(id)? {
            let fp = deserialize_teleological_fingerprint(&data)?;
            if fp.content_hash != computed_hash {
                error!(
                    "CONTENT ERROR: Hash mismatch for fingerprint {}. \
                     Expected: {:02x?}, Computed: {:02x?}. \
                     Content length: {} bytes.",
                    id,
                    &fp.content_hash[..8],
                    &computed_hash[..8],
                    content_bytes.len()
                );
                return Err(CoreError::Internal(format!(
                    "Content hash mismatch for fingerprint {}. Expected hash does not match provided content.",
                    id
                )));
            }
            debug!(
                "Content hash verified for fingerprint {} ({} bytes)",
                id,
                content_bytes.len()
            );
        } else {
            debug!(
                "Storing content for non-existent fingerprint {} ({} bytes). \
                 Hash verification will occur when fingerprint is created.",
                id,
                content_bytes.len()
            );
        }

        // 4. Store content
        let cf = self.cf_content();
        let key = content_key(&id);

        self.db.put_cf(cf, key, content_bytes).map_err(|e| {
            error!(
                "ROCKSDB ERROR: Failed to store content for fingerprint {}: {}",
                id, e
            );
            TeleologicalStoreError::rocksdb_op("put_content", CF_CONTENT, Some(id), e)
        })?;

        info!(
            "Stored content for fingerprint {} ({} bytes, LZ4 compressed)",
            id,
            content_bytes.len()
        );
        Ok(())
    }

    /// Retrieve content for a fingerprint (internal async wrapper).
    ///
    /// DAT-8: Verifies SHA-256 hash on retrieval to detect data corruption.
    /// If the fingerprint exists and its stored content_hash does not match
    /// the recomputed hash of the retrieved bytes, returns an error.
    pub(crate) async fn get_content_async(&self, id: Uuid) -> CoreResult<Option<String>> {
        let key = content_key(&id);
        let cf = self.cf_content();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => {
                // DAT-8: Recompute SHA-256 and verify against stored content_hash
                if let Some(fp_data) = self.get_fingerprint_raw(id)? {
                    if let Ok(fp) = deserialize_teleological_fingerprint(&fp_data) {
                        let mut hasher = Sha256::new();
                        hasher.update(&bytes);
                        let computed: [u8; 32] = hasher.finalize().into();
                        if computed != fp.content_hash {
                            error!(
                                "DAT-8: Content hash mismatch on retrieval for {}. \
                                 Stored: {:02x?}, Computed: {:02x?}. \
                                 Data corruption detected ({} bytes).",
                                id,
                                &fp.content_hash[..8],
                                &computed[..8],
                                bytes.len()
                            );
                            return Err(CoreError::Internal(format!(
                                "Content integrity check failed for {}: SHA-256 mismatch. \
                                 Stored content may be corrupted.",
                                id
                            )));
                        }
                    }
                }

                String::from_utf8(bytes).map(Some).map_err(|e| {
                    error!(
                        "CONTENT ERROR: Invalid UTF-8 in stored content for fingerprint {}. \
                             This indicates data corruption. Error: {}. Bytes length: {}",
                        id,
                        e,
                        e.as_bytes().len()
                    );
                    CoreError::Internal(format!(
                        "Invalid UTF-8 in content for {}: {}. Data corruption detected.",
                        id, e
                    ))
                })
            }
            Ok(None) => {
                debug!("No content found for fingerprint {}", id);
                Ok(None)
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to read content for fingerprint {}: {}",
                    id, e
                );
                Err(CoreError::StorageError(format!(
                    "Failed to read content for {}: {}",
                    id, e
                )))
            }
        }
    }

    /// Batch retrieve content (internal async wrapper).
    pub(crate) async fn get_content_batch_async(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<String>>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = ids.len();
        debug!("Batch retrieving content for {} fingerprints", batch_size);

        let db = Arc::clone(&self.db);
        let ids = ids.to_vec();

        let contents = tokio::task::spawn_blocking(move || -> CoreResult<Vec<Option<String>>> {
            let cf = db
                .cf_handle(CF_CONTENT)
                .ok_or_else(|| CoreError::Internal("CF_CONTENT not found".to_string()))?;

            let keys: Vec<_> = ids
                .iter()
                .map(|id| (cf, content_key(id).to_vec()))
                .collect();

            let results = db.multi_get_cf(keys);

            let mut contents = Vec::with_capacity(ids.len());
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(Some(bytes)) => {
                        let content = String::from_utf8(bytes).map_err(|e| {
                            error!(
                                "CONTENT ERROR: Invalid UTF-8 in batch content for fingerprint {}. \
                                 Index: {}, Error: {}",
                                ids[i], i, e
                            );
                            CoreError::Internal(format!(
                                "Invalid UTF-8 in content for {}: {}. Data corruption detected.",
                                ids[i], e
                            ))
                        })?;
                        contents.push(Some(content));
                    }
                    Ok(None) => contents.push(None),
                    Err(e) => {
                        error!(
                            "ROCKSDB ERROR: Batch read failed at index {} (fingerprint {}): {}",
                            i, ids[i], e
                        );
                        return Err(CoreError::StorageError(format!(
                            "Failed to read content batch at index {}: {}",
                            i, e
                        )));
                    }
                }
            }

            Ok(contents)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        let found_count = contents.iter().filter(|c| c.is_some()).count();
        debug!(
            "Batch content retrieval complete: {} requested, {} found",
            batch_size,
            found_count
        );
        Ok(contents)
    }

    /// Delete content for a fingerprint (internal async wrapper).
    pub(crate) async fn delete_content_async(&self, id: Uuid) -> CoreResult<bool> {
        let key = content_key(&id);
        let cf = self.cf_content();

        let exists = match self.db.get_cf(cf, key) {
            Ok(Some(_)) => true,
            Ok(None) => {
                debug!("No content to delete for fingerprint {}", id);
                return Ok(false);
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to check content existence for fingerprint {}: {}",
                    id, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to check content existence for {}: {}",
                    id, e
                )));
            }
        };

        if exists {
            self.db.delete_cf(cf, key).map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to delete content for fingerprint {}: {}",
                    id, e
                );
                CoreError::StorageError(format!("Failed to delete content for {}: {}", id, e))
            })?;
            info!("Deleted content for fingerprint {}", id);
        }

        Ok(exists)
    }
}
