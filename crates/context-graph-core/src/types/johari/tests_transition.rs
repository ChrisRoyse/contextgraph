//! Tests for TransitionTrigger enum.

use super::*;

#[test]
fn test_transition_trigger_all_variants() {
    let all = TransitionTrigger::all();
    assert_eq!(
        all.len(),
        6,
        "TransitionTrigger should have exactly 6 variants"
    );
    assert!(all.contains(&TransitionTrigger::ExplicitShare));
    assert!(all.contains(&TransitionTrigger::SelfRecognition));
    assert!(all.contains(&TransitionTrigger::PatternDiscovery));
    assert!(all.contains(&TransitionTrigger::Privatize));
    assert!(all.contains(&TransitionTrigger::ExternalObservation));
    assert!(all.contains(&TransitionTrigger::DreamConsolidation));
}

#[test]
fn test_transition_trigger_description_not_empty() {
    for trigger in TransitionTrigger::all() {
        let desc = trigger.description();
        assert!(!desc.is_empty(), "Description empty for {:?}", trigger);
        assert!(desc.len() > 10, "Description too short for {:?}", trigger);
    }
}

#[test]
fn test_transition_trigger_display() {
    assert_eq!(
        format!("{}", TransitionTrigger::ExplicitShare),
        "ExplicitShare"
    );
    assert_eq!(
        format!("{}", TransitionTrigger::SelfRecognition),
        "SelfRecognition"
    );
    assert_eq!(
        format!("{}", TransitionTrigger::PatternDiscovery),
        "PatternDiscovery"
    );
    assert_eq!(format!("{}", TransitionTrigger::Privatize), "Privatize");
    assert_eq!(
        format!("{}", TransitionTrigger::ExternalObservation),
        "ExternalObservation"
    );
    assert_eq!(
        format!("{}", TransitionTrigger::DreamConsolidation),
        "DreamConsolidation"
    );
}

#[test]
fn test_transition_trigger_serde_roundtrip() {
    for trigger in TransitionTrigger::all() {
        let json = serde_json::to_string(&trigger).expect("serialize failed");
        let parsed: TransitionTrigger = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(trigger, parsed, "Roundtrip failed for {:?}", trigger);
    }
}

#[test]
fn test_transition_trigger_serde_snake_case() {
    assert_eq!(
        serde_json::to_string(&TransitionTrigger::ExplicitShare).unwrap(),
        "\"explicit_share\""
    );
    assert_eq!(
        serde_json::to_string(&TransitionTrigger::SelfRecognition).unwrap(),
        "\"self_recognition\""
    );
    assert_eq!(
        serde_json::to_string(&TransitionTrigger::PatternDiscovery).unwrap(),
        "\"pattern_discovery\""
    );
    assert_eq!(
        serde_json::to_string(&TransitionTrigger::Privatize).unwrap(),
        "\"privatize\""
    );
    assert_eq!(
        serde_json::to_string(&TransitionTrigger::ExternalObservation).unwrap(),
        "\"external_observation\""
    );
    assert_eq!(
        serde_json::to_string(&TransitionTrigger::DreamConsolidation).unwrap(),
        "\"dream_consolidation\""
    );
}

#[test]
fn test_transition_trigger_copy() {
    let original = TransitionTrigger::ExplicitShare;
    let copied = original;
    let cloned = original;
    assert_eq!(original, copied);
    assert_eq!(original, cloned);
}

#[test]
fn test_transition_trigger_hash_consistency() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    for trigger in TransitionTrigger::all() {
        assert!(set.insert(trigger), "Duplicate hash for {:?}", trigger);
    }
    assert_eq!(set.len(), 6);
}

#[test]
fn test_johari_transition_new() {
    let memory_id = uuid::Uuid::new_v4();
    let embedder_idx = 5;
    let transition = JohariTransition::new(
        memory_id,
        embedder_idx,
        JohariQuadrant::Hidden,
        JohariQuadrant::Open,
        TransitionTrigger::ExplicitShare,
    );
    assert_eq!(transition.memory_id, memory_id);
    assert_eq!(transition.embedder_idx, embedder_idx);
    assert_eq!(transition.from, JohariQuadrant::Hidden);
    assert_eq!(transition.to, JohariQuadrant::Open);
    assert_eq!(transition.trigger, TransitionTrigger::ExplicitShare);
    let now = chrono::Utc::now();
    let diff = (now - transition.timestamp).num_seconds().abs();
    assert!(diff < 2, "Timestamp should be within 2 seconds of now");
}

#[test]
fn test_johari_transition_serde_roundtrip() {
    let memory_id = uuid::Uuid::new_v4();
    let transition = JohariTransition::new(
        memory_id,
        3,
        JohariQuadrant::Hidden,
        JohariQuadrant::Open,
        TransitionTrigger::ExplicitShare,
    );
    let json = serde_json::to_string(&transition).expect("serialize failed");
    let parsed: JohariTransition = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(transition.memory_id, parsed.memory_id);
    assert_eq!(transition.embedder_idx, parsed.embedder_idx);
    assert_eq!(transition.from, parsed.from);
    assert_eq!(transition.to, parsed.to);
    assert_eq!(transition.trigger, parsed.trigger);
    let diff = (transition.timestamp - parsed.timestamp)
        .num_milliseconds()
        .abs();
    assert!(diff < 1000, "Timestamps should be within 1 second");
}

#[test]
fn test_johari_transition_is_clone_not_copy() {
    let memory_id = uuid::Uuid::new_v4();
    let original = JohariTransition::new(
        memory_id,
        7,
        JohariQuadrant::Open,
        JohariQuadrant::Hidden,
        TransitionTrigger::Privatize,
    );
    let cloned = original.clone();
    assert_eq!(original.memory_id, cloned.memory_id);
    assert_eq!(original.embedder_idx, cloned.embedder_idx);
    assert_eq!(original.from, cloned.from);
    assert_eq!(original.to, cloned.to);
    assert_eq!(original.trigger, cloned.trigger);
}
