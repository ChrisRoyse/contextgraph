//! Unit tests for MemoryNode struct.
//!
//! Tests cover:
//! - Node creation and initialization
//! - Access tracking and timestamps
//! - Decay computation and consolidation
//! - Validation logic
//! - Field constraints and edge cases

use super::*;

#[test]
fn test_memory_node_creation() {
    let embedding = vec![0.1; 1536];
    let node = MemoryNode::new("test content".to_string(), embedding.clone());

    assert_eq!(node.content, "test content");
    assert_eq!(node.embedding.len(), 1536);
    assert_eq!(node.importance, 0.5);
    assert_eq!(node.access_count, 0);
    assert!(!node.metadata.deleted);
}

#[test]
fn test_memory_node_with_id() {
    let id = uuid::Uuid::new_v4();
    let embedding = vec![0.1; 1536];
    let node = MemoryNode::with_id(id, "test".to_string(), embedding);
    assert_eq!(node.id, id);
}

#[test]
fn test_record_access() {
    let embedding = vec![0.1; 1536];
    let mut node = MemoryNode::new("test".to_string(), embedding);
    let initial_accessed = node.accessed_at;

    std::thread::sleep(std::time::Duration::from_millis(10));
    node.record_access();

    assert_eq!(node.access_count, 1);
    assert!(node.accessed_at > initial_accessed);
}

#[test]
fn test_record_access_saturating() {
    let embedding = vec![0.1; 1536];
    let mut node = MemoryNode::new("test".to_string(), embedding);
    node.access_count = u64::MAX;
    node.record_access();
    assert_eq!(node.access_count, u64::MAX);
}

#[test]
fn test_compute_decay_in_valid_range() {
    let node = MemoryNode::new("test".to_string(), vec![0.0; 1536]);
    let decay = node.compute_decay();
    assert!(
        (0.0..=1.0).contains(&decay),
        "Decay {} must be in [0,1]",
        decay
    );
}

#[test]
fn test_compute_decay_high_access_count() {
    let mut node = MemoryNode::new("test".to_string(), vec![0.0; 1536]);
    node.access_count = 1000;
    let decay = node.compute_decay();
    assert!((0.0..=1.0).contains(&decay));
}

#[test]
fn test_age_seconds_non_negative() {
    let node = MemoryNode::new("test".to_string(), vec![0.0; 1536]);
    assert!(node.age_seconds() >= 0);
}

#[test]
fn test_time_since_access_non_negative() {
    let node = MemoryNode::new("test".to_string(), vec![0.0; 1536]);
    assert!(node.time_since_access_seconds() >= 0);
}

#[test]
fn test_validate_valid_node() {
    let dim = DEFAULT_EMBEDDING_DIM;
    let val = 1.0 / (dim as f32).sqrt();
    let embedding: Vec<f32> = vec![val; dim];

    let node = MemoryNode::new("valid content".to_string(), embedding);
    assert!(node.validate().is_ok());
}

#[test]
fn test_validate_wrong_embedding_dimension() {
    let node = MemoryNode::new("test".to_string(), vec![0.1; 512]);
    let result = node.validate();
    assert!(matches!(
        result,
        Err(ValidationError::InvalidEmbeddingDimension { .. })
    ));
}

#[test]
fn test_validate_importance_out_of_range() {
    let dim = DEFAULT_EMBEDDING_DIM;
    let val = 1.0 / (dim as f32).sqrt();
    let mut node = MemoryNode::new("test".to_string(), vec![val; dim]);
    node.importance = 1.5;

    let result = node.validate();
    assert!(matches!(result, Err(ValidationError::OutOfBounds { .. })));
}

#[test]
fn test_validate_valence_out_of_range() {
    let dim = DEFAULT_EMBEDDING_DIM;
    let val = 1.0 / (dim as f32).sqrt();
    let mut node = MemoryNode::new("test".to_string(), vec![val; dim]);
    node.emotional_valence = -1.5;

    let result = node.validate();
    assert!(matches!(result, Err(ValidationError::OutOfBounds { .. })));
}

