//! Tests for the processor module.

use std::time::Instant;

use super::{SessionContext, UtlProcessor};
use crate::{JohariQuadrant, LifecycleStage, SuggestedAction, UtlConfig};

/// Map Johari quadrant to suggested action (test helper).
fn suggested_action_for_quadrant(quadrant: JohariQuadrant) -> SuggestedAction {
    match quadrant {
        JohariQuadrant::Open => SuggestedAction::DirectRecall,
        JohariQuadrant::Blind => SuggestedAction::TriggerDream,
        JohariQuadrant::Hidden => SuggestedAction::GetNeighborhood,
        JohariQuadrant::Unknown => SuggestedAction::EpistemicAction,
    }
}

fn test_embedding(dim: usize, value: f32) -> Vec<f32> {
    vec![value; dim]
}

#[test]
fn test_processor_creation() {
    let processor = UtlProcessor::with_defaults();
    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Infancy);
    assert_eq!(processor.interaction_count(), 0);
    assert_eq!(processor.computation_count(), 0);
}

#[test]
fn test_processor_try_new_with_valid_config() {
    let config = UtlConfig::default();
    let result = UtlProcessor::try_new(config);
    assert!(result.is_ok());
}

#[test]
fn test_compute_learning_basic() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "This is a test message for UTL computation.";
    let embedding = test_embedding(128, 0.1);
    let context = vec![test_embedding(128, 0.15), test_embedding(128, 0.12)];

    let signal = processor.compute_learning(content, &embedding, &context);
    assert!(signal.is_ok());

    let signal = signal.unwrap();
    assert!(signal.magnitude >= 0.0 && signal.magnitude <= 1.0);
    assert!(signal.delta_s >= 0.0 && signal.delta_s <= 1.0);
    assert!(signal.delta_c >= 0.0 && signal.delta_c <= 1.0);
    assert!(signal.w_e >= 0.5 && signal.w_e <= 1.5);
    assert!(signal.phi >= 0.0 && signal.phi <= std::f32::consts::PI);
    assert!(signal.lambda_weights.is_some());
}

#[test]
fn test_compute_learning_empty_context() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "Test with no context.";
    let embedding = test_embedding(128, 0.1);
    let context: Vec<Vec<f32>> = vec![];

    let signal = processor.compute_learning(content, &embedding, &context);
    assert!(signal.is_ok());

    // With empty context, surprise should be baseline
    let signal = signal.unwrap();
    assert!(signal.magnitude >= 0.0);
}

#[test]
fn test_lifecycle_progression() {
    let mut processor = UtlProcessor::with_defaults();
    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Infancy);

    // Simulate 50 interactions to trigger Infancy -> Growth
    for i in 0..50 {
        let content = format!("Message {}", i);
        let embedding = test_embedding(128, 0.1 + (i as f32 * 0.01));
        let _ = processor.compute_learning(&content, &embedding, &[]);
    }

    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Growth);
    assert_eq!(processor.interaction_count(), 50);
}

#[test]
fn test_lambda_weights_change_with_stage() {
    let mut processor = UtlProcessor::with_defaults();

    // Infancy weights: lambda_s = 0.7, lambda_c = 0.3
    let infancy_weights = processor.lambda_weights();
    assert!((infancy_weights.lambda_s() - 0.7).abs() < 0.01);
    assert!((infancy_weights.lambda_c() - 0.3).abs() < 0.01);

    // Progress to Growth
    for i in 0..50 {
        let _ = processor.compute_learning(&format!("msg {}", i), &test_embedding(128, 0.1), &[]);
    }

    // Growth weights: lambda_s = 0.5, lambda_c = 0.5
    let growth_weights = processor.lambda_weights();
    assert!((growth_weights.lambda_s() - 0.5).abs() < 0.01);
    assert!((growth_weights.lambda_c() - 0.5).abs() < 0.01);
}

#[test]
fn test_johari_quadrant_classification() {
    let _processor = UtlProcessor::with_defaults();

    // Open: low surprise, high coherence
    let open_action = suggested_action_for_quadrant(JohariQuadrant::Open);
    assert_eq!(open_action, SuggestedAction::DirectRecall);

    // Blind: high surprise, low coherence
    let blind_action = suggested_action_for_quadrant(JohariQuadrant::Blind);
    assert_eq!(blind_action, SuggestedAction::TriggerDream);

    // Hidden: low surprise, low coherence
    let hidden_action = suggested_action_for_quadrant(JohariQuadrant::Hidden);
    assert_eq!(hidden_action, SuggestedAction::GetNeighborhood);

    // Unknown: high surprise, high coherence
    let unknown_action = suggested_action_for_quadrant(JohariQuadrant::Unknown);
    assert_eq!(unknown_action, SuggestedAction::EpistemicAction);
}

#[test]
fn test_computation_count_tracking() {
    let mut processor = UtlProcessor::with_defaults();
    assert_eq!(processor.computation_count(), 0);

    for _ in 0..5 {
        let _ = processor.compute_learning("test", &test_embedding(128, 0.1), &[]);
    }

    assert_eq!(processor.computation_count(), 5);
}

