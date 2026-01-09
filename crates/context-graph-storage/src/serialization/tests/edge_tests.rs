//! Tests for GraphEdge serialization.

use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
use context_graph_core::types::GraphEdge;
use uuid::Uuid;

use crate::serialization::{deserialize_edge, serialize_edge};

/// Create a valid GraphEdge with real data.
fn create_test_edge() -> GraphEdge {
    GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::Code,
    )
}

#[test]
fn test_edge_roundtrip() {
    let edge = create_test_edge();
    let bytes = serialize_edge(&edge).expect("serialize failed");
    let restored = deserialize_edge(&bytes).expect("deserialize failed");
    assert_eq!(edge, restored, "Round-trip must preserve all fields");
}

#[test]
fn test_edge_size_reasonable() {
    let edge = create_test_edge();
    let bytes = serialize_edge(&edge).unwrap();
    assert!(
        bytes.len() > 100,
        "Edge should be at least 100 bytes, got {}",
        bytes.len()
    );
    assert!(
        bytes.len() < 500,
        "Edge should be less than 500 bytes, got {}",
        bytes.len()
    );
}

#[test]
fn test_edge_preserves_all_13_fields() {
    let mut edge = create_test_edge();
    edge.weight = 0.85;
    edge.confidence = 0.95;
    edge.is_amortized_shortcut = true;
    edge.steering_reward = 0.75;
    edge.traversal_count = 42;
    edge.neurotransmitter_weights = NeurotransmitterWeights::for_domain(Domain::Medical);
    edge.record_traversal();

    let bytes = serialize_edge(&edge).unwrap();
    let restored = deserialize_edge(&bytes).unwrap();

    assert_eq!(edge.id, restored.id);
    assert_eq!(edge.source_id, restored.source_id);
    assert_eq!(edge.target_id, restored.target_id);
    assert_eq!(edge.edge_type, restored.edge_type);
    assert_eq!(edge.weight, restored.weight);
    assert_eq!(edge.confidence, restored.confidence);
    assert_eq!(edge.domain, restored.domain);
    assert_eq!(
        edge.neurotransmitter_weights,
        restored.neurotransmitter_weights
    );
    assert_eq!(edge.is_amortized_shortcut, restored.is_amortized_shortcut);
    assert_eq!(edge.steering_reward, restored.steering_reward);
    assert_eq!(edge.traversal_count, restored.traversal_count);
    assert_eq!(edge.created_at, restored.created_at);
    assert!(restored.last_traversed_at.is_some());
}

#[test]
fn test_edge_with_all_marblestone_fields() {
    let mut edge = create_test_edge();
    edge.is_amortized_shortcut = true;
    edge.steering_reward = 0.75;
    edge.traversal_count = 42;
    edge.confidence = 0.9;
    edge.neurotransmitter_weights = NeurotransmitterWeights::for_domain(Domain::Medical);
    edge.record_traversal();

    let bytes = serialize_edge(&edge).unwrap();
    let restored = deserialize_edge(&bytes).unwrap();

    assert_eq!(edge.is_amortized_shortcut, restored.is_amortized_shortcut);
    assert_eq!(edge.steering_reward, restored.steering_reward);
    assert_eq!(edge.traversal_count, restored.traversal_count);
    assert_eq!(
        edge.neurotransmitter_weights,
        restored.neurotransmitter_weights
    );
    assert!(restored.last_traversed_at.is_some());
}

#[test]
fn test_edge_all_types() {
    for edge_type in EdgeType::all() {
        for domain in Domain::all() {
            let edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), edge_type, domain);
            let bytes = serialize_edge(&edge).unwrap();
            let restored = deserialize_edge(&bytes).unwrap();
            assert_eq!(edge, restored, "Failed for {:?} / {:?}", edge_type, domain);
        }
    }
}

#[test]
fn test_edge_timestamps_preserved() {
    let mut edge = create_test_edge();
    edge.record_traversal();
    let original_created = edge.created_at;
    let original_traversed = edge.last_traversed_at;

    let bytes = serialize_edge(&edge).unwrap();
    let restored = deserialize_edge(&bytes).unwrap();

    assert_eq!(restored.created_at, original_created);
    assert_eq!(restored.last_traversed_at, original_traversed);
}

#[test]
fn edge_case_boundary_values() {
    let mut edge = create_test_edge();
    edge.weight = 0.0;
    edge.confidence = 1.0;
    edge.steering_reward = -1.0;
    edge.traversal_count = u64::MAX;

    println!("=== EDGE CASE: Boundary Values ===");
    println!(
        "BEFORE: weight={}, confidence={}, steering_reward={}, traversal_count={}",
        edge.weight, edge.confidence, edge.steering_reward, edge.traversal_count
    );

    let bytes = serialize_edge(&edge).unwrap();
    let restored = deserialize_edge(&bytes).unwrap();

    println!(
        "AFTER: weight={}, confidence={}, steering_reward={}, traversal_count={}",
        restored.weight, restored.confidence, restored.steering_reward, restored.traversal_count
    );

    assert_eq!(edge.weight, restored.weight);
    assert_eq!(edge.confidence, restored.confidence);
    assert_eq!(edge.steering_reward, restored.steering_reward);
    assert_eq!(edge.traversal_count, restored.traversal_count);
    println!("RESULT: PASS - All boundary values preserved");
}

#[test]
fn test_deserialization_invalid_bytes() {
    let garbage = vec![0xFF, 0x00, 0xAB, 0xCD];
    let result = deserialize_edge(&garbage);
    assert!(result.is_err());
}

#[test]
fn test_deserialization_empty_bytes() {
    let empty: Vec<u8> = vec![];
    let result = deserialize_edge(&empty);
    assert!(result.is_err());
}

#[test]
fn test_deserialization_truncated_edge() {
    let edge = create_test_edge();
    let bytes = serialize_edge(&edge).unwrap();
    let truncated = &bytes[..bytes.len() / 2];
    let result = deserialize_edge(truncated);
    assert!(result.is_err());
}

#[test]
fn test_edge_default_can_serialize() {
    let edge = GraphEdge::default();
    let bytes = serialize_edge(&edge).unwrap();
    let restored = deserialize_edge(&bytes).unwrap();
    assert_eq!(edge, restored);
}
