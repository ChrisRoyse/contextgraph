//! Unit tests for GraphEdge struct fields, types, and serialization.

use super::*;
use crate::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
use crate::types::NodeId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// Helper function
pub(super) fn create_test_edge() -> GraphEdge {
    GraphEdge {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        target_id: Uuid::new_v4(),
        edge_type: EdgeType::Semantic,
        weight: 0.5,
        confidence: 0.8,
        domain: Domain::General,
        neurotransmitter_weights: NeurotransmitterWeights::default(),
        is_amortized_shortcut: false,
        steering_reward: 0.0,
        traversal_count: 0,
        created_at: Utc::now(),
        last_traversed_at: None,
    }
}

// Struct Field Existence Tests
#[test]
fn test_graph_edge_has_all_13_fields() {
    let edge = GraphEdge {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        target_id: Uuid::new_v4(),
        edge_type: EdgeType::Semantic,
        weight: 0.5,
        confidence: 0.8,
        domain: Domain::General,
        neurotransmitter_weights: NeurotransmitterWeights::default(),
        is_amortized_shortcut: false,
        steering_reward: 0.0,
        traversal_count: 0,
        created_at: Utc::now(),
        last_traversed_at: None,
    };
    // Verify all fields are accessible
    let _id: EdgeId = edge.id;
    let _src: NodeId = edge.source_id;
    let _tgt: NodeId = edge.target_id;
    let _et: EdgeType = edge.edge_type;
    let _w: f32 = edge.weight;
    let _c: f32 = edge.confidence;
    let _d: Domain = edge.domain;
    let _nt: NeurotransmitterWeights = edge.neurotransmitter_weights;
    let _short: bool = edge.is_amortized_shortcut;
    let _sr: f32 = edge.steering_reward;
    let _tc: u64 = edge.traversal_count;
    let _ca: DateTime<Utc> = edge.created_at;
    let _lt: Option<DateTime<Utc>> = edge.last_traversed_at;
}

#[test]
fn test_edge_id_is_uuid() {
    let edge_id: EdgeId = Uuid::new_v4();
    assert_eq!(edge_id.get_version_num(), 4);
}

// Field Type Tests
#[test]
fn test_source_id_is_node_id() {
    let source: NodeId = Uuid::new_v4();
    let edge = create_test_edge();
    let _: NodeId = edge.source_id;
    assert_ne!(source, edge.source_id);
}

#[test]
fn test_target_id_is_node_id() {
    let edge = create_test_edge();
    let _: NodeId = edge.target_id;
}

#[test]
fn test_edge_type_uses_marblestone_enum() {
    let edge = create_test_edge();
    let _weight = edge.edge_type.default_weight();
}

#[test]
fn test_domain_uses_marblestone_enum() {
    let edge = create_test_edge();
    let _desc = edge.domain.description();
}

#[test]
fn test_nt_weights_uses_marblestone_struct() {
    let edge = create_test_edge();
    let _eff = edge.neurotransmitter_weights.compute_effective_weight(0.5);
}

// Serde Serialization Tests
#[test]
fn test_serde_roundtrip() {
    let edge = create_test_edge();
    let json = serde_json::to_string(&edge).expect("serialize failed");
    let restored: GraphEdge = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(edge, restored);
}

#[test]
fn test_serde_json_contains_all_fields() {
    let edge = create_test_edge();
    let json = serde_json::to_string(&edge).unwrap();
    assert!(json.contains("\"id\"") && json.contains("\"source_id\""));
    assert!(json.contains("\"target_id\"") && json.contains("\"edge_type\""));
    assert!(json.contains("\"weight\"") && json.contains("\"confidence\""));
    assert!(json.contains("\"domain\"") && json.contains("\"neurotransmitter_weights\""));
    assert!(json.contains("\"is_amortized_shortcut\"") && json.contains("\"steering_reward\""));
    assert!(json.contains("\"traversal_count\"") && json.contains("\"created_at\""));
    assert!(json.contains("\"last_traversed_at\""));
}

#[test]
fn test_serde_with_last_traversed_at_some() {
    let mut edge = create_test_edge();
    edge.last_traversed_at = Some(Utc::now());
    let json = serde_json::to_string(&edge).unwrap();
    let restored: GraphEdge = serde_json::from_str(&json).unwrap();
    assert!(restored.last_traversed_at.is_some());
}

