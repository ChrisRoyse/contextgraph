//! GPU HDBSCAN error types.
//!
//! All errors are explicit with detailed messages for debugging.
//! No fallbacks - errors mean the operation cannot proceed.

use std::fmt;
use thiserror::Error;

/// Errors that can occur during GPU HDBSCAN operations.
#[derive(Error, Debug)]
pub enum GpuHdbscanError {
    /// GPU is not available - HDBSCAN cannot proceed.
    ///
    /// Per constitution ARCH-GPU-05: HDBSCAN MUST run on GPU.
    /// No CPU fallback is provided.
    #[error("GPU not available for HDBSCAN. Constitution ARCH-GPU-05 requires GPU. Error: {reason}")]
    GpuNotAvailable { reason: String },

    /// FAISS GPU resources allocation failed.
    #[error("Failed to allocate FAISS GPU resources: {operation} returned code {code}")]
    FaissResourceError { operation: String, code: i32 },

    /// FAISS index creation failed.
    #[error("Failed to create FAISS GPU index: {operation} - {reason}")]
    FaissIndexError { operation: String, reason: String },

    /// FAISS k-NN search failed.
    #[error("FAISS k-NN search failed: {operation} returned code {code}")]
    FaissSearchError { operation: String, code: i32 },

    /// Invalid parameters for HDBSCAN.
    #[error("Invalid HDBSCAN parameter: {parameter} = {value}. Requirement: {requirement}")]
    InvalidParameter {
        parameter: String,
        value: String,
        requirement: String,
    },

    /// Insufficient data for clustering.
    #[error("Insufficient data for HDBSCAN: got {actual} points, need at least {required} (min_cluster_size)")]
    InsufficientData { required: usize, actual: usize },

    /// Dimension mismatch in input data.
    #[error("Dimension mismatch: embeddings count ({embeddings}) != memory_ids count ({ids})")]
    DimensionMismatch { embeddings: usize, ids: usize },

    /// Vector dimension is invalid.
    #[error("Invalid vector dimension: {dimension}. Must be > 0")]
    InvalidDimension { dimension: usize },

    /// NaN or Infinity found in embeddings.
    #[error("Invalid value in embedding at index {index}: value is {value_type}. All values must be finite.")]
    NonFiniteValue { index: usize, value_type: String },

    /// Internal algorithm error.
    #[error("Internal GPU HDBSCAN error in {phase}: {message}")]
    InternalError { phase: String, message: String },
}

impl GpuHdbscanError {
    /// Create a GPU not available error.
    pub fn gpu_not_available(reason: impl Into<String>) -> Self {
        Self::GpuNotAvailable {
            reason: reason.into(),
        }
    }

    /// Create a FAISS resource error.
    pub fn faiss_resource(operation: impl Into<String>, code: i32) -> Self {
        Self::FaissResourceError {
            operation: operation.into(),
            code,
        }
    }

    /// Create a FAISS index error.
    pub fn faiss_index(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::FaissIndexError {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Create a FAISS search error.
    pub fn faiss_search(operation: impl Into<String>, code: i32) -> Self {
        Self::FaissSearchError {
            operation: operation.into(),
            code,
        }
    }

    /// Create an invalid parameter error.
    pub fn invalid_parameter(
        parameter: impl Into<String>,
        value: impl fmt::Display,
        requirement: impl Into<String>,
    ) -> Self {
        Self::InvalidParameter {
            parameter: parameter.into(),
            value: value.to_string(),
            requirement: requirement.into(),
        }
    }

    /// Create an insufficient data error.
    pub fn insufficient_data(required: usize, actual: usize) -> Self {
        Self::InsufficientData { required, actual }
    }

    /// Create a dimension mismatch error.
    pub fn dimension_mismatch(embeddings: usize, ids: usize) -> Self {
        Self::DimensionMismatch { embeddings, ids }
    }

    /// Create an invalid dimension error.
    pub fn invalid_dimension(dimension: usize) -> Self {
        Self::InvalidDimension { dimension }
    }

    /// Create a non-finite value error.
    pub fn non_finite_value(index: usize, value: f32) -> Self {
        let value_type = if value.is_nan() {
            "NaN"
        } else if value.is_infinite() {
            if value > 0.0 { "+Infinity" } else { "-Infinity" }
        } else {
            "Unknown non-finite"
        };
        Self::NonFiniteValue {
            index,
            value_type: value_type.to_string(),
        }
    }

    /// Create an internal error.
    pub fn internal(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InternalError {
            phase: phase.into(),
            message: message.into(),
        }
    }
}

/// Result type for GPU HDBSCAN operations.
pub type GpuHdbscanResult<T> = Result<T, GpuHdbscanError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages_are_descriptive() {
        let err = GpuHdbscanError::gpu_not_available("nvidia-smi not found");
        assert!(err.to_string().contains("GPU not available"));
        assert!(err.to_string().contains("ARCH-GPU-05"));
        assert!(err.to_string().contains("nvidia-smi not found"));

        let err = GpuHdbscanError::insufficient_data(3, 2);
        assert!(err.to_string().contains("got 2 points"));
        assert!(err.to_string().contains("need at least 3"));

        let err = GpuHdbscanError::non_finite_value(5, f32::NAN);
        assert!(err.to_string().contains("index 5"));
        assert!(err.to_string().contains("NaN"));
    }
}
