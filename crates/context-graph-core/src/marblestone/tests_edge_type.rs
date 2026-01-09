//! Tests for EdgeType enum.

use super::edge_type::EdgeType;

#[test]
fn test_edge_type_default_is_semantic() {
    assert_eq!(EdgeType::default(), EdgeType::Semantic);
}

#[test]
fn test_edge_type_description_non_empty() {
    for edge_type in EdgeType::all() {
        let desc = edge_type.description();
        assert!(!desc.is_empty(), "Description for {:?} is empty", edge_type);
        assert!(desc.len() > 10, "Description for {:?} too short", edge_type);
    }
}

#[test]
fn test_edge_type_semantic_description() {
    assert!(EdgeType::Semantic
        .description()
        .to_lowercase()
        .contains("similar"));
}

#[test]
fn test_edge_type_temporal_description() {
    assert!(EdgeType::Temporal
        .description()
        .to_lowercase()
        .contains("time"));
}

#[test]
fn test_edge_type_causal_description() {
    assert!(EdgeType::Causal
        .description()
        .to_lowercase()
        .contains("cause"));
}

#[test]
fn test_edge_type_hierarchical_description() {
    assert!(EdgeType::Hierarchical
        .description()
        .to_lowercase()
        .contains("parent"));
}

#[test]
fn test_edge_type_all_returns_5_variants() {
    assert_eq!(EdgeType::all().len(), 5);
}

#[test]
fn test_edge_type_all_contains_all_variants() {
    let all = EdgeType::all();
    assert!(all.contains(&EdgeType::Semantic));
    assert!(all.contains(&EdgeType::Temporal));
    assert!(all.contains(&EdgeType::Causal));
    assert!(all.contains(&EdgeType::Hierarchical));
    assert!(all.contains(&EdgeType::Contradicts));
}

#[test]
fn test_edge_type_all_order() {
    let all = EdgeType::all();
    assert_eq!(all[0], EdgeType::Semantic);
    assert_eq!(all[1], EdgeType::Temporal);
    assert_eq!(all[2], EdgeType::Causal);
    assert_eq!(all[3], EdgeType::Hierarchical);
    assert_eq!(all[4], EdgeType::Contradicts);
}

#[test]
fn test_edge_type_default_weight_semantic() {
    assert!((EdgeType::Semantic.default_weight() - 0.5).abs() < 0.001);
}

#[test]
fn test_edge_type_default_weight_temporal() {
    assert!((EdgeType::Temporal.default_weight() - 0.7).abs() < 0.001);
}

#[test]
fn test_edge_type_default_weight_causal() {
    assert!((EdgeType::Causal.default_weight() - 0.8).abs() < 0.001);
}

#[test]
fn test_edge_type_default_weight_hierarchical() {
    assert!((EdgeType::Hierarchical.default_weight() - 0.9).abs() < 0.001);
}

#[test]
fn test_edge_type_weights_in_valid_range() {
    for edge_type in EdgeType::all() {
        let weight = edge_type.default_weight();
        assert!(
            (0.0..=1.0).contains(&weight),
            "Weight {} for {:?} out of range",
            weight,
            edge_type
        );
    }
}

#[test]
fn test_edge_type_weights_increasing_strength() {
    assert!(EdgeType::Hierarchical.default_weight() > EdgeType::Causal.default_weight());
    assert!(EdgeType::Causal.default_weight() > EdgeType::Temporal.default_weight());
    assert!(EdgeType::Temporal.default_weight() > EdgeType::Semantic.default_weight());
}

#[test]
fn test_edge_type_display_semantic() {
    assert_eq!(EdgeType::Semantic.to_string(), "semantic");
}

#[test]
fn test_edge_type_display_temporal() {
    assert_eq!(EdgeType::Temporal.to_string(), "temporal");
}

#[test]
fn test_edge_type_display_causal() {
    assert_eq!(EdgeType::Causal.to_string(), "causal");
}

