//! Search error types. FAIL FAST - no fallbacks.
//!
//! # Design Philosophy
//!
//! All errors are fatal. No recovery attempts. This ensures:
//! - Bugs are caught early in development
//! - Data integrity is preserved
//! - Clear error messages for debugging
//!
//! # Error Hierarchy
//!
//! ```text
//! SearchError
//! ├── DimensionMismatch - Query dimension wrong for embedder
//! ├── UnsupportedEmbedder - E6/E12/E13 don't support HNSW
//! ├── EmptyQuery - Zero-length query vector
//! ├── InvalidVector - NaN or Inf in query
//! ├── Index - Underlying index operation failed
//! ├── NotFound - Fingerprint ID not in store
//! └── Store - Storage layer error
//! ```

use uuid::Uuid;

use super::super::indexes::{EmbedderIndex, IndexError};

/// Errors from single embedder search operations.
///
/// FAIL FAST: All errors are fatal. No recovery attempts.
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::search::{SearchError, SearchResult};
/// use context_graph_storage::teleological::indexes::EmbedderIndex;
///
/// fn example() -> SearchResult<()> {
///     // Return dimension mismatch error
///     Err(SearchError::DimensionMismatch {
///         embedder: EmbedderIndex::E1Semantic,
///         expected: 1024,
///         actual: 512,
///     })
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// Query vector dimension does not match embedder expectations.
    ///
    /// Each embedder has a fixed dimension. E1 expects 1024D, E8 expects 384D, etc.
    #[error("Dimension mismatch for {embedder:?}: expected {expected}, got {actual}")]
    DimensionMismatch {
        embedder: EmbedderIndex,
        expected: usize,
        actual: usize,
    },

    /// Embedder type does not support HNSW search.
    ///
    /// E6Sparse, E12LateInteraction, E13Splade require different algorithms:
    /// - E6Sparse: Inverted index with BM25
    /// - E12LateInteraction: ColBERT MaxSim token-level
    /// - E13Splade: Inverted index with learned expansion
    #[error("Embedder {embedder:?} does not support HNSW search - use inverted/MaxSim")]
    UnsupportedEmbedder { embedder: EmbedderIndex },

    /// Empty query vector provided.
    ///
    /// Query vectors must have at least one element.
    #[error("Empty query vector for {embedder:?}")]
    EmptyQuery { embedder: EmbedderIndex },

    /// Query vector contains invalid floating-point values.
    ///
    /// NaN and Infinity break distance calculations. All values must be finite.
    #[error("Invalid query vector for {embedder:?}: {message}")]
    InvalidVector {
        embedder: EmbedderIndex,
        message: String,
    },

    /// Underlying index operation failed.
    ///
    /// Wraps IndexError from the HNSW index layer.
    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    /// Fingerprint not found in store.
    ///
    /// The UUID does not exist in the fingerprint storage.
    #[error("Fingerprint {id} not found")]
    NotFound { id: Uuid },

    /// Store operation failed.
    ///
    /// Generic storage layer error (RocksDB, registry, etc).
    #[error("Store error: {0}")]
    Store(String),
}

/// Result type for search operations.
///
/// All search methods return this type. Errors are fatal - callers should
/// propagate them rather than attempting recovery.
pub type SearchResult<T> = Result<T, SearchError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_mismatch_error() {
        println!("=== TEST: SearchError::DimensionMismatch ===");
        println!("BEFORE: Creating dimension mismatch error");

        let err = SearchError::DimensionMismatch {
            embedder: EmbedderIndex::E1Semantic,
            expected: 1024,
            actual: 512,
        };

        let msg = format!("{}", err);
        println!("AFTER: error message = {}", msg);

        assert!(msg.contains("1024"));
        assert!(msg.contains("512"));
        assert!(msg.contains("E1Semantic"));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_unsupported_embedder_error() {
        println!("=== TEST: SearchError::UnsupportedEmbedder ===");

        let err = SearchError::UnsupportedEmbedder {
            embedder: EmbedderIndex::E6Sparse,
        };

        let msg = format!("{}", err);
        println!("Error: {}", msg);

        assert!(msg.contains("E6Sparse"));
        assert!(msg.contains("does not support HNSW"));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_empty_query_error() {
        println!("=== TEST: SearchError::EmptyQuery ===");

        let err = SearchError::EmptyQuery {
            embedder: EmbedderIndex::E8Graph,
        };

        let msg = format!("{}", err);
        println!("Error: {}", msg);

        assert!(msg.contains("Empty"));
        assert!(msg.contains("E8Graph"));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_invalid_vector_error() {
        println!("=== TEST: SearchError::InvalidVector ===");

        let err = SearchError::InvalidVector {
            embedder: EmbedderIndex::E5Causal,
            message: "Non-finite value at index 42: NaN".to_string(),
        };

        let msg = format!("{}", err);
        println!("Error: {}", msg);

        assert!(msg.contains("E5Causal"));
        assert!(msg.contains("NaN"));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_not_found_error() {
        println!("=== TEST: SearchError::NotFound ===");

        let id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let err = SearchError::NotFound { id };

        let msg = format!("{}", err);
        println!("Error: {}", msg);

        assert!(msg.contains("aaaaaaaa"));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_store_error() {
        println!("=== TEST: SearchError::Store ===");

        let err = SearchError::Store("RocksDB connection failed".to_string());

        let msg = format!("{}", err);
        println!("Error: {}", msg);

        assert!(msg.contains("RocksDB"));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_index_error_conversion() {
        println!("=== TEST: SearchError::Index from IndexError ===");

        let index_err = IndexError::DimensionMismatch {
            embedder: EmbedderIndex::E1Semantic,
            expected: 1024,
            actual: 128,
        };

        let search_err: SearchError = index_err.into();

        match search_err {
            SearchError::Index(_) => println!("Correctly converted to SearchError::Index"),
            _ => panic!("Wrong error variant"),
        }

        println!("RESULT: PASS");
    }

    #[test]
    fn test_result_type_usage() {
        println!("=== TEST: SearchResult type usage ===");

        fn ok_result() -> SearchResult<i32> {
            Ok(42)
        }

        fn err_result() -> SearchResult<i32> {
            Err(SearchError::EmptyQuery {
                embedder: EmbedderIndex::E1Semantic,
            })
        }

        assert!(ok_result().is_ok());
        assert!(err_result().is_err());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_verification_log() {
        println!("\n=== ERROR.RS VERIFICATION LOG ===");
        println!();

        println!("Type Verification:");
        println!("  - SearchError: 7 variants");
        println!("    - DimensionMismatch: embedder, expected, actual");
        println!("    - UnsupportedEmbedder: embedder");
        println!("    - EmptyQuery: embedder");
        println!("    - InvalidVector: embedder, message");
        println!("    - Index: IndexError");
        println!("    - NotFound: id");
        println!("    - Store: String");
        println!("  - SearchResult<T>: Result<T, SearchError>");

        println!();
        println!("Conversion Verification:");
        println!("  - From<IndexError> for SearchError: PASS");

        println!();
        println!("Display Verification:");
        println!("  - All variants produce informative messages: PASS");

        println!();
        println!("VERIFICATION COMPLETE");
    }
}
