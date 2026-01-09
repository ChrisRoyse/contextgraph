//! Serialization tests for MemoryNode and NodeMetadata.
//!
//! TC-GHOST-006: Serialization Safety Tests
//! These tests verify JSON serialization round-trips preserve all data.

use super::*;

#[test]
fn test_memory_node_json_serialization_round_trip() {
    let embedding = vec![0.5; 1536];
    let mut node = MemoryNode::new("Test content for serialization".to_string(), embedding);
    node.importance = 0.85;
    node.access_count = 42;
    node.metadata.source = Some("test-source".to_string());
    node.metadata.language = Some("en".to_string());
    node.metadata.tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];
    node.metadata.utl_score = Some(0.75);
    node.metadata.consolidated = true;
    node.metadata.rationale = Some("Testing serialization round-trip".to_string());

    let json_str = serde_json::to_string(&node).expect("MemoryNode must serialize to JSON");
    let restored: MemoryNode =
        serde_json::from_str(&json_str).expect("MemoryNode must deserialize from JSON");

    assert_eq!(
        restored, node,
        "Deserialized node must match original exactly"
    );
}

#[test]
fn test_memory_node_complex_metadata_serialization() {
    let embedding = vec![0.1, 0.2, 0.3];
    let mut node = MemoryNode::new("Complex metadata test".to_string(), embedding);

    node.metadata.source = Some("conversation:abc123".to_string());
    node.metadata.language = Some("en-US".to_string());
    node.metadata.tags = vec![
        "important".to_string(),
        "technical".to_string(),
        "machine-learning".to_string(),
        "neural-networks".to_string(),
    ];
    node.metadata.utl_score = Some(0.9876543);
    node.metadata.consolidated = true;
    node.metadata.rationale =
        Some("This is a complex test case with special chars: @#$%^&*()".to_string());

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(
        restored.metadata.source,
        Some("conversation:abc123".to_string())
    );
    assert_eq!(restored.metadata.language, Some("en-US".to_string()));
    assert_eq!(restored.metadata.tags.len(), 4);
    assert!(restored
        .metadata
        .tags
        .contains(&"machine-learning".to_string()));
    assert_eq!(restored.metadata.utl_score, Some(0.9876543));
    assert!(restored.metadata.consolidated);
    assert!(restored
        .metadata
        .rationale
        .as_ref()
        .unwrap()
        .contains("special chars"));
}

#[test]
fn test_memory_node_embedding_precision_preserved() {
    let mut embedding = Vec::with_capacity(1536);
    for i in 0..1536 {
        let value = (i as f32 / 1536.0) * std::f32::consts::PI;
        embedding.push(value);
    }

    let node = MemoryNode::new("Precision test".to_string(), embedding.clone());
    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(restored.embedding.len(), 1536);
    for (i, (original, restored_val)) in embedding.iter().zip(restored.embedding.iter()).enumerate()
    {
        assert_eq!(
            original, restored_val,
            "Embedding value at index {} must be preserved: {} vs {}",
            i, original, restored_val
        );
    }
}

#[test]
fn test_memory_node_timestamps_preserved() {
    let embedding = vec![0.1; 10];
    let node = MemoryNode::new("Timestamp test".to_string(), embedding);
    let original_created_at = node.created_at;
    let original_accessed_at = node.accessed_at;

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(
        restored.created_at, original_created_at,
        "created_at must be preserved"
    );
    assert_eq!(
        restored.accessed_at, original_accessed_at,
        "accessed_at must be preserved"
    );
}

#[test]
fn test_memory_node_uuid_preserved() {
    let embedding = vec![0.1; 10];
    let node = MemoryNode::new("UUID test".to_string(), embedding);
    let original_id = node.id;

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(restored.id, original_id, "UUID must be exactly preserved");
}

#[test]
fn test_memory_node_optional_fields_none_serialization() {
    let embedding = vec![0.1; 10];
    let node = MemoryNode::new("Optional fields test".to_string(), embedding);

    assert!(node.metadata.source.is_none());
    assert!(node.metadata.language.is_none());
    assert!(node.metadata.utl_score.is_none());
    assert!(node.metadata.rationale.is_none());

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert!(
        restored.metadata.source.is_none(),
        "None source must remain None"
    );
    assert!(
        restored.metadata.language.is_none(),
        "None language must remain None"
    );
    assert!(
        restored.metadata.utl_score.is_none(),
        "None utl_score must remain None"
    );
    assert!(
        restored.metadata.rationale.is_none(),
        "None rationale must remain None"
    );
}

