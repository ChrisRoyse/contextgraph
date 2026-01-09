//! Tests for GraphEdge serialization and deserialization.

use uuid::Uuid;

use crate::storage::edges::{Domain, EdgeType, GraphEdge};

#[test]
fn test_bincode_roundtrip() {
    let edge = GraphEdge::new(
        42,
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Causal,
        0.75,
        Domain::Medical,
    );

    let serialized = bincode::serialize(&edge).expect("serialize failed");
    let deserialized: GraphEdge = bincode::deserialize(&serialized).expect("deserialize failed");

    assert_eq!(edge.id, deserialized.id);
    assert_eq!(edge.source, deserialized.source);
    assert_eq!(edge.target, deserialized.target);
    assert_eq!(edge.edge_type, deserialized.edge_type);
    assert!((edge.weight - deserialized.weight).abs() < 1e-6);
    assert_eq!(edge.domain, deserialized.domain);
}

#[test]
fn test_json_roundtrip() {
    let edge = GraphEdge::new(
        1,
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Temporal,
        0.6,
        Domain::Research,
    );

    let json = serde_json::to_string(&edge).expect("json serialize failed");
    let deserialized: GraphEdge = serde_json::from_str(&json).expect("json deserialize failed");

    assert_eq!(edge.id, deserialized.id);
    assert_eq!(edge.edge_type, deserialized.edge_type);
}

#[test]
fn test_has_13_fields() {
    // This test verifies the struct has all 13 fields by checking serialized JSON
    let edge = GraphEdge::default();
    let json: serde_json::Value = serde_json::to_value(&edge).expect("to_value failed");
    let obj = json.as_object().expect("should be object");

    // The 13 expected fields
    let expected_fields = [
        "id",
        "source",
        "target",
        "edge_type",
        "weight",
        "confidence",
        "domain",
        "neurotransmitter_weights",
        "is_amortized_shortcut",
        "steering_reward",
        "traversal_count",
        "created_at",
        "last_traversed_at",
    ];

    assert_eq!(
        obj.len(),
        13,
        "GraphEdge should have exactly 13 fields, found {}",
        obj.len()
    );

    for field in expected_fields {
        assert!(obj.contains_key(field), "Missing field: {}", field);
    }
}
