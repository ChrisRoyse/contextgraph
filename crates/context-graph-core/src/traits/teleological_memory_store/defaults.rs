//! Default implementations for optional TeleologicalMemoryStore methods.
//!
//! This module provides default implementations for content storage and
//! source metadata methods. These are separated from the core trait to keep
//! the main trait file under the 500-line limit while maintaining clear organization.
//!
//! # Content Storage (TASK-CONTENT-003)
//!
//! Content storage allows associating original text content with fingerprints.
//! The default implementations return errors or empty results for backends
//! that don't support content storage.
//!
//! # Source Metadata Storage
//!
//! Source metadata allows tracking memory provenance (e.g., file path for
//! MDFileChunk memories). This enables context injection to display where
//! memories originated from.

use async_trait::async_trait;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::types::SourceMetadata;

use super::backend::TeleologicalStorageBackend;

/// Extension trait providing default implementations for optional storage features.
///
/// This trait is automatically implemented for all types implementing
/// `TeleologicalMemoryStore`. It provides default behavior for:
/// - Content storage (store, get, delete, batch get)
///
/// Backends that support these features should override the corresponding
/// methods in `TeleologicalMemoryStore` directly.
#[allow(dead_code)] // Reserved for future extension trait implementations
#[async_trait]
pub trait TeleologicalMemoryStoreDefaults: Send + Sync {
    /// Get the storage backend type for error messages.
    fn backend_type(&self) -> TeleologicalStorageBackend;

    // ==================== Content Storage Defaults ====================

    /// Default: Store content - returns unsupported error.
    ///
    /// Content is stored separately from the fingerprint for efficiency.
    /// This allows large text content to be optionally retrieved without
    /// loading it for every search result.
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    /// * `content` - Original text content (max 1MB)
    ///
    /// # Errors
    /// - `CoreError::Internal` - Content storage not supported by backend
    async fn store_content_default(&self, id: Uuid, content: &str) -> CoreResult<()> {
        let _ = (id, content); // Suppress unused warnings
        Err(CoreError::Internal(format!(
            "Content storage not supported by {} backend",
            self.backend_type()
        )))
    }

    /// Default: Get content - returns None (graceful degradation).
    ///
    /// Returns the original text content that was stored with the fingerprint.
    /// Returns None if content was never stored or backend doesn't support it.
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    ///
    /// # Returns
    /// `None` - Backend does not support content storage.
    async fn get_content_default(&self, id: Uuid) -> CoreResult<Option<String>> {
        let _ = id; // Suppress unused warnings
        Ok(None)
    }

    /// Default: Delete content - returns false (nothing to delete).
    ///
    /// Called automatically when fingerprint is deleted (cascade delete).
    /// Can also be called directly to remove content while keeping the fingerprint.
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    ///
    /// # Returns
    /// `false` - No content existed to delete.
    async fn delete_content_default(&self, id: Uuid) -> CoreResult<bool> {
        let _ = id; // Suppress unused warnings
        Ok(false)
    }

