//! File index operations for RocksDbTeleologicalStore.
//!
//! Contains methods for managing the file path to fingerprint ID index.
//! This enables O(1) lookup of fingerprints by file path for the file watcher
//! management MCP tools.
//!
//! # FAIL FAST Policy
//!
//! All operations error immediately without fallbacks. Every error includes:
//! - The operation that failed
//! - The file path involved
//! - The underlying RocksDB error (if applicable)
//!
//! # Concurrency
//!
//! O(n) scan operations (list_indexed_files) use `spawn_blocking` to avoid
//! blocking the Tokio async runtime. Single-key operations use sync RocksDB
//! calls directly since they're typically fast (<1ms).

use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::types::file_index::{FileIndexEntry, FileWatcherStats};

use crate::teleological::column_families::CF_FILE_INDEX;

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

// ============================================================================
// RocksDB File Index Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Get the file_index column family handle (FAIL FAST on missing).
    #[inline]
    pub(crate) fn cf_file_index(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_FILE_INDEX)
            .expect("CF_FILE_INDEX must exist - database initialization failed")
    }

    /// List all indexed files with their metadata.
    ///
    /// # Returns
    /// Vector of FileIndexEntry for all files in the index.
    ///
    /// # Errors
    /// - Storage error if iteration fails
    pub(crate) async fn list_indexed_files_async(&self) -> CoreResult<Vec<FileIndexEntry>> {
        let db = Arc::clone(&self.db);

        let entries = tokio::task::spawn_blocking(move || -> CoreResult<Vec<FileIndexEntry>> {
            let cf = db
                .cf_handle(CF_FILE_INDEX)
                .ok_or_else(|| CoreError::Internal("CF_FILE_INDEX not found".to_string()))?;
            let mut entries = Vec::new();

            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter {
                match item {
                    Ok((key, value)) => {
                        // Deserialize the entry
                        match serde_json::from_slice::<FileIndexEntry>(&value) {
                            Ok(entry) => {
                                entries.push(entry);
                            }
                            Err(e) => {
                                let key_str = String::from_utf8_lossy(&key);
                                error!(
                                    "FILE_INDEX ERROR: Failed to deserialize entry for key '{}': {}. \
                                     This indicates data corruption. Skipping entry.",
                                    key_str, e
                                );
                                // FAIL FAST: Don't silently skip - return error
                                return Err(CoreError::Internal(format!(
                                    "Failed to deserialize file index entry for '{}': {}. Data corruption detected.",
                                    key_str, e
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        error!("FILE_INDEX ERROR: Iterator error: {}", e);
                        return Err(CoreError::StorageError(format!(
                            "Failed to iterate file index: {}",
                            e
                        )));
                    }
                }
            }

            Ok(entries)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        debug!("Listed {} indexed files", entries.len());
        Ok(entries)
    }

    /// Get fingerprint IDs for a specific file path (O(1) lookup).
    ///
    /// # Arguments
    /// * `file_path` - The file path to look up
    ///
    /// # Returns
    /// Vector of fingerprint UUIDs for the file, or empty if not found.
    pub(crate) async fn get_fingerprints_for_file_async(
        &self,
        file_path: &str,
    ) -> CoreResult<Vec<Uuid>> {
        let cf = self.cf_file_index();
        let key = file_path.as_bytes();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => {
                let entry: FileIndexEntry = serde_json::from_slice(&bytes).map_err(|e| {
                    error!(
                        "FILE_INDEX ERROR: Failed to deserialize entry for '{}': {}. \
                         Data corruption detected.",
                        file_path, e
                    );
                    CoreError::Internal(format!(
                        "Failed to deserialize file index entry for '{}': {}",
                        file_path, e
                    ))
                })?;
                debug!(
                    "Found {} fingerprints for file '{}'",
                    entry.fingerprint_ids.len(),
                    file_path
                );
                Ok(entry.fingerprint_ids)
            }
            Ok(None) => {
                debug!("No fingerprints found for file '{}'", file_path);
                Ok(Vec::new())
            }
            Err(e) => {
                error!(
                    "FILE_INDEX ERROR: Failed to read entry for '{}': {}",
                    file_path, e
                );
                Err(CoreError::StorageError(format!(
                    "Failed to read file index for '{}': {}",
                    file_path, e
                )))
            }
        }
    }

    /// Add a fingerprint ID to the file index.
    ///
    /// Creates the entry if it doesn't exist, or adds to existing entry.
    ///
    /// # Arguments
    /// * `file_path` - The file path
    /// * `fingerprint_id` - The fingerprint UUID to add
    pub(crate) async fn index_file_fingerprint_async(
        &self,
        file_path: &str,
        fingerprint_id: Uuid,
    ) -> CoreResult<()> {
        // STG-10 FIX: Hold secondary_index_lock for the entire read-modify-write cycle.
        // Without this, concurrent calls for the same file_path race on the read-then-write,
        // causing one caller's fingerprint_id to be silently dropped from the index.
        let _index_guard = self.secondary_index_lock.lock();

        let cf = self.cf_file_index();
        let key = file_path.as_bytes();

        // Get existing entry or create new one
        let mut entry = match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => serde_json::from_slice::<FileIndexEntry>(&bytes).map_err(|e| {
                error!(
                    "FILE_INDEX ERROR: Failed to deserialize entry for '{}': {}",
                    file_path, e
                );
                CoreError::Internal(format!(
                    "Failed to deserialize file index entry for '{}': {}",
                    file_path, e
                ))
            })?,
            Ok(None) => FileIndexEntry::new(file_path.to_string()),
            Err(e) => {
                error!(
                    "FILE_INDEX ERROR: Failed to read entry for '{}': {}",
                    file_path, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to read file index for '{}': {}",
                    file_path, e
                )));
            }
        };

        // Add the fingerprint
        entry.add_fingerprint(fingerprint_id);

        // Serialize and store
        let bytes = serde_json::to_vec(&entry).map_err(|e| {
            error!(
                "FILE_INDEX ERROR: Failed to serialize entry for '{}': {}",
                file_path, e
            );
            CoreError::Internal(format!(
                "Failed to serialize file index entry for '{}': {}",
                file_path, e
            ))
        })?;

        self.db.put_cf(cf, key, &bytes).map_err(|e| {
            error!(
                "FILE_INDEX ERROR: Failed to store entry for '{}': {}",
                file_path, e
            );
            TeleologicalStoreError::rocksdb_op("put_file_index", CF_FILE_INDEX, None, e)
        })?;

        info!(
            "Indexed fingerprint {} for file '{}' (total: {})",
            fingerprint_id,
            file_path,
            entry.fingerprint_count()
        );
        Ok(())
    }

    /// Remove a fingerprint ID from the file index.
    ///
    /// If the entry becomes empty after removal, deletes the entire entry.
    ///
    /// # Arguments
    /// * `file_path` - The file path
    /// * `fingerprint_id` - The fingerprint UUID to remove
    ///
    /// # Returns
    /// true if the fingerprint was found and removed, false otherwise.
    pub(crate) async fn unindex_file_fingerprint_async(
        &self,
        file_path: &str,
        fingerprint_id: Uuid,
    ) -> CoreResult<bool> {
        // STG-10 FIX: Hold lock during read-modify-write of file index
        let _index_guard = self.secondary_index_lock.lock();

        let cf = self.cf_file_index();
        let key = file_path.as_bytes();

        // Get existing entry
        let mut entry = match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => serde_json::from_slice::<FileIndexEntry>(&bytes).map_err(|e| {
                error!(
                    "FILE_INDEX ERROR: Failed to deserialize entry for '{}': {}",
                    file_path, e
                );
                CoreError::Internal(format!(
                    "Failed to deserialize file index entry for '{}': {}",
                    file_path, e
                ))
            })?,
            Ok(None) => {
                debug!(
                    "No index entry for file '{}', nothing to unindex",
                    file_path
                );
                return Ok(false);
            }
            Err(e) => {
                error!(
                    "FILE_INDEX ERROR: Failed to read entry for '{}': {}",
                    file_path, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to read file index for '{}': {}",
                    file_path, e
                )));
            }
        };

        // Remove the fingerprint
        if !entry.remove_fingerprint(fingerprint_id) {
            debug!(
                "Fingerprint {} not found in index for file '{}'",
                fingerprint_id, file_path
            );
            return Ok(false);
        }

        // If entry is now empty, delete it entirely
        if entry.is_empty() {
            self.db.delete_cf(cf, key).map_err(|e| {
                error!(
                    "FILE_INDEX ERROR: Failed to delete empty entry for '{}': {}",
                    file_path, e
                );
                CoreError::StorageError(format!(
                    "Failed to delete empty file index entry for '{}': {}",
                    file_path, e
                ))
            })?;
            info!("Deleted empty file index entry for '{}'", file_path);
        } else {
            // Update the entry
            let bytes = serde_json::to_vec(&entry).map_err(|e| {
                error!(
                    "FILE_INDEX ERROR: Failed to serialize entry for '{}': {}",
                    file_path, e
                );
                CoreError::Internal(format!(
                    "Failed to serialize file index entry for '{}': {}",
                    file_path, e
                ))
            })?;

            self.db.put_cf(cf, key, &bytes).map_err(|e| {
                error!(
                    "FILE_INDEX ERROR: Failed to update entry for '{}': {}",
                    file_path, e
                );
                TeleologicalStoreError::rocksdb_op("put_file_index", CF_FILE_INDEX, None, e)
            })?;

            info!(
                "Unindexed fingerprint {} from file '{}' (remaining: {})",
                fingerprint_id,
                file_path,
                entry.fingerprint_count()
            );
        }

        Ok(true)
    }

    /// Clear all fingerprints from the file index for a file path.
    ///
    /// # Arguments
    /// * `file_path` - The file path to clear
    ///
    /// # Returns
    /// Number of fingerprints that were in the index before clearing.
    pub(crate) async fn clear_file_index_async(&self, file_path: &str) -> CoreResult<usize> {
        // STG-10 FIX: Hold lock to prevent concurrent index_file_fingerprint_async
        // from adding entries between our read and delete.
        let _index_guard = self.secondary_index_lock.lock();

        let cf = self.cf_file_index();
        let key = file_path.as_bytes();

        // Get existing entry to count fingerprints
        let count = match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => {
                let entry: FileIndexEntry = serde_json::from_slice(&bytes).map_err(|e| {
                    error!(
                        "FILE_INDEX ERROR: Failed to deserialize entry for '{}': {}",
                        file_path, e
                    );
                    CoreError::Internal(format!(
                        "Failed to deserialize file index entry for '{}': {}",
                        file_path, e
                    ))
                })?;
                entry.fingerprint_count()
            }
            Ok(None) => {
                debug!("No index entry for file '{}', nothing to clear", file_path);
                return Ok(0);
            }
            Err(e) => {
                error!(
                    "FILE_INDEX ERROR: Failed to read entry for '{}': {}",
                    file_path, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to read file index for '{}': {}",
                    file_path, e
                )));
            }
        };

        // Delete the entry
        self.db.delete_cf(cf, key).map_err(|e| {
            error!(
                "FILE_INDEX ERROR: Failed to delete entry for '{}': {}",
                file_path, e
            );
            CoreError::StorageError(format!(
                "Failed to delete file index entry for '{}': {}",
                file_path, e
            ))
        })?;

        info!(
            "Cleared file index for '{}' ({} fingerprints removed)",
            file_path, count
        );
        Ok(count)
    }

    /// Get statistics about file watcher content.
    ///
    /// # Returns
    /// FileWatcherStats with aggregated information.
    pub(crate) async fn get_file_watcher_stats_async(&self) -> CoreResult<FileWatcherStats> {
        let entries = self.list_indexed_files_async().await?;

        if entries.is_empty() {
            return Ok(FileWatcherStats {
                total_files: 0,
                total_chunks: 0,
                avg_chunks_per_file: 0.0,
                min_chunks: 0,
                max_chunks: 0,
            });
        }

        let total_files = entries.len();
        let chunk_counts: Vec<usize> = entries.iter().map(|e| e.fingerprint_count()).collect();
        let total_chunks: usize = chunk_counts.iter().sum();
        let min_chunks = *chunk_counts.iter().min().unwrap_or(&0);
        let max_chunks = *chunk_counts.iter().max().unwrap_or(&0);
        let avg_chunks_per_file = total_chunks as f64 / total_files as f64;

        Ok(FileWatcherStats {
            total_files,
            total_chunks,
            avg_chunks_per_file,
            min_chunks,
            max_chunks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_index_entry_serialization_json() {
        let mut entry = FileIndexEntry::new("/test/path.md".to_string());
        entry.add_fingerprint(Uuid::new_v4());
        entry.add_fingerprint(Uuid::new_v4());

        // Test JSON serialization (used for RocksDB storage)
        let bytes = serde_json::to_vec(&entry).expect("Serialization should succeed");
        let deserialized: FileIndexEntry =
            serde_json::from_slice(&bytes).expect("Deserialization should succeed");

        assert_eq!(deserialized.file_path, entry.file_path);
        assert_eq!(deserialized.fingerprint_count(), entry.fingerprint_count());
    }
}
