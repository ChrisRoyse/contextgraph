//! MultiSpaceIndexManager trait definition.
//!
//! Manages 12 HNSW indexes + 2 inverted indexes for the 5-stage retrieval pipeline.
//!
//! # Performance Requirements (constitution.yaml)
//!
//! - `add_vector()`: <1ms per index
//! - `search()`: <10ms per index
//! - `search_splade()`: <5ms for Stage 1
//! - `search_matryoshka()`: <10ms for Stage 2
//! - `persist()`: <1s for 100K vectors

use async_trait::async_trait;
use std::path::Path;
use uuid::Uuid;

use crate::types::fingerprint::SemanticFingerprint;
pub use super::config::EmbedderIndex;

use super::error::IndexResult;
use super::status::IndexStatus;

/// Manages 12 HNSW indexes + 2 inverted indexes for 5-stage retrieval.
///
/// # Index Architecture
///
/// | Index Type | Count | Purpose | Stage |
/// |------------|-------|---------|-------|
/// | HNSW | 10 | E1-E5, E7-E11 dense | Stage 3 |
/// | HNSW | 1 | E1 Matryoshka 128D | Stage 2 |
/// | HNSW | 1 | PurposeVector 13D | Stage 4 |
/// | Inverted | 1 | E13 SPLADE | Stage 1 |
/// | MaxSim | 1 | E12 ColBERT | Stage 5 |
///
/// # Error Handling
///
/// - All methods return `Result<T, IndexError>`
/// - Dimension mismatches error immediately
/// - Invalid embedder operations error immediately
/// - NO fallbacks or silent failures
///
/// # Example
///
/// ```ignore
/// let mut manager = HnswMultiSpaceIndex::new();
/// manager.initialize().await?;
///
/// // Add vector to E1 semantic index
/// manager.add_vector(EmbedderIndex::E1Semantic, memory_id, &vector).await?;
///
/// // Search E1 semantic index
/// let results = manager.search(EmbedderIndex::E1Semantic, &query, 10).await?;
/// ```
#[async_trait]
pub trait MultiSpaceIndexManager: Send + Sync {
    /// Initialize all 12 HNSW indexes + 2 inverted indexes.
    ///
    /// Must be called before any other operations.
    ///
    /// # Errors
    ///
    /// Returns `IndexError::StorageError` if initialization fails.
    async fn initialize(&mut self) -> IndexResult<()>;

    /// Add vector to specific HNSW index.
    ///
    /// # Arguments
    ///
    /// - `embedder`: Target embedder index (must use HNSW)
    /// - `memory_id`: UUID of the memory being indexed
    /// - `vector`: Dense vector matching embedder dimension
    ///
    /// # Errors
    ///
    /// - `IndexError::InvalidEmbedder`: If embedder doesn't use HNSW (E6, E12, E13)
    /// - `IndexError::DimensionMismatch`: If vector dimension doesn't match
    /// - `IndexError::NotInitialized`: If initialize() hasn't been called
    /// - `IndexError::ZeroNormVector`: If vector has zero magnitude
    async fn add_vector(
        &mut self,
        embedder: EmbedderIndex,
        memory_id: Uuid,
        vector: &[f32],
    ) -> IndexResult<()>;

    /// Add all embeddings from SemanticFingerprint to respective indexes.
    ///
    /// Automatically routes each embedding to its corresponding index:
    /// - Dense embeddings (E1-E5, E7-E11) → HNSW indexes
    /// - E6 sparse → E6 inverted index (legacy)
    /// - E12 tokens → ColBERT MaxSim index
    /// - E13 SPLADE → SPLADE inverted index
    ///
    /// Also adds:
    /// - E1[..128] → Matryoshka 128D HNSW
    ///
    /// # Arguments
    ///
    /// - `memory_id`: UUID of the memory
    /// - `fingerprint`: Complete semantic fingerprint
    ///
    /// # Errors
    ///
    /// Returns first error encountered during indexing.
    async fn add_fingerprint(
        &mut self,
        memory_id: Uuid,
        fingerprint: &SemanticFingerprint,
    ) -> IndexResult<()>;

