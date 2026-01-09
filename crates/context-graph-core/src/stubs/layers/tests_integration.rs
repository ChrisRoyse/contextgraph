//! Integration and cross-layer tests for stub layers.
//!
//! This module contains tests that verify:
//! - Stubs return NotImplemented per AP-007 (fail-fast)
//! - Layer identifiers are correctly reported
//! - Health check returns false for unimplemented stubs

use crate::error::CoreError;
use crate::traits::NervousLayer;
use crate::types::{LayerId, LayerInput};

use super::{
    StubCoherenceLayer, StubLearningLayer, StubMemoryLayer, StubReflexLayer, StubSensingLayer,
};

/// Helper to create test input for integration testing.
fn test_input(content: &str) -> LayerInput {
    LayerInput::new("test-request-123".to_string(), content.to_string())
}

#[tokio::test]
async fn test_all_layers_unhealthy() {
    // Per AP-007: Stubs must report unhealthy since they are not real implementations
    let sensing = StubSensingLayer::new();
    let reflex = StubReflexLayer::new();
    let memory = StubMemoryLayer::new();
    let learning = StubLearningLayer::new();
    let coherence = StubCoherenceLayer::new();

    assert!(
        !sensing.health_check().await.unwrap(),
        "Sensing stub should be unhealthy"
    );
    assert!(
        !reflex.health_check().await.unwrap(),
        "Reflex stub should be unhealthy"
    );
    assert!(
        !memory.health_check().await.unwrap(),
        "Memory stub should be unhealthy"
    );
    assert!(
        !learning.health_check().await.unwrap(),
        "Learning stub should be unhealthy"
    );
    assert!(
        !coherence.health_check().await.unwrap(),
        "Coherence stub should be unhealthy"
    );
}

#[tokio::test]
async fn test_pipeline_all_fail_fast() {
    // Per AP-007: All stubs must return NotImplemented error - no mock data
    let sensing = StubSensingLayer::new();
    let reflex = StubReflexLayer::new();
    let memory = StubMemoryLayer::new();
    let learning = StubLearningLayer::new();
    let coherence = StubCoherenceLayer::new();

    let input = test_input("pipeline test");

    // All layers must fail with NotImplemented
    let r1 = sensing.process(input.clone()).await;
    let r2 = reflex.process(input.clone()).await;
    let r3 = memory.process(input.clone()).await;
    let r4 = learning.process(input.clone()).await;
    let r5 = coherence.process(input.clone()).await;

    assert!(r1.is_err(), "Sensing must fail");
    assert!(r2.is_err(), "Reflex must fail");
    assert!(r3.is_err(), "Memory must fail");
    assert!(r4.is_err(), "Learning must fail");
    assert!(r5.is_err(), "Coherence must fail");

    // Verify all are NotImplemented errors
    assert!(matches!(r1, Err(CoreError::NotImplemented(_))));
    assert!(matches!(r2, Err(CoreError::NotImplemented(_))));
    assert!(matches!(r3, Err(CoreError::NotImplemented(_))));
    assert!(matches!(r4, Err(CoreError::NotImplemented(_))));
    assert!(matches!(r5, Err(CoreError::NotImplemented(_))));
}

#[tokio::test]
async fn test_layer_errors_reference_documentation() {
    // Verify that NotImplemented errors include documentation reference
    let layers: Vec<Box<dyn NervousLayer>> = vec![
        Box::new(StubSensingLayer::new()),
        Box::new(StubReflexLayer::new()),
        Box::new(StubMemoryLayer::new()),
        Box::new(StubLearningLayer::new()),
        Box::new(StubCoherenceLayer::new()),
    ];

    for layer in layers {
        let input = LayerInput::new("test-request".to_string(), "test content".to_string());
        let result = layer.process(input).await;

        if let Err(CoreError::NotImplemented(msg)) = result {
            assert!(
                msg.contains("docs2") || msg.contains("See:"),
                "{} error should reference documentation, got: {}",
                layer.layer_name(),
                msg
            );
        } else {
            panic!("{} should return NotImplemented error", layer.layer_name());
        }
    }
}

#[tokio::test]
async fn test_layer_id_correctly_reported() {
    // Verify layer_id() method works correctly even though process() fails
    let test_cases = vec![
        (
            Box::new(StubSensingLayer::new()) as Box<dyn NervousLayer>,
            LayerId::Sensing,
        ),
        (Box::new(StubReflexLayer::new()), LayerId::Reflex),
        (Box::new(StubMemoryLayer::new()), LayerId::Memory),
        (Box::new(StubLearningLayer::new()), LayerId::Learning),
        (Box::new(StubCoherenceLayer::new()), LayerId::Coherence),
    ];

    for (layer, expected_id) in test_cases {
        assert_eq!(
            layer.layer_id(),
            expected_id,
            "{} must report correct LayerId {:?}",
            layer.layer_name(),
            expected_id
        );
    }
}

#[tokio::test]
async fn test_layer_names_indicate_not_implemented() {
    let layers: Vec<Box<dyn NervousLayer>> = vec![
        Box::new(StubSensingLayer::new()),
        Box::new(StubReflexLayer::new()),
        Box::new(StubMemoryLayer::new()),
        Box::new(StubLearningLayer::new()),
        Box::new(StubCoherenceLayer::new()),
    ];

    for layer in layers {
        let name = layer.layer_name();
        assert!(
            name.contains("NOT IMPLEMENTED"),
            "Layer name should indicate NOT IMPLEMENTED: {}",
            name
        );
    }
}

#[tokio::test]
async fn test_latency_budgets_defined() {
    // Even though stubs fail, they should still have latency budgets defined
    let layers: Vec<(Box<dyn NervousLayer>, u64)> = vec![
        (Box::new(StubSensingLayer::new()), 5),    // 5ms
        (Box::new(StubReflexLayer::new()), 0),     // 100us (< 1ms)
        (Box::new(StubMemoryLayer::new()), 1),     // 1ms
        (Box::new(StubLearningLayer::new()), 10),  // 10ms
        (Box::new(StubCoherenceLayer::new()), 10), // 10ms
    ];

    for (layer, expected_ms) in layers {
        let budget = layer.latency_budget();
        if expected_ms > 0 {
            assert_eq!(
                budget.as_millis() as u64,
                expected_ms,
                "{} has unexpected latency budget",
                layer.layer_name()
            );
        } else {
            // Reflex layer budget is 100us
            assert_eq!(
                budget.as_micros() as u64,
                100,
                "Reflex layer should have 100us budget"
            );
        }
    }
}
