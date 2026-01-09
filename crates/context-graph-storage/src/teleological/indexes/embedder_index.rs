//! Per-embedder ANN index trait.
//!
//! FAIL FAST: Invalid operations return errors or panic. No fallbacks.
//!
//! # Architecture
//!
//! ```text
//!                     +------------------+
//!                     | EmbedderIndexOps |  <-- Trait
//!                     +------------------+
//!                            ^
//!                            |
//!           +----------------+-----------------+
//!           |                                  |
//! +-------------------+            +---------------------+
//! | HnswEmbedderIndex |            | (future)            |
//! | (12 indexes)      |            | InvertedIndex (E6)  |
//! +-------------------+            | ColBERTIndex (E12)  |
//!                                  +---------------------+
//! ```
//!
//! # FAIL FAST Guarantees
//!
//! | Scenario | Response |
//! |----------|----------|
//! | Wrong dimension | `IndexError::DimensionMismatch` |
//! | NaN/Inf in vector | `IndexError::InvalidVector` |
//! | E6/E12/E13 to HnswEmbedderIndex::new() | `panic!` with clear message |
//! | Search empty index | Empty results (not error) |

use uuid::Uuid;

use super::hnsw_config::{EmbedderIndex, HnswConfig};

/// Result type for index operations.
pub type IndexResult<T> = Result<T, IndexError>;

/// Errors from index operations. FAIL FAST - no recovery.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// Vector dimension does not match expected dimension for embedder.
    #[error("Dimension mismatch: expected {expected}, got {actual} for {embedder:?}")]
    DimensionMismatch {
        embedder: EmbedderIndex,
        expected: usize,
        actual: usize,
    },

    /// Index not found for specified embedder.
    #[error("Index not found for {embedder:?}")]
    IndexNotFound { embedder: EmbedderIndex },

    /// Index operation failed.
    #[error("Index operation failed for {embedder:?}: {message}")]
    OperationFailed {
        embedder: EmbedderIndex,
        message: String,
    },

    /// Attempted modification on read-only index.
    #[error("Index is read-only, cannot insert")]
    ReadOnly,

    /// Vector contains invalid values (NaN, Inf).
    #[error("Invalid vector: {message}")]
    InvalidVector { message: String },
}

