//! Edge Case Tests (per M05-T25 specification)
//!
//! Tests for boundary conditions and special input cases

use context_graph_utl::processor::UtlProcessor;

use super::helpers::{generate_context, generate_embedding, uniform_embedding};

// =============================================================================
// EDGE CASE TESTS (per M05-T25 specification)
// =============================================================================

#[test]
fn test_empty_context() {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = generate_embedding(1536, 42);
    let context: Vec<Vec<f32>> = vec![];

    let result = processor.compute_learning("test with no context", &embedding, &context);
    assert!(result.is_ok(), "empty context should produce valid signal");

    let signal = result.unwrap();
    // With empty context, computation should still succeed with valid bounds
    assert!(signal.magnitude >= 0.0 && signal.magnitude <= 1.0);
    assert!(signal.delta_s >= 0.0 && signal.delta_s <= 1.0);
    assert!(signal.delta_c >= 0.0 && signal.delta_c <= 1.0);
}

#[test]
fn test_zero_embedding() {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = vec![0.0; 1536];
    let context = generate_context(10, 1536, 100);

    let result = processor.compute_learning("test with zero embedding", &embedding, &context);
    assert!(result.is_ok(), "zero embedding should produce valid signal");

    let signal = result.unwrap();
    // Zero embedding might produce low surprise (similar to nothing)
    assert!(signal.magnitude >= 0.0 && signal.magnitude <= 1.0);
}

#[test]
fn test_empty_content() {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = generate_embedding(128, 42);
    let context = vec![generate_embedding(128, 100)];

    let result = processor.compute_learning("", &embedding, &context);
    assert!(result.is_ok(), "empty content should produce valid signal");

    let signal = result.unwrap();
    // Empty content should give neutral emotional weight (close to 1.0)
    assert!(
        (signal.w_e - 1.0).abs() < 0.3,
        "empty content should give near-neutral w_e, got {}",
        signal.w_e
    );
}

#[test]
fn test_single_context() {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = generate_embedding(1536, 42);
    let context = vec![generate_embedding(1536, 100)];

    let result = processor.compute_learning("test with single context", &embedding, &context);
    assert!(result.is_ok(), "single context should compute coherence");

    let signal = result.unwrap();
    assert!(signal.delta_c >= 0.0 && signal.delta_c <= 1.0);
}

#[test]
fn test_identical_embeddings() {
    let mut processor = UtlProcessor::with_defaults();

    // All embeddings are identical
    let embedding = uniform_embedding(128, 0.5);
    let context = vec![
        uniform_embedding(128, 0.5),
        uniform_embedding(128, 0.5),
        uniform_embedding(128, 0.5),
    ];

    let result = processor.compute_learning("identical test", &embedding, &context);
    assert!(result.is_ok());

    let signal = result.unwrap();
    // Identical embeddings should have low surprise
    assert!(
        signal.delta_s < 0.5,
        "identical embeddings should have low surprise, got {}",
        signal.delta_s
    );
}
