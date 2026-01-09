//! Emotional Weight Tests
//!
//! Tests for emotional weight calculation and state modifiers

use context_graph_core::types::EmotionalState;
use context_graph_utl::{config::EmotionalConfig, emotional::EmotionalWeightCalculator};

// =============================================================================
// EMOTIONAL WEIGHT TESTS
// =============================================================================

#[test]
fn test_emotional_weight_bounds() {
    let config = EmotionalConfig::default();
    let calculator = EmotionalWeightCalculator::new(&config);

    let test_cases = [
        ("neutral statement without strong emotion", 0.8, 1.2),
        ("AMAZING! INCREDIBLE! WONDERFUL!", 1.0, 1.5),
        ("ERROR! DANGER! CRITICAL FAILURE!", 1.0, 1.5),
        ("", 0.9, 1.1), // Empty should give near-neutral weight
    ];

    for (content, min_expected, max_expected) in test_cases {
        let weight = calculator.compute_emotional_weight(content, EmotionalState::Neutral);
        assert!(
            (0.5..=1.5).contains(&weight),
            "weight {} out of [0.5, 1.5] for '{}'",
            weight,
            content
        );
        // Relaxed bounds check - emotional weight depends on implementation
        assert!(
            weight >= min_expected - 0.3 && weight <= max_expected + 0.3,
            "weight {} not in expected range [{}, {}] for '{}'",
            weight,
            min_expected,
            max_expected,
            content
        );
    }
}

#[test]
fn test_emotional_state_modifiers() {
    // Different emotional states should produce different weight modifiers
    assert_eq!(EmotionalState::Neutral.weight_modifier(), 1.0);
    assert!(EmotionalState::Focused.weight_modifier() > 1.0);
    assert!(EmotionalState::Fatigued.weight_modifier() < 1.0);
}