    /// Default: Batch get content - calls get_content sequentially.
    ///
    /// More efficient than individual `get_content` calls for bulk retrieval.
    /// Returns Vec with Some for found content, None for not found.
    ///
    /// # Arguments
    /// * `ids` - Slice of fingerprint UUIDs
    ///
    /// # Returns
    /// Vector of `None` values (backend does not support content storage).
    async fn get_content_batch_default(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<String>>> {
        // Default returns None for all IDs since content storage is not supported
        Ok(vec![None; ids.len()])
    }

    // ==================== Source Metadata Defaults ====================

    /// Default: Store source metadata - returns unsupported error.
    ///
    /// Source metadata tracks memory provenance (e.g., file path for MDFileChunk).
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    /// * `metadata` - Source metadata to store
    ///
    /// # Errors
    /// - `CoreError::Internal` - Source metadata storage not supported by backend
    async fn store_source_metadata_default(
        &self,
        id: Uuid,
        metadata: &SourceMetadata,
    ) -> CoreResult<()> {
        let _ = (id, metadata); // Suppress unused warnings
        Err(CoreError::Internal(format!(
            "Source metadata storage not supported by {} backend",
            self.backend_type()
        )))
    }

    /// Default: Get source metadata - returns None (graceful degradation).
    ///
    /// Returns the source metadata that was stored with the fingerprint.
    /// Returns None if metadata was never stored or backend doesn't support it.
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    ///
    /// # Returns
    /// `None` - Backend does not support source metadata storage.
    async fn get_source_metadata_default(&self, id: Uuid) -> CoreResult<Option<SourceMetadata>> {
        let _ = id; // Suppress unused warnings
        Ok(None)
    }

    /// Default: Delete source metadata - returns false (nothing to delete).
    ///
    /// Called automatically when fingerprint is deleted (cascade delete).
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    ///
    /// # Returns
    /// `false` - No metadata existed to delete.
    async fn delete_source_metadata_default(&self, id: Uuid) -> CoreResult<bool> {
        let _ = id; // Suppress unused warnings
        Ok(false)
    }

    /// Default: Batch get source metadata - returns vec of None.
    ///
    /// # Arguments
    /// * `ids` - Slice of fingerprint UUIDs
    ///
    /// # Returns
    /// Vector of `None` values (backend does not support source metadata storage).
    async fn get_source_metadata_batch_default(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<SourceMetadata>>> {
        // Default returns None for all IDs since source metadata storage is not supported
        Ok(vec![None; ids.len()])
    }

    /// Default: Find fingerprints by file path - returns empty vec.
    ///
    /// # Arguments
    /// * `file_path` - The file path to search for
    ///
    /// # Returns
    /// Empty vector (backend does not support source metadata scanning).
    async fn find_fingerprints_by_file_path_default(&self, file_path: &str) -> CoreResult<Vec<Uuid>> {
        let _ = file_path; // Suppress unused warnings
        Ok(vec![])
    }

    // ==================== File Index Defaults ====================

    /// Default: List indexed files - returns unsupported error.
    ///
    /// # Returns
    /// Error indicating file index not supported by backend.
    async fn list_indexed_files_default(&self) -> CoreResult<Vec<crate::types::FileIndexEntry>> {
        Err(CoreError::Internal(format!(
            "File index not supported by {} backend",
            self.backend_type()
        )))
    }

    /// Default: Get fingerprints for file - returns empty vec.
    ///
    /// # Arguments
    /// * `file_path` - The file path to look up
    ///
    /// # Returns
    /// Empty vector (backend does not support file index).
    async fn get_fingerprints_for_file_default(&self, file_path: &str) -> CoreResult<Vec<Uuid>> {
        let _ = file_path; // Suppress unused warnings
        Ok(vec![])
    }

    /// Default: Index file fingerprint - returns unsupported error.
    ///
    /// # Arguments
    /// * `file_path` - The file path
    /// * `fingerprint_id` - The fingerprint UUID to add
    ///
    /// # Returns
    /// Error indicating file index not supported by backend.
    async fn index_file_fingerprint_default(
        &self,
        file_path: &str,
        fingerprint_id: Uuid,
    ) -> CoreResult<()> {
        let _ = (file_path, fingerprint_id); // Suppress unused warnings
        Err(CoreError::Internal(format!(
            "File index not supported by {} backend",
            self.backend_type()
        )))
    }

    /// Default: Unindex file fingerprint - returns false.
    ///
    /// # Arguments
    /// * `file_path` - The file path
    /// * `fingerprint_id` - The fingerprint UUID to remove
    ///
    /// # Returns
    /// false (backend does not support file index).
    async fn unindex_file_fingerprint_default(
        &self,
        file_path: &str,
        fingerprint_id: Uuid,
    ) -> CoreResult<bool> {
        let _ = (file_path, fingerprint_id); // Suppress unused warnings
        Ok(false)
    }

    /// Default: Clear file index - returns 0.
    ///
    /// # Arguments
    /// * `file_path` - The file path to clear
    ///
    /// # Returns
    /// 0 (backend does not support file index).
    async fn clear_file_index_default(&self, file_path: &str) -> CoreResult<usize> {
        let _ = file_path; // Suppress unused warnings
        Ok(0)
    }

    /// Default: Get file watcher stats - returns empty stats.
    ///
    /// # Returns
    /// Empty FileWatcherStats.
    async fn get_file_watcher_stats_default(&self) -> CoreResult<crate::types::FileWatcherStats> {
        Ok(crate::types::FileWatcherStats::default())
    }

    // ==================== Topic Portfolio Persistence Defaults ====================

    /// Default: Persist topic portfolio - returns unsupported error.
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    /// * `portfolio` - The topic portfolio to persist
    ///
    /// # Errors
    /// - `CoreError::Internal` - Topic portfolio persistence not supported by backend
    async fn persist_topic_portfolio_default(
        &self,
        session_id: &str,
        portfolio: &crate::clustering::PersistedTopicPortfolio,
    ) -> CoreResult<()> {
        let _ = (session_id, portfolio); // Suppress unused warnings
        Err(CoreError::Internal(format!(
            "Topic portfolio persistence not supported by {} backend",
            self.backend_type()
        )))
    }

    /// Default: Load topic portfolio - returns None (graceful degradation).
    ///
    /// # Arguments
    /// * `session_id` - The session identifier to load
    ///
    /// # Returns
    /// `None` - Backend does not support topic portfolio persistence.
    async fn load_topic_portfolio_default(
        &self,
        session_id: &str,
    ) -> CoreResult<Option<crate::clustering::PersistedTopicPortfolio>> {
        let _ = session_id; // Suppress unused warnings
        Ok(None)
    }

    /// Default: Load latest topic portfolio - returns None (graceful degradation).
    ///
    /// # Returns
    /// `None` - Backend does not support topic portfolio persistence.
    async fn load_latest_topic_portfolio_default(
        &self,
    ) -> CoreResult<Option<crate::clustering::PersistedTopicPortfolio>> {
        Ok(None)
    }
}