#[test]
fn test_serde_with_last_traversed_at_none() {
    let edge = create_test_edge();
    assert!(edge.last_traversed_at.is_none());
    let json = serde_json::to_string(&edge).unwrap();
    let restored: GraphEdge = serde_json::from_str(&json).unwrap();
    assert!(restored.last_traversed_at.is_none());
}

#[test]
fn test_serde_edge_type_snake_case() {
    let mut edge = create_test_edge();
    edge.edge_type = EdgeType::Hierarchical;
    let json = serde_json::to_string(&edge).unwrap();
    assert!(json.contains("\"hierarchical\""));
}

#[test]
fn test_serde_domain_snake_case() {
    let mut edge = create_test_edge();
    edge.domain = Domain::Medical;
    let json = serde_json::to_string(&edge).unwrap();
    assert!(json.contains("\"medical\""));
}

// Derive Trait Tests
#[test]
fn test_debug_format() {
    let edge = create_test_edge();
    let debug = format!("{:?}", edge);
    assert!(debug.contains("GraphEdge") && debug.contains("source_id"));
}

#[test]
fn test_clone() {
    let edge = create_test_edge();
    let cloned = edge.clone();
    assert_eq!(edge, cloned);
}

#[test]
fn test_partial_eq() {
    let edge1 = create_test_edge();
    let edge2 = edge1.clone();
    assert_eq!(edge1, edge2);
    let mut edge3 = edge1.clone();
    edge3.weight = 0.9;
    assert_ne!(edge1, edge3);
}

// Field Value Range Tests
#[test]
fn test_weight_boundaries() {
    let mut edge = create_test_edge();
    edge.weight = 0.0;
    assert_eq!(edge.weight, 0.0);
    edge.weight = 1.0;
    assert_eq!(edge.weight, 1.0);
}

#[test]
fn test_confidence_boundaries() {
    let mut edge = create_test_edge();
    edge.confidence = 0.0;
    assert_eq!(edge.confidence, 0.0);
    edge.confidence = 1.0;
    assert_eq!(edge.confidence, 1.0);
}

#[test]
fn test_steering_reward_boundaries() {
    let mut edge = create_test_edge();
    edge.steering_reward = -1.0;
    assert_eq!(edge.steering_reward, -1.0);
    edge.steering_reward = 1.0;
    assert_eq!(edge.steering_reward, 1.0);
}

#[test]
fn test_steering_reward_zero_is_neutral() {
    let edge = create_test_edge();
    assert_eq!(edge.steering_reward, 0.0);
}

#[test]
fn test_traversal_count_starts_at_zero() {
    let edge = create_test_edge();
    assert_eq!(edge.traversal_count, 0);
}

#[test]
fn test_is_amortized_shortcut_defaults_false() {
    let edge = create_test_edge();
    assert!(!edge.is_amortized_shortcut);
}

#[test]
fn test_is_amortized_shortcut_can_be_true() {
    let mut edge = create_test_edge();
    edge.is_amortized_shortcut = true;
    assert!(edge.is_amortized_shortcut);
}

// All EdgeType/Domain Variants Tests
#[test]
fn test_all_edge_types_work() {
    for edge_type in EdgeType::all() {
        let mut edge = create_test_edge();
        edge.edge_type = edge_type;
        let json = serde_json::to_string(&edge).unwrap();
        let restored: GraphEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.edge_type, edge_type);
    }
}

#[test]
fn test_all_domains_work() {
    for domain in Domain::all() {
        let mut edge = create_test_edge();
        edge.domain = domain;
        edge.neurotransmitter_weights = NeurotransmitterWeights::for_domain(domain);
        let json = serde_json::to_string(&edge).unwrap();
        let restored: GraphEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.domain, domain);
    }
}

// Timestamp and UUID Tests
#[test]
fn test_created_at_is_required() {
    let edge = create_test_edge();
    let _: DateTime<Utc> = edge.created_at;
}

#[test]
fn test_timestamps_preserved_through_serde() {
    let edge = create_test_edge();
    let original_created = edge.created_at;
    let json = serde_json::to_string(&edge).unwrap();
    let restored: GraphEdge = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.created_at, original_created);
}

#[test]
fn test_id_is_v4_uuid() {
    let edge = create_test_edge();
    assert_eq!(edge.id.get_version_num(), 4);
}

#[test]
fn test_source_and_target_are_different() {
    let edge = create_test_edge();
    assert_ne!(edge.source_id, edge.target_id);
}
