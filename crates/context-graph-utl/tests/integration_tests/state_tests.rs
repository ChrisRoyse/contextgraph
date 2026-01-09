//! UTL State and Learning Intensity Tests
//!
//! Tests for UtlState, LearningSignal, and processor state management

use context_graph_utl::{
    johari::{JohariQuadrant, SuggestedAction},
    lifecycle::{LifecycleLambdaWeights, LifecycleStage},
    processor::UtlProcessor,
    LearningIntensity, LearningSignal, UtlState,
};

use super::helpers::uniform_embedding;

// =============================================================================
// LEARNING INTENSITY TESTS
// =============================================================================

#[test]
fn test_learning_intensity_categories() {
    // Low: magnitude < 0.3
    let low = LearningSignal::new(
        0.2,
        0.3,
        0.3,
        1.0,
        0.5,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        false,
        100,
    )
    .unwrap();
    assert_eq!(low.intensity_category(), LearningIntensity::Low);
    assert!(low.is_low_learning());
    assert!(!low.is_high_learning());

    // Medium: 0.3 <= magnitude < 0.7
    let medium = LearningSignal::new(
        0.5,
        0.5,
        0.5,
        1.0,
        0.5,
        None,
        JohariQuadrant::Open,
        SuggestedAction::DirectRecall,
        false,
        true,
        100,
    )
    .unwrap();
    assert_eq!(medium.intensity_category(), LearningIntensity::Medium);
    assert!(!medium.is_low_learning());
    assert!(!medium.is_high_learning());

    // High: magnitude >= 0.7
    let high = LearningSignal::new(
        0.8,
        0.8,
        0.8,
        1.2,
        0.3,
        None,
        JohariQuadrant::Unknown,
        SuggestedAction::EpistemicAction,
        true,
        true,
        100,
    )
    .unwrap();
    assert_eq!(high.intensity_category(), LearningIntensity::High);
    assert!(!high.is_low_learning());
    assert!(high.is_high_learning());
}

// =============================================================================
// UTL STATE TESTS
// =============================================================================

#[test]
fn test_utl_state_from_signal() {
    let signal = LearningSignal::new(
        0.7,
        0.6,
        0.8,
        1.2,
        0.5,
        Some(LifecycleLambdaWeights::for_stage(LifecycleStage::Growth)),
        JohariQuadrant::Open,
        SuggestedAction::DirectRecall,
        true,
        true,
        1500,
    )
    .unwrap();

    let state = UtlState::from_signal(&signal);

    assert_eq!(state.delta_s, signal.delta_s);
    assert_eq!(state.delta_c, signal.delta_c);
    assert_eq!(state.w_e, signal.w_e);
    assert_eq!(state.phi, signal.phi);
    assert_eq!(state.learning_magnitude, signal.magnitude);
    assert_eq!(state.quadrant, signal.quadrant);
}

#[test]
fn test_utl_state_validation() {
    // Valid state
    let valid = UtlState::empty();
    assert!(valid.validate().is_ok());

    // State with NaN should fail validation
    let invalid = UtlState {
        delta_s: f32::NAN,
        ..UtlState::empty()
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn test_utl_state_serialization() {
    let state = UtlState {
        delta_s: 0.6,
        delta_c: 0.8,
        w_e: 1.2,
        phi: 0.5,
        learning_magnitude: 0.7,
        quadrant: JohariQuadrant::Blind,
        last_computed: chrono::Utc::now(),
    };

    let json = serde_json::to_string(&state).expect("serialization must succeed");
    let deserialized: UtlState = serde_json::from_str(&json).expect("deserialization must succeed");

    assert_eq!(deserialized.delta_s, state.delta_s);
    assert_eq!(deserialized.quadrant, state.quadrant);
}

// =============================================================================
// PROCESSOR STATE MANAGEMENT TESTS
// =============================================================================

#[test]
fn test_processor_reset() {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = uniform_embedding(128, 0.5);

    // Do some computations
    for _ in 0..10 {
        let _ = processor.compute_learning("test", &embedding, &[]);
    }

    assert!(processor.computation_count() > 0);
    assert!(processor.interaction_count() > 0);

    // Reset
    processor.reset();

    assert_eq!(processor.computation_count(), 0);
    assert_eq!(processor.interaction_count(), 0);
    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Infancy);
}

#[test]
fn test_processor_restore_lifecycle() {
    let mut processor = UtlProcessor::with_defaults();

    // Restore to Growth stage (100 interactions)
    processor.restore_lifecycle(100);

    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Growth);
    assert_eq!(processor.interaction_count(), 100);

    // Restore to Maturity stage (600 interactions)
    processor.reset();
    processor.restore_lifecycle(600);

    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Maturity);
    assert_eq!(processor.interaction_count(), 600);
}
