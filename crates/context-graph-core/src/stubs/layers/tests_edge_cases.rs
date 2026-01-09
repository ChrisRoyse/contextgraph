//! Edge case tests for modularized layers verification.
//!
//! This module tests boundary conditions to verify the modularization
//! maintains correct behavior in edge cases.
//!
//! NOTE: Per AP-007, stubs now return NotImplemented errors (fail-fast).
//! These tests verify that failure occurs regardless of input.

use crate::error::CoreError;
use crate::traits::NervousLayer;
use crate::types::LayerInput;

use super::{
    StubCoherenceLayer, StubLearningLayer, StubMemoryLayer, StubReflexLayer, StubSensingLayer,
};

// =============================================================================
// EDGE CASE 1: Empty Input Handling - Should Still Fail Fast
// =============================================================================

#[tokio::test]
async fn test_edge_case_empty_input_sensing() {
    println!("\n=== EDGE CASE 1: Empty Input - Sensing Layer ===");
    let layer = StubSensingLayer::new();
    let input = LayerInput::new("test".to_string(), "".to_string());

    println!(
        "BEFORE STATE: input.content = \"{}\" (len={})",
        input.content,
        input.content.len()
    );

    let result = layer.process(input).await;

    println!("AFTER STATE: result.is_err() = {}", result.is_err());

    // SOURCE OF TRUTH VERIFICATION: AP-007 - No mock data in production
    assert!(
        result.is_err(),
        "Empty input should still fail (NotImplemented)"
    );
    assert!(matches!(result, Err(CoreError::NotImplemented(_))));

    println!("VERIFICATION: Empty input correctly triggers NotImplemented error");
}

#[tokio::test]
async fn test_edge_case_empty_input_all_layers() {
    println!("\n=== EDGE CASE 1b: Empty Input - All Layers ===");

    let layers: Vec<(Box<dyn NervousLayer>, &str)> = vec![
        (Box::new(StubSensingLayer::new()), "Sensing"),
        (Box::new(StubReflexLayer::new()), "Reflex"),
        (Box::new(StubMemoryLayer::new()), "Memory"),
        (Box::new(StubLearningLayer::new()), "Learning"),
        (Box::new(StubCoherenceLayer::new()), "Coherence"),
    ];

    for (layer, name) in layers {
        let input = LayerInput::new("test".to_string(), "".to_string());
        let result = layer.process(input).await;

        println!("{}: is_err={}", name, result.is_err());

        // AP-007: All stubs must fail fast
        assert!(result.is_err(), "{} should fail with NotImplemented", name);
    }

    println!("VERIFICATION: All layers correctly return NotImplemented for empty input");
}

// =============================================================================
// EDGE CASE 2: Large Input Handling - Should Still Fail Fast
// =============================================================================

#[tokio::test]
async fn test_edge_case_large_input() {
    println!("\n=== EDGE CASE 2: Large Input (10KB) ===");

    let large_content = "x".repeat(10_000);
    let layer = StubSensingLayer::new();
    let input = LayerInput::new("test".to_string(), large_content.clone());

    println!(
        "BEFORE STATE: input.content.len() = {} bytes",
        input.content.len()
    );

    let result = layer.process(input).await;

    println!("AFTER STATE: result.is_err() = {}", result.is_err());

    // SOURCE OF TRUTH VERIFICATION: AP-007 - No mock data regardless of input size
    assert!(
        result.is_err(),
        "Large input should still fail (NotImplemented)"
    );
    assert!(matches!(result, Err(CoreError::NotImplemented(_))));

    println!("VERIFICATION: Large input correctly triggers NotImplemented error");
}

// =============================================================================
// EDGE CASE 3: Various Inputs All Trigger NotImplemented
// =============================================================================

