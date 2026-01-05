//! Multi-embedding query executor trait.
//!
//! This module defines the `MultiEmbeddingQueryExecutor` trait for
//! searching across all 13 embedding spaces in parallel.
//!
//! # Performance Targets (constitution.yaml)
//! - Total latency: <60ms @ 1M memories
//! - Query embedding: <30ms
//!
//! # Thread Safety
//! Required to be `Send + Sync` for concurrent query execution.

use async_trait::async_trait;

use crate::error::CoreResult;
use crate::types::fingerprint::SemanticFingerprint;

use super::{EmbeddingSpaceMask, MultiEmbeddingQuery, MultiEmbeddingResult};

/// Multi-embedding query executor trait.
///
/// Executes queries across 13 embedding spaces in parallel,
/// aggregating results using the configured strategy (default: RRF).
///
/// # Performance Targets (constitution.yaml)
/// - Total latency: <60ms @ 1M memories
/// - Query embedding: <30ms
///
/// # Thread Safety
/// Required to be `Send + Sync` for concurrent query execution.
///
/// # Fail-Fast Behavior
/// All methods return `CoreError` on failure with detailed context.
/// No silent failures or fallback to default values.
#[async_trait]
pub trait MultiEmbeddingQueryExecutor: Send + Sync {
    /// Execute a multi-embedding query.
    ///
    /// # Arguments
    /// - query: Query configuration (validated internally)
    ///
    /// # Returns
    /// `MultiEmbeddingResult` with ranked results and timing
    ///
    /// # Errors
    /// - `CoreError::ValidationError` - Invalid query parameters
    /// - `CoreError::Embedding` - Embedding generation failed
    /// - `CoreError::IndexError` - HNSW/index search failed
    /// - `CoreError::StorageError` - Storage backend failure
    ///
    /// # Example
    /// ```ignore
    /// let query = MultiEmbeddingQuery {
    ///     query_text: "How does memory consolidation work?".to_string(),
    ///     active_spaces: EmbeddingSpaceMask::ALL,
    ///     final_limit: 10,
    ///     ..Default::default()
    /// };
    /// let result = executor.execute(query).await?;
    /// assert!(result.total_time.as_millis() < 60);
    /// ```
    async fn execute(&self, query: MultiEmbeddingQuery) -> CoreResult<MultiEmbeddingResult>;

    /// Execute with pre-computed query embeddings (skip embedding step).
    ///
    /// Use when embeddings are already available (e.g., from cache).
    ///
    /// # Arguments
    /// - embeddings: Pre-computed 13-embedding fingerprint
    /// - query: Query configuration (query_text ignored)
    ///
    /// # Errors
    /// Same as `execute()` except no `CoreError::Embedding`
    async fn execute_with_embeddings(
        &self,
        embeddings: &SemanticFingerprint,
        query: MultiEmbeddingQuery,
    ) -> CoreResult<MultiEmbeddingResult>;

    /// Get information about available embedding spaces.
    ///
    /// Returns status for all 13 spaces including:
    /// - Whether index is loaded
    /// - Index size (number of vectors)
    /// - Dimension
    fn available_spaces(&self) -> Vec<SpaceInfo>;

    /// Warm up specific spaces by pre-loading indexes.
    ///
    /// Call before queries for predictable latency.
    ///
    /// # Errors
    /// - `CoreError::IndexError` - Failed to load index
    async fn warm_up(&self, spaces: EmbeddingSpaceMask) -> CoreResult<()>;

    /// Execute 5-stage pipeline query.
    ///
    /// Full pipeline with all stages:
    /// 1. SPLADE sparse recall
    /// 2. Matryoshka 128D filtering
    /// 3. Full 13-space HNSW search
    /// 4. Teleological alignment filter
    /// 5. Late interaction reranking
    ///
    /// # Performance Target
    /// <60ms total @ 1M memories
    async fn execute_pipeline(&self, query: MultiEmbeddingQuery)
        -> CoreResult<MultiEmbeddingResult>;
}

/// Information about a single embedding space.
#[derive(Clone, Debug)]
pub struct SpaceInfo {
    /// Space index (0-12).
    pub index: usize,

    /// Space name (e.g., "E1_Semantic").
    pub name: &'static str,

    /// Embedding dimension (0 for sparse spaces E6, E13).
    pub dimension: usize,

    /// Number of vectors in index.
    pub index_size: usize,

    /// Whether index is loaded in memory.
    pub is_loaded: bool,

    /// Index type (HNSW, Inverted, etc.).
    pub index_type: IndexType,
}

impl SpaceInfo {
    /// Create new space info.
    pub fn new(
        index: usize,
        dimension: usize,
        index_size: usize,
        is_loaded: bool,
        index_type: IndexType,
    ) -> Self {
        Self {
            index,
            name: EmbeddingSpaceMask::space_name(index),
            dimension,
            index_size,
            is_loaded,
            index_type,
        }
    }

    /// Create space info for a dense HNSW space.
    pub fn dense_hnsw(index: usize, dimension: usize, index_size: usize, is_loaded: bool) -> Self {
        Self::new(index, dimension, index_size, is_loaded, IndexType::Hnsw)
    }

    /// Create space info for a sparse inverted index space.
    pub fn sparse_inverted(index: usize, index_size: usize, is_loaded: bool) -> Self {
        Self::new(index, 0, index_size, is_loaded, IndexType::Inverted)
    }
}

/// Type of index used for a space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexType {
    /// HNSW for dense vectors.
    Hnsw,
    /// Inverted index for sparse vectors (E6, E13).
    Inverted,
    /// No index (linear scan).
    None,
}

impl std::fmt::Display for IndexType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexType::Hnsw => write!(f, "HNSW"),
            IndexType::Inverted => write!(f, "Inverted"),
            IndexType::None => write!(f, "None"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_space_info_dense() {
        let info = SpaceInfo::dense_hnsw(0, 1024, 1000000, true);

        assert_eq!(info.index, 0);
        assert_eq!(info.name, "E1_Semantic");
        assert_eq!(info.dimension, 1024);
        assert_eq!(info.index_size, 1000000);
        assert!(info.is_loaded);
        assert_eq!(info.index_type, IndexType::Hnsw);

        println!("[VERIFIED] SpaceInfo::dense_hnsw");
    }

    #[test]
    fn test_space_info_sparse() {
        let info = SpaceInfo::sparse_inverted(12, 500000, true);

        assert_eq!(info.index, 12);
        assert_eq!(info.name, "E13_SPLADE");
        assert_eq!(info.dimension, 0);
        assert_eq!(info.index_type, IndexType::Inverted);

        println!("[VERIFIED] SpaceInfo::sparse_inverted");
    }

    #[test]
    fn test_index_type_display() {
        assert_eq!(format!("{}", IndexType::Hnsw), "HNSW");
        assert_eq!(format!("{}", IndexType::Inverted), "Inverted");
        assert_eq!(format!("{}", IndexType::None), "None");

        println!("[VERIFIED] IndexType Display");
    }
}
