//! Core TeleologicalMemoryStore trait for 5-stage teleological retrieval.
//!
//! This module defines the core storage trait for the Context Graph system's
//! teleological memory architecture.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use uuid::Uuid;

use crate::error::CoreResult;
use crate::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
use crate::types::SourceMetadata;

use super::backend::TeleologicalStorageBackend;
use super::options::TeleologicalSearchOptions;
use super::result::TeleologicalSearchResult;

/// Core trait for teleological memory storage operations.
///
/// This trait defines the complete interface for storing, retrieving,
/// and searching TeleologicalFingerprints. Implementations must support:
/// - Full CRUD operations with soft/hard delete
/// - Multi-space semantic search
/// - Sparse (SPLADE) search for efficient recall
/// - Batch operations for throughput
/// - Persistence and recovery
///
/// # Implementation Notes
///
/// - All methods are async for I/O flexibility
/// - All errors use `CoreError` variants for consistent handling
/// - The trait requires `Send + Sync` for concurrent access
/// - Implementations should log errors via `tracing` before returning
///
/// # Example
///
/// ```ignore
/// use context_graph_core::traits::TeleologicalMemoryStore;
/// use context_graph_core::stubs::InMemoryTeleologicalStore;
///
/// let store = InMemoryTeleologicalStore::new();
/// let id = store.store(fingerprint).await?;
/// let retrieved = store.retrieve(id).await?;
/// ```
#[async_trait]
pub trait TeleologicalMemoryStore: Send + Sync {
    // ==================== CRUD Operations ====================

