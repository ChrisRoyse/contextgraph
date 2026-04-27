//! Unit tests for GraphEdge constructor methods.

use super::*;
use uuid::Uuid;

// =========================================================================
// new() Constructor Tests
// =========================================================================

#[test]
fn test_new_uses_edge_type_default_weight() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Causal);
    assert_eq!(edge.weight, EdgeType::Causal.default_weight());
    assert_eq!(edge.weight, 0.8);
}

#[test]
fn test_new_sets_confidence_to_half() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    assert_eq!(edge.confidence, 0.5);
}

#[test]
fn test_new_sets_steering_reward_to_zero() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    assert_eq!(edge.steering_reward, 0.0);
}

#[test]
fn test_new_sets_traversal_count_to_zero() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    assert_eq!(edge.traversal_count, 0);
}

#[test]
fn test_new_sets_is_amortized_shortcut_false() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    assert!(!edge.is_amortized_shortcut);
}

#[test]
fn test_new_sets_last_traversed_at_none() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    assert!(edge.last_traversed_at.is_none());
}

#[test]
fn test_new_sets_domain_none() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    assert!(edge.domain.is_none());
}

#[test]
fn test_new_generates_unique_id() {
    let edge1 = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    let edge2 = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic);
    assert_ne!(edge1.id, edge2.id);
}

#[test]
fn test_new_all_edge_types() {
    for edge_type in EdgeType::all() {
        let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), edge_type);
        assert_eq!(edge.weight, edge_type.default_weight());
    }
}

// =========================================================================
// with_weight() Constructor Tests
// =========================================================================

#[test]
fn test_with_weight_sets_explicit_values() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        0.75,
        0.95,
    );
    assert_eq!(edge.weight, 0.75);
    assert_eq!(edge.confidence, 0.95);
}

#[test]
fn test_with_weight_clamps_weight_high() {
    let edge = GraphEdge::with_weight(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic, 1.5, 0.5);
    assert_eq!(edge.weight, 1.0);
}

#[test]
fn test_with_weight_clamps_weight_low() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        -0.5,
        0.5,
    );
    assert_eq!(edge.weight, 0.0);
}

#[test]
fn test_with_weight_clamps_confidence_high() {
    let edge = GraphEdge::with_weight(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic, 0.5, 1.5);
    assert_eq!(edge.confidence, 1.0);
}

#[test]
fn test_with_weight_clamps_confidence_low() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        0.5,
        -0.5,
    );
    assert_eq!(edge.confidence, 0.0);
}

#[test]
fn test_with_weight_preserves_source_target() {
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let edge = GraphEdge::with_weight(source, target, EdgeType::Causal, 0.9, 0.85);
    assert_eq!(edge.source_id, source);
    assert_eq!(edge.target_id, target);
    assert_eq!(edge.edge_type, EdgeType::Causal);
}

#[test]
fn test_with_weight_boundary_values() {
    let edge = GraphEdge::with_weight(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic, 0.0, 1.0);
    assert_eq!(edge.weight, 0.0);
    assert_eq!(edge.confidence, 1.0);
}

// =========================================================================
// with_domain() Builder Tests
// =========================================================================

#[test]
fn test_with_domain_sets_domain() {
    let edge =
        GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic).with_domain("code");
    assert_eq!(edge.domain.as_deref(), Some("code"));
}

#[test]
fn test_with_domain_from_string() {
    let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic)
        .with_domain(String::from("research"));
    assert_eq!(edge.domain.as_deref(), Some("research"));
}