#[tokio::test]
async fn test_edge_case_various_inputs() {
    println!("\n=== EDGE CASE 3: Various Input Patterns ===");

    let layers: Vec<(Box<dyn NervousLayer>, &str)> = vec![
        (Box::new(StubSensingLayer::new()), "Sensing"),
        (Box::new(StubReflexLayer::new()), "Reflex"),
        (Box::new(StubMemoryLayer::new()), "Memory"),
        (Box::new(StubLearningLayer::new()), "Learning"),
        (Box::new(StubCoherenceLayer::new()), "Coherence"),
    ];

    let large_input = "x".repeat(1000);
    let test_inputs = vec!["", "a", "test", "boundary value test", large_input.as_str()];

    for (layer, name) in &layers {
        for test_content in &test_inputs {
            let input = LayerInput::new("test".to_string(), test_content.to_string());
            let result = layer.process(input).await;

            // SOURCE OF TRUTH VERIFICATION: AP-007 - All inputs fail
            assert!(
                result.is_err(),
                "{} should fail for input len {}",
                name,
                test_content.len()
            );
        }
        println!("{}: All inputs correctly trigger NotImplemented", name);
    }

    println!("\nVERIFICATION: All input patterns correctly return NotImplemented error");
}

// =============================================================================
// EDGE CASE 4: Health Check Returns Unhealthy
// =============================================================================

#[tokio::test]
async fn test_edge_case_health_check() {
    println!("\n=== EDGE CASE 4: Health Check Returns Unhealthy ===");

    let layers: Vec<(Box<dyn NervousLayer>, &str)> = vec![
        (Box::new(StubSensingLayer::new()), "Sensing"),
        (Box::new(StubReflexLayer::new()), "Reflex"),
        (Box::new(StubMemoryLayer::new()), "Memory"),
        (Box::new(StubLearningLayer::new()), "Learning"),
        (Box::new(StubCoherenceLayer::new()), "Coherence"),
    ];

    for (layer, name) in layers {
        let health = layer.health_check().await.unwrap();
        println!("{}: healthy={}", name, health);

        // Stubs should report unhealthy since they aren't real implementations
        assert!(!health, "{} stub should report unhealthy", name);
    }

    println!("\nVERIFICATION: All stubs correctly report unhealthy");
}

// =============================================================================
// SOURCE OF TRUTH VERIFICATION: Module Structure
// =============================================================================

#[test]
fn test_source_of_truth_module_structure() {
    println!("\n=== SOURCE OF TRUTH: Module Re-exports ===");

    // Verify all types are accessible from the expected paths
    let _sensing: StubSensingLayer = StubSensingLayer::new();
    let _reflex: StubReflexLayer = StubReflexLayer::new();
    let _memory: StubMemoryLayer = StubMemoryLayer::new();
    let _learning: StubLearningLayer = StubLearningLayer::new();
    let _coherence: StubCoherenceLayer = StubCoherenceLayer::new();

    println!("StubSensingLayer: accessible");
    println!("StubReflexLayer: accessible");
    println!("StubMemoryLayer: accessible");
    println!("StubLearningLayer: accessible");
    println!("StubCoherenceLayer: accessible");

    println!("\nVERIFICATION: All layer types properly re-exported");
}

// =============================================================================
// SOURCE OF TRUTH VERIFICATION: Layer Names Show NOT IMPLEMENTED
// =============================================================================

#[test]
fn test_source_of_truth_layer_names() {
    println!("\n=== SOURCE OF TRUTH: Layer Names ===");

    let layers: Vec<(Box<dyn NervousLayer>, &str)> = vec![
        (Box::new(StubSensingLayer::new()), "Sensing"),
        (Box::new(StubReflexLayer::new()), "Reflex"),
        (Box::new(StubMemoryLayer::new()), "Memory"),
        (Box::new(StubLearningLayer::new()), "Learning"),
        (Box::new(StubCoherenceLayer::new()), "Coherence"),
    ];

    for (layer, expected_type) in layers {
        let name = layer.layer_name();
        println!("{}: layer_name = \"{}\"", expected_type, name);

        // Name should indicate NOT IMPLEMENTED
        assert!(
            name.contains("NOT IMPLEMENTED"),
            "{} layer name should indicate NOT IMPLEMENTED",
            expected_type
        );
    }

    println!("\nVERIFICATION: All layer names correctly indicate NOT IMPLEMENTED");
}