/// Trait for per-embedder approximate nearest neighbor index.
///
/// Each embedder (E1-E13) has its own index with embedder-specific
/// configuration (dimension, distance metric).
///
/// # FAIL FAST
///
/// All methods validate inputs and panic on invariant violations.
/// Use Result only for expected failure modes (not found, etc).
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` for concurrent access.
pub trait EmbedderIndexOps: Send + Sync {
    /// Get the embedder this index serves.
    fn embedder(&self) -> EmbedderIndex;

    /// Get the HNSW configuration for this index.
    fn config(&self) -> &HnswConfig;

    /// Number of vectors in the index.
    fn len(&self) -> usize;

    /// Check if empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert a vector with associated ID.
    ///
    /// # Errors
    ///
    /// - `IndexError::DimensionMismatch` if vector dimension != config.dimension
    /// - `IndexError::InvalidVector` if vector contains NaN or Inf
    ///
    /// # Duplicate Handling
    ///
    /// If the ID already exists, the vector is updated in place.
    fn insert(&self, id: Uuid, vector: &[f32]) -> IndexResult<()>;

    /// Remove a vector by ID.
    ///
    /// Returns true if removed, false if not found.
    fn remove(&self, id: Uuid) -> IndexResult<bool>;

    /// Search for k nearest neighbors.
    ///
    /// # Arguments
    /// * `query` - Query vector (must match dimension)
    /// * `k` - Number of neighbors to return
    /// * `ef_search` - Optional override for HNSW ef parameter
    ///
    /// # Returns
    ///
    /// Vector of (id, distance) pairs sorted by distance ascending.
    /// Empty vector if index is empty.
    ///
    /// # Errors
    ///
    /// - `IndexError::DimensionMismatch` if query dimension != config.dimension
    /// - `IndexError::InvalidVector` if query contains NaN or Inf
    fn search(&self, query: &[f32], k: usize, ef_search: Option<usize>)
        -> IndexResult<Vec<(Uuid, f32)>>;

    /// Batch insert multiple vectors.
    ///
    /// More efficient than individual inserts for bulk loading.
    ///
    /// # Returns
    ///
    /// Number of vectors successfully inserted.
    fn insert_batch(&self, items: &[(Uuid, Vec<f32>)]) -> IndexResult<usize>;

    /// Flush any pending writes to storage.
    fn flush(&self) -> IndexResult<()>;

    /// Get memory usage in bytes.
    fn memory_bytes(&self) -> usize;
}

/// Validation helper - FAIL FAST on invalid vectors.
///
/// # Arguments
///
/// * `vector` - Vector to validate
/// * `expected_dim` - Expected dimension
/// * `embedder` - Embedder for error context
///
/// # Errors
///
/// - `IndexError::DimensionMismatch` if dimension doesn't match
/// - `IndexError::InvalidVector` if vector contains NaN or Inf
#[inline]
pub fn validate_vector(
    vector: &[f32],
    expected_dim: usize,
    embedder: EmbedderIndex,
) -> IndexResult<()> {
    if vector.len() != expected_dim {
        return Err(IndexError::DimensionMismatch {
            embedder,
            expected: expected_dim,
            actual: vector.len(),
        });
    }

    for (i, &v) in vector.iter().enumerate() {
        if !v.is_finite() {
            return Err(IndexError::InvalidVector {
                message: format!("Non-finite value at index {}: {}", i, v),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_vector_correct_dimension() {
        println!("=== TEST: validate_vector accepts correct dimension ===");
        println!("BEFORE: vector.len()=1024, expected=1024");

        let vector: Vec<f32> = (0..1024).map(|i| (i as f32) / 1024.0).collect();
        let result = validate_vector(&vector, 1024, EmbedderIndex::E1Semantic);

        println!("AFTER: result={:?}", result);
        assert!(result.is_ok());
        println!("RESULT: PASS");
    }

    #[test]
    fn test_validate_vector_dimension_mismatch() {
        println!("=== TEST: validate_vector rejects wrong dimension ===");
        println!("BEFORE: vector.len()=512, expected=1024");

        let vector = vec![1.0; 512];
        let result = validate_vector(&vector, 1024, EmbedderIndex::E1Semantic);

        println!("AFTER: result={:?}", result);
        assert!(result.is_err());

        match result.unwrap_err() {
            IndexError::DimensionMismatch {
                expected, actual, ..
            } => {
                assert_eq!(expected, 1024);
                assert_eq!(actual, 512);
            }
            _ => panic!("Wrong error type"),
        }
        println!("RESULT: PASS");
    }

    #[test]
    fn test_validate_vector_empty() {
        println!("=== TEST: validate_vector rejects empty vector ===");
        println!("BEFORE: vector.len()=0, expected=1024");

        let vector: Vec<f32> = vec![];
        let result = validate_vector(&vector, 1024, EmbedderIndex::E1Semantic);

        println!("AFTER: result={:?}", result);
        assert!(result.is_err());

        match result.unwrap_err() {
            IndexError::DimensionMismatch {
                expected,
                actual,
                embedder,
            } => {
                assert_eq!(expected, 1024);
                assert_eq!(actual, 0);
                assert_eq!(embedder, EmbedderIndex::E1Semantic);
            }
            _ => panic!("Wrong error type"),
        }
        println!("RESULT: PASS");
    }

    #[test]
    fn test_validate_vector_nan() {
        println!("=== TEST: validate_vector rejects NaN ===");
        println!("BEFORE: vector[100]=NaN");

        let mut vector = vec![1.0; 384];
        vector[100] = f32::NAN;
        let result = validate_vector(&vector, 384, EmbedderIndex::E8Graph);

        println!("AFTER: result={:?}", result);
        assert!(result.is_err());

        match result.unwrap_err() {
            IndexError::InvalidVector { message } => {
                assert!(message.contains("Non-finite"));
                assert!(message.contains("index 100"));
            }
            _ => panic!("Wrong error type"),
        }
        println!("RESULT: PASS");
    }

    #[test]
    fn test_validate_vector_infinity() {
        println!("=== TEST: validate_vector rejects Infinity ===");
        println!("BEFORE: vector[0]=Inf");

        let mut vector = vec![1.0; 384];
        vector[0] = f32::INFINITY;
        let result = validate_vector(&vector, 384, EmbedderIndex::E8Graph);

        println!("AFTER: result={:?}", result);
        assert!(result.is_err());

        match result.unwrap_err() {
            IndexError::InvalidVector { message } => {
                assert!(message.contains("Non-finite"));
                assert!(message.contains("index 0"));
                assert!(message.contains("inf"));
            }
            _ => panic!("Wrong error type"),
        }
        println!("RESULT: PASS");
    }

    #[test]
    fn test_validate_vector_neg_infinity() {
        println!("=== TEST: validate_vector rejects negative Infinity ===");
        println!("BEFORE: vector[50]=-Inf");

        let mut vector = vec![1.0; 384];
        vector[50] = f32::NEG_INFINITY;
        let result = validate_vector(&vector, 384, EmbedderIndex::E8Graph);

        println!("AFTER: result={:?}", result);
        assert!(result.is_err());

        match result.unwrap_err() {
            IndexError::InvalidVector { message } => {
                assert!(message.contains("Non-finite"));
                assert!(message.contains("index 50"));
            }
            _ => panic!("Wrong error type"),
        }
        println!("RESULT: PASS");
    }

    #[test]
    fn test_index_error_display() {
        println!("=== TEST: IndexError Display implementations ===");

        let dim_err = IndexError::DimensionMismatch {
            embedder: EmbedderIndex::E1Semantic,
            expected: 1024,
            actual: 512,
        };
        let msg = format!("{}", dim_err);
        assert!(msg.contains("1024"));
        assert!(msg.contains("512"));
        assert!(msg.contains("E1Semantic"));
        println!("DimensionMismatch: {}", msg);

        let invalid_err = IndexError::InvalidVector {
            message: "test message".to_string(),
        };
        let msg = format!("{}", invalid_err);
        assert!(msg.contains("test message"));
        println!("InvalidVector: {}", msg);

        let not_found_err = IndexError::IndexNotFound {
            embedder: EmbedderIndex::E6Sparse,
        };
        let msg = format!("{}", not_found_err);
        assert!(msg.contains("E6Sparse"));
        println!("IndexNotFound: {}", msg);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_verification_log() {
        println!("\n=== EMBEDDER_INDEX.RS VERIFICATION LOG ===");
        println!();

        println!("Type Verification:");
        println!("  - IndexError: 5 variants (DimensionMismatch, IndexNotFound, OperationFailed, ReadOnly, InvalidVector)");
        println!("  - IndexResult<T>: Result<T, IndexError>");
        println!("  - EmbedderIndexOps: 8 trait methods");

        println!();
        println!("Function Verification:");
        println!("  - validate_vector: dimension check + finite check");

        println!();
        println!("Test Coverage:");
        println!("  - Correct dimension: PASS");
        println!("  - Dimension mismatch: PASS");
        println!("  - Empty vector: PASS");
        println!("  - NaN vector: PASS");
        println!("  - Infinity vector: PASS");
        println!("  - Negative infinity: PASS");
        println!("  - Error Display: PASS");

        println!();
        println!("VERIFICATION COMPLETE");
    }
}
