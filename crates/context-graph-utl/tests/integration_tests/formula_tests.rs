//! UTL Formula Tests
//!
//! Tests for the core UTL formula: `L = f((ΔS × ΔC) · wₑ · cos φ)`

use context_graph_utl::{
    compute_learning_magnitude,
    johari::{JohariQuadrant, SuggestedAction},
    lifecycle::{LifecycleLambdaWeights, LifecycleStage},
    processor::UtlProcessor,
    LearningSignal,
};

use super::helpers::{generate_context, generate_embedding};

// =============================================================================
// FULL UTL PIPELINE TESTS
// =============================================================================

#[test]
fn test_full_utl_pipeline_with_real_data() {
    let mut processor = UtlProcessor::with_defaults();

    let embedding = generate_embedding(1536, 42);
    let context = generate_context(50, 1536, 100);
    let content = "This is a moderately surprising statement about quantum mechanics!";

    let result = processor
        .compute_learning(content, &embedding, &context)
        .expect("UTL computation must succeed with valid inputs");

    // Post-condition: all bounds satisfied
    assert!(
        result.magnitude >= 0.0 && result.magnitude <= 1.0,
        "magnitude {} out of bounds [0,1]",
        result.magnitude
    );
    assert!(
        result.delta_s >= 0.0 && result.delta_s <= 1.0,
        "delta_s {} out of bounds [0,1]",
        result.delta_s
    );
    assert!(
        result.delta_c >= 0.0 && result.delta_c <= 1.0,
        "delta_c {} out of bounds [0,1]",
        result.delta_c
    );
    assert!(
        result.w_e >= 0.5 && result.w_e <= 1.5,
        "w_e {} out of bounds [0.5,1.5]",
        result.w_e
    );
    assert!(
        result.phi >= 0.0 && result.phi <= std::f32::consts::PI,
        "phi {} out of bounds [0,π]",
        result.phi
    );

    // Verify lambda weights are present
    assert!(
        result.lambda_weights.is_some(),
        "Lambda weights must be present in LearningSignal"
    );

    // Verify quadrant is valid
    assert!(matches!(
        result.quadrant,
        JohariQuadrant::Open
            | JohariQuadrant::Blind
            | JohariQuadrant::Hidden
            | JohariQuadrant::Unknown
    ));
}

#[test]
fn test_formula_mathematical_properties() {
    // Property 1: Zero surprise = zero learning
    let result = compute_learning_magnitude(0.0, 0.5, 1.0, 0.5);
    assert_eq!(result, 0.0, "zero surprise must yield zero learning");

    // Property 2: Zero coherence = zero learning
    let result = compute_learning_magnitude(0.5, 0.0, 1.0, 0.5);
    assert_eq!(result, 0.0, "zero coherence must yield zero learning");

    // Property 3: phi=π/2 = zero learning (cos(π/2) ≈ 0)
    let result = compute_learning_magnitude(1.0, 1.0, 1.0, std::f32::consts::FRAC_PI_2);
    assert!(
        result.abs() < 1e-6,
        "phi=π/2 must yield near-zero learning, got {}",
        result
    );

    // Property 4: Maximum learning at optimal conditions
    // L = (1.0 * 1.0) * 1.5 * cos(0) = 1.5, clamped to 1.0
    let result = compute_learning_magnitude(1.0, 1.0, 1.5, 0.0);
    assert!(
        (result - 1.0).abs() < 1e-5,
        "max conditions should yield L=1.0, got {}",
        result
    );
}

#[test]
fn test_formula_computation_accuracy() {
    // Test exact formula: L = (ΔS × ΔC) · wₑ · cos(φ)
    let delta_s = 0.8;
    let delta_c = 0.7;
    let w_e = 1.2;
    let phi = 0.0;

    let result = compute_learning_magnitude(delta_s, delta_c, w_e, phi);

    // Expected: (0.8 * 0.7) * 1.2 * cos(0) = 0.56 * 1.2 * 1.0 = 0.672
    let expected = (delta_s * delta_c) * w_e * phi.cos();
    let expected_clamped = expected.clamp(0.0, 1.0);

    assert!(
        (result - expected_clamped).abs() < 1e-5,
        "formula mismatch: got {} expected {}",
        result,
        expected_clamped
    );
}

#[test]
fn test_anti_phase_suppresses_learning() {
    // At phi = π, cos(π) = -1, so learning should be suppressed to 0
    let result = compute_learning_magnitude(0.8, 0.7, 1.2, std::f32::consts::PI);

    // Raw: (0.8 * 0.7) * 1.2 * (-1) = -0.672, clamped to 0
    assert_eq!(result, 0.0, "anti-phase (φ=π) must suppress learning to 0");
}

#[test]
fn test_max_values() {
    // Maximum input values: all 1.0, w_e at max 1.5, phi at 0 (cos=1)
    let result = compute_learning_magnitude(1.0, 1.0, 1.5, 0.0);

    // L = (1 * 1) * 1.5 * cos(0) = 1.5, clamped to 1.0
    assert_eq!(result, 1.0, "max values should clamp to L=1.0");
}

#[test]
fn test_min_values() {
    // Minimum meaningful values
    let result = compute_learning_magnitude(0.0, 0.0, 0.5, 0.0);

    // L = (0 * 0) * 0.5 * 1 = 0
    assert_eq!(result, 0.0, "min values should yield L=0.0");
}

#[test]
fn test_phi_orthogonal() {
    // At phi = π/2, cos(π/2) ≈ 0, so learning should be ~0
    let result = compute_learning_magnitude(1.0, 1.0, 1.5, std::f32::consts::FRAC_PI_2);

    assert!(
        result.abs() < 1e-6,
        "phi=π/2 should yield L≈0, got {}",
        result
    );
}

// =============================================================================
// SERIALIZATION ROUNDTRIP TESTS
// =============================================================================

#[test]
fn test_learning_signal_serialization() {
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

    let json = serde_json::to_string(&signal).expect("serialization must succeed");
    let deserialized: LearningSignal =
        serde_json::from_str(&json).expect("deserialization must succeed");

    assert_eq!(deserialized.magnitude, signal.magnitude);
    assert_eq!(deserialized.delta_s, signal.delta_s);
    assert_eq!(deserialized.quadrant, signal.quadrant);
}
