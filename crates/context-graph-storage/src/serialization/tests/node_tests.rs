//! Tests for MemoryNode serialization.

use context_graph_core::types::{
    EmbeddingVector, JohariQuadrant, MemoryNode, NodeMetadata, DEFAULT_EMBEDDING_DIM,
    MAX_CONTENT_SIZE,
};
use serde_json::json;

use crate::serialization::{deserialize_node, serialize_node};

/// Create a valid normalized embedding vector.
/// Normalization ensures magnitude ~= 1.0 (validates per MemoryNode::validate).
fn create_normalized_embedding(dim: usize) -> EmbeddingVector {
    let val = 1.0 / (dim as f32).sqrt();
    vec![val; dim]
}

/// Create a valid MemoryNode with real data.
fn create_test_node() -> MemoryNode {
    let embedding = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);
    let mut node = MemoryNode::new("Test content for serialization".to_string(), embedding);
    node.importance = 0.75;
    node.emotional_valence = 0.5;
    node.quadrant = JohariQuadrant::Open;
    node.metadata = NodeMetadata::new()
        .with_source("test-source")
        .with_language("en");
    node
}

#[test]
fn test_node_roundtrip() {
    let node = create_test_node();
    let bytes = serialize_node(&node).expect("serialize failed");
    let restored = deserialize_node(&bytes).expect("deserialize failed");
    assert_eq!(node, restored, "Round-trip must preserve all fields");
}

#[test]
fn test_node_size_reasonable() {
    let node = create_test_node();
    let bytes = serialize_node(&node).unwrap();
    // 1536 * 4 = 6144 bytes for embedding alone
    // Total should be ~6.5-8KB with other fields
    // MessagePack with named format includes field names adding ~500 bytes overhead
    assert!(
        bytes.len() > 6000,
        "Node should be at least 6KB, got {}",
        bytes.len()
    );
    assert!(
        bytes.len() < 10000,
        "Node should be less than 10KB, got {}",
        bytes.len()
    );
}

#[test]
fn test_node_preserves_all_fields() {
    let mut node = create_test_node();
    node.importance = 0.999;
    node.emotional_valence = -0.555;
    node.access_count = 12345;
    node.quadrant = JohariQuadrant::Hidden;
    node.metadata.add_tag("important");
    node.metadata.add_tag("verified");
    node.metadata.set_custom("priority", json!(5));

    let bytes = serialize_node(&node).unwrap();
    let restored = deserialize_node(&bytes).unwrap();

    assert_eq!(node.id, restored.id);
    assert_eq!(node.content, restored.content);
    assert_eq!(node.embedding, restored.embedding);
    assert_eq!(node.quadrant, restored.quadrant);
    assert_eq!(node.importance, restored.importance);
    assert_eq!(node.emotional_valence, restored.emotional_valence);
    assert_eq!(node.created_at, restored.created_at);
    assert_eq!(node.accessed_at, restored.accessed_at);
    assert_eq!(node.access_count, restored.access_count);
    assert_eq!(node.metadata, restored.metadata);
}

#[test]
fn test_node_with_all_metadata() {
    let mut node = create_test_node();
    node.metadata.add_tag("important");
    node.metadata.add_tag("verified");
    node.metadata.set_custom("priority", json!(5));
    node.metadata.mark_consolidated();
    node.metadata.rationale = Some("Testing serialization".to_string());

    let bytes = serialize_node(&node).unwrap();
    let restored = deserialize_node(&bytes).unwrap();

    assert_eq!(node.metadata.tags, restored.metadata.tags);
    assert_eq!(
        node.metadata.get_custom("priority"),
        restored.metadata.get_custom("priority")
    );
    assert!(restored.metadata.consolidated);
    assert_eq!(node.metadata.rationale, restored.metadata.rationale);
}

