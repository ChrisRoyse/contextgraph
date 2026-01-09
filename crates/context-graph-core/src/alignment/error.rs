//! Alignment computation errors.
//!
//! Defines errors that can occur during goal alignment calculations.
//! All errors are designed for robust debugging with specific context.

use uuid::Uuid;

/// Errors that can occur during alignment computation.
///
/// Each error variant includes specific context for debugging:
/// - What operation failed
/// - What state was expected vs actual
/// - Suggestions for resolution
#[derive(Debug, thiserror::Error)]
pub enum AlignmentError {
    /// No North Star goal defined in the hierarchy.
    ///
    /// Resolution: Add a North Star goal using `GoalNode::autonomous_goal()` with `GoalLevel::NorthStar`.
    #[error(
        "No North Star goal defined in hierarchy - cannot compute alignment without a North Star"
    )]
    NoNorthStar,

    /// Goal not found in hierarchy.
    ///
    /// Resolution: Ensure the goal was added to the hierarchy before referencing.
    #[error("Goal not found in hierarchy: {0}")]
    GoalNotFound(Uuid),

    /// Empty fingerprint - no embeddings to compute alignment.
    ///
    /// Resolution: Provide a fingerprint with valid embedding data.
    #[error("Empty fingerprint - no embeddings to compute alignment")]
    EmptyFingerprint,

    /// Purpose vector dimension mismatch.
    ///
    /// The purpose vector must have exactly 13 alignment values (one per embedder).
    #[error("Purpose vector dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch {
        /// Expected dimension (always 13 for NUM_EMBEDDERS)
        expected: usize,
        /// Actual dimension received
        got: usize,
    },

    /// Invalid configuration provided.
    ///
    /// Resolution: Check configuration values are within valid ranges.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Computation timeout exceeded.
    ///
    /// Resolution: Reduce batch size or increase timeout limit.
    #[error("Computation timeout exceeded: {elapsed_ms}ms > {limit_ms}ms")]
    Timeout {
        /// Elapsed time in milliseconds
        elapsed_ms: u64,
        /// Configured limit in milliseconds
        limit_ms: u64,
    },

    /// Hierarchy validation failed.
    ///
    /// The goal hierarchy has structural issues (orphan nodes, cycles, etc.).
    #[error("Hierarchy validation failed: {0}")]
    InvalidHierarchy(String),

    /// General computation failure.
    ///
    /// Catch-all for unexpected errors during computation.
    #[error("Computation failed: {0}")]
    ComputationFailed(String),
}

impl AlignmentError {
    /// Check if this error is recoverable through retry.
    #[inline]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Check if this error requires configuration changes.
    #[inline]
    pub fn requires_config_change(&self) -> bool {
        matches!(
            self,
            Self::NoNorthStar | Self::InvalidConfig(_) | Self::InvalidHierarchy(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_messages() {
        println!("\n=== AlignmentError Display Messages ===");

        let e1 = AlignmentError::NoNorthStar;
        println!("NoNorthStar: {}", e1);
        assert!(e1.to_string().contains("North Star"));

        let test_uuid = Uuid::new_v4();
        let e2 = AlignmentError::GoalNotFound(test_uuid);
        println!("GoalNotFound: {}", e2);
        assert!(e2.to_string().contains(&test_uuid.to_string()));

        let e3 = AlignmentError::EmptyFingerprint;
        println!("EmptyFingerprint: {}", e3);
        assert!(e3.to_string().contains("Empty"));

        let e4 = AlignmentError::DimensionMismatch {
            expected: 13,
            got: 10,
        };
        println!("DimensionMismatch: {}", e4);
        assert!(e4.to_string().contains("13"));
        assert!(e4.to_string().contains("10"));

        let e5 = AlignmentError::InvalidConfig("weights must sum to 1.0".into());
        println!("InvalidConfig: {}", e5);
        assert!(e5.to_string().contains("weights"));

        let e6 = AlignmentError::Timeout {
            elapsed_ms: 10,
            limit_ms: 5,
        };
        println!("Timeout: {}", e6);
        assert!(e6.to_string().contains("10ms"));
        assert!(e6.to_string().contains("5ms"));

        let e7 = AlignmentError::InvalidHierarchy("orphan node detected".into());
        println!("InvalidHierarchy: {}", e7);
        assert!(e7.to_string().contains("orphan"));

        let e8 = AlignmentError::ComputationFailed("NaN in alignment".into());
        println!("ComputationFailed: {}", e8);
        assert!(e8.to_string().contains("NaN"));

        println!("[VERIFIED] All error display messages are correct and descriptive");
    }

    #[test]
    fn test_error_is_recoverable() {
        assert!(!AlignmentError::NoNorthStar.is_recoverable());
        assert!(!AlignmentError::GoalNotFound(Uuid::new_v4()).is_recoverable());
        assert!(!AlignmentError::EmptyFingerprint.is_recoverable());
        assert!(!AlignmentError::InvalidConfig("test".into()).is_recoverable());

        assert!(AlignmentError::Timeout {
            elapsed_ms: 10,
            limit_ms: 5
        }
        .is_recoverable());

        println!("[VERIFIED] is_recoverable returns true only for Timeout errors");
    }

    #[test]
    fn test_error_requires_config_change() {
        assert!(AlignmentError::NoNorthStar.requires_config_change());
        assert!(AlignmentError::InvalidConfig("test".into()).requires_config_change());
        assert!(AlignmentError::InvalidHierarchy("test".into()).requires_config_change());

        assert!(!AlignmentError::GoalNotFound(Uuid::new_v4()).requires_config_change());
        assert!(!AlignmentError::EmptyFingerprint.requires_config_change());
        assert!(!AlignmentError::Timeout {
            elapsed_ms: 10,
            limit_ms: 5
        }
        .requires_config_change());

        println!("[VERIFIED] requires_config_change returns true for config-related errors");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AlignmentError>();
        println!("[VERIFIED] AlignmentError is Send + Sync (thread-safe)");
    }
}
