//! Unit tests for GraphEdge traversal, shortcut, and default trait methods.

use super::*;
use crate::marblestone::{Domain, EdgeType};
use uuid::Uuid;

// record_traversal() Tests
#[test]
fn test_record_traversal_increments_count() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert_eq!(edge.traversal_count, 0);
    edge.record_traversal();
    assert_eq!(edge.traversal_count, 1);
    edge.record_traversal();
    assert_eq!(edge.traversal_count, 2);
}

#[test]
fn test_record_traversal_updates_timestamp() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert!(edge.last_traversed_at.is_none());
    edge.record_traversal();
    assert!(edge.last_traversed_at.is_some());
}

#[test]
fn test_record_traversal_saturates() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.traversal_count = u64::MAX;
    edge.record_traversal();
    assert_eq!(edge.traversal_count, u64::MAX);
}

#[test]
fn test_record_traversal_updates_timestamp_each_time() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.record_traversal();
    let first = edge.last_traversed_at.unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    edge.record_traversal();
    let second = edge.last_traversed_at.unwrap();
    assert!(second >= first);
}

// is_reliable_shortcut() Tests
#[test]
fn test_is_reliable_shortcut_all_conditions_met() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = true;
    edge.traversal_count = 5;
    edge.steering_reward = 0.5;
    edge.confidence = 0.8;
    assert!(edge.is_reliable_shortcut());
}

#[test]
fn test_is_reliable_shortcut_fails_not_shortcut() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = false;
    edge.traversal_count = 5;
    edge.steering_reward = 0.5;
    edge.confidence = 0.8;
    assert!(!edge.is_reliable_shortcut());
}

#[test]
fn test_is_reliable_shortcut_fails_low_traversal() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = true;
    edge.traversal_count = 2;
    edge.steering_reward = 0.5;
    edge.confidence = 0.8;
    assert!(!edge.is_reliable_shortcut());
}

#[test]
fn test_is_reliable_shortcut_fails_low_reward() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = true;
    edge.traversal_count = 5;
    edge.steering_reward = 0.2;
    edge.confidence = 0.8;
    assert!(!edge.is_reliable_shortcut());
}

#[test]
fn test_is_reliable_shortcut_fails_low_confidence() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = true;
    edge.traversal_count = 5;
    edge.steering_reward = 0.5;
    edge.confidence = 0.6;
    assert!(!edge.is_reliable_shortcut());
}

#[test]
fn test_is_reliable_shortcut_boundary_traversal() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = true;
    edge.traversal_count = 3;
    edge.steering_reward = 0.5;
    edge.confidence = 0.8;
    assert!(edge.is_reliable_shortcut());
}

#[test]
fn test_is_reliable_shortcut_boundary_reward() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = true;
    edge.traversal_count = 5;
    edge.steering_reward = 0.3; // Exactly 0.3 - should FAIL (need > 0.3)
    edge.confidence = 0.8;
    assert!(!edge.is_reliable_shortcut());
}

#[test]
fn test_is_reliable_shortcut_boundary_confidence() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.is_amortized_shortcut = true;
    edge.traversal_count = 5;
    edge.steering_reward = 0.5;
    edge.confidence = 0.7;
    assert!(edge.is_reliable_shortcut());
}

// mark_as_shortcut() Tests
#[test]
fn test_mark_as_shortcut() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert!(!edge.is_amortized_shortcut);
    edge.mark_as_shortcut();
    assert!(edge.is_amortized_shortcut);
}

#[test]
fn test_mark_as_shortcut_idempotent() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.mark_as_shortcut();
    edge.mark_as_shortcut();
    assert!(edge.is_amortized_shortcut);
}

// age_seconds() Tests
#[test]
fn test_age_seconds_non_negative() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert!(edge.age_seconds() >= 0);
}

#[test]
fn test_age_seconds_increases() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    let age1 = edge.age_seconds();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let age2 = edge.age_seconds();
    assert!(age2 >= age1);
}

// Default Trait Tests
#[test]
fn test_default_uses_nil_uuids() {
    let edge = GraphEdge::default();
    assert_eq!(edge.source_id, Uuid::nil());
    assert_eq!(edge.target_id, Uuid::nil());
}

#[test]
fn test_default_uses_semantic_edge_type() {
    let edge = GraphEdge::default();
    assert_eq!(edge.edge_type, EdgeType::Semantic);
}

#[test]
fn test_default_uses_general_domain() {
    let edge = GraphEdge::default();
    assert_eq!(edge.domain, Domain::General);
}

#[test]
fn test_default_has_valid_nt_weights() {
    let edge = GraphEdge::default();
    assert!(edge.neurotransmitter_weights.validate());
}

#[test]
fn test_default_weight_matches_semantic() {
    let edge = GraphEdge::default();
    assert_eq!(edge.weight, EdgeType::Semantic.default_weight());
}

#[test]
fn test_default_steering_reward_zero() {
    let edge = GraphEdge::default();
    assert_eq!(edge.steering_reward, 0.0);
}

#[test]
fn test_default_not_shortcut() {
    let edge = GraphEdge::default();
    assert!(!edge.is_amortized_shortcut);
}

#[test]
fn test_default_traversal_count_zero() {
    let edge = GraphEdge::default();
    assert_eq!(edge.traversal_count, 0);
}

#[test]
fn test_default_last_traversed_at_none() {
    let edge = GraphEdge::default();
    assert!(edge.last_traversed_at.is_none());
}
