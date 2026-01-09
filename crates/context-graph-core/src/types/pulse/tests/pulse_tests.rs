//! Tests for CognitivePulse struct.

use chrono::Utc;

use crate::types::nervous::LayerId;
use crate::types::pulse::{CognitivePulse, SuggestedAction};
use crate::types::utl::{EmotionalState, UtlMetrics};

#[test]
fn test_pulse_default() {
    let pulse = CognitivePulse::default();
    assert_eq!(pulse.entropy, 0.5);
    assert_eq!(pulse.coherence, 0.5);
    assert_eq!(pulse.coherence_delta, 0.0);
    assert_eq!(pulse.emotional_weight, 1.0);
    assert_eq!(pulse.suggested_action, SuggestedAction::Continue);
    assert!(pulse.source_layer.is_none());
    // Timestamp should be recent (within last second)
    let now = Utc::now();
    let diff = now.signed_duration_since(pulse.timestamp);
    assert!(diff.num_seconds().abs() < 2);
}

#[test]
fn test_pulse_new_with_all_fields() {
    let pulse = CognitivePulse::new(
        0.5,
        0.7,
        0.1,
        1.2,
        SuggestedAction::Explore,
        Some(LayerId::Learning),
    );
    assert_eq!(pulse.entropy, 0.5);
    assert_eq!(pulse.coherence, 0.7);
    assert_eq!(pulse.coherence_delta, 0.1);
    assert_eq!(pulse.emotional_weight, 1.2);
    assert_eq!(pulse.suggested_action, SuggestedAction::Explore);
    assert_eq!(pulse.source_layer, Some(LayerId::Learning));
}

#[test]
fn test_pulse_computed_from_metrics() {
    let metrics = UtlMetrics {
        entropy: 0.3,
        coherence: 0.8,
        learning_score: 0.5,
        surprise: 0.4,
        coherence_change: 0.15,
        emotional_weight: 1.3,
        alignment: 0.9,
    };

    let pulse = CognitivePulse::computed(&metrics, Some(LayerId::Coherence));

    assert_eq!(pulse.entropy, 0.3);
    assert_eq!(pulse.coherence, 0.8);
    assert_eq!(pulse.coherence_delta, 0.15);
    assert_eq!(pulse.emotional_weight, 1.3);
    assert_eq!(pulse.suggested_action, SuggestedAction::Ready); // low entropy, high coherence
    assert_eq!(pulse.source_layer, Some(LayerId::Coherence));
}

#[test]
fn test_pulse_from_values() {
    let pulse = CognitivePulse::from_values(0.9, 0.2);
    assert_eq!(pulse.entropy, 0.9);
    assert_eq!(pulse.coherence, 0.2);
    assert_eq!(pulse.coherence_delta, 0.0);
    assert_eq!(pulse.emotional_weight, 1.0);
    assert_eq!(pulse.suggested_action, SuggestedAction::Stabilize);
    assert!(pulse.source_layer.is_none());
}

#[test]
fn test_pulse_with_emotion() {
    let pulse =
        CognitivePulse::with_emotion(0.5, 0.6, EmotionalState::Focused, Some(LayerId::Memory));
    assert_eq!(pulse.entropy, 0.5);
    assert_eq!(pulse.coherence, 0.6);
    assert_eq!(pulse.coherence_delta, 0.0);
    assert_eq!(pulse.emotional_weight, 1.3); // Focused weight
    assert_eq!(pulse.source_layer, Some(LayerId::Memory));
}

#[test]
fn test_pulse_computed_stabilize() {
    let pulse = CognitivePulse::from_values(0.9, 0.2);
    assert_eq!(pulse.suggested_action, SuggestedAction::Stabilize);
}

#[test]
fn test_pulse_computed_ready() {
    let pulse = CognitivePulse::from_values(0.3, 0.8);
    assert_eq!(pulse.suggested_action, SuggestedAction::Ready);
}

