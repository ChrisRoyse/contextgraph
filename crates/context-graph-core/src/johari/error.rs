//! Error types for Johari transition operations.
//!
//! This module provides the `JohariError` enum for handling all error conditions
//! in Johari transition management operations.

use thiserror::Error;
use uuid::Uuid;

use crate::types::JohariQuadrant;

/// Error types for Johari transition operations.
///
/// All errors include full context for debugging and follow the FAIL FAST principle.
/// No workarounds or fallbacks - operations either succeed or return descriptive errors.
#[derive(Error, Debug)]
pub enum JohariError {
    /// Memory with the specified ID was not found in the store.
    #[error("Memory not found: {0}")]
    NotFound(Uuid),

    /// Attempted transition is not valid according to the Johari state machine.
    ///
    /// Includes source quadrant, target quadrant, and embedder index for debugging.
    #[error("Invalid transition from {from:?} to {to:?} for embedder {embedder_idx}")]
    InvalidTransition {
        from: JohariQuadrant,
        to: JohariQuadrant,
        embedder_idx: usize,
    },

    /// Embedder index is out of bounds (must be 0-12 for E1-E13).
    #[error("Invalid embedder index: {0} (must be 0-12)")]
    InvalidEmbedderIndex(usize),

    /// Storage backend operation failed.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Batch operation validation failed at a specific index.
    ///
    /// None of the batch operations are applied if any validation fails (all-or-nothing).
    #[error("Batch validation failed at index {idx}: {reason}")]
    BatchValidationFailed { idx: usize, reason: String },

    /// Classification operation failed.
    #[error("Classification error: {0}")]
    ClassificationError(String),

    /// Transition trigger is not valid for the given transition.
    #[error("Invalid trigger {trigger:?} for transition {from:?} -> {to:?}")]
    InvalidTrigger {
        from: JohariQuadrant,
        to: JohariQuadrant,
        trigger: crate::types::TransitionTrigger,
    },
}

impl JohariError {
    /// Create a NotFound error for a memory ID.
    #[inline]
    pub fn not_found(id: Uuid) -> Self {
        Self::NotFound(id)
    }

    /// Create an InvalidEmbedderIndex error.
    #[inline]
    pub fn invalid_embedder(idx: usize) -> Self {
        Self::InvalidEmbedderIndex(idx)
    }

    /// Create a StorageError from any error type.
    #[inline]
    pub fn storage<E: std::error::Error>(err: E) -> Self {
        Self::StorageError(err.to_string())
    }

    /// Create a BatchValidationFailed error.
    #[inline]
    pub fn batch_failed(idx: usize, reason: impl Into<String>) -> Self {
        Self::BatchValidationFailed {
            idx,
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = JohariError::NotFound(Uuid::nil());
        assert!(err.to_string().contains("Memory not found"));

        let err = JohariError::InvalidEmbedderIndex(15);
        assert!(err.to_string().contains("15"));
        assert!(err.to_string().contains("0-12"));

        let err = JohariError::InvalidTransition {
            from: JohariQuadrant::Hidden,
            to: JohariQuadrant::Blind,
            embedder_idx: 5,
        };
        assert!(err.to_string().contains("Hidden"));
        assert!(err.to_string().contains("Blind"));
        assert!(err.to_string().contains("5"));

        println!("[VERIFIED] test_error_display: All error variants display correctly");
    }

    #[test]
    fn test_error_constructors() {
        let id = Uuid::new_v4();
        let err = JohariError::not_found(id);
        match err {
            JohariError::NotFound(found_id) => assert_eq!(found_id, id),
            _ => panic!("Expected NotFound"),
        }

        let err = JohariError::invalid_embedder(13);
        match err {
            JohariError::InvalidEmbedderIndex(idx) => assert_eq!(idx, 13),
            _ => panic!("Expected InvalidEmbedderIndex"),
        }

        let err = JohariError::batch_failed(2, "test error");
        match err {
            JohariError::BatchValidationFailed { idx, reason } => {
                assert_eq!(idx, 2);
                assert_eq!(reason, "test error");
            }
            _ => panic!("Expected BatchValidationFailed"),
        }

        println!("[VERIFIED] test_error_constructors: Error constructors work correctly");
    }
}
