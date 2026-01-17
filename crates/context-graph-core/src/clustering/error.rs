//! Error types for clustering operations.

use thiserror::Error;

use crate::error::StorageError;
use crate::teleological::Embedder;

/// Errors that can occur during clustering operations.
#[derive(Debug, Error)]
pub enum ClusterError {
    /// Not enough data points for clustering.
    #[error("Insufficient data: required {required}, actual {actual}")]
    InsufficientData {
        /// Minimum required data points
        required: usize,
        /// Actual data points provided
        actual: usize,
    },

    /// Embedding dimension doesn't match expected dimension for space.
    #[error("Dimension mismatch: expected {expected}, actual {actual}")]
    DimensionMismatch {
        /// Expected dimension for this embedding space
        expected: usize,
        /// Actual dimension provided
        actual: usize,
    },

    /// No valid clusters found (all points are noise).
    #[error("No valid clusters found")]
    NoValidClusters,

    /// Storage operation failed.
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    /// Invalid parameter provided.
    #[error("Invalid parameter: {message}")]
    InvalidParameter {
        /// Description of what's wrong with the parameter
        message: String,
    },

    /// Embedding space not initialized for clustering.
    #[error("Space not initialized: {0:?}")]
    SpaceNotInitialized(Embedder),
}

impl ClusterError {
    /// Create an InsufficientData error.
    pub fn insufficient_data(required: usize, actual: usize) -> Self {
        Self::InsufficientData { required, actual }
    }

    /// Create a DimensionMismatch error.
    pub fn dimension_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create an InvalidParameter error.
    pub fn invalid_parameter(message: impl Into<String>) -> Self {
        Self::InvalidParameter {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insufficient_data_error() {
        let err = ClusterError::insufficient_data(3, 1);
        let msg = err.to_string();

        assert!(msg.contains("required 3"), "should mention required count");
        assert!(msg.contains("actual 1"), "should mention actual count");

        println!("[PASS] test_insufficient_data_error - message: {}", msg);
    }

    #[test]
    fn test_dimension_mismatch_error() {
        let err = ClusterError::dimension_mismatch(1024, 512);
        let msg = err.to_string();

        assert!(msg.contains("expected 1024"), "should mention expected dim");
        assert!(msg.contains("actual 512"), "should mention actual dim");

        println!("[PASS] test_dimension_mismatch_error - message: {}", msg);
    }

    #[test]
    fn test_no_valid_clusters_error() {
        let err = ClusterError::NoValidClusters;
        let msg = err.to_string();

        assert!(msg.contains("No valid clusters"), "should describe the error");

        println!("[PASS] test_no_valid_clusters_error - message: {}", msg);
    }

    #[test]
    fn test_invalid_parameter_error() {
        let err = ClusterError::invalid_parameter("min_cluster_size must be > 0");
        let msg = err.to_string();

        assert!(msg.contains("min_cluster_size"), "should include parameter info");

        println!("[PASS] test_invalid_parameter_error - message: {}", msg);
    }

    #[test]
    fn test_space_not_initialized_error() {
        let err = ClusterError::SpaceNotInitialized(Embedder::Semantic);
        let msg = err.to_string();

        assert!(msg.contains("Semantic"), "should mention the embedder");

        println!("[PASS] test_space_not_initialized_error - message: {}", msg);
    }

    #[test]
    fn test_error_variants_are_debug() {
        // Ensure all variants implement Debug
        let errors: Vec<ClusterError> = vec![
            ClusterError::insufficient_data(3, 1),
            ClusterError::dimension_mismatch(1024, 512),
            ClusterError::NoValidClusters,
            ClusterError::invalid_parameter("test"),
            ClusterError::SpaceNotInitialized(Embedder::Semantic),
        ];

        for err in errors {
            let debug = format!("{:?}", err);
            assert!(!debug.is_empty(), "Debug should produce output");
        }

        println!("[PASS] test_error_variants_are_debug - all variants implement Debug");
    }
}
