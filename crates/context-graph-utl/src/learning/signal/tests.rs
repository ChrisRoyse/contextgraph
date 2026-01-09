//! Tests for LearningSignal

use super::*;

#[test]
fn test_learning_signal_creation_valid() {
    let signal = LearningSignal::new(
        0.7,
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
    );
    assert!(signal.is_ok());
    let signal = signal.unwrap();
    assert_eq!(signal.magnitude, 0.7);
    assert_eq!(signal.delta_s, 0.6);
    assert_eq!(signal.delta_c, 0.8);
    assert_eq!(signal.w_e, 1.2);
    assert_eq!(signal.phi, 0.5);
    assert!(signal.lambda_weights.is_none());
    assert_eq!(signal.quadrant, JohariQuadrant::Open);
    assert_eq!(signal.suggested_action, SuggestedAction::DirectRecall);
    assert!(signal.should_consolidate);
    assert!(signal.should_store);
    assert_eq!(signal.latency_us, 1500);
}

#[test]
fn test_learning_signal_validation_nan_magnitude() {
    let signal = LearningSignal::new(
        f32::NAN,
        0.5,
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        true,
        100,
    );
    assert!(signal.is_err());
    match signal.unwrap_err() {
        UtlError::InvalidComputation { reason, .. } => {
            assert!(reason.contains("NaN"));
        }
        _ => panic!("Expected InvalidComputation error"),
    }
}

#[test]
fn test_learning_signal_validation_infinity_magnitude() {
    let signal = LearningSignal::new(
        f32::INFINITY,
        0.5,
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        true,
        100,
    );
    assert!(signal.is_err());
    match signal.unwrap_err() {
        UtlError::InvalidComputation { reason, .. } => {
            assert!(reason.contains("Infinity"));
        }
        _ => panic!("Expected InvalidComputation error"),
    }
}

#[test]
fn test_learning_signal_validation_nan_component() {
    let signal = LearningSignal::new(
        0.5,
        f32::NAN, // delta_s is NaN
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        true,
        100,
    );
    assert!(signal.is_err());
    match signal.unwrap_err() {
        UtlError::InvalidComputation { reason, .. } => {
            assert!(reason.contains("NaN"));
        }
        _ => panic!("Expected InvalidComputation error"),
    }
}

#[test]
fn test_learning_intensity_boundary_values() {
    // Low: < 0.3
    let low_signal = LearningSignal::new(
        0.29,
        0.5,
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        false,
        100,
    )
    .unwrap();
    assert_eq!(low_signal.intensity_category(), LearningIntensity::Low);
    assert!(low_signal.is_low_learning());
    assert!(!low_signal.is_high_learning());

    // Medium: 0.3 - 0.7
    let med_signal = LearningSignal::new(
        0.5,
        0.5,
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        true,
        100,
    )
    .unwrap();
    assert_eq!(med_signal.intensity_category(), LearningIntensity::Medium);
    assert!(!med_signal.is_low_learning());
    assert!(!med_signal.is_high_learning());

    // High: > 0.7
    let high_signal = LearningSignal::new(
        0.85,
        0.5,
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Open,
        SuggestedAction::DirectRecall,
        true,
        true,
        100,
    )
    .unwrap();
    assert_eq!(high_signal.intensity_category(), LearningIntensity::High);
    assert!(!high_signal.is_low_learning());
    assert!(high_signal.is_high_learning());

    // Boundary at 0.3 (should be Medium)
    let boundary_low = LearningSignal::new(
        0.3,
        0.5,
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        true,
        100,
    )
    .unwrap();
    assert_eq!(boundary_low.intensity_category(), LearningIntensity::Medium);

    // Boundary at 0.7 (should be High since magnitude >= 0.7)
    let boundary_high = LearningSignal::new(
        0.7,
        0.5,
        0.5,
        1.0,
        0.0,
        None,
        JohariQuadrant::Open,
        SuggestedAction::DirectRecall,
        true,
        true,
        100,
    )
    .unwrap();
    assert_eq!(boundary_high.intensity_category(), LearningIntensity::High);
}

#[test]
fn test_learning_signal_serialization_roundtrip() {
    let original = LearningSignal::new(
        0.7,
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

    let json = serde_json::to_string(&original).expect("Serialization failed");
    let deserialized: LearningSignal = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(deserialized.magnitude, original.magnitude);
    assert_eq!(deserialized.delta_s, original.delta_s);
    assert_eq!(deserialized.delta_c, original.delta_c);
    assert_eq!(deserialized.w_e, original.w_e);
    assert_eq!(deserialized.phi, original.phi);
    assert!(deserialized.lambda_weights.is_none());
    assert_eq!(deserialized.quadrant, original.quadrant);
    assert_eq!(deserialized.suggested_action, original.suggested_action);
    assert_eq!(deserialized.should_consolidate, original.should_consolidate);
    assert_eq!(deserialized.should_store, original.should_store);
    assert_eq!(deserialized.latency_us, original.latency_us);
}

#[test]
fn test_learning_signal_with_lambda_weights() {
    use crate::lifecycle::LifecycleStage;

    let weights = LifecycleLambdaWeights::for_stage(LifecycleStage::Growth);
    let signal = LearningSignal::new(
        0.7,
        0.6,
        0.8,
        1.2,
        0.5,
        Some(weights),
        JohariQuadrant::Open,
        SuggestedAction::DirectRecall,
        true,
        true,
        1500,
    )
    .unwrap();

    assert!(signal.lambda_weights.is_some());
    let lw = signal.lambda_weights.unwrap();
    assert!((lw.lambda_s() - weights.lambda_s()).abs() < 0.001);
    assert!((lw.lambda_c() - weights.lambda_c()).abs() < 0.001);
}
