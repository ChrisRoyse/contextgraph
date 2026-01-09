//! TeleologicalMemoryStore trait for 5-stage teleological retrieval.
//!
//! This module defines the core storage trait for the Context Graph system's
//! teleological memory architecture. It supports:
//! - CRUD operations for TeleologicalFingerprint
//! - Multi-space semantic search (13 embedding spaces)
//! - Purpose vector alignment search
//! - Sparse (SPLADE) search for Stage 1 recall
//! - Batch operations for efficiency
//! - Persistence and checkpointing
//!
//! # 5-Stage Retrieval Pipeline Support
//!
//! | Stage | Name | Method |
//! |-------|------|--------|
//! | 1 | Recall | `search_sparse()` - E13 SPLADE inverted index |
//! | 2 | Semantic | `search_semantic()` - E1 Matryoshka 128D ANN |
//! | 3 | Precision | `search_semantic()` - Full E1-E12 dense embeddings |
//! | 4 | Rerank | External - E12 ColBERT late interaction |
//! | 5 | Teleological | `search_purpose()` - 13D purpose vector alignment |
//!
//! # Design Philosophy
//!
//! - **NO BACKWARDS COMPATIBILITY**: Old MemoryStore trait deleted
//! - **FAIL FAST**: All errors return `CoreError` variants with context
//! - **UNIT TESTS**: Tests use stub `InMemoryTeleologicalStore` (not persistent RocksDB)
//! - **13 EMBEDDERS**: Full E1-E13 semantic fingerprint support

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::constants::alignment;
use crate::error::CoreResult;
use crate::types::fingerprint::{
    PurposeVector, SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};

/// Storage backend types for teleological memory stores.
///
/// This enum identifies the underlying storage implementation,
/// enabling runtime introspection and backend-specific optimizations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TeleologicalStorageBackend {
    /// In-memory storage (HashMap-based, no persistence).
    /// Used for testing and development.
    InMemory,

    /// RocksDB storage with 8 column families.
    /// Production storage with full persistence and indexing.
    RocksDb,

    /// TimescaleDB storage for time-series evolution data.
    /// Used for purpose evolution archival.
    TimescaleDb,

    /// Hybrid storage combining RocksDB + TimescaleDB.
    /// Full production deployment.
    Hybrid,
}

impl std::fmt::Display for TeleologicalStorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InMemory => write!(f, "InMemory"),
            Self::RocksDb => write!(f, "RocksDB"),
            Self::TimescaleDb => write!(f, "TimescaleDB"),
            Self::Hybrid => write!(f, "Hybrid (RocksDB + TimescaleDB)"),
        }
    }
}

/// Search options for teleological memory queries.
///
/// Controls filtering, pagination, and result formatting for
/// semantic and purpose-based searches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalSearchOptions {
    /// Maximum number of results to return.
    /// Default: 10, Max: 1000
    pub top_k: usize,

    /// Minimum similarity threshold [0.0, 1.0].
    /// Results below this threshold are filtered out.
    /// Default: 0.0 (no filtering)
    pub min_similarity: f32,

    /// Include soft-deleted items in results.
    /// Default: false
    pub include_deleted: bool,

    /// Filter by dominant Johari quadrant.
    /// None = no filtering.
    pub johari_quadrant_filter: Option<usize>,

    /// Filter by minimum alignment to North Star.
    /// None = no filtering.
    pub min_alignment: Option<f32>,

    /// Embedder indices to use for search (0-12).
    /// Empty = use all embedders.
    pub embedder_indices: Vec<usize>,
}

impl Default for TeleologicalSearchOptions {
    fn default() -> Self {
        Self {
            top_k: 10,
            min_similarity: 0.0,
            include_deleted: false,
            johari_quadrant_filter: None,
            min_alignment: None,
            embedder_indices: Vec::new(),
        }
    }
}

impl TeleologicalSearchOptions {
    /// Create options for a quick top-k search.
    #[inline]
    pub fn quick(top_k: usize) -> Self {
        Self {
            top_k,
            ..Default::default()
        }
    }

    /// Create options with minimum similarity threshold.
    #[inline]
    pub fn with_min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = threshold;
        self
    }

    /// Create options with alignment filter.
    #[inline]
    pub fn with_min_alignment(mut self, threshold: f32) -> Self {
        self.min_alignment = Some(threshold);
        self
    }

    /// Create options filtering by specific embedders.
    #[inline]
    pub fn with_embedders(mut self, indices: Vec<usize>) -> Self {
        self.embedder_indices = indices;
        self
    }
}