#[test]
fn test_is_healthy() {
    let healthy = CognitivePulse::from_values(0.5, 0.6);
    assert!(healthy.is_healthy());

    let unhealthy = CognitivePulse::from_values(0.9, 0.2);
    assert!(!unhealthy.is_healthy());
}

#[test]
fn test_pulse_clamps_values() {
    let pulse = CognitivePulse::new(
        1.5,  // should clamp to 1.0
        -0.5, // should clamp to 0.0
        2.0,  // should clamp to 1.0
        3.0,  // should clamp to 2.0
        SuggestedAction::Continue,
        None,
    );
    assert_eq!(pulse.entropy, 1.0);
    assert_eq!(pulse.coherence, 0.0);
    assert_eq!(pulse.coherence_delta, 1.0);
    assert_eq!(pulse.emotional_weight, 2.0);
}

#[test]
fn test_pulse_clamps_negative_coherence_delta() {
    let pulse = CognitivePulse::new(
        0.5,
        0.5,
        -2.0, // should clamp to -1.0
        1.0,
        SuggestedAction::Continue,
        None,
    );
    assert_eq!(pulse.coherence_delta, -1.0);
}

#[test]
fn test_pulse_timestamp_is_current() {
    let before = Utc::now();
    let pulse = CognitivePulse::default();
    let after = Utc::now();

    assert!(pulse.timestamp >= before);
    assert!(pulse.timestamp <= after);
}

#[test]
fn test_pulse_serde_roundtrip() {
    let pulse = CognitivePulse::new(
        0.5,
        0.7,
        0.1,
        1.2,
        SuggestedAction::Explore,
        Some(LayerId::Learning),
    );

    let json = serde_json::to_string(&pulse).unwrap();
    let parsed: CognitivePulse = serde_json::from_str(&json).unwrap();

    assert_eq!(pulse.entropy, parsed.entropy);
    assert_eq!(pulse.coherence, parsed.coherence);
    assert_eq!(pulse.coherence_delta, parsed.coherence_delta);
    assert_eq!(pulse.emotional_weight, parsed.emotional_weight);
    assert_eq!(pulse.suggested_action, parsed.suggested_action);
    assert_eq!(pulse.source_layer, parsed.source_layer);
    assert_eq!(pulse.timestamp, parsed.timestamp);
}

#[test]
fn test_pulse_all_seven_fields_present() {
    let pulse = CognitivePulse::default();

    // Verify all 7 fields exist and have valid values
    let _entropy: f32 = pulse.entropy;
    let _coherence: f32 = pulse.coherence;
    let _coherence_delta: f32 = pulse.coherence_delta;
    let _emotional_weight: f32 = pulse.emotional_weight;
    let _suggested_action: SuggestedAction = pulse.suggested_action;
    let _source_layer: Option<LayerId> = pulse.source_layer;
    let _timestamp: chrono::DateTime<Utc> = pulse.timestamp;

    // All fields are valid
    assert!(pulse.entropy >= 0.0 && pulse.entropy <= 1.0);
    assert!(pulse.coherence >= 0.0 && pulse.coherence <= 1.0);
    assert!(pulse.coherence_delta >= -1.0 && pulse.coherence_delta <= 1.0);
    assert!(pulse.emotional_weight >= 0.0 && pulse.emotional_weight <= 2.0);
}

#[test]
fn test_computed_derives_all_fields_from_metrics() {
    let metrics = UtlMetrics {
        entropy: 0.45,
        coherence: 0.75,
        learning_score: 0.6,
        surprise: 0.5,
        coherence_change: -0.1,
        emotional_weight: 0.8,
        alignment: 0.95,
    };

    let pulse = CognitivePulse::computed(&metrics, Some(LayerId::Sensing));

    // Verify derived values
    assert_eq!(pulse.entropy, metrics.entropy);
    assert_eq!(pulse.coherence, metrics.coherence);
    assert_eq!(pulse.coherence_delta, metrics.coherence_change);
    assert_eq!(pulse.emotional_weight, metrics.emotional_weight);
    assert_eq!(pulse.source_layer, Some(LayerId::Sensing));
}