#[test]
fn test_validate_content_too_large() {
    let dim = DEFAULT_EMBEDDING_DIM;
    let val = 1.0 / (dim as f32).sqrt();
    let big_content = "x".repeat(MAX_CONTENT_SIZE + 1);
    let node = MemoryNode::new(big_content, vec![val; dim]);

    let result = node.validate();
    assert!(matches!(
        result,
        Err(ValidationError::ContentTooLarge { .. })
    ));
}

#[test]
fn test_validate_embedding_not_normalized() {
    let dim = DEFAULT_EMBEDDING_DIM;
    let node = MemoryNode::new("test".to_string(), vec![0.5; dim]);

    let result = node.validate();
    assert!(matches!(
        result,
        Err(ValidationError::EmbeddingNotNormalized { .. })
    ));
}

#[test]
fn test_default_embedding_fails_validation() {
    let node = MemoryNode::default();
    assert!(node.validate().is_err());
}

#[test]
fn test_memory_node_has_all_required_fields() {
    let embedding = vec![0.1; 1536];
    let node = MemoryNode::new("content".to_string(), embedding);

    // Verify all fields exist and have expected types
    let _id: NodeId = node.id;
    let _content: &String = &node.content;
    let _embedding: &EmbeddingVector = &node.embedding;
    let _quadrant: &crate::types::JohariQuadrant = &node.quadrant;
    let _importance: f32 = node.importance;
    let _valence: f32 = node.emotional_valence;
    let _created: chrono::DateTime<chrono::Utc> = node.created_at;
    let _accessed: chrono::DateTime<chrono::Utc> = node.accessed_at;
    let _count: u64 = node.access_count;
    let _meta: &NodeMetadata = &node.metadata;
}

#[test]
fn test_memory_node_new_defaults() {
    let embedding = vec![0.1; 1536];
    let node = MemoryNode::new("test".to_string(), embedding);

    assert_eq!(node.importance, 0.5);
    assert_eq!(node.emotional_valence, 0.0);
    assert_eq!(node.access_count, 0);
    assert_eq!(node.quadrant, crate::types::JohariQuadrant::default());
}

#[test]
fn test_memory_node_emotional_valence_range() {
    let embedding = vec![0.1; 1536];
    let mut node = MemoryNode::new("test".to_string(), embedding);

    // Test valid negative valence
    node.emotional_valence = -1.0;
    assert_eq!(node.emotional_valence, -1.0);

    // Test valid positive valence
    node.emotional_valence = 1.0;
    assert_eq!(node.emotional_valence, 1.0);

    // Test neutral
    node.emotional_valence = 0.0;
    assert_eq!(node.emotional_valence, 0.0);
}

#[test]
fn test_memory_node_quadrant_field() {
    let embedding = vec![0.1; 1536];
    let mut node = MemoryNode::new("test".to_string(), embedding);

    node.quadrant = crate::types::JohariQuadrant::Blind;
    assert_eq!(node.quadrant, crate::types::JohariQuadrant::Blind);
}

#[test]
fn test_memory_node_modality_via_metadata() {
    let embedding = vec![0.1; 1536];
    let mut node = MemoryNode::new("test".to_string(), embedding);

    node.metadata.modality = crate::types::Modality::Code;
    assert_eq!(node.metadata.modality, crate::types::Modality::Code);
}

#[test]
fn test_memory_node_clone() {
    let embedding = vec![0.1; 1536];
    let original = MemoryNode::new("test".to_string(), embedding);
    let cloned = original.clone();

    assert_eq!(original, cloned);
}

#[test]
fn test_memory_node_partial_eq() {
    let embedding = vec![0.1; 1536];
    let node1 = MemoryNode::with_id(uuid::Uuid::nil(), "test".to_string(), embedding.clone());
    let node2 = MemoryNode::with_id(uuid::Uuid::nil(), "test".to_string(), embedding);

    // Nodes with different timestamps won't be equal, but same ID
    assert_eq!(node1.id, node2.id);
}
