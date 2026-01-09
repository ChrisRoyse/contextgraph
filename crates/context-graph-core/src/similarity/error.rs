//! Error types for cross-space similarity computation.
//!
//! This module provides the `SimilarityError` enum with explicit error types
//! for all failure modes. Per constitution.yaml, there are NO fallbacks or
//! workarounds - all errors must be explicit.

use thiserror::Error;

/// Errors from cross-space similarity computation.
///
/// # Design Philosophy
///
/// Per constitution.yaml, all errors are explicit:
/// - **No silent truncation**: Dimension mismatches always error
/// - **No substitution**: Zero norm vectors always error
/// - **No guessing**: Insufficient spaces always error
/// - **No partial results**: RRF failures always error
///
/// # Example
///
/// ```rust,ignore
/// match result {
///     Err(SimilarityError::InsufficientSpaces { required, found }) => {
///         log::warn!("Need {} spaces, only {} active", required, found);
///     }
///     Err(SimilarityError::DimensionMismatch { embedder, expected, actual }) => {
///         log::error!("E{} dimension wrong: {} vs {}", embedder + 1, expected, actual);
///     }
///     Err(e) => log::error!("Similarity error: {}", e),
///     Ok(sim) => println!("Score: {:.4}", sim.score),
/// }
/// ```
#[derive(Debug, Error)]
pub enum SimilarityError {
    /// Fewer than required embedding spaces are active.
    ///
    /// This occurs when:
    /// - Fingerprints have unpopulated embeddings
    /// - `config.min_active_spaces` is set higher than available spaces
    /// - `MissingSpaceHandling::RequireAll` is set but spaces are missing
    #[error("Insufficient active spaces: required {required}, found {found}")]
    InsufficientSpaces {
        /// Minimum number of spaces required by config
        required: usize,
        /// Actual number of active spaces found
        found: usize,
    },

    /// Embedding dimension doesn't match expected size.
    ///
    /// This indicates data corruption or version mismatch.
    /// Per constitution.yaml: NEVER silently truncate.
    #[error("Dimension mismatch for embedder {embedder}: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Index of the embedder (0-12)
        embedder: usize,
        /// Expected dimension per constitution.yaml
        expected: usize,
        /// Actual dimension found in fingerprint
        actual: usize,
    },

    /// One or both vectors have zero norm (all zeros).
    ///
    /// Cosine similarity is undefined for zero vectors.
    /// Per constitution.yaml: NEVER substitute with a default.
    #[error("Zero norm vector in embedder {embedder}")]
    ZeroNormVector {
        /// Index of the embedder with zero norm (0-12)
        embedder: usize,
    },

    /// RRF computation failed.
    ///
    /// This can occur when:
    /// - Ranked lists are empty
    /// - Invalid rank values
    /// - Memory allocation failure
    #[error("RRF computation failed: {reason}")]
    RrfError {
        /// Description of what went wrong
        reason: String,
    },

    /// Purpose vector computation failed.
    ///
    /// This can occur when:
    /// - Purpose vector is uninitialized
    /// - Alignment values are NaN or infinity
    #[error("Purpose vector computation failed: {0}")]
    PurposeError(String),

    /// Late interaction (E12 ColBERT) embeddings are required but missing.
    ///
    /// This occurs when `WeightingStrategy::LateInteraction` is used
    /// but the fingerprint has no E12 tokens.
    #[error("Late interaction requires E12 ColBERT embeddings")]
    MissingLateInteraction,

    /// Sparse embedding (E6 or E13 SPLADE) operation failed.
    #[error("Sparse embedding error for embedder {embedder}: {reason}")]
    SparseEmbeddingError {
        /// Index of the sparse embedder (5 for E6, 12 for E13)
        embedder: usize,
        /// Description of what went wrong
        reason: String,
    },

    /// Invalid configuration provided.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Batch operation failed for a specific candidate.
    #[error("Batch operation failed at index {index}: {source}")]
    BatchError {
        /// Index of the failed candidate
        index: usize,
        /// The underlying error
        #[source]
        source: Box<SimilarityError>,
    },

    /// NaN or infinity encountered in computation.
    ///
    /// Per constitution.yaml: NaN/Infinity in UTL must be clamped or error.
    #[error("Invalid numeric value in computation: {context}")]
    InvalidNumericValue {
        /// Where the invalid value was encountered
        context: String,
    },
}

impl SimilarityError {
    /// Create an InsufficientSpaces error.
    #[inline]
    pub fn insufficient_spaces(required: usize, found: usize) -> Self {
        Self::InsufficientSpaces { required, found }
    }

    /// Create a DimensionMismatch error.
    #[inline]
    pub fn dimension_mismatch(embedder: usize, expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch {
            embedder,
            expected,
            actual,
        }
    }