#[test]
fn test_node_embedding_precision_preserved() {
    // Use specific float values that might have precision issues
    let mut embedding = Vec::with_capacity(1536);
    for i in 0..1536 {
        let value = (i as f32 / 1536.0) * std::f32::consts::PI;
        embedding.push(value);
    }

    let node = MemoryNode::new("Precision test".to_string(), embedding.clone());
    let bytes = serialize_node(&node).unwrap();
    let restored = deserialize_node(&bytes).unwrap();

    for (i, (orig, rest)) in embedding.iter().zip(restored.embedding.iter()).enumerate() {
        assert_eq!(
            orig.to_bits(),
            rest.to_bits(),
            "Embedding value at index {} differs: {} vs {}",
            i,
            orig,
            rest
        );
    }
}

#[test]
fn test_timestamps_preserved() {
    let node = create_test_node();
    let original_created = node.created_at;
    let original_accessed = node.accessed_at;

    let bytes = serialize_node(&node).unwrap();
    let restored = deserialize_node(&bytes).unwrap();

    assert_eq!(restored.created_at, original_created);
    assert_eq!(restored.accessed_at, original_accessed);
}

#[test]
fn edge_case_empty_content() {
    let mut node = create_test_node();
    node.content = String::new();

    println!("=== EDGE CASE 1: Empty Content ===");
    println!("BEFORE: content.len() = {}", node.content.len());

    let bytes = serialize_node(&node).unwrap();
    println!("SERIALIZED: bytes.len() = {}", bytes.len());

    let restored = deserialize_node(&bytes).unwrap();
    println!("AFTER: content.len() = {}", restored.content.len());

    assert_eq!(node.content, restored.content);
    println!("RESULT: PASS - Empty content preserved");
}

#[test]
fn edge_case_max_content() {
    let mut node = create_test_node();
    node.content = "x".repeat(MAX_CONTENT_SIZE);

    println!("=== EDGE CASE 2: Maximum Content Size ===");
    println!(
        "BEFORE: content.len() = {} (MAX_CONTENT_SIZE = {})",
        node.content.len(),
        MAX_CONTENT_SIZE
    );

    let bytes = serialize_node(&node).unwrap();
    println!(
        "SERIALIZED: bytes.len() = {} (~{:.2}MB)",
        bytes.len(),
        bytes.len() as f64 / 1_048_576.0
    );

    let restored = deserialize_node(&bytes).unwrap();
    println!("AFTER: content.len() = {}", restored.content.len());

    assert_eq!(node.content.len(), restored.content.len());
    println!("RESULT: PASS - Max content size preserved");
}

#[test]
fn edge_case_special_unicode_content() {
    let mut node = create_test_node();
    node.content = "Unicode: 日本語 emojis lambda alpha beta gamma delta epsilon zeta".to_string();

    println!("=== EDGE CASE: Special Unicode Content ===");
    println!("BEFORE: content = {:?}", node.content);
    println!("BEFORE: content.len() = {} bytes", node.content.len());

    let bytes = serialize_node(&node).unwrap();
    let restored = deserialize_node(&bytes).unwrap();

    println!("AFTER: content = {:?}", restored.content);
    println!("AFTER: content.len() = {} bytes", restored.content.len());

    assert_eq!(node.content, restored.content);
    println!("RESULT: PASS - Unicode content preserved");
}

#[test]
fn test_deserialization_invalid_bytes() {
    let garbage = vec![0xFF, 0x00, 0xAB, 0xCD];
    let result = deserialize_node(&garbage);
    assert!(result.is_err());
}

#[test]
fn test_deserialization_empty_bytes() {
    let empty: Vec<u8> = vec![];
    let result = deserialize_node(&empty);
    assert!(result.is_err());
}

#[test]
fn test_deserialization_truncated_node() {
    let node = create_test_node();
    let bytes = serialize_node(&node).unwrap();
    let truncated = &bytes[..bytes.len() / 2];
    let result = deserialize_node(truncated);
    assert!(result.is_err());
}

#[test]
fn test_node_default_can_serialize() {
    let node = MemoryNode::default();
    let bytes = serialize_node(&node).unwrap();
    let restored = deserialize_node(&bytes).unwrap();
    assert_eq!(node, restored);
}