#[test]
fn test_node_metadata_serialization_isolated() {
    let metadata = NodeMetadata {
        source: Some("isolated-test".to_string()),
        tags: vec!["a".to_string(), "b".to_string()],
        utl_score: Some(0.5),
        ..NodeMetadata::default()
    };

    let json_str = serde_json::to_string(&metadata).unwrap();
    let restored: NodeMetadata = serde_json::from_str(&json_str).unwrap();

    assert_eq!(restored.source, Some("isolated-test".to_string()));
    assert_eq!(restored.tags, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(restored.utl_score, Some(0.5));
}

#[test]
fn test_memory_node_binary_json_equivalence() {
    let embedding = vec![0.5; 100];
    let mut node = MemoryNode::new("Binary equivalence test".to_string(), embedding);
    node.metadata.tags = vec!["test".to_string()];

    let compact_json = serde_json::to_string(&node).unwrap();
    let from_compact: MemoryNode = serde_json::from_str(&compact_json).unwrap();

    let pretty_json = serde_json::to_string_pretty(&node).unwrap();
    let from_pretty: MemoryNode = serde_json::from_str(&pretty_json).unwrap();

    assert_eq!(
        from_compact, from_pretty,
        "Compact and pretty JSON must deserialize identically"
    );
    assert_eq!(from_compact, node, "Both must match original");
}

#[test]
fn test_memory_node_special_content_serialization() {
    let special_content = r#"Content with "quotes", 'apostrophes', \backslashes\, and
newlines, plus unicode: 日本語 🎉 émojis"#;

    let embedding = vec![0.1; 10];
    let node = MemoryNode::new(special_content.to_string(), embedding);

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(
        restored.content, special_content,
        "Special characters must be preserved"
    );
}

#[test]
fn test_memory_node_extreme_values() {
    let mut embedding = vec![0.0; 10];
    embedding[0] = f32::MIN_POSITIVE;
    embedding[1] = f32::MAX;
    embedding[2] = f32::MIN;
    embedding[3] = 1e-38;
    embedding[4] = 1e38;

    let mut node = MemoryNode::new("Extreme values test".to_string(), embedding.clone());
    node.importance = 0.0;
    node.access_count = u64::MAX;

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(restored.importance, 0.0);
    assert_eq!(restored.access_count, u64::MAX);
    assert_eq!(restored.embedding[0], f32::MIN_POSITIVE);
}

#[test]
fn test_memory_node_serde_with_emotional_valence() {
    let embedding = vec![0.1; 1536];
    let mut node = MemoryNode::new("valence test".to_string(), embedding);

    // Test with negative valence
    node.emotional_valence = -0.75;
    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();
    assert_eq!(restored.emotional_valence, -0.75);

    // Test with positive valence
    node.emotional_valence = 0.95;
    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();
    assert_eq!(restored.emotional_valence, 0.95);
}

#[test]
fn test_node_metadata_serde_roundtrip() {
    use serde_json::json;

    let mut meta = NodeMetadata::new();
    meta.source = Some("test-source".to_string());
    meta.language = Some("en".to_string());
    meta.modality = crate::types::Modality::Code;
    meta.add_tag("test");
    meta.utl_score = Some(0.75);
    meta.mark_consolidated();
    meta.version = 5;
    meta.parent_id = Some(uuid::Uuid::new_v4());
    meta.child_ids.push(uuid::Uuid::new_v4());
    meta.set_custom("key", json!("value"));
    meta.rationale = Some("test rationale".to_string());

    let json_str = serde_json::to_string(&meta).expect("serialize failed");
    let restored: NodeMetadata = serde_json::from_str(&json_str).expect("deserialize failed");

    assert_eq!(
        meta, restored,
        "Round-trip serialization must preserve all fields"
    );
}

#[test]
fn test_node_metadata_serde_with_deleted() {
    let mut meta = NodeMetadata::new();
    meta.mark_deleted();

    let json_str = serde_json::to_string(&meta).expect("serialize failed");
    let restored: NodeMetadata = serde_json::from_str(&json_str).expect("deserialize failed");

    assert!(restored.deleted);
    assert!(restored.deleted_at.is_some());
    assert_eq!(meta.deleted_at, restored.deleted_at);
}

#[test]
fn test_memory_node_empty_content() {
    let embedding = vec![0.1; 10];
    let node = MemoryNode::new(String::new(), embedding);

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(restored.content, "");
}

#[test]
fn test_memory_node_max_valid_content() {
    let embedding = vec![0.1; 10];
    let content = "x".repeat(MAX_CONTENT_SIZE);
    let node = MemoryNode::new(content.clone(), embedding);

    let json_str = serde_json::to_string(&node).unwrap();
    let restored: MemoryNode = serde_json::from_str(&json_str).unwrap();

    assert_eq!(restored.content.len(), MAX_CONTENT_SIZE);
}
