//! Index error types with fail-fast semantics.
//!
//! # CRITICAL: NO FALLBACKS
//!
//! All errors are fatal and must be handled explicitly.
//! Invalid configurations or operations panic with detailed context.

use thiserror::Error;
use uuid::Uuid;

/// Re-export EmbedderIndex for error context
pub use super::config::EmbedderIndex;

/// Index operation errors with detailed context for debugging.
///
/// # Error Handling Philosophy
///
/// - **Dimension errors**: Immediate rejection, no silent truncation
/// - **Invalid embedders**: Panic for HNSW ops on non-HNSW types
/// - **Storage errors**: Propagate with full context
/// - **Corrupted state**: Panic with recovery instructions
#[derive(Error, Debug)]
pub enum IndexError {
    /// Dimension mismatch between vector and index configuration.
    ///
    /// Triggered when `add_vector()` receives a vector with wrong dimension.
    #[error("INDEX ERROR: Dimension mismatch for {embedder:?} - expected {expected}, got {actual}")]
    DimensionMismatch {
        /// The embedder being targeted
        embedder: EmbedderIndex,
        /// Expected dimension from configuration
        expected: usize,
        /// Actual dimension received
        actual: usize,
    },

    /// Attempted HNSW operation on non-HNSW embedder (E6, E12, E13).
    ///
    /// Call `EmbedderIndex::uses_hnsw()` before HNSW operations.
    #[error("INDEX ERROR: {embedder:?} does not use HNSW indexing")]
    InvalidEmbedder {
        /// The embedder that doesn't support HNSW
        embedder: EmbedderIndex,
    },

    /// Index not initialized before use.
    ///
    /// Call `initialize()` before any index operations.
    #[error("INDEX ERROR: Index for {embedder:?} not initialized")]
    NotInitialized {
        /// The uninitialized embedder
        embedder: EmbedderIndex,
    },

    /// Invalid term ID in SPLADE sparse vector.
    ///
    /// Term IDs must be in range [0, vocab_size).
    #[error("INDEX ERROR: Invalid term_id {term_id} (vocab_size={vocab_size})")]
    InvalidTermId {
        /// The invalid term ID
        term_id: usize,
        /// Maximum valid term ID + 1
        vocab_size: usize,
    },

    /// Zero-norm vector cannot be indexed.
    ///
    /// Vectors must have non-zero magnitude for similarity computation.
    #[error("INDEX ERROR: Zero-norm vector for memory {memory_id}")]
    ZeroNormVector {
        /// Memory ID with zero-norm vector
        memory_id: Uuid,
    },

    /// Memory not found in index.
    ///
    /// Attempted operation on non-existent memory.
    #[error("INDEX ERROR: Memory {memory_id} not found in indexes")]
    NotFound {
        /// The missing memory ID
        memory_id: Uuid,
    },

    /// Storage operation failed.
    ///
    /// Wraps underlying storage errors with operation context.
    #[error("INDEX ERROR: Storage operation failed - {context}: {message}")]
    StorageError {
        /// What operation was attempted
        context: String,
        /// Underlying error message
        message: String,
    },

    /// Index file corrupted or incompatible version.
    ///
    /// Requires full index rebuild from primary storage.
    #[error("INDEX ERROR: Corrupted index file at {path}")]
    CorruptedIndex {
        /// Path to corrupted file
        path: String,
    },

    /// IO operation failed during persist/load.
    #[error("INDEX ERROR: IO error - {context}: {message}")]
    IoError {
        /// What operation was attempted
        context: String,
        /// Underlying error message
        message: String,
    },

    /// Serialization/deserialization failed.
    #[error("INDEX ERROR: Serialization error - {context}: {message}")]
    SerializationError {
        /// What was being serialized
        context: String,
        /// Underlying error message
        message: String,
    },
}

impl IndexError {
    /// Create a storage error with context.
    pub fn storage(context: impl Into<String>, error: impl std::fmt::Display) -> Self {
        Self::StorageError {
            context: context.into(),
            message: error.to_string(),
        }
    }

    /// Create an IO error with context.
    pub fn io(context: impl Into<String>, error: impl std::fmt::Display) -> Self {
        Self::IoError {
            context: context.into(),
            message: error.to_string(),
        }
    }

    /// Create a serialization error with context.
    pub fn serialization(context: impl Into<String>, error: impl std::fmt::Display) -> Self {
        Self::SerializationError {
            context: context.into(),
            message: error.to_string(),
        }
    }
}

/// Result type for index operations.
pub type IndexResult<T> = Result<T, IndexError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_mismatch_error_message() {
        let err = IndexError::DimensionMismatch {
            embedder: EmbedderIndex::E1Semantic,
            expected: 1024,
            actual: 512,
        };
        let msg = err.to_string();
        assert!(msg.contains("E1Semantic"));
        assert!(msg.contains("1024"));
        assert!(msg.contains("512"));
        println!("[VERIFIED] DimensionMismatch error format: {}", msg);
    }

    #[test]
    fn test_invalid_embedder_error_message() {
        let err = IndexError::InvalidEmbedder {
            embedder: EmbedderIndex::E6Sparse,
        };
        let msg = err.to_string();
        assert!(msg.contains("E6Sparse"));
        assert!(msg.contains("does not use HNSW"));
        println!("[VERIFIED] InvalidEmbedder error format: {}", msg);
    }

    #[test]
    fn test_invalid_term_id_error_message() {
        let err = IndexError::InvalidTermId {
            term_id: 40000,
            vocab_size: 30522,
        };
        let msg = err.to_string();
        assert!(msg.contains("40000"));
        assert!(msg.contains("30522"));
        println!("[VERIFIED] InvalidTermId error format: {}", msg);
    }

    #[test]
    fn test_zero_norm_error_message() {
        let id = Uuid::new_v4();
        let err = IndexError::ZeroNormVector { memory_id: id };
        let msg = err.to_string();
        assert!(msg.contains(&id.to_string()));
        println!("[VERIFIED] ZeroNormVector error format: {}", msg);
    }

    #[test]
    fn test_storage_error_helper() {
        let err = IndexError::storage("writing index", "disk full");
        let msg = err.to_string();
        assert!(msg.contains("writing index"));
        assert!(msg.contains("disk full"));
        println!("[VERIFIED] StorageError helper: {}", msg);
    }
}
