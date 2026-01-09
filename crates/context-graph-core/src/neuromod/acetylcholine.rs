//! Acetylcholine (ACh) - Learning Rate Modulator
//!
//! Range: [0.001, 0.002]
//! Parameter: utl.lr
//! Trigger: meta_cognitive.dream
//!
//! ## Constitution Reference: neuromod.Acetylcholine (lines 191-206)
//!
//! Acetylcholine modulates UTL (Unified Theory of Learning) learning rate:
//! - High ACh (0.002): Faster learning rate (during dreams/consolidation)
//! - Low ACh (0.001): Normal learning rate (baseline operation)
//!
//! ## Important: Integration with GWT Meta-Cognitive
//!
//! The actual ACh state is managed by `gwt::meta_cognitive::MetaCognitiveLoop`.
//! This module provides:
//! 1. Constants for the ACh range
//! 2. The `AcetylcholineProvider` trait for abstraction
//! 3. A bridge function to read ACh from MetaCognitiveLoop
//!
//! ## Trigger: meta_cognitive.dream
//!
//! When MetaCognitiveLoop detects 5+ consecutive low meta-scores (<0.5),
//! it triggers an introspective dream and increases ACh to boost learning.

use crate::gwt::meta_cognitive::MetaCognitiveLoop;

/// Acetylcholine baseline level (minimum learning rate)
/// Constitution v4.0.0: neuromod.Acetylcholine.range = "[0.001, 0.002]"
pub const ACH_BASELINE: f32 = 0.001;

/// Acetylcholine maximum level (elevated learning rate during dreams)
pub const ACH_MAX: f32 = 0.002;

/// Acetylcholine decay rate (matches gwt::meta_cognitive)
pub const ACH_DECAY_RATE: f32 = 0.1;

/// Trait for reading ACh level from any provider
///
/// This allows abstraction over the ACh source, enabling testing
/// and alternative implementations while keeping GWT as the canonical source.
pub trait AcetylcholineProvider {
    /// Get current acetylcholine level
    fn get_acetylcholine(&self) -> f32;

    /// Check if system is in elevated learning state
    fn is_learning_elevated(&self) -> bool {
        self.get_acetylcholine() > ACH_BASELINE + (ACH_MAX - ACH_BASELINE) / 2.0
    }

    /// Get UTL learning rate (the parameter controlled by ACh)
    fn get_utl_learning_rate(&self) -> f32 {
        self.get_acetylcholine()
    }
}

/// Bridge implementation: Read ACh from MetaCognitiveLoop
impl AcetylcholineProvider for MetaCognitiveLoop {
    fn get_acetylcholine(&self) -> f32 {
        self.acetylcholine()
    }
}

/// Get ACh from MetaCognitiveLoop (convenience function)
///
/// This is the primary way to read ACh in the system, as MetaCognitiveLoop
/// is the canonical manager of acetylcholine state.
///
/// # Arguments
/// - `meta_loop`: Reference to the MetaCognitiveLoop
///
/// # Returns
/// Current ACh level in range [ACH_BASELINE, ACH_MAX]
pub fn get_ach_from_meta_cognitive(meta_loop: &MetaCognitiveLoop) -> f32 {
    meta_loop.acetylcholine()
}

/// Read-only ACh snapshot for NeuromodulationState
///
/// Since ACh is managed by GWT, this struct provides a snapshot
/// for inclusion in the overall neuromodulation state.
#[derive(Debug, Clone, Copy)]
pub struct AcetylcholineSnapshot {
    /// Current ACh value
    pub value: f32,
    /// Whether learning is elevated
    pub learning_elevated: bool,
}

impl AcetylcholineSnapshot {
    /// Create snapshot from current ACh value
    pub fn from_value(value: f32) -> Self {
        Self {
            value,
            learning_elevated: value > ACH_BASELINE + (ACH_MAX - ACH_BASELINE) / 2.0,
        }
    }

    /// Create snapshot from MetaCognitiveLoop
    pub fn from_meta_cognitive(meta_loop: &MetaCognitiveLoop) -> Self {
        Self::from_value(meta_loop.acetylcholine())
    }
}

/// Validate ACh value is within constitution-mandated range
pub fn validate_ach_value(value: f32) -> Result<f32, &'static str> {
    if value < ACH_BASELINE {
        Err("ACh below baseline (0.001)")
    } else if value > ACH_MAX {
        Err("ACh above maximum (0.002)")
    } else {
        Ok(value)
    }
}

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_ach_constants() {
        // Verify constitution-mandated values
        assert!((ACH_BASELINE - 0.001).abs() < f32::EPSILON);
        assert!((ACH_MAX - 0.002).abs() < f32::EPSILON);
        assert!(ACH_BASELINE < ACH_MAX);
    }

    #[test]
    fn test_ach_validate_in_range() {
        assert!(validate_ach_value(0.001).is_ok());
        assert!(validate_ach_value(0.0015).is_ok());
        assert!(validate_ach_value(0.002).is_ok());
    }

    #[test]
    fn test_ach_validate_out_of_range() {
        assert!(validate_ach_value(0.0005).is_err());
        assert!(validate_ach_value(0.003).is_err());
    }

    #[test]
    fn test_ach_snapshot() {
        let snapshot = AcetylcholineSnapshot::from_value(ACH_BASELINE);
        assert!((snapshot.value - ACH_BASELINE).abs() < f32::EPSILON);
        assert!(!snapshot.learning_elevated);

        let elevated = AcetylcholineSnapshot::from_value(ACH_MAX);
        assert!(elevated.learning_elevated);
    }

    #[test]
    fn test_ach_from_meta_cognitive() {
        let meta_loop = MetaCognitiveLoop::new();
        let ach = get_ach_from_meta_cognitive(&meta_loop);

        // Should start at baseline
        assert!((ach - ACH_BASELINE).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ach_provider_trait() {
        let meta_loop = MetaCognitiveLoop::new();

        // Test trait methods
        let ach = meta_loop.get_acetylcholine();
        assert!((ach - ACH_BASELINE).abs() < f32::EPSILON);

        let lr = meta_loop.get_utl_learning_rate();
        assert!((lr - ach).abs() < f32::EPSILON);

        assert!(!meta_loop.is_learning_elevated());
    }

    #[tokio::test]
    async fn test_ach_elevated_after_dream_trigger() {
        let mut meta_loop = MetaCognitiveLoop::new();

        // Trigger 5 consecutive low scores to trigger dream
        for _ in 0..5 {
            let _ = meta_loop.evaluate(0.1, 0.9).await;
        }

        // ACh should now be elevated
        assert!(
            meta_loop.get_acetylcholine() > ACH_BASELINE,
            "ACh should be elevated after dream trigger"
        );
    }
}