    /// Store a new teleological fingerprint.
    ///
    /// # Arguments
    /// * `fingerprint` - The fingerprint to store
    ///
    /// # Returns
    /// The UUID assigned to the stored fingerprint.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::ValidationError` - Invalid fingerprint data
    /// - `CoreError::SerializationError` - Serialization failure
    async fn store(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<Uuid>;

    /// Retrieve a fingerprint by its UUID.
    ///
    /// # Arguments
    /// * `id` - The UUID of the fingerprint to retrieve
    ///
    /// # Returns
    /// `Some(fingerprint)` if found, `None` if not found or soft-deleted.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::SerializationError` - Deserialization failure
    async fn retrieve(&self, id: Uuid) -> CoreResult<Option<TeleologicalFingerprint>>;

    /// Update an existing fingerprint.
    ///
    /// Replaces the entire fingerprint with the new data.
    /// The fingerprint's `id` field determines which record to update.
    ///
    /// # Arguments
    /// * `fingerprint` - The updated fingerprint (must have existing ID)
    ///
    /// # Returns
    /// `true` if updated, `false` if ID not found.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::ValidationError` - Invalid fingerprint data
    async fn update(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<bool>;

    /// Delete a fingerprint.
    ///
    /// # Arguments
    /// * `id` - The UUID of the fingerprint to delete
    /// * `soft` - If true, mark as deleted but retain data; if false, permanently remove
    ///
    /// # Returns
    /// `true` if deleted, `false` if ID not found.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn delete(&self, id: Uuid, soft: bool) -> CoreResult<bool>;

    // ==================== Search Operations ====================

    /// Search by semantic similarity using the 13-embedding fingerprint.
    ///
    /// Computes similarity across all 13 embedding spaces and aggregates
    /// using Reciprocal Rank Fusion (RRF) or weighted averaging.
    ///
    /// # Arguments
    /// * `query` - The semantic fingerprint to search for
    /// * `options` - Search options (top_k, filters, etc.)
    ///
    /// # Returns
    /// Vector of search results sorted by similarity (descending).
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::ValidationError` - Invalid query fingerprint
    async fn search_semantic(
        &self,
        query: &SemanticFingerprint,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>>;

    /// Full-text search using text query (generates embeddings internally).
    ///
    /// This method handles embedding generation for the text query and
    /// delegates to `search_semantic`. Implementations may cache embeddings.
    ///
    /// # Arguments
    /// * `text` - The text query to search for
    /// * `options` - Search options (top_k, filters, etc.)
    ///
    /// # Returns
    /// Vector of search results sorted by relevance (descending).
    ///
    /// # Errors
    /// - `CoreError::Embedding` - Embedding generation failure
    /// - `CoreError::StorageError` - Storage backend failure
    async fn search_text(
        &self,
        text: &str,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>>;

    /// Sparse search using E13 SPLADE embeddings.
    ///
    /// Stage 1 (Recall) of the 5-stage pipeline. Uses inverted index
    /// for efficient initial candidate retrieval.
    ///
    /// # Arguments
    /// * `sparse_query` - The sparse vector query (E13 SPLADE)
    /// * `top_k` - Maximum number of candidates to return
    ///
    /// # Returns
    /// Vector of (UUID, score) pairs sorted by sparse dot product (descending).
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::IndexError` - Inverted index failure
    async fn search_sparse(
        &self,
        sparse_query: &SparseVector,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>>;

    /// E6 sparse recall using V_selectivity inverted index.
    ///
    /// Stage 1 (Recall) for exact keyword matching. Finds candidates that
    /// share terms with the query that E1 semantic search might miss due to
    /// embedding averaging.
    ///
    /// Per Constitution: E6 finds "exact keyword matches" that E1 misses by
    /// "diluting through averaging".
    ///
    /// # Arguments
    /// * `sparse_query` - The E6 sparse vector query
    /// * `max_candidates` - Maximum number of candidates to return
    ///
    /// # Returns
    /// Vector of (UUID, term_overlap_count) pairs sorted by overlap (descending).
    /// The count indicates how many query terms matched the document.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::IndexError` - Inverted index failure
    async fn search_e6_sparse(
        &self,
        sparse_query: &SparseVector,
        max_candidates: usize,
    ) -> CoreResult<Vec<(Uuid, usize)>>;

    // ==================== Batch Operations ====================

    /// Store multiple fingerprints in a batch.
    ///
    /// More efficient than individual `store` calls for bulk ingestion.
    ///
    /// # Arguments
    /// * `fingerprints` - Vector of fingerprints to store
    ///
    /// # Returns
    /// Vector of UUIDs assigned to each fingerprint (same order as input).
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::ValidationError` - Invalid fingerprint in batch
    async fn store_batch(
        &self,
        fingerprints: Vec<TeleologicalFingerprint>,
    ) -> CoreResult<Vec<Uuid>>;

    /// Retrieve multiple fingerprints by their UUIDs.
    ///
    /// # Arguments
    /// * `ids` - Slice of UUIDs to retrieve
    ///
    /// # Returns
    /// Vector of `Option<TeleologicalFingerprint>` (same order as input).
    /// `None` entries indicate IDs not found or soft-deleted.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn retrieve_batch(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<TeleologicalFingerprint>>>;

    // ==================== Statistics ====================

    /// Get the total number of stored fingerprints.
    ///
    /// # Returns
    /// Count of all fingerprints (excludes soft-deleted by default).
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn count(&self) -> CoreResult<usize>;

    /// Get total storage size in bytes.
    ///
    /// Returns the approximate heap memory used by the store.
    /// For persistent backends, this is the in-memory cache size.
    fn storage_size_bytes(&self) -> usize;

    /// Get the storage backend type.
    ///
    /// Returns the enum variant identifying this implementation.
    fn backend_type(&self) -> TeleologicalStorageBackend;

    // ==================== Persistence ====================

    /// Flush all pending writes to durable storage.
    ///
    /// For in-memory stores, this is a no-op.
    /// For persistent stores, ensures all data is written to disk.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Flush failure
    async fn flush(&self) -> CoreResult<()>;

    /// Create a checkpoint of the current store state.
    ///
    /// Returns the path to the checkpoint directory/file.
    /// Checkpoints enable point-in-time recovery.
    ///
    /// # Returns
    /// Path to the created checkpoint.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Checkpoint creation failure
    async fn checkpoint(&self) -> CoreResult<PathBuf>;

    /// Restore store state from a checkpoint.
    ///
    /// Replaces current state with checkpoint data.
    /// **WARNING**: Destructive operation - current data is lost.
    ///
    /// # Arguments
    /// * `checkpoint_path` - Path to the checkpoint to restore
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Restore failure
    /// - `CoreError::ConfigError` - Invalid checkpoint path
    async fn restore(&self, checkpoint_path: &Path) -> CoreResult<()>;

    /// Compact the storage to reclaim space.
    ///
    /// Removes soft-deleted entries and defragments storage.
    /// For RocksDB, triggers manual compaction.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Compaction failure
    async fn compact(&self) -> CoreResult<()>;

    // ==================== Content Storage (TASK-CONTENT-003) ====================
    // See `defaults.rs` for detailed documentation on default implementations.

    /// Store content text associated with a fingerprint.
    /// Default: Returns unsupported error. Override for content-capable backends.
    async fn store_content(&self, id: Uuid, content: &str) -> CoreResult<()>;

    /// Retrieve content text for a fingerprint.
    /// Default: Returns None. Override for content-capable backends.
    async fn get_content(&self, id: Uuid) -> CoreResult<Option<String>>;

    /// Delete content for a fingerprint.
    /// Default: Returns false. Override for content-capable backends.
    async fn delete_content(&self, id: Uuid) -> CoreResult<bool>;

    /// Batch retrieve content for multiple fingerprints.
    /// Default: Returns vec of None. Override for batch-optimized retrieval.
    async fn get_content_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<String>>>;

    // ==================== Source Metadata Storage ====================
    // Enables tracking memory provenance (e.g., file path for MDFileChunk)
    // See `defaults.rs` for default implementations.

    /// Store source metadata associated with a fingerprint.
    ///
    /// Source metadata provides provenance tracking, enabling context injection
    /// to display where memories originated from (e.g., file paths for chunked
    /// markdown files).
    ///
    /// # Arguments
    /// * `id` - The fingerprint UUID
    /// * `metadata` - Source metadata to store
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::SerializationError` - Serialization failure
    async fn store_source_metadata(&self, id: Uuid, metadata: &SourceMetadata) -> CoreResult<()>;

    /// Retrieve source metadata for a fingerprint.
    ///
    /// # Arguments
    /// * `id` - The fingerprint UUID
    ///
    /// # Returns
    /// `Some(metadata)` if found, `None` if no metadata stored.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::SerializationError` - Deserialization failure
    async fn get_source_metadata(&self, id: Uuid) -> CoreResult<Option<SourceMetadata>>;

    /// Delete source metadata for a fingerprint.
    ///
    /// # Arguments
    /// * `id` - The fingerprint UUID
    ///
    /// # Returns
    /// `true` if metadata was deleted, `false` if not found.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn delete_source_metadata(&self, id: Uuid) -> CoreResult<bool>;

    /// Batch retrieve source metadata for multiple fingerprints.
    ///
    /// # Arguments
    /// * `ids` - Slice of fingerprint UUIDs
    ///
    /// # Returns
    /// Vector of `Option<SourceMetadata>` (same order as input).
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn get_source_metadata_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<SourceMetadata>>>;

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
    /// * `Ok(Vec<Uuid>)` - UUIDs of matching fingerprints (may be empty)
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn find_fingerprints_by_file_path(&self, file_path: &str) -> CoreResult<Vec<Uuid>>;

    // ==================== File Index Storage ====================
    // Enables O(1) lookup of fingerprints by file path for file watcher management.
    // See `defaults.rs` for default implementations.

    /// List all files that have embeddings in the knowledge graph.
    ///
    /// Returns entries for all files tracked in the file index.
    ///
    /// # Returns
    /// Vector of FileIndexEntry for all indexed files.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn list_indexed_files(&self) -> CoreResult<Vec<crate::types::FileIndexEntry>>;

    /// Get all fingerprint IDs for a specific file path (O(1) via index).
    ///
    /// # Arguments
    /// * `file_path` - The file path to look up
    ///
    /// # Returns
    /// Vector of fingerprint UUIDs for the file, empty if not found.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn get_fingerprints_for_file(&self, file_path: &str) -> CoreResult<Vec<Uuid>>;

    /// Add fingerprint ID to file index (called on MDFileChunk store).
    ///
    /// Creates the index entry if it doesn't exist, or adds to existing entry.
    ///
    /// # Arguments
    /// * `file_path` - The file path
    /// * `fingerprint_id` - The fingerprint UUID to add
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn index_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<()>;

    /// Remove fingerprint ID from file index (called on delete).
    ///
    /// If the entry becomes empty after removal, deletes the entire entry.
    ///
    /// # Arguments
    /// * `file_path` - The file path
    /// * `fingerprint_id` - The fingerprint UUID to remove
    ///
    /// # Returns
    /// true if the fingerprint was found and removed, false otherwise.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn unindex_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<bool>;

    /// Clear all fingerprints for a file from index (bulk delete).
    ///
    /// # Arguments
    /// * `file_path` - The file path to clear
    ///
    /// # Returns
    /// Number of fingerprints that were in the index before clearing.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn clear_file_index(&self, file_path: &str) -> CoreResult<usize>;

    /// Get statistics about file watcher content.
    ///
    /// # Returns
    /// FileWatcherStats with aggregated information.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn get_file_watcher_stats(&self) -> CoreResult<crate::types::FileWatcherStats>;

    // ==================== Topic Portfolio Persistence ====================
    // Enables session-to-session topic continuity per PRD Section 9.1.
    // See `defaults.rs` for default implementations.

    /// Persist topic portfolio for a session.
    ///
    /// Called by SessionEnd hook to save discovered topics. Stores both
    /// under the session_id key and as "__latest__" for cross-session restoration.
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    /// * `portfolio` - The topic portfolio to persist
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::SerializationError` - Serialization failure
    async fn persist_topic_portfolio(
        &self,
        session_id: &str,
        portfolio: &crate::clustering::PersistedTopicPortfolio,
    ) -> CoreResult<()>;

    /// Load topic portfolio for a specific session.
    ///
    /// Called by SessionStart hook to restore topics from a previous session.
    ///
    /// # Arguments
    /// * `session_id` - The session identifier to load
    ///
    /// # Returns
    /// `Some(portfolio)` if found, `None` if no portfolio for this session.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::SerializationError` - Deserialization failure
    async fn load_topic_portfolio(
        &self,
        session_id: &str,
    ) -> CoreResult<Option<crate::clustering::PersistedTopicPortfolio>>;

    /// Load the most recent topic portfolio across all sessions.
    ///
    /// Fallback when no specific session portfolio is available. Uses the
    /// "__latest__" sentinel key.
    ///
    /// # Returns
    /// `Some(portfolio)` if any portfolio exists, `None` otherwise.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::SerializationError` - Deserialization failure
    async fn load_latest_topic_portfolio(
        &self,
    ) -> CoreResult<Option<crate::clustering::PersistedTopicPortfolio>>;

    // =========================================================================
    // Clustering Support
    // =========================================================================

    /// Scan all fingerprints and return their embeddings for clustering.
    ///
    /// This method is used by detect_topics to populate the cluster_manager
    /// with all existing fingerprints from storage before running HDBSCAN.
    ///
    /// Returns a vector of (fingerprint_id, embeddings_array) tuples.
    /// The embeddings_array is the 13-element array of embedding vectors.
    ///
    /// # Arguments
    /// * `limit` - Optional limit on number of fingerprints to scan (None = all)
    ///
    /// # Returns
    /// Vector of (Uuid, [Vec<f32>; 13]) tuples for each fingerprint.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    /// - `CoreError::SerializationError` - Deserialization failure
    async fn scan_fingerprints_for_clustering(
        &self,
        limit: Option<usize>,
    ) -> CoreResult<Vec<(uuid::Uuid, [Vec<f32>; 13])>>;
}
