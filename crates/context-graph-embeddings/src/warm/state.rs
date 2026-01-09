//! Warm Model State Management
//!
//! Defines the [`WarmModelState`] enum for tracking model loading lifecycle.
//!
//! # State Transitions
//!
//! ```text
//! Pending --> Loading --> Validating --> Warm
//!               |             |
//!               v             v
//!            Failed        Failed
//! ```
//!
//! Models start in `Pending`, transition through `Loading` and `Validating`,
//! and end in either `Warm` (success) or `Failed` (error).

/// State of a model in the warm-loading pipeline.
///
/// Tracks the lifecycle of model loading from queue to ready-for-inference.
/// Used for health check status reporting (REQ-WARM-006).
#[derive(Debug, Clone, PartialEq)]
pub enum WarmModelState {
    /// Model is queued but loading has not started.
    Pending,
    /// Model is actively being loaded into VRAM.
    Loading {
        /// Progress percentage (0-100).
        progress_percent: u8,
        /// Number of bytes loaded so far.
        bytes_loaded: usize,
    },
    /// Model is loaded; validation is in progress.
    Validating,
    /// Model is ready for inference.
    Warm,
    /// Loading or validation failed.
    Failed {
        /// Numeric error code for programmatic handling.
        error_code: u16,
        /// Human-readable error description.
        error_message: String,
    },
}

impl WarmModelState {
    /// Returns `true` if the model is ready for inference.
    #[inline]
    pub fn is_warm(&self) -> bool {
        matches!(self, Self::Warm)
    }

    /// Returns `true` if loading or validation failed.
    #[inline]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns `true` if the model is currently loading.
    #[inline]
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_predicates() {
        let s = WarmModelState::Pending;
        assert!(!s.is_warm() && !s.is_failed() && !s.is_loading());
    }

    #[test]
    fn test_loading_predicates() {
        let s = WarmModelState::Loading {
            progress_percent: 50,
            bytes_loaded: 1024,
        };
        assert!(!s.is_warm() && !s.is_failed() && s.is_loading());
    }

    #[test]
    fn test_validating_predicates() {
        let s = WarmModelState::Validating;
        assert!(!s.is_warm() && !s.is_failed() && !s.is_loading());
    }

    #[test]
    fn test_warm_predicates() {
        let s = WarmModelState::Warm;
        assert!(s.is_warm() && !s.is_failed() && !s.is_loading());
    }

    #[test]
    fn test_failed_predicates() {
        let s = WarmModelState::Failed {
            error_code: 101,
            error_message: "VRAM".into(),
        };
        assert!(!s.is_warm() && s.is_failed() && !s.is_loading());
    }
}