    /// Create a ZeroNormVector error.
    #[inline]
    pub fn zero_norm(embedder: usize) -> Self {
        Self::ZeroNormVector { embedder }
    }

    /// Create an RrfError.
    #[inline]
    pub fn rrf_error(reason: impl Into<String>) -> Self {
        Self::RrfError {
            reason: reason.into(),
        }
    }

    /// Create a PurposeError.
    #[inline]
    pub fn purpose_error(msg: impl Into<String>) -> Self {
        Self::PurposeError(msg.into())
    }

    /// Create a SparseEmbeddingError.
    #[inline]
    pub fn sparse_error(embedder: usize, reason: impl Into<String>) -> Self {
        Self::SparseEmbeddingError {
            embedder,
            reason: reason.into(),
        }
    }

    /// Create an InvalidConfig error.
    #[inline]
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        Self::InvalidConfig(msg.into())
    }

    /// Create a BatchError.
    #[inline]
    pub fn batch_error(index: usize, source: SimilarityError) -> Self {
        Self::BatchError {
            index,
            source: Box::new(source),
        }
    }

    /// Create an InvalidNumericValue error.
    #[inline]
    pub fn invalid_numeric(context: impl Into<String>) -> Self {
        Self::InvalidNumericValue {
            context: context.into(),
        }
    }

    /// Check if this is a recoverable error (can retry with different config).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::InsufficientSpaces { .. } | Self::MissingLateInteraction | Self::InvalidConfig(_)
        )
    }

    /// Check if this indicates data corruption.
    pub fn is_data_corruption(&self) -> bool {
        matches!(
            self,
            Self::DimensionMismatch { .. } | Self::InvalidNumericValue { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_messages() {
        let err1 = SimilarityError::insufficient_spaces(5, 3);
        assert!(err1.to_string().contains("required 5"));
        assert!(err1.to_string().contains("found 3"));

        let err2 = SimilarityError::dimension_mismatch(0, 1024, 512);
        assert!(err2.to_string().contains("embedder 0"));
        assert!(err2.to_string().contains("expected 1024"));
        assert!(err2.to_string().contains("got 512"));

        let err3 = SimilarityError::zero_norm(4);
        assert!(err3.to_string().contains("embedder 4"));

        let err4 = SimilarityError::rrf_error("empty ranked list");
        assert!(err4.to_string().contains("empty ranked list"));

        println!("[PASS] All error messages format correctly:");
        println!("  - InsufficientSpaces: {}", err1);
        println!("  - DimensionMismatch: {}", err2);
        println!("  - ZeroNormVector: {}", err3);
        println!("  - RrfError: {}", err4);
    }

    #[test]
    fn test_error_is_recoverable() {
        assert!(SimilarityError::insufficient_spaces(5, 3).is_recoverable());
        assert!(SimilarityError::MissingLateInteraction.is_recoverable());
        assert!(SimilarityError::invalid_config("bad").is_recoverable());

        assert!(!SimilarityError::dimension_mismatch(0, 1024, 512).is_recoverable());
        assert!(!SimilarityError::zero_norm(0).is_recoverable());
        assert!(!SimilarityError::rrf_error("fail").is_recoverable());

        println!("[PASS] is_recoverable correctly categorizes errors");
    }

    #[test]
    fn test_error_is_data_corruption() {
        assert!(SimilarityError::dimension_mismatch(0, 1024, 512).is_data_corruption());
        assert!(SimilarityError::invalid_numeric("NaN in score").is_data_corruption());

        assert!(!SimilarityError::insufficient_spaces(5, 3).is_data_corruption());
        assert!(!SimilarityError::zero_norm(0).is_data_corruption());

        println!("[PASS] is_data_corruption correctly identifies corruption errors");
    }

    #[test]
    fn test_batch_error_nesting() {
        let inner = SimilarityError::zero_norm(5);
        let outer = SimilarityError::batch_error(42, inner);

        assert!(outer.to_string().contains("index 42"));

        if let SimilarityError::BatchError { index, source } = outer {
            assert_eq!(index, 42);
            assert!(matches!(
                *source,
                SimilarityError::ZeroNormVector { embedder: 5 }
            ));
        } else {
            panic!("Expected BatchError");
        }

        println!("[PASS] BatchError correctly wraps inner errors");
    }

    #[test]
    fn test_sparse_error() {
        let err = SimilarityError::sparse_error(12, "invalid SPLADE indices");
        assert!(err.to_string().contains("embedder 12"));
        assert!(err.to_string().contains("invalid SPLADE indices"));

        println!("[PASS] SparseEmbeddingError formats correctly: {}", err);
    }
}