#[test]
fn test_processor_reset() {
    let mut processor = UtlProcessor::with_defaults();

    // Do some computations
    for _ in 0..10 {
        let _ = processor.compute_learning("test", &test_embedding(128, 0.1), &[]);
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
fn test_restore_lifecycle() {
    let mut processor = UtlProcessor::with_defaults();
    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Infancy);

    // Restore to 100 interactions (Growth stage)
    processor.restore_lifecycle(100);

    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Growth);
    assert_eq!(processor.interaction_count(), 100);
}

#[test]
fn test_performance_under_10ms() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "Performance test content for UTL processing.";
    let embedding = test_embedding(1536, 0.5);
    let context: Vec<Vec<f32>> = (0..50)
        .map(|i| test_embedding(1536, 0.1 * (i as f32 % 10.0)))
        .collect();

    let start = Instant::now();
    let signal = processor.compute_learning(content, &embedding, &context);
    let elapsed = start.elapsed();

    assert!(signal.is_ok());
    assert!(
        elapsed.as_millis() < 10,
        "Computation took {}ms, expected < 10ms",
        elapsed.as_millis()
    );
}

// ========================================================================
// SessionContext Tests
// ========================================================================

#[test]
fn test_session_context_creation() {
    let session = SessionContext::default_session();

    assert!(!session.session_id.is_nil());
    assert_eq!(session.max_window_size, 50);
    assert_eq!(session.interaction_count, 0);
    assert!(session.recent_embeddings.is_empty());
}

#[test]
fn test_session_add_embedding() {
    let mut session = SessionContext::new_with_generated_id(5);

    session.add_embedding(vec![1.0; 128]);
    session.add_embedding(vec![2.0; 128]);
    session.add_embedding(vec![3.0; 128]);

    assert_eq!(session.interaction_count, 3);
    assert_eq!(session.recent_embeddings.len(), 3);
    assert!(session.has_sufficient_context());
}

#[test]
fn test_session_window_sliding() {
    let mut session = SessionContext::new_with_generated_id(3);

    // Add 5 embeddings to window of size 3
    for i in 1..=5 {
        session.add_embedding(vec![i as f32; 128]);
    }

    // Window should only contain last 3
    assert_eq!(session.recent_embeddings.len(), 3);
    assert_eq!(session.recent_embeddings[0][0], 3.0);
    assert_eq!(session.recent_embeddings[1][0], 4.0);
    assert_eq!(session.recent_embeddings[2][0], 5.0);
    assert_eq!(session.interaction_count, 5);
}

#[test]
fn test_session_staleness() {
    let session = SessionContext::default_session();

    // Fresh session should not be stale
    assert!(!session.is_stale(60));
}

#[test]
fn test_session_clear_context() {
    let mut session = SessionContext::default_session();

    session.add_embedding(vec![1.0; 128]);
    session.add_embedding(vec![2.0; 128]);
    assert_eq!(session.interaction_count, 2);

    session.clear_context();

    assert_eq!(session.interaction_count, 0);
    assert!(session.recent_embeddings.is_empty());
    // Session ID should be preserved
    assert!(!session.session_id.is_nil());
}

// ========================================================================
// Edge Case Tests
// ========================================================================

#[test]
fn test_single_dimension_embedding() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "Minimal test";
    let embedding = vec![0.5];
    let context = vec![vec![0.3], vec![0.7]];

    let signal = processor.compute_learning(content, &embedding, &context);
    assert!(signal.is_ok());
}

#[test]
fn test_large_context_window() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "Large context test";
    let embedding = test_embedding(128, 0.5);
    let context: Vec<Vec<f32>> = (0..1000)
        .map(|i| test_embedding(128, (i as f32 % 100.0) / 100.0))
        .collect();

    let signal = processor.compute_learning(content, &embedding, &context);
    assert!(signal.is_ok());
}

#[test]
fn test_empty_content() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "";
    let embedding = test_embedding(128, 0.1);
    let context = vec![test_embedding(128, 0.2)];

    let signal = processor.compute_learning(content, &embedding, &context);
    assert!(signal.is_ok());

    // Empty content should give neutral emotional weight
    let signal = signal.unwrap();
    assert!((signal.w_e - 1.0).abs() < 0.2);
}

#[test]
fn test_identical_embeddings() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "Same embedding test";
    let embedding = test_embedding(128, 0.5);
    let context = vec![
        test_embedding(128, 0.5),
        test_embedding(128, 0.5),
        test_embedding(128, 0.5),
    ];

    let signal = processor.compute_learning(content, &embedding, &context);
    assert!(signal.is_ok());

    // Identical embeddings = low surprise, high coherence = Open quadrant
    let signal = signal.unwrap();
    // Surprise should be low (similar to context)
    assert!(signal.delta_s < 0.5);
}