/// Search result from teleological memory queries.
///
/// Contains the matched fingerprint along with scoring metadata
/// for ranking and analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalSearchResult {
    /// The matched teleological fingerprint.
    pub fingerprint: TeleologicalFingerprint,

    /// Overall similarity score [0.0, 1.0].
    /// Computed differently depending on search type.
    pub similarity: f32,

    /// Per-embedder similarity scores (13 values for E1-E13).
    /// Sparse embeddings (E6, E13) use sparse dot product.
    pub embedder_scores: [f32; 13],

    /// Purpose alignment score (cosine similarity of purpose vectors).
    pub purpose_alignment: f32,

    /// Stage scores from the 5-stage retrieval pipeline.
    /// [sparse_recall, semantic_ann, precision, rerank, teleological]
    pub stage_scores: [f32; 5],
}

impl TeleologicalSearchResult {
    /// Create a new search result with computed scores.
    pub fn new(
        fingerprint: TeleologicalFingerprint,
        similarity: f32,
        embedder_scores: [f32; 13],
        purpose_alignment: f32,
    ) -> Self {
        Self {
            fingerprint,
            similarity,
            embedder_scores,
            purpose_alignment,
            stage_scores: [0.0; 5], // Populated by pipeline stages
        }
    }

    /// Get the dominant embedder (highest score).
    pub fn dominant_embedder(&self) -> usize {
        self.embedder_scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

/// Core trait for teleological memory storage operations.
///
/// This trait defines the complete interface for storing, retrieving,
/// and searching TeleologicalFingerprints. Implementations must support:
/// - Full CRUD operations with soft/hard delete
/// - Multi-space semantic search
/// - Purpose vector alignment search
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

    /// Search by purpose vector alignment.
    ///
    /// Finds fingerprints with similar purpose alignment to the North Star goal.
    /// Used in Stage 5 (Teleological) of the retrieval pipeline.
    ///
    /// # Arguments
    /// * `query` - The purpose vector to match
    /// * `options` - Search options (top_k, filters, etc.)
    ///
    /// # Returns
    /// Vector of search results sorted by purpose alignment (descending).
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn search_purpose(
        &self,
        query: &PurposeVector,
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

    /// Get fingerprint counts by Johari quadrant.
    ///
    /// Returns counts for each of the 4 quadrants:
    /// [Open, Hidden, Blind, Unknown]
    ///
    /// Classification uses the dominant quadrant of the aggregate
    /// across all 13 embedders.
    ///
    /// # Returns
    /// Array of 4 counts: [open_count, hidden_count, blind_count, unknown_count]
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn count_by_quadrant(&self) -> CoreResult<[usize; 4]>;

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

    // ==================== Scanning Operations ====================

    /// List fingerprints by dominant Johari quadrant.
    ///
    /// This performs a full scan of the store, filtering by the dominant
    /// Johari quadrant aggregated across all 13 embedders.
    ///
    /// # AP-007: PROPER SCANNING
    ///
    /// This method exists to support quadrant-based queries WITHOUT using
    /// zero-magnitude semantic queries (which have undefined cosine similarity).
    ///
    /// # Arguments
    /// * `quadrant` - Johari quadrant index (0=Open, 1=Hidden, 2=Blind, 3=Unknown)
    /// * `limit` - Maximum results to return
    ///
    /// # Returns
    /// Vector of (MemoryId, JohariFingerprint) pairs matching the quadrant.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn list_by_quadrant(
        &self,
        quadrant: usize,
        limit: usize,
    ) -> CoreResult<Vec<(Uuid, crate::types::fingerprint::JohariFingerprint)>>;

    /// List all fingerprints with their Johari state.
    ///
    /// # AP-007: PROPER SCANNING
    ///
    /// This method exists to support pattern-based quadrant queries that need
    /// to inspect per-embedder quadrant state. It replaces the broken pattern
    /// of using zeroed semantic queries for "get all" operations.
    ///
    /// # Arguments
    /// * `limit` - Maximum results to return
    ///
    /// # Returns
    /// Vector of (MemoryId, JohariFingerprint) pairs.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - Storage backend failure
    async fn list_all_johari(
        &self,
        limit: usize,
    ) -> CoreResult<Vec<(Uuid, crate::types::fingerprint::JohariFingerprint)>>;
}

/// Extension trait for convenient TeleologicalMemoryStore operations.
///
/// Provides helper methods built on top of the core trait.
#[async_trait]
pub trait TeleologicalMemoryStoreExt: TeleologicalMemoryStore {
    /// Check if a fingerprint exists by ID.
    async fn exists(&self, id: Uuid) -> CoreResult<bool> {
        Ok(self.retrieve(id).await?.is_some())
    }

