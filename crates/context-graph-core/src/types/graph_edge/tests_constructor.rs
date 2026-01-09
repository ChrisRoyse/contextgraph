//! Unit tests for GraphEdge constructor methods.

use super::*;
use crate::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
use uuid::Uuid;

// =========================================================================
// new() Constructor Tests
// =========================================================================

#[test]
fn test_new_creates_edge_with_domain_nt_weights() {
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let edge = GraphEdge::new(source, target, EdgeType::Semantic, Domain::Code);

    let expected_nt = NeurotransmitterWeights::for_domain(Domain::Code);
    assert_eq!(edge.neurotransmitter_weights, expected_nt);
}

#[test]
fn test_new_uses_edge_type_default_weight() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Causal,
        Domain::General,
    );
    assert_eq!(edge.weight, EdgeType::Causal.default_weight());
    assert_eq!(edge.weight, 0.8);
}

#[test]
fn test_new_sets_confidence_to_half() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert_eq!(edge.confidence, 0.5);
}

#[test]
fn test_new_sets_steering_reward_to_zero() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert_eq!(edge.steering_reward, 0.0);
}

#[test]
fn test_new_sets_traversal_count_to_zero() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert_eq!(edge.traversal_count, 0);
}

#[test]
fn test_new_sets_is_amortized_shortcut_false() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert!(!edge.is_amortized_shortcut);
}

#[test]
fn test_new_sets_last_traversed_at_none() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert!(edge.last_traversed_at.is_none());
}

#[test]
fn test_new_generates_unique_id() {
    let edge1 = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    let edge2 = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    assert_ne!(edge1.id, edge2.id);
}

#[test]
fn test_new_all_edge_types() {
    for edge_type in EdgeType::all() {
        let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), edge_type, Domain::General);
        assert_eq!(edge.weight, edge_type.default_weight());
    }
}

#[test]
fn test_new_all_domains() {
    for domain in Domain::all() {
        let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), EdgeType::Semantic, domain);
        assert_eq!(edge.domain, domain);
        assert_eq!(
            edge.neurotransmitter_weights,
            NeurotransmitterWeights::for_domain(domain)
        );
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
        Domain::General,
        0.75,
        0.95,
    );
    assert_eq!(edge.weight, 0.75);
    assert_eq!(edge.confidence, 0.95);
}

#[test]
fn test_with_weight_clamps_weight_high() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
        1.5,
        0.5,
    );
    assert_eq!(edge.weight, 1.0);
}

#[test]
fn test_with_weight_clamps_weight_low() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
        -0.5,
        0.5,
    );
    assert_eq!(edge.weight, 0.0);
}

#[test]
fn test_with_weight_clamps_confidence_high() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
        0.5,
        1.5,
    );
    assert_eq!(edge.confidence, 1.0);
}

#[test]
fn test_with_weight_clamps_confidence_low() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
        0.5,
        -0.5,
    );
    assert_eq!(edge.confidence, 0.0);
}

#[test]
fn test_with_weight_preserves_source_target() {
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let edge = GraphEdge::with_weight(source, target, EdgeType::Causal, Domain::Code, 0.9, 0.85);
    assert_eq!(edge.source_id, source);
    assert_eq!(edge.target_id, target);
    assert_eq!(edge.edge_type, EdgeType::Causal);
    assert_eq!(edge.domain, Domain::Code);
}

#[test]
fn test_with_weight_boundary_values() {
    // Test exact boundary values
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
        0.0,
        1.0,
    );
    assert_eq!(edge.weight, 0.0);
    assert_eq!(edge.confidence, 1.0);
}

#[test]
fn test_with_weight_uses_domain_nt_weights() {
    let edge = GraphEdge::with_weight(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::Medical,
        0.6,
        0.7,
    );
    assert_eq!(
        edge.neurotransmitter_weights,
        NeurotransmitterWeights::for_domain(Domain::Medical)
    );
}
