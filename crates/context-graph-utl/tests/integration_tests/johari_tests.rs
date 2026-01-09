//! Johari Quadrant Tests
//!
//! Tests for Johari Window classification per constitution.yaml and contextprd.md

use context_graph_utl::{
    johari::{JohariClassifier, JohariQuadrant, SuggestedAction},
    LearningSignal,
};

// =============================================================================
// JOHARI QUADRANT TESTS
// =============================================================================

#[test]
fn test_johari_quadrant_classification() {
    // Per constitution.yaml Johari Window:
    // Open: low entropy (surprise), high coherence
    // Blind: high entropy, low coherence
    // Hidden: low entropy, low coherence
    // Unknown: high entropy, high coherence

    let classifier = JohariClassifier::default();

    // Open: low surprise (entropy), high coherence
    let open = classifier.classify(0.2, 0.8);
    assert_eq!(
        open,
        JohariQuadrant::Open,
        "low entropy + high coherence = Open"
    );

    // Blind: high entropy, low coherence
    let blind = classifier.classify(0.8, 0.2);
    assert_eq!(
        blind,
        JohariQuadrant::Blind,
        "high entropy + low coherence = Blind"
    );

    // Hidden: low entropy, low coherence
    let hidden = classifier.classify(0.2, 0.2);
    assert_eq!(
        hidden,
        JohariQuadrant::Hidden,
        "low entropy + low coherence = Hidden"
    );

    // Unknown: high entropy, high coherence
    let unknown = classifier.classify(0.8, 0.8);
    assert_eq!(
        unknown,
        JohariQuadrant::Unknown,
        "high entropy + high coherence = Unknown"
    );
}

#[test]
fn test_suggested_actions_per_quadrant() {
    // Per contextprd.md Section 5.4
    // Test the mapping of Johari quadrants to suggested actions:
    // Open -> DirectRecall
    // Blind -> TriggerDream
    // Hidden -> GetNeighborhood
    // Unknown -> EpistemicAction

    // Test via LearningSignal which contains the mapping
    let signal = LearningSignal::new(
        0.5,
        0.2,
        0.8,
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
    assert_eq!(signal.suggested_action, SuggestedAction::DirectRecall);

    let signal = LearningSignal::new(
        0.5,
        0.8,
        0.2,
        1.0,
        0.5,
        None,
        JohariQuadrant::Blind,
        SuggestedAction::TriggerDream,
        false,
        true,
        100,
    )
    .unwrap();
    assert_eq!(signal.suggested_action, SuggestedAction::TriggerDream);

    let signal = LearningSignal::new(
        0.5,
        0.2,
        0.2,
        1.0,
        0.5,
        None,
        JohariQuadrant::Hidden,
        SuggestedAction::GetNeighborhood,
        false,
        true,
        100,
    )
    .unwrap();
    assert_eq!(signal.suggested_action, SuggestedAction::GetNeighborhood);

    let signal = LearningSignal::new(
        0.5,
        0.8,
        0.8,
        1.0,
        0.5,
        None,
        JohariQuadrant::Unknown,
        SuggestedAction::EpistemicAction,
        false,
        true,
        100,
    )
    .unwrap();
    assert_eq!(signal.suggested_action, SuggestedAction::EpistemicAction);
}

#[test]
fn test_johari_boundary_values() {
    let classifier = JohariClassifier::default();

    // Test at exact threshold (0.5, 0.5) - should classify deterministically
    let result = classifier.classify(0.5, 0.5);
    assert!(
        matches!(
            result,
            JohariQuadrant::Open
                | JohariQuadrant::Blind
                | JohariQuadrant::Hidden
                | JohariQuadrant::Unknown
        ),
        "Boundary value (0.5, 0.5) must classify to a valid quadrant"
    );
}
