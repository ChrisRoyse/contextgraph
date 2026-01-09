#![deny(deprecated)]

//! UTL (Unified Theory of Learning) computation engine for Context Graph.
//!
//! This crate provides the core UTL computation engine that implements the
//! learning score formula: `L = f((ΔS × ΔC) · wₑ · cos φ)`
//!
//! # Modules
//!
//! - [`config`]: Configuration types for all UTL subsystems
//! - [`error`]: Error types and result aliases
//! - [`surprise`]: Surprise (ΔS) computation using KL divergence and embedding distance
//! - [`coherence`]: Coherence (ΔC) computation with structural and graph-based metrics
//! - [`emotional`]: Emotional weight (wₑ) computation with lexicon and arousal/valence
//! - [`phase`]: Phase oscillation (φ) and consolidation detection (NREM/REM)
//! - [`johari`]: Johari quadrant classification and retrieval weighting
//! - [`lifecycle`]: Lifecycle stage management with Marblestone lambda weights
//! - [`learning`]: Learning magnitude computation and signal types
//!
//! # Constitution Reference
//!
//! The UTL formula is defined as:
//! - `ΔS`: Entropy/novelty change in range `[0, 1]`
//! - `ΔC`: Coherence/understanding change in range `[0, 1]`
//! - `wₑ`: Emotional weight in range `[0.5, 1.5]`
//! - `φ`: Phase synchronization angle in range `[0, π]`
//!
//! # Example
//!
//! ```
//! use context_graph_utl::{compute_learning_magnitude, UtlConfig};
//!
//! // Compute learning magnitude from UTL components
//! let learning = compute_learning_magnitude(0.7, 0.6, 1.2, 0.0);
//! assert!(learning >= 0.0 && learning <= 1.0);
//!
//! // Or use the configuration-based approach
//! let config = UtlConfig::default();
//! assert!(config.validate().is_ok());
//! ```

pub mod config;
pub mod error;
pub mod surprise;

// Core UTL modules
pub mod coherence;
pub mod emotional;
pub mod johari;
pub mod learning;
pub mod lifecycle;
pub mod metrics;
pub mod phase;
pub mod processor;

// Re-export commonly used types from this crate
pub use config::UtlConfig;
pub use error::{UtlError, UtlResult};

// Re-export core types from context-graph-core (DO NOT DUPLICATE)
pub use context_graph_core::types::{EmotionalState, UtlContext, UtlMetrics};

// Re-export lifecycle types for convenience
pub use lifecycle::{LifecycleLambdaWeights, LifecycleManager, LifecycleStage};

// Re-export johari types for convenience
pub use johari::{classify_quadrant, JohariClassifier, JohariQuadrant, SuggestedAction};

// Re-export phase types for convenience
pub use phase::{ConsolidationPhase, PhaseDetector, PhaseOscillator};

// Re-export processor types for convenience
pub use processor::{SessionContext, UtlProcessor};

// Re-export metrics types for convenience
pub use metrics::{
    QuadrantDistribution, StageThresholds, ThresholdsResponse, UtlComputationMetrics, UtlStatus,
    UtlStatusResponse,
};

// Re-export learning types for backwards compatibility
pub use learning::{
    compute_learning_magnitude, compute_learning_magnitude_validated, LearningIntensity,
    LearningSignal, UtlState,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_re_exports_exist() {
        // Verify core re-exports are accessible
        let _metrics = UtlMetrics::default();
        let _context = UtlContext::default();
        let _state = EmotionalState::default();
        let _quadrant = JohariQuadrant::default();
    }

    #[test]
    fn test_lifecycle_re_exports() {
        let stage = LifecycleStage::from_interaction_count(100);
        assert_eq!(stage, LifecycleStage::Growth);

        let weights = LifecycleLambdaWeights::for_stage(stage);
        assert!((weights.lambda_s() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_johari_re_exports() {
        let quadrant = classify_quadrant(0.3, 0.7);
        assert_eq!(quadrant, JohariQuadrant::Open);
    }

    #[test]
    fn test_phase_re_exports() {
        let detector = PhaseDetector::new();
        let phase = detector.detect_phase(0.3);
        assert!(matches!(
            phase,
            ConsolidationPhase::NREM | ConsolidationPhase::REM | ConsolidationPhase::Wake
        ));
    }

    #[test]
    fn test_emotional_state_weight_modifier() {
        assert_eq!(EmotionalState::Neutral.weight_modifier(), 1.0);
        assert_eq!(EmotionalState::Focused.weight_modifier(), 1.3);
        assert_eq!(EmotionalState::Fatigued.weight_modifier(), 0.6);
    }

    #[test]
    fn test_config_validation() {
        let config = UtlConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_learning_magnitude_api() {
        // Verify learning magnitude functions are accessible
        let learning = compute_learning_magnitude(0.8, 0.7, 1.2, 0.0);
        assert!(learning > 0.5);

        let result = compute_learning_magnitude_validated(0.5, 0.6, 1.0, 0.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_learning_signal_api() {
        // Verify LearningSignal is accessible
        let signal = LearningSignal::new(
            0.75,
            0.6,
            0.8,
            1.2,
            0.5,
            None,
            JohariQuadrant::Open,
            SuggestedAction::DirectRecall,
            true,
            true,
            1500,
        )
        .unwrap();
        assert!(signal.is_high_learning()); // is_high_learning() uses > 0.7 threshold
    }

    #[test]
    fn test_utl_state_api() {
        // Verify UtlState is accessible
        let state = UtlState::empty();
        assert_eq!(state.learning_magnitude, 0.5);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn test_learning_intensity_api() {
        // Verify LearningIntensity is accessible
        let all = LearningIntensity::all();
        assert_eq!(all.len(), 3);
    }
}
