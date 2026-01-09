//! Tests for SuggestedAction enum.

use std::collections::HashSet;

use crate::types::pulse::SuggestedAction;

#[test]
fn test_suggested_action_default_is_continue() {
    let action = SuggestedAction::default();
    assert_eq!(action, SuggestedAction::Continue);
}

#[test]
fn test_suggested_action_serde_roundtrip() {
    let actions = [
        SuggestedAction::Ready,
        SuggestedAction::Continue,
        SuggestedAction::Explore,
        SuggestedAction::Consolidate,
        SuggestedAction::Prune,
        SuggestedAction::Stabilize,
        SuggestedAction::Review,
    ];
    for action in actions {
        let json = serde_json::to_string(&action).unwrap();
        let parsed: SuggestedAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, parsed);
    }
}

#[test]
fn test_suggested_action_serde_snake_case() {
    // Verify snake_case serialization
    let json = serde_json::to_string(&SuggestedAction::Ready).unwrap();
    assert_eq!(json, "\"ready\"");

    let json = serde_json::to_string(&SuggestedAction::Continue).unwrap();
    assert_eq!(json, "\"continue\"");
}

#[test]
fn test_suggested_action_descriptions_not_empty() {
    let actions = [
        SuggestedAction::Ready,
        SuggestedAction::Continue,
        SuggestedAction::Explore,
        SuggestedAction::Consolidate,
        SuggestedAction::Prune,
        SuggestedAction::Stabilize,
        SuggestedAction::Review,
    ];
    for action in actions {
        let desc = action.description();
        assert!(!desc.is_empty(), "{:?} has empty description", action);
        assert!(
            desc.len() > 20,
            "{:?} description too short: {}",
            action,
            desc
        );
    }
}

#[test]
fn test_suggested_action_descriptions_unique() {
    let actions = [
        SuggestedAction::Ready,
        SuggestedAction::Continue,
        SuggestedAction::Explore,
        SuggestedAction::Consolidate,
        SuggestedAction::Prune,
        SuggestedAction::Stabilize,
        SuggestedAction::Review,
    ];
    let descriptions: HashSet<_> = actions.iter().map(|a| a.description()).collect();
    assert_eq!(
        descriptions.len(),
        actions.len(),
        "Descriptions must be unique"
    );
}

#[test]
fn test_suggested_action_copy_semantics() {
    let action = SuggestedAction::Explore;
    let copied = action; // Copy, not move
    assert_eq!(action, copied);
    assert_eq!(action.description(), copied.description());
}

#[test]
fn test_suggested_action_hash() {
    let mut set = HashSet::new();
    set.insert(SuggestedAction::Ready);
    set.insert(SuggestedAction::Continue);
    set.insert(SuggestedAction::Ready); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_suggested_action_invalid_serde_rejected() {
    // Verify invalid variant is correctly rejected
    let json = "\"unknown_action\"";
    let result: Result<SuggestedAction, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Invalid variant should be rejected");
}

#[test]
fn test_suggested_action_descriptions_contain_mcp_tools() {
    // Verify key actions have MCP tool guidance
    assert!(SuggestedAction::Explore
        .description()
        .contains("epistemic_action"));
    assert!(SuggestedAction::Explore
        .description()
        .contains("trigger_dream"));
    assert!(SuggestedAction::Consolidate
        .description()
        .contains("trigger_dream"));
    assert!(SuggestedAction::Consolidate
        .description()
        .contains("merge_concepts"));
    assert!(SuggestedAction::Stabilize
        .description()
        .contains("critique_context"));
    assert!(SuggestedAction::Review
        .description()
        .contains("reflect_on_memory"));
}