    /// Add purpose vector to the 13D HNSW index.
    ///
    /// # Arguments
    ///
    /// - `memory_id`: UUID of the memory
    /// - `purpose`: 13D teleological alignment vector
    ///
    /// # Errors
    ///
    /// - `IndexError::DimensionMismatch`: If vector is not 13D
    async fn add_purpose_vector(&mut self, memory_id: Uuid, purpose: &[f32]) -> IndexResult<()>;

    /// Add to SPLADE inverted index (E13).
    ///
    /// # Arguments
    ///
    /// - `memory_id`: UUID of the memory
    /// - `sparse`: Sparse vector as (term_id, weight) pairs
    ///
    /// # Errors
    ///
    /// - `IndexError::InvalidTermId`: If term_id >= vocab_size (30522)
    /// - `IndexError::ZeroNormVector`: If all weights are zero
    async fn add_splade(&mut self, memory_id: Uuid, sparse: &[(usize, f32)]) -> IndexResult<()>;

    /// Search single HNSW index.
    ///
    /// # Arguments
    ///
    /// - `embedder`: Target embedder index (must use HNSW)
    /// - `query`: Query vector matching embedder dimension
    /// - `k`: Number of results to return
    ///
    /// # Returns
    ///
    /// Vec of (memory_id, similarity_score) pairs, sorted by descending similarity.
    ///
    /// # Errors
    ///
    /// - `IndexError::InvalidEmbedder`: If embedder doesn't use HNSW
    /// - `IndexError::DimensionMismatch`: If query dimension doesn't match
    async fn search(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>>;

    /// Stage 1: SPLADE sparse retrieval (BM25+SPLADE hybrid).
    ///
    /// # Arguments
    ///
    /// - `sparse_query`: Sparse query vector as (term_id, weight) pairs
    /// - `k`: Number of candidates to retrieve
    ///
    /// # Returns
    ///
    /// Vec of (memory_id, score) pairs, sorted by descending BM25 score.
    /// Target: <5ms for 10K candidates.
    async fn search_splade(
        &self,
        sparse_query: &[(usize, f32)],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>>;

    /// Stage 2: Matryoshka 128D fast filtering.
    ///
    /// Uses E1 truncated to 128D for fast ANN filtering.
    ///
    /// # Arguments
    ///
    /// - `query_128d`: 128D query vector (E1 semantic truncated)
    /// - `k`: Number of candidates to retrieve
    ///
    /// # Returns
    ///
    /// Vec of (memory_id, similarity) pairs, sorted by descending similarity.
    /// Target: <10ms for 1K candidates.
    async fn search_matryoshka(
        &self,
        query_128d: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>>;

    /// Stage 4: Purpose vector teleological filter.
    ///
    /// # Arguments
    ///
    /// - `purpose_query`: 13D purpose vector query
    /// - `k`: Number of candidates to retrieve
    ///
    /// # Returns
    ///
    /// Vec of (memory_id, alignment) pairs, sorted by descending alignment.
    /// Target: <10ms.
    async fn search_purpose(
        &self,
        purpose_query: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>>;

    /// Remove memory from all indexes.
    ///
    /// # Arguments
    ///
    /// - `memory_id`: UUID of memory to remove
    ///
    /// # Errors
    ///
    /// - `IndexError::NotFound`: If memory not in any index (warning, not fatal)
    async fn remove(&mut self, memory_id: Uuid) -> IndexResult<()>;

    /// Get status of all indexes.
    ///
    /// Returns status for all 14 indexes (12 HNSW + 2 inverted).
    fn status(&self) -> Vec<IndexStatus>;

    /// Persist all indexes to disk.
    ///
    /// # Arguments
    ///
    /// - `path`: Directory to write index files
    ///
    /// # Errors
    ///
    /// - `IndexError::IoError`: If file operations fail
    /// - `IndexError::SerializationError`: If serialization fails
    ///
    /// Target: <1s for 100K vectors.
    async fn persist(&self, path: &Path) -> IndexResult<()>;

    /// Load all indexes from disk.
    ///
    /// # Arguments
    ///
    /// - `path`: Directory containing index files
    ///
    /// # Errors
    ///
    /// - `IndexError::IoError`: If file operations fail
    /// - `IndexError::CorruptedIndex`: If index files are corrupted
    async fn load(&mut self, path: &Path) -> IndexResult<()>;
}