#[test]
fn test_edge_type_display_hierarchical() {
    assert_eq!(EdgeType::Hierarchical.to_string(), "hierarchical");
}

#[test]
fn test_edge_type_display_all_lowercase() {
    for edge_type in EdgeType::all() {
        let s = edge_type.to_string();
        assert_eq!(
            s,
            s.to_lowercase(),
            "Display for {:?} not lowercase",
            edge_type
        );
    }
}

#[test]
fn test_edge_type_serde_snake_case() {
    let edge = EdgeType::Semantic;
    let json = serde_json::to_string(&edge).unwrap();
    assert_eq!(json, r#""semantic""#);
}

#[test]
fn test_edge_type_serde_roundtrip() {
    for edge_type in EdgeType::all() {
        let json = serde_json::to_string(&edge_type).unwrap();
        let restored: EdgeType = serde_json::from_str(&json).unwrap();
        assert_eq!(edge_type, restored, "Roundtrip failed for {:?}", edge_type);
    }
}

#[test]
fn test_edge_type_serde_deserialize() {
    let edge: EdgeType = serde_json::from_str(r#""causal""#).unwrap();
    assert_eq!(edge, EdgeType::Causal);
}

#[test]
fn test_edge_type_serde_invalid_variant_fails() {
    let result: Result<EdgeType, _> = serde_json::from_str(r#""invalid""#);
    assert!(
        result.is_err(),
        "Invalid variant should fail deserialization"
    );
}

#[test]
fn test_edge_type_clone() {
    let edge = EdgeType::Temporal;
    let cloned = edge;
    assert_eq!(edge, cloned);
}

#[test]
fn test_edge_type_copy() {
    let edge = EdgeType::Causal;
    let copied = edge;
    assert_eq!(edge, copied);
    let _still_valid = edge;
}

#[test]
fn test_edge_type_debug() {
    let debug = format!("{:?}", EdgeType::Hierarchical);
    assert!(debug.contains("Hierarchical"));
}

#[test]
fn test_edge_type_partial_eq() {
    assert_eq!(EdgeType::Semantic, EdgeType::Semantic);
    assert_ne!(EdgeType::Semantic, EdgeType::Temporal);
}

#[test]
fn test_edge_type_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(EdgeType::Semantic);
    set.insert(EdgeType::Temporal);
    set.insert(EdgeType::Semantic);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_edge_type_all_unique() {
    use std::collections::HashSet;
    let all = EdgeType::all();
    let unique: HashSet<_> = all.iter().collect();
    assert_eq!(unique.len(), 5);
}

// ========== M04-T26: Contradicts Tests ==========

#[test]
fn test_edge_type_contradicts_description() {
    assert!(EdgeType::Contradicts
        .description()
        .to_lowercase()
        .contains("contradict"));
}

#[test]
fn test_edge_type_contradicts_weight() {
    assert!((EdgeType::Contradicts.default_weight() - 0.3).abs() < 0.001);
}

#[test]
fn test_edge_type_contradicts_display() {
    assert_eq!(EdgeType::Contradicts.to_string(), "contradicts");
}

#[test]
fn test_edge_type_contradicts_serde() {
    let edge: EdgeType = serde_json::from_str(r#""contradicts""#).unwrap();
    assert_eq!(edge, EdgeType::Contradicts);
}

#[test]
fn test_edge_type_is_contradiction() {
    assert!(EdgeType::Contradicts.is_contradiction());
    assert!(!EdgeType::Semantic.is_contradiction());
}

#[test]
fn test_edge_type_is_symmetric() {
    // Symmetric edges
    assert!(EdgeType::Semantic.is_symmetric());
    assert!(EdgeType::Contradicts.is_symmetric());
    // Directed edges
    assert!(!EdgeType::Temporal.is_symmetric());
    assert!(!EdgeType::Causal.is_symmetric());
    assert!(!EdgeType::Hierarchical.is_symmetric());
}

#[test]
fn test_edge_type_default_in_all() {
    assert!(EdgeType::all().contains(&EdgeType::default()));
}