    /// Validate a fingerprint before storage.
    ///
    /// Performs comprehensive validation of the TeleologicalFingerprint:
    /// - Validates all 13 embedder dimensions in the SemanticFingerprint
    /// - Validates sparse vector vocabulary bounds (E6, E13)
    /// - Validates ColBERT token dimensions (E12)
    ///
    /// # FAIL FAST
    ///
    /// Returns immediately on first validation failure with a detailed error
    /// message. No partial validation or fallback values.
    ///
    /// # Arguments
    ///
    /// * `fingerprint` - The fingerprint to validate
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Fingerprint is valid for storage
    /// * `Err(CoreError::ValidationError)` - Validation failed with details
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::traits::TeleologicalMemoryStoreExt;
    ///
    /// let store = get_store();
    /// let fingerprint = build_fingerprint();
    ///
    /// // Validate before storing - FAIL FAST on invalid data
    /// store.validate_for_storage(&fingerprint)?;
    /// let id = store.store(fingerprint).await?;
    /// ```
    fn validate_for_storage(
        &self,
        fingerprint: &TeleologicalFingerprint,
    ) -> CoreResult<()> {
        fingerprint.semantic.validate().map_err(|msg| {
            crate::error::CoreError::ValidationError {
                field: "semantic".to_string(),
                message: msg,
            }
        })
    }

    /// Get fingerprints with optimal alignment (θ ≥ alignment::OPTIMAL).
    ///
    /// Constitution: `teleological.thresholds.optimal`
    async fn get_optimal_aligned(
        &self,
        top_k: usize,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        let options = TeleologicalSearchOptions::quick(top_k).with_min_alignment(alignment::OPTIMAL);
        let query = PurposeVector::default();
        self.search_purpose(&query, options).await
    }

    /// Get fingerprints with critical misalignment (θ < alignment::CRITICAL).
    ///
    /// Constitution: `teleological.thresholds.critical`
    async fn get_critical_misaligned(&self) -> CoreResult<Vec<TeleologicalFingerprint>> {
        // This requires iteration - implementations may override for efficiency
        let options = TeleologicalSearchOptions::quick(1000);
        let query = PurposeVector::default();
        let results = self.search_purpose(&query, options).await?;

        Ok(results
            .into_iter()
            .filter(|r| r.fingerprint.theta_to_north_star < alignment::CRITICAL)
            .map(|r| r.fingerprint)
            .collect())
    }
}

// Blanket implementation for all TeleologicalMemoryStore implementations
impl<T: TeleologicalMemoryStore> TeleologicalMemoryStoreExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fingerprint::JohariFingerprint;

    #[test]
    fn test_search_options_default() {
        let opts = TeleologicalSearchOptions::default();
        assert_eq!(opts.top_k, 10);
        assert_eq!(opts.min_similarity, 0.0);
        assert!(!opts.include_deleted);
        assert!(opts.johari_quadrant_filter.is_none());
        assert!(opts.min_alignment.is_none());
        assert!(opts.embedder_indices.is_empty());
    }

    #[test]
    fn test_search_options_quick() {
        let opts = TeleologicalSearchOptions::quick(50);
        assert_eq!(opts.top_k, 50);
    }

    #[test]
    fn test_search_options_builder() {
        let opts = TeleologicalSearchOptions::quick(20)
            .with_min_similarity(0.5)
            .with_min_alignment(0.75)
            .with_embedders(vec![0, 1, 2]);

        assert_eq!(opts.top_k, 20);
        assert_eq!(opts.min_similarity, 0.5);
        assert_eq!(opts.min_alignment, Some(0.75));
        assert_eq!(opts.embedder_indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_storage_backend_display() {
        assert_eq!(TeleologicalStorageBackend::InMemory.to_string(), "InMemory");
        assert_eq!(TeleologicalStorageBackend::RocksDb.to_string(), "RocksDB");
        assert_eq!(
            TeleologicalStorageBackend::TimescaleDb.to_string(),
            "TimescaleDB"
        );
        assert_eq!(
            TeleologicalStorageBackend::Hybrid.to_string(),
            "Hybrid (RocksDB + TimescaleDB)"
        );
    }

    #[test]
    fn test_search_result_dominant_embedder() {
        let mut scores = [0.1; 13];
        scores[5] = 0.9; // E6 is dominant

        let result = TeleologicalSearchResult {
            fingerprint: TeleologicalFingerprint::new(
                SemanticFingerprint::zeroed(),
                PurposeVector::default(),
                JohariFingerprint::zeroed(),
                [0u8; 32],
            ),
            similarity: 0.8,
            embedder_scores: scores,
            purpose_alignment: 0.7,
            stage_scores: [0.0; 5],
        };

        assert_eq!(result.dominant_embedder(), 5);
    }
}
